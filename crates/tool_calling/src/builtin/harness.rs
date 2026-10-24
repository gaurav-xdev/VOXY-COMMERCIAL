//! Autonomous coding harness tools: repo indexing, symbol search, sandboxed execution, and safe patching.

use async_trait::async_trait;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

use voxy_harness::{PatchEngine, RepositoryIndexer, SandboxedRunner};

/// Index repository structure and symbols.
pub struct HarnessIndexTool {
    metadata: ToolMetadata,
}

impl HarnessIndexTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "harness_index_repo",
                "Index repository files and symbols to construct a semantic project map",
                ToolCategory::Coding,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "root_path": { "type": "string", "description": "Optional repository root path" }
                }
            })),
        }
    }
}

impl Default for HarnessIndexTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HarnessIndexTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let root = resolve_repo_root(&params, ctx);

        let mut indexer = RepositoryIndexer::new(&root);
        let file_count = indexer
            .index_repository()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to index repo: {}", e)))?;

        Ok(ToolResult::success(json!({
            "root": root,
            "file_count": file_count,
        }))
        .with_observation(format!("Indexed repo {:?}: {} files", root, file_count))
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Search indexed symbols (functions, structs, traits) in the repository.
pub struct HarnessSearchTool {
    metadata: ToolMetadata,
}

impl HarnessSearchTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "harness_search_symbols",
                "Search symbols (functions, structs, traits) across indexed code",
                ToolCategory::Coding,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": { "type": "string", "description": "Symbol name or substring to search for" },
                    "root_path": { "type": "string" }
                }
            })),
        }
    }
}

impl Default for HarnessSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HarnessSearchTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'query'".into()))?;

        let root = resolve_repo_root(&params, ctx);
        let mut indexer = RepositoryIndexer::new(&root);
        indexer
            .index_repository()
            .map_err(|e| ToolError::ExecutionFailed(format!("Indexing failed: {}", e)))?;

        let matches = indexer.find_symbols(query);
        let count = matches.len();

        Ok(ToolResult::success(json!({
            "query": query,
            "count": count,
            "matches": matches,
        }))
        .with_observation(format!("Found {} symbols matching '{}'", count, query))
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Run a sandboxed terminal/build/test command within the repository.
pub struct HarnessRunCommandTool {
    metadata: ToolMetadata,
}

impl HarnessRunCommandTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "harness_run_command",
                "Execute a sandboxed build, test, or terminal command strictly bounded by repository root",
                ToolCategory::Coding,
                RiskTier::Privileged,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["program", "args"],
                "properties": {
                    "program": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "relative_cwd": { "type": "string" },
                    "timeout_secs": { "type": "integer", "default": 60 }
                }
            }))
            .with_confirmation(true),
        }
    }
}

impl Default for HarnessRunCommandTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HarnessRunCommandTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let program = params
            .get("program")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'program'".into()))?;

        let args_val = params
            .get("args")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError::InvalidParams("Missing or invalid 'args' array".into()))?;

        let args_strings: Vec<String> = args_val
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        let args_refs: Vec<&str> = args_strings.iter().map(|s| s.as_str()).collect();

        let rel_cwd = params
            .get("relative_cwd")
            .and_then(|v| v.as_str())
            .map(Path::new);

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);

        let root = resolve_repo_root(&params, ctx);
        let runner = SandboxedRunner::new(&root);

        let output = runner
            .run_command(
                program,
                &args_refs,
                rel_cwd,
                Duration::from_secs(timeout_secs),
            )
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Command runner failed: {}", e)))?;

        let status_desc = if output.was_success {
            "success"
        } else {
            "non-zero exit"
        };

        Ok(ToolResult::success(json!({
            "program": program,
            "exit_code": output.exit_code,
            "was_success": output.was_success,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "duration_ms": output.duration.as_millis(),
        }))
        .with_observation(format!(
            "Ran '{}' -> {} in {}ms",
            program,
            status_desc,
            output.duration.as_millis()
        ))
        .with_verification(if output.was_success {
            "Process exited with status 0"
        } else {
            "Process returned non-zero exit code"
        })
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Snapshot for patch rollback.
#[derive(Debug, Clone)]
pub struct PatchRollbackSnapshot {
    pub id: String,
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub file_path: PathBuf,
    pub existed: bool,
    pub original_content: String,
    pub created_at: Instant,
    pub rolled_back: bool,
}

/// In-memory bounded rollback store for patch modifications.
pub struct PatchRollbackStore {
    entries: RwLock<HashMap<String, PatchRollbackSnapshot>>,
}

impl PatchRollbackStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn insert(&self, entry: PatchRollbackSnapshot) {
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

    pub fn get(&self, id: &str) -> Option<PatchRollbackSnapshot> {
        self.entries.read().get(id).cloned()
    }

    pub fn mark_rolled_back(&self, id: &str) {
        if let Some(entry) = self.entries.write().get_mut(id) {
            entry.rolled_back = true;
        }
    }
}

impl Default for PatchRollbackStore {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_PATCH_ROLLBACK_STORE: OnceLock<Arc<PatchRollbackStore>> = OnceLock::new();

pub fn get_patch_rollback_store() -> Arc<PatchRollbackStore> {
    GLOBAL_PATCH_ROLLBACK_STORE
        .get_or_init(|| Arc::new(PatchRollbackStore::new()))
        .clone()
}

/// Apply a patch to repository files with automatic snapshot rollback.
pub struct HarnessApplyPatchTool {
    metadata: ToolMetadata,
    rollback_store: Arc<PatchRollbackStore>,
}

impl HarnessApplyPatchTool {
    pub fn new() -> Self {
        Self::with_rollback_store(get_patch_rollback_store())
    }

    pub fn with_rollback_store(store: Arc<PatchRollbackStore>) -> Self {
        Self {
            metadata: ToolMetadata::new(
                "harness_apply_patch",
                "Apply structured code modifications with automatic snapshot rollback support",
                ToolCategory::Coding,
                RiskTier::Modify,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["file_path", "modified_content"],
                "properties": {
                    "file_path": { "type": "string" },
                    "modified_content": { "type": "string" }
                }
            }))
            .with_rollback(true),
            rollback_store: store,
        }
    }

    pub fn rollback_store(&self) -> Arc<PatchRollbackStore> {
        Arc::clone(&self.rollback_store)
    }
}

