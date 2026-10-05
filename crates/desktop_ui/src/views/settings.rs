use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    Voice,
    Models,
    Memory,
    Automation,
    Privacy,
    Updates,
    Performance,
    Developer,
    Plugins,
    Appearance,
}

#[component]
pub fn SettingsView() -> Element {
    let bridge = use_context::<AppBridge>();
    let active_tab = use_signal(|| SettingsTab::Voice);
    let save_status = use_signal(|| None::<String>);

    let save_settings = {
        let settings = bridge.settings.clone();
        let status = save_status;
        move |_: Event<MouseData>| {
            let s = settings.clone();
            let snap = s.get();
            let mut st = status;
            spawn(async move {
                match s.update(snap) {
                    Ok(()) => st.set(Some("Settings saved".to_string())),
                    Err(e) => st.set(Some(format!("Save error: {}", e))),
                }
            });
        }
    };

    rsx! {
        div {
            div { class: "tab-bar",
                TabBtn { active_tab: active_tab, tab: SettingsTab::Voice, label: "Voice" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Models, label: "Models" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Memory, label: "Memory" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Automation, label: "Automation" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Privacy, label: "Privacy" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Updates, label: "Updates" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Performance, label: "Performance" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Developer, label: "Developer" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Plugins, label: "Plugins" }
                TabBtn { active_tab: active_tab, tab: SettingsTab::Appearance, label: "Appearance" }
            }

            if let Some(msg) = save_status.read().as_ref() {
                div { style: "margin-bottom: 16px; padding: 10px; background: rgba(0,210,211,0.1); border: 1px solid var(--success); border-radius: var(--radius-md); font-size: 13px; color: var(--success);",
                    "{msg}"
                }
            }

            match *active_tab.read() {
                SettingsTab::Voice => rsx! { VoiceSettingsTab {} },
                SettingsTab::Models => rsx! { ModelsSettingsTab {} },
                SettingsTab::Memory => rsx! { MemorySettingsTab {} },
                SettingsTab::Automation => rsx! { AutomationSettingsTab {} },
                SettingsTab::Privacy => rsx! { PrivacySettingsTab {} },
                SettingsTab::Updates => rsx! { UpdatesSettingsTab {} },
                SettingsTab::Performance => rsx! { PerformanceSettingsTab {} },
                SettingsTab::Developer => rsx! { DeveloperSettingsTab {} },
                SettingsTab::Plugins => rsx! { PluginsSettingsTab {} },
                SettingsTab::Appearance => rsx! { AppearanceSettingsTab {} },
            }

            div { style: "margin-top: 16px; display: flex; gap: 8px;",
                button { class: "btn btn-primary", onclick: save_settings, "Save Settings" }
                button { class: "btn btn-secondary", onclick: {
                    let settings = bridge.settings.clone();
                    move |_: Event<MouseData>| {
                        let s = settings.clone();
                        spawn(async move { let _ = s.rollback(); });
                    }
                }, "Rollback" }
            }
        }
    }
}

#[component]
fn TabBtn(mut active_tab: Signal<SettingsTab>, tab: SettingsTab, label: &'static str) -> Element {
    let is_active = *active_tab.read() == tab;
    rsx! {
        button {
            class: if is_active { "tab active" } else { "tab" },
            onclick: move |_| active_tab.set(tab),
            "{label}"
        }
    }
}

fn toggle_class(active: bool) -> &'static str {
    if active {
        "toggle active"
    } else {
        "toggle"
    }
}

