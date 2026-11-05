//! Runtime settings system.
//!
//! Comprehensive settings with validation, hot reload, persistence, and rollback.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{broadcast, watch};
use tracing::{error, info, warn};

use crate::error::{Result, RuntimeError};

/// Counter used to keep temp file names unique within this process.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn default_speed() -> f64 {
    1.0
}

fn default_pitch() -> f64 {
    1.0
}

fn default_false() -> bool {
    false
}

fn default_coding_model() -> String {
    "llama3.2:3b".to_string()
}

fn default_voice_model() -> String {
    "llama3.2:3b".to_string()
}

fn default_embedding_model() -> String {
    "nomic-embed-text".to_string()
}

fn default_risk_threshold() -> String {
    "prompt".to_string()
}

fn default_model_unload_minutes() -> u32 {
    15
}

/// Application settings snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsSnapshot {
    pub app_name: String,
    #[serde(default)]
    pub onboarded: bool,
    pub voice: VoiceSettings,
    pub models: ModelSettings,
    pub memory: MemorySettings,
    pub automation: AutomationSettings,
    pub privacy: PrivacySettings,
    pub updates: UpdateSettings,
    pub performance: PerformanceSettings,
    pub developer: DeveloperSettings,
    pub plugins: PluginSettings,
    pub appearance: AppearanceSettings,
    #[serde(default)]
    pub profile: ProfileSettings,
    #[serde(default)]
    pub companion: CompanionSettings,
    #[serde(default)]
    pub security: SecuritySettings,
    #[serde(default)]
    pub integrations: HashMap<String, IntegrationEntry>,
}

/// Voice settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSettings {
    pub enabled: bool,
    pub wake_word: String,
    #[serde(default)]
    pub wake_words: Vec<String>,
    pub wake_word_sensitivity: f64,
    pub always_listening: bool,
    pub noise_suppression: bool,
    pub echo_cancellation: bool,
    pub auto_gain_control: bool,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub language: String,
    #[serde(default)]
    pub tts_provider: Option<String>,
    #[serde(default)]
    pub stt_provider: Option<String>,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    #[serde(default = "default_speed")]
    pub speech_speed: f64,
    #[serde(default = "default_pitch")]
    pub speech_pitch: f64,
    #[serde(default = "default_true")]
    pub interruption_barge_in: bool,
    #[serde(default)]
    pub voice_name: Option<String>,
}

fn default_sample_rate() -> u32 {
    48000
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            wake_word: "Hey OSMOO".to_string(),
            wake_words: vec!["Hey OSMOO".to_string(), "OSMOO".to_string()],
            wake_word_sensitivity: 0.5,
            always_listening: true,
            noise_suppression: true,
            echo_cancellation: true,
            auto_gain_control: true,
            input_device: None,
            output_device: None,
            language: "en".to_string(),
            tts_provider: Some("cartesia".to_string()),
            stt_provider: Some("groq".to_string()),
            sample_rate: 48000,
            speech_speed: 1.0,
            speech_pitch: 1.0,
            interruption_barge_in: true,
            voice_name: Some("en-us-amber".to_string()),
        }
    }
}

/// Model settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSettings {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub max_tokens: u32,
    pub temperature: f64,
    pub local_only: bool,
    #[serde(default)]
    pub fallback_providers: Vec<String>,
    #[serde(default)]
    pub enable_fallback: bool,
    #[serde(default = "default_coding_model")]
    pub coding_model: String,
    #[serde(default = "default_voice_model")]
    pub voice_model: String,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    #[serde(default)]
    pub provider_keys: HashMap<String, String>,
}

impl Default for ModelSettings {
    fn default() -> Self {
        Self {
            provider: "ollama".to_string(),
            model: "llama3.2".to_string(),
            api_key: None,
            base_url: None,
            max_tokens: 4096,
            temperature: 0.7,
            local_only: true,
            fallback_providers: vec![
                "nvidia".to_string(),
                "groq".to_string(),
                "openai".to_string(),
            ],
            enable_fallback: false,
            coding_model: "llama3.2:3b".to_string(),
            voice_model: "llama3.2:3b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            provider_keys: HashMap::new(),
        }
    }
}

/// Memory settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySettings {
    pub enabled: bool,
    pub max_items: usize,
    pub auto_consolidate: bool,
    pub retention_days: u32,
    pub embedding_model: String,
    #[serde(default)]
    pub private_session: bool,
    #[serde(default)]
    pub user_persona_notes: String,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_items: 10000,
            auto_consolidate: true,
            retention_days: 90,
            embedding_model: "default".to_string(),
            private_session: false,
            user_persona_notes: String::new(),
        }
    }
}

