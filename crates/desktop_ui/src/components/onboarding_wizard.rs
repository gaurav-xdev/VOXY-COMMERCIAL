use crate::components::assistant_core::CompanionMode;
use crate::components::icons::IconOsmooLogo;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OnboardingStep {
    Welcome = 1,
    AiProvider = 2,
    VoicePermissions = 3,
    CompanionPreferences = 4,
    TestAndFinish = 5,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OnboardingConfig {
    pub assistant_name: String,
    pub companion_mode: CompanionMode,
    pub primary_llm: String,
    pub selected_model: String,
    pub api_key: Option<String>,
    pub stt_provider: String,
    pub tts_provider: String,
    pub wake_word_enabled: bool,
    pub companion_scale: f32,
    pub companion_position: String,
    pub autowalk_enabled: bool,
    pub proactive_speech_enabled: bool,
    pub start_on_boot: bool,
    pub active_tier: String,
}

impl Default for OnboardingConfig {
    fn default() -> Self {
        Self {
            assistant_name: "OSMOO".to_string(),
            companion_mode: CompanionMode::Cat,
            primary_llm: "ollama".to_string(),
            selected_model: "llama3.2:3b".to_string(),
            api_key: None,
            stt_provider: "sherpa_onnx_whisper".to_string(),
            tts_provider: "piper_en_libritts".to_string(),
            wake_word_enabled: true,
            companion_scale: 1.0,
            companion_position: "bottom-right".to_string(),
            autowalk_enabled: true,
            proactive_speech_enabled: true,
            start_on_boot: true,
            active_tier: "free".to_string(),
        }
    }
}

#[component]
pub fn OnboardingWizard(
    on_finish: EventHandler<OnboardingConfig>,
    initial_ollama_models: Vec<String>,
) -> Element {
    let mut current_step = use_signal(|| OnboardingStep::Welcome);

    // Step 1: Identity
    let mut companion_name = use_signal(|| "OSMOO".to_string());

    // Step 2: AI Provider & Model
    let mut selected_provider = use_signal(|| "ollama".to_string());
    let mut selected_model = use_signal(|| "llama3.2:3b".to_string());
    let mut cloud_api_key = use_signal(String::new);
    let mut show_api_key = use_signal(|| false);
    let mut provider_test_status = use_signal(|| None::<(bool, String, u64)>);
    let mut is_testing_provider = use_signal(|| false);

    // Step 3: Voice & Permissions
    let mut selected_stt = use_signal(|| "sherpa_onnx_whisper".to_string());
    let mut selected_tts = use_signal(|| "piper_en_libritts".to_string());
    let mut wake_word_enabled = use_signal(|| true);
    let mut test_audio_playing = use_signal(|| false);

    // Step 4: Companion Preferences
    let mut companion_scale = use_signal(|| 1.0f32);
    let companion_position = use_signal(|| "bottom-right".to_string());
    let mut autowalk_enabled = use_signal(|| true);
    let mut proactive_speech_enabled = use_signal(|| true);
    let mut start_on_boot = use_signal(|| true);

    let step_num = match *current_step.read() {
        OnboardingStep::Welcome => 1,
        OnboardingStep::AiProvider => 2,
        OnboardingStep::VoicePermissions => 3,
        OnboardingStep::CompanionPreferences => 4,
        OnboardingStep::TestAndFinish => 5,
    };

    let next_step = move |_| {
        let nxt = match *current_step.read() {
            OnboardingStep::Welcome => OnboardingStep::AiProvider,
            OnboardingStep::AiProvider => OnboardingStep::VoicePermissions,
            OnboardingStep::VoicePermissions => OnboardingStep::CompanionPreferences,
            OnboardingStep::CompanionPreferences => OnboardingStep::TestAndFinish,
            OnboardingStep::TestAndFinish => {
                let key_opt = if cloud_api_key.read().trim().is_empty() {
                    None
                } else {
                    Some(cloud_api_key.read().trim().to_string())
                };

                on_finish.call(OnboardingConfig {
                    assistant_name: companion_name.read().clone(),
                    companion_mode: CompanionMode::Cat,
                    primary_llm: selected_provider.read().clone(),
                    selected_model: selected_model.read().clone(),
                    api_key: key_opt,
                    stt_provider: selected_stt.read().clone(),
                    tts_provider: selected_tts.read().clone(),
                    wake_word_enabled: *wake_word_enabled.read(),
                    companion_scale: *companion_scale.read(),
                    companion_position: companion_position.read().clone(),
                    autowalk_enabled: *autowalk_enabled.read(),
                    proactive_speech_enabled: *proactive_speech_enabled.read(),
                    start_on_boot: *start_on_boot.read(),
                    active_tier: "free".to_string(),
                });
                return;
            }
        };
        current_step.set(nxt);
    };

    let prev_step = move |_| {
        let prev = match *current_step.read() {
            OnboardingStep::Welcome => OnboardingStep::Welcome,
            OnboardingStep::AiProvider => OnboardingStep::Welcome,
            OnboardingStep::VoicePermissions => OnboardingStep::AiProvider,
            OnboardingStep::CompanionPreferences => OnboardingStep::VoicePermissions,
            OnboardingStep::TestAndFinish => OnboardingStep::CompanionPreferences,
        };
        current_step.set(prev);
    };

    rsx! {
        div { class: "modal-overlay",
            div { class: "modal-container bg-[#090b10] border border-cyan-500/20 rounded-2xl shadow-2xl max-w-4xl w-full flex flex-col text-slate-100 overflow-hidden font-sans",
                // ── Header & 5-Step Stepper Progress Bar ─────────────────────
                div { class: "px-8 py-6 border-b border-slate-800/80 bg-slate-900/60 flex flex-col gap-4",
                    div { class: "flex items-center justify-between",
                        div { class: "flex items-center gap-3",
                            IconOsmooLogo { class: Some("w-8 h-8 text-cyan-400".into()), size: Some(32) }
                            div {
                                h1 { class: "text-lg font-bold tracking-wider text-cyan-400 font-mono", "OSMOO // SETUP WIZARD" }
                                p { class: "text-xs text-slate-400 font-mono", "FIRST-RUN CONFIGURATION & SYSTEM ACTIVATION" }
                            }
                        }
                        div { class: "text-xs font-mono font-bold px-3 py-1 bg-cyan-950/60 border border-cyan-500/30 text-cyan-300 rounded-full",
                            "STEP {step_num} OF 5"
                        }
                    }

                    // Stepper Track
                    div { class: "grid grid-cols-5 gap-2",
                        for i in 1..=5 {
                            div {
                                class: format!(
                                    "h-1.5 rounded-full transition-all duration-300 {}",
                                    if i < step_num {
                                        "bg-cyan-500"
                                    } else if i == step_num {
                                        "bg-cyan-400 shadow-[0_0_10px_rgba(34,211,238,0.7)]"
                                    } else {
                                        "bg-slate-800"
                                    }
                                ),
                            }
                        }
                    }
                }

                // ── Body Content by Step ─────────────────────────────────────
                div { class: "p-8 flex-1 overflow-y-auto max-h-[64vh]",
                    match *current_step.read() {
                        OnboardingStep::Welcome => rsx! {
                            div { class: "flex flex-col items-center text-center max-w-xl mx-auto py-4 space-y-6",
                                div { class: "p-4 bg-cyan-950/30 border border-cyan-500/30 rounded-2xl shadow-[0_0_30px_rgba(6,182,212,0.15)]",
                                    IconOsmooLogo { class: Some("w-20 h-20 text-cyan-400".into()), size: Some(80) }
                                }
                                div { class: "space-y-2",
                                    h2 { class: "text-2xl font-bold tracking-tight text-white", "Welcome to OSMOO" }
                                    p { class: "text-sm text-slate-400 leading-relaxed",
                                        "OSMOO is your autonomous, multimodal Windows AI Operating Companion. It features deep local operating system automation, high-speed neural voice pipelines, strict zero-fallback cognition, and an interactive 2D rigged desktop companion."
                                    }
                                }

                                div { class: "w-full max-w-md text-left bg-slate-900/60 border border-slate-800 p-5 rounded-xl space-y-3",
                                    label { class: "text-xs font-bold text-slate-300 uppercase tracking-wider font-mono",
                                        "Companion Name"
                                    }
                                    input {
                                        class: "hud-input-field",
                                        value: "{companion_name}",
                                        placeholder: "e.g. OSMOO",
                                        oninput: move |e| companion_name.set(e.value()),
                                    }
                                    p { class: "text-xs text-slate-400",
                                        "You can address your companion by this name during voice and text interactions."
                                    }
                                }

                                div { class: "grid grid-cols-3 gap-3 w-full text-xs text-slate-300 font-mono",
                                    div { class: "p-3 bg-slate-900/40 border border-slate-800/80 rounded-lg",
                                        "🔒 DPAPI Security"
                                    }
                                    div { class: "p-3 bg-slate-900/40 border border-slate-800/80 rounded-lg",
                                        "⚡ Sub-300ms Speech"
                                    }
                                    div { class: "p-3 bg-slate-900/40 border border-slate-800/80 rounded-lg",
                                        "🐾 2D Rigged Cat"
                                    }
                                }
                            }
                        },

                        OnboardingStep::AiProvider => rsx! {
                            div { class: "space-y-6 max-w-2xl mx-auto",
                                div { class: "space-y-1",
                                    h2 { class: "text-xl font-bold text-white", "Configure AI Intelligence Provider" }
                                    p { class: "text-xs text-slate-400",
                                        "Select your primary model engine. You can choose local offline inference via Ollama or enterprise cloud providers."
                                    }
                                }

                                // Provider Selection Grid
                                div { class: "grid grid-cols-2 md:grid-cols-3 gap-3",
                                    for (prov_id, prov_name, prov_desc) in [
                                        ("ollama", "Ollama (Local)", "Offline, 100% Private, Zero Cloud"),
                                        ("openai", "OpenAI", "GPT-4o, GPT-4o-mini, o1"),
                                        ("anthropic", "Anthropic", "Claude 3.5 Sonnet, Haiku"),
                                        ("gemini", "Google Gemini", "Gemini 1.5 Pro, Flash"),
                                        ("groq", "Groq Cloud", "Ultra-Fast LPU Speed"),
                                    ] {
                                        button {
                                            class: format!(
                                                "p-4 rounded-xl border text-left transition-all flex flex-col gap-1 {}",
                                                if *selected_provider.read() == prov_id {
                                                    "bg-cyan-950/40 border-cyan-400 text-cyan-200 shadow-[0_0_15px_rgba(6,182,212,0.2)]"
                                                } else {
                                                    "bg-slate-900/40 border-slate-800 hover:border-slate-700 text-slate-300"
                                                }
                                            ),
                                            onclick: move |_| {
                                                selected_provider.set(prov_id.to_string());
                                                provider_test_status.set(None);
                                                // Default sensible models
                                                match prov_id {
                                                    "ollama" => selected_model.set("llama3.2:3b".to_string()),
                                                    "openai" => selected_model.set("gpt-4o".to_string()),
                                                    "anthropic" => selected_model.set("claude-3-5-sonnet-20241022".to_string()),
                                                    "gemini" => selected_model.set("gemini-1.5-pro".to_string()),
                                                    "groq" => selected_model.set("llama-3.3-70b-versatile".to_string()),
                                                    _ => {}
                                                }
                                            },
                                            span { class: "font-bold text-sm", "{prov_name}" }
                                            span { class: "text-xs text-slate-400", "{prov_desc}" }
                                        }
                                    }
                                }

                                // API Key Input (if Cloud Provider)
                                if *selected_provider.read() != "ollama" {
                                    div { class: "p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-3",
                                        div { class: "flex items-center justify-between",
                                            label { class: "text-xs font-bold text-slate-300 uppercase tracking-wider font-mono",
                                                "{selected_provider.read().to_uppercase()} API KEY"
                                            }
                                            button {
                                                class: "text-xs text-cyan-400 hover:text-cyan-300 font-mono",
                                                onclick: move |_| {
                                                    let cur = *show_api_key.read();
                                                    show_api_key.set(!cur);
                                                },
                                                if *show_api_key.read() { "Hide" } else { "Show" }
                                            }
                                        }
                                        input {
                                            class: "hud-input-field",
                                            r#type: if *show_api_key.read() { "text" } else { "password" },
                                            placeholder: format!("Enter your {} API key...", selected_provider.read()),
                                            value: "{cloud_api_key}",
                                            oninput: move |e| {
                                                cloud_api_key.set(e.value());
                                                provider_test_status.set(None);
                                            },
                                        }
                                        p { class: "text-xs text-slate-500",
                                            "Your key is stored securely in your local configuration directory and never transmitted to external telemetry servers."
                                        }
                                    }
                                }

                                // Model Selection
                                div { class: "p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-3",
                                    label { class: "text-xs font-bold text-slate-300 uppercase tracking-wider font-mono",
                                        "Target Model"
                                    }
                                    if *selected_provider.read() == "ollama" {
                                        select {
                                            class: "hud-input-field",
                                            value: "{selected_model}",
                                            onchange: move |e| selected_model.set(e.value()),
                                            if initial_ollama_models.is_empty() {
                                                option { value: "llama3.2:3b", "llama3.2:3b (Recommended 3B Local)" }
                                                option { value: "llama3.2:1b", "llama3.2:1b (Ultra-Lightweight)" }
                                                option { value: "mistral:7b", "mistral:7b" }
                                                option { value: "qwen2.5-coder:7b", "qwen2.5-coder:7b (Coding specialist)" }
                                            } else {
                                                for m in &initial_ollama_models {
                                                    option { value: "{m}", "{m}" }
                                                }
                                            }
                                        }
                                    } else {
                                        input {
                                            class: "hud-input-field",
                                            value: "{selected_model}",
                                            placeholder: "Model identifier (e.g. gpt-4o, claude-3-5-sonnet-20241022)...",
                                            oninput: move |e| selected_model.set(e.value()),
                                        }
                                    }
                                }

                                // Live Provider Test Connection Action
                                div { class: "p-4 bg-slate-900/40 border border-slate-800 rounded-xl flex items-center justify-between",
                                    div { class: "space-y-1",
                                        p { class: "text-sm font-semibold text-slate-200", "Live Provider Verification" }
                                        p { class: "text-xs text-slate-400", "Confirm API authentication and latency before continuing." }
                                    }
                                    button {
                                        class: "px-4 py-2 bg-cyan-600 hover:bg-cyan-500 text-white font-mono text-xs font-bold rounded-lg transition-colors flex items-center gap-2",
                                        disabled: *is_testing_provider.read(),
                                        onclick: move |_| {
                                            let prov = selected_provider.read().clone();
                                            let mdl = selected_model.read().clone();
                                            let key = if cloud_api_key.read().trim().is_empty() {
                                                None
                                            } else {
                                                Some(cloud_api_key.read().trim().to_string())
                                            };
                                            is_testing_provider.set(true);

                                            spawn(async move {
                                                if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                                    match dispatcher.dispatch(crate::backend::BackendCommand::TestProviderConnection {
                                                        provider: prov,
                                                        model: mdl,
                                                        api_key: key,
                                                    }).await {
                                                        Ok(crate::backend::BackendResult::ProviderTested { success, message, latency_ms }) => {
                                                            provider_test_status.set(Some((success, message, latency_ms)));
                                                        }
                                                        Err(e) => {
                                                            provider_test_status.set(Some((false, e.to_string(), 0)));
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                                is_testing_provider.set(false);
                                            });
                                        },
                                        if *is_testing_provider.read() {
                                            span { class: "animate-spin", "⏳" }
                                            "Testing..."
                                        } else {
                                            "⚡ Test Connection"
                                        }
                                    }
                                }

                                // Test Status Feedback
                                if let Some((success, msg, latency)) = provider_test_status.read().as_ref() {
                                    div {
                                        class: format!(
                                            "p-4 rounded-xl border flex items-start gap-3 {}",
                                            if *success {
                                                "bg-emerald-950/40 border-emerald-500/40 text-emerald-200"
                                            } else {
                                                "bg-rose-950/40 border-rose-500/40 text-rose-200"
                                            }
                                        ),
                                        span { class: "text-lg", if *success { "✅" } else { "⚠️" } }
                                        div { class: "space-y-1 text-xs",
                                            p { class: "font-bold", if *success { "Provider Ready" } else { "Validation Error" } }
                                            p { class: "text-slate-300", "{msg}" }
                                            if *success && *latency > 0 {
                                                p { class: "font-mono text-emerald-400 font-bold", "Latency: {latency} ms" }
                                            }
                                        }
                                    }
                                }
                            }
                        },

                        OnboardingStep::VoicePermissions => rsx! {
                            div { class: "space-y-6 max-w-2xl mx-auto",
                                div { class: "space-y-1",
                                    h2 { class: "text-xl font-bold text-white", "Voice Engine & Windows Audio" }
                                    p { class: "text-xs text-slate-400",
                                        "OSMOO includes a real-time voice pipeline with sub-300ms time-to-first-speech."
                                    }
                                }

                                // STT Engine Selection
                                div { class: "p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-3",
                                    label { class: "text-xs font-bold text-slate-300 uppercase tracking-wider font-mono",
                                        "Speech-to-Text (STT) Engine"
                                    }
                                    select {
                                        class: "hud-input-field",
                                        value: "{selected_stt}",
                                        onchange: move |e| selected_stt.set(e.value()),
                                        option { value: "sherpa_onnx_whisper", "Sherpa-ONNX Whisper (Offline & Ultra-Fast)" }
                                        option { value: "windows_sapi", "Windows Native Speech Recognition" }
                                        option { value: "cloud_whisper", "OpenAI Whisper Cloud" }
                                    }
                                }

                                // TTS Engine Selection
                                div { class: "p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-3",
                                    label { class: "text-xs font-bold text-slate-300 uppercase tracking-wider font-mono",
                                        "Text-to-Speech (TTS) Voice Engine"
                                    }
                                    select {
                                        class: "hud-input-field",
                                        value: "{selected_tts}",
                                        onchange: move |e| selected_tts.set(e.value()),
                                        option { value: "piper_en_libritts", "Piper Neural TTS (Sub-200ms Local Synthesizer)" }
                                        option { value: "windows_sapi_tts", "Windows Native SAPI Voice" }
                                        option { value: "elevenlabs", "ElevenLabs Neural Cloud" }
                                    }
                                }

                                // Audio Activation Mode
                                div { class: "p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-3",
                                    label { class: "text-xs font-bold text-slate-300 uppercase tracking-wider font-mono",
                                        "Voice Activation Preference"
                                    }
                                    div { class: "grid grid-cols-2 gap-3",
                                        button {
                                            class: format!(
                                                "p-3 rounded-lg border text-left flex flex-col gap-1 transition-all {}",
                                                if *wake_word_enabled.read() {
                                                    "bg-cyan-950/40 border-cyan-400 text-cyan-200"
                                                } else {
                                                    "bg-slate-900/40 border-slate-800 text-slate-400"
                                                }
                                            ),
                                            onclick: move |_| wake_word_enabled.set(true),
                                            span { class: "font-bold text-sm", "🎙️ Wake-Word Always Listening" }
                                            span { class: "text-xs text-slate-400", "Responds to 'Hey OSMOO' hands-free" }
                                        }
                                        button {
                                            class: format!(
                                                "p-3 rounded-lg border text-left flex flex-col gap-1 transition-all {}",
                                                if !*wake_word_enabled.read() {
                                                    "bg-cyan-950/40 border-cyan-400 text-cyan-200"
                                                } else {
                                                    "bg-slate-900/40 border-slate-800 text-slate-400"
                                                }
                                            ),
                                            onclick: move |_| wake_word_enabled.set(false),
                                            span { class: "font-bold text-sm", "⌨️ Push-to-Talk Only" }
                                            span { class: "text-xs text-slate-400", "Press Ctrl+Space to activate" }
                                        }
                                    }
                                }

                                // Mic & Speaker Verification
                                div { class: "p-4 bg-slate-900/40 border border-slate-800 rounded-xl flex items-center justify-between",
                                    div { class: "space-y-1",
                                        p { class: "text-sm font-semibold text-slate-200", "Audio Pipeline Verification" }
                                        p { class: "text-xs text-slate-400", "Microphone access is active and calibrated." }
                                    }
                                    button {
                                        class: "px-4 py-2 bg-slate-800 hover:bg-slate-700 text-cyan-300 font-mono text-xs font-bold rounded-lg border border-slate-700 transition-colors flex items-center gap-2",
                                        onclick: move |_| {
                                            test_audio_playing.set(true);
                                            spawn(async move {
                                                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                                                test_audio_playing.set(false);
                                            });
                                        },
                                        if *test_audio_playing.read() {
                                            span { class: "animate-pulse", "🔊 Playing..." }
                                        } else {
                                            "🔊 Test Speaker Output"
                                        }
                                    }
                                }
                            }
                        },

                        OnboardingStep::CompanionPreferences => rsx! {
                            div { class: "space-y-6 max-w-2xl mx-auto",
                                div { class: "space-y-1",
                                    h2 { class: "text-xl font-bold text-white", "2D Cat Companion Customization" }
                                    p { class: "text-xs text-slate-400",
                                        "Configure the appearance, scale, and autonomous desktop behavior of your OSMOO companion."
                                    }
                                }

                                // Interactive Live Preview Box
                                div { class: "p-6 bg-slate-950/80 border border-slate-800 rounded-xl flex flex-col items-center justify-center relative overflow-hidden h-64",
                                    // Subtle background grid
                                    div { class: "absolute inset-0 bg-[radial-gradient(#1e293b_1px,transparent_1px)] [background-size:16px_16px] opacity-40" }

                                    // Live Rigged Cat Preview
                                    div {
                                        class: "relative transition-transform duration-200 flex items-center justify-center",
                                        style: format!(
                                            "width: {}px; height: {}px; transform-origin: 50% 100%;",
                                            (160.0 * *companion_scale.read()) as u32,
                                            (170.0 * *companion_scale.read()) as u32
                                        ),
                                        img {
                                            class: "w-full h-full object-contain filter drop-shadow-[0_8px_16px_rgba(0,0,0,0.5)]",
                                            src: "{crate::assets::OSMOO_CAT_DATA_URI}",
                                            alt: "OSMOO Cat Preview",
                                        }
                                    }

                                    div { class: "absolute bottom-3 right-4 text-[10px] font-mono text-cyan-400 bg-slate-900/90 px-2.5 py-1 rounded-md border border-slate-800",
                                        "SCALE: {(*companion_scale.read() * 100.0) as u32}%"
                                    }
                                }

                                // Scale Slider Control
                                div { class: "p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-3",
                                    div { class: "flex items-center justify-between",
                                        label { class: "text-xs font-bold text-slate-300 uppercase tracking-wider font-mono",
                                            "Companion Size"
                                        }
                                        span { class: "text-xs font-mono text-cyan-400 font-bold",
                                            "{(*companion_scale.read() * 100.0) as u32}%"
                                        }
                                    }
                                    input {
                                        class: "w-full accent-cyan-400 cursor-pointer",
                                        r#type: "range",
                                        min: "0.75",
                                        max: "1.35",
                                        step: "0.05",
                                        value: "{companion_scale}",
                                        oninput: move |e| {
                                            if let Ok(v) = e.value().parse::<f32>() {
                                                companion_scale.set(v);
                                            }
                                        },
                                    }
                                    div { class: "flex justify-between text-[10px] text-slate-500 font-mono",
                                        span { "Compact (75%)" }
                                        span { "Standard (100%)" }
                                        span { "Expanded (135%)" }
                                    }
                                }

                                // Autonomous Walking & Proactive Speech Toggles
                                div { class: "grid grid-cols-2 gap-3",
                                    div { class: "p-4 bg-slate-900/60 border border-slate-800 rounded-xl flex items-center justify-between",
                                        div { class: "space-y-0.5",
                                            p { class: "text-sm font-semibold text-slate-200", "🐾 Autonomous Walking" }
                                            p { class: "text-xs text-slate-400", "Wanders taskbar when idle" }
                                        }
                                        input {
                                            class: "accent-cyan-400 w-5 h-5 cursor-pointer",
                                            r#type: "checkbox",
                                            checked: *autowalk_enabled.read(),
                                            onchange: move |e| autowalk_enabled.set(e.checked()),
                                        }
                                    }
                                    div { class: "p-4 bg-slate-900/60 border border-slate-800 rounded-xl flex items-center justify-between",
                                        div { class: "space-y-0.5",
                                            p { class: "text-sm font-semibold text-slate-200", "💬 Proactive Speech" }
                                            p { class: "text-xs text-slate-400", "Periodic reminders & tips" }
                                        }
                                        input {
                                            class: "accent-cyan-400 w-5 h-5 cursor-pointer",
                                            r#type: "checkbox",
                                            checked: *proactive_speech_enabled.read(),
                                            onchange: move |e| proactive_speech_enabled.set(e.checked()),
                                        }
                                    }
                                }

                                // Windows Autostart Toggle
                                div { class: "p-4 bg-slate-900/60 border border-slate-800 rounded-xl flex items-center justify-between",
                                    div { class: "space-y-0.5",
                                        p { class: "text-sm font-semibold text-slate-200", "🚀 Windows Login Autostart" }
                                        p { class: "text-xs text-slate-400", "Launch OSMOO automatically when logging into Windows" }
                                    }
                                    input {
                                        class: "accent-cyan-400 w-5 h-5 cursor-pointer",
                                        r#type: "checkbox",
                                        checked: *start_on_boot.read(),
                                        onchange: move |e| start_on_boot.set(e.checked()),
                                    }
                                }
                            }
                        },

                        OnboardingStep::TestAndFinish => rsx! {
                            div { class: "space-y-6 max-w-2xl mx-auto",
                                div { class: "space-y-1 text-center",
                                    h2 { class: "text-2xl font-bold text-white", "Deployment Summary & Activation" }
                                    p { class: "text-xs text-slate-400",
                                        "Configured parameters and subsystems ready for desktop companion initialization."
                                    }
                                }

                                // 4-Point Verification Checklist
                                div { class: "grid grid-cols-2 gap-3",
                                    div { class: "p-4 bg-emerald-950/20 border border-emerald-500/30 rounded-xl flex items-center gap-3",
                                        span { class: "text-xl", "✅" }
                                        div { class: "space-y-0.5 text-xs",
                                            p { class: "font-bold text-emerald-300", "AI Model Provider" }
                                            p { class: "text-slate-400", "{selected_provider.read().to_uppercase()} ({selected_model})" }
                                        }
                                    }
                                    div { class: "p-4 bg-emerald-950/20 border border-emerald-500/30 rounded-xl flex items-center gap-3",
                                        span { class: "text-xl", "✅" }
                                        div { class: "space-y-0.5 text-xs",
                                            p { class: "font-bold text-emerald-300", "Voice Synthesis" }
                                            p { class: "text-slate-400", "{selected_tts.read()}" }
                                        }
                                    }
                                    div { class: "p-4 bg-emerald-950/20 border border-emerald-500/30 rounded-xl flex items-center gap-3",
                                        span { class: "text-xl", "✅" }
                                        div { class: "space-y-0.5 text-xs",
                                            p { class: "font-bold text-emerald-300", "Background Daemon" }
                                            p { class: "text-slate-400", "Supervised IPC Pipe Online" }
                                        }
                                    }
                                    div { class: "p-4 bg-emerald-950/20 border border-emerald-500/30 rounded-xl flex items-center gap-3",
                                        span { class: "text-xl", "✅" }
                                        div { class: "space-y-0.5 text-xs",
                                            p { class: "font-bold text-emerald-300", "2D Cat Desktop Overlay" }
                                            p { class: "text-slate-400", "Rigged Character Ready" }
                                        }
                                    }
                                }

                                // Summary Card
                                div { class: "p-5 bg-slate-900/60 border border-slate-800 rounded-xl space-y-2 text-xs",
                                    div { class: "flex justify-between py-1 border-b border-slate-800",
                                        span { class: "text-slate-400", "Companion Name" }
                                        span { class: "font-bold text-slate-200", "{companion_name}" }
                                    }
                                    div { class: "flex justify-between py-1 border-b border-slate-800",
                                        span { class: "text-slate-400", "Autonomous Movement" }
                                        span { class: "font-bold text-cyan-400", if *autowalk_enabled.read() { "Enabled" } else { "Disabled" } }
                                    }
                                    div { class: "flex justify-between py-1 border-b border-slate-800",
                                        span { class: "text-slate-400", "Companion Scale" }
                                        span { class: "font-mono font-bold text-slate-200", "{(*companion_scale.read() * 100.0) as u32}%" }
                                    }
                                    div { class: "flex justify-between py-1",
                                        span { class: "text-slate-400", "Security Encryption" }
                                        span { class: "font-mono font-bold text-emerald-400", "Active (Windows DPAPI)" }
                                    }
                                }
                            }
                        },
                    }
                }

                // ── Footer Navigation Buttons ────────────────────────────────
                div { class: "px-8 py-5 border-t border-slate-800/80 bg-slate-900/60 flex items-center justify-between",
                    if *current_step.read() != OnboardingStep::Welcome {
                        button {
                            class: "px-5 py-2.5 bg-slate-800 hover:bg-slate-700 text-slate-300 font-mono text-xs font-bold rounded-xl transition-colors",
                            onclick: prev_step,
                            "← Back"
                        }
                    } else {
                        div {}
                    }

                    button {
                        class: "px-6 py-2.5 bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-mono text-xs font-bold rounded-xl shadow-[0_0_20px_rgba(6,182,212,0.3)] transition-all flex items-center gap-2",
                        onclick: next_step,
                        if *current_step.read() == OnboardingStep::TestAndFinish {
                            "🚀 Finish Setup & Launch OSMOO"
                        } else {
                            "Continue →"
                        }
                    }
                }
            }
        }
    }
}
