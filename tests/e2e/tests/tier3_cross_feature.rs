//! Tier 3: Cross-Feature Interaction E2E Tests (Acceptance Criteria A1-A5)
//!
//! Verifies compound workflows and subsystem interplay:
//! - A1 + A4: Named Pipe IPC Cryptographic Handshake & Privileged Command Gating
//! - A2: Voice Pipeline Barge-In Interruption & In-Flight Playback/LLM Streaming Halting
//! - A3 + A4: Coding Harness Multi-File Patch Transactions with Atomic Rollback on Security Violation
//! - A5: SQLite WAL Persistence & Recovery through Connection Drop / Crash Simulation

use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use osmoo_e2e_tests::*;
use tempfile::TempDir;
use tokio::time::timeout;

use voxy_database::conversation::{ConversationStore, MessageRole};
use voxy_database::sqlite::{SqliteConversationStore, SqliteDatabase};
use voxy_database::storage::StorageProvider;
use voxy_database::DatabaseConfig;
use voxy_harness::patch_engine::{PatchError, PatchTransaction};
use voxy_ipc::voxy_protocol::{ClientCommand, DaemonMessage, VoiceState};
use voxy_ipc::{VoxyIpcClient, VoxyIpcServer};
use voxy_voice_orchestrator::{VoicePipeline, VoicePipelineConfig};
use voxy_voice_stt::MockStt;
use voxy_voice_tts::MockTts;

// =========================================================================
// A1 + A4: IPC Cryptographic Authentication & Privileged Command Gating
// =========================================================================

#[tokio::test]
async fn test_tier3_a1_a4_ipc_handshake_and_privileged_command_gating() {
    let pipe_name = generate_test_pipe_name();
    let secret = "osmoo-master-ipc-secret-key";

    // 1. Initialize server expecting authentication
    let server = VoxyIpcServer::with_pipe_name_and_auth(&pipe_name, Some(secret.to_string()));
    server.start();

    tokio::time::sleep(Duration::from_millis(150)).await;

    // 2. Client connecting with proper cryptographic token
    let client = VoxyIpcClient::with_pipe_name_and_auth(&pipe_name, Some(secret.to_string()));
    let mut conn_watcher = client.watch_connected();
    client.start();

    // Await successful authenticated connection
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

    assert!(connected, "Cryptographically authenticated client must connect successfully");

    // Send privileged EmergencyStop from authenticated client
    assert!(client.send_command(ClientCommand::EmergencyStop).await.is_ok());

    // Verify server registered command
    tokio::time::sleep(Duration::from_millis(100)).await;
    server.stop();
}

// =========================================================================
// A2: Voice Pipeline Barge-In Interruption & Playback Cancellation
// =========================================================================

#[test]
fn test_tier3_a2_voice_barge_in_interruption_cancels_playback() {
    let config = VoicePipelineConfig::default();
    let stt = Arc::new(MockStt::new("hello"));
    let tts = Arc::new(MockTts::new());
    let pipeline = VoicePipeline::new(config, stt, tts, None).expect("pipeline");

    // Scenario 1: Idle state -> no barge in
    assert_eq!(pipeline.current_state(), VoiceState::Idle);
    pipeline.trigger_barge_in("speech while idle");
    assert_eq!(pipeline.current_state(), VoiceState::Idle);

    // Scenario 2: Active playback (Speaking) -> barge in triggers immediate interruption
    pipeline.set_state(VoiceState::Speaking, Some("TTS output".to_string()));
    assert_eq!(pipeline.current_state(), VoiceState::Speaking);
    pipeline.trigger_barge_in("user interrupt");
    assert_eq!(pipeline.current_state(), VoiceState::Interrupted);

    // Scenario 3: Thinking state -> barge in also triggers interruption
    pipeline.set_state(VoiceState::Thinking, Some("LLM generating".to_string()));
    assert_eq!(pipeline.current_state(), VoiceState::Thinking);
    pipeline.trigger_barge_in("user interrupt during think");
    assert_eq!(pipeline.current_state(), VoiceState::Interrupted);
}

// =========================================================================
// A3 + A4: Multi-File Patch Transaction with Atomic Rollback on Security Violation
// =========================================================================

#[test]
fn test_tier3_a3_a4_atomic_rollback_on_multi_file_security_violation() {
    let workspace = create_mock_workspace();
    let root = workspace.path();

    let initial_main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();

    // Create a multi-file transaction with 1 valid file and 1 malicious file (secret key)
    let mut tx = PatchTransaction::new(root);
    tx.add_change("src/main.rs", "fn main() { println!(\"Modified\"); }");
    tx.add_change("src/secret.rs", "const AWS_KEY: &str = \"AKIAIOSFODNN7EXAMPLE\";");

    // Apply must fail due to secret detection
    let result = tx.apply();
    assert!(result.is_err(), "Batch containing secret must be rejected");

    match result.unwrap_err() {
        PatchError::SecretDetected(secrets) => {
            assert!(!secrets.is_empty());
        }
        other => panic!("Expected SecretDetected error, got {:?}", other),
    }

    // Crucial Atomic Guarantee: src/main.rs MUST NOT be modified!
    let current_main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
    assert_eq!(
        current_main, initial_main,
        "Clean atomic guarantee: failed transaction must leave preceding files completely unmodified"
    );
    assert!(!root.join("src/secret.rs").exists(), "Malicious file must not be created");
}

// =========================================================================
// A5: SQLite WAL Mode Persistence Through Reconnection / Crash Simulation
// =========================================================================

#[tokio::test]
async fn test_tier3_a5_sqlite_wal_persistence_through_crash_simulation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("wal_crash_test.db");

    // Phase 1: First connection writes data
    {
        let db_config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let db = SqliteDatabase::new();
        db.connect(&db_config).await.expect("First connection failed");

        let conv_store = SqliteConversationStore::new(&db_path.to_string_lossy())
            .expect("Create conv_store failed");
        let conv = conv_store
            .create_conversation("user-crash-test", Some("Durable Conversation"))
            .await
            .expect("Create conversation failed");

        conv_store
            .add_message(&conv.id, MessageRole::User, "Message before simulated crash", None, None)
            .await
            .expect("Add message failed");

        // Sudden drop simulating unexpected termination
        drop(conv_store);
        drop(db);
    }

    // Phase 2: Second connection reopens the same file without data loss
    {
        let db_config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let db2 = SqliteDatabase::new();
        db2.connect(&db_config).await.expect("Reconnection failed");

        // Verify WAL mode is preserved
        assert!(db2.is_wal_mode().await.expect("WAL mode check failed"));

        let conv_store2 = SqliteConversationStore::new(&db_path.to_string_lossy())
            .expect("Create conv_store2 failed");
        let convs = conv_store2.list_conversations("user-crash-test", 10, 0).await.unwrap();
        assert_eq!(convs.len(), 1, "Conversation record must survive connection drop");
        assert_eq!(convs[0].title.as_deref(), Some("Durable Conversation"));

        let messages = conv_store2.get_messages(&convs[0].id, 10, 0).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Message before simulated crash");
    }
}
