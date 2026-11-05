use crate::components::assistant_core::CompanionMode;
use crate::components::icons::{
    IconLayers, IconMessageSquare, IconOsmooLogo, IconSend, IconSettings, IconStopCircle,
};
use dioxus::prelude::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatTurn {
    pub id: u64,
    pub sender: String,
    pub text: String,
    pub timestamp: String,
    pub is_user: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TelemetryStats {
    pub daemon_connected: bool,
    pub active_llm: String,
}

#[component]
pub fn AssistantHud(
    on_send_prompt: EventHandler<String>,
    on_open_settings: EventHandler<()>,
    on_emergency_stop: EventHandler<()>,
    on_toggle_mode: EventHandler<CompanionMode>,
    #[props(default)]
    on_toggle_overlay: Option<EventHandler<()>>,
    telemetry: TelemetryStats,
    chat_history: Vec<ChatTurn>,
    current_mode: CompanionMode,
) -> Element {
    let mut drawer_open = use_signal(|| false);
    let mut prompt_input = use_signal(String::new);

    let daemon_color = if telemetry.daemon_connected {
        "var(--status-listening)"
    } else {
        "var(--osmoo-metallic)"
    };

    rsx! {
        div { class: "hud-root-layer",
            // ── Top Floating Navigation & Telemetry Bar ─────────────────
            div { class: "hud-top-bar",
                // Brand Monogram & Title
                div { class: "hud-brand-pill",
                    IconOsmooLogo { size: 22, class: Some("brand-helical-mark".to_string()) }
                    span { class: "hud-brand-name", "OSMOO" }
                    span { class: "hud-brand-sub", "COMPUTATIONAL COMPANION" }
                }

                // Telemetry Cluster
                div { class: "hud-telemetry-cluster",
                    div { class: "telemetry-pill",
                        span {
                            class: "telemetry-dot",
                            style: "background-color: {daemon_color};",
                        }
                        span { class: "telemetry-text",
                            if telemetry.daemon_connected { "PIPE: ONLINE" } else { "PIPE: SEARCHING" }
                        }
                    }

                    div { class: "telemetry-pill",
                        span { class: "telemetry-text", "{telemetry.active_llm}" }
                    }
                }

                // Top Controls
                div { class: "hud-controls-cluster",
                    if let Some(on_overlay) = on_toggle_overlay {
                        button {
                            class: "hud-icon-btn",
                            title: "Launch / Bring Forward Desktop Overlay Mode",
                            onclick: move |_| on_overlay.call(()),
                            IconLayers { size: 16 }
                        }
                    }

                    button {
                        class: if *drawer_open.read() { "hud-icon-btn active" } else { "hud-icon-btn" },
                        title: "Conversation Drawer",
                        onclick: move |_| {
                            let cur = *drawer_open.read();
                            drawer_open.set(!cur);
                        },
                        IconMessageSquare { size: 16 }
                    }

                    button {
                        class: "hud-icon-btn emergency",
                        title: "Emergency Halt (Abort Running Subsystems)",
                        onclick: move |_| on_emergency_stop.call(()),
                        IconStopCircle { size: 16 }
                    }

                    button {
                        class: "hud-icon-btn",
                        title: "System Preferences",
                        onclick: move |_| on_open_settings.call(()),
                        IconSettings { size: 16 }
                    }
                }
            }

            // ── Bottom Dock: Mode Segmented Switch + Input Capsule ──────
            div { class: "hud-bottom-dock",
                // Companion Desktop Status Pill
                div {
                    class: "telemetry-pill",
                    style: "cursor: pointer;",
                    title: "Click to bring forward the Desktop Cat Overlay",
                    onclick: move |_| {
                        if let Some(on_overlay) = on_toggle_overlay {
                            on_overlay.call(());
                        }
                    },
                    span {
                        class: "telemetry-dot",
                        style: "background-color: var(--status-listening); box-shadow: 0 0 6px var(--status-listening);",
                    }
                    span { class: "telemetry-text", "🐱 CAT OVERLAY ACTIVE" }
                }

                // Prompt Input Capsule
                div { class: "hud-input-capsule",
                    input {
                        class: "hud-text-input",
                        r#type: "text",
                        placeholder: "Direct OSMOO Companion or execute OS automation...",
                        value: "{prompt_input.read()}",
                        oninput: move |e: FormEvent| prompt_input.set(e.value()),
                        onkeydown: move |e: KeyboardEvent| {
                            if e.key() == Key::Enter {
                                let text = prompt_input.read().trim().to_string();
                                if !text.is_empty() {
                                    on_send_prompt.call(text);
                                    prompt_input.set(String::new());
                                }
                            }
                        },
                    }
                    button {
                        class: "hud-icon-btn",
                        style: "width: 32px; height: 32px; background: var(--osmoo-ivory); color: var(--osmoo-onyx);",
                        title: "Dispatch Command",
                        onclick: move |_| {
                            let text = prompt_input.read().trim().to_string();
                            if !text.is_empty() {
                                on_send_prompt.call(text);
                                prompt_input.set(String::new());
                            }
                        },
                        IconSend { size: 14 }
                    }
                }
            }

            // ── Slide-in Conversation Drawer ────────────────────────────
            if *drawer_open.read() {
                div {
                    class: "hud-drawer-backdrop",
                    onclick: move |_| drawer_open.set(false),
                }

                div { class: "hud-drawer-panel",
                    div { class: "drawer-header",
                        span { class: "drawer-title", "COGNITIVE DIALOGUE STREAM" }
                        button {
                            class: "hud-icon-btn",
                            style: "width: 28px; height: 28px; font-size: 16px;",
                            onclick: move |_| drawer_open.set(false),
                            "×"
                        }
                    }

                    div { class: "drawer-content",
                        for turn in chat_history.iter() {
                            {
                                let b_class = if turn.is_user { "chat-bubble user" } else { "chat-bubble assistant" };
                                rsx! {
                                    div { class: "{b_class}",
                                        div { class: "chat-sender-label", "{turn.sender} · {turn.timestamp}" }
                                        div { "{turn.text}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
