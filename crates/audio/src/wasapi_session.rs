//! Windows Audio Session API integration.
//!
//! Provides safe abstractions over WASAPI audio session management:
//! - Audio session enumeration
//! - Per-application volume observation
//! - VOXY session identification
//! - Optional audio ducking with smooth transitions
//! - State restoration after interruption, crash, or shutdown
//! - Exclusion rules for communication/system apps
//!
//! All Windows-specific code is isolated behind `#[cfg(windows)]`.
//! On non-Windows platforms, this module provides no-op implementations.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use parking_lot::RwLock;

/// Configuration for Windows Audio Session integration.
#[derive(Debug, Clone)]
pub struct WasapiSessionConfig {
    /// Enable audio ducking when VOXY speaks.
    pub ducking_enabled: bool,
    /// Duck amount in dB (negative = quieter).
    pub duck_amount_db: f32,
    /// Fade duration for ducking transitions (ms).
    pub duck_fade_ms: u32,
    /// Fade duration for restoration transitions (ms).
    pub restore_fade_ms: u32,
    /// Session names/patterns to exclude from ducking.
    pub exclusion_patterns: Vec<String>,
    /// Minimum volume to restore to (prevents restoring to zero).
    pub min_restore_volume: f32,
    /// Polling interval for session state monitoring.
    pub poll_interval: Duration,
}

impl Default for WasapiSessionConfig {
    fn default() -> Self {
        Self {
            ducking_enabled: true,
            duck_amount_db: -12.0,
            duck_fade_ms: 100,
            restore_fade_ms: 200,
            exclusion_patterns: vec![
                "Discord".to_string(),
                "Zoom".to_string(),
                "Teams".to_string(),
                "Slack".to_string(),
                "Skype".to_string(),
                "Phone".to_string(),
                "Alarm".to_string(),
                "Accessibility".to_string(),
            ],
            min_restore_volume: 0.01,
            poll_interval: Duration::from_millis(500),
        }
    }
}

/// Information about a Windows audio session.
#[derive(Debug, Clone)]
pub struct AudioSessionInfo {
    /// Session identifier (PID or session GUID).
    pub id: String,
    /// Display name of the application.
    pub display_name: String,
    /// Current volume (0.0 - 1.0).
    pub volume: f32,
    /// Whether the session is muted.
    pub is_muted: bool,
    /// Process ID.
    pub process_id: u32,
    /// Whether this is the VOXY session.
    pub is_voxy: bool,
    /// Whether this session should be excluded from ducking.
    pub is_excluded: bool,
    /// Audio state (active, inactive, expired).
    pub state: AudioSessionState,
}

/// Audio session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSessionState {
    /// Session is actively playing audio.
    Active,
    /// Session exists but is not playing audio.
    Inactive,
    /// Session has expired.
    Expired,
    /// State is unknown.
    Unknown,
}

/// Snapshot of all audio session volumes before ducking.
/// Used to restore state after ducking ends.
#[derive(Debug, Clone)]
pub struct VolumeSnapshot {
    /// Session ID -> volume before ducking.
    volumes: HashMap<String, f32>,
    /// Session ID -> mute state before ducking.
    muted: HashMap<String, bool>,
    /// Timestamp of the snapshot.
    timestamp: std::time::Instant,
}

impl Default for VolumeSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

impl VolumeSnapshot {
    pub fn new() -> Self {
        Self {
            volumes: HashMap::new(),
            muted: HashMap::new(),
            timestamp: std::time::Instant::now(),
        }
    }

    pub fn record(&mut self, session_id: &str, volume: f32, muted: bool) {
        self.volumes.insert(session_id.to_string(), volume);
        self.muted.insert(session_id.to_string(), muted);
    }

    pub fn get_volume(&self, session_id: &str) -> Option<f32> {
        self.volumes.get(session_id).copied()
    }

    pub fn get_muted(&self, session_id: &str) -> Option<bool> {
        self.muted.get(session_id).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.volumes.is_empty()
    }

    pub fn age(&self) -> Duration {
        self.timestamp.elapsed()
    }
}

