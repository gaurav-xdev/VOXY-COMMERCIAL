use std::time::Duration;
use voxy_ipc::{
    ClientCommand, DaemonMessage, DaemonStateSnapshot, ToolStepStatus, VoiceState, VoxyIpcClient,
    VoxyIpcServer,
};

#[test]
fn test_voice_states_serde() {
    let states = [
        VoiceState::Idle,
        VoiceState::Listening,
        VoiceState::Thinking,
        VoiceState::Speaking,
        VoiceState::Interrupted,
        VoiceState::Error,
    ];

    for state in states {
        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: VoiceState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(state, deserialized);
    }
}

#[test]
fn test_tool_step_status_serde() {
    let statuses = [
        ToolStepStatus::Running,
        ToolStepStatus::Completed,
        ToolStepStatus::Failed,
    ];

    for s in statuses {
        let serialized = serde_json::to_string(&s).unwrap();
        let deserialized: ToolStepStatus = serde_json::from_str(&serialized).unwrap();
        assert_eq!(s, deserialized);
    }
}

#[test]
fn test_state_snapshot_generation() {
    let snap = DaemonStateSnapshot {
        voice_state: VoiceState::Speaking,
        transcript: "Test speech transcript".to_string(),
        emergency_stopped: false,
        ..Default::default()
    };

    let msg = snap.to_snapshot_message();
    match msg {
        DaemonMessage::StateSnapshot {
            voice_state,
            transcript,
            emergency_stopped,
            ..
        } => {
            assert_eq!(voice_state, VoiceState::Speaking);
            assert_eq!(transcript, "Test speech transcript");
            assert!(!emergency_stopped);
        }
        _ => panic!("Expected StateSnapshot variant"),
    }
}

#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_e2e_communication() {
    // Unique pipe name for isolated test
    let pipe_name = format!(r"\\.\pipe\voxy-test-pipe-{}", uuid::Uuid::new_v4());

    let server = VoxyIpcServer::with_pipe_name(&pipe_name);
    server.start();
    assert!(server.is_running());

    // Give server small moment to bind pipe
    tokio::time::sleep(Duration::from_millis(50)).await;

    let client = VoxyIpcClient::with_pipe_name(&pipe_name);
    let mut event_rx = client.subscribe();
    let mut conn_watch = client.watch_connected();

    client.start();

    // 1. Wait for client to connect
    let mut connected = false;
    for _ in 0..30 {
        if *conn_watch.borrow() {
            connected = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = conn_watch.changed().await;
    }
    assert!(connected, "Client failed to connect to named pipe server");

    // 2. Client should receive initial StateSnapshot immediately
    let first_msg = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("Timeout waiting for snapshot")
        .expect("Channel closed");

    match first_msg {
        DaemonMessage::StateSnapshot { voice_state, .. } => {
            assert_eq!(voice_state, VoiceState::Idle);
        }
        other => panic!("Expected StateSnapshot as first message, got: {:?}", other),
    }

    // 3. Test state transition broadcast from daemon
    server
        .set_voice_state(VoiceState::Listening, Some("PTT active".to_string()))
        .await;

    let state_msg = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("Timeout waiting for state transition")
        .expect("Channel closed");

    match state_msg {
        DaemonMessage::VoiceStateChanged { state, reason } => {
            assert_eq!(state, VoiceState::Listening);
            assert_eq!(reason, Some("PTT active".to_string()));
        }
        other => panic!("Expected VoiceStateChanged, got: {:?}", other),
    }

    // 4. Test tool step broadcast from daemon
    server.broadcast_tool_step(
        101,
        "open_app".to_string(),
        "Launching Calculator".to_string(),
        "calc.exe".to_string(),
        ToolStepStatus::Running,
    );

    let tool_msg = tokio::time::timeout(Duration::from_secs(2), event_rx.recv())
        .await
        .expect("Timeout waiting for tool step")
        .expect("Channel closed");

    match tool_msg {
        DaemonMessage::ToolStep {
            id, tool, status, ..
        } => {
            assert_eq!(id, 101);
            assert_eq!(tool, "open_app");
            assert_eq!(status, ToolStepStatus::Running);
        }
        other => panic!("Expected ToolStep, got: {:?}", other),
    }

    // 5. First command received by daemon should be client handshake
    let handshake_cmd = tokio::time::timeout(Duration::from_secs(2), server.recv_command())
        .await
        .expect("Timeout waiting for handshake")
        .expect("Server command channel closed");

    match handshake_cmd {
        ClientCommand::ClientHandshake { client_name, .. } => {
            assert_eq!(client_name, "voxy-overlay");
        }
        other => panic!("Expected ClientHandshake, got: {:?}", other),
    }

    // 6. Test client sending EmergencyStop command to daemon
    client
        .send_command(ClientCommand::EmergencyStop)
        .await
        .expect("Failed to send command");

    let received_cmd = tokio::time::timeout(Duration::from_secs(2), server.recv_command())
        .await
        .expect("Timeout waiting for client command")
        .expect("Server command channel closed");

    assert_eq!(received_cmd, ClientCommand::EmergencyStop);

    // 7. Clean stop
    client.stop();
    server.stop();
}

