use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::Notify;

#[cfg(test)]
use crate::config::AudioRuntimeConfig;
use crate::device::AudioDeviceManager;
use crate::error::Result;

/// Events emitted when audio devices change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceChangeEvent {
    /// A new device was connected.
    DeviceConnected { id: String, name: String },
    /// A device was disconnected.
    DeviceDisconnected { id: String, name: String },
    /// The default input device changed.
    DefaultInputChanged { id: String, name: String },
    /// The default output device changed.
    DefaultOutputChanged { id: String, name: String },
}

impl std::fmt::Display for DeviceChangeEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DeviceConnected { id, name } => write!(f, "Device connected: {name} ({id})"),
            Self::DeviceDisconnected { id, name } => {
                write!(f, "Device disconnected: {name} ({id})")
            }
            Self::DefaultInputChanged { id, name } => {
                write!(f, "Default input changed: {name} ({id})")
            }
            Self::DefaultOutputChanged { id, name } => {
                write!(f, "Default output changed: {name} ({id})")
            }
        }
    }
}

/// Polling-based device change watcher.
/// Periodically queries the device manager for device list changes.
pub struct DeviceChangeWatcher {
    poll_interval: Duration,
    last_input_ids: RwLock<HashSet<String>>,
    last_output_ids: RwLock<HashSet<String>>,
    last_default_input: RwLock<Option<String>>,
    last_default_output: RwLock<Option<String>>,
    change_signal: Arc<Notify>,
    is_watching: Arc<AtomicBool>,
}

impl DeviceChangeWatcher {
    pub fn new() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            last_input_ids: RwLock::new(HashSet::new()),
            last_output_ids: RwLock::new(HashSet::new()),
            last_default_input: RwLock::new(None),
            last_default_output: RwLock::new(None),
            change_signal: Arc::new(Notify::new()),
            is_watching: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Initialize baseline device state (call once after device manager is ready).
    pub async fn initialize_baseline(&self, manager: &dyn AudioDeviceManager) -> Result<()> {
        let inputs = manager.list_inputs().await?;
        let outputs = manager.list_outputs().await?;
        let default_in = manager.default_input().await.ok();
        let default_out = manager.default_output().await.ok();

        *self.last_input_ids.write() = inputs.iter().map(|d| d.id.clone()).collect();
        *self.last_output_ids.write() = outputs.iter().map(|d| d.id.clone()).collect();
        *self.last_default_input.write() = default_in.map(|d| d.id);
        *self.last_default_output.write() = default_out.map(|d| d.id);

        Ok(())
    }

    /// Poll for device changes and return any events detected.
    pub async fn poll_changes(&self, manager: &dyn AudioDeviceManager) -> Vec<DeviceChangeEvent> {
        let mut events = Vec::new();

        // Check inputs
        if let Ok(inputs) = manager.list_inputs().await {
            let current_ids: HashSet<String> = inputs.iter().map(|d| d.id.clone()).collect();
            let prev_ids = self.last_input_ids.read().clone();

            // Detect disconnections
            for id in &prev_ids {
                if !current_ids.contains(id) {
                    let name = inputs
                        .iter()
                        .find(|d| &d.id == id)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| id.clone());
                    events.push(DeviceChangeEvent::DeviceDisconnected {
                        id: id.clone(),
                        name,
                    });
                }
            }

            // Detect connections (new IDs not in previous set)
            // Note: we can't get the name from the disconnected device,
            // so we log the ID for disconnected devices
            for id in &current_ids {
                if !prev_ids.contains(id) {
                    if let Some(dev) = inputs.iter().find(|d| &d.id == id) {
                        events.push(DeviceChangeEvent::DeviceConnected {
                            id: id.clone(),
                            name: dev.name.clone(),
                        });
                    }
                }
            }

            *self.last_input_ids.write() = current_ids;
        }

        // Check outputs
        if let Ok(outputs) = manager.list_outputs().await {
            let current_ids: HashSet<String> = outputs.iter().map(|d| d.id.clone()).collect();
            let prev_ids = self.last_output_ids.read().clone();

            for id in &prev_ids {
                if !current_ids.contains(id) {
                    let name = outputs
                        .iter()
                        .find(|d| &d.id == id)
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| id.clone());
                    events.push(DeviceChangeEvent::DeviceDisconnected {
                        id: id.clone(),
                        name,
                    });
                }
            }

            for id in &current_ids {
                if !prev_ids.contains(id) {
                    if let Some(dev) = outputs.iter().find(|d| &d.id == id) {
                        events.push(DeviceChangeEvent::DeviceConnected {
                            id: id.clone(),
                            name: dev.name.clone(),
                        });
                    }
                }
            }

