use dioxus::prelude::*;

use crate::bridge::AppBridge;
use crate::router::Route;

#[component]
pub fn Sidebar(route: Signal<Route>) -> Element {
    let bridge = use_context::<AppBridge>();
    let session = bridge.auth.current_session();

    let user_name = session
        .as_ref()
        .map(|s| s.email.split('@').next().unwrap_or("User").to_string())
        .unwrap_or_else(|| "Guest".to_string());

    let plan_name = session
        .as_ref()
        .map(|s| match s.active_tier.to_lowercase().as_str() {
            "pro" => "Pro Plan",
            "enterprise" => "Enterprise",
            _ => "Free Tier",
        })
        .unwrap_or("Free Tier");
    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-header",
                div { class: "sidebar-logo", "V" }
                div {
                    div { class: "sidebar-title", "VOXY" }
                    div { class: "sidebar-version", "v0.1.0" }
                }
            }

            div { class: "sidebar-nav",
                // MAIN
                div { class: "nav-section",
                    div { class: "nav-section-label", "MAIN" }
                    SidebarItem { route: route, target: Route::Chat, label: "Chat", icon: "\u{1F4AC}" }
                    SidebarItem { route: route, target: Route::Orb, label: "Voice Orb", icon: "\u{1F300}" }
                }

                // MANAGE
                div { class: "nav-section",
                    div { class: "nav-section-label", "MANAGE" }
                    SidebarItem { route: route, target: Route::Memory, label: "Memory", icon: "\u{1F4E6}" }
                    SidebarItem { route: route, target: Route::Plugins, label: "Plugins", icon: "\u{1F50C}" }
                    SidebarItem { route: route, target: Route::Downloads, label: "Downloads", icon: "\u{2B07}" }
                }

                // SYSTEM
                div { class: "nav-section",
                    div { class: "nav-section-label", "SYSTEM" }
                    SidebarItem { route: route, target: Route::Notifications, label: "Notifications", icon: "\u{1F514}" }
                    SidebarItem { route: route, target: Route::Health, label: "Health", icon: "\u{1F4CA}" }
                    SidebarItem { route: route, target: Route::Settings, label: "Settings", icon: "\u{2699}" }
                }

                // ACCOUNT
                div { class: "nav-section",
                    div { class: "nav-section-label", "ACCOUNT" }
                    SidebarItem { route: route, target: Route::Account, label: "Account", icon: "\u{1F464}" }
                    SidebarItem { route: route, target: Route::Subscription, label: "Subscription", icon: "\u{2B50}" }
                }
            }

            div { class: "sidebar-footer",
                div { class: "user-info",
                    div { class: "user-avatar", "\u{1F464}" }
                    div {
                        div { class: "user-name", "{user_name}" }
                        div { class: "user-plan", "{plan_name}" }
                    }
                }
            }
        }
    }
}

#[component]
fn SidebarItem(
    route: Signal<Route>,
    target: Route,
    label: &'static str,
    icon: &'static str,
) -> Element {
    let is_active = *route.read() == target;
    rsx! {
        button {
            class: if is_active { "nav-item active" } else { "nav-item" },
            onclick: move |_| route.set(target),
            span { class: "nav-item-icon", "{icon}" }
            span { "{label}" }
        }
    }
}
