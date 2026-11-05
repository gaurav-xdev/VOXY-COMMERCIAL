use crate::a2a::{A2AMessage, A2AMessageBus, A2AMessageType};
use crate::blackboard::{HandoffEnvelope, SwarmBlackboard};
use crate::error::{Result, SwarmError};
use crate::provider_bridge::SwarmProviderBridge;
use crate::security::SwarmSecurityBoundary;
use crate::task_graph::TaskGraph;
use crate::types::{
    AgentId, AgentRole, SwarmExecutionReceipt, SwarmPlan, SwarmTask, TaskPriority, TaskResult,
    TaskStatus,
};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use uuid::Uuid;

/// Swarm Coordinator integrating Trinity A2A communication
/// and SwarmClaw dynamic team delegation and parallel execution.
pub struct SwarmCoordinator {
    bridge: SwarmProviderBridge,
    security: SwarmSecurityBoundary,
    message_bus: A2AMessageBus,
    blackboard: SwarmBlackboard,
    max_concurrency: usize,
    workspace_root: PathBuf,
}

impl SwarmCoordinator {
    pub fn new(
        bridge: SwarmProviderBridge,
        security: SwarmSecurityBoundary,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            bridge,
            security,
            message_bus: A2AMessageBus::default(),
            blackboard: SwarmBlackboard::default(),
            max_concurrency: 4,
            workspace_root: workspace_root.into(),
        }
    }

    pub fn with_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = max_concurrency.max(1);
        self
    }

    pub fn security(&self) -> &SwarmSecurityBoundary {
        &self.security
    }

    pub fn message_bus(&self) -> &A2AMessageBus {
        &self.message_bus
    }

    pub fn blackboard(&self) -> &SwarmBlackboard {
        &self.blackboard
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    /// Autonomous decomposition: Supervisor analyzes the high-level mission
    /// and breaks it down into a Trinity DAG of specialized tasks.
    pub async fn plan_mission(
        &self,
        mission_id: &str,
        goal: &str,
        caller: &str,
        token: &str,
    ) -> Result<SwarmPlan> {
        self.security.verify_caller(caller, token)?;

        let supervisor_prompt = format!(
            "Decompose the following engineering objective into a structured task breakdown with dependencies.\nGoal: {goal}\n\nAvailable Roles:\n- architect: System design & schema\n- specialist_worker: Code implementation\n- reviewer: Quality & security review\n- qa_tester: Test verification & canary check\n\nGenerate tasks in order of execution."
        );

        let plan_raw = self
            .bridge
            .execute_agent_prompt(AgentRole::Supervisor, &supervisor_prompt, "")
            .await?;

        // Construct standard Trinity DAG for the mission
        let mut tasks = Vec::new();

        // 1. Architect Task
        let task_arch = SwarmTask::new(
            format!("{mission_id}-t1-architect"),
            "Architectural & Contract Specification",
            format!("Design architecture, interfaces, and validation criteria for: {goal}"),
            AgentRole::Architect,
        )
        .with_priority(TaskPriority::High);
        tasks.push(task_arch);

        // 2. Implementation Worker Task (depends on Architect)
        let task_worker = SwarmTask::new(
            format!("{mission_id}-t2-implementation"),
            "Specialist Code Implementation",
            format!("Implement code changes adhering to architectural spec for: {goal}"),
            AgentRole::SpecialistWorker,
        )
        .with_dependency(format!("{mission_id}-t1-architect"))
        .with_priority(TaskPriority::Critical);
        tasks.push(task_worker);

        // 3. Reviewer Task (depends on Implementation)
        let task_review = SwarmTask::new(
            format!("{mission_id}-t3-review"),
            "Kodus & Security Review",
            format!("Review code changes, safety invariants, and vulnerability checks for: {goal}"),
            AgentRole::Reviewer,
        )
        .with_dependency(format!("{mission_id}-t2-implementation"))
        .with_priority(TaskPriority::High);
        tasks.push(task_review);

        // 4. QA & Canary Task (depends on Review)
        let task_qa = SwarmTask::new(
            format!("{mission_id}-t4-qa-canary"),
            "Canary Validation & Regression Testing",
            format!("Execute test suites and verify acceptance criteria for: {goal}"),
            AgentRole::QaTester,
        )
        .with_dependency(format!("{mission_id}-t3-review"))
        .with_priority(TaskPriority::Critical)
        .with_canary("verification_suite");
        tasks.push(task_qa);

        // Notify Trinity A2A Message Bus
        self.message_bus.publish(A2AMessage::new(
            mission_id,
            AgentId("trinity-supervisor".into()),
            AgentRole::Supervisor,
            A2AMessageType::TaskProposal,
            serde_json::json!({
                "plan_summary": plan_raw,
                "task_count": tasks.len(),
            }),
        ));

        self.blackboard.set_context("supervisor_plan", plan_raw);

        Ok(SwarmPlan {
            mission_id: mission_id.to_string(),
            goal: goal.to_string(),
            tasks,
            created_at: Utc::now(),
        })
    }

    /// Execute a discrete swarm task with Trinity A2A notifications and SwarmClaw handoffs.
    pub async fn execute_task(
        &self,
        task: &SwarmTask,
        conversation_id: &str,
    ) -> Result<TaskResult> {
        let start = Instant::now();
        let agent_id = AgentId(format!(
            "{}-{}",
            task.assigned_role.display_name(),
            Uuid::new_v4()
        ));

        // Emit Trinity A2A TaskAccepted
        self.message_bus.publish(
            A2AMessage::new(
                conversation_id,
                agent_id.clone(),
                task.assigned_role,
                A2AMessageType::TaskAccepted,
                serde_json::json!({ "task_id": task.id, "title": task.title }),
            )
            .to_agent(AgentId("trinity-supervisor".into())),
        );

        // Retrieve relevant context from blackboard
        let shared_context = self
            .blackboard
            .get_context("supervisor_plan")
            .unwrap_or_default();

        let prompt = format!(
            "Execute Task ID: {}\nTitle: {}\nDescription: {}\nCanary Criteria: {:?}",
            task.id, task.title, task.description, task.canary_criteria
        );

        let execution_output = self
            .bridge
            .execute_agent_prompt(task.assigned_role, &prompt, &shared_context)
            .await?;

        // Register artifact in blackboard
        let artifact_name = format!("{}-{}.out", task.id, task.assigned_role.display_name());
        let artifact_id = self.blackboard.register_artifact(
            artifact_name.clone(),
            task.assigned_role,
            format!("blackboard://{}", artifact_name),
            execution_output.chars().take(200).collect::<String>(),
        );

        // Emit Handoff envelope if applicable
        let target_role = match task.assigned_role {
            AgentRole::Architect => Some(AgentRole::SpecialistWorker),
            AgentRole::SpecialistWorker => Some(AgentRole::Reviewer),
            AgentRole::Reviewer => Some(AgentRole::QaTester),
            AgentRole::QaTester => Some(AgentRole::Supervisor),
            AgentRole::Supervisor => None,
        };

        if let Some(to_role) = target_role {
            self.blackboard.post_handoff(HandoffEnvelope {
                handoff_id: format!("handoff-{}", Uuid::new_v4()),
                from_agent: agent_id.clone(),
                from_role: task.assigned_role,
                to_role,
                task_id: task.id.clone(),
                summary: format!("Completed {} for task {}", task.title, task.id),
                artifact_ids: vec![artifact_id.clone()],
                instructions: format!(
                    "Proceed with next phase based on output for task {}",
                    task.id
                ),
                created_at: Utc::now(),
            });
        }

        let elapsed = start.elapsed().as_millis() as u64;

        // Emit Trinity A2A TaskCompleted
        self.message_bus.publish(
            A2AMessage::new(
                conversation_id,
                agent_id,
                task.assigned_role,
                A2AMessageType::TaskCompleted,
                serde_json::json!({
                    "task_id": task.id,
                    "artifact_id": artifact_id,
                    "execution_ms": elapsed,
                }),
            )
            .to_agent(AgentId("trinity-supervisor".into())),
        );

        Ok(TaskResult {
            task_id: task.id.clone(),
            success: true,
            output: execution_output,
            artifacts_produced: vec![artifact_id],
            execution_ms: elapsed,
            error_message: None,
        })
    }

    /// Run full mission DAG through the SwarmClaw parallel coordinator with Trinity supervision.
    pub async fn execute_mission(
        &self,
        goal: &str,
        caller: &str,
        token: &str,
    ) -> Result<SwarmExecutionReceipt> {
        self.security.verify_caller(caller, token)?;

        let mission_start = Instant::now();
        let mission_id = format!("mission-{}", Uuid::new_v4());

        // 1. Supervisor plans the mission DAG
        let plan = self.plan_mission(&mission_id, goal, caller, token).await?;

        // 2. Initialize task graph
        let graph = Arc::new(TaskGraph::new());
        graph.add_tasks(plan.tasks.clone());
        graph.validate_dag()?;

        // 3. Parallel dispatch loop
        let semaphore = Arc::new(Semaphore::new(self.max_concurrency));
        let mut completed_results = Vec::new();
        let mut artifacts = Vec::new();

        while !graph.is_finished() {
            let ready_tasks = graph.get_ready_tasks();
            if ready_tasks.is_empty() {
                // If no tasks are ready and graph is not finished, check for deadlock or break
                if graph.is_finished() {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                continue;
            }

            for task in ready_tasks {
                let _permit =
                    semaphore.clone().acquire_owned().await.map_err(|_| {
                        SwarmError::Cancelled("Concurrency semaphore closed".into())
                    })?;

                // Mark running
                graph.update_task_status(&task.id, TaskStatus::Running)?;

                // Execute task
                let res = self.execute_task(&task, &mission_id).await;
                match res {
                    Ok(task_result) => {
                        artifacts.extend(task_result.artifacts_produced.clone());
                        completed_results.push(task_result.clone());
                        graph.set_task_result(&task.id, task_result)?;
                    }
                    Err(e) => {
                        let failure = TaskResult {
                            task_id: task.id.clone(),
                            success: false,
                            output: String::new(),
                            artifacts_produced: Vec::new(),
                            execution_ms: 0,
                            error_message: Some(e.to_string()),
                        };
                        completed_results.push(failure.clone());
                        graph.set_task_result(&task.id, failure)?;
                    }
                }
            }
        }

        let total_time = mission_start.elapsed().as_millis() as u64;
        let success = graph.all_succeeded();
        let completed_count = completed_results.iter().filter(|r| r.success).count();
        let failed_count = completed_results.iter().filter(|r| !r.success).count();

        let summary = format!(
            "Trinity/SwarmClaw Mission '{}': {}/{} tasks completed successfully in {}ms.",
            goal,
            completed_count,
            plan.tasks.len(),
            total_time
        );

        Ok(SwarmExecutionReceipt {
            mission_id,
            goal: goal.to_string(),
            success,
            total_tasks: plan.tasks.len(),
            completed_tasks: completed_count,
            failed_tasks: failed_count,
            execution_time_ms: total_time,
            task_results: completed_results,
            artifacts,
            summary,
        })
    }
}
