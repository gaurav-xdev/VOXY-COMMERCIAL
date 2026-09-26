use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::dashboard::DashboardData;
use crate::error::Result;
use crate::heartbeat::{HeartbeatConfig, HeartbeatTracker};
use crate::self_healing::{HealingConfig, SelfHealer};
use crate::snapshot::{RuntimeSnapshot, SubsystemStatus};
use voxy_health::HealthMonitor;

/// Configuration for the RuntimeGuard.
#[derive(Debug, Clone)]
pub struct GuardConfig {
    pub healing: HealingConfig,
    pub heartbeat: HeartbeatConfig,
    pub snapshot_interval_ms: u64,
    pub enable_self_healing: bool,
    pub enable_heartbeat_watchdog: bool,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            healing: HealingConfig::default(),
            heartbeat: HeartbeatConfig::default(),
            snapshot_interval_ms: 5000,
            enable_self_healing: true,
            enable_heartbeat_watchdog: true,
        }
    }
}

/// The central runtime health guard.
///
/// Orchestrates health monitoring, self-healing, heartbeat tracking,
/// and dashboard data for all VOXY subsystems.
pub struct RuntimeGuard {
    config: GuardConfig,
    running: Arc<AtomicBool>,
    health_monitor: Arc<HealthMonitor>,
    heartbeat_tracker: Arc<HeartbeatTracker>,
    self_healer: Arc<SelfHealer>,
    snapshots: Arc<RwLock<RuntimeSnapshot>>,
    start_time: Instant,
    total_restarts: Arc<AtomicU32>,
    /// Per-subsystem custom metadata (mood, presence, goals, etc.)
    metadata: Arc<RwLock<HashMap<String, String>>>,
    /// Handle to the background monitoring task for proper cleanup.
    monitor_handle: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl RuntimeGuard {
    pub fn new(config: GuardConfig) -> Self {
        let health_monitor = Arc::new(HealthMonitor::new(config.snapshot_interval_ms));
        let heartbeat_tracker = Arc::new(HeartbeatTracker::new(config.heartbeat.clone()));
        let self_healer = Arc::new(SelfHealer::new(config.healing.clone()));

        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            health_monitor,
            heartbeat_tracker,
            self_healer,
            snapshots: Arc::new(RwLock::new(RuntimeSnapshot::new())),
            start_time: Instant::now(),
            total_restarts: Arc::new(AtomicU32::new(0)),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            monitor_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Register a subsystem for monitoring with a health check function.
    pub async fn register_subsystem<F, Fut>(&self, name: &str, health_fn: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = voxy_health::HealthReport> + Send + Sync + 'static,
    {
        let name_owned = name.to_string();
        self.health_monitor
            .register_check(
                name,
                Box::new(move || {
                    let fut = health_fn();
                    Box::pin(fut)
                }),
            )
            .await;

        self.heartbeat_tracker.register(name);
        info!("Subsystem registered: {}", name_owned);
    }

    /// Register a subsystem with a restart function for self-healing.
    pub async fn register_healable<FH, FutH, FR, FutR>(
        &self,
        name: &str,
        health_fn: FH,
        restart_fn: FR,
    ) where
        FH: Fn() -> FutH + Send + Sync + 'static,
        FutH: std::future::Future<Output = voxy_health::HealthReport> + Send + Sync + 'static,
        FR: Fn() -> FutR + Send + Sync + 'static,
        FutR: std::future::Future<Output = std::result::Result<(), String>> + Send + Sync + 'static,
    {
        self.health_monitor
            .register_check(
                name,
                Box::new(move || {
                    let fut = health_fn();
                    Box::pin(fut)
                }),
            )
            .await;

        self.heartbeat_tracker.register(name);
        self.self_healer.register(name, restart_fn).await;
        info!("Healable subsystem registered: {}", name);
    }

    /// Record a heartbeat from a subsystem.
    pub fn heartbeat(&self, name: &str) {
        self.heartbeat_tracker.beat(name);
    }

    /// Set custom metadata (mood, presence, goals, etc.)
    pub async fn set_metadata(&self, key: &str, value: &str) {
        let mut meta = self.metadata.write().await;
        meta.insert(key.to_string(), value.to_string());
    }

    /// Get a snapshot of the current runtime health.
    pub async fn snapshot(&self) -> RuntimeSnapshot {
        let mut snap = self.snapshots.read().await.clone();
        snap.uptime_secs = self.start_time.elapsed().as_secs();
        snap.total_restarts = self.total_restarts.load(Ordering::Relaxed);

        // Pull metadata
        let meta = self.metadata.read().await;
        snap.current_mood = meta.get("mood").cloned();
        snap.presence_state = meta.get("presence").cloned();
        snap.current_goal = meta.get("goal").cloned();
        snap.desktop_activity = meta.get("activity").cloned();
        snap.current_plan = meta.get("plan").cloned();
        snap.current_workflow = meta.get("workflow").cloned();

        // Collect system metrics
        snap.thread_count = num_cpus::get();
        let (ram_used, ram_total) = get_memory_usage();
        snap.ram_usage_mb = ram_used / (1024 * 1024);
        snap.ram_total_mb = ram_total / (1024 * 1024);

        // Collect subsystem health
        let reports = self.health_monitor.check_all().await;
        for (name, report) in &reports {
            let restart_count = self
                .self_healer
                .get_state(name)
                .await
                .map(|s| s.attempt_count)
                .unwrap_or(0);

            snap.subsystems.insert(
                name.clone(),
                SubsystemStatus {
                    name: name.clone(),
                    health: report.status.clone(),
                    last_heartbeat: self
                        .heartbeat_tracker
                        .last_seen(name)
                        .unwrap_or_else(Utc::now),
                    restart_count,
                    last_error: report.details.clone(),
                    latency_ms: report.latency_ms,
                    uptime_secs: self.start_time.elapsed().as_secs(),
                },
            );
        }

        snap.recalculate_overall_health();
        snap
    }

    /// Generate dashboard data.
    pub async fn dashboard(&self) -> DashboardData {
        let snap = self.snapshot().await;
        DashboardData::from_snapshot(snap)
    }

    /// Start the guard's background monitoring loop.
    pub fn start(self: &Arc<Self>) {
        self.running.store(true, Ordering::SeqCst);
        let guard = self.clone();

        let handle = tokio::spawn(async move {
            info!("RuntimeGuard started");
            let mut interval =
                tokio::time::interval(Duration::from_millis(guard.config.snapshot_interval_ms));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if !guard.running.load(Ordering::Relaxed) {
                            break;
                        }

                        // Check heartbeats
                        if guard.config.enable_heartbeat_watchdog {
                            let dead = guard.heartbeat_tracker.check_all_alive();
                            for name in &dead {
                                let missed = guard.heartbeat_tracker.increment_missed(name);
                                warn!(
                                    "Heartbeat missed for {} (missed: {})",
                                    name, missed
                                );

                                // Auto-heal if enabled
                                if guard.config.enable_self_healing && missed >= 2 {
                                    if guard.self_healer.can_heal(name).await {
                                        error!("Triggering self-healing for {}", name);
                                        let healer = guard.self_healer.clone();
                                        let total_restarts = guard.total_restarts.clone();
                                        let name = name.clone();
                                        // Run healing off the monitor loop so one slow or
                                        // hung subsystem cannot block heartbeat checks and
                                        // shutdown handling for all other subsystems.
                                        tokio::spawn(async move {
                                            match healer.heal(&name).await {
                                                Ok(()) => {
                                                    total_restarts.fetch_add(1, Ordering::Relaxed);
                                                    info!("Self-healing succeeded for {}", name);
                                                }
                                                Err(e) => {
                                                    error!("Self-healing failed for {}: {}", name, e);
                                                }
                                            }
                                        });
                                    } else {
                                        error!(
                                            "Subsystem {} exceeded max restart attempts",
                                            name
                                        );
                                    }
                                }
                            }
                        }

                        // Update snapshot
                        let snap = guard.snapshot().await;
                        *guard.snapshots.write().await = snap;
                    }
                    _ = tokio::signal::ctrl_c() => {
                        break;
                    }
                }
            }

            info!("RuntimeGuard stopped");
        });

        // Store the handle for proper cleanup
        let monitor_handle = self.monitor_handle.clone();
        tokio::spawn(async move {
            *monitor_handle.write().await = Some(handle);
        });
    }

