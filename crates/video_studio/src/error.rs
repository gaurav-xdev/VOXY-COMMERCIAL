use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, StudioError>;

#[derive(Debug, Error)]
pub enum StudioError {
    #[error("Unauthorized enclave access: {0}")]
    Unauthorized(String),

    #[error("Video studio enclave path not found: {0}")]
    EnclaveNotFound(PathBuf),

    #[error("Pipeline error: {0}")]
    PipelineError(String),

    #[error("Process execution error: {0}")]
    ProcessFailed(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
