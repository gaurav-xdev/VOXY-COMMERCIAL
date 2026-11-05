use crate::components::assistant_hud::ChatTurn;
use crate::components::onboarding_wizard::OnboardingConfig;
use crate::views::settings::SettingsFormState;
use serde::{Deserialize, Serialize};

/// High-level typed commands dispatched from Dioxus frontend to backend services.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BackendCommand {
    LoadSettings,
    SaveSettings(SettingsFormState),
    SendPrompt {
        text: String,
    },
    EmergencyStop,
    GetChatHistory {
        limit: usize,
        offset: usize,
    },
    GetSystemTelemetry,
    CompleteOnboarding(OnboardingConfig),
    TestProviderConnection {
        provider: String,
        model: String,
        api_key: Option<String>,
    },
    ExportDiagnostics,
    PurgeStoredMemories,
}

/// Typed results returned from backend command execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BackendResult {
    SettingsLoaded(SettingsFormState),
    SettingsSaved,
    PromptResponse {
        text: String,
        conversation_id: String,
    },
    EmergencyStopTriggered,
    ChatHistory(Vec<ChatTurn>),
    SystemTelemetry {
        daemon_connected: bool,
        active_llm: String,
        cpu_usage_pct: f32,
        memory_used_mb: u64,
    },
    OnboardingCompleted,
    ProviderTested {
        success: bool,
        message: String,
        latency_ms: u64,
    },
    DiagnosticsExported {
        sanitized_report: String,
    },
    MemoriesPurged {
        count: usize,
    },
}

/// Domain errors produced during command validation, execution, and persistence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackendError {
    #[error("Input validation failed: {0}")]
    Validation(String),

    #[error("Persistence or database failure: {0}")]
    Storage(String),

    #[error("Cognitive reasoning engine failure: {0}")]
    Cognition(String),

    #[error("IPC communication failure: {0}")]
    Ipc(String),

    #[error("Runtime configuration failure: {0}")]
    Settings(String),

    #[error("Authentication or entitlement failure: {0}")]
    Auth(String),
}
