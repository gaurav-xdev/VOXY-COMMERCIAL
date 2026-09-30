//! Windows Named Pipe IPC transport for VOXY COM.
//!
//! Provides high-performance, framed, zero-network, local-only IPC between
//! `voxy-daemon` (authoritative server) and `voxy-overlay` (client).
//!
//! Features:
//! - Length-delimited message framing (4-byte BE length prefix, max 1MB)
//! - Multi-client connection handling with broadcast state streaming
//! - Immediate state snapshot sent upon client connection
//! - Independent client reader/writer tasks preventing deadlocks
//! - Automatic reconnection with exponential backoff
//! - Safe recovery on client disconnect or server restart

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, watch, RwLock};
use tracing::{debug, error, info, warn};

use crate::voxy_protocol::{
    decode_ipc_payload, encode_ipc_frame, ClientCommand, DaemonMessage, IpcEnvelope,
    VoiceState, MAX_IPC_FRAME_SIZE, VOXY_PIPE_NAME,
};

// ==============================================================================
// State Snapshot for Fresh Connections
// ==============================================================================

/// Current snapshot of daemon state, cached to immediately sync new clients.
#[derive(Debug, Clone)]
pub struct DaemonStateSnapshot {
    pub voice_state: VoiceState,
    pub transcript: String,
    pub desktop_mode: String,
    pub profile_name: String,
    pub daemon_version: String,
    pub emergency_stopped: bool,
}

impl Default for DaemonStateSnapshot {
    fn default() -> Self {
        Self {
            voice_state: VoiceState::Idle,
            transcript: String::new(),
            desktop_mode: "FullExperience".to_string(),
            profile_name: "VOXY // COM".to_string(),
            daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            emergency_stopped: false,
        }
    }
}

impl DaemonStateSnapshot {
    pub fn to_snapshot_message(&self) -> DaemonMessage {
        DaemonMessage::StateSnapshot {
            voice_state: self.voice_state,
            transcript: self.transcript.clone(),
            desktop_mode: self.desktop_mode.clone(),
            profile_name: self.profile_name.clone(),
            daemon_version: self.daemon_version.clone(),
            emergency_stopped: self.emergency_stopped,
        }
    }
}

// ==============================================================================
// Framed I/O Helpers
// ==============================================================================

/// Read a single length-delimited frame from an async reader.
pub async fn read_framed_message<R, T>(reader: &mut R) -> Result<IpcEnvelope<T>, String>
where
    R: AsyncRead + Unpin,
    T: for<'de> serde::Deserialize<'de>,
{
    let mut len_bytes = [0u8; 4];
    reader
        .read_exact(&mut len_bytes)
        .await
        .map_err(|e| format!("Failed to read frame length: {e}"))?;

    let frame_len = u32::from_be_bytes(len_bytes) as usize;
    if frame_len > MAX_IPC_FRAME_SIZE {
        return Err(format!(
            "Frame length exceeds limit: {frame_len} > {MAX_IPC_FRAME_SIZE}"
        ));
    }

    let mut buf = vec![0u8; frame_len];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(|e| format!("Failed to read frame payload: {e}"))?;

    decode_ipc_payload(&buf)
}

/// Write a single length-delimited frame to an async writer.
pub async fn write_framed_message<W, T>(writer: &mut W, message: &IpcEnvelope<T>) -> Result<(), String>
where
    W: AsyncWrite + Unpin,
    T: serde::Serialize,
{
    let frame_bytes = encode_ipc_frame(message)?;
    writer
        .write_all(&frame_bytes)
        .await
        .map_err(|e| format!("Failed to write frame: {e}"))?;
    writer
        .flush()
        .await
        .map_err(|e| format!("Failed to flush frame: {e}"))?;
    Ok(())
}

// ==============================================================================
// Voxy Named Pipe Server (Daemon Side)
// ==============================================================================

/// High-level IPC server hosted inside VOXY Daemon.
pub struct VoxyIpcServer {
    pipe_name: String,
    snapshot: Arc<RwLock<DaemonStateSnapshot>>,
    broadcast_tx: broadcast::Sender<IpcEnvelope<DaemonMessage>>,
    command_tx: mpsc::Sender<ClientCommand>,
    command_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<ClientCommand>>>,
    is_running: Arc<AtomicBool>,
}