impl Default for HarnessApplyPatchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HarnessApplyPatchTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let file_path = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'file_path'".into()))?;

        let modified_content = params
            .get("modified_content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'modified_content'".into()))?;

        let root = resolve_repo_root(&params, ctx);
        let full_path = root.join(file_path);

        // Pre-validate path containment and protected files
        voxy_harness::validate_path(&root, Path::new(file_path))
            .map_err(|e| ToolError::SecurityViolation(format!("Path validation failed: {}", e)))?;

        // Read pre-modification snapshot
        let (existed, original_content) = if full_path.exists() {
            (
                true,
                std::fs::read_to_string(&full_path).map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to read target file {:?}: {}",
                        full_path, e
                    ))
                })?,
            )
        } else {
            (false, String::new())
        };

        let token = format!("patch_rb_{}", uuid::Uuid::new_v4().simple());
        let mut patch_engine = PatchEngine::new(&root);

        let diff = patch_engine
            .apply_modification(Path::new(file_path), modified_content)
            .map_err(|e| ToolError::ExecutionFailed(format!("Patch engine error: {}", e)))?;

        self.rollback_store.insert(PatchRollbackSnapshot {
            id: token.clone(),
            session_id: ctx.session_id.clone(),
            workspace_root: root.canonicalize().unwrap_or(root.clone()),
            file_path: full_path,
            existed,
            original_content,
            created_at: Instant::now(),
            rolled_back: false,
        });

        Ok(ToolResult::success(json!({
            "file_path": file_path,
            "unified_diff": diff,
        }))
        .with_observation(format!("Applied patch to {:?}", file_path))
        .with_verification("Verified patch written and snapshot recorded")
        .with_rollback_token(token)
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }

    async fn rollback(&self, token: &str) -> Result<()> {
        let entry = self.rollback_store.get(token).ok_or_else(|| {
            ToolError::ExecutionFailed(format!(
                "Rollback failed: unknown or expired patch rollback token '{}'",
                token
            ))
        })?;

        // Re-validate containment at rollback time
        if !voxy_harness::path_is_contained(&entry.file_path, &entry.workspace_root) {
            return Err(ToolError::SecurityViolation(format!(
                "Rollback target {:?} escapes workspace root {:?}",
                entry.file_path, entry.workspace_root
            )));
        }

        if entry.rolled_back {
            return Ok(());
        }

        if entry.existed {
            tokio::fs::write(&entry.file_path, &entry.original_content)
                .await
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to restore original file content during patch rollback {:?}: {}",
                        entry.file_path, e
                    ))
                })?;
        } else if tokio::fs::try_exists(&entry.file_path).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to stat {:?}: {}", entry.file_path, e))
        })? {
            tokio::fs::remove_file(&entry.file_path)
                .await
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to remove created file during patch rollback {:?}: {}",
                        entry.file_path, e
                    ))
                })?;
        }

        self.rollback_store.mark_rolled_back(token);
        Ok(())
    }
}

/// Parse compiler or test output into structured diagnostics.
pub struct HarnessParseDiagnosticsTool {
    metadata: ToolMetadata,
}

impl HarnessParseDiagnosticsTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "harness_parse_diagnostics",
                "Parse build output, compiler errors, or test tracebacks into structured diagnostics",
                ToolCategory::Coding,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["output"],
                "properties": {
                    "output": { "type": "string", "description": "Raw terminal output from cargo, tsc, pytest, etc." }
                }
            })),
        }
    }
}

impl Default for HarnessParseDiagnosticsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HarnessParseDiagnosticsTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let output = params
            .get("output")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'output'".into()))?;

        let report = voxy_harness::DiagnosticParser::parse(output);

        Ok(ToolResult::success(json!({
            "success": report.success,
            "total_errors": report.total_errors,
            "total_warnings": report.total_warnings,
            "diagnostics": report.items,
        }))
        .with_observation(format!(
            "Parsed diagnostics: {} errors, {} warnings",
            report.total_errors, report.total_warnings
        ))
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

#[derive(Debug, Clone)]
pub struct TransactionRollbackSnapshot {
    pub id: String,
    pub session_id: String,
    pub workspace_root: PathBuf,
    pub files: Vec<(PathBuf, bool, String)>,
    pub created_at: Instant,
    pub rolled_back: bool,
}

pub struct TransactionRollbackStore {
    entries: RwLock<HashMap<String, TransactionRollbackSnapshot>>,
}

impl TransactionRollbackStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn insert(&self, entry: TransactionRollbackSnapshot) {
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

    pub fn get(&self, id: &str) -> Option<TransactionRollbackSnapshot> {
        self.entries.read().get(id).cloned()
    }

    pub fn mark_rolled_back(&self, id: &str) {
        if let Some(entry) = self.entries.write().get_mut(id) {
            entry.rolled_back = true;
        }
    }
}

static GLOBAL_TX_ROLLBACK_STORE: OnceLock<Arc<TransactionRollbackStore>> = OnceLock::new();

pub fn get_tx_rollback_store() -> Arc<TransactionRollbackStore> {
    GLOBAL_TX_ROLLBACK_STORE
        .get_or_init(|| Arc::new(TransactionRollbackStore::new()))
        .clone()
}

/// Apply multiple file patches in an atomic transaction with rollback and secret checks.
pub struct HarnessApplyPatchTransactionTool {
    metadata: ToolMetadata,
    rollback_store: Arc<TransactionRollbackStore>,
}

impl HarnessApplyPatchTransactionTool {
    pub fn new() -> Self {
        Self::with_rollback_store(get_tx_rollback_store())
    }

    pub fn with_rollback_store(store: Arc<TransactionRollbackStore>) -> Self {
        Self {
            metadata: ToolMetadata::new(
                "harness_apply_patch_transaction",
                "Apply multi-file atomic patches with pre-flight secret scanning and automatic rollback on failure",
                ToolCategory::Coding,
                RiskTier::Modify,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["patches"],
                "properties": {
                    "patches": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["file_path", "modified_content"],
                            "properties": {
                                "file_path": { "type": "string" },
                                "modified_content": { "type": "string" }
                            }
                        }
                    },
                    "root_path": { "type": "string" }
                }
            }))
            .with_rollback(true)
            .with_confirmation(true),
            rollback_store: store,
        }
    }

    pub fn rollback_store(&self) -> Arc<TransactionRollbackStore> {
        Arc::clone(&self.rollback_store)
    }
}

impl Default for HarnessApplyPatchTransactionTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HarnessApplyPatchTransactionTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let patches_val = params
            .get("patches")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ToolError::InvalidParams("Missing or invalid 'patches' array".into()))?;

        let root = resolve_repo_root(&params, ctx);
        let mut tx = voxy_harness::PatchTransaction::new(&root);
        let mut pre_snapshots = Vec::new();

        for patch_item in patches_val {
            let path_str = patch_item
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidParams("Patch item missing 'file_path'".into()))?;
            let content = patch_item
                .get("modified_content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ToolError::InvalidParams("Patch item missing 'modified_content'".into())
                })?;

            let full = root.join(path_str);
            let (existed, orig) = if full.exists() {
                (true, std::fs::read_to_string(&full).unwrap_or_default())
            } else {
                (false, String::new())
            };
            pre_snapshots.push((full, existed, orig));

            tx.add_change(path_str, content);
        }

        let diffs = tx.apply().map_err(|e| {
            ToolError::ExecutionFailed(format!("Patch transaction rejected: {}", e))
        })?;

        // Commit transaction to lock in changes
        tx.commit();

        let token = format!("patch_tx_rb_{}", uuid::Uuid::new_v4().simple());
        self.rollback_store.insert(TransactionRollbackSnapshot {
            id: token.clone(),
            session_id: ctx.session_id.clone(),
            workspace_root: root.canonicalize().unwrap_or(root),
            files: pre_snapshots,
            created_at: Instant::now(),
            rolled_back: false,
        });

        let count = diffs.len();
        Ok(ToolResult::success(json!({
            "files_modified": count,
            "diffs": diffs,
        }))
        .with_observation(format!("Atomically applied {} file patches", count))
        .with_verification("All files modified cleanly with boundary and secret validation")
        .with_rollback_token(token)
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }

    async fn rollback(&self, token: &str) -> Result<()> {
        let entry = self.rollback_store.get(token).ok_or_else(|| {
            ToolError::ExecutionFailed(format!(
                "Rollback failed: unknown or expired transaction rollback token '{}'",
                token
            ))
        })?;

        if entry.rolled_back {
            return Ok(());
        }

        for (path, existed, original) in &entry.files {
            if !voxy_harness::path_is_contained(path, &entry.workspace_root) {
                return Err(ToolError::SecurityViolation(format!(
                    "Rollback target {:?} escapes workspace root {:?}",
                    path, entry.workspace_root
                )));
            }

            if *existed {
                tokio::fs::write(path, original).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to restore original file content during transaction rollback {:?}: {}",
                        path, e
                    ))
                })?;
            } else if tokio::fs::try_exists(path).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to stat {:?}: {}", path, e))
            })? {
                tokio::fs::remove_file(path).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!(
                        "Failed to remove created file during transaction rollback {:?}: {}",
                        path, e
                    ))
                })?;
            }
        }

        self.rollback_store.mark_rolled_back(token);
        Ok(())
    }
}

