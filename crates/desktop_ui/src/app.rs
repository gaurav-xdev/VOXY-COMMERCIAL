use dioxus::prelude::*;

use crate::router::Route;
use crate::styles;

#[component]
pub fn App() -> Element {
    let bridge = crate::BRIDGE.get().expect("Bridge not initialized").clone();
    use_context_provider(|| bridge);

    let route = use_signal(|| Route::Chat);

    rsx! {
        document::Link { rel: "stylesheet", href: styles::APP_CSS }
        document::Link { rel: "preconnect", href: "https://fonts.googleapis.com" }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap"
        }

        div { class: "app-layout",
            crate::components::sidebar::Sidebar { route: route }

            div { class: "main-content",
                div { class: "content-header",
                    h1 { "{route.read().label()}" }
                }

                div { class: "content-body",
                    match *route.read() {
                        Route::Chat => rsx! { crate::views::chat::ChatView {} },
                        Route::Settings => rsx! { crate::views::settings::SettingsView {} },
                        Route::Memory => rsx! { crate::views::memory::MemoryView {} },
                        Route::Plugins => rsx! { crate::views::plugins::PluginsView {} },
                        Route::Downloads => rsx! { crate::views::downloads::DownloadsView {} },
                        Route::Notifications => rsx! { crate::views::notifications::NotificationsView {} },
                        Route::Account => rsx! { crate::views::account::AccountView {} },
                        Route::Subscription => rsx! { crate::views::subscription::SubscriptionView {} },
                        Route::Login => rsx! { crate::views::login::LoginView {} },
                        Route::Health => rsx! { crate::views::health::HealthView {} },
                        Route::Orb => rsx! { crate::views::orb::OrbView {} },
                    }
                }
            }
        }
    }
}
