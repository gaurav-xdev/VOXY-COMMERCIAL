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

/// Determines if a path targets a protected system or configuration file.
pub fn is_protected_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy().replace('\\', "/");
    let lower = path_str.to_ascii_lowercase();

    for comp in path.components() {
        let comp_str = comp.as_os_str().to_string_lossy().to_ascii_lowercase();
        if comp_str == ".git" || comp_str == ".ssh" || comp_str == "target" {
            return true;
        }
        if comp_str == "cargo.lock" {
            return true;
        }
        if comp_str == ".env" || comp_str.starts_with(".env.") || comp_str.ends_with(".env") {
            return true;
        }
        if comp_str == "id_rsa"
            || comp_str == "id_ed25519"
            || comp_str == "id_ecdsa"
            || comp_str == "id_dsa"
            || comp_str == "authorized_keys"
            || comp_str == "known_hosts"
            || comp_str.starts_with("id_rsa")
            || comp_str.starts_with("id_ed25519")
            || comp_str.starts_with("id_ecdsa")
            || comp_str.starts_with("id_dsa")
        {
            return true;
        }
    }

    if let Some(file_name) = path.file_name() {
        let name = file_name.to_string_lossy().to_ascii_lowercase();
        if name == "cargo.lock" {
            return true;
        }
        if name == ".env" || name.starts_with(".env.") || name.ends_with(".env") {
            return true;
        }
        if name == "id_rsa"
            || name == "id_ed25519"
            || name == "id_ecdsa"
            || name == "id_dsa"
            || name == "authorized_keys"
            || name == "known_hosts"
            || name.starts_with("id_rsa")
            || name.starts_with("id_ed25519")
            || name.starts_with("id_ecdsa")
            || name.starts_with("id_dsa")
        {
            return true;
        }
    }

    for seg in lower.split('/') {
        if seg == ".git" || seg == ".ssh" || seg == "target" || seg == "cargo.lock" {
            return true;
        }
        if seg == ".env" || seg.starts_with(".env.") || seg.ends_with(".env") {
            return true;
        }
        if seg == "id_rsa"
            || seg == "id_ed25519"
            || seg == "id_ecdsa"
            || seg == "id_dsa"
            || seg == "authorized_keys"
            || seg == "known_hosts"
            || seg.starts_with("id_rsa")
            || seg.starts_with("id_ed25519")
            || seg.starts_with("id_ecdsa")
            || seg.starts_with("id_dsa")
        {
            return true;
        }
    }

    false
}

/// Checks if a file or directory is a symbolic link or Windows reparse point (junction).
pub fn is_symlink_or_reparse_point(path: &Path) -> bool {
    if let Ok(meta) = path.symlink_metadata() {
        if meta.file_type().is_symlink() {
            return true;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            if (meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                return true;
            }
        }
    }
    false
}

/// Verifies whether any ancestor or component along the path from root to target is a symlink or reparse point.
pub fn contains_symlink_or_reparse(root: &Path, rel: &Path) -> bool {
    let mut current = root.to_path_buf();
    for comp in rel.components() {
        if let std::path::Component::Normal(c) = comp {
            current.push(c);
            if is_symlink_or_reparse_point(&current) {
                return true;
            }
        }
    }
    false
}

fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", stripped))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

/// Checks whether `child` is contained inside `parent` (case-insensitively on Windows, case-sensitively on Unix).
pub fn path_is_contained(child: &Path, parent: &Path) -> bool {
    let norm_child = strip_verbatim_prefix(child);
    let norm_parent = strip_verbatim_prefix(parent);

    #[cfg(windows)]
    {
        let child_comps: Vec<_> = norm_child
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect();
        let parent_comps: Vec<_> = norm_parent
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
            .collect();

        if child_comps.len() < parent_comps.len() {
            return false;
        }
        child_comps[..parent_comps.len()] == parent_comps[..]
    }

    #[cfg(not(windows))]
    {
        norm_child.starts_with(&norm_parent)
    }
}

