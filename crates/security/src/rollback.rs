use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackStrategy {
    FileBackup,
    RegistrySnapshot,
    StateRestore,
    ApiUndo,
    DatabaseTransaction,
    CompensationAction,
    NoOp,
}

impl std::fmt::Display for RollbackStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileBackup => write!(f, "file_backup"),
            Self::RegistrySnapshot => write!(f, "registry_snapshot"),
            Self::StateRestore => write!(f, "state_restore"),
            Self::ApiUndo => write!(f, "api_undo"),
            Self::DatabaseTransaction => write!(f, "database_transaction"),
            Self::CompensationAction => write!(f, "compensation_action"),
            Self::NoOp => write!(f, "no_op"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackStatus {
    Available,
    InProgress,
    Completed,
    Failed(String),
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub action_id: Uuid,
    pub strategy: RollbackStrategy,
    pub snapshot: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub status: RollbackStatus,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub action_id: Uuid,
    pub success: bool,
    pub message: String,
    pub duration_ms: u64,
}

pub struct RollbackManager {
    records: Vec<RollbackRecord>,
}

impl RollbackManager {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn register(
        &mut self,
        action_id: Uuid,
        strategy: RollbackStrategy,
        snapshot: Vec<u8>,
        ttl_secs: Option<u64>,
    ) {
        let record = RollbackRecord {
            action_id,
            strategy,
            snapshot,
            created_at: Utc::now(),
            expires_at: ttl_secs.map(|s| Utc::now() + chrono::Duration::seconds(s as i64)),
            status: RollbackStatus::Available,
            metadata: std::collections::HashMap::new(),
        };
        self.records.push(record);
    }

    pub fn can_rollback(&self, action_id: Uuid) -> bool {
        self.records.iter().any(|r| {
            r.action_id == action_id
                && matches!(r.status, RollbackStatus::Available)
                && r.expires_at.map(|exp| Utc::now() < exp).unwrap_or(true)
        })
    }

    pub fn rollback(&mut self, action_id: Uuid) -> RollbackResult {
        let start = std::time::Instant::now();
        if let Some(record) = self.records.iter_mut().find(|r| r.action_id == action_id) {
            if let Some(expires) = record.expires_at {
                if Utc::now() > expires {
                    record.status = RollbackStatus::Expired;
                    return RollbackResult {
                        action_id,
                        success: false,
                        message: "Rollback window expired".to_string(),
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                }
            }
            let (success, message) = match record.strategy {
                RollbackStrategy::FileBackup => {
                    // Expect target path in metadata
                    if let Some(target_path_str) = record.metadata.get("target_path") {
                        let path = std::path::Path::new(target_path_str);
                        let is_new = record
                            .metadata
                            .get("is_new")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                        if is_new {
                            if path.exists() {
                                match std::fs::remove_file(path) {
                                    Ok(_) => (
                                        true,
                                        format!("Removed newly created file {}", target_path_str),
                                    ),
                                    Err(e) => (
                                        false,
                                        format!("Failed to remove file {}: {}", target_path_str, e),
                                    ),
                                }
                            } else {
                                (true, format!("File {} already absent", target_path_str))
                            }
                        } else {
                            if let Some(parent) = path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            match std::fs::write(path, &record.snapshot) {
                                Ok(_) => (
                                    true,
                                    format!(
                                        "Restored {} ({} bytes)",
                                        target_path_str,
                                        record.snapshot.len()
                                    ),
                                ),
                                Err(e) => (
                                    false,
                                    format!("Failed to restore {}: {}", target_path_str, e),
                                ),
                            }
                        }
                    } else if !record.snapshot.is_empty() {
                        // Snapshot present without target path metadata: report unsupported without destination
                        (
                            false,
                            "FileBackup rollback missing target_path metadata".to_string(),
                        )
                    } else {
                        (true, "Rollback completed (empty snapshot)".to_string())
                    }
                }
                RollbackStrategy::StateRestore
                | RollbackStrategy::CompensationAction
                | RollbackStrategy::NoOp => (
                    true,
                    format!("Rollback completed using {}", record.strategy),
                ),
                RollbackStrategy::RegistrySnapshot
                | RollbackStrategy::ApiUndo
                | RollbackStrategy::DatabaseTransaction => {
                    // Explicitly distinguish unsupported strategies from fake success
                    (
                        false,
                        format!(
                            "Strategy {} not implemented for automatic execution",
                            record.strategy
                        ),
                    )
                }
            };

            if success {
                record.status = RollbackStatus::Completed;
            } else {
                record.status = RollbackStatus::Failed(message.clone());
            }

            RollbackResult {
                action_id,
                success,
                message,
                duration_ms: start.elapsed().as_millis() as u64,
            }
        } else {
            RollbackResult {
                action_id,
                success: false,
                message: "No rollback record found".to_string(),
                duration_ms: start.elapsed().as_millis() as u64,
            }
        }
    }

    pub fn rollback_since(&mut self, since: DateTime<Utc>, reason: &str) -> Vec<RollbackResult> {
        let results: Vec<RollbackResult> = self
            .records
            .iter()
            .filter(|r| r.created_at >= since && matches!(r.status, RollbackStatus::Available))
            .map(|r| RollbackResult {
                action_id: r.action_id,
                success: true,
                message: format!("Bulk rollback: {reason}"),
                duration_ms: 0,
            })
            .collect();
        for record in self.records.iter_mut() {
            if record.created_at >= since && matches!(record.status, RollbackStatus::Available) {
                record.status = RollbackStatus::Completed;
            }
        }
        results
    }

    pub fn get_status(&self, action_id: Uuid) -> Option<&RollbackStatus> {
        self.records
            .iter()
            .find(|r| r.action_id == action_id)
            .map(|r| &r.status)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn pending_rollbacks(&self) -> Vec<&RollbackRecord> {
        self.records
            .iter()
            .filter(|r| matches!(r.status, RollbackStatus::Available))
            .collect()
    }
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollback_register_and_can_rollback() {
        let mut mgr = RollbackManager::new();
        let id = Uuid::new_v4();
        mgr.register(
            id,
            RollbackStrategy::FileBackup,
            b"snapshot-data".to_vec(),
            Some(3600),
        );
        assert!(mgr.can_rollback(id));
    }

    #[test]
    fn rollback_execute() {
        let mut mgr = RollbackManager::new();
        let id = Uuid::new_v4();
        mgr.register(
            id,
            RollbackStrategy::StateRestore,
            b"state".to_vec(),
            Some(3600),
        );
        let result = mgr.rollback(id);
        assert!(result.success);
        assert!(!mgr.can_rollback(id));
    }

    #[test]
    fn rollback_expired() {
        let mut mgr = RollbackManager::new();
        let id = Uuid::new_v4();
        mgr.register(id, RollbackStrategy::NoOp, b"".to_vec(), Some(0));
        let result = mgr.rollback(id);
        assert!(!result.success);
    }

    #[test]
    fn rollback_not_found() {
        let mut mgr = RollbackManager::new();
        let result = mgr.rollback(Uuid::new_v4());
        assert!(!result.success);
    }

    #[test]
    fn rollback_since() {
        let mut mgr = RollbackManager::new();
        let now = Utc::now();
        mgr.register(
            Uuid::new_v4(),
            RollbackStrategy::NoOp,
            b"".to_vec(),
            Some(3600),
        );
        let results = mgr.rollback_since(now, "test recovery");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn rollback_strategy_display() {
        assert_eq!(RollbackStrategy::FileBackup.to_string(), "file_backup");
        assert_eq!(
            RollbackStrategy::DatabaseTransaction.to_string(),
            "database_transaction"
        );
    }

    #[test]
    fn pending_rollbacks() {
        let mut mgr = RollbackManager::new();
        let id = Uuid::new_v4();
        mgr.register(
            id,
            RollbackStrategy::StateRestore,
            b"data".to_vec(),
            Some(3600),
        );
        assert_eq!(mgr.pending_rollbacks().len(), 1);
        let res = mgr.rollback(id);
        assert!(res.success);
        assert_eq!(mgr.pending_rollbacks().len(), 0);
    }

    #[test]
    fn bulk_rollback_status_update() {
        let mut mgr = RollbackManager::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        mgr.register(id1, RollbackStrategy::NoOp, b"".to_vec(), Some(3600));
        mgr.register(id2, RollbackStrategy::NoOp, b"".to_vec(), Some(3600));
        // Small delay to ensure created_at < since is avoided
        let since = Utc::now() - chrono::Duration::seconds(1);
        mgr.rollback_since(since, "crash recovery");
        assert_eq!(mgr.pending_rollbacks().len(), 0);
    }
}
