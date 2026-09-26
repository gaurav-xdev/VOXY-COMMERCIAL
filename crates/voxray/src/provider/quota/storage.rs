use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

#[derive(Debug, Clone, Default)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub request_count: u32,
    pub characters: usize,
    pub audio_seconds: f64,
}

#[derive(Debug, Clone, Default)]
pub struct ProviderState {
    pub history: Vec<UsageRecord>,
    pub active_concurrency: u32,
    pub cooldown_until: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub latencies_ms: Vec<u64>,
    pub count_429: u32,
    pub count_timeout: u32,
    pub count_5xx: u32,
    pub invalid_credentials: bool,
}

#[async_trait]
pub trait QuotaStorage: Send + Sync {
    async fn record_usage(
        &self,
        provider: &str,
        chars: usize,
        audio_secs: f64,
    );
    async fn increment_concurrency(&self, provider: &str) -> u32;
    async fn decrement_concurrency(&self, provider: &str);
    async fn set_cooldown(&self, provider: &str, until: DateTime<Utc>);
    async fn record_error(&self, provider: &str, is_429: bool, is_timeout: bool, is_auth: bool);
    async fn record_success(&self, provider: &str, latency_ms: u64);
    async fn get_state(&self, provider: &str) -> ProviderState;
    async fn clean_old_records(&self, provider: &str, older_than: DateTime<Utc>);
}

/// In-memory thread-safe quota storage.
#[derive(Clone, Default)]
pub struct InMemoryQuotaStorage {
    states: Arc<RwLock<HashMap<String, ProviderState>>>,
}

impl InMemoryQuotaStorage {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl QuotaStorage for InMemoryQuotaStorage {
    async fn record_usage(
        &self,
        provider: &str,
        chars: usize,
        audio_secs: f64,
    ) {
        let mut map = self.states.write();
        let state = map.entry(provider.to_string()).or_default();
        state.history.push(UsageRecord {
            timestamp: Utc::now(),
            request_count: 1,
            characters: chars,
            audio_seconds: audio_secs,
        });
    }

    async fn increment_concurrency(&self, provider: &str) -> u32 {
        let mut map = self.states.write();
        let state = map.entry(provider.to_string()).or_default();
        state.active_concurrency = state.active_concurrency.saturating_add(1);
        state.active_concurrency
    }

    async fn decrement_concurrency(&self, provider: &str) {
        let mut map = self.states.write();
        let state = map.entry(provider.to_string()).or_default();
        state.active_concurrency = state.active_concurrency.saturating_sub(1);
    }

    async fn set_cooldown(&self, provider: &str, until: DateTime<Utc>) {
        let mut map = self.states.write();
        let state = map.entry(provider.to_string()).or_default();
        state.cooldown_until = Some(until);
    }

    async fn record_error(&self, provider: &str, is_429: bool, is_timeout: bool, is_auth: bool) {
        let mut map = self.states.write();
        let state = map.entry(provider.to_string()).or_default();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if is_429 {
            state.count_429 = state.count_429.saturating_add(1);
        }
        if is_timeout {
            state.count_timeout = state.count_timeout.saturating_add(1);
        }
        if is_auth {
            state.invalid_credentials = true;
        }
    }

    async fn record_success(&self, provider: &str, latency_ms: u64) {
        let mut map = self.states.write();
        let state = map.entry(provider.to_string()).or_default();
        state.consecutive_failures = 0;
        state.latencies_ms.push(latency_ms);
        if state.latencies_ms.len() > 100 {
            state.latencies_ms.drain(0..state.latencies_ms.len() - 100);
        }
    }

    async fn get_state(&self, provider: &str) -> ProviderState {
        let map = self.states.read();
        map.get(provider).cloned().unwrap_or_default()
    }

    async fn clean_old_records(&self, provider: &str, older_than: DateTime<Utc>) {
        let mut map = self.states.write();
        if let Some(state) = map.get_mut(provider) {
            state.history.retain(|r| r.timestamp > older_than);
        }
    }
}

/// Distributed Redis-ready storage abstraction stub.
pub struct RedisQuotaStorage {
    #[allow(dead_code)]
    redis_url: String,
    fallback: InMemoryQuotaStorage,
}

impl RedisQuotaStorage {
    pub fn new(redis_url: impl Into<String>) -> Self {
        Self {
            redis_url: redis_url.into(),
            fallback: InMemoryQuotaStorage::new(),
        }
    }
}

#[async_trait]
impl QuotaStorage for RedisQuotaStorage {
    async fn record_usage(&self, provider: &str, chars: usize, audio_secs: f64) {
        // Atomic fallback to local in-memory
        self.fallback.record_usage(provider, chars, audio_secs).await;
    }

    async fn increment_concurrency(&self, provider: &str) -> u32 {
        self.fallback.increment_concurrency(provider).await
    }

    async fn decrement_concurrency(&self, provider: &str) {
        self.fallback.decrement_concurrency(provider).await;
    }

    async fn set_cooldown(&self, provider: &str, until: DateTime<Utc>) {
        self.fallback.set_cooldown(provider, until).await;
    }

    async fn record_error(&self, provider: &str, is_429: bool, is_timeout: bool, is_auth: bool) {
        self.fallback.record_error(provider, is_429, is_timeout, is_auth).await;
    }

    async fn record_success(&self, provider: &str, latency_ms: u64) {
        self.fallback.record_success(provider, latency_ms).await;
    }

    async fn get_state(&self, provider: &str) -> ProviderState {
        self.fallback.get_state(provider).await
    }

    async fn clean_old_records(&self, provider: &str, older_than: DateTime<Utc>) {
        self.fallback.clean_old_records(provider, older_than).await;
    }
}
