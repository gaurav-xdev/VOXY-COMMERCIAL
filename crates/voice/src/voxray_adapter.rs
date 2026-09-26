use std::sync::Arc;
use async_trait::async_trait;
use chrono::Utc;
use voxy_voice_orchestrator::{
    AudioChunk, AudioStream, Result, SttEngine, TtsEngine, VoiceOrchestratorError,
};
use voxy_voxray::stt::VoxraySttService;
use voxy_voxray::tts::VoxrayTtsService;

/// Adapter connecting VoxraySttService into voxy_voice_orchestrator::SttEngine.
pub struct VoxraySttEngine {
    name: String,
    service: Arc<dyn VoxraySttService>,
}

impl VoxraySttEngine {
    pub fn new(service: Arc<dyn VoxraySttService>) -> Self {
        Self {
            name: "voxray-stt".into(),
            service,
        }
    }
}

#[async_trait]
impl SttEngine for VoxraySttEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn transcribe(&self, audio: &AudioChunk) -> Result<String> {
        let mut pcm = Vec::with_capacity(audio.data.len() * 2);
        for sample in &audio.data {
            let s = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
            pcm.extend_from_slice(&s.to_le_bytes());
        }
        self.service
            .transcribe(&pcm, audio.sample_rate, audio.channels as u16)
            .await
            .map_err(VoiceOrchestratorError::TranscriptionFailed)
    }

    async fn transcribe_stream(&self, mut stream: Box<dyn AudioStream>) -> Result<String> {
        let mut accum_pcm = Vec::new();
        let mut sr = stream.sample_rate();
        let mut ch = stream.channels() as u16;
        while let Some(chunk) = stream.next_chunk().await {
            sr = chunk.sample_rate;
            ch = chunk.channels as u16;
            for sample in &chunk.data {
                let s = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                accum_pcm.extend_from_slice(&s.to_le_bytes());
            }
        }
        self.service
            .transcribe(&accum_pcm, sr, ch)
            .await
            .map_err(VoiceOrchestratorError::TranscriptionFailed)
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["en".into()]
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// Single-chunk in-memory stream wrapper for TTS playback.
pub struct SingleChunkAudioStream {
    chunk: Option<AudioChunk>,
    sample_rate: u32,
    channels: u8,
}

impl SingleChunkAudioStream {
    pub fn new(chunk: AudioChunk) -> Self {
        let sample_rate = chunk.sample_rate;
        let channels = chunk.channels;
        Self {
            chunk: Some(chunk),
            sample_rate,
            channels,
        }
    }
}

#[async_trait]
impl AudioStream for SingleChunkAudioStream {
    async fn next_chunk(&mut self) -> Option<AudioChunk> {
        self.chunk.take()
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u8 {
        self.channels
    }

    fn is_complete(&self) -> bool {
        self.chunk.is_none()
    }
}

/// Adapter connecting VoxrayTtsService into voxy_voice_orchestrator::TtsEngine.
pub struct VoxrayTtsEngine {
    name: String,
    service: Arc<dyn VoxrayTtsService>,
    sample_rate: u32,
}

impl VoxrayTtsEngine {
    pub fn new(service: Arc<dyn VoxrayTtsService>, sample_rate: u32) -> Self {
        Self {
            name: "voxray-tts".into(),
            service,
            sample_rate,
        }
    }
}

#[async_trait]
impl TtsEngine for VoxrayTtsEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn synthesize(&self, text: &str) -> Result<AudioChunk> {
        let pcm_bytes = self
            .service
            .synthesize(text, self.sample_rate)
            .await
            .map_err(VoiceOrchestratorError::SynthesisFailed)?;

        let samples: Vec<f32> = pcm_bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / i16::MAX as f32)
            .collect();

        Ok(AudioChunk {
            data: samples,
            sample_rate: self.sample_rate,
            channels: 1,
            timestamp: Utc::now(),
            sequence: 0,
            is_final: true,
        })
    }

    async fn synthesize_stream(&self, text: &str) -> Result<Box<dyn AudioStream>> {
        let chunk = self.synthesize(text).await?;
        Ok(Box::new(SingleChunkAudioStream::new(chunk)))
    }

    fn list_voices(&self) -> Vec<String> {
        vec!["default".into()]
    }

    fn is_available(&self) -> bool {
        true
    }
}
