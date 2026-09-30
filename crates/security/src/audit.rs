use std::collections::VecDeque;

use crate::policy::AuditLevel;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Structured audit event types as defined in the security architecture spec §2.8.
/// Replaces ad-hoc string-based action tracking with typed, queryable events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    Authentication {
        method: String,
        success: bool,
    },
    Authorization {
        decision: String,
    },
    Consent {
        capability: String,
        granted: bool,
    },
    SecretAccess {
        secret_id: String,
        operation: String,
    },
    PolicyChange {
        policy_id: String,
        change: String,
    },
    ConfigurationChange {
        config: String,
        old_value: String,
        new_value: String,
    },
    ComponentLoad {
        component: String,
        hash: String,
    },
    ComponentUnload {
        component: String,
    },
    IntegrityCheck {
        result: String,
    },
    ThreatDetected {
        threat_type: String,
        severity: String,
    },
    GuardianModeActivated {
        reason: String,
    },
    RecoveryModeActivated {
        reason: String,
    },
    Zeroization {
        reason: String,
    },
}

impl AuditEventType {
    pub fn category(&self) -> &'static str {
        match self {
            Self::Authentication { .. } => "authentication",
            Self::Authorization { .. } => "authorization",
            Self::Consent { .. } => "consent",
            Self::SecretAccess { .. } => "secret_access",
            Self::PolicyChange { .. } => "policy_change",
            Self::ConfigurationChange { .. } => "config_change",
            Self::ComponentLoad { .. } => "component_load",
            Self::ComponentUnload { .. } => "component_unload",
            Self::IntegrityCheck { .. } => "integrity_check",
            Self::ThreatDetected { .. } => "threat_detected",
            Self::GuardianModeActivated { .. } => "guardian_mode",
            Self::RecoveryModeActivated { .. } => "recovery_mode",
            Self::Zeroization { .. } => "zeroization",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub subject: String,
    pub action: String,
    pub resource: Option<String>,
    pub result: String,
    pub reason: Option<String>,
    pub risk_level: String,
    pub trust_level: String,
    pub previous_hash: String,
    pub hash: String,
    pub audit_level: AuditLevel,
    pub metadata: std::collections::HashMap<String, String>,
    /// Structured event type for typed querying (optional for backward compat).
    pub event_type: Option<AuditEventType>,
}

impl AuditEntry {
    fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.to_string().as_bytes());
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(self.subject.as_bytes());
        hasher.update(self.action.as_bytes());
        hasher.update(self.result.as_bytes());
        hasher.update(self.previous_hash.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn verify_integrity(&self) -> bool {
        let computed = self.compute_hash();
        computed == self.hash
    }
}

pub struct AuditLog {
    entries: VecDeque<AuditEntry>,
    last_hash: String,
    max_entries: usize,
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            last_hash: String::new(),
            max_entries: 10000,
        }
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = max;
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        subject: &str,
        action: &str,
        resource: Option<&str>,
        result: &str,
        reason: Option<&str>,
        risk_level: &str,
        trust_level: &str,
        audit_level: AuditLevel,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let mut entry = AuditEntry {
            id,
            timestamp: Utc::now(),
            subject: subject.to_string(),
            action: action.to_string(),
            resource: resource.map(|r| r.to_string()),
            result: result.to_string(),
            reason: reason.map(|r| r.to_string()),
            risk_level: risk_level.to_string(),
            trust_level: trust_level.to_string(),
            previous_hash: self.last_hash.clone(),
            hash: String::new(),
            audit_level,
            metadata: std::collections::HashMap::new(),
            event_type: None,
        };
        entry.hash = entry.compute_hash();
        self.last_hash = entry.hash.clone();
        self.entries.push_back(entry);