fn resolve_repo_root(params: &serde_json::Value, ctx: &ToolContext) -> PathBuf {
    if let Some(r) = params.get("root_path").and_then(|v| v.as_str()) {
        PathBuf::from(r)
    } else if let Some(ref cwd) = ctx.working_dir {
        cwd.clone()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_harness_apply_patch_tool_and_rollback() {
        let temp_dir =
            std::env::temp_dir().join(format!("voxy_patch_test_{}", uuid::Uuid::new_v4().simple()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let sample_file = temp_dir.join("code.rs");
        tokio::fs::write(
            &sample_file,
            "fn main() {\n    println!(\"original\");\n}\n",
        )
        .await
        .unwrap();

        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let patch_tool = HarnessApplyPatchTool::new();

        // 1. Apply patch
        let res = patch_tool
            .execute(
                json!({
                    "file_path": "code.rs",
                    "modified_content": "fn main() {\n    println!(\"patched\");\n}\n"
                }),
                &ctx,
            )
            .await
            .expect("Patch application should succeed");

        assert!(res.success);
        let token = res.rollback_token.expect("Must have rollback token");
        let content_after_patch = tokio::fs::read_to_string(&sample_file).await.unwrap();
        assert!(content_after_patch.contains("patched"));

        // 2. Rollback with valid token
        patch_tool
            .rollback(&token)
            .await
            .expect("Rollback should succeed");
        let content_after_rb = tokio::fs::read_to_string(&sample_file).await.unwrap();
        assert_eq!(
            content_after_rb,
            "fn main() {\n    println!(\"original\");\n}\n"
        );

        // 3. Repeated rollback is idempotent
        patch_tool
            .rollback(&token)
            .await
            .expect("Repeated rollback is idempotent");

        // 4. Forged token is rejected with error
        let forged_err = patch_tool
            .rollback("patch_rb_fake123")
            .await
            .expect_err("Forged token fails");
        assert!(matches!(forged_err, ToolError::ExecutionFailed(_)));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_harness_apply_patch_transaction_tool_and_rollback() {
        let temp_dir = std::env::temp_dir().join(format!(
            "voxy_patch_tx_test_{}",
            uuid::Uuid::new_v4().simple()
        ));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let f1 = temp_dir.join("a.txt");
        let f2 = temp_dir.join("b.txt");
        tokio::fs::write(&f1, "Original A").await.unwrap();
        tokio::fs::write(&f2, "Original B").await.unwrap();

        let ctx = ToolContext::new("test-session").with_working_dir(temp_dir.clone());
        let tx_tool = HarnessApplyPatchTransactionTool::new();

        let res = tx_tool
            .execute(
                json!({
                    "patches": [
                        { "file_path": "a.txt", "modified_content": "Modified A" },
                        { "file_path": "b.txt", "modified_content": "Modified B" }
                    ]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(res.success);
        let token = res
            .rollback_token
            .expect("Must have transaction rollback token");

        assert_eq!(tokio::fs::read_to_string(&f1).await.unwrap(), "Modified A");
        assert_eq!(tokio::fs::read_to_string(&f2).await.unwrap(), "Modified B");

        // Rollback
        tx_tool.rollback(&token).await.unwrap();

        assert_eq!(tokio::fs::read_to_string(&f1).await.unwrap(), "Original A");
        assert_eq!(tokio::fs::read_to_string(&f2).await.unwrap(), "Original B");

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
