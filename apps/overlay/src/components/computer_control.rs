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
                    let client = crate::get_ipc_client();
                    spawn(async move {
                        let _ = client.send_command(voxy_ipc::ClientCommand::EmergencyStop).await;
                    });
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

#[component]
pub fn VoxyCursorBeacon(
    x: i32,
    y: i32,
    state_name: String,
    target_element: String,
    action_description: String,
    confidence: f32,
    requires_confirmation: bool,
    action_id: Option<u64>,
    is_active: bool,
) -> Element {
    if !is_active {
        return rsx! {};
    }

    let pos_style = format!(
        "left: {}px; top: {}px; transform: translate(-50%, -50%); position: fixed; pointer-events: none; z-index: 999999;",
        x, y
    );

    rsx! {
        div {
            id: "voxy-cursor-beacon",
            style: "{pos_style}",
            div { class: "voxy-cursor-ring" }
            div { class: "voxy-cursor-dot" }
            div { class: "voxy-cursor-label",
                span { class: "voxy-cursor-badge", "{state_name}" }
                span { class: "voxy-cursor-target", "{target_element}" }
            }
        }
        if requires_confirmation {
            div {
                class: "voxy-confirmation-overlay",
                div {
                    class: "voxy-confirmation-modal clickable",
                    div { class: "modal-warning-header", "ACTION REQUIRES HUMAN CONFIRMATION" }
                    div { class: "modal-desc", "{action_description}" }
                    div { class: "modal-target", "Target: {target_element}" }
                    div { class: "modal-actions",
                        button {
                            class: "btn-confirm-approve clickable",
                            onclick: move |_| {
                                if let Some(aid) = action_id {
                                    let client = crate::get_ipc_client();
                                    spawn(async move {
                                        let _ = client.send_command(voxy_ipc::ClientCommand::ConfirmAction {
                                            action_id: aid,
                                            approved: true,
                                        }).await;
                                    });
                                }
                            },
                            "APPROVE [Y]"
                        }
                        button {
                            class: "btn-confirm-reject clickable",
                            onclick: move |_| {
                                if let Some(aid) = action_id {
                                    let client = crate::get_ipc_client();
                                    spawn(async move {
                                        let _ = client.send_command(voxy_ipc::ClientCommand::ConfirmAction {
                                            action_id: aid,
                                            approved: false,
                                        }).await;
                                    });
                                }
                            },
                            "REJECT [N]"
                        }
                    }
                }
            }
        }
    }
}
