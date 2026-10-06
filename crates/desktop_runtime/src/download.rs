//! Download manager with progress tracking and concurrency control.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::error::{Result, RuntimeError};
use parking_lot::RwLock;
use tokio::sync::Semaphore;
use tracing::{error, info};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub id: u64,
    pub url: String,
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub status: DownloadStatus,
}

struct DownloadState {
    url: String,
    filename: String,
    #[allow(dead_code)]
    dest_path: PathBuf,
    bytes_downloaded: Arc<AtomicU64>,
    total_bytes: Option<u64>,
    status: DownloadStatus,
}

pub struct DownloadManager {
    downloads: Arc<RwLock<HashMap<u64, DownloadState>>>,
    counter: AtomicU64,
    semaphore: Arc<Semaphore>,
    download_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(max_concurrent: usize, download_dir: Option<&str>) -> Result<Self> {
        let dir = download_dir.map(PathBuf::from).unwrap_or_else(|| {
            dirs::download_dir().unwrap_or_else(|| std::env::temp_dir().join("voxy_downloads"))
        });
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            downloads: Arc::new(RwLock::new(HashMap::new())),
            counter: AtomicU64::new(0),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            download_dir: dir,
        })
    }

    pub async fn download(&self, url: &str, filename: &str) -> Result<u64> {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        let safe_name = sanitize_filename(filename)?;
        let dest = self.download_dir.join(&safe_name);
        // Download to a temp file and only move it into place on success so a
        // failed or partial download never leaves a file that looks complete.
        let temp_dest = dest.with_extension(format!("{}.part", id));
        let bytes = Arc::new(AtomicU64::new(0));
        self.downloads.write().insert(
            id,
            DownloadState {
                url: url.to_string(),
                filename: safe_name.clone(),
                dest_path: dest.clone(),
                bytes_downloaded: bytes.clone(),
                total_bytes: None,
                status: DownloadStatus::Pending,
            },
        );

        let url_clone = url.to_string();
        let filename_clone = safe_name;
        let sem = self.semaphore.clone();
        let dls = self.downloads.clone();

        tokio::spawn(async move {
            let _permit = sem.acquire().await;
            if let Some(s) = dls.write().get_mut(&id) {
                s.status = DownloadStatus::Downloading;
            }
            match do_download(&url_clone, &temp_dest, &dest, bytes.clone()).await {
                Ok(total) => {
                    if let Some(s) = dls.write().get_mut(&id) {
                        s.status = DownloadStatus::Completed;
                        s.total_bytes = Some(total);
                    }
                    info!("Download completed: {} ({} bytes)", filename_clone, total);
                }
                Err(e) => {
                    if let Some(s) = dls.write().get_mut(&id) {
                        s.status = DownloadStatus::Failed(e.to_string());
                    }
                    error!("Download failed: {} - {}", filename_clone, e);
                }
            }
        });

        info!("Download started: {} -> {} (id={})", url, filename, id);
        Ok(id)
    }

    pub fn cancel(&self, id: u64) -> Result<()> {
        if let Some(s) = self.downloads.write().get_mut(&id) {
            s.status = DownloadStatus::Cancelled;
        }
        Ok(())
    }

    pub fn progress(&self, id: u64) -> Option<DownloadProgress> {
        self.downloads.read().get(&id).map(|s| DownloadProgress {
            id,
            url: s.url.clone(),
            filename: s.filename.clone(),
            bytes_downloaded: s.bytes_downloaded.load(Ordering::Relaxed),
            total_bytes: s.total_bytes,
            status: s.status.clone(),
        })
    }

    pub fn all_downloads(&self) -> Vec<DownloadProgress> {
        self.downloads
            .read()
            .iter()
            .map(|(id, s)| DownloadProgress {
                id: *id,
                url: s.url.clone(),
                filename: s.filename.clone(),
                bytes_downloaded: s.bytes_downloaded.load(Ordering::Relaxed),
                total_bytes: s.total_bytes,
                status: s.status.clone(),
            })
            .collect()
    }

    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }
}

