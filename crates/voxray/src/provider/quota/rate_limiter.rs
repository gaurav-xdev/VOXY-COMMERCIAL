use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub rpm: u32,
    pub rpd: u32,
    pub characters_per_minute: usize,
    pub characters_per_day: usize,
    pub audio_seconds_per_day: f64,
    pub max_concurrency: u32,
    /// Conservative safety margin below documented provider limit.
    /// Default is 0.35 (35% safe margin, strictly preserving 30-40% quota buffer).
    pub safety_margin: f64,
    /// Maximum burst allowance in requests before throttling.
    pub burst_capacity: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            rpm: 60,
            rpd: 2000,
            characters_per_minute: 40_000,
            characters_per_day: 500_000,
            audio_seconds_per_day: 7200.0, // 2 hours
            max_concurrency: 3,
            safety_margin: 0.35, // 35% default safety margin
            burst_capacity: 5,
        }
    }
}

impl RateLimitConfig {
    pub fn safe_rpm(&self) -> u32 {
        let margin = (self.rpm as f64 * (1.0 - self.safety_margin)).floor() as u32;
        margin.max(1)
    }

    pub fn safe_rpd(&self) -> u32 {
        let margin = (self.rpd as f64 * (1.0 - self.safety_margin)).floor() as u32;
        margin.max(1)
    }

    pub fn safe_concurrency(&self) -> u32 {
        let margin = (self.max_concurrency as f64 * (1.0 - self.safety_margin)).floor() as u32;
        margin.max(1)
    }

    pub fn safe_cpm(&self) -> usize {
        let margin = (self.characters_per_minute as f64 * (1.0 - self.safety_margin)).floor() as usize;
        margin.max(100)
    }

    pub fn safe_cpd(&self) -> usize {
        let margin = (self.characters_per_day as f64 * (1.0 - self.safety_margin)).floor() as usize;
        margin.max(1000)
    }

    /// Load provider rate limit config from environment variables with fallback to documented defaults.
    pub fn from_env_with_defaults(
        provider_prefix: &str,
        documented_rpm: u32,
        documented_rpd: u32,
        documented_concurrency: u32,
        documented_cpm: usize,
        documented_cpd: usize,
        default_safety_margin: f64,
    ) -> Self {
        let prefix = provider_prefix.to_uppercase();

        let safety_margin = std::env::var(format!("{}_SAFETY_MARGIN", prefix))
            .or_else(|_| std::env::var("VOXY_VOICE_SAFETY_MARGIN"))
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(default_safety_margin)
            .clamp(0.05, 0.90);

        let rpm = std::env::var(format!("{}_RPM_LIMIT", prefix))
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(documented_rpm);

        let rpd = std::env::var(format!("{}_RPD_LIMIT", prefix))
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(documented_rpd);

        let max_concurrency = std::env::var(format!("{}_CONCURRENCY_LIMIT", prefix))
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(documented_concurrency);

        let characters_per_minute = std::env::var(format!("{}_CPM_LIMIT", prefix))
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(documented_cpm);

        let characters_per_day = std::env::var(format!("{}_CPD_LIMIT", prefix))
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(documented_cpd);

        let burst_capacity = std::env::var(format!("{}_BURST_CAPACITY", prefix))
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or_else(|| ((rpm as f64 * (1.0 - safety_margin)) / 6.0).ceil() as u32)
            .max(2);

        Self {
            rpm,
            rpd,
            characters_per_minute,
            characters_per_day,
            audio_seconds_per_day: 7200.0,
            max_concurrency,
            safety_margin,
            burst_capacity,
        }
    }
}

/// Token bucket state tracking for smooth rate replenishment and burst protection.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    pub capacity: f64,
    pub tokens: f64,
    pub fill_rate_per_sec: f64,
    pub last_update: DateTime<Utc>,
}

impl TokenBucket {
    pub fn new(capacity: f64, fill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            fill_rate_per_sec: fill_rate_per_sec.max(0.001),
            last_update: Utc::now(),
        }
    }

    pub fn try_consume(&mut self, tokens: f64, now: DateTime<Utc>) -> bool {
        self.replenish(now);

        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn replenish(&mut self, now: DateTime<Utc>) {
        let elapsed_secs = (now - self.last_update).num_milliseconds().max(0) as f64 / 1000.0;
        self.tokens = (self.tokens + elapsed_secs * self.fill_rate_per_sec).min(self.capacity);
        self.last_update = now;
    }

    /// Calculate dynamic wait time until at least `needed_tokens` are available in bucket.
    pub fn time_until_available(&mut self, needed_tokens: f64, now: DateTime<Utc>) -> Duration {
        self.replenish(now);
        if self.tokens >= needed_tokens {
            Duration::ZERO
        } else {
            let deficit = needed_tokens - self.tokens;
            let secs = deficit / self.fill_rate_per_sec;
            Duration::from_secs_f64(secs.max(0.001))
        }
    }
}
