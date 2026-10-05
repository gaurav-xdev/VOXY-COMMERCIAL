//! Sandboxed Skill Execution Runtime with Technical Permission Enforcement.
//!
//! Lifecycle:
//! LOAD -> VALIDATE -> RESOLVE DEPENDENCIES -> AUTHORIZE -> INITIALIZE -> EXECUTE -> VERIFY -> FINALIZE

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{info, warn};

use crate::error::{Result, SkillsError};
use crate::types::{SkillManifest, SkillPermission, SkillTrustLevel};
use voxy_database::CommercialStore;
use voxy_security::{ApprovalBroker, ApprovalRiskLevel};
use voxy_tool_calling::{ToolContext, ToolRegistry, ToolResult};

pub struct SkillRuntime {
    tool_registry: Arc<ToolRegistry>,
    approval_broker: Option<Arc<ApprovalBroker>>,
    store: Option<Arc<CommercialStore>>,
}

impl SkillRuntime {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            approval_broker: None,
            store: None,
        }
    }

    pub fn with_approval_broker(mut self, broker: Arc<ApprovalBroker>) -> Self {
        self.approval_broker = Some(broker);
        self
    }

    pub fn with_store(mut self, store: Arc<CommercialStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Validates and registers a skill manifest in the database.
    pub async fn install_skill(&self, manifest: SkillManifest) -> Result<()> {
        manifest
            .validate()
            .map_err(|e| SkillsError::ManifestInvalid(e))?;

        if let Some(ref store) = self.store {
            let manifest_json = serde_json::to_string(&manifest)
                .map_err(|e| SkillsError::InvalidConfig(e.to_string()))?;

            let trust_str = match manifest.trust_level {
                SkillTrustLevel::System => "system",
                SkillTrustLevel::Verified => "verified",
                SkillTrustLevel::Published => "published",
                SkillTrustLevel::Untrusted => "untrusted",
                SkillTrustLevel::Blocked => "blocked",
            };

            store
                .register_skill(
                    &manifest.name,
                    &manifest.display_name,
                    &manifest.description,
                    &manifest.version,
                    &manifest.publisher_id,
                    trust_str,
                    &manifest_json,
                    &manifest.checksum,
                )
                .await
                .map_err(|e| SkillsError::InvalidConfig(format!("DB store failed: {}", e)))?;
        }

        Ok(())
    }

    /// Executes a tool call strictly within the security boundaries and declared permissions of a skill.
    pub async fn execute_skill_action(
        &self,
        manifest: &SkillManifest,
        tool_name: &str,
        params: serde_json::Value,
        session_id: &str,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        // 1. Check if skill is blocked
        if manifest.trust_level == SkillTrustLevel::Blocked {
            return Err(SkillsError::UntrustedSkill(format!(
                "Skill '{}' is blocked",
                manifest.name
            )));
        }

        // 2. Validate tool permissions
        let required_permission = map_tool_to_permission(tool_name);
        if let Some(perm) = required_permission {
            if !manifest.has_permission(perm) {
                warn!(
                    "Skill '{}' attempted to call tool '{}' without required permission '{:?}'",
                    manifest.name, tool_name, perm
                );
                return Err(SkillsError::PermissionDenied(format!(
                    "Skill '{}' is missing required permission '{:?}' for tool '{}'",
                    manifest.name, perm, tool_name
                )));
            }
        }

        // 3. Approval Broker Check for destructive or privileged operations
        let mut user_confirmed = false;
        if is_privileged_tool(tool_name) {
            if let Some(ref broker) = self.approval_broker {
                let risk = match tool_name {
                    "file_delete" | "process_kill" | "memory_forget" => ApprovalRiskLevel::Destructive,
                    _ => ApprovalRiskLevel::Privileged,
                };

                let justification = format!(
                    "Skill '{}' requests execution of privileged tool '{}'",
                    manifest.name, tool_name
                );

                let (_req, rx) = broker.submit_request(
                    session_id,
                    tool_name,
                    &params,
                    risk,
                    justification,
                    Duration::from_secs(30),
                );

                match rx.await {
                    Ok(decision) if decision.status == voxy_security::ApprovalStatus::Approved => {
                        user_confirmed = true;
                    }
                    _ => {
                        return Err(SkillsError::PermissionDenied(format!(
                            "Human approval was denied or timed out for action '{}'",
                            tool_name
                        )));
                    }
                }
            } else {
                user_confirmed = true;
            }
        }

        // 4. Construct execution context
        let mut ctx = ToolContext::new(session_id);
        if user_confirmed {
            ctx = ctx.with_confirmation(true);
        }

        // 5. Sandboxed execution with timeout
        let max_time = Duration::from_secs(manifest.timeout_seconds);
        let exec_future = self.tool_registry.execute(tool_name, params, &ctx);

        let tool_result = timeout(max_time, exec_future)
            .await
            .map_err(|_| SkillsError::Timeout(format!("Skill execution exceeded {}s", manifest.timeout_seconds)))?
            .map_err(|e| SkillsError::SkillExecutionFailed(format!("Tool execution error: {}", e)))?;

        info!(
            "Skill '{}' executed tool '{}' successfully in {}ms",
            manifest.name,
            tool_name,
            start.elapsed().as_millis()
        );

        Ok(tool_result)
    }
}

fn map_tool_to_permission(tool_name: &str) -> Option<SkillPermission> {
    match tool_name {
        "file_read" | "file_list" => Some(SkillPermission::ReadFiles),
        "file_write" => Some(SkillPermission::WriteFiles),
        "file_delete" => Some(SkillPermission::DeleteFiles),
        "browser_open" | "browser_navigate" | "browser_extract" | "browser_observe" => {
            Some(SkillPermission::BrowserAccess)
        }
        "process_list" | "process_info" | "process_kill" => Some(SkillPermission::ProcessControl),
        "window_list" | "window_focus" | "window_close" => Some(SkillPermission::ComputerControl),
        "memory_recall" => Some(SkillPermission::MemoryRead),
        "memory_store" => Some(SkillPermission::MemoryWrite),
        "memory_forget" => Some(SkillPermission::MemoryWrite),
        "research_investigate" => Some(SkillPermission::ResearchAccess),
        "harness_search"
        | "harness_index"
        | "harness_apply_patch"
        | "harness_apply_patch_transaction"
        | "harness_parse_diagnostics" => Some(SkillPermission::CodeHarnessAccess),
        "task_history_get" | "task_artifacts_list" => Some(SkillPermission::TaskHistoryAccess),
        _ => None,
    }
}

fn is_privileged_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "file_delete"
            | "process_kill"
            | "harness_run_command"
            | "harness_apply_patch"
            | "harness_apply_patch_transaction"
            | "memory_forget"
    )
}
