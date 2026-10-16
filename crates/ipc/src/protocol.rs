use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const IPC_PROTOCOL_VERSION: &str = "1.0.0";
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    Heartbeat,
    HeartbeatAck,
    MethodCall,
    MethodResponse,
    MethodError,
    StreamOpen,
    StreamOpenAck,
    StreamData,
    StreamDataAck,
    StreamClose,
    StreamCloseAck,
    EventPublish,
    EventNotify,
    EventSubscribe,
    EventUnsubscribe,
    AuthRequest,
    AuthResponse,
    CancelRequest,
    CancelAck,
    VersionNegotiation,
    VersionAccepted,
    VersionRejected,
}

impl MessageType {
    pub fn is_request(&self) -> bool {
        matches!(
            self,
            Self::Heartbeat
                | Self::MethodCall
                | Self::StreamOpen
                | Self::EventSubscribe
                | Self::AuthRequest
                | Self::VersionNegotiation
                | Self::CancelRequest
        )
    }

    pub fn is_response(&self) -> bool {
        matches!(
            self,
            Self::HeartbeatAck
                | Self::MethodResponse
                | Self::MethodError
                | Self::StreamOpenAck
                | Self::StreamCloseAck
                | Self::EventUnsubscribe
                | Self::AuthResponse
                | Self::CancelAck
                | Self::VersionAccepted
                | Self::VersionRejected
        )
    }

