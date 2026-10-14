//! Tier 4: Real-World End-to-End Scenarios (Acceptance Criteria A1-A5)
//!
//! Simulates realistic end-to-end user workflows:
//! - Scenario 1: Autonomous Coding Loop (Index -> Patch -> Diagnose -> Rollback)
//! - Scenario 2: Full Voice Interaction & Interruption Loop (Capture -> VAD -> STT -> Streaming -> Playback -> Barge-in)
//! - Scenario 3: Production Release Packaging & Deployment Verification (NSIS, SHA-256, Binaries)

use std::path::PathBuf;
use std::time::{Duration, Instant};

use osmoo_e2e_tests::*;
use tokio::time::timeout;

use voxy_harness::diagnostic_parser::{DiagnosticParser, DiagnosticSeverity};
use voxy_harness::patch_engine::PatchTransaction;
use voxy_harness::repo_indexer::RepositoryIndexer;
use voxy_ipc::voxy_protocol::{ClientCommand, DaemonMessage, VoiceState};
use voxy_ipc::{VoxyIpcClient, VoxyIpcServer};
use voxy_voice::detection::EnergyVadDetector;
use voxy_voice_orchestrator::{AudioChunk, VadDetector};
use voxy_voice_stream::barge_in::{BargeInCoordinator, BargeInState};
use voxy_voice_stream::chunker::SentenceChunker;

// =========================================================================
// Scenario 1: Autonomous Coding Loop
// =========================================================================

