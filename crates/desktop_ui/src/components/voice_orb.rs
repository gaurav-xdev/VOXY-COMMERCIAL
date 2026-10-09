use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OrbState {
    Idle,
    Listening,
    Speaking,
    Thinking,
    Executing,
    WaitingForApproval,
    Error,
}

#[component]
pub fn VoiceOrb(state: Option<OrbState>, on_click: Option<EventHandler<()>>) -> Element {
    let orb_state = state.unwrap_or(OrbState::Idle);

    let (class, state_label, status_subtext) = match orb_state {
        OrbState::Idle => ("orb state-idle", "IDLE // READY", "Click orb or hold Space to talk"),
        OrbState::Listening => ("orb state-listening", "LISTENING", "Acoustic stream active (48kHz)"),
        OrbState::Speaking => ("orb state-speaking", "SYNTHESIZING SPEECH", "Streaming natural voice output"),
        OrbState::Thinking => ("orb state-thinking", "COGNITIVE PROCESSING", "Evaluating context & planning"),
        OrbState::Executing => ("orb state-executing", "EXECUTING TOOL", "Autonomous operation in progress"),
        OrbState::WaitingForApproval => ("orb state-waiting", "AWAITING APPROVAL", "Action requires user confirmation"),
        OrbState::Error => ("orb state-error", "ATTENTION REQUIRED", "Service error or emergency stopped"),
    };

    rsx! {
        div { class: "orb-volumetric-container",
            // Ambient outer radiance field
            div { class: "orb-radiance-field" }

            // Outer harmonic orbit ring
            div { class: "orb-orbit-ring" }

            // Secondary gyro counter-rotation ring
            div { class: "orb-gyro-ring" }

            // Core Interactive 3D Volumetric Orb
            div {
                class: "{class}",
                onclick: move |_| {
                    if let Some(handler) = &on_click {
                        handler.call(());
                    }
                },
                // Internal volumetric core layers
                div { class: "orb-inner-filament" }
                div { class: "orb-plasma-core" }
                div { class: "orb-specular-glint" }
            }

            // Real State and Telemetry Pill
            div { class: "orb-status-pill-container",
                div { class: "orb-state-pill",
                    span { class: "orb-state-dot" }
                    "{state_label}"
                }
                div { class: "orb-state-subtext",
                    "{status_subtext}"
                }
            }
        }
    }
}
