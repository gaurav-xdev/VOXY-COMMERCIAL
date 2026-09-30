use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{GuardError, Result};

/// Configuration for self-healing behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealingConfig {
    /// Maximum number of restart attempts before giving up.
    pub max_restart_attempts: u32,
    /// Base delay for exponential backoff (ms).
    pub base_backoff_ms: u64,
    /// Maximum backoff delay (ms).
    pub max_backoff_ms: u64,
    /// Cooldown after successful recovery before resetting counter (secs).
    pub cooldown_secs: u64,
}

impl Default for HealingConfig {
    fn default() -> Self {
        Self {
            max_restart_attempts: 5,
            base_backoff_ms: 1000,
            max_backoff_ms: 30000,
            cooldown_secs: 300,
        }
    }
}

/// State of a subsystem's recovery attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryState {
    pub subsystem: String,
    pub attempt_count: u32,
    pub last_attempt: DateTime<Utc>,
    pub last_error: Option<String>,
    pub last_success: Option<DateTime<Utc>>,
    pub current_backoff_ms: u64,
    pub is_recovering: bool,
}

type RestartFn = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = std::result::Result<(), String>> + Send + Sync>>
        + Send
        + Sync,
>;

pub struct SelfHealer {
    config: HealingConfig,
    states: RwLock<HashMap<String, RecoveryState>>,
    restart_fns: RwLock<HashMap<String, RestartFn>>,
}

impl SelfHealer {
    pub fn new(config: HealingConfig) -> Self {
        Self {
            config,
            states: RwLock::new(HashMap::new()),
            restart_fns: RwLock::new(HashMap::new()),
        }
    }

