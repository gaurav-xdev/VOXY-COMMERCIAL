use crate::components::icons::{IconCheck, IconSettings, IconShield};
use crate::fixtures::PROVIDER_CATALOG;
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SettingsTab {
    General,
    AgentIdentity,
    AiProviders,
    LocalModels,
    Voice,
    Appearance,
    PrivacySecurity,
    Subscription,
    Diagnostics,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingsFormState {

    pub assistant_name: String,
    pub primary_llm: String,
    pub ollama_model: String,
    pub autolaunch: bool,
    pub stt_provider: String,
    pub tts_provider: String,
    pub audio_sample_rate: u32,
    pub vad_sensitivity: f32,
    pub minimize_to_tray: bool,
    pub user_email: String,
    pub active_tier: String,
}

impl Default for SettingsFormState {
    fn default() -> Self {
        Self {
            assistant_name: "OSMOO Core".to_string(),
            primary_llm: "ollama".to_string(),
            ollama_model: "llama3.2:3b".to_string(),
            autolaunch: true,
            stt_provider: "sherpa_onnx_whisper".to_string(),
            tts_provider: "piper_en_libritts".to_string(),
            audio_sample_rate: 48000,
            vad_sensitivity: 0.5,
            minimize_to_tray: true,
            user_email: "operator@osmoo.in".to_string(),
            active_tier: "free".to_string(),
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
    let mut save_status = use_signal(|| None::<String>);

    let tab = *current_tab.read();

    rsx! {
        div { class: "modal-overlay",
            div { class: "machined-card modal-container", style: "max-width: 860px; height: 680px;",
                div { class: "machined-card-inner", style: "padding: 0; display: flex; flex-direction: column; height: 100%;",
                    // Header
                    div { class: "modal-header",
                        div { style: "display: flex; align-items: center; gap: 10px;",
                            IconSettings { size: 18 }
                            span { style: "font-family: var(--font-mono); font-size: 13px; font-weight: 700; letter-spacing: 0.12em; color: var(--osmoo-ivory);",
                                "SYSTEM PREFERENCES & HARDWARE SUBSYSTEMS"
                            }
                        }
                        button {
                            class: "hud-icon-btn",
                            style: "width: 32px; height: 32px; font-size: 18px;",
                            onclick: move |_| on_close.call(()),
                            "×"
                        }
                    }

                    // 2-Column Layout: Sidebar Nav + Scrollable Section Body
                    div { style: "display: flex; flex: 1; overflow: hidden;",
                        // Left Sidebar Navigation (9 Sections)
                        div { style: "width: 220px; border-right: 1px solid var(--border-machined); background: rgba(14, 16, 23, 0.4); padding: 16px 12px; display: flex; flex-direction: column; gap: 4px;",
                            {
                                let tabs = [
                                    (SettingsTab::General, "General"),
                                    (SettingsTab::AgentIdentity, "Agent Identity"),
                                    (SettingsTab::AiProviders, "AI Providers"),
                                    (SettingsTab::LocalModels, "Local Models"),
                                    (SettingsTab::Voice, "Voice & Audio"),
                                    (SettingsTab::Appearance, "Appearance"),
                                    (SettingsTab::PrivacySecurity, "Privacy & Security"),
                                    (SettingsTab::Subscription, "Subscription"),
                                    (SettingsTab::Diagnostics, "Diagnostics"),
                                ];

                                rsx! {
                                    for (t, label) in tabs.iter() {
                                        {
                                            let is_active = tab == *t;
                                            let t_val = *t;
                                            let btn_bg = if is_active { "rgba(244, 240, 232, 0.08)" } else { "transparent" };
                                            let btn_color = if is_active { "var(--osmoo-ivory)" } else { "var(--text-secondary)" };
                                            rsx! {
                                                button {
                                                    style: "text-align: left; padding: 10px 14px; border-radius: 8px; border: none; font-size: 12px; font-weight: 600; cursor: pointer; transition: all 0.2s ease; background: {btn_bg}; color: {btn_color};",
                                                    onclick: move |_| current_tab.set(t_val),
                                                    "{label}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Right Section Content
                        div { class: "modal-body", style: "flex: 1; padding: 24px 28px;",
                            // 1. General
                            if tab == SettingsTab::General {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "General System Settings" }
                                    div { style: "display: flex; flex-direction: column; gap: 16px;",
                                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                                            div {
                                                div { style: "font-size: 13px; font-weight: 600;", "Launch on Windows Startup" }
                                                div { style: "font-size: 11px; color: var(--text-muted);", "Automatically supervise companion daemon upon user login." }
                                            }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().autolaunch,
                                                onchange: move |e: FormEvent| form.write().autolaunch = e.value().parse().unwrap_or(false),
                                            }
                                        }
                                        div { style: "display: flex; justify-content: space-between; align-items: center;",
                                            div {
                                                div { style: "font-size: 13px; font-weight: 600;", "Minimize to System Tray" }
                                                div { style: "font-size: 11px; color: var(--text-muted);", "Keep background listener alive in notification area." }
                                            }
                                            input {
                                                r#type: "checkbox",
                                                checked: form.read().minimize_to_tray,
                                                onchange: move |e: FormEvent| form.write().minimize_to_tray = e.value().parse().unwrap_or(false),
                                            }
                                        }
                                    }
                                }
                            }

                            // 2. Agent Identity
                            if tab == SettingsTab::AgentIdentity {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Companion Persona & Designation" }
                                    div { style: "margin-bottom: 16px;",
                                        label { class: "form-label", "Operator Designation" }
                                        input {
                                            class: "hud-input-field",
                                            value: "{form.read().assistant_name}",
                                            oninput: move |e: FormEvent| form.write().assistant_name = e.value(),
                                        }
                                    }
                                }
                            }

                            // 3. AI Providers
                            if tab == SettingsTab::AiProviders {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Cloud Reasoning Providers" }
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 10px;",
                                        for p in PROVIDER_CATALOG.iter() {
                                            {
                                                let is_sel = form.read().primary_llm == p.id;
                                                let p_id = p.id.to_string();
                                                let card_bg = if is_sel { "rgba(244, 240, 232, 0.08)" } else { "rgba(14, 16, 23, 0.5)" };
                                                rsx! {
                                                    div {
                                                        style: "padding: 12px; border: 1px solid var(--border-machined); border-radius: 10px; cursor: pointer; background: {card_bg};",
                                                        onclick: move |_| form.write().primary_llm = p_id.clone(),
                                                        div { style: "display: flex; justify-content: space-between;",
                                                            span { style: "font-size: 13px; font-weight: 600;", "{p.name}" }
                                                            if is_sel { IconCheck { size: 14 } }
                                                        }
                                                        span { style: "font-size: 10px; font-family: var(--font-mono); color: var(--osmoo-metallic);", "{p.badge}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 4. Local Models
                            if tab == SettingsTab::LocalModels {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Local Hardware Runtime Models" }
                                    if ollama_models.is_empty() {
                                        div { style: "background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.3); border-radius: 10px; padding: 16px; text-align: center;",
                                            div { style: "font-family: var(--font-mono); font-weight: 700; color: #fca5a5; font-size: 13px;", "NO LOCAL AVAILABLE" }
                                            span { style: "font-size: 11px; color: var(--text-muted);", "Ensure Ollama service is active at http://127.0.0.1:11434" }
                                        }
                                    } else {
                                        div {
                                            for model in ollama_models.iter() {
                                                div { style: "padding: 10px 14px; border: 1px solid var(--border-machined); border-radius: 8px; margin-bottom: 6px; font-family: var(--font-mono); font-size: 12px;",
                                                    "{model}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // 5. Voice
                            if tab == SettingsTab::Voice {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Voice & Audio Synthesizer Parameters" }
                                    div { style: "margin-bottom: 16px;",
                                        label { class: "form-label", "Audio Sample Rate (Hz)" }
                                        input {
                                            class: "hud-input-field",
                                            r#type: "number",
                                            value: "{form.read().audio_sample_rate}",
                                            oninput: move |e: FormEvent| form.write().audio_sample_rate = e.value().parse().unwrap_or(48000),
                                        }
                                    }
                                }
                            }

                            // 6. Appearance
                            if tab == SettingsTab::Appearance {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Visual Presentation & Form" }
                                    p { style: "font-size: 12px; color: var(--text-secondary);",
                                        "Palette: Warm Ivory, Deep Onyx, Oxblood, and Warm Metallic Neutral. WebGL 3D Companion acceleration is active."
                                    }
                                }
                            }

                            // 7. Privacy & Security
                            if tab == SettingsTab::PrivacySecurity {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Security Controls & Zero-Storage Rules" }
                                    div { style: "background: rgba(185, 169, 149, 0.1); border: 1px solid rgba(185, 169, 149, 0.25); border-radius: 10px; padding: 14px;",
                                        div { style: "display: flex; gap: 8px; align-items: center; margin-bottom: 6px;",
                                            IconShield { size: 16 }
                                            span { style: "font-weight: 700; font-size: 13px;", "Protected Runtime Boundary" }
                                        }
                                        p { style: "font-size: 11px; color: var(--osmoo-soft-stone); line-height: 1.5;",
                                            "All sensitive credentials in prototype workflows remain isolated in memory and are discarded upon application termination."
                                        }
                                    }
                                }
                            }

                            // 8. Subscription
                            if tab == SettingsTab::Subscription {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Commercial Account Entitlement" }
                                    div { style: "display: flex; justify-content: space-between; align-items: center; background: rgba(14, 16, 23, 0.8); border: 1px solid var(--border-machined); border-radius: 12px; padding: 16px; margin-bottom: 16px;",
                                        div {
                                            div { style: "font-weight: 700; font-size: 14px;", "CURRENT ENTITLEMENT: FREE COMMUNITY" }
                                            div { style: "font-size: 11px; color: var(--text-muted); margin-top: 2px;", "Operator: operator@osmoo.in" }
                                        }
                                        button { class: "hud-btn primary", "Upgrade to Pro" }
                                    }
                                }
                            }

                            // 9. Diagnostics
                            if tab == SettingsTab::Diagnostics {
                                div {
                                    h3 { style: "font-size: 16px; margin-bottom: 16px; font-weight: 700;", "Subsystem Diagnostic Metrics" }
                                    div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-secondary); background: rgba(0, 0, 0, 0.4); padding: 16px; border-radius: 10px; border: 1px solid var(--border-machined);",
                                        div { "DIAGNOSTIC NOTICE: Real-time telemetry is reported exclusively via authenticated named pipe IPC." }
                                        div { style: "margin-top: 8px; color: var(--osmoo-metallic);", "PIPE TARGET: \\\\.\\pipe\\voxy-com-ipc" }
                                        div { style: "color: var(--osmoo-metallic);", "CLIENT ROLE: DESKTOP_UI_SUPERVISED" }
                                    }
                                }
                            }
                        }
                    }

                    // Footer Save Action
                    div { class: "modal-footer",
                        if let Some(msg) = save_status.read().as_ref() {
                            span { style: "font-size: 12px; color: #86efac; font-family: var(--font-mono);", "{msg}" }
                        } else {
                            div {}
                        }
                        div { style: "display: flex; gap: 10px;",
                            button { class: "hud-btn", onclick: move |_| on_close.call(()), "Cancel" }
                            button {
                                class: "hud-btn primary",
                                onclick: move |_| {
                                    on_save.call(form.read().clone());
                                    save_status.set(Some("Preferences synchronized.".to_string()));
                                },
                                "Apply Settings"
                            }
                        }
                    }
                }
            }
        }
    }
}
