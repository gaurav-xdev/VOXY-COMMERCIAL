use thiserror::Error;

#[derive(Debug, Error)]
pub enum OfficeError {
    #[error("Permission denied: required scope '{required}' is not granted in session")]
    PermissionDenied { required: String },

    #[error("Path traversal security violation: path '{path}' escapes allowed root '{root}'")]
    PathTraversalAttempt { path: String, root: String },

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid data format: {0}")]
    InvalidData(String),

    #[error("Scheduling conflict: event overlaps with '{conflict_title}' from {start} to {end}")]
    ConflictDetected {
        conflict_title: String,
        start: String,
        end: String,
    },

    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    #[error("I/O error: {0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, OfficeError>;
