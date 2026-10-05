use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use voxy_security::{ApprovalBroker, ApprovalRiskLevel, ApprovalStatus};

use crate::builtin::*;
use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

/// Central tool registry managing discovery, validation, and safe invocation.
pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
    approval_broker: Option<Arc<ApprovalBroker>>,
}

impl ToolRegistry {
    /// Create an empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            approval_broker: None,
        }
    }

    /// Attach an authoritative ApprovalBroker to handle human-in-the-loop approvals.
    pub fn with_approval_broker(mut self, broker: Arc<ApprovalBroker>) -> Self {
        self.approval_broker = Some(broker);
        self
    }

    /// Create a tool registry pre-loaded with all native Windows, coding harness, and browser tools.
    pub fn with_builtins() -> Self {
        let registry = Self::new();
        registry.register_builtin_sync(Arc::new(WindowListTool::new()));
        registry.register_builtin_sync(Arc::new(WindowFocusTool::new()));
        registry.register_builtin_sync(Arc::new(WindowCloseTool::new()));
        registry.register_builtin_sync(Arc::new(WindowMinimizeTool::new()));
        registry.register_builtin_sync(Arc::new(WindowMaximizeTool::new()));
        registry.register_builtin_sync(Arc::new(ProcessListTool::new()));
        registry.register_builtin_sync(Arc::new(ProcessInfoTool::new()));
        registry.register_builtin_sync(Arc::new(ProcessKillTool::new()));
        registry.register_builtin_sync(Arc::new(FileListTool::new()));
        registry.register_builtin_sync(Arc::new(FileReadTool::new()));
        registry.register_builtin_sync(Arc::new(FileWriteTool::new()));
        registry.register_builtin_sync(Arc::new(FileDeleteTool::new()));
        registry.register_builtin_sync(Arc::new(SystemInfoTool::new()));
        registry.register_builtin_sync(Arc::new(SystemScreenshotTool::new()));
        registry.register_builtin_sync(Arc::new(HarnessIndexTool::new()));
        registry.register_builtin_sync(Arc::new(HarnessSearchTool::new()));
        registry.register_builtin_sync(Arc::new(HarnessRunCommandTool::new()));
        registry.register_builtin_sync(Arc::new(HarnessApplyPatchTool::new()));
        registry.register_builtin_sync(Arc::new(HarnessParseDiagnosticsTool::new()));
        registry.register_builtin_sync(Arc::new(HarnessApplyPatchTransactionTool::new()));
        registry.register_builtin_sync(Arc::new(ResearchInvestigateTool::new()));
        registry.register_builtin_sync(Arc::new(TaskHistoryGetTool::new()));
        registry.register_builtin_sync(Arc::new(TaskArtifactsListTool::new()));
        registry.register_builtin_sync(Arc::new(MemoryStoreTool::new()));
        registry.register_builtin_sync(Arc::new(MemoryRecallTool::new()));
        registry.register_builtin_sync(Arc::new(MemoryForgetTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserOpenTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserFetchTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserLaunchTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserCloseTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserAttachTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserNavigateTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserListPagesTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserSwitchPageTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserObserveTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserExtractTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserScreenshotTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserDownloadTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserUploadTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserWaitTool::new()));
        registry.register_builtin_sync(Arc::new(BrowserGetUrlTool::new()));
        registry
    }

    fn register_builtin_sync(&self, tool: Arc<dyn Tool>) {
        let name = tool.metadata().name.clone();
        if let Ok(mut lock) = self.tools.try_write() {
            lock.insert(name, tool);
        }
    }

    /// Register a tool into the registry.
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.metadata().name.clone();
        let mut lock = self.tools.write().await;
        lock.insert(name, tool);
    }

    /// Unregister a tool by name.
    pub async fn unregister(&self, name: &str) -> bool {
        let mut lock = self.tools.write().await;
        lock.remove(name).is_some()
    }

    /// Retrieve tool metadata for all currently registered tools.
    pub async fn list_tools(&self) -> Vec<ToolMetadata> {
        let lock = self.tools.read().await;
        lock.values().map(|t| t.metadata().clone()).collect()
    }

    /// Get a specific tool by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let lock = self.tools.read().await;
        lock.get(name).cloned()
    }

    /// Number of registered tools.
    pub async fn count(&self) -> usize {
        let lock = self.tools.read().await;
        lock.len()
    }

    /// Execute a tool with risk validation, confirmation checking, timeout, and verification.
    pub async fn execute(
        &self,
        name: &str,
        params: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let tool = {
            let lock = self.tools.read().await;
            lock.get(name).cloned().ok_or_else(|| {
                ToolError::ToolNotFound(format!("Tool '{}' is not registered", name))
            })?
        };

        let meta = tool.metadata();

        // 1. Permission and Confirmation Enforcement Gate
        let mut confirmed = ctx.user_confirmed;
        if meta.requires_confirmation && !confirmed {
            if let Some(ref broker) = self.approval_broker {
                let risk_level = match meta.risk_tier {
                    RiskTier::Destructive => ApprovalRiskLevel::Destructive,
                    RiskTier::Privileged => ApprovalRiskLevel::Privileged,
                    RiskTier::Modify => ApprovalRiskLevel::Modify,
                    RiskTier::LowRisk => ApprovalRiskLevel::LowRisk,
                    RiskTier::Read => ApprovalRiskLevel::Safe,
                };
                let reason = format!(
                    "Tool '{}' requires authorization before execution",
                    name
                );
                let timeout = Duration::from_millis(meta.timeout_ms.max(30_000));
                let (req, rx) = broker.submit_request(
                    &ctx.session_id,
                    name,
                    &params,
                    risk_level,
                    reason,
                    timeout,
                );

                info!(
                    tool = %name,
                    request_id = %req.id,
                    "Awaiting human approval via ApprovalBroker"
                );

                match rx.await {
                    Ok(decision) => match decision.status {
                        ApprovalStatus::Approved => {
                            info!(tool = %name, request_id = %req.id, "Action approved by user");
                            confirmed = true;
                        }
                        ApprovalStatus::Denied => {
                            warn!(tool = %name, request_id = %req.id, "Action denied by user");
                            return Err(ToolError::ExecutionFailed(format!(
                                "Action '{}' was explicitly denied by user",
                                name
                            )));
                        }
                        ApprovalStatus::Expired => {
                            warn!(tool = %name, request_id = %req.id, "Action approval expired");
                            return Err(ToolError::Timeout(format!(
                                "Approval request for action '{}' expired without response",
                                name
                            )));
                        }
                        ApprovalStatus::Cancelled => {
                            warn!(tool = %name, request_id = %req.id, "Action approval cancelled");
                            return Err(ToolError::ExecutionFailed(format!(
                                "Action '{}' was cancelled",
                                name
                            )));
                        }
                        ApprovalStatus::Pending => unreachable!(),
                    },
                    Err(_) => {
                        return Err(ToolError::ExecutionFailed(
                            "Approval broker channel dropped before decision".into(),
                        ));
                    }
                }
            } else {
                warn!(
                    tool = %name,
                    risk = ?meta.risk_tier,
                    session = %ctx.session_id,
                    "Tool execution blocked: human confirmation required"
                );
                return Err(ToolError::ConfirmationRequired(format!(
                    "Action '{}' has risk tier {:?} and requires explicit confirmation",
                    name, meta.risk_tier
                )));
            }
        }

        if meta.requires_confirmation && !confirmed {
            return Err(ToolError::ConfirmationRequired(format!(
                "Action '{}' requires confirmed authorization",
                name
            )));
        }

        info!(
            tool = %name,
            risk = ?meta.risk_tier,
            session = %ctx.session_id,
            "Executing tool"
        );

        // 2. Invocation with strict timeout boundary
        let mut exec_ctx = ctx.clone();
        exec_ctx.user_confirmed = confirmed;

        let timeout_dur = Duration::from_millis(meta.timeout_ms);
        let exec_result =
            tokio::time::timeout(timeout_dur, tool.execute(params.clone(), &exec_ctx)).await;

        let result = match exec_result {
            Ok(Ok(res)) => res,
            Ok(Err(e)) => {
                error!(tool = %name, error = %e, "Tool execution failed");
                return Err(e);
            }
            Err(_) => {
                error!(tool = %name, timeout_ms = meta.timeout_ms, "Tool execution timed out");
                return Err(ToolError::Timeout(format!(
                    "Tool '{}' timed out after {}ms",
                    name, meta.timeout_ms
                )));
            }
        };

        // 3. Post-action verification
        if result.success {
            let verified = tool.verify(&params, &result).await.unwrap_or(false);
            if !verified {
                warn!(tool = %name, "Post-action verification check failed");
            }
        }

        Ok(result)
    }

    /// Rollback a previous tool operation by its rollback token.
    pub async fn rollback(&self, tool_name: &str, token: &str) -> Result<()> {
        let tool = self
            .get(tool_name)
            .await
            .ok_or_else(|| ToolError::ToolNotFound(tool_name.to_string()))?;

        tool.rollback(token).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
