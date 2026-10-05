use dioxus::prelude::*;

use crate::components::core::{CoreState, OsmooCore};
use crate::components::sidebar::Sidebar;
use crate::components::startup::CinematicStartup;
use crate::router::Route;
use crate::styles;
use crate::views::auth::AuthView;
use crate::views::onboarding::OnboardingFlow;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppPhase {
    Startup,
    Auth,
    Onboarding,
    Main,
}

#[component]
pub fn App() -> Element {
    let bridge = crate::BRIDGE.get().expect("Bridge not initialized").clone();
    use_context_provider(|| bridge.clone());

    let mut phase = use_signal(|| AppPhase::Startup);
    let mut route = use_signal(|| Route::Core);
    let mut sidebar_open = use_signal(|| false);
    let core_state = use_signal(|| CoreState::Idle);
    let mut prompt_input = use_signal(String::new);
    let mut active_approval = use_signal(|| None::<voxy_ipc::IpcApprovalRequest>);

    // Live IPC and Voice telemetry loop to keep CoreState and Approvals updated
    {
        let mut core_state = core_state;
        let mut active_approval = active_approval;
        let ipc = bridge.ipc.clone();

        spawn(async move {
            let mut event_rx = ipc.subscribe();

            while let Ok(msg) = event_rx.recv().await {
                match msg {
                    voxy_ipc::DaemonMessage::VoiceStateChanged { state, .. } => {
                        match state {
                            voxy_ipc::VoiceState::Listening => core_state.set(CoreState::Listening),
                            voxy_ipc::VoiceState::Thinking => core_state.set(CoreState::Thinking),
                            voxy_ipc::VoiceState::Speaking => core_state.set(CoreState::Speaking),
                            voxy_ipc::VoiceState::Error => core_state.set(CoreState::Error),
                            _ => core_state.set(CoreState::Idle),
                        }
                    }
                    voxy_ipc::DaemonMessage::ErrorNotification { .. } => {
                        core_state.set(CoreState::Error);
                    }
                    voxy_ipc::DaemonMessage::ApprovalRequested(req) => {
                        core_state.set(CoreState::ApprovalRequired);
                        active_approval.set(Some(req));
                    }
                    voxy_ipc::DaemonMessage::ApprovalResolved { request_id, .. } => {
                        let should_clear = active_approval
                            .read()
                            .as_ref()
                            .map(|curr| curr.request_id == request_id)
                            .unwrap_or(false);
                        if should_clear {
                            active_approval.set(None);
                            core_state.set(CoreState::Idle);
                        }
                    }
                    voxy_ipc::DaemonMessage::EmergencyStopChanged { is_stopped } => {
                        if is_stopped {
                            active_approval.set(None);
                            core_state.set(CoreState::Idle);
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    let on_startup_complete = {
        let bridge = bridge.clone();
        move |()| {
            let is_auth = bridge.auth.is_authenticated();
            let is_onboarded = bridge.settings.get().onboarded;

            if !is_auth {
                phase.set(AppPhase::Auth);
            } else if !is_onboarded {
                phase.set(AppPhase::Onboarding);
            } else {
                phase.set(AppPhase::Main);
            }
        }
    };

    let on_authenticated = {
        let bridge = bridge.clone();
        move |()| {
            let is_onboarded = bridge.settings.get().onboarded;
            if !is_onboarded {
                phase.set(AppPhase::Onboarding);
            } else {
                phase.set(AppPhase::Main);
            }
        }
    };

    let on_onboarding_complete = move |()| {
        phase.set(AppPhase::Main);
    };

    let on_core_click = {
        let voice = bridge.voice.clone();
        let mut core_state = core_state;
        move |()| {
            let v = voice.clone();
            spawn(async move {
                if v.is_speaking() {
                    v.interrupt_tts().await;
                    core_state.set(CoreState::Idle);
                } else if v.is_running() {
                    let _ = v.barge_in().await;
                    core_state.set(CoreState::Listening);
                } else {
                    let _ = v.start_listening().await;
                    core_state.set(CoreState::Listening);
                }
            });
        }
    };

    let send_prompt = {
        let mut prompt_input = prompt_input;
        let cognition = bridge.cognition.clone();
        let voice = bridge.voice.clone();
        let ipc = bridge.ipc.clone();
        let mut core_state = core_state;

        move |_: Event<MouseData>| {
            let text = prompt_input.read().trim().to_string();
            if text.is_empty() {
                return;
            }
            prompt_input.set(String::new());
            core_state.set(CoreState::Thinking);

            let cog = cognition.clone();
            let v = voice.clone();
            let ipc_client = ipc.clone();

            spawn(async move {
                let _ = ipc_client
                    .send_command(voxy_ipc::ClientCommand::SendTextInput { text: text.clone() })
                    .await;

                let intent_input = voxy_cognition::IntentInput {
                    raw_text: text,
                    context: None,
                    source: "desktop_ui_core".to_string(),
                    metadata: std::collections::HashMap::new(),
                };

                match cog.process(&intent_input).await {
                    Ok(result) => {
                        core_state.set(CoreState::Speaking);
                        let response_text = serde_json::to_string_pretty(&result.result)
                            .unwrap_or_else(|_| format!("{:?}", result.result));
                        let _ = v.speak(&response_text).await;
                        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                        core_state.set(CoreState::Idle);
                    }
                    Err(_) => {
                        core_state.set(CoreState::Error);
                        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                        core_state.set(CoreState::Idle);
                    }
                }
            });
        }
    };

    rsx! {
        style { "{styles::APP_CSS}" }
        document::Link { rel: "preconnect", href: "https://fonts.googleapis.com" }
        document::Link {
            rel: "stylesheet",
            href: "https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap"
        }

        match *phase.read() {
            AppPhase::Startup => rsx! {
                CinematicStartup { on_complete: on_startup_complete }
            },
            AppPhase::Auth => rsx! {
                AuthView { on_authenticated: on_authenticated }
            },
            AppPhase::Onboarding => rsx! {
                OnboardingFlow { on_completed: on_onboarding_complete }
            },
            AppPhase::Main => rsx! {
                div { class: "app-layout", style: "position: relative; width: 100vw; height: 100vh; overflow: hidden;",
                    // HUD Top Bar
                    div { class: "hud-topbar",
                        div { class: "hud-brand",
                            div { class: "hud-logo", "O" }
                            div {
                                div { class: "hud-title", "OSMOO" }
                                div { class: "hud-parent", "OSMIORA COMPUTATIONAL SYSTEMS" }
                            }
                        }

                        div { class: "hud-controls",
                            button {
                                class: if *route.read() == Route::Core { "hud-btn active" } else { "hud-btn" },
                                onclick: move |_| route.set(Route::Core),
                                span { style: "font-size: 13px;", "\u{25C9}" }
                                span { "Core" }
                            }
                            button {
                                class: if *sidebar_open.read() { "hud-btn active" } else { "hud-btn" },
                                onclick: move |_| {
                                    let current = *sidebar_open.read();
                                    sidebar_open.set(!current);
                                },
                                span { style: "font-size: 13px;", "\u{2630}" }
                                span { "Menu" }
                            }
                            button {
                                class: "hud-btn",
                                title: "Minimize to Background Companion",
                                onclick: {
                                    let bridge = bridge.clone();
                                    move |_| {
                                        let settings = bridge.settings.get();
                                        if settings.appearance.close_to_overlay {
                                            tracing::info!("Hiding window to background overlay companion");
                                            let _ = bridge.window_tracker.minimize_to_tray();
                                        } else {
                                            tracing::info!("Standard minimize requested");
                                            let _ = bridge.window_tracker.minimize_to_tray();
                                        }
                                    }
                                },
                                span { style: "font-size: 11px;", "\u{25BD}" }
                                span { "Companion" }
                            }
                        }
                    }

                    // Backdrop for slide-in drawer
                    div {
                        class: if *sidebar_open.read() { "sidebar-overlay-backdrop open" } else { "sidebar-overlay-backdrop" },
                        onclick: move |_| sidebar_open.set(false),
                    }

                    // Slide-in Sidebar Drawer
                    div {
                        class: if *sidebar_open.read() { "sidebar-drawer open" } else { "sidebar-drawer" },
                        Sidebar {
                            route: route,
                            on_close: move |()| sidebar_open.set(false),
                        }
                    }

                    // Main Viewport
                    if *route.read() == Route::Core {
                        // Full-screen Snow Black 3D Gimbal Core
                        div { style: "width: 100%; height: 100%; position: relative;",
                            OsmooCore {
                                state: Some(*core_state.read()),
                                on_click: on_core_click,
                            }

                            // Interactive minimalist prompt bar docked at bottom
                            div { class: "hud-prompt-bar",
                                input {
                                    class: "hud-prompt-input",
                                    r#type: "text",
                                    placeholder: "Transmit command to OSMOO...",
                                    value: "{prompt_input.read()}",
                                    oninput: move |evt: Event<FormData>| prompt_input.set(evt.value()),
                                    onkeydown: {
                                        let mut prompt_input = prompt_input;
                                        let cognition = bridge.cognition.clone();
                                        let voice = bridge.voice.clone();
                                        let ipc = bridge.ipc.clone();
                                        let mut core_state = core_state;

                                        move |evt: Event<KeyboardData>| {
                                            if evt.key() == Key::Enter {
                                                let text = prompt_input.read().trim().to_string();
                                                if text.is_empty() {
                                                    return;
                                                }
                                                prompt_input.set(String::new());
                                                core_state.set(CoreState::Thinking);

                                                let cog = cognition.clone();
                                                let v = voice.clone();
                                                let ipc_client = ipc.clone();

                                                spawn(async move {
                                                    let _ = ipc_client
                                                        .send_command(voxy_ipc::ClientCommand::SendTextInput { text: text.clone() })
                                                        .await;

                                                    let intent_input = voxy_cognition::IntentInput {
                                                        raw_text: text,
                                                        context: None,
                                                        source: "desktop_ui_core".to_string(),
                                                        metadata: std::collections::HashMap::new(),
                                                    };

                                                    match cog.process(&intent_input).await {
                                                        Ok(result) => {
                                                            core_state.set(CoreState::Speaking);
                                                            let response_text = serde_json::to_string_pretty(&result.result)
                                                                .unwrap_or_else(|_| format!("{:?}", result.result));
                                                            let _ = v.speak(&response_text).await;
                                                            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                                                            core_state.set(CoreState::Idle);
                                                        }
                                                        Err(_) => {
                                                            core_state.set(CoreState::Error);
                                                            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                                                            core_state.set(CoreState::Idle);
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    }
                                }
                                button {
                                    class: "hud-send-btn",
                                    onclick: send_prompt,
                                    "Transmit"
                                }
                            }
                        }
                    } else {
                        // Subview active overlay container
                        div { class: "active-view-container",
                            div { style: "display: flex; align-items: center; justify-content: space-between; margin-bottom: 24px; border-bottom: 1px solid #1f1f28; padding-bottom: 16px;",
                                h1 { style: "font-size: 18px; font-weight: 600; color: #ededed; letter-spacing: 0.05em;", "{route.read().label()}" }
                                button {
                                    class: "hud-btn",
                                    onclick: move |_| route.set(Route::Core),
                                    span { "\u{2190}" }
                                    span { "Return to Core" }
                                }
                            }

                            match *route.read() {
                                Route::Core => rsx! {},
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

                    // Floating Human Approval & Governance Modal
                    if let Some(req) = active_approval.read().as_ref() {
                        crate::components::approval_modal::ApprovalModal {
                            request: req.clone(),
                            on_close: move |()| {
                                active_approval.set(None);
                            },
                        }
                    }
                }
            }
        }
    }
}
