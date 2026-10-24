use crate::engine::KodusReviewEngine;
use crate::types::{ReviewScope, ReviewTrigger};
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use voxy_tool_calling::{
    RiskTier, Tool, ToolCategory, ToolContext, ToolMetadata, ToolResult,
};

/// Internal OSMOO tool for executing Kodus code reviews during autonomous workflows.
pub struct KodusInternalReviewTool {
    metadata: ToolMetadata,
    engine: Arc<KodusReviewEngine>,
}

impl KodusInternalReviewTool {
    pub fn new(engine: Arc<KodusReviewEngine>) -> Self {
        let metadata = ToolMetadata::new(
            "kodus_internal_code_review",
            "Internal OSMOO code review engine powered by Kodus AI. Accessible exclusively to OSMOO's autonomous agent.",
            ToolCategory::Coding,
            RiskTier::Privileged,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["internal_token", "caller_identity"],
            "properties": {
                "internal_token": { "type": "string", "description": "Cryptographic bearer token held by OSMOO agent" },
                "caller_identity": { "type": "string", "description": "Must be OSMOO_AUTONOMOUS_OPERATOR" },
                "scope": {
                    "type": "string",
                    "enum": ["working_tree", "staged", "files"],
                    "default": "working_tree"
                },
                "files": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "trigger": {
                    "type": "string",
                    "enum": ["substantial_code_change", "pre_task_completion", "pre_pr_merge", "security_sensitive", "test_failure", "explicit_agent_request"],
                    "default": "pre_task_completion"
                }
            }
        }));

        Self { metadata, engine }
    }
}

#[async_trait]
impl Tool for KodusInternalReviewTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> voxy_tool_calling::Result<ToolResult> {
        let caller = params["caller_identity"].as_str().unwrap_or("untrusted_user");
        let token = params["internal_token"].as_str().unwrap_or("");

        // 1. Strict internal permission boundary
        if let Err(e) = self.engine.security().verify_caller(caller, token) {
            return Ok(ToolResult::failure(format!("ACCESS DENIED: {e}"))
                .with_observation("Direct user attempt to invoke internal Kodus reviewer was blocked"));
        }

        // 2. Parse scope
        let scope_str = params["scope"].as_str().unwrap_or("working_tree");
        let scope = match scope_str {
            "staged" => ReviewScope::StagedOnly,
            "files" => {
                let paths = params["files"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(PathBuf::from))
                            .collect()
                    })
                    .unwrap_or_default();
                ReviewScope::Files(paths)
            }
            _ => ReviewScope::WorkingTree,
        };

        // 3. Parse trigger
        let trigger_str = params["trigger"].as_str().unwrap_or("pre_task_completion");
        let trigger = match trigger_str {
            "substantial_code_change" => ReviewTrigger::SubstantialCodeChange,
            "pre_pr_merge" => ReviewTrigger::PrePullRequestMerge,
            "security_sensitive" => ReviewTrigger::SecuritySensitiveChange,
            "test_failure" => ReviewTrigger::TestFailureInvestigation,
            "explicit_agent_request" => ReviewTrigger::ExplicitAgentRequest,
            _ => ReviewTrigger::PreTaskCompletion,
        };

        match self.engine.review(scope, trigger, token).await {
            Ok(report) => {
                let blocking_count = report.blocking_findings().len();
                let findings_count = report.findings.len();
                Ok(ToolResult::success(json!({
                    "report_id": report.report_id,
                    "provider": report.provider,
                    "findings_count": findings_count,
                    "blocking_count": blocking_count,
                    "has_blocking_defects": report.has_blocking_defects,
                    "findings": report.findings,
                    "duration_ms": report.duration_ms,
                }))
                .with_observation(format!(
                    "Kodus internal review completed: {} findings ({} blocking defects).",
                    findings_count, blocking_count
                )))
            }
            Err(e) => Ok(ToolResult::failure(format!("Review error: {e}"))),
        }
    }
}
