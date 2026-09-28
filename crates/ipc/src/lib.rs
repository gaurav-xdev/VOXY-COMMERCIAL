//! VOXY IPC layer: wire protocol, transport abstraction, streaming,
//! events, auth, cancellation, version negotiation, and event replay.
//!
//! ## Architecture
//!
//! This crate defines the protocol and abstractions that all VOXY
//! inter-process communication is built on. It provides:
//!
//! - **Frame-based wire protocol** with message type discrimination
//! - **Transport abstraction** for named pipes, TCP, WebSocket, QUIC
//! - **Request/Response** method calls with timeout and idempotency
//! - **Streaming** for audio, video, events, and binary data
//! - **Event integration** with publish/subscribe semantics
//! - **Capability-based authentication** with token lifecycle
//! - **Version negotiation** for protocol evolution
//! - **Cancellation tokens** for cooperative cancellation
//! - **Event replay** with Live/Replay/Snapshot/History/Recovery modes

pub mod auth;
pub mod auth_middleware;
pub mod cancellation;
pub mod error;
pub mod protocol;
pub mod replay;
pub mod transport;
pub mod voxy_protocol;
pub mod named_pipe;

pub use auth::{AuthMethod, AuthRequest, AuthResponse, CapabilityClaim, CapabilityToken};
pub use auth_middleware::{AuthError, AuthMiddleware};
pub use cancellation::{CancellationHandle, CancellationToken};
pub use error::{IpcError, Result};
pub use protocol::{
    CancelRequest, EventMessage, Frame, FrameFlags, Heartbeat, MessageType, MethodCall,
    MethodError, MethodResponse, StreamClose, StreamData, StreamOpen, StreamOpenAck, StreamType,
    Version, VersionNegotiation, IPC_PROTOCOL_VERSION, MAX_FRAME_SIZE,
};
pub use replay::{
    DeliveryGuarantee, EventMetadata, EventStore, RecoveryCheckpoint, ReplayMode, ReplayState,
    ReplayStatus, ReplaySubscription, StoredEvent,
};
pub use transport::{
    JsonCodec, Transport, TransportAddr, TransportConnection, TransportListener, WireCodec,
};
pub use voxy_protocol::{
    encode_ipc_frame, decode_ipc_payload, ClientCommand, DaemonMessage, IpcEnvelope,
    ToolStepStatus, VoiceState, MAX_IPC_FRAME_SIZE, VOXY_IPC_VERSION, VOXY_PIPE_NAME,
};
pub use named_pipe::{DaemonStateSnapshot, VoxyIpcClient, VoxyIpcServer};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn protocol_version_constant() {
        assert_eq!(IPC_PROTOCOL_VERSION, "1.0.0");
        assert_eq!(MAX_FRAME_SIZE, 16 * 1024 * 1024);
    }

    #[test]
    fn roundtrip_frame_via_json() {
        let codec = JsonCodec::new();
        let frame = Frame::new(MessageType::Heartbeat)
            .with_payload(vec![1, 2, 3])
            .with_stream_id(42);
        let bytes = codec.encode(&frame).unwrap();
        let decoded = codec.decode(&bytes).unwrap();
        assert_eq!(decoded.message_type, MessageType::Heartbeat);
        assert_eq!(decoded.payload, vec![1, 2, 3]);
        assert_eq!(decoded.stream_id, Some(42));
    }

    #[test]
    fn cancellation_lifecycle() {
        let token = CancellationToken::new();
        let handle = CancellationHandle::new(token.clone());
        assert!(!token.is_cancelled());
        assert!(!handle.token().is_cancelled());
        handle.cancel();
        assert!(token.is_cancelled());
        assert!(handle.token().is_cancelled());
    }

    #[test]
    fn capability_gating() {
        let claims = vec![CapabilityClaim {
            capability: "admin:*".to_string(),
            resource: None,
            constraints: vec![],
        }];
        let token = CapabilityToken::new("admin-user", "system", claims, 3600, vec![0xAB]);
        assert!(token.has_capability("admin:*"));
        assert!(!token.has_capability("voice:*"));
        assert!(!token.is_expired());
        assert!(token.is_valid());
    }

    #[test]
    fn version_negotiation() {
        let local = Version::new(1, 2, 0);
        let remote = Version::new(1, 3, 0);
        assert!(local.compatible_with(&remote));
        assert!(remote.compatible_with(&local));
    }

    #[test]
    fn replay_modes_comprehensive() {
        let modes = vec![
            ReplayMode::Live,
            ReplayMode::Replay,
            ReplayMode::Snapshot,
            ReplayMode::History,
            ReplayMode::Recovery,
        ];
        assert_eq!(modes.len(), 5);
        for mode in &modes {
            match mode {
                ReplayMode::Live
                | ReplayMode::Replay
                | ReplayMode::Snapshot
                | ReplayMode::History
                | ReplayMode::Recovery => {}
            }
        }
    }

    #[test]
    fn auth_request_roundtrip() {
        let req = AuthRequest {
            method: AuthMethod::Token,
            credentials: serde_json::json!({"token": "test-token"}),
            requested_capabilities: vec!["storage:read".to_string()],
            client_version: "1.0.0".to_string(),
            client_name: "voxy-test".to_string(),
        };
        let response = AuthResponse {
            success: true,
            token: Some(CapabilityToken::new(
                "test-user",
                "system",
                vec![],
                3600,
                vec![0xAB],
            )),
            session_id: Some(Uuid::new_v4()),
            error: None,
            granted_capabilities: vec!["storage:read".to_string()],
        };
        let req_json = serde_json::to_string(&req).unwrap();
        let res_json = serde_json::to_string(&response).unwrap();
        let _req_restored: AuthRequest = serde_json::from_str(&req_json).unwrap();
        let _res_restored: AuthResponse = serde_json::from_str(&res_json).unwrap();
    }

    #[test]
    fn transport_enum_variants() {
        let addrs = vec![
            TransportAddr::NamedPipe("voxy".into()),
            TransportAddr::UnixSocket("/tmp/voxy.sock".into()),
            TransportAddr::Tcp {
                host: "localhost".into(),
                port: 9000,
            },
            TransportAddr::WebSocket {
                url: "ws://localhost:9000/ws".into(),
            },
            TransportAddr::Quic {
                host: "localhost".into(),
                port: 9001,
            },
            TransportAddr::InMemory("test".into()),
        ];
        assert_eq!(addrs.len(), 6);
        for addr in &addrs {
            let _s = addr.to_string();
        }
    }
}
