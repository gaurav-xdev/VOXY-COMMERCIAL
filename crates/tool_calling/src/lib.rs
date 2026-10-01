//! Tool framework: registration, validation, invocation, risk tiering, and execution.

pub mod builtin;
pub mod error;
pub mod metadata;
pub mod registry;
pub mod traits;

pub use builtin::*;
pub use error::{Result, ToolError};
pub use metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
pub use registry::ToolRegistry;
pub use traits::Tool;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn tool_registry_with_builtins() {
        let registry = ToolRegistry::with_builtins();
        assert!(registry.count().await >= 15);

        let tools = registry.list_tools().await;
        assert!(tools.iter().any(|t| t.name == "window_list"));
        assert!(tools.iter().any(|t| t.name == "file_list"));
        assert!(tools.iter().any(|t| t.name == "process_list"));
        assert!(tools.iter().any(|t| t.name == "system_info"));
        assert!(tools.iter().any(|t| t.name == "harness_index_repo"));
    }

    #[tokio::test]
    async fn confirmation_gating_blocks_unconfirmed_destructive_tool() {
        let registry = ToolRegistry::with_builtins();
        let unconfirmed_ctx = ToolContext::new("test-session").with_confirmation(false);

        let res = registry
            .execute("process_kill", json!({ "pid": 999999 }), &unconfirmed_ctx)
            .await;

        match res {
            Err(ToolError::ConfirmationRequired(msg)) => {
                assert!(msg.contains("requires explicit confirmation"));
            }
            other => panic!("Expected ConfirmationRequired error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn system_info_tool_executes_successfully() {
        let registry = ToolRegistry::with_builtins();
        let ctx = ToolContext::new("test-session");

        let res = registry
            .execute("system_info", json!({}), &ctx)
            .await
            .unwrap();

        assert!(res.success);
        assert!(res.data.get("os").is_some());
    }

    #[tokio::test]
    async fn file_operations_flow_with_rollback() {
        let registry = ToolRegistry::with_builtins();
        let temp_dir = std::env::temp_dir().join(format!("voxy_test_{}", uuid::Uuid::new_v4()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let test_file = temp_dir.join("sample.txt");

        // 1. Write file
        let write_res = registry
            .execute(
                "file_write",
                json!({
                    "path": test_file.to_string_lossy(),
                    "content": "Hello VOXY Autonomous World"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(write_res.success);
        let token = write_res.rollback_token.expect("Expected rollback token");

        // 2. Read file
        let read_res = registry
            .execute(
                "file_read",
                json!({ "path": test_file.to_string_lossy() }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(
            read_res.data.get("content").and_then(|v| v.as_str()),
            Some("Hello VOXY Autonomous World")
        );

        // 3. Rollback
        registry.rollback("file_write", &token).await.unwrap();
        assert!(!tokio::fs::try_exists(&test_file).await.unwrap_or(true));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