/// Automation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationSettings {
    pub enabled: bool,
    pub require_consent: bool,
    pub max_concurrent_tasks: usize,
    pub timeout_seconds: u64,
    #[serde(default = "default_true")]
    pub background_tasks_allowed: bool,
    #[serde(default = "default_true")]
    pub pause_on_battery: bool,
    #[serde(default = "default_risk_threshold")]
    pub action_risk_threshold: String,
    #[serde(default)]
    pub global_tasks_paused: bool,
}

impl Default for AutomationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            require_consent: true,
            max_concurrent_tasks: 3,
            timeout_seconds: 30,
            background_tasks_allowed: true,
            pause_on_battery: true,
            action_risk_threshold: "prompt".to_string(),
            global_tasks_paused: false,
        }
    }
}

/// Privacy settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    pub telemetry_enabled: bool,
    pub crash_reports: bool,
    pub usage_analytics: bool,
    pub local_processing_only: bool,
    pub data_retention_days: u32,
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            telemetry_enabled: false,
            crash_reports: true,
            usage_analytics: false,
            local_processing_only: true,
            data_retention_days: 30,
        }
    }
}

/// Update settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettings {
    pub auto_update: bool,
    pub check_interval_hours: u64,
    pub channel: String,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_update: true,
            check_interval_hours: 24,
            channel: "stable".to_string(),
        }
    }
}

/// Performance settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    pub max_memory_mb: u64,
    pub max_cpu_percent: f64,
    pub gpu_acceleration: bool,
    pub background_throttle: bool,
    #[serde(default = "default_model_unload_minutes")]
    pub model_unload_timeout_minutes: u32,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            max_memory_mb: 2048,
            max_cpu_percent: 50.0,
            gpu_acceleration: true,
            background_throttle: true,
            model_unload_timeout_minutes: 15,
        }
    }
}

/// Developer settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeveloperSettings {
    pub debug_mode: bool,
    pub log_level: String,
    pub show_fps: bool,
    pub expose_metrics: bool,
    pub metrics_port: u16,
}

impl Default for DeveloperSettings {
    fn default() -> Self {
        Self {
            debug_mode: false,
            log_level: "info".to_string(),
            show_fps: false,
            expose_metrics: false,
            metrics_port: 9090,
        }
    }
}

/// Plugin settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSettings {
    pub enabled: bool,
    pub allowed_plugins: Vec<String>,
    pub sandbox_mode: bool,
}

impl Default for PluginSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            allowed_plugins: Vec::new(),
            sandbox_mode: true,
        }
    }
}

/// Appearance and window/overlay behavior settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub theme: String,
    pub opacity: f64,
    pub font_size: u32,
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub start_with_windows: bool,
    #[serde(default = "default_true")]
    pub overlay_enabled: bool,
    #[serde(default = "default_true")]
    pub close_to_overlay: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            opacity: 0.95,
            font_size: 14,
            always_on_top: false,
            start_with_windows: true,
            overlay_enabled: true,
            close_to_overlay: true,
        }
    }
}

/// User profile and general preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSettings {
    #[serde(default = "default_user_name")]
    pub user_name: String,
    #[serde(default = "default_user_email")]
    pub user_email: String,
    #[serde(default = "default_org")]
    pub organization: String,
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_close_behavior")]
    pub close_behavior: String,
}

fn default_user_name() -> String {
    "Operator".to_string()
}
fn default_user_email() -> String {
    "operator@osmoo.in".to_string()
}
fn default_org() -> String {
    "OSMOO Workspace".to_string()
}
fn default_role() -> String {
    "Primary Administrator".to_string()
}
fn default_language() -> String {
    "en".to_string()
}
fn default_close_behavior() -> String {
    "minimize_to_tray".to_string()
}

impl Default for ProfileSettings {
    fn default() -> Self {
        Self {
            user_name: default_user_name(),
            user_email: default_user_email(),
            organization: default_org(),
            role: default_role(),
            language: default_language(),
            close_behavior: default_close_behavior(),
        }
    }
}

