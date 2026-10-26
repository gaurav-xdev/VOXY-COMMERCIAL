//! Filesystem inspection, reading, safe writing, and deletion tools.

use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

use voxy_security::path_guard::{
    is_protected_file, is_symlink_or_reparse_point, path_is_contained,
};

#[cfg(windows)]
use voxy_platform_core::traits::FileSystemPlatform;
#[cfg(windows)]
use voxy_platform_windows::WindowsPlatform;

/// Security-reviewed path resolution and workspace containment enforcement.
///
/// Rejects:
/// - Directory traversal ('..')
/// - Absolute paths escaping the workspace root
/// - Windows device namespaces (`\\.\`, `\\?\`, etc.)
/// - UNC network shares
/// - Sibling directories with matching prefixes (e.g. `C:\workspace-evil` vs `C:\workspace`)
/// - Symlinks and reparse points escaping containment
/// - Protected system or configuration files (.git, .env, cargo.lock, keys)
pub fn resolve_and_validate_path(raw: &str, ctx: &ToolContext) -> Result<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ToolError::InvalidParams(
            "Target path cannot be empty".into(),
        ));
    }
    if trimmed.contains('\0') {
        return Err(ToolError::SecurityViolation(
            "Path contains forbidden null byte".into(),
        ));
    }

    let lower_raw = trimmed.to_lowercase();
    if lower_raw.starts_with(r"\\.\")
        || lower_raw.starts_with(r"\\?\")
        || lower_raw.starts_with(r"\??\")
        || lower_raw.starts_with(r"//./")
        || lower_raw.starts_with(r"//?/")
    {
        return Err(ToolError::SecurityViolation(
            "Windows device namespace and verbatim paths are prohibited".into(),
        ));
    }
    if trimmed.starts_with(r"\\") || trimmed.starts_with("//") {
        return Err(ToolError::SecurityViolation(
            "UNC network paths are prohibited".into(),
        ));
    }

    let raw_path = Path::new(trimmed);
    for comp in raw_path.components() {
        match comp {
            std::path::Component::ParentDir => {
                return Err(ToolError::SecurityViolation(
                    "Parent directory traversal ('..') is prohibited".into(),
                ));
            }
            std::path::Component::Prefix(prefix) => match prefix.kind() {
                std::path::Prefix::Verbatim(_)
                | std::path::Prefix::VerbatimUNC(_, _)
                | std::path::Prefix::VerbatimDisk(_)
                | std::path::Prefix::DeviceNS(_)
                | std::path::Prefix::UNC(_, _) => {
                    return Err(ToolError::SecurityViolation(
                        "Device and UNC path prefixes are prohibited".into(),
                    ));
                }
                std::path::Prefix::Disk(_) => {}
            },
            _ => {}
        }
    }

    let root = match &ctx.working_dir {
        Some(d) => d.clone(),
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    let canon_root = root.canonicalize().map_err(|e| {
        ToolError::ExecutionFailed(format!("Invalid workspace root {:?}: {}", root, e))
    })?;

    let candidate = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        canon_root.join(raw_path)
    };

    if is_protected_file(&candidate) {
        return Err(ToolError::SecurityViolation(format!(
            "Target path {:?} is a protected system or configuration file",
            candidate
        )));
    }

    if candidate.exists() {
        let canon_target = candidate.canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to canonicalize {:?}: {}", candidate, e))
        })?;

        if !path_is_contained(&canon_target, &canon_root) {
            return Err(ToolError::SecurityViolation(format!(
                "Path {:?} escapes workspace boundary {:?}",
                candidate, canon_root
            )));
        }

        if is_symlink_or_reparse_point(&candidate) || is_symlink_or_reparse_point(&canon_target) {
            return Err(ToolError::SecurityViolation(
                "Symlinks and reparse points are prohibited".into(),
            ));
        }

        Ok(canon_target)
    } else {
        let mut ancestor = candidate.clone();
        while !ancestor.exists() {
            if let Some(parent) = ancestor.parent() {
                ancestor = parent.to_path_buf();
            } else {
                break;
            }
        }

        let canon_ancestor = ancestor.canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to resolve ancestor {:?}: {}", ancestor, e))
        })?;

        if !path_is_contained(&canon_ancestor, &canon_root) {
            return Err(ToolError::SecurityViolation(format!(
                "Target path {:?} escapes workspace boundary {:?}",
                candidate, canon_root
            )));
        }

        if !path_is_contained(&candidate, &canon_root) {
            return Err(ToolError::SecurityViolation(format!(
                "Target path {:?} escapes workspace boundary {:?}",
                candidate, canon_root
            )));
        }

        if is_symlink_or_reparse_point(&ancestor) || is_symlink_or_reparse_point(&canon_ancestor) {
            return Err(ToolError::SecurityViolation(
                "Ancestor path contains a symlink or reparse point".into(),
            ));
        }

        Ok(candidate)
    }
}