/// Validates that a relative path does not escape the repository root, does not target protected files,
/// and contains no symlinks or Windows reparse points.
pub fn validate_path(root: &Path, rel: &Path) -> Result<(), PatchError> {
    if rel.is_absolute() {
        return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
    }

    let rel_str = rel.to_string_lossy();
    if rel_str.contains("..") {
        return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
    }

    for comp in rel.components() {
        match comp {
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::ParentDir => {
                return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
            }
            _ => {}
        }
    }

    if is_protected_file(rel) {
        return Err(PatchError::ProtectedFile(rel.to_path_buf()));
    }

    if contains_symlink_or_reparse(root, rel) {
        return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
    }

    let full_target = root.join(rel);
    if is_symlink_or_reparse_point(&full_target) {
        return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
    }

    let mut check_path = full_target.clone();
    while !check_path.exists() {
        if let Some(parent) = check_path.parent() {
            check_path = parent.to_path_buf();
        } else {
            break;
        }
    }

    if let (Ok(canon_check), Ok(canon_root)) = (check_path.canonicalize(), root.canonicalize()) {
        if !path_is_contained(&canon_check, &canon_root) {
            return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
        }
    } else if !path_is_contained(&check_path, root) {
        return Err(PatchError::BoundaryViolation(rel.to_path_buf()));
    }

    Ok(())
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
            validate_path(&self.root_path, rel)?;

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
        let patches = self.patches.clone();

        for patch in patches {
            if let Err(e) = validate_path(&self.root_path, &patch.relative_path) {
                self.rollback();
                return Err(e);
            }

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

            // Atomic file write via temporary file
            let tmp_path =
                full_path.with_extension(format!("tmp_{}", uuid::Uuid::new_v4().simple()));
            if let Err(e) = std::fs::write(&tmp_path, &patch.new_content) {
                let _ = std::fs::remove_file(&tmp_path);
                self.rollback();
                return Err(PatchError::Io(e));
            }

            if let Err(e) = std::fs::rename(&tmp_path, &full_path) {
                let _ = std::fs::remove_file(&tmp_path);
                self.rollback();
                return Err(PatchError::Io(e));
            }

            let diff = PatchEngine::generate_diff(
                &original,
                &patch.new_content,
                &patch.relative_path.to_string_lossy(),
            );
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
        validate_path(&self.root_path, rel)?;

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

        // Atomic write via temporary file
        let tmp_path = full_path.with_extension(format!("tmp_{}", uuid::Uuid::new_v4().simple()));
        if let Err(e) = std::fs::write(&tmp_path, new_content) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(PatchError::Io(e));
        }

        if let Err(e) = std::fs::rename(&tmp_path, &full_path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(PatchError::Io(e));
        }

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

    #[test]
    fn test_cargo_lock_and_env_and_ssh_rejection() {
        let dir = tempdir().unwrap();
        let mut engine = PatchEngine::new(dir.path());

        // Cargo.lock rejection
        let res_lock = engine.apply_modification("Cargo.lock", "# lockfile");
        assert!(matches!(res_lock, Err(PatchError::ProtectedFile(_))));

        let res_lock_nested =
            engine.apply_modification("crates/foo/Cargo.lock", "# nested lockfile");
        assert!(matches!(res_lock_nested, Err(PatchError::ProtectedFile(_))));

        // .env variations
        let res_env_local = engine.apply_modification(".env.local", "FOO=BAR");
        assert!(matches!(res_env_local, Err(PatchError::ProtectedFile(_))));

        let res_env_prod = engine.apply_modification("config/.env.production", "FOO=BAR");
        assert!(matches!(res_env_prod, Err(PatchError::ProtectedFile(_))));

        // SSH keys
        let res_ssh_dir = engine.apply_modification(".ssh/id_rsa", "fake_key");
        assert!(matches!(res_ssh_dir, Err(PatchError::ProtectedFile(_))));

        let res_id_rsa = engine.apply_modification("id_rsa", "fake_key");
        assert!(matches!(res_id_rsa, Err(PatchError::ProtectedFile(_))));

        let res_id_ed25519 = engine.apply_modification("id_ed25519", "fake_key");
        assert!(matches!(res_id_ed25519, Err(PatchError::ProtectedFile(_))));

        let res_auth_keys = engine.apply_modification("authorized_keys", "ssh-rsa ...");
        assert!(matches!(res_auth_keys, Err(PatchError::ProtectedFile(_))));

        // .git directory
        let res_git = engine.apply_modification(".git/config", "[core]");
        assert!(matches!(res_git, Err(PatchError::ProtectedFile(_))));

        let res_git_nested = engine.apply_modification("sub/.git/HEAD", "ref: refs/heads/main");
        assert!(matches!(res_git_nested, Err(PatchError::ProtectedFile(_))));
    }

    #[test]
    fn test_path_traversal_rejection() {
        let dir = tempdir().unwrap();
        let mut engine = PatchEngine::new(dir.path());

        let res1 = engine.apply_modification("../escape.rs", "fn evil() {}");
        assert!(matches!(res1, Err(PatchError::BoundaryViolation(_))));

        let res2 = engine.apply_modification("sub/../../escape.rs", "fn evil() {}");
        assert!(matches!(res2, Err(PatchError::BoundaryViolation(_))));

        let mut tx = PatchTransaction::new(dir.path());
        tx.add_change("../escape.rs", "fn evil() {}");
        assert!(matches!(tx.apply(), Err(PatchError::BoundaryViolation(_))));
    }

    #[test]
    fn test_symlink_or_reparse_point_rejection() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Check if Windows junction creation is supported in this environment
        #[cfg(windows)]
        {
            let target_dir = root.join("target_folder");
            let junction_dir = root.join("junc_link");
            std::fs::create_dir_all(&target_dir).unwrap();
            let status = std::process::Command::new("cmd")
                .args([
                    "/c",
                    "mklink",
                    "/J",
                    &junction_dir.to_string_lossy(),
                    &target_dir.to_string_lossy(),
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();

            if let Ok(st) = status {
                if st.success() {
                    let mut engine = PatchEngine::new(root);
                    let res = engine.apply_modification("junc_link/evil.rs", "fn exploit() {}");
                    assert!(matches!(res, Err(PatchError::BoundaryViolation(_))));
                }
            }
        }
    }
}
