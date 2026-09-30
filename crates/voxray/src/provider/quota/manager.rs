use chrono::{DateTime, Duration as ChronoDuration, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::budget::RoutingMode;
use super::rate_limiter::{RateLimitConfig, TokenBucket};
use super::storage::{InMemoryQuotaStorage, QuotaStorage};
use crate::provider::traits::{HealthStatus, QuotaConfidence, QuotaStatus};

#[derive(Debug, thiserror::Error, Clone)]
pub enum QuotaViolation {
    #[error("RPM limit exceeded for provider '{0}', available in {1:?}")]
    RpmExceeded(String, Duration),
    #[error("Daily request limit exceeded for provider '{0}'")]
    RpdExceeded(String),
    #[error("Daily budget pacing exceeded for provider '{0}' in mode {1:?}")]
    BudgetExceeded(String, RoutingMode),
    #[error("Character limit exceeded for provider '{0}'")]
    CharacterLimitExceeded(String),
    #[error("Concurrency limit exceeded for provider '{0}' (active requests at capacity)")]
    ConcurrencyExceeded(String),
    #[error("Sudden burst limit exceeded for provider '{0}', available in {1:?}")]
    BurstLimitExceeded(String, Duration),
    #[error("Provider '{0}' is in cooldown until {1}")]
    InCooldown(String, DateTime<Utc>),
    #[error("Provider '{0}' has invalid credentials")]
    InvalidCredentials(String),
}

/// RAII Guard that automatically decrements active concurrency counter when dropped.
pub struct ConcurrencyPermit {
    provider: String,
    manager: Arc<ProviderQuotaManager>,
}

impl std::fmt::Debug for ConcurrencyPermit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConcurrencyPermit")
            .field("provider", &self.provider)
            .finish()
    }
}