impl VoxyIpcServer {
    /// Create a new IPC server listening on the standard named pipe.
    pub fn new() -> Self {
        Self::with_pipe_name(VOXY_PIPE_NAME)
    }

    /// Create with a custom pipe name (useful for isolated tests).
    pub fn with_pipe_name(pipe_name: impl Into<String>) -> Self {
        let (broadcast_tx, _) = broadcast::channel(256);
        let (command_tx, command_rx) = mpsc::channel(64);

        Self {
            pipe_name: pipe_name.into(),
            snapshot: Arc::new(RwLock::new(DaemonStateSnapshot::default())),
            broadcast_tx,
            command_tx,
            command_rx: Arc::new(tokio::sync::Mutex::new(command_rx)),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Update the current voice state, updating the snapshot and broadcasting to all clients.
    pub async fn set_voice_state(&self, state: VoiceState, reason: Option<String>) {
        {
            let mut snap = self.snapshot.write().await;
            snap.voice_state = state;
        }
        self.broadcast(DaemonMessage::VoiceStateChanged { state, reason });
    }

    /// Update the current transcript, updating the snapshot and broadcasting to all clients.
    pub async fn set_transcript(&self, text: String, is_final: bool, confidence: f32) {
        if is_final {
            let mut snap = self.snapshot.write().await;
            snap.transcript = text.clone();
        }
        self.broadcast(DaemonMessage::TranscriptUpdate {
            text,
            is_final,
            confidence,
        });
    }

    /// Broadcast a tool execution step event to all connected clients.
    pub fn broadcast_tool_step(&self, id: u64, tool: String, title: String, detail: String, status: crate::voxy_protocol::ToolStepStatus) {
        self.broadcast(DaemonMessage::ToolStep {
            id,
            tool,
            title,
            detail,
            status,
        });
    }

    /// Update emergency stop state in snapshot and broadcast.
    pub async fn set_emergency_stop(&self, is_stopped: bool) {
        {
            let mut snap = self.snapshot.write().await;
            snap.emergency_stopped = is_stopped;
        }
        self.broadcast(DaemonMessage::EmergencyStopChanged { is_stopped });
    }

    /// Broadcast a routing status update.
    pub fn broadcast_routing_status(
        &self,
        mode: String,
        active_llm: String,
        active_stt: String,
        active_tts: String,
        is_offline: bool,
    ) {
        self.broadcast(DaemonMessage::RoutingStatusUpdate {
            mode,
            active_llm,
            active_stt,
            active_tts,
            is_offline,
        });
    }

    /// Broadcast a hardware status update.
    pub fn broadcast_hardware_status(
        &self,
        cpu_brand: String,
        cpu_cores: usize,
        ram_gb: f64,
        gpu_name: Option<String>,
        vram_gb: f64,
    ) {
        self.broadcast(DaemonMessage::HardwareStatusUpdate {
            cpu_brand,
            cpu_cores,
            ram_gb,
            gpu_name,
            vram_gb,
        });
    }

    /// Broadcast an arbitrary DaemonMessage to all connected clients.
    pub fn broadcast(&self, message: DaemonMessage) {
        let envelope = IpcEnvelope::new(message);
        let _ = self.broadcast_tx.send(envelope);
    }

    /// Receive the next command sent by a client (e.g. EmergencyStop, text input).
    pub async fn recv_command(&self) -> Option<ClientCommand> {
        let mut rx = self.command_rx.lock().await;
        rx.recv().await
    }

    /// Check if the server accept loop is running.
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Start the background server accept loop.
    pub fn start(&self) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            warn!("VoxyIpcServer already started");
            return;
        }

        let pipe_name = self.pipe_name.clone();
        let snapshot = Arc::clone(&self.snapshot);
        let broadcast_tx = self.broadcast_tx.clone();
        let command_tx = self.command_tx.clone();
        let is_running = Arc::clone(&self.is_running);

        tokio::spawn(async move {
            info!(pipe = %pipe_name, "VOXY COM Named Pipe Server listening");

            #[cfg(windows)]
            {
                use tokio::net::windows::named_pipe::ServerOptions;

                let mut is_first = true;
                while is_running.load(Ordering::SeqCst) {
                    let server_res = if is_first {
                        is_first = false;
                        ServerOptions::new()
                            .first_pipe_instance(true)
                            .max_instances(16)
                            .reject_remote_clients(true)
                            .create(&pipe_name)
                    } else {
                        ServerOptions::new()
                            .max_instances(16)
                            .reject_remote_clients(true)
                            .create(&pipe_name)
                    };

                    let server = match server_res {
                        Ok(s) => s,
                        Err(e) => {
                            error!(error = %e, "Failed to create Named Pipe instance; retrying in 1s");
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            continue;
                        }
                    };

                    // Wait for a client to connect to this pipe instance
                    if let Err(e) = server.connect().await {
                        debug!(error = %e, "Named Pipe connect aborted");
                        continue;
                    }

                    info!("New client connected to VOXY Named Pipe");
                    let snap = snapshot.read().await.clone();
                    let client_rx = broadcast_tx.subscribe();
                    let cmd_tx = command_tx.clone();

                    tokio::spawn(async move {
                        handle_client_connection(server, snap, client_rx, cmd_tx).await;
                    });
                }
            }

            #[cfg(not(windows))]
            {
                // Cross-platform mock for Unix testing
                warn!("Named Pipes only natively supported on Windows; stub running on this platform");
            }
        });
    }

