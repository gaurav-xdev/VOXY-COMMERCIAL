use dioxus::prelude::*;
use voxy_ipc::{ClientCommand, IpcApprovalRequest};

#[component]
pub fn ApprovalModal(
    request: IpcApprovalRequest,
    on_close: EventHandler<()>,
) -> Element {
    let bridge = crate::BRIDGE.get().expect("Bridge not initialized").clone();
    let ipc = bridge.ipc.clone();

    let req_id = request.request_id;
    let tool_name = request.tool_name.clone();
    let risk = request.risk_level.clone();
    let reason = request.reason.clone();
    let formatted_params = serde_json::to_string_pretty(&request.parameters_redacted)
        .unwrap_or_else(|_| request.parameters_redacted.to_string());

    let (badge_class, badge_label) = match risk.to_lowercase().as_str() {
        "destructive" => ("risk-badge-destructive", "DESTRUCTIVE OPERATION"),
        "privileged" => ("risk-badge-privileged", "PRIVILEGED ACCESS"),
        "modify" => ("risk-badge-modify", "SYSTEM MODIFICATION"),
        "lowrisk" => ("risk-badge-low", "LOW RISK"),
        _ => ("risk-badge-safe", "STANDARD OPERATION"),
    };

    let on_approve = {
        let ipc = ipc.clone();
        let on_close = on_close.clone();
        move |_| {
            let ipc = ipc.clone();
            let on_close = on_close.clone();
            spawn(async move {
                let _ = ipc
                    .send_command(ClientCommand::RespondApproval {
                        request_id: req_id,
                        approved: true,
                        reason: Some("Approved via Desktop UI".to_string()),
                    })
                    .await;
                on_close.call(());
            });
        }
    };

    let on_deny = {
        let ipc = ipc.clone();
        let on_close = on_close.clone();
        move |_| {
            let ipc = ipc.clone();
            let on_close = on_close.clone();
            spawn(async move {
                let _ = ipc
                    .send_command(ClientCommand::RespondApproval {
                        request_id: req_id,
                        approved: false,
                        reason: Some("Denied via Desktop UI".to_string()),
                    })
                    .await;
                on_close.call(());
            });
        }
    };

    let on_emergency_stop = {
        let ipc = ipc.clone();
        let on_close = on_close.clone();
        move |_| {
            let ipc = ipc.clone();
            let on_close = on_close.clone();
            spawn(async move {
                let _ = ipc.send_command(ClientCommand::EmergencyStop).await;
                on_close.call(());
            });
        }
    };

    rsx! {
        div {
            class: "approval-backdrop",
            div {
                class: "approval-modal",
                div {
                    class: "approval-header",
                    div {
                        class: "approval-header-left",
                        div { class: "approval-shield-icon", "\u{26A0}" }
                        div {
                            div { class: "approval-title", "AUTHORIZATION REQUIRED" }
                            div { class: "approval-subtitle", "OSMOO Governance Boundary Enforcement" }
                        }
                    }
                    div { class: "approval-risk-tag {badge_class}", "{badge_label}" }
                }

                div { class: "approval-body",
                    div { class: "approval-meta-row",
                        span { class: "approval-meta-label", "Target Tool" }
                        span { class: "approval-meta-value tool-badge", "{tool_name}" }
                    }

                    div { class: "approval-meta-row",
                        span { class: "approval-meta-label", "Reason" }
                        span { class: "approval-meta-value", "{reason}" }
                    }

                    div { class: "approval-params-section",
                        div { class: "approval-params-title", "Inspected Parameters (Credentials Sanitized):" }
                        pre { class: "approval-params-code", "{formatted_params}" }
                    }
                }

                div { class: "approval-footer",
                    button {
                        class: "approval-btn-stop",
                        onclick: on_emergency_stop,
                        title: "Immediately halt all automated computer control",
                        "\u{25A0} EMERGENCY STOP"
                    }
                    div { class: "approval-footer-actions",
                        button {
                            class: "approval-btn-deny",
                            onclick: on_deny,
                            "DENY [ESC]"
                        }
                        button {
                            class: "approval-btn-approve",
                            onclick: on_approve,
                            "AUTHORIZE ACTION \u{2713}"
                        }
                    }
                }
            }
        }
    }
}
