//! Unified Human-in-the-Loop Approval Broker for Sensitive & Destructive Actions.
//!
//! Provides deterministic risk-based authorization, secret redaction,
//! non-blocking asynchronous awaiting, timeout auto-expiry, and emergency-stop cancellation.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::{Result, SecurityError};

/// Risk classification level for operations requiring approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRiskLevel {
    /// Read-only / Safe operations.
    Safe,
    /// Low-risk state changes.
    LowRisk,
    /// Moderate modification (file edits, window changes).
    Modify,
    /// Elevated execution (sandboxed commands, process launch).
    Privileged,
    /// Destructive operation (file deletion, process killing, disk/system alterations).
    Destructive,
}

impl ApprovalRiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Safe => "Safe",
            Self::LowRisk => "LowRisk",
            Self::Modify => "Modify",
            Self::Privileged => "Privileged",
            Self::Destructive => "Destructive",
        }
    }
}

/// Status of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    Cancelled,
}

impl ApprovalStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Approved | Self::Denied | Self::Expired | Self::Cancelled
        )
    }
}

/// Description of an action awaiting human approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub session_id: String,
    pub tool_name: String,
    pub parameters_redacted: serde_json::Value,
    pub risk_level: ApprovalRiskLevel,
    pub reason: String,
    pub requested_at: DateTime<Utc>,
    pub timeout_secs: u64,
}

impl ApprovalRequest {
    pub fn new(
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        raw_params: &serde_json::Value,
        risk_level: ApprovalRiskLevel,
        reason: impl Into<String>,
        timeout_secs: u64,
    ) -> Self {
        let redacted = redact_sensitive_values(raw_params);
        Self {
            id: Uuid::new_v4(),
            session_id: session_id.into(),
            tool_name: tool_name.into(),
            parameters_redacted: redacted,
            risk_level,
            reason: reason.into(),
            requested_at: Utc::now(),
            timeout_secs,
        }
    }
}

/// Outcome of a resolved approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub request_id: Uuid,
    pub status: ApprovalStatus,
    pub decided_at: DateTime<Utc>,
    pub reason: Option<String>,
}

/// Internal entry storing metadata and the completion sender.
struct PendingApprovalEntry {
    request: ApprovalRequest,
    status: ApprovalStatus,
    created_at_instant: Instant,
    timeout: Duration,
    tx: Option<oneshot::Sender<ApprovalDecision>>,
}

use tokio::sync::broadcast;

/// Unified, thread-safe Approval Broker.
#[derive(Clone)]
pub struct ApprovalBroker {
    entries: Arc<RwLock<HashMap<Uuid, PendingApprovalEntry>>>,
    history: Arc<RwLock<Vec<ApprovalDecision>>>,
    request_tx: broadcast::Sender<ApprovalRequest>,
    decision_tx: broadcast::Sender<ApprovalDecision>,
}

