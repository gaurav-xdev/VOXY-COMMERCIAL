#![allow(dead_code)]

use std::sync::Arc;
use std::sync::OnceLock;

use tracing_subscriber::EnvFilter;
use voxy_cognition::{CognitionConfig, InMemoryCognitiveEngine};
use voxy_companion_intelligence::{ExperienceBridge, IntelligenceConfig};
use voxy_config::{AppConfig, ConfigManager, FileConfigProvider};
use voxy_database::{SqliteAuditLogStore, SqliteConversationStore};
use voxy_desktop_runtime::{
    download::DownloadManager, notifications::NotificationManager, settings::SettingsManager,
};
use voxy_event_bus::EventBus;
use voxy_health::HealthMonitor;
use voxy_memory::{MemoryApi, MemoryConfig, SqliteMemoryEngine};
use voxy_personality::{
    CommunicationStyle, MoodState, PersonalityConfig, PersonalityManager, PersonalityProfile,
};
use voxy_plugin_runtime::PluginManager;
use voxy_security::{CapabilityRegistry, GuardianConfig, GuardianEngine, PolicyEngine};
use voxy_voice::{VoiceConfig, VoicePipeline};

use crate::bridge::AppBridge;

mod app;
mod bridge;
mod components;
mod router;
mod styles;
mod views;

struct InMemoryPersonalityManager {
    profiles: parking_lot::RwLock<std::collections::HashMap<String, PersonalityConfig>>,
}

impl InMemoryPersonalityManager {
    fn new() -> Self {
        let mut profiles = std::collections::HashMap::new();
        profiles.insert("default".to_string(), PersonalityConfig::default());
        Self {
            profiles: parking_lot::RwLock::new(profiles),
        }
    }
}

#[async_trait::async_trait]
impl PersonalityManager for InMemoryPersonalityManager {
    async fn load_profile(
        &self,
        id: &str,
    ) -> voxy_personality::Result<Box<dyn PersonalityProfile>> {
        let profiles = self.profiles.read();
        let config = profiles.get(id).cloned().unwrap_or_default();
        Ok(Box::new(InMemoryPersonalityProfile { config }))
    }

    async fn save_profile(
        &self,
        profile: Box<dyn PersonalityProfile>,
    ) -> voxy_personality::Result<()> {
        let mut profiles = self.profiles.write();
        profiles.insert(
            profile.profile_id().to_string(),
            PersonalityConfig::default(),
        );
        Ok(())
    }

    async fn list_profiles(&self) -> voxy_personality::Result<Vec<String>> {
        Ok(self.profiles.read().keys().cloned().collect())
    }

    async fn delete_profile(&self, id: &str) -> voxy_personality::Result<()> {
        self.profiles.write().remove(id);
        Ok(())
    }

    async fn default_profile(&self) -> voxy_personality::Result<Box<dyn PersonalityProfile>> {
        Ok(Box::new(InMemoryPersonalityProfile {
            config: PersonalityConfig::default(),
        }))
    }
}

struct InMemoryPersonalityProfile {
    config: PersonalityConfig,
}

impl PersonalityProfile for InMemoryPersonalityProfile {
    fn profile_id(&self) -> &str {
        &self.config.profile_id
    }

    fn profile_name(&self) -> &str {
        &self.config.profile_name
    }

    fn get_trait(&self, name: &str) -> Option<f64> {
        self.config.traits.get(name).copied()
    }

    fn set_trait(&mut self, name: &str, value: f64) -> voxy_personality::Result<()> {
        self.config.traits.insert(name.to_string(), value);
        Ok(())
    }

    fn all_traits(&self) -> std::collections::HashMap<String, f64> {
        self.config.traits.clone()
    }

    fn mood(&self) -> MoodState {
        self.config.default_mood.clone()
    }

    fn set_mood(&mut self, mood: MoodState) {
        self.config.default_mood = mood;
    }

    fn communication_style(&self) -> CommunicationStyle {
        self.config.communication_style.clone()
    }

    fn set_communication_style(&mut self, style: CommunicationStyle) {
        self.config.communication_style = style;
    }
}

static BRIDGE: OnceLock<AppBridge> = OnceLock::new();

