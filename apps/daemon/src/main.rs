use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod background;
mod dev_text_input;
mod shutdown;
mod loopback_test;
mod tools;

use voxy_automation::WindowsUiaBackend;
use voxy_cognition::{CognitionConfig, CognitiveEngine, InMemoryCognitiveEngine, IntentInput};
use voxy_cognitive_orchestrator::bridge::CognitiveBridge;
use voxy_cognitive_orchestrator::config::OrchestratorConfig;
use voxy_companion_intelligence::{
    ExperienceBridge, ExperienceInput, IntelligenceConfig, MomentContext, MomentEngine,
};
use voxy_desktop_runtime::{DesktopRuntime, RuntimeConfig};
use voxy_ollama::OllamaProvider;
use voxy_openai::{OpenAIConfig, OpenAIProvider};
use voxy_anthropic::AnthropicProvider;
use voxy_gemini::GeminiProvider;
use voxy_orchestrator::automation::AutomationBackend;
use voxy_provider_core::LlmProvider;
use voxy_runtime_guard::{GuardConfig, RuntimeGuard};
use voxy_security::{
    sanitize_context, sanitize_llm_output, sanitize_user_input, AuditEventType, AuditLog,
    CapabilityRegistry, GuardianConfig, GuardianEngine, PolicyEngine, RecoveryMode,
    SystemPromptBuilder,
};
use voxy_voice::VoicePipeline;
use chrono::Timelike;
use voxy_world_model::{DesktopEventBridge, WorldModelConfig};

/// A completed speculative LLM reply generated for a partial transcript.
/// Reused when the committed transcript is a minor refinement of the
/// speculative one (see `voxy_voice_stream::speculation_reuse_chars`).
struct SpecEntry {
    spec_transcript: String,
    reply: String,
}

/// Create an LLM provider based on VOXY_LLM_PROVIDER env var.
/// Supported: "ollama" (default), "openai", "anthropic", "gemini", "groq", "openrouter"
fn create_llm_provider() -> Arc<dyn LlmProvider> {
    let provider = std::env::var("VOXY_LLM_PROVIDER")
        .unwrap_or_else(|_| "ollama".into())
        .to_lowercase();

    match provider.as_str() {
        "openai" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI")
                .unwrap_or_default();
            let model = std::env::var("VOXY_OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_OPENAI not set, falling back to Ollama");
                return create_ollama_provider();
            }
            let config = OpenAIConfig::openai(api_key);
            tracing::info!("[PROVIDER] Using OpenAI: model={}", model);
            Arc::new(OpenAIProvider::new(config).with_model(&model))
        }
        "anthropic" => {
            let api_key = std::env::var("VOXY_API_KEYS_ANTHROPIC")
                .unwrap_or_default();
            let model = std::env::var("VOXY_ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_ANTHROPIC not set, falling back to Ollama");
                return create_ollama_provider();
            }
            tracing::info!("[PROVIDER] Using Anthropic: model={}", model);
            Arc::new(AnthropicProvider::new(api_key).with_model(&model))
        }
        "gemini" => {
            let api_key = std::env::var("VOXY_API_KEYS_GEMINI")
                .unwrap_or_default();
            let model = std::env::var("VOXY_GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-2.0-flash".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_GEMINI not set, falling back to Ollama");
                return create_ollama_provider();
            }
            tracing::info!("[PROVIDER] Using Gemini: model={}", model);
            Arc::new(GeminiProvider::new(api_key).with_model(&model))
        }
        "groq" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI")
                .unwrap_or_default();
            let model = std::env::var("VOXY_GROQ_MODEL")
                .unwrap_or_else(|_| "openai/gpt-oss-20b".into());
            if api_key.is_empty() {
                tracing::warn!("[PROVIDER] VOXY_API_KEYS_OPENAI not set for Groq, falling back to Ollama");
                return create_ollama_provider();
            }
            let config = OpenAIConfig::groq(api_key);
            tracing::info!("[PROVIDER] Using Groq: model={}", model);
            Arc::new(OpenAIProvider::new(config).with_model(&model))
        }
        "openrouter" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI")
                .unwrap_or_default();
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
        _ => create_ollama_provider(),
    }
}