async fn do_download(url: &str, temp: &Path, dest: &Path, bytes: Arc<AtomicU64>) -> Result<u64> {
    // 1. SSRF & Scheme Validation
    let parsed_url = reqwest::Url::parse(url)
        .map_err(|e| RuntimeError::Download(format!("Invalid download URL: {e}")))?;
    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return Err(RuntimeError::Download(format!(
            "Unsupported scheme '{}'. Only HTTP/HTTPS permitted.",
            parsed_url.scheme()
        )));
    }
    if let Some(host) = parsed_url.host_str() {
        let host_lower = host.to_lowercase();
        if host_lower == "localhost"
            || host_lower == "127.0.0.1"
            || host_lower == "::1"
            || host_lower.starts_with("127.")
            || host_lower == "169.254.169.254"
        {
            return Err(RuntimeError::Download(
                "Downloads from loopback or cloud metadata IP addresses are prohibited".to_string(),
            ));
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| RuntimeError::Download(format!("Client build failed: {}", e)))?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| RuntimeError::Download(format!("Request failed: {}", e)))?;
    if !resp.status().is_success() {
        return Err(RuntimeError::Download(format!("HTTP {}", resp.status())));
    }
    let mut file = tokio::fs::File::create(temp)
        .await
        .map_err(|e| RuntimeError::Download(format!("File create failed: {}", e)))?;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;
    const MAX_DOWNLOAD_BYTES: u64 = 500 * 1024 * 1024; // 500 MB hard limit

    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| RuntimeError::Download(format!("Stream error: {}", e)))?;
        downloaded += chunk.len() as u64;
        if downloaded > MAX_DOWNLOAD_BYTES {
            let _ = tokio::fs::remove_file(temp).await;
            return Err(RuntimeError::Download(format!(
                "Download exceeds maximum security limit of {MAX_DOWNLOAD_BYTES} bytes"
            )));
        }
        file.write_all(&chunk)
            .await
            .map_err(|e| RuntimeError::Download(format!("Write error: {}", e)))?;
        bytes.store(downloaded, Ordering::Relaxed);
    }
    file.flush()
        .await
        .map_err(|e| RuntimeError::Download(format!("Flush error: {}", e)))?;
    drop(file);

    // Move the fully-downloaded temp file into place. On failure, clean up the
    // partial temp file so nothing misleading is left behind.
    if let Err(e) = tokio::fs::rename(temp, dest).await {
        let _ = tokio::fs::remove_file(temp).await;
        return Err(RuntimeError::Download(format!("Finalize error: {}", e)));
    }
    Ok(downloaded)
}

/// Validate a download filename against path traversal and platform quirks.
///
/// Rejects absolute paths, path separators, and Windows reserved device names
/// so a caller-controlled filename cannot escape the download directory.
fn sanitize_filename(filename: &str) -> Result<String> {
    if filename.is_empty() || filename.contains(['/', '\\']) {
        return Err(RuntimeError::Download(format!(
            "Invalid download filename: {filename:?}"
        )));
    }

    let file_name = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if file_name.is_empty() || file_name == "." || file_name == ".." {
        return Err(RuntimeError::Download(format!(
            "Invalid download filename: {filename:?}"
        )));
    }

    // Windows trims trailing spaces and dots and treats the resulting name as
    // the real filename, which can cause collisions; refuse them up front.
    let trimmed = file_name.trim_end_matches([' ', '.']);
    if trimmed.is_empty() {
        return Err(RuntimeError::Download(format!(
            "Invalid download filename: {filename:?}"
        )));
    }

    // Windows reserved device names (with or without extension).
    let stem = trimmed
        .split('.')
        .next()
        .unwrap_or(trimmed)
        .to_ascii_uppercase();
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        return Err(RuntimeError::Download(format!(
            "Invalid download filename: {filename:?}"
        )));
    }

    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_manager_creation() {
        assert!(DownloadManager::new(3, None).is_ok());
    }

    #[test]
    fn download_progress_empty() {
        let mgr = DownloadManager::new(3, None).unwrap();
        assert!(mgr.progress(0).is_none());
        assert!(mgr.all_downloads().is_empty());
    }

    #[test]
    fn download_cancel_nonexistent() {
        let mgr = DownloadManager::new(3, None).unwrap();
        assert!(mgr.cancel(999).is_ok());
    }

    #[test]
    fn sanitize_accepts_plain_filenames() {
        assert_eq!(sanitize_filename("model.bin").unwrap(), "model.bin");
        assert_eq!(sanitize_filename("readme.txt").unwrap(), "readme.txt");
        assert_eq!(
            sanitize_filename("file with spaces.zip").unwrap(),
            "file with spaces.zip"
        );
    }

    #[test]
    fn sanitize_rejects_path_traversal() {
        assert!(sanitize_filename("../../etc/passwd").is_err());
        assert!(sanitize_filename("../evil.exe").is_err());
        assert!(sanitize_filename("C:\\windows\\system32\\x.dll").is_err());
        assert!(sanitize_filename("").is_err());
        assert!(sanitize_filename(".").is_err());
        assert!(sanitize_filename("..").is_err());
    }

    #[test]
    fn sanitize_rejects_windows_reserved_names() {
        assert!(sanitize_filename("CON").is_err());
        assert!(sanitize_filename("con.txt").is_err());
        assert!(sanitize_filename("NUL").is_err());
        assert!(sanitize_filename("COM1").is_err());
        assert!(sanitize_filename("LPT9").is_err());
    }

    #[test]
    fn sanitize_normalizes_trailing_dots_or_spaces() {
        // Windows trims trailing dots/spaces from real filenames; normalizing
        // avoids two names resolving to the same file.
        assert_eq!(sanitize_filename("evil.").unwrap(), "evil");
        assert_eq!(sanitize_filename("evil ").unwrap(), "evil");
    }

    #[test]
    fn sanitize_trims_trailing_whitespace() {
        assert_eq!(sanitize_filename("file.txt ").unwrap(), "file.txt");
    }
}
