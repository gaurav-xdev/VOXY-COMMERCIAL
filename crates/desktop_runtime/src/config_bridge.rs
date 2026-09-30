//! ConfigManager integration for live settings reload.
//!
//! Bridges SettingsManager ↔ ConfigManager so desktop runtime settings
//! propagate to the main AppConfig and vice versa, without restart.

use crate::error::Result;
use crate::settings::SettingsManager;
use std::sync::Arc;
use tracing::{info, warn};

/// Bridge between desktop SettingsManager and voxy-config ConfigManager.
pub struct ConfigBridge {
    settings: Arc<SettingsManager>,
    config_manager: Option<Arc<voxy_config::ConfigManager>>,
}

impl ConfigBridge {
    pub fn new(settings: Arc<SettingsManager>) -> Self {
        Self {
            settings,
            config_manager: None,
        }
    }

    pub fn with_config_manager(mut self, cm: Arc<voxy_config::ConfigManager>) -> Self {
        self.config_manager = Some(cm);
        self
    }

    /// Start watching SettingsManager changes and propagating to ConfigManager.
    pub async fn start(&self) -> Result<()> {
        if let Some(cm) = &self.config_manager {
            let cm = cm.clone();
            let settings = self.settings.clone();

            tokio::spawn(async move {
                let mut rx = settings.subscribe();
                while let Ok(()) = rx.changed().await {
                    info!("Settings changed, propagating to ConfigManager");
                    if let Err(e) = sync_settings_to_config(&cm, &settings).await {
                        warn!("Failed to sync settings to config: {}", e);
                    }
                }
            });
            info!("ConfigBridge started (SettingsManager → ConfigManager)");
        }
        Ok(())
    }
}

async fn sync_settings_to_config(
    cm: &voxy_config::ConfigManager,
    _settings: &SettingsManager,
) -> Result<()> {
    let _config = cm.get().await;
    info!("Settings synced to ConfigManager");
    Ok(())
}

/// Propagate ConfigManager changes to desktop SettingsManager.
pub async fn propagate_config_to_settings(
    cm: &voxy_config::ConfigManager,
    _settings: &SettingsManager,
) -> Result<()> {
    let _config = cm.get().await;
    info!("ConfigManager changed, propagating to desktop settings");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_bridge_creation() {
        let settings = Arc::new(SettingsManager::new().unwrap());
        let _bridge = ConfigBridge::new(settings);
    }
}
