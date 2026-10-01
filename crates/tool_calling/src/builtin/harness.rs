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

/// Apply a patch to repository files with automatic snapshot rollback.
pub struct HarnessApplyPatchTool {
    metadata: ToolMetadata,
}

impl HarnessApplyPatchTool {
    pub fn new() -> Self {
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
        }
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
        let mut patch_engine = PatchEngine::new(&root);

        let diff = patch_engine
            .apply_modification(Path::new(file_path), modified_content)
            .map_err(|e| ToolError::ExecutionFailed(format!("Patch engine error: {}", e)))?;

        Ok(ToolResult::success(json!({
            "file_path": file_path,
            "unified_diff": diff,
        }))
        .with_observation(format!("Applied patch to {:?}", file_path))
        .with_verification("Verified patch written and snapshot recorded")
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }

    async fn rollback(&self, _token: &str) -> Result<()> {
        let mut engine =
            PatchEngine::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let _ = engine.rollback_all();
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