#[component]
fn VoiceSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Voice Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Enable Voice" }, div { class: "setting-desc", "Turn on voice input/output" } }
                    div {
                        class: toggle_class(s.read().voice.enabled),
                        onclick: move |_| {
                            let mut snap = s.write();
                            snap.voice.enabled = !snap.voice.enabled;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Wake Word" }, div { class: "setting-desc", "Word to activate voice" } }
                    input {
                        class: "input",
                        value: "{s.read().voice.wake_word}",
                        style: "width: 200px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().voice.wake_word = evt.value();
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Always Listening" }, div { class: "setting-desc", "Keep microphone active" } }
                    div {
                        class: toggle_class(s.read().voice.always_listening),
                        onclick: move |_| {
                            let val = s.read().voice.always_listening;
                            s.write().voice.always_listening = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Noise Suppression" }, div { class: "setting-desc", "Reduce background noise" } }
                    div {
                        class: toggle_class(s.read().voice.noise_suppression),
                        onclick: move |_| {
                            let val = s.read().voice.noise_suppression;
                            s.write().voice.noise_suppression = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Echo Cancellation" }, div { class: "setting-desc", "Cancel speaker echo" } }
                    div {
                        class: toggle_class(s.read().voice.echo_cancellation),
                        onclick: move |_| {
                            let val = s.read().voice.echo_cancellation;
                            s.write().voice.echo_cancellation = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Language" }, div { class: "setting-desc", "Voice language" } }
                    input {
                        class: "input",
                        value: "{s.read().voice.language}",
                        style: "width: 160px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().voice.language = evt.value();
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "STT Provider" }, div { class: "setting-desc", "Speech-to-Text engine" } }
                    input {
                        class: "input",
                        value: "{s.read().voice.stt_provider.clone().unwrap_or_else(|| \"groq\".to_string())}",
                        style: "width: 160px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().voice.stt_provider = Some(evt.value());
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "TTS Provider" }, div { class: "setting-desc", "Text-to-Speech synthesizer" } }
                    input {
                        class: "input",
                        value: "{s.read().voice.tts_provider.clone().unwrap_or_else(|| \"cartesia\".to_string())}",
                        style: "width: 160px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().voice.tts_provider = Some(evt.value());
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Registered Wake Words" }, div { class: "setting-desc", "Phrases that awaken OSMOO (comma-separated)" } }
                    input {
                        class: "input",
                        value: "{s.read().voice.wake_words.join(\", \")}",
                        style: "width: 280px;",
                        oninput: move |evt: Event<FormData>| {
                            let words: Vec<String> = evt.value().split(',').map(|w| w.trim().to_string()).filter(|w| !w.is_empty()).collect();
                            s.write().voice.wake_words = words;
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ModelsSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Model Configuration" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Provider" }, div { class: "setting-desc", "LLM provider" } }
                    input {
                        class: "input",
                        value: "{s.read().models.provider}",
                        style: "width: 200px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().models.provider = evt.value();
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Model" }, div { class: "setting-desc", "Model to use" } }
                    input {
                        class: "input",
                        value: "{s.read().models.model}",
                        style: "width: 200px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().models.model = evt.value();
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Temperature" }, div { class: "setting-desc", "Response creativity" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().models.temperature}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<f64>() {
                                s.write().models.temperature = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Max Tokens" }, div { class: "setting-desc", "Maximum response length" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().models.max_tokens}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u32>() {
                                s.write().models.max_tokens = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Local Only" }, div { class: "setting-desc", "Prevent cloud API calls" } }
                    div {
                        class: toggle_class(s.read().models.local_only),
                        onclick: move |_| {
                            let val = s.read().models.local_only;
                            s.write().models.local_only = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Fallback Providers" }, div { class: "setting-desc", "Ordered backup providers (comma-separated)" } }
                    input {
                        class: "input",
                        value: "{s.read().models.fallback_providers.join(\", \")}",
                        style: "width: 240px;",
                        oninput: move |evt: Event<FormData>| {
                            let providers: Vec<String> = evt.value().split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
                            s.write().models.fallback_providers = providers;
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn MemorySettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Memory Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Enable Memory" }, div { class: "setting-desc", "Allow OSMOO to remember things" } }
                    div {
                        class: toggle_class(s.read().memory.enabled),
                        onclick: move |_| {
                            let val = s.read().memory.enabled;
                            s.write().memory.enabled = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Auto Consolidate" }, div { class: "setting-desc", "Automatically merge similar memories" } }
                    div {
                        class: toggle_class(s.read().memory.auto_consolidate),
                        onclick: move |_| {
                            let val = s.read().memory.auto_consolidate;
                            s.write().memory.auto_consolidate = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Retention Days" }, div { class: "setting-desc", "How long to keep memories" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().memory.retention_days}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u32>() {
                                s.write().memory.retention_days = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Max Items" }, div { class: "setting-desc", "Maximum memory entries" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().memory.max_items}",
                        style: "width: 120px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<usize>() {
                                s.write().memory.max_items = v;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AutomationSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Automation Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Enable Automation" }, div { class: "setting-desc", "Allow automated tasks" } }
                    div {
                        class: toggle_class(s.read().automation.enabled),
                        onclick: move |_| {
                            let val = s.read().automation.enabled;
                            s.write().automation.enabled = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Require Consent" }, div { class: "setting-desc", "Ask before executing tasks" } }
                    div {
                        class: toggle_class(s.read().automation.require_consent),
                        onclick: move |_| {
                            let val = s.read().automation.require_consent;
                            s.write().automation.require_consent = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Max Concurrent" }, div { class: "setting-desc", "Parallel task limit" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().automation.max_concurrent_tasks}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<usize>() {
                                s.write().automation.max_concurrent_tasks = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Timeout (s)" }, div { class: "setting-desc", "Task execution timeout" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().automation.timeout_seconds}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u64>() {
                                s.write().automation.timeout_seconds = v;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PrivacySettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Privacy Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Telemetry" }, div { class: "setting-desc", "Send usage telemetry" } }
                    div {
                        class: toggle_class(s.read().privacy.telemetry_enabled),
                        onclick: move |_| {
                            let val = s.read().privacy.telemetry_enabled;
                            s.write().privacy.telemetry_enabled = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Crash Reports" }, div { class: "setting-desc", "Send crash reports" } }
                    div {
                        class: toggle_class(s.read().privacy.crash_reports),
                        onclick: move |_| {
                            let val = s.read().privacy.crash_reports;
                            s.write().privacy.crash_reports = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Local Processing Only" }, div { class: "setting-desc", "Never send data to cloud" } }
                    div {
                        class: toggle_class(s.read().privacy.local_processing_only),
                        onclick: move |_| {
                            let val = s.read().privacy.local_processing_only;
                            s.write().privacy.local_processing_only = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Data Retention (days)" }, div { class: "setting-desc", "How long to keep data" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().privacy.data_retention_days}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u32>() {
                                s.write().privacy.data_retention_days = v;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn UpdatesSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Update Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Auto Update" }, div { class: "setting-desc", "Automatically install updates" } }
                    div {
                        class: toggle_class(s.read().updates.auto_update),
                        onclick: move |_| {
                            let val = s.read().updates.auto_update;
                            s.write().updates.auto_update = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Check Interval (hours)" }, div { class: "setting-desc", "How often to check" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().updates.check_interval_hours}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u64>() {
                                s.write().updates.check_interval_hours = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Channel" }, div { class: "setting-desc", "Update channel" } }
                    input {
                        class: "input",
                        value: "{s.read().updates.channel}",
                        style: "width: 160px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().updates.channel = evt.value();
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PerformanceSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Performance Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Max Memory (MB)" }, div { class: "setting-desc", "Memory usage limit" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().performance.max_memory_mb}",
                        style: "width: 120px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u64>() {
                                s.write().performance.max_memory_mb = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Max CPU %" }, div { class: "setting-desc", "CPU usage limit" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().performance.max_cpu_percent}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<f64>() {
                                s.write().performance.max_cpu_percent = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "GPU Acceleration" }, div { class: "setting-desc", "Use GPU for inference" } }
                    div {
                        class: toggle_class(s.read().performance.gpu_acceleration),
                        onclick: move |_| {
                            let val = s.read().performance.gpu_acceleration;
                            s.write().performance.gpu_acceleration = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Background Throttle" }, div { class: "setting-desc", "Reduce usage when minimized" } }
                    div {
                        class: toggle_class(s.read().performance.background_throttle),
                        onclick: move |_| {
                            let val = s.read().performance.background_throttle;
                            s.write().performance.background_throttle = !val;
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DeveloperSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Developer Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Debug Mode" }, div { class: "setting-desc", "Enable debug features" } }
                    div {
                        class: toggle_class(s.read().developer.debug_mode),
                        onclick: move |_| {
                            let val = s.read().developer.debug_mode;
                            s.write().developer.debug_mode = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Log Level" }, div { class: "setting-desc", "Logging verbosity" } }
                    input {
                        class: "input",
                        value: "{s.read().developer.log_level}",
                        style: "width: 160px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().developer.log_level = evt.value();
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Show FPS" }, div { class: "setting-desc", "Display frame rate" } }
                    div {
                        class: toggle_class(s.read().developer.show_fps),
                        onclick: move |_| {
                            let val = s.read().developer.show_fps;
                            s.write().developer.show_fps = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Expose Metrics" }, div { class: "setting-desc", "Enable metrics endpoint" } }
                    div {
                        class: toggle_class(s.read().developer.expose_metrics),
                        onclick: move |_| {
                            let val = s.read().developer.expose_metrics;
                            s.write().developer.expose_metrics = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Metrics Port" }, div { class: "setting-desc", "Prometheus port" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().developer.metrics_port}",
                        style: "width: 100px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u16>() {
                                s.write().developer.metrics_port = v;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PluginsSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Plugin Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Enable Plugins" }, div { class: "setting-desc", "Allow plugin loading" } }
                    div {
                        class: toggle_class(s.read().plugins.enabled),
                        onclick: move |_| {
                            let val = s.read().plugins.enabled;
                            s.write().plugins.enabled = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Sandbox Mode" }, div { class: "setting-desc", "Run plugins in sandbox" } }
                    div {
                        class: toggle_class(s.read().plugins.sandbox_mode),
                        onclick: move |_| {
                            let val = s.read().plugins.sandbox_mode;
                            s.write().plugins.sandbox_mode = !val;
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AppearanceSettingsTab() -> Element {
    let bridge = use_context::<AppBridge>();
    let mut s = use_signal(|| bridge.settings.get());
    rsx! {
        div {
            div { class: "card",
                div { class: "card-header", span { class: "card-title", "Appearance Settings" } }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Theme" }, div { class: "setting-desc", "Color theme" } }
                    input {
                        class: "input",
                        value: "{s.read().appearance.theme}",
                        style: "width: 160px;",
                        oninput: move |evt: Event<FormData>| {
                            s.write().appearance.theme = evt.value();
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Font Size" }, div { class: "setting-desc", "Text size" } }
                    input {
                        class: "input",
                        r#type: "number",
                        value: "{s.read().appearance.font_size}",
                        style: "width: 80px;",
                        oninput: move |evt: Event<FormData>| {
                            if let Ok(v) = evt.value().parse::<u32>() {
                                s.write().appearance.font_size = v;
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Always on Top" }, div { class: "setting-desc", "Keep window on top" } }
                    div {
                        class: toggle_class(s.read().appearance.always_on_top),
                        onclick: move |_| {
                            let val = s.read().appearance.always_on_top;
                            s.write().appearance.always_on_top = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Start OSMOO with Windows" }, div { class: "setting-desc", "Silently initialize OSMOO and desktop companion on system boot" } }
                    div {
                        class: toggle_class(s.read().appearance.start_with_windows),
                        onclick: move |_| {
                            let current = s.read().appearance.start_with_windows;
                            let new_val = !current;
                            s.write().appearance.start_with_windows = new_val;
                            let al = voxy_desktop_runtime::autolaunch::AutoLauncher::new("OSMOO");
                            if new_val {
                                let _ = al.enable();
                            } else {
                                let _ = al.disable();
                            }
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Smart Desktop Overlay" }, div { class: "setting-desc", "Display the volumetric 3D gimbal companion overlay on your desktop" } }
                    div {
                        class: toggle_class(s.read().appearance.overlay_enabled),
                        onclick: move |_| {
                            let val = s.read().appearance.overlay_enabled;
                            s.write().appearance.overlay_enabled = !val;
                        }
                    }
                }
                div { class: "setting-row",
                    div { div { class: "setting-label", "Close Window to Overlay" }, div { class: "setting-desc", "Closing the main window keeps OSMOO companion running in background/overlay" } }
                    div {
                        class: toggle_class(s.read().appearance.close_to_overlay),
                        onclick: move |_| {
                            let val = s.read().appearance.close_to_overlay;
                            s.write().appearance.close_to_overlay = !val;
                        }
                    }
                }
            }
        }
    }
}
