use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[component]
pub fn HealthView() -> Element {
    let bridge = use_context::<AppBridge>();
    let checks = use_signal(Vec::<(String, String, String)>::new);
    let sys_info = use_signal(Vec::<(String, String)>::new);

    let refresh = {
        let hm = bridge.health.clone();
        let sys = sys_info;
        let event_bus = bridge.event_bus.clone();
        move |_: Event<MouseData>| {
            let h = hm.clone();
            let mut c = checks;
            let mut s = sys;
            let bus = event_bus.clone();
            spawn(async move {
                let results = h.check_all().await;
                let mut formatted: Vec<(String, String, String)> = results
                    .iter()
                    .map(|(name, report)| {
                        let status_str = format!("{:?}", report.status);
                        let detail = report.details.clone().unwrap_or_default();
                        (name.clone(), status_str, detail)
                    })
                    .collect();
                formatted.sort_by(|a, b| a.0.cmp(&b.0));
                c.set(formatted);

                let mut sys_items = Vec::new();
                let num_cpus = num_cpus::get();
                sys_items.push(("CPU Cores".to_string(), format!("{}", num_cpus)));

                let mut sysinfo_sys = sysinfo::System::new();
                sysinfo_sys.refresh_memory();
                let total_mem = sysinfo_sys.total_memory() / (1024 * 1024);
                let used_mem = sysinfo_sys.used_memory() / (1024 * 1024);
                let pct = if total_mem > 0 {
                    used_mem as f64 / total_mem as f64 * 100.0
                } else {
                    0.0
                };
                sys_items.push((
                    "Memory".to_string(),
                    format!("{}MB / {}MB ({:.1}%)", used_mem, total_mem, pct),
                ));

                let topic_count = bus.topic_count().await;
                let topic_names = bus.topic_names().await;
                sys_items.push(("EventBus Topics".to_string(), format!("{}", topic_count)));
                if !topic_names.is_empty() {
                    sys_items.push(("Active Topics".to_string(), topic_names.join(", ")));
                }
                sys_items.push((
                    "Health Checks".to_string(),
                    format!("{}", h.checks().await.len()),
                ));

                s.set(sys_items);
            });
        }
    };

    let all_healthy = checks.read().iter().all(|(_, s, _)| s == "Healthy");
    let no_checks = checks.read().is_empty();
    let badge_class = if no_checks {
        "badge badge-info"
    } else if all_healthy {
        "badge badge-success"
    } else {
        "badge badge-warning"
    };
    let status_text = if no_checks {
        "Click Refresh"
    } else if all_healthy {
        "All Systems Operational"
    } else {
        "Some Systems Degraded"
    };

    rsx! {
        div {
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;",
                div { style: "display: flex; gap: 8px;",
                    span { class: badge_class, "{status_text}" }
                }
                button { class: "btn btn-secondary", onclick: refresh, "Refresh" }
            }

            if !sys_info.read().is_empty() {
                div { class: "card", style: "margin-bottom: 20px;",
                    div { class: "card-header",
                        span { class: "card-title", "System Information" }
                    }
                    for (label, value) in sys_info.read().iter() {
                        div { class: "setting-row",
                            div { class: "setting-label", "{label}" }
                            span { style: "color: var(--text-secondary); font-size: 13px;", "{value}" }
                        }
                    }
                }
            }

            if no_checks {
                div { class: "empty-state",
                    div { class: "empty-state-icon", "\u{1F4CA}" }
                    div { class: "empty-state-title", "Health Monitor Active" }
                    div { class: "empty-state-desc",
                        "Click Refresh to run health checks. CPU and Memory monitors are registered."
                    }
                }
            } else {
                div { class: "health-grid",
                    for (name, status, detail) in checks.read().iter() {
                        HealthCard { name: name.clone(), status: status.clone(), detail: detail.clone() }
                    }
                }
            }
        }
    }
}

#[component(eq = false)]
fn HealthCard(name: String, status: String, detail: String) -> Element {
    let dot_class = match status.as_str() {
        "Healthy" => "status-dot healthy",
        "Degraded" => "status-dot degraded",
        "Unhealthy" => "status-dot unhealthy",
        _ => "status-dot unknown",
    };
    let detail_text = if detail.is_empty() {
        "No details"
    } else {
        &detail
    };

    rsx! {
        div { class: "health-card",
            div { class: dot_class }
            div { class: "health-card-info",
                div { class: "health-card-name", "{name}" }
                div { class: "health-card-detail", "{detail_text}" }
            }
        }
    }
}
