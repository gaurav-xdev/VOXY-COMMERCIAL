//! Hardened, secure zip and tar archive extraction and validation engine.
//! Protects against:
//! - Zip slips & Path traversal (`../`, absolute paths, drive letters, UNC paths, null bytes)
//! - NTFS Alternate Data Streams (`:`)
//! - Hardlinks and symlinks escaping extraction root
//! - Zip bombs (decompression bombs, extreme compression ratios, falsified headers)
//! - Excessive entry count, path depth, and excessive cumulative file size
//! - Clean rollback of all partially extracted files and directories on any error.

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

    #[error("Tar format error: {0}")]
    TarError(String),

    #[error("IO error during archive operation: {0}")]
    IoError(#[from] io::Error),
}

/// Tracks files and directories created during an archive extraction transaction.
/// If an error occurs prior to commitment, clean rollback removes all partially
/// created files and directories.
#[derive(Debug)]
pub struct ArchiveRollbackTracker {
    created_files: Vec<PathBuf>,
    created_dirs: Vec<PathBuf>,
    committed: bool,
}

impl ArchiveRollbackTracker {
    pub fn new() -> Self {
        Self {
            created_files: Vec::new(),
            created_dirs: Vec::new(),
            committed: false,
        }
    }

    pub fn record_dir(&mut self, path: &Path) {
        self.created_dirs.push(path.to_path_buf());
    }

    pub fn record_file(&mut self, path: &Path) {
        self.created_files.push(path.to_path_buf());
    }

    pub fn commit(&mut self) {
        self.committed = true;
    }

    pub fn rollback(&mut self) {
        if self.committed {
            return;
        }

        // 1. Delete all partially extracted files
        for file in &self.created_files {
            if file.exists() {
                let _ = std::fs::remove_file(file);
            }
        }

        // 2. Delete all created directories in reverse order (leaf to root)
        for dir in self.created_dirs.iter().rev() {
            if dir.exists() {
                let _ = std::fs::remove_dir(dir);
            }
        }
    }
}

impl Default for ArchiveRollbackTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ArchiveRollbackTracker {
    fn drop(&mut self) {
        self.rollback();
    }
}

/// Helper to create and track directories so rollback can clean them up.
fn ensure_dir_tracked(
    tracker: &mut ArchiveRollbackTracker,
    dir: &Path,
    dest_root: &Path,
) -> Result<(), io::Error> {
    if dir.exists() {
        return Ok(());
    }

    // Collect chain of uncreated parent directories up to dest_root
    let mut to_create = Vec::new();
    let mut curr = dir;
    while curr.starts_with(dest_root) && curr != dest_root {
        if !curr.exists() {
            to_create.push(curr.to_path_buf());
        }
        if let Some(parent) = curr.parent() {
            curr = parent;
        } else {
            break;
        }
    }

    // to_create has deepest dir first, reverse to create ancestors first
    for d in to_create.into_iter().rev() {
        if !d.exists() {
            std::fs::create_dir(&d)?;
            tracker.record_dir(&d);
        }
    }

    Ok(())
}