impl Default for ApprovalBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalBroker {
    pub fn new() -> Self {
        let (request_tx, _) = broadcast::channel(128);
        let (decision_tx, _) = broadcast::channel(128);
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            request_tx,
            decision_tx,
        }
    }

    /// Subscribe to new approval request broadcasts.
    pub fn subscribe_requests(&self) -> broadcast::Receiver<ApprovalRequest> {
        self.request_tx.subscribe()
    }

    /// Subscribe to approval decision broadcasts.
    pub fn subscribe_decisions(&self) -> broadcast::Receiver<ApprovalDecision> {
        self.decision_tx.subscribe()
    }

    /// Register a new action requiring human approval.
    /// Returns the `ApprovalRequest` and an async receiver that resolves once approved, denied, expired, or cancelled.
    pub fn submit_request(
        &self,
        session_id: impl Into<String>,
        tool_name: impl Into<String>,
        raw_params: &serde_json::Value,
        risk_level: ApprovalRiskLevel,
        reason: impl Into<String>,
        timeout: Duration,
    ) -> (ApprovalRequest, oneshot::Receiver<ApprovalDecision>) {
        let timeout_secs = timeout.as_secs().max(1);
        let request = ApprovalRequest::new(
            session_id,
            tool_name,
            raw_params,
            risk_level,
            reason,
            timeout_secs,
        );
        let id = request.id;
        let (tx, rx) = oneshot::channel();

        let entry = PendingApprovalEntry {
            request: request.clone(),
            status: ApprovalStatus::Pending,
            created_at_instant: Instant::now(),
            timeout,
            tx: Some(tx),
        };

        self.entries.write().insert(id, entry);

        let _ = self.request_tx.send(request.clone());

        info!(
            request_id = %id,
            tool = %request.tool_name,
            risk = ?request.risk_level,
            "Approval request registered"
        );

        (request, rx)
    }

    /// Resolve an approval request with user decision.
    pub fn resolve(
        &self,
        request_id: Uuid,
        approved: bool,
        reason: Option<String>,
    ) -> Result<ApprovalDecision> {
        let mut entries = self.entries.write();
        let entry = entries
            .get_mut(&request_id)
            .ok_or_else(|| SecurityError::Internal(format!("Approval request {request_id} not found")))?;

        // Check if already expired by wall clock
        if entry.created_at_instant.elapsed() > entry.timeout {
            entry.status = ApprovalStatus::Expired;
            let decision = ApprovalDecision {
                request_id,
                status: ApprovalStatus::Expired,
                decided_at: Utc::now(),
                reason: Some("Request timed out before decision was received".to_string()),
            };
            if let Some(tx) = entry.tx.take() {
                let _ = tx.send(decision.clone());
            }
            self.history.write().push(decision.clone());
            let _ = self.decision_tx.send(decision);
            return Err(SecurityError::Internal(format!(
                "Approval request {request_id} expired"
            )));
        }

        if entry.status.is_terminal() {
            return Err(SecurityError::Internal(format!(
                "Approval request {request_id} is already in terminal state {:?}",
                entry.status
            )));
        }

        let status = if approved {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Denied
        };

        entry.status = status;

        let decision = ApprovalDecision {
            request_id,
            status,
            decided_at: Utc::now(),
            reason,
        };

        if let Some(tx) = entry.tx.take() {
            let _ = tx.send(decision.clone());
        }

        self.history.write().push(decision.clone());
        let _ = self.decision_tx.send(decision.clone());

        info!(
            request_id = %request_id,
            status = ?status,
            "Approval request resolved"
        );

        Ok(decision)
    }

    /// Cancel a specific request (e.g. user aborted subtask).
    pub fn cancel(&self, request_id: Uuid, reason: Option<String>) -> Result<ApprovalDecision> {
        let mut entries = self.entries.write();
        let entry = entries
            .get_mut(&request_id)
            .ok_or_else(|| SecurityError::Internal(format!("Approval request {request_id} not found")))?;

        if entry.status.is_terminal() {
            return Err(SecurityError::Internal(format!(
                "Approval request {request_id} is already terminal"
            )));
        }

        entry.status = ApprovalStatus::Cancelled;

        let decision = ApprovalDecision {
            request_id,
            status: ApprovalStatus::Cancelled,
            decided_at: Utc::now(),
            reason,
        };

        if let Some(tx) = entry.tx.take() {
            let _ = tx.send(decision.clone());
        }

        self.history.write().push(decision.clone());
        let _ = self.decision_tx.send(decision.clone());
        Ok(decision)
    }

    /// Emergency Stop: Cancel ALL currently pending approval requests immediately.
    pub fn emergency_stop_all(&self) -> Vec<ApprovalDecision> {
        warn!("Emergency stop triggered in ApprovalBroker: cancelling all pending requests");
        let mut entries = self.entries.write();
        let mut cancelled = Vec::new();

        for (id, entry) in entries.iter_mut() {
            if entry.status == ApprovalStatus::Pending {
                entry.status = ApprovalStatus::Cancelled;
                let decision = ApprovalDecision {
                    request_id: *id,
                    status: ApprovalStatus::Cancelled,
                    decided_at: Utc::now(),
                    reason: Some("Global Emergency Stop invoked".to_string()),
                };
                if let Some(tx) = entry.tx.take() {
                    let _ = tx.send(decision.clone());
                }
                let _ = self.decision_tx.send(decision.clone());
                cancelled.push(decision);
            }
        }

        let mut hist = self.history.write();
        hist.extend(cancelled.clone());
        cancelled
    }

    /// Clean up expired pending requests.
    pub fn sweep_expired(&self) -> Vec<ApprovalDecision> {
        let mut entries = self.entries.write();
        let mut expired = Vec::new();

        for (id, entry) in entries.iter_mut() {
            if entry.status == ApprovalStatus::Pending
                && entry.created_at_instant.elapsed() > entry.timeout
            {
                entry.status = ApprovalStatus::Expired;
                let decision = ApprovalDecision {
                    request_id: *id,
                    status: ApprovalStatus::Expired,
                    decided_at: Utc::now(),
                    reason: Some("Request timed out automatically".to_string()),
                };
                if let Some(tx) = entry.tx.take() {
                    let _ = tx.send(decision.clone());
                }
                let _ = self.decision_tx.send(decision.clone());
                expired.push(decision);
            }
        }

        if !expired.is_empty() {
            let mut hist = self.history.write();
            hist.extend(expired.clone());
        }

        expired
    }

    /// Get details of all currently pending requests.
    pub fn get_pending(&self) -> Vec<ApprovalRequest> {
        let entries = self.entries.read();
        entries
            .values()
            .filter(|e| e.status == ApprovalStatus::Pending && e.created_at_instant.elapsed() <= e.timeout)
            .map(|e| e.request.clone())
            .collect()
    }

    /// Get current status of a specific request.
    pub fn get_status(&self, request_id: Uuid) -> Option<ApprovalStatus> {
        let entries = self.entries.read();
        entries.get(&request_id).map(|e| {
            if e.status == ApprovalStatus::Pending && e.created_at_instant.elapsed() > e.timeout {
                ApprovalStatus::Expired
            } else {
                e.status
            }
        })
    }
}

