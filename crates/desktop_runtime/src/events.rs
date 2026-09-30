//! EventBus integration for desktop runtime events.
//!
//! Publishes desktop events (tray, notifications, downloads, shortcuts, window)
//! to the voxy-event-bus for downstream consumers (voice, cognition, UI).

use crate::error::Result;
use std::sync::Arc;
use tracing::info;
use voxy_event_bus::EventBus;
use voxy_shared::Event;

/// Topic constants for desktop runtime events.
pub mod topics {
    pub const TRAY_ACTION: &str = "desktop.tray.action";
    pub const NOTIFICATION_SENT: &str = "desktop.notification.sent";
    pub const NOTIFICATION_CLICKED: &str = "desktop.notification.clicked";
    pub const DOWNLOAD_STARTED: &str = "desktop.download.started";
    pub const DOWNLOAD_COMPLETED: &str = "desktop.download.completed";
    pub const DOWNLOAD_FAILED: &str = "desktop.download.failed";
    pub const SHORTCUT_PRESSED: &str = "desktop.shortcut.pressed";
    pub const WINDOW_STATE_CHANGED: &str = "desktop.window.state_changed";
    pub const SETTINGS_CHANGED: &str = "desktop.settings.changed";
    pub const PUSH_TO_TALK: &str = "desktop.voice.push_to_talk";
}