            *self.last_output_ids.write() = current_ids;
        }

        // Check default input change
        if let Ok(default_in) = manager.default_input().await {
            let mut prev_default = self.last_default_input.write();
            if prev_default.as_ref() != Some(&default_in.id) {
                events.push(DeviceChangeEvent::DefaultInputChanged {
                    id: default_in.id.clone(),
                    name: default_in.name.clone(),
                });
                *prev_default = Some(default_in.id);
            }
        }

        // Check default output change
        if let Ok(default_out) = manager.default_output().await {
            let mut prev_default = self.last_default_output.write();
            if prev_default.as_ref() != Some(&default_out.id) {
                events.push(DeviceChangeEvent::DefaultOutputChanged {
                    id: default_out.id.clone(),
                    name: default_out.name.clone(),
                });
                *prev_default = Some(default_out.id);
            }
        }

        if !events.is_empty() {
            self.change_signal.notify_waiters();
        }

        events
    }

    /// Start background watching. Returns a handle to stop it.
    pub fn start_watching(
        self: &Arc<Self>,
        manager: Arc<Box<dyn AudioDeviceManager>>,
    ) -> DeviceWatcherHandle {
        self.is_watching.store(true, Ordering::SeqCst);
        let watcher = self.clone();
        let is_watching = self.is_watching.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(watcher.poll_interval);
            interval.tick().await; // skip first immediate tick
            loop {
                if !is_watching.load(Ordering::SeqCst) {
                    break;
                }
                interval.tick().await;
                if !is_watching.load(Ordering::SeqCst) {
                    break;
                }
                let events = watcher.poll_changes(&**manager).await;
                for event in &events {
                    tracing::info!(event = %event, "Device change detected");
                }
            }
        });

        DeviceWatcherHandle { task: handle }
    }

    /// Stop watching.
    pub fn stop(&self) {
        self.is_watching.store(false, Ordering::SeqCst);
    }

    /// Wait for the next device change event.
    pub async fn wait_for_change(&self) {
        self.change_signal.notified().await;
    }

    pub fn is_watching(&self) -> bool {
        self.is_watching.load(Ordering::SeqCst)
    }
}

impl Default for DeviceChangeWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle for a running device watcher task.
pub struct DeviceWatcherHandle {
    task: tokio::task::JoinHandle<()>,
}

impl DeviceWatcherHandle {
    pub async fn stop(self) {
        self.task.abort();
        let _ = tokio::time::timeout(Duration::from_secs(2), self.task).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{AudioDeviceInfo, InMemoryDeviceManager};

    #[allow(dead_code)]
    fn make_device(id: &str, name: &str) -> AudioDeviceInfo {
        AudioDeviceInfo {
            id: id.to_string(),
            name: name.to_string(),
            device_type: voxy_hardware::DeviceType::Microphone,
            status: voxy_hardware::DeviceStatus::Available,
            supported_sample_rates: vec![16000],
            supported_channels: vec![1],
            is_default: false,
        }
    }

    #[tokio::test]
    async fn watcher_creation() {
        let w = DeviceChangeWatcher::new();
        assert!(!w.is_watching());
    }

    #[tokio::test]
    async fn watcher_initialize_baseline() {
        let manager = InMemoryDeviceManager::default();
        let config = AudioRuntimeConfig::default();
        manager.initialize(&config).await.unwrap();

        let w = DeviceChangeWatcher::new();
        w.initialize_baseline(&manager).await.unwrap();

        let inputs = w.last_input_ids.read().clone();
        assert!(inputs.contains("mem-input-001"));
        let outputs = w.last_output_ids.read().clone();
        assert!(outputs.contains("mem-output-001"));
    }

    #[tokio::test]
    async fn watcher_no_changes_on_same_state() {
        let manager = InMemoryDeviceManager::default();
        let config = AudioRuntimeConfig::default();
        manager.initialize(&config).await.unwrap();

        let w = DeviceChangeWatcher::new();
        w.initialize_baseline(&manager).await.unwrap();

        let events = w.poll_changes(&manager).await;
        assert!(events.is_empty());
    }

    #[test]
    fn watcher_event_display() {
        let event = DeviceChangeEvent::DeviceConnected {
            id: "usb-001".into(),
            name: "Blue Yeti".into(),
        };
        assert!(format!("{}", event).contains("Blue Yeti"));

        let event = DeviceChangeEvent::DeviceDisconnected {
            id: "usb-001".into(),
            name: "Blue Yeti".into(),
        };
        assert!(format!("{}", event).contains("disconnected"));

        let event = DeviceChangeEvent::DefaultInputChanged {
            id: "new-001".into(),
            name: "New Mic".into(),
        };
        assert!(format!("{}", event).contains("Default input"));
    }

    #[test]
    fn watcher_default_trait() {
        let w = DeviceChangeWatcher::default();
        assert!(!w.is_watching());
    }
}
