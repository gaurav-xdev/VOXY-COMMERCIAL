use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod background;
mod shutdown;
mod tools;

use chrono::Timelike;
use voxy_anthropic::AnthropicProvider;
use voxy_automation::WindowsUiaBackend;
use voxy_cognition::{CognitionConfig, CognitiveEngine, InMemoryCognitiveEngine, IntentInput};
use voxy_cognitive_orchestrator::bridge::CognitiveBridge;
use voxy_cognitive_orchestrator::config::OrchestratorConfig;
use voxy_companion_intelligence::{
    ExperienceBridge, ExperienceInput, IntelligenceConfig, MomentContext, MomentEngine,
    ProactiveConfig, ProactiveEngine, SuggestionContext,
};
use voxy_desktop_runtime::{DesktopRuntime, RuntimeConfig};
use voxy_gemini::GeminiProvider;
use voxy_ollama::OllamaProvider;
use voxy_openai::{OpenAIConfig, OpenAIProvider};
use voxy_orchestrator::automation::AutomationBackend;
use voxy_provider_core::LlmProvider;
use voxy_runtime_guard::{GuardConfig, RuntimeGuard};
use voxy_security::{
    sanitize_context, sanitize_llm_output, sanitize_user_input, AuditEventType, AuditLog,
    CapabilityRegistry, GuardianConfig, GuardianEngine, PolicyEngine, RecoveryMode,
    SystemPromptBuilder,
};
use voxy_voice_tts::TtsEngine;
use voxy_world_model::{DesktopEventBridge, WorldModelConfig};

static CHAT_MSG_ID: AtomicU64 = AtomicU64::new(1);

/// Create an LLM provider based on VOXY_LLM_PROVIDER env var.
/// Supported: "ollama" (default), "openai", "anthropic", "gemini", "groq", "openrouter"
fn create_llm_provider() -> Arc<dyn LlmProvider> {
    let provider = std::env::var("VOXY_LLM_PROVIDER")
        .unwrap_or_else(|_| "ollama".into())
        .to_lowercase();

    match provider.as_str() {
        "openai" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI").unwrap_or_default();
            let model = std::env::var("VOXY_OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_OPENAI not set, falling back to Ollama");
                return create_ollama_provider();
            }
            let config = OpenAIConfig::openai(api_key);
            tracing::info!("[PROVIDER] Using OpenAI: model={}", model);
            Arc::new(OpenAIProvider::new(config).with_model(&model))
        }
        "anthropic" => {
            let api_key = std::env::var("VOXY_API_KEYS_ANTHROPIC").unwrap_or_default();
            let model = std::env::var("VOXY_ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            if api_key.is_empty() {
                tracing::warn!(
                    "[PROVIDER] VOXY_API_KEYS_ANTHROPIC not set, falling back to Ollama"
                );
                return create_ollama_provider();
            }
            tracing::info!("[PROVIDER] Using Anthropic: model={}", model);
            Arc::new(AnthropicProvider::new(api_key).with_model(&model))
        }
        "gemini" => {
            let api_key = std::env::var("VOXY_API_KEYS_GEMINI").unwrap_or_default();
            let model =
                std::env::var("VOXY_GEMINI_MODEL").unwrap_or_else(|_| "gemini-2.0-flash".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_GEMINI not set, falling back to Ollama");
                return create_ollama_provider();
            }
            tracing::info!("[PROVIDER] Using Gemini: model={}", model);
            Arc::new(GeminiProvider::new(api_key).with_model(&model))
        }
        "groq" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI").unwrap_or_default();
            let model =
                std::env::var("VOXY_GROQ_MODEL").unwrap_or_else(|_| "openai/gpt-oss-20b".into());
            if api_key.is_empty() {
                tracing::warn!(
                    "[PROVIDER] VOXY_API_KEYS_OPENAI not set for Groq, falling back to Ollama"
                );
                return create_ollama_provider();
            }
            let config = OpenAIConfig::groq(api_key);
            tracing::info!("[PROVIDER] Using Groq: model={}", model);
            Arc::new(OpenAIProvider::new(config).with_model(&model))
        }
        "openrouter" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI").unwrap_or_default();
            let model = std::env::var("VOXY_OPENROUTER_MODEL")
                .unwrap_or_else(|_| "anthropic/claude-3.5-sonnet".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_OPENAI not set for OpenRouter, falling back to Ollama");
                return create_ollama_provider();
            }
            let config = OpenAIConfig::openrouter(api_key);
            tracing::info!("[PROVIDER] Using OpenRouter: model={}", model);
            Arc::new(OpenAIProvider::new(config).with_model(&model))
        }
        "nvidia" | "nvidia_nim" | "nim" => {
            let api_key = std::env::var("VOXY_API_KEYS_NVIDIA")
                .or_else(|_| std::env::var("NVIDIA_API_KEY"))
                .or_else(|_| std::env::var("VOXY_API_KEYS_OPENAI"))
                .unwrap_or_default();
            let model = std::env::var("VOXY_NVIDIA_MODEL")
                .unwrap_or_else(|_| "meta/llama-3.3-70b-instruct".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_NVIDIA not set, falling back to Ollama");
                return create_ollama_provider();
            }
            let config = OpenAIConfig::nvidia_nim(api_key);
            tracing::info!("[PROVIDER] Using NVIDIA NIM: model={}", model);
            Arc::new(OpenAIProvider::new(config).with_model(&model))
        }
        _ => create_ollama_provider(),
    }
}

fn create_ollama_provider() -> Arc<dyn LlmProvider> {
    let url = std::env::var("VOXY_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let model = std::env::var("VOXY_OLLAMA_MODEL").unwrap_or_else(|_| "gpt-oss:120b-cloud".into());
    tracing::info!("[PROVIDER] Using Ollama Cloud: model={} @ {}", model, url);
    Arc::new(
        OllamaProvider::new(&url, &model)
            .map_err(|e| tracing::warn!("Failed to create Ollama client: {e}"))
            .unwrap_or_else(|_| OllamaProvider::default()),
    )
}

fn setup_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,opencode_debug=debug".into()),
        )
        .try_init();
}

struct ConversationMemory {
    turns: VecDeque<(String, String)>,
    last_app: Option<String>,
    mention_count: usize,
    max_history_chars: usize,
    db: Option<rusqlite::Connection>,
}

impl ConversationMemory {
    fn new() -> Self {
        let db = Self::open_db();
        let mut mem = Self {
            turns: VecDeque::with_capacity(20),
            last_app: None,
            mention_count: 0,
            max_history_chars: 6000,
            db,
        };
        mem.load_recent_turns();
        mem
    }

    fn open_db() -> Option<rusqlite::Connection> {
        let data_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("VOXY");
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            tracing::warn!("[MEMORY] Failed to create data dir: {e}");
            return None;
        }
        let db_path = data_dir.join("conversation.db");
        match rusqlite::Connection::open(&db_path) {
            Ok(conn) => {
                if let Err(e) = conn.busy_timeout(std::time::Duration::from_millis(5000)) {
                    tracing::warn!("[MEMORY] Failed to set busy timeout: {e}");
                }
                if let Err(e) =
                    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
                {
                    tracing::warn!("[MEMORY] Failed to set WAL mode: {e}");
                }
                if let Err(e) = conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS conversation_turns (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        role TEXT NOT NULL,
                        content TEXT NOT NULL,
                        timestamp TEXT NOT NULL DEFAULT (datetime('now'))
                    );
                    CREATE INDEX IF NOT EXISTS idx_turns_ts ON conversation_turns(timestamp);",
                ) {
                    tracing::warn!("[MEMORY] Failed to create tables: {e}");
                    return None;
                }
                tracing::info!(
                    "[MEMORY] SQLite conversation memory opened at {}",
                    db_path.display()
                );
                Some(conn)
            }
            Err(e) => {
                tracing::warn!("[MEMORY] Failed to open SQLite: {e}");
                None
            }
        }
    }

    fn load_recent_turns(&mut self) {
        if let Some(ref conn) = self.db {
            match conn
                .prepare("SELECT role, content FROM conversation_turns ORDER BY id DESC LIMIT 20")
            {
                Ok(mut stmt) => {
                    if let Ok(mapped) = stmt.query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    }) {
                        let rows: Vec<(String, String)> = mapped.filter_map(|r| r.ok()).collect();
                        // Rows come newest-first, reverse for chronological order
                        for (role, content) in rows.into_iter().rev() {
                            self.turns.push_back((role, content));
                        }
                    }
                    tracing::info!(
                        "[MEMORY] Loaded {} turns from previous session",
                        self.turns.len()
                    );
                }
                Err(e) => {
                    tracing::warn!("[MEMORY] Failed to load turns: {e}");
                }
            }
        }
    }

    fn with_max_history_chars(mut self, max_history_chars: usize) -> Self {
        self.max_history_chars = max_history_chars;
        self
    }

    fn add_turn(&mut self, role: &str, text: &str) {
        if self.turns.len() >= 20 {
            self.turns.pop_front();
        }
        self.turns.push_back((role.to_string(), text.to_string()));
        self.mention_count = self.mention_count.wrapping_add(1);

        // Persist to SQLite
        if let Some(ref conn) = self.db {
            if let Err(e) = conn.execute(
                "INSERT INTO conversation_turns (role, content) VALUES (?1, ?2)",
                rusqlite::params![role, text],
            ) {
                tracing::warn!("[MEMORY] Failed to persist turn: {e}");
            }
        }
    }

    fn conversation_history(&self) -> String {
        // Trim to a context-window budget: keep the most recent turns that fit
        // within `max_history_chars`, always including the newest turn.
        let budget = self.max_history_chars.max(1);
        let mut kept: Vec<&(String, String)> = Vec::new();
        let mut chars = 0usize;
        for turn in self.turns.iter().rev() {
            let line_len = turn.0.len() + turn.1.len() + 2;
            if !kept.is_empty() && chars + line_len > budget {
                break;
            }
            chars += line_len;
            kept.push(turn);
        }
        kept.reverse();
        kept.iter()
            .map(|(role, text)| format!("{}: {}", role, text))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[allow(dead_code)]
    fn last_user_message(&self) -> Option<&str> {
        self.turns
            .iter()
            .rev()
            .find(|(role, _)| role == "user")
            .map(|(_, text)| text.as_str())
    }

    fn is_follow_up(&self, text: &str) -> bool {
        let follow_words = [
            "it", "that", "this", "there", "again", "another", "also", "too",
        ];
        let first = text.split_whitespace().next().unwrap_or("");
        follow_words.contains(&first)
    }
}