/// Companion character appearance and interaction settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompanionSettings {
    #[serde(default = "default_companion_char")]
    pub character_id: String, // "cat", "dragon", "robot", "particle_orb"
    #[serde(default = "default_char_size")]
    pub character_size: f64, // 0.5 to 2.0
    #[serde(default = "default_pos_preset")]
    pub position_preset: String, // "bottom-right", "bottom-left", "top-right", "center-floating"
    #[serde(default = "default_true")]
    pub visibility: bool,
    #[serde(default = "default_true")]
    pub animations_enabled: bool,
    #[serde(default = "default_expression")]
    pub expression: String, // "neutral", "curious", "focused", "celebratory"
    #[serde(default = "default_true")]
    pub lip_sync: bool,
    #[serde(default = "default_true")]
    pub draggable: bool,
    #[serde(default = "default_false")]
    pub click_through: bool,
    #[serde(default = "default_overlay_mode")]
    pub desktop_overlay_mode: String,
}

fn default_companion_char() -> String {
    "cat".to_string()
}
fn default_char_size() -> f64 {
    1.0
}
fn default_pos_preset() -> String {
    "bottom-right".to_string()
}
fn default_expression() -> String {
    "neutral".to_string()
}
fn default_overlay_mode() -> String {
    "floating".to_string()
}

impl Default for CompanionSettings {
    fn default() -> Self {
        Self {
            character_id: default_companion_char(),
            character_size: default_char_size(),
            position_preset: default_pos_preset(),
            visibility: true,
            animations_enabled: true,
            expression: default_expression(),
            lip_sync: true,
            draggable: true,
            click_through: false,
            desktop_overlay_mode: default_overlay_mode(),
        }
    }
}

/// Security controls and permission bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecuritySettings {
    #[serde(default = "default_fs_access")]
    pub file_system_access: String, // "workspace_only", "read_only", "full"
    #[serde(default = "default_browser_access")]
    pub browser_automation: String, // "prompt", "sandboxed", "disabled"
    #[serde(default = "default_shell_access")]
    pub shell_execution: String, // "require_confirmation", "disabled", "allow_workspace"
    #[serde(default = "default_true")]
    pub require_approval_sensitive: bool,
    #[serde(default = "default_true")]
    pub require_approval_financial: bool,
    #[serde(default = "default_true")]
    pub require_approval_email: bool,
    #[serde(default = "default_true")]
    pub require_approval_scripts: bool,
    #[serde(default = "default_false")]
    pub emergency_stopped: bool,
    #[serde(default = "default_true")]
    pub audit_logging_enabled: bool,
}

fn default_fs_access() -> String {
    "workspace_only".to_string()
}
fn default_browser_access() -> String {
    "prompt".to_string()
}
fn default_shell_access() -> String {
    "require_confirmation".to_string()
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            file_system_access: default_fs_access(),
            browser_automation: default_browser_access(),
            shell_execution: default_shell_access(),
            require_approval_sensitive: true,
            require_approval_financial: true,
            require_approval_email: true,
            require_approval_scripts: true,
            emergency_stopped: false,
            audit_logging_enabled: true,
        }
    }
}

/// Status record for external connected integrations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntegrationEntry {
    pub name: String,
    pub category: String,
    pub status: String, // "not_configured", "disconnected", "connected", "expired", "error"
    pub granted_scopes: Vec<String>,
    pub last_checked: Option<String>,
}

impl Default for SettingsSnapshot {
    fn default() -> Self {
        Self {
            app_name: "OSMOO".to_string(),
            onboarded: false,
            voice: VoiceSettings::default(),
            models: ModelSettings::default(),
            memory: MemorySettings::default(),
            automation: AutomationSettings::default(),
            privacy: PrivacySettings::default(),
            updates: UpdateSettings::default(),
            performance: PerformanceSettings::default(),
            developer: DeveloperSettings::default(),
            plugins: PluginSettings::default(),
            appearance: AppearanceSettings::default(),
            profile: ProfileSettings::default(),
            companion: CompanionSettings::default(),
            security: SecuritySettings::default(),
            integrations: HashMap::new(),
        }
    }
}

/// Settings validation error.
#[derive(Debug)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// Settings manager with validation, hot reload, persistence, and rollback.
pub struct SettingsManager {
    settings: RwLock<SettingsSnapshot>,
    history: RwLock<Vec<SettingsSnapshot>>,
    max_history: usize,
    path: PathBuf,
    change_tx: broadcast::Sender<SettingsSnapshot>,
}

