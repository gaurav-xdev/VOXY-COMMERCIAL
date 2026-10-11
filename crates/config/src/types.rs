//! Configuration type definitions.
//!
//! All config structs use private fields with getters for forward compatibility.
//! Environment variable overrides are supported via the `with_env_overrides` method.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application configuration.
///
/// This is the root configuration type. All sub-configs are nested here.
/// Fields are private to allow adding new fields without breaking the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Schema version for migration support.
    #[serde(default = "default_schema_version")]
    schema_version: String,
    kernel: KernelConfig,
    logging: LoggingConfig,
    metrics: MetricsConfig,
    event_bus: EventBusConfig,
    #[serde(default)]
    api_keys: ApiKeysConfig,
}

fn default_schema_version() -> String {
    "1.0.0".to_string()
}

impl AppConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> crate::Result<()> {
        self.kernel.validate()?;
        self.logging.validate()?;
        self.metrics.validate()?;
        self.event_bus.validate()?;
        self.api_keys.validate()?;
        Ok(())
    }

    /// Get the schema version.
    pub fn schema_version(&self) -> &str {
        &self.schema_version
    }

    /// Get the kernel configuration.
    pub fn kernel(&self) -> &KernelConfig {
        &self.kernel
    }

    /// Get the logging configuration.
    pub fn logging(&self) -> &LoggingConfig {
        &self.logging
    }

    /// Get the metrics configuration.
    pub fn metrics(&self) -> &MetricsConfig {
        &self.metrics
    }

    /// Get the event bus configuration.
    pub fn event_bus(&self) -> &EventBusConfig {
        &self.event_bus
    }

    /// Get the API keys configuration.
    pub fn api_keys(&self) -> &ApiKeysConfig {
        &self.api_keys
    }

    /// Get the default config file path.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("voxy").join("config.toml"))
    }

    /// Load configuration from a file (detects format by extension).
    pub fn load_from_file(path: &std::path::Path) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::error::ConfigError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Failed to read config {}: {}", path.display(), e),
            ))
        })?;

        let config: AppConfig = match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => toml::from_str(&content)
                .map_err(|e| crate::error::ConfigError::Toml(e.to_string()))?,
            Some("json") => {
                serde_json::from_str(&content).map_err(crate::error::ConfigError::Json)?
            }
            _ => toml::from_str(&content)
                .map_err(|e| crate::error::ConfigError::Toml(e.to_string()))?,
        };
        Ok(config)
    }

    /// Save configuration to a file (atomic write via temp file + rename).
    pub fn save_to_file(&self, path: &std::path::Path) -> crate::Result<()> {
        let content = match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => toml::to_string_pretty(self)
                .map_err(|e| crate::error::ConfigError::Toml(e.to_string()))?,
            Some("json") => {
                serde_json::to_string_pretty(self).map_err(crate::error::ConfigError::Json)?
            }
            _ => toml::to_string_pretty(self)
                .map_err(|e| crate::error::ConfigError::Toml(e.to_string()))?,
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                crate::error::ConfigError::Io(std::io::Error::other(format!(
                    "Failed to create config directory: {}",
                    e
                )))
            })?;
        }

        let temp_path = path.with_extension("toml.tmp");
        std::fs::write(&temp_path, &content).map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            crate::error::ConfigError::Io(std::io::Error::other(format!(
                "Failed to write temp config: {}",
                e
            )))
        })?;
        std::fs::rename(&temp_path, path).map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            crate::error::ConfigError::Io(std::io::Error::other(format!(
                "Failed to rename temp config to target: {}",
                e
            )))
        })?;
        Ok(())
    }

    /// Apply environment variable overrides.
    ///
    /// Environment variables override config values using the pattern:
    /// `VOXY_<SECTION>_<KEY>` (e.g., `VOXY_KERNEL_THREAD_POOL_SIZE=4`).
    pub fn with_env_overrides(mut self) -> Self {
        if let Ok(val) = std::env::var("VOXY_KERNEL_THREAD_POOL_SIZE") {
            if let Ok(n) = val.parse() {
                self.kernel.thread_pool_size = n;
            }
        }
        if let Ok(val) = std::env::var("VOXY_KERNEL_MEMORY_QUOTA_MB") {
            if let Ok(n) = val.parse() {
                self.kernel.memory_quota_mb = n;
            }
        }
        if let Ok(val) = std::env::var("VOXY_LOGGING_LEVEL") {
            self.logging.level = val;
        }
        if let Ok(val) = std::env::var("VOXY_LOGGING_JSON_FORMAT") {
            if let Ok(b) = val.parse() {
                self.logging.json_format = b;
            }
        }
        if let Ok(val) = std::env::var("VOXY_METRICS_ENABLED") {
            if let Ok(b) = val.parse() {
                self.metrics.enabled = b;
            }
        }
        if let Ok(val) = std::env::var("VOXY_METRICS_PROMETHEUS_PORT") {
            if let Ok(n) = val.parse() {
                self.metrics.prometheus_port = Some(n);
            }
        }
        if let Ok(val) = std::env::var("VOXY_EVENT_BUS_BUFFER_SIZE") {
            if let Ok(n) = val.parse() {
                self.event_bus.buffer_size = n;
            }
        }
        if let Ok(val) = std::env::var("VOXY_API_KEYS_OPENAI") {
            self.api_keys.openai = Some(val);
        }
        if let Ok(val) = std::env::var("VOXY_API_KEYS_ANTHROPIC") {
            self.api_keys.anthropic = Some(val);
        }
        if let Ok(val) = std::env::var("VOXY_API_KEYS_GEMINI") {
            self.api_keys.gemini = Some(val);
        }
        if let Ok(val) = std::env::var("VOXY_API_KEYS_ELEVENLABS") {
            self.api_keys.elevenlabs = Some(val);
        }
        if let Ok(val) = std::env::var("VOXY_API_KEYS_NVIDIA") {
            self.api_keys.nvidia = Some(val);
        }
        self
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            kernel: KernelConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            event_bus: EventBusConfig::default(),
            api_keys: ApiKeysConfig::default(),
        }
    }
}

