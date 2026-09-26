use dioxus::prelude::*;
use crate::types::VisualState;

#[component]
pub fn CoreEntity(
    visual_state: VisualState,
    is_collapsed: bool,
    identity_name: String,
    profile_label: String,
    on_toggle: EventHandler<MouseEvent>,
) -> Element {
    let state_label = visual_state.label();
    let state_color = visual_state.color();
    let state_bg = visual_state.bg_tint();

    let root_class = match visual_state {
        VisualState::Idle => "presence-container clickable state-idle",
        VisualState::Listening => "presence-container clickable state-listening",
        VisualState::Processing => "presence-container clickable state-processing",
        VisualState::Thinking => "presence-container clickable state-thinking",
        VisualState::Speaking => "presence-container clickable state-speaking",
        VisualState::Executing => "presence-container clickable state-executing",
        VisualState::WaitingForUser => "presence-container clickable state-waiting",
        VisualState::Success => "presence-container clickable state-success",
        VisualState::Error => "presence-container clickable state-error",
        VisualState::Offline => "presence-container clickable state-offline",
        VisualState::Disabled => "presence-container clickable state-disabled",
    };

    rsx! {
        div {
            class: "{root_class}",
            onclick: move |e| on_toggle.call(e),
            title: if is_collapsed { "Click or clap to expand VOXY" } else { "Click to collapse" },

            // Ambient background halo with state tint
            div { class: "presence-halo" }

            // Clean, non-distracting SVG boundary ring
            svg {
                class: "presence-canvas",
                view_box: "0 0 380 380",
                circle {
                    cx: "190",
                    cy: "190",
                    r: "120",
                    fill: "none",
                    stroke: "rgba(255, 255, 255, 0.05)",
                    stroke_width: "1.0",
                }
                circle {
                    cx: "190",
                    cy: "190",
                    r: "78",
                    fill: "none",
                    stroke: "rgba(255, 255, 255, 0.12)",
                    stroke_width: "1.5",
                }
            }

            // Real-time Audio Reactive Volumetric Canvas
            canvas {
                id: "presence-core-canvas",
                class: "presence-canvas",
                width: "380",
                height: "380",
            }

            // Core Identity and Dynamic State Badge
            div { class: "core-badge-container",
                div { class: "core-title", "{identity_name}" }
                if !is_collapsed {
                    div { class: "core-subtitle", "{profile_label}" }
                }
                div {
                    id: "core-badge",
                    class: "state-pill",
                    style: "color: {state_color}; background: {state_bg}; border-color: {state_color};",
                    "{state_label}"
                }
                if !is_collapsed {
                    div {
                        style: "font-family: var(--font-mono); font-size: 9px; color: var(--text-muted); margin-top: 6px; letter-spacing: 1px;",
                        "WASAPI LOW-LATENCY // 48kHz"
                    }
                }
            }
        }
    }
}
