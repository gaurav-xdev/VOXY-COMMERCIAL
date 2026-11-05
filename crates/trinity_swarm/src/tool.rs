use crate::coordinator::SwarmCoordinator;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use voxy_tool_calling::{RiskTier, Tool, ToolCategory, ToolContext, ToolMetadata, ToolResult};

/// Internal OSMOO tool for executing multi-agent swarm tasks using Trinity & SwarmClaw.
pub struct OsmooSwarmOrchestratorTool {
    metadata: ToolMetadata,
    coordinator: Arc<SwarmCoordinator>,
}

impl OsmooSwarmOrchestratorTool {
    pub fn new(coordinator: Arc<SwarmCoordinator>) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_swarm_orchestrator",
            "Internal OSMOO multi-agent orchestration engine powered by Trinity A2A protocols and SwarmClaw dynamic team coordination. Accessible exclusively to OSMOO's autonomous agent.",
            ToolCategory::Coding,
            RiskTier::Privileged,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["internal_token", "caller_identity", "goal"],
            "properties": {
                "internal_token": { "type": "string", "description": "Cryptographic bearer token held by OSMOO agent" },
                "caller_identity": { "type": "string", "description": "Must be OSMOO_AUTONOMOUS_OPERATOR" },
                "goal": { "type": "string", "description": "High-level engineering mission or objective" },
                "max_concurrency": { "type": "integer", "description": "Maximum parallel subtasks (1-8)", "default": 4 }
            }
        }));

        Self {
            metadata,
            coordinator,
        }
    }
}

#[async_trait]
impl Tool for OsmooSwarmOrchestratorTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> voxy_tool_calling::Result<ToolResult> {
        let caller = params["caller_identity"]
            .as_str()
            .unwrap_or("untrusted_user");
        let token = params["internal_token"].as_str().unwrap_or("");
        let goal = params["goal"].as_str().unwrap_or("");

        // 1. Strict internal permission boundary
        if let Err(e) = self.coordinator.security().verify_caller(caller, token) {
            return Ok(ToolResult::failure(format!("ACCESS DENIED: {e}"))
                .with_observation("Direct user or external attempt to invoke internal swarm orchestrator was blocked"));
        }

        if goal.trim().is_empty() {
            return Ok(ToolResult::failure("Goal must not be empty"));
        }

        // 2. Execute mission
        match self.coordinator.execute_mission(goal, caller, token).await {
            Ok(receipt) => {
                let observation = format!(
                    "Swarm completed mission '{}' with {} tasks. Success: {}",
                    receipt.goal, receipt.total_tasks, receipt.success
                );
                let json_data = serde_json::to_value(&receipt)
                    .unwrap_or_else(|_| json!({ "error": "receipt serialization failed" }));

                Ok(ToolResult::success(json_data).with_observation(observation))
            }
            Err(e) => Ok(ToolResult::failure(format!("Swarm execution failed: {e}"))),
        }
    }
}

/// Internal OSMOO tool for inspecting the Trinity A2A message bus and SwarmClaw blackboard.
pub struct OsmooSwarmStatusTool {
    metadata: ToolMetadata,
    coordinator: Arc<SwarmCoordinator>,
}

impl OsmooSwarmStatusTool {
    pub fn new(coordinator: Arc<SwarmCoordinator>) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_swarm_status",
            "Inspect active Trinity A2A message history and SwarmClaw blackboard artifacts. Accessible exclusively to OSMOO's autonomous agent.",
            ToolCategory::System,
            RiskTier::Privileged,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["internal_token", "caller_identity"],
            "properties": {
                "internal_token": { "type": "string", "description": "Cryptographic bearer token held by OSMOO agent" },
                "caller_identity": { "type": "string", "description": "Must be OSMOO_AUTONOMOUS_OPERATOR" }
            }
        }));

        Self {
            metadata,
            coordinator,
        }
    }
}

#[async_trait]
impl Tool for OsmooSwarmStatusTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> voxy_tool_calling::Result<ToolResult> {
        let caller = params["caller_identity"]
            .as_str()
            .unwrap_or("untrusted_user");
        let token = params["internal_token"].as_str().unwrap_or("");

        // Strict internal permission boundary
        if let Err(e) = self.coordinator.security().verify_caller(caller, token) {
            return Ok(
                ToolResult::failure(format!("ACCESS DENIED: {e}")).with_observation(
                    "Direct user attempt to inspect internal swarm status was blocked",
                ),
            );
        }

        let total_messages = self.coordinator.message_bus().count();
        let artifacts = self.coordinator.blackboard().all_artifacts();

        let data = json!({
            "total_a2a_messages": total_messages,
            "total_artifacts": artifacts.len(),
            "artifacts": artifacts,
        });

        Ok(ToolResult::success(data).with_observation(format!(
            "Swarm Status: {total_messages} A2A messages, {} artifacts",
            artifacts.len()
        )))
    }
}