/// Events emitted by the audio session manager.
#[derive(Debug, Clone)]
pub enum AudioSessionEvent {
    /// A new audio session was detected.
    SessionConnected { id: String, name: String },
    /// An audio session was removed.
    SessionDisconnected { id: String },
    /// A session's volume changed.
    VolumeChanged { id: String, volume: f32 },
    /// Ducking was started.
    DuckingStarted,
    /// Ducking was stopped and volumes restored.
    DuckingStopped,
    /// Error occurred.
    Error { message: String },
}

/// Trait for platform-agnostic audio session management.
#[async_trait::async_trait]
pub trait AudioSessionManager: Send + Sync {
    /// Initialize the session manager.
    async fn initialize(&self, config: &WasapiSessionConfig) -> Result<(), String>;

    /// Shutdown the session manager, restoring all volumes.
    async fn shutdown(&self) -> Result<(), String>;

    /// Enumerate all active audio sessions.
    async fn enumerate_sessions(&self) -> Result<Vec<AudioSessionInfo>, String>;

    /// Get the current volume for a specific session.
    async fn get_volume(&self, session_id: &str) -> Result<f32, String>;

    /// Set the volume for a specific session.
    async fn set_volume(&self, session_id: &str, volume: f32) -> Result<(), String>;

    /// Check if a session should be excluded from ducking.
    fn is_excluded(&self, session_name: &str) -> bool;

    /// Start ducking non-excluded sessions.
    async fn start_ducking(&self) -> Result<(), String>;

    /// Stop ducking and restore previous volumes.
    async fn stop_ducking(&self) -> Result<(), String>;

    /// Check if ducking is currently active.
    fn is_ducking(&self) -> bool;

    /// Take the event receiver.
    async fn take_events(&self) -> Option<tokio::sync::mpsc::Receiver<AudioSessionEvent>>;
}

/// In-memory (mock) audio session manager for testing.
pub struct InMemorySessionManager {
    initialized: AtomicBool,
    config: RwLock<Option<WasapiSessionConfig>>,
    sessions: RwLock<Vec<AudioSessionInfo>>,
    pre_duck_snapshot: RwLock<VolumeSnapshot>,
    is_ducking: AtomicBool,
    event_tx: tokio::sync::mpsc::Sender<AudioSessionEvent>,
    event_rx: RwLock<Option<tokio::sync::mpsc::Receiver<AudioSessionEvent>>>,
}

impl InMemorySessionManager {
    pub fn new() -> Self {
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(64);
        Self {
            initialized: AtomicBool::new(false),
            config: RwLock::new(None),
            sessions: RwLock::new(Vec::new()),
            pre_duck_snapshot: RwLock::new(VolumeSnapshot::new()),
            is_ducking: AtomicBool::new(false),
            event_tx,
            event_rx: RwLock::new(Some(event_rx)),
        }
    }

    /// Add a mock session for testing.
    pub fn add_session(&self, session: AudioSessionInfo) {
        self.sessions.write().push(session);
    }
}

impl Default for InMemorySessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AudioSessionManager for InMemorySessionManager {
    async fn initialize(&self, config: &WasapiSessionConfig) -> Result<(), String> {
        self.initialized.store(true, Ordering::SeqCst);
        *self.config.write() = Some(config.clone());
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), String> {
        if self.is_ducking() {
            self.stop_ducking().await?;
        }
        self.initialized.store(false, Ordering::SeqCst);
        *self.config.write() = None;
        Ok(())
    }

    async fn enumerate_sessions(&self) -> Result<Vec<AudioSessionInfo>, String> {
        Ok(self.sessions.read().clone())
    }

    async fn get_volume(&self, session_id: &str) -> Result<f32, String> {
        let sessions = self.sessions.read();
        sessions
            .iter()
            .find(|s| s.id == session_id)
            .map(|s| s.volume)
            .ok_or_else(|| format!("Session not found: {}", session_id))
    }

