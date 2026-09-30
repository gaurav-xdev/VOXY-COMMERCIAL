use std::fmt;
use std::time::Duration;

use crate::error::{ErrorKind, Severity, VoxyError};

/// Structured error context for production logging and debugging.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub component: String,
    pub operation: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub severity: Severity,
    pub recoverable: bool,
    pub metadata: std::collections::HashMap<String, String>,
}

impl ErrorContext {
    pub fn new(component: &str, operation: &str, severity: Severity) -> Self {
        Self {
            component: component.to_string(),
            operation: operation.to_string(),
            timestamp: chrono::Utc::now(),
            severity,
            recoverable: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    pub fn with_recoverable(mut self, recoverable: bool) -> Self {
        self.recoverable = recoverable;
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn wrap_error(&self, error: VoxyError) -> VoxyError {
        let meta = self.format_metadata();
        let message = format!(
            "[{}:{}] {} {}",
            self.component,
            self.operation,
            error.message(),
            meta
        );
        VoxyError::with_source(*error.kind(), message, error)
    }

    fn format_metadata(&self) -> String {
        if self.metadata.is_empty() {
            String::new()
        } else {
            let pairs: Vec<_> = self
                .metadata
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            format!("[{}]", pairs.join(", "))
        }
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}:{}] at {} ({})",
            self.component,
            self.operation,
            self.timestamp.format("%H:%M:%S%.3f"),
            self.severity
        )
    }
}

/// Retry configuration for recoverable operations.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub retryable_kinds: Vec<ErrorKind>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            retryable_kinds: vec![
                ErrorKind::Timeout,
                ErrorKind::ResourceExhausted,
                ErrorKind::IO,
                ErrorKind::Dependency,
                ErrorKind::Network,
            ],
        }
    }
}

impl RetryPolicy {
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            ..Default::default()
        }
    }

    pub fn conservative() -> Self {
        Self {
            max_attempts: 2,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 3.0,
            ..Default::default()
        }
    }

    pub fn is_retryable(&self, error: &VoxyError) -> bool {
        self.retryable_kinds.contains(error.kind())
    }

    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32 - 1);
        let delay = Duration::from_millis(delay_ms as u64);
        delay.min(self.max_delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_context_display() {
        let ctx = ErrorContext::new("voice", "transcribe", Severity::Error)
            .with_metadata("engine", "whisper");
        let s = format!("{}", ctx);
        assert!(s.contains("voice:transcribe"));
        assert!(s.contains("ERROR"));
    }

    #[test]
    fn retry_policy_defaults() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert!(policy.is_retryable(&VoxyError::new(ErrorKind::Timeout, "test")));
        assert!(!policy.is_retryable(&VoxyError::new(ErrorKind::Internal, "test")));
    }

    #[test]
    fn retry_delay_backoff() {
        let policy = RetryPolicy::default();
        let d1 = policy.delay_for_attempt(1);
        let d2 = policy.delay_for_attempt(2);
        let d3 = policy.delay_for_attempt(3);
        assert!(d1 < d2);
        assert!(d2 < d3);
    }

    #[test]
    fn retry_policy_max_delay_cap() {
        let policy = RetryPolicy::default();
        let d = policy.delay_for_attempt(100);
        assert!(d <= policy.max_delay);
    }
}
