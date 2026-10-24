pub mod commands;

pub use commands::{BackendCommand, BackendError, BackendResult};

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
        let primary_llm = bridge.settings.get().models.provider;
        let model_router = Self::create_default_model_router(&primary_llm);
        let tool_registry = Arc::new(ToolRegistry::with_builtins());

        // Initialize OSMOO Sovereign Video Studio Enclave (accessible exclusively to OSMOO)
        let enclave_path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("crates/video_studio/enclave");
        let video_enclave = voxy_video_studio::VideoStudioEnclave::new(enclave_path);
        tool_registry.register_sync(Arc::new(voxy_video_studio::VideoStudioPipelineTool::new(video_enclave.clone())));
        tool_registry.register_sync(Arc::new(voxy_video_studio::VideoStudioInspectTool::new(video_enclave)));

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
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.trim().is_empty() {
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
        }

        // 3. Anthropic Claude
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.trim().is_empty() {
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
        }

        // 4. Gemini Cloud
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            if !key.trim().is_empty() {
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
                let form = SettingsFormState {
                    assistant_name: snapshot.app_name,
                    primary_llm: snapshot.models.provider,
                    ollama_model: snapshot.models.model,
                    autolaunch: true,
                    stt_provider: snapshot
                        .voice
                        .stt_provider
                        .unwrap_or_else(|| "groq".to_string()),
                    tts_provider: snapshot
                        .voice
                        .tts_provider
                        .unwrap_or_else(|| "cartesia".to_string()),
                    audio_sample_rate: snapshot.voice.sample_rate,
                    vad_sensitivity: snapshot.voice.wake_word_sensitivity as f32,
                    minimize_to_tray: true,
                    user_email: self
                        .bridge
                        .auth
                        .current_session()
                        .map(|s| s.email)
                        .unwrap_or_else(|| "operator@osmoo.in".to_string()),
                    active_tier: self
                        .bridge
                        .auth
                        .current_session()
                        .map(|s| s.active_tier)
                        .unwrap_or_else(|| "free".to_string()),
                };
                Ok(BackendResult::SettingsLoaded(form))
            }

            BackendCommand::SaveSettings(form) => {
                // Input validation
                if form.assistant_name.trim().is_empty() {
                    return Err(BackendError::Validation(
                        "Assistant designation must not be empty".to_string(),
                    ));
                }

                let mut current = self.bridge.settings.get();
                current.app_name = form.assistant_name;
                current.models.provider = form.primary_llm.clone();
                current.models.model = form.ollama_model;
                current.voice.stt_provider = Some(form.stt_provider);
                current.voice.tts_provider = Some(form.tts_provider);
                current.voice.sample_rate = form.audio_sample_rate;
                current.voice.wake_word_sensitivity = form.vad_sensitivity as f64;

                self.bridge
                    .settings
                    .update(current)
                    .map_err(|e| BackendError::Settings(e.to_string()))?;

                // Update model router mode dynamically
                let router_mode = match form.primary_llm.to_lowercase().as_str() {
                    "local" | "ollama" => RoutingMode::ForceProvider("ollama".into()),
                    "openai" => RoutingMode::ForceProvider("openai".into()),
                    "anthropic" | "claude" => RoutingMode::ForceProvider("anthropic".into()),
                    "gemini" => RoutingMode::ForceProvider("gemini".into()),
                    _ => RoutingMode::Auto,
                };
                self.model_router.set_mode(router_mode);

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
                let assistant_text = if let Some(tool_output) = self.try_execute_native_tool(trimmed).await? {
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
                                        && cog_res.result != serde_json::json!({"status": "processed"})
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
                current.models.provider = cfg.primary_llm;
                current.models.model = cfg.selected_model;
                current.voice.stt_provider = Some(cfg.stt_provider);
                current.voice.tts_provider = Some(cfg.tts_provider);

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
                        res.data.get("os").and_then(|v| v.as_str()).unwrap_or("Windows"),
                        res.data.get("version").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                        res.data.get("arch").and_then(|v| v.as_str()).unwrap_or("x86_64"),
                        res.data.get("hostname").and_then(|v| v.as_str()).unwrap_or("localhost"),
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
                            let name =
                                proc.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
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

        if lower == "screenshot"
            || lower == "take screenshot"
            || lower == "capture screen"
        {
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
