pub mod auth;
pub mod cognition;
pub mod discovery;
pub mod events;
pub mod ipc;
pub mod memory;
pub mod settings;
pub mod voice;

use std::sync::Arc;

pub use auth::AuthBridge;
pub use discovery::LocalModelDiscovery;
pub use ipc::IpcBridge;
pub use settings::SettingsBridge;
pub use voice::VoiceBridge;
pub use cognition::CognitionBridge;
pub use memory::MemoryBridge;
pub use events::EventBridge;

use voxy_cognition::CognitiveEngine;
use voxy_companion_intelligence::{ExperienceBridge, ExperienceInput};
use voxy_config::ConfigManager;
use voxy_database::{AuditLogStore, ConversationStore};
use voxy_desktop_runtime::{
    download::DownloadManager, notifications::NotificationManager, settings::SettingsManager,
    window_manager::WindowTracker,
};
use voxy_event_bus::EventBus;
use voxy_health::HealthMonitor;
use voxy_memory::MemoryApi;
use voxy_personality::PersonalityManager;
use voxy_plugin_runtime::PluginManager;
use voxy_security::GuardianEngine;
use voxy_voice::VoicePipeline;

#[derive(Clone)]
pub struct AppBridge {
    pub event_bus: Arc<EventBus>,
    pub settings: Arc<SettingsManager>,
    pub config_manager: Arc<ConfigManager>,
    pub voice: Arc<VoicePipeline>,
    pub cognition: Arc<dyn CognitiveEngine>,
    pub memory: Arc<dyn MemoryApi>,
    pub personality: Arc<dyn PersonalityManager>,
    pub security: Arc<GuardianEngine>,
    pub plugins: Arc<PluginManager>,
    pub health: Arc<HealthMonitor>,
    pub downloads: Arc<DownloadManager>,
    pub notifications: Arc<NotificationManager>,
    pub conversations: Arc<dyn ConversationStore>,
    pub audit_log: Arc<dyn AuditLogStore>,
    pub experience: Arc<ExperienceBridge>,
    pub experience_input: tokio::sync::broadcast::Sender<ExperienceInput>,
    pub ipc: Arc<IpcBridge>,
    pub auth: Arc<AuthBridge>,
    pub window_tracker: Arc<WindowTracker>,
}

impl AppBridge {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_bus: Arc<EventBus>,
        settings: Arc<SettingsManager>,
        config_manager: Arc<ConfigManager>,
        voice: Arc<VoicePipeline>,
        cognition: Arc<dyn CognitiveEngine>,
        memory: Arc<dyn MemoryApi>,
        personality: Arc<dyn PersonalityManager>,
        security: Arc<GuardianEngine>,
        plugins: Arc<PluginManager>,
        health: Arc<HealthMonitor>,
        downloads: Arc<DownloadManager>,
        notifications: Arc<NotificationManager>,
        conversations: Arc<dyn ConversationStore>,
        audit_log: Arc<dyn AuditLogStore>,
        experience: Arc<ExperienceBridge>,
        experience_input: tokio::sync::broadcast::Sender<ExperienceInput>,
        ipc: Arc<IpcBridge>,
        auth: Arc<AuthBridge>,
        window_tracker: Arc<WindowTracker>,
    ) -> Self {
        Self {
            event_bus,
            settings,
            config_manager,
            voice,
            cognition,
            memory,
            personality,
            security,
            plugins,
            health,
            downloads,
            notifications,
            conversations,
            audit_log,
            experience,
            experience_input,
            ipc,
            auth,
            window_tracker,
        }
    }
}
