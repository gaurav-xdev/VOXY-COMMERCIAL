//! Strongly typed, versioned IPC protocol for VOXY COM.
//!
//! Provides deterministic state streaming, tool execution reporting,
//! audio telemetry, and command dispatch between VOXY Daemon (server)
//! and VOXY Overlay / UI (client).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Current IPC protocol version.
pub const VOXY_IPC_VERSION: u32 = 1;

/// Maximum allowed frame size in bytes (1 MB safety cap against memory exhaustion).
pub const MAX_IPC_FRAME_SIZE: usize = 1024 * 1024;

/// Default Windows Named Pipe path for VOXY COM.
pub const VOXY_PIPE_NAME: &str = r"\\.\pipe\voxy-com-ipc";

/// Fallback TCP port for non-pipe environments or cross-platform tests.
pub const VOXY_IPC_TEST_PORT: u16 = 18889;

// ==============================================================================
// Voice Pipeline States
// ==============================================================================

/// High-level runtime voice states published by the authoritative daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VoiceState {
    /// Idle, standby mode. Audio capture is passive or awaiting wake / PTT.
    #[default]
    Idle,
    /// Active microphone listening / user speaking.
    Listening,
    /// STT complete, LLM generating response / reasoning.
    Thinking,
    /// TTS audio synthesizing and actively playing through speakers.
    Speaking,
    /// Acoustic barge-in or user cutoff detected.
    Interrupted,
    /// Error condition (network dropout, provider rate-limit, mic failure).
    Error,
}

impl fmt::Display for VoiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Listening => write!(f, "Listening"),
            Self::Thinking => write!(f, "Thinking"),
            Self::Speaking => write!(f, "Speaking"),
            Self::Interrupted => write!(f, "Interrupted"),
            Self::Error => write!(f, "Error"),
        }
    }
}

// ==============================================================================
// Tool Execution Types
// ==============================================================================

/// Lifecycle status for tool / computer control steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolStepStatus {
    Running,
    Completed,
    Failed,
}

impl fmt::Display for ToolStepStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "Running"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

// ==============================================================================
// Daemon -> Overlay Messages (Events)
// ==============================================================================

/// Authoritative events streamed from VOXY Daemon to VOXY Overlay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DaemonMessage {
    /// State snapshot provided immediately upon client connection.
    StateSnapshot {
        voice_state: VoiceState,
        transcript: String,
        desktop_mode: String,
        profile_name: String,
        daemon_version: String,
        emergency_stopped: bool,
    },

    /// Real-time voice state change.
    VoiceStateChanged {
        state: VoiceState,
        reason: Option<String>,
    },

    /// Real-time transcript update (partial streaming or final committed).
    TranscriptUpdate {
        text: String,
        is_final: bool,
        confidence: f32,
    },

    /// Audio energy levels for local UI visualizer / orb animation.
    AudioEnergy { mic_rms: f32, output_rms: f32 },

    /// Acoustic double-talk / user interruption detected.
    Interrupted { latency_ms: u32, reason: String },

    /// Computer control / tool execution step progress.
    ToolStep {
        id: u64,
        tool: String,
        title: String,
        detail: String,
        status: ToolStepStatus,
    },

    /// Conversation turn added (for chat drawer display).
    ChatMessage {
        id: u64,
        sender: String,
        text: String,
        timestamp: String,
        is_user: bool,
    },

    /// System or provider error notification.
    ErrorNotification { code: String, message: String },

    /// Emergency stop state changed.
    EmergencyStopChanged { is_stopped: bool },

    /// Routing mode and active provider status update.
    RoutingStatusUpdate {
        mode: String,
        active_llm: String,
        active_stt: String,
        active_tts: String,
        is_offline: bool,
    },

    /// Host hardware summary update.
    HardwareStatusUpdate {
        cpu_brand: String,
        cpu_cores: usize,
        ram_gb: f64,
        gpu_name: Option<String>,
        vram_gb: f64,
    },

    /// Real-time VOXY visual cursor and computer control telemetry.
    CursorUpdate(VoxyCursorTelemetry),

    /// Heartbeat ping from daemon (every 5-10s).
    Heartbeat { uptime_secs: u64 },
}