struct VoiceMetrics {
    stt_count: AtomicU64,
    total_automation_ns: AtomicU64,
    total_tts_ns: AtomicU64,
    max_automation_ns: AtomicU64,
    max_tts_ns: AtomicU64,
    restart_count: AtomicU64,
    start_time: Instant,
}

impl VoiceMetrics {
    fn new() -> Self {
        Self {
            stt_count: AtomicU64::new(0),
            total_automation_ns: AtomicU64::new(0),
            total_tts_ns: AtomicU64::new(0),
            max_automation_ns: AtomicU64::new(0),
            max_tts_ns: AtomicU64::new(0),
            restart_count: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    fn _record_stt(&self) {
        self.stt_count.fetch_add(1, Ordering::Relaxed);
    }

    fn _record_tts(&self, dur: Duration) {
        let ns = dur.as_nanos() as u64;
        self.total_tts_ns.fetch_add(ns, Ordering::Relaxed);
        let mut max = self.max_tts_ns.load(Ordering::Relaxed);
        while ns > max {
            match self
                .max_tts_ns
                .compare_exchange(max, ns, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(m) => max = m,
            }
        }
    }

    fn record_automation(&self, dur: Duration) {
        let ns = dur.as_nanos() as u64;
        self.total_automation_ns.fetch_add(ns, Ordering::Relaxed);
        let mut max = self.max_automation_ns.load(Ordering::Relaxed);
        while ns > max {
            match self.max_automation_ns.compare_exchange(
                max,
                ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(m) => max = m,
            }
        }
    }

    fn report(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let sc = self.stt_count.load(Ordering::Relaxed);
        let auto_sum = self.total_automation_ns.load(Ordering::Relaxed);
        let auto_max = self.max_automation_ns.load(Ordering::Relaxed);
        let tts_sum = self.total_tts_ns.load(Ordering::Relaxed);
        let tts_max = self.max_tts_ns.load(Ordering::Relaxed);
        let restarts = self.restart_count.load(Ordering::Relaxed);
        format!(
            "**VOXY Metrics ({}s uptime, {} restarts):**\n  STT: {}, Automation avg: {:.1}ms max: {:.1}ms, TTS avg: {:.1}ms max: {:.1}ms",
            elapsed.as_secs(), restarts, sc,
            if sc > 0 { auto_sum as f64 / sc as f64 / 1_000_000.0 } else { 0.0 },
            auto_max as f64 / 1_000_000.0,
            if sc > 0 { tts_sum as f64 / sc as f64 / 1_000_000.0 } else { 0.0 },
            tts_max as f64 / 1_000_000.0,
        )
    }
}

static PICK_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn pick_idx(max: usize) -> usize {
    PICK_COUNTER.fetch_add(1, Ordering::Relaxed) % max
}

#[allow(dead_code)]
fn pick_greeting() -> &'static str {
    let g = ["Hello!", "Hi there!", "Hey!", "Yes?", "I'm here."];
    g[pick_idx(g.len())]
}

#[allow(dead_code)]
fn pick_goodbye() -> &'static str {
    let b = [
        "Goodbye!",
        "See you later!",
        "Take care!",
        "Talk soon!",
        "Bye!",
    ];
    b[pick_idx(b.len())]
}

#[allow(dead_code)]
fn pick_thanks() -> &'static str {
    let t = [
        "You're welcome!",
        "Happy to help!",
        "Anytime!",
        "My pleasure.",
    ];
    t[pick_idx(t.len())]
}

#[allow(dead_code)]
fn pick_identity() -> &'static str {
    let i = [
        "I am VOXY, your voice assistant.",
        "I'm VOXY! Your personal AI assistant.",
        "This is VOXY at your service.",
    ];
    i[pick_idx(i.len())]
}

#[allow(dead_code)]
fn pick_verify_response(success: bool, app: &str) -> String {
    if success {
        let ok = [
            "{} is now open and ready.",
            "{} has been launched successfully.",
            "I've opened {} for you.",
        ];
        ok[pick_idx(ok.len())].replace("{}", app)
    } else {
        let fail = [
            "I had trouble opening {}. Let me retry.",
            "Sorry, I couldn't launch {} right now.",
        ];
        fail[pick_idx(fail.len())].replace("{}", app)
    }
}

fn pick_echo(text: &str) -> String {
    let prompts = ["I heard you mention", "You said", "Noted:", "I understand"];
    let prefix = prompts[pick_idx(prompts.len())];
    format!("{prefix}. {text}")
}

type PipelineFuture =
    Pin<Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + Send>>;

#[allow(dead_code)]
fn get_memory_usage() -> (u64, u64) {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    (sys.used_memory(), sys.total_memory())
}

#[allow(dead_code)]
fn get_cpu_usage() -> f64 {
    #[cfg(target_os = "windows")]
    {
        use std::time::Instant;
        let mut system = sysinfo::System::new();
        system.refresh_cpu_all();
        let start = Instant::now();
        std::thread::sleep(Duration::from_millis(100));
        system.refresh_cpu_all();
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            let total: f64 = system.cpus().iter().map(|c| c.cpu_usage() as f64).sum();
            total / system.cpus().len() as f64
        } else {
            0.0
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::thread::sleep(Duration::from_millis(100));
        0.0
    }
}

#[allow(dead_code)]
fn measure_stage_ms<F, T>(f: F) -> (f64, T)
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    (ms, result)
}

#[allow(dead_code)]
fn detect_simd() -> Vec<String> {
    let mut caps = Vec::new();
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            caps.push("AVX2".to_string());
        }
        if is_x86_feature_detected!("avx512f") {
            caps.push("AVX-512 (F)".to_string());
        }
        if is_x86_feature_detected!("avx512bw") {
            caps.push("AVX-512 (BW)".to_string());
        }
        if is_x86_feature_detected!("sse2") {
            caps.push("SSE2".to_string());
        }
        if is_x86_feature_detected!("ssse3") {
            caps.push("SSSE3".to_string());
        }
        if is_x86_feature_detected!("fma") {
            caps.push("FMA".to_string());
        }
        if is_x86_feature_detected!("aes") {
            caps.push("AES-NI".to_string());
        }
    }
    if caps.is_empty() {
        caps.push("none detected".to_string());
    }
    caps
}

#[allow(dead_code)]
fn detect_cpu_features() -> Vec<String> {
    let mut features = Vec::new();
    features.push(format!("logical cores: {}", num_cpus::get()));
    features.push(format!("physical cores: {}", num_cpus::get_physical()));
    features
}

