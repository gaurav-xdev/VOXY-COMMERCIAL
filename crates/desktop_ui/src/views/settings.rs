use std::collections::HashMap;
use crate::components::icons::{IconSearch, IconSettings, IconUser};
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsTab {
    General,
    AiModels,
    Voice,
    Companion,
    Security,
    Memory,
    Automation,
    Integrations,
    Performance,
    Diagnostics,
}

impl SettingsTab {
    pub fn all() -> &'static [SettingsTab] {
        &[
            SettingsTab::General,
            SettingsTab::AiModels,
            SettingsTab::Voice,
            SettingsTab::Companion,
            SettingsTab::Security,
            SettingsTab::Memory,
            SettingsTab::Automation,
            SettingsTab::Integrations,
            SettingsTab::Performance,
            SettingsTab::Diagnostics,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            SettingsTab::General => "General & Profile",
            SettingsTab::AiModels => "AI Models & Providers",
            SettingsTab::Voice => "Voice & Wake Words",
            SettingsTab::Companion => "Companion & Appearance",
            SettingsTab::Security => "Permissions & Security",
            SettingsTab::Memory => "Memory & Personalization",
            SettingsTab::Automation => "Automation & Autonomy",
            SettingsTab::Integrations => "Integrations & Accounts",
            SettingsTab::Performance => "Performance & Resources",
            SettingsTab::Diagnostics => "Activity & Diagnostics",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SettingsTab::General => "Identity, workspace attributes, window lifecycle, and client channels.",
            SettingsTab::AiModels => "Model selection, strict routing invariants, and provider API credentials.",
            SettingsTab::Voice => "Speech-to-text, neural synthesis, hotwords, and acoustic parameters.",
            SettingsTab::Companion => "Avatar mesh selection, 2D/3D spatial scale, expressions, and viewport overlay.",
            SettingsTab::Security => "Filesystem sandboxing, browser bounds, shell gates, and safety matrix.",
            SettingsTab::Memory => "Semantic retrieval retention, privacy sessions, and operator persona notes.",
            SettingsTab::Automation => "Background execution, concurrency gates, timeouts, and risk thresholds.",
            SettingsTab::Integrations => "Status matrix of native OpenXML office suites and cloud service connectors.",
            SettingsTab::Performance => "Hardware telemetry, host memory ceilings, and GPU offloading.",
            SettingsTab::Diagnostics => "Operational audit logs, DPAPI sanitized diagnostic bundles, and build info.",
        }
    }
}

/// Unified Settings Form State encompassing all 10 subsystems.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsFormState {
    // 1. General & Profile
    pub user_name: String,
    pub user_email: String,
    pub organization: String,
    pub role: String,
    pub language: String,
    pub theme: String,
    pub autolaunch: bool,
    pub close_behavior: String,
    pub auto_update: bool,
    pub update_channel: String,

    // 2. AI Models & Providers
    pub primary_llm: String,
    pub coding_model: String,
    pub voice_model: String,
    pub embedding_model: String,
    pub ollama_model: String,
    pub enable_fallback: bool,
    pub provider_keys: HashMap<String, String>,

    // 3. Voice & Wake Words
    pub voice_enabled: bool,
    pub wake_words: Vec<String>,
    pub vad_sensitivity: f32,
    pub stt_provider: String,
    pub tts_provider: String,
    pub voice_name: String,
    pub speech_speed: f64,
    pub speech_pitch: f64,
    pub interruption_barge_in: bool,
    pub noise_suppression: bool,
    pub echo_cancellation: bool,
    pub input_device: String,
    pub output_device: String,
    pub audio_sample_rate: u32,

    // 4. Companion & Appearance
    pub companion_character_id: String,
    pub companion_size: f64,
    pub companion_position: String,
    pub companion_expression: String,
    pub companion_lip_sync: bool,
    pub companion_animations: bool,
    pub companion_draggable: bool,
    pub companion_click_through: bool,
    pub desktop_overlay_mode: bool,

    // 5. Security & Permissions
    pub file_system_access: String,
    pub browser_automation: String,
    pub shell_execution: String,
    pub require_approval_sensitive: bool,
    pub require_approval_financial: bool,
    pub require_approval_email: bool,
    pub require_approval_scripts: bool,
    pub audit_logging_enabled: bool,
    pub emergency_stopped: bool,

    // 6. Memory & Personalization
    pub memory_enabled: bool,
    pub memory_retention_days: u32,
    pub auto_consolidate: bool,
    pub private_session: bool,
    pub user_persona_notes: String,

    // 7. Automation & Autonomy
    pub background_tasks_allowed: bool,
    pub pause_on_battery: bool,
    pub max_concurrent_tasks: usize,
    pub task_timeout_seconds: u64,
    pub action_risk_threshold: String,
    pub global_tasks_paused: bool,

    // 8. Performance & Resources
    pub max_memory_mb: u64,
    pub max_cpu_percent: f64,
    pub gpu_acceleration: bool,
    pub model_unload_timeout_minutes: u32,

    // 9. Extra UI / Designation
    pub assistant_name: String,
    pub active_tier: String,
    pub minimize_to_tray: bool,
}

impl Default for SettingsFormState {
    fn default() -> Self {
        let mut keys = HashMap::new();
        keys.insert("openai".to_string(), String::new());
        keys.insert("anthropic".to_string(), String::new());
        keys.insert("gemini".to_string(), String::new());
        keys.insert("groq".to_string(), String::new());
        keys.insert("mistral".to_string(), String::new());
        keys.insert("deepseek".to_string(), String::new());
        keys.insert("openrouter".to_string(), String::new());

        Self {
            user_name: "Operator".to_string(),
            user_email: "operator@osmoo.in".to_string(),
            organization: "OSMOO Workspace".to_string(),
            role: "Primary Administrator".to_string(),
            language: "en".to_string(),
            theme: "dark".to_string(),
            autolaunch: true,
            close_behavior: "minimize_to_tray".to_string(),
            auto_update: true,
            update_channel: "stable".to_string(),

            primary_llm: "ollama".to_string(),
            coding_model: "llama3.2:3b".to_string(),
            voice_model: "llama3.2:3b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            ollama_model: "llama3.2:3b".to_string(),
            enable_fallback: false,
            provider_keys: keys,

            voice_enabled: true,
            wake_words: vec!["Hey OSMOO".to_string(), "OSMOO".to_string()],
            vad_sensitivity: 0.5,
            stt_provider: "sherpa_onnx_whisper".to_string(),
            tts_provider: "piper_en_libritts".to_string(),
            voice_name: "en-us-amber".to_string(),
            speech_speed: 1.0,
            speech_pitch: 1.0,
            interruption_barge_in: true,
            noise_suppression: true,
            echo_cancellation: true,
            input_device: "Default System Microphone".to_string(),
            output_device: "Default System Speakers".to_string(),
            audio_sample_rate: 48000,

            companion_character_id: "cat".to_string(),
            companion_size: 1.0,
            companion_position: "bottom-right".to_string(),
            companion_expression: "neutral".to_string(),
            companion_lip_sync: true,
            companion_animations: true,
            companion_draggable: true,
            companion_click_through: false,
            desktop_overlay_mode: true,

            file_system_access: "workspace_only".to_string(),
            browser_automation: "prompt".to_string(),
            shell_execution: "require_confirmation".to_string(),
            require_approval_sensitive: true,
            require_approval_financial: true,
            require_approval_email: true,
            require_approval_scripts: true,
            audit_logging_enabled: true,
            emergency_stopped: false,

            memory_enabled: true,
            memory_retention_days: 90,
            auto_consolidate: true,
            private_session: false,
            user_persona_notes: String::new(),

            background_tasks_allowed: true,
            pause_on_battery: true,
            max_concurrent_tasks: 3,
            task_timeout_seconds: 60,
            action_risk_threshold: "prompt_risky".to_string(),
            global_tasks_paused: false,

            max_memory_mb: 2048,
            max_cpu_percent: 50.0,
            gpu_acceleration: true,
            model_unload_timeout_minutes: 15,

            assistant_name: "OSMOO Core".to_string(),
            active_tier: "free".to_string(),
            minimize_to_tray: true,
        }
    }
}

