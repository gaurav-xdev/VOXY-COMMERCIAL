use dioxus::prelude::*;

use crate::bridge::AppBridge;
use crate::components::voice_orb::{OrbState, VoiceOrb};

#[component]
pub fn OrbView() -> Element {
    let bridge = use_context::<AppBridge>();
    let orb_state = use_signal(|| OrbState::Idle);
    let mut is_listening = use_signal(|| false);
    let is_speaking = use_signal(|| false);
    let metrics = use_signal(|| (0u64, 0u64, 0u64));
    let mood_text = use_signal(|| String::from("Calm"));
    let presence_text = use_signal(|| String::from("Idle"));
    let daemon_connected = use_signal(|| false);

    // 1. Subscribe to real Named Pipe IPC Daemon events
    {
        let mut orb_state = orb_state;
        let mut daemon_connected = daemon_connected;
        let ipc = bridge.ipc.clone();

        spawn(async move {
            let mut event_rx = ipc.subscribe();
            let mut conn_rx = ipc.watch_connected();

            loop {
                tokio::select! {
                    Ok(_changed) = conn_rx.changed() => {
                        let is_conn = *conn_rx.borrow();
                        daemon_connected.set(is_conn);
                    }
                    Ok(msg) = event_rx.recv() => {
                        match msg {
                            voxy_ipc::DaemonMessage::VoiceStateChanged { state, .. } => {
                                let new_state = match state {
                                    voxy_ipc::VoiceState::Idle => OrbState::Idle,
                                    voxy_ipc::VoiceState::Listening => OrbState::Listening,
                                    voxy_ipc::VoiceState::Thinking => OrbState::Thinking,
                                    voxy_ipc::VoiceState::Speaking => OrbState::Speaking,
                                    voxy_ipc::VoiceState::Interrupted => OrbState::Idle,
                                    voxy_ipc::VoiceState::Error => OrbState::Error,
                                };
                                orb_state.set(new_state);
                            }
                            voxy_ipc::DaemonMessage::StateSnapshot { voice_state, .. } => {
                                let new_state = match voice_state {
                                    voxy_ipc::VoiceState::Idle => OrbState::Idle,
                                    voxy_ipc::VoiceState::Listening => OrbState::Listening,
                                    voxy_ipc::VoiceState::Thinking => OrbState::Thinking,
                                    voxy_ipc::VoiceState::Speaking => OrbState::Speaking,
                                    voxy_ipc::VoiceState::Interrupted => OrbState::Idle,
                                    voxy_ipc::VoiceState::Error => OrbState::Error,
                                };
                                orb_state.set(new_state);
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    }

    // 2. Poll ExperienceBridge as companion intelligence fallback/complement
    {
        let mut mood_text = mood_text;
        let mut presence_text = presence_text;
        let experience = bridge.experience.clone();

        spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                let snapshot = experience.get_snapshot().await;
                mood_text.set(format!("{:?}", snapshot.current_mood));
                presence_text.set(format!("{:?}", snapshot.presence.state));
            }
        });
    }

    let toggle_orb = {
        let voice = bridge.voice.clone();
        let experience_input = bridge.experience_input.clone();
        let ipc = bridge.ipc.clone();
        move |_: Event<MouseData>| {
            let current = *is_listening.read();
            is_listening.set(!current);
            let v = voice.clone();
            let mut state = orb_state;
            let listening = is_listening;
            let exp_input = experience_input.clone();
            let ipc_clone = ipc.clone();

            spawn(async move {
                if *listening.read() {
                    state.set(OrbState::Listening);
                    let _ = v.start_listening().await;
                    let _ = exp_input.send(
                        voxy_companion_intelligence::ExperienceInput::VoiceActivity {
                            active: true,
                        },
                    );
                } else {
                    v.stop_listening().await;
                    state.set(OrbState::Idle);
                    let _ = exp_input.send(
                        voxy_companion_intelligence::ExperienceInput::VoiceActivity {
                            active: false,
                        },
                    );
                    let _ = ipc_clone
                        .send_command(voxy_ipc::ClientCommand::InterruptSpeech)
                        .await;
                }
            });
        }
    };

    let speak_text = {
        let voice = bridge.voice.clone();
        let experience_input = bridge.experience_input.clone();
        move |_: Event<MouseData>| {
            let v = voice.clone();
            let mut speaking = is_speaking;
            let mut state = orb_state;
            let exp_input = experience_input.clone();
            spawn(async move {
                state.set(OrbState::Speaking);
                speaking.set(true);
                let _ = v.speak("Hello, I am VOXY. How can I help you today?").await;
                let _ = exp_input.send(voxy_companion_intelligence::ExperienceInput::SystemEvent {
                    event_type: "task_complete".to_string(),
                    data: Some("greeting_spoken".to_string()),
                });
                speaking.set(false);
                state.set(OrbState::Idle);
            });
        }
    };

    rsx! {
        div {
            style: "display: flex; flex-direction: column; align-items: center; justify-content: center; height: 100%; gap: 32px;",

            VoiceOrb { state: *orb_state.read(), on_click: None }

            div { style: "display: flex; gap: 12px;",
                button {
                    class: if *is_listening.read() { "voice-btn active" } else { "voice-btn" },
                    onclick: toggle_orb,
                    style: "width: 56px; height: 56px; font-size: 24px;",
                    if *is_listening.read() { "\u{23F9}" } else { "\u{1F3A4}" }
                }
                button {
                    class: "voice-btn",
                    style: "width: 56px; height: 56px; font-size: 24px;",
                    onclick: speak_text,
                    "\u{25B6}"
                }
            }

            div { style: "text-align: center;",
                div { style: "font-size: 14px; color: var(--text-secondary);",
                    if *is_listening.read() {
                        "Listening for your voice..."
                    } else if *is_speaking.read() {
                        "Speaking..."
                    } else {
                        "Click the microphone to start"
                    }
                }
                div { style: "font-size: 12px; color: var(--text-muted); margin-top: 8px; display: flex; align-items: center; justify-content: center; gap: 8px;",
                    span {
                        class: if *daemon_connected.read() { "badge badge-success" } else { "badge badge-warning" },
                        if *daemon_connected.read() { "Daemon: Connected" } else { "Daemon: Connecting..." }
                    }
                    span { "• Push-to-Talk: Hold Space" }
                }
            }

            div { style: "display: grid; grid-template-columns: repeat(3, 1fr); gap: 16px; width: 100%; max-width: 500px;",
                div { class: "card", style: "text-align: center; padding: 16px;",
                    div { style: "font-size: 20px; font-weight: 600; color: var(--success);", "{metrics.read().0}ms" }
                    div { style: "font-size: 11px; color: var(--text-muted);", "STT Latency" }
                }
                div { class: "card", style: "text-align: center; padding: 16px;",
                    div { style: "font-size: 20px; font-weight: 600; color: var(--info);", "{metrics.read().1}ms" }
                    div { style: "font-size: 11px; color: var(--text-muted);", "LLM First Token" }
                }
                div { class: "card", style: "text-align: center; padding: 16px;",
                    div { style: "font-size: 20px; font-weight: 600; color: var(--warning);", "{metrics.read().2}ms" }
                    div { style: "font-size: 11px; color: var(--text-muted);", "TTS First Chunk" }
                }
            }

            div { style: "display: grid; grid-template-columns: repeat(2, 1fr); gap: 16px; width: 100%; max-width: 500px; margin-top: 16px;",
                div { class: "card", style: "text-align: center; padding: 12px;",
                    div { style: "font-size: 16px; font-weight: 600; color: var(--accent);", "{mood_text.read()}" }
                    div { style: "font-size: 11px; color: var(--text-muted);", "Current Mood" }
                }
                div { class: "card", style: "text-align: center; padding: 12px;",
                    div { style: "font-size: 16px; font-weight: 600; color: var(--accent);", "{presence_text.read()}" }
                    div { style: "font-size: 11px; color: var(--text-muted);", "Presence State" }
                }
            }
        }
    }
}