    /// Stop the server.
    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        info!("VOXY COM Named Pipe Server stopped");
    }
}

impl Default for VoxyIpcServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
async fn handle_client_connection(
    pipe: tokio::net::windows::named_pipe::NamedPipeServer,
    initial_snapshot: DaemonStateSnapshot,
    mut broadcast_rx: broadcast::Receiver<IpcEnvelope<DaemonMessage>>,
    command_tx: mpsc::Sender<ClientCommand>,
) {
    let (mut reader, mut writer) = tokio::io::split(pipe);

    // 1. Send state snapshot immediately upon connection
    let snapshot_msg = IpcEnvelope::new(initial_snapshot.to_snapshot_message());
    if let Err(e) = write_framed_message(&mut writer, &snapshot_msg).await {
        warn!(error = %e, "Failed to send initial snapshot to client");
        return;
    }

    // 2. Writer task: streams broadcast events from daemon to client
    let writer_task = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(msg) => {
                    if let Err(e) = write_framed_message(&mut writer, &msg).await {
                        debug!(error = %e, "Client pipe write failed, closing writer loop");
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(missed = n, "Overlay client lagged behind broadcast stream; skipped {n} messages");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Broadcast channel closed, terminating writer task");
                    break;
                }
            }
        }
    });

    // 3. Reader task: reads incoming client commands
    let reader_task = tokio::spawn(async move {
        let mut handshaken = false;
        loop {
            match read_framed_message::<_, ClientCommand>(&mut reader).await {
                Ok(envelope) => {
                    match &envelope.payload {
                        ClientCommand::ClientHandshake { client_name, client_version } => {
                            info!(client = %client_name, version = %client_version, "Client handshake accepted");
                            handshaken = true;
                        }
                        _ => {
                            if !handshaken {
                                warn!("Unauthenticated command received from client before handshake; aborting connection");
                                break;
                            }
                        }
                    }
                    debug!(cmd = ?envelope.payload, "Received client command");
                    let _ = command_tx.send(envelope.payload).await;
                }
                Err(e) => {
                    debug!(error = %e, "Client pipe read failed or disconnected");
                    break;
                }
            }
        }
    });

    // If either reader or writer finishes (e.g. disconnect), abort the other
    tokio::select! {
        _ = writer_task => {},
        _ = reader_task => {},
    }
    info!("Client disconnected from VOXY Named Pipe");
}

// ==============================================================================
// Voxy Named Pipe Client (Overlay Side)
// ==============================================================================

/// High-level IPC client used inside VOXY Overlay / UI.
pub struct VoxyIpcClient {
    pipe_name: String,
    is_connected: Arc<watch::Sender<bool>>,
    event_tx: broadcast::Sender<DaemonMessage>,
    out_cmd_tx: mpsc::Sender<ClientCommand>,
    out_cmd_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<ClientCommand>>>,
    running: Arc<AtomicBool>,
}

