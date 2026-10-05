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
    let daemon_connected = use_signal(|| false);

    // Listen to real Named Pipe IPC messages from Daemon
    {
        let mut messages = messages;
        let mut is_listening = is_listening;
        let mut is_speaking = is_speaking;
        let mut is_thinking = is_thinking;
        let mut error_msg = error_msg;
        let mut daemon_connected = daemon_connected;
        let ipc = bridge.ipc.clone();

        spawn(async move {
            let mut event_rx = ipc.subscribe();
            let mut conn_rx = ipc.watch_connected();

            loop {
                tokio::select! {
                    Ok(_changed) = conn_rx.changed() => {
                        let is_conn = *conn_rx.borrow();
                        daemon_connected.set(is_conn);
                    }
                    Ok(msg) = event_rx.recv() => {
                        match msg {
                            voxy_ipc::DaemonMessage::VoiceStateChanged { state, .. } => {
                                match state {
                                    voxy_ipc::VoiceState::Listening => {
                                        is_listening.set(true);
                                        is_speaking.set(false);
                                        is_thinking.set(false);
                                    }
                                    voxy_ipc::VoiceState::Thinking => {
                                        is_thinking.set(true);
                                        is_listening.set(false);
                                        is_speaking.set(false);
                                    }
                                    voxy_ipc::VoiceState::Speaking => {
                                        is_speaking.set(true);
                                        is_listening.set(false);
                                        is_thinking.set(false);
                                    }
                                    voxy_ipc::VoiceState::Error => {
                                        error_msg.set(Some("Daemon encountered voice error".to_string()));
                                        is_listening.set(false);
                                        is_thinking.set(false);
                                        is_speaking.set(false);
                                    }
                                    _ => {
                                        is_listening.set(false);
                                        is_speaking.set(false);
                                        is_thinking.set(false);
                                    }
                                }
                            }
                            voxy_ipc::DaemonMessage::TranscriptUpdate { text, is_final, .. } => {
                                if is_final && !text.is_empty() {
                                    messages.write().push(Message {
                                        role: "user".to_string(),
                                        content: text,
                                    });
                                }
                            }
                            voxy_ipc::DaemonMessage::ChatMessage { sender, text, .. } => {
                                messages.write().push(Message {
                                    role: sender,
                                    content: text,
                                });
                            }
                            voxy_ipc::DaemonMessage::ErrorNotification { code, message } => {
                                error_msg.set(Some(format!("{code}: {message}")));
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    }

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
        let ipc = bridge.ipc.clone();

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
            let ipc_client = ipc.clone();

            spawn(async move {
                // Forward input to daemon via IPC as well
                let _ = ipc_client
                    .send_command(voxy_ipc::ClientCommand::SendTextInput { text: text.clone() })
                    .await;

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
        let ipc = bridge.ipc.clone();
        move |_: Event<MouseData>| {
            let current = *is_listening.read();
            is_listening.set(!current);
            let v = voice.clone();
            let ipc_client = ipc.clone();
            spawn(async move {
                if !current {
                    let _ = v.start_listening().await;
                } else {
                    v.stop_listening().await;
                    let _ = ipc_client
                        .send_command(voxy_ipc::ClientCommand::InterruptSpeech)
                        .await;
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
                            "OSMOO is voice-first. Speak aloud or use push-to-talk."
                        }
                        div { style: "margin-top: 8px;",
                            span {
                                class: if *daemon_connected.read() { "badge badge-success" } else { "badge badge-warning" },
                                if *daemon_connected.read() { "Daemon: Connected" } else { "Daemon: Standalone / Connecting" }
                            }
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
                                if msg.role == "user" { "\u{1F464}" } else { "O" }
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
                        placeholder: "Voice-first active — or type intent here...",
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
