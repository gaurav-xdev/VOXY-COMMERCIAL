//! Filesystem inspection, reading, safe writing, and deletion tools.

use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

#[cfg(windows)]
use voxy_platform_core::traits::FileSystemPlatform;
#[cfg(windows)]
use voxy_platform_windows::WindowsPlatform;

/// List files and folders in a target directory.
pub struct FileListTool {
    metadata: ToolMetadata,
}

impl FileListTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "file_list",
                "List contents of a directory with file names, sizes, and timestamps",
                ToolCategory::Filesystem,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Absolute or workspace-relative directory path" }
                }
            })),
        }
    }
}

impl Default for FileListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileListTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let raw_path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;

        let resolved_path = resolve_path(raw_path, ctx);

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            let entries = platform
                .list_dir(&resolved_path.to_string_lossy())
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            let count = entries.len();
            Ok(ToolResult::success(
                json!({ "entries": entries, "count": count, "path": resolved_path }),
            )
            .with_observation(format!("Found {} entries in {:?}", count, resolved_path))
            .with_duration_ms(start.elapsed().as_millis() as u64))
        }
        #[cfg(not(windows))]
        {
            let _ = resolved_path;
            Ok(ToolResult::success(json!({ "entries": [], "count": 0 }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Read text file content safely with size limits.
pub struct FileReadTool {
    metadata: ToolMetadata,
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "file_read",
                "Read text content of a file (up to 1MB)",
                ToolCategory::Filesystem,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Path of file to read" },
                    "max_bytes": { "type": "integer", "default": 1048576 }
                }
            })),
        }
    }
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let raw_path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;

        let max_bytes = params
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(1024 * 1024) as usize;

        let resolved_path = resolve_path(raw_path, ctx);

        let meta = tokio::fs::metadata(&resolved_path).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to stat {:?}: {}", resolved_path, e))
        })?;

        if meta.len() as usize > max_bytes {
            return Err(ToolError::ExecutionFailed(format!(
                "File size ({} bytes) exceeds limit ({} bytes)",
                meta.len(),
                max_bytes
            )));
        }

        let content = tokio::fs::read_to_string(&resolved_path)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to read {:?}: {}", resolved_path, e))
            })?;

        let line_count = content.lines().count();
        Ok(ToolResult::success(json!({
            "path": resolved_path,
            "content": content,
            "bytes": meta.len(),
            "lines": line_count,
        }))
        .with_observation(format!(
            "Read {} lines ({} bytes) from {:?}",
            line_count,
            meta.len(),
            resolved_path
        ))
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Safely write content to a file, capturing prior state for rollback.
pub struct FileWriteTool {
    metadata: ToolMetadata,
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "file_write",
                "Write or overwrite text content to a file with rollback support",
                ToolCategory::Filesystem,
                RiskTier::Modify,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": { "type": "string", "description": "Target file path" },
                    "content": { "type": "string", "description": "Text content to write" }
                }
            }))
            .with_rollback(true),
        }
    }
}

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let raw_path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;

        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'content'".into()))?;

        let resolved_path = resolve_path(raw_path, ctx);

        // Capture previous content for rollback if file exists
        let rollback_token = if tokio::fs::try_exists(&resolved_path).await.unwrap_or(false) {
            let prior = tokio::fs::read_to_string(&resolved_path).await.ok();
            prior.map(|p| format!("prior_content:{}:{}", resolved_path.display(), p))
        } else {
            Some(format!("prior_absent:{}", resolved_path.display()))
        };

        // Ensure parent directory exists
        if let Some(parent) = resolved_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to create parent dir: {}", e))
            })?;
        }

        tokio::fs::write(&resolved_path, content)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to write {:?}: {}", resolved_path, e))
            })?;

        // Post-verification check
        let written_len = tokio::fs::metadata(&resolved_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);

        let mut res = ToolResult::success(json!({
            "path": resolved_path,
            "bytes_written": content.len(),
            "verified_size": written_len,
        }))
        .with_observation(format!(
            "Wrote {} bytes to {:?}",
            content.len(),
            resolved_path
        ))
        .with_verification(if written_len == content.len() as u64 {
            "Verified file size on disk matches written bytes"
        } else {
            "Warning: size on disk mismatch"
        })
        .with_duration_ms(start.elapsed().as_millis() as u64);

        if let Some(token) = rollback_token {
            res = res.with_rollback_token(token);
        }

        Ok(res)
    }

    async fn rollback(&self, token: &str) -> Result<()> {
        if let Some(path_str) = token.strip_prefix("prior_absent:") {
            let p = Path::new(path_str);
            if tokio::fs::try_exists(p).await.unwrap_or(false) {
                let _ = tokio::fs::remove_file(p).await;
            }
        } else if let Some(rest) = token.strip_prefix("prior_content:") {
            if let Some((path_str, prior_content)) = rest.split_once(':') {
                let p = Path::new(path_str);
                let _ = tokio::fs::write(p, prior_content).await;
            }
        }
        Ok(())
    }
}

/// Delete a file. Requires user confirmation.
pub struct FileDeleteTool {
    metadata: ToolMetadata,
}

impl FileDeleteTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "file_delete",
                "Delete a target file (Destructive — requires confirmation)",
                ToolCategory::Filesystem,
                RiskTier::Destructive,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "Path to delete" }
                }
            }))
            .with_confirmation(true),
        }
    }
}

impl Default for FileDeleteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileDeleteTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let raw_path = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;

        let resolved_path = resolve_path(raw_path, ctx);

        if !ctx.user_confirmed {
            return Err(ToolError::ConfirmationRequired(format!(
                "Deleting {:?} requires explicit user confirmation",
                resolved_path
            )));
        }

        tokio::fs::remove_file(&resolved_path).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to delete {:?}: {}", resolved_path, e))
        })?;

        let exists = tokio::fs::try_exists(&resolved_path).await.unwrap_or(true);
        let verified = !exists;

        Ok(
            ToolResult::success(json!({ "path": resolved_path, "deleted": true }))
                .with_observation(format!("Deleted file {:?}", resolved_path))
                .with_verification(if verified {
                    "Verified file no longer exists"
                } else {
                    "Warning: file still detected after deletion"
                })
                .with_duration_ms(start.elapsed().as_millis() as u64),
        )
    }
}

fn resolve_path(raw: &str, ctx: &ToolContext) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else if let Some(ref cwd) = ctx.working_dir {
        cwd.join(p)
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}
