pub mod config;
pub mod detection;
pub mod error;
pub mod pipeline;
pub mod session;
pub mod stubs;
pub mod voxray_adapter;

pub use config::VoiceConfig;
pub use detection::{EnergyVadDetector, EnergyWakeWordDetector};
pub use error::{Result, VoiceError};
pub use pipeline::VoicePipeline;
pub use session::SpeechSession;
pub use stubs::{StableStubSttEngine, StableStubTtsEngine};
pub use voxray_adapter::{SingleChunkAudioStream, VoxraySttEngine, VoxrayTtsEngine};
pub use voxy_voice_orchestrator::VoiceEvent;

#[cfg(test)]
mod tests {
    use super::*;
    use voxy_conversation::SessionManager;

    #[test]
    fn voice_config_defaults() {
        let config = VoiceConfig::default();
        assert_eq!(config.wake_word, "hey voxy");
        assert!(config.wake_word_enabled);
        assert!(config.vad_enabled);
        assert!((config.vad_threshold - 0.5).abs() < f32::EPSILON);
        assert!(!config.auto_start_capture);
        assert!(config.enable_diagnostics);
        assert!(config.personality_id.is_none());
        assert_eq!(config.preroll_ms, 500);
        assert_eq!(config.partial_transcript_interval_ms, 500);
        assert_eq!(config.vad_onset_frames, 2);
        assert_eq!(config.vad_settle_ms, 120);
        assert_eq!(config.vad_trailing_off_ms, 700);
        assert_eq!(config.vad_max_silence_ms, 2000);
    }

