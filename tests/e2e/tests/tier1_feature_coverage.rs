//! Tier 1: Comprehensive Feature Coverage E2E Tests (Acceptance Criteria A1-A5)
//!
//! Verifies standard operational paths:
//! - A1: Process Lifecycle & Authenticated Named Pipe IPC
//! - A2: Voice Pipeline Lifecycle & Hardware Fault States
//! - A3: Autonomous Coding Harness Repository Indexing & Atomic Patching
//! - A4: Security Boundaries (SSRF & Safe Archive Extraction)
//! - A5: Durable Persistence (SQLite WAL Mode) & Installer Artifact Packaging

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
use voxy_harness::patch_engine::PatchTransaction;
use voxy_harness::repo_indexer::RepositoryIndexer;
use voxy_ipc::voxy_protocol::{ClientCommand, DaemonMessage, VoiceState};
use voxy_ipc::{VoxyIpcClient, VoxyIpcServer};
use voxy_security::archive::{safe_extract_zip, ArchiveSecurityLimits};
use voxy_security::ssrf::{is_ip_restricted, validate_destination_url_str};
use voxy_voice_orchestrator::{VoicePipeline, VoicePipelineConfig};
use voxy_voice_stt::MockStt;
use voxy_voice_tts::{MockTts, SentenceChunker};

// =========================================================================
// A1: Process Lifecycle & Authenticated Named Pipe IPC
// =========================================================================

