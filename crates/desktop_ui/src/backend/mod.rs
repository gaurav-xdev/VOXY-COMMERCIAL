pub mod commands;

pub use commands::{BackendCommand, BackendError, BackendResult};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;
use voxy_cognition::IntentInput;
use voxy_database::{ConversationStore, MessageRole};
use voxy_ipc::ClientCommand;
use voxy_model_router::{ModelRouter, RoutingMode};
use voxy_provider_core::{
    DefaultProviderRegistry, ProviderCapability, ProviderHealth, ProviderInfo, ProviderKind,
    ProviderStatus,
};
use voxy_tool_calling::{ToolContext, ToolRegistry};

use crate::bridge::AppBridge;
use crate::components::assistant_hud::ChatTurn;
use crate::views::settings::SettingsFormState;

/// Primary dispatcher connecting frontend Dioxus UI actions
/// to real backend services, SQLite persistence, and IPC.
#[derive(Clone)]
pub struct BackendDispatcher {
    bridge: AppBridge,
    default_conversation_id: Arc<RwLock<Option<String>>>,
    model_router: Arc<ModelRouter>,
    tool_registry: Arc<ToolRegistry>,
}

impl BackendDispatcher {
    pub fn new(bridge: AppBridge) -> Self {
        let snapshot = bridge.settings.get();
        let primary_llm = snapshot.models.provider.clone();
        let model_router = Self::create_default_model_router_with_keys(
            &primary_llm,
            Some(&snapshot.models.provider_keys),
        );
        let tool_registry = Arc::new(ToolRegistry::with_builtins());

        // Initialize OSMOO Sovereign Video Studio Enclave (accessible exclusively to OSMOO)
        let enclave_path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("crates/video_studio/enclave");
        let video_enclave = voxy_video_studio::VideoStudioEnclave::new(enclave_path);
        tool_registry.register_sync(Arc::new(voxy_video_studio::VideoStudioPipelineTool::new(
            video_enclave.clone(),
        )));
        tool_registry.register_sync(Arc::new(voxy_video_studio::VideoStudioInspectTool::new(
            video_enclave,
        )));

        // Initialize OSMOO Internal Kodus AI Code Review Engine (accessible exclusively to OSMOO)
        let kodus_security = voxy_kodus_review::KodusSecurityBoundary::new();
        let kodus_bridge = voxy_kodus_review::KodusProviderBridge::new(
            model_router.clone(),
            kodus_security.clone(),
        );
        let kodus_engine = Arc::new(voxy_kodus_review::KodusReviewEngine::new(
            ".",
            kodus_bridge,
            kodus_security,
        ));
        tool_registry.register_sync(Arc::new(voxy_kodus_review::KodusInternalReviewTool::new(
            kodus_engine,
        )));

        // Initialize OSMOO Trinity & SwarmClaw Multi-Agent Engine (accessible exclusively to OSMOO)
        let swarm_security = voxy_trinity_swarm::SwarmSecurityBoundary::new();
        let swarm_bridge = voxy_trinity_swarm::SwarmProviderBridge::new(
            model_router.clone(),
            swarm_security.clone(),
        );
        let swarm_coordinator = Arc::new(voxy_trinity_swarm::SwarmCoordinator::new(
            swarm_bridge,
            swarm_security,
            ".",
        ));
        tool_registry.register_sync(Arc::new(
            voxy_trinity_swarm::OsmooSwarmOrchestratorTool::new(swarm_coordinator.clone()),
        ));
        tool_registry.register_sync(Arc::new(voxy_trinity_swarm::OsmooSwarmStatusTool::new(
            swarm_coordinator,
        )));

        let dispatcher = Self {
            bridge,
            default_conversation_id: Arc::new(RwLock::new(None)),
            model_router,
            tool_registry,
        };
        let _ = GLOBAL_DISPATCHER.set(dispatcher.clone());
        dispatcher
    }

    pub fn with_router_and_tools(
        bridge: AppBridge,
        model_router: Arc<ModelRouter>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        let dispatcher = Self {
            bridge,
            default_conversation_id: Arc::new(RwLock::new(None)),
            model_router,
            tool_registry,
        };
        let _ = GLOBAL_DISPATCHER.set(dispatcher.clone());
        dispatcher
    }

