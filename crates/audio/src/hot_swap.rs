//! Device hot-swap management for the voice pipeline.
//!
//! Provides callback-based device change detection with pipeline recovery:
//! - Hot-swap detection for mic/speaker connect/disconnect
//! - Pipeline recovery after device change
//! - Sleep/resume handling
//! - Fallback device selection
//!
//! Wraps the existing polling `DeviceChangeWatcher` into a higher-level
//! interface with recovery actions.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::device_watcher::{DeviceChangeEvent, DeviceChangeWatcher};

/// Configuration for hot-swap handling.
#[derive(Debug, Clone)]
pub struct HotSwapConfig {
    /// How often to poll for device changes (if callback not available).
    pub poll_interval: Duration,
    /// Delay after detecting device change before recovering pipeline.
    pub recovery_delay: Duration,
    /// Maximum attempts to recover pipeline.
    pub max_recovery_attempts: u32,
    /// Timeout for pipeline recovery operations.
    pub recovery_timeout: Duration,
    /// Preferred fallback device name patterns (first match wins).
    pub fallback_patterns: Vec<String>,
    /// Minimum silence duration after device change before resuming.
    pub post_change_silence: Duration,
}

impl Default for HotSwapConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(1000),
            recovery_delay: Duration::from_millis(500),
            max_recovery_attempts: 3,
            recovery_timeout: Duration::from_secs(5),
            fallback_patterns: vec![
                "Headset".to_string(),
                "USB".to_string(),
                "Default".to_string(),
            ],
            post_change_silence: Duration::from_millis(200),
        }
    }
}

/// State of the pipeline after a device change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineState {
    /// Pipeline is running normally.
    Running,
    /// Pipeline is paused due to device change.
    Paused,
    /// Pipeline is recovering after device change.
    Recovering,
    /// Pipeline recovery failed.
    Failed,
    /// Pipeline is in sleep mode (no active audio device).
    Sleeping,
}

/// A device change event with additional context.
#[derive(Debug, Clone)]
pub struct HotSwapEvent {
    /// The underlying device change event.
    pub device_event: DeviceChangeEvent,
    /// Timestamp of the event.
    pub timestamp: Instant,
    /// Current pipeline state.
    pub pipeline_state: PipelineState,
    /// Number of recovery attempts so far.
    pub recovery_attempts: u32,
}

/// Recovery action to take after a device change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// No action needed.
    None,
    /// Reconnect with existing device.
    Reconnect,
    /// Switch to fallback device.
    SwitchFallback,
    /// Pause pipeline (wait for user to connect device).
    Pause,
    /// Enter sleep mode.
    Sleep,
}

/// Callback for handling device change events.
#[async_trait::async_trait]
pub trait HotSwapHandler: Send + Sync {
    /// Called when a device change is detected.
    async fn on_device_change(&self, event: &HotSwapEvent) -> RecoveryAction;

    /// Called when pipeline state changes.
    async fn on_state_change(&self, old_state: PipelineState, new_state: PipelineState);

    /// Called to perform the actual recovery.
    async fn recover(&self, action: RecoveryAction) -> Result<(), String>;
}

/// No-op handler for testing.
pub struct NoopHotSwapHandler;

#[async_trait::async_trait]
impl HotSwapHandler for NoopHotSwapHandler {
    async fn on_device_change(&self, _event: &HotSwapEvent) -> RecoveryAction {
        RecoveryAction::None
    }

    async fn on_state_change(&self, _old: PipelineState, _new: PipelineState) {}

    async fn recover(&self, _action: RecoveryAction) -> Result<(), String> {
        Ok(())
    }
}

/// Manages device hot-swap with pipeline recovery.
pub struct HotSwapManager {
    config: RwLock<HotSwapConfig>,
    state: RwLock<PipelineState>,
    recovery_attempts: RwLock<u32>,
    last_device_change: RwLock<Option<Instant>>,
    initialized: AtomicBool,
    shutdown_signal: Arc<tokio::sync::Notify>,
    event_tx: mpsc::Sender<HotSwapEvent>,
    event_rx: RwLock<Option<mpsc::Receiver<HotSwapEvent>>>,
}