fn strip_verbatim_prefix(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

/// Validates an entry's relative path before extraction.
/// Ensures the path is strictly relative and contains no escapes, UNC shares,
/// null bytes, or alternate data streams.
pub fn validate_archive_path(
    raw_path: &str,
    limits: &ArchiveSecurityLimits,
) -> Result<PathBuf, ArchiveSecurityError> {
    if raw_path.is_empty() {
        return Err(ArchiveSecurityError::PathTraversalDetected {
            path: raw_path.to_string(),
        });
    }

    if raw_path.len() > limits.max_path_length {
        return Err(ArchiveSecurityError::PathLengthExceeded {
            length: raw_path.len(),
            max: limits.max_path_length,
        });
    }

    // Reject null bytes
    if raw_path.contains('\0') {
        return Err(ArchiveSecurityError::PathTraversalDetected {
            path: raw_path.to_string(),
        });
    }

    // Reject NTFS Alternate Data Streams and drive colons
    if raw_path.contains(':') {
        return Err(ArchiveSecurityError::IllegalStreamDetected {
            path: raw_path.to_string(),
        });
    }

    // Reject Windows UNC and network share paths
    if raw_path.starts_with(r"\\") || raw_path.starts_with("//") {
        return Err(ArchiveSecurityError::PathTraversalDetected {
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

    // Check components across both forward and backward slashes
    let normalized = raw_path.replace('\\', "/");
    for comp in path.components() {
        match comp {
            Component::Normal(c) => {
                let name = c.to_string_lossy();
                if name == ".." || name == "." || name.contains(':') || name.contains('\0') {
                    return Err(ArchiveSecurityError::PathTraversalDetected {
                        path: raw_path.to_string(),
                    });
                }
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ArchiveSecurityError::PathTraversalDetected {
                    path: raw_path.to_string(),
                });
            }
            Component::CurDir => {}
        }
    }

    for seg in normalized.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." || seg.contains(':') || seg.contains('\0') {
            return Err(ArchiveSecurityError::PathTraversalDetected {
                path: raw_path.to_string(),
            });
        }
        sanitized.push(seg);
        depth += 1;
    }

    if depth > limits.max_path_depth {
        return Err(ArchiveSecurityError::PathDepthExceeded {
            depth,
            max: limits.max_path_depth,
        });
    }

    if depth == 0 {
        return Err(ArchiveSecurityError::PathTraversalDetected {
            path: raw_path.to_string(),
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

    let norm_target = strip_verbatim_prefix(&target);
    let norm_root = strip_verbatim_prefix(root);
    let norm_canon_root = strip_verbatim_prefix(&canonical_root);

    if !target.starts_with(root)
        && !target.starts_with(&canonical_root)
        && !norm_target.starts_with(&norm_root)
        && !norm_target.starts_with(&norm_canon_root)
    {
        return Err(ArchiveSecurityError::PathTraversalDetected {
            path: target.to_string_lossy().to_string(),
        });
    }

    Ok(target)
}

/// Safely extracts a zip archive from a reader into a destination directory.
/// Enforces all ArchiveSecurityLimits: max size, max entries, max entry size,
/// compression ratios, path depth, symlink prohibition, and atomic rollback on error.
pub fn safe_extract_zip<R: io::Read + io::Seek>(
    mut reader: R,
    destination: &Path,
    limits: &ArchiveSecurityLimits,
) -> Result<usize, ArchiveSecurityError> {
    // 0. Verify compressed size if seekable
    if let Ok(stream_len) = reader.seek(io::SeekFrom::End(0)) {
        if stream_len > limits.max_compressed_bytes {
            return Err(ArchiveSecurityError::CompressedSizeExceeded {
                size: stream_len,
                max: limits.max_compressed_bytes,
            });
        }
        let _ = reader.seek(io::SeekFrom::Start(0));
    }

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

    let mut tracker = ArchiveRollbackTracker::new();
    let mut total_extracted: u64 = 0;

    for i in 0..entry_count {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ArchiveSecurityError::ZipError(e.to_string()))?;

        let raw_name = entry.name().to_string();

        // 1. Validate relative path & prevent traversal
        let safe_rel_path = validate_archive_path(&raw_name, limits)?;

        // 2. Prohibit symlinks/hardlinks
        let is_symlink = entry.is_symlink()
            || entry.unix_mode().map_or(false, |m| (m & 0o170000) == 0o120000);
        if is_symlink {
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
            ensure_dir_tracked(&mut tracker, &target_path, &canonical_dest)?;
        } else {
            if let Some(parent) = target_path.parent() {
                ensure_dir_tracked(&mut tracker, parent, &canonical_dest)?;
            }

            tracker.record_file(&target_path);
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
                    return Err(ArchiveSecurityError::EntrySizeExceeded {
                        entry_size: entry_extracted,
                        max: limits.max_entry_uncompressed_bytes,
                    });
                }

                if total_extracted > limits.max_total_uncompressed_bytes {
                    return Err(ArchiveSecurityError::TotalDecompressedSizeExceeded {
                        extracted: total_extracted,
                        max: limits.max_total_uncompressed_bytes,
                    });
                }

                if compressed_size > 0 && entry_extracted > 1024 {
                    let live_ratio = entry_extracted as f64 / compressed_size as f64;
                    if live_ratio > limits.max_compression_ratio {
                        return Err(ArchiveSecurityError::SuspiciousCompressionRatio {
                            ratio: live_ratio,
                            max: limits.max_compression_ratio,
                        });
                    }
                }

                io::Write::write_all(&mut out_file, &buffer[..n])?;
            }
        }
    }

    // Success: commit transaction to keep extracted files
    tracker.commit();
    Ok(entry_count)
}

/// Safely extracts a tar archive from a reader into a destination directory.
/// Enforces all ArchiveSecurityLimits: max size, max entries, max entry size,
/// symlink and device node rejection, path traversal prevention, and clean rollback on error.
pub fn safe_extract_tar<R: io::Read>(
    reader: R,
    destination: &Path,
    limits: &ArchiveSecurityLimits,
) -> Result<usize, ArchiveSecurityError> {
    std::fs::create_dir_all(destination)?;
    let canonical_dest = destination.canonicalize()?;

    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|e| ArchiveSecurityError::TarError(e.to_string()))?;

    let mut tracker = ArchiveRollbackTracker::new();
    let mut entry_count: usize = 0;
    let mut total_extracted: u64 = 0;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|e| ArchiveSecurityError::TarError(e.to_string()))?;
        entry_count += 1;
        if entry_count > limits.max_entry_count {
            return Err(ArchiveSecurityError::EntryCountExceeded {
                count: entry_count,
                max: limits.max_entry_count,
            });
        }

        let raw_name = entry
            .path()
            .map_err(|e| ArchiveSecurityError::TarError(e.to_string()))?
            .to_string_lossy()
            .to_string();

        let safe_rel_path = validate_archive_path(&raw_name, limits)?;

        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(ArchiveSecurityError::IllegalLinkDetected { path: raw_name });
        }
        if entry_type.is_block_special()
            || entry_type.is_character_special()
            || entry_type.is_fifo()
        {
            return Err(ArchiveSecurityError::IllegalLinkDetected { path: raw_name });
        }

        let uncompressed_size = entry
            .header()
            .size()
            .map_err(|e| ArchiveSecurityError::TarError(e.to_string()))?;
        if uncompressed_size > limits.max_entry_uncompressed_bytes {
            return Err(ArchiveSecurityError::EntrySizeExceeded {
                entry_size: uncompressed_size,
                max: limits.max_entry_uncompressed_bytes,
            });
        }

        let target_path = ensure_within_root(&canonical_dest, &safe_rel_path)?;

        if entry_type.is_dir() {
            ensure_dir_tracked(&mut tracker, &target_path, &canonical_dest)?;
        } else {
            if let Some(parent) = target_path.parent() {
                ensure_dir_tracked(&mut tracker, parent, &canonical_dest)?;
            }

            tracker.record_file(&target_path);
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
                    return Err(ArchiveSecurityError::EntrySizeExceeded {
                        entry_size: entry_extracted,
                        max: limits.max_entry_uncompressed_bytes,
                    });
                }

                if total_extracted > limits.max_total_uncompressed_bytes {
                    return Err(ArchiveSecurityError::TotalDecompressedSizeExceeded {
                        extracted: total_extracted,
                        max: limits.max_total_uncompressed_bytes,
                    });
                }

                io::Write::write_all(&mut out_file, &buffer[..n])?;
            }
        }
    }

    // Success: commit transaction
    tracker.commit();
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
        assert!(validate_archive_path(r"foo\..\evil.txt", &limits).is_err());
        assert!(validate_archive_path(r"\\server\share\evil.txt", &limits).is_err());
        assert!(validate_archive_path("foo\0bar.txt", &limits).is_err());
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
    fn test_safe_extract_zip_rejects_zip_slip_and_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("evil.zip");
        let dest = dir.path().join("extracted");

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        // First a valid file
        zip_writer.start_file("innocent.txt", options).unwrap();
        zip_writer.write_all(b"innocent content").unwrap();

        // Then a traversal file
        zip_writer.start_file("../evil.txt", options).unwrap();
        zip_writer.write_all(b"PWNED").unwrap();
        zip_writer.finish().unwrap();

        let zip_file = std::fs::File::open(&zip_path).unwrap();
        let limits = ArchiveSecurityLimits::default();
        let res = safe_extract_zip(zip_file, &dest, &limits);

        assert!(res.is_err());
        // Verify rollback: even innocent.txt must be cleaned up!
        assert!(!dest.join("innocent.txt").exists());
        assert!(!dest.join("../evil.txt").exists());
    }

    #[test]
    fn test_safe_extract_zip_rejects_entry_size_limit_and_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("oversized.zip");
        let dest = dir.path().join("extracted");

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        // First valid file
        zip_writer.start_file("small.txt", options).unwrap();
        zip_writer.write_all(b"small").unwrap();

        // Oversized file
        zip_writer.start_file("large.bin", options).unwrap();
        zip_writer.write_all(&vec![0u8; 10_000]).unwrap();
        zip_writer.finish().unwrap();

        let mut limits = ArchiveSecurityLimits::default();
        limits.max_entry_uncompressed_bytes = 1000;

        let zip_file = std::fs::File::open(&zip_path).unwrap();
        let res = safe_extract_zip(zip_file, &dest, &limits);
        assert!(res.is_err());

        // Verify clean rollback of all files
        assert!(!dest.join("small.txt").exists());
        assert!(!dest.join("large.bin").exists());
    }

    #[test]
    fn test_safe_extract_zip_rejects_compression_ratio_bomb() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("bomb.zip");
        let dest = dir.path().join("extracted");

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip_writer = zip::ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // Highly compressible data: 200 KB of zeros compresses to ~200 bytes (ratio ~1000x)
        zip_writer.start_file("bomb.bin", options).unwrap();
        zip_writer.write_all(&vec![0u8; 200_000]).unwrap();
        zip_writer.finish().unwrap();

        let mut limits = ArchiveSecurityLimits::default();
        limits.max_compression_ratio = 10.0; // strict ratio limit

        let zip_file = std::fs::File::open(&zip_path).unwrap();
        let res = safe_extract_zip(zip_file, &dest, &limits);
        assert!(res.is_err());
        match res.unwrap_err() {
            ArchiveSecurityError::SuspiciousCompressionRatio { ratio, max } => {
                assert!(ratio > max);
            }
            other => panic!("Expected SuspiciousCompressionRatio, got {:?}", other),
        }
        assert!(!dest.join("bomb.bin").exists());
    }

    #[test]
    fn test_safe_extract_tar_valid_archive() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("tar_extracted");

        let mut tar_builder = tar::Builder::new(Vec::new());

        let mut header1 = tar::Header::new_gnu();
        header1.set_path("hello.txt").unwrap();
        header1.set_size(12);
        header1.set_mode(0o644);
        header1.set_cksum();
        tar_builder.append(&header1, &b"hello tar world"[..12]).unwrap();

        let mut header2 = tar::Header::new_gnu();
        header2.set_path("sub/nested.txt").unwrap();
        header2.set_size(14);
        header2.set_mode(0o644);
        header2.set_cksum();
        tar_builder.append(&header2, &b"nested tar file"[..14]).unwrap();

        let tar_data = tar_builder.into_inner().unwrap();
        let limits = ArchiveSecurityLimits::default();

        let count = safe_extract_tar(&tar_data[..], &dest, &limits).unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            std::fs::read_to_string(dest.join("hello.txt")).unwrap(),
            "hello tar wo"
        );
        assert_eq!(
            std::fs::read_to_string(dest.join("sub").join("nested.txt")).unwrap(),
            "nested tar fil"
        );
    }

    #[test]
    fn test_safe_extract_tar_rejects_path_traversal_and_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("tar_extracted");

        let mut tar_builder = tar::Builder::new(Vec::new());

        // File 1: Innocent
        let mut header1 = tar::Header::new_gnu();
        header1.set_path("innocent.txt").unwrap();
        header1.set_size(8);
        header1.set_mode(0o644);
        header1.set_cksum();
        tar_builder.append(&header1, &b"innocent"[..]).unwrap();

        // File 2: Evil traversal (malicious archive crafted with raw traversal path)
        let mut header2 = tar::Header::new_gnu();
        let evil_path = b"../evil.txt\0";
        header2.as_mut_bytes()[..evil_path.len()].copy_from_slice(evil_path);
        header2.set_size(4);
        header2.set_mode(0o644);
        header2.set_cksum();
        tar_builder.append(&header2, &b"evil"[..]).unwrap();

        let tar_data = tar_builder.into_inner().unwrap();
        let limits = ArchiveSecurityLimits::default();

        let res = safe_extract_tar(&tar_data[..], &dest, &limits);
        assert!(res.is_err());

        // Verify rollback: innocent.txt must be deleted!
        assert!(!dest.join("innocent.txt").exists());
        assert!(!dest.join("../evil.txt").exists());
    }

    #[test]
    fn test_safe_extract_tar_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("tar_extracted");

        let mut tar_builder = tar::Builder::new(Vec::new());

        let mut header = tar::Header::new_gnu();
        header.set_path("link.txt").unwrap();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_link_name("/etc/passwd").unwrap();
        header.set_size(0);
        header.set_cksum();
        tar_builder.append(&header, std::io::empty()).unwrap();

        let tar_data = tar_builder.into_inner().unwrap();
        let limits = ArchiveSecurityLimits::default();

        let res = safe_extract_tar(&tar_data[..], &dest, &limits);
        assert!(res.is_err());
        match res.unwrap_err() {
            ArchiveSecurityError::IllegalLinkDetected { path } => {
                assert_eq!(path, "link.txt");
            }
            other => panic!("Expected IllegalLinkDetected, got {:?}", other),
        }
    }
}