fn init_bridge() -> AppBridge {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    rt.block_on(async {
        let event_bus = Arc::new(EventBus::new(256));

        let settings = Arc::new(SettingsManager::new().expect("Failed to create SettingsManager"));

        let config_manager = Arc::new(
            ConfigManager::new(FileConfigProvider::new(
                AppConfig::default_path().unwrap_or_default(),
            ))
            .await
            .expect("Failed to create ConfigManager"),
        );

        let cognition: Arc<dyn voxy_cognition::CognitiveEngine> =
            Arc::new(InMemoryCognitiveEngine::new(CognitionConfig::default()));

        let memory: Arc<dyn voxy_memory::MemoryApi> = {
            let engine = SqliteMemoryEngine::new();
            let mem_config = MemoryConfig::default();
            let _ = engine.init(&mem_config).await;
            Arc::new(engine)
        };

        let personality: Arc<dyn PersonalityManager> = Arc::new(InMemoryPersonalityManager::new());

        let mut registry = CapabilityRegistry::new();
        let _ = registry.register(voxy_security::Capability::new(
            "voice:capture",
            "Capture audio from microphone",
            voxy_security::RiskLevel::Low,
            voxy_security::CapabilityCategory::Audio,
        ));
        let _ = registry.register(voxy_security::Capability::new(
            "memory:read",
            "Read memory items",
            voxy_security::RiskLevel::Low,
            voxy_security::CapabilityCategory::Memory,
        ));
        let _ = registry.register(voxy_security::Capability::new(
            "memory:write",
            "Write memory items",
            voxy_security::RiskLevel::Medium,
            voxy_security::CapabilityCategory::Memory,
        ));
        let _ = registry.register(voxy_security::Capability::new(
            "automation:execute",
            "Execute automated tasks",
            voxy_security::RiskLevel::High,
            voxy_security::CapabilityCategory::Automation,
        ));
        let security = Arc::new(GuardianEngine::new(
            registry,
            PolicyEngine::new(),
            GuardianConfig::default(),
        ));

        let plugins = Arc::new(PluginManager::new());

        let health = Arc::new(HealthMonitor::new(5000));
        health.add_cpu_check("system-cpu").await;
        health.add_memory_check("system-memory").await;

        let voice_config = VoiceConfig::default();
        let voice = Arc::new(VoicePipeline::new(voice_config));

        let download_dir = dirs::download_dir()
            .or_else(dirs::data_local_dir)
            .map(|p| p.join("voxy"))
            .unwrap_or_default();
        let downloads = Arc::new(
            DownloadManager::new(3, Some(download_dir.to_string_lossy().as_ref()))
                .expect("Failed to create DownloadManager"),
        );

        let notifications =
            Arc::new(NotificationManager::new().expect("Failed to create NotificationManager"));

        let data_dir = dirs::config_dir()
            .or_else(dirs::data_local_dir)
            .map(|p| p.join("voxy"))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        std::fs::create_dir_all(&data_dir).ok();

        let conversations: Arc<dyn voxy_database::ConversationStore> = {
            let db_path = data_dir.join("conversations.db");
            match SqliteConversationStore::new(db_path.to_str().unwrap_or(":memory:")) {
                Ok(store) => {
                    tracing::info!("Loaded SQLite conversation store: {}", db_path.display());
                    Arc::new(store)
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to open conversation DB, falling back to in-memory: {e}"
                    );
                    Arc::new(voxy_database::InMemoryConversationStore::new())
                }
            }
        };

        let audit_log: Arc<dyn voxy_database::AuditLogStore> = {
            let db_path = data_dir.join("audit.db");
            match SqliteAuditLogStore::new(db_path.to_str().unwrap_or(":memory:")) {
                Ok(store) => {
                    tracing::info!("Loaded SQLite audit log store: {}", db_path.display());
                    Arc::new(store)
                }
                Err(e) => {
                    tracing::warn!("Failed to open audit DB, falling back to in-memory: {e}");
                    Arc::new(voxy_database::InMemoryAuditLogStore::new())
                }
            }
        };

        // ── Experience Layer ──────────────────────────────────────────────
        let intelligence_config = IntelligenceConfig::default();
        let (exp_bridge, exp_input_tx, _exp_output_rx) = ExperienceBridge::new(intelligence_config);
        exp_bridge.start().await;
        let exp_bridge = Arc::new(exp_bridge);
        tracing::info!("Experience Layer started in Desktop UI");

        tracing::info!(
            "Backend initialized: EventBus, Settings, Cognition(InMemory), Memory(SQLite), \
             Personality(InMemory), Security, Plugins, Health, Voice, Downloads, Notifications, \
             Conversations(SQLite), AuditLog(SQLite), ExperienceBridge"
        );

        AppBridge::new(
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
            exp_bridge,
            exp_input_tx,
        )
    })
}

fn main() {
    // Load .env file before anything else so env vars are available to all crates
    match dotenvy::dotenv() {
        Ok(path) => println!("Loaded .env from {}", path.display()),
        Err(dotenvy::Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("No .env file found — using environment variables only");
        }
        Err(e) => println!("Warning: failed to load .env: {e}"),
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting VOXY Desktop UI v{}", env!("CARGO_PKG_VERSION"));

    let bridge = init_bridge();

    BRIDGE.set(bridge.clone()).ok();

    let cfg = dioxus::desktop::Config::new()
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_title("VOXY — Windows AI Operating Companion")
                .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 820.0))
                .with_resizable(true),
        )
        .with_custom_head(format!("<style>{}</style>", styles::APP_CSS));

    dioxus::LaunchBuilder::desktop()
        .with_cfg(cfg)
        .launch(app::App);
}
