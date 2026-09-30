//! Structured logging with tracing.
//!
//! Provides:
//! - JSON and plain text log formats
//! - File rotation with configurable retention
//! - Environment-based log level filtering
//! - Structured span creation helpers
//! - Secrets filtering (redacts JWT, API keys, passwords, tokens)
//! - Crash log support

use voxy_config::LoggingConfig;

/// Logging result type.
pub type Result<T> = std::result::Result<T, LoggingError>;

/// Logging error type.
#[derive(Debug)]
pub enum LoggingError {
    /// Initialization failed.
    InitFailed(String),
}

impl LoggingError {
    /// Get the error code.
    pub fn code(&self) -> &str {
        match self {
            Self::InitFailed(_) => "LG001",
        }
    }
}

impl std::fmt::Display for LoggingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InitFailed(msg) => write!(f, "Logging init failed: {}", msg),
        }
    }
}

impl std::error::Error for LoggingError {}

impl From<LoggingError> for voxy_shared::VoxyError {
    fn from(e: LoggingError) -> Self {
        voxy_shared::VoxyError::with_source(voxy_shared::ErrorKind::Internal, e.to_string(), e)
    }
}

/// Patterns that indicate sensitive data in log output.
const SECRET_PATTERNS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "api-key",
    "authorization",
    "bearer",
    "credential",
    "private_key",
    "private-key",
    "jwt",
    "refresh_token",
    "refresh-token",
    "access_token",
    "access-token",
];

/// Check if a key-value pair contains sensitive data.
fn is_sensitive_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    SECRET_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Redact sensitive values in a string.
///
/// Replaces values that look like secrets with `[REDACTED]`.
/// Does not redact short strings (< 8 chars) to avoid false positives on
/// placeholder values like "test" or "none".
pub fn redact_secrets(input: &str) -> String {
    let lower = input.to_lowercase();
    for pattern in SECRET_PATTERNS {
        if lower.contains(pattern) {
            // Only redact if the value is long enough to be a real secret
            if input.len() >= 8 {
                return "[REDACTED]".to_string();
            }
        }
    }
    input.to_string()
}

/// Filter a log line by redacting sensitive key-value pairs.
///
/// Handles structured log lines (JSON) and plain text.
pub fn filter_log_line(line: &str) -> String {
    // Try JSON parsing first
    if let Ok(mut parsed) = serde_json::from_str::<serde_json::Value>(line) {
        filter_json_value(&mut parsed);
        return serde_json::to_string(&parsed).unwrap_or_else(|_| line.to_string());
    }

    // Plain text: redact lines containing sensitive patterns
    let lower = line.to_lowercase();
    for pattern in SECRET_PATTERNS {
        if lower.contains(pattern) {
            return "[REDACTED - sensitive data filtered]".to_string();
        }
    }
    line.to_string()
}

/// Recursively filter sensitive fields in a JSON value.
fn filter_json_value(val: &mut serde_json::Value) {
    match val {
        serde_json::Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *value = serde_json::Value::String("[REDACTED]".to_string());
                } else {
                    filter_json_value(value);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                filter_json_value(item);
            }
        }
        _ => {}
    }
}

/// Initialize the logging subsystem with file rotation.
pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    use tracing_subscriber::EnvFilter;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config.level()));

    if let Some(file_path) = config.file_path() {
        let log_dir = std::path::Path::new(file_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        std::fs::create_dir_all(log_dir).map_err(|e| {
            LoggingError::InitFailed(format!("Failed to create log directory: {}", e))
        })?;

        let file_name = std::path::Path::new(file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "voxy.log".to_string());

        // Clean up old log files based on retention policy
        cleanup_old_logs(log_dir, &file_name, config.max_files());

        let file_appender = tracing_appender::rolling::daily(log_dir, &file_name);
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

        // Leak the guard intentionally — tracing requires a static writer.
        // In production, this is acceptable as logging lives for the process lifetime.
        std::mem::forget(_guard);

        if config.json_format() {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .json()
                .with_writer(non_blocking)
                .init();
        } else {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_writer(non_blocking)
                .init();
        }
    } else if config.json_format() {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    tracing::info!(
        level = %config.level(),
        json_format = config.json_format(),
        "Logging initialized"
    );

    Ok(())
}

/// Clean up old log files beyond the retention limit.
fn cleanup_old_logs(dir: &std::path::Path, base_name: &str, max_files: u32) {
    if max_files == 0 {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    let mut log_files: Vec<(std::time::SystemTime, std::path::PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(base_name.trim_end_matches(".log"))
        })
        .filter_map(|e| {
            let modified = e.metadata().ok().and_then(|m| m.modified().ok())?;
            Some((modified, e.path()))
        })
        .collect();

    // Sort by modification time (oldest first)
    log_files.sort_by_key(|(t, _)| *t);

    // Remove oldest files beyond retention limit
    if log_files.len() > max_files as usize {
        let to_remove = log_files.len() - max_files as usize;
        for (_, path) in log_files.iter().take(to_remove) {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Shutdown the logging subsystem.
pub fn shutdown_logging() -> Result<()> {
    tracing::info!("Logging shutdown");
    Ok(())
}

/// Create a tracing span with structured context.
pub fn create_span(name: &str) -> tracing::Span {
    tracing::info_span!("event", name = name)
}

/// Create a tracing span with module path.
pub fn module_span(module: &str) -> tracing::Span {
    tracing::info_span!("module", module = module)
}

/// Write a crash log with panic information.
#[allow(deprecated)]
pub fn write_crash_log(panic_info: &std::panic::PanicInfo) {
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("unnamed");

    let backtrace = std::backtrace::Backtrace::force_capture();

    let crash_log = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "level": "CRASH",
        "thread": thread_name,
        "message": panic_info.payload().downcast_ref::<String>().unwrap_or(
            &panic_info.payload().downcast_ref::<&str>().unwrap_or(&"unknown panic").to_string()
        ).clone(),
        "location": panic_info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column())).unwrap_or_else(|| "unknown".to_string()),
        "backtrace": format!("{}", backtrace),
    });

    // Write to stderr (last resort if logging is down)
    eprintln!("CRASH: {}", crash_log);

    // Try to write to crash log file
    if let Some(log_dir) = dirs::data_local_dir() {
        let crash_dir = log_dir.join("voxy").join("crashes");
        let _ = std::fs::create_dir_all(&crash_dir);
        let crash_file = crash_dir.join(format!(
            "crash-{}.log",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ));
        let _ = std::fs::write(&crash_file, crash_log.to_string());
    }
}