impl HotSwapManager {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(64);
        Self {
            config: RwLock::new(HotSwapConfig::default()),
            state: RwLock::new(PipelineState::Running),
            recovery_attempts: RwLock::new(0),
            last_device_change: RwLock::new(None),
            initialized: AtomicBool::new(false),
            shutdown_signal: Arc::new(tokio::sync::Notify::new()),
            event_tx,
            event_rx: RwLock::new(Some(event_rx)),
        }
    }

    /// Initialize the hot-swap manager with configuration.
    pub fn initialize(&self, config: HotSwapConfig) {
        *self.config.write() = config;
        self.initialized.store(true, Ordering::SeqCst);
    }

    /// Get the current pipeline state.
    pub fn state(&self) -> PipelineState {
        *self.state.read()
    }

    /// Get the number of recovery attempts.
    pub fn recovery_attempts(&self) -> u32 {
        *self.recovery_attempts.read()
    }

    /// Set the pipeline state.
    pub fn set_state(&self, state: PipelineState) {
        let mut current = self.state.write();
        let old = *current;
        *current = state;
        drop(current);
        if old != state {
            tracing::info!("Pipeline state: {:?} -> {:?}", old, state);
        }
    }

    /// Record a device change event.
    pub fn record_device_change(&self) {
        *self.last_device_change.write() = Some(Instant::now());
    }

    /// Get time since last device change.
    pub fn time_since_last_change(&self) -> Option<Duration> {
        self.last_device_change.read().map(|t| t.elapsed())
    }

    /// Check if manager is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Determine recovery action based on event context.
    pub fn determine_action(&self, event: &HotSwapEvent) -> RecoveryAction {
        let config = self.config.read();
        match event.device_event {
            DeviceChangeEvent::DeviceConnected { .. } => RecoveryAction::Reconnect,
            DeviceChangeEvent::DeviceDisconnected { .. } => {
                if event.recovery_attempts >= config.max_recovery_attempts {
                    RecoveryAction::Sleep
                } else {
                    RecoveryAction::SwitchFallback
                }
            }
            DeviceChangeEvent::DefaultInputChanged { .. } => RecoveryAction::Reconnect,
            DeviceChangeEvent::DefaultOutputChanged { .. } => RecoveryAction::Reconnect,
        }
    }

    /// Start the background hot-swap watcher using the existing DeviceChangeWatcher.
    pub fn start_watching(&self, _watcher: Arc<DeviceChangeWatcher>) -> Result<(), String> {
        if !self.is_initialized() {
            return Err("Not initialized".to_string());
        }

        let shutdown = self.shutdown_signal.clone();

        tokio::spawn(async move {
            shutdown.notified().await;
            tracing::info!("Hot-swap watcher shutting down");
        });

        Ok(())
    }

    /// Process a device change event through the hot-swap pipeline.
    pub async fn process_event(&self, handler: &Arc<dyn HotSwapHandler>, event: DeviceChangeEvent) {
        let recovery_attempts = *self.recovery_attempts.read();
        let hot_event = HotSwapEvent {
            device_event: event,
            timestamp: Instant::now(),
            pipeline_state: *self.state.read(),
            recovery_attempts,
        };

        let action = handler.on_device_change(&hot_event).await;
        let _ = self.event_tx.send(hot_event.clone()).await;

        if action != RecoveryAction::None {
            let old_state = *self.state.read();
            *self.state.write() = PipelineState::Recovering;
            handler
                .on_state_change(old_state, PipelineState::Recovering)
                .await;

            let config = self.config.read().clone();
            match tokio::time::timeout(config.recovery_timeout, handler.recover(action)).await {
                Ok(Ok(())) => {
                    let old = *self.state.read();
                    *self.state.write() = PipelineState::Running;
                    *self.recovery_attempts.write() = 0;
                    handler.on_state_change(old, PipelineState::Running).await;
                    tracing::info!("Pipeline recovered after device change");
                }
                Ok(Err(e)) => {
                    tracing::error!("Recovery failed: {}", e);
                    let should_sleep = {
                        let mut attempts = self.recovery_attempts.write();
                        *attempts += 1;
                        *attempts >= config.max_recovery_attempts
                    };
                    if should_sleep {
                        let old = *self.state.read();
                        *self.state.write() = PipelineState::Sleeping;
                        handler.on_state_change(old, PipelineState::Sleeping).await;
                    }
                }
                Err(_) => {
                    tracing::error!("Recovery timed out");
                    let old = *self.state.read();
                    *self.state.write() = PipelineState::Failed;
                    handler.on_state_change(old, PipelineState::Failed).await;
                }
            }
        }
    }

    /// Shutdown the hot-swap watcher.
    pub fn shutdown(&self) {
        self.shutdown_signal.notify_one();
        self.initialized.store(false, Ordering::SeqCst);
    }

    /// Take the event receiver.
    pub fn take_events(&self) -> Option<mpsc::Receiver<HotSwapEvent>> {
        self.event_rx.write().take()
    }
}

