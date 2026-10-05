//! Patch Generation, Safe Application & Rollback Engine.

use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::secret_scanner::{DetectedSecret, SecretScanner};

#[derive(Debug, Error)]
pub enum PatchError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Protected file cannot be modified: {0}")]
    ProtectedFile(PathBuf),

    #[error("Path escapes repository boundary: {0}")]
    BoundaryViolation(PathBuf),

    #[error("Secret detected in patch: {0:?}")]
    SecretDetected(Vec<DetectedSecret>),

    #[error("Patch application conflict: target content does not match original")]
    PatchConflict,
}

pub struct PatchSnapshot {
    pub file_path: PathBuf,
    pub original_content: String,
}

/// Represents an atomic multi-file modification transaction.
#[derive(Debug, Clone)]
pub struct FilePatch {
    pub relative_path: PathBuf,
    pub new_content: String,
}

pub struct PatchTransaction {
    root_path: PathBuf,
    patches: Vec<FilePatch>,
    applied_snapshots: HashMap<PathBuf, String>,
    committed: bool,
}

impl PatchTransaction {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root_path: root.as_ref().to_path_buf(),
            patches: Vec::new(),
            applied_snapshots: HashMap::new(),
            committed: false,
        }
    }

    pub fn add_change(&mut self, relative_path: impl AsRef<Path>, new_content: impl Into<String>) {
        self.patches.push(FilePatch {
            relative_path: relative_path.as_ref().to_path_buf(),
            new_content: new_content.into(),
        });
    }

    /// Pre-validates boundaries, protected paths, and secret scanning across all changes.
    pub fn validate(&self) -> Result<(), PatchError> {
        for patch in &self.patches {
            let rel = &patch.relative_path;
            let rel_str = rel.to_string_lossy();

            if rel.is_absolute()
                || rel.components().any(|c| matches!(c, std::path::Component::Prefix(_) | std::path::Component::RootDir | std::path::Component::ParentDir))
            {
                return Err(PatchError::BoundaryViolation(rel.clone()));
            }

            if rel_str.contains(".git") || rel_str == ".env" || rel_str.starts_with("target") || rel_str.contains(".ssh") {
                return Err(PatchError::ProtectedFile(rel.clone()));
            }

            // Run secret scanner
            if let Err(detected) = SecretScanner::scan_patch(rel, &patch.new_content) {
                return Err(PatchError::SecretDetected(detected));
            }
        }
        Ok(())
    }

    /// Atomically applies all patches in the transaction. If any write fails, all preceding changes are rolled back.
    pub fn apply(&mut self) -> Result<Vec<String>, PatchError> {
        self.validate()?;

        let mut diffs = Vec::new();

        for patch in &self.patches {
            let full_path = self.root_path.join(&patch.relative_path);

            let original = if full_path.exists() {
                match std::fs::read_to_string(&full_path) {
                    Ok(c) => c,
                    Err(e) => {
                        self.rollback();
                        return Err(PatchError::Io(e));
                    }
                }
            } else {
                String::new()
            };

            self.applied_snapshots
                .entry(full_path.clone())
                .or_insert_with(|| original.clone());

            if let Some(parent) = full_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    self.rollback();
                    return Err(PatchError::Io(e));
                }
            }

            if let Err(e) = std::fs::write(&full_path, &patch.new_content) {
                self.rollback();
                return Err(PatchError::Io(e));
            }

            let diff = PatchEngine::generate_diff(&original, &patch.new_content, &patch.relative_path.to_string_lossy());
            diffs.push(diff);
        }

        Ok(diffs)
    }

    /// Roll back all modifications applied in this transaction.
    pub fn rollback(&mut self) -> usize {
        let mut count = 0;
        for (path, original) in self.applied_snapshots.drain() {
            if original.is_empty() {
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                    count += 1;
                }
            } else {
                if std::fs::write(&path, &original).is_ok() {
                    count += 1;
                }
            }
        }
        count
    }

    /// Commit the transaction, making changes permanent and clearing rollback snapshots.
    pub fn commit(&mut self) {
        self.committed = true;
        self.applied_snapshots.clear();
    }
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
        let rel = relative_path.as_ref();
        let rel_str = rel.to_string_lossy();

        if rel.is_absolute()
            || rel.components().any(|c| matches!(c, std::path::Component::Prefix(_) | std::path::Component::RootDir | std::path::Component::ParentDir))
        {
            return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
        }

        // Validate protected paths
        if rel_str.contains(".git") || rel_str == ".env" || rel_str.starts_with("target") || rel_str.contains(".ssh") {
            return Err(PatchError::ProtectedFile(rel.to_path_buf()));
        }

        // Secret scan
        if let Err(detected) = SecretScanner::scan_patch(rel, new_content) {
            return Err(PatchError::SecretDetected(detected));
        }

        let full_path = self.root_path.join(rel);

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

    #[test]
    fn test_secret_detection_blocks_patch() {
        let dir = tempdir().unwrap();
        let mut engine = PatchEngine::new(dir.path());

        let res = engine.apply_modification("src/auth.rs", "let token = \"ghp_1234567890abcdef\";");
        assert!(matches!(res, Err(PatchError::SecretDetected(_))));
    }

    #[test]
    fn test_patch_transaction_atomic_rollback() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let f1 = root.join("file1.txt");
        let f2 = root.join("file2.txt");
        std::fs::write(&f1, "initial1").unwrap();
        std::fs::write(&f2, "initial2").unwrap();

        let mut tx = PatchTransaction::new(root);
        tx.add_change("file1.txt", "updated1");
        // file2 introduces a secret, which causes validate() to fail
        tx.add_change("file2.txt", "updated2 with AKIAIOSFODNN7EXAMPLE");

        let res = tx.apply();
        assert!(matches!(res, Err(PatchError::SecretDetected(_))));

        // file1 must not have been modified
        assert_eq!(std::fs::read_to_string(&f1).unwrap(), "initial1");
        assert_eq!(std::fs::read_to_string(&f2).unwrap(), "initial2");

        // Successful transaction
        let mut tx_ok = PatchTransaction::new(root);
        tx_ok.add_change("file1.txt", "updated1");
        tx_ok.add_change("file2.txt", "updated2");
        let diffs = tx_ok.apply().unwrap();
        assert_eq!(diffs.len(), 2);
        assert_eq!(std::fs::read_to_string(&f1).unwrap(), "updated1");
        assert_eq!(std::fs::read_to_string(&f2).unwrap(), "updated2");

        // Rollback all in transaction
        let count = tx_ok.rollback();
        assert_eq!(count, 2);
        assert_eq!(std::fs::read_to_string(&f1).unwrap(), "initial1");
        assert_eq!(std::fs::read_to_string(&f2).unwrap(), "initial2");
    }
}
