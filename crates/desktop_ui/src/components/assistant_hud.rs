use dioxus::prelude::*;
use crate::components::icons::{
    IconAlertCircle, IconCheck, IconLayers, IconMessageSquare, IconMic, IconMicOff,
    IconRefresh, IconSend, IconSettings, IconShield, IconStopCircle,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ChatTurn {
    pub id: u64,
    pub sender: String,
    pub text: String,
    pub timestamp: String,
    pub is_user: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TelemetryStats {
    pub mic_rms: f32,
    pub daemon_connected: bool,
    pub audio_latency_ms: u32,
    pub active_llm: String,
    pub active_stt: String,
    pub active_tts: String,
}

#[component]
pub fn AssistantHud(
    on_toggle_voice: EventHandler<()>,
    on_send_prompt: EventHandler<String>,
    on_open_settings: EventHandler<()>,
    on_emergency_stop: EventHandler<()>,
    mic_active: bool,
    telemetry: TelemetryStats,
    chat_history: Vec<ChatTurn>,
) -> Element {
    let mut drawer_open = use_signal(|| false);
    let mut prompt_input = use_signal(String::new);

    let mic_color = if mic_active { "#10b981" } else { "#64748b" };
    let daemon_color = if telemetry.daemon_connected { "#10b981" } else { "#f59e0b" };

    rsx! {
        div { class: "hud-root-layer",
            // ── Top Floating Header Bar ─────────────────────────────────
            div { class: "hud-top-bar",
                div { class: "hud-brand-pill",
                    span { class: "hud-brand-dot" }
                    span { class: "hud-brand-name", "OSMOO" }
                    span { class: "hud-brand-sub", "OSMIORA COMPUTATIONAL SYSTEMS" }
                }

                div { class: "hud-telemetry-cluster",
                    // Daemon IPC Status Pill
                    div { class: "telemetry-pill",
                        span {
                            class: "telemetry-dot",
                            style: "background-color: {daemon_color};",
                        }
                        span { class: "telemetry-text",
                            if telemetry.daemon_connected { "PIPE: CONNECTED" } else { "PIPE: SEARCHING" }
                        }
                    }

                    // Audio Stream Status Pill
                    div { class: "telemetry-pill",
                        span {
                            class: "telemetry-dot",
                            style: "background-color: {mic_color};",
                        }
                        span { class: "telemetry-text",
                            if mic_active { "MIC: 48kHz LIVE" } else { "MIC: STANDBY" }
                        }
                    }

                    // Active Provider Pill
                    div { class: "telemetry-pill",
                        span { class: "telemetry-text", "{telemetry.active_llm} // {telemetry.active_stt}" }
                    }
                }

                div { class: "hud-controls-cluster",
                    // Mic Privacy Toggle Button
                    button {
                        class: if mic_active { "hud-icon-btn active" } else { "hud-icon-btn" },
                        title: if mic_active { "Mute Microphone" } else { "Unmute Microphone" },
                        onclick: move |_| on_toggle_voice.call(()),
                        if mic_active {
                            IconMic { size: 16 }
                        } else {
                            IconMicOff { size: 16 }
                        }
                    }

                    // Drawer Toggle Button
                    button {
                        class: if *drawer_open.read() { "hud-icon-btn active" } else { "hud-icon-btn" },
                        title: "Toggle Conversation History",
                        onclick: move |_| {
                            let cur = *drawer_open.read();
                            drawer_open.set(!cur);
                        },
                        IconMessageSquare { size: 16 }
                    }

                    // Emergency Stop Button
                    button {
                        class: "hud-icon-btn emergency",
                        title: "Emergency Stop (Abort Autonomous Tools)",
                        onclick: move |_| on_emergency_stop.call(()),
                        IconStopCircle { size: 16 }
                    }

                    // Settings Button
                    button {
                        class: "hud-icon-btn",
                        title: "System Settings",
                        onclick: move |_| on_open_settings.call(()),
                        IconSettings { size: 16 }
                    }
                }
            }

            // ── Slide-in Conversation Drawer (Secondary Interface) ───────
            if *drawer_open.read() {
                div {
                    class: "hud-drawer-backdrop",
                    onclick: move |_| drawer_open.set(false),
                }
            }

            div {
                class: if *drawer_open.read() { "hud-history-drawer open" } else { "hud-history-drawer" },
                div { class: "drawer-header",
                    span { class: "drawer-title", "CONVERSATION & EVENT LOG" }
                    button {
                        class: "drawer-close-btn",
                        onclick: move |_| drawer_open.set(false),
                        "×"
                    }
                }

                div { class: "drawer-turns-scroll",
                    if chat_history.is_empty() {
                        div { class: "drawer-empty", "No messages in session buffer." }
                    } else {
                        for turn in chat_history.iter() {
                            div {
                                class: if turn.is_user { "chat-turn user" } else { "chat-turn assistant" },
                                div { class: "turn-meta",
                                    span { class: "turn-sender", "{turn.sender}" }
                                    span { class: "turn-time", "{turn.timestamp}" }
                                }
                                div { class: "turn-body", "{turn.text}" }
                            }
                        }
                    }
                }

                // Interactive Text Input Inside Drawer
                div { class: "drawer-input-row",
                    input {
                        class: "hud-input",
                        r#type: "text",
                        placeholder: "Type a prompt or system command...",
                        value: "{prompt_input.read()}",
                        oninput: move |e: FormEvent| prompt_input.set(e.value()),
                        onkeydown: {
                            let mut prompt_sig = prompt_input;
                            let on_send = on_send_prompt;
                            move |e: KeyboardEvent| {
                                if e.key() == Key::Enter {
                                    let txt = prompt_sig.read().trim().to_string();
                                    if !txt.is_empty() {
                                        prompt_sig.set(String::new());
                                        on_send.call(txt);
                                    }
                                }
                            }
                        },
                    }
                    button {
                        class: "hud-btn primary",
                        onclick: {
                            let mut prompt_sig = prompt_input;
                            let on_send = on_send_prompt;
                            move |_| {
                                let txt = prompt_sig.read().trim().to_string();
                                if !txt.is_empty() {
                                    prompt_sig.set(String::new());
                                    on_send.call(txt);
                                }
                            }
                        },
                        IconSend { size: 16 }
                    }
                }
            }

            // ── Floating Action Bar at Bottom of HUD ─────────────────────
            div { class: "hud-bottom-actions",
                div { class: "action-hint",
                    span { class: "kbd-key", "SPACE" }
                    span { class: "hint-text", "HOLD TO TALK" }
                }
                div { class: "action-divider", "·" }
                div { class: "action-hint",
                    span { class: "kbd-key", "ALT + O" }
                    span { class: "hint-text", "TOGGLE OVERLAY" }
                }
                div { class: "action-divider", "·" }
                div { class: "action-hint",
                    span { class: "kbd-key", "ESC" }
                    span { class: "hint-text", "HIDE TO TRAY" }
                }
            }
        }
    }
}
