//! Error types for the VOXY platform.

use std::fmt;

/// Result type alias for VOXY operations.
pub type Result<T> = std::result::Result<T, VoxyError>;

/// The main error type for all VOXY operations.
#[derive(Debug)]
pub struct VoxyError {
    kind: ErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl VoxyError {
    /// Create a new error with a kind and message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Create a new error with a source.
    pub fn with_source(
        kind: ErrorKind,
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Create a new error with a boxed source.
    pub fn with_boxed_source(
        kind: ErrorKind,
        message: impl Into<String>,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source),
        }
    }

    /// Get the error kind.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Get the error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Get the error code as a string.
    pub fn code(&self) -> &str {
        self.kind.code()
    }

    /// Get the error severity.
    pub fn severity(&self) -> Severity {
        self.kind.severity()
    }

    /// Check if this error is recoverable (retryable).
    pub fn is_recoverable(&self) -> bool {
        self.kind.is_recoverable()
    }

    /// Convert into a different error kind, preserving the source chain.
    pub fn into_kind(self, new_kind: ErrorKind) -> Self {
        Self {
            kind: new_kind,
            message: self.message,
            source: self.source,
        }
    }
}

impl fmt::Display for VoxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.kind.code(), self.message)
    }
}

impl std::error::Error for VoxyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &_)
    }
}

impl From<std::io::Error> for VoxyError {
    fn from(e: std::io::Error) -> Self {
        Self::with_source(ErrorKind::IO, e.to_string(), e)
    }
}

impl From<serde_json::Error> for VoxyError {
    fn from(e: serde_json::Error) -> Self {
        Self::with_source(ErrorKind::Serialization, e.to_string(), e)
    }
}

/// Error classification.
///
/// Each variant maps to a unique error code for machine-readable identification.
/// Error codes follow the pattern: `{CRATE_PREFIX}{NUMBER}` where:
/// - `SH` = shared
/// - `CF` = config
/// - `LG` = logging
/// - `MT` = metrics
/// - `EB` = event bus
/// - `KN` = kernel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ErrorKind {
    /// Configuration error.
    Config,
    /// IO error.
    IO,
    /// Serialization/deserialization error.
    Serialization,
    /// Operation timed out.
    Timeout,
    /// Operation was cancelled.
    Cancelled,
    /// Internal error (should not happen).
    Internal,
    /// Resource not found.
    NotFound,
    /// Permission denied.
    PermissionDenied,
    /// Invalid input provided.
    InvalidInput,
    /// Resource limit exceeded.
    ResourceExhausted,
    /// Feature not implemented.
    NotImplemented,
    /// Dependency error (upstream service failure).
    Dependency,
    /// Lifecycle management error.
    Lifecycle,
    /// IPC communication error.
    IPC,
    /// Plugin error.
    Plugin,
    /// Network error.
    Network,
    /// Authentication/authorization error.
    Auth,
}

impl ErrorKind {
    /// Get the machine-readable error code.
    pub fn code(&self) -> &str {
        match self {
            Self::Config => "SH001",
            Self::IO => "SH002",
            Self::Serialization => "SH003",
            Self::Timeout => "SH004",
            Self::Cancelled => "SH005",
            Self::Internal => "SH006",
            Self::NotFound => "SH007",
            Self::PermissionDenied => "SH008",
            Self::InvalidInput => "SH009",
            Self::ResourceExhausted => "SH010",
            Self::NotImplemented => "SH011",
            Self::Dependency => "SH012",
            Self::Lifecycle => "SH013",
            Self::IPC => "SH014",
            Self::Plugin => "SH015",
            Self::Network => "SH016",
            Self::Auth => "SH017",
        }
    }

    /// Get the severity level for this error kind.
    pub fn severity(&self) -> Severity {
        match self {
            Self::Cancelled | Self::NotFound => Severity::Debug,
            Self::Config | Self::InvalidInput | Self::NotImplemented => Severity::Warning,
            Self::IO | Self::Timeout | Self::ResourceExhausted | Self::Network => Severity::Error,
            Self::PermissionDenied
            | Self::Dependency
            | Self::Lifecycle
            | Self::IPC
            | Self::Plugin
            | Self::Auth => Severity::Error,
            Self::Serialization | Self::Internal => Severity::Critical,
        }
    }

