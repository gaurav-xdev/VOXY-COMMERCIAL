mod types;
mod styles;
mod client_js;
mod components;

use dioxus::prelude::*;
use std::sync::Arc;
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;
use voxy_event_bus::EventBus;

use types::{ChatMessage, DesktopMode, StepStatus, SystemTelemetry, ToolStep, VisualState};
use styles::GLOBAL_STYLES;
use client_js::CLIENT_JS;
use components::core_entity::CoreEntity;
use components::context_layer::{ContextBottomBar, ContextTopBar};
use components::computer_control::ComputerControlBanner;
use components::depth_drawer::{DepthDrawer, DrawerTab};
use components::mode_views::{CompactModeView, EdgeDockView, MinimalView};

struct GlobalAppState {
    visual_state: parking_lot::RwLock<VisualState>,
    desktop_mode: parking_lot::RwLock<DesktopMode>,
    is_collapsed: parking_lot::RwLock<bool>,
    drawer_open: parking_lot::RwLock<bool>,
    transcript: parking_lot::RwLock<String>,
    sensor_gesture: parking_lot::RwLock<String>,
    turn_count: parking_lot::RwLock<usize>,
    messages: parking_lot::RwLock<Vec<ChatMessage>>,
    tool_steps: parking_lot::RwLock<Vec<ToolStep>>,
}

impl GlobalAppState {
    fn new() -> Self {
        Self {
            visual_state: parking_lot::RwLock::new(VisualState::Idle),
            desktop_mode: parking_lot::RwLock::new(DesktopMode::FullExperience),
            is_collapsed: parking_lot::RwLock::new(false),
            drawer_open: parking_lot::RwLock::new(false),
            transcript: parking_lot::RwLock::new(
                "VOXY AI Operating Companion initialized. Standing by for voice or camera interaction.".to_string(),
            ),
            sensor_gesture: parking_lot::RwLock::new("Camera Online".to_string()),
            turn_count: parking_lot::RwLock::new(0),
            messages: parking_lot::RwLock::new(vec![
                ChatMessage {
                    id: 1,
                    sender: "VOXY CORE".into(),
                    text: "System initialized. Audio capture and speech pipeline active.".into(),
                    timestamp: "Startup".into(),
                    is_user: false,
                }
            ]),
            tool_steps: parking_lot::RwLock::new(vec![
                ToolStep {
                    id: 1,
                    title: "Audio Stack Initialization".into(),
                    detail: "WASAPI audio device stream calibrated".into(),
                    status: StepStatus::Completed,
                }
            ]),
        }
    }
}

static APP_STATE: OnceLock<Arc<GlobalAppState>> = OnceLock::new();

fn get_app_state() -> Arc<GlobalAppState> {
    APP_STATE.get_or_init(|| Arc::new(GlobalAppState::new())).clone()
}

