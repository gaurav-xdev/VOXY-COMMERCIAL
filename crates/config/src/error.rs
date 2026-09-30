//! Configuration error types.

use std::fmt;

/// Configuration error type.
#[derive(Debug)]
pub enum ConfigError {
    /// Configuration file not found.
    FileNotFound(String),
    /// Configuration parse error.
    ParseError(String),
    /// Configuration validation failed.
    ValidationFailed(String),
    /// IO error.
    Io(std::io::Error),
    /// JSON serialization error.
    Json(serde_json::Error),
    /// TOML serialization error.
    Toml(String),
}

impl ConfigError {
    /// Get the error code.
    pub fn code(&self) -> &str {
        match self {
            Self::FileNotFound(_) => "CF001",
            Self::ParseError(_) => "CF002",
            Self::ValidationFailed(_) => "CF003",
            Self::Io(_) => "CF004",
            Self::Json(_) => "CF005",
            Self::Toml(_) => "CF006",
        }
    }

    /// Check if this error is recoverable.
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Self::FileNotFound(_))
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileNotFound(path) => write!(f, "Config file not found: {}", path),
            Self::ParseError(msg) => write!(f, "Config parse error: {}", msg),
            Self::ValidationFailed(msg) => write!(f, "Config validation failed: {}", msg),
            Self::Io(e) => write!(f, "IO error: {}", e),
            Self::Json(e) => write!(f, "JSON error: {}", e),
            Self::Toml(e) => write!(f, "TOML error: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        Self::Toml(e.to_string())
    }
}

impl From<toml::ser::Error> for ConfigError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Toml(e.to_string())
    }
}

impl From<ConfigError> for voxy_shared::VoxyError {
    fn from(e: ConfigError) -> Self {
        let kind = match &e {
            ConfigError::FileNotFound(_) => voxy_shared::ErrorKind::NotFound,
            ConfigError::ParseError(_) | ConfigError::Json(_) | ConfigError::Toml(_) => {
                voxy_shared::ErrorKind::Serialization
            }
            ConfigError::ValidationFailed(_) => voxy_shared::ErrorKind::InvalidInput,
            ConfigError::Io(_) => voxy_shared::ErrorKind::IO,
        };
        voxy_shared::VoxyError::with_boxed_source(kind, e.to_string(), Box::new(e))
    }
}

/// Configuration result type.
pub type Result<T> = std::result::Result<T, ConfigError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_unique() {
        let file_not_found = ConfigError::FileNotFound("".into());
        let parse_error = ConfigError::ParseError("".into());
        let validation = ConfigError::ValidationFailed("".into());
        let io_err = ConfigError::Io(std::io::Error::other("test"));
        let codes = [
            file_not_found.code(),
            parse_error.code(),
            validation.code(),
            io_err.code(),
        ];
        let mut unique = codes.to_vec();
        unique.sort();
        unique.dedup();
        assert_eq!(codes.len(), unique.len());
    }

    #[test]
    fn error_source_chain() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let config_err = ConfigError::Io(io_err);
        assert!(std::error::Error::source(&config_err).is_some());
    }

    #[test]
    fn config_error_into_voxy_error() {
        let err = ConfigError::ValidationFailed("test".into());
        let voxy_err: voxy_shared::VoxyError = err.into();
        assert_eq!(voxy_err.kind(), &voxy_shared::ErrorKind::InvalidInput);
    }
}