    /// Check if this error is recoverable (retryable).
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::ResourceExhausted | Self::IO | Self::Dependency | Self::Network
        )
    }

    /// Check if this error is fatal (should terminate the process).
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Internal | Self::Serialization)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config => write!(f, "Configuration Error"),
            Self::IO => write!(f, "IO Error"),
            Self::Serialization => write!(f, "Serialization Error"),
            Self::Timeout => write!(f, "Timeout"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Internal => write!(f, "Internal Error"),
            Self::NotFound => write!(f, "Not Found"),
            Self::PermissionDenied => write!(f, "Permission Denied"),
            Self::InvalidInput => write!(f, "Invalid Input"),
            Self::ResourceExhausted => write!(f, "Resource Exhausted"),
            Self::NotImplemented => write!(f, "Not Implemented"),
            Self::Dependency => write!(f, "Dependency Error"),
            Self::Lifecycle => write!(f, "Lifecycle Error"),
            Self::IPC => write!(f, "IPC Error"),
            Self::Plugin => write!(f, "Plugin Error"),
            Self::Network => write!(f, "Network Error"),
            Self::Auth => write!(f, "Authentication Error"),
        }
    }
}

/// Error severity levels.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Severity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Error => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_creation() {
        let err = VoxyError::new(ErrorKind::IO, "test error");
        assert_eq!(err.kind(), &ErrorKind::IO);
        assert_eq!(err.message(), "test error");
        assert_eq!(err.code(), "SH002");
    }

    #[test]
    fn error_display() {
        let err = VoxyError::new(ErrorKind::Config, "bad config");
        assert_eq!(err.to_string(), "[SH001] bad config");
    }

    #[test]
    fn error_recoverable() {
        assert!(ErrorKind::Timeout.is_recoverable());
        assert!(ErrorKind::Network.is_recoverable());
        assert!(!ErrorKind::Internal.is_recoverable());
        assert!(!ErrorKind::Auth.is_recoverable());
    }

    #[test]
    fn error_fatal() {
        assert!(ErrorKind::Internal.is_fatal());
        assert!(ErrorKind::Serialization.is_fatal());
        assert!(!ErrorKind::IO.is_fatal());
        assert!(!ErrorKind::Timeout.is_fatal());
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err: VoxyError = io_err.into();
        assert_eq!(err.kind(), &ErrorKind::IO);
    }

    #[test]
    fn error_from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let err: VoxyError = json_err.into();
        assert_eq!(err.kind(), &ErrorKind::Serialization);
    }

    #[test]
    fn error_into_kind() {
        let err = VoxyError::new(ErrorKind::IO, "test");
        let err = err.into_kind(ErrorKind::Network);
        assert_eq!(err.kind(), &ErrorKind::Network);
        assert_eq!(err.message(), "test");
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Debug < Severity::Info);
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Critical);
    }

    #[test]
    fn error_kind_ordering() {
        // ErrorKind implements Ord for use in sorted collections
        let mut kinds = [ErrorKind::IO, ErrorKind::Config, ErrorKind::Timeout];
        kinds.sort();
        assert_eq!(kinds[0], ErrorKind::Config);
    }

    #[test]
    fn all_error_codes_unique() {
        let codes: Vec<&str> = vec![
            ErrorKind::Config.code(),
            ErrorKind::IO.code(),
            ErrorKind::Serialization.code(),
            ErrorKind::Timeout.code(),
            ErrorKind::Cancelled.code(),
            ErrorKind::Internal.code(),
            ErrorKind::NotFound.code(),
            ErrorKind::PermissionDenied.code(),
            ErrorKind::InvalidInput.code(),
            ErrorKind::ResourceExhausted.code(),
            ErrorKind::NotImplemented.code(),
            ErrorKind::Dependency.code(),
            ErrorKind::Lifecycle.code(),
            ErrorKind::IPC.code(),
            ErrorKind::Plugin.code(),
            ErrorKind::Network.code(),
            ErrorKind::Auth.code(),
        ];
        let mut unique = codes.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(codes.len(), unique.len(), "Error codes must be unique");
    }
}