/// Recursively redact parameters that could contain secrets or credentials.
pub fn redact_sensitive_values(val: &serde_json::Value) -> serde_json::Value {
    match val {
        serde_json::Value::Object(map) => {
            let mut sanitized = serde_json::Map::new();
            for (k, v) in map {
                let lower_k = k.to_lowercase();
                if lower_k.contains("secret")
                    || lower_k.contains("token")
                    || lower_k.contains("password")
                    || lower_k.contains("passwd")
                    || lower_k.contains("api_key")
                    || lower_k.contains("apikey")
                    || lower_k.contains("auth")
                    || lower_k.contains("credential")
                    || lower_k.contains("private_key")
                    || lower_k.contains("bearer")
                    || lower_k.contains("session_key")
                    || lower_k.contains("cert")
                    || lower_k.contains("cookie")
                {
                    sanitized.insert(k.clone(), serde_json::json!("[REDACTED]"));
                } else {
                    sanitized.insert(k.clone(), redact_sensitive_values(v));
                }
            }
            serde_json::Value::Object(sanitized)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(redact_sensitive_values).collect())
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_redaction() {
        let params = serde_json::json!({
            "target": "C:\\Windows\\system32",
            "api_key": "sk-1234567890abcdef",
            "db_password": "supersecretpassword",
            "auth_token": "bearer eyJhbGciOi...",
            "nested": {
                "client_secret": "xyz",
                "normal_field": "visible value"
            }
        });

        let redacted = redact_sensitive_values(&params);
        assert_eq!(redacted["target"], "C:\\Windows\\system32");
        assert_eq!(redacted["api_key"], "[REDACTED]");
        assert_eq!(redacted["db_password"], "[REDACTED]");
        assert_eq!(redacted["auth_token"], "[REDACTED]");
        assert_eq!(redacted["nested"]["client_secret"], "[REDACTED]");
        assert_eq!(redacted["nested"]["normal_field"], "visible value");
    }

    #[tokio::test]
    async fn test_broker_approve_flow() {
        let broker = ApprovalBroker::new();
        let (req, rx) = broker.submit_request(
            "sess-1",
            "file_delete",
            &serde_json::json!({"path": "/tmp/test"}),
            ApprovalRiskLevel::Destructive,
            "User requested file cleanup",
            Duration::from_secs(10),
        );

        assert_eq!(broker.get_status(req.id), Some(ApprovalStatus::Pending));

        let res = broker.resolve(req.id, true, Some("Approved by user".into()));
        assert!(res.is_ok());

        let decision = rx.await.unwrap();
        assert_eq!(decision.status, ApprovalStatus::Approved);
        assert_eq!(broker.get_status(req.id), Some(ApprovalStatus::Approved));
    }

    #[tokio::test]
    async fn test_broker_deny_flow() {
        let broker = ApprovalBroker::new();
        let (req, rx) = broker.submit_request(
            "sess-1",
            "process_kill",
            &serde_json::json!({"pid": 1234}),
            ApprovalRiskLevel::Destructive,
            "Kill background process",
            Duration::from_secs(10),
        );

        let res = broker.resolve(req.id, false, Some("Denied by user".into()));
        assert!(res.is_ok());

        let decision = rx.await.unwrap();
        assert_eq!(decision.status, ApprovalStatus::Denied);
        assert_eq!(broker.get_status(req.id), Some(ApprovalStatus::Denied));
    }

    #[tokio::test]
    async fn test_broker_timeout_flow() {
        let broker = ApprovalBroker::new();
        let (req, rx) = broker.submit_request(
            "sess-1",
            "file_delete",
            &serde_json::json!({}),
            ApprovalRiskLevel::Destructive,
            "Timeout test",
            Duration::from_millis(50),
        );

        tokio::time::sleep(Duration::from_millis(70)).await;

        let expired = broker.sweep_expired();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].status, ApprovalStatus::Expired);

        let decision = rx.await.unwrap();
        assert_eq!(decision.status, ApprovalStatus::Expired);
        assert_eq!(broker.get_status(req.id), Some(ApprovalStatus::Expired));

        // Approving after expiry should fail
        assert!(broker.resolve(req.id, true, None).is_err());
    }

    #[tokio::test]
    async fn test_broker_cancellation_and_emergency_stop() {
        let broker = ApprovalBroker::new();
        let (req1, rx1) = broker.submit_request(
            "sess-1",
            "tool_1",
            &serde_json::json!({}),
            ApprovalRiskLevel::Privileged,
            "Task 1",
            Duration::from_secs(10),
        );
        let (req2, rx2) = broker.submit_request(
            "sess-1",
            "tool_2",
            &serde_json::json!({}),
            ApprovalRiskLevel::Destructive,
            "Task 2",
            Duration::from_secs(10),
        );

        let cancelled = broker.emergency_stop_all();
        assert_eq!(cancelled.len(), 2);

        let d1 = rx1.await.unwrap();
        let d2 = rx2.await.unwrap();
        assert_eq!(d1.status, ApprovalStatus::Cancelled);
        assert_eq!(d2.status, ApprovalStatus::Cancelled);

        assert_eq!(broker.get_status(req1.id), Some(ApprovalStatus::Cancelled));
        assert_eq!(broker.get_status(req2.id), Some(ApprovalStatus::Cancelled));
    }

    #[tokio::test]
    async fn test_replay_prevention() {
        let broker = ApprovalBroker::new();
        let (req, _rx) = broker.submit_request(
            "sess-1",
            "file_delete",
            &serde_json::json!({}),
            ApprovalRiskLevel::Destructive,
            "Replay test",
            Duration::from_secs(10),
        );

        assert!(broker.resolve(req.id, true, None).is_ok());
        // Second resolve must fail
        assert!(broker.resolve(req.id, true, None).is_err());
        assert!(broker.resolve(req.id, false, None).is_err());
    }

    #[tokio::test]
    async fn test_race_condition_resolve_vs_emergency_stop() {
        let broker = ApprovalBroker::new();
        let (req, rx) = broker.submit_request(
            "sess-1",
            "file_delete",
            &serde_json::json!({}),
            ApprovalRiskLevel::Destructive,
            "Race condition test",
            Duration::from_secs(10),
        );

        // Emergency stop first
        let stopped = broker.emergency_stop_all();
        assert_eq!(stopped.len(), 1);

        // Attempting to resolve after emergency stop must fail
        let res = broker.resolve(req.id, true, None);
        assert!(res.is_err());

        let decision = rx.await.unwrap();
        assert_eq!(decision.status, ApprovalStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_race_condition_resolve_vs_cancel() {
        let broker = ApprovalBroker::new();
        let (req, rx) = broker.submit_request(
            "sess-1",
            "db_drop",
            &serde_json::json!({}),
            ApprovalRiskLevel::Destructive,
            "Cancel race test",
            Duration::from_secs(10),
        );

        let cancel_res = broker.cancel(req.id, Some("User pressed cancel".into()));
        assert!(cancel_res.is_ok());

        // Subsequent approve must fail
        let resolve_res = broker.resolve(req.id, true, None);
        assert!(resolve_res.is_err());

        let decision = rx.await.unwrap();
        assert_eq!(decision.status, ApprovalStatus::Cancelled);
    }

    #[test]
    fn test_unknown_and_malformed_request_resolution() {
        let broker = ApprovalBroker::new();
        let bogus_id = Uuid::new_v4();

        // Resolving an unknown request ID must fail
        let res = broker.resolve(bogus_id, true, None);
        assert!(res.is_err());

        // Cancelling an unknown request ID must fail
        let cancel_res = broker.cancel(bogus_id, None);
        assert!(cancel_res.is_err());
    }

    #[test]
    fn test_audit_trail_history_does_not_contain_raw_secrets() {
        let broker = ApprovalBroker::new();
        let (req, _rx) = broker.submit_request(
            "sess-audit",
            "api_call",
            &serde_json::json!({
                "api_key": "super_secret_token_12345",
                "session_cookie": "sess_abcde",
                "normal_param": "safe"
            }),
            ApprovalRiskLevel::Privileged,
            "Audit redaction verification",
            Duration::from_secs(10),
        );

        // The request parameters stored in the broker must be redacted
        assert_eq!(req.parameters_redacted["api_key"], "[REDACTED]");
        assert_eq!(req.parameters_redacted["session_cookie"], "[REDACTED]");
        assert_eq!(req.parameters_redacted["normal_param"], "safe");

        let pending = broker.get_pending();
        assert_eq!(pending.len(), 1);
        let serialized = serde_json::to_string(&pending[0]).unwrap();
        assert!(!serialized.contains("super_secret_token_12345"));
        assert!(!serialized.contains("sess_abcde"));
    }
}
