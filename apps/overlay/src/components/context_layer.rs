use crate::types::VisualState;
use dioxus::prelude::*;

#[component]
pub fn ContextTopBar(
    identity_name: String,
    turn_count: usize,
    sensor_gesture: String,
    drawer_open: bool,
    on_toggle_drawer: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        div { class: "top-nav-bar clickable",
            // Brand & System Identity
            div { class: "nav-brand",
                div { id: "connection-status-dot", class: "brand-dot" }
                div { class: "brand-label", "{identity_name}" }
                div {
                    style: "font-family: var(--font-mono); font-size: 10px; color: var(--text-muted); padding-left: 8px;",
                    "AI OPERATING COMPANION"
                }
            }

            // Real System Metrics & Sensor Controls
            div { class: "nav-metrics",
                // Camera / Gesture Tracking Indicator
                div {
                    class: "metric-chip",
                    title: "Hardware Camera Gesture Tracker",
                    span { style: "color: var(--text-muted);", "SENSOR:" }
                    span { style: "color: var(--text-primary); font-weight: 600;", "{sensor_gesture}" }
                }

                // Privacy Microphone Toggle
                button {
                    id: "mic-privacy-toggle",
                    class: "sensor-toggle active clickable",
                    onclick: move |_| {
                        let _ = document::eval("window.voxyToggleMic();");
                    },
                    title: "Toggle Hardware Microphone Mute",
                    span { "MIC: ON" }
                }

                // Turn Counter
                div { class: "metric-chip",
                    span { style: "color: var(--text-muted);", "TURN" }
                    span { style: "color: var(--accent-core); font-weight: 600;", "#{turn_count}" }
                }

                // Depth Drawer Toggle Button
                button {
                    class: "drawer-toggle-btn clickable",
                    onclick: move |e| on_toggle_drawer.call(e),
                    if drawer_open {
                        "CLOSE INSPECTOR [X]"
                    } else {
                        "DEPTH INSPECTOR [D]"
                    }
                }
            }
        }
    }
}

#[component]
pub fn ContextBottomBar(visual_state: VisualState, transcript: String) -> Element {
    let tag_label = match visual_state {
        VisualState::Listening => "USER SPEECH //",
        VisualState::Speaking => "VOXY SPEECH //",
        VisualState::Thinking | VisualState::Processing => "REASONING //",
        VisualState::Executing => "AUTOMATION //",
        VisualState::Error => "ALERT //",
        _ => "STANDBY //",
    };

    rsx! {
        div { class: "bottom-context-bar clickable",
            div { class: "live-transcript-strip",
                div { class: "transcript-tag", "{tag_label}" }
                div { id: "live-transcript-text", class: "transcript-text", "{transcript}" }
            }
        }
    }
}
