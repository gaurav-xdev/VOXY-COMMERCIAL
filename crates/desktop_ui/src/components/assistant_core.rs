use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum CompanionMode {
    Cat,
    Robot,
    ParticleOrb,
}

impl Default for CompanionMode {
    fn default() -> Self {
        Self::Cat
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum CoreState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Error,
    Offline,
}

impl CoreState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Listening => "Listening",
            Self::Thinking => "Thinking",
            Self::Speaking => "Speaking",
            Self::Error => "Error",
            Self::Offline => "Offline",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "READY // DESKTOP COMPANION ACTIVE",
            Self::Listening => "LISTENING // AUDIO STREAM ENGAGED",
            Self::Thinking => "COGNITIVE REASONING ACTIVE",
            Self::Speaking => "DISPATCHING SYNTHESIZED SPEECH",
            Self::Error => "SUBSYSTEM ATTENTION REQUIRED",
            Self::Offline => "CORE DISCONNECTED",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            Self::Idle => "#94a3b8",
            Self::Listening => "#10b981",
            Self::Thinking => "#f59e0b",
            Self::Speaking => "#38bdf8",
            Self::Error => "#ef4444",
            Self::Offline => "#64748b",
        }
    }
}

/// Retired Three.js script; retained as empty string for backward compatibility.
pub const THREE_JS_CORE_SCRIPT: &str = "";

#[component]
pub fn AssistantCore(state: CoreState, mode: CompanionMode) -> Element {
    let _state_str = state.as_str();
    let state_color = state.color();
    let state_label = state.label();

    rsx! {
        div {
            id: "osmoo-viewport-container",
            style: "width: 100%; height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; pointer-events: none;",

            // Main Dashboard Workspace Companion Status Card
            div {
                class: "workspace-companion-banner",
                style: "display: flex; flex-direction: column; align-items: center; text-align: center; gap: 14px; max-width: 580px; padding: 32px 40px; background: rgba(15, 23, 42, 0.65); backdrop-filter: blur(20px); border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 20px; box-shadow: 0 20px 50px rgba(0, 0, 0, 0.4);",

                // Compact Companion Identity Pill
                div {
                    style: "display: inline-flex; align-items: center; gap: 8px; padding: 6px 14px; background: rgba(255, 255, 255, 0.04); border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 9999px;",
                    span {
                        style: "width: 8px; height: 8px; border-radius: 50%; background-color: {state_color}; box-shadow: 0 0 10px {state_color};",
                    }
                    span {
                        style: "font-family: var(--font-mono); font-size: 11px; font-weight: 700; letter-spacing: 1px; color: {state_color};",
                        "{state_label}"
                    }
                }

                // Companion Hero Header
                div {
                    h2 {
                        style: "font-family: var(--font-sans); font-size: 24px; font-weight: 700; color: #f8fafc; letter-spacing: -0.5px; margin-bottom: 6px;",
                        "OSMOO Cat Companion"
                    }
                    p {
                        style: "font-size: 14px; color: #94a3b8; line-height: 1.5;",
                        "Standing by directly on your Windows desktop. Closing or minimizing this window leaves the Cat companion active above your desktop wallpaper."
                    }
                }

                // Desktop Status Indicators
                div {
                    style: "display: flex; align-items: center; gap: 20px; margin-top: 8px;",
                    div {
                        style: "display: flex; align-items: center; gap: 6px; font-size: 12px; color: #cbd5e1;",
                        span { "🐱 2D Animated Overlay" }
                    }
                    div {
                        style: "display: flex; align-items: center; gap: 6px; font-size: 12px; color: #cbd5e1;",
                        span { "🎙️ Real-time Audio Lip Sync" }
                    }
                    div {
                        style: "display: flex; align-items: center; gap: 6px; font-size: 12px; color: #cbd5e1;",
                        span { "🖱️ Draggable Desktop Window" }
                    }
                }
            }
        }
    }
}
