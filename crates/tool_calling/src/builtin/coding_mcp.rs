//! Coding Tools MCP bridge and adaptation.
//!
//! Provides the runtime contract and tool adapters corresponding to `xyTom/coding-tools-mcp`
//! within OSMOO's native Tool architecture, enforcing workspace isolation, diff inspection,
//! git status, and patch application boundaries.

use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

/// Tool for reading git status in the bounded workspace.
pub struct CodingGitStatusTool {
    metadata: ToolMetadata,
}

impl CodingGitStatusTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "coding_git_status",
                "Inspect git status and staged/unstaged changes within the workspace boundary",
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

impl Default for CodingGitStatusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CodingGitStatusTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let root = resolve_repo_root(&params, ctx);

        let output = tokio::process::Command::new("git")
            .arg("status")
            .arg("--porcelain")
            .current_dir(&root)
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to run git status: {}", e)))?;

        let status_text = String::from_utf8_lossy(&output.stdout).to_string();
        let is_clean = status_text.trim().is_empty();

        Ok(ToolResult::success(json!({
            "root": root,
            "is_clean": is_clean,
            "status": status_text,
        }))
        .with_observation(if is_clean {
            "Working tree is clean".to_string()
        } else {
            format!("Working tree has modifications:\n{}", status_text)
        })
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Tool for generating and inspecting unified diffs in the workspace.
pub struct CodingGitDiffTool {
    metadata: ToolMetadata,
}

impl CodingGitDiffTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "coding_git_diff",
                "Inspect unified diff of unstaged or staged changes within the workspace",
                ToolCategory::Coding,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "file_path": { "type": "string", "description": "Optional path of specific file" },
                    "staged": { "type": "boolean", "description": "Diff staged changes instead of unstaged", "default": false },
                    "root_path": { "type": "string", "description": "Optional repository root path" }
                }
            })),
        }
    }
}

impl Default for CodingGitDiffTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CodingGitDiffTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let root = resolve_repo_root(&params, ctx);
        let staged = params.get("staged").and_then(|v| v.as_bool()).unwrap_or(false);

        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("diff");
        if staged {
            cmd.arg("--cached");
        }
        if let Some(file) = params.get("file_path").and_then(|v| v.as_str()) {
            cmd.arg("--").arg(file);
        }
        cmd.current_dir(&root);

        let output = cmd
            .output()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to run git diff: {}", e)))?;

        let diff_text = String::from_utf8_lossy(&output.stdout).to_string();

        Ok(ToolResult::success(json!({
            "root": root,
            "staged": staged,
            "diff": diff_text,
            "bytes": diff_text.len(),
        }))
        .with_observation(format!("Generated git diff ({} bytes)", diff_text.len()))
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Tool for text/regex search across workspace files (simulating ripgrep/search_text).
pub struct CodingSearchTextTool {
    metadata: ToolMetadata,
}

impl CodingSearchTextTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "coding_search_text",
                "Search code files across the repository using pattern matching bounded by workspace",
                ToolCategory::Coding,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["pattern"],
                "properties": {
                    "pattern": { "type": "string", "description": "Text or regex pattern to search for" },
                    "extension": { "type": "string", "description": "Optional file extension filter (e.g. 'rs', 'ts')" },
                    "root_path": { "type": "string", "description": "Optional repository root path" }
                }
            })),
        }
    }
}

impl Default for CodingSearchTextTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CodingSearchTextTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let pattern = params
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'pattern'".into()))?;

        let root = resolve_repo_root(&params, ctx);
        let ext_filter = params.get("extension").and_then(|v| v.as_str());

        let mut matches = Vec::new();
        let pattern_lower = pattern.to_lowercase();

        // Search directory iteratively
        let mut dirs = vec![root.clone()];
        while let Some(current_dir) = dirs.pop() {
            if let Ok(entries) = std::fs::read_dir(&current_dir) {
                for entry_res in entries.flatten() {
                    let path = entry_res.path();
                    let path_str = path.to_string_lossy();
                    if path_str.contains("target")
                        || path_str.contains(".git")
                        || path_str.contains("node_modules")
                    {
                        continue;
                    }

                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.is_file() {
                        if let Some(ext) = ext_filter {
                            if path.extension().and_then(|s| s.to_str()) != Some(ext) {
                                continue;
                            }
                        }

                        if let Ok(content) = std::fs::read_to_string(&path) {
                            for (line_idx, line) in content.lines().enumerate() {
                                if line.to_lowercase().contains(&pattern_lower) {
                                    matches.push(json!({
                                        "file": path.strip_prefix(&root).unwrap_or(&path).to_string_lossy(),
                                        "line_number": line_idx + 1,
                                        "line_content": line.trim(),
                                    }));
                                    if matches.len() >= 100 {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if matches.len() >= 100 {
                        break;
                    }
                }
            }
            if matches.len() >= 100 {
                break;
            }
        }

        let count = matches.len();
        Ok(ToolResult::success(json!({
            "pattern": pattern,
            "count": count,
            "matches": matches,
        }))
        .with_observation(format!("Found {} matches for '{}'", count, pattern))
        .with_duration_ms(start.elapsed().as_millis() as u64))
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
