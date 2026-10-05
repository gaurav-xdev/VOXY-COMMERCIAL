use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoreState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Executing,
    Waiting,
    ApprovalRequired,
    Error,
}

#[derive(Props, Clone, PartialEq)]
pub struct OsmooCoreProps {
    #[props(default)]
    pub state: Option<CoreState>,
    #[props(default)]
    pub compact: bool,
    #[props(default)]
    pub on_click: EventHandler<()>,
}

#[component]
pub fn OsmooCore(props: OsmooCoreProps) -> Element {
    let current_state = props.state.unwrap_or(CoreState::Idle);

    let state_class = match current_state {
        CoreState::Idle => "core-state-idle",
        CoreState::Listening => "core-state-listening",
        CoreState::Thinking => "core-state-thinking",
        CoreState::Speaking => "core-state-speaking",
        CoreState::Executing => "core-state-executing",
        CoreState::Waiting => "core-state-waiting",
        CoreState::ApprovalRequired => "core-state-approval",
        CoreState::Error => "core-state-error",
    };

    let status_label = match current_state {
        CoreState::Idle => "OSMOO STANDING BY",
        CoreState::Listening => "LISTENING",
        CoreState::Thinking => "COMPUTING",
        CoreState::Speaking => "COMMUNICATING",
        CoreState::Executing => "EXECUTING SYSTEM ACTION",
        CoreState::Waiting => "AWAITING COMPLETION",
        CoreState::ApprovalRequired => "HUMAN APPROVAL REQUIRED",
        CoreState::Error => "SUBSYSTEM ATTENTION REQUIRED",
    };

    let compact_class = if props.compact { "compact" } else { "" };

    rsx! {
        div {
            class: "osmoo-spatial-stage {state_class} {compact_class}",
            onclick: move |_| {
                props.on_click.call(());
            },

            // Ambient background depth glow (deep Snow Black volumetric plane)
            div { class: "core-ambient-plane" }
            div { class: "core-volumetric-haze" }

            // 3D Gimbal Multi-Ring Assembly
            div { class: "core-gimbal-assembly",
                // Outer Ring Alpha (460px, inclined 68deg)
                div { class: "core-ring ring-alpha",
                    div { class: "ring-tick tick-0" }
                    div { class: "ring-tick tick-90" }
                    div { class: "ring-tick tick-180" }
                    div { class: "ring-tick tick-270" }
                    div { class: "ring-subtick subtick-30" }
                    div { class: "ring-subtick subtick-150" }
                    div { class: "ring-subtick subtick-210" }
                    div { class: "ring-subtick subtick-330" }
                }

                // Intermediate Ring Beta (360px, inclined -54deg, reverse spin)
                div { class: "core-ring ring-beta",
                    div { class: "ring-subtick subtick-45" }
                    div { class: "ring-subtick subtick-135" }
                    div { class: "ring-subtick subtick-225" }
                    div { class: "ring-subtick subtick-315" }
                }

                // Precision Ring Gamma (270px, inclined 35deg, fine coordinate ticks)
                div { class: "core-ring ring-gamma",
                    div { class: "ring-tick tick-0" }
                    div { class: "ring-tick tick-180" }
                }

                // Inner Focal Ring Delta (190px, inclined -20deg)
                div { class: "core-ring ring-delta",
                    div { class: "ring-subtick subtick-45" }
                    div { class: "ring-subtick subtick-225" }
                }

                // Central Computational Nucleus (layered monolithic sphere)
                div { class: "core-nucleus",
                    div { class: "nucleus-lens-outer" }
                    div { class: "nucleus-shell" }
                    div { class: "nucleus-lattice" }
                    div { class: "nucleus-emitter" }
                    div { class: "nucleus-singularity" }
                    div { class: "nucleus-lens-reflection" }
                }
            }

            // Minimalist Monospaced Status Telemetry (suppressed in ultra-compact overlay)
            if !props.compact {
                div { class: "core-telemetry",
                    div { class: "telemetry-dot" }
                    span { class: "telemetry-label", "{status_label}" }
                }
            }
        }
    }
}