impl SettingsManager {
    /// Create a new settings manager.
    pub fn new() -> Result<Self> {
        let path = dirs::config_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("voxy")
            .join("settings.toml");

        let (change_tx, _) = broadcast::channel(16);

        let settings = if path.exists() {
            Self::load_from_file(&path)?
        } else {
            SettingsSnapshot::default()
        };

        Ok(Self {
            settings: RwLock::new(settings),
            history: RwLock::new(Vec::new()),
            max_history: 10,
            path,
            change_tx,
        })
    }

    /// Create a new settings manager with a custom file path.
    pub fn with_path(path: std::path::PathBuf) -> Result<Self> {
        let (change_tx, _) = broadcast::channel(16);

        let settings = if path.exists() {
            Self::load_from_file(&path)?
        } else {
            SettingsSnapshot::default()
        };

        Ok(Self {
            settings: RwLock::new(settings),
            history: RwLock::new(Vec::new()),
            max_history: 10,
            path,
            change_tx,
        })
    }

    /// Get current settings.
    pub fn get(&self) -> SettingsSnapshot {
        self.settings.read().clone()
    }

    /// Update settings with validation.
    pub fn update(&self, new_settings: SettingsSnapshot) -> Result<()> {
        self.validate(&new_settings)?;

        let mut history = self.history.write();
        let current = self.settings.read().clone();
        if history.len() >= self.max_history {
            history.remove(0);
        }
        history.push(current);
        drop(history);

        *self.settings.write() = new_settings.clone();
        self.save_to_file(&self.path)?;
        let _ = self.change_tx.send(new_settings);

        info!("Settings updated and saved");
        Ok(())
    }

    /// Rollback to previous settings.
    pub fn rollback(&self) -> Result<()> {
        let previous = {
            let mut history = self.history.write();
            history.pop()
        };

        if let Some(prev) = previous {
            *self.settings.write() = prev.clone();
            self.save_to_file(&self.path)?;
            let _ = self.change_tx.send(prev);
            info!("Settings rolled back");
            Ok(())
        } else {
            Err(RuntimeError::Settings(
                "No previous settings to rollback".to_string(),
            ))
        }
    }

