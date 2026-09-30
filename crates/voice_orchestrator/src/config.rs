#[derive(Clone)]
pub struct VoiceOrchestratorConfig {
    pub enabled: bool,
    pub wake_word_enabled: bool,
    pub wake_word: String,
    pub vad_enabled: bool,
    pub vad_threshold: f32,
    pub silence_timeout_ms: u64,
    pub max_duration_seconds: u64,
    pub auto_punctuate: bool,
    pub stt_timeout_seconds: u64,
    pub tts_timeout_seconds: u64,
    pub preferred_stt_provider: Option<String>,
    pub preferred_tts_provider: Option<String>,
    pub voice_activity_timeout_ms: u64,
}

impl Default for VoiceOrchestratorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            wake_word_enabled: true,
            wake_word: "hey voxy".into(),
            vad_enabled: true,
            vad_threshold: 0.5,
            // Phase 28: 1500ms of forced silence before commit added ~0.9s of
            // dead time to every turn. 600ms still avoids mid-sentence cuts
            // for natural phrase pauses while keeping turns snappy.
            silence_timeout_ms: 600,
            max_duration_seconds: 30,
            auto_punctuate: true,
            stt_timeout_seconds: 10,
            tts_timeout_seconds: 10,
            preferred_stt_provider: None,
            preferred_tts_provider: None,
            voice_activity_timeout_ms: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_config_default() {
        let config = VoiceOrchestratorConfig::default();
        assert!(config.enabled);
        assert!(config.wake_word_enabled);
        assert_eq!(config.wake_word, "hey voxy");
        assert!(config.vad_enabled);
        assert!((config.vad_threshold - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.silence_timeout_ms, 600);
        assert_eq!(config.max_duration_seconds, 30);
        assert!(config.auto_punctuate);
        assert_eq!(config.stt_timeout_seconds, 10);
        assert_eq!(config.tts_timeout_seconds, 10);
        assert!(config.preferred_stt_provider.is_none());
        assert!(config.preferred_tts_provider.is_none());
        assert_eq!(config.voice_activity_timeout_ms, 5000);
    }
}
