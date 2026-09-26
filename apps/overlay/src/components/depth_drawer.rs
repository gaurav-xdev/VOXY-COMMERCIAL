use dioxus::prelude::*;
use crate::types::{ChatMessage, DesktopMode, SystemTelemetry, ToolStep};
use crate::components::computer_control::ToolExecutionTimeline;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawerTab {
    Conversation,
    Automation,
    Telemetry,
    Settings,
}

#[component]
pub fn DepthDrawer(
    is_open: bool,
    active_tab: Signal<DrawerTab>,
    messages: Vec<ChatMessage>,
    tool_steps: Vec<ToolStep>,
    telemetry: SystemTelemetry,
    current_mode: Signal<DesktopMode>,
    on_close: EventHandler<MouseEvent>,
) -> Element {
    let drawer_class = if is_open { "depth-drawer open clickable" } else { "depth-drawer clickable" };

    rsx! {
        div { class: "{drawer_class}",
            // Drawer Header
            div { class: "drawer-header",
                div { style: "display: flex; align-items: center; gap: 8px;",
                    div { style: "width: 6px; height: 6px; border-radius: 50%; background: var(--accent-core);" }
                    span { style: "font-family: var(--font-mono); font-size: 11px; font-weight: 700; letter-spacing: 1px;",
                        "VOXY DEPTH INSPECTOR"
                    }
                }
                button {
                    class: "drawer-toggle-btn",
                    onclick: move |e| on_close.call(e),
                    "CLOSE"
                }
            }

            // Navigation Tabs
            div { class: "drawer-tabs",
                button {
                    class: if *active_tab.read() == DrawerTab::Conversation { "drawer-tab active" } else { "drawer-tab" },
                    onclick: move |_| active_tab.set(DrawerTab::Conversation),
                    "CONVERSATION"
                }
                button {
                    class: if *active_tab.read() == DrawerTab::Automation { "drawer-tab active" } else { "drawer-tab" },
                    onclick: move |_| active_tab.set(DrawerTab::Automation),
                    "AUTOMATION"
                }
                button {
                    class: if *active_tab.read() == DrawerTab::Telemetry { "drawer-tab active" } else { "drawer-tab" },
                    onclick: move |_| active_tab.set(DrawerTab::Telemetry),
                    "TELEMETRY"
                }
                button {
                    class: if *active_tab.read() == DrawerTab::Settings { "drawer-tab active" } else { "drawer-tab" },
                    onclick: move |_| active_tab.set(DrawerTab::Settings),
                    "MODES & PREFS"
                }
            }

            // Drawer Content Body
            div { class: "drawer-body",
                match *active_tab.read() {
                    DrawerTab::Conversation => rsx! {
                        div { style: "display: flex; flex-direction: column; gap: 10px;",
                            if messages.is_empty() {
                                div { style: "color: var(--text-muted); font-size: 12px; text-align: center; margin-top: 40px;",
                                    "No speech dialogue recorded in this session."
                                }
                            }
                            for msg in messages {
                                {
                                    let bubble_class = if msg.is_user { "chat-bubble user" } else { "chat-bubble voxy" };
                                    rsx! {
                                        div { key: "{msg.id}", class: "{bubble_class}",
                                            div { style: "display: flex; justify-content: space-between; align-items: center;",
                                                span { class: "chat-sender", "{msg.sender}" }
                                                span { style: "font-family: var(--font-mono); font-size: 9px; color: var(--text-muted);", "{msg.timestamp}" }
                                            }
                                            div { "{msg.text}" }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    DrawerTab::Automation => rsx! {
                        div { style: "display: flex; flex-direction: column; gap: 12px;",
                            div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-secondary);",
                                "SYSTEM AUTOMATION & TOOL PIPELINE"
                            }
                            ToolExecutionTimeline { steps: tool_steps }
                        }
                    },
                    DrawerTab::Telemetry => rsx! {
                        div { style: "display: flex; flex-direction: column; gap: 12px;",
                            div { class: "telemetry-card",
                                div { style: "font-family: var(--font-mono); font-size: 10px; font-weight: 700; color: var(--accent-core); margin-bottom: 4px;",
                                    "HARDWARE AUDIO SUBSYSTEM"
                                }
                                div { class: "telemetry-item",
                                    span { class: "telemetry-key", "Driver:" }
                                    span { class: "telemetry-val", "{telemetry.audio_driver}" }
                                }
                                div { class: "telemetry-item",
                                    span { class: "telemetry-key", "Sampling:" }
                                    span { class: "telemetry-val", "{telemetry.sample_rate}" }
                                }
                                div { class: "telemetry-item",
                                    span { class: "telemetry-key", "Barge-in:" }
                                    span { class: "telemetry-val highlight", "{telemetry.barge_in_status}" }
                                }
                            }

                            div { class: "telemetry-card",
                                div { style: "font-family: var(--font-mono); font-size: 10px; font-weight: 700; color: var(--accent-amber); margin-bottom: 4px;",
                                    "CLOUD AI & VOICE PIPELINE"
                                }
                                div { class: "telemetry-item",
                                    span { class: "telemetry-key", "Language Model:" }
                                    span { class: "telemetry-val", "{telemetry.llm_model}" }
                                }
                                div { class: "telemetry-item",
                                    span { class: "telemetry-key", "STT Engine:" }
                                    span { class: "telemetry-val", "{telemetry.stt_provider} ({telemetry.stt_latency_ms:.0}ms)" }
                                }
                                div { class: "telemetry-item",
                                    span { class: "telemetry-key", "TTS Engine:" }
                                    span { class: "telemetry-val", "{telemetry.tts_provider} ({telemetry.tts_latency_ms:.0}ms)" }
                                }
                            }
                        }
                    },
                    DrawerTab::Settings => rsx! {
                        div { style: "display: flex; flex-direction: column; gap: 16px;",
                            div { style: "font-family: var(--font-mono); font-size: 11px; color: var(--text-secondary); font-weight: 600;",
                                "DESKTOP OPERATING MODE"
                            }
                            div { style: "display: flex; flex-direction: column; gap: 8px;",
                                for mode in [
                                    DesktopMode::FullExperience,
                                    DesktopMode::Compact,
                                    DesktopMode::Floating,
                                    DesktopMode::EdgeDock,
                                    DesktopMode::Minimal,
                                    DesktopMode::ComputerControl,
                                ] {
                                    {
                                        let is_sel = *current_mode.read() == mode;
                                        let btn_style = if is_sel {
                                            "padding: 10px 14px; border-radius: 8px; border: 1px solid var(--accent-core); background: rgba(56, 189, 248, 0.12); color: var(--text-primary); text-align: left; font-family: var(--font-mono); font-size: 11px; cursor: pointer;"
                                        } else {
                                            "padding: 10px 14px; border-radius: 8px; border: 1px solid var(--border-subtle); background: rgba(0, 0, 0, 0.2); color: var(--text-secondary); text-align: left; font-family: var(--font-mono); font-size: 11px; cursor: pointer;"
                                        };
                                        rsx! {
                                            button {
                                                style: "{btn_style}",
                                                onclick: move |_| current_mode.set(mode),
                                                div { style: "font-weight: 700;", "{mode.label()}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}