impl Drop for ConcurrencyPermit {
    fn drop(&mut self) {
        self.manager.release_concurrency(&self.provider);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetrics {
    pub provider_name: String,
    pub health: HealthStatus,
    pub current_rpm: u32,
    pub limit_rpm: u32,
    pub current_rph: u32,
    pub limit_rph: u32,
    pub current_rpd: u32,
    pub limit_rpd: u32,
    pub active_concurrency: u32,
    pub limit_concurrency: u32,
    pub total_requests: u64,
    pub total_failures: u64,
    pub total_rate_limits: u64,
    pub total_timeouts: u64,
    pub total_cost_usd: f64,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub recent_failure_rate: f64,
    pub is_cooldown: bool,
}

struct ProviderInternalStats {
    active_concurrency: u32,
    consecutive_failures: u32,
    total_failures: u64,
    total_rate_limits: u64,
    total_timeouts: u64,
    total_requests: u64,
    total_cost_usd: f64,
    chars_consumed_today: usize,
    latencies_ms: Vec<u64>,
    cooldown_until: Option<DateTime<Utc>>,
    invalid_credentials: bool,
    history_timestamps: Vec<DateTime<Utc>>,
    recent_errors: Vec<DateTime<Utc>>,
    token_bucket: TokenBucket,
}

impl ProviderInternalStats {
    fn new(config: &RateLimitConfig) -> Self {
        let capacity = config.burst_capacity as f64;
        let fill_rate = (config.safe_rpm() as f64 / 60.0).max(0.01);
        Self {
            active_concurrency: 0,
            consecutive_failures: 0,
            total_failures: 0,
            total_rate_limits: 0,
            total_timeouts: 0,
            total_requests: 0,
            total_cost_usd: 0.0,
            chars_consumed_today: 0,
            latencies_ms: Vec::new(),
            cooldown_until: None,
            invalid_credentials: false,
            history_timestamps: Vec::new(),
            recent_errors: Vec::new(),
            token_bucket: TokenBucket::new(capacity, fill_rate),
        }
    }
}

pub struct ProviderQuotaManager {
    _storage: Arc<dyn QuotaStorage>,
    configs: RwLock<HashMap<String, RateLimitConfig>>,
    stats: RwLock<HashMap<String, ProviderInternalStats>>,
    routing_mode: RwLock<RoutingMode>,
}

impl ProviderQuotaManager {
    pub fn new(storage: Arc<dyn QuotaStorage>, routing_mode: RoutingMode) -> Self {
        Self {
            _storage: storage,
            configs: RwLock::new(HashMap::new()),
            stats: RwLock::new(HashMap::new()),
            routing_mode: RwLock::new(routing_mode),
        }
    }

    pub fn with_default_in_memory() -> Self {
        Self::new(Arc::new(InMemoryQuotaStorage::new()), RoutingMode::Balanced)
    }

    pub fn routing_mode(&self) -> RoutingMode {
        *self.routing_mode.read()
    }

    pub fn set_routing_mode(&self, mode: RoutingMode) {
        *self.routing_mode.write() = mode;
    }

    pub fn register_provider(&self, provider: &str, config: RateLimitConfig) {
        let mut stats_lock = self.stats.write();
        let stats = ProviderInternalStats::new(&config);
        stats_lock.insert(provider.to_string(), stats);
        self.configs.write().insert(provider.to_string(), config);
    }

    pub fn get_config(&self, provider: &str) -> RateLimitConfig {
        self.configs
            .read()
            .get(provider)
            .cloned()
            .unwrap_or_default()
    }

    pub fn is_in_cooldown(&self, provider: &str) -> bool {
        let stats = self.stats.read();
        if let Some(s) = stats.get(provider) {
            if let Some(until) = s.cooldown_until {
                return until > Utc::now();
            }
        }
        false
    }

    /// Calculate dynamic wait time until this provider can accept a new request.
    /// Returns Duration::ZERO if provider is immediately available.
    pub fn calculate_wait_time(&self, provider: &str, _chars: u32) -> Duration {
        let now = Utc::now();
        let mut stats_lock = self.stats.write();
        if let Some(stats) = stats_lock.get_mut(provider) {
            if stats.invalid_credentials {
                return Duration::from_secs(3600);
            }
            if let Some(cooldown) = stats.cooldown_until {
                if cooldown > now {
                    let diff = (cooldown - now).num_milliseconds().max(0) as u64;
                    return Duration::from_millis(diff);
                }
            }

            let config = self.get_config(provider);

            // Check concurrency wait
            if stats.active_concurrency >= config.safe_concurrency() {
                return Duration::from_millis(100);
            }

            // Check token bucket burst availability
            let bucket_wait = stats.token_bucket.time_until_available(1.0, now);
            if bucket_wait > Duration::ZERO {
                return bucket_wait;
            }

            // Check rolling 60s RPM window
            let one_min_ago = now - ChronoDuration::minutes(1);
            let recent_reqs: Vec<&DateTime<Utc>> = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= one_min_ago)
                .collect();

            if recent_reqs.len() as u32 >= config.safe_rpm() {
                if let Some(&&oldest) = recent_reqs.first() {
                    let expiry = oldest + ChronoDuration::minutes(1);
                    if expiry > now {
                        let ms = (expiry - now).num_milliseconds().max(10) as u64;
                        return Duration::from_millis(ms);
                    }
                }
            }
        }
        Duration::ZERO
    }

    /// Preflight check for rate limits, token bucket burst availability, daily quota, and credentials.
    pub fn check_preflight(
        &self,
        provider: &str,
        chars: u32,
        _audio_secs: f64,
    ) -> Result<(), QuotaViolation> {
        let now = Utc::now();
        let mut stats_lock = self.stats.write();
        if let Some(stats) = stats_lock.get_mut(provider) {
            if stats.invalid_credentials {
                return Err(QuotaViolation::InvalidCredentials(provider.to_string()));
            }

            if let Some(cooldown) = stats.cooldown_until {
                if cooldown > now {
                    return Err(QuotaViolation::InCooldown(provider.to_string(), cooldown));
                }
            }

            let config = self.get_config(provider);

            // 1. Concurrency limit check
            if stats.active_concurrency >= config.safe_concurrency() {
                return Err(QuotaViolation::ConcurrencyExceeded(provider.to_string()));
            }

            // 2. Token Bucket Burst check
            if !stats.token_bucket.try_consume(1.0, now) {
                let wait = stats.token_bucket.time_until_available(1.0, now);
                return Err(QuotaViolation::BurstLimitExceeded(
                    provider.to_string(),
                    wait,
                ));
            }

            // 3. Sliding 60-second RPM window check
            let one_min_ago = now - ChronoDuration::minutes(1);
            let reqs_min = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= one_min_ago)
                .count() as u32;

            if reqs_min >= config.safe_rpm() {
                let wait = self.calculate_wait_time_internal(stats, &config, now);
                return Err(QuotaViolation::RpmExceeded(provider.to_string(), wait));
            }

            // 4. Daily RPD limit check
            let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let reqs_today = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= start_of_day)
                .count() as u32;

            if reqs_today >= config.safe_rpd() {
                return Err(QuotaViolation::RpdExceeded(provider.to_string()));
            }

            // 5. Daily budget pacing mode check
            let mode = *self.routing_mode.read();
            let target_allowance = mode.calculate_target_allowance(config.safe_rpd(), now);
            if reqs_today >= target_allowance {
                return Err(QuotaViolation::BudgetExceeded(provider.to_string(), mode));
            }

            // 6. Character limit check
            if chars > 0 && stats.chars_consumed_today + (chars as usize) > config.safe_cpd() {
                return Err(QuotaViolation::CharacterLimitExceeded(provider.to_string()));
            }
        }

