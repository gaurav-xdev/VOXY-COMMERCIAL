//! Desktop Runtime for VOXY.
//!
//! Provides system tray, auto-launch, hot reload, global shortcuts,
//! notifications, clipboard, window management, download manager,
//! comprehensive settings system, EventBus integration, ConfigManager
//! bridge, and push-to-talk support.

pub mod autolaunch;
pub mod benchmarks;
pub mod clipboard;
pub mod config_bridge;
pub mod core;
pub mod download;
pub mod error;
pub mod events;
pub mod notifications;
pub mod push_to_talk;
pub mod settings;
pub mod shortcuts;
pub mod single_instance;
pub mod tray;
pub mod window_manager;

pub use config_bridge::ConfigBridge;
pub use core::{DesktopRuntime, RuntimeConfig};
pub use error::{Result, RuntimeError};
pub use events::DesktopEventBridge;
pub use push_to_talk::{PttState, PushToTalk};
pub use single_instance::SingleInstanceGuard;