#[test]
fn test_tier4_scenario1_autonomous_coding_loop() {
    let workspace = create_mock_workspace();
    let root = workspace.path();

    // Setup multi-file project
    std::fs::write(
        root.join("src/calculator.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    // 1. Index repository
    let mut indexer = RepositoryIndexer::new(root);
    let count = indexer.index_repository().expect("Indexing must succeed");
    assert!(count >= 2, "Workspace must contain indexed files");
    let rs_files = indexer.find_files_by_extension("rs");
    assert!(rs_files.len() >= 2);

    // 2. Apply multi-file refactoring patch
    let mut tx = PatchTransaction::new(root);
    tx.add_change(
        "src/calculator.rs",
        "pub fn add(a: i32, b: i32) -> i32 {\n    // Refactored with logging\n    a + b\n}\n\npub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}\n",
    );
    tx.add_change(
        "src/main.rs",
        "mod calculator;\n\nfn main() {\n    let sum = calculator::add(2, 3);\n    println!(\"Sum: {sum}\");\n}\n",
    );

    let diffs = tx.apply().expect("Patch application must succeed");
    assert_eq!(diffs.len(), 2, "Both files must generate diffs");

    // 3. Simulate compiler diagnostic error parsing
    let rustc_error_line = "warning[unused_variables]: unused variable: `sum`\n --> src/main.rs:4:9";
    let report = DiagnosticParser::parse(rustc_error_line);
    assert_eq!(report.items.len(), 1);
    assert_eq!(report.items[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(report.items[0].file_path, Some(PathBuf::from("src/main.rs")));
    assert_eq!(report.items[0].line, Some(4));

    // 4. Test atomic rollback: user cancels task or test fails
    let rolled_back = tx.rollback();
    assert_eq!(rolled_back, 2, "Both files must be rolled back cleanly");

    let restored_main = std::fs::read_to_string(root.join("src/main.rs")).unwrap();
    assert!(restored_main.contains("Hello from OSMOO!"));
    let restored_calc = std::fs::read_to_string(root.join("src/calculator.rs")).unwrap();
    assert!(!restored_calc.contains("multiply"));
}

// =========================================================================
// Scenario 2: Full Voice Interaction & Interruption Loop
// =========================================================================

#[tokio::test]
async fn test_tier4_scenario2_voice_interaction_and_interruption_loop() {
    let pipe_name = generate_test_pipe_name();
    let auth_token = generate_test_auth_token();

    // 1. Start Daemon IPC Server
    let server = VoxyIpcServer::with_pipe_name_and_auth(&pipe_name, Some(auth_token.clone()));
    server.start();

    tokio::time::sleep(Duration::from_millis(150)).await;

    // 2. Connect Overlay Client
    let client = VoxyIpcClient::with_pipe_name_and_auth(&pipe_name, Some(auth_token.clone()));
    let mut conn_watcher = client.watch_connected();
    let mut event_rx = client.subscribe();
    client.start();

    // Await authenticated connection
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
    assert!(connected, "Client must connect to server successfully");

    // Await initial snapshot to confirm full pipe handshake and subscription
    let got_snapshot = timeout(Duration::from_secs(3), async {
        while let Ok(msg) = event_rx.recv().await {
            if matches!(msg, DaemonMessage::StateSnapshot { .. }) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(got_snapshot, "Client must receive initial StateSnapshot from server");

    // 3. User Speech Audio Ingestion -> VAD Trigger
    let vad = EnergyVadDetector::new(0.01, 16000).with_min_speech_frames(1);
    let speech_samples = create_synthetic_audio_chunk(16000, 1, 100, 440.0);
    let chunk = AudioChunk {
        data: speech_samples,
        sample_rate: 16000,
        channels: 1,
        timestamp: chrono::Utc::now(),
        sequence: 1,
        is_final: false,
    };
    let is_voice = vad.is_voice(&chunk).await.expect("VAD check failed");
    assert!(is_voice, "VAD must detect synthetic voice signal");

    // 4. Server transitions to Listening
    server.set_voice_state(VoiceState::Listening, Some("voice activity detected".to_string())).await;

    // 5. STT Transcription arrives
    server.set_transcript("What is the current time?".to_string(), true, 0.98).await;

    // 6. Assistant LLM generates streaming answer -> Sentence Chunker chunks for TTS
    server.set_voice_state(VoiceState::Speaking, Some("TTS playback".to_string())).await;

    let mut chunker = SentenceChunker::new(20);
    let tokens = ["The ", "current ", "time ", "is ", "3:15 ", "PM. ", "Would you ", "like me to "];
    let mut sentences = Vec::new();
    for t in tokens {
        sentences.extend(chunker.feed(t));
    }
    assert_eq!(sentences.len(), 1);
    assert_eq!(sentences[0], "The current time is 3:15 PM.");

    // 7. User Interrupts (Barge-In)
    let mut barge_in = BargeInCoordinator::new(true, 100, 200);
    let playback_state = BargeInState {
        llm_streaming: true,
        pending_generates: 0,
        playback_active: true,
        playback_elapsed_ms: 250,
    };

    let should_interrupt = barge_in.should_barge_in(&playback_state, Instant::now());
    assert!(should_interrupt, "Barge-in coordinator must trigger on user interruption");

    // Immediate cancellation
    server.set_voice_state(VoiceState::Listening, Some("interrupted by user".to_string())).await;
    let _ = client.send_command(ClientCommand::EmergencyStop).await;

    // Verify client received the transition back to Listening
    let received_listening = timeout(Duration::from_secs(3), async {
        while let Ok(msg) = event_rx.recv().await {
            if let DaemonMessage::VoiceStateChanged { state, reason } = msg {
                if state == VoiceState::Listening && reason.as_deref() == Some("interrupted by user") {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);

    assert!(received_listening, "Client must receive state change back to Listening after interruption");

    server.stop();
}

// =========================================================================
// Scenario 3: Release Packaging & Deployment Integrity
// =========================================================================

#[test]
fn test_tier4_scenario3_packaging_and_deployment_integrity() {
    // 1. Verify NSIS installer script definition
    let root = workspace_root();
    let nsi_path = root.join("installer/nsis/osmoo.nsi");
    assert!(nsi_path.exists(), "installer/nsis/osmoo.nsi script must exist at {:?}", nsi_path);

    let script = std::fs::read_to_string(&nsi_path).expect("Read osmoo.nsi failed");

    // Verify key attributes in installer script
    assert!(script.contains("PRODUCT_NAME \"OSMOO\""));
    assert!(script.contains("PRODUCT_PUBLISHER \"Osmiora\""));
    assert!(script.contains("OSMOO.exe"));
    assert!(script.contains("voxy-daemon.exe"));
    assert!(script.contains("voxy-overlay.exe"));
    assert!(script.contains("uninstall.exe"));

    // Verify clean uninstallation preserves user data
    assert!(script.contains("Preserving User Data"));

    // 2. Check binary release artifact if compiled
    let installer_bin = root.join("installer/OSMOO/OSMOO-Setup-x64.exe");
    if installer_bin.exists() {
        let hash = compute_sha256(&installer_bin).expect("Compute hash failed");
        assert_eq!(hash.len(), 64);
        let meta = std::fs::metadata(&installer_bin).expect("Metadata failed");
        assert!(meta.len() > 100_000, "Installer must not be an empty or stub file");
    }
}
