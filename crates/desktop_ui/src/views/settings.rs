use dioxus::prelude::*;
use crate::components::icons::{
    IconAlertCircle, IconCheck, IconCpu, IconMic, IconRefresh, IconSettings, IconShield, IconSparkles,
};

#[derive(Clone, Debug, PartialEq)]
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
            stt_provider: "deepgram".to_string(),
            tts_provider: "cartesia".to_string(),
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
    let mut save_status = use_signal(|| None::<String>);

    let tier_str = form.read().active_tier.to_lowercase();
    let tier_badge_class = format!("tier-name-badge {}", tier_str);

    rsx! {
        div { class: "settings-view-overlay",
            div { class: "settings-modal-card",
                div { class: "settings-header",
                    div { class: "settings-header-title-group",
                        IconSettings { size: 20 }
                        span { class: "settings-title", "SYSTEM PREFERENCES & HARDWARE" }
                    }
                    button {
                        class: "drawer-close-btn",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }

                div { class: "settings-content-scroll",
                    // Commercial Subscription & Account Tier
                    div { class: "settings-section",
                        h3 { class: "settings-section-title", "ACCOUNT & SUBSCRIPTION ENTITLEMENT" }
                        div { class: "tier-status-card",
                            div { class: "tier-status-left",
                                div { class: "tier-badge-cluster",
                                    span { class: "{tier_badge_class}", "{form.read().active_tier} TIER" }
                                    span { class: "tier-account-email", "{form.read().user_email}" }
                                }
                                div { class: "tier-features-summary",
                                    if tier_str == "pro" {
                                        "Full Local & Hybrid LLM Reasoning · Autonomous Workspace Tools · Permanent Memory"
                                    } else if tier_str == "enterprise" {
                                        "Enterprise Security · Custom Models · Team Memory Synchronization"
                                    } else {
                                        "Standard Local Features · Upgrade for Cloud Multimodal Models & Autonomous Agents"
                                    }
                                }
                            }
                            if tier_str == "free" {
                                button {
                                    class: "hud-btn upgrade-cta",
                                    onclick: move |_| {
                                        #[cfg(windows)]
                                        let _ = std::process::Command::new("cmd")
                                            .args(["/c", "start", "", "https://osmoo.in/pricing"])
                                            .spawn();
                                        #[cfg(not(windows))]
                                        let _ = std::process::Command::new("xdg-open")
                                            .arg("https://osmoo.in/pricing")
                                            .spawn();
                                    },
                                    IconSparkles { size: 14 }
                                    span { "Upgrade to Pro" }
                                }
                            }
                        }
                    }

                    // General Identity
                    div { class: "settings-section",
                        h3 { class: "settings-section-title", "IDENTITY & GENERAL" }
                        div { class: "form-group",
                            label { class: "form-label", "Companion Designation" }
                            input {
                                class: "hud-input",
                                r#type: "text",
                                value: "{form.read().assistant_name}",
                                oninput: move |e: FormEvent| {
                                    let mut f = form.read().clone();
                                    f.assistant_name = e.value();
                                    form.set(f);
                                }
                            }
                        }

                        div { class: "toggle-row",
                            div { class: "toggle-label-group",
                                div { class: "toggle-title", "Launch at Windows Startup" }
                                div { class: "toggle-desc", "Autostart in background companion mode upon login." }
                            }
                            input {
                                r#type: "checkbox",
                                class: "hud-checkbox",
                                checked: form.read().autolaunch,
                                onchange: move |e: FormEvent| {
                                    let mut f = form.read().clone();
                                    f.autolaunch = e.value() == "true";
                                    form.set(f);
                                }
                            }
                        }

                        div { class: "toggle-row",
                            div { class: "toggle-label-group",
                                div { class: "toggle-title", "Minimize to Tray on Close" }
                                div { class: "toggle-desc", "Keep audio pipeline and wake-word active when window is closed." }
                            }
                            input {
                                r#type: "checkbox",
                                class: "hud-checkbox",
                                checked: form.read().minimize_to_tray,
                                onchange: move |e: FormEvent| {
                                    let mut f = form.read().clone();
                                    f.minimize_to_tray = e.value() == "true";
                                    form.set(f);
                                }
                            }
                        }
                    }

                    // LLM & Cognition
                    div { class: "settings-section",
                        h3 { class: "settings-section-title", "REASONING & MODELS" }
                        div { class: "form-group",
                            label { class: "form-label", "Primary Provider" }
                            select {
                                class: "hud-select",
                                value: "{form.read().primary_llm}",
                                onchange: move |e: FormEvent| {
                                    let mut f = form.read().clone();
                                    f.primary_llm = e.value();
                                    form.set(f);
                                },
                                option { value: "ollama", "Ollama (Local Private Inference)" }
                                option { value: "cloud_hybrid", "Cloud Hybrid (Anthropic / OpenAI)" }
                            }
                        }

                        if form.read().primary_llm == "ollama" {
                            div { class: "form-group",
                                label { class: "form-label", "Installed Ollama Model" }
                                select {
                                    class: "hud-select",
                                    value: "{form.read().ollama_model}",
                                    onchange: move |e: FormEvent| {
                                        let mut f = form.read().clone();
                                        f.ollama_model = e.value();
                                        form.set(f);
                                    },
                                    for m in ollama_models.iter() {
                                        option { value: "{m}", "{m}" }
                                    }
                                }
                            }
                        }
                    }

                    // Audio & Acoustics
                    div { class: "settings-section",
                        h3 { class: "settings-section-title", "AUDIO & HARDWARE" }
                        div { class: "form-group",
                            label { class: "form-label", "Speech-to-Text Pipeline" }
                            select {
                                class: "hud-select",
                                value: "{form.read().stt_provider}",
                                onchange: move |e: FormEvent| {
                                    let mut f = form.read().clone();
                                    f.stt_provider = e.value();
                                    form.set(f);
                                },
                                option { value: "deepgram", "Deepgram Nova-2 (Lowest Latency Cloud)" }
                                option { value: "groq", "Groq Whisper Large-v3 (Ultra Fast)" }
                                option { value: "whisper_local", "Whisper Local (WASAPI On-Device)" }
                            }
                        }

                        div { class: "form-group",
                            label { class: "form-label", "Text-to-Speech Engine" }
                            select {
                                class: "hud-select",
                                value: "{form.read().tts_provider}",
                                onchange: move |e: FormEvent| {
                                    let mut f = form.read().clone();
                                    f.tts_provider = e.value();
                                    form.set(f);
                                },
                                option { value: "cartesia", "Cartesia Sonic (Sub-100ms Natural)" }
                                option { value: "elevenlabs", "ElevenLabs Turbo v2.5" }
                                option { value: "kokoro", "Kokoro-82M (Neural On-Device)" }
                                option { value: "sapi", "Windows SAPI (Offline Local Fallback)" }
                            }
                        }
                    }
                }

                div { class: "settings-footer",
                    if let Some(msg) = save_status.read().as_ref() {
                        span { class: "save-status-msg", "{msg}" }
                    } else {
                        span {}
                    }

                    div { class: "settings-btn-group",
                        button {
                            class: "hud-btn secondary",
                            onclick: move |_| on_close.call(()),
                            "Cancel"
                        }
                        button {
                            class: "hud-btn primary",
                            onclick: move |_| {
                                let f = form.read().clone();
                                on_save.call(f);
                                save_status.set(Some("Preferences saved".to_string()));
                            },
                            "Save Changes"
                        }
                    }
                }
            }
        }
    }
}
