use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditStatus {
    Success,
    Denied,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficeAuditEvent {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub connector: String,
    pub operation: String,
    pub resource: String,
    pub user_id: Option<String>,
    pub required_scope: String,
    pub status: AuditStatus,
    pub details: serde_json::Value,
}

impl OfficeAuditEvent {
    pub fn new(
        connector: &str,
        operation: &str,
        resource: &str,
        user_id: Option<&str>,
        required_scope: &str,
        status: AuditStatus,
        details: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            connector: connector.to_string(),
            operation: operation.to_string(),
            resource: resource.to_string(),
            user_id: user_id.map(|s| s.to_string()),
            required_scope: required_scope.to_string(),
            status,
            details,
        }
    }
}

/// In-memory structured audit trail for office automation actions.
#[derive(Debug, Clone)]
pub struct OfficeAuditTrail {
    events: Arc<RwLock<Vec<OfficeAuditEvent>>>,
    max_records: usize,
}

impl Default for OfficeAuditTrail {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl OfficeAuditTrail {
    pub fn new(max_records: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            max_records,
        }
    }

    pub fn record(&self, event: OfficeAuditEvent) {
        let mut events = self.events.write();
        if events.len() >= self.max_records {
            events.remove(0);
        }
        events.push(event);
    }

    pub fn get_events(&self) -> Vec<OfficeAuditEvent> {
        self.events.read().clone()
    }

    pub fn filter_by_connector(&self, connector: &str) -> Vec<OfficeAuditEvent> {
        self.events
            .read()
            .iter()
            .filter(|e| e.connector == connector)
            .cloned()
            .collect()
    }

    pub fn filter_by_status(&self, status: AuditStatus) -> Vec<OfficeAuditEvent> {
        self.events
            .read()
            .iter()
            .filter(|e| e.status == status)
            .cloned()
            .collect()
    }
}
