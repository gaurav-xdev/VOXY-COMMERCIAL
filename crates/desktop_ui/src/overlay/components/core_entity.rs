use crate::assets::{
    RIG_BODY_DATA_URI, RIG_EAR_LEFT_DATA_URI, RIG_EAR_RIGHT_DATA_URI, RIG_HEAD_DATA_URI,
    RIG_SUNGLASSES_DATA_URI, RIG_TAIL_DATA_URI,
};
use crate::overlay::types::VisualState;
use dioxus::prelude::*;

#[component]
pub fn CoreEntity(
    visual_state: VisualState,
    is_collapsed: bool,
    identity_name: String,
    profile_label: String,
    on_toggle: EventHandler<MouseEvent>,
) -> Element {
    let state_label = visual_state.label();

    let root_class = match visual_state {
        VisualState::Idle => "clickable state-idle",
        VisualState::Listening => "clickable state-listening",
        VisualState::Processing => "clickable state-processing",
        VisualState::Thinking => "clickable state-thinking",
        VisualState::Speaking => "clickable state-speaking",
        VisualState::Executing => "clickable state-executing",
        VisualState::WaitingForUser => "clickable state-waiting",
        VisualState::Success => "clickable state-success",
        VisualState::Error => "clickable state-error",
        VisualState::Offline => "clickable state-offline",
        VisualState::Disabled => "clickable state-disabled",
    };

    let default_mouth_svg = match visual_state {
        VisualState::Success => {
            r##"<path d="M 4,7 Q 14,15 24,7" fill="none" stroke="#1e293b" stroke-width="2.4" stroke-linecap="round"/>"##
        }
        VisualState::Error => {
            r##"<path d="M 5,11 Q 14,6 23,11" fill="none" stroke="#1e293b" stroke-width="2.2" stroke-linecap="round"/>"##
        }
        _ => {
            r##"<path d="M 5,8 Q 10,12 14,9 Q 18,12 23,8" fill="none" stroke="#1e293b" stroke-width="2.2" stroke-linecap="round"/>"##
        }
    };

    rsx! {
        div {
            id: "osmoo-cat-companion",
            class: "{root_class}",
            title: "OSMOO Cat Companion (Drag to move, Double-click to open Dashboard, Right-click for options)",

            // Soft contact shadow beneath paws
            div { class: "cat-contact-shadow" }

            // Primary Rigged 2D Character Frame
            div { class: "cat-character-frame",
                // 1. Animated Tail Layer (Swings organically behind body)
                div { class: "cat-tail-container",
                    img {
                        class: "cat-tail-img",
                        src: "{RIG_TAIL_DATA_URI}",
                        alt: "Cat Tail",
                    }
                }

                // 2. Animated Body Layer (White torso in black 'osmoo' t-shirt and legs)
                div { class: "cat-body-container",
                    img {
                        class: "cat-body-img",
                        src: "{RIG_BODY_DATA_URI}",
                        alt: "Cat Body",
                    }
                }

                // 3. Animated Head Layer (Nodding, tilting, looking around)
                div { class: "cat-head-container",
                    img {
                        class: "cat-head-img",
                        src: "{RIG_HEAD_DATA_URI}",
                        alt: "Cat Head",
                    }

                    // Left Ear Layer (Periodic autonomous twitching)
                    div { id: "cat-ear-left", class: "cat-ear-left-container",
                        img {
                            class: "cat-ear-img",
                            src: "{RIG_EAR_LEFT_DATA_URI}",
                            alt: "Left Ear",
                        }
                    }

                    // Right Ear Layer (Periodic autonomous twitching)
                    div { id: "cat-ear-right", class: "cat-ear-right-container",
                        img {
                            class: "cat-ear-img",
                            src: "{RIG_EAR_RIGHT_DATA_URI}",
                            alt: "Right Ear",
                        }
                    }

                    // Sunglasses Layer with Dynamic Specular Glint & Blink
                    div { class: "cat-sunglasses-container",
                        img {
                            class: "cat-sunglasses-img",
                            src: "{RIG_SUNGLASSES_DATA_URI}",
                            alt: "Sunglasses",
                        }
                        div { class: "cat-glasses-glint" }
                        div { id: "cat-eyelids", class: "cat-eyelids" }
                    }

                    // Live Audio-Driven SVG Lip-Sync Mouth
                    div { class: "cat-mouth-container",
                        svg {
                            id: "cat-mouth-svg",
                            class: "cat-mouth-svg",
                            view_box: "0 0 28 18",
                            dangerous_inner_html: "{default_mouth_svg}",
                        }
                    }
                }
            }

            // Compact State Pill
            div { class: "cat-state-pill",
                span { id: "cat-state-dot", class: "cat-state-dot" }
                span { id: "cat-state-text", "{state_label}" }
            }

            // Desktop Right-Click Context Menu
            div { id: "cat-context-menu", class: "cat-context-menu",
                button {
                    class: "cat-menu-item",
                    onclick: move |_| {
                        let _ = document::eval("window.voxyOpenDashboard();");
                    },
                    "📂 Open OSMOO Dashboard"
                }
                button {
                    class: "cat-menu-item",
                    onclick: move |_| {
                        let _ = document::eval("window.voxyToggleMic();");
                    },
                    "🎙️ Toggle Microphone"
                }
                button {
                    class: "cat-menu-item",
                    onclick: move |_| {
                        let _ = document::eval("window.voxyToggleAutoWalk();");
                    },
                    "🐾 Toggle Autonomous Walking"
                }
                div { class: "cat-menu-divider" }
                button {
                    class: "cat-menu-item",
                    onclick: move |_| {
                        let _ = document::eval("window.voxySetCatScale(0.75);");
                    },
                    "📐 Size: Compact (75%)"
                }
                button {
                    class: "cat-menu-item",
                    onclick: move |_| {
                        let _ = document::eval("window.voxySetCatScale(1.0);");
                    },
                    "📐 Size: Standard (100%)"
                }
                button {
                    class: "cat-menu-item",
                    onclick: move |_| {
                        let _ = document::eval("window.voxySetCatScale(1.35);");
                    },
                    "📐 Size: Expanded (135%)"
                }
                div { class: "cat-menu-divider" }
                button {
                    class: "cat-menu-item",
                    onclick: move |_| {
                        let _ = document::eval("window.voxyToggleAlwaysOnTop();");
                    },
                    "📌 Toggle Always On Top"
                }
                button {
                    class: "cat-menu-item danger",
                    onclick: move |_| {
                        let _ = document::eval("window.voxyEmergencyStop();");
                        spawn(async move {
                            if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                let _ = dispatcher.dispatch(crate::backend::BackendCommand::EmergencyStop).await;
                            }
                        });
                    },
                    "🛑 Emergency Halt"
                }
                button {
                    class: "cat-menu-item danger",
                    onclick: move |_| {
                        let _ = document::eval("window.voxyExitApp();");
                        let _ = std::thread::spawn(|| { std::process::exit(0); });
                    },
                    "❌ Exit OSMOO"
                }
            }
        }
    }
}