    pub fn is_stream(&self) -> bool {
        matches!(
            self,
            Self::StreamOpen | Self::StreamData | Self::StreamClose
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn compatible_with(&self, other: &Version) -> bool {
        self.major == other.major
    }

    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(3, '.').collect();
        if parts.len() != 3 {
            return None;
        }
        Some(Self {
            major: parts[0].parse().ok()?,
            minor: parts[1].parse().ok()?,
            patch: parts[2].parse().ok()?,
        })
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub message_type: MessageType,
    pub stream_id: Option<u64>,
    pub request_id: Option<Uuid>,
    pub correlation_id: Option<Uuid>,
    pub payload: Vec<u8>,
    pub flags: FrameFlags,
    pub timestamp_ms: i64,
}

impl Frame {
    pub fn new(message_type: MessageType) -> Self {
        Self {
            message_type,
            stream_id: None,
            request_id: None,
            correlation_id: None,
            payload: Vec::new(),
            flags: FrameFlags::default(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn with_payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_stream_id(mut self, stream_id: u64) -> Self {
        self.stream_id = Some(stream_id);
        self
    }

    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn with_flags(mut self, flags: FrameFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn payload_json<'de, T: serde::Deserialize<'de>>(
        &'de self,
    ) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.payload)
    }

    pub fn from_json<T: Serialize>(
        message_type: MessageType,
        value: &T,
    ) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_vec(value)?;
        Ok(Self::new(message_type).with_payload(payload))
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct FrameFlags {
    pub priority_high: bool,
    pub priority_critical: bool,
    pub compressed: bool,
    pub encrypted: bool,
    pub requires_ack: bool,
    pub is_cancelled: bool,
    pub is_replay: bool,
    pub is_recovery: bool,
}

impl FrameFlags {
    pub fn as_u16(&self) -> u16 {
        let mut bits = 0u16;
        if self.priority_high {
            bits |= 1 << 0;
        }
        if self.priority_critical {
            bits |= 1 << 1;
        }
        if self.compressed {
            bits |= 1 << 2;
        }
        if self.encrypted {
            bits |= 1 << 3;
        }
        if self.requires_ack {
            bits |= 1 << 4;
        }
        if self.is_cancelled {
            bits |= 1 << 5;
        }
        if self.is_replay {
            bits |= 1 << 6;
        }
        if self.is_recovery {
            bits |= 1 << 7;
        }
        bits
    }

    pub fn from_u16(bits: u16) -> Self {
        Self {
            priority_high: bits & (1 << 0) != 0,
            priority_critical: bits & (1 << 1) != 0,
            compressed: bits & (1 << 2) != 0,
            encrypted: bits & (1 << 3) != 0,
            requires_ack: bits & (1 << 4) != 0,
            is_cancelled: bits & (1 << 5) != 0,
            is_replay: bits & (1 << 6) != 0,
            is_recovery: bits & (1 << 7) != 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodCall {
    pub method: String,
    pub params: serde_json::Value,
    pub timeout_ms: Option<u64>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodResponse {
    pub result: serde_json::Value,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOpen {
    pub stream_type: StreamType,
    pub content_type: String,
    pub parameters: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamType {
    Video,
    Text,
    Binary,
    Event,
    File,
}

impl std::fmt::Display for StreamType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Video => write!(f, "video"),
            Self::Text => write!(f, "text"),
            Self::Binary => write!(f, "binary"),
            Self::Event => write!(f, "event"),
            Self::File => write!(f, "file"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOpenAck {
    pub stream_id: u64,
    pub max_chunk_size: usize,
    pub max_chunks_per_second: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamData {
    pub sequence: u64,
    pub data: Vec<u8>,
    pub is_last: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamClose {
    pub reason: Option<String>,
    pub bytes_transferred: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMessage {
    pub topic: String,
    pub source: String,
    pub schema_version: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub sequence: u64,
    pub timestamp_ms: i64,
    pub load: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelRequest {
    pub target_request_id: Uuid,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionNegotiation {
    pub supported_versions: Vec<Version>,
    pub selected_version: Option<Version>,
    pub features: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        let v = Version::parse("1.2.3").unwrap();
        assert_eq!(v, Version::new(1, 2, 3));
    }

    #[test]
    fn version_compatibility() {
        let v1 = Version::new(1, 2, 0);
        let v2 = Version::new(1, 3, 0);
        let v3 = Version::new(2, 0, 0);
        assert!(v1.compatible_with(&v2));
        assert!(v2.compatible_with(&v1));
        assert!(!v1.compatible_with(&v3));
    }

    #[test]
    fn message_type_classification() {
        assert!(MessageType::MethodCall.is_request());
        assert!(MessageType::MethodResponse.is_response());
        assert!(MessageType::StreamOpen.is_stream());
        assert!(MessageType::Heartbeat.is_request());
        assert!(!MessageType::Heartbeat.is_response());
    }

    #[test]
    fn frame_flags_roundtrip() {
        let flags = FrameFlags {
            priority_high: true,
            compressed: true,
            requires_ack: true,
            ..Default::default()
        };
        let bits = flags.as_u16();
        let restored = FrameFlags::from_u16(bits);
        assert_eq!(flags.priority_high, restored.priority_high);
        assert_eq!(flags.compressed, restored.compressed);
        assert_eq!(flags.requires_ack, restored.requires_ack);
        assert!(!restored.priority_critical);
        assert!(!restored.encrypted);
    }

    #[test]
    fn frame_with_payload() {
        let payload = b"hello".to_vec();
        let frame = Frame::new(MessageType::MethodCall).with_payload(payload.clone());
        assert_eq!(frame.payload, payload);
        assert_eq!(frame.message_type, MessageType::MethodCall);
    }

    #[test]
    fn frame_with_builder() {
        let id = Uuid::new_v4();
        let frame = Frame::new(MessageType::MethodResponse)
            .with_request_id(id)
            .with_stream_id(42);
        assert_eq!(frame.request_id, Some(id));
        assert_eq!(frame.stream_id, Some(42));
    }

    #[test]
    fn version_display() {
        assert_eq!(Version::new(1, 2, 3).to_string(), "1.2.3");
    }

    #[test]
    fn version_parse_invalid() {
        assert!(Version::parse("1.2").is_none());
        assert!(Version::parse("abc").is_none());
    }

    #[test]
    fn stream_type_display() {
        assert_eq!(StreamType::Video.to_string(), "video");
        assert_eq!(StreamType::Event.to_string(), "event");
    }

    #[test]
    fn method_call_serialization() {
        let call = MethodCall {
            method: "test".to_string(),
            params: serde_json::json!({"key": "value"}),
            timeout_ms: Some(5000),
            idempotency_key: None,
        };
        let json = serde_json::to_string(&call).unwrap();
        let restored: MethodCall = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.method, "test");
    }

    #[test]
    fn frame_json_payload() {
        let call = MethodCall {
            method: "ping".to_string(),
            params: serde_json::json!({}),
            timeout_ms: None,
            idempotency_key: None,
        };
        let frame = Frame::from_json(MessageType::MethodCall, &call).unwrap();
        assert_eq!(frame.message_type, MessageType::MethodCall);
        let restored: MethodCall = frame.payload_json().unwrap();
        assert_eq!(restored.method, "ping");
    }

    #[test]
    fn error_frame_serialization() {
        let err = MethodError {
            code: "ERR001".to_string(),
            message: "Something went wrong".to_string(),
            details: Some(serde_json::json!({"code": 42})),
        };
        let json = serde_json::to_string(&err).unwrap();
        let restored: MethodError = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.code, "ERR001");
        assert!(restored.details.is_some());
    }

    #[test]
    fn stream_open_parameters() {
        let mut params = std::collections::HashMap::new();
        params.insert("codec".to_string(), "h264".to_string());
        let open = StreamOpen {
            stream_type: StreamType::Video,
            content_type: "video/mp4".to_string(),
            parameters: params,
        };
        assert_eq!(open.stream_type, StreamType::Video);
        assert_eq!(open.parameters.get("codec").unwrap(), "h264");
    }

    #[test]
    fn stream_data_sequence() {
        let d1 = StreamData {
            sequence: 0,
            data: vec![0, 1, 2],
            is_last: false,
        };
        let d2 = StreamData {
            sequence: 1,
            data: vec![3, 4, 5],
            is_last: true,
        };
        assert!(d2.is_last);
        assert!(!d1.is_last);
        assert!(d2.sequence > d1.sequence);
    }

    #[test]
    fn heartbeat_fields() {
        let hb = Heartbeat {
            sequence: 1,
            timestamp_ms: 1000,
            load: Some(0.5),
        };
        assert_eq!(hb.sequence, 1);
        assert_eq!(hb.load, Some(0.5));
    }

    #[test]
    fn cancel_request_creation() {
        let target = Uuid::new_v4();
        let cancel = CancelRequest {
            target_request_id: target,
            reason: Some("timeout".to_string()),
        };
        assert_eq!(cancel.target_request_id, target);
    }
}