        if self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        id
    }

    /// Record an audit entry with a structured AuditEventType.
    /// This is the preferred method for new code — provides typed, queryable events.
    #[allow(clippy::too_many_arguments)]
    pub fn record_typed(
        &mut self,
        subject: &str,
        action: &str,
        resource: Option<&str>,
        result: &str,
        reason: Option<&str>,
        risk_level: &str,
        trust_level: &str,
        audit_level: AuditLevel,
        event_type: AuditEventType,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let mut entry = AuditEntry {
            id,
            timestamp: Utc::now(),
            subject: subject.to_string(),
            action: action.to_string(),
            resource: resource.map(|r| r.to_string()),
            result: result.to_string(),
            reason: reason.map(|r| r.to_string()),
            risk_level: risk_level.to_string(),
            trust_level: trust_level.to_string(),
            previous_hash: self.last_hash.clone(),
            hash: String::new(),
            audit_level,
            metadata: std::collections::HashMap::new(),
            event_type: Some(event_type),
        };
        entry.hash = entry.compute_hash();
        self.last_hash = entry.hash.clone();
        self.entries.push_back(entry);

        if self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        id
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_with_metadata(
        &mut self,
        subject: &str,
        action: &str,
        resource: Option<&str>,
        result: &str,
        reason: Option<&str>,
        risk_level: &str,
        trust_level: &str,
        audit_level: AuditLevel,
        metadata: std::collections::HashMap<String, String>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let mut entry = AuditEntry {
            id,
            timestamp: Utc::now(),
            subject: subject.to_string(),
            action: action.to_string(),
            resource: resource.map(|r| r.to_string()),
            result: result.to_string(),
            reason: reason.map(|r| r.to_string()),
            risk_level: risk_level.to_string(),
            trust_level: trust_level.to_string(),
            previous_hash: self.last_hash.clone(),
            hash: String::new(),
            audit_level,
            metadata,
            event_type: None,
        };
        entry.hash = entry.compute_hash();
        self.last_hash = entry.hash.clone();
        self.entries.push_back(entry);

        if self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        id
    }

    pub fn verify_chain(&self) -> bool {
        let mut prev_hash = String::new();
        for entry in &self.entries {
            if entry.previous_hash != prev_hash {
                return false;
            }
            if !entry.verify_integrity() {
                return false;
            }
            prev_hash = entry.hash.clone();
        }
        true
    }

    pub fn query_by_subject(&self, subject: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.subject == subject)
            .collect()
    }

    pub fn query_by_action(&self, action: &str) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.action == action).collect()
    }

    pub fn query_by_time_range(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= from && e.timestamp <= to)
            .collect()
    }

    /// Query entries that have a specific AuditEventType category.
    pub fn query_by_event_category(&self, category: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| {
                e.event_type
                    .as_ref()
                    .map(|et| et.category() == category)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Get the last N entries (most recent first).
    pub fn recent(&self, count: usize) -> Vec<&AuditEntry> {
        self.entries.iter().rev().take(count).collect()
    }

    /// Export audit entries as JSON for compliance/audit.
    pub fn export_json(&self) -> serde_json::Value {
        serde_json::to_value::<VecDeque<AuditEntry>>(self.entries.clone())
            .unwrap_or(serde_json::Value::Null)
    }

    pub fn entries(&self) -> &VecDeque<AuditEntry> {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_log_records_entry() {
        let mut log = AuditLog::new();
        let id = log.record(
            "user-1",
            "file:read",
            Some("/etc/config"),
            "allowed",
            None,
            "medium",
            "trusted",
            AuditLevel::Basic,
        );
        assert_eq!(log.len(), 1);
        assert!(!id.is_nil());
    }

    #[test]
    fn audit_chain_integrity() {
        let mut log = AuditLog::new();
        log.record(
            "user-1",
            "file:read",
            None,
            "allowed",
            None,
            "low",
            "trusted",
            AuditLevel::Basic,
        );
        log.record(
            "user-2",
            "file:write",
            None,
            "denied",
            Some("no permission"),
            "high",
            "known",
            AuditLevel::Detailed,
        );
        assert!(log.verify_chain());
    }

    #[test]
    fn audit_chain_tamper_detection() {
        let mut log = AuditLog::new();
        log.record(
            "user-1",
            "file:read",
            None,
            "allowed",
            None,
            "low",
            "trusted",
            AuditLevel::Basic,
        );
        log.record(
            "user-2",
            "file:write",
            None,
            "denied",
            Some("no permission"),
            "high",
            "known",
            AuditLevel::Detailed,
        );
        if let Some(entry) = log.entries.back_mut() {
            entry.result = "allowed".to_string();
        }
        assert!(!log.verify_chain());
    }

    #[test]
    fn audit_entry_integrity_check() {
        let mut log = AuditLog::new();
        log.record(
            "user",
            "action",
            None,
            "ok",
            None,
            "low",
            "trusted",
            AuditLevel::Basic,
        );
        let entry = log.entries.front().unwrap();
        assert!(entry.verify_integrity());
    }

    #[test]
    fn audit_query_by_subject() {
        let mut log = AuditLog::new();
        log.record(
            "alice",
            "file:read",
            None,
            "allowed",
            None,
            "low",
            "trusted",
            AuditLevel::Basic,
        );
        log.record(
            "bob",
            "file:write",
            None,
            "denied",
            None,
            "high",
            "known",
            AuditLevel::Basic,
        );
        log.record(
            "alice",
            "file:read",
            None,
            "allowed",
            None,
            "low",
            "trusted",
            AuditLevel::Basic,
        );
        assert_eq!(log.query_by_subject("alice").len(), 2);
        assert_eq!(log.query_by_subject("bob").len(), 1);
    }

    #[test]
    fn audit_export_json() {
        let mut log = AuditLog::new();
        log.record(
            "user",
            "action",
            None,
            "ok",
            None,
            "low",
            "trusted",
            AuditLevel::Basic,
        );
        let json = log.export_json();
        assert!(json.is_array());
    }

    #[test]
    fn audit_max_entries() {
        let mut log = AuditLog::new().with_max_entries(5);
        for i in 0..10 {
            log.record(
                &format!("user-{i}"),
                "action",
                None,
                "ok",
                None,
                "low",
                "trusted",
                AuditLevel::Basic,
            );
        }
        assert_eq!(log.len(), 5);
    }

    #[test]
    fn audit_recent_returns_last_n() {
        let mut log = AuditLog::new();
        for i in 0..10 {
            log.record(
                &format!("user-{i}"),
                "action",
                None,
                "ok",
                None,
                "low",
                "trusted",
                AuditLevel::Basic,
            );
        }
        let recent = log.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].subject, "user-9");
        assert_eq!(recent[2].subject, "user-7");
    }

    #[test]
    fn audit_query_by_event_category() {
        let mut log = AuditLog::new();
        log.record_typed(
            "user-1",
            "tool_call",
            None,
            "allowed",
            None,
            "low",
            "trusted",
            AuditLevel::Basic,
            AuditEventType::Consent {
                capability: "tool_call".to_string(),
                granted: true,
            },
        );
        log.record_typed(
            "user-2",
            "file_write",
            None,
            "denied",
            None,
            "high",
            "verified",
            AuditLevel::Full,
            AuditEventType::SecretAccess {
                secret_id: "db-password".to_string(),
                operation: "read".to_string(),
            },
        );
        let consent_events = log.query_by_event_category("consent");
        assert_eq!(consent_events.len(), 1);
        assert_eq!(consent_events[0].subject, "user-1");
        let secret_events = log.query_by_event_category("secret_access");
        assert_eq!(secret_events.len(), 1);
        assert_eq!(secret_events[0].subject, "user-2");
    }
}
