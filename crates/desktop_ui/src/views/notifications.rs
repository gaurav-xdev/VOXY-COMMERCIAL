use dioxus::prelude::*;

use crate::bridge::AppBridge;

#[component]
pub fn NotificationsView() -> Element {
    let bridge = use_context::<AppBridge>();
    let notifications = use_signal(Vec::<String>::new);

    let refresh = {
        let nm = bridge.notifications.clone();
        let notifs = notifications;
        move |_: Event<MouseData>| {
            let n = nm.clone();
            let mut list = notifs;
            spawn(async move {
                let history = n.history();
                let formatted: Vec<String> = history
                    .iter()
                    .map(|r| {
                        format!(
                            "[{:?}] {} - {} ({})",
                            r.notification.priority,
                            r.notification.title,
                            r.notification.message,
                            r.timestamp.format("%H:%M:%S")
                        )
                    })
                    .collect();
                list.set(formatted);
            });
        }
    };

    let send_test = {
        let nm = bridge.notifications.clone();
        let notifs = notifications;
        move |_: Event<MouseData>| {
            let n = nm.clone();
            let mut list = notifs;
            spawn(async move {
                n.send(voxy_desktop_runtime::notifications::Notification::info(
                    "Test Notification",
                    "This is a real notification from the NotificationManager",
                ));
                let history = n.history();
                let formatted: Vec<String> = history
                    .iter()
                    .map(|r| {
                        format!(
                            "[{:?}] {} - {} ({})",
                            r.notification.priority,
                            r.notification.title,
                            r.notification.message,
                            r.timestamp.format("%H:%M:%S")
                        )
                    })
                    .collect();
                list.set(formatted);
            });
        }
    };

    let clear_all = {
        let nm = bridge.notifications.clone();
        let notifs = notifications;
        move |_: Event<MouseData>| {
            let n = nm.clone();
            let mut list = notifs;
            spawn(async move {
                n.clear_history();
                list.set(Vec::new());
            });
        }
    };

    rsx! {
        div {
            div { style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;",
                div { style: "font-size: 13px; color: var(--text-secondary);",
                    "{notifications.read().len()} notifications"
                }
                div { style: "display: flex; gap: 8px;",
                    button { class: "btn btn-primary", onclick: send_test, "Send Test" }
                    button { class: "btn btn-secondary", onclick: refresh, "Refresh" }
                    button { class: "btn btn-secondary", onclick: clear_all, "Clear All" }
                }
            }

            if notifications.read().is_empty() {
                div { class: "empty-state",
                    div { class: "empty-state-icon", "\u{1F514}" }
                    div { class: "empty-state-title", "No Notifications" }
                    div { class: "empty-state-desc",
                        "Notifications from the NotificationManager will appear here."
                    }
                }
            } else {
                for notif in notifications.read().iter() {
                    div { class: "notification-item",
                        div { class: "notification-icon info", "\u{2139}" }
                        div { class: "notification-content",
                            div { class: "notification-title", "{notif}" }
                        }
                    }
                }
            }
        }
    }
}