    /// Stop the guard and await the monitoring task.
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let handle = self.monitor_handle.write().await.take();
        if let Some(h) = handle {
            let _ = tokio::time::timeout(Duration::from_secs(5), h).await;
        }
    }

    /// Trigger manual healing for a subsystem.
    pub async fn heal(&self, name: &str) -> Result<()> {
        self.self_healer.heal(name).await
    }

    /// Check if a specific subsystem is alive via heartbeat.
    pub fn is_alive(&self, name: &str) -> bool {
        self.heartbeat_tracker.is_alive(name)
    }

    /// Get the self-healer reference.
    pub fn self_healer(&self) -> &Arc<SelfHealer> {
        &self.self_healer
    }

    /// Get the heartbeat tracker reference.
    pub fn heartbeat_tracker(&self) -> &Arc<HeartbeatTracker> {
        &self.heartbeat_tracker
    }

    /// Get the health monitor reference.
    pub fn health_monitor(&self) -> &Arc<HealthMonitor> {
        &self.health_monitor
    }
}

#[cfg(target_os = "windows")]
fn get_memory_usage() -> (u64, u64) {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    (sys.used_memory(), sys.total_memory())
}

#[cfg(not(target_os = "windows"))]
fn get_memory_usage() -> (u64, u64) {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    (sys.used_memory(), sys.total_memory())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn guard_creation() {
        let guard = RuntimeGuard::new(GuardConfig::default());
        let snap = guard.snapshot().await;
        assert_eq!(snap.health_pct, 100.0);
        assert_eq!(snap.total_restarts, 0);
    }

    #[tokio::test]
    async fn register_subsystem() {
        let guard = RuntimeGuard::new(GuardConfig::default());
        guard
            .register_subsystem("test_svc", || async {
                voxy_health::HealthReport::new("test_svc", voxy_shared::HealthStatus::Healthy)
            })
            .await;
        let snap = guard.snapshot().await;
        assert!(snap.subsystems.contains_key("test_svc"));
    }

    #[tokio::test]
    async fn heartbeat_tracking() {
        let guard = RuntimeGuard::new(GuardConfig::default());
        guard
            .register_subsystem("audio", || async {
                voxy_health::HealthReport::new("audio", voxy_shared::HealthStatus::Healthy)
            })
            .await;
        guard.heartbeat("audio");
        assert!(guard.is_alive("audio"));
    }

    #[tokio::test]
    async fn set_metadata() {
        let guard = RuntimeGuard::new(GuardConfig::default());
        guard.set_metadata("mood", "cheerful").await;
        guard.set_metadata("activity", "coding").await;
        let snap = guard.snapshot().await;
        assert_eq!(snap.current_mood.as_deref(), Some("cheerful"));
        assert_eq!(snap.desktop_activity.as_deref(), Some("coding"));
    }

    #[tokio::test]
    async fn dashboard_generation() {
        let guard = RuntimeGuard::new(GuardConfig::default());
        guard
            .register_subsystem("svc", || async {
                voxy_health::HealthReport::new("svc", voxy_shared::HealthStatus::Healthy)
            })
            .await;
        let dashboard = guard.dashboard().await;
        assert!(dashboard.html.contains("VOXY Runtime Dashboard"));
    }
}
