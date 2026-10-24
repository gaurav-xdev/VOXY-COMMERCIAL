use thiserror::Error;

pub type Result<T> = std::result::Result<T, SwarmError>;

#[derive(Debug, Error)]
pub enum SwarmError {
    #[error("Unauthorized swarm access: {0}")]
    AccessDenied(String),

    #[error("Internal security token mismatch")]
    InvalidToken,

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Cyclic dependency detected in task graph: {0}")]
    CyclicDependency(String),

    #[error("Task execution failed for '{task_id}': {reason}")]
    TaskExecutionFailed {
        task_id: String,
        reason: String,
    },

    #[error("Canary check failed for task '{task_id}': {details}")]
    CanaryFailed {
        task_id: String,
        details: String,
    },

    #[error("Task '{0}' timed out")]
    Timeout(String),

    #[error("Swarm workflow was cancelled: {0}")]
    Cancelled(String),

    #[error("Invalid agent handoff from '{from}' to '{to}': {reason}")]
    InvalidHandoff {
        from: String,
        to: String,
        reason: String,
    },

    #[error("Model provider error: {0}")]
    ProviderError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Harness error: {0}")]
    HarnessError(String),
}