    /// Register a subsystem with its restart function.
    pub async fn register<F, Fut>(&self, name: &str, restart_fn: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = std::result::Result<(), String>> + Send + Sync + 'static,
    {
        let mut states = self.states.write().await;
        states.insert(
            name.to_string(),
            RecoveryState {
                subsystem: name.to_string(),
                attempt_count: 0,
                last_attempt: Utc::now(),
                last_error: None,
                last_success: None,
                current_backoff_ms: self.config.base_backoff_ms,
                is_recovering: false,
            },
        );

        let mut fns = self.restart_fns.write().await;
        fns.insert(
            name.to_string(),
            Arc::new(move || {
                let fut = restart_fn();
                Box::pin(fut)
            }),
        );

        info!("Self-healer registered: {}", name);
    }

    /// Trigger self-healing for a failed subsystem.
    pub async fn heal(&self, name: &str) -> Result<()> {
        let attempt = {
            let mut states = self.states.write().await;
            let state = states
                .get_mut(name)
                .ok_or_else(|| GuardError::SubsystemNotFound(name.to_string()))?;

            if state.is_recovering {
                return Ok(());
            }

            if state.attempt_count >= self.config.max_restart_attempts {
                return Err(GuardError::MaxRestartsExceeded(name.to_string()));
            }

            state.attempt_count += 1;
            state.last_attempt = Utc::now();
            state.is_recovering = true;
            state.attempt_count
        };

        info!(
            "Self-healing attempt {}/{} for {}",
            attempt, self.config.max_restart_attempts, name
        );

        // Get the restart function
        let restart_fn = {
            let fns = self.restart_fns.read().await;
            fns.get(name)
                .cloned()
                .ok_or_else(|| GuardError::SubsystemNotFound(name.to_string()))?
        };

        // Execute with exponential backoff
        let backoff_ms = self.calculate_backoff(attempt);
        if backoff_ms > 0 {
            info!(
                "Waiting {}ms before restart attempt for {}",
                backoff_ms, name
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
        }

        match restart_fn().await {
            Ok(()) => {
                let mut states = self.states.write().await;
                if let Some(state) = states.get_mut(name) {
                    state.is_recovering = false;
                    state.last_success = Some(Utc::now());
                    state.last_error = None;
                    state.current_backoff_ms = self.config.base_backoff_ms;
                    // A successful heal resets the attempt budget so a subsystem
                    // that recovered is not permanently refused future healing.
                    state.attempt_count = 0;
                }
                info!("Self-healing succeeded for {}", name);
                Ok(())
            }
            Err(e) => {
                let mut states = self.states.write().await;
                if let Some(state) = states.get_mut(name) {
                    state.is_recovering = false;
                    state.last_error = Some(e.clone());
                    state.current_backoff_ms =
                        (state.current_backoff_ms * 2).min(self.config.max_backoff_ms);
                }
                warn!("Self-healing failed for {}: {}", name, e);
                Err(GuardError::SelfHealingFailed {
                    subsystem: name.to_string(),
                    reason: e,
                })
            }
        }
    }

    /// Calculate exponential backoff delay.
    fn calculate_backoff(&self, attempt: u32) -> u64 {
        if attempt == 0 {
            return 0;
        }
        let base = self.config.base_backoff_ms as f64;
        let backoff = base * 2.0_f64.powi(attempt as i32 - 1);
        (backoff as u64).min(self.config.max_backoff_ms)
    }

    /// Get recovery state for a subsystem.
    pub async fn get_state(&self, name: &str) -> Option<RecoveryState> {
        let states = self.states.read().await;
        states.get(name).cloned()
    }

    /// Check if a subsystem can be healed (hasn't exceeded max attempts).
    pub async fn can_heal(&self, name: &str) -> bool {
        let states = self.states.read().await;
        states
            .get(name)
            .map(|s| s.attempt_count < self.config.max_restart_attempts)
            .unwrap_or(false)
    }

    /// Reset recovery state for a subsystem (e.g., after successful manual recovery).
    pub async fn reset(&self, name: &str) {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(name) {
            state.attempt_count = 0;
            state.current_backoff_ms = self.config.base_backoff_ms;
            state.is_recovering = false;
            state.last_error = None;
            info!("Recovery state reset for {}", name);
        }
    }

    /// Get all recovery states.
    pub async fn all_states(&self) -> HashMap<String, RecoveryState> {
        self.states.read().await.clone()
    }

    /// Check if cooldown has passed since last successful recovery.
    pub async fn is_cooled_down(&self, name: &str) -> bool {
        let states = self.states.read().await;
        if let Some(state) = states.get(name) {
            if let Some(last_success) = state.last_success {
                let elapsed = (Utc::now() - last_success).num_seconds() as u64;
                elapsed >= self.config.cooldown_secs
            } else {
                true
            }
        } else {
            true
        }
    }
}

impl Default for SelfHealer {
    fn default() -> Self {
        Self::new(HealingConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn healing_config_defaults() {
        let config = HealingConfig::default();
        assert_eq!(config.max_restart_attempts, 5);
        assert_eq!(config.base_backoff_ms, 1000);
        assert_eq!(config.max_backoff_ms, 30000);
    }

    #[tokio::test]
    async fn register_and_get_state() {
        let healer = SelfHealer::new(HealingConfig::default());
        healer.register("whisper", || async { Ok(()) }).await;
        let state = healer.get_state("whisper").await;
        assert!(state.is_some());
        assert_eq!(state.unwrap().attempt_count, 0);
    }

    #[tokio::test]
    async fn heal_success() {
        let healer = SelfHealer::new(HealingConfig {
            base_backoff_ms: 0,
            ..Default::default()
        });
        let healed = Arc::new(AtomicBool::new(false));
        let healed_clone = healed.clone();
        healer
            .register("audio", move || {
                let healed = healed_clone.clone();
                async move {
                    healed.store(true, Ordering::SeqCst);
                    Ok(())
                }
            })
            .await;

        let result = healer.heal("audio").await;
        assert!(result.is_ok());
        assert!(healed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn heal_failure() {
        let healer = SelfHealer::new(HealingConfig {
            base_backoff_ms: 0,
            ..Default::default()
        });
        healer
            .register("broken", || async { Err("broken".into()) })
            .await;

        let result = healer.heal("broken").await;
        assert!(result.is_err());
        let state = healer.get_state("broken").await.unwrap();
        assert_eq!(state.attempt_count, 1);
        assert_eq!(state.last_error.as_deref(), Some("broken"));
    }

    #[tokio::test]
    async fn max_attempts_exceeded() {
        let healer = SelfHealer::new(HealingConfig {
            max_restart_attempts: 2,
            base_backoff_ms: 0,
            ..Default::default()
        });
        healer
            .register("fail", || async { Err("fail".into()) })
            .await;

        let _ = healer.heal("fail").await;
        let _ = healer.heal("fail").await;
        let result = healer.heal("fail").await;
        assert!(result.is_err());
        assert!(!healer.can_heal("fail").await);
    }

    #[tokio::test]
    async fn reset_state() {
        let healer = SelfHealer::new(HealingConfig {
            base_backoff_ms: 0,
            max_restart_attempts: 1,
            ..Default::default()
        });
        healer.register("svc", || async { Err("err".into()) }).await;
        let _ = healer.heal("svc").await;
        assert!(!healer.can_heal("svc").await);

        healer.reset("svc").await;
        assert!(healer.can_heal("svc").await);
    }

    #[test]
    fn backoff_calculation() {
        let healer = SelfHealer::new(HealingConfig {
            base_backoff_ms: 1000,
            max_backoff_ms: 30000,
            ..Default::default()
        });
        assert_eq!(healer.calculate_backoff(0), 0);
        assert_eq!(healer.calculate_backoff(1), 1000);
        assert_eq!(healer.calculate_backoff(2), 2000);
        assert_eq!(healer.calculate_backoff(3), 4000);
        assert_eq!(healer.calculate_backoff(10), 30000); // capped
    }

    #[tokio::test]
    async fn heal_success_resets_attempt_budget() {
        let healer = SelfHealer::new(HealingConfig {
            max_restart_attempts: 2,
            base_backoff_ms: 0,
            ..Default::default()
        });
        healer.register("flaky", || async { Ok(()) }).await;

        assert!(healer.heal("flaky").await.is_ok());
        let state = healer.get_state("flaky").await.unwrap();
        assert_eq!(
            state.attempt_count, 0,
            "success must reset the attempt budget"
        );
        assert!(healer.can_heal("flaky").await);

        // A later failure can still be retried within the budget.
        assert!(healer.heal("flaky").await.is_ok());
    }

    #[tokio::test]
    async fn heal_is_idempotent_while_recovering() {
        let (release_tx, release_rx) = tokio::sync::watch::channel(false);
        let healer = Arc::new(SelfHealer::new(HealingConfig {
            max_restart_attempts: 3,
            base_backoff_ms: 0,
            ..Default::default()
        }));
        healer
            .register("slow", move || {
                let release = release_rx.clone();
                async move {
                    // Block until the first heal is released, simulating a slow restart.
                    loop {
                        if *release.borrow() {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                    }
                    Ok(())
                }
            })
            .await;

        let for_task = healer.clone();
        let first = tokio::spawn(async move { for_task.heal("slow").await });
        // Give the first heal a chance to set is_recovering.
        tokio::task::yield_now().await;
        let second = healer.heal("slow").await;

        assert!(
            second.is_ok(),
            "a second heal while recovering must not error"
        );

        let _ = release_tx.send(true);
        assert!(first.await.unwrap().is_ok());
        let state = healer.get_state("slow").await.unwrap();
        assert_eq!(state.attempt_count, 0);
    }
}