    /// Subscribe to settings changes.
    pub fn subscribe(&self) -> watch::Receiver<SettingsSnapshot> {
        let (tx, rx) = watch::channel(self.get());
        let mut broadcast_rx = self.change_tx.subscribe();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            while let Ok(settings) = broadcast_rx.recv().await {
                let _ = tx_clone.send(settings);
            }
        });

        rx
    }

    /// Validate settings.
    pub fn validate(&self, settings: &SettingsSnapshot) -> Result<()> {
        let mut errors = Vec::new();

        if settings.voice.wake_word_sensitivity < 0.0 || settings.voice.wake_word_sensitivity > 1.0
        {
            errors.push(ValidationError {
                field: "voice.wake_word_sensitivity".to_string(),
                message: "Must be between 0.0 and 1.0".to_string(),
            });
        }

        if settings.models.temperature < 0.0 || settings.models.temperature > 2.0 {
            errors.push(ValidationError {
                field: "models.temperature".to_string(),
                message: "Must be between 0.0 and 2.0".to_string(),
            });
        }

        if settings.performance.max_memory_mb == 0 {
            errors.push(ValidationError {
                field: "performance.max_memory_mb".to_string(),
                message: "Must be greater than 0".to_string(),
            });
        }

        if settings.performance.max_cpu_percent <= 0.0
            || settings.performance.max_cpu_percent > 100.0
        {
            errors.push(ValidationError {
                field: "performance.max_cpu_percent".to_string(),
                message: "Must be between 0 and 100".to_string(),
            });
        }

        if settings.appearance.opacity < 0.1 || settings.appearance.opacity > 1.0 {
            errors.push(ValidationError {
                field: "appearance.opacity".to_string(),
                message: "Must be between 0.1 and 1.0".to_string(),
            });
        }

        if settings.appearance.font_size < 8 || settings.appearance.font_size > 72 {
            errors.push(ValidationError {
                field: "appearance.font_size".to_string(),
                message: "Must be between 8 and 72".to_string(),
            });
        }

        if settings.companion.character_size < 0.2 || settings.companion.character_size > 3.0 {
            errors.push(ValidationError {
                field: "companion.character_size".to_string(),
                message: "Must be between 0.2 and 3.0".to_string(),
            });
        }

        if settings.voice.speech_speed < 0.25 || settings.voice.speech_speed > 3.0 {
            errors.push(ValidationError {
                field: "voice.speech_speed".to_string(),
                message: "Must be between 0.25 and 3.0".to_string(),
            });
        }

        if settings.voice.wake_words.len() > 10 {
            errors.push(ValidationError {
                field: "voice.wake_words".to_string(),
                message: "A maximum of 10 wake words is supported".to_string(),
            });
        }

        if !settings.profile.user_email.is_empty() && !settings.profile.user_email.contains('@') {
            errors.push(ValidationError {
                field: "profile.user_email".to_string(),
                message: "Must be a valid email address".to_string(),
            });
        }

        if !errors.is_empty() {
            let msg: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            return Err(RuntimeError::Settings(format!(
                "Validation failed: {}",
                msg.join("; ")
            )));
        }

        Ok(())
    }

    /// Hot reload loop.
    pub async fn hot_reload_loop(
        &self,
        interval: std::time::Duration,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) {
        let mut last_modified = self.get_modified_time();
        let mut ticker = tokio::time::interval(interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let current = self.get_modified_time();
                    if let (Some(cur), Some(last)) = (current, last_modified) {
                        if cur > last {
                            match Self::load_from_file(&self.path) {
                                Ok(new_settings) => {
                                    if let Err(e) = self.validate(&new_settings) {
                                        warn!("Hot reload validation failed: {}", e);
                                    } else {
                                        let mut history = self.history.write();
                                        let old = self.settings.read().clone();
                                        if history.len() >= self.max_history {
                                            history.remove(0);
                                        }
                                        history.push(old);
                                        drop(history);

                                        *self.settings.write() = new_settings.clone();
                                        let _ = self.change_tx.send(new_settings);
                                        info!("Settings hot-reloaded from file");
                                    }
                                }
                                Err(e) => error!("Failed to load settings: {}", e),
                            }
                            last_modified = Some(cur);
                        }
                    } else if current.is_some() {
                        last_modified = current;
                    }
                }
                _ = shutdown_rx.recv() => break,
            }
        }
    }

    fn get_modified_time(&self) -> Option<std::time::SystemTime> {
        std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok())
    }

    fn load_from_file(path: &Path) -> Result<SettingsSnapshot> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| RuntimeError::Settings(format!("Read failed: {}", e)))?;

        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            serde_json::from_str(&content)
                .map_err(|e| RuntimeError::Settings(format!("JSON parse failed: {}", e)))
        } else {
            toml::from_str(&content)
                .map_err(|e| RuntimeError::Settings(format!("TOML parse failed: {}", e)))
        }
    }

    fn save_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            serde_json::to_string_pretty(&*self.settings.read())
                .map_err(|e| RuntimeError::Settings(format!("JSON serialize failed: {}", e)))?
        } else {
            toml::to_string_pretty(&*self.settings.read())
                .map_err(|e| RuntimeError::Settings(format!("TOML serialize failed: {}", e)))?
        };

        // Atomic write: serialize to a unique temp file first, then rename it
        // over the target. Concurrent writers and readers never observe a
        // partially-written settings file.
        let temp_path = path.with_extension(format!(
            "tmp{}_{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        if let Err(e) = std::fs::write(&temp_path, &content) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(e.into());
        }
        if let Err(e) = std::fs::rename(&temp_path, path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(e.into());
        }

        Ok(())
    }

    /// Get the settings file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults() {
        let s = SettingsSnapshot::default();
        assert_eq!(s.app_name, "OSMOO");
        assert!(s.voice.enabled);
        assert!(s.privacy.local_processing_only);
    }

    #[test]
    fn settings_serializes_toml() {
        let s = SettingsSnapshot::default();
        let toml = toml::to_string_pretty(&s).unwrap();
        assert!(toml.contains("wake_word"));
        let deserialized: SettingsSnapshot = toml::from_str(&toml).unwrap();
        assert_eq!(deserialized.app_name, s.app_name);
    }

    #[test]
    fn settings_serializes_json() {
        let s = SettingsSnapshot::default();
        let json = serde_json::to_string_pretty(&s).unwrap();
        let deserialized: SettingsSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.app_name, s.app_name);
    }

    #[test]
    fn settings_manager_new() {
        let mgr = SettingsManager::new().unwrap();
        let settings = mgr.get();
        assert_eq!(settings.app_name, "OSMOO");
    }

    #[test]
    fn settings_validate_valid() {
        let mgr = SettingsManager::new().unwrap();
        let s = SettingsSnapshot::default();
        assert!(mgr.validate(&s).is_ok());
    }

    #[test]
    fn settings_validate_invalid_sensitivity() {
        let mgr = SettingsManager::new().unwrap();
        let mut s = SettingsSnapshot::default();
        s.voice.wake_word_sensitivity = 1.5;
        assert!(mgr.validate(&s).is_err());
    }

    #[test]
    fn settings_validate_invalid_temperature() {
        let mgr = SettingsManager::new().unwrap();
        let mut s = SettingsSnapshot::default();
        s.models.temperature = 3.0;
        assert!(mgr.validate(&s).is_err());
    }

    #[test]
    fn settings_rollback_no_history() {
        let mgr = SettingsManager::new().unwrap();
        assert!(mgr.rollback().is_err());
    }

    #[test]
    fn settings_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");

        let s = SettingsSnapshot::default();
        let toml = toml::to_string_pretty(&s).unwrap();
        std::fs::write(&path, &toml).unwrap();

        let loaded = SettingsManager::load_from_file(&path).unwrap();
        assert_eq!(loaded.app_name, s.app_name);
    }

    #[test]
    fn settings_atomic_save_replaces_file_and_cleans_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        std::fs::write(&path, "garbage").unwrap();

        let (change_tx, _) = broadcast::channel(16);
        let mgr = SettingsManager {
            settings: RwLock::new(SettingsSnapshot::default()),
            history: RwLock::new(Vec::new()),
            max_history: 10,
            path: path.clone(),
            change_tx,
        };

        mgr.save_to_file(&path).unwrap();
        let loaded = SettingsManager::load_from_file(&path).unwrap();
        assert_eq!(
            loaded.app_name, "OSMOO",
            "old content must be fully replaced"
        );

        let leftovers: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("tmp"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "no temp files may be left behind: {:?}",
            leftovers
        );
    }

    #[test]
    fn settings_expanded_sections_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings_expanded.toml");

        let mut s = SettingsSnapshot::default();
        s.profile.user_name = "Alex Mercer".to_string();
        s.profile.language = "es".to_string();
        s.companion.character_id = "dragon".to_string();
        s.companion.character_size = 1.25;
        s.companion.expression = "focused".to_string();
        s.security.file_system_access = "workspace_only".to_string();
        s.security.require_approval_financial = true;
        s.models.enable_fallback = false;
        s.models.coding_model = "claude-3-5-sonnet-20241022".to_string();
        s.voice.wake_words = vec![
            "Hey OSMOO".to_string(),
            "OSMOO".to_string(),
            "Dragon".to_string(),
        ];
        s.voice.speech_speed = 1.15;

        let toml_str = toml::to_string_pretty(&s).unwrap();
        std::fs::write(&path, &toml_str).unwrap();

        let loaded = SettingsManager::load_from_file(&path).unwrap();
        assert_eq!(loaded.profile.user_name, "Alex Mercer");
        assert_eq!(loaded.profile.language, "es");
        assert_eq!(loaded.companion.character_id, "dragon");
        assert_eq!(loaded.companion.character_size, 1.25);
        assert_eq!(loaded.companion.expression, "focused");
        assert!(!loaded.models.enable_fallback);
        assert_eq!(loaded.models.coding_model, "claude-3-5-sonnet-20241022");
        assert_eq!(loaded.voice.wake_words.len(), 3);
        assert_eq!(loaded.voice.speech_speed, 1.15);
    }

    #[test]
    fn settings_validate_wake_words_limit() {
        let mgr = SettingsManager::new().unwrap();
        let mut s = SettingsSnapshot::default();
        // 11 wake words exceeds maximum of 10
        s.voice.wake_words = (1..=11).map(|i| format!("WakeWord{}", i)).collect();
        let res = mgr.validate(&s);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("maximum of 10 wake words"));
    }

    #[test]
    fn settings_validate_character_size_bounds() {
        let mgr = SettingsManager::new().unwrap();
        let mut s = SettingsSnapshot::default();
        s.companion.character_size = 4.0;
        assert!(mgr.validate(&s).is_err());
        s.companion.character_size = 0.1;
        assert!(mgr.validate(&s).is_err());
        s.companion.character_size = 1.0;
        assert!(mgr.validate(&s).is_ok());
    }

    #[test]
    fn settings_strict_routing_policy_preserved() {
        let s = SettingsSnapshot::default();
        assert!(
            !s.models.enable_fallback,
            "OSMOO strict model routing must default to enable_fallback = false"
        );
    }
}
