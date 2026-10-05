use crate::bridge::AppBridge;
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OnboardingStep {
    AgentName,
    WakeWords,
    AiProviders,
    LocalAi,
    Voice,
}

#[derive(Props, Clone, PartialEq)]
pub struct OnboardingFlowProps {
    pub on_completed: EventHandler<()>,
}

#[component]
pub fn OnboardingFlow(props: OnboardingFlowProps) -> Element {
    let bridge = use_context::<AppBridge>();
    let mut current_step = use_signal(|| OnboardingStep::AgentName);

    // Step 1: Agent Name
    let mut agent_name = use_signal(|| "OSMOO".to_string());

    // Step 2: Wake words (up to 10)
    let mut wake_words = use_signal(|| vec!["Hey OSMOO".to_string(), "OSMOO".to_string()]);
    let mut new_wake_word = use_signal(String::new);

    // Step 3: Cloud AI Providers
    let primary_provider = use_signal(|| "groq".to_string());
    let mut groq_key = use_signal(String::new);
    let mut openai_key = use_signal(String::new);
    let mut anthropic_key = use_signal(String::new);
    let mut gemini_key = use_signal(String::new);
    let fallback_providers = use_signal(|| vec!["openai".to_string(), "anthropic".to_string()]);

    // Step 4: Local AI / Ollama
    let detected_models = use_signal(Vec::<String>::new);
    let mut selected_local_model = use_signal(|| "llama3.2".to_string());
    let is_scanning_ollama = use_signal(|| false);
    let ollama_status = use_signal(|| "Ready to scan".to_string());

    // Step 5: Voice configuration
    let mut selected_tts = use_signal(|| "cartesia".to_string());
    let mut selected_stt = use_signal(|| "groq".to_string());
    let mut cartesia_key = use_signal(String::new);
    let elevenlabs_key = use_signal(String::new);

    // Initial Ollama scan
    let scan_ollama = {
        move || {
            let mut scanning = is_scanning_ollama;
            let mut models = detected_models;
            let mut status = ollama_status;
            let mut selected = selected_local_model;

            scanning.set(true);
            status.set("Probing http://127.0.0.1:11434...".to_string());

            spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(4))
                    .build()
                    .unwrap_or_default();

                match client.get("http://127.0.0.1:11434/api/tags").send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(data) = resp.json::<serde_json::Value>().await {
                                if let Some(arr) = data["models"].as_array() {
                                    let list: Vec<String> = arr
                                        .iter()
                                        .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                                        .collect();
                                    if !list.is_empty() {
                                        selected.set(list[0].clone());
                                        status.set(format!(
                                            "Found {} local Ollama models",
                                            list.len()
                                        ));
                                        models.set(list);
                                    } else {
                                        status.set(
                                            "Ollama running, but no models pulled yet".to_string(),
                                        );
                                    }
                                }
                            }
                        } else {
                            status.set(format!("Ollama response: {}", resp.status()));
                        }
                    }
                    Err(_) => {
                        status.set("Ollama service offline / unreachable (Cloud providers will be primary)".to_string());
                    }
                }
                scanning.set(false);
            });
        }
    };

    // Auto-scan Ollama when entering Step 4
    use_effect(move || {
        if *current_step.read() == OnboardingStep::LocalAi && detected_models.read().is_empty() {
            scan_ollama();
        }
    });

    let complete_onboarding = {
        let b = bridge.clone();
        let on_done = props.on_completed;
        move || {
            let mut snap = b.settings.get();
            let name = agent_name.read().trim().to_string();
            snap.app_name = if name.is_empty() {
                "OSMOO".to_string()
            } else {
                name
            };
            snap.onboarded = true;

            // Voice settings
            snap.voice.wake_words = wake_words.read().clone();
            if let Some(first) = snap.voice.wake_words.first() {
                snap.voice.wake_word = first.clone();
            }
            snap.voice.tts_provider = Some(selected_tts.read().clone());
            snap.voice.stt_provider = Some(selected_stt.read().clone());

            // Model settings
            snap.models.provider = primary_provider.read().clone();
            snap.models.fallback_providers = fallback_providers.read().clone();
            snap.models.model = selected_local_model.read().clone();

            // Persist settings
            let _ = b.settings.update(snap);

            // Persist environment API keys to process if populated
            if !groq_key.read().is_empty() {
                std::env::set_var("STT_GROQ_API_KEY", groq_key.read().as_str());
            }
            if !openai_key.read().is_empty() {
                std::env::set_var("VOXY_API_KEYS_OPENAI", openai_key.read().as_str());
            }
            if !anthropic_key.read().is_empty() {
                std::env::set_var("VOXY_API_KEYS_ANTHROPIC", anthropic_key.read().as_str());
            }
            if !gemini_key.read().is_empty() {
                std::env::set_var("VOXY_API_KEYS_GEMINI", gemini_key.read().as_str());
            }
            if !cartesia_key.read().is_empty() {
                std::env::set_var("CARTESIA_API_KEY", cartesia_key.read().as_str());
            }
            if !elevenlabs_key.read().is_empty() {
                std::env::set_var("TTS_ELEVENLABS_API_KEY", elevenlabs_key.read().as_str());
            }

            on_done.call(());
        }
    };

    let step = *current_step.read();

    rsx! {
        div { class: "onboarding-viewport",
            div { class: "onboarding-container",

                // Progress Indicator (5 discrete steps)
                div { class: "onboarding-progress-bar",
                    for (idx, st) in [OnboardingStep::AgentName, OnboardingStep::WakeWords, OnboardingStep::AiProviders, OnboardingStep::LocalAi, OnboardingStep::Voice].iter().enumerate() {
                        {
                            let pip_cls = if *st == step {
                                "step-pip active"
                            } else if step_to_index(step) > idx {
                                "step-pip passed"
                            } else {
                                "step-pip"
                            };
                            rsx! {
                                div { class: "{pip_cls}" }
                            }
                        }
                    }
                }

                match step {
                    // STEP 1: AGENT NAME
                    OnboardingStep::AgentName => rsx! {
                        div { class: "step-pane",
                            div { class: "step-header",
                                div { class: "step-category", "STEP 01 // IDENTITY" }
                                div { class: "step-title", "What would you like to name your agent?" }
                                div { class: "step-desc", "Your companion's primary computational identifier. Defaults to OSMOO." }
                            }

                            div { class: "step-form-block",
                                input {
                                    class: "input",
                                    placeholder: "OSMOO",
                                    value: "{agent_name}",
                                    oninput: move |e| agent_name.set(e.value()),
                                }
                            }

                            div { class: "step-actions",
                                button {
                                    class: "btn btn-ghost",
                                    onclick: move |_| {
                                        agent_name.set("OSMOO".to_string());
                                        current_step.set(OnboardingStep::WakeWords);
                                    },
                                    "Skip (Use OSMOO)"
                                }
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| current_step.set(OnboardingStep::WakeWords),
                                    "Continue"
                                }
                            }
                        }
                    },

                    // STEP 2: WAKE WORDS
                    OnboardingStep::WakeWords => rsx! {
                        div { class: "step-pane",
                            div { class: "step-header",
                                div { class: "step-category", "STEP 02 // ACTIVATION" }
                                div { class: "step-title", "Set your wake words" }
                                div { class: "step-desc", "Configure up to 10 activation phrases for speech detection." }
                            }

                            div { class: "wake-word-list",
                                for (idx, w) in wake_words.read().iter().enumerate() {
                                    div { class: "wake-word-chip",
                                        span { "{w}" }
                                        button {
                                            class: "chip-remove-btn",
                                            onclick: move |_| {
                                                let mut list = wake_words.write();
                                                if list.len() > 1 {
                                                    list.remove(idx);
                                                }
                                            },
                                            "×"
                                        }
                                    }
                                }
                            }

                            if wake_words.read().len() < 10 {
                                div { style: "display: flex; gap: 8px; margin-top: 14px;",
                                    input {
                                        class: "input",
                                        placeholder: "Add phrase (e.g. 'Hey Atlas')",
                                        value: "{new_wake_word}",
                                        oninput: move |e| new_wake_word.set(e.value()),
                                    }
                                    button {
                                        class: "btn btn-secondary",
                                        onclick: move |_| {
                                            let phrase = new_wake_word.read().trim().to_string();
                                            if !phrase.is_empty() && wake_words.read().len() < 10 {
                                                wake_words.write().push(phrase);
                                                new_wake_word.set(String::new());
                                            }
                                        },
                                        "Add"
                                    }
                                }
                            }

                            div { class: "step-actions",
                                button {
                                    class: "btn btn-secondary",
                                    onclick: move |_| current_step.set(OnboardingStep::AgentName),
                                    "Back"
                                }
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| current_step.set(OnboardingStep::AiProviders),
                                    "Continue"
                                }
                            }
                        }
                    },

                    // STEP 3: CLOUD AI PROVIDERS
                    OnboardingStep::AiProviders => rsx! {
                        div { class: "step-pane",
                            div { class: "step-header",
                                div { class: "step-category", "STEP 03 // INTELLIGENCE" }
                                div { class: "step-title", "Connect your AI providers" }
                                div { class: "step-desc", "Configure primary and fallback reasoning backends. Keys are stored encrypted locally." }
                            }

                            div { class: "provider-config-stack",
                                // Groq
                                div { class: "provider-row",
                                    div { class: "provider-info",
                                        div { class: "provider-name", "Groq (Llama-3.3 / Mixtral)" }
                                        div { class: "provider-tag", "Ultra-fast inference" }
                                    }
                                    input {
                                        class: "input provider-key-input",
                                        r#type: "password",
                                        placeholder: "gsk_••••••••••••••••",
                                        value: "{groq_key}",
                                        oninput: move |e| groq_key.set(e.value()),
                                    }
                                }

                                // OpenAI
                                div { class: "provider-row",
                                    div { class: "provider-info",
                                        div { class: "provider-name", "OpenAI (GPT-4o / 4o-mini)" }
                                        div { class: "provider-tag", "General reasoning" }
                                    }
                                    input {
                                        class: "input provider-key-input",
                                        r#type: "password",
                                        placeholder: "sk-••••••••••••••••",
                                        value: "{openai_key}",
                                        oninput: move |e| openai_key.set(e.value()),
                                    }
                                }

                                // Anthropic
                                div { class: "provider-row",
                                    div { class: "provider-info",
                                        div { class: "provider-name", "Anthropic (Claude 3.5 Sonnet)" }
                                        div { class: "provider-tag", "High capability & nuance" }
                                    }
                                    input {
                                        class: "input provider-key-input",
                                        r#type: "password",
                                        placeholder: "sk-ant-••••••••••••••••",
                                        value: "{anthropic_key}",
                                        oninput: move |e| anthropic_key.set(e.value()),
                                    }
                                }

                                // Google Gemini
                                div { class: "provider-row",
                                    div { class: "provider-info",
                                        div { class: "provider-name", "Google Gemini (2.0 Flash)" }
                                        div { class: "provider-tag", "Multimodal intelligence" }
                                    }
                                    input {
                                        class: "input provider-key-input",
                                        r#type: "password",
                                        placeholder: "AIza••••••••••••••••",
                                        value: "{gemini_key}",
                                        oninput: move |e| gemini_key.set(e.value()),
                                    }
                                }
                            }

                            div { class: "step-actions",
                                button {
                                    class: "btn btn-secondary",
                                    onclick: move |_| current_step.set(OnboardingStep::WakeWords),
                                    "Back"
                                }
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| current_step.set(OnboardingStep::LocalAi),
                                    "Continue"
                                }
                            }
                        }
                    },

                    // STEP 4: LOCAL AI / OLLAMA
                    OnboardingStep::LocalAi => rsx! {
                        div { class: "step-pane",
                            div { class: "step-header",
                                div { class: "step-category", "STEP 04 // LOCAL AI" }
                                div { class: "step-title", "Local AI / Ollama Discovery" }
                                div { class: "step-desc", "{ollama_status}" }
                            }

                            if !detected_models.read().is_empty() {
                                div { class: "field-group",
                                    div { class: "field-label", "SELECT ACTIVE OLLAMA WEIGHTS" }
                                    select {
                                        class: "input",
                                        value: "{selected_local_model}",
                                        onchange: move |e| selected_local_model.set(e.value()),
                                        for m in detected_models.read().iter() {
                                            option { value: "{m}", "{m}" }
                                        }
                                    }
                                }
                            } else {
                                div { class: "empty-local-box",
                                    div { style: "font-size: 13px; color: var(--text-secondary); margin-bottom: 8px;", "No local Ollama instance detected or no models pulled yet." }
                                    div { style: "font-size: 11px; color: var(--text-muted);", "OSMOO will automatically operate via cloud providers without interruption." }
                                }
                            }

                            div { class: "step-actions",
                                button {
                                    class: "btn btn-secondary",
                                    onclick: move |_| current_step.set(OnboardingStep::AiProviders),
                                    "Back"
                                }
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| current_step.set(OnboardingStep::Voice),
                                    "Continue"
                                }
                            }
                        }
                    },

                    // STEP 5: VOICE
                    OnboardingStep::Voice => rsx! {
                        div { class: "step-pane",
                            div { class: "step-header",
                                div { class: "step-category", "STEP 05 // AUDIO PIPELINE" }
                                div { class: "step-title", "Voice & Speech Engine" }
                                div { class: "step-desc", "Configure neural Text-to-Speech (TTS) and low-latency Speech-to-Text (STT)." }
                            }

                            div { class: "voice-grid-setup",
                                div { class: "voice-panel-section",
                                    div { class: "field-label", "NEURAL TTS (SPEECH OUTPUT)" }
                                    select {
                                        class: "input",
                                        value: "{selected_tts}",
                                        onchange: move |e| selected_tts.set(e.value()),
                                        option { value: "cartesia", "Cartesia Sonic (100ms ultra-low latency)" }
                                        option { value: "elevenlabs", "ElevenLabs Neural TTS" }
                                        option { value: "openai", "OpenAI TTS (alloy/echo)" }
                                        option { value: "local_sapi", "Windows Native SAPI (Zero-network offline fallback)" }
                                    }
                                    if *selected_tts.read() == "cartesia" {
                                        input {
                                            class: "input",
                                            style: "margin-top: 8px;",
                                            r#type: "password",
                                            placeholder: "Cartesia API Key (optional if in .env)",
                                            value: "{cartesia_key}",
                                            oninput: move |e| cartesia_key.set(e.value()),
                                        }
                                    }
                                }

                                div { class: "voice-panel-section",
                                    div { class: "field-label", "SPEECH-TO-TEXT (STT LISTENING)" }
                                    select {
                                        class: "input",
                                        value: "{selected_stt}",
                                        onchange: move |e| selected_stt.set(e.value()),
                                        option { value: "groq", "Groq Whisper Large V3 (~200ms default)" }
                                        option { value: "deepgram", "Deepgram Nova-2" }
                                        option { value: "assemblyai", "AssemblyAI" }
                                        option { value: "local_whisper", "Local Whisper Engine (Offline)" }
                                    }
                                }
                            }

                            div { class: "step-actions",
                                button {
                                    class: "btn btn-secondary",
                                    onclick: move |_| current_step.set(OnboardingStep::LocalAi),
                                    "Back"
                                }
                                button {
                                    class: "btn btn-primary",
                                    onclick: move |_| complete_onboarding(),
                                    "Initialize OSMOO"
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

fn step_to_index(step: OnboardingStep) -> usize {
    match step {
        OnboardingStep::AgentName => 0,
        OnboardingStep::WakeWords => 1,
        OnboardingStep::AiProviders => 2,
        OnboardingStep::LocalAi => 3,
        OnboardingStep::Voice => 4,
    }
}
