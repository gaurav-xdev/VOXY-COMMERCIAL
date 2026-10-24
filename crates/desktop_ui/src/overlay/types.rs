use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualState {
    Idle,
    Listening,
    Processing,
    Thinking,
    Speaking,
    Executing,
    WaitingForUser,
    Success,
    Error,
    Offline,
    Disabled,
}

impl VisualState {
    pub fn label(&self) -> &'static str {
        match self {
            VisualState::Idle => "IDLE",
            VisualState::Listening => "LISTENING",
            VisualState::Processing => "PROCESSING",
            VisualState::Thinking => "THINKING",
            VisualState::Speaking => "SPEAKING",
            VisualState::Executing => "EXECUTING",
            VisualState::WaitingForUser => "AWAITING CONFIRMATION",
            VisualState::Success => "COMPLETED",
            VisualState::Error => "SYSTEM ALERT",
            VisualState::Offline => "OFFLINE",
            VisualState::Disabled => "MUTED",
        }
    }

    pub fn color(&self) -> &'static str {
        match self {
            VisualState::Idle => "#94a3b8",
            VisualState::Listening => "#10b981",
            VisualState::Processing => "#38bdf8",
            VisualState::Thinking => "#f59e0b",
            VisualState::Speaking => "#0ea5e9",
            VisualState::Executing => "#f97316",
            VisualState::WaitingForUser => "#eab308",
            VisualState::Success => "#22c55e",
            VisualState::Error => "#ef4444",
            VisualState::Offline => "#64748b",
            VisualState::Disabled => "#475569",
        }
    }

    pub fn bg_tint(&self) -> &'static str {
        match self {
            VisualState::Idle => "rgba(148, 163, 184, 0.08)",
            VisualState::Listening => "rgba(16, 185, 129, 0.12)",
            VisualState::Processing => "rgba(56, 189, 248, 0.12)",
            VisualState::Thinking => "rgba(245, 158, 11, 0.12)",
            VisualState::Speaking => "rgba(14, 165, 233, 0.12)",
            VisualState::Executing => "rgba(249, 115, 22, 0.15)",
            VisualState::WaitingForUser => "rgba(234, 179, 8, 0.15)",
            VisualState::Success => "rgba(34, 197, 94, 0.12)",
            VisualState::Error => "rgba(239, 68, 68, 0.15)",
            VisualState::Offline => "rgba(100, 116, 139, 0.06)",
            VisualState::Disabled => "rgba(71, 85, 105, 0.06)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DesktopMode {
    FullExperience,
    Compact,
    Floating,
    EdgeDock,
    Minimal,
    ComputerControl,
}

impl DesktopMode {
    pub fn label(&self) -> &'static str {
        match self {
            DesktopMode::FullExperience => "Full Presence",
            DesktopMode::Compact => "Compact Capsule",
            DesktopMode::Floating => "Floating Orb",
            DesktopMode::EdgeDock => "Edge Dock",
            DesktopMode::Minimal => "Minimal",
            DesktopMode::ComputerControl => "Computer Control",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: usize,
    pub sender: String,
    pub text: String,
    pub timestamp: String,
    pub is_user: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolStep {
    pub id: usize,
    pub title: String,
    pub detail: String,
    pub status: StepStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SystemTelemetry {
    pub audio_driver: String,
    pub sample_rate: String,
    pub llm_model: String,
    pub stt_provider: String,
    pub tts_provider: String,
    pub barge_in_status: String,
    pub stt_latency_ms: f32,
    pub tts_latency_ms: f32,
    pub turn_count: usize,
    pub mic_rms: f32,
    pub output_rms: f32,
}

impl Default for SystemTelemetry {
    fn default() -> Self {
        Self {
            audio_driver: "Windows WASAPI (Low-Latency Shared)".into(),
            sample_rate: "48000 Hz / Stereo Array".into(),
            llm_model: "Ollama Cloud (gpt-oss:120b-cloud)".into(),
            stt_provider: "Groq Whisper Large v3 Turbo".into(),
            tts_provider: "Voxray Hybrid Engine".into(),
            barge_in_status: "Active (Acoustic Shield Gated)".into(),
            stt_latency_ms: 182.0,
            tts_latency_ms: 215.0,
            turn_count: 0,
            mic_rms: 0.0,
            output_rms: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_eleven_visual_states() {
        let states = [
            VisualState::Idle,
            VisualState::Listening,
            VisualState::Processing,
            VisualState::Thinking,
            VisualState::Speaking,
            VisualState::Executing,
            VisualState::WaitingForUser,
            VisualState::Success,
            VisualState::Error,
            VisualState::Offline,
            VisualState::Disabled,
        ];

        assert_eq!(states.len(), 11);
        for s in states {
            assert!(!s.label().is_empty());
            assert!(s.color().starts_with('#'));
            assert!(s.bg_tint().starts_with("rgba"));
        }
    }

    #[test]
    fn test_all_six_desktop_modes() {
        let modes = [
            DesktopMode::FullExperience,
            DesktopMode::Compact,
            DesktopMode::Floating,
            DesktopMode::EdgeDock,
            DesktopMode::Minimal,
            DesktopMode::ComputerControl,
        ];

        assert_eq!(modes.len(), 6);
        for m in modes {
            assert!(!m.label().is_empty());
        }
    }

    #[test]
    fn test_system_telemetry_defaults_and_serialization() {
        let telem = SystemTelemetry::default();
        assert_eq!(telem.audio_driver, "Windows WASAPI (Low-Latency Shared)");
        assert_eq!(telem.sample_rate, "48000 Hz / Stereo Array");

        let json = serde_json::to_string(&telem).expect("serialization failed");
        let decoded: SystemTelemetry = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(decoded.llm_model, telem.llm_model);
        assert_eq!(decoded.stt_provider, telem.stt_provider);
    }

    #[test]
    fn test_tool_step_serialization() {
        let step = ToolStep {
            id: 42,
            title: "Launch Browser".into(),
            detail: "Open https://example.com".into(),
            status: StepStatus::Running,
        };

        let json = serde_json::to_string(&step).expect("serialization failed");
        let decoded: ToolStep = serde_json::from_str(&json).expect("deserialization failed");
        assert_eq!(decoded.id, 42);
        assert_eq!(decoded.status, StepStatus::Running);
    }
}
