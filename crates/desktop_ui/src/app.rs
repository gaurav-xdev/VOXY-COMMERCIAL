use dioxus::prelude::*;

use crate::bridge::LocalModelDiscovery;
use crate::components::{
    AssistantCore, AssistantHud, ChatTurn, CompanionMode, CoreState, OnboardingConfig,
    OnboardingWizard, StartupSplash, TelemetryStats, THREE_JS_CORE_SCRIPT,
};
use crate::views::{AuthView, SettingsFormState, SettingsView};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AppScreen {
    Splash,
    Auth,
    Onboarding,
    MainHud,
}

#[component]
pub fn App() -> Element {
    let mut screen = use_signal(|| AppScreen::Splash);
    let splash_progress = use_signal(|| 0.15f32);
    let splash_status = use_signal(|| "INITIALIZING OSMIORA CORE ARCHITECTURE...".to_string());

    let mut core_state = use_signal(|| CoreState::Idle);
    let mut companion_mode = use_signal(|| CompanionMode::Robot);
    let mut show_settings = use_signal(|| false);

    let mut chat_history = use_signal(|| {
        vec![ChatTurn {
            id: 1,
            sender: "OSMOO Core".to_string(),
            text: "OSMOO AI Operating Companion online. Ready for multimodal interaction."
                .to_string(),
            timestamp: "System Init".to_string(),
            is_user: false,
        }]
    });

    let telemetry = use_signal(|| TelemetryStats {
        daemon_connected: false,
        active_llm: "Ollama (Local Engine)".to_string(),
    });

    let discovered_models = use_signal(Vec::new);

    // Initial Startup Splash Telemetry Task
    use_effect(move || {
        let mut progress_sig = splash_progress;
        let mut status_sig = splash_status;
        let mut screen_sig = screen;
        let mut models_sig = discovered_models;
        let mut chat_sig = chat_history;
        let mut telem_sig = telemetry;

        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            progress_sig.set(0.40);
            status_sig.set("SYNCHRONIZING SECURE NAMED PIPE IPC & PERSISTENCE...".to_string());

            // Query Ollama local models
            if let Ok(models) = LocalModelDiscovery::fetch_ollama_models().await {
                models_sig.set(models);
            }

            tokio::time::sleep(std::time::Duration::from_millis(450)).await;
            progress_sig.set(0.75);
            status_sig.set("INITIALIZING 3D COMPANION ENGINE...".to_string());

            // Hydrate historical chat turns from SQLite if available
            if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                if let Ok(crate::backend::BackendResult::ChatHistory(turns)) = dispatcher
                    .dispatch(crate::backend::BackendCommand::GetChatHistory {
                        limit: 50,
                        offset: 0,
                    })
                    .await
                {
                    if !turns.is_empty() {
                        chat_sig.set(turns);
                    }
                }

                if let Ok(crate::backend::BackendResult::SystemTelemetry {
                    daemon_connected,
                    active_llm,
                    ..
                }) = dispatcher
                    .dispatch(crate::backend::BackendCommand::GetSystemTelemetry)
                    .await
                {
                    telem_sig.set(TelemetryStats {
                        daemon_connected,
                        active_llm,
                    });
                }
            }

            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            progress_sig.set(1.0);
            status_sig.set("SYSTEM OPERATIONAL".to_string());

            tokio::time::sleep(std::time::Duration::from_millis(300)).await;

            // Check if provisioning flag exists and if session is authenticated
            let cfg_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("osmoo")
                .join("provisioned.flag");

            let is_auth = crate::bridge::AppBridge::get_global()
                .map(|b| b.auth.is_authenticated())
                .unwrap_or(false);

            if !cfg_path.exists() {
                screen_sig.set(AppScreen::Onboarding);
            } else if !is_auth {
                screen_sig.set(AppScreen::Auth);
            } else {
                screen_sig.set(AppScreen::MainHud);
            }
        });
    });

    // Background IPC Listener
    use_effect(move || {
        let mut telem_sig = telemetry;
        let mut core_sig = core_state;
        let mut chat_sig = chat_history;

        spawn(async move {
            let bridge = crate::bridge::IpcBridge::new();
            let mut rx = bridge.subscribe();
            let mut conn_rx = bridge.watch_connected();

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            voxy_ipc::DaemonMessage::VoiceStateChanged { state, .. } => {
                                let mapped = match state {
                                    voxy_ipc::VoiceState::Listening => CoreState::Listening,
                                    voxy_ipc::VoiceState::Thinking => CoreState::Thinking,
                                    voxy_ipc::VoiceState::Speaking => CoreState::Speaking,
                                    voxy_ipc::VoiceState::Idle => CoreState::Idle,
                                    voxy_ipc::VoiceState::Error => CoreState::Error,
                                    voxy_ipc::VoiceState::Responding => CoreState::Speaking,
                                    voxy_ipc::VoiceState::Interrupted => CoreState::Idle,
                                };
                                core_sig.set(mapped);
                            }
                            voxy_ipc::DaemonMessage::TranscriptUpdate { text, is_final, .. } => {
                                if is_final && !text.trim().is_empty() {
                                    let next_id = chat_sig.read().len() as u64 + 1;
                                    chat_sig.write().push(ChatTurn {
                                        id: next_id,
                                        sender: "Operator".to_string(),
                                        text,
                                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                        is_user: true,
                                    });
                                }
                            }
                            voxy_ipc::DaemonMessage::ChatMessage { sender, text, timestamp, is_user, .. } => {
                                let next_id = chat_sig.read().len() as u64 + 1;
                                chat_sig.write().push(ChatTurn {
                                    id: next_id,
                                    sender,
                                    text,
                                    timestamp,
                                    is_user,
                                });
                            }
                            _ => {}
                        }
                    }
                    Ok(()) = conn_rx.changed() => {
                        let is_conn = *conn_rx.borrow();
                        telem_sig.write().daemon_connected = is_conn;
                        if !is_conn {
                            core_sig.set(CoreState::Offline);
                        } else {
                            core_sig.set(CoreState::Idle);
                        }
                    }
                }
            }
        });
    });

    rsx! {
        div { id: "main-app-container",
            style { {crate::styles::APP_CSS} }

            // Embedded Three.js Engine Scripts
            div { dangerous_inner_html: "{THREE_JS_CORE_SCRIPT}" }

            // ── Screen Rendering ────────────────────────────────────────
            match *screen.read() {
                AppScreen::Splash => rsx! {
                    StartupSplash {
                        progress: *splash_progress.read(),
                        status_text: splash_status.read().clone(),
                    }
                },
                AppScreen::Auth => rsx! {
                    AuthView {
                        on_success: move |_| screen.set(AppScreen::MainHud),
                        on_cancel: move |_| screen.set(AppScreen::MainHud),
                    }
                },
                AppScreen::Onboarding => rsx! {
                    OnboardingWizard {
                        initial_ollama_models: discovered_models.read().clone(),
                        on_finish: move |cfg: OnboardingConfig| {
                            companion_mode.set(cfg.companion_mode);
                            let cfg_clone = cfg.clone();
                            spawn(async move {
                                if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                    let _ = dispatcher.dispatch(crate::backend::BackendCommand::CompleteOnboarding(cfg_clone)).await;
                                }
                            });
                            screen.set(AppScreen::MainHud);
                        },
                    }
                },
                AppScreen::MainHud => rsx! {
                    // 3D Viewport Layer (Obsidian Bot & Starlight Orb)
                    AssistantCore {
                        state: *core_state.read(),
                        mode: *companion_mode.read(),
                    }

                    // Floating 2D HUD Controls & Telemetry
                    AssistantHud {
                        telemetry: telemetry.read().clone(),
                        chat_history: chat_history.read().clone(),
                        current_mode: *companion_mode.read(),
                        on_open_settings: move |_| show_settings.set(true),
                        on_toggle_mode: move |m: CompanionMode| companion_mode.set(m),
                        on_toggle_overlay: move |_| {
                            if let Some(sup) = crate::supervisor::get_global() {
                                let _ = sup.start_overlay();
                            }
                        },
                        on_emergency_stop: move |_| {
                            spawn(async move {
                                if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                    let _ = dispatcher.dispatch(crate::backend::BackendCommand::EmergencyStop).await;
                                }
                            });
                        },
                        on_send_prompt: move |text: String| {
                            let next_id = chat_history.read().len() as u64 + 1;
                            chat_history.write().push(ChatTurn {
                                id: next_id,
                                sender: "Operator".to_string(),
                                text: text.clone(),
                                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                is_user: true,
                            });
                            core_state.set(CoreState::Thinking);

                            let mut chat_sig = chat_history;
                            let mut core_sig = core_state;
                            spawn(async move {
                                if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                    match dispatcher.dispatch(crate::backend::BackendCommand::SendPrompt { text }).await {
                                        Ok(crate::backend::BackendResult::PromptResponse { text: resp, .. }) => {
                                            let resp_id = chat_sig.read().len() as u64 + 1;
                                            chat_sig.write().push(ChatTurn {
                                                id: resp_id,
                                                sender: "OSMOO Core".to_string(),
                                                text: resp,
                                                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                                is_user: false,
                                            });
                                            core_sig.set(CoreState::Speaking);
                                        }
                                        Err(e) => {
                                            let err_id = chat_sig.read().len() as u64 + 1;
                                            chat_sig.write().push(ChatTurn {
                                                id: err_id,
                                                sender: "OSMOO Core".to_string(),
                                                text: format!("[SYSTEM ERROR]: {}", e),
                                                timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                                                is_user: false,
                                            });
                                            core_sig.set(CoreState::Error);
                                        }
                                        _ => {
                                            core_sig.set(CoreState::Idle);
                                        }
                                    }
                                }
                            });
                        },
                    }

                    // Modal Preferences Drawer
                    if *show_settings.read() {
                        SettingsView {
                            initial_settings: {
                                if let Some(br) = crate::bridge::AppBridge::get_global() {
                                    let s = br.settings.get();
                                    SettingsFormState {
                                        assistant_name: s.app_name,
                                        primary_llm: s.models.provider,
                                        ollama_model: s.models.model,
                                        autolaunch: true,
                                        stt_provider: s.voice.stt_provider.unwrap_or_else(|| "groq".to_string()),
                                        tts_provider: s.voice.tts_provider.unwrap_or_else(|| "cartesia".to_string()),
                                        audio_sample_rate: s.voice.sample_rate,
                                        vad_sensitivity: s.voice.wake_word_sensitivity as f32,
                                        minimize_to_tray: true,
                                        user_email: br.auth.current_session().map(|sess| sess.email).unwrap_or_else(|| "operator@osmoo.in".to_string()),
                                        active_tier: br.auth.current_session().map(|sess| sess.active_tier).unwrap_or_else(|| "free".to_string()),
                                    }
                                } else {
                                    SettingsFormState::default()
                                }
                            },
                            ollama_models: discovered_models.read().clone(),
                            on_close: move |_| show_settings.set(false),
                            on_save: move |updated: SettingsFormState| {
                                spawn(async move {
                                    if let Some(dispatcher) = crate::backend::BackendDispatcher::get_global() {
                                        let _ = dispatcher.dispatch(crate::backend::BackendCommand::SaveSettings(updated)).await;
                                    }
                                });
                                show_settings.set(false);
                            },
                        }
                    }
                },
            }
        }
    }
}