/// Visual interaction state of the dedicated VOXY computer control cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum VoxyCursorState {
    #[default]
    Idle,
    Moving,
    Clicking,
    Typing,
    Dragging,
    ExecutingAction,
    WaitingConfirmation,
}

/// Real-time visual cursor telemetry broadcast over IPC to ensure all automated control is visible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoxyCursorTelemetry {
    pub x: i32,
    pub y: i32,
    pub state: VoxyCursorState,
    pub target_element: String,
    pub action_description: String,
    pub confidence: f32,
    pub requires_confirmation: bool,
    pub action_id: Option<u64>,
}

// ==============================================================================
// Overlay -> Daemon Commands (Client Controls)
// ==============================================================================

/// Commands sent from VOXY Overlay / UI to VOXY Daemon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ClientCommand {
    /// Client handshake upon connection.
    ClientHandshake {
        client_name: String,
        client_version: String,
        #[serde(default)]
        auth_token: Option<String>,
        #[serde(default)]
        nonce: Option<String>,
        #[serde(default)]
        timestamp_ms: Option<u64>,
    },

    /// Request a fresh state snapshot from the daemon.
    RequestSnapshot,

    /// Trigger Emergency Stop: aborts all active computer control tools immediately.
    EmergencyStop,

    /// Reset Emergency Stop: re-enables computer control.
    ResetEmergencyStop,

    /// User approval or rejection for a pending high-risk / destructive action.
    ConfirmAction { action_id: u64, approved: bool },

    /// Interrupt ongoing speech synthesis immediately.
    InterruptSpeech,

    /// Send text input into conversational pipeline (dev/text mode alternative to voice).
    SendTextInput { text: String },

    /// Change the active AI routing mode ("Auto", "LocalOnly", "CloudOnly").
    SetRoutingMode { mode: String },

    /// Request an updated hardware status report.
    RequestHardwareStatus,

    /// Client heartbeat pong response.
    HeartbeatPong,
}

impl ClientCommand {
    /// Returns true if this command performs a privileged system action
    /// (e.g. aborting tools, changing AI routing, approving actions, or sending user input).
    pub fn is_privileged(&self) -> bool {
        matches!(
            self,
            ClientCommand::EmergencyStop
                | ClientCommand::ResetEmergencyStop
                | ClientCommand::ConfirmAction { .. }
                | ClientCommand::SendTextInput { .. }
                | ClientCommand::SetRoutingMode { .. }
        )
    }
}

// ==============================================================================
// Versioned Envelope
// ==============================================================================

/// Standard versioned envelope wrapping all IPC messages over the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IpcEnvelope<T> {
    pub version: u32,
    pub timestamp_ms: u64,
    pub payload: T,
}

impl<T> IpcEnvelope<T> {
    pub fn new(payload: T) -> Self {
        let timestamp_ms = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
            Ok(d) => d.as_millis() as u64,
            Err(_) => 0,
        };
        Self {
            version: VOXY_IPC_VERSION,
            timestamp_ms,
            payload,
        }
    }
}

// ==============================================================================
// Length-Delimited Wire Framing
// ==============================================================================

/// Encode an IPC envelope with a 4-byte big-endian length prefix.
pub fn encode_ipc_frame<T: Serialize>(payload: &T) -> Result<Vec<u8>, String> {
    let json_bytes =
        serde_json::to_vec(payload).map_err(|e| format!("IPC serialization failed: {e}"))?;

    if json_bytes.len() > MAX_IPC_FRAME_SIZE {
        return Err(format!(
            "IPC payload exceeds maximum size limit ({} > {})",
            json_bytes.len(),
            MAX_IPC_FRAME_SIZE
        ));
    }

    let len = json_bytes.len() as u32;
    let mut frame = Vec::with_capacity(4 + json_bytes.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&json_bytes);
    Ok(frame)
}

