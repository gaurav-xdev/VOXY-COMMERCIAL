use crate::types::{HarnessError, Result};
use serde_json::Value;
use std::sync::Arc;
use voxy_tool_calling::{ToolContext, ToolMetadata, ToolRegistry, ToolResult};

pub struct McpBridge {
    registry: Arc<ToolRegistry>,
}

impl McpBridge {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    pub async fn list_available_tools(&self) -> Vec<ToolMetadata> {
        self.registry.list_tools().await
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        params: Value,
        session_id: &str,
        confirmed: bool,
    ) -> Result<ToolResult> {
        let ctx = ToolContext::new(session_id).with_confirmation(confirmed);
        self.registry
            .execute(tool_name, params, &ctx)
            .await
            .map_err(|e| HarnessError::CommandError(format!("Tool execution error for '{}': {}", tool_name, e)))
    }
}
