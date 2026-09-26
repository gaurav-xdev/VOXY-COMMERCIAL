use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RoutingMode {
    /// Maximum quota protection with 40% safety margin and strict pacing.
    Conservative,
    /// Balanced production mode with 35% safety margin and dynamic pacing (default).
    #[default]
    Balanced,
    /// Lower 15% safety margin while strictly respecting documented provider limits.
    Performance,
}

impl RoutingMode {
    pub fn from_env(var_name: &str) -> Self {
        match std::env::var(var_name)
            .unwrap_or_else(|_| "balanced".into())
            .to_lowercase()
            .as_str()
        {
            "conservative" => Self::Conservative,
            "performance" | "aggressive" => Self::Performance,
            _ => Self::Balanced,
        }
    }

    /// Default safety margin (fraction below documented limit) for this routing mode.
    /// - Conservative: 40% safety margin (use max 60% of documented limit)
    /// - Balanced: 35% safety margin (use max 65% of documented limit, within 30-40% range)
    /// - Performance: 15% safety margin (use max 85% of documented limit)
    pub fn default_safety_margin(&self) -> f64 {
        match self {
            Self::Conservative => 0.40,
            Self::Balanced => 0.35,
            Self::Performance => 0.15,
        }
    }

    /// Calculate target allowable requests at this point in the day.
    /// Prevents burning the whole daily quota in the first hour.
    pub fn calculate_target_allowance(&self, daily_limit: u32, now: DateTime<Utc>) -> u32 {
        if daily_limit == 0 {
            return 0;
        }

        // Active daytime hours assumed 16 hours (e.g. 8am to midnight)
        let hour = now.hour() as f64;
        let minute = now.minute() as f64;
        let elapsed_fraction = ((hour * 60.0 + minute) / 1440.0).clamp(0.05, 1.0);

        match self {
            Self::Conservative => {
                // Allows linearly up to elapsed fraction with 10% buffer
                let base = (daily_limit as f64 * elapsed_fraction * 1.10).ceil() as u32;
                base.min(daily_limit)
            }
            Self::Balanced => {
                // Allows up to 1.5x of linear progression, capped at daily limit
                let base = (daily_limit as f64 * (elapsed_fraction * 1.5).min(1.0)).ceil() as u32;
                base.min(daily_limit)
            }
            Self::Performance => {
                // Unlimited pacing up to the hard daily cap
                daily_limit
            }
        }
    }
}
