use std::sync::Arc;

use voxy_desktop_runtime::settings::SettingsManager;

#[derive(Clone)]
pub struct SettingsBridge {
    manager: Arc<SettingsManager>,
}

impl SettingsBridge {
    pub fn new(manager: Arc<SettingsManager>) -> Self {
        Self { manager }
    }

    pub fn get_snapshot(&self) -> voxy_desktop_runtime::settings::SettingsSnapshot {
        self.manager.get()
    }

    pub fn update(&self, settings: voxy_desktop_runtime::settings::SettingsSnapshot) -> Result<(), String> {
        self.manager
            .update(settings)
            .map_err(|e| format!("Update failed: {}", e))
    }

    pub fn rollback(&self) -> Result<(), String> {
        self.manager
            .rollback()
            .map_err(|e| format!("Rollback failed: {}", e))
    }

    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<voxy_desktop_runtime::settings::SettingsSnapshot> {
        self.manager.subscribe()
    }
}
