use async_trait::async_trait;

use voxy_voice_orchestrator::{
    AudioChunk, AudioStream, SttEngine, TtsEngine, VoiceOrchestratorError,
};

pub struct StableStubSttEngine {
    name: String,
}

impl StableStubSttEngine {
    pub fn new() -> Self {
        Self {
            name: "stub-stt".into(),
        }
    }
}

#[async_trait]
impl SttEngine for StableStubSttEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn transcribe(&self, _audio: &AudioChunk) -> Result<String, VoiceOrchestratorError> {
        Err(VoiceOrchestratorError::TranscriptionFailed(
            "STT engine not configured. Enable Voxray STT service (e.g. CloudOpenAiSttService or EchoSttService) \
             or configure a cloud STT endpoint."
                .to_string(),
        ))
    }

    async fn transcribe_stream(
        &self,
        _stream: Box<dyn AudioStream>,
    ) -> Result<String, VoiceOrchestratorError> {
        Err(VoiceOrchestratorError::TranscriptionFailed(
            "STT streaming not configured. Enable Voxray STT service (e.g. CloudOpenAiSttService or EchoSttService) \
             or configure a cloud STT endpoint."
                .to_string(),
        ))
    }

    fn supported_languages(&self) -> Vec<String> {
        vec![]
    }

    fn is_available(&self) -> bool {
        false
    }
}

pub struct StableStubTtsEngine {
    name: String,
}

impl StableStubTtsEngine {
    pub fn new() -> Self {
        Self {
            name: "stub-tts".into(),
        }
    }
}

struct StubAudioStream {
    sample_rate: u32,
}

#[allow(dead_code)]
impl StubAudioStream {
    fn new(sample_rate: u32) -> Self {
        Self { sample_rate }
    }
}

#[async_trait]
impl AudioStream for StubAudioStream {
    async fn next_chunk(&mut self) -> Option<AudioChunk> {
        None
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn channels(&self) -> u8 {
        1
    }
    fn is_complete(&self) -> bool {
        true
    }
}

#[async_trait]
impl TtsEngine for StableStubTtsEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn synthesize(&self, _text: &str) -> Result<AudioChunk, VoiceOrchestratorError> {
        Err(VoiceOrchestratorError::SynthesisFailed(
            "TTS engine not configured. Enable Voxray TTS service (e.g. CloudOpenAiTtsService or ToneTtsService) \
             or configure a cloud TTS endpoint."
                .to_string(),
        ))
    }

    async fn synthesize_stream(
        &self,
        _text: &str,
    ) -> Result<Box<dyn AudioStream>, VoiceOrchestratorError> {
        Err(VoiceOrchestratorError::SynthesisFailed(
            "TTS streaming not configured. Enable Voxray TTS service (e.g. CloudOpenAiTtsService or ToneTtsService) \
             or configure a cloud TTS endpoint."
                .to_string(),
        ))
    }

    fn list_voices(&self) -> Vec<String> {
        vec![]
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_chunk() -> AudioChunk {
        AudioChunk {
            data: vec![0.1; 480],
            sample_rate: 16000,
            channels: 1,
            timestamp: Utc::now(),
            sequence: 0,
            is_final: false,
        }
    }

    #[tokio::test]
    async fn stub_stt_returns_error() {
        let stt = StableStubSttEngine::new();
        let err = stt.transcribe(&make_chunk()).await.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("STT engine not configured"));
    }

    #[tokio::test]
    async fn stub_stt_stream_returns_error() {
        let stt = StableStubSttEngine::new();
        let stream = Box::new(StubAudioStream { sample_rate: 16000 });
        let err = stt.transcribe_stream(stream).await.unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("STT streaming not configured"));
    }

    #[tokio::test]
    async fn stub_stt_not_available() {
        let stt = StableStubSttEngine::new();
        assert!(!stt.is_available());
        assert!(stt.supported_languages().is_empty());
        assert_eq!(stt.name(), "stub-stt");
    }

    #[tokio::test]
    async fn stub_tts_returns_error() {
        let tts = StableStubTtsEngine::new();
        match tts.synthesize("hello").await {
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("TTS engine not configured"));
            }
            Ok(_) => panic!("Expected error"),
        }
    }

    #[tokio::test]
    async fn stub_tts_stream_returns_error() {
        let tts = StableStubTtsEngine::new();
        match tts.synthesize_stream("hello").await {
            Err(e) => {
                let msg = format!("{}", e);
                assert!(msg.contains("TTS streaming not configured"));
            }
            Ok(_) => panic!("Expected error"),
        }
    }

    #[tokio::test]
    async fn stub_tts_not_available() {
        let tts = StableStubTtsEngine::new();
        assert!(!tts.is_available());
        assert!(tts.list_voices().is_empty());
        assert_eq!(tts.name(), "stub-tts");
    }

    #[tokio::test]
    async fn stub_audio_stream_immediately_complete() {
        let mut stream = StubAudioStream { sample_rate: 16000 };
        assert!(stream.is_complete());
        assert!(stream.next_chunk().await.is_none());
        assert_eq!(stream.sample_rate(), 16000);
        assert_eq!(stream.channels(), 1);
    }
}