        Ok(())
    }

    fn calculate_wait_time_internal(
        &self,
        stats: &ProviderInternalStats,
        _config: &RateLimitConfig,
        now: DateTime<Utc>,
    ) -> Duration {
        let one_min_ago = now - ChronoDuration::minutes(1);
        let oldest = stats.history_timestamps.iter().find(|&&t| t >= one_min_ago);
        if let Some(&t) = oldest {
            let release = t + ChronoDuration::minutes(1);
            if release > now {
                let ms = (release - now).num_milliseconds().max(10) as u64;
                return Duration::from_millis(ms);
            }
        }
        Duration::from_millis(250)
    }

    /// Acquire in-flight concurrency permit for a request.
    pub fn acquire_concurrency(
        self: &Arc<Self>,
        provider: &str,
    ) -> Result<ConcurrencyPermit, QuotaViolation> {
        let mut stats_lock = self.stats.write();
        let config = self.get_config(provider);
        let stats = stats_lock
            .entry(provider.to_string())
            .or_insert_with(|| ProviderInternalStats::new(&config));

        if stats.active_concurrency >= config.safe_concurrency() {
            return Err(QuotaViolation::ConcurrencyExceeded(provider.to_string()));
        }

        stats.active_concurrency += 1;
        Ok(ConcurrencyPermit {
            provider: provider.to_string(),
            manager: Arc::clone(self),
        })
    }

    pub fn release_concurrency(&self, provider: &str) {
        let mut stats_lock = self.stats.write();
        if let Some(stats) = stats_lock.get_mut(provider) {
            stats.active_concurrency = stats.active_concurrency.saturating_sub(1);
        }
    }

    pub fn record_success(
        &self,
        provider: &str,
        elapsed: Duration,
        chars: u32,
        _audio_dur: f64,
        cost_usd: f64,
    ) {
        let now = Utc::now();
        let mut stats_lock = self.stats.write();
        let config = self.get_config(provider);
        let stats = stats_lock
            .entry(provider.to_string())
            .or_insert_with(|| ProviderInternalStats::new(&config));

        stats.total_requests += 1;
        stats.consecutive_failures = 0;
        stats.total_cost_usd += cost_usd;
        stats.chars_consumed_today += chars as usize;
        stats.history_timestamps.push(now);

        let ms = elapsed.as_millis() as u64;
        stats.latencies_ms.push(ms);
        if stats.latencies_ms.len() > 100 {
            stats.latencies_ms.remove(0);
        }

        let one_day_ago = now - ChronoDuration::days(1);
        stats.history_timestamps.retain(|&t| t >= one_day_ago);

        let five_mins_ago = now - ChronoDuration::minutes(5);
        stats.recent_errors.retain(|&t| t >= five_mins_ago);
    }

    pub fn record_failure(&self, provider: &str) {
        let now = Utc::now();
        let mut stats_lock = self.stats.write();
        let config = self.get_config(provider);
        let stats = stats_lock
            .entry(provider.to_string())
            .or_insert_with(|| ProviderInternalStats::new(&config));
        stats.total_failures += 1;
        stats.consecutive_failures += 1;
        stats.recent_errors.push(now);

        let five_mins_ago = now - ChronoDuration::minutes(5);
        stats.recent_errors.retain(|&t| t >= five_mins_ago);

        if stats.consecutive_failures >= 3 {
            // Exponential backoff with consecutive failures
            let backoff_secs = (15 * (1 << (stats.consecutive_failures - 3).min(4))).min(300);
            let until = now + ChronoDuration::seconds(backoff_secs as i64);
            stats.cooldown_until = Some(until);
        }
    }

    pub fn record_timeout(&self, provider: &str) {
        let now = Utc::now();
        let mut stats_lock = self.stats.write();
        let config = self.get_config(provider);
        let stats = stats_lock
            .entry(provider.to_string())
            .or_insert_with(|| ProviderInternalStats::new(&config));
        stats.total_timeouts += 1;
        stats.total_failures += 1;
        stats.consecutive_failures += 1;
        stats.recent_errors.push(now);

        // Put in short cooldown on timeout
        let until = now + ChronoDuration::seconds(15);
        stats.cooldown_until = Some(until);
    }

    pub fn record_rate_limit(&self, provider: &str, retry_after: Option<Duration>) {
        let now = Utc::now();
        let mut stats_lock = self.stats.write();
        let config = self.get_config(provider);
        let stats = stats_lock
            .entry(provider.to_string())
            .or_insert_with(|| ProviderInternalStats::new(&config));
        stats.total_rate_limits += 1;
        stats.consecutive_failures += 1;
        stats.recent_errors.push(now);

        let ms = retry_after.map(|d| d.as_millis() as i64).unwrap_or(30_000);
        let until = now + ChronoDuration::milliseconds(ms.max(10));
        stats.cooldown_until = Some(until);
    }

    pub fn record_invalid_credentials(&self, provider: &str) {
        let mut stats_lock = self.stats.write();
        let config = self.get_config(provider);
        let stats = stats_lock
            .entry(provider.to_string())
            .or_insert_with(|| ProviderInternalStats::new(&config));
        stats.invalid_credentials = true;
    }

    pub fn recent_failure_rate(&self, provider: &str) -> f64 {
        let now = Utc::now();
        let five_mins_ago = now - ChronoDuration::minutes(5);
        let stats_lock = self.stats.read();
        if let Some(stats) = stats_lock.get(provider) {
            let recent_reqs = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= five_mins_ago)
                .count();
            let recent_errs = stats
                .recent_errors
                .iter()
                .filter(|&&t| t >= five_mins_ago)
                .count();

            if recent_reqs + recent_errs == 0 {
                0.0
            } else {
                recent_errs as f64 / (recent_reqs + recent_errs) as f64
            }
        } else {
            0.0
        }
    }

    pub fn reset_cooldown(&self, provider: &str) {
        let mut stats_lock = self.stats.write();
        if let Some(stats) = stats_lock.get_mut(provider) {
            stats.cooldown_until = None;
            stats.consecutive_failures = 0;
        }
    }

    pub fn reset_all_cooldowns(&self) {
        let mut stats_lock = self.stats.write();
        for stats in stats_lock.values_mut() {
            stats.cooldown_until = None;
            stats.consecutive_failures = 0;
        }
    }

    pub fn get_status(&self, provider: &str) -> QuotaStatus {
        let now = Utc::now();
        let stats_lock = self.stats.read();
        let config = self.get_config(provider);

        if let Some(stats) = stats_lock.get(provider) {
            let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let reqs_today = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= start_of_day)
                .count() as u32;

            let remaining_today = config.safe_rpd().saturating_sub(reqs_today);
            let remaining_pct = if config.safe_rpd() > 0 {
                remaining_today as f64 / config.safe_rpd() as f64
            } else {
                1.0
            };

            let in_cooldown = stats
                .cooldown_until
                .map(|until| until > now)
                .unwrap_or(false);

            let health = if stats.invalid_credentials {
                HealthStatus::InvalidCredentials("Invalid API credentials".into())
            } else if in_cooldown {
                HealthStatus::Cooldown
            } else if stats.consecutive_failures >= 3 {
                HealthStatus::Degraded("Consecutive failures threshold exceeded".into())
            } else if remaining_pct <= 0.05 {
                HealthStatus::Exhausted
            } else {
                HealthStatus::Healthy
            };

            let mut sorted_lat = stats.latencies_ms.clone();
            sorted_lat.sort_unstable();
            let p50 = if !sorted_lat.is_empty() {
                sorted_lat[sorted_lat.len() * 50 / 100] as f64
            } else {
                120.0
            };

            QuotaStatus {
                confidence: QuotaConfidence::EstimatedRemaining,
                rpm_limit: config.rpm,
                rpd_limit: config.rpd,
                requests_today: reqs_today,
                estimated_remaining_today: Some(remaining_today),
                concurrency_limit: config.max_concurrency,
                active_requests: stats.active_concurrency,
                in_cooldown_until: stats.cooldown_until,
                health,
                is_cooldown: in_cooldown,
                latency_p50_ms: p50,
                remaining_pct,
            }
        } else {
            QuotaStatus::default()
        }
    }

    pub fn get_all_metrics(&self) -> HashMap<String, ProviderMetrics> {
        let now = Utc::now();
        let stats_lock = self.stats.read();
        let mut map = HashMap::new();

        for (id, stats) in stats_lock.iter() {
            let config = self.get_config(id);
            let one_min_ago = now - ChronoDuration::minutes(1);
            let one_hour_ago = now - ChronoDuration::hours(1);
            let start_of_day = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

            let reqs_min = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= one_min_ago)
                .count() as u32;
            let reqs_hour = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= one_hour_ago)
                .count() as u32;
            let reqs_today = stats
                .history_timestamps
                .iter()
                .filter(|&&t| t >= start_of_day)
                .count() as u32;

            let in_cooldown = stats
                .cooldown_until
                .map(|until| until > now)
                .unwrap_or(false);

            let health = if stats.invalid_credentials {
                HealthStatus::InvalidCredentials("Invalid API credentials".into())
            } else if in_cooldown {
                HealthStatus::Cooldown
            } else if stats.consecutive_failures >= 3 {
                HealthStatus::Degraded("Frequent failures".into())
            } else {
                HealthStatus::Healthy
            };

            let mut sorted_lat = stats.latencies_ms.clone();
            sorted_lat.sort_unstable();
            let (p50, p95) = if !sorted_lat.is_empty() {
                let p50_idx = sorted_lat.len() * 50 / 100;
                let p95_idx = (sorted_lat.len() * 95 / 100).min(sorted_lat.len() - 1);
                (sorted_lat[p50_idx] as f64, sorted_lat[p95_idx] as f64)
            } else {
                (120.0, 150.0)
            };

            let failure_rate = self.recent_failure_rate(id);

            map.insert(
                id.clone(),
                ProviderMetrics {
                    provider_name: id.clone(),
                    health,
                    current_rpm: reqs_min,
                    limit_rpm: config.rpm,
                    current_rph: reqs_hour,
                    limit_rph: config.rpm * 60,
                    current_rpd: reqs_today,
                    limit_rpd: config.rpd,
                    active_concurrency: stats.active_concurrency,
                    limit_concurrency: config.max_concurrency,
                    total_requests: stats.total_requests,
                    total_failures: stats.total_failures,
                    total_rate_limits: stats.total_rate_limits,
                    total_timeouts: stats.total_timeouts,
                    total_cost_usd: stats.total_cost_usd,
                    latency_p50_ms: p50,
                    latency_p95_ms: p95,
                    recent_failure_rate: failure_rate,
                    is_cooldown: in_cooldown,
                },
            );
        }

        map
    }

    pub fn get_metrics(&self, provider: &str) -> Option<ProviderMetrics> {
        self.get_all_metrics().remove(provider)
    }
}
