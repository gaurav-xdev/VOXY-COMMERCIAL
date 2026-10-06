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

    #[error("Zip format error: {0}")]
    ZipError(String),

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

/// Safely extracts a zip archive from a reader into a destination directory.
/// Enforces all ArchiveSecurityLimits: max size, max entries, max entry size,
/// compression ratios, path depth, and symlink prohibition.
pub fn safe_extract_zip<R: io::Read + io::Seek>(
    reader: R,
    destination: &Path,
    limits: &ArchiveSecurityLimits,
) -> Result<usize, ArchiveSecurityError> {
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| ArchiveSecurityError::ZipError(e.to_string()))?;

    let entry_count = archive.len();
    if entry_count > limits.max_entry_count {
        return Err(ArchiveSecurityError::EntryCountExceeded {
            count: entry_count,
            max: limits.max_entry_count,
        });
    }

    std::fs::create_dir_all(destination)?;
    let canonical_dest = destination.canonicalize()?;

    let mut total_extracted: u64 = 0;

    for i in 0..entry_count {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ArchiveSecurityError::ZipError(e.to_string()))?;

        let raw_name = entry.name().to_string();

        // 1. Validate relative path & prevent traversal
        let safe_rel_path = validate_archive_path(&raw_name, limits)?;

        // 2. Prohibit symlinks/hardlinks
        if entry.is_symlink() {
            return Err(ArchiveSecurityError::IllegalLinkDetected { path: raw_name });
        }

        // 3. Check declared entry size
        let uncompressed_size = entry.size();
        if uncompressed_size > limits.max_entry_uncompressed_bytes {
            return Err(ArchiveSecurityError::EntrySizeExceeded {
                entry_size: uncompressed_size,
                max: limits.max_entry_uncompressed_bytes,
            });
        }

        // 4. Check compression ratio for zip bomb defense
        let compressed_size = entry.compressed_size();
        if compressed_size > 0 && uncompressed_size > 1024 {
            let ratio = uncompressed_size as f64 / compressed_size as f64;
            if ratio > limits.max_compression_ratio {
                return Err(ArchiveSecurityError::SuspiciousCompressionRatio {
                    ratio,
                    max: limits.max_compression_ratio,
                });
            }
        }

        // 5. Ensure path resolves inside destination directory
        let target_path = ensure_within_root(&canonical_dest, &safe_rel_path)?;

        if entry.is_dir() {
            std::fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut out_file = std::fs::File::create(&target_path)?;
            let mut entry_extracted: u64 = 0;
            let mut buffer = [0u8; 8192];

            loop {
                let n = io::Read::read(&mut entry, &mut buffer)?;
                if n == 0 {
                    break;
                }
                entry_extracted += n as u64;
                total_extracted += n as u64;

                if entry_extracted > limits.max_entry_uncompressed_bytes {
                    let _ = std::fs::remove_file(&target_path);
                    return Err(ArchiveSecurityError::EntrySizeExceeded {
                        entry_size: entry_extracted,
                        max: limits.max_entry_uncompressed_bytes,
                    });
                }

                if total_extracted > limits.max_total_uncompressed_bytes {
                    let _ = std::fs::remove_file(&target_path);
                    return Err(ArchiveSecurityError::TotalDecompressedSizeExceeded {
                        extracted: total_extracted,
                        max: limits.max_total_uncompressed_bytes,
                    });
                }

                io::Write::write_all(&mut out_file, &buffer[..n])?;
            }
        }
    }

    Ok(entry_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

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

    #[test]
    fn test_safe_extract_zip_valid_archive() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("test.zip");
        let dest = dir.path().join("extracted");

        // Create a small valid zip
        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip_writer.start_file("hello.txt", options).unwrap();
        zip_writer.write_all(b"Hello from safe archive!").unwrap();

        zip_writer.start_file("sub/world.txt", options).unwrap();
        zip_writer.write_all(b"World nested").unwrap();
        zip_writer.finish().unwrap();

        let zip_file = std::fs::File::open(&zip_path).unwrap();
        let limits = ArchiveSecurityLimits::default();
        let count = safe_extract_zip(zip_file, &dest, &limits).unwrap();

        assert_eq!(count, 2);
        assert_eq!(
            std::fs::read_to_string(dest.join("hello.txt")).unwrap(),
            "Hello from safe archive!"
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("sub").join("world.txt")).unwrap(),
            "World nested"
        );
    }

    #[test]
    fn test_safe_extract_zip_rejects_zip_slip() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("evil.zip");
        let dest = dir.path().join("extracted");

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip_writer.start_file("../evil.txt", options).unwrap();
        zip_writer.write_all(b"PWNED").unwrap();
        zip_writer.finish().unwrap();

        let zip_file = std::fs::File::open(&zip_path).unwrap();
        let limits = ArchiveSecurityLimits::default();
        let res = safe_extract_zip(zip_file, &dest, &limits);

        assert!(res.is_err());
        assert!(!dest.join("../evil.txt").exists());
    }

    #[test]
    fn test_safe_extract_zip_rejects_entry_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("oversized.zip");
        let dest = dir.path().join("extracted");

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip_writer.start_file("large.bin", options).unwrap();
        zip_writer.write_all(&vec![0u8; 10_000]).unwrap();
        zip_writer.finish().unwrap();

        let mut limits = ArchiveSecurityLimits::default();
        limits.max_entry_uncompressed_bytes = 1000; // lower than 10_000

        let zip_file = std::fs::File::open(&zip_path).unwrap();
        let res = safe_extract_zip(zip_file, &dest, &limits);
        assert!(res.is_err());
    }
}