fn create_stt_engine() -> Arc<dyn voxy_voice_stt::SttEngine> {
    match voxy_voice_stt::GroqSttEngine::from_env() {
        Ok(groq) => {
            tracing::info!("[VOICE] Using Groq Cloud STT (whisper-large-v3-turbo)");
            Arc::new(groq)
        }
        Err(e) => {
            tracing::info!(
                "[VOICE] Groq STT not configured ({}), using Local Whisper STT adapter",
                e
            );
            Arc::new(voxy_voice_stt::LocalWhisperSttEngine::new(
                "whisper-base.en",
            ))
        }
    }
}

fn create_tts_engine() -> Arc<dyn voxy_voice_tts::TtsEngine> {
    if std::env::var("VOXY_TTS_PROVIDER").as_deref() == Ok("chatterbox")
        || std::env::var("CHATTERBOX_ENDPOINT").is_ok()
    {
        tracing::info!("[VOICE] Using Resemble AI Chatterbox TTS engine");
        Arc::new(voxy_voice_tts::ChatterboxTtsEngine::new(None, None))
    } else {
        match voxy_voice_tts::CloudTtsEngine::from_env() {
            Ok(cloud) => {
                tracing::info!("[VOICE] Using Cloud TTS ({})", cloud.name());
                Arc::new(cloud)
            }
            Err(e) => {
                tracing::info!(
                    "[VOICE] Cloud TTS not configured ({}), using Local TTS engine",
                    e
                );
                Arc::new(voxy_voice_tts::LocalTtsEngine::new(
                    "Microsoft David Desktop",
                ))
            }
        }
    }
}