/// Event payloads (JSON-serializable).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct TrayActionPayload {
    pub action: String,
    pub item_id: Option<u32>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct NotificationPayload {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub level: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct DownloadPayload {
    pub id: u64,
    pub url: String,
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ShortcutPayload {
    pub shortcut_id: u32,
    pub name: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub vk_code: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WindowStatePayload {
    pub hwnd: u64,
    pub title: String,
    pub state: String,
    pub is_foreground: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SettingsChangedPayload {
    pub section: String,
    pub timestamp: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PushToTalkPayload {
    pub pressed: bool,
    pub shortcut_name: String,
}

/// EventBus bridge for the desktop runtime.
pub struct DesktopEventBridge {
    bus: Arc<EventBus>,
}

impl DesktopEventBridge {
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self { bus }
    }

    pub async fn publish_tray_action(&self, action: &str, item_id: Option<u32>) -> Result<()> {
        let payload = TrayActionPayload {
            action: action.to_string(),
            item_id,
        };
        let event = Event::from_json(topics::TRAY_ACTION, "desktop_runtime", &payload)?;
        self.bus
            .publish(topics::TRAY_ACTION, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        info!("Published tray action: {}", action);
        Ok(())
    }

    pub async fn publish_notification_sent(
        &self,
        id: u64,
        title: &str,
        message: &str,
        level: &str,
    ) -> Result<()> {
        let payload = NotificationPayload {
            id,
            title: title.to_string(),
            message: message.to_string(),
            level: level.to_string(),
        };
        let event = Event::from_json(topics::NOTIFICATION_SENT, "desktop_runtime", &payload)?;
        self.bus
            .publish(topics::NOTIFICATION_SENT, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        Ok(())
    }

    pub async fn publish_notification_clicked(
        &self,
        id: u64,
        title: &str,
        message: &str,
        level: &str,
    ) -> Result<()> {
        let payload = NotificationPayload {
            id,
            title: title.to_string(),
            message: message.to_string(),
            level: level.to_string(),
        };
        let event = Event::from_json(topics::NOTIFICATION_CLICKED, "desktop_runtime", &payload)?;
        self.bus
            .publish(topics::NOTIFICATION_CLICKED, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn publish_download_event(
        &self,
        topic: &str,
        id: u64,
        url: &str,
        filename: &str,
        bytes: u64,
        total: Option<u64>,
        status: &str,
    ) -> Result<()> {
        let payload = DownloadPayload {
            id,
            url: url.to_string(),
            filename: filename.to_string(),
            bytes_downloaded: bytes,
            total_bytes: total,
            status: status.to_string(),
        };
        let event = Event::from_json(topic, "desktop_runtime", &payload)?;
        self.bus
            .publish(topic, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        Ok(())
    }

    pub async fn publish_shortcut_pressed(
        &self,
        id: u32,
        name: &str,
        ctrl: bool,
        alt: bool,
        shift: bool,
        vk_code: u32,
    ) -> Result<()> {
        let payload = ShortcutPayload {
            shortcut_id: id,
            name: name.to_string(),
            ctrl,
            alt,
            shift,
            vk_code,
        };
        let event = Event::from_json(topics::SHORTCUT_PRESSED, "desktop_runtime", &payload)?;
        self.bus
            .publish(topics::SHORTCUT_PRESSED, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        info!("Published shortcut: {} (id={})", name, id);
        Ok(())
    }

    pub async fn publish_window_state_changed(
        &self,
        hwnd: u64,
        title: &str,
        state: &str,
        is_foreground: bool,
    ) -> Result<()> {
        let payload = WindowStatePayload {
            hwnd,
            title: title.to_string(),
            state: state.to_string(),
            is_foreground,
        };
        let event = Event::from_json(topics::WINDOW_STATE_CHANGED, "desktop_runtime", &payload)?;
        self.bus
            .publish(topics::WINDOW_STATE_CHANGED, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        Ok(())
    }

    pub async fn publish_settings_changed(&self, section: &str) -> Result<()> {
        let payload = SettingsChangedPayload {
            section: section.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let event = Event::from_json(topics::SETTINGS_CHANGED, "desktop_runtime", &payload)?;
        self.bus
            .publish(topics::SETTINGS_CHANGED, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        info!("Published settings changed: {}", section);
        Ok(())
    }

    pub async fn publish_push_to_talk(&self, pressed: bool) -> Result<()> {
        let payload = PushToTalkPayload {
            pressed,
            shortcut_name: "push_to_talk".to_string(),
        };
        let event = Event::from_json(topics::PUSH_TO_TALK, "desktop_runtime", &payload)?;
        self.bus
            .publish(topics::PUSH_TO_TALK, event)
            .await
            .map_err(|e| crate::error::RuntimeError::Other(e.to_string()))?;
        Ok(())
    }

    pub fn bus(&self) -> &Arc<EventBus> {
        &self.bus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn event_bridge_creation() {
        let bus = Arc::new(EventBus::new(10));
        let _bridge = DesktopEventBridge::new(bus);
    }

    #[tokio::test]
    async fn publish_tray_action() {
        let bus = Arc::new(EventBus::new(10));
        let bridge = DesktopEventBridge::new(bus.clone());
        let mut rx = bus.subscribe(topics::TRAY_ACTION).await.unwrap();
        bridge.publish_tray_action("open", Some(1)).await.unwrap();
        let event = rx.recv().await.unwrap();
        let payload: TrayActionPayload = event.to_json().unwrap();
        assert_eq!(payload.action, "open");
        assert_eq!(payload.item_id, Some(1));
    }

    #[tokio::test]
    async fn publish_shortcut() {
        let bus = Arc::new(EventBus::new(10));
        let bridge = DesktopEventBridge::new(bus.clone());
        let mut rx = bus.subscribe(topics::SHORTCUT_PRESSED).await.unwrap();
        bridge
            .publish_shortcut_pressed(1, "voice_toggle", true, false, true, 0x56)
            .await
            .unwrap();
        let event = rx.recv().await.unwrap();
        let payload: ShortcutPayload = event.to_json().unwrap();
        assert_eq!(payload.name, "voice_toggle");
        assert!(payload.ctrl);
    }

    #[tokio::test]
    async fn publish_push_to_talk() {
        let bus = Arc::new(EventBus::new(10));
        let bridge = DesktopEventBridge::new(bus.clone());
        let mut rx = bus.subscribe(topics::PUSH_TO_TALK).await.unwrap();
        bridge.publish_push_to_talk(true).await.unwrap();
        let event = rx.recv().await.unwrap();
        let payload: PushToTalkPayload = event.to_json().unwrap();
        assert!(payload.pressed);
    }

    #[tokio::test]
    async fn publish_settings_changed() {
        let bus = Arc::new(EventBus::new(10));
        let bridge = DesktopEventBridge::new(bus.clone());
        let mut rx = bus.subscribe(topics::SETTINGS_CHANGED).await.unwrap();
        bridge.publish_settings_changed("voice").await.unwrap();
        let event = rx.recv().await.unwrap();
        let payload: SettingsChangedPayload = event.to_json().unwrap();
        assert_eq!(payload.section, "voice");
    }

    #[tokio::test]
    async fn publish_notification() {
        let bus = Arc::new(EventBus::new(10));
        let bridge = DesktopEventBridge::new(bus.clone());
        let mut rx = bus.subscribe(topics::NOTIFICATION_SENT).await.unwrap();
        bridge
            .publish_notification_sent(1, "Test", "Hello", "info")
            .await
            .unwrap();
        let event = rx.recv().await.unwrap();
        let payload: NotificationPayload = event.to_json().unwrap();
        assert_eq!(payload.title, "Test");
    }
}