/// Kernel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelConfig {
    thread_pool_size: usize,
    memory_quota_mb: u64,
    init_timeout_seconds: u64,
    shutdown_timeout_seconds: u64,
}

impl KernelConfig {
    /// Validate the kernel configuration.
    pub fn validate(&self) -> crate::Result<()> {
        if self.thread_pool_size == 0 {
            return Err(crate::error::ConfigError::ValidationFailed(
                "thread_pool_size must be > 0".to_string(),
            ));
        }
        if self.memory_quota_mb == 0 {
            return Err(crate::error::ConfigError::ValidationFailed(
                "memory_quota_mb must be > 0".to_string(),
            ));
        }
        if self.init_timeout_seconds == 0 {
            return Err(crate::error::ConfigError::ValidationFailed(
                "init_timeout_seconds must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Get the thread pool size.
    pub fn thread_pool_size(&self) -> usize {
        self.thread_pool_size
    }

    /// Get the memory quota in MB.
    pub fn memory_quota_mb(&self) -> u64 {
        self.memory_quota_mb
    }

    /// Get the init timeout in seconds.
    pub fn init_timeout_seconds(&self) -> u64 {
        self.init_timeout_seconds
    }

    /// Get the shutdown timeout in seconds.
    pub fn shutdown_timeout_seconds(&self) -> u64 {
        self.shutdown_timeout_seconds
    }
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            thread_pool_size: num_cpus::get() * 2,
            memory_quota_mb: 2048,
            init_timeout_seconds: 30,
            shutdown_timeout_seconds: 30,
        }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    level: String,
    json_format: bool,
    file_path: Option<String>,
    max_file_size_mb: u64,
    max_files: u32,
}

impl LoggingConfig {
    /// Validate the logging configuration.
    pub fn validate(&self) -> crate::Result<()> {
        let valid_levels = ["TRACE", "DEBUG", "INFO", "WARN", "ERROR"];
        let level_upper = self.level.to_uppercase();
        if !valid_levels.contains(&level_upper.as_str()) {
            return Err(crate::error::ConfigError::ValidationFailed(format!(
                "Invalid log level: {}. Must be one of: TRACE, DEBUG, INFO, WARN, ERROR",
                self.level
            )));
        }
        Ok(())
    }

    /// Get the log level.
    pub fn level(&self) -> &str {
        &self.level
    }

    /// Get whether JSON format is enabled.
    pub fn json_format(&self) -> bool {
        self.json_format
    }

    /// Get the file path for log output.
    pub fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    /// Get the max file size in MB.
    pub fn max_file_size_mb(&self) -> u64 {
        self.max_file_size_mb
    }

    /// Get the max number of log files.
    pub fn max_files(&self) -> u32 {
        self.max_files
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "INFO".to_string(),
            json_format: false,
            file_path: None,
            max_file_size_mb: 100,
            max_files: 5,
        }
    }
}

/// Metrics configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    enabled: bool,
    export_interval_seconds: u64,
    prometheus_port: Option<u16>,
}

impl MetricsConfig {
    /// Validate the metrics configuration.
    pub fn validate(&self) -> crate::Result<()> {
        if let Some(port) = self.prometheus_port {
            if port == 0 {
                return Err(crate::error::ConfigError::ValidationFailed(
                    "prometheus_port must be > 0".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Get whether metrics are enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Get the export interval in seconds.
    pub fn export_interval_seconds(&self) -> u64 {
        self.export_interval_seconds
    }

    /// Get the Prometheus port.
    pub fn prometheus_port(&self) -> Option<u16> {
        self.prometheus_port
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            export_interval_seconds: 15,
            prometheus_port: None,
        }
    }
}

/// Event bus configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusConfig {
    buffer_size: usize,
    max_message_size: usize,
    dead_letter_enabled: bool,
    dead_letter_max_size: usize,
}

impl EventBusConfig {
    /// Validate the event bus configuration.
    pub fn validate(&self) -> crate::Result<()> {
        if self.buffer_size == 0 {
            return Err(crate::error::ConfigError::ValidationFailed(
                "buffer_size must be > 0".to_string(),
            ));
        }
        if self.max_message_size == 0 {
            return Err(crate::error::ConfigError::ValidationFailed(
                "max_message_size must be > 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Get the buffer size.
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    /// Get the max message size.
    pub fn max_message_size(&self) -> usize {
        self.max_message_size
    }

    /// Get whether dead letter is enabled.
    pub fn dead_letter_enabled(&self) -> bool {
        self.dead_letter_enabled
    }

    /// Get the dead letter max size.
    pub fn dead_letter_max_size(&self) -> usize {
        self.dead_letter_max_size
    }
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            max_message_size: 1024 * 1024,
            dead_letter_enabled: true,
            dead_letter_max_size: 100,
        }
    }
}

/// API keys configuration for provider integrations.
///
/// Keys are stored locally and never transmitted except to the configured provider.
/// Supports loading from environment variables: VOXY_API_KEYS_OPENAI, etc.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeysConfig {
    /// OpenAI API key (for Whisper STT and GPT models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<String>,
    /// Anthropic API key (for Claude models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<String>,
    /// Google Gemini API key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gemini: Option<String>,
    /// ElevenLabs API key (for TTS).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevenlabs: Option<String>,
    /// NVIDIA NIM API key (for NVIDIA NIM hosted models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nvidia: Option<String>,
    /// Custom provider base URLs.
    #[serde(default)]
    pub custom_endpoints: std::collections::HashMap<String, String>,
}

impl ApiKeysConfig {
    pub fn validate(&self) -> crate::Result<()> {
        Ok(())
    }

    /// Check if any API key is configured.
    pub fn has_any_key(&self) -> bool {
        self.openai.is_some()
            || self.anthropic.is_some()
            || self.gemini.is_some()
            || self.elevenlabs.is_some()
            || self.nvidia.is_some()
    }

    /// Get a key by provider name.
    pub fn get_key(&self, provider: &str) -> Option<&str> {
        match provider {
            "openai" => self.openai.as_deref(),
            "anthropic" => self.anthropic.as_deref(),
            "gemini" => self.gemini.as_deref(),
            "elevenlabs" => self.elevenlabs.as_deref(),
            "nvidia" | "nvidia_nim" | "nim" => self.nvidia.as_deref(),
            _ => self.custom_endpoints.get(provider).map(|s| s.as_str()),
        }
    }

    /// Set a key for a provider.
    pub fn set_key(&mut self, provider: &str, key: String) {
        match provider {
            "openai" => self.openai = Some(key),
            "anthropic" => self.anthropic = Some(key),
            "gemini" => self.gemini = Some(key),
            "elevenlabs" => self.elevenlabs = Some(key),
            "nvidia" | "nvidia_nim" | "nim" => self.nvidia = Some(key),
            _ => {
                self.custom_endpoints.insert(provider.to_string(), key);
            }
        }
    }

    /// Remove a key for a provider.
    pub fn remove_key(&mut self, provider: &str) {
        match provider {
            "openai" => self.openai = None,
            "anthropic" => self.anthropic = None,
            "gemini" => self.gemini = None,
            "elevenlabs" => self.elevenlabs = None,
            "nvidia" | "nvidia_nim" | "nim" => self.nvidia = None,
            _ => {
                self.custom_endpoints.remove(provider);
            }
        }
    }

    /// Get list of configured provider names (keys only, no values).
    pub fn configured_providers(&self) -> Vec<&'static str> {
        let mut providers = Vec::new();
        if self.openai.is_some() {
            providers.push("openai");
        }
        if self.anthropic.is_some() {
            providers.push("anthropic");
        }
        if self.gemini.is_some() {
            providers.push("gemini");
        }
        if self.elevenlabs.is_some() {
            providers.push("elevenlabs");
        }
        if self.nvidia.is_some() {
            providers.push("nvidia");
        }
        providers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_valid() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_schema_version() {
        let config = AppConfig::default();
        assert_eq!(config.schema_version(), "1.0.0");
    }

    #[test]
    fn config_getters() {
        let config = AppConfig::default();
        assert!(config.kernel().thread_pool_size() > 0);
        assert_eq!(config.logging().level(), "INFO");
        assert!(config.metrics().enabled());
        assert!(config.event_bus().dead_letter_enabled());
    }

    #[test]
    fn config_serializes_toml() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("thread_pool_size"));
        let deserialized: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            deserialized.kernel().thread_pool_size(),
            config.kernel().thread_pool_size()
        );
    }

    #[test]
    fn config_serializes_json() {
        let config = AppConfig::default();
        let json_str = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json_str).unwrap();
        assert_eq!(
            deserialized.kernel().thread_pool_size(),
            config.kernel().thread_pool_size()
        );
    }