#[cfg(windows)]
#[tokio::test]
async fn test_windows_named_pipe_auth_enforcement_and_privileged_command_rejection() {
    let pipe_name = format!(r"\\.\pipe\voxy-test-auth-{}", uuid::Uuid::new_v4());
    let valid_secret = "commercial-production-shared-secret-12345";

    // 1. Start server requiring the valid secret
    let server = VoxyIpcServer::with_pipe_name_and_auth(&pipe_name, Some(valid_secret.to_string()));
    server.start();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // 2. Client with INVALID token connects
    let bad_client = VoxyIpcClient::with_pipe_name_and_auth(
        &pipe_name,
        Some("attacker-invalid-token".to_string()),
    );
    bad_client.start();

    let mut bad_conn_watch = bad_client.watch_connected();
    for _ in 0..20 {
        if *bad_conn_watch.borrow() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = bad_conn_watch.changed().await;
    }

    // Drain handshake attempt from bad client
    let _ = tokio::time::timeout(Duration::from_secs(1), server.recv_command()).await;

    // 3. Attacker sends privileged EmergencyStop command
    bad_client
        .send_command(ClientCommand::EmergencyStop)
        .await
        .unwrap();

    // The privileged command MUST NOT be processed by the server
    let bad_res = tokio::time::timeout(Duration::from_millis(400), server.recv_command()).await;
    assert!(
        bad_res.is_err(),
        "Expected timeout: unauthenticated client's privileged command must be dropped by the server"
    );
    bad_client.stop();

    // Give server moment to cycle pipe
    tokio::time::sleep(Duration::from_millis(100)).await;

    // 4. Authenticated client connects with CORRECT secret
    let good_client =
        VoxyIpcClient::with_pipe_name_and_auth(&pipe_name, Some(valid_secret.to_string()));
    good_client.start();

    let mut good_conn_watch = good_client.watch_connected();
    for _ in 0..20 {
        if *good_conn_watch.borrow() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _ = good_conn_watch.changed().await;
    }

    // Handshake command received
    let good_handshake = tokio::time::timeout(Duration::from_secs(2), server.recv_command())
        .await
        .expect("Timeout on good handshake")
        .expect("Server closed");
    match good_handshake {
        ClientCommand::ClientHandshake { auth_token, .. } => {
            assert!(auth_token.is_some(), "Expected cryptographic auth token");
        }
        other => panic!("Expected ClientHandshake, got: {:?}", other),
    }

    // 5. Authenticated client sends privileged EmergencyStop
    good_client
        .send_command(ClientCommand::EmergencyStop)
        .await
        .unwrap();

    let good_cmd = tokio::time::timeout(Duration::from_secs(2), server.recv_command())
        .await
        .expect("Timeout on good privileged command")
        .expect("Server closed");
    assert_eq!(good_cmd, ClientCommand::EmergencyStop);

    good_client.stop();
    server.stop();
}
