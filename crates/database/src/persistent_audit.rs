use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
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
    pub audit_level: String,
    pub metadata: Option<String>,
}

#[async_trait]
pub trait AuditLogStore: Send + Sync {
    async fn record_entry(&self, entry: &AuditLogEntry) -> Result<(), String>;
    async fn get_entries(&self, limit: usize, offset: usize) -> Result<Vec<AuditLogEntry>, String>;
    async fn get_entries_by_subject(
        &self,
        subject: &str,
        limit: usize,
    ) -> Result<Vec<AuditLogEntry>, String>;
    async fn get_entries_by_action(
        &self,
        action: &str,
        limit: usize,
    ) -> Result<Vec<AuditLogEntry>, String>;
    async fn get_entries_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<AuditLogEntry>, String>;
    async fn verify_chain(&self) -> Result<bool, String>;
    async fn count(&self) -> Result<usize, String>;
    async fn clear(&self) -> Result<(), String>;
}

pub struct InMemoryAuditLogStore {
    entries: parking_lot::RwLock<Vec<AuditLogEntry>>,
}

impl InMemoryAuditLogStore {
    pub fn new() -> Self {
        Self {
            entries: parking_lot::RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryAuditLogStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AuditLogStore for InMemoryAuditLogStore {
    async fn record_entry(&self, entry: &AuditLogEntry) -> Result<(), String> {
        self.entries.write().push(entry.clone());
        Ok(())
    }

    async fn get_entries(&self, limit: usize, offset: usize) -> Result<Vec<AuditLogEntry>, String> {
        let entries = self.entries.read();
        let mut sorted: Vec<_> = entries.iter().cloned().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.timestamp));
        Ok(sorted.into_iter().skip(offset).take(limit).collect())
    }

    async fn get_entries_by_subject(
        &self,
        subject: &str,
        limit: usize,
    ) -> Result<Vec<AuditLogEntry>, String> {
        Ok(self
            .entries
            .read()
            .iter()
            .filter(|e| e.subject == subject)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn get_entries_by_action(
        &self,
        action: &str,
        limit: usize,
    ) -> Result<Vec<AuditLogEntry>, String> {
        Ok(self
            .entries
            .read()
            .iter()
            .filter(|e| e.action == action)
            .take(limit)
            .cloned()
            .collect())
    }

    async fn get_entries_in_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<AuditLogEntry>, String> {
        Ok(self
            .entries
            .read()
            .iter()
            .filter(|e| e.timestamp >= from && e.timestamp <= to)
            .cloned()
            .collect())
    }

    async fn verify_chain(&self) -> Result<bool, String> {
        let entries = self.entries.read();
        let mut prev_hash = String::new();
        for entry in entries.iter() {
            if entry.previous_hash != prev_hash {
                return Ok(false);
            }
            prev_hash = entry.hash.clone();
        }
        Ok(true)
    }

    async fn count(&self) -> Result<usize, String> {
        Ok(self.entries.read().len())
    }

    async fn clear(&self) -> Result<(), String> {
        self.entries.write().clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(subject: &str, action: &str, previous_hash: &str) -> AuditLogEntry {
        AuditLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            subject: subject.to_string(),
            action: action.to_string(),
            resource: None,
            result: "allowed".to_string(),
            reason: None,
            risk_level: "low".to_string(),
            trust_level: "trusted".to_string(),
            previous_hash: previous_hash.to_string(),
            hash: "test-hash".to_string(),
            audit_level: "basic".to_string(),
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_record_and_retrieve() {
        let store = InMemoryAuditLogStore::new();
        let entry = make_entry("user-1", "file:read", "");
        store.record_entry(&entry).await.unwrap();
        let entries = store.get_entries(10, 0).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].subject, "user-1");
    }

    #[tokio::test]
    async fn test_verify_chain() {
        let store = InMemoryAuditLogStore::new();
        store.record_entry(&make_entry("u", "a", "")).await.unwrap();
        store
            .record_entry(&make_entry("u", "b", "test-hash"))
            .await
            .unwrap();
        assert!(store.verify_chain().await.unwrap());
    }

    #[tokio::test]
    async fn test_count() {
        let store = InMemoryAuditLogStore::new();
        store.record_entry(&make_entry("u", "a", "")).await.unwrap();
        store.record_entry(&make_entry("u", "b", "")).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_clear() {
        let store = InMemoryAuditLogStore::new();
        store.record_entry(&make_entry("u", "a", "")).await.unwrap();
        store.clear().await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }
}