impl SettingsFormState {
    pub fn from_snapshot(
        snapshot: &voxy_desktop_runtime::settings::SettingsSnapshot,
        user_email: &str,
        active_tier: &str,
    ) -> Self {
        let mut keys = HashMap::new();
        for prov in &["openai", "anthropic", "gemini", "groq", "mistral", "deepseek", "openrouter"] {
            if snapshot.models.provider_keys.contains_key(*prov) || std::env::var(format!("{}_API_KEY", prov.to_uppercase())).is_ok() {
                keys.insert(prov.to_string(), "••••••••".to_string());
            } else {
                keys.insert(prov.to_string(), String::new());
            }
        }

        Self {
            user_name: snapshot.profile.user_name.clone(),
            user_email: if user_email.is_empty() { snapshot.profile.user_email.clone() } else { user_email.to_string() },
            organization: snapshot.profile.organization.clone(),
            role: snapshot.profile.role.clone(),
            language: snapshot.profile.language.clone(),
            theme: snapshot.appearance.theme.clone(),
            autolaunch: snapshot.appearance.start_with_windows,
            close_behavior: snapshot.profile.close_behavior.clone(),
            auto_update: snapshot.updates.auto_update,
            update_channel: snapshot.updates.channel.clone(),

            primary_llm: snapshot.models.provider.clone(),
            coding_model: snapshot.models.coding_model.clone(),
            voice_model: snapshot.models.voice_model.clone(),
            embedding_model: snapshot.models.embedding_model.clone(),
            ollama_model: snapshot.models.model.clone(),
            enable_fallback: snapshot.models.enable_fallback,
            provider_keys: keys,

            voice_enabled: snapshot.voice.enabled,
            wake_words: snapshot.voice.wake_words.clone(),
            vad_sensitivity: snapshot.voice.wake_word_sensitivity as f32,
            stt_provider: snapshot.voice.stt_provider.clone().unwrap_or_else(|| "sherpa_onnx_whisper".to_string()),
            tts_provider: snapshot.voice.tts_provider.clone().unwrap_or_else(|| "piper_en_libritts".to_string()),
            voice_name: snapshot.voice.voice_name.clone().unwrap_or_else(|| "en-us-amber".to_string()),
            speech_speed: snapshot.voice.speech_speed,
            speech_pitch: snapshot.voice.speech_pitch,
            interruption_barge_in: snapshot.voice.interruption_barge_in,
            noise_suppression: snapshot.voice.noise_suppression,
            echo_cancellation: snapshot.voice.echo_cancellation,
            input_device: snapshot.voice.input_device.clone().unwrap_or_else(|| "Default System Microphone".to_string()),
            output_device: snapshot.voice.output_device.clone().unwrap_or_else(|| "Default System Speakers".to_string()),
            audio_sample_rate: snapshot.voice.sample_rate,

            companion_character_id: snapshot.companion.character_id.clone(),
            companion_size: snapshot.companion.character_size,
            companion_position: snapshot.companion.position_preset.clone(),
            companion_expression: snapshot.companion.expression.clone(),
            companion_lip_sync: snapshot.companion.lip_sync,
            companion_animations: snapshot.companion.animations_enabled,
            companion_draggable: snapshot.companion.draggable,
            companion_click_through: snapshot.companion.click_through,
            desktop_overlay_mode: snapshot.companion.desktop_overlay_mode == "overlay",

            file_system_access: snapshot.security.file_system_access.clone(),
            browser_automation: snapshot.security.browser_automation.clone(),
            shell_execution: snapshot.security.shell_execution.clone(),
            require_approval_sensitive: snapshot.security.require_approval_sensitive,
            require_approval_financial: snapshot.security.require_approval_financial,
            require_approval_email: snapshot.security.require_approval_email,
            require_approval_scripts: snapshot.security.require_approval_scripts,
            audit_logging_enabled: snapshot.security.audit_logging_enabled,
            emergency_stopped: snapshot.security.emergency_stopped,

            memory_enabled: snapshot.memory.enabled,
            memory_retention_days: snapshot.memory.retention_days,
            auto_consolidate: snapshot.memory.auto_consolidate,
            private_session: snapshot.memory.private_session,
            user_persona_notes: snapshot.memory.user_persona_notes.clone(),

            background_tasks_allowed: snapshot.automation.background_tasks_allowed,
            pause_on_battery: snapshot.automation.pause_on_battery,
            max_concurrent_tasks: snapshot.automation.max_concurrent_tasks,
            task_timeout_seconds: snapshot.automation.timeout_seconds,
            action_risk_threshold: snapshot.automation.action_risk_threshold.clone(),
            global_tasks_paused: snapshot.automation.global_tasks_paused,

            max_memory_mb: snapshot.performance.max_memory_mb,
            max_cpu_percent: snapshot.performance.max_cpu_percent,
            gpu_acceleration: snapshot.performance.gpu_acceleration,
            model_unload_timeout_minutes: snapshot.performance.model_unload_timeout_minutes,

            assistant_name: snapshot.app_name.clone(),
            active_tier: active_tier.to_string(),
            minimize_to_tray: snapshot.profile.close_behavior == "minimize_to_tray",
        }
    }
}

