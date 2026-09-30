use dioxus::prelude::*;

use crate::bridge::AppBridge;
use crate::components::voice_orb::{OrbState, VoiceOrb};

#[derive(Clone)]
struct Message {
    role: String,
    content: String,
}

#[component]
pub fn ChatView() -> Element {
    let bridge = use_context::<AppBridge>();
    let messages = use_signal(Vec::<Message>::new);
    let mut input_text = use_signal(String::new);
    let is_listening = use_signal(|| false);
    let is_speaking = use_signal(|| false);
    let is_thinking = use_signal(|| false);
    let error_msg = use_signal(|| Option::<String>::None);

    let orb_state = if *is_thinking.read() {
        OrbState::Thinking
    } else if *is_speaking.read() {
        OrbState::Speaking
    } else if *is_listening.read() {
        OrbState::Listening
    } else if error_msg.read().is_some() {
        OrbState::Error
    } else {
        OrbState::Idle
    };

    let process_message = {
        let mut messages = messages;
        let mut input_text = input_text;
        let mut is_thinking = is_thinking;
        let mut error_msg = error_msg;
        let cognition = bridge.cognition.clone();
        let voice = bridge.voice.clone();
        move |text: String| {
            if text.is_empty() {
                return;
            }

            messages.write().push(Message {
                role: "user".to_string(),
                content: text.clone(),
            });
            input_text.set(String::new());
            error_msg.set(None);
            is_thinking.set(true);

            let mut msgs = messages;
            let mut thinking = is_thinking;
            let mut speaking = is_speaking;
            let mut err = error_msg;
            let cog = cognition.clone();
            let v = voice.clone();

            spawn(async move {
                let intent_input = voxy_cognition::IntentInput {
                    raw_text: text,
                    context: None,
                    source: "desktop_ui".to_string(),
                    metadata: std::collections::HashMap::new(),
                };

                match cog.process(&intent_input).await {
                    Ok(result) => {
                        let response = serde_json::to_string_pretty(&result.result)
                            .unwrap_or_else(|_| format!("{:?}", result.result));

                        let confidence_pct = (result.confidence.value * 100.0) as u32;
                        let display = format!(
                            "{}\n\n[Confidence: {}% | Duration: {}ms]",
                            response, confidence_pct, result.duration_ms
                        );

                        msgs.write().push(Message {
                            role: "assistant".to_string(),
                            content: display,
                        });
                        thinking.set(false);
                        speaking.set(true);

                        let _ = v.speak(&response).await;

                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        speaking.set(false);
                    }
                    Err(e) => {
                        thinking.set(false);
                        err.set(Some(format!("Cognition error: {}", e)));
                        msgs.write().push(Message {
                            role: "assistant".to_string(),
                            content: format!("Error processing: {}", e),
                        });
                    }
                }
            });
        }
    };

    let send_message = {
        let mut process = process_message.clone();
        let input = input_text;
        move |_: Event<MouseData>| {
            let text = input.read().trim().to_string();
            process(text);
        }
    };

    let toggle_listening = {
        let mut is_listening = is_listening;
        let voice = bridge.voice.clone();
        move |_: Event<MouseData>| {
            let current = *is_listening.read();
            is_listening.set(!current);
            let v = voice.clone();
            spawn(async move {
                if !*is_listening.read() {
                    let _ = v.start_listening().await;
                } else {
                    v.stop_listening().await;
                }
            });
        }
    };

    let on_keydown = {
        let mut process = process_message.clone();
        let input = input_text;
        move |e: Event<KeyboardData>| {
            if e.key() == Key::Enter {
                let text = input.read().trim().to_string();
                process(text);
            }
        }
    };

    rsx! {
        div { class: "chat-container",
            div { class: "chat-messages",
                if messages.read().is_empty() {
                    div { class: "empty-state",
                        VoiceOrb { state: orb_state, on_click: None }
                        div { class: "empty-state-title", "How can I help you today?" }
                        div { class: "empty-state-desc",
                            "Type a message or click the orb to start a voice conversation."
                        }
                        if let Some(err) = error_msg.read().as_ref() {
                            div { style: "color: var(--error); margin-top: 12px; font-size: 12px;",
                                "{err}"
                            }
                        }
                    }
                } else {
                    for msg in messages.read().iter() {
                        div {
                            class: if msg.role == "user" { "message user" } else { "message assistant" },
                            div { class: "message-avatar",
                                if msg.role == "user" { "\u{1F464}" } else { "V" }
                            }
                            div { class: "message-bubble", "{msg.content}" }
                        }
                    }
                }
            }

            div { class: "chat-input-area",
                div { class: "voice-controls",
                    button {
                        class: if *is_listening.read() { "voice-btn active" } else { "voice-btn" },
                        onclick: toggle_listening,
                        if *is_listening.read() { "\u{23F9}" } else { "\u{1F3A4}" }
                    }
                }

                div { class: "chat-input-wrapper",
                    textarea {
                        class: "chat-input",
                        placeholder: "Type a message...",
                        value: "{input_text}",
                        oninput: move |e| input_text.set(e.value()),
                        onkeydown: on_keydown,
                        rows: "1",
                    }
                    button {
                        class: "send-btn",
                        onclick: send_message,
                        "\u{27A4}"
                    }
                }
            }
        }
    }
}
