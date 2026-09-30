use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReplayMode {
    Live,
    Replay,
    Snapshot,
    History,
    Recovery,
}

impl std::fmt::Display for ReplayMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Live => write!(f, "live"),
            Self::Replay => write!(f, "replay"),
            Self::Snapshot => write!(f, "snapshot"),
            Self::History => write!(f, "history"),
            Self::Recovery => write!(f, "recovery"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySubscription {
    pub subscription_id: Uuid,
    pub topic: String,
    pub mode: ReplayMode,
    pub replay_from: Option<DateTime<Utc>>,
    pub replay_to: Option<DateTime<Utc>>,
    pub replay_rate: Option<f64>,
    pub checkpoint_id: Option<String>,
    pub filter: serde_json::Value,
    pub delivery: DeliveryGuarantee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryGuarantee {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayStatus {
    pub subscription_id: Uuid,
    pub mode: ReplayMode,
    pub total_events: u64,
    pub events_delivered: u64,
    pub estimated_duration_ms: u64,
    pub elapsed_ms: u64,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub state: ReplayState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplayState {
    Pending,
    Replaying,
    CaughtUp,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryCheckpoint {
    pub subscription_id: Uuid,
    pub last_replayed_event_id: String,
    pub state_hash: String,
    pub events_since_checkpoint: u64,
    pub next_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub event_id: String,
    pub topic: String,
    pub payload: Vec<u8>,
    pub metadata: EventMetadata,
    pub timestamp: DateTime<Utc>,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetadata {
    pub source: String,
    pub correlation_id: Option<Uuid>,
    pub schema_version: String,
    pub size_bytes: u64,
}

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn store(&self, topic: &str, event: &StoredEvent) -> crate::error::Result<()>;
    async fn replay(
        &self,
        topic: &str,
        from: DateTime<Utc>,
        to: Option<DateTime<Utc>>,
        rate: f64,
    ) -> crate::error::Result<Vec<StoredEvent>>;
    async fn snapshot(
        &self,
        topic: &str,
    ) -> crate::error::Result<std::collections::HashMap<String, StoredEvent>>;
    async fn recovery(&self, checkpoint_id: &str) -> crate::error::Result<Vec<StoredEvent>>;
    async fn cleanup_expired(&self) -> crate::error::Result<u64>;
}

impl Default for ReplaySubscription {
    fn default() -> Self {
        Self {
            subscription_id: Uuid::new_v4(),
            topic: String::new(),
            mode: ReplayMode::Live,
            replay_from: None,
            replay_to: None,
            replay_rate: None,
            checkpoint_id: None,
            filter: serde_json::json!({}),
            delivery: DeliveryGuarantee::AtLeastOnce,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_mode_display() {
        assert_eq!(ReplayMode::Live.to_string(), "live");
        assert_eq!(ReplayMode::Replay.to_string(), "replay");
        assert_eq!(ReplayMode::Snapshot.to_string(), "snapshot");
        assert_eq!(ReplayMode::Recovery.to_string(), "recovery");
    }

    #[test]
    fn replay_subscription_default() {
        let sub = ReplaySubscription {
            subscription_id: Uuid::new_v4(),
            topic: "voxy.test".to_string(),
            mode: ReplayMode::Live,
            replay_from: None,
            replay_to: None,
            replay_rate: None,
            checkpoint_id: None,
            filter: serde_json::json!({}),
            delivery: DeliveryGuarantee::AtLeastOnce,
        };
        assert_eq!(sub.topic, "voxy.test");
        assert_eq!(sub.mode, ReplayMode::Live);
    }

    #[test]
    fn replay_subscription_with_replay() {
        let now = Utc::now();
        let sub = ReplaySubscription {
            subscription_id: Uuid::new_v4(),
            topic: "voxy.voice.transcript".to_string(),
            mode: ReplayMode::Replay,
            replay_from: Some(now),
            replay_rate: Some(2.0),
            ..Default::default()
        };
        assert_eq!(sub.mode, ReplayMode::Replay);
        assert!(sub.replay_from.is_some());
        assert_eq!(sub.replay_rate, Some(2.0));
    }

    #[test]
    fn replay_status_transition() {
        let status = ReplayStatus {
            subscription_id: Uuid::new_v4(),
            mode: ReplayMode::Replay,
            total_events: 1000,
            events_delivered: 500,
            estimated_duration_ms: 5000,
            elapsed_ms: 2500,
            from: Some(Utc::now()),
            to: None,
            state: ReplayState::Replaying,
        };
        assert_eq!(status.state, ReplayState::Replaying);
        assert_eq!(status.events_delivered, 500);
    }

    #[test]
    fn stored_event_with_ttl() {
        let event = StoredEvent {
            event_id: "evt_001".to_string(),
            topic: "voxy.test".to_string(),
            payload: vec![1, 2, 3],
            metadata: EventMetadata {
                source: "test".to_string(),
                correlation_id: None,
                schema_version: "1.0.0".to_string(),
                size_bytes: 3,
            },
            timestamp: Utc::now(),
            ttl_secs: 86400,
        };
        assert_eq!(event.ttl_secs, 86400);
        assert_eq!(event.metadata.size_bytes, 3);
    }

    #[test]
    fn recovery_checkpoint_creation() {
        let cp = RecoveryCheckpoint {
            subscription_id: Uuid::new_v4(),
            last_replayed_event_id: "evt_99999".to_string(),
            state_hash: "sha256:abc123".to_string(),
            events_since_checkpoint: 42,
            next_event_id: "evt_100000".to_string(),
        };
        assert_eq!(cp.last_replayed_event_id, "evt_99999");
        assert_eq!(cp.events_since_checkpoint, 42);
    }

    #[test]
    fn delivery_guarantee_ordering() {
        assert!((DeliveryGuarantee::AtMostOnce as u8) < (DeliveryGuarantee::AtLeastOnce as u8));
        assert!((DeliveryGuarantee::AtLeastOnce as u8) < (DeliveryGuarantee::ExactlyOnce as u8));
    }

    #[test]
    fn replay_state_variants() {
        assert_eq!(format!("{:?}", ReplayState::Pending), "Pending");
        assert_eq!(format!("{:?}", ReplayState::CaughtUp), "CaughtUp");
        match ReplayState::Failed("error".to_string()) {
            ReplayState::Failed(msg) => assert_eq!(msg, "error"),
            _ => panic!("Wrong variant"),
        }
    }
}
