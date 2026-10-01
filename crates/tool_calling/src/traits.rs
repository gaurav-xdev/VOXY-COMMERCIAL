//! Tool trait and behavior contracts.

use crate::error::Result;
use crate::metadata::{ToolContext, ToolMetadata, ToolResult};
use async_trait::async_trait;

/// Core contract for an executable tool in VOXY.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the metadata describing this tool.
    fn metadata(&self) -> &ToolMetadata;

    /// Execute the tool logic with given parameters and invocation context.
    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult>;

    /// Post-action verification hook to verify the intended effect was achieved.
    async fn verify(&self, _params: &serde_json::Value, _result: &ToolResult) -> Result<bool> {
        Ok(true)
    }

    /// Rollback the side effect associated with the rollback token if supported.
    async fn rollback(&self, _token: &str) -> Result<()> {
        Ok(())
    }
}
