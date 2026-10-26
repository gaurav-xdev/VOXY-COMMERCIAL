use crate::debugger::{DebugAnalysis, Debugger};
use crate::journal::{ActiveTransaction, TransactionJournal};
use crate::mcp_bridge::McpBridge;
use crate::patcher::Patcher;
use crate::process_runner::ProcessRunner;
use crate::reviewer::Reviewer;
use crate::session_store::SessionStore;
use crate::toolchain::{LanguageToolchain, ToolchainDetector};
use crate::types::{
    CommandExecutionInput, CommandExecutionResult, FileEditInput, FileEditResult, HarnessError,
    Result,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use voxy_model_router::ModelRouter;
use voxy_security::approval::{ApprovalBroker, ApprovalRiskLevel, ApprovalStatus};

pub struct CodingEngine {
    workspace_root: PathBuf,
    journal: TransactionJournal,
    #[allow(dead_code)]
    session_store: SessionStore,
    reviewer: Reviewer,
    approval_broker: Option<Arc<ApprovalBroker>>,
    model_router: Option<Arc<ModelRouter>>,
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

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}