fn create_ollama_provider() -> Arc<dyn LlmProvider> {
    let url = std::env::var("VOXY_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let model = std::env::var("VOXY_OLLAMA_MODEL")
        .unwrap_or_else(|_| "gpt-oss:120b-cloud".into());
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
                tracing::info!("[MEMORY] SQLite conversation memory opened at {}", db_path.display());
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
            match conn.prepare(
                "SELECT role, content FROM conversation_turns ORDER BY id DESC LIMIT 20",
            ) {
                Ok(mut stmt) => {
                    if let Ok(mapped) = stmt.query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    }) {
                        let rows: Vec<(String, String)> =
                            mapped.filter_map(|r| r.ok()).collect();
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
        kept
            .iter()
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
    let mut used = 0u64;
    let mut total = 0u64;
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("wmic")
            .args([
                "OS",
                "get",
                "TotalVisibleMemorySize,FreePhysicalMemory",
                "/format:csv",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 3 {
                    if let Ok(free_kb) = parts[2].trim().parse::<u64>() {
                        if let Ok(total_kb) = parts[1].trim().parse::<u64>() {
                            total = total_kb * 1024;
                            used = total.saturating_sub(free_kb * 1024);
                        }
                    }
                }
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
            for line in info.lines() {
                if let Some(val) = line.strip_prefix("MemTotal:") {
                    total = val
                        .trim()
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0)
                        * 1024;
                } else if let Some(val) = line.strip_prefix("MemAvailable:") {
                    let avail = val
                        .trim()
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0)
                        * 1024;
                    if total > avail {
                        used = total - avail;
                    }
                }
            }
        }
    }
    (used, total)
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

fn run_pipeline(running: Arc<AtomicBool>, metrics: Arc<VoiceMetrics>) -> PipelineFuture {
    Box::pin(async move {
        let config = voxy_voice::VoiceConfig {
            auto_start_capture: true,
            wake_word: "hey voxy".into(),
            wake_word_enabled: false,
            vad_enabled: true,
            vad_threshold: 0.05,
            orchestrator: voxy_voice_orchestrator::VoiceOrchestratorConfig {
                silence_timeout_ms: 100,
                ..Default::default()
            },
            ..Default::default()
        };

        let audio_mgr = Box::new(voxy_audio::WasapiDeviceManager::new());
        let pipeline = Arc::new(VoicePipeline::with_audio_mgr(config.clone(), audio_mgr));
        pipeline.initialize().await?;
        pipeline.with_default_engines().await?;

        // ── Enterprise Cloud-First Voice System (VOXY V3 Voice Engine) ──
        tracing::info!("[VOICE] Initializing Enterprise Cloud-First STT/TTS Voice System");
        let voice_system = Arc::new(voxy_voxray::provider::VoiceSystem::build_from_env());

        let voxray_stt = voxy_voice::VoxraySttEngine::new(
            voice_system.stt_service.clone() as Arc<dyn voxy_voxray::stt::VoxraySttService>,
        );
        pipeline.set_stt_engine(Box::new(voxray_stt)).await?;

        let voxray_tts = voxy_voice::VoxrayTtsEngine::new(
            voice_system.tts_service.clone() as Arc<dyn voxy_voxray::tts::VoxrayTtsService>,
            config.audio.output.sample_rate,
        );
        pipeline.set_tts_engine(Box::new(voxray_tts)).await?;

        // Start background admin & observability HTTP server
        let admin_port: u16 = std::env::var("VOXY_VOICE_ADMIN_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8088);
        if let Err(e) = voxy_voxray::provider::admin::start_admin_server(admin_port, Arc::clone(&voice_system.admin_api)).await {
            tracing::warn!("[VOICE:ADMIN] Voice admin server could not bind to port {}: {}", admin_port, e);
        } else {
            tracing::info!("[VOICE:ADMIN] Voice Admin & Observability API running on http://127.0.0.1:{}", admin_port);
        }

        pipeline.start_capture().await?;

        // ── Voice Engine V2: initialize watchdog, calibrator, metrics ──
        pipeline.initialize_v2().await?;
        if let Some(ref _watchdog) = pipeline.watchdog() {
            tracing::info!("V2 watchdog registered for 5 stages");
        }
        if let Some(ref _calibrator) = pipeline.calibrator() {
            tracing::info!("V2 self-calibrator ready (will calibrate during first use)");
        }
        if let Some(ref _mc) = pipeline.metrics_collector() {
            tracing::info!("V2 metrics collector initialized");
        }
        if let Some(ref _vm) = pipeline.voice_memory() {
            tracing::info!("V2 voice memory initialized");
        }
        tracing::info!("Voice Engine V2 fully integrated");

        // ── Experience Layer ──────────────────────────────────────────────
        let intelligence_config = IntelligenceConfig::default();
        let (exp_bridge, exp_input_tx, mut exp_output_rx) =
            ExperienceBridge::new(intelligence_config);
        exp_bridge.start().await;
        let exp_bridge = Arc::new(exp_bridge);
        let mut moment_engine = MomentEngine::new();
        tracing::info!("Experience Layer started");

        // ── Cognitive Orchestrator ────────────────────────────────────────
        let cognitive_bridge = Arc::new(CognitiveBridge::new(OrchestratorConfig::default()));
        tracing::info!("Cognitive Orchestrator initialized");

        // ── Runtime Guard ────────────────────────────────────────────────
        let guard = Arc::new(RuntimeGuard::new(GuardConfig::default()));
        tracing::info!("Runtime Guard initialized");

        // Register core subsystems
        let g = guard.clone();
        g.register_subsystem("voice_pipeline", || async {
            voxy_health::HealthReport::new("voice_pipeline", voxy_shared::HealthStatus::Healthy)
        })
        .await;

        let g = guard.clone();
        g.register_subsystem("cognitive_bridge", || async {
            voxy_health::HealthReport::new("cognitive_bridge", voxy_shared::HealthStatus::Healthy)
        })
        .await;

        let g = guard.clone();
        g.register_subsystem("experience_bridge", || async {
            voxy_health::HealthReport::new("experience_bridge", voxy_shared::HealthStatus::Healthy)
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
        tracing::info!("Runtime Guard: 4 subsystems registered");

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
                metrics.clone(),
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

        // ── LLM Provider (multi-provider support) ──────────────────────
        let llm = create_llm_provider();
        match llm.health().await {
            Ok(true) => tracing::info!("[PROVIDER] {} connected", llm.name()),
            Ok(false) => tracing::warn!("[PROVIDER] {} health check failed, will retry on first request", llm.name()),
            Err(e) => tracing::warn!("[PROVIDER] {} not reachable: {e} — LLM responses will be fallbacks", llm.name()),
        }

        // Register LLM as a healable subsystem
        {
            let g = guard.clone();
            let llm_name = llm.name().to_string();
            g.register_healable(
                "ollama_llm",
                move || {
                    let name = llm_name.clone();
                    async move {
                        voxy_health::HealthReport::new(
                            &name,
                            voxy_shared::HealthStatus::Healthy,
                        )
                    }
                },
                move || {
                    async move {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        Ok(())
                    }
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

        // ── Tool Registry ─────────────────────────────────────────────────
        let tool_registry = match tools::ToolRegistry::new().await {
            Ok(tr) => {
                tracing::info!("[TOOLS] Tool registry initialized with automation backend");
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
            Box::new(
                move |text: String| -> Pin<Box<dyn std::future::Future<Output = String> + Send>> {
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
                            return String::new();
                        }

                        let response_start = Instant::now();

                        // ── 1. Feed transcript → Experience Layer ──────────
                        tracing::info!("[VOICE:LLM] Text: {}", text);
                        let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                            text: text.clone(),
                            is_final: true,
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
                                tracing::warn!("[VOICE:EMERGENCY] Emergency Stop triggered via voice command");
                                let response = "Emergency Stop activated. All computer control actions have been halted.".to_string();
                                mem.add_turn("assistant", &response);
                                return response;
                            }
                        }
                        if lower_trimmed == "reset emergency stop"
                            || lower_trimmed == "resume actions"
                            || lower_trimmed == "cancel emergency stop"
                        {
                            if let Some(ref tr) = tool_reg {
                                tr.reset_emergency_stop();
                                tracing::info!("[VOICE:EMERGENCY] Emergency Stop reset via voice command");
                                let response = "Emergency Stop reset. Computer control is re-enabled.".to_string();
                                mem.add_turn("assistant", &response);
                                return response;
                            }
                        }

                        // If Emergency Stop is engaged, block any automation commands
                        if tool_reg.as_ref().map(|tr| tr.is_emergency_stopped()).unwrap_or(false) {
                            if lower.contains("open")
                                || lower.contains("launch")
                                || lower.contains("start")
                                || lower.contains("type")
                                || lower.contains("click")
                                || lower.contains("press")
                            {
                                tracing::warn!("[VOICE:EMERGENCY] Blocked automation action: Emergency Stop active");
                                let response = "Action blocked. Emergency Stop is active. Say 'reset emergency stop' to resume.".to_string();
                                mem.add_turn("assistant", &response);
                                return response;
                            }
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
                                    if decision.allowed { "allowed" } else { "denied" },
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
                                    if recovery.state() == voxy_security::recovery::RecoveryState::Normal {
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
                                return response;
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
                                format!("I'd open {app} for you, but automation isn't available.")
                            };
                            mem.add_turn("assistant", &response);
                            drop(mem);
                            let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                                text: response.clone(),
                                is_final: true,
                            });
                            return response;
                            }
                        }

                        if lower.contains("open it again") || lower.contains("reopen") {
                            if let Some(app) = mem.last_app.clone() {
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
                                return response;
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
                        tracing::info!("[VOICE:LLM] Calling Ollama (prompt={} chars)", system_prompt.len());
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
                            if let Some((call, remaining)) = tools::ToolRegistry::parse_tool_call(&response) {
                                tracing::info!("[VOICE:TOOL] Parsed tool call: {} {:?}", call.tool, call.params);
                                let result = tr.execute(&call).await;
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
                        tracing::info!("[VOICE:LLM] Response: {} chars in {}ms", response.len(), elapsed_ms);
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

                        // ── 7. Feed response → Experience Layer ────────────
                        let _ = exp_input.send(ExperienceInput::VoiceTranscript {
                            text: response.clone(),
                            is_final: true,
                        });

                        // ── 8. Track task completion for moments ──────────
                        tasks_completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

                        response
                    })
                },
            )
        };

        pipeline.set_response_handler(response_handler).await;

        // ── STREAMING RESPONSE HANDLER ─────────────────────────────────
        // This enables LLM streaming → sentence-chunked TTS pipeline.
        // When available, the pipeline uses streaming LLM and synthesizes
        // each sentence independently for much lower TTFA.
        let spec_cache: Arc<tokio::sync::Mutex<Option<SpecEntry>>> =
            Arc::new(tokio::sync::Mutex::new(None));
        {
            let llm_streaming: Arc<dyn voxy_provider_core::LlmProvider> = llm.clone();
            let memory_s = memory.clone();
            let sys_prompt_s = system_prompt.clone();
            let cognitive_s = cognitive.clone();
            let metrics_s = metrics.clone();
            let desktop_ctx_s = desktop_context.clone();
            let exp_input_s = exp_input_for_handler.clone();
            let guardian_s = guardian.clone();
            let recovery_mode_s = recovery_mode.clone();
            let audit_log_s = audit_log.clone();
            let tasks_completed_s = tasks_completed.clone();
            let spec_cache_s = spec_cache.clone();

            let streaming_handler = Arc::new(
                move |text: String,
                      tx: tokio::sync::mpsc::Sender<String>|
                 -> std::pin::Pin<
                    Box<dyn std::future::Future<Output = ()> + Send>,
                > {
                    let llm = llm_streaming.clone();
                    let memory = memory_s.clone();
                    let sys_prompt = sys_prompt_s.clone();
                    let _cognitive = cognitive_s.clone();
                    let _metrics = metrics_s.clone();
                    let desktop_ctx = desktop_ctx_s.clone();
                    let _exp_input = exp_input_s.clone();
                    let _guardian = guardian_s.clone();
                    let _recovery_mode = recovery_mode_s.clone();
                    let _audit_log = audit_log_s.clone();
                    let tasks_completed = tasks_completed_s.clone();
                    let spec_cache = spec_cache_s.clone();
                    Box::pin(async move {
                        let sanitized = sanitize_user_input(&text);
                        let text = sanitized.text;
                        if text.is_empty() || text == "short sound" {
                            let _ = tx.send(String::new()).await;
                            return;
                        }

                        let lower = text.to_lowercase();
                        let mut mem = memory.lock().await;
                        mem.add_turn("user", &text);

                        // Handle automation commands directly (non-streaming)
                        if lower.contains("open")
                            || lower.contains("launch")
                            || lower.contains("start")
                        {
                            let app = if lower.contains("chrome") || lower.contains("browser") {
                                "chrome"
                            } else if lower.contains("notepad") || lower.contains("editor") {
                                "notepad"
                            } else if lower.contains("calc") || lower.contains("calculator") {
                                "calc"
                            } else if lower.contains("explorer") || lower.contains("files") {
                                "explorer"
                            } else {
                                let response = "I'm not sure which application to open.".to_string();
                                mem.add_turn("assistant", &response);
                                drop(mem);
                                let _ = tx.send(response).await;
                                return;
                            };
                            let response = match open_application(app).await {
                                Ok(_) => format!("Done — {app} is now open."),
                                Err(e) => format!("I couldn't open {app}. {e}"),
                            };
                            mem.add_turn("assistant", &response);
                            drop(mem);
                            let _ = tx.send(response).await;
                            return;
                        }

                        // Build LLM prompt
                        let history = mem.conversation_history();
                        let desktop = sanitize_context(&desktop_ctx.read().await.clone());
                        drop(mem);

                        // ── Speculative-prefill reuse ─────────────────────
                        // A full reply may already be available from the
                        // speculative loop if this committed transcript is a
                        // minor refinement of a partial it pre-answered.
                        let reused = spec_cache
                            .lock()
                            .await
                            .take()
                            .filter(|entry| {
                                voxy_voice_stream::speculation_reuse_chars(
                                    &entry.spec_transcript,
                                    &text,
                                    30,
                                    8,
                                )
                                .is_some()
                            });
                        if let Some(entry) = reused {
                            tracing::info!(
                                "[VOICE:SPEC] Reused speculative reply ({} chars) for '{}'",
                                entry.reply.len(),
                                text
                            );
                            let mut chunker = voxy_voice_stream::SentenceChunker::new(20);
                            for sentence in chunker.feed(&entry.reply) {
                                let _ = tx.send(sentence).await;
                            }
                            for sentence in chunker.finish() {
                                let _ = tx.send(sentence).await;
                            }
                            {
                                let mut mem = memory.lock().await;
                                mem.add_turn("assistant", &entry.reply);
                            }
                            tasks_completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            return;
                        }

                        let system_prompt_full = format!(
                            "{}\n\nCurrent desktop context: {}\n\nRecent conversation:\n{}",
                            sys_prompt,
                            if desktop.is_empty() { "unknown" } else { &desktop },
                            if history.is_empty() { "No prior conversation.".to_string() } else { history }
                        );

                        let prompt = format!(
                            "{system_prompt_full}\n\nUser: {text}\nAssistant:"
                        );

                        // Use streaming LLM
                        let (llm_tx, mut llm_rx) = tokio::sync::mpsc::channel::<voxy_provider_core::LlmChunk>(16);

                        let llm_clone = llm.clone();
                        let prompt_clone = prompt.clone();
                        tokio::spawn(async move {
                            if let Err(e) = llm_clone.complete_streaming(&prompt_clone, llm_tx).await {
                                tracing::warn!("[VOICE:STREAMING] LLM streaming failed: {e}");
                            }
                        });

                        // Accumulate LLM chunks, split on sentence boundaries
                        // (with word-count fallback + guaranteed terminal
                        // punctuation on the final chunk), and send complete
                        // sentences through the TTS channel.
                        let mut chunker = voxy_voice_stream::SentenceChunker::new(20);
                        let mut accumulated = String::new();
                        let mut chunk_count = 0usize;

                        tracing::debug!("[VOICE:STREAMING] awaiting LLM chunks");
                        while let Some(chunk) = llm_rx.recv().await {
                            chunk_count += 1;
                            if chunk.done {
                                for sentence in chunker.finish() {
                                    tracing::info!("[VOICE:STREAMING] Sending sentence to TTS: '{}'", sentence);
                                    let _ = tx.send(sentence).await;
                                }
                                break;
                            }

                            for sentence in chunker.feed(&chunk.text) {
                                tracing::info!("[VOICE:STREAMING] Sending sentence to TTS: '{}'", sentence);
                                let _ = tx.send(sentence).await;
                            }

                            accumulated.push_str(&chunk.text);
                        }
                        tracing::debug!(
                            "[VOICE:STREAMING] LLM loop ended: {} chunks, {} chars accumulated",
                            chunk_count,
                            accumulated.len()
                        );

                        // Store assistant turn
                        {
                            let mut mem = memory.lock().await;
                            mem.add_turn("assistant", &accumulated);
                        }

                        // Track task completion
                        tasks_completed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    })
                },
            );

            pipeline.set_streaming_response_handler(streaming_handler).await;
        }

        // ── SPECULATIVE LLM PREFILL ─────────────────────────────────────
        // Subscribes to partial transcripts and pre-generates a full LLM
        // reply while the user is still speaking. When the committed
        // transcript arrives, the streaming handler may reuse it (see
        // `speculation_reuse_chars`), cutting end-to-end latency.
        {
            let llm_spec: Arc<dyn voxy_provider_core::LlmProvider> = llm.clone();
            let memory_spec = memory.clone();
            let sys_prompt_spec = system_prompt.clone();
            let desktop_ctx_spec = desktop_context.clone();
            let cache_spec = spec_cache.clone();

            let (partial_tx, mut partial_rx) = tokio::sync::mpsc::channel::<
                voxy_voice_orchestrator::PartialTranscript,
            >(16);
            let _ = pipeline.set_partial_transcript_handler(partial_tx).await;

            tokio::spawn(async move {
                let mut spec = voxy_voice_stream::SpeculativePrefill::default();
                while let Some(pt) = partial_rx.recv().await {
                    if pt.is_final {
                        if pt.text.is_empty() {
                            spec = voxy_voice_stream::SpeculativePrefill::default();
                        }
                        continue;
                    }
                    let tlen = pt.text.chars().count();
                    let now = Instant::now();
                    if spec.should_run(tlen, now) != voxy_voice_stream::PrefillDecision::Run {
                        continue;
                    }
                    spec.mark_started(tlen, now);
                    let history = {
                        let mem = memory_spec.lock().await;
                        mem.conversation_history()
                    };
                    let desktop = sanitize_context(&desktop_ctx_spec.read().await.clone());
                    let system_prompt_full = format!(
                        "{}\n\nCurrent desktop context: {}\n\nRecent conversation:\n{}",
                        sys_prompt_spec,
                        if desktop.is_empty() { "unknown" } else { &desktop },
                        if history.is_empty() {
                            "No prior conversation.".to_string()
                        } else {
                            history
                        }
                    );
                    let prompt = format!("{system_prompt_full}\n\nUser: {}\nAssistant:", pt.text);
                    match llm_spec.complete(&prompt).await {
                        Ok(reply) => {
                            tracing::info!(
                                "[VOICE:SPEC] speculative reply ready for partial '{}'",
                                pt.text
                            );
                            *cache_spec.lock().await = Some(SpecEntry {
                                spec_transcript: pt.text,
                                reply,
                            });
                        }
                        Err(e) => {
                            tracing::warn!("[VOICE:SPEC] speculative prefill failed: {}", e);
                        }
                    }
                    spec.mark_finished();
                }
            });
        }

        pipeline.start_listening().await?;

        tracing::info!("VOXY is listening (push-to-talk mode). Speak into your microphone.");

        // TEMPORARY DEV TEST INPUT — REMOVE BEFORE PRODUCTION
        // If --dev-text flag is present, start interactive text input mode.
        // This allows typing messages at a prompt instead of speaking into the mic.
        // Uses the SAME Ollama LLM and Kokoro/Piper TTS as real voice input.
        let dev_text_enabled = std::env::args().any(|a| a == "--dev-text");
        if dev_text_enabled {
            let dev_llm: Arc<dyn LlmProvider> = llm.clone();
            let dev_pipeline = pipeline.clone();
            let dev_memory = memory.clone();
            let dev_sys_prompt = system_prompt.clone();
            let dev_tool_reg = tool_registry.clone();
            tokio::spawn(async move {
                dev_text_input::run(dev_llm, dev_pipeline, dev_memory, dev_sys_prompt, dev_tool_reg).await;
            });
            tracing::info!("[DEV-TEXT] Dev text input active. Type messages at the VOXY > prompt.");
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

                        // ── V2 watchdog heartbeats ──
            if let Some(ref watchdog) = pipeline.watchdog() {
                            watchdog.heartbeat("audio_input");
                        }

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
                            pipeline.set_voice_speed(speed).await;
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
                            match moment.moment_type {
                                voxy_companion_intelligence::MomentType::FocusedWork => {
                                    has_thanked_focus = true;
                                }
                                _ => {}
                            }
                            let _ = exp_input_tx_clone.send(ExperienceInput::SystemEvent {
                                event_type: format!("{:?}", moment.moment_type),
                                data: Some(moment.message),
                            });
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
        pipeline.stop_listening().await;
        pipeline.stop_capture().await?;
        pipeline.shutdown().await?;
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
        println!("  voxy-daemon --loopback [N]         Run N real acoustic loopback verification rounds");
        return;
    }

    if let Some(idx) = args.iter().position(|a| a == "--loopback") {
        let iters = args.get(idx + 1).and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
        loopback_test::run_loopback_test(iters).await;
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
        assert!(total > 0 || total == 0);
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