    pub fn model_router(&self) -> &Arc<ModelRouter> {
        &self.model_router
    }

    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }

    pub fn create_default_model_router(settings_provider: &str) -> Arc<ModelRouter> {
        Self::create_default_model_router_with_keys(settings_provider, None)
    }

    pub fn create_default_model_router_with_keys(
        settings_provider: &str,
        provider_keys: Option<&HashMap<String, String>>,
    ) -> Arc<ModelRouter> {
        let registry = Arc::new(DefaultProviderRegistry::new());
        let router = Arc::new(ModelRouter::new(registry.clone()));

        // 1. Ollama (Local)
        let ollama = Arc::new(voxy_ollama::OllamaProvider::default());
        let ollama_info = ProviderInfo {
            id: "ollama".into(),
            name: "Ollama (Local)".into(),
            kind: ProviderKind::Local,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![],
            health: ProviderHealth::new_healthy(None),
            base_url: Some("http://127.0.0.1:11434".into()),
            priority: 1,
        };
        registry.register_sync(ollama_info);
        router.register_llm("ollama", ollama);

        // 2. OpenAI Cloud
        let openai_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| provider_keys.and_then(|k| k.get("openai").cloned()))
            .filter(|k| !k.trim().is_empty() && !k.chars().all(|c| c == '•' || c == '*'));
        if let Some(key) = openai_key {
            let config = voxy_openai::OpenAIConfig {
                api_key: key.into(),
                ..Default::default()
            };
            let openai = Arc::new(voxy_openai::OpenAIProvider::new(config));
            let openai_info = ProviderInfo {
                id: "openai".into(),
                name: "OpenAI Cloud".into(),
                kind: ProviderKind::Cloud,
                capability: ProviderCapability::Llm,
                status: ProviderStatus::Available,
                models: vec![],
                health: ProviderHealth::new_healthy(None),
                base_url: None,
                priority: 2,
            };
            registry.register_sync(openai_info);
            router.register_llm("openai", openai);
        }

        // 3. Anthropic Claude
        let anthropic_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| provider_keys.and_then(|k| k.get("anthropic").cloned()))
            .filter(|k| !k.trim().is_empty() && !k.chars().all(|c| c == '•' || c == '*'));
        if let Some(key) = anthropic_key {
            let anthropic = Arc::new(voxy_anthropic::AnthropicProvider::new(key));
            let anthropic_info = ProviderInfo {
                id: "anthropic".into(),
                name: "Anthropic Claude".into(),
                kind: ProviderKind::Cloud,
                capability: ProviderCapability::Llm,
                status: ProviderStatus::Available,
                models: vec![],
                health: ProviderHealth::new_healthy(None),
                base_url: None,
                priority: 3,
            };
            registry.register_sync(anthropic_info);
            router.register_llm("anthropic", anthropic);
        }

        // 4. Gemini Cloud
        let gemini_key = std::env::var("GEMINI_API_KEY")
            .ok()
            .filter(|k| !k.trim().is_empty())
            .or_else(|| provider_keys.and_then(|k| k.get("gemini").cloned()))
            .filter(|k| !k.trim().is_empty() && !k.chars().all(|c| c == '•' || c == '*'));
        if let Some(key) = gemini_key {
            let gemini = Arc::new(voxy_gemini::GeminiProvider::new(key));
            let gemini_info = ProviderInfo {
                id: "gemini".into(),
                name: "Google Gemini".into(),
                kind: ProviderKind::Cloud,
                capability: ProviderCapability::Llm,
                status: ProviderStatus::Available,
                models: vec![],
                health: ProviderHealth::new_healthy(None),
                base_url: None,
                priority: 4,
            };
            registry.register_sync(gemini_info);
            router.register_llm("gemini", gemini);
        }

        let mode = match settings_provider.to_lowercase().as_str() {
            "local" | "ollama" => RoutingMode::ForceProvider("ollama".into()),
            "openai" => RoutingMode::ForceProvider("openai".into()),
            "anthropic" | "claude" => RoutingMode::ForceProvider("anthropic".into()),
            "gemini" => RoutingMode::ForceProvider("gemini".into()),
            _ => RoutingMode::Auto,
        };
        router.set_mode(mode);
        router
    }

    pub fn register_cloud_providers(&self, keys: &HashMap<String, String>) {
        if let Some(key) = keys
            .get("openai")
            .filter(|k| !k.trim().is_empty() && !k.chars().all(|c| c == '•' || c == '*'))
        {
            let config = voxy_openai::OpenAIConfig {
                api_key: key.clone().into(),
                ..Default::default()
            };
            let openai = Arc::new(voxy_openai::OpenAIProvider::new(config));
            self.model_router.register_llm("openai", openai);
        }
        if let Some(key) = keys
            .get("anthropic")
            .filter(|k| !k.trim().is_empty() && !k.chars().all(|c| c == '•' || c == '*'))
        {
            let anthropic = Arc::new(voxy_anthropic::AnthropicProvider::new(key.clone()));
            self.model_router.register_llm("anthropic", anthropic);
        }
        if let Some(key) = keys
            .get("gemini")
            .filter(|k| !k.trim().is_empty() && !k.chars().all(|c| c == '•' || c == '*'))
        {
            let gemini = Arc::new(voxy_gemini::GeminiProvider::new(key.clone()));
            self.model_router.register_llm("gemini", gemini);
        }
    }

    pub fn get_global() -> Option<BackendDispatcher> {
        GLOBAL_DISPATCHER
            .get()
            .cloned()
            .or_else(|| AppBridge::get_global().map(Self::new))
    }

    /// Helper to construct an isolated dispatcher for integration tests.
    pub async fn create_for_test(test_dir: &Path) -> Self {
        use voxy_memory::MemoryApi;

        let event_bus = Arc::new(voxy_event_bus::EventBus::new(64));
        let settings = Arc::new(
            voxy_desktop_runtime::settings::SettingsManager::with_path(
                test_dir.join("settings.toml"),
            )
            .expect("test SettingsManager"),
        );

        let cfg_file = test_dir.join("config.toml");
        let def_cfg = voxy_config::AppConfig::default();
        let _ = def_cfg.save_to_file(&cfg_file);

        let config_manager = Arc::new(
            voxy_config::ConfigManager::new(voxy_config::FileConfigProvider::new(cfg_file))
                .await
                .expect("test ConfigManager"),
        );

        let cognition: Arc<dyn voxy_cognition::CognitiveEngine> = Arc::new(
            voxy_cognition::InMemoryCognitiveEngine::new(voxy_cognition::CognitionConfig::default()),
        );

        let memory: Arc<dyn voxy_memory::MemoryApi> = {
            let engine = voxy_memory::SqliteMemoryEngine::with_path(test_dir.join("memory.db"));
            let mem_config = voxy_memory::MemoryConfig::default();
            let _ = engine.init(&mem_config).await;
            Arc::new(engine)
        };

        struct DummyPersonality;
        #[async_trait::async_trait]
        impl voxy_personality::PersonalityManager for DummyPersonality {
            async fn load_profile(
                &self,
                _id: &str,
            ) -> voxy_personality::Result<Box<dyn voxy_personality::PersonalityProfile>>
            {
                Ok(Box::new(DummyProfile))
            }
            async fn save_profile(
                &self,
                _p: Box<dyn voxy_personality::PersonalityProfile>,
            ) -> voxy_personality::Result<()> {
                Ok(())
            }
            async fn list_profiles(&self) -> voxy_personality::Result<Vec<String>> {
                Ok(vec!["default".to_string()])
            }
            async fn delete_profile(&self, _id: &str) -> voxy_personality::Result<()> {
                Ok(())
            }
            async fn default_profile(
                &self,
            ) -> voxy_personality::Result<Box<dyn voxy_personality::PersonalityProfile>>
            {
                Ok(Box::new(DummyProfile))
            }
        }
        struct DummyProfile;
        impl voxy_personality::PersonalityProfile for DummyProfile {
            fn profile_id(&self) -> &str {
                "default"
            }
            fn profile_name(&self) -> &str {
                "OSMOO Core"
            }
            fn get_trait(&self, _n: &str) -> Option<f64> {
                None
            }
            fn set_trait(&mut self, _n: &str, _v: f64) -> voxy_personality::Result<()> {
                Ok(())
            }
            fn all_traits(&self) -> std::collections::HashMap<String, f64> {
                std::collections::HashMap::new()
            }
            fn mood(&self) -> voxy_personality::MoodState {
                voxy_personality::MoodState::Neutral
            }
            fn set_mood(&mut self, _m: voxy_personality::MoodState) {}
            fn communication_style(&self) -> voxy_personality::CommunicationStyle {
                voxy_personality::CommunicationStyle::Professional
            }
            fn set_communication_style(&mut self, _s: voxy_personality::CommunicationStyle) {}
        }

        let personality: Arc<dyn voxy_personality::PersonalityManager> = Arc::new(DummyPersonality);

        let security = Arc::new(voxy_security::GuardianEngine::new(
            voxy_security::CapabilityRegistry::new(),
            voxy_security::PolicyEngine::new(),
            voxy_security::GuardianConfig::default(),
        ));

        let plugins = Arc::new(voxy_plugin_runtime::PluginManager::new());
        let health = Arc::new(voxy_health::HealthMonitor::new(5000));
        let downloads = Arc::new(
            voxy_desktop_runtime::download::DownloadManager::new(
                2,
                Some(test_dir.to_str().unwrap()),
            )
            .expect("downloads"),
        );
        let notifications = Arc::new(
            voxy_desktop_runtime::notifications::NotificationManager::new().expect("notifications"),
        );

        let conv_db_path = test_dir.join("conversations.db");
        let conversations: Arc<dyn ConversationStore> = Arc::new(
            voxy_database::SqliteConversationStore::new(conv_db_path.to_str().unwrap())
                .expect("conversations"),
        );

        let audit_db_path = test_dir.join("audit.db");
        let audit_log: Arc<dyn voxy_database::AuditLogStore> = Arc::new(
            voxy_database::SqliteAuditLogStore::new(audit_db_path.to_str().unwrap())
                .expect("audit_log"),
        );

        let (exp_bridge, exp_input_tx, _exp_output_rx) =
            voxy_companion_intelligence::ExperienceBridge::new(
                voxy_companion_intelligence::IntelligenceConfig::default(),
            );
        let exp_bridge = Arc::new(exp_bridge);

        let ipc = Arc::new(crate::bridge::IpcBridge::new());

        let commercial_db_path = test_dir.join("commercial.db");
        let commercial_sqlite = Arc::new(voxy_database::SqliteDatabase::new());
        let db_cfg = voxy_database::DatabaseConfig {
            kind: voxy_database::DatabaseKind::Sqlite,
            path: Some(commercial_db_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        use voxy_database::StorageProvider;
        commercial_sqlite
            .connect(&db_cfg)
            .await
            .expect("commercial db");
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
        let auth = Arc::new(crate::bridge::AuthBridge::new(api_router));
        let window_tracker = Arc::new(voxy_desktop_runtime::window_manager::WindowTracker::new());

        let bridge = AppBridge::new(
            event_bus,
            settings,
            config_manager,
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
        );

        Self::new(bridge)
    }

    /// Ensures an active conversation session exists in SQLite.
    pub async fn ensure_active_conversation(&self) -> Result<String, BackendError> {
        let existing = self.default_conversation_id.read().clone();
        if let Some(id) = existing {
            return Ok(id);
        }

        // Try to query existing active conversation for user
        let user_id = self
            .bridge
            .auth
            .current_session()
            .map(|s| s.user_id)
            .unwrap_or_else(|| "local_operator".to_string());

        let list = self
            .bridge
            .conversations
            .list_conversations(&user_id, 1, 0)
            .await
            .map_err(BackendError::Storage)?;

        if let Some(first) = list.into_iter().next() {
            *self.default_conversation_id.write() = Some(first.id.clone());
            return Ok(first.id);
        }

        // Otherwise create new conversation
        let new_conv = self
            .bridge
            .conversations
            .create_conversation(&user_id, Some("OSMOO Primary Session"))
            .await
            .map_err(BackendError::Storage)?;

        *self.default_conversation_id.write() = Some(new_conv.id.clone());
        Ok(new_conv.id)
    }

    /// Dispatches a typed command, executing business logic and persisting state.
    pub async fn dispatch(&self, cmd: BackendCommand) -> Result<BackendResult, BackendError> {
        match cmd {
            BackendCommand::LoadSettings => {
                let snapshot = self.bridge.settings.get();
                let session = self.bridge.auth.current_session();
                let email = if !snapshot.profile.user_email.trim().is_empty() {
                    snapshot.profile.user_email.as_str()
                } else {
                    session
                        .as_ref()
                        .map(|s| s.email.as_str())
                        .unwrap_or("operator@osmoo.in")
                };
                let tier = session
                    .as_ref()
                    .map(|s| s.active_tier.as_str())
                    .unwrap_or("free");
                let form = SettingsFormState::from_snapshot(&snapshot, email, tier);
                Ok(BackendResult::SettingsLoaded(form))
            }

            BackendCommand::SaveSettings(form) => {
                // Input validation
                if form.assistant_name.trim().is_empty() {
                    return Err(BackendError::Validation(
                        "Assistant designation must not be empty".to_string(),
                    ));
                }
                if form.wake_words.len() > 10 {
                    return Err(BackendError::Validation(
                        "Wake words count exceeds the maximum limit of 10".to_string(),
                    ));
                }
                if form.companion_size < 0.2 || form.companion_size > 3.0 {
                    return Err(BackendError::Validation(
                        "Companion scale must be between 0.2 and 3.0".to_string(),
                    ));
                }
                if form.speech_speed < 0.25 || form.speech_speed > 3.0 {
                    return Err(BackendError::Validation(
                        "Speech speed must be between 0.25 and 3.0".to_string(),
                    ));
                }
                if form.speech_pitch < 0.25 || form.speech_pitch > 3.0 {
                    return Err(BackendError::Validation(
                        "Speech pitch must be between 0.25 and 3.0".to_string(),
                    ));
                }
                if form.max_concurrent_tasks == 0 || form.max_concurrent_tasks > 10 {
                    return Err(BackendError::Validation(
                        "Concurrent tasks limit must be between 1 and 10".to_string(),
                    ));
                }
                if !form.user_email.contains('@') {
                    return Err(BackendError::Validation(
                        "Invalid operator email address".to_string(),
                    ));
                }

                let mut current = self.bridge.settings.get();
                current.app_name = form.assistant_name;

                // 1. Profile & General
                current.profile.user_name = form.user_name;
                current.profile.user_email = form.user_email;
                current.profile.organization = form.organization;
                current.profile.role = form.role;
                current.profile.language = form.language;
                current.profile.close_behavior = form.close_behavior;
                current.appearance.theme = form.theme;
                current.appearance.start_with_windows = form.autolaunch;
                current.updates.auto_update = form.auto_update;
                current.updates.channel = form.update_channel;

                // 2. Models & Providers
                current.models.provider = form.primary_llm.clone();
                current.models.model = form.ollama_model;
                current.models.coding_model = form.coding_model;
                current.models.voice_model = form.voice_model;
                current.models.embedding_model = form.embedding_model;
                current.models.enable_fallback = form.enable_fallback;
                for (k, v) in form.provider_keys {
                    if !v.trim().is_empty() && v != "••••••••" {
                        current.models.provider_keys.insert(k, v);
                    }
                }

                // 3. Voice & Audio
                current.voice.enabled = form.voice_enabled;
                current.voice.wake_words = form.wake_words;
                if let Some(first) = current.voice.wake_words.first() {
                    current.voice.wake_word = first.clone();
                }
                current.voice.stt_provider = Some(form.stt_provider);
                current.voice.tts_provider = Some(form.tts_provider);
                current.voice.voice_name = Some(form.voice_name);
                current.voice.speech_speed = form.speech_speed;
                current.voice.speech_pitch = form.speech_pitch;
                current.voice.interruption_barge_in = form.interruption_barge_in;
                current.voice.noise_suppression = form.noise_suppression;
                current.voice.echo_cancellation = form.echo_cancellation;
                current.voice.sample_rate = form.audio_sample_rate;
                current.voice.wake_word_sensitivity = form.vad_sensitivity as f64;
                current.voice.input_device = if form.input_device.trim().is_empty()
                    || form.input_device.starts_with("Default")
                {
                    None
                } else {
                    Some(form.input_device)
                };
                current.voice.output_device = if form.output_device.trim().is_empty()
                    || form.output_device.starts_with("Default")
                {
                    None
                } else {
                    Some(form.output_device)
                };

                // 4. Companion & Viewport
                current.companion.character_id = form.companion_character_id;
                current.companion.character_size = form.companion_size;
                current.companion.position_preset = form.companion_position;
                current.companion.expression = form.companion_expression;
                current.companion.lip_sync = form.companion_lip_sync;
                current.companion.animations_enabled = form.companion_animations;
                current.companion.draggable = form.companion_draggable;
                current.companion.click_through = form.companion_click_through;
                current.companion.desktop_overlay_mode = if form.desktop_overlay_mode {
                    "overlay".to_string()
                } else {
                    "windowed".to_string()
                };

                // 5. Security & Permissions
                current.security.file_system_access = form.file_system_access;
                current.security.browser_automation = form.browser_automation;
                current.security.shell_execution = form.shell_execution;
                current.security.require_approval_sensitive = form.require_approval_sensitive;
                current.security.require_approval_financial = form.require_approval_financial;
                current.security.require_approval_email = form.require_approval_email;
                current.security.require_approval_scripts = form.require_approval_scripts;
                current.security.audit_logging_enabled = form.audit_logging_enabled;
                current.security.emergency_stopped = form.emergency_stopped;

                // 6. Memory & Personalization
                current.memory.enabled = form.memory_enabled;
                current.memory.retention_days = form.memory_retention_days;
                current.memory.auto_consolidate = form.auto_consolidate;
                current.memory.private_session = form.private_session;
                current.memory.user_persona_notes = form.user_persona_notes;

                // 7. Automation & Autonomy
                current.automation.background_tasks_allowed = form.background_tasks_allowed;
                current.automation.pause_on_battery = form.pause_on_battery;
                current.automation.max_concurrent_tasks = form.max_concurrent_tasks;
                current.automation.timeout_seconds = form.task_timeout_seconds;
                current.automation.action_risk_threshold = form.action_risk_threshold;
                current.automation.global_tasks_paused = form.global_tasks_paused;

                // 8. Performance & Hardware
                current.performance.max_memory_mb = form.max_memory_mb;
                current.performance.max_cpu_percent = form.max_cpu_percent;
                current.performance.gpu_acceleration = form.gpu_acceleration;
                current.performance.model_unload_timeout_minutes =
                    form.model_unload_timeout_minutes;

                self.register_cloud_providers(&current.models.provider_keys);

                // Strict model routing policy:
                // When fallback is disabled, force the specified provider if available, otherwise Auto
                let router_mode = if !form.enable_fallback {
                    match form.primary_llm.to_lowercase().as_str() {
                        "local" | "ollama" => RoutingMode::ForceProvider("ollama".into()),
                        "openai"
                            if current.models.provider_keys.contains_key("openai")
                                || std::env::var("OPENAI_API_KEY")
                                    .map(|v| !v.trim().is_empty())
                                    .unwrap_or(false) =>
                        {
                            RoutingMode::ForceProvider("openai".into())
                        }
                        "anthropic" | "claude"
                            if current.models.provider_keys.contains_key("anthropic")
                                || std::env::var("ANTHROPIC_API_KEY")
                                    .map(|v| !v.trim().is_empty())
                                    .unwrap_or(false) =>
                        {
                            RoutingMode::ForceProvider("anthropic".into())
                        }
                        "gemini"
                            if current.models.provider_keys.contains_key("gemini")
                                || std::env::var("GEMINI_API_KEY")
                                    .map(|v| !v.trim().is_empty())
                                    .unwrap_or(false) =>
                        {
                            RoutingMode::ForceProvider("gemini".into())
                        }
                        _ => RoutingMode::Auto,
                    }
                } else {
                    RoutingMode::Auto
                };
                self.model_router.set_mode(router_mode);

                self.bridge
                    .settings
                    .update(current)
                    .map_err(|e| BackendError::Settings(e.to_string()))?;

                // Notify IPC daemon of routing change if connected
                let _ = self
                    .bridge
                    .ipc
                    .send_command(ClientCommand::SetRoutingMode {
                        mode: form.primary_llm,
                    })
                    .await;

                Ok(BackendResult::SettingsSaved)
            }

            BackendCommand::SendPrompt { text } => {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return Err(BackendError::Validation(
                        "Prompt text cannot be empty".to_string(),
                    ));
                }

                let conv_id = self.ensure_active_conversation().await?;

                // 1. Persist user turn in SQLite
                self.bridge
                    .conversations
                    .add_message(&conv_id, MessageRole::User, trimmed, None, None)
                    .await
                    .map_err(BackendError::Storage)?;

                // 2. Check for native tool execution intent first
                let assistant_text = if let Some(tool_output) =
                    self.try_execute_native_tool(trimmed).await?
                {
                    tool_output
                } else {
                    // 3. AI Reasoning via ModelRouter with Memory Context
                    let mut augmented_prompt = trimmed.to_string();
                    let query = voxy_memory::MemoryQuery {
                        query_text: Some(trimmed.to_string()),
                        memory_types: None,
                        states: None,
                        tags: None,
                        time_range: None,
                        min_importance: Some(0.3),
                        max_results: 3,
                        include_embeddings: false,
                        source_filter: None,
                    };
                    if let Ok(results) = self.bridge.memory.search(&query).await {
                        if !results.is_empty() {
                            let context_lines: Vec<String> = results
                                .into_iter()
                                .map(|r| r.item.content.to_string())
                                .collect();
                            augmented_prompt = format!(
                                "Context from persistent memory:\n{}\n\nUser request: {}",
                                context_lines.join("\n"),
                                trimmed
                            );
                        }
                    }

                    match self.model_router.route_complete(&augmented_prompt).await {
                        Ok(reply) if !reply.trim().is_empty() => reply,
                        Ok(_) | Err(_) => {
                            // When LLM cannot be reached or returns empty, invoke cognitive engine or smart operational guide
                            let intent_input = IntentInput {
                                raw_text: trimmed.to_string(),
                                context: None,
                                source: "desktop_hud".to_string(),
                                metadata: std::collections::HashMap::new(),
                            };

                            match self.bridge.cognition.process(&intent_input).await {
                                Ok(cog_res) => {
                                    if let Some(out_str) = cog_res.result.as_str() {
                                        out_str.to_string()
                                    } else if !cog_res.result.is_null()
                                        && cog_res.result
                                            != serde_json::json!({"status": "processed"})
                                    {
                                        cog_res.result.to_string()
                                    } else {
                                        format!(
                                            "OSMOO Core: Understood query '{}' (Intent: {}).\n\n\
                                            Note: No active LLM endpoint (Ollama local / OpenAI / Anthropic / Gemini) could complete this query.\n\
                                            • For local AI: Ensure Ollama is running (`ollama run llama3.2:3b`).\n\
                                            • For cloud AI: Set OPENAI_API_KEY, ANTHROPIC_API_KEY, or GEMINI_API_KEY in Settings.\n\
                                            • Desktop tools: You can run 'system info', 'list windows', or 'list processes' directly.",
                                            trimmed,
                                            cog_res.intent.primary_action
                                        )
                                    }
                                }
                                Err(_) => {
                                    format!(
                                        "OSMOO Core: No active LLM endpoint responded for query '{}'.\n\
                                        Ensure Ollama is running on localhost:11434 or configure cloud provider credentials in Settings.",
                                        trimmed
                                    )
                                }
                            }
                        }
                    }
                };

                // 4. Persist assistant turn in SQLite
                self.bridge
                    .conversations
                    .add_message(
                        &conv_id,
                        MessageRole::Assistant,
                        &assistant_text,
                        None,
                        None,
                    )
                    .await
                    .map_err(BackendError::Storage)?;

                // 5. Store turn into persistent episodic memory
                let memory_item = voxy_memory::MemoryItem {
                    id: voxy_memory::MemoryId(uuid::Uuid::new_v4().to_string()),
                    memory_type: voxy_memory::MemoryType::Episodic,
                    state: voxy_memory::MemoryState::Active,
                    content: serde_json::json!({
                        "user": trimmed,
                        "assistant": assistant_text,
                    }),
                    importance: 0.5,
                    timestamp: chrono::Utc::now(),
                    last_accessed: chrono::Utc::now(),
                    access_count: 1,
                    context_tags: vec!["conversation".to_string(), "hud".to_string()],
                    source: "desktop_hud".to_string(),
                    version: 1,
                    ttl: None,
                    metadata: std::collections::HashMap::new(),
                    embedding: None,
                    parent_id: None,
                    related_ids: vec![],
                };
                let _ = self.bridge.memory.store(memory_item).await;

                // 6. Also forward over IPC if daemon active
                let _ = self
                    .bridge
                    .ipc
                    .send_command(ClientCommand::SendTextInput {
                        text: trimmed.to_string(),
                    })
                    .await;

                Ok(BackendResult::PromptResponse {
                    text: assistant_text,
                    conversation_id: conv_id,
                })
            }

            BackendCommand::EmergencyStop => {
                // Cancel active cognitive goals if in progress
                if let Some(intent_id) = self.bridge.cognition.current_intent().await {
                    let _ = self.bridge.cognition.cancel(&intent_id).await;
                }
                let _ = self.bridge.cognition.pause().await;

                // Transmit IPC EmergencyStop
                self.bridge
                    .ipc
                    .send_command(ClientCommand::EmergencyStop)
                    .await
                    .map_err(|e| BackendError::Ipc(e.to_string()))?;

                Ok(BackendResult::EmergencyStopTriggered)
            }

            BackendCommand::GetChatHistory { limit, offset } => {
                let conv_id = self.ensure_active_conversation().await?;
                let messages = self
                    .bridge
                    .conversations
                    .get_messages(&conv_id, limit, offset)
                    .await
                    .map_err(BackendError::Storage)?;

                let mut turns = Vec::new();
                for (idx, m) in messages.into_iter().enumerate() {
                    let is_user = m.role == MessageRole::User;
                    let sender = if is_user {
                        "Operator".to_string()
                    } else {
                        "OSMOO Core".to_string()
                    };
                    turns.push(ChatTurn {
                        id: (idx + 1) as u64,
                        sender,
                        text: m.content,
                        timestamp: m.timestamp.format("%H:%M:%S").to_string(),
                        is_user,
                    });
                }

                Ok(BackendResult::ChatHistory(turns))
            }

            BackendCommand::GetSystemTelemetry => {
                let daemon_conn = *self.bridge.ipc.watch_connected().borrow();
                let snapshot = self.bridge.settings.get();

                let mut sys = sysinfo::System::new();
                sys.refresh_memory();
                sys.refresh_cpu_all();
                let cpu_usage_pct = sys.global_cpu_usage();
                let memory_used_mb = sys.used_memory() / (1024 * 1024);

                Ok(BackendResult::SystemTelemetry {
                    daemon_connected: daemon_conn,
                    active_llm: snapshot.models.provider,
                    cpu_usage_pct,
                    memory_used_mb,
                })
            }

            BackendCommand::CompleteOnboarding(cfg) => {
                let mut current = self.bridge.settings.get();
                current.app_name = cfg.assistant_name;
                current.onboarded = true;
                current.models.provider = cfg.primary_llm.clone();
                current.models.model = cfg.selected_model.clone();
                if let Some(ref key) = cfg.api_key {
                    let trimmed = key.trim();
                    if !trimmed.is_empty() {
                        current
                            .models
                            .provider_keys
                            .insert(cfg.primary_llm.to_lowercase(), trimmed.to_string());
                    }
                }
                current.voice.stt_provider = Some(cfg.stt_provider);
                current.voice.tts_provider = Some(cfg.tts_provider);
                current.voice.always_listening = cfg.wake_word_enabled;
                current.companion.character_size = cfg.companion_scale as f64;
                current.companion.position_preset = cfg.companion_position;
                current.appearance.start_with_windows = cfg.start_on_boot;
                current.companion.animations_enabled = cfg.autowalk_enabled;

                self.register_cloud_providers(&current.models.provider_keys);
                let router_mode = match cfg.primary_llm.to_lowercase().as_str() {
                    "local" | "ollama" => RoutingMode::ForceProvider("ollama".into()),
                    "openai" => RoutingMode::ForceProvider("openai".into()),
                    "anthropic" | "claude" => RoutingMode::ForceProvider("anthropic".into()),
                    "gemini" => RoutingMode::ForceProvider("gemini".into()),
                    _ => RoutingMode::Auto,
                };
                self.model_router.set_mode(router_mode);

                self.bridge
                    .settings
                    .update(current)
                    .map_err(|e| BackendError::Settings(e.to_string()))?;

                // Create provisioned flag in config dir
                let cfg_dir = dirs::config_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("osmoo");
                let _ = std::fs::create_dir_all(&cfg_dir);
                let _ = std::fs::write(cfg_dir.join("provisioned.flag"), "provisioned");

                // Initialize default conversation
                let conv_id = self.ensure_active_conversation().await?;
                let _ = self
                    .bridge
                    .conversations
                    .add_message(
                        &conv_id,
                        MessageRole::Assistant,
                        "OSMOO computational architecture initialized and operational.",
                        None,
                        None,
                    )
                    .await;

                Ok(BackendResult::OnboardingCompleted)
            }

            BackendCommand::TestProviderConnection {
                provider,
                model,
                api_key,
            } => {
                let start = std::time::Instant::now();
                let lower = provider.to_lowercase();
                if lower == "ollama" || lower == "local" {
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_millis(2500))
                        .build()
                        .unwrap_or_default();
                    match client.get("http://127.0.0.1:11434/api/tags").send().await {
                        Ok(resp) if resp.status().is_success() => {
                            let latency_ms = start.elapsed().as_millis() as u64;
                            Ok(BackendResult::ProviderTested {
                                success: true,
                                message: format!("Ollama daemon responsive. Target model: {model}"),
                                latency_ms,
                            })
                        }
                        Ok(resp) => {
                            let latency_ms = start.elapsed().as_millis() as u64;
                            Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Ollama returned HTTP status {}", resp.status()),
                                latency_ms,
                            })
                        }
                        Err(e) => {
                            let latency_ms = start.elapsed().as_millis() as u64;
                            Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Could not reach Ollama at 127.0.0.1:11434: {e}"),
                                latency_ms,
                            })
                        }
                    }
                } else {
                    let snapshot = self.bridge.settings.get();
                    let effective_key = api_key
                        .filter(|k| {
                            !k.trim().is_empty() && !k.chars().all(|c| c == '•' || c == '*')
                        })
                        .or_else(|| match lower.as_str() {
                            "openai" => std::env::var("OPENAI_API_KEY").ok(),
                            "anthropic" => std::env::var("ANTHROPIC_API_KEY").ok(),
                            "gemini" => std::env::var("GEMINI_API_KEY").ok(),
                            "groq" => std::env::var("GROQ_API_KEY").ok(),
                            _ => None,
                        })
                        .or_else(|| snapshot.models.provider_keys.get(&lower).cloned());

                    let latency_ms = start.elapsed().as_millis() as u64;
                    if let Some(key) = effective_key {
                        let trimmed = key.trim();
                        if trimmed.is_empty() {
                            return Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Missing API key for provider '{provider}'."),
                                latency_ms,
                            });
                        }

                        // Validate format
                        let is_valid_format = match lower.as_str() {
                            "openai" => trimmed.starts_with("sk-"),
                            "anthropic" => trimmed.starts_with("sk-ant-"),
                            "gemini" => trimmed.starts_with("AIza") || trimmed.len() >= 20,
                            "groq" => trimmed.starts_with("gsk_") || trimmed.len() >= 20,
                            _ => trimmed.len() >= 10,
                        };

                        if !is_valid_format {
                            return Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Invalid API key format for {}. Key prefix does not match expected provider signature.", provider),
                                latency_ms,
                            });
                        }

                        // Try live HTTP ping to provider
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_millis(3500))
                            .build()
                            .unwrap_or_default();

                        let ping_res = match lower.as_str() {
                            "openai" => {
                                client
                                    .get("https://api.openai.com/v1/models")
                                    .header("Authorization", format!("Bearer {trimmed}"))
                                    .send()
                                    .await
                            }
                            "anthropic" => {
                                client
                                    .get("https://api.anthropic.com/v1/models")
                                    .header("x-api-key", trimmed)
                                    .header("anthropic-version", "2023-06-01")
                                    .send()
                                    .await
                            }
                            "groq" => {
                                client
                                    .get("https://api.groq.com/openai/v1/models")
                                    .header("Authorization", format!("Bearer {trimmed}"))
                                    .send()
                                    .await
                            }
                            _ => {
                                let latency = start.elapsed().as_millis() as u64;
                                return Ok(BackendResult::ProviderTested {
                                    success: true,
                                    message: format!("Provider '{provider}' format validated (latency: {latency}ms)."),
                                    latency_ms: latency,
                                });
                            }
                        };

                        let latency = start.elapsed().as_millis() as u64;
                        match ping_res {
                            Ok(r) if r.status().is_success() => Ok(BackendResult::ProviderTested {
                                success: true,
                                message: format!("Connection confirmed to {provider} ({latency}ms). Credentials valid."),
                                latency_ms: latency,
                            }),
                            Ok(r) if r.status() == 401 || r.status() == 403 => {
                                Ok(BackendResult::ProviderTested {
                                    success: false,
                                    message: format!("Authentication failed (HTTP {}): API key is rejected by {provider}.", r.status()),
                                    latency_ms: latency,
                                })
                            }
                            Ok(r) if r.status().is_server_error() => Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Provider server error (HTTP {}): service temporarily unavailable.", r.status()),
                                latency_ms: latency,
                            }),
                            Ok(r) if r.status() == 429 => Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Rate limited (HTTP 429): Quota exceeded for {provider}."),
                                latency_ms: latency,
                            }),
                            Ok(r) => Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Provider returned unexpected status (HTTP {}).", r.status()),
                                latency_ms: latency,
                            }),
                            Err(e) => Ok(BackendResult::ProviderTested {
                                success: false,
                                message: format!("Network error reaching {provider}: {e}"),
                                latency_ms: latency,
                            }),
                        }
                    } else {
                        Ok(BackendResult::ProviderTested {
                            success: false,
                            message: format!("Provider '{provider}' API key not configured."),
                            latency_ms,
                        })
                    }
                }
            }

            BackendCommand::ExportDiagnostics => {
                let snapshot = self.bridge.settings.get();
                let mut sys = sysinfo::System::new();
                sys.refresh_memory();
                sys.refresh_cpu_all();
                let cpu_pct = sys.global_cpu_usage();
                let mem_mb = sys.used_memory() / (1024 * 1024);
                let total_mem_mb = sys.total_memory() / (1024 * 1024);

                let report = format!(
                    "=== OSMOO SANITIZED SYSTEM DIAGNOSTICS ===\n                    Timestamp: {}\n                    OSMOO Version: v0.1.0-commercial\n                    Build Channel: {}\n                    Host Platform: {} (Arch: {})\n                    CPU Usage: {:.1}%\n                    Memory Usage: {} MB / {} MB\n                    \n                    --- Active Routing & Providers ---\n                    Primary Provider: {}\n                    Strict Routing: {}\n                    Reasoning Model: {}\n                    Coding Model: {}\n                    Voice Model: {}\n                    Embedding Model: {}\n                    STT Engine: {}\n                    TTS Engine: {}\n                    \n                    --- Companion & Viewport ---\n                    Avatar Mesh: {}\n                    Scale: {:.2}x\n                    Position Preset: {}\n                    Overlay Mode: {}\n                    \n                    --- Security Policies ---\n                    Filesystem Access: {}\n                    Browser Automation: {}\n                    Shell Execution Gate: {}\n                    Audit Logging Active: {}\n                    Emergency Stopped: {}\n                    \n                    --- Memory & Automation Subsystems ---\n                    Memory Retain Days: {}\n                    Private Session: {}\n                    Max Concurrent Tasks: {}\n                    Task Timeout: {}s\n                    Risk Threshold: {}\n                    Global Tasks Paused: {}\n                    \n                    [DPAPI REDACTED: All API keys, authorization tokens, and personal operator secrets masked.]",
                    chrono::Utc::now().to_rfc3339(),
                    snapshot.updates.channel,
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                    cpu_pct,
                    mem_mb,
                    total_mem_mb,
                    snapshot.models.provider,
                    if snapshot.models.enable_fallback { "Disabled (Fallback Allowed)" } else { "Active (Zero Silent Fallback)" },
                    snapshot.models.model,
                    snapshot.models.coding_model,
                    snapshot.models.voice_model,
                    snapshot.models.embedding_model,
                    snapshot.voice.stt_provider.as_deref().unwrap_or("none"),
                    snapshot.voice.tts_provider.as_deref().unwrap_or("none"),
                    snapshot.companion.character_id,
                    snapshot.companion.character_size,
                    snapshot.companion.position_preset,
                    snapshot.companion.desktop_overlay_mode,
                    snapshot.security.file_system_access,
                    snapshot.security.browser_automation,
                    snapshot.security.shell_execution,
                    snapshot.security.audit_logging_enabled,
                    snapshot.security.emergency_stopped,
                    snapshot.memory.retention_days,
                    snapshot.memory.private_session,
                    snapshot.automation.max_concurrent_tasks,
                    snapshot.automation.timeout_seconds,
                    snapshot.automation.action_risk_threshold,
                    snapshot.automation.global_tasks_paused,
                );

                Ok(BackendResult::DiagnosticsExported {
                    sanitized_report: report,
                })
            }

            BackendCommand::PurgeStoredMemories => {
                let stats = self
                    .bridge
                    .memory
                    .stats()
                    .await
                    .map(|s| s.total_items)
                    .unwrap_or(0);
                self.bridge
                    .memory
                    .clear()
                    .await
                    .map_err(|e| BackendError::Storage(e.to_string()))?;
                Ok(BackendResult::MemoriesPurged { count: stats })
            }
        }
    }

    /// Evaluates user queries for native Windows desktop operations and executes
    /// the corresponding registered tool if matched.
    async fn try_execute_native_tool(&self, prompt: &str) -> Result<Option<String>, BackendError> {
        let lower = prompt.trim().to_lowercase();
        let ctx = ToolContext::new("desktop_operator_hud").with_confirmation(true);

        if lower == "system info"
            || lower == "specs"
            || lower == "pc info"
            || lower == "os info"
            || lower == "telemetry"
            || lower == "system telemetry"
        {
            match self
                .tool_registry
                .execute("system_info", serde_json::json!({}), &ctx)
                .await
            {
                Ok(res) => {
                    let formatted = format!(
                        "🖥️ **OSMOO Native System Diagnostics**\n\
                        • **Operating System**: {}\n\
                        • **Version**: {}\n\
                        • **Architecture**: {}\n\
                        • **Host Name**: {}\n\
                        • **Displays Attached**: {}\n\
                        • **Execution Time**: {}ms",
                        res.data
                            .get("os")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Windows"),
                        res.data
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown"),
                        res.data
                            .get("arch")
                            .and_then(|v| v.as_str())
                            .unwrap_or("x86_64"),
                        res.data
                            .get("hostname")
                            .and_then(|v| v.as_str())
                            .unwrap_or("localhost"),
                        res.data
                            .get("displays")
                            .and_then(|v| v.as_array())
                            .map(|a| a.len())
                            .unwrap_or(1),
                        res.duration_ms
                    );
                    return Ok(Some(formatted));
                }
                Err(e) => return Ok(Some(format!("System info execution failed: {e}"))),
            }
        }

        if lower == "list windows"
            || lower == "show windows"
            || lower == "windows"
            || lower == "active windows"
        {
            match self
                .tool_registry
                .execute("window_list", serde_json::json!({}), &ctx)
                .await
            {
                Ok(res) => {
                    let windows = res.data.get("windows").and_then(|v| v.as_array());
                    let mut out = String::from("🪟 **Active Windows**:\n");
                    if let Some(list) = windows {
                        for (i, win) in list.iter().take(15).enumerate() {
                            let title = win
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("<Untitled>");
                            let class_name =
                                win.get("class_name").and_then(|v| v.as_str()).unwrap_or("");
                            out.push_str(&format!("{}. **{}** `[{}]`\n", i + 1, title, class_name));
                        }
                        if list.len() > 15 {
                            out.push_str(&format!("*... and {} more windows.*\n", list.len() - 15));
                        }
                    } else {
                        out.push_str("No active top-level windows detected.\n");
                    }
                    return Ok(Some(out));
                }
                Err(e) => return Ok(Some(format!("Window listing failed: {e}"))),
            }
        }

        if lower == "list processes"
            || lower == "processes"
            || lower == "ps"
            || lower == "top processes"
            || lower == "running processes"
        {
            match self
                .tool_registry
                .execute("process_list", serde_json::json!({}), &ctx)
                .await
            {
                Ok(res) => {
                    let procs = res.data.get("processes").and_then(|v| v.as_array());
                    let mut out = String::from("⚙️ **Running Processes (Sample)**:\n");
                    if let Some(list) = procs {
                        for (i, proc) in list.iter().take(15).enumerate() {
                            let name = proc
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let pid = proc.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                            let mem = proc
                                .get("memory_bytes")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                / (1024 * 1024);
                            out.push_str(&format!(
                                "{}. `{}` (PID: {}, Memory: {} MB)\n",
                                i + 1,
                                name,
                                pid,
                                mem
                            ));
                        }
                        if list.len() > 15 {
                            out.push_str(&format!("*Total processes running: {}*\n", list.len()));
                        }
                    }
                    return Ok(Some(out));
                }
                Err(e) => return Ok(Some(format!("Process listing failed: {e}"))),
            }
        }

        if lower == "screenshot" || lower == "take screenshot" || lower == "capture screen" {
            match self
                .tool_registry
                .execute("system_screenshot", serde_json::json!({}), &ctx)
                .await
            {
                Ok(res) => {
                    let dims = format!(
                        "{}x{}",
                        res.data.get("width").and_then(|v| v.as_u64()).unwrap_or(0),
                        res.data.get("height").and_then(|v| v.as_u64()).unwrap_or(0)
                    );
                    return Ok(Some(format!(
                        "📸 **Desktop Screenshot Captured**\n• Dimensions: {}\n• Execution: {}ms",
                        dims, res.duration_ms
                    )));
                }
                Err(e) => return Ok(Some(format!("Screenshot capture failed: {e}"))),
            }
        }

        if lower == "git status" {
            match self
                .tool_registry
                .execute("coding_git_status", serde_json::json!({}), &ctx)
                .await
            {
                Ok(res) => {
                    let branch = res
                        .data
                        .get("branch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let clean = res
                        .data
                        .get("clean")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    return Ok(Some(format!(
                        "📦 **Git Repository Status**\n• Branch: `{}`\n• Working Tree Clean: **{}**",
                        branch, clean
                    )));
                }
                Err(e) => return Ok(Some(format!("Git status failed: {e}"))),
            }
        }

        Ok(None)
    }
}

static GLOBAL_DISPATCHER: std::sync::OnceLock<BackendDispatcher> = std::sync::OnceLock::new();