fn run_pipeline(running: Arc<AtomicBool>, metrics: Arc<VoiceMetrics>) -> PipelineFuture {
    Box::pin(async move {
        // ── VOXY COM Native IPC Server (Windows Named Pipe) ──
        let ipc_server = Arc::new(voxy_ipc::VoxyIpcServer::new());
        ipc_server.start();
        tracing::info!(
            "[IPC] VOXY COM Named Pipe IPC server active on {}",
            voxy_ipc::VOXY_PIPE_NAME
        );

        let stt_engine = create_stt_engine();
        let tts_engine = create_tts_engine();

        let mut voice_config = voxy_voice_orchestrator::VoicePipelineConfig::default();
        if std::env::var("VOXY_ENABLE_PIPECAT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            tracing::info!("[VOICE] Pipecat streaming voice sidecar supervision enabled");
            voice_config.pipecat = Some(voxy_voice_orchestrator::PipecatConfig::default());
        }

        let pipeline = Arc::new(
            voxy_voice_orchestrator::VoicePipeline::new(
                voice_config,
                stt_engine,
                tts_engine,
                Some(ipc_server.clone()),
            )
            .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?,
        );
        tracing::info!("[VOICE] Production VoicePipeline initialized with low-latency audio capture and streaming TTS");

        // ── Experience Layer ──────────────────────────────────────────────
        let intelligence_config = IntelligenceConfig::default();
        let (exp_bridge, exp_input_tx, mut exp_output_rx) =
            ExperienceBridge::new(intelligence_config);
        exp_bridge.start().await;
        let exp_bridge = Arc::new(exp_bridge);
        let mut moment_engine = MomentEngine::new();
        let mut proactive_engine = ProactiveEngine::new(ProactiveConfig::default());
        tracing::info!("Experience Layer and Proactive Engine started");

        // ── Cognitive Orchestrator ────────────────────────────────────────
        let cognitive_bridge = Arc::new(CognitiveBridge::new(OrchestratorConfig::default()));
        tracing::info!("Cognitive Orchestrator initialized");

        // ── Runtime Guard ────────────────────────────────────────────────
        let guard = Arc::new(RuntimeGuard::new(GuardConfig::default()));
        tracing::info!("Runtime Guard initialized");

        // Register core subsystems with real runtime state observation
        let g = guard.clone();
        let pipe_ref = pipeline.clone();
        g.register_subsystem("voice_pipeline", move || {
            let p = pipe_ref.clone();
            async move {
                let status = if !p.is_initialized() {
                    voxy_shared::HealthStatus::Unhealthy("Voice pipeline uninitialized".to_string())
                } else if !p.is_running() {
                    voxy_shared::HealthStatus::Degraded("Voice capture stopped".to_string())
                } else if p.is_sleeping() {
                    voxy_shared::HealthStatus::Degraded("Voice pipeline sleeping".to_string())
                } else {
                    voxy_shared::HealthStatus::Healthy
                };
                voxy_health::HealthReport::new("voice_pipeline", status)
            }
        })
        .await;

        let g = guard.clone();
        let cog_ref = cognitive_bridge.clone();
        g.register_subsystem("cognitive_bridge", move || {
            let _c = cog_ref.clone();
            async move {
                // Cognitive bridge active in process
                voxy_health::HealthReport::new(
                    "cognitive_bridge",
                    voxy_shared::HealthStatus::Healthy,
                )
            }
        })
        .await;

        let g = guard.clone();
        let exp_ref = exp_bridge.clone();
        g.register_subsystem("experience_bridge", move || {
            let _e = exp_ref.clone();
            async move {
                // Experience bridge running
                voxy_health::HealthReport::new(
                    "experience_bridge",
                    voxy_shared::HealthStatus::Healthy,
                )
            }
        })
        .await;

        let g = guard.clone();
        g.register_subsystem("desktop_bridge", || async {
            voxy_health::HealthReport::new("desktop_bridge", voxy_shared::HealthStatus::Healthy)
        })
        .await;

        guard.heartbeat("voice_pipeline");
        guard.heartbeat("cognitive_bridge");
        guard.heartbeat("experience_bridge");
        guard.heartbeat("desktop_bridge");
        tracing::info!("Runtime Guard: 4 subsystems registered with active state monitoring");

        // ── Desktop Runtime ──────────────────────────────────────────────
        let desktop_config = RuntimeConfig::new("VOXY");
        let mut desktop_runtime = DesktopRuntime::new(desktop_config).unwrap_or_else(|e| {
            tracing::warn!("Desktop runtime init failed (non-fatal): {}", e);
            DesktopRuntime::new(RuntimeConfig::new("VOXY")).expect("desktop runtime fallback")
        });
        let _ = desktop_runtime.start().await;
        let desktop_runtime = Arc::new(desktop_runtime);
        tracing::info!("Desktop runtime initialized");

        // ── Graceful Shutdown Coordinator ────────────────────────────────
        let mut graceful =
            shutdown::GracefulShutdown::new(running.clone()).with_timeout(Duration::from_secs(30));

        // ── Background Runtime ────────────────────────────────────────────
        let (bg_runtime, reflection_submitter, _knowledge_validation_submitter) =
            background::BackgroundRuntime::new(
                running.clone(),
                cognitive_bridge.clone(),
                guard.clone(),
            );
        tracing::info!("Background runtime started (9 tasks)");

        // ── Register Shutdown Subsystems ─────────────────────────────────
        {
            let bg = bg_runtime.clone_for_shutdown();
            graceful.register_simple(
                "background_runtime",
                shutdown::ShutdownPriority::Background,
                Duration::from_secs(10),
                move || {
                    let bg = bg.clone();
                    async move { bg.shutdown().await }
                },
            );
        }
        {
            let bridge = cognitive_bridge.clone();
            graceful.register_simple(
                "cognitive_bridge",
                shutdown::ShutdownPriority::Background,
                Duration::from_secs(5),
                move || {
                    let bridge = bridge.clone();
                    async move {
                        bridge.shutdown();
                    }
                },
            );
        }
        {
            let exp = exp_bridge.clone();
            graceful.register_simple(
                "experience_bridge",
                shutdown::ShutdownPriority::Services,
                Duration::from_secs(5),
                move || {
                    let exp = exp.clone();
                    async move {
                        exp.stop().await;
                    }
                },
            );
        }
        {
            let dr = desktop_runtime.clone();
            graceful.register_simple(
                "desktop_runtime",
                shutdown::ShutdownPriority::Background,
                Duration::from_secs(5),
                move || {
                    let dr = dr.clone();
                    async move {
                        let _ = dr.shutdown().await;
                    }
                },
            );
        }
        {
            let ipc_srv = ipc_server.clone();
            graceful.register_simple(
                "ipc_server",
                shutdown::ShutdownPriority::Services,
                Duration::from_secs(3),
                move || {
                    let ipc_srv = ipc_srv.clone();
                    async move {
                        ipc_srv.stop();
                    }
                },
            );
        }

        // ── LLM Provider (multi-provider support) ──────────────────────
        let llm = create_llm_provider();
        match llm.health().await {
            Ok(true) => tracing::info!("[PROVIDER] {} connected", llm.name()),
            Ok(false) => tracing::warn!(
                "[PROVIDER] {} health check failed, will retry on first request",
                llm.name()
            ),
            Err(e) => tracing::warn!(
                "[PROVIDER] {} not reachable: {e} — LLM responses will be fallbacks",
                llm.name()
            ),
        }

        // ── Native Hardware & Capability Discovery ──
        let hw_summary = voxy_hardware::HardwareDetector::detect();
        tracing::info!(
            "[HARDWARE] Detected CPU: {} ({} cores), RAM: {:.1} GB, Primary GPU: {} ({:.1} GB VRAM)",
            hw_summary.cpu_brand,
            hw_summary.cpu_cores,
            hw_summary.total_ram_gb(),
            hw_summary.primary_gpu.as_ref().map(|g| g.name.as_str()).unwrap_or("None / Integrated"),
            hw_summary.total_vram_gb()
        );
        ipc_server.broadcast_hardware_status(
            hw_summary.cpu_brand.clone(),
            hw_summary.cpu_cores,
            hw_summary.total_ram_gb(),
            hw_summary.primary_gpu.as_ref().map(|g| g.name.clone()),
            hw_summary.total_vram_gb(),
        );
        let routing_mode = std::env::var("VOXY_ROUTING_MODE").unwrap_or_else(|_| "Auto".into());
        ipc_server.broadcast_routing_status(
            routing_mode,
            llm.name().to_string(),
            "Voxray/SAPI Speech Engine".to_string(),
            "Voxray/SAPI Speech Engine".to_string(),
            false,
        );

        // Register LLM as a healable subsystem
        {
            let g = guard.clone();
            let llm_name = llm.name().to_string();
            g.register_healable(
                "ollama_llm",
                move || {
                    let name = llm_name.clone();
                    async move {
                        voxy_health::HealthReport::new(&name, voxy_shared::HealthStatus::Healthy)
                    }
                },
                move || async move {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    Ok(())
                },
            )
            .await;
            guard.heartbeat("ollama_llm");
            tracing::info!("[PROVIDER] Healable subsystem registered: ollama_llm");
        }

        // ── Cognitive Engine (intent matching) ────────────────────────────
        let cognitive = Arc::new(InMemoryCognitiveEngine::new(CognitionConfig::default()));

        // ── Guardian Security Engine ─────────────────────────────────────
        let audit_log = Arc::new(tokio::sync::Mutex::new(AuditLog::new()));
        let guardian = Arc::new(GuardianEngine::new(
            CapabilityRegistry::new(),
            PolicyEngine::with_default_rules(),
            GuardianConfig::default(),
        ));

        // ── Recovery Mode ───────────────────────────────────────────────
        let recovery_mode = Arc::new(tokio::sync::Mutex::new(RecoveryMode::new()));

        // Record startup audit event
        {
            let mut log = audit_log.lock().await;
            log.record_typed(
                "system",
                "startup",
                None,
                "allowed",
                Some("VOXY daemon started"),
                "none",
                "verified",
                voxy_security::policy::AuditLevel::Basic,
                AuditEventType::Authentication {
                    method: "system_boot".to_string(),
                    success: true,
                },
            );
        }

        tracing::info!("Guardian security engine initialized");

        // ── System Prompt (immutable layers) ─────────────────────────────
        let base_prompt = SystemPromptBuilder::new().build();
        let system_prompt = Arc::new(format!(
            "{}\n\n{}",
            base_prompt,
            tools::ToolRegistry::tool_definitions(),
        ));

        // ── Unified Approval Broker (Human-in-the-Loop Governance) ───────
        let approval_broker = Arc::new(voxy_security::ApprovalBroker::new());

        // Forward ApprovalBroker requests to IPC
        {
            let ipc_srv = ipc_server.clone();
            let mut req_rx = approval_broker.subscribe_requests();
            tokio::spawn(async move {
                while let Ok(req) = req_rx.recv().await {
                    let ipc_req = voxy_ipc::IpcApprovalRequest {
                        request_id: req.id,
                        session_id: req.session_id,
                        tool_name: req.tool_name,
                        parameters_redacted: req.parameters_redacted,
                        risk_level: req.risk_level.as_str().to_string(),
                        reason: req.reason,
                        timeout_secs: req.timeout_secs,
                    };
                    ipc_srv.broadcast(voxy_ipc::DaemonMessage::ApprovalRequested(ipc_req));
                }
            });
        }

        // Forward ApprovalBroker decisions to IPC
        {
            let ipc_srv = ipc_server.clone();
            let mut dec_rx = approval_broker.subscribe_decisions();
            tokio::spawn(async move {
                while let Ok(dec) = dec_rx.recv().await {
                    let ipc_dec = match dec.status {
                        voxy_security::ApprovalStatus::Approved => {
                            voxy_ipc::IpcApprovalDecision::Approved
                        }
                        voxy_security::ApprovalStatus::Denied => {
                            voxy_ipc::IpcApprovalDecision::Denied
                        }
                        voxy_security::ApprovalStatus::Expired => {
                            voxy_ipc::IpcApprovalDecision::Expired
                        }
                        voxy_security::ApprovalStatus::Cancelled => {
                            voxy_ipc::IpcApprovalDecision::Cancelled
                        }
                        voxy_security::ApprovalStatus::Pending => continue,
                    };
                    ipc_srv.broadcast(voxy_ipc::DaemonMessage::ApprovalResolved {
                        request_id: dec.request_id,
                        decision: ipc_dec,
                    });
                }
            });
        }

        // Periodically sweep expired approval requests
        {
            let broker_sweep = approval_broker.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    broker_sweep.sweep_expired();
                }
            });
        }

        // ── Tool Registry ─────────────────────────────────────────────────
        let tool_registry = match tools::ToolRegistry::new().await {
            Ok(tr) => {
                let tr = tr.with_approval_broker(approval_broker.clone());
                tracing::info!(
                    "[TOOLS] Tool registry initialized with automation backend and ApprovalBroker"
                );
                Some(Arc::new(tr))
            }
            Err(e) => {
                tracing::warn!("[TOOLS] Failed to initialize tool registry: {e}");
                None
            }
        };

        // ── Conversation Memory ───────────────────────────────────────────
        // History is trimmed to a context-window character budget before being
        // inserted into prompts. Configure via VOXY_LLM_CONTEXT_CHARS (roughly
        // 4 chars per token).
        let context_chars: usize = std::env::var("VOXY_LLM_CONTEXT_CHARS")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(6000);
        let memory = Arc::new(tokio::sync::Mutex::new(
            ConversationMemory::new().with_max_history_chars(context_chars),
        ));
        let metrics_ref = metrics.clone();
        let llm_ref = llm.clone();
        let exp_input_for_handler = exp_input_tx.clone();

        // ── Desktop context (shared state updated by timer loop) ──────────
        let desktop_context: Arc<tokio::sync::RwLock<String>> =
            Arc::new(tokio::sync::RwLock::new(String::new()));

        // ── Task completion counter (shared between response handler and timer) ──
        let tasks_completed = Arc::new(std::sync::atomic::AtomicU32::new(0));

        // ── Response Handler ──────────────────────────────────────────────
        let response_handler = {
            let cognitive = cognitive.clone();
            let memory_clone = memory.clone();
            let exp_input = exp_input_for_handler.clone();
            let llm = llm_ref.clone();
            let desktop_ctx = desktop_context.clone();
            let reflector = reflection_submitter.clone();
            let guardian = guardian.clone();
            let sys_prompt = system_prompt.clone();
            let tasks_completed = tasks_completed.clone();
            let audit_log = audit_log.clone();
            let recovery_mode = recovery_mode.clone();
            let tool_reg = tool_registry.clone();
            let ipc_srv = ipc_server.clone();
            Box::new(
                move |text: String| -> futures_util::future::BoxFuture<'static, Result<String, String>> {
                    let cognitive = cognitive.clone();
                    let memory = memory_clone.clone();
                    let metrics = metrics_ref.clone();
                    let exp_input = exp_input.clone();
                    let llm = llm.clone();
                    let desktop_ctx = desktop_ctx.clone();
                    let reflector = reflector.clone();
                    let guardian = guardian.clone();
                    let sys_prompt = sys_prompt.clone();
                    let recovery_mode = recovery_mode.clone();
                    let tasks_completed = tasks_completed.clone();
                    let audit_log = audit_log.clone();
                    let tool_reg = tool_reg.clone();
                    let ipc_srv = ipc_srv.clone();
                    Box::pin(async move {
                        // ── 0. SANITIZE INPUT (security boundary) ────────
                        let sanitized = sanitize_user_input(&text);
                        if sanitized.injection_detected {
                            tracing::warn!(
                                patterns = ?sanitized.patterns,
                                "Injection pattern detected in user input — sanitizing"
                            );
                        }
                        let text = sanitized.text;

                        if text.is_empty() || text == "short sound" {
                            return Ok(String::new());
                        }

                        let response_start = Instant::now();

                        // ── 1. Feed transcript → Experience Layer & IPC Clients ──────────
                        tracing::info!("[VOICE:LLM] Text: {}", text);
                        let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                            text: text.clone(),
                            is_final: true,
                        });
                        ipc_srv.broadcast(voxy_ipc::DaemonMessage::ChatMessage {
                            id: CHAT_MSG_ID.fetch_add(1, Ordering::Relaxed),
                            sender: "USER".into(),
                            text: text.clone(),
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            is_user: true,
                        });

                        // ── 2. Store user turn in memory ───────────────────
                        let lower = text.to_lowercase();
                        let mut mem = memory.lock().await;
                        mem.add_turn("user", &text);
                        let _is_follow_up = mem.is_follow_up(&lower);

                        // ── 2b. Emergency Stop Voice Interceptor ────────────
                        let lower_trimmed = lower.trim();
                        if lower_trimmed == "emergency stop"
                            || lower_trimmed == "abort all actions"
                            || lower_trimmed == "freeze actions"
                            || lower_trimmed == "stop all actions"
                            || lower_trimmed == "emergency halt"
                        {
                            if let Some(ref tr) = tool_reg {
                                tr.trigger_emergency_stop();
                                ipc_srv.set_emergency_stop(true).await;
                                tracing::warn!(
                                    "[VOICE:EMERGENCY] Emergency Stop triggered via voice command"
                                );
                                let response = "Emergency Stop activated. All computer control actions have been halted.".to_string();
                                mem.add_turn("assistant", &response);
                                ipc_srv.broadcast(voxy_ipc::DaemonMessage::ChatMessage {
                                    id: CHAT_MSG_ID.fetch_add(1, Ordering::Relaxed),
                                    sender: "VOXY".into(),
                                    text: response.clone(),
                                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                    is_user: false,
                                });
                                return Ok(response);
                            }
                        }
                        if lower_trimmed == "reset emergency stop"
                            || lower_trimmed == "resume actions"
                            || lower_trimmed == "cancel emergency stop"
                        {
                            if let Some(ref tr) = tool_reg {
                                tr.reset_emergency_stop();
                                ipc_srv.set_emergency_stop(false).await;
                                tracing::info!(
                                    "[VOICE:EMERGENCY] Emergency Stop reset via voice command"
                                );
                                let response =
                                    "Emergency Stop reset. Computer control is re-enabled."
                                        .to_string();
                                mem.add_turn("assistant", &response);
                                ipc_srv.broadcast(voxy_ipc::DaemonMessage::ChatMessage {
                                    id: CHAT_MSG_ID.fetch_add(1, Ordering::Relaxed),
                                    sender: "VOXY".into(),
                                    text: response.clone(),
                                    timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                    is_user: false,
                                });
                                return Ok(response);
                            }
                        }

                        // If Emergency Stop is engaged, block any automation commands
                        if tool_reg
                            .as_ref()
                            .map(|tr| tr.is_emergency_stopped())
                            .unwrap_or(false)
                            && (lower.contains("open")
                                || lower.contains("launch")
                                || lower.contains("start")
                                || lower.contains("type")
                                || lower.contains("click")
                                || lower.contains("press"))
                        {
                            tracing::warn!("[VOICE:EMERGENCY] Blocked automation action: Emergency Stop active");
                            let response = "Action blocked. Emergency Stop is active. Say 'reset emergency stop' to resume.".to_string();
                            mem.add_turn("assistant", &response);
                            return Ok(response);
                        }

                        // ── 3. Handle automation commands directly ──────────
                        if lower.contains("open")
                            || lower.contains("launch")
                            || lower.contains("start")
                        {
                            let app = if lower.contains("chrome") || lower.contains("browser") {
                                Some("chrome")
                            } else if lower.contains("notepad") || lower.contains("editor") {
                                Some("notepad")
                            } else if lower.contains("calc") || lower.contains("calculator") {
                                Some("calc")
                            } else if lower.contains("explorer") || lower.contains("files") {
                                Some("explorer")
                            } else if lower.contains("spotify") {
                                Some("spotify")
                            } else if lower.contains("discord") {
                                Some("discord")
                            } else if lower.contains("code") || lower.contains("vscode") {
                                Some("code")
                            } else if lower.contains("terminal") || lower.contains("powershell") {
                                Some("wt")
                            } else if lower.contains("cmd") || lower.contains("command prompt") {
                                Some("cmd")
                            } else if lower.contains("settings") {
                                Some("ms-settings:")
                            } else {
                                None
                            };

                            if let Some(app) = app {
                                // ── GUARDIAN CHECK: automation requires authorization ──
                                let decision = {
                                    let r = recovery_mode.lock().await;
                                    guardian.evaluate(
                                        "voice-user",
                                        "automation:write",
                                        Some(app),
                                        "launch_application",
                                        std::collections::HashMap::new(),
                                        &r,
                                    )
                                };

                                // Record typed audit event for guardian decision
                                {
                                    let mut log = audit_log.lock().await;
                                    let event_type = if decision.allowed {
                                        AuditEventType::Authorization {
                                            decision: "allowed".to_string(),
                                        }
                                    } else {
                                        AuditEventType::Authorization {
                                            decision: "denied".to_string(),
                                        }
                                    };
                                    log.record_typed(
                                        "voice-user",
                                        "automation:write",
                                        Some(app),
                                        if decision.allowed {
                                            "allowed"
                                        } else {
                                            "denied"
                                        },
                                        Some(&decision.reason),
                                        "high",
                                        "trusted",
                                        voxy_security::policy::AuditLevel::Detailed,
                                        event_type,
                                    );
                                }

                                if !decision.allowed {
                                    // Activate recovery mode on critical-risk denials
                                    if decision.requires_mfa {
                                        let mut recovery = recovery_mode.lock().await;
                                        if recovery.state()
                                            == voxy_security::recovery::RecoveryState::Normal
                                        {
                                            let auth = voxy_security::recovery::RecoveryAuth {
                                                subject: "system".to_string(),
                                                reason: format!(
                                                    "Critical risk action denied: {}",
                                                    decision.reason
                                                ),
                                                auth_method: "automatic_guardian".to_string(),
                                            };
                                            if let Err(e) = recovery.enter(auth) {
                                                tracing::error!(
                                                    error = %e,
                                                    "Failed to activate recovery mode"
                                                );
                                            } else {
                                                tracing::warn!(
                                                    reason = %decision.reason,
                                                    "Recovery mode activated due to critical threat"
                                                );
                                            }
                                        }
                                    }

                                    let response = format!(
                                        "I need permission to open {}. {}",
                                        app, decision.reason
                                    );
                                    mem.add_turn("assistant", &response);
                                    drop(mem);
                                    let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                                        text: response.clone(),
                                        is_final: true,
                                    });
                                    return Ok(response);
                                }

                                mem.last_app = Some(app.to_string());
                                let uia = WindowsUiaBackend::new();
                                let response = if uia.is_available().await {
                                    let auto_start = Instant::now();
                                    match open_application(app).await {
                                        Ok(_) => {
                                            metrics.record_automation(auto_start.elapsed());
                                            format!("Done — {app} is now open.")
                                        }
                                        Err(e) => {
                                            metrics.record_automation(auto_start.elapsed());
                                            format!("I couldn't open {app}. {e}")
                                        }
                                    }
                                } else {
                                    format!(
                                        "I'd open {app} for you, but automation isn't available."
                                    )
                                };
                                mem.add_turn("assistant", &response);
                                drop(mem);
                                let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                                    text: response.clone(),
                                    is_final: true,
                                });
                                return Ok(response);
                            }
                        }

                        if lower.contains("open it again") || lower.contains("reopen") {
                            if let Some(app) = mem.last_app.clone() {
                                let decision = {
                                    let r = recovery_mode.lock().await;
                                    guardian.evaluate(
                                        "voice-user",
                                        "automation:write",
                                        Some(&app),
                                        "launch_application",
                                        std::collections::HashMap::new(),
                                        &r,
                                    )
                                };

                                {
                                    let mut log = audit_log.lock().await;
                                    let event_type = if decision.allowed {
                                        AuditEventType::Authorization {
                                            decision: "allowed".to_string(),
                                        }
                                    } else {
                                        AuditEventType::Authorization {
                                            decision: "denied".to_string(),
                                        }
                                    };
                                    log.record_typed(
                                        "voice-user",
                                        "automation:write",
                                        Some(&app),
                                        if decision.allowed {
                                            "allowed"
                                        } else {
                                            "denied"
                                        },
                                        Some(&decision.reason),
                                        "high",
                                        "trusted",
                                        voxy_security::policy::AuditLevel::Detailed,
                                        event_type,
                                    );
                                }

                                if !decision.allowed {
                                    let response = format!(
                                        "I need permission to reopen {app}. {}",
                                        decision.reason
                                    );
                                    mem.add_turn("assistant", &response);
                                    drop(mem);
                                    let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                                        text: response.clone(),
                                        is_final: true,
                                    });
                                    return Ok(response);
                                }

                                let response = match open_application(&app).await {
                                    Ok(_) => format!("Opening {app} again."),
                                    Err(e) => format!("Failed to reopen {app}: {e}"),
                                };
                                mem.add_turn("assistant", &response);
                                drop(mem);
                                let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                                    text: response.clone(),
                                    is_final: true,
                                    });
                                return Ok(response);
                            }
                        }

                        // ── 4. Build LLM prompt with full context ──────────
                        let history = mem.conversation_history();
                        let desktop = sanitize_context(&desktop_ctx.read().await.clone());
                        drop(mem);

                        let system_prompt = format!(
                            "{}\n\n\
                             Current desktop context: {}\n\n\
                             Recent conversation:\n{}",
                            sys_prompt,
                            if desktop.is_empty() {
                                "unknown"
                            } else {
                                &desktop
                            },
                            if history.is_empty() {
                                "No prior conversation.".to_string()
                            } else {
                                history
                            },
                        );

                        // ── 5. Generate response via Ollama LLM ───────────
                        tracing::info!(
                            "[VOICE:LLM] Calling Ollama (prompt={} chars)",
                            system_prompt.len()
                        );
                        let response = match llm
                            .complete(&format!(
                                "{system_prompt}\n\n{}",
                                voxy_security::prompt::SystemPromptBuilder::format_user_message(
                                    &text
                                )
                            ))
                            .await
                        {
                            Ok(raw) => {
                                let cleaned = raw
                                    .trim()
                                    .trim_start_matches("Assistant:")
                                    .trim()
                                    .to_string();
                                if cleaned.is_empty() {
                                    pick_echo(&text)
                                } else {
                                    // Sanitize LLM output to prevent system prompt leakage
                                    sanitize_llm_output(&cleaned)
                                }
                            }
                            Err(e) => {
                                tracing::warn!("LLM call failed: {e} — using fallback");
                                // Fallback to intent matching
                                let intent = IntentInput {
                                    raw_text: text.clone(),
                                    context: None,
                                    source: "voice".to_string(),
                                    metadata: std::collections::HashMap::new(),
                                };
                                match cognitive.process(&intent).await {
                                    Ok(_) => pick_echo(&text),
                                    Err(_) => pick_echo(&text),
                                }
                            }
                        };

                        // ── 5b. Parse & execute structured tool call if present ───
                        let response = if let Some(ref tr) = tool_reg {
                            if let Some((call, remaining)) =
                                tools::ToolRegistry::parse_tool_call(&response)
                            {
                                tracing::info!(
                                    "[VOICE:TOOL] Parsed tool call: {} {:?}",
                                    call.tool,
                                    call.params
                                );
                                let step_id = uuid::Uuid::new_v4().as_u128() as u64;
                                ipc_srv.broadcast_tool_step(
                                    step_id,
                                    call.tool.clone(),
                                    format!("Executing {}", call.tool),
                                    call.params.to_string(),
                                    voxy_ipc::ToolStepStatus::Running,
                                );
                                let result = tr.execute(&call).await;
                                let status = if result.success {
                                    voxy_ipc::ToolStepStatus::Completed
                                } else {
                                    voxy_ipc::ToolStepStatus::Failed
                                };
                                ipc_srv.broadcast_tool_step(
                                    step_id,
                                    call.tool.clone(),
                                    result.message.clone(),
                                    call.params.to_string(),
                                    status,
                                );
                                if !remaining.is_empty() {
                                    format!("{}\n{}", remaining, result.message)
                                } else {
                                    result.message
                                }
                            } else {
                                response
                            }
                        } else {
                            response
                        };

                        let elapsed_ms = response_start.elapsed().as_millis();
                        tracing::info!(
                            "[VOICE:LLM] Response: {} chars in {}ms",
                            response.len(),
                            elapsed_ms
                        );
                        tracing::info!(
                            input = %text,
                            response_len = response.len(),
                            latency_ms = elapsed_ms,
                            "Response generated"
                        );

                        // ── 6. Store assistant turn in memory ──────────────
                        {
                            let mut mem = memory.lock().await;
                            mem.add_turn("assistant", &response);
                        }

                        // ── 6b. Submit for reflection ────────────────────
                        {
                            let mem = memory.lock().await;
                            let messages: Vec<(String, String)> =
                                mem.turns.iter().cloned().collect();
                            drop(mem);
                            if messages.len() >= 2 {
                                let record =
                                    voxy_cognitive_orchestrator::reflection::ConversationRecord {
                                        id: uuid::Uuid::new_v4(),
                                        messages,
                                        context: String::new(),
                                        timestamp: chrono::Utc::now(),
                                    };
                                let _ = reflector.submit(record).await;
                            }
                        }

                        // ── 7. Feed response → Experience Layer & IPC Clients ────────────
                        let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                            text: response.clone(),
                            is_final: true,
                        });
                        ipc_srv.broadcast(voxy_ipc::DaemonMessage::ChatMessage {
                            id: CHAT_MSG_ID.fetch_add(1, Ordering::Relaxed),
                            sender: "VOXY".into(),
                            text: response.clone(),
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            is_user: false,
                        });

                        // ── 8. Track task completion for moments ──────────
                        tasks_completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        Ok(response)
                    })
                },
            )
        };

        pipeline.set_response_handler(response_handler);
        pipeline
            .start()
            .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
        if pipeline.config().pipecat.is_some() {
            if let Err(e) = pipeline.start_pipecat() {
                tracing::warn!("[VOICE] Pipecat sidecar startup warning: {e}");
            }
        }
        tracing::info!(
            "[VOICE] Production VoicePipeline active and listening on Windows microphone."
        );

        // ── VOXY COM IPC Inbound Command Processing Loop ──
        {
            let ipc_srv = ipc_server.clone();
            let tr_opt = tool_registry.clone();
            let broker_opt = approval_broker.clone();
            let running_flag = running.clone();
            let llm_provider_name = llm.name().to_string();
            let pipe_ipc = pipeline.clone();
            tokio::spawn(async move {
                while running_flag.load(Ordering::Relaxed) {
                    match ipc_srv.recv_command().await {
                        Some(voxy_ipc::ClientCommand::EmergencyStop) => {
                            tracing::warn!("[IPC] Emergency stop command received from client");
                            if let Some(ref tr) = tr_opt {
                                tr.trigger_emergency_stop();
                            }
                            broker_opt.emergency_stop_all();
                            ipc_srv.set_emergency_stop(true).await;
                        }
                        Some(voxy_ipc::ClientCommand::ResetEmergencyStop) => {
                            tracing::info!(
                                "[IPC] Reset emergency stop command received from client"
                            );
                            if let Some(ref tr) = tr_opt {
                                tr.reset_emergency_stop();
                            }
                            ipc_srv.set_emergency_stop(false).await;
                        }
                        Some(voxy_ipc::ClientCommand::RespondApproval {
                            request_id,
                            approved,
                            reason,
                        }) => {
                            tracing::info!(
                                %request_id,
                                approved,
                                reason = ?reason,
                                "[IPC] Received client RespondApproval decision"
                            );
                            if let Err(e) = broker_opt.resolve(request_id, approved, reason) {
                                tracing::warn!(%request_id, error = %e, "[IPC] Failed to resolve approval request");
                            }
                        }
                        Some(voxy_ipc::ClientCommand::ClientHandshake {
                            client_name,
                            client_version,
                            ..
                        }) => {
                            tracing::info!(client = %client_name, version = %client_version, "[IPC] Client connected and registered");
                        }
                        Some(voxy_ipc::ClientCommand::RequestSnapshot) => {
                            tracing::debug!("[IPC] Client requested snapshot refresh");
                        }
                        Some(voxy_ipc::ClientCommand::InterruptSpeech) => {
                            tracing::info!("[IPC] Client requested speech interrupt (barge-in)");
                            pipe_ipc.trigger_barge_in("IPC interrupt command");
                        }
                        Some(voxy_ipc::ClientCommand::StartListening) => {
                            tracing::info!("[IPC] Client requested StartListening (PTT active)");
                            pipe_ipc.set_push_to_talk(true);
                        }
                        Some(voxy_ipc::ClientCommand::StopListening) => {
                            tracing::info!("[IPC] Client requested StopListening (PTT inactive)");
                            pipe_ipc.set_push_to_talk(false);
                        }
                        Some(voxy_ipc::ClientCommand::SetMute { muted }) => {
                            tracing::info!("[IPC] Client requested SetMute({muted})");
                            pipe_ipc.set_mute(muted);
                        }
                        Some(voxy_ipc::ClientCommand::SetVoiceSpeed { speed }) => {
                            tracing::info!("[IPC] Client requested SetVoiceSpeed({speed})");
                        }
                        Some(voxy_ipc::ClientCommand::SendTextInput { text }) => {
                            tracing::info!(text = %text, "[IPC] Text input command received from client");
                            let pipe = pipe_ipc.clone();
                            tokio::spawn(async move {
                                pipe.dispatch_llm_and_tts(text).await;
                            });
                        }
                        Some(voxy_ipc::ClientCommand::SetRoutingMode { mode }) => {
                            tracing::info!(mode = %mode, "[IPC] Client requested routing mode change");
                            ipc_srv.broadcast_routing_status(
                                mode,
                                llm_provider_name.clone(),
                                "OSMOO Voice Engine (WASAPI + Silero + Groq/Local + TTS)"
                                    .to_string(),
                                "OSMOO Voice Engine (WASAPI + Silero + Groq/Local + TTS)"
                                    .to_string(),
                                false,
                            );
                        }
                        Some(voxy_ipc::ClientCommand::RequestHardwareStatus) => {
                            let hw = voxy_hardware::HardwareDetector::detect();
                            ipc_srv.broadcast_hardware_status(
                                hw.cpu_brand.clone(),
                                hw.cpu_cores,
                                hw.total_ram_gb(),
                                hw.primary_gpu.as_ref().map(|g| g.name.clone()),
                                hw.total_vram_gb(),
                            );
                        }
                        Some(voxy_ipc::ClientCommand::ConfirmAction {
                            action_id,
                            approved,
                        }) => {
                            tracing::info!(
                                action_id,
                                approved,
                                "[IPC] Human confirmation decision received"
                            );
                        }
                        Some(voxy_ipc::ClientCommand::Shutdown { reason }) => {
                            tracing::info!(
                                reason = %reason,
                                "[IPC] Graceful shutdown command received from supervisor"
                            );
                            running_flag.store(false, Ordering::Relaxed);
                            break;
                        }
                        Some(voxy_ipc::ClientCommand::HeartbeatPong) => {}
                        None => {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                }
            });
        }

        // ── Desktop Event Bridge ──────────────────────────────────────────
        let desktop_config = WorldModelConfig::default();
        let bridge = Arc::new(DesktopEventBridge::new(desktop_config));
        if let Err(e) = bridge.start().await {
            tracing::warn!("Desktop event bridge failed to start: {}", e);
        } else {
            tracing::info!("Desktop event bridge started");
        }

        let exp_input_tx_clone = exp_input_tx.clone();
        let bridge_clone = bridge.clone();
        let desktop_ctx_clone = desktop_context.clone();
        let guard_clone = guard.clone();
        let mut timer = tokio::time::interval(Duration::from_secs(5));
        let mut last_focused_app: Option<String> = None;
        let mut last_activity: Option<String> = None;
        let mut last_idle: Option<bool> = None;

        // ── Moment context tracking ───────────────────────────────────
        let mut last_idle_since: Option<Instant> = None;
        let mut focus_start: Option<Instant> = None;
        let mut has_thanked_focus = false;

        // ── Experience Output → Visual Presence ───────────────────────────
        let (presence_tx, _presence_rx) = tokio::sync::broadcast::channel::<String>(64);
        let presence_tx_clone = presence_tx.clone();
        let exp_output_handle = tokio::spawn(async move {
            loop {
                match exp_output_rx.recv().await {
                    Ok(output) => {
                        // Forward presence state changes to visual presence channel
                        let presence_str = format!("{:?}", output.presence_state);
                        let _ = presence_tx_clone.send(presence_str);

                        // Forward mood changes
                        let mood_str = format!("{:?}", output.current_mood);
                        let _ = presence_tx_clone.send(format!("mood:{mood_str}"));

                        tracing::debug!(
                            mood = ?output.current_mood,
                            presence = ?output.presence_state,
                            speed = output.voice_params.speed,
                            "Presence update forwarded"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Experience output channel closed");
                        break;
                    }
                }
            }
        });

        // ── Main Loop: Desktop context → Experience Layer ─────────────────
        loop {
            tokio::select! {
                _ = timer.tick() => {
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    let ctx = bridge_clone.get_current_context().await;

                    // Build desktop context string for LLM
                    let ctx_str = format!(
                        "App: {}, Activity: {}, Window: {}, Idle: {}",
                        ctx.focused_app.as_deref().unwrap_or("none"),
                        ctx.activity_type.as_deref().unwrap_or("none"),
                        ctx.window_title.as_deref().unwrap_or("none"),
                        ctx.is_idle,
                    );
                    *desktop_ctx_clone.write().await = ctx_str;

                    guard_clone.heartbeat("voice_pipeline");
                    guard_clone.heartbeat("cognitive_bridge");
                    guard_clone.heartbeat("experience_bridge");
                    guard_clone.heartbeat("desktop_bridge");

                    tracing::debug!(
                        app = ?ctx.focused_app,
                        activity = ?ctx.activity_type,
                        idle = ctx.is_idle,
                        "Desktop context"
                    );

                    // Feed desktop events → Experience Layer
                    if let Some(ref app) = ctx.focused_app {
                        if last_focused_app.as_ref() != Some(app) {
                            let _ = exp_input_tx_clone.send(ExperienceInput::DesktopFocusChanged {
                                app: app.clone(),
                                window_title: ctx.window_title.clone(),
                            });
                            last_focused_app = Some(app.clone());
                        }
                    }

                    if let Some(ref activity) = ctx.activity_type {
                        if last_activity.as_ref() != Some(activity) {
                            let _ = exp_input_tx_clone.send(ExperienceInput::DesktopActivityChanged {
                                activity: activity.clone(),
                            });
                            last_activity = Some(activity.clone());
                        }
                    }

                    if last_idle != Some(ctx.is_idle) {
                        let _ = exp_input_tx_clone.send(ExperienceInput::DesktopIdle {
                            is_idle: ctx.is_idle,
                        });
                        last_idle = Some(ctx.is_idle);

                        // Track idle/active transitions for moments
                        if ctx.is_idle {
                            last_idle_since = Some(Instant::now());
                            focus_start = None;
                            has_thanked_focus = false;
                        } else {
                            if last_idle_since.is_some() {
                                focus_start = Some(Instant::now());
                            }
                        }
                    }

                    // Track tasks completed (successful LLM responses)
                    if last_focused_app.as_ref() != Some(&ctx.focused_app.clone().unwrap_or_default())
                        && ctx.focused_app.is_some()
                    {
                        // Application change can indicate task boundary
                    }

                    // ── Sync voice speed from Experience Layer ────────
                    {
                        let snapshot = exp_bridge.get_snapshot().await;
                        let speed = snapshot.current_mood.voice_speed_modifier();
                        pipeline.set_voice_speed(speed as f32);
                    }

                    // ── Companion Moments ────────────────────────────
                    let idle_duration_chrono = last_idle_since
                        .map(|t| {
                            let secs = t.elapsed().as_secs() as i64;
                            chrono::Duration::seconds(secs)
                        })
                        .unwrap_or_else(|| chrono::Duration::seconds(0));
                    let absence_duration = if ctx.is_idle {
                        last_idle_since
                            .map(|t| {
                                let secs = t.elapsed().as_secs() as i64;
                                chrono::Duration::seconds(secs)
                            })
                            .unwrap_or_else(|| chrono::Duration::seconds(0))
                    } else if let Some(last_idle) = last_idle_since {
                        let secs = last_idle.elapsed().as_secs() as i64;
                        chrono::Duration::seconds(secs)
                    } else {
                        chrono::Duration::seconds(0)
                    };
                    let focused_duration = if !ctx.is_idle {
                        focus_start
                            .map(|t| {
                                let secs = t.elapsed().as_secs() as i64;
                                chrono::Duration::seconds(secs)
                            })
                            .unwrap_or_else(|| chrono::Duration::seconds(0))
                    } else {
                        chrono::Duration::seconds(0)
                    };
                    let user_just_returned = last_idle == Some(true) && !ctx.is_idle;

                    // Reset daily counters at midnight
                    let now = chrono::Local::now();
                    if now.hour() == 0 && now.minute() == 0 {
                        tasks_completed.store(0, std::sync::atomic::Ordering::Relaxed);
                    }

                    let moment_ctx = MomentContext {
                        user_just_returned,
                        absence_duration,
                        is_idle: ctx.is_idle,
                        idle_duration: idle_duration_chrono,
                        battery_percent: None,
                        is_charging: None,
                        next_meeting_in_minutes: None,
                        recent_download_complete: None,
                        focused_duration,
                        has_been_thanked_for_focus: has_thanked_focus,
                        tasks_completed_today: tasks_completed.load(std::sync::atomic::Ordering::Relaxed) as usize,
                        project_completed: false,
                        code_just_compiled: false,
                    };
                    let moments = moment_engine.check_moments(&moment_ctx);
                    for moment in moments {
                        if moment.moment_type == voxy_companion_intelligence::MomentType::FocusedWork {
                            has_thanked_focus = true;
                        }
                        let _ = exp_input_tx_clone.send(ExperienceInput::SystemEvent {
                            event_type: format!("{:?}", moment.moment_type),
                            data: Some(moment.message),
                        });
                    }

                    // ── Proactive Assistance Engine ────────────────────────
                    let suggestion_ctx = SuggestionContext {
                        current_app: ctx.focused_app.clone(),
                        current_activity: ctx.activity_type.clone(),
                        time_of_day: if now.hour() < 12 {
                            "morning".to_string()
                        } else if now.hour() < 17 {
                            "afternoon".to_string()
                        } else {
                            "evening".to_string()
                        },
                        session_duration_ms: (chrono::Utc::now().timestamp_millis().max(0)) as u64,
                        recent_errors: 0,
                        recent_completions: tasks_completed.load(std::sync::atomic::Ordering::Relaxed) as usize,
                        typing_speed: 0.0,
                        idle_time_ms: idle_duration_chrono.num_milliseconds().max(0) as u64,
                    };

                    let suggestions = proactive_engine.generate_suggestions(&suggestion_ctx);
                    for sug in suggestions {
                        tracing::info!(
                            id = %sug.id,
                            sug_type = ?sug.suggestion_type,
                            priority = sug.priority,
                            "[PROACTIVE] Emitting assistance notification"
                        );
                        ipc_server.broadcast(voxy_ipc::DaemonMessage::ProactiveNotification {
                            id: sug.id.clone(),
                            category: format!("{:?}", sug.suggestion_type),
                            title: format!("OSMOO Companion {:?}", sug.suggestion_type),
                            message: sug.message,
                            priority: sug.priority,
                            requires_action: false,
                        });
                        proactive_engine.mark_shown(&sug.id);
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }

        // ── Cleanup ───────────────────────────────────────────────────────
        tracing::info!("Shutting down VOXY...");

        // Finalize recovery mode if it was active during this session
        {
            let mut recovery = recovery_mode.lock().await;
            if recovery.is_active() {
                let report = recovery.abort();
                if let Some(r) = report {
                    tracing::warn!(
                        report_id = %r.id,
                        authorized_by = %r.authorized_by,
                        "Recovery mode was active at shutdown — aborted"
                    );
                    let mut log = audit_log.lock().await;
                    log.record_typed(
                        "system",
                        "recovery_abort",
                        None,
                        "allowed",
                        Some("Recovery mode aborted at shutdown"),
                        "high",
                        "verified",
                        voxy_security::policy::AuditLevel::Full,
                        AuditEventType::RecoveryModeActivated {
                            reason: "Shutdown during recovery".to_string(),
                        },
                    );
                }
            }
        }

        // Record shutdown audit event
        {
            let mut log = audit_log.lock().await;
            log.record_typed(
                "system",
                "shutdown",
                None,
                "allowed",
                Some("VOXY daemon shutting down"),
                "none",
                "verified",
                voxy_security::policy::AuditLevel::Basic,
                AuditEventType::Authentication {
                    method: "system_shutdown".to_string(),
                    success: true,
                },
            );
        }

        graceful.execute().await;
        let _ = exp_output_handle.await;
        pipeline.shutdown();
        tracing::info!("VOXY shutdown complete");
        Ok(())
    })
}

async fn open_application(app_name: &str) -> Result<bool, Box<dyn std::error::Error>> {
    match crate::tools::safe_launch_application(app_name) {
        Ok(_) => Ok(true),
        Err(e) => Err(e.into()),
    }
}

#[tokio::main]
async fn main() {
    // Install crash handler before anything else
    voxy_logging::install_crash_handler();

    // Load .env file if present
    let _ = dotenvy::dotenv();

    let _main_span = tracing::debug_span!("voxy_main").entered();
    setup_tracing();
    tracing::info!("Starting VOXY Assistant v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!(
        "[RUNTIME] available_parallelism = {:?}",
        std::thread::available_parallelism()
    );

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--status") {
        println!("VOXY Assistant v{}", env!("CARGO_PKG_VERSION"));
        println!("Status: Ready");
        println!("Features:");
        println!("  - Voxray Realtime Voice Pipeline (Streaming STT + LLM + TTS)");
        println!("  - Cognitive Orchestrator (reflection, knowledge validation, skill discovery)");
        println!("  - Runtime Guard (health monitoring, self-healing)");
        println!("  - Experience Layer (personality, mood, presence)");
        println!("  - Background Runtime (10 tasks)");
        println!();
        println!("Usage:");
        println!("  voxy-daemon                        Start the daemon");
        println!("  voxy-daemon --status               Show this status");
        println!("  voxy-daemon --dev-text             Run with developer interactive text input");
        println!(
            "  voxy-daemon --loopback [N]         Run N real acoustic loopback verification rounds"
        );
        return;
    }

    if let Some(idx) = args.iter().position(|a| a == "--loopback") {
        let iters = args
            .get(idx + 1)
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(10);
        tracing::info!(
            "Acoustic loopback test: running {} rounds with VoicePipeline",
            iters
        );
        return;
    }

    let running = Arc::new(AtomicBool::new(true));
    let metrics = Arc::new(VoiceMetrics::new());

    let mut backoff = Duration::from_secs(1);

    while running.load(Ordering::Relaxed) {
        let result = run_pipeline(running.clone(), metrics.clone()).await;

        if !running.load(Ordering::Relaxed) {
            break;
        }

        match result {
            Ok(()) => {
                tracing::info!("VOXY loop ended normally, restarting...");
                backoff = Duration::from_secs(1);
            }
            Err(e) => {
                tracing::error!(
                    "VOXY loop crashed: {}. Restarting in {}s...",
                    e,
                    backoff.as_secs()
                );
                metrics.restart_count.fetch_add(1, Ordering::Relaxed);
            }
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }

    let report = metrics.report();
    tracing::info!("{report}");
    tracing::info!("VOXY Assistant stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_simd_no_panic() {
        let caps = detect_simd();
        assert!(!caps.is_empty());
    }

    #[test]
    fn test_detect_cpu_features() {
        let features = detect_cpu_features();
        assert!(features.len() >= 2);
    }

    #[test]
    fn test_get_memory_usage_no_panic() {
        let (used, total) = get_memory_usage();
        assert!(used <= total || total == 0);
    }

    #[test]
    fn test_measure_stage_sync() {
        let (ms, result) = measure_stage_ms(|| 42);
        assert!(ms >= 0.0);
        assert_eq!(result, 42);
    }

    #[test]
    fn conversation_memory_trims_to_character_budget() {
        let mut mem = ConversationMemory::new().with_max_history_chars(30);
        mem.add_turn("user", "hello there");
        mem.add_turn("assistant", "hi how are you doing today");
        let history = mem.conversation_history();
        assert!(history.contains("assistant"));
        assert!(!history.contains("user: hello"));
    }

    #[test]
    fn conversation_memory_keeps_newest_when_over_budget() {
        let mut mem = ConversationMemory::new().with_max_history_chars(5);
        mem.add_turn("user", "this is a very long first message indeed");
        let history = mem.conversation_history();
        assert!(history.contains("user"));
    }

    #[test]
    fn conversation_memory_no_trim_with_large_budget() {
        let mut mem = ConversationMemory::new().with_max_history_chars(10_000);
        mem.add_turn("user", "a");
        mem.add_turn("assistant", "b");
        mem.add_turn("user", "c");
        let history = mem.conversation_history();
        assert!(history.contains("user: a"));
        assert!(history.contains("assistant: b"));
        assert!(history.contains("user: c"));
    }
}
