use dioxus::prelude::*;

use crate::bridge::LocalModelDiscovery;
use crate::components::{
    AssistantCore, AssistantHud, ChatTurn, CoreState, OnboardingConfig, OnboardingWizard,
    StartupSplash, TelemetryStats,
};
use crate::views::{SettingsFormState, SettingsView};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AppScreen {
    Splash,
    Onboarding,
    MainHud,
}

#[component]
pub fn App() -> Element {
    let mut screen = use_signal(|| AppScreen::Splash);
    let mut splash_progress = use_signal(|| 0.15f32);
    let mut splash_status = use_signal(|| "INITIALIZING HARDWARE SUBSYSTEMS...".to_string());

    let mut core_state = use_signal(|| CoreState::Idle);
    let mut mic_active = use_signal(|| true);
    let mut show_settings = use_signal(|| false);

    let mut chat_history = use_signal(|| {
        vec![ChatTurn {
            id: 1,
            sender: "OSMOO Core".to_string(),
            text: "OSMOO Windows AI Operating Companion online. Ready for voice interaction."
                .to_string(),
            timestamp: "System Init".to_string(),
            is_user: false,
        }]
    });

    let mut telemetry = use_signal(|| TelemetryStats {
        mic_rms: 0.05,
        daemon_connected: false,
        audio_latency_ms: 12,
        active_llm: "Ollama (Local)".to_string(),
        active_stt: "Deepgram".to_string(),
        active_tts: "Cartesia".to_string(),
    });

    let mut discovered_models = use_signal(Vec::new);

    // Initial Startup Splash Telemetry Task
    use_effect(move || {
        let mut progress_sig = splash_progress;
        let mut status_sig = splash_status;
        let mut screen_sig = screen;
        let mut models_sig = discovered_models;

        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            progress_sig.set(0.35);
            status_sig.set("CONNECTING AUTHORITATIVE NAMED PIPE IPC...".to_string());

            // Query Ollama local models
            if let Ok(models) = LocalModelDiscovery::fetch_ollama_models().await {
                models_sig.set(models);
            }

            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            progress_sig.set(0.70);
            status_sig.set("CALIBRATING WASAPI AUDIO & ACOUSTIC MODELS...".to_string());

            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            progress_sig.set(1.0);
            status_sig.set("SYSTEM OPERATIONAL".to_string());

            tokio::time::sleep(std::time::Duration::from_millis(300)).await;

            // Check if onboarding was completed before
            let cfg_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("osmoo")
                .join("provisioned.flag");

            if cfg_path.exists() {
                screen_sig.set(AppScreen::MainHud);
            } else {
                screen_sig.set(AppScreen::Onboarding);
            }
        });
    });

    // Background IPC Listener
    use_effect(move || {
        let mut telem_sig = telemetry;
        let mut core_sig = core_state;
        let mut chat_sig = chat_history;

        spawn(async move {
            let bridge = crate::get_bridge();
            let mut rx = bridge.ipc.subscribe();
            let mut conn_rx = bridge.ipc.watch_connected();

            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        match msg {
                            voxy_ipc::DaemonMessage::VoiceStateChanged { state, .. } => {
                                let mapped = match state {
                                    voxy_ipc::VoiceState::Idle => CoreState::Idle,
                                    voxy_ipc::VoiceState::Listening => CoreState::Listening,
                                    voxy_ipc::VoiceState::Thinking => CoreState::Processing,
                                    voxy_ipc::VoiceState::Speaking => CoreState::Speaking,
                                    voxy_ipc::VoiceState::Interrupted => CoreState::Interrupted,
                                    voxy_ipc::VoiceState::Error => CoreState::Error,
                                };
                                core_sig.set(mapped);
                            }
                            voxy_ipc::DaemonMessage::AudioEnergy { mic_rms, .. } => {
                                let mut t = telem_sig.read().clone();
                                t.mic_rms = mic_rms;
                                telem_sig.set(t);
                            }
                            voxy_ipc::DaemonMessage::ChatMessage { id, sender, text, timestamp, is_user } => {
                                let mut list = chat_sig.read().clone();
                                list.push(ChatTurn { id, sender, text, timestamp, is_user });
                                chat_sig.set(list);
                            }
                            voxy_ipc::DaemonMessage::RoutingStatusUpdate { active_llm, active_stt, active_tts, .. } => {
                                let mut t = telem_sig.read().clone();
                                t.active_llm = active_llm;
                                t.active_stt = active_stt;
                                t.active_tts = active_tts;
                                telem_sig.set(t);
                            }
                            _ => {}
                        }
                    }
                    Ok(_) = conn_rx.changed() => {
                        let connected = *conn_rx.borrow();
                        let mut t = telem_sig.read().clone();
                        t.daemon_connected = connected;
                        telem_sig.set(t);
                    }
                }
            }
        });
    });

    // Handlers
    let on_toggle_voice_core = move |()| {
        let new_active = !*mic_active.read();
        mic_active.set(new_active);

        spawn(async move {
            let bridge = crate::get_bridge();
            if new_active {
                let _ = bridge.voice.start_listening().await;
                core_state.set(CoreState::Listening);
            } else {
                bridge.voice.stop_listening().await;
                core_state.set(CoreState::Idle);
            }
        });
    };

    let on_toggle_voice_hud = move |()| {
        let new_active = !*mic_active.read();
        mic_active.set(new_active);

        spawn(async move {
            let bridge = crate::get_bridge();
            if new_active {
                let _ = bridge.voice.start_listening().await;
                core_state.set(CoreState::Listening);
            } else {
                bridge.voice.stop_listening().await;
                core_state.set(CoreState::Idle);
            }
        });
    };

    let on_send_prompt = move |text: String| {
        let mut list = chat_history.read().clone();
        list.push(ChatTurn {
            id: list.len() as u64 + 1,
            sender: "User".to_string(),
            text: text.clone(),
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
            is_user: true,
        });
        chat_history.set(list);

        core_state.set(CoreState::Processing);

        spawn(async move {
            let bridge = crate::get_bridge();
            let _ = bridge
                .ipc
                .send_command(voxy_ipc::ClientCommand::SendTextInput { text: text.clone() })
                .await;

            let intent_input = voxy_cognition::IntentInput {
                raw_text: text,
                context: None,
                source: "desktop_ui_core".to_string(),
                metadata: std::collections::HashMap::new(),
            };

            match bridge.cognition.process(&intent_input).await {
                Ok(result) => {
                    core_state.set(CoreState::Speaking);
                    let response_text = serde_json::to_string_pretty(&result.result)
                        .unwrap_or_else(|_| format!("{:?}", result.result));

                    let mut l = chat_history.read().clone();
                    l.push(ChatTurn {
                        id: l.len() as u64 + 1,
                        sender: "OSMOO Core".to_string(),
                        text: response_text.clone(),
                        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
                        is_user: false,
                    });
                    chat_history.set(l);

                    let _ = bridge.voice.speak(&response_text).await;
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    core_state.set(CoreState::Idle);
                }
                Err(_) => {
                    core_state.set(CoreState::Error);
                    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                    core_state.set(CoreState::Idle);
                }
            }
        });
    };

    let on_onboarding_finish = move |cfg: OnboardingConfig| {
        // Persist autolaunch setting
        if cfg.autolaunch_enabled {
            let autolauncher = voxy_desktop_runtime::autolaunch::AutoLauncher::new("OSMOO");
            let _ = autolauncher.enable();
        }

        // Flag completion
        let cfg_dir = dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("osmoo");
        let _ = std::fs::create_dir_all(&cfg_dir);
        let _ = std::fs::write(cfg_dir.join("provisioned.flag"), "true");

        screen.set(AppScreen::MainHud);
    };

    let on_emergency_stop = move |()| {
        core_state.set(CoreState::Error);
        spawn(async move {
            let bridge = crate::get_bridge();
            let _ = bridge
                .ipc
                .send_command(voxy_ipc::ClientCommand::EmergencyStop)
                .await;
        });
    };

    rsx! {
        div { id: "main-app-container",
            match *screen.read() {
                AppScreen::Splash => rsx! {
                    StartupSplash {
                        progress: *splash_progress.read(),
                        status_text: splash_status.read().clone(),
                    }
                },
                AppScreen::Onboarding => rsx! {
                    OnboardingWizard {
                        on_finish: on_onboarding_finish,
                        initial_ollama_models: discovered_models.read().clone(),
                    }
                },
                AppScreen::MainHud => rsx! {
                    // Hero 3D Volumetric Assistant Core
                    AssistantCore {
                        state: *core_state.read(),
                        on_toggle_voice: on_toggle_voice_core,
                        mic_active: *mic_active.read(),
                    }

                    // Floating Glassmorphic HUD & Secondary Drawer
                    AssistantHud {
                        on_toggle_voice: on_toggle_voice_hud,
                        on_send_prompt: on_send_prompt,
                        on_open_settings: move |()| show_settings.set(true),
                        on_emergency_stop: on_emergency_stop,
                        mic_active: *mic_active.read(),
                        telemetry: telemetry.read().clone(),
                        chat_history: chat_history.read().clone(),
                    }

                    // System Preferences & Hardware Modal
                    if *show_settings.read() {
                        {
                            let bridge = crate::get_bridge();
                            let current_sess = bridge.auth.current_session();
                            let user_email = current_sess.as_ref().map(|s| s.email.clone()).unwrap_or_else(|| "guest@osmoo.in".to_string());
                            let active_tier = current_sess.as_ref().map(|s| s.active_tier.clone()).unwrap_or_else(|| "free".to_string());
                            let autolauncher = voxy_desktop_runtime::autolaunch::AutoLauncher::new("OSMOO");
                            let is_autolaunch = autolauncher.is_enabled();

                            let init_form = SettingsFormState {
                                assistant_name: "OSMOO Core".to_string(),
                                primary_llm: "ollama".to_string(),
                                ollama_model: discovered_models.read().first().cloned().unwrap_or_else(|| "llama3.2:3b".to_string()),
                                autolaunch: is_autolaunch,
                                stt_provider: "deepgram".to_string(),
                                tts_provider: "cartesia".to_string(),
                                audio_sample_rate: 48000,
                                vad_sensitivity: 0.5,
                                minimize_to_tray: true,
                                user_email,
                                active_tier,
                            };

                            rsx! {
                                SettingsView {
                                    on_close: move |()| show_settings.set(false),
                                    on_save: move |saved_settings: SettingsFormState| {
                                        let autolauncher = voxy_desktop_runtime::autolaunch::AutoLauncher::new("OSMOO");
                                        if saved_settings.autolaunch {
                                            let _ = autolauncher.enable();
                                        } else {
                                            let _ = autolauncher.disable();
                                        }
                                        show_settings.set(false);
                                    },
                                    initial_settings: init_form,
                                    ollama_models: discovered_models.read().clone(),
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