#[component]
fn App() -> Element {
    let global_state = get_app_state();
    let visual_state = use_signal(|| *global_state.visual_state.read());
    let desktop_mode = use_signal(|| *global_state.desktop_mode.read());
    let is_collapsed = use_signal(|| *global_state.is_collapsed.read());
    let drawer_open = use_signal(|| *global_state.drawer_open.read());
    let drawer_tab = use_signal(|| DrawerTab::Conversation);
    let transcript = use_signal(|| global_state.transcript.read().clone());
    let sensor_gesture = use_signal(|| global_state.sensor_gesture.read().clone());
    let turn_count = use_signal(|| *global_state.turn_count.read());
    let messages = use_signal(|| global_state.messages.read().clone());
    let tool_steps = use_signal(|| global_state.tool_steps.read().clone());
    let telemetry = use_signal(SystemTelemetry::default);

    // Listen to local EventBus for voice pipeline transitions
    use_effect(move || {
        let vs = visual_state.clone();
        let ts = transcript.clone();
        let tc = turn_count.clone();
        let msgs = messages.clone();
        let gs = global_state.clone();

        spawn(async move {
            let bus = Arc::new(EventBus::new(256));

            // 1. Wake event -> Listening
            if let Ok(mut rx) = bus.subscribe("voice.wake").await {
                let gs_c = gs.clone();
                let mut vs_c = vs.clone();
                let mut ts_c = ts.clone();
                spawn(async move {
                    while let Ok(_event) = rx.recv().await {
                        *gs_c.visual_state.write() = VisualState::Listening;
                        vs_c.set(VisualState::Listening);
                        ts_c.set("Listening to microphone...".to_string());
                    }
                });
            }

            // 2. STT Final -> Thinking
            if let Ok(mut rx) = bus.subscribe("stt.final").await {
                let gs_c = gs.clone();
                let mut vs_c = vs.clone();
                let mut ts_c = ts.clone();
                let mut tc_c = tc.clone();
                let mut msgs_c = msgs.clone();
                spawn(async move {
                    while let Ok(event) = rx.recv().await {
                        let text = String::from_utf8_lossy(event.payload()).to_string();
                        *gs_c.visual_state.write() = VisualState::Thinking;
                        let count = *tc_c.read() + 1;
                        *tc_c.write() = count;

                        let new_msg = ChatMessage {
                            id: count * 2,
                            sender: "USER".into(),
                            text: text.clone(),
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            is_user: true,
                        };
                        gs_c.messages.write().push(new_msg.clone());
                        msgs_c.write().push(new_msg);

                        vs_c.set(VisualState::Thinking);
                        ts_c.set(format!("User: \"{}\"", text));
                    }
                });
            }

            // 3. LLM Response -> Speaking
            if let Ok(mut rx) = bus.subscribe("llm.response").await {
                let gs_c = gs.clone();
                let mut vs_c = vs.clone();
                let mut ts_c = ts.clone();
                let mut msgs_c = msgs.clone();
                spawn(async move {
                    while let Ok(event) = rx.recv().await {
                        let text = String::from_utf8_lossy(event.payload()).to_string();
                        *gs_c.visual_state.write() = VisualState::Speaking;
                        vs_c.set(VisualState::Speaking);
                        ts_c.set(format!("VOXY: \"{}\"", text));

                        let new_msg = ChatMessage {
                            id: msgs_c.read().len() + 1,
                            sender: "VOXY".into(),
                            text: text.clone(),
                            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                            is_user: false,
                        };
                        gs_c.messages.write().push(new_msg.clone());
                        msgs_c.write().push(new_msg);
                    }
                });
            }

            // 4. Barge-in cutoff -> Acoustic interruption
            if let Ok(mut rx) = bus.subscribe("voice.barge_in").await {
                let gs_c = gs.clone();
                let mut vs_c = vs.clone();
                let mut ts_c = ts.clone();
                spawn(async move {
                    while let Ok(_event) = rx.recv().await {
                        *gs_c.visual_state.write() = VisualState::Error;
                        vs_c.set(VisualState::Error);
                        ts_c.set("Acoustic barge-in detected. Listening to speaker...".to_string());
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                        *gs_c.visual_state.write() = VisualState::Listening;
                        vs_c.set(VisualState::Listening);
                    }
                });
            }
        });
    });

    let profile_var = std::env::var("VOXY_PROFILE").unwrap_or_else(|_| "production".into());
    let (identity_name, profile_label) = if profile_var.to_lowercase() == "pro" {
        ("VOXY // PRO", "PROFILE: PROFESSIONAL")
    } else {
        ("VOXY // COM", "PROFILE: PRODUCTION")
    };

    let cur_mode = *desktop_mode.read();
    let cur_visual_state = *visual_state.read();
    let collapsed = *is_collapsed.read();
    let drawer_is_open = *drawer_open.read();

    let root_class = if collapsed { "voxy-app-root collapsed" } else { "voxy-app-root" };

    let toggle_spatial = move |_| {
        let mut ic = is_collapsed.clone();
        let nxt = !*ic.read();
        *get_app_state().is_collapsed.write() = nxt;
        ic.set(nxt);
    };

    let toggle_drawer = move |_| {
        let mut dr = drawer_open.clone();
        let nxt = !*dr.read();
        *get_app_state().drawer_open.write() = nxt;
        dr.set(nxt);
    };

    let switch_to_full = move |_| {
        let mut dm = desktop_mode.clone();
        *get_app_state().desktop_mode.write() = DesktopMode::FullExperience;
        dm.set(DesktopMode::FullExperience);
    };

    rsx! {
        div { id: "app-root", class: "{root_class}",
            match cur_mode {
                DesktopMode::Compact => rsx! {
                    CompactModeView {
                        visual_state: cur_visual_state,
                        transcript: transcript.read().clone(),
                        on_switch_full: switch_to_full,
                    }
                },
                DesktopMode::EdgeDock => rsx! {
                    EdgeDockView {
                        visual_state: cur_visual_state,
                        transcript: transcript.read().clone(),
                        on_switch_full: switch_to_full,
                    }
                },
                DesktopMode::Minimal => rsx! {
                    MinimalView {
                        visual_state: cur_visual_state,
                        on_switch_full: switch_to_full,
                    }
                },
                DesktopMode::FullExperience | DesktopMode::Floating | DesktopMode::ComputerControl => rsx! {
                    // Layer 2: Context Top Bar
                    ContextTopBar {
                        identity_name: identity_name.to_string(),
                        turn_count: *turn_count.read(),
                        sensor_gesture: sensor_gesture.read().clone(),
                        drawer_open: drawer_is_open,
                        on_toggle_drawer: toggle_drawer,
                    }

                    // Layer 1: Presence Living Core
                    CoreEntity {
                        visual_state: cur_visual_state,
                        is_collapsed: collapsed,
                        identity_name: identity_name.to_string(),
                        profile_label: profile_label.to_string(),
                        on_toggle: toggle_spatial,
                    }

                    // Computer Control Active Banner
                    div { style: "width: 100%; max-width: 1320px;",
                        ComputerControlBanner {
                            target_app: String::from("Active Desktop Environment"),
                            current_action: String::from("Standing by for user command"),
                            is_visible: cur_visual_state == VisualState::Executing || cur_mode == DesktopMode::ComputerControl,
                        }
                    }

                    // Layer 2: Context Bottom Transcript Strip
                    ContextBottomBar {
                        visual_state: cur_visual_state,
                        transcript: transcript.read().clone(),
                    }

                    // Layer 3: Depth Drawer (Expandable Secondary Surface)
                    DepthDrawer {
                        is_open: drawer_is_open,
                        active_tab: drawer_tab,
                        messages: messages.read().clone(),
                        tool_steps: tool_steps.read().clone(),
                        telemetry: telemetry.read().clone(),
                        current_mode: desktop_mode,
                        on_close: toggle_drawer,
                    }
                },
            }
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting VOXY AI Operating Companion Overlay v{}", env!("CARGO_PKG_VERSION"));

    let _state = APP_STATE.get_or_init(|| Arc::new(GlobalAppState::new()));

    let head_content = format!("{}{}", GLOBAL_STYLES, CLIENT_JS);

    let cfg = dioxus::desktop::Config::new()
        .with_custom_head(head_content)
        .with_window(
            dioxus::desktop::WindowBuilder::new()
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top(true)
                .with_maximized(true)
                .with_title("VOXY AI Operating Companion")
        );

    dioxus::LaunchBuilder::desktop().with_cfg(cfg).launch(App);
}