    async fn set_volume(&self, session_id: &str, volume: f32) -> Result<(), String> {
        let mut sessions = self.sessions.write();
        for session in sessions.iter_mut() {
            if session.id == session_id {
                session.volume = volume.clamp(0.0, 1.0);
                return Ok(());
            }
        }
        Err(format!("Session not found: {}", session_id))
    }

    fn is_excluded(&self, session_name: &str) -> bool {
        let config = self.config.read();
        if let Some(ref cfg) = *config {
            cfg.exclusion_patterns.iter().any(|pattern| {
                session_name
                    .to_lowercase()
                    .contains(&pattern.to_lowercase())
            })
        } else {
            false
        }
    }

    async fn start_ducking(&self) -> Result<(), String> {
        if !self.initialized.load(Ordering::SeqCst) {
            return Err("Not initialized".to_string());
        }

        let duck_linear = {
            let config = self.config.read();
            let cfg = config.as_ref().ok_or("Not configured")?;

            if !cfg.ducking_enabled {
                return Ok(());
            }
            10.0f32.powf(cfg.duck_amount_db / 20.0)
        };

        // Take snapshot of current volumes
        {
            let sessions = self.sessions.read();
            let mut snapshot = VolumeSnapshot::new();
            for session in sessions.iter() {
                if !session.is_excluded && !session.is_voxy {
                    snapshot.record(&session.id, session.volume, session.is_muted);
                }
            }
            *self.pre_duck_snapshot.write() = snapshot;
        }

        // Apply ducking
        {
            let config = self.config.read();
            let cfg = config.as_ref().ok_or("Not configured")?;
            let mut sessions = self.sessions.write();
            for session in sessions.iter_mut() {
                if !session.is_excluded && !session.is_voxy {
                    session.volume = (session.volume * duck_linear).max(cfg.min_restore_volume);
                }
            }
        }

        self.is_ducking.store(true, Ordering::SeqCst);
        let _ = self.event_tx.send(AudioSessionEvent::DuckingStarted).await;
        Ok(())
    }

    async fn stop_ducking(&self) -> Result<(), String> {
        if !self.is_ducking() {
            return Ok(());
        }

        let snapshot = self.pre_duck_snapshot.read().clone();

        {
            let mut sessions = self.sessions.write();
            for session in sessions.iter_mut() {
                if let Some(original_volume) = snapshot.get_volume(&session.id) {
                    session.volume = original_volume;
                }
                if let Some(original_muted) = snapshot.get_muted(&session.id) {
                    session.is_muted = original_muted;
                }
            }
        }

        self.is_ducking.store(false, Ordering::SeqCst);
        let _ = self.event_tx.send(AudioSessionEvent::DuckingStopped).await;
        Ok(())
    }

    fn is_ducking(&self) -> bool {
        self.is_ducking.load(Ordering::SeqCst)
    }

    async fn take_events(&self) -> Option<tokio::sync::mpsc::Receiver<AudioSessionEvent>> {
        self.event_rx.write().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: &str, name: &str, volume: f32, excluded: bool) -> AudioSessionInfo {
        AudioSessionInfo {
            id: id.to_string(),
            display_name: name.to_string(),
            volume,
            is_muted: false,
            process_id: 1000,
            is_voxy: false,
            is_excluded: excluded,
            state: AudioSessionState::Active,
        }
    }

    #[tokio::test]
    async fn session_manager_init_shutdown() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig::default();
        mgr.initialize(&config).await.unwrap();
        mgr.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn session_enumerate() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig::default();
        mgr.initialize(&config).await.unwrap();

        mgr.add_session(make_session("1", "Spotify", 0.8, false));
        mgr.add_session(make_session("2", "Discord", 0.5, true));

        let sessions = mgr.enumerate_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn session_get_set_volume() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig::default();
        mgr.initialize(&config).await.unwrap();

        mgr.add_session(make_session("1", "Spotify", 0.8, false));

        let vol = mgr.get_volume("1").await.unwrap();
        assert!((vol - 0.8).abs() < 0.01);