impl VoxyIpcClient {
    /// Create a new IPC client targeting the standard named pipe.
    pub fn new() -> Self {
        Self::with_pipe_name(VOXY_PIPE_NAME)
    }

    /// Create with custom pipe name.
    pub fn with_pipe_name(pipe_name: impl Into<String>) -> Self {
        let (is_connected, _) = watch::channel(false);
        let (event_tx, _) = broadcast::channel(256);
        let (out_cmd_tx, out_cmd_rx) = mpsc::channel(64);

        Self {
            pipe_name: pipe_name.into(),
            is_connected: Arc::new(is_connected),
            event_tx,
            out_cmd_tx,
            out_cmd_rx: Arc::new(tokio::sync::Mutex::new(out_cmd_rx)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Subscribe to daemon events.
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonMessage> {
        self.event_tx.subscribe()
    }

    /// Watch connection status.
    pub fn watch_connected(&self) -> watch::Receiver<bool> {
        self.is_connected.subscribe()
    }

    /// Send a command to the daemon.
    pub async fn send_command(&self, cmd: ClientCommand) -> Result<(), String> {
        self.out_cmd_tx
            .send(cmd)
            .await
            .map_err(|e| format!("Command sender error: {e}"))
    }

    /// Start the background connection and auto-reconnect loop.
    pub fn start(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }

        let pipe_name = self.pipe_name.clone();
        let is_connected = Arc::clone(&self.is_connected);
        let event_tx = self.event_tx.clone();
        let out_cmd_rx = Arc::clone(&self.out_cmd_rx);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut backoff_ms = 200u64;

            while running.load(Ordering::SeqCst) {
                #[cfg(windows)]
                {
                    use tokio::net::windows::named_pipe::ClientOptions;

                    match ClientOptions::new().open(&pipe_name) {
                        Ok(client) => {
                            info!("Connected to VOXY Daemon Named Pipe");
                            let _ = is_connected.send(true);
                            backoff_ms = 200; // Reset backoff on success

                            let (mut reader, mut writer) = tokio::io::split(client);

                            // Send handshake
                            let handshake = IpcEnvelope::new(ClientCommand::ClientHandshake {
                                client_name: "voxy-overlay".to_string(),
                                client_version: env!("CARGO_PKG_VERSION").to_string(),
                            });
                            let _ = write_framed_message(&mut writer, &handshake).await;

                            let event_tx_clone = event_tx.clone();

                            // Read loop
                            let reader_handle = tokio::spawn(async move {
                                loop {
                                    match read_framed_message::<_, DaemonMessage>(&mut reader).await {
                                        Ok(env) => {
                                            let _ = event_tx_clone.send(env.payload);
                                        }
                                        Err(e) => {
                                            debug!(error = %e, "Overlay pipe read stream ended");
                                            break;
                                        }
                                    }
                                }
                            });

                            // Write loop
                            let out_cmd_rx_clone = Arc::clone(&out_cmd_rx);
                            let writer_handle = tokio::spawn(async move {
                                loop {
                                    let cmd_opt = {
                                        let mut rx = out_cmd_rx_clone.lock().await;
                                        rx.recv().await
                                    };
                                    match cmd_opt {
                                        Some(cmd) => {
                                            let env = IpcEnvelope::new(cmd);
                                            if let Err(e) = write_framed_message(&mut writer, &env).await {
                                                debug!(error = %e, "Overlay pipe write stream ended");
                                                break;
                                            }
                                        }
                                        None => break,
                                    }
                                }
                            });

                            tokio::select! {
                                _ = reader_handle => {},
                                _ = writer_handle => {},
                            }

                            warn!("Disconnected from VOXY Daemon; will reconnect");
                            let _ = is_connected.send(false);
                        }
                        Err(_e) => {
                            // Daemon is not running yet or pipe busy
                            let _ = is_connected.send(false);
                        }
                    }
                }

                #[cfg(not(windows))]
                {
                    // Non-windows sleep
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }

                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(3000);
            }
        });
    }

    /// Stop the client.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Default for VoxyIpcClient {
    fn default() -> Self {
        Self::new()
    }
}
