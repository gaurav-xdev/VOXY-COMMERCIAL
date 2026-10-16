use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct BargeInConfig {
    pub enabled: bool,
    pub sensitivity: f32,
    pub require_voice_activity: bool,
    pub debounce_ms: u64,
    pub allow_during_synthesis: bool,
    pub allow_during_processing: bool,
}

impl Default for BargeInConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sensitivity: 0.5,
            require_voice_activity: false,
            debounce_ms: 300,
            allow_during_synthesis: true,
            allow_during_processing: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SignalPacket {
    pub peak_level: f32,
}

#[derive(Debug, Clone)]
pub struct InterruptionEvent {
    pub timestamp: DateTime<Utc>,
    pub triggered_by: InterruptionSource,
    pub confidence: f32,
    pub audio_level: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterruptionSource {
    Manual,
    SignalLevel { peak: f32, threshold: f32 },
}

#[async_trait]
pub trait BargeInManager: Send + Sync {
    fn config(&self) -> &BargeInConfig;
    async fn set_config(&mut self, config: BargeInConfig);
    async fn analyze_signal(&mut self, packet: &SignalPacket) -> Result<Option<InterruptionEvent>>;
    fn is_interrupted(&self) -> bool;
    async fn clear_interruption(&mut self);
    fn last_interruption(&self) -> Option<InterruptionEvent>;
    fn interruption_count(&self) -> u64;
}

#[derive(Debug, Clone)]
pub struct InMemoryBargeInManager {
    config: BargeInConfig,
    interrupted: bool,
    last_event: Option<InterruptionEvent>,
    count: u64,
    last_audio_level: f32,
}

impl InMemoryBargeInManager {
    pub fn new(config: BargeInConfig) -> Self {
        Self {
            config,
            interrupted: false,
            last_event: None,
            count: 0,
            last_audio_level: 0.0,
        }
    }
}

#[async_trait]
impl BargeInManager for InMemoryBargeInManager {
    fn config(&self) -> &BargeInConfig {
        &self.config
    }

    async fn set_config(&mut self, config: BargeInConfig) {
        self.config = config;
    }

    async fn analyze_signal(&mut self, packet: &SignalPacket) -> Result<Option<InterruptionEvent>> {
        self.last_audio_level = packet.peak_level;
        if !self.config.enabled {
            return Ok(None);
        }
        let threshold = self.config.sensitivity;
        if packet.peak_level > threshold {
            let event = InterruptionEvent {
                timestamp: Utc::now(),
                triggered_by: InterruptionSource::SignalLevel {
                    peak: packet.peak_level,
                    threshold,
                },
                confidence: (packet.peak_level / threshold).min(1.0),
                audio_level: packet.peak_level,
            };
            self.interrupted = true;
            self.last_event = Some(event.clone());
            self.count += 1;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    fn is_interrupted(&self) -> bool {
        self.interrupted
    }

    async fn clear_interruption(&mut self) {
        self.interrupted = false;
    }

    fn last_interruption(&self) -> Option<InterruptionEvent> {
        self.last_event.clone()
    }

    fn interruption_count(&self) -> u64 {
        self.count
    }
}
