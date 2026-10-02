use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[component]
pub fn SubscriptionView() -> Element {
    let bridge = use_context::<AppBridge>();
    let session = bridge.auth.current_session();
    let refresh_status = use_signal(|| Option::<String>::None);
    let is_checking = use_signal(|| false);

    let active_tier = session
        .as_ref()
        .map(|s| s.active_tier.clone())
        .unwrap_or_else(|| "free".to_string());

    let (tier_name, tier_desc, tier_color) = match active_tier.to_lowercase().as_str() {
        "pro" => (
            "Pro Plan",
            "Unlimited local & cloud voice, coding harness, desktop automation",
            "var(--accent-primary)",
        ),
        "enterprise" => (
            "Enterprise Plan",
            "Multi-agent teams, priority cloud routing, full Windows automation",
            "var(--success)",
        ),
        _ => (
            "Free Tier",
            "Standard features with local whisper and foundational model capabilities",
            "var(--text-muted)",
        ),
    };

    let on_refresh = {
        let auth = bridge.auth.clone();
        move |_: Event<MouseData>| {
            let a = auth.clone();
            let mut status = refresh_status;
            let mut checking = is_checking;

            checking.set(true);
            status.set(None);

            spawn(async move {
                match a.check_entitlement("coding_harness").await {
                    Ok(res) => {
                        status.set(Some(format!(
                            "Entitlements verified: Tier '{}' (Access: {})",
                            res.active_tier, res.has_access
                        )));
                    }
                    Err(e) => {
                        status.set(Some(format!("Verification error: {e}")));
                    }
                }
                checking.set(false);
            });
        }
    };

    rsx! {
        div {
            div { class: "card",
                div { class: "card-header",
                    span { class: "card-title", "Subscription & Entitlements" }
                }
                div { style: "padding: 24px; text-align: center;",
                    div { style: "font-size: 28px; font-weight: 700; color: {tier_color};",
                        "{tier_name}"
                    }
                    div { style: "font-size: 13px; color: var(--text-muted); margin-top: 8px; max-width: 480px; margin-left: auto; margin-right: auto;",
                        "{tier_desc}"
                    }

                    if let Some(msg) = refresh_status.read().as_ref() {
                        div { style: "margin-top: 16px; font-size: 12px; color: var(--info);", "{msg}" }
                    }

                    div { style: "margin-top: 20px; display: flex; justify-content: center; gap: 12px;",
                        button {
                            class: "btn btn-primary",
                            disabled: *is_checking.read(),
                            onclick: on_refresh,
                            if *is_checking.read() { "Reconciling with Backend..." } else { "Reconcile Entitlements" }
                        }
                    }
                }
            }

            div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; margin-top: 20px;",
                div { class: "card", style: "border-top: 3px solid var(--text-muted);",
                    div { style: "text-align: center; padding: 16px;",
                        div { style: "font-size: 18px; font-weight: 600;", "Free" }
                        div { style: "font-size: 24px; font-weight: 700; margin: 12px 0;", "$0" }
                        div { style: "font-size: 12px; color: var(--text-secondary); line-height: 1.8;",
                            "Local STT / TTS\nBasic Tool Registry\nSingle Device\nLocal Memory"
                        }
                    }
                }

                div { class: "card", style: "border-top: 3px solid var(--accent-primary);",
                    div { style: "text-align: center; padding: 16px;",
                        div { style: "font-size: 18px; font-weight: 600;", "Pro" }
                        div { style: "font-size: 24px; font-weight: 700; color: var(--accent-primary); margin: 12px 0;", "$29 / mo" }
                        div { style: "font-size: 12px; color: var(--text-secondary); line-height: 1.8;",
                            "Coding Harness\nCloud Streaming Voice\nUnlimited Models\nComputer Control\nOffice Automation"
                        }
                    }
                }

                div { class: "card", style: "border-top: 3px solid var(--success);",
                    div { style: "text-align: center; padding: 16px;",
                        div { style: "font-size: 18px; font-weight: 600;", "Enterprise" }
                        div { style: "font-size: 24px; font-weight: 700; color: var(--success); margin: 12px 0;", "$99 / mo" }
                        div { style: "font-size: 12px; color: var(--text-secondary); line-height: 1.8;",
                            "All Pro Features\nMulti-Agent Teams\nPriority Cloud Routing\nMulti-Device Sync\nCustom Guardians"
                        }
                    }
                }
            }
        }
    }
}