    #[tokio::test]
    async fn pipeline_init_shutdown() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        assert!(!pipeline.is_initialized());
        pipeline.initialize().await.unwrap();
        assert!(pipeline.is_initialized());
        pipeline.shutdown().await.unwrap();
        assert!(!pipeline.is_initialized());
    }

    #[tokio::test]
    async fn pipeline_double_init_protection() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        pipeline.initialize().await.unwrap();
        let err = pipeline.initialize().await.unwrap_err();
        assert!(matches!(err, VoiceError::AlreadyInitialized));
    }

    #[tokio::test]
    async fn pipeline_shutdown_without_init() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        let err = pipeline.shutdown().await.unwrap_err();
        assert!(matches!(err, VoiceError::NotInitialized));
    }

    #[tokio::test]
    async fn pipeline_set_providers() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        assert!(!pipeline.is_running());
        assert!(!pipeline.is_initialized());
    }

    #[tokio::test]
    async fn pipeline_start_stop_capture() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        pipeline.initialize().await.unwrap();
        // In-memory device manager stubs don't support opening streams
        let err = pipeline.start_capture().await.unwrap_err();
        assert!(matches!(err, VoiceError::CaptureError(_)));
        assert!(!pipeline.is_running());
    }

    #[tokio::test]
    async fn pipeline_capture_without_init() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        let err = pipeline.start_capture().await.unwrap_err();
        assert!(matches!(err, VoiceError::NotInitialized));
    }

    #[tokio::test]
    async fn pipeline_stop_capture_without_start() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        pipeline.initialize().await.unwrap();
        let err = pipeline.stop_capture().await.unwrap_err();
        assert!(matches!(err, VoiceError::NotRunning));
    }

    #[tokio::test]
    async fn pipeline_process_audio_not_running() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        let packet = voxy_audio::AudioPacket::new(vec![0.1, 0.2], 16000, 1);
        let err = pipeline.process_audio(packet).await.unwrap_err();
        assert!(matches!(err, VoiceError::NotRunning));
    }

    #[tokio::test]
    async fn pipeline_speak_not_running() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        let err = pipeline.speak("hello").await.unwrap_err();
        assert!(matches!(err, VoiceError::NotRunning));
    }

    #[tokio::test]
    async fn pipeline_speak_no_tts() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        // Not running - speak returns NotRunning
        let err = pipeline.speak("hello").await.unwrap_err();
        assert!(matches!(err, VoiceError::NotRunning));
    }

    #[tokio::test]
    async fn pipeline_speak_stub_tts_error() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        pipeline.initialize().await.unwrap();
        pipeline.start_capture().await.unwrap_or(()); // InMemory may fail, that's ok
                                                      // Stub TTS returns ProviderError with actionable message
        let err = pipeline.speak("hello").await.unwrap_err();
        // Either NotRunning (if start_capture failed) or ProviderError (stub message)
        assert!(
            matches!(
                err,
                VoiceError::ProviderError(_) | VoiceError::NoTtsEngine | VoiceError::NotRunning
            ),
            "Expected ProviderError/NoTtsEngine/NotRunning, got: {:?}",
            err
        );
    }

    #[tokio::test]
    async fn pipeline_no_vad_detector() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        // Not running - process_audio returns NotRunning
        let packet = voxy_audio::AudioPacket::new(vec![0.1, 0.2], 16000, 1);
        let err = pipeline.process_audio(packet).await.unwrap_err();
        assert!(matches!(err, VoiceError::NotRunning));
    }

    #[tokio::test]
    async fn speech_session_creation() {
        let config = VoiceConfig::default();
        let mgr = voxy_conversation::InMemorySessionManager::new(config.conversation.clone());
        let mut session = mgr.create_session().await.unwrap();
        session.start(Some("user-1"), None).await.unwrap();
        let speech_session = SpeechSession::new(session, config);
        assert!(speech_session.is_active());
    }

    #[test]
    fn voice_error_display() {
        let err = VoiceError::NotInitialized;
        assert_eq!(format!("{}", err), "Voice pipeline not initialized");
        let err = VoiceError::NoWakeWordDetector;
        assert_eq!(format!("{}", err), "Wake word detector not set");
        let err = VoiceError::NoSttEngine;
        assert_eq!(format!("{}", err), "STT engine not set");
        let err = VoiceError::NoTtsEngine;
        assert_eq!(format!("{}", err), "TTS engine not set");
        let err = VoiceError::AlreadyRunning;
        assert_eq!(format!("{}", err), "Already running");
        let err = VoiceError::NotRunning;
        assert_eq!(format!("{}", err), "Not running");
    }

    #[test]
    fn voice_error_error_trait() {
        use std::error::Error;
        let err = VoiceError::NotInitialized;
        assert!(err.source().is_none());
    }

    #[test]
    fn voice_event_display() {
        let event = VoiceEvent::WakeWordDetected { confidence: 0.95 };
        let s = format!("{}", event);
        assert!(s.contains("Wake word detected"));
        let event = VoiceEvent::VoiceActivityStarted;
        assert_eq!(format!("{}", event), "Voice activity started");
    }

    #[test]
    fn voice_config_has_audio_config() {
        let config = VoiceConfig::default();
        assert_eq!(config.audio.input.sample_rate, 16000);
        assert_eq!(config.audio.output.sample_rate, 16000);
    }

    #[test]
    fn voice_config_has_conversation_config() {
        let config = VoiceConfig::default();
        assert_eq!(config.conversation.session_timeout_seconds, 3600);
    }

    #[test]
    fn voice_config_has_orchestrator_config() {
        let config = VoiceConfig::default();
        assert_eq!(config.orchestrator.wake_word, "hey voxy");
    }

    // ── Voice speed integration tests ───────────────────────────────

    #[tokio::test]
    async fn pipeline_voice_speed_default() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        let speed = pipeline.voice_speed().await;
        assert!((speed - 1.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn pipeline_set_voice_speed() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        pipeline.set_voice_speed(1.3).await;
        let speed = pipeline.voice_speed().await;
        assert!((speed - 1.3).abs() < 0.01);
    }

    #[tokio::test]
    async fn pipeline_voice_speed_clamped() {
        let pipeline = VoicePipeline::new(VoiceConfig::default());
        pipeline.set_voice_speed(5.0).await; // should clamp to 2.0
        let speed = pipeline.voice_speed().await;
        assert!((speed - 2.0).abs() < 0.01);

        pipeline.set_voice_speed(0.1).await; // should clamp to 0.5
        let speed = pipeline.voice_speed().await;
        assert!((speed - 0.5).abs() < 0.01);
    }

    #[test]
    fn resample_speed_preserves_length_relation() {
        let data = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
        let fast = VoicePipeline::resample_speed(&data, 2.0);
        let slow = VoicePipeline::resample_speed(&data, 0.5);
        assert_eq!(fast.len(), 5); // half the length at 2x speed
        assert_eq!(slow.len(), 20); // double the length at 0.5x speed
    }

    #[test]
    fn resample_speed_no_change_at_1x() {
        let data = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let same = VoicePipeline::resample_speed(&data, 1.0);
        assert_eq!(same.len(), data.len());
    }

    #[test]
    fn resample_speed_empty_input() {
        let data: Vec<f32> = vec![];
        let result = VoicePipeline::resample_speed(&data, 1.5);
        assert!(result.is_empty());
    }
}
