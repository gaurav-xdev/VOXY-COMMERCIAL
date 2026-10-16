#[derive(Debug, thiserror::Error)]
pub enum ConversationError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Session already exists: {0}")]
    SessionAlreadyExists(String),
    #[error("Session not active: {0}")]
    SessionNotActive(String),
    #[error("Invalid state transition: from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },
    #[error("Turn error: {0}")]
    TurnError(String),
    #[error("Barge-in failed: {0}")]
    BargeInFailed(String),
    #[error("Interruption not allowed in current state")]
    InterruptionNotAllowed,
    #[error("Wake state error: {0}")]
    WakeStateError(String),
    #[error("Context error: {0}")]
    ContextError(String),
    #[error("Personality hook failed: {0}")]
    PersonalityHookFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error(transparent)]
    Personality(#[from] voxy_personality::PersonalityError),
}

pub type Result<T> = std::result::Result<T, ConversationError>;
