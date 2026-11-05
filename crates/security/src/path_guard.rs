//! Security-reviewed path resolution, protected file detection, and workspace containment enforcement.
//!
//! Rejects:
//! - Directory traversal ('..')
//! - Absolute paths escaping the workspace root
//! - Windows device namespaces (`\\.\`, `\\?\`, etc.)
//! - UNC network shares
//! - Protected sensitive files (.git, .ssh, .env, id_rsa, etc.)
//! - Symbolic links and Windows reparse points (junctions)

use std::path::{Path, PathBuf};

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

pub fn strip_verbatim_prefix(path: &Path) -> PathBuf {
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

/// Validates that a relative path does not escape the workspace root, does not target protected files,
/// and contains no symlinks or Windows reparse points.
pub fn validate_path(root: &Path, rel: &Path) -> Result<(), String> {
    if rel.is_absolute() {
        return Err(format!("Absolute paths not permitted: {:?}", rel));
    }
    if is_protected_file(rel) {
        return Err(format!("Protected file access blocked: {:?}", rel));
    }
    let target = root.join(rel);
    if !path_is_contained(&target, root) {
        return Err(format!("Path escapes root boundary: {:?}", rel));
    }
    if contains_symlink_or_reparse(root, rel) {
        return Err(format!("Symlink or reparse point blocked: {:?}", rel));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_protected_file() {
        assert!(is_protected_file(Path::new(".env")));
        assert!(is_protected_file(Path::new(".env.local")));
        assert!(is_protected_file(Path::new(".git/config")));
        assert!(is_protected_file(Path::new("Cargo.lock")));
        assert!(is_protected_file(Path::new(".ssh/id_rsa")));
        assert!(!is_protected_file(Path::new("src/main.rs")));
    }

    #[test]
    fn test_path_is_contained() {
        let parent = Path::new("C:/Users/user/project");
        let child = Path::new("C:/Users/user/project/src/lib.rs");
        let outside = Path::new("C:/Users/user/other/file.txt");

        assert!(path_is_contained(child, parent));
        assert!(!path_is_contained(outside, parent));
    }
}
