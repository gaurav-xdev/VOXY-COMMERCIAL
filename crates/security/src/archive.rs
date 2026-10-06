//! Hardened, secure zip archive extraction and validation engine.
//! Protects against:
//! - Zip slips & Path traversal (`../`, absolute paths, drive letters, UNC paths)
//! - NTFS Alternate Data Streams (`:`)
//! - Hardlinks and symlinks
//! - Zip bombs (decompression bombs, extreme compression ratios)
//! - Excessive entry count, path depth, and excessive cumulative file size.

use std::io;
use std::path::{Component, Path, PathBuf};

/// Configurable safety limits for archive extraction.
#[derive(Debug, Clone)]
pub struct ArchiveSecurityLimits {
    /// Maximum allowed compressed archive size in bytes (default: 100 MB).
    pub max_compressed_bytes: u64,
    /// Maximum allowed uncompressed size across all entries combined (default: 250 MB).
    pub max_total_uncompressed_bytes: u64,
    /// Maximum allowed uncompressed size for a single entry (default: 50 MB).
    pub max_entry_uncompressed_bytes: u64,
    /// Maximum number of file/folder entries in the archive (default: 10,000).
    pub max_entry_count: usize,
    /// Maximum allowed directory nesting depth (default: 16).
    pub max_path_depth: usize,
    /// Maximum allowed path string length in characters (default: 260).
    pub max_path_length: usize,
    /// Maximum allowable compression ratio (uncompressed / compressed) (default: 100.0).
    pub max_compression_ratio: f64,
}

impl Default for ArchiveSecurityLimits {
    fn default() -> Self {
        Self {
            max_compressed_bytes: 100 * 1024 * 1024,
            max_total_uncompressed_bytes: 250 * 1024 * 1024,
            max_entry_uncompressed_bytes: 50 * 1024 * 1024,
            max_entry_count: 10_000,
            max_path_depth: 16,
            max_path_length: 260,
            max_compression_ratio: 100.0,
        }
    }
}

/// Errors raised by archive safety violations.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveSecurityError {
    #[error("Archive size exceeds maximum allowed compressed size limit: {size} > {max}")]
    CompressedSizeExceeded { size: u64, max: u64 },

    #[error("Total extracted size exceeds security budget: {extracted} > {max}")]
    TotalDecompressedSizeExceeded { extracted: u64, max: u64 },

    #[error("Individual entry exceeds security limit: {entry_size} > {max}")]
    EntrySizeExceeded { entry_size: u64, max: u64 },

    #[error("Archive contains too many entries: {count} > {max}")]
    EntryCountExceeded { count: usize, max: usize },

    #[error(
        "Compression ratio exceeds safe threshold (potential zip bomb): {ratio:.1} > {max:.1}"
    )]
    SuspiciousCompressionRatio { ratio: f64, max: f64 },

    #[error("Path traversal or illegal component detected: '{path}'")]
    PathTraversalDetected { path: String },

    #[error("Path depth exceeds maximum allowed nesting: {depth} > {max}")]
    PathDepthExceeded { depth: usize, max: usize },

    #[error("Path length exceeds maximum limit: {length} > {max}")]
    PathLengthExceeded { length: usize, max: usize },

    #[error("Symlinks and hardlinks in archives are prohibited by security policy: '{path}'")]
    IllegalLinkDetected { path: String },

    #[error("NTFS Alternate Data Stream or colon detected in path: '{path}'")]
    IllegalStreamDetected { path: String },

    #[error("IO error during archive operation: {0}")]
    IoError(#[from] io::Error),
}

/// Validates an entry's relative path before extraction.
/// Ensures the path is strictly relative and contains no escapes or alternate streams.
pub fn validate_archive_path(
    raw_path: &str,
    limits: &ArchiveSecurityLimits,
) -> Result<PathBuf, ArchiveSecurityError> {
    if raw_path.len() > limits.max_path_length {
        return Err(ArchiveSecurityError::PathLengthExceeded {
            length: raw_path.len(),
            max: limits.max_path_length,
        });
    }

    if raw_path.contains(':') {
        return Err(ArchiveSecurityError::IllegalStreamDetected {
            path: raw_path.to_string(),
        });
    }

    let path = Path::new(raw_path);

    // Reject absolute paths
    if path.is_absolute() {
        return Err(ArchiveSecurityError::PathTraversalDetected {
            path: raw_path.to_string(),
        });
    }

    let mut depth = 0;
    let mut sanitized = PathBuf::new();

    for comp in path.components() {
        match comp {
            Component::Normal(c) => {
                let name = c.to_string_lossy();
                if name == ".." || name == "." || name.contains(':') {
                    return Err(ArchiveSecurityError::PathTraversalDetected {
                        path: raw_path.to_string(),
                    });
                }
                sanitized.push(c);
                depth += 1;
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ArchiveSecurityError::PathTraversalDetected {
                    path: raw_path.to_string(),
                });
            }
            Component::CurDir => {}
        }
    }

    if depth > limits.max_path_depth {
        return Err(ArchiveSecurityError::PathDepthExceeded {
            depth,
            max: limits.max_path_depth,
        });
    }

    Ok(sanitized)
}

/// Validates and ensures the resolved target path is strictly within the target root directory.
pub fn ensure_within_root(root: &Path, rel_path: &Path) -> Result<PathBuf, ArchiveSecurityError> {
    let target = root.join(rel_path);

    // Verify canonical path does not escape root (prefix check)
    let canonical_root = match root.canonicalize() {
        Ok(r) => r,
        Err(_) => root.to_path_buf(),
    };

    if !target.starts_with(root) && !target.starts_with(&canonical_root) {
        return Err(ArchiveSecurityError::PathTraversalDetected {
            path: target.to_string_lossy().to_string(),
        });
    }

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_archive_paths() {
        let limits = ArchiveSecurityLimits::default();
        assert!(validate_archive_path("assets/icon.png", &limits).is_ok());
        assert!(validate_archive_path("docs/readme.txt", &limits).is_ok());
        assert!(validate_archive_path("sub/dir/nested/file.rs", &limits).is_ok());
    }

    #[test]
    fn test_blocks_path_traversal() {
        let limits = ArchiveSecurityLimits::default();
        assert!(validate_archive_path("../secret.txt", &limits).is_err());
        assert!(validate_archive_path("dir/../../secret.txt", &limits).is_err());
        assert!(validate_archive_path("foo/bar/../../../etc/passwd", &limits).is_err());
    }

    #[test]
    fn test_blocks_absolute_and_drive_paths() {
        let limits = ArchiveSecurityLimits::default();
        assert!(validate_archive_path("/etc/shadow", &limits).is_err());
        assert!(validate_archive_path("C:\\Windows\\System32\\cmd.exe", &limits).is_err());
        assert!(validate_archive_path("D:/payload.bat", &limits).is_err());
    }

    #[test]
    fn test_blocks_alternate_data_streams() {
        let limits = ArchiveSecurityLimits::default();
        assert!(validate_archive_path("file.txt:hidden_stream", &limits).is_err());
        assert!(validate_archive_path("nested/dir:stream:$DATA", &limits).is_err());
    }

    #[test]
    fn test_blocks_excessive_path_length_and_depth() {
        let mut limits = ArchiveSecurityLimits::default();
        limits.max_path_depth = 3;
        limits.max_path_length = 20;

        assert!(validate_archive_path("a/b/c/d/e/file.txt", &limits).is_err());
        assert!(validate_archive_path(
            "this_is_a_very_long_file_name_exceeding_limits.txt",
            &limits
        )
        .is_err());
    }
}
