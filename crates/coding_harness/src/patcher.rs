use crate::types::{FileEditInput, FileEditResult, HarnessError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use voxy_security::path_guard::{contains_symlink_or_reparse, is_protected_file, validate_path};

pub struct Patcher;

impl Patcher {
    pub fn canonicalize_workspace_rel(
        workspace_root: &Path,
        file_path_str: &str,
    ) -> Result<PathBuf> {
        let ws_canonical = workspace_root.canonicalize().map_err(|e| {
            HarnessError::PathViolation(format!("Cannot canonicalize workspace: {}", e))
        })?;

        let raw_path = PathBuf::from(file_path_str);
        let rel_path = if raw_path.is_absolute() {
            match raw_path.canonicalize() {
                Ok(abs_canon) => abs_canon
                    .strip_prefix(&ws_canonical)
                    .map_err(|_| {
                        HarnessError::PathViolation(format!(
                            "Target path '{}' is outside workspace '{}'",
                            file_path_str,
                            ws_canonical.display()
                        ))
                    })?
                    .to_path_buf(),
                Err(_) => {
                    // Path does not exist yet (e.g. creating new file)
                    raw_path
                        .strip_prefix(&ws_canonical)
                        .map_err(|_| {
                            HarnessError::PathViolation(format!(
                                "Target path '{}' does not reside within workspace '{}'",
                                file_path_str,
                                ws_canonical.display()
                            ))
                        })?
                        .to_path_buf()
                }
            }
        } else {
            raw_path
        };

        // Guard against traversal sequences
        if rel_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(HarnessError::PathViolation(
                "Path contains '..' traversal".into(),
            ));
        }

        // Validate relative path using voxy_security::path_guard
        validate_path(&ws_canonical, &rel_path).map_err(|e| {
            HarnessError::PathViolation(format!("Path guard validation failed: {}", e))
        })?;

        if is_protected_file(&rel_path) {
            return Err(HarnessError::Security(format!(
                "File '{}' is a protected system configuration file",
                rel_path.display()
            )));
        }

        let full_target = ws_canonical.join(&rel_path);
        if full_target.exists() && contains_symlink_or_reparse(&ws_canonical, &rel_path) {
            return Err(HarnessError::Security(format!(
                "File '{}' contains symlinks or reparse points",
                full_target.display()
            )));
        }

        Ok(rel_path)
    }

    pub fn apply_edit(workspace_root: &Path, input: &FileEditInput) -> Result<FileEditResult> {
        if input.old_string == input.new_string {
            return Err(HarnessError::EditError(
                "old_string and new_string must be different".into(),
            ));
        }

        let rel_path = Self::canonicalize_workspace_rel(workspace_root, &input.file_path)?;
        let full_path = workspace_root.join(&rel_path);

        if !full_path.exists() {
            return Err(HarnessError::EditError(format!(
                "Target file '{}' does not exist",
                full_path.display()
            )));
        }

        let original_content = fs::read_to_string(&full_path)
            .map_err(|e| HarnessError::EditError(format!("Failed to read target file: {}", e)))?;

        let count = original_content.matches(&input.old_string).count();
        if count == 0 {
            return Err(HarnessError::EditError(format!(
                "old_string was not found in '{}'",
                rel_path.display()
            )));
        }

        if count > 1 && !input.replace_all {
            return Err(HarnessError::EditError(format!(
                "old_string matches {} occurrences in '{}'. Provide more context or set replace_all: true",
                count,
                rel_path.display()
            )));
        }

        let new_content = if input.replace_all {
            original_content.replace(&input.old_string, &input.new_string)
        } else {
            // Replace single occurrence
            let mut result = String::with_capacity(original_content.len() + input.new_string.len());
            if let Some(pos) = original_content.find(&input.old_string) {
                result.push_str(&original_content[..pos]);
                result.push_str(&input.new_string);
                result.push_str(&original_content[pos + input.old_string.len()..]);
            }
            result
        };

        let diff = Self::compute_unified_diff(
            &rel_path.to_string_lossy(),
            &original_content,
            &new_content,
        );

        fs::write(&full_path, &new_content)
            .map_err(|e| HarnessError::EditError(format!("Failed to write updated file: {}", e)))?;

        Ok(FileEditResult {
            file_path: rel_path.to_string_lossy().to_string(),
            applied: true,
            occurrences: count,
            diff,
        })
    }

    pub fn compute_unified_diff(file_name: &str, old: &str, new: &str) -> String {
        let mut diff = String::new();
        diff.push_str(&format!("--- a/{}\n", file_name));
        diff.push_str(&format!("+++ b/{}\n", file_name));

        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        diff.push_str(&format!(
            "@@ -1,{} +1,{} @@\n",
            old_lines.len(),
            new_lines.len()
        ));

        for line in &old_lines {
            if !new_lines.contains(line) {
                diff.push_str(&format!("-{}\n", line));
            }
        }
        for line in &new_lines {
            if !old_lines.contains(line) {
                diff.push_str(&format!("+{}\n", line));
            } else {
                diff.push_str(&format!(" {}\n", line));
            }
        }
        diff
    }
}
