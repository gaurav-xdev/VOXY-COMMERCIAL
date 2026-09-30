//! Push-to-Talk integration.
//!
//! Global shortcut → Voice pipeline activation with barge-in support.
//! When the shortcut is pressed, publishes a PUSH_TO_TALK event
//! that the voice pipeline consumes to start/stop recording.

use crate::error::Result;
use crate::events::DesktopEventBridge;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::info;

/// Push-to-talk state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PttState {
    Idle,
    Active,
    Barging,
}

/// Push-to-Talk controller.
pub struct PushToTalk {
    state: RwLock<PttState>,
    bridge: Arc<DesktopEventBridge>,
    #[allow(dead_code)]
    shortcut_name: String,
}

impl PushToTalk {
    pub fn new(bridge: Arc<DesktopEventBridge>, shortcut_name: &str) -> Self {
        Self {
            state: RwLock::new(PttState::Idle),
            bridge,
            shortcut_name: shortcut_name.to_string(),
        }
    }

    /// Called when the push-to-talk shortcut is pressed.
    pub async fn on_pressed(&self) -> Result<()> {
        let (active, msg) = {
            let mut state = self.state.write();
            match *state {
                PttState::Idle => {
                    *state = PttState::Active;
                    (true, "Push-to-talk activated")
                }
                PttState::Active => {
                    *state = PttState::Idle;
                    (false, "Push-to-talk deactivated")
                }
                PttState::Barging => {
                    *state = PttState::Idle;
                    (false, "Barge-in cancelled")
                }
            }
        };

        self.bridge.publish_push_to_talk(active).await?;
        info!("{}", msg);
        Ok(())
    }

    /// Called when barge-in is triggered (e.g., user speaks during TTS).
    pub async fn on_barge_in(&self) -> Result<()> {
        let barged = {
            let mut state = self.state.write();
            if *state == PttState::Active {
                *state = PttState::Barging;
                true
            } else {
                false
            }
        };
        if barged {
            info!("Barge-in activated");
        }
        Ok(())
    }

    /// Force deactivate push-to-talk.
    pub async fn deactivate(&self) -> Result<()> {
        let should_deactivate = {
            let mut state = self.state.write();
            if *state != PttState::Idle {
                *state = PttState::Idle;
                true
            } else {
                false
            }
        };
        if should_deactivate {
            self.bridge.publish_push_to_talk(false).await?;
            info!("Push-to-talk force deactivated");
        }
        Ok(())
    }

    pub fn state(&self) -> PttState {
        *self.state.read()
    }

    pub fn is_active(&self) -> bool {
        *self.state.read() != PttState::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ptt_creation() {
        let bus = Arc::new(voxy_event_bus::EventBus::new(10));
        let bridge = Arc::new(DesktopEventBridge::new(bus));
        let ptt = PushToTalk::new(bridge, "push_to_talk");
        assert_eq!(ptt.state(), PttState::Idle);
        assert!(!ptt.is_active());
    }

    #[tokio::test]
    async fn ptt_toggle() {
        let bus = Arc::new(voxy_event_bus::EventBus::new(10));
        let bridge = Arc::new(DesktopEventBridge::new(bus));
        let ptt = PushToTalk::new(bridge, "push_to_talk");

        ptt.on_pressed().await.unwrap();
        assert_eq!(ptt.state(), PttState::Active);
        assert!(ptt.is_active());

        ptt.on_pressed().await.unwrap();
        assert_eq!(ptt.state(), PttState::Idle);
        assert!(!ptt.is_active());
    }

    #[tokio::test]
    async fn ptt_barge_in() {
        let bus = Arc::new(voxy_event_bus::EventBus::new(10));
        let bridge = Arc::new(DesktopEventBridge::new(bus));
        let ptt = PushToTalk::new(bridge, "push_to_talk");

        ptt.on_pressed().await.unwrap();
        ptt.on_barge_in().await.unwrap();
        assert_eq!(ptt.state(), PttState::Barging);

        ptt.on_pressed().await.unwrap();
        assert_eq!(ptt.state(), PttState::Idle);
    }

    #[tokio::test]
    async fn ptt_force_deactivate() {
        let bus = Arc::new(voxy_event_bus::EventBus::new(10));
        let bridge = Arc::new(DesktopEventBridge::new(bus));
        let ptt = PushToTalk::new(bridge, "push_to_talk");

        ptt.on_pressed().await.unwrap();
        assert!(ptt.is_active());

        ptt.deactivate().await.unwrap();
        assert!(!ptt.is_active());
    }
}
