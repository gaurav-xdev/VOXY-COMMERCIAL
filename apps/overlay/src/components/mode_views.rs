use dioxus::prelude::*;
use crate::types::VisualState;

#[component]
pub fn CompactModeView(
    visual_state: VisualState,
    transcript: String,
    on_switch_full: EventHandler<MouseEvent>,
) -> Element {
    let state_color = visual_state.color();
    let state_label = visual_state.label();

    rsx! {
        div { class: "compact-capsule clickable",
            // Mini core status indicator
            div {
                style: "width: 12px; height: 12px; border-radius: 50%; background: {state_color}; box-shadow: 0 0 10px {state_color};"
            }
            div { style: "font-family: var(--font-mono); font-size: 11px; font-weight: 700; color: var(--text-primary);",
                "VOXY // {state_label}"
            }
            div { style: "font-size: 12px; color: var(--text-secondary); max-width: 480px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                "{transcript}"
            }
            button {
                class: "drawer-toggle-btn",
                onclick: move |e| on_switch_full.call(e),
                "EXPAND"
            }
        }
    }
}

#[component]
pub fn EdgeDockView(
    visual_state: VisualState,
    transcript: String,
    on_switch_full: EventHandler<MouseEvent>,
) -> Element {
    let state_color = visual_state.color();

    rsx! {
        div { class: "edge-dock clickable",
            div {
                style: "width: 10px; height: 10px; border-radius: 50%; background: {state_color}; box-shadow: 0 0 8px {state_color};"
            }
            div { style: "font-family: var(--font-mono); font-size: 11px; font-weight: 600; color: var(--text-primary);",
                "VOXY DOCK"
            }
            div { style: "font-size: 12px; color: var(--text-secondary); max-width: 420px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                "{transcript}"
            }
            button {
                class: "drawer-toggle-btn",
                onclick: move |e| on_switch_full.call(e),
                "EXPAND"
            }
        }
    }
}

#[component]
pub fn MinimalView(
    visual_state: VisualState,
    on_switch_full: EventHandler<MouseEvent>,
) -> Element {
    let state_color = visual_state.color();

    rsx! {
        div {
            class: "minimal-orb clickable",
            onclick: move |e| on_switch_full.call(e),
            title: "VOXY Active — Click to expand",
            div {
                style: "width: 14px; height: 14px; border-radius: 50%; background: {state_color}; box-shadow: 0 0 12px {state_color};"
            }
        }
    }
}
