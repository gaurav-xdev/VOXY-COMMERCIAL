use crate::components::assistant_core::CompanionMode;
use crate::components::icons::{IconCheck, IconChevronRight, IconRobot, IconSparkles};
use crate::fixtures::{PROVIDER_CATALOG, SUBSCRIPTION_PLANS, VOICE_CATALOG};
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OnboardingStep {
    Identity = 1,
    LocalRuntime = 2,
    CloudProviders = 3,
    CredentialEntry = 4,
    VoiceConfig = 5,
    SubscriptionTiers = 6,
    Summary = 7,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OnboardingConfig {
    pub assistant_name: String,
    pub companion_mode: CompanionMode,
    pub primary_llm: String,
    pub selected_model: String,
    pub stt_provider: String,
    pub tts_provider: String,
    pub active_tier: String,
}

impl Default for OnboardingConfig {
    fn default() -> Self {
        Self {
            assistant_name: "OSMOO Core".to_string(),
            companion_mode: CompanionMode::Robot,
            primary_llm: "ollama".to_string(),
            selected_model: "llama3.2:3b".to_string(),
            stt_provider: "sherpa_onnx_whisper".to_string(),
            tts_provider: "piper_en_libritts".to_string(),
            active_tier: "free".to_string(),
        }
    }
}

#[component]
pub fn OnboardingWizard(
    on_finish: EventHandler<OnboardingConfig>,
    initial_ollama_models: Vec<String>,
) -> Element {
    let mut current_step = use_signal(|| OnboardingStep::Identity);
    let mut name_input = use_signal(|| "OSMOO Core".to_string());
    let mut selected_mode = use_signal(|| CompanionMode::Robot);

    let ollama_models = use_signal(move || initial_ollama_models);
    let mut selected_model = use_signal(|| "llama3.2:3b".to_string());

    let mut selected_cloud_provider = use_signal(|| "openai".to_string());
    let mut prototype_api_key = use_signal(String::new);
    let mut show_key_masked = use_signal(|| true);

    let selected_stt = use_signal(|| "sherpa_onnx_whisper".to_string());
    let mut selected_tts = use_signal(|| "piper_en_libritts".to_string());

    let mut selected_tier = use_signal(|| "free".to_string());

    let step_num = match *current_step.read() {
        OnboardingStep::Identity => 1,
        OnboardingStep::LocalRuntime => 2,
        OnboardingStep::CloudProviders => 3,
        OnboardingStep::CredentialEntry => 4,
        OnboardingStep::VoiceConfig => 5,
        OnboardingStep::SubscriptionTiers => 6,
        OnboardingStep::Summary => 7,
    };

    let next_step = move |_| {
        let next = match *current_step.read() {
            OnboardingStep::Identity => OnboardingStep::LocalRuntime,
            OnboardingStep::LocalRuntime => OnboardingStep::CloudProviders,
            OnboardingStep::CloudProviders => OnboardingStep::CredentialEntry,
            OnboardingStep::CredentialEntry => OnboardingStep::VoiceConfig,
            OnboardingStep::VoiceConfig => OnboardingStep::SubscriptionTiers,
            OnboardingStep::SubscriptionTiers => OnboardingStep::Summary,
            OnboardingStep::Summary => {
                on_finish.call(OnboardingConfig {
                    assistant_name: name_input.read().clone(),
                    companion_mode: *selected_mode.read(),
                    primary_llm: selected_cloud_provider.read().clone(),
                    selected_model: selected_model.read().clone(),
                    stt_provider: selected_stt.read().clone(),
                    tts_provider: selected_tts.read().clone(),
                    active_tier: selected_tier.read().clone(),
                });
                return;
            }
        };
        current_step.set(next);
    };

    let prev_step = move |_| {
        let prev = match *current_step.read() {
            OnboardingStep::Identity => OnboardingStep::Identity,
            OnboardingStep::LocalRuntime => OnboardingStep::Identity,
            OnboardingStep::CloudProviders => OnboardingStep::LocalRuntime,
            OnboardingStep::CredentialEntry => OnboardingStep::CloudProviders,
            OnboardingStep::VoiceConfig => OnboardingStep::CredentialEntry,
            OnboardingStep::SubscriptionTiers => OnboardingStep::VoiceConfig,
            OnboardingStep::Summary => OnboardingStep::SubscriptionTiers,
        };
        current_step.set(prev);
    };

    rsx! {
        div { class: "modal-overlay",
            div { class: "machined-card modal-container",
                div { class: "machined-card-inner",
                    // Header Bar & 7-Step Navigation Indicator
                    div { class: "modal-header",
                        div { style: "display: flex; gap: 8px; align-items: center;",
                            for s in 1..=7 {
                                {
                                    let is_active = s == step_num;
                                    let is_done = s < step_num;
                                    let pill_bg = if is_active {
                                        "var(--osmoo-ivory)"
                                    } else if is_done {
                                        "var(--osmoo-metallic)"
                                    } else {
                                        "rgba(244, 240, 232, 0.1)"
                                    };
                                    let text_color = if is_active || is_done {
                                        "var(--osmoo-onyx)"
                                    } else {
                                        "var(--text-muted)"
                                    };
                                    rsx! {
                                        div {
                                            style: "width: 24px; height: 24px; border-radius: 9999px; background: {pill_bg}; color: {text_color}; display: flex; align-items: center; justify-content: center; font-size: 11px; font-weight: 700; font-family: var(--font-mono);",
                                            if is_done {
                                                IconCheck { size: 12 }
                                            } else {
                                                span { "{s}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        span { style: "font-family: var(--font-mono); font-size: 11px; letter-spacing: 0.12em; color: var(--osmoo-metallic);",
                            "STEP {step_num} OF 7 // INITIAL PROVISIONING"
                        }
                    }

                    // Body
                    div { class: "modal-body",
                        // Step 1: Agent Identity & Form
                        if *current_step.read() == OnboardingStep::Identity {
                            div {
                                h2 { style: "font-size: 20px; font-weight: 700; margin-bottom: 8px;", "Agent Identity & Visual Form" }
                                p { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 24px;",
                                    "Define the companion persona and choose between the Obsidian Robot or Luminous Particle Orb form."
                                }

                                div { style: "margin-bottom: 20px;",
                                    label { class: "form-label", "Companion Designation" }
                                    input {
                                        class: "hud-input-field",
                                        r#type: "text",
                                        value: "{name_input.read()}",
                                        oninput: move |e: FormEvent| name_input.set(e.value()),
                                    }
                                    span { class: "form-help", "This designation identifies the operator in workspace notifications." }
                                }

                                div {
                                    label { class: "form-label", "Visual Archetype Form" }
                                    div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px;",
                                        div {
                                            class: if *selected_mode.read() == CompanionMode::Robot { "machined-card" } else { "" },
                                            style: "padding: 16px; border: 1px solid var(--border-machined); border-radius: 14px; cursor: pointer; background: rgba(14, 16, 23, 0.6);",
                                            onclick: move |_| selected_mode.set(CompanionMode::Robot),
                                            div { style: "display: flex; align-items: center; gap: 10px; margin-bottom: 8px;",
                                                IconRobot { size: 20 }
                                                span { style: "font-weight: 700; font-size: 14px;", "Articulated Obsidian Bot" }
                                            }
                                            p { style: "font-size: 12px; color: var(--text-secondary);",
                                                "Floating spherical head with glowing pill eye apertures and teardrop torso pod."
                                            }
                                        }
                                        div {
                                            class: if *selected_mode.read() == CompanionMode::ParticleOrb { "machined-card" } else { "" },
                                            style: "padding: 16px; border: 1px solid var(--border-machined); border-radius: 14px; cursor: pointer; background: rgba(14, 16, 23, 0.6);",
                                            onclick: move |_| selected_mode.set(CompanionMode::ParticleOrb),
                                            div { style: "display: flex; align-items: center; gap: 10px; margin-bottom: 8px;",
                                                IconSparkles { size: 20 }
                                                span { style: "font-weight: 700; font-size: 14px;", "Luminous Particle Orb" }
                                            }
                                            p { style: "font-size: 12px; color: var(--text-secondary);",
                                                "14,000 starlight particle sphere with volumetric curl-noise surface perturbations."
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Step 2: Local AI Runtime Selection
                        if *current_step.read() == OnboardingStep::LocalRuntime {
                            div {
                                h2 { style: "font-size: 20px; font-weight: 700; margin-bottom: 8px;", "Local AI Runtime Discovery" }
                                p { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 20px;",
                                    "Zero-cloud inference engine for offline intelligence and complete data privacy."
                                }

                                if ollama_models.read().is_empty() {
                                    div { style: "background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.3); border-radius: 12px; padding: 20px; text-align: center; margin-bottom: 20px;",
                                        div { style: "font-family: var(--font-mono); font-weight: 700; color: #fca5a5; font-size: 14px; letter-spacing: 0.1em;", "NO LOCAL AVAILABLE" }
                                        p { style: "font-size: 12px; color: var(--text-secondary); margin-top: 6px;",
                                            "No local Ollama or LM Studio models were discovered on standard localhost endpoints."
                                        }
                                    }
                                } else {
                                    div { style: "margin-bottom: 16px;",
                                        label { class: "form-label", "Discovered Local Models" }
                                        for model in ollama_models.read().iter() {
                                            {
                                                let is_sel = *selected_model.read() == *model;
                                                let m_clone = model.clone();
                                                let card_bg = if is_sel { "rgba(244, 240, 232, 0.08)" } else { "transparent" };
                                                rsx! {
                                                    div {
                                                        style: "display: flex; justify-content: space-between; align-items: center; padding: 12px 16px; border: 1px solid var(--border-machined); border-radius: 10px; margin-bottom: 8px; cursor: pointer; background: {card_bg};",
                                                        onclick: move |_| selected_model.set(m_clone.clone()),
                                                        span { style: "font-family: var(--font-mono); font-size: 13px;", "{model}" }
                                                        if is_sel {
                                                            IconCheck { size: 16 }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Step 3: Cloud AI Provider Selection
                        if *current_step.read() == OnboardingStep::CloudProviders {
                            div {
                                h2 { style: "font-size: 20px; font-weight: 700; margin-bottom: 8px;", "Cloud Provider Catalogue" }
                                p { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 20px;",
                                    "Catalogue of external high-parameter reasoning engines. Selecting a provider sets the operational route."
                                }

                                div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 12px;",
                                    for p in PROVIDER_CATALOG.iter() {
                                        {
                                            let is_sel = *selected_cloud_provider.read() == p.id;
                                            let p_id = p.id.to_string();
                                            let card_bg = if is_sel { "rgba(244, 240, 232, 0.08)" } else { "rgba(14, 16, 23, 0.5)" };
                                            rsx! {
                                                div {
                                                    style: "padding: 14px; border: 1px solid var(--border-machined); border-radius: 12px; cursor: pointer; background: {card_bg};",
                                                    onclick: move |_| selected_cloud_provider.set(p_id.clone()),
                                                    div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;",
                                                        span { style: "font-weight: 700; font-size: 13px;", "{p.name}" }
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

                        // Step 4: Credential Entry (Prototype Only)
                        if *current_step.read() == OnboardingStep::CredentialEntry {
                            div {
                                h2 { style: "font-size: 20px; font-weight: 700; margin-bottom: 8px;", "Credential Architecture (Prototype)" }
                                div { style: "background: rgba(185, 169, 149, 0.1); border: 1px solid rgba(185, 169, 149, 0.25); border-radius: 10px; padding: 14px; margin-bottom: 20px;",
                                    p { style: "font-size: 12px; color: var(--osmoo-soft-stone); line-height: 1.5;",
                                        "PROTOTYPE NOTICE: In accordance with frontend boundary constraints, keys entered here are evaluated solely in memory for UI presentation and are NEVER stored to disk or transmitted."
                                    }
                                }

                                div { style: "margin-bottom: 20px;",
                                    label { class: "form-label", "API Key / Access Token" }
                                    input {
                                        class: "hud-input-field",
                                        r#type: if *show_key_masked.read() { "password" } else { "text" },
                                        placeholder: "sk-...",
                                        value: "{prototype_api_key.read()}",
                                        oninput: move |e: FormEvent| prototype_api_key.set(e.value()),
                                    }
                                    div { style: "display: flex; justify-content: space-between; margin-top: 8px;",
                                        button {
                                            class: "hud-btn",
                                            style: "padding: 4px 10px; font-size: 11px;",
                                            onclick: move |_| {
                                                let cur = *show_key_masked.read();
                                                show_key_masked.set(!cur);
                                            },
                                            if *show_key_masked.read() { "Reveal Key" } else { "Mask Key" }
                                        }
                                        span { class: "form-help", "Zero-storage security enforced" }
                                    }
                                }
                            }
                        }

                        // Step 5: Voice Configuration
                        if *current_step.read() == OnboardingStep::VoiceConfig {
                            div {
                                h2 { style: "font-size: 20px; font-weight: 700; margin-bottom: 8px;", "Voice & Audio Synthesizer" }
                                p { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 20px;",
                                    "Configure speech recognition and audio synthesis models."
                                }

                                div { style: "margin-bottom: 16px;",
                                    label { class: "form-label", "Audio Synthesis Pipeline" }
                                    for v in VOICE_CATALOG.iter() {
                                        {
                                            let is_sel = *selected_tts.read() == v.id;
                                            let v_id = v.id.to_string();
                                            let card_bg = if is_sel { "rgba(244, 240, 232, 0.08)" } else { "transparent" };
                                            rsx! {
                                                div {
                                                    style: "display: flex; justify-content: space-between; align-items: center; padding: 12px 16px; border: 1px solid var(--border-machined); border-radius: 10px; margin-bottom: 8px; cursor: pointer; background: {card_bg};",
                                                    onclick: move |_| selected_tts.set(v_id.clone()),
                                                    div {
                                                        div { style: "font-weight: 600; font-size: 13px;", "{v.name}" }
                                                        div { style: "font-size: 10px; font-family: var(--font-mono); color: var(--text-muted);", "{v.provider} // {v.modality}" }
                                                    }
                                                    if is_sel { IconCheck { size: 16 } }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Step 6: Subscription Presentation
                        if *current_step.read() == OnboardingStep::SubscriptionTiers {
                            div {
                                h2 { style: "font-size: 20px; font-weight: 700; margin-bottom: 8px;", "Subscription Tiers" }
                                p { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 20px;",
                                    "Commercial entitlement tiers for Osmiora companion capabilities."
                                }

                                div { style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 12px;",
                                    for plan in SUBSCRIPTION_PLANS.iter() {
                                        {
                                            let is_sel = *selected_tier.read() == plan.id;
                                            let p_id = plan.id.to_string();
                                            let card_bg = if is_sel { "rgba(244, 240, 232, 0.09)" } else { "rgba(14, 16, 23, 0.6)" };
                                            rsx! {
                                                div {
                                                    style: "padding: 16px; border: 1px solid var(--border-machined); border-radius: 14px; cursor: pointer; background: {card_bg}; display: flex; flex-direction: column; justify-content: space-between;",
                                                    onclick: move |_| selected_tier.set(p_id.clone()),
                                                    div {
                                                        div { style: "font-size: 11px; font-family: var(--font-mono); color: var(--osmoo-metallic); margin-bottom: 4px;", "{plan.badge}" }
                                                        div { style: "font-size: 16px; font-weight: 700; margin-bottom: 4px;", "{plan.name}" }
                                                        div { style: "font-size: 20px; font-weight: 700; color: var(--osmoo-ivory); margin-bottom: 12px;",
                                                            "{plan.price} "
                                                            span { style: "font-size: 11px; color: var(--text-muted); font-weight: 400;", "{plan.cadence}" }
                                                        }
                                                    }
                                                    if is_sel {
                                                        div { style: "display: flex; align-items: center; gap: 4px; color: var(--osmoo-ivory); font-size: 12px; font-weight: 700;",
                                                            IconCheck { size: 14 }
                                                            span { "Selected" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Step 7: Setup Summary
                        if *current_step.read() == OnboardingStep::Summary {
                            div {
                                h2 { style: "font-size: 20px; font-weight: 700; margin-bottom: 8px;", "Operational Provisioning Summary" }
                                p { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 20px;",
                                    "Review configured parameters before activating the OSMOO Operating HUD."
                                }

                                div { style: "background: rgba(14, 16, 23, 0.6); border: 1px solid var(--border-machined); border-radius: 12px; padding: 20px; display: flex; flex-direction: column; gap: 12px;",
                                    div { style: "display: flex; justify-content: space-between;",
                                        span { style: "color: var(--text-muted); font-size: 12px;", "Companion Designation:" }
                                        span { style: "font-weight: 600; font-size: 13px;", "{name_input.read()}" }
                                    }
                                    div { style: "display: flex; justify-content: space-between;",
                                        span { style: "color: var(--text-muted); font-size: 12px;", "Visual Form:" }
                                        span { style: "font-weight: 600; font-size: 13px;",
                                            if *selected_mode.read() == CompanionMode::Robot { "Articulated Obsidian Bot" } else { "Luminous Particle Orb" }
                                        }
                                    }
                                    div { style: "display: flex; justify-content: space-between;",
                                        span { style: "color: var(--text-muted); font-size: 12px;", "Selected Provider:" }
                                        span { style: "font-family: var(--font-mono); font-size: 12px; color: var(--osmoo-metallic);", "{selected_cloud_provider.read()}" }
                                    }
                                    div { style: "display: flex; justify-content: space-between;",
                                        span { style: "color: var(--text-muted); font-size: 12px;", "Subscription Entitlement:" }
                                        span { style: "font-weight: 700; font-size: 12px; text-transform: uppercase;", "{selected_tier.read()}" }
                                    }
                                }
                            }
                        }
                    }

                    // Footer Navigation Actions
                    div { class: "modal-footer",
                        if step_num > 1 {
                            button {
                                class: "hud-btn",
                                onclick: prev_step,
                                "Previous"
                            }
                        } else {
                            div {}
                        }

                        button {
                            class: "hud-btn primary",
                            onclick: next_step,
                            if step_num == 7 { "Activate Companion" } else { "Continue" }
                            IconChevronRight { size: 14 }
                        }
                    }
                }
            }
        }
    }
}
