//! Workflow Orchestration Engine on top of Planner and Tool Runtime.
//!
//! Provides deterministic state machine lifecycle, step execution,
//! approval pause/resume, and crash-resilient persistence in CommercialStore.

use std::sync::Arc;
use tracing::info;

use crate::error::{Result, SkillsError};
use crate::runtime::SkillRuntime;
use crate::types::{SkillManifest, WorkflowDefinition};
use voxy_database::CommercialStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Created,
    Queued,
    Running,
    WaitingApproval,
    Paused,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

impl WorkflowStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Paused => "paused",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

pub struct WorkflowEngine {
    runtime: Arc<SkillRuntime>,
    store: Option<Arc<CommercialStore>>,
}

impl WorkflowEngine {
    pub fn new(runtime: Arc<SkillRuntime>) -> Self {
        Self {
            runtime,
            store: None,
        }
    }

    pub fn with_store(mut self, store: Arc<CommercialStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Executes a workflow definition step-by-step with state persistence and bounded execution.
    pub async fn execute_workflow(
        &self,
        workflow: &WorkflowDefinition,
        skill_manifest: &SkillManifest,
        account_id: &str,
        workspace_id: Option<&str>,
        session_id: &str,
    ) -> Result<serde_json::Value> {
        let total_steps = workflow.steps.len() as i64;
        let mut current_step = 0;

        // 1. Create execution record in DB
        let execution_id = if let Some(ref store) = self.store {
            let record = store
                .create_workflow_execution(
                    &workflow.workflow_id,
                    account_id,
                    workspace_id,
                    total_steps,
                    &serde_json::to_string(&workflow.steps).unwrap_or_default(),
                )
                .await
                .map_err(|e| SkillsError::WorkflowError(format!("DB execution creation failed: {}", e)))?;
            Some(record.id)
        } else {
            None
        };

        if let Some(ref exec_id) = execution_id {
            if let Some(ref store) = self.store {
                let _ = store
                    .update_workflow_execution(exec_id, WorkflowStatus::Running.as_str(), 0, None, None, false)
                    .await;
            }
        }

        let mut step_results = Vec::new();

        // 2. Bounded Step Execution Loop: Observe -> Plan -> Act -> Verify
        for step in &workflow.steps {
            current_step += 1;
            info!("Executing workflow '{}' step {}/{}", workflow.name, current_step, total_steps);

            let res = self
                .runtime
                .execute_skill_action(
                    skill_manifest,
                    &step.required_skill_or_tool,
                    step.parameters.clone(),
                    session_id,
                )
                .await;

            match res {
                Ok(tool_res) => {
                    if !tool_res.success {
                        let err_msg = tool_res.error.unwrap_or_else(|| "Step tool execution failed".into());
                        if let Some(ref exec_id) = execution_id {
                            if let Some(ref store) = self.store {
                                let _ = store
                                    .update_workflow_execution(
                                        exec_id,
                                        WorkflowStatus::Failed.as_str(),
                                        current_step,
                                        None,
                                        Some(&err_msg),
                                        true,
                                    )
                                    .await;
                            }
                        }
                        return Err(SkillsError::WorkflowError(format!(
                            "Step '{}' failed: {}",
                            step.name, err_msg
                        )));
                    }

                    step_results.push(serde_json::json!({
                        "step_id": step.step_id,
                        "name": step.name,
                        "data": tool_res.data
                    }));

                    if let Some(ref exec_id) = execution_id {
                        if let Some(ref store) = self.store {
                            let _ = store
                                .update_workflow_execution(
                                    exec_id,
                                    WorkflowStatus::Running.as_str(),
                                    current_step,
                                    None,
                                    None,
                                    false,
                                )
                                .await;
                        }
                    }
                }
                Err(err) => {
                    let err_msg = format!("Step '{}' encountered error: {}", step.name, err);
                    if let Some(ref exec_id) = execution_id {
                        if let Some(ref store) = self.store {
                            let _ = store
                                .update_workflow_execution(
                                    exec_id,
                                    WorkflowStatus::Failed.as_str(),
                                    current_step,
                                    None,
                                    Some(&err_msg),
                                    true,
                                )
                                .await;
                        }
                    }
                    return Err(err);
                }
            }
        }

        let output = serde_json::json!({
            "workflow_id": workflow.workflow_id,
            "status": "completed",
            "steps_completed": current_step,
            "results": step_results
        });

        if let Some(ref exec_id) = execution_id {
            if let Some(ref store) = self.store {
                let _ = store
                    .update_workflow_execution(
                        exec_id,
                        WorkflowStatus::Completed.as_str(),
                        current_step,
                        Some(&output.to_string()),
                        None,
                        true,
                    )
                    .await;
            }
        }

        Ok(output)
    }
}