/// Snapshot recorded in the trusted rollback store.
#[derive(Debug, Clone)]
pub struct FileRollbackEntry {
    pub id: String,
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub target_path: PathBuf,
    pub existed: bool,
    pub original_content: Option<String>,
    pub created_at: Instant,
    pub rolled_back: bool,
}

/// In-memory bounded rollback store for file modifications.
pub struct FileRollbackStore {
    entries: RwLock<HashMap<String, FileRollbackEntry>>,
}

impl FileRollbackStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn insert(&self, entry: FileRollbackEntry) {
        let mut map = self.entries.write();
        if map.len() >= 1000 {
            let mut items: Vec<_> = map.iter().map(|(k, v)| (k.clone(), v.created_at)).collect();
            items.sort_by_key(|(_, t)| *t);
            for (k, _) in items.iter().take(200) {
                map.remove(k);
            }
        }
        map.insert(entry.id.clone(), entry);
    }

    pub fn get(&self, id: &str) -> Option<FileRollbackEntry> {
        self.entries.read().get(id).cloned()
    }

    pub fn mark_rolled_back(&self, id: &str) {
        if let Some(entry) = self.entries.write().get_mut(id) {
            entry.rolled_back = true;
        }
    }
}

impl Default for FileRollbackStore {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_FILE_ROLLBACK_STORE: OnceLock<Arc<FileRollbackStore>> = OnceLock::new();

pub fn get_file_rollback_store() -> Arc<FileRollbackStore> {
    GLOBAL_FILE_ROLLBACK_STORE
        .get_or_init(|| Arc::new(FileRollbackStore::new()))
        .clone()
}

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

        let resolved_path = resolve_and_validate_path(raw_path, ctx)?;

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