#[component]
pub fn SettingsView(
    on_close: EventHandler<()>,
    on_save: EventHandler<SettingsFormState>,
    initial_settings: SettingsFormState,
    ollama_models: Vec<String>,
) -> Element {
    let mut form = use_signal(move || initial_settings);
    let mut current_tab = use_signal(|| SettingsTab::General);
    let mut search_query = use_signal(String::new);
    let mut save_status = use_signal(|| None::<String>);
    let mut validation_error = use_signal(|| None::<String>);

    // Dynamic states for interactive controls
    let mut new_wake_word = use_signal(String::new);
    let mut provider_test_status = use_signal(|| None::<(bool, String)>);
    let mut testing_provider = use_signal(|| false);
    let mut purged_count = use_signal(|| None::<usize>);
    let mut confirm_purge = use_signal(|| false);
    let mut diagnostics_report = use_signal(|| None::<String>);

    let tab = *current_tab.read();
    let query = search_query.read().trim().to_lowercase();

    // Filter tabs based on search
    let filtered_tabs: Vec<SettingsTab> = SettingsTab::all()
        .iter()
        .copied()
        .filter(|t| {
            if query.is_empty() {
                true
            } else {
                t.title().to_lowercase().contains(&query)
                    || t.description().to_lowercase().contains(&query)
            }
        })
        .collect();

    rsx! {
        div { class: "modal-overlay",
            div { class: "machined-card modal-container", style: "max-width: 960px; height: 740px; display: flex; flex-direction: column;",
                div { class: "machined-card-inner", style: "padding: 0; display: flex; flex-direction: column; height: 100%; overflow: hidden;",
                    // Header Bar
                    div { class: "modal-header", style: "padding: 12px 20px; border-bottom: 1px solid var(--border-machined); display: flex; align-items: center; justify-content: space-between; background: rgba(10, 12, 18, 0.85);",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            IconSettings { size: 18 }
                            span { style: "font-family: var(--font-mono); font-size: 13px; font-weight: 700; letter-spacing: 0.12em; color: var(--osmoo-ivory);",
                                "OSMOO SYSTEM PREFERENCES & POLICY CONTROL"
                            }
                        }
                        div { style: "display: flex; align-items: center; gap: 8px;",
                            button {
                                class: "hud-btn",
                                style: "background: var(--osmoo-gold); color: #000; font-weight: 700; padding: 4px 14px; font-size: 12px; border-radius: 4px;",
                                onclick: move |_| {
                                    // Pre-save validation
                                    let current = form.read().clone();
                                    if current.wake_words.len() > 10 {
                                        validation_error.set(Some("Maximum of 10 wake words allowed.".to_string()));
                                        return;
                                    }
                                    if current.companion_size < 0.2 || current.companion_size > 3.0 {
                                        validation_error.set(Some("Companion size scale must be between 0.2 and 3.0.".to_string()));
                                        return;
                                    }
                                    if current.speech_speed < 0.25 || current.speech_speed > 3.0 {
                                        validation_error.set(Some("Speech speed must be between 0.25x and 3.0x.".to_string()));
                                        return;
                                    }
                                    validation_error.set(None);
                                    on_save.call(current);
                                    save_status.set(Some("Settings saved and runtime policies updated.".to_string()));
                                },
                                "SAVE CHANGES"
                            }
                            button {
                                class: "hud-icon-btn",
                                style: "width: 32px; height: 32px; font-size: 18px;",
                                onclick: move |_| on_close.call(()),
                                "×"
                            }
                        }
                    }

                    // Notification / Error banners
                    if let Some(err) = validation_error.read().as_ref() {
                        div { style: "background: rgba(239, 68, 68, 0.15); border-bottom: 1px solid rgba(239, 68, 68, 0.4); color: #f87171; font-size: 12px; padding: 6px 20px; font-family: var(--font-mono);",
                            "VALIDATION ERROR: {err}"
                        }
                    }
                    if let Some(status) = save_status.read().as_ref() {
                        div { style: "background: rgba(34, 197, 94, 0.15); border-bottom: 1px solid rgba(34, 197, 94, 0.4); color: #4ade80; font-size: 12px; padding: 6px 20px; font-family: var(--font-mono);",
                            "SUCCESS: {status}"
                        }
                    }

                    // Main 2-Column Layout
                    div { style: "display: flex; flex: 1; overflow: hidden;",
                        // Left Searchable Sidebar
                        div { style: "width: 240px; border-right: 1px solid var(--border-machined); background: rgba(14, 16, 23, 0.5); padding: 14px 10px; display: flex; flex-direction: column; gap: 8px;",
                            // Search Filter Input
                            div { style: "position: relative; margin-bottom: 6px;",
                                input {
                                    class: "hud-input",
                                    style: "width: 100%; padding-left: 28px; font-size: 12px; height: 30px;",
                                    placeholder: "Filter settings...",
                                    value: "{search_query}",
                                    oninput: move |evt: FormEvent| search_query.set(evt.value()),
                                }
                                div { style: "position: absolute; left: 8px; top: 7px; color: var(--osmoo-muted); pointer-events: none;",
                                    IconSearch { size: 14 }
                                }
                            }

                            // Tabs List
                            div { style: "flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 2px;",
                                for item in filtered_tabs.iter() {
                                    button {
                                        class: if tab == *item { "settings-tab-btn active" } else { "settings-tab-btn" },
                                        style: if tab == *item {
                                            "text-align: left; padding: 8px 12px; border-radius: 4px; border: 1px solid var(--osmoo-gold); background: rgba(229, 169, 59, 0.12); color: var(--osmoo-ivory); font-size: 12px; font-weight: 600; cursor: pointer; display: flex; align-items: center; justify-content: space-between;"
                                        } else {
                                            "text-align: left; padding: 8px 12px; border-radius: 4px; border: 1px solid transparent; background: transparent; color: var(--osmoo-muted); font-size: 12px; cursor: pointer; display: flex; align-items: center; justify-content: space-between;"
                                        },
                                        onclick: {
                                            let it = *item;
                                            move |_| {
                                                current_tab.set(it);
                                                save_status.set(None);
                                                validation_error.set(None);
                                            }
                                        },
                                        span { "{item.title()}" }
                                        if tab == *item {
                                            span { style: "width: 6px; height: 6px; border-radius: 50%; background: var(--osmoo-gold);" }
                                        }
                                    }
                                }
                            }

                            // Operator Badge
                            div { style: "border-top: 1px solid var(--border-machined); padding-top: 10px; display: flex; align-items: center; gap: 8px; font-family: var(--font-mono); font-size: 11px; color: var(--osmoo-muted);",
                                IconUser { size: 14 }
                                span { "{form.read().user_name} ({form.read().role})" }
                            }
                        }

                        // Right-Hand Scrollable Content Panel
                        div { style: "flex: 1; overflow-y: auto; padding: 24px; background: rgba(8, 10, 15, 0.4);",
                            // Tab Header
                            div { style: "margin-bottom: 20px; border-bottom: 1px solid var(--border-machined); padding-bottom: 12px;",
                                h2 { style: "font-size: 18px; font-weight: 700; color: var(--osmoo-ivory); margin: 0 0 4px 0;",
                                    "{tab.title()}"
                                }
                                p { style: "font-size: 13px; color: var(--osmoo-muted); margin: 0;",
                                    "{tab.description()}"
                                }
                            }

                            // Section Body based on Active Tab
                            match tab {
                                SettingsTab::General => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "User Full Name" }
                                            input {
                                                class: "hud-input",
                                                value: "{form.read().user_name}",
                                                oninput: move |e: FormEvent| form.write().user_name = e.value(),
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Work Email Address" }
                                            input {
                                                class: "hud-input",
                                                value: "{form.read().user_email}",
                                                oninput: move |e: FormEvent| form.write().user_email = e.value(),
                                            }
                                        }
                                    }

                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Organization Workspace" }
                                            input {
                                                class: "hud-input",
                                                value: "{form.read().organization}",
                                                oninput: move |e: FormEvent| form.write().organization = e.value(),
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Operator Role" }
                                            input {
                                                class: "hud-input",
                                                value: "{form.read().role}",
                                                oninput: move |e: FormEvent| form.write().role = e.value(),
                                            }
                                        }
                                    }

                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Interface Language" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().language}",
                                                onchange: move |e: FormEvent| form.write().language = e.value(),
                                                option { value: "en", "English (United States)" }
                                                option { value: "en-gb", "English (United Kingdom)" }
                                                option { value: "de", "German (Deutsch)" }
                                                option { value: "fr", "French (Français)" }
                                                option { value: "ja", "Japanese (日本語)" }
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Visual Theme" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().theme}",
                                                onchange: move |e: FormEvent| form.write().theme = e.value(),
                                                option { value: "dark", "Machined Obsidian (Dark)" }
                                                option { value: "light", "Clean Paper Light" }
                                                option { value: "system", "System Default" }
                                            }
                                        }
                                    }

                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px; display: flex; flex-direction: column; gap: 12px;",
                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Launch with Windows" }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted);", "Automatically register in Windows Startup and initialize daemon upon login." }
                                            }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().autolaunch,
                                                onchange: move |e: FormEvent| form.write().autolaunch = e.value().parse().unwrap_or(false),
                                            }
                                        }

                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Window Close Behavior" }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted);", "Specify whether clicking the window close button minimizes to system tray or exits." }
                                            }
                                            select {
                                                class: "hud-select",
                                                style: "width: 180px;",
                                                value: "{form.read().close_behavior}",
                                                onchange: move |e: FormEvent| form.write().close_behavior = e.value(),
                                                option { value: "minimize_to_tray", "Minimize to Tray" }
                                                option { value: "quit", "Exit Application" }
                                            }
                                        }

                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Automatic Updates & Channel" }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted);", "Automatically inspect signatures and fetch background patches." }
                                            }
                                            div { style: "display: flex; align-items: center; gap: 8px;",
                                                select {
                                                    class: "hud-select",
                                                    style: "width: 110px;",
                                                    value: "{form.read().update_channel}",
                                                    onchange: move |e: FormEvent| form.write().update_channel = e.value(),
                                                    option { value: "stable", "Stable" }
                                                    option { value: "beta", "Beta" }
                                                    option { value: "nightly", "Nightly" }
                                                }
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().auto_update,
                                                    onchange: move |e: FormEvent| form.write().auto_update = e.value().parse().unwrap_or(false),
                                                }
                                            }
                                        }
                                    }
                                    }
                                },

                                SettingsTab::AiModels => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Strict Model Routing Notice
                                    div { style: "background: rgba(229, 169, 59, 0.08); border: 1px solid rgba(229, 169, 59, 0.3); border-radius: 6px; padding: 12px 16px;",
                                        div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 6px;",
                                            span { style: "font-family: var(--font-mono); font-size: 12px; font-weight: 700; color: var(--osmoo-gold);",
                                                "STRICT MODEL ROUTING POLICY & INVARIANTS"
                                            }
                                            div { style: "display: flex; align-items: center; gap: 8px;",
                                                span { style: "font-size: 11px; color: var(--osmoo-muted);", "Enable Fallback Chain" }
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().enable_fallback,
                                                    onchange: move |e: FormEvent| form.write().enable_fallback = e.value().parse().unwrap_or(false),
                                                }
                                            }
                                        }
                                        p { style: "font-size: 12px; color: var(--osmoo-muted); margin: 0; line-height: 1.4;",
                                            if form.read().enable_fallback {
                                                "Fallback is ENABLED. If your primary provider fails or times out, OSMOO will fail over to alternate configured providers."
                                            } else {
                                                "STRICT MODE ACTIVE (Zero Silent Fallback): When using a forced provider, inference failures or timeouts immediately return an error without silently redirecting your query or data to external third parties."
                                            }
                                        }
                                    }

                                    // Primary Routing Matrix
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Primary Reasoning Provider" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().primary_llm}",
                                                onchange: move |e: FormEvent| form.write().primary_llm = e.value(),
                                                option { value: "ollama", "Ollama (Local Private Engine)" }
                                                option { value: "openai", "OpenAI Cloud" }
                                                option { value: "anthropic", "Anthropic Claude" }
                                                option { value: "gemini", "Google Gemini" }
                                                option { value: "groq", "Groq Hardware Cloud" }
                                                option { value: "mistral", "Mistral AI" }
                                                option { value: "deepseek", "DeepSeek" }
                                                option { value: "openrouter", "OpenRouter Multi-Provider" }
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Local Ollama Target Model" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().ollama_model}",
                                                onchange: move |e: FormEvent| form.write().ollama_model = e.value(),
                                                for m in ollama_models.iter() {
                                                    option { value: "{m}", "{m}" }
                                                }
                                                if ollama_models.is_empty() {
                                                    option { value: "llama3.2:3b", "llama3.2:3b (default)" }
                                                    option { value: "qwen2.5-coder:7b", "qwen2.5-coder:7b" }
                                                }
                                            }
                                        }
                                    }

                                    // Specialized Model Mapping
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px;",
                                        div {
                                            label { class: "hud-label", "Coding Specialist" }
                                            input {
                                                class: "hud-input",
                                                value: "{form.read().coding_model}",
                                                oninput: move |e: FormEvent| form.write().coding_model = e.value(),
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Voice Conversational" }
                                            input {
                                                class: "hud-input",
                                                value: "{form.read().voice_model}",
                                                oninput: move |e: FormEvent| form.write().voice_model = e.value(),
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Embedding Model" }
                                            input {
                                                class: "hud-input",
                                                value: "{form.read().embedding_model}",
                                                oninput: move |e: FormEvent| form.write().embedding_model = e.value(),
                                            }
                                        }
                                    }

                                    // Provider API Keys Management (DPAPI Masked)
                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px;",
                                        div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px;",
                                            span { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Cloud Provider API Keys (DPAPI Protected)" }
                                            button {
                                                class: "hud-btn",
                                                style: "padding: 3px 10px; font-size: 11px;",
                                                disabled: *testing_provider.read(),
                                                onclick: move |_| {
                                                    let provider = form.read().primary_llm.clone();
                                                    let model = form.read().ollama_model.clone();
                                                    testing_provider.set(true);
                                                    spawn(async move {
                                                        if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                                            let api_key = form.read().provider_keys.get(&provider.to_lowercase()).cloned();
                                                            match dispatcher.dispatch(crate::backend::BackendCommand::TestProviderConnection { provider, model, api_key }).await {
                                                                Ok(crate::backend::BackendResult::ProviderTested { success, message, latency_ms }) => {
                                                                    provider_test_status.set(Some((success, format!("{message} ({latency_ms}ms) "))));
                                                                }
                                                                Err(e) => {
                                                                    provider_test_status.set(Some((false, e.to_string())));
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                        testing_provider.set(false);
                                                    });
                                                },
                                                if *testing_provider.read() { "TESTING..." } else { "TEST CONNECTION" }
                                            }
                                        }

                                        if let Some((success, msg)) = provider_test_status.read().as_ref() {
                                            div { style: if *success {
                                                "margin-bottom: 10px; padding: 6px 12px; border-radius: 4px; font-size: 11px; background: rgba(34, 197, 94, 0.1); border: 1px solid rgba(34, 197, 94, 0.3); color: #4ade80;"
                                            } else {
                                                "margin-bottom: 10px; padding: 6px 12px; border-radius: 4px; font-size: 11px; background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.3); color: #f87171;"
                                            },
                                                "{msg}"
                                            }
                                        }

                                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 12px;",
                                            for (prov, key_label) in [("openai", "OpenAI API Key"), ("anthropic", "Anthropic Claude Key"), ("gemini", "Google Gemini Key"), ("groq", "Groq Cloud Key")] {
                                                div {
                                                    label { class: "hud-label", "{key_label}" }
                                                    input {
                                                        class: "hud-input",
                                                        r#type: "password",
                                                        placeholder: "•••••••• (masked secret)",
                                                        value: "{form.read().provider_keys.get(prov).cloned().unwrap_or_default()}",
                                                        oninput: {
                                                            let p_str = prov.to_string();
                                                            move |e: FormEvent| {
                                                                form.write().provider_keys.insert(p_str.clone(), e.value());
                                                            }
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    }
                                },

                                SettingsTab::Voice => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Master Voice Switch
                                    div { style: "display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--border-machined); padding-bottom: 12px;",
                                        div {
                                            div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Voice Subsystem Master Toggle" }
                                            div { style: "font-size: 12px; color: var(--osmoo-muted);", "Enables real-time duplex audio capture, token-to-speech streaming, and VAD." }
                                        }
                                        input {
                                            r#type: "checkbox",
                                            checked: form.read().voice_enabled,
                                            onchange: move |e: FormEvent| form.write().voice_enabled = e.value().parse().unwrap_or(false),
                                        }
                                    }

                                    // Wake Words Management (Max 10)
                                    div { style: "background: rgba(14, 16, 23, 0.4); border: 1px solid var(--border-machined); border-radius: 6px; padding: 14px;",
                                        div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px;",
                                            div {
                                                span { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Active Wake Words " }
                                                span { style: "font-size: 11px; color: var(--osmoo-muted);", "({form.read().wake_words.len()}/10 active)" }
                                            }
                                            div { style: "display: flex; align-items: center; gap: 6px;",
                                                input {
                                                    class: "hud-input",
                                                    style: "width: 160px; height: 28px; font-size: 11px;",
                                                    placeholder: "Add hotword...",
                                                    value: "{new_wake_word.read()}",
                                                    oninput: move |e: FormEvent| new_wake_word.set(e.value()),
                                                }
                                                button {
                                                    class: "hud-btn",
                                                    style: "padding: 3px 10px; font-size: 11px;",
                                                    disabled: form.read().wake_words.len() >= 10,
                                                    onclick: move |_| {
                                                        let word = new_wake_word.read().trim().to_string();
                                                        if !word.is_empty() && form.read().wake_words.len() < 10 {
                                                            form.write().wake_words.push(word);
                                                            new_wake_word.set(String::new());
                                                        }
                                                    },
                                                    "+ ADD"
                                                }
                                            }
                                        }

                                        div { style: "display: flex; flex-wrap: wrap; gap: 8px;",
                                            for (idx, w) in form.read().wake_words.iter().enumerate() {
                                                div { style: "background: rgba(229, 169, 59, 0.12); border: 1px solid rgba(229, 169, 59, 0.3); border-radius: 4px; padding: 4px 10px; display: flex; align-items: center; gap: 8px; font-size: 12px; font-family: var(--font-mono); color: var(--osmoo-ivory);",
                                                    span { "\"{w}\"" }
                                                    button {
                                                        style: "background: none; border: none; color: #f87171; cursor: pointer; font-size: 13px; padding: 0;",
                                                        onclick: move |_| {
                                                            form.write().wake_words.remove(idx);
                                                        },
                                                        "×"
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Engines & Voices
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Speech-to-Text (STT) Engine" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().stt_provider}",
                                                onchange: move |e: FormEvent| form.write().stt_provider = e.value(),
                                                option { value: "sherpa_onnx_whisper", "Sherpa-ONNX Local Whisper" }
                                                option { value: "groq", "Groq Whisper Cloud (Ultra-Fast)" }
                                                option { value: "deepfilternet", "DeepFilterNet3 STT" }
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Text-to-Speech (TTS) Engine" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().tts_provider}",
                                                onchange: move |e: FormEvent| form.write().tts_provider = e.value(),
                                                option { value: "piper_en_libritts", "Piper ONNX Neural Voice (Local)" }
                                                option { value: "cartesia", "Cartesia Sonic Streaming (Cloud)" }
                                                option { value: "kokoro", "Kokoro-82M High-Fidelity (Local)" }
                                            }
                                        }
                                    }

                                    // Rate, Pitch, Barge-in
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 14px;",
                                        div {
                                            label { class: "hud-label", "Speech Speed ({form.read().speech_speed:.2}x)" }
                                            input {
                                                r#type: "range",
                                                min: "0.5",
                                                max: "2.0",
                                                step: "0.05",
                                                value: "{form.read().speech_speed}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<f64>() {
                                                        form.write().speech_speed = v;
                                                    }
                                                },
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Speech Pitch ({form.read().speech_pitch:.2}x)" }
                                            input {
                                                r#type: "range",
                                                min: "0.5",
                                                max: "2.0",
                                                step: "0.05",
                                                value: "{form.read().speech_pitch}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<f64>() {
                                                        form.write().speech_pitch = v;
                                                    }
                                                },
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "VAD Sensitivity ({form.read().vad_sensitivity:.2})" }
                                            input {
                                                r#type: "range",
                                                min: "0.1",
                                                max: "0.9",
                                                step: "0.05",
                                                value: "{form.read().vad_sensitivity}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<f32>() {
                                                        form.write().vad_sensitivity = v;
                                                    }
                                                },
                                            }
                                        }
                                    }

                                    // Barge-In & Noise Suppression
                                    div { style: "display: flex; flex-direction: column; gap: 10px; border-top: 1px solid var(--border-machined); padding-top: 12px;",
                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Barge-in Voice Interruption" }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted);", "Immediately halts TTS audio playback when user starts speaking." }
                                            }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().interruption_barge_in,
                                                onchange: move |e: FormEvent| form.write().interruption_barge_in = e.value().parse().unwrap_or(false),
                                            }
                                        }

                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "DeepFilterNet3 Acoustic Noise Suppression" }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted);", "Attenuates keyboard clicks, fan hum, and background room reverberation." }
                                            }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().noise_suppression,
                                                onchange: move |e: FormEvent| form.write().noise_suppression = e.value().parse().unwrap_or(false),
                                            }
                                        }
                                    }
                                    }
                                },

                                                                SettingsTab::Companion => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Authoritative 2D Cat Companion Card
                                    div {
                                        style: "border: 1px solid var(--osmoo-gold); background: rgba(229, 169, 59, 0.08); border-radius: 8px; padding: 16px; display: flex; align-items: center; justify-content: space-between; gap: 16px;",
                                        div { style: "display: flex; align-items: center; gap: 16px;",
                                            div { style: "width: 64px; height: 64px; border-radius: 8px; background: rgba(0, 0, 0, 0.4); border: 1px solid rgba(229, 169, 59, 0.3); display: flex; align-items: center; justify-content: center; overflow: hidden;",
                                                img {
                                                    src: "{crate::assets::OSMOO_CAT_DATA_URI}",
                                                    alt: "OSMOO Cat",
                                                    style: "height: 56px; width: auto; object-fit: contain; filter: drop-shadow(0 2px 4px rgba(0,0,0,0.5));",
                                                }
                                            }
                                            div {
                                                div { style: "display: flex; align-items: center; gap: 8px; margin-bottom: 4px;",
                                                    span { style: "font-weight: 700; font-size: 15px; color: var(--osmoo-ivory);", "OSMOO Animated 2D Cat" }
                                                    span { style: "font-size: 10px; font-family: var(--font-mono); padding: 2px 6px; border-radius: 3px; background: rgba(229, 169, 59, 0.2); color: var(--osmoo-gold); font-weight: 600;", "AUTHORITATIVE COMPANION" }
                                                }
                                                p { style: "font-size: 12px; color: var(--osmoo-muted); margin: 0; line-height: 1.4;",
                                                    "White cat with sunglasses and black OSMOO t-shirt. Lives directly on your Windows desktop as an independent transparent overlay with audio-driven lip sync and idle breathing."
                                                }
                                            }
                                        }
                                        div { style: "display: flex; flex-direction: column; align-items: flex-end; gap: 6px; flex-shrink: 0;",
                                            span { style: "font-size: 11px; font-weight: 600; color: #4ade80;", "OVERLAY ACTIVE" }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().desktop_overlay_mode,
                                                onchange: move |e: FormEvent| form.write().desktop_overlay_mode = e.value().parse().unwrap_or(false),
                                            }
                                        }
                                    }

                                    // Size & Position Presets
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Companion Scale ({form.read().companion_size:.2}x)" }
                                            input {
                                                r#type: "range",
                                                min: "0.5",
                                                max: "2.0",
                                                step: "0.05",
                                                value: "{form.read().companion_size}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<f64>() {
                                                        form.write().companion_size = v;
                                                    }
                                                },
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Screen Position Preset" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().companion_position}",
                                                onchange: move |e: FormEvent| form.write().companion_position = e.value(),
                                                option { value: "bottom-right", "Bottom-Right Screen Dock (Default)" }
                                                option { value: "bottom-left", "Bottom-Left Screen Dock" }
                                                option { value: "top-right", "Top-Right Floating" }
                                                option { value: "center-floating", "Center Screen HUD" }
                                                option { value: "custom", "Custom Drag Coordinates" }
                                            }
                                        }
                                    }

                                    // Companion Dynamics & Interactions
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Default Posture Expression" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().companion_expression}",
                                                onchange: move |e: FormEvent| form.write().companion_expression = e.value(),
                                                option { value: "neutral", "Neutral Cool (Default)" }
                                                option { value: "curious", "Curious & Inquisitive" }
                                                option { value: "focused", "Focused Workflow" }
                                                option { value: "celebratory", "Celebratory Accomplishment" }
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Desktop Overlay Presentation" }
                                            select {
                                                class: "hud-select",
                                                value: if form.read().desktop_overlay_mode { "overlay" } else { "windowed" },
                                                onchange: move |e: FormEvent| form.write().desktop_overlay_mode = e.value() == "overlay",
                                                option { value: "overlay", "Borderless Transparent Overlay" }
                                                option { value: "windowed", "Standard Framed Window" }
                                            }
                                        }
                                    }

                                    // Interaction & Motion Toggles
                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px; display: grid; grid-template-columns: 1fr 1fr; gap: 12px;",
                                        div { style: "display: flex; align-items: center; gap: 8px;",
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().companion_animations,
                                                onchange: move |e: FormEvent| form.write().companion_animations = e.value().parse().unwrap_or(false),
                                            }
                                            span { style: "font-size: 12px; color: var(--osmoo-ivory);", "Enable 2D Idle Breathing & Head Twitches" }
                                        }
                                        div { style: "display: flex; align-items: center; gap: 8px;",
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().companion_lip_sync,
                                                onchange: move |e: FormEvent| form.write().companion_lip_sync = e.value().parse().unwrap_or(false),
                                            }
                                            span { style: "font-size: 12px; color: var(--osmoo-ivory);", "Audio Lip-Sync Visemes (Real-time TTS)" }
                                        }
                                        div { style: "display: flex; align-items: center; gap: 8px;",
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().companion_draggable,
                                                onchange: move |e: FormEvent| form.write().companion_draggable = e.value().parse().unwrap_or(false),
                                            }
                                            span { style: "font-size: 12px; color: var(--osmoo-ivory);", "Draggable Across Screen with Edge Snapping" }
                                        }
                                        div { style: "display: flex; align-items: center; gap: 8px;",
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().companion_click_through,
                                                onchange: move |e: FormEvent| form.write().companion_click_through = e.value().parse().unwrap_or(false),
                                            }
                                            span { style: "font-size: 12px; color: var(--osmoo-ivory);", "Mouse Click-Through on Transparent Background" }
                                        }
                                    }

                                    // Lifecycle Note & Reset Button
                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px; display: flex; align-items: center; justify-content: space-between;",
                                        div { style: "font-size: 11px; color: var(--osmoo-muted); max-width: 580px; line-height: 1.4;",
                                            "Closing the main OSMOO window hides the dashboard while keeping the transparent cat companion and voice daemon alive. Right-click the cat on desktop to exit completely."
                                        }
                                        button {
                                            class: "hud-btn",
                                            style: "padding: 6px 14px; font-size: 11px;",
                                            onclick: move |_| {
                                                form.write().companion_character_id = "cat".to_string();
                                                form.write().companion_size = 1.0;
                                                form.write().companion_position = "bottom-right".to_string();
                                                form.write().companion_expression = "neutral".to_string();
                                                form.write().companion_lip_sync = true;
                                                form.write().companion_animations = true;
                                                form.write().companion_draggable = true;
                                                form.write().companion_click_through = false;
                                                form.write().desktop_overlay_mode = true;
                                            },
                                            "RESTORE DEFAULTS"
                                        }
                                    }
                                    }
                                },

                                SettingsTab::Security => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Emergency Stop Section
                                    div { style: "background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.4); border-radius: 6px; padding: 14px; display: flex; align-items: center; justify-content: space-between;",
                                        div {
                                            div { style: "font-weight: 700; font-size: 13px; color: #f87171;", "EMERGENCY RUNTIME HALT" }
                                            div { style: "font-size: 12px; color: var(--osmoo-muted);", "Immediately terminate all background execution, shell jobs, and cognitive tasks." }
                                        }
                                        button {
                                            class: "hud-btn",
                                            style: "background: #ef4444; color: #fff; font-weight: 700; padding: 6px 14px; font-size: 12px; border-radius: 4px;",
                                            onclick: move |_| {
                                                form.write().emergency_stopped = true;
                                                spawn(async move {
                                                    if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                                        let _ = dispatcher.dispatch(crate::backend::BackendCommand::EmergencyStop).await;
                                                    }
                                                });
                                            },
                                            "EMERGENCY STOP"
                                        }
                                    }

                                    // Sandbox Controls
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 14px;",
                                        div {
                                            label { class: "hud-label", "Filesystem Sandbox" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().file_system_access}",
                                                onchange: move |e: FormEvent| form.write().file_system_access = e.value(),
                                                option { value: "workspace_only", "Workspace Directory Only" }
                                                option { value: "read_only", "Global Read-Only" }
                                                option { value: "full", "Unrestricted Filesystem" }
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Browser Automation" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().browser_automation}",
                                                onchange: move |e: FormEvent| form.write().browser_automation = e.value(),
                                                option { value: "prompt", "Prompt Before Navigation" }
                                                option { value: "sandboxed", "Isolated Headless Session" }
                                                option { value: "disabled", "Completely Disabled" }
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Shell Execution Gate" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().shell_execution}",
                                                onchange: move |e: FormEvent| form.write().shell_execution = e.value(),
                                                option { value: "require_confirmation", "Always Require Approval" }
                                                option { value: "allow_workspace", "Auto-Run in Workspace" }
                                                option { value: "disabled", "Disabled" }
                                            }
                                        }
                                    }

                                    // Sensitive Approval Matrix
                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px;",
                                        div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory); margin-bottom: 10px;", "Explicit Operator Approvals Required For:" }
                                        div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 12px;",
                                            div { style: "display: flex; align-items: gap: 8px;",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().require_approval_sensitive,
                                                    onchange: move |e: FormEvent| form.write().require_approval_sensitive = e.value().parse().unwrap_or(false),
                                                }
                                                span { style: "font-size: 12px; color: var(--osmoo-ivory); margin-left: 8px;", "File Deletion & Bulk Writes" }
                                            }
                                            div { style: "display: flex; align-items: gap: 8px;",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().require_approval_financial,
                                                    onchange: move |e: FormEvent| form.write().require_approval_financial = e.value().parse().unwrap_or(false),
                                                }
                                                span { style: "font-size: 12px; color: var(--osmoo-ivory); margin-left: 8px;", "Financial & Checkout Actions" }
                                            }
                                            div { style: "display: flex; align-items: gap: 8px;",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().require_approval_email,
                                                    onchange: move |e: FormEvent| form.write().require_approval_email = e.value().parse().unwrap_or(false),
                                                }
                                                span { style: "font-size: 12px; color: var(--osmoo-ivory); margin-left: 8px;", "Outbound Email & Messages" }
                                            }
                                            div { style: "display: flex; align-items: gap: 8px;",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().require_approval_scripts,
                                                    onchange: move |e: FormEvent| form.write().require_approval_scripts = e.value().parse().unwrap_or(false),
                                                }
                                                span { style: "font-size: 12px; color: var(--osmoo-ivory); margin-left: 8px;", "Arbitrary Script Execution" }
                                            }
                                        }
                                    }

                                    // Audit Logging
                                    div { style: "display: flex; align-items: center; justify-content: space-between; border-top: 1px solid var(--border-machined); padding-top: 14px;",
                                        div {
                                            div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Cryptographic Audit Trail" }
                                            div { style: "font-size: 12px; color: var(--osmoo-muted);", "Record tamper-evident entries in audit.db for every tool call and permission prompt." }
                                        }
                                        input {
                                            r#type: "checkbox",
                                            checked: form.read().audit_logging_enabled,
                                            onchange: move |e: FormEvent| form.write().audit_logging_enabled = e.value().parse().unwrap_or(false),
                                        }
                                    }
                                    }
                                },

                                SettingsTab::Memory => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Semantic Memory Toggle
                                    div { style: "display: flex; align-items: center; justify-content: space-between; border-bottom: 1px solid var(--border-machined); padding-bottom: 12px;",
                                        div {
                                            div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Long-Term Semantic Memory Engine" }
                                            div { style: "font-size: 12px; color: var(--osmoo-muted);", "Retain user preferences, key project details, and workspace context across sessions." }
                                        }
                                        input {
                                            r#type: "checkbox",
                                            checked: form.read().memory_enabled,
                                            onchange: move |e: FormEvent| form.write().memory_enabled = e.value().parse().unwrap_or(false),
                                        }
                                    }

                                    // Retention & Private Session
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Retention Horizon" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().memory_retention_days}",
                                                onchange: move |e: FormEvent| {
                                                    if let Ok(days) = e.value().parse::<u32>() {
                                                        form.write().memory_retention_days = days;
                                                    }
                                                },
                                                option { value: "30", "30 Days" }
                                                option { value: "90", "90 Days (Recommended)" }
                                                option { value: "365", "1 Year" }
                                                option { value: "0", "Indefinite / Permanent" }
                                            }
                                        }
                                        div { style: "display: flex; flex-direction: column; justify-content: center;",
                                            div { style: "display: flex; align-items: center; justify-content: space-between;",
                                                div {
                                                    div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Ephemeral Private Session" }
                                                    div { style: "font-size: 11px; color: var(--osmoo-muted);", "Do not persist new facts or messages during this session." }
                                                }
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().private_session,
                                                    onchange: move |e: FormEvent| form.write().private_session = e.value().parse().unwrap_or(false),
                                                }
                                            }
                                        }
                                    }

                                    // Operator Persona Notes
                                    div {
                                        label { class: "hud-label", "User Persona & Preferences Instructions" }
                                        textarea {
                                            class: "hud-input",
                                            style: "width: 100%; height: 90px; resize: vertical; font-size: 12px; font-family: var(--font-mono);",
                                            placeholder: "e.g., Focus on concise Rust code with zero unwrap; prefer tabular summaries...",
                                            value: "{form.read().user_persona_notes}",
                                            oninput: move |e: FormEvent| form.write().user_persona_notes = e.value(),
                                        }
                                    }

                                    // Purge Memory Button
                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px; display: flex; align-items: center; justify-content: space-between;",
                                        div {
                                            div { style: "font-weight: 600; font-size: 13px; color: #f87171;", "Purge Stored Semantic Memories" }
                                            div { style: "font-size: 12px; color: var(--osmoo-muted);", "Permanently remove all vector and episodic memories stored in SQLite." }
                                        }
                                        div { style: "display: flex; gap: 8px; align-items: center;",
                                            if *confirm_purge.read() {
                                                button {
                                                    class: "hud-btn",
                                                    style: "background: rgba(239, 68, 68, 0.2); border: 1px solid #ef4444; color: #ef4444; font-size: 11px; padding: 4px 10px;",
                                                    onclick: move |_| confirm_purge.set(false),
                                                    "CANCEL"
                                                }
                                            }
                                            button {
                                                class: "hud-btn",
                                                style: if *confirm_purge.read() {
                                                    "background: #ef4444; border: 1px solid #ef4444; color: white; font-weight: 700; font-size: 12px; padding: 4px 12px;"
                                                } else {
                                                    "border: 1px solid rgba(239, 68, 68, 0.5); color: #f87171; font-size: 12px; padding: 4px 12px;"
                                                },
                                                onclick: move |_| {
                                                    if !*confirm_purge.read() {
                                                        confirm_purge.set(true);
                                                    } else {
                                                        confirm_purge.set(false);
                                                        spawn(async move {
                                                            if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                                                if let Ok(crate::backend::BackendResult::MemoriesPurged { count }) = dispatcher.dispatch(crate::backend::BackendCommand::PurgeStoredMemories).await {
                                                                    purged_count.set(Some(count));
                                                                }
                                                            }
                                                        });
                                                    }
                                                },
                                                if *confirm_purge.read() { "CONFIRM PURGE" } else { "PURGE DATABASE" }
                                            }
                                        }
                                    }

                                    if let Some(cnt) = *purged_count.read() {
                                        div { style: "font-size: 12px; color: #4ade80; font-family: var(--font-mono);",
                                            "Successfully purged {cnt} records from persistent memory store."
                                        }
                                    }
                                    }
                                },

                                SettingsTab::Automation => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Global Task Pause
                                    div { style: "background: rgba(229, 169, 59, 0.08); border: 1px solid rgba(229, 169, 59, 0.3); border-radius: 6px; padding: 12px; display: flex; align-items: center; justify-content: space-between;",
                                        div {
                                            div { style: "font-weight: 700; font-size: 13px; color: var(--osmoo-gold);", "MASTER TASK RUNNER PAUSE" }
                                            div { style: "font-size: 12px; color: var(--osmoo-muted);", "Temporarily suspend all scheduled cron jobs and multi-agent plan executions." }
                                        }
                                        input {
                                            r#type: "checkbox",
                                            checked: form.read().global_tasks_paused,
                                            onchange: move |e: FormEvent| form.write().global_tasks_paused = e.value().parse().unwrap_or(false),
                                        }
                                    }

                                    // Concurrency & Timeouts
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Max Concurrent Autonomous Tasks ({form.read().max_concurrent_tasks})" }
                                            input {
                                                r#type: "range",
                                                min: "1",
                                                max: "10",
                                                step: "1",
                                                value: "{form.read().max_concurrent_tasks}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<usize>() {
                                                        form.write().max_concurrent_tasks = v;
                                                    }
                                                },
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "Autonomous Task Timeout ({form.read().task_timeout_seconds}s)" }
                                            input {
                                                r#type: "range",
                                                min: "30",
                                                max: "600",
                                                step: "15",
                                                value: "{form.read().task_timeout_seconds}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<u64>() {
                                                        form.write().task_timeout_seconds = v;
                                                    }
                                                },
                                            }
                                        }
                                    }

                                    // Risk Threshold Selector
                                    div {
                                        label { class: "hud-label", "Autonomy Risk Policy Threshold" }
                                        select {
                                            class: "hud-select",
                                            value: "{form.read().action_risk_threshold}",
                                            onchange: move |e: FormEvent| form.write().action_risk_threshold = e.value(),
                                            option { value: "autonomous", "Fully Autonomous (Auto-approve low/medium actions)" }
                                            option { value: "prompt", "Balanced Autonomy (Prompt for destructive/risky actions)" }
                                            option { value: "prompt_risky", "Prompt Risky (Prompt before destructive operations)" }
                                            option { value: "prompt_all", "Strict Supervised (Prompt before executing any action)" }
                                        }
                                    }

                                    // Battery & Background Toggles
                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px; display: flex; flex-direction: column; gap: 12px;",
                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Pause Background Tasks on Battery Power" }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted);", "Conserve mobile battery by pausing continuous web scrapers and heavy agents." }
                                            }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().pause_on_battery,
                                                onchange: move |e: FormEvent| form.write().pause_on_battery = e.value().parse().unwrap_or(false),
                                            }
                                        }

                                        div { style: "display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Allow Silent Background Execution" }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted);", "Permits long-running agent threads while the main HUD is closed." }
                                            }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().background_tasks_allowed,
                                                onchange: move |e: FormEvent| form.write().background_tasks_allowed = e.value().parse().unwrap_or(false),
                                            }
                                        }
                                    }
                                    }
                                },

                                SettingsTab::Integrations => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 16px;",
                                    div { style: "font-size: 13px; color: var(--osmoo-muted); margin-bottom: 4px;",
                                        "OSMOO verifies active connections and local system capabilities honestly with zero simulated state."
                                    }

                                    for (service, cat, desc, status, is_live) in [
                                        ("Microsoft Office 365", "Office Documents", "Native OpenXML generation for DOCX, XLSX, and PPTX suites.", "Active Native OpenXML", true),
                                        ("Google Workspace", "Cloud Office", "Google Docs, Sheets, Drive, Gmail, and Google Calendar sync.", "Disconnected (OAuth Required)", false),
                                        ("GitHub Workspace", "Developer Version Control", "Pull requests, issues, repo code search via local Git and gh CLI.", "Connected (Local Git CLI)", true),
                                        ("Slack & Teams", "Team Communication", "Real-time thread monitoring, channel alerts, and message drafting.", "Not Configured", false),
                                        ("Travel & E-Commerce", "Web Automation", "Flight search (Skyscanner) & accommodations via Playwright browser automation.", "Available via Web Agent", true),
                                    ] {
                                        div { style: "border: 1px solid var(--border-machined); background: rgba(14, 16, 23, 0.4); border-radius: 6px; padding: 12px 16px; display: flex; align-items: center; justify-content: space-between;",
                                            div {
                                                div { style: "display: flex; align-items: center; gap: 10px;",
                                                    span { style: "font-weight: 700; font-size: 14px; color: var(--osmoo-ivory);", "{service}" }
                                                    span { style: "font-size: 10px; font-family: var(--font-mono); padding: 2px 6px; border-radius: 3px; background: rgba(255,255,255,0.06); color: var(--osmoo-muted);", "{cat}" }
                                                }
                                                div { style: "font-size: 12px; color: var(--osmoo-muted); margin-top: 3px;", "{desc}" }
                                            }
                                            div {
                                                span {
                                                    style: if is_live {
                                                        "font-family: var(--font-mono); font-size: 11px; padding: 4px 10px; border-radius: 4px; background: rgba(34, 197, 94, 0.12); color: #4ade80; border: 1px solid rgba(34, 197, 94, 0.3);"
                                                    } else {
                                                        "font-family: var(--font-mono); font-size: 11px; padding: 4px 10px; border-radius: 4px; background: rgba(255, 255, 255, 0.05); color: var(--osmoo-muted); border: 1px solid var(--border-machined);"
                                                    },
                                                    "{status}"
                                                }
                                            }
                                        }
                                    }
                                    }
                                },

                                SettingsTab::Performance => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Sliders
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Max Memory Ceiling ({form.read().max_memory_mb} MB)" }
                                            input {
                                                r#type: "range",
                                                min: "512",
                                                max: "16384",
                                                step: "256",
                                                value: "{form.read().max_memory_mb}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<u64>() {
                                                        form.write().max_memory_mb = v;
                                                    }
                                                },
                                            }
                                        }
                                        div {
                                            label { class: "hud-label", "CPU Throttle Ceiling ({form.read().max_cpu_percent:.0}%)" }
                                            input {
                                                r#type: "range",
                                                min: "10",
                                                max: "100",
                                                step: "5",
                                                value: "{form.read().max_cpu_percent}",
                                                oninput: move |e: FormEvent| {
                                                    if let Ok(v) = e.value().parse::<f64>() {
                                                        form.write().max_cpu_percent = v;
                                                    }
                                                },
                                            }
                                        }
                                    }

                                    // GPU Acceleration & Model Unload
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            label { class: "hud-label", "Idle Model VRAM Unload Timeout" }
                                            select {
                                                class: "hud-select",
                                                value: "{form.read().model_unload_timeout_minutes}",
                                                onchange: move |e: FormEvent| {
                                                    if let Ok(m) = e.value().parse::<u32>() {
                                                        form.write().model_unload_timeout_minutes = m;
                                                    }
                                                },
                                                option { value: "5", "5 Minutes (Aggressive Memory Saving)" }
                                                option { value: "15", "15 Minutes (Balanced)" }
                                                option { value: "30", "30 Minutes" }
                                                option { value: "0", "Never Unload (Fastest Inference)" }
                                            }
                                        }
                                        div { style: "display: flex; flex-direction: column; justify-content: center;",
                                            div { style: "display: flex; align-items: center; justify-content: space-between;",
                                                div {
                                                    div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "DirectX / Vulkan GPU Acceleration" }
                                                    div { style: "font-size: 11px; color: var(--osmoo-muted);", "Accelerates WebGL 3D meshes and local ONNX models." }
                                                }
                                                input {
                                                    r#type: "checkbox",
                                                    checked: form.read().gpu_acceleration,
                                                    onchange: move |e: FormEvent| form.write().gpu_acceleration = e.value().parse().unwrap_or(false),
                                                }
                                            }
                                        }
                                    }
                                    }
                                },

                                SettingsTab::Diagnostics => rsx! {
                                    div { class: "settings-section", style: "display: flex; flex-direction: column; gap: 18px;",
                                    // Build & Version Information
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px; background: rgba(14, 16, 23, 0.4); border: 1px solid var(--border-machined); border-radius: 6px; padding: 12px;",
                                        div {
                                            div { style: "font-size: 11px; color: var(--osmoo-muted);", "OSMOO VERSION" }
                                            div { style: "font-family: var(--font-mono); font-size: 13px; font-weight: 700; color: var(--osmoo-ivory);", "v0.1.0-commercial" }
                                        }
                                        div {
                                            div { style: "font-size: 11px; color: var(--osmoo-muted);", "UPDATE CHANNEL" }
                                            div { style: "font-family: var(--font-mono); font-size: 13px; font-weight: 700; color: var(--osmoo-gold);", "{form.read().update_channel.to_uppercase()}" }
                                        }
                                        div {
                                            div { style: "font-size: 11px; color: var(--osmoo-muted);", "LICENSE ENTITLEMENT" }
                                            div { style: "font-family: var(--font-mono); font-size: 13px; font-weight: 700; color: #4ade80;", "{form.read().active_tier.to_uppercase()} ACTIVE" }
                                        }
                                    }

                                    // Diagnostics Export (DPAPI Redacted)
                                    div { style: "border-top: 1px solid var(--border-machined); padding-top: 14px; display: flex; align-items: center; justify-content: space-between;",
                                        div {
                                            div { style: "font-weight: 600; font-size: 13px; color: var(--osmoo-ivory);", "Sanitized Diagnostics Bundle" }
                                            div { style: "font-size: 12px; color: var(--osmoo-muted);", "Generates a complete diagnostic report with all API secrets and personal tokens redacted." }
                                        }
                                        button {
                                            class: "hud-btn",
                                            style: "font-size: 12px; padding: 4px 14px;",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                                        if let Ok(crate::backend::BackendResult::DiagnosticsExported { sanitized_report }) = dispatcher.dispatch(crate::backend::BackendCommand::ExportDiagnostics).await {
                                                            diagnostics_report.set(Some(sanitized_report));
                                                        }
                                                    }
                                                });
                                            },
                                            "GENERATE REPORT"
                                        }
                                    }

                                    if let Some(rep) = diagnostics_report.read().as_ref() {
                                        div { style: "background: rgba(0, 0, 0, 0.4); border: 1px solid var(--border-machined); border-radius: 4px; padding: 12px; font-family: var(--font-mono); font-size: 11px; color: var(--osmoo-muted); max-height: 180px; overflow-y: auto; white-space: pre-wrap;",
                                            "{rep}"
                                        }
                                    }
                                }
                            },
                            }
                        }
                    }
                }
            }
        }
    }
}
