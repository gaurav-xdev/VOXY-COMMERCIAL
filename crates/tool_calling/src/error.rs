//! Tool error types.

use thiserror::Error;

/// Tool error type.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Confirmation required for action: {0}")]
    ConfirmationRequired(String),

    #[error("Execution timed out after {0}")]
    Timeout(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("IO error: {0}")]
    IoError(String),
}

pub type Result<T> = std::result::Result<T, ToolError>;