    #[test]
    fn kernel_validation_rejects_zero_threads() {
        let config = KernelConfig {
            thread_pool_size: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn logging_validation_rejects_invalid_level() {
        let config = LoggingConfig {
            level: "INVALID".to_string(),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn event_bus_validation_rejects_zero_buffer() {
        let config = EventBusConfig {
            buffer_size: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn save_and_load_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let config = AppConfig::default();
        config.save_to_file(&path).unwrap();
        let loaded = AppConfig::load_from_file(&path).unwrap();
        assert_eq!(
            loaded.kernel().thread_pool_size(),
            config.kernel().thread_pool_size()
        );
    }

    #[test]
    fn save_and_load_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let config = AppConfig::default();
        config.save_to_file(&path).unwrap();
        let loaded = AppConfig::load_from_file(&path).unwrap();
        assert_eq!(
            loaded.kernel().thread_pool_size(),
            config.kernel().thread_pool_size()
        );
    }

    #[test]
    fn env_overrides() {
        std::env::set_var("VOXY_KERNEL_THREAD_POOL_SIZE", "8");
        std::env::set_var("VOXY_LOGGING_LEVEL", "DEBUG");

        let config = AppConfig::default().with_env_overrides();
        assert_eq!(config.kernel().thread_pool_size(), 8);
        assert_eq!(config.logging().level(), "DEBUG");

        std::env::remove_var("VOXY_KERNEL_THREAD_POOL_SIZE");
        std::env::remove_var("VOXY_LOGGING_LEVEL");
    }
}