/// Decode an IPC envelope from payload bytes.
pub fn decode_ipc_payload<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, String> {
    if bytes.len() > MAX_IPC_FRAME_SIZE {
        return Err(format!(
            "IPC frame exceeds maximum size limit ({} > {})",
            bytes.len(),
            MAX_IPC_FRAME_SIZE
        ));
    }
    serde_json::from_slice(bytes).map_err(|e| format!("IPC deserialization failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_message_roundtrip() {
        let msg = DaemonMessage::VoiceStateChanged {
            state: VoiceState::Listening,
            reason: Some("Mic speech detected".to_string()),
        };
        let env = IpcEnvelope::new(msg.clone());
        let frame = encode_ipc_frame(&env).unwrap();

        assert!(frame.len() > 4);
        let len = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        assert_eq!(len, frame.len() - 4);

        let decoded: IpcEnvelope<DaemonMessage> = decode_ipc_payload(&frame[4..]).unwrap();
        assert_eq!(decoded.version, VOXY_IPC_VERSION);
        assert_eq!(decoded.payload, msg);
    }

    #[test]
    fn test_client_command_roundtrip() {
        let cmd = ClientCommand::SetRoutingMode {
            mode: "LocalOnly".into(),
        };
        let env = IpcEnvelope::new(cmd.clone());
        let frame = encode_ipc_frame(&env).unwrap();

        let decoded: IpcEnvelope<ClientCommand> = decode_ipc_payload(&frame[4..]).unwrap();
        assert_eq!(decoded.version, VOXY_IPC_VERSION);
        assert_eq!(decoded.payload, cmd);
    }

    #[test]
    fn test_routing_status_update_roundtrip() {
        let msg = DaemonMessage::RoutingStatusUpdate {
            mode: "Auto".into(),
            active_llm: "Ollama (llama-3.2-3b)".into(),
            active_stt: "Local SAPI".into(),
            active_tts: "Local SAPI".into(),
            is_offline: true,
        };
        let env = IpcEnvelope::new(msg.clone());
        let frame = encode_ipc_frame(&env).unwrap();

        let decoded: IpcEnvelope<DaemonMessage> = decode_ipc_payload(&frame[4..]).unwrap();
        assert_eq!(decoded.version, VOXY_IPC_VERSION);
        assert_eq!(decoded.payload, msg);
    }

    #[test]
    fn test_hardware_status_update_roundtrip() {
        let msg = DaemonMessage::HardwareStatusUpdate {
            cpu_brand: "AMD Ryzen 9".into(),
            cpu_cores: 16,
            ram_gb: 32.0,
            gpu_name: Some("NVIDIA GeForce RTX 4090".into()),
            vram_gb: 24.0,
        };
        let env = IpcEnvelope::new(msg.clone());
        let frame = encode_ipc_frame(&env).unwrap();

        let decoded: IpcEnvelope<DaemonMessage> = decode_ipc_payload(&frame[4..]).unwrap();
        assert_eq!(decoded.version, VOXY_IPC_VERSION);
        assert_eq!(decoded.payload, msg);
    }

    #[test]
    fn test_max_frame_size_enforced() {
        let oversized = vec![0u8; MAX_IPC_FRAME_SIZE + 10];
        let err = decode_ipc_payload::<DaemonMessage>(&oversized).unwrap_err();
        assert!(err.contains("exceeds maximum size"));
    }

    #[test]
    fn test_voice_state_display() {
        assert_eq!(VoiceState::Listening.to_string(), "Listening");
        assert_eq!(VoiceState::Thinking.to_string(), "Thinking");
        assert_eq!(VoiceState::Speaking.to_string(), "Speaking");
        assert_eq!(VoiceState::Interrupted.to_string(), "Interrupted");
    }
}