        let resolved_path = resolve_and_validate_path(raw_path, ctx)?;

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
    rollback_store: Arc<FileRollbackStore>,
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self::with_rollback_store(get_file_rollback_store())
    }

    pub fn with_rollback_store(store: Arc<FileRollbackStore>) -> Self {
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
            rollback_store: store,
        }
    }

    pub fn rollback_store(&self) -> Arc<FileRollbackStore> {
        Arc::clone(&self.rollback_store)
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

        let resolved_path = resolve_and_validate_path(raw_path, ctx)?;

        // Capture previous content for rollback if file exists
        let (existed, original_content) =
            if tokio::fs::try_exists(&resolved_path).await.unwrap_or(false) {
                (true, tokio::fs::read_to_string(&resolved_path).await.ok())
            } else {
                (false, None)
            };

        let token = format!("fs_rb_{}", uuid::Uuid::new_v4().simple());
        let root = match &ctx.working_dir {
            Some(d) => d.clone(),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        let canon_root = root.canonicalize().unwrap_or(root);

        self.rollback_store.insert(FileRollbackEntry {
            id: token.clone(),
            session_id: ctx.session_id.clone(),
            workspace_root: canon_root,
            target_path: resolved_path.clone(),
            existed,
            original_content,
            created_at: Instant::now(),
            rolled_back: false,
        });

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

        let res = ToolResult::success(json!({
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
        .with_rollback_token(token)
        .with_duration_ms(start.elapsed().as_millis() as u64);

        Ok(res)
    }

    async fn rollback(&self, token: &str) -> Result<()> {
        let entry = self.rollback_store.get(token).ok_or_else(|| {
            ToolError::ExecutionFailed(format!(
                "Rollback failed: unknown or expired token '{}'",
                token
            ))
        })?;

        // Re-validate containment at rollback time
        if !path_is_contained(&entry.target_path, &entry.workspace_root) {
            return Err(ToolError::SecurityViolation(format!(
                "Rollback target {:?} escapes workspace root {:?}",
                entry.target_path, entry.workspace_root
            )));
        }

        if entry.rolled_back {
            // Idempotent: already rolled back
            return Ok(());
        }

        if entry.existed {
            if let Some(ref orig) = entry.original_content {
                tokio::fs::write(&entry.target_path, orig)
                    .await
                    .map_err(|e| {
                        ToolError::ExecutionFailed(format!(
                            "Failed to restore original file content for {:?}: {}",
                            entry.target_path, e
                        ))
                    })?;
            } else {
                return Err(ToolError::ExecutionFailed(format!(
                    "Cannot restore content for {:?}: snapshot content missing",
                    entry.target_path
                )));
            }
        } else if tokio::fs::try_exists(&entry.target_path)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to check existence for {:?}: {}",
                    entry.target_path, e
                ))
            })?
        {
            tokio::fs::remove_file(&entry.target_path)
                .await
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to delete created file during rollback {:?}: {}",
                        entry.target_path, e
                    ))
                })?;
        }

        self.rollback_store.mark_rolled_back(token);
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

        let resolved_path = resolve_and_validate_path(raw_path, ctx)?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workspace_relative_write_and_read_succeeds() {
        let temp_dir =
            std::env::temp_dir().join(format!("voxy_fs_test_{}", uuid::Uuid::new_v4().simple()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let write_tool = FileWriteTool::new();
        let read_tool = FileReadTool::new();

        let write_res = write_tool
            .execute(
                json!({
                    "path": "test_nested/hello.txt",
                    "content": "Autonomous safe write"
                }),
                &ctx,
            )
            .await
            .expect("Workspace-relative write should succeed");

        assert!(write_res.success);
        let token = write_res.rollback_token.expect("Expected rollback token");

        let read_res = read_tool
            .execute(json!({ "path": "test_nested/hello.txt" }), &ctx)
            .await
            .expect("Workspace-relative read should succeed");

        assert_eq!(
            read_res.data.get("content").and_then(|v| v.as_str()),
            Some("Autonomous safe write")
        );

        // Rollback test
        write_tool
            .rollback(&token)
            .await
            .expect("Rollback should succeed");
        assert!(
            !tokio::fs::try_exists(temp_dir.join("test_nested/hello.txt"))
                .await
                .unwrap_or(true)
        );

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_traversal_escaping_workspace_is_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("voxy_fs_test_{}", uuid::Uuid::new_v4().simple()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let write_tool = FileWriteTool::new();

        let err = write_tool
            .execute(
                json!({
                    "path": "../evil.txt",
                    "content": "Malicious content"
                }),
                &ctx,
            )
            .await
            .expect_err("Traversal should be rejected");

        match err {
            ToolError::SecurityViolation(msg) => {
                assert!(msg.contains("Parent directory traversal") || msg.contains("escapes"));
            }
            other => panic!("Expected SecurityViolation, got: {:?}", other),
        }

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_absolute_external_path_is_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("voxy_fs_test_{}", uuid::Uuid::new_v4().simple()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let outside_dir =
            std::env::temp_dir().join(format!("voxy_fs_outside_{}", uuid::Uuid::new_v4().simple()));
        let _ = tokio::fs::create_dir_all(&outside_dir).await;

        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let write_tool = FileWriteTool::new();

        let target_outside = outside_dir.join("outside.txt");
        let err = write_tool
            .execute(
                json!({
                    "path": target_outside.to_string_lossy(),
                    "content": "Escaping write"
                }),
                &ctx,
            )
            .await
            .expect_err("Outside absolute path should be rejected");

        assert!(matches!(err, ToolError::SecurityViolation(_)));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        let _ = tokio::fs::remove_dir_all(&outside_dir).await;
    }

    #[tokio::test]
    async fn test_device_and_unc_paths_rejected() {
        let temp_dir =
            std::env::temp_dir().join(format!("voxy_fs_test_{}", uuid::Uuid::new_v4().simple()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;
        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let write_tool = FileWriteTool::new();

        let unc_err = write_tool
            .execute(
                json!({
                    "path": r"\\server\share\file.txt",
                    "content": "UNC"
                }),
                &ctx,
            )
            .await
            .expect_err("UNC path should be rejected");
        assert!(matches!(unc_err, ToolError::SecurityViolation(_)));

        let dev_err = write_tool
            .execute(
                json!({
                    "path": r"\\.\PhysicalDrive0",
                    "content": "Raw"
                }),
                &ctx,
            )
            .await
            .expect_err("Device path should be rejected");
        assert!(matches!(dev_err, ToolError::SecurityViolation(_)));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_rollback_forged_and_expired_tokens_rejected() {
        let write_tool = FileWriteTool::new();
        let err = write_tool
            .rollback("forged_token_12345")
            .await
            .expect_err("Forged token should fail");
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_rollback_restores_preexisting_file_bytes() {
        let temp_dir =
            std::env::temp_dir().join(format!("voxy_fs_test_{}", uuid::Uuid::new_v4().simple()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let target = temp_dir.join("existing.txt");
        tokio::fs::write(&target, "Original authentic content")
            .await
            .unwrap();

        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let write_tool = FileWriteTool::new();

        let res = write_tool
            .execute(
                json!({
                    "path": "existing.txt",
                    "content": "Overwritten modified content"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(
            tokio::fs::read_to_string(&target).await.unwrap(),
            "Overwritten modified content"
        );

        let token = res.rollback_token.expect("Must have token");
        write_tool.rollback(&token).await.unwrap();

        assert_eq!(
            tokio::fs::read_to_string(&target).await.unwrap(),
            "Original authentic content"
        );

        // Idempotency: second rollback succeeds without modifying
        write_tool.rollback(&token).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(&target).await.unwrap(),
            "Original authentic content"
        );

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