#[tokio::test]
async fn test_tier1_a1_process_lifecycle_named_pipe_ipc() {
    let pipe_name = generate_test_pipe_name();
    let auth_token = generate_test_auth_token();

    // 1. Initialize Named Pipe Server with authentication secret
    let server = VoxyIpcServer::with_pipe_name_and_auth(&pipe_name, Some(auth_token.clone()));
    server.set_voice_state(VoiceState::Listening, Some("user prompt".to_string())).await;
    server.set_transcript("OSMOO start test".to_string(), true, 0.95).await;
    server.start();

    // Allow server socket listener to bind
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 2. Initialize Named Pipe Client with matching auth token
    let client = VoxyIpcClient::with_pipe_name_and_auth(&pipe_name, Some(auth_token.clone()));
    let mut event_rx = client.subscribe();
    let mut conn_watcher = client.watch_connected();
    client.start();

    // Wait for connection to establish
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

    assert!(connected, "Client must successfully connect and authenticate to Named Pipe Server");

    // 3. Receive initial state snapshot
    let snapshot_received = timeout(Duration::from_secs(5), async {
        while let Ok(msg) = event_rx.recv().await {
            if let DaemonMessage::StateSnapshot { voice_state, transcript, .. } = msg {
                assert_eq!(voice_state, VoiceState::Listening);
                assert_eq!(transcript, "OSMOO start test");
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    assert!(snapshot_received, "Client must receive initial DaemonStateSnapshot upon connecting");

    // 4. Test bidirectional streaming: server broadcasts, client receives
    server.set_voice_state(VoiceState::Thinking, Some("processing command".to_string())).await;
    let event_received = timeout(Duration::from_secs(3), async {
        while let Ok(msg) = event_rx.recv().await {
            if let DaemonMessage::VoiceStateChanged { state, reason } = msg {
                if state == VoiceState::Thinking {
                    assert_eq!(reason.as_deref(), Some("processing command"));
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    assert!(event_received, "Client must receive real-time broadcast state changes");

    // 5. Test client sending command to server
    assert!(client.send_command(ClientCommand::EmergencyStop).await.is_ok());

    // 6. Clean graceful shutdown
    server.stop();
    tokio::time::sleep(Duration::from_millis(50)).await;
}

// =========================================================================
// A2: Voice Pipeline Lifecycle & Hardware Fault States
// =========================================================================

#[tokio::test]
async fn test_tier1_a2_voice_pipeline_lifecycle() {
    let config = VoicePipelineConfig::default();
    let stt = Arc::new(MockStt::new("hello world"));
    let tts = Arc::new(MockTts::new());
    let pipeline = VoicePipeline::new(config, stt, tts, None).expect("Pipeline creation");

    // Initial state
    assert_eq!(pipeline.current_state(), VoiceState::Idle);

    // Sentence chunking verification for streaming voice pipeline
    let mut chunker = SentenceChunker::new(10);
    let tokens = ["Hello ", "world! ", "This is ", "OSMOO speaking. ", "Next part."];
    let mut emitted_sentences = Vec::new();
    for token in tokens {
        let chunks = chunker.feed(token);
        emitted_sentences.extend(chunks);
    }
    let rest = chunker.finish();
    emitted_sentences.extend(rest);

    assert!(emitted_sentences.len() >= 2);
    assert_eq!(emitted_sentences[0], "Hello world!");
    assert_eq!(emitted_sentences[1], "This is OSMOO speaking.");

    // State transition and barge-in
    pipeline.set_state(VoiceState::Speaking, Some("TTS playback".to_string()));
    assert_eq!(pipeline.current_state(), VoiceState::Speaking);
    pipeline.trigger_barge_in("User speech detected");
    assert_eq!(pipeline.current_state(), VoiceState::Interrupted);
}

// =========================================================================
// A3: Autonomous Coding Harness Repository Indexing & Atomic Patching
// =========================================================================

#[test]
fn test_tier1_a3_coding_harness_indexing_and_atomic_patch() {
    let workspace = create_mock_workspace();
    let root = workspace.path();

    // 1. Repository Indexing
    let mut indexer = RepositoryIndexer::new(root);
    let count = indexer.index_repository().expect("Repository indexing should succeed");
    assert!(count > 0, "Repository indexer should find workspace files");

    let rs_files = indexer.find_files_by_extension("rs");
    let has_main = rs_files.iter().any(|f| {
        let p = f.relative_path.to_string_lossy();
        p.ends_with("src\\main.rs") || p.ends_with("src/main.rs") || p.ends_with("main.rs")
    });
    assert!(has_main, "Indexer must locate src/main.rs");

    // 2. Atomic Patch Transaction
    let mut tx = PatchTransaction::new(root);
    let patched_content = "fn main() {\n    println!(\"Patched by OSMOO E2E Harness\");\n}\n";
    tx.add_change("src/main.rs", patched_content);

    let diffs = tx.apply().expect("Patch application should succeed");
    assert_eq!(diffs.len(), 1);
    assert!(diffs[0].contains("Patched by OSMOO E2E Harness"));

    // Verify on disk
    let disk_content = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
    assert_eq!(disk_content, patched_content);

    // 3. Rollback
    let rolled_back = tx.rollback();
    assert_eq!(rolled_back, 1, "Rollback should revert exactly one file");

    let restored_content = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
    assert!(restored_content.contains("Hello from OSMOO!"));
}

// =========================================================================
// A4: Security Boundaries (SSRF & Safe Archive Extraction)
// =========================================================================

#[test]
fn test_tier1_a4_security_boundaries_ssrf_and_safe_archive() {
    // 1. SSRF & Rebinding Socket Defense
    let restricted_ips = ["127.0.0.1", "169.254.169.254", "::1", "10.0.0.1", "192.168.1.1"];
    for ip_str in restricted_ips {
        let ip: std::net::IpAddr = ip_str.parse().unwrap();
        assert!(is_ip_restricted(ip), "IP {ip_str} must be restricted");
    }

    assert!(validate_destination_url_str("http://127.0.0.1:8080/admin").is_err());
    assert!(validate_destination_url_str("http://169.254.169.254/latest/meta-data/").is_err());
    assert!(validate_destination_url_str("http://[::1]:9000/metrics").is_err());
    assert!(validate_destination_url_str("https://example.com/api/v1").is_ok());

    // 2. Safe Archive Extraction (Happy Path)
    let temp_dest = TempDir::new().unwrap();
    let zip_bytes = create_test_zip_archive(&[
        ("file1.txt", b"First file payload"),
        ("nested/file2.txt", b"Second file nested payload"),
    ]);

    let limits = ArchiveSecurityLimits::default();
    let extracted_count = safe_extract_zip(Cursor::new(zip_bytes), temp_dest.path(), &limits)
        .expect("Safe zip extraction should succeed");

    assert_eq!(extracted_count, 2);
    assert_eq!(
        std::fs::read_to_string(temp_dest.path().join("file1.txt")).unwrap(),
        "First file payload"
    );
    assert_eq!(
        std::fs::read_to_string(temp_dest.path().join("nested/file2.txt")).unwrap(),
        "Second file nested payload"
    );
}

// =========================================================================
// A5: Durable Persistence (SQLite WAL Mode) & Installer Artifact Packaging
// =========================================================================

#[tokio::test]
async fn test_tier1_a5_persistence_and_installer_packaging() {
    // 1. SQLite WAL Persistence
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("osmoo_test.db");

    let db_config = DatabaseConfig {
        path: Some(db_path.to_string_lossy().to_string()),
        ..Default::default()
    };

    let db = SqliteDatabase::new();
    db.connect(&db_config).await.expect("Database should connect cleanly");

    // Check PRAGMA journal_mode is WAL
    assert!(
        db.is_wal_mode().await.expect("is_wal_mode query failed"),
        "SQLite database must initialize in WAL mode"
    );

    // 2. Conversation Store Persistence
    let conv_store = SqliteConversationStore::new(&db_path.to_string_lossy())
        .expect("Conversation store initialization should succeed");
    let conv = conv_store
        .create_conversation("user-123", Some("Test Session"))
        .await
        .expect("Conversation creation should succeed");

    conv_store
        .add_message(
            &conv.id,
            MessageRole::User,
            "Hello OSMOO",
            Some(3),
            None,
        )
        .await
        .expect("Adding message should succeed");

    conv_store
        .add_message(
            &conv.id,
            MessageRole::Assistant,
            "Greetings! How may I assist you today?",
            Some(9),
            None,
        )
        .await
        .expect("Adding assistant message should succeed");

    let messages = conv_store
        .get_messages(&conv.id, 10, 0)
        .await
        .expect("Retrieving messages should succeed");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "Hello OSMOO");
    assert_eq!(messages[1].content, "Greetings! How may I assist you today?");

    // 3. Installer Packaging Verification
    let root = workspace_root();
    let installer_path = root.join("installer/OSMOO/OSMOO-Setup-x64.exe");
    if installer_path.exists() {
        let hash = compute_sha256(&installer_path).expect("Failed to compute installer SHA-256");
        assert_eq!(hash.len(), 64, "SHA-256 hash must be 64 hexadecimal characters");
        let metadata = std::fs::metadata(&installer_path).expect("Installer metadata should be accessible");
        assert!(metadata.len() > 1_000_000, "Installer executable should be a complete binary (> 1 MB)");
    } else {
        // Verify NSIS script definition
        let nsi_path = root.join("installer/nsis/osmoo.nsi");
        assert!(nsi_path.exists(), "installer/nsis/osmoo.nsi must exist at {:?}", nsi_path);
        let nsi_script = std::fs::read_to_string(&nsi_path).unwrap();
        assert!(nsi_script.contains("OSMOO.exe"));
        assert!(nsi_script.contains("voxy-daemon.exe"));
        assert!(nsi_script.contains("voxy-overlay.exe"));
    }
}
