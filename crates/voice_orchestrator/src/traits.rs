use async_trait::async_trait;

use crate::config::VoiceOrchestratorConfig;
use crate::error::Result;
use crate::event::VoiceEvent;

pub struct AudioChunk {
    pub data: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u8,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub sequence: u64,
    pub is_final: bool,
}

#[async_trait]
pub trait WakeWordDetector: Send + Sync {
    fn name(&self) -> &str;
    fn wake_word(&self) -> &str;
    async fn detect(&self, audio: &AudioChunk) -> Result<Option<f32>>;
    async fn reset(&self) -> Result<()>;
    fn is_available(&self) -> bool;
}

#[async_trait]
pub trait VadDetector: Send + Sync {
    fn name(&self) -> &str;
    async fn is_voice(&self, audio: &AudioChunk) -> Result<bool>;
    async fn reset(&self) -> Result<()>;
    fn threshold(&self) -> f32;
    fn is_available(&self) -> bool;
}

#[async_trait]
pub trait SttEngine: Send + Sync {
    fn name(&self) -> &str;
    async fn transcribe(&self, audio: &AudioChunk) -> Result<String>;
    async fn transcribe_stream(&self, stream: Box<dyn AudioStream>) -> Result<String>;
    /// Transcribe the accumulated audio as an intermediate (non-final) result.
    /// Backends that cannot produce partial transcriptions return the empty
    /// string, or may re-decode the accumulated audio (see the implementing
    /// engine's documentation). It must not consume the engine's internal
    /// audio buffer.
    async fn transcribe_partial(&self, _audio: &AudioChunk) -> Result<String> {
        Ok(String::new())
    }
    fn supports_partial_transcription(&self) -> bool {
        false
    }
    fn supported_languages(&self) -> Vec<String>;
    fn is_available(&self) -> bool;
}

/// A partial or final transcript emitted while the user is still speaking.
#[derive(Debug, Clone)]
pub struct PartialTranscript {
    pub text: String,
    pub is_final: bool,
}

#[async_trait]
pub trait TtsEngine: Send + Sync {
    fn name(&self) -> &str;
    async fn synthesize(&self, text: &str) -> Result<AudioChunk>;
    async fn synthesize_stream(&self, text: &str) -> Result<Box<dyn AudioStream>>;
    fn list_voices(&self) -> Vec<String>;
    fn is_available(&self) -> bool;
}

#[async_trait]
pub trait AudioStream: Send + Sync {
    async fn next_chunk(&mut self) -> Option<AudioChunk>;
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u8;
    fn is_complete(&self) -> bool;
}

pub struct VoiceActivityState {
    pub is_active: bool,
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
    pub silence_duration_ms: u64,
    pub total_duration_ms: u64,
}

#[async_trait]
pub trait VoicePipeline: Send + Sync {
    async fn init(&self, config: &VoiceOrchestratorConfig) -> Result<()>;
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    fn is_running(&self) -> bool;
    fn config(&self) -> &VoiceOrchestratorConfig;
    async fn set_wake_word_detector(&self, detector: Box<dyn WakeWordDetector>) -> Result<()>;
    async fn set_vad_detector(&self, vad: Box<dyn VadDetector>) -> Result<()>;
    async fn set_stt_engine(&self, engine: Box<dyn SttEngine>) -> Result<()>;
    async fn set_tts_engine(&self, engine: Box<dyn TtsEngine>) -> Result<()>;
    async fn process_audio(&self, chunk: AudioChunk) -> Result<()>;
    async fn speak(&self, text: &str) -> Result<()>;
    async fn voice_activity(&self) -> VoiceActivityState;
    async fn on_event(&self, handler: Box<dyn Fn(VoiceEvent) + Send + Sync>) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VoiceOrchestratorError;

    #[test]
    fn test_audio_chunk_creation() {
        let chunk = AudioChunk {
            data: vec![0.0; 160],
            sample_rate: 16000,
            channels: 1,
            timestamp: chrono::Utc::now(),
            sequence: 0,
            is_final: false,
        };
        assert_eq!(chunk.data.len(), 160);
        assert_eq!(chunk.sample_rate, 16000);
        assert_eq!(chunk.channels, 1);
        assert!(!chunk.is_final);
    }

    #[test]
    fn test_audio_chunk_final() {
        let chunk = AudioChunk {
            data: vec![],
            sample_rate: 16000,
            channels: 1,
            timestamp: chrono::Utc::now(),
            sequence: 10,
            is_final: true,
        };
        assert!(chunk.is_final);
        assert_eq!(chunk.sequence, 10);
    }

    #[test]
    fn test_voice_activity_state_default() {
        let state = VoiceActivityState {
            is_active: false,
            last_activity: None,
            silence_duration_ms: 0,
            total_duration_ms: 0,
        };
        assert!(!state.is_active);
        assert!(state.last_activity.is_none());
        assert_eq!(state.silence_duration_ms, 0);
    }

    #[test]
    fn test_voice_activity_state_active() {
        let state = VoiceActivityState {
            is_active: true,
            last_activity: Some(chrono::Utc::now()),
            silence_duration_ms: 500,
            total_duration_ms: 3000,
        };
        assert!(state.is_active);
        assert!(state.last_activity.is_some());
        assert_eq!(state.silence_duration_ms, 500);
        assert_eq!(state.total_duration_ms, 3000);
    }

    #[test]
    fn test_voice_event_display() {
        let event = VoiceEvent::WakeWordDetected { confidence: 0.95 };
        let s = format!("{}", event);
        assert!(s.contains("Wake word detected"));
        assert!(s.contains("0.95"));

        let event = VoiceEvent::VoiceActivityStarted;
        assert_eq!(format!("{}", event), "Voice activity started");

        let event = VoiceEvent::TranscriptionResult {
            text: "hello world".into(),
            is_final: true,
            confidence: 0.9,
        };
        let s = format!("{}", event);
        assert!(s.contains("hello world"));
        assert!(s.contains("final: true"));

        let event = VoiceEvent::SynthesisCompleted { duration_ms: 1500 };
        let s = format!("{}", event);
        assert!(s.contains("1500ms"));
    }

    #[test]
    fn test_voice_event_debug() {
        let event = VoiceEvent::VoiceActivityEnded { duration_ms: 200 };
        let s = format!("{:?}", event);
        assert!(s.contains("VoiceActivityEnded"));
        assert!(s.contains("200"));
    }

    #[test]
    fn test_orchestrator_error_display() {
        let err = VoiceOrchestratorError::NoWakeWordDetector;
        assert_eq!(format!("{}", err), "No wake word detector available");

        let err = VoiceOrchestratorError::PipelineError("test".into());
        assert_eq!(format!("{}", err), "Pipeline error: test");

        let err = VoiceOrchestratorError::TranscriptionFailed("timeout".into());
        assert_eq!(format!("{}", err), "Transcription failed: timeout");
    }

    #[test]
    fn test_orchestrator_error_error_trait() {
        use std::error::Error;
        let err = VoiceOrchestratorError::PipelineNotInitialized;
        assert!(err.source().is_none());
    }
}
