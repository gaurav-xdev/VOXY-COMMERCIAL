//! Patch Generation, Safe Application & Rollback Engine.

use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Protected file cannot be modified: {0}")]
    ProtectedFile(PathBuf),

    #[error("Patch application conflict: target content does not match original")]
    PatchConflict,
}

pub struct PatchSnapshot {
    pub file_path: PathBuf,
    pub original_content: String,
}

pub struct PatchEngine {
    root_path: PathBuf,
    snapshots: HashMap<PathBuf, String>,
}

impl PatchEngine {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root_path: root.as_ref().to_path_buf(),
            snapshots: HashMap::new(),
        }
    }

    /// Computes a unified diff between two text strings.
    pub fn generate_diff(original: &str, modified: &str, file_label: &str) -> String {
        let diff = TextDiff::from_lines(original, modified);
        let mut output = format!("--- a/{}\n+++ b/{}\n", file_label, file_label);

        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            output.push_str(&format!("{}{}", sign, change));
        }

        output
    }

    /// Safely updates a file, creating a pre-modification snapshot for rollback.
    pub fn apply_modification(
        &mut self,
        relative_path: impl AsRef<Path>,
        new_content: &str,
    ) -> Result<String, PatchError> {
        let full_path = self.root_path.join(relative_path.as_ref());

        // Validate protected paths
        let rel_str = relative_path.as_ref().to_string_lossy();
        if rel_str.contains(".git") || rel_str == ".env" || rel_str.starts_with("target") {
            return Err(PatchError::ProtectedFile(full_path));
        }

        let original = if full_path.exists() {
            std::fs::read_to_string(&full_path)?
        } else {
            String::new()
        };

        // Snapshot original content before writing
        self.snapshots
            .entry(full_path.clone())
            .or_insert_with(|| original.clone());

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&full_path, new_content)?;

        let diff = Self::generate_diff(&original, new_content, &rel_str);
        Ok(diff)
    }

    /// Rolls back all modifications performed during the current session.
    pub fn rollback_all(&mut self) -> Result<usize, PatchError> {
        let mut count = 0;
        for (path, original) in self.snapshots.drain() {
            if original.is_empty() {
                // Was a new file created; remove it
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                    count += 1;
                }
            } else {
                std::fs::write(&path, &original)?;
                count += 1;
            }
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_harness_patch_generation_and_rollback() {
        let dir = tempdir().unwrap();
        let file_rel = Path::new("src").join("config.rs");
        let full_path = dir.path().join(&file_rel);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        let orig = "pub fn get_port() -> u16 { 8080 }\n";
        let modified = "pub fn get_port() -> u16 { 9090 }\n";
        std::fs::write(&full_path, orig).unwrap();

        let mut engine = PatchEngine::new(dir.path());

        // Modify with diff
        let diff = engine.apply_modification(&file_rel, modified).unwrap();
        assert!(diff.contains("-pub fn get_port() -> u16 { 8080 }"));
        assert!(diff.contains("+pub fn get_port() -> u16 { 9090 }"));
        assert_eq!(std::fs::read_to_string(&full_path).unwrap(), modified);

        // Rollback restores original
        let restored_count = engine.rollback_all().unwrap();
        assert_eq!(restored_count, 1);
        assert_eq!(std::fs::read_to_string(&full_path).unwrap(), orig);
    }

    #[test]
    fn test_protected_file_rejection() {
        let dir = tempdir().unwrap();
        let mut engine = PatchEngine::new(dir.path());

        let res = engine.apply_modification(".env", "SECRET=123");
        assert!(matches!(res, Err(PatchError::ProtectedFile(_))));
    }
}
