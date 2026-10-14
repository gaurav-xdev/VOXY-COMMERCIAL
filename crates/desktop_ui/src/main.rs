mod app;
mod bridge;
mod components;
mod styles;
pub mod supervisor;
mod views;

pub use supervisor::{Supervisor, SupervisorConfig, SupervisorStatus, WindowsJobObject};

use std::sync::{Arc, OnceLock};
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

use bridge::AppBridge;
use components::THREE_JS_CORE_SCRIPT;
use styles::APP_CSS;

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

pub fn get_bridge() -> AppBridge {
    BRIDGE.get().expect("AppBridge not initialized").clone()
}

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

        let data_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("VOXY");
        let _ = std::fs::create_dir_all(&data_dir);

        let cognition: Arc<dyn voxy_cognition::CognitiveEngine> =
            Arc::new(InMemoryCognitiveEngine::new(CognitionConfig::default()));

        let memory: Arc<dyn voxy_memory::MemoryApi> = {
            let mem_db_path = data_dir.join("memory.db");
            let engine = SqliteMemoryEngine::with_path(mem_db_path);
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
            .map(|p| p.join("VOXY"))
            .unwrap_or_default();
        let downloads = Arc::new(
            DownloadManager::new(3, Some(download_dir.to_string_lossy().as_ref()))
                .expect("Failed to create DownloadManager"),
        );

        let notifications =
            Arc::new(NotificationManager::new().expect("Failed to create NotificationManager"));

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
                    panic!(
                        "CRITICAL: Failed to open persistent audit log store at {}: {e}",
                        db_path.display()
                    );
                }
            }
        };

        let intelligence_config = IntelligenceConfig::default();
        let (exp_bridge, exp_input_tx, _exp_output_rx) =
            ExperienceBridge::new(intelligence_config);
        exp_bridge.start().await;
        let exp_bridge = Arc::new(exp_bridge);

        let ipc = Arc::new(bridge::IpcBridge::new());

        let commercial_db_path = data_dir.join("commercial.db");
        let commercial_sqlite = Arc::new(voxy_database::SqliteDatabase::new());
        let db_cfg = voxy_database::DatabaseConfig {
            kind: voxy_database::DatabaseKind::Sqlite,
            path: Some(commercial_db_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        use voxy_database::StorageProvider;
        if let Err(e) = commercial_sqlite.connect(&db_cfg).await {
            panic!("CRITICAL: Failed to connect persistent commercial database: {e}");
        }

        let commercial_store = Arc::new(voxy_database::CommercialStore::new(commercial_sqlite));
        let _ = commercial_store.initialize_schema().await;

        let auth_middleware = Arc::new(voxy_api_server::middleware::AuthMiddleware::new(
            commercial_store.clone(),
        ));
        let rate_limiter = Arc::new(voxy_api_server::middleware::RateLimitMiddleware::new(
            120,
            std::time::Duration::from_secs(60),
        ));
        let api_handlers = Arc::new(voxy_api_server::handlers::ApiHandlers::new(
            commercial_store,
            None,
            None,
        ));
        let api_router = Arc::new(voxy_api_server::router::ApiRouter::new(
            api_handlers,
            auth_middleware,
            rate_limiter,
        ));
        let auth = Arc::new(bridge::AuthBridge::new(api_router));

        let window_tracker = Arc::new(voxy_desktop_runtime::window_manager::WindowTracker::new());

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
            ipc,
            auth,
            window_tracker,
        )
    })
}

static SUPERVISOR: OnceLock<Arc<Supervisor>> = OnceLock::new();

pub fn get_supervisor() -> Option<Arc<Supervisor>> {
    SUPERVISOR.get().cloned()
}

fn main() {
    match dotenvy::dotenv() {
        Ok(path) => println!("Loaded .env from {}", path.display()),
        Err(_) => println!("Running with system environment variables"),
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|arg| arg == "--version" || arg == "-v") {
        println!("OSMOO — Windows AI Operating Companion v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("OSMOO — Windows AI Operating Companion v{}", env!("CARGO_PKG_VERSION"));
        println!("Usage: OSMOO.exe [OPTIONS]");
        println!();
        println!("Options:");
        println!("  -h, --help           Show this help message");
        println!("  -v, --version        Show application version");
        println!("  --status             Display supervisor and component status");
        println!("  --daemon-only        Run background daemon under supervision without GUI");
        println!("  --no-daemon          Launch UI without auto-spawning background daemon");
        println!("  --no-overlay         Launch without auto-spawning overlay");
        println!("  -a, --autostart      Start minimized to system tray");
        return;
    }

    tracing::info!("Starting OSMOO Desktop UI v{}", env!("CARGO_PKG_VERSION"));

    let no_daemon = args.iter().any(|arg| arg == "--no-daemon");
    let no_overlay = args.iter().any(|arg| arg == "--no-overlay");
    let daemon_only = args.iter().any(|arg| arg == "--daemon-only" || arg == "--headless");
    let is_autostart = args.iter().any(|arg| arg == "--autostart" || arg == "-a");

    let supervisor_config = SupervisorConfig {
        autostart_daemon: !no_daemon,
        autostart_overlay: !no_overlay && !daemon_only,
        ..Default::default()
    };

    let supervisor = match Supervisor::new(supervisor_config) {
        Ok(sup) => {
            let sup = Arc::new(sup);
            let _ = SUPERVISOR.set(sup.clone());
            sup
        }
        Err(e) => {
            tracing::warn!("Failed to initialize supervisor: {e}");
            return;
        }
    };

    if args.iter().any(|arg| arg == "--status") {
        let status = supervisor.status();
        println!("{}", serde_json::to_string_pretty(&status).unwrap_or_default());
        return;
    }

    let _ = supervisor.start_all();

    if daemon_only {
        tracing::info!("Running OSMOO in daemon-only supervisor mode. Press Ctrl+C to terminate.");
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("Shutdown signal received");
            let _ = supervisor.shutdown().await;
        });
        return;
    }

    let bridge = init_bridge();
    BRIDGE.set(bridge).ok();

    let mut window_builder = dioxus::desktop::WindowBuilder::new()
        .with_title("OSMOO — Windows AI Operating Companion")
        .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 820.0))
        .with_resizable(true);

    if is_autostart {
        window_builder = window_builder.with_visible(false);
    }

    let head_content = format!(
        "<style>{}</style>\n{}",
        APP_CSS, THREE_JS_CORE_SCRIPT
    );

    let cfg = dioxus::desktop::Config::new()
        .with_window(window_builder)
        .with_custom_head(head_content);

    dioxus::LaunchBuilder::desktop()
        .with_cfg(cfg)
        .launch(app::App);

    // Clean shutdown of supervisor and all supervised children when UI exits
    tracing::info!("OSMOO main window closed; initiating supervisor shutdown");
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        let _ = supervisor.shutdown().await;
    });
}
