use crate::context_explorer::ContextExplorer;
use crate::debugger::{DebugAnalysis, Debugger};
use crate::journal::{ActiveTransaction, TransactionJournal};
use crate::mcp_bridge::McpBridge;
use crate::patcher::Patcher;
use crate::process_runner::ProcessRunner;
use crate::reviewer::Reviewer;
use crate::session_store::SessionStore;
use crate::skill_router::SkillRouter;
use crate::toolchain::{LanguageToolchain, ToolchainDetector, ToolchainStatus};
use crate::types::{
    CommandExecutionInput, CommandExecutionResult, FileEditInput, FileEditResult, HarnessError,
    Result,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use voxy_model_router::ModelRouter;
use voxy_security::approval::{ApprovalBroker, ApprovalRiskLevel, ApprovalStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutonomousRepairResult {
    pub success: bool,
    pub steps_executed: usize,
    pub proposed_patch: Option<FileEditInput>,
    pub diff: Option<String>,
    pub verification_stdout: String,
    pub review_blocking: bool,
}

pub struct CodingEngine {
    workspace_root: PathBuf,
    journal: TransactionJournal,
    #[allow(dead_code)]
    session_store: SessionStore,
    reviewer: Reviewer,
    approval_broker: Option<Arc<ApprovalBroker>>,
    model_router: Option<Arc<ModelRouter>>,
    #[allow(dead_code)]
    mcp_bridge: Option<Arc<McpBridge>>,
}

impl CodingEngine {
    pub fn new(workspace_root: &Path) -> Self {
        let ws = workspace_root.to_path_buf();
        Self {
            workspace_root: ws.clone(),
            journal: TransactionJournal::new(&ws),
            session_store: SessionStore::new(&ws),
            reviewer: Reviewer::new(),
            approval_broker: None,
            model_router: None,
            mcp_bridge: None,
        }
    }

    pub fn with_approval_broker(mut self, broker: Arc<ApprovalBroker>) -> Self {
        self.approval_broker = Some(broker);
        self
    }

    pub fn with_model_router(mut self, router: Arc<ModelRouter>) -> Self {
        self.model_router = Some(router);
        self
    }

    pub fn with_mcp_bridge(mut self, bridge: Arc<McpBridge>) -> Self {
        self.mcp_bridge = Some(bridge);
        self
    }

    pub fn initialize(&self) -> Result<Vec<Uuid>> {
        // Startup crash recovery: rolls back any uncommitted transactions left from crashes
        self.journal.recover_pending_transactions()
    }

    pub fn begin_transaction(&self) -> Result<ActiveTransaction> {
        self.journal.begin()
    }

    pub fn apply_file_edit(
        &self,
        tx: &mut ActiveTransaction,
        input: &FileEditInput,
    ) -> Result<FileEditResult> {
        let rel_path = Patcher::canonicalize_workspace_rel(&self.workspace_root, &input.file_path)?;

        // Record pre-image in write-ahead journal before modifying file
        tx.record_before_modify(&rel_path)?;

        let result = Patcher::apply_edit(&self.workspace_root, input)?;

        // Run independent code review on the generated diff
        if let Ok(review) = self.reviewer.review_diff(&result.diff) {
            if Reviewer::is_blocking(&review) {
                tracing::warn!(
                    file = %result.file_path,
                    defects = ?review.findings,
                    "Blocking defects detected during independent code review"
                );
            }
        }

        Ok(result)
    }

    pub async fn execute_command(
        &self,
        input: CommandExecutionInput,
        session_id: &str,
    ) -> Result<CommandExecutionResult> {
        let cmd_lower = input.command.to_lowercase();
        let is_destructive = cmd_lower.contains("rm ")
            || cmd_lower.contains("del ")
            || cmd_lower.contains("format ")
            || cmd_lower.contains("git push")
            || cmd_lower.contains("drop table");

        if is_destructive {
            if let Some(ref broker) = self.approval_broker {
                let params = serde_json::json!({ "command": input.command });
                let (_req, rx) = broker.submit_request(
                    session_id,
                    "execute_command",
                    &params,
                    ApprovalRiskLevel::Destructive,
                    "Executing potentially destructive shell command",
                    Duration::from_secs(60),
                );

                match rx.await {
                    Ok(decision) => {
                        if decision.status != ApprovalStatus::Approved {
                            return Err(HarnessError::Security(format!(
                                "Destructive command '{}' was rejected or expired by user approval",
                                input.command
                            )));
                        }
                    }
                    Err(_) => {
                        return Err(HarnessError::Security("Approval channel dropped".into()));
                    }
                }
            }
        }

        ProcessRunner::run_command(&input, &self.workspace_root).await
    }

    pub fn detect_toolchains(&self) -> Vec<LanguageToolchain> {
        ToolchainDetector::detect_all(&self.workspace_root)
    }

    pub fn diagnose_failure(&self, compiler_output: &str) -> DebugAnalysis {
        Debugger::analyze_compiler_output(compiler_output)
    }

    /// End-to-end model-driven autonomous repair loop:
    /// 1. Discovers workspace files and toolchains.
    /// 2. Discovers skills and matches prompt keywords.
    /// 3. Executes the test runner to surface real failure diagnostics.
    /// 4. Dispatches prompt + failure diagnostics + file contents to ModelRouter.
    /// 5. Dynamically parses model-proposed patch without hardcoding.
    /// 6. Applies atomic edit under a durable write-ahead transaction.
    /// 7. Re-executes tests to verify repair.
    /// 8. Runs independent code review gate before committing.
    pub async fn execute_autonomous_repair_task(
        &self,
        task_instruction: &str,
        session_id: &str,
    ) -> Result<AutonomousRepairResult> {
        let router = self.model_router.as_ref().ok_or_else(|| {
            HarnessError::RoutingError(
                "ModelRouter must be configured for autonomous task execution".into(),
            )
        })?;

        // 1. Toolchain detection
        let toolchains = self.detect_toolchains();
        let rust_avail = toolchains.iter().any(|t| {
            t.language == crate::toolchain::Language::Rust
                && matches!(t.status, ToolchainStatus::Available { .. })
        });

        if !rust_avail {
            return Err(HarnessError::PlanningError(
                "No supported toolchain found for autonomous repair in workspace".into(),
            ));
        }

        // 2. Discover skills and compose prompt injection
        let skills =
            SkillRouter::select_skills_for_prompt(task_instruction, &[&self.workspace_root]);
        let skill_prompt =
            SkillRouter::compose_skill_system_prompt(&skills, &[&self.workspace_root]);

        // 3. Find target file(s)
        let rs_files = ContextExplorer::glob_files(&self.workspace_root, "*.rs", None)?;
        let target_rel = rs_files.first().ok_or_else(|| {
            HarnessError::PlanningError("No source files found in workspace".into())
        })?;

        // 4. Run baseline test to surface the defect
        let test_cmd = CommandExecutionInput {
            command: "cargo test --lib".into(),
            cwd: None,
            timeout_ms: Some(60_000),
            description: Some("Run tests to capture failure diagnostics".into()),
            run_in_background: false,
        };
        let baseline = self.execute_command(test_cmd.clone(), session_id).await?;

        if baseline.exit_code == 0 {
            // Already passing
            return Ok(AutonomousRepairResult {
                success: true,
                steps_executed: 1,
                proposed_patch: None,
                diff: None,
                verification_stdout: baseline.stdout,
                review_blocking: false,
            });
        }

        // 5. Diagnose failure
        let diagnosis = self.diagnose_failure(&format!("{}\n{}", baseline.stdout, baseline.stderr));

        // 6. Read target file content
        let full_target = self.workspace_root.join(target_rel);
        let source_content = std::fs::read_to_string(&full_target).map_err(HarnessError::Io)?;

        // 7. Prompt the model to propose a fix
        let model_prompt = format!(
            "{}\n\nTask: {}\n\nFailure Diagnosis:\nPrimary: {}\nRecommended Action: {}\n\nFile '{}':\n```rust\n{}\n```\n\nOutput only a JSON object with this schema:\n{{\"file_path\": \"{}\", \"old_string\": \"<exact lines to replace>\", \"new_string\": \"<corrected lines>\", \"replace_all\": false}}",
            skill_prompt,
            task_instruction,
            diagnosis.primary_cause,
            diagnosis.recommended_action,
            target_rel,
            source_content,
            target_rel,
        );

        let model_response = router
            .route_complete(&model_prompt)
            .await
            .map_err(|e| HarnessError::RoutingError(format!("ModelRouter query failed: {}", e)))?;

        // 8. Dynamically parse the model's proposed patch
        let patch_input = Self::extract_patch_json(&model_response).ok_or_else(|| {
            HarnessError::EditError(format!(
                "Failed to parse structured JSON patch from model response: {}",
                model_response
            ))
        })?;

        // 9. Begin durable transaction and apply patch
        let mut tx = self.begin_transaction()?;
        let edit_res = match self.apply_file_edit(&mut tx, &patch_input) {
            Ok(res) => res,
            Err(e) => {
                let _ = tx.rollback();
                return Err(e);
            }
        };

        // 10. Run verification build
        let verify_res = match self.execute_command(test_cmd, session_id).await {
            Ok(res) => res,
            Err(e) => {
                let _ = tx.rollback();
                return Err(e);
            }
        };
        let is_success = verify_res.exit_code == 0;

        let mut is_blocking_review = false;
        if let Ok(review) = self.reviewer.review_diff(&edit_res.diff) {
            is_blocking_review = Reviewer::is_blocking(&review);
        }

        if is_success && !is_blocking_review {
            tx.commit()?;
        } else {
            tx.rollback()?;
        }

        Ok(AutonomousRepairResult {
            success: is_success && !is_blocking_review,
            steps_executed: 5,
            proposed_patch: Some(patch_input),
            diff: Some(edit_res.diff),
            verification_stdout: verify_res.stdout,
            review_blocking: is_blocking_review,
        })
    }

    /// Helper to extract structured JSON patch from model output even if wrapped in markdown code blocks.
    fn extract_patch_json(text: &str) -> Option<FileEditInput> {
        let cleaned = if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                &text[start..=end]
            } else {
                text
            }
        } else {
            text
        };

        serde_json::from_str::<FileEditInput>(cleaned).ok()
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}
