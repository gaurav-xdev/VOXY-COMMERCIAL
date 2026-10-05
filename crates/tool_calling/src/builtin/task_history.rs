//! Task History, Execution Records & Workspace Artifacts Tools.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

use voxy_database::CommercialStore;

/// Retrieve persistent task execution records and status.
pub struct TaskHistoryGetTool {
    metadata: ToolMetadata,
    store: Option<Arc<CommercialStore>>,
}

impl TaskHistoryGetTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "task_history_get",
                "Retrieve durable task execution status, timeline, and error details",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["task_id"],
                "properties": {
                    "task_id": { "type": "string", "description": "Unique identifier of the task" }
                }
            })),
            store: None,
        }
    }

    pub fn with_store(mut self, store: Arc<CommercialStore>) -> Self {
        self.store = Some(store);
        self
    }
}

impl Default for TaskHistoryGetTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskHistoryGetTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'task_id'".into()))?;

        if let Some(ref store) = self.store {
            let task = store
                .get_task(task_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Database query failed: {}", e)))?;

            if let Some(t) = task {
                Ok(ToolResult::success(json!({
                    "id": t.id,
                    "session_id": t.session_id,
                    "workspace_id": t.workspace_id,
                    "user_goal": t.user_goal,
                    "task_type": t.task_type,
                    "status": t.status.as_str(),
                    "phase": t.phase,
                    "progress": t.progress,
                    "error_message": t.error_message,
                    "result_summary": t.result_summary,
                    "created_at": t.created_at,
                    "started_at": t.started_at,
                    "ended_at": t.ended_at,
                }))
                .with_observation(format!("Retrieved task '{}' in state {:?}", t.id, t.status))
                .with_duration_ms(start.elapsed().as_millis() as u64))
            } else {
                Err(ToolError::ExecutionFailed(format!(
                    "Task '{}' not found",
                    task_id
                )))
            }
        } else {
            // Standalone fallback when store is not injected
            Ok(ToolResult::success(json!({
                "task_id": task_id,
                "status": "QUEUED",
                "simulated": true,
            }))
            .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Retrieve artifacts linked to a specific task.
pub struct TaskArtifactsListTool {
    metadata: ToolMetadata,
    store: Option<Arc<CommercialStore>>,
}

impl TaskArtifactsListTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "task_artifacts_list",
                "List generated patches, research reports, and files associated with a task",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["task_id"],
                "properties": {
                    "task_id": { "type": "string" }
                }
            })),
            store: None,
        }
    }

    pub fn with_store(mut self, store: Arc<CommercialStore>) -> Self {
        self.store = Some(store);
        self
    }
}

impl Default for TaskArtifactsListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TaskArtifactsListTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let task_id = params
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'task_id'".into()))?;

        if let Some(ref store) = self.store {
            let artifacts = store
                .get_artifacts_for_task(task_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Database query failed: {}", e)))?;

            let count = artifacts.len();
            Ok(ToolResult::success(json!({
                "task_id": task_id,
                "count": count,
                "artifacts": artifacts,
            }))
            .with_observation(format!("Found {} artifacts for task '{}'", count, task_id))
            .with_duration_ms(start.elapsed().as_millis() as u64))
        } else {
            Ok(ToolResult::success(json!({
                "task_id": task_id,
                "count": 0,
                "artifacts": [],
                "simulated": true,
            }))
            .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}
