//! Shared E2E Test Infrastructure & Utilities for OSMOO
//!
//! Provides reusable test fixtures, named pipe generators, audio buffer mocks,
//! archive factories, and assertion helpers for Acceptance Criteria A1-A5.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

/// Returns the root repository path from the test runner.
pub fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if manifest.ends_with("e2e") {
        if let Some(parent) = manifest.parent() {
            if parent.ends_with("tests") {
                return parent.parent().unwrap_or(parent).to_path_buf();
            }
            return parent.to_path_buf();
        }
    }
    manifest
}

/// Generates a unique, collision-free Windows named pipe path for isolated tests.
pub fn generate_test_pipe_name() -> String {
    format!(r"\\.\pipe\osmoo-test-{}", uuid::Uuid::new_v4().simple())
}

/// Generates a test auth secret for IPC handshake verification.
pub fn generate_test_auth_token() -> String {
    format!("osmoo-auth-token-{}", uuid::Uuid::new_v4().simple())
}

/// Generates an HMAC / salted SHA-256 token matching the server's expected validation logic.
pub fn compute_client_auth_token(secret: &str, nonce: &str, timestamp_ms: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{secret}:{nonce}:{timestamp_ms}").as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Computes SHA-256 checksum of any file.
pub fn compute_sha256(path: impl AsRef<Path>) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Creates a temporary workspace sandbox containing realistic repo layout.
pub fn create_mock_workspace() -> TempDir {
    let dir = TempDir::new().expect("Failed to create temporary workspace directory");
    let root = dir.path();

    // Create .git directory to simulate repository boundary
    let git_dir = root.join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    std::fs::write(git_dir.join("config"), "[core]\n\trepositoryformatversion = 0\n").unwrap();

    // Create standard source tree
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(
        src_dir.join("main.rs"),
        "fn main() {\n    println!(\"Hello from OSMOO!\");\n}\n",
    )
    .unwrap();

    // Create Cargo.toml and Cargo.lock
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"sample-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(root.join("Cargo.lock"), "# Lockfile v3\n").unwrap();

    // Create .env file with dummy variables
    std::fs::write(root.join(".env"), "PORT=8080\nDEBUG=false\n").unwrap();

    dir
}

/// Creates an audio chunk with synthetic PCM data for voice testing.
pub fn create_synthetic_audio_chunk(
    sample_rate: u32,
    channels: u8,
    duration_ms: usize,
    frequency_hz: f32,
) -> Vec<f32> {
    let num_samples = (sample_rate as usize * duration_ms) / 1000 * channels as usize;
    let mut samples = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let val = (2.0 * std::f32::consts::PI * frequency_hz * t).sin() * 0.5;
        samples.push(val);
    }
    samples
}

/// Creates a safe in-memory zip archive for archive extraction testing.
pub fn create_test_zip_archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;

    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        for (name, data) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    buf
}

/// Creates a malicious zip archive with path traversal (Zip Slip).
pub fn create_traversal_zip_archive(target_filename: &str) -> Vec<u8> {
    use std::io::Cursor;
    use zip::write::SimpleFileOptions;

    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        let traversal_name = format!("../../escape_{target_filename}");
        zip.start_file(traversal_name, options).unwrap();
        zip.write_all(b"malicious escape content").unwrap();
        zip.finish().unwrap();
    }
    buf
}
