use crate::patcher::Patcher;
use crate::types::{FileReadInput, FileReadResult, HarnessError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepMatch {
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
}

pub struct ContextExplorer;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".osmoo",
    "__pycache__",
    ".venv",
    ".idea",
    ".vscode",
];

impl ContextExplorer {
    pub fn read_file(workspace_root: &Path, input: &FileReadInput) -> Result<FileReadResult> {
        let rel_path = Patcher::canonicalize_workspace_rel(workspace_root, &input.file_path)?;
        let full_path = workspace_root.join(&rel_path);

        if !full_path.exists() {
            return Err(HarnessError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", rel_path.display()),
            )));
        }

        let content = fs::read_to_string(&full_path).map_err(HarnessError::Io)?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let offset = input.offset.unwrap_or(1).saturating_sub(1);
        let limit = input.limit.unwrap_or(total_lines.saturating_sub(offset));

        let selected_lines: Vec<&str> = lines.into_iter().skip(offset).take(limit).collect();

        Ok(FileReadResult {
            file_path: rel_path.to_string_lossy().to_string(),
            content: selected_lines.join("\n"),
            total_lines,
            offset: offset + 1,
            limit,
        })
    }

    pub fn glob_files(
        workspace_root: &Path,
        pattern: &str,
        sub_path: Option<&str>,
    ) -> Result<Vec<String>> {
        let base_dir = if let Some(sp) = sub_path {
            let rel = Patcher::canonicalize_workspace_rel(workspace_root, sp)?;
            workspace_root.join(rel)
        } else {
            workspace_root.to_path_buf()
        };

        let regex_pattern = format!(
            "^{}$",
            pattern
                .replace('.', "\\.")
                .replace("**", ".*")
                .replace('*', "[^/]*")
                .replace('?', ".")
        );
        let re = Regex::new(&regex_pattern)
            .map_err(|e| HarnessError::PlanningError(format!("Invalid glob regex: {}", e)))?;

        let mut matched = Vec::new();
        for entry in WalkDir::new(&base_dir)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !IGNORED_DIRS.contains(&name.as_ref())
            })
            .flatten()
        {
            if entry.file_type().is_file() {
                let file_name = entry.file_name().to_string_lossy();
                if let Ok(rel) = entry.path().strip_prefix(workspace_root) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if re.is_match(&file_name) || re.is_match(&rel_str) {
                        matched.push(rel_str);
                    }
                }
            }
        }

        matched.sort();
        Ok(matched)
    }

    pub fn grep_pattern(
        workspace_root: &Path,
        pattern: &str,
        sub_path: Option<&str>,
        head_limit: Option<usize>,
    ) -> Result<Vec<GrepMatch>> {
        let base_dir = if let Some(sp) = sub_path {
            let rel = Patcher::canonicalize_workspace_rel(workspace_root, sp)?;
            workspace_root.join(rel)
        } else {
            workspace_root.to_path_buf()
        };

        let re = Regex::new(pattern)
            .map_err(|e| HarnessError::PlanningError(format!("Invalid regex pattern: {}", e)))?;

        let limit = head_limit.unwrap_or(250);
        let mut matches = Vec::new();

        for entry in WalkDir::new(&base_dir)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !IGNORED_DIRS.contains(&name.as_ref())
            })
            .flatten()
        {
            if entry.file_type().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    let rel_path = entry
                        .path()
                        .strip_prefix(workspace_root)
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_else(|_| entry.path().to_string_lossy().to_string());

                    for (idx, line) in content.lines().enumerate() {
                        if re.is_match(line) {
                            matches.push(GrepMatch {
                                file_path: rel_path.clone(),
                                line_number: idx + 1,
                                line_content: line.to_string(),
                            });
                            if matches.len() >= limit {
                                return Ok(matches);
                            }
                        }
                    }
                }
            }
        }

        Ok(matches)
    }
}
