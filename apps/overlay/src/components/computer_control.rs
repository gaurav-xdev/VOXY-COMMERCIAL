use crate::types::{StepStatus, ToolStep};
use dioxus::prelude::*;

#[component]
pub fn ComputerControlBanner(
    target_app: String,
    current_action: String,
    is_visible: bool,
) -> Element {
    let display_style = if is_visible {
        "display: flex;"
    } else {
        "display: none;"
    };

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

#[component]
pub fn OverlayApprovalModal(
    request: voxy_ipc::IpcApprovalRequest,
    on_close: EventHandler<()>,
) -> Element {
    let req_id = request.request_id;
    let tool = request.tool_name.clone();
    let reason = request.reason.clone();
    let risk = request.risk_level.clone();
    let params_str = serde_json::to_string_pretty(&request.parameters_redacted)
        .unwrap_or_else(|_| request.parameters_redacted.to_string());

    let (risk_color, risk_label) = match risk.to_lowercase().as_str() {
        "destructive" => ("#ef4444", "DESTRUCTIVE OPERATION"),
        "privileged" => ("#f59e0b", "PRIVILEGED ACCESS"),
        "modify" => ("#60a5fa", "SYSTEM MODIFICATION"),
        _ => ("#10b981", "OPERATION AUTHORIZATION"),
    };

    let on_approve = {
        let on_close = on_close.clone();
        move |_| {
            let on_close = on_close.clone();
            spawn(async move {
                let client = crate::get_ipc_client();
                let _ = client
                    .send_command(voxy_ipc::ClientCommand::RespondApproval {
                        request_id: req_id,
                        approved: true,
                        reason: Some("Approved via Overlay UI".to_string()),
                    })
                    .await;
                on_close.call(());
            });
        }
    };

    let on_deny = {
        let on_close = on_close.clone();
        move |_| {
            let on_close = on_close.clone();
            spawn(async move {
                let client = crate::get_ipc_client();
                let _ = client
                    .send_command(voxy_ipc::ClientCommand::RespondApproval {
                        request_id: req_id,
                        approved: false,
                        reason: Some("Denied via Overlay UI".to_string()),
                    })
                    .await;
                on_close.call(());
            });
        }
    };

    let on_stop = {
        let on_close = on_close.clone();
        move |_| {
            let on_close = on_close.clone();
            spawn(async move {
                let client = crate::get_ipc_client();
                let _ = client
                    .send_command(voxy_ipc::ClientCommand::EmergencyStop)
                    .await;
                on_close.call(());
            });
        }
    };

    rsx! {
        div {
            class: "voxy-confirmation-overlay",
            style: "position: fixed; inset: 0; background: rgba(5, 5, 8, 0.85); backdrop-filter: blur(8px); display: flex; align-items: center; justify-content: center; z-index: 9999999;",
            div {
                class: "voxy-confirmation-modal clickable",
                style: "background: #0d0d12; border: 1px solid #272733; border-radius: 12px; width: 90%; max-width: 520px; overflow: hidden; box-shadow: 0 16px 48px rgba(0,0,0,0.9);",

                div { style: "padding: 16px 20px; background: #13131c; border-bottom: 1px solid #1f1f2a; display: flex; align-items: center; justify-content: space-between;",
                    div { style: "display: flex; align-items: center; gap: 10px;",
                        span { style: "color: #f59e0b; font-size: 18px;", "\u{26A0}" }
                        div {
                            div { style: "font-size: 13px; font-weight: 700; color: #ededed; letter-spacing: 0.06em;", "OSMOO GOVERNANCE GATE" }
                            div { style: "font-size: 11px; color: #71717a;", "Action paused awaiting human approval" }
                        }
                    }
                    div { style: "font-size: 10px; font-weight: 700; color: {risk_color}; border: 1px solid {risk_color}; padding: 3px 8px; border-radius: 4px; text-transform: uppercase;", "{risk_label}" }
                }

                div { style: "padding: 20px; display: flex; flex-direction: column; gap: 12px;",
                    div { style: "display: flex; align-items: center; gap: 12px;",
                        span { style: "font-size: 12px; font-weight: 600; color: #71717a; width: 80px;", "Tool:" }
                        span { style: "font-family: monospace; font-size: 12px; background: #181822; padding: 2px 8px; border-radius: 4px; border: 1px solid #282838; color: #e4e4e7;", "{tool}" }
                    }
                    div { style: "display: flex; align-items: center; gap: 12px;",
                        span { style: "font-size: 12px; font-weight: 600; color: #71717a; width: 80px;", "Reason:" }
                        span { style: "font-size: 12px; color: #d4d4d8;", "{reason}" }
                    }
                    div { style: "margin-top: 4px;",
                        div { style: "font-size: 11px; font-weight: 600; color: #71717a; text-transform: uppercase; margin-bottom: 4px;", "Parameters (Credentials Sanitized):" }
                        pre { style: "background: #07070a; border: 1px solid #1c1c26; border-radius: 6px; padding: 10px 12px; font-family: monospace; font-size: 11px; color: #a1a1aa; max-height: 120px; overflow-y: auto; white-space: pre-wrap; word-break: break-all;", "{params_str}" }
                    }
                }

                div { style: "padding: 14px 20px; background: #111118; border-top: 1px solid #1f1f2a; display: flex; align-items: center; justify-content: space-between;",
                    button {
                        style: "background: transparent; border: 1px solid rgba(239, 68, 68, 0.4); color: #ef4444; font-size: 11px; font-weight: 700; padding: 7px 12px; border-radius: 6px; cursor: pointer;",
                        onclick: on_stop,
                        "\u{25A0} EMERGENCY STOP"
                    }
                    div { style: "display: flex; align-items: center; gap: 8px;",
                        button {
                            style: "background: #1a1a24; border: 1px solid #2d2d3d; color: #a1a1aa; font-size: 12px; font-weight: 600; padding: 7px 14px; border-radius: 6px; cursor: pointer;",
                            onclick: on_deny,
                            "DENY [ESC]"
                        }
                        button {
                            style: "background: #ededed; border: 1px solid #ffffff; color: #070709; font-size: 12px; font-weight: 700; padding: 7px 18px; border-radius: 6px; cursor: pointer;",
                            onclick: on_approve,
                            "APPROVE \u{2713}"
                        }
                    }
                }
            }
        }
    }
}