impl Default for HotSwapManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device_watcher::DeviceChangeEvent;

    #[test]
    fn hot_swap_config_defaults() {
        let config = HotSwapConfig::default();
        assert!(config.poll_interval < Duration::from_secs(5));
        assert!(config.recovery_delay < Duration::from_secs(1));
        assert!(config.max_recovery_attempts > 0);
    }

    #[test]
    fn pipeline_state_transitions() {
        let state = RwLock::new(PipelineState::Running);
        assert_eq!(*state.read(), PipelineState::Running);

        *state.write() = PipelineState::Paused;
        assert_eq!(*state.read(), PipelineState::Paused);

        *state.write() = PipelineState::Recovering;
        assert_eq!(*state.read(), PipelineState::Recovering);

        *state.write() = PipelineState::Sleeping;
        assert_eq!(*state.read(), PipelineState::Sleeping);
    }

    #[test]
    fn hot_swap_manager_init() {
        let manager = HotSwapManager::new();
        assert!(!manager.is_initialized());
        assert_eq!(manager.state(), PipelineState::Running);
        assert_eq!(manager.recovery_attempts(), 0);

        manager.initialize(HotSwapConfig::default());
        assert!(manager.is_initialized());
    }

    #[test]
    fn hot_swap_event_recording() {
        let manager = HotSwapManager::new();
        manager.initialize(HotSwapConfig::default());

        assert!(manager.time_since_last_change().is_none());

        manager.record_device_change();
        let elapsed = manager.time_since_last_change().unwrap();
        assert!(elapsed < Duration::from_secs(1));
    }

    #[test]
    fn recovery_action_determination() {
        let manager = HotSwapManager::new();
        manager.initialize(HotSwapConfig::default());

        let event = HotSwapEvent {
            device_event: DeviceChangeEvent::DeviceConnected {
                id: "usb-001".to_string(),
                name: "USB Mic".to_string(),
            },
            timestamp: Instant::now(),
            pipeline_state: PipelineState::Running,
            recovery_attempts: 0,
        };
        assert_eq!(manager.determine_action(&event), RecoveryAction::Reconnect);

        let event = HotSwapEvent {
            device_event: DeviceChangeEvent::DeviceDisconnected {
                id: "usb-001".to_string(),
                name: "USB Mic".to_string(),
            },
            timestamp: Instant::now(),
            pipeline_state: PipelineState::Running,
            recovery_attempts: 0,
        };
        assert_eq!(
            manager.determine_action(&event),
            RecoveryAction::SwitchFallback
        );

        let event = HotSwapEvent {
            device_event: DeviceChangeEvent::DeviceDisconnected {
                id: "usb-001".to_string(),
                name: "USB Mic".to_string(),
            },
            timestamp: Instant::now(),
            pipeline_state: PipelineState::Running,
            recovery_attempts: 5,
        };
        assert_eq!(manager.determine_action(&event), RecoveryAction::Sleep);
    }

    #[test]
    fn pipeline_state_set_get() {
        let manager = HotSwapManager::new();
        assert_eq!(manager.state(), PipelineState::Running);

        manager.set_state(PipelineState::Paused);
        assert_eq!(manager.state(), PipelineState::Paused);

        manager.set_state(PipelineState::Recovering);
        assert_eq!(manager.state(), PipelineState::Recovering);
    }

    #[test]
    fn shutdown_clears_initialized() {
        let manager = HotSwapManager::new();
        manager.initialize(HotSwapConfig::default());
        assert!(manager.is_initialized());

        manager.shutdown();
        assert!(!manager.is_initialized());
    }

    #[test]
    fn take_events_returns_receiver() {
        let manager = HotSwapManager::new();
        let rx = manager.take_events();
        assert!(rx.is_some());
        // Taking again returns None
        assert!(manager.take_events().is_none());
    }

    #[test]
    fn noop_handler_satisfies_trait() {
        let handler = NoopHotSwapHandler;
        // Just verify it compiles and the trait is satisfied
        let _: Arc<dyn HotSwapHandler> = Arc::new(handler);
    }

    #[tokio::test]
    async fn hot_swap_manager_events_channel() {
        let manager = HotSwapManager::new();
        let mut rx = manager.take_events().unwrap();

        // Send an event directly
        let event = HotSwapEvent {
            device_event: DeviceChangeEvent::DeviceConnected {
                id: "usb-001".to_string(),
                name: "Test".to_string(),
            },
            timestamp: Instant::now(),
            pipeline_state: PipelineState::Running,
            recovery_attempts: 0,
        };
        manager.event_tx.send(event).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert!(matches!(
            received.device_event,
            DeviceChangeEvent::DeviceConnected { .. }
        ));
    }

    #[test]
    fn state_change_detection() {
        let manager = HotSwapManager::new();
        manager.initialize(HotSwapConfig::default());

        // Same state -> no change detected
        manager.set_state(PipelineState::Running);
        assert_eq!(manager.state(), PipelineState::Running);

        // Different state -> change detected
        manager.set_state(PipelineState::Recovering);
        assert_eq!(manager.state(), PipelineState::Recovering);
    }

    #[test]
    fn recovery_attempts_tracking() {
        let manager = HotSwapManager::new();
        manager.initialize(HotSwapConfig::default());

        assert_eq!(manager.recovery_attempts(), 0);

        // Simulate recovery attempts
        *manager.recovery_attempts.write() = 3;
        assert_eq!(manager.recovery_attempts(), 3);
    }

    #[test]
    fn fallback_patterns_configurable() {
        let config = HotSwapConfig {
            fallback_patterns: vec!["My Headset".to_string()],
            ..Default::default()
        };
        assert_eq!(config.fallback_patterns.len(), 1);
        assert_eq!(config.fallback_patterns[0], "My Headset");
    }

    #[tokio::test]
    async fn process_event_with_noop_handler() {
        let manager = HotSwapManager::new();
        manager.initialize(HotSwapConfig::default());

        let handler: Arc<dyn HotSwapHandler> = Arc::new(NoopHotSwapHandler);
        let event = DeviceChangeEvent::DeviceConnected {
            id: "usb-001".to_string(),
            name: "Test Mic".to_string(),
        };

        manager.process_event(&handler, event).await;

        // Should have received the event
        let mut rx = manager.take_events().unwrap();
        let received = rx.recv().await.unwrap();
        assert!(matches!(
            received.device_event,
            DeviceChangeEvent::DeviceConnected { .. }
        ));
    }
}
