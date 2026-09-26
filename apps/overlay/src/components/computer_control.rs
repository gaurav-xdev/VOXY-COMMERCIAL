use dioxus::prelude::*;
use crate::types::{StepStatus, ToolStep};

#[component]
pub fn ComputerControlBanner(
    target_app: String,
    current_action: String,
    is_visible: bool,
) -> Element {
    let display_style = if is_visible { "display: flex;" } else { "display: none;" };

    rsx! {
        div {
            id: "computer-control-banner",
            class: "computer-control-banner clickable",
            style: "{display_style}",

            div { class: "control-target-info",
                div { class: "control-pulse" }
                div {
                    div { class: "control-title", "AUTOMATED COMPUTER CONTROL IN PROGRESS" }
                    div { id: "control-action-text", class: "control-action",
                        "Target: {target_app} — {current_action}"
                    }
                }
            }

            // Emergency Stop Button - Non-negotiable safety control
            button {
                class: "emergency-stop-btn clickable",
                onclick: move |_| {
                    let _ = document::eval("window.voxyEmergencyStop();");
                },
                title: "Immediately halt all automated mouse, keyboard, and system actions",
                span { "STOP" }
                span { "EMERGENCY HALT [ESC]" }
            }
        }
    }
}

#[component]
pub fn ToolExecutionTimeline(steps: Vec<ToolStep>) -> Element {
    rsx! {
        div { style: "display: flex; flex-direction: column; gap: 8px; width: 100%;",
            for step in steps {
                {
                    let status_dot_class = match step.status {
                        StepStatus::Completed => "tool-status-dot status-completed",
                        StepStatus::Running => "tool-status-dot status-running",
                        StepStatus::Pending => "tool-status-dot status-pending",
                        StepStatus::Failed => "tool-status-dot status-failed",
                    };
                    rsx! {
                        div { key: "{step.id}", class: "tool-timeline-item",
                            div { class: "{status_dot_class}" }
                            div { style: "display: flex; flex-direction: column; gap: 2px;",
                                div { style: "font-size: 11px; font-weight: 600; color: var(--text-primary);", "{step.title}" }
                                div { style: "font-family: var(--font-mono); font-size: 10px; color: var(--text-secondary);", "{step.detail}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
