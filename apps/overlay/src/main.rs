mod types;
mod styles;
mod client_js;
mod components;

use dioxus::prelude::*;
use std::sync::Arc;
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;

use types::{ChatMessage, DesktopMode, StepStatus, SystemTelemetry, ToolStep, VisualState};
use styles::GLOBAL_STYLES;
use client_js::CLIENT_JS;
use components::core_entity::CoreEntity;
use components::context_layer::{ContextBottomBar, ContextTopBar};
use components::computer_control::{ComputerControlBanner, VoxyCursorBeacon};
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

static IPC_CLIENT: OnceLock<Arc<voxy_ipc::VoxyIpcClient>> = OnceLock::new();

pub fn get_ipc_client() -> Arc<voxy_ipc::VoxyIpcClient> {
    IPC_CLIENT
        .get_or_init(|| {
            let client = Arc::new(voxy_ipc::VoxyIpcClient::new());
            client.start();
            client
        })
        .clone()
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
    let cursor_telem = use_signal(|| None::<voxy_ipc::VoxyCursorTelemetry>);

    // Listen to VOXY Daemon via native Windows Named Pipe IPC
    use_effect(move || {
        let vs = visual_state.clone();
        let ts = transcript.clone();
        let tc = turn_count.clone();
        let msgs = messages.clone();
        let t_steps = tool_steps.clone();
        let mut telem = telemetry.clone();
        let gs = global_state.clone();
        let mut ct_mut = cursor_telem.clone();

        spawn(async move {
            let client = get_ipc_client();
            let mut event_rx = client.subscribe();
            let conn_watch = client.watch_connected();

            // Background task watching connection state
            {
                let mut vs_conn = vs.clone();
                let mut ts_conn = ts.clone();
                let mut conn_rx = conn_watch.clone();
                spawn(async move {
                    while conn_rx.changed().await.is_ok() {
                        let is_conn = *conn_rx.borrow();
                        let _ = document::eval(&format!(
                            "if (window.voxySetConnectionStatus) window.voxySetConnectionStatus({});",
                            is_conn
                        ));
                        if !is_conn {
                            vs_conn.set(VisualState::Offline);
                            ts_conn.set("Connecting to VOXY Daemon...".to_string());
                        }
                    }
                });
            }

            // Main event dispatch loop
            while let Ok(daemon_msg) = event_rx.recv().await {
                match daemon_msg {
                    voxy_ipc::DaemonMessage::StateSnapshot {
                        voice_state,
                        transcript: snap_t,
                        emergency_stopped,
                        ..
                    } => {
                        let state = match voice_state {
                            voxy_ipc::VoiceState::Idle => VisualState::Idle,
                            voxy_ipc::VoiceState::Listening => VisualState::Listening,
                            voxy_ipc::VoiceState::Thinking => VisualState::Thinking,
                            voxy_ipc::VoiceState::Speaking => VisualState::Speaking,
                            voxy_ipc::VoiceState::Interrupted | voxy_ipc::VoiceState::Error => {
                                VisualState::Error
                            }
                        };
                        let state = if emergency_stopped {
                            VisualState::Error
                        } else {
                            state
                        };
                        *gs.visual_state.write() = state;
                        let mut vs_mut = vs.clone();
                        vs_mut.set(state);
                        *gs.transcript.write() = snap_t.clone();
                        let mut ts_mut = ts.clone();
                        ts_mut.set(snap_t);
                    }
                    voxy_ipc::DaemonMessage::VoiceStateChanged { state, .. } => {
                        let v_state = match state {
                            voxy_ipc::VoiceState::Idle => VisualState::Idle,
                            voxy_ipc::VoiceState::Listening => VisualState::Listening,
                            voxy_ipc::VoiceState::Thinking => VisualState::Thinking,
                            voxy_ipc::VoiceState::Speaking => VisualState::Speaking,
                            voxy_ipc::VoiceState::Interrupted | voxy_ipc::VoiceState::Error => {
                                VisualState::Error
                            }
                        };
                        *gs.visual_state.write() = v_state;
                        let mut vs_mut = vs.clone();
                        vs_mut.set(v_state);
                        let _ = document::eval(&format!(
                            "if (window.voxySetVisualState) window.voxySetVisualState('{}');",
                            v_state.label()
                        ));
                    }
                    voxy_ipc::DaemonMessage::TranscriptUpdate { text, .. } => {
                        *gs.transcript.write() = text.clone();
                        let mut ts_mut = ts.clone();
                        ts_mut.set(text.clone());
                        let json_text = serde_json::to_string(&text).unwrap_or_default();
                        let _ = document::eval(&format!(
                            "if (window.voxySetTranscript) window.voxySetTranscript({});",
                            json_text
                        ));
                    }
                    voxy_ipc::DaemonMessage::AudioEnergy {
                        mic_rms,
                        output_rms,
                    } => {
                        telem.write().mic_rms = mic_rms;
                        telem.write().output_rms = output_rms;
                        let _ = document::eval(&format!(
                            "if (window.voxyUpdateAudioEnergy) window.voxyUpdateAudioEnergy({}, {});",
                            mic_rms, output_rms
                        ));
                    }
                    voxy_ipc::DaemonMessage::ToolStep {
                        id,
                        title,
                        detail,
                        status,
                        ..
                    } => {
                        let step_status = match status {
                            voxy_ipc::ToolStepStatus::Running => StepStatus::Running,
                            voxy_ipc::ToolStepStatus::Completed => StepStatus::Completed,
                            voxy_ipc::ToolStepStatus::Failed => StepStatus::Failed,
                        };
                        let new_step = ToolStep {
                            id: id as usize,
                            title,
                            detail,
                            status: step_status,
                        };
                        let mut steps = t_steps.clone();
                        let mut found = false;
                        for s in steps.write().iter_mut() {
                            if s.id == new_step.id {
                                *s = new_step.clone();
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            steps.write().push(new_step);
                        }
                        if status == voxy_ipc::ToolStepStatus::Running {
                            let mut vs_mut = vs.clone();
                            vs_mut.set(VisualState::Executing);
                        }
                    }
                    voxy_ipc::DaemonMessage::ChatMessage {
                        id,
                        sender,
                        text,
                        timestamp,
                        is_user,
                    } => {
                        let chat_msg = ChatMessage {
                            id: id as usize,
                            sender,
                            text,
                            timestamp,
                            is_user,
                        };
                        let mut m_mut = msgs.clone();
                        m_mut.write().push(chat_msg.clone());
                        gs.messages.write().push(chat_msg);
                        let count = *tc.read() + 1;
                        let mut tc_mut = tc.clone();
                        tc_mut.set(count);
                    }
                    voxy_ipc::DaemonMessage::Interrupted { .. } => {
                        let mut vs_mut = vs.clone();
                        vs_mut.set(VisualState::Error);
                        let mut ts_mut = ts.clone();
                        ts_mut.set("Interrupted by user speech...".to_string());
                        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                        vs_mut.set(VisualState::Listening);
                    }
                    voxy_ipc::DaemonMessage::EmergencyStopChanged { is_stopped } => {
                        if is_stopped {
                            let mut vs_mut = vs.clone();
                            vs_mut.set(VisualState::Error);
                            let mut ts_mut = ts.clone();
                            ts_mut.set(
                                "EMERGENCY STOP ACTIVATED: All computer control halted.".to_string(),
                            );
                        }
                    }
                    voxy_ipc::DaemonMessage::RoutingStatusUpdate {
                        mode,
                        active_llm,
                        active_stt,
                        active_tts,
                        is_offline,
                    } => {
                        let mode_json = serde_json::to_string(&mode).unwrap_or_default();
                        let llm_json = serde_json::to_string(&active_llm).unwrap_or_default();
                        let stt_json = serde_json::to_string(&active_stt).unwrap_or_default();
                        let tts_json = serde_json::to_string(&active_tts).unwrap_or_default();
                        let _ = document::eval(&format!(
                            "if (window.voxySetRoutingStatus) window.voxySetRoutingStatus({}, {}, {}, {}, {});",
                            mode_json, llm_json, stt_json, tts_json, is_offline
                        ));
                    }
                    voxy_ipc::DaemonMessage::HardwareStatusUpdate {
                        cpu_brand,
                        cpu_cores,
                        ram_gb,
                        gpu_name,
                        vram_gb,
                    } => {
                        let cpu_json = serde_json::to_string(&cpu_brand).unwrap_or_default();
                        let gpu_str = gpu_name.unwrap_or_else(|| "CPU Only".into());
                        let gpu_json = serde_json::to_string(&gpu_str).unwrap_or_default();
                        let _ = document::eval(&format!(
                            "if (window.voxySetHardwareStatus) window.voxySetHardwareStatus({}, {}, {:.1}, {}, {:.1});",
                            cpu_json, cpu_cores, ram_gb, gpu_json, vram_gb
                        ));
                    }
                    voxy_ipc::DaemonMessage::CursorUpdate(telem_data) => {
                        let is_active = telem_data.state != voxy_ipc::VoxyCursorState::Idle;
                        let desc = telem_data.action_description.clone();
                        let elem = telem_data.target_element.clone();
                        let elem_json = serde_json::to_string(&elem).unwrap_or_default();
                        let desc_json = serde_json::to_string(&desc).unwrap_or_default();
                        let state_str = format!("{:?}", telem_data.state);
                        let state_json = serde_json::to_string(&state_str).unwrap_or_default();
                        ct_mut.set(Some(telem_data.clone()));
                        let _ = document::eval(&format!(
                            "if (window.voxySetCursorTelemetry) window.voxySetCursorTelemetry({}, {}, {}, {}, {}, {}, {}, {});",
                            telem_data.x, telem_data.y, state_json, elem_json, desc_json, telem_data.confidence, telem_data.requires_confirmation, telem_data.action_id.unwrap_or(0)
                        ));
                        if is_active && telem_data.requires_confirmation {
                            let mut vs_mut = vs.clone();
                            vs_mut.set(VisualState::Thinking);
                        }
                    }
                    _ => {}
                }
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
            if let Some(ct) = cursor_telem.read().as_ref() {
                VoxyCursorBeacon {
                    x: ct.x,
                    y: ct.y,
                    state_name: format!("{:?}", ct.state),
                    target_element: ct.target_element.clone(),
                    action_description: ct.action_description.clone(),
                    confidence: ct.confidence,
                    requires_confirmation: ct.requires_confirmation,
                    action_id: ct.action_id,
                    is_active: ct.state != voxy_ipc::VoxyCursorState::Idle,
                }
            }
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };
        eprintln!("[FATAL PANIC] Overlay crashed at {location}: {message}");
        tracing::error!("[FATAL PANIC] Overlay crashed at {location}: {message}");
    }));

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
