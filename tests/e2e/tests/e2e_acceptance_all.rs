//! Unified Acceptance Verification Test Runner (A1-A5)
//!
//! Validates the entire acceptance criteria matrix in a single test invocation:
//! - [x] A1. Process Lifecycle & IPC
//! - [x] A2. Voice Pipeline & Interruption Handling
//! - [x] A3. Coding Harness & Secret Defense
//! - [x] A4. Security Boundaries (SSRF & Safe Archives)
//! - [x] A5. Persistence (SQLite WAL) & Release Packaging

use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use osmoo_e2e_tests::*;
use tempfile::TempDir;
use tokio::time::timeout;

use voxy_database::conversation::{ConversationStore, MessageRole};
use voxy_database::sqlite::{SqliteConversationStore, SqliteDatabase};
use voxy_database::storage::StorageProvider;
use voxy_database::DatabaseConfig;
use voxy_harness::patch_engine::{PatchError, PatchTransaction};
use voxy_harness::repo_indexer::RepositoryIndexer;
use voxy_ipc::voxy_protocol::{ClientCommand, DaemonMessage, VoiceState};
use voxy_ipc::{VoxyIpcClient, VoxyIpcServer};
use voxy_security::archive::{safe_extract_zip, ArchiveSecurityLimits};
use voxy_security::ssrf::{is_ip_restricted, validate_destination_url_str};
use voxy_voice_audio::AudioRingBuffer;
use voxy_voice_orchestrator::{VoicePipeline, VoicePipelineConfig};
use voxy_voice_stt::MockStt;
use voxy_voice_tts::MockTts;

#[tokio::test]
async fn test_acceptance_criteria_a1_process_lifecycle_and_ipc() {
    let pipe_name = generate_test_pipe_name();
    let auth_token = generate_test_auth_token();

    let server = VoxyIpcServer::with_pipe_name_and_auth(&pipe_name, Some(auth_token.clone()));
    server.start();

    tokio::time::sleep(Duration::from_millis(150)).await;

    let client = VoxyIpcClient::with_pipe_name_and_auth(&pipe_name, Some(auth_token.clone()));
    let mut conn_watcher = client.watch_connected();
    client.start();

    let connected = timeout(Duration::from_secs(5), async {
        while !*conn_watcher.borrow_and_update() {
            if conn_watcher.changed().await.is_err() {
                break;
            }
        }
        true
    })
    .await
    .unwrap_or(false);

    assert!(connected, "A1 Acceptance: Authenticated IPC connection failed");

    // Clean shutdown
    server.stop();
}

#[tokio::test]
async fn test_acceptance_criteria_a2_voice_pipeline() {
    let config = VoicePipelineConfig::default();
    let stt = Arc::new(MockStt::new("hello world"));
    let tts = Arc::new(MockTts::new());
    let pipeline = VoicePipeline::new(config, stt, tts, None).expect("A2 Acceptance: Pipeline creation failed");

    assert_eq!(pipeline.current_state(), VoiceState::Idle);

    // Audio ring buffer verification
    let rb = AudioRingBuffer::new(16000);
    rb.push(&vec![0.1; 1600]);
    assert_eq!(rb.available_samples(), 1600);
    rb.clear();
    assert_eq!(rb.available_samples(), 0);

    // Interruption / Barge-in check
    pipeline.set_state(VoiceState::Speaking, Some("TTS playback".to_string()));
    assert_eq!(pipeline.current_state(), VoiceState::Speaking);
    pipeline.trigger_barge_in("User speech detected");
    assert_eq!(pipeline.current_state(), VoiceState::Interrupted);
}

#[test]
fn test_acceptance_criteria_a3_coding_harness() {
    let workspace = create_mock_workspace();
    let root = workspace.path();

    // Indexing
    let mut indexer = RepositoryIndexer::new(root);
    let count = indexer.index_repository().expect("A3 Acceptance: Indexing failed");
    assert!(count > 0, "A3 Acceptance: Discovered zero files");

    // Atomic patch and rollback
    let mut tx = PatchTransaction::new(root);
    tx.add_change("src/main.rs", "// Modified by A3 Acceptance Test\n");
    let diffs = tx.apply().expect("A3 Acceptance: Patch apply failed");
    assert_eq!(diffs.len(), 1);

    let rolled_back = tx.rollback();
    assert_eq!(rolled_back, 1, "A3 Acceptance: Rollback failed");

    // Security boundary: reject protected file
    let mut tx_protected = PatchTransaction::new(root);
    tx_protected.add_change(".git/config", "pwned");
    assert!(
        tx_protected.apply().is_err(),
        "A3 Acceptance: Modifying .git must be strictly blocked"
    );

    // Security boundary: reject secrets
    let mut tx_secret = PatchTransaction::new(root);
    tx_secret.add_change("src/key.rs", "const KEY: &str = \"AKIAIOSFODNN7EXAMPLE\";");
    assert!(
        tx_secret.apply().is_err(),
        "A3 Acceptance: AWS key injection must be blocked"
    );
}

#[test]
fn test_acceptance_criteria_a4_security_boundaries() {
    // SSRF validation
    assert!(is_ip_restricted("127.0.0.1".parse().unwrap()));
    assert!(is_ip_restricted("169.254.169.254".parse().unwrap()));
    assert!(is_ip_restricted("::1".parse().unwrap()));
    assert!(validate_destination_url_str("http://127.0.0.1/").is_err());
    assert!(validate_destination_url_str("http://169.254.169.254/").is_err());

    // Safe Archive extraction
    let temp_dest = TempDir::new().unwrap();
    let zip_bytes = create_test_zip_archive(&[("safe.txt", b"Safe payload content")]);
    let limits = ArchiveSecurityLimits::default();
    let count = safe_extract_zip(Cursor::new(zip_bytes), temp_dest.path(), &limits)
        .expect("A4 Acceptance: Extraction failed");
    assert_eq!(count, 1);

    // Zip slip rejection
    let evil_zip = create_traversal_zip_archive("trap.txt");
    assert!(
        safe_extract_zip(Cursor::new(evil_zip), temp_dest.path(), &limits).is_err(),
        "A4 Acceptance: Zip Slip must be rejected"
    );
}

#[tokio::test]
async fn test_acceptance_criteria_a5_persistence_and_packaging() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("a5_acceptance.db");

    let db_config = DatabaseConfig {
        path: Some(db_path.to_string_lossy().to_string()),
        ..Default::default()
    };

    let db = SqliteDatabase::new();
    db.connect(&db_config).await.expect("A5 Acceptance: DB connect failed");

    assert!(
        db.is_wal_mode().await.expect("A5 Acceptance: WAL mode check failed"),
        "A5 Acceptance: Journal mode must be WAL"
    );

    let conv_store = SqliteConversationStore::new(&db_path.to_string_lossy())
        .expect("A5 Acceptance: Init conv store failed");
    let conv = conv_store
        .create_conversation("user-a5", Some("A5 Acceptance Session"))
        .await
        .unwrap();
    conv_store
        .add_message(&conv.id, MessageRole::User, "Test query", None, None)
        .await
        .unwrap();
    let msgs = conv_store.get_messages(&conv.id, 10, 0).await.unwrap();
    assert_eq!(msgs.len(), 1);

    // NSIS script presence
    let root = workspace_root();
    let nsi_path = root.join("installer/nsis/osmoo.nsi");
    assert!(nsi_path.exists(), "A5 Acceptance: installer/nsis/osmoo.nsi must exist at {:?}", nsi_path);
}