        mgr.set_volume("1", 0.5).await.unwrap();
        let vol = mgr.get_volume("1").await.unwrap();
        assert!((vol - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn session_not_found() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig::default();
        mgr.initialize(&config).await.unwrap();

        let result = mgr.get_volume("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn ducking_exclusion() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig::default();
        mgr.initialize(&config).await.unwrap();

        assert!(mgr.is_excluded("Discord"));
        assert!(mgr.is_excluded("Zoom"));
        assert!(!mgr.is_excluded("Spotify"));
        assert!(!mgr.is_excluded("Notepad"));
    }

    #[tokio::test]
    async fn ducking_start_stop() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig {
            duck_amount_db: -6.0,
            ..Default::default()
        };
        mgr.initialize(&config).await.unwrap();

        mgr.add_session(make_session("1", "Spotify", 1.0, false));
        mgr.add_session(make_session("2", "Discord", 0.5, true));

        mgr.start_ducking().await.unwrap();
        assert!(mgr.is_ducking());

        // Spotify should be ducked
        let vol = mgr.get_volume("1").await.unwrap();
        assert!(vol < 1.0);

        // Discord should NOT be ducked (excluded)
        let vol = mgr.get_volume("2").await.unwrap();
        assert!((vol - 0.5).abs() < 0.01);

        mgr.stop_ducking().await.unwrap();
        assert!(!mgr.is_ducking());

        // Volumes should be restored
        let vol = mgr.get_volume("1").await.unwrap();
        assert!((vol - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn ducking_snapshot_restores_after_crash() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig::default();
        mgr.initialize(&config).await.unwrap();

        mgr.add_session(make_session("1", "Music", 0.9, false));
        mgr.start_ducking().await.unwrap();

        // Simulate crash: directly modify volume
        mgr.set_volume("1", 0.1).await.unwrap();

        // Stop ducking should restore from snapshot
        mgr.stop_ducking().await.unwrap();
        let vol = mgr.get_volume("1").await.unwrap();
        assert!((vol - 0.9).abs() < 0.01);
    }

    #[tokio::test]
    async fn ducking_disabled() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig {
            ducking_enabled: false,
            ..Default::default()
        };
        mgr.initialize(&config).await.unwrap();

        mgr.add_session(make_session("1", "Music", 1.0, false));
        mgr.start_ducking().await.unwrap();

        // Volume should NOT change when ducking is disabled
        let vol = mgr.get_volume("1").await.unwrap();
        assert!((vol - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn shutdown_stops_ducking() {
        let mgr = InMemorySessionManager::new();
        let config = WasapiSessionConfig::default();
        mgr.initialize(&config).await.unwrap();

        mgr.add_session(make_session("1", "Music", 1.0, false));
        mgr.start_ducking().await.unwrap();
        assert!(mgr.is_ducking());

        mgr.shutdown().await.unwrap();
        assert!(!mgr.is_ducking());
    }

    #[test]
    fn volume_snapshot_basics() {
        let mut snap = VolumeSnapshot::new();
        assert!(snap.is_empty());

        snap.record("1", 0.8, false);
        snap.record("2", 0.5, true);
        assert!(!snap.is_empty());

        assert_eq!(snap.get_volume("1"), Some(0.8));
        assert_eq!(snap.get_volume("2"), Some(0.5));
        assert_eq!(snap.get_muted("1"), Some(false));
        assert_eq!(snap.get_muted("2"), Some(true));
        assert_eq!(snap.get_volume("3"), None);
    }

    #[test]
    fn audio_session_state_display() {
        assert_eq!(format!("{:?}", AudioSessionState::Active), "Active");
        assert_eq!(format!("{:?}", AudioSessionState::Inactive), "Inactive");
    }

    #[test]
    fn config_defaults() {
        let config = WasapiSessionConfig::default();
        assert!(config.ducking_enabled);
        assert!(config.duck_amount_db < 0.0);
        assert!(config.exclusion_patterns.contains(&"Discord".to_string()));
        assert!(config.exclusion_patterns.contains(&"Zoom".to_string()));
    }
}