/// Install a global panic handler that writes crash logs.
pub fn install_crash_handler() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        write_crash_log(panic_info);
        default_hook(panic_info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_with_default_config() {
        let config = LoggingConfig::default();
        assert!(!config.json_format());
        assert_eq!(config.level(), "INFO");
        assert!(config.file_path().is_none());
    }

    #[test]
    fn logging_error_display() {
        let err = LoggingError::InitFailed("test".to_string());
        assert_eq!(err.to_string(), "Logging init failed: test");
    }

    #[test]
    fn logging_error_into_voxy_error() {
        let err = LoggingError::InitFailed("test".to_string());
        let voxy_err: voxy_shared::VoxyError = err.into();
        assert_eq!(voxy_err.kind(), &voxy_shared::ErrorKind::Internal);
    }

    #[test]
    fn logging_error_code() {
        let err = LoggingError::InitFailed("test".to_string());
        assert_eq!(err.code(), "LG001");
    }

    #[test]
    fn create_span_works() {
        let _span = create_span("test_span");
    }

    #[test]
    fn module_span_works() {
        let _span = module_span("voxy::test");
    }

    #[test]
    fn redact_secrets_password() {
        assert_eq!(redact_secrets("password=mysecret123"), "[REDACTED]");
    }

    #[test]
    fn redact_secrets_api_key() {
        assert_eq!(redact_secrets("api_key=sk-abc123def456"), "[REDACTED]");
    }

    #[test]
    fn redact_secrets_short_value_not_redacted() {
        // Strings shorter than 8 chars are not redacted even if they contain sensitive keywords
        let result = redact_secrets("tok");
        assert_eq!(result, "tok");
    }

    #[test]
    fn redact_secrets_normal_text() {
        assert_eq!(
            redact_secrets("User logged in successfully"),
            "User logged in successfully"
        );
    }

    #[test]
    fn filter_json_redacts_sensitive_fields() {
        let input = r#"{"username":"alice","password":"hunter2","api_key":"sk-12345678"}"#;
        let result = filter_log_line(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["username"], "alice");
        assert_eq!(parsed["password"], "[REDACTED]");
        assert_eq!(parsed["api_key"], "[REDACTED]");
    }

    #[test]
    fn filter_json_preserves_non_sensitive() {
        let input = r#"{"level":"info","message":"hello world"}"#;
        let result = filter_log_line(input);
        assert!(result.contains("hello world"));
    }

    #[test]
    fn filter_plain_text_redacts_secrets() {
        let result = filter_log_line("Authorization: Bearer eyJhbG...");
        assert_eq!(result, "[REDACTED - sensitive data filtered]");
    }

    #[test]
    fn filter_plain_text_preserves_normal() {
        let result = filter_log_line("Server started on port 8080");
        assert_eq!(result, "Server started on port 8080");
    }

    #[test]
    fn is_sensitive_key_detection() {
        assert!(is_sensitive_key("password"));
        assert!(is_sensitive_key("api_key"));
        assert!(is_sensitive_key("Authorization"));
        assert!(is_sensitive_key("refresh_token"));
        assert!(!is_sensitive_key("username"));
        assert!(!is_sensitive_key("level"));
    }

    #[test]
    fn cleanup_old_logs_does_not_panic() {
        let dir = tempfile::tempdir().unwrap();
        cleanup_old_logs(dir.path(), "voxy.log", 5);
    }
}
