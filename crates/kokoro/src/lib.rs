use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "piper-engine")]
use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "piper-engine")]
use parking_lot::Mutex;
#[cfg(feature = "piper-engine")]
use tracing::info;
use tracing::warn;
use voxy_voice_orchestrator::{AudioChunk, AudioStream, TtsEngine};

pub use voxy_provider_core as core_traits;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Emotion {
    Neutral,
    Happy,
    Calm,
    Excited,
    Serious,
    Empathetic,
    Warning,
    Urgent,
}

impl std::str::FromStr for Emotion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "happy" | "cheerful" => Self::Happy,
            "calm" | "relaxed" | "gentle" => Self::Calm,
            "excited" | "enthusiastic" => Self::Excited,
            "serious" | "formal" | "professional" => Self::Serious,
            "empathetic" | "sympathetic" | "caring" => Self::Empathetic,
            "warning" | "caution" => Self::Warning,
            "urgent" | "alarm" => Self::Urgent,
            _ => Self::Neutral,
        })
    }
}

impl Emotion {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Happy => "happy",
            Self::Calm => "calm",
            Self::Excited => "excited",
            Self::Serious => "serious",
            Self::Empathetic => "empathetic",
            Self::Warning => "warning",
            Self::Urgent => "urgent",
        }
    }

    pub fn speed_pitch(&self) -> (f32, f32) {
        match self {
            Self::Neutral => (1.0, 1.0),
            Self::Happy => (1.15, 1.2),
            Self::Calm => (0.85, 0.9),
            Self::Excited => (1.3, 1.25),
            Self::Serious => (0.9, 0.85),
            Self::Empathetic => (0.9, 1.0),
            Self::Warning => (1.1, 1.05),
            Self::Urgent => (1.4, 1.15),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum KokoroError {
    #[error("Model not loaded")]
    ModelNotLoaded,
    #[error("Model file not found: {0}")]
    ModelNotFound(String),
    #[error("Synthesis failed: {0}")]
    SynthesisFailed(String),
}

pub struct KokoroTtsEngine {
    name: String,
    voice: String,
    emotion: Emotion,
    speed: f32,
    pitch: f32,
    sample_rate: u32,
    channels: u8,
    model_path: Option<PathBuf>,
    #[cfg(feature = "piper-engine")]
    synthesizer: Arc<Mutex<Option<piper_rs::Piper>>>,
    is_loaded: AtomicBool,
}

impl KokoroTtsEngine {
    pub fn new() -> Self {
        Self {
            name: "kokoro-tts".into(),
            voice: "default".into(),
            emotion: Emotion::Neutral,
            speed: 1.0,
            pitch: 1.0,
            sample_rate: 22050,
            channels: 1,
            model_path: None,
            #[cfg(feature = "piper-engine")]
            synthesizer: Arc::new(Mutex::new(None)),
            is_loaded: AtomicBool::new(false),
        }
    }

    pub fn with_model_path(mut self, path: PathBuf) -> Self {
        self.model_path = Some(path);
        self
    }

    pub fn with_voice(mut self, voice: &str) -> Self {
        self.voice = voice.to_string();
        self
    }

    pub fn with_emotion(mut self, style: &str) -> Self {
        self.emotion = style.parse::<Emotion>().unwrap_or(Emotion::Neutral);
        let (s, p) = self.emotion.speed_pitch();
        self.speed = s;
        self.pitch = p;
        self
    }

    pub fn emotion(&self) -> &str {
        self.emotion.as_str()
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed.clamp(0.5, 2.0);
        self
    }

    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch.clamp(0.5, 2.0);
        self
    }

    pub fn with_sample_rate(mut self, sr: u32) -> Self {
        self.sample_rate = sr;
        self
    }

    #[cfg(feature = "piper-engine")]
    pub fn load_model(&self) -> Result<(), KokoroError> {
        let path = self
            .model_path
            .as_ref()
            .ok_or(KokoroError::ModelNotLoaded)?;

        if !path.exists() {
            return Err(KokoroError::ModelNotFound(path.display().to_string()));
        }

        let model_dir = path
            .parent()
            .ok_or_else(|| KokoroError::ModelNotFound("Invalid model path".to_string()))?;

        let model_file = path;
        let config_file = {
            let stem = model_file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("model");
            let mut cfg = model_dir.join(format!("{}.json", stem));
            if !cfg.exists() {
                cfg = model_dir.join("config.json");
            }
            if !cfg.exists() {
                return Err(KokoroError::ModelNotFound(format!(
                    "No config found near {}",
                    model_file.display()
                )));
            }
            cfg
        };

        let piper = piper_rs::Piper::new(model_file, &config_file)
            .map_err(|e| KokoroError::SynthesisFailed(e.to_string()))?;

        *self.synthesizer.lock() = Some(piper);
        self.is_loaded.store(true, Ordering::SeqCst);

        info!("Piper TTS model loaded from {}", model_dir.display());
        Ok(())
    }

    #[cfg(not(feature = "piper-engine"))]
    pub fn load_model(&self) -> Result<(), KokoroError> {
        let path = self
            .model_path
            .as_ref()
            .ok_or(KokoroError::ModelNotLoaded)?;

        if !path.exists() {
            return Err(KokoroError::ModelNotFound(path.display().to_string()));
        }

        warn!(
            "piper-engine feature disabled; model at {} not loaded",
            path.display()
        );
        Ok(())
    }

    pub fn is_model_loaded(&self) -> bool {
        self.is_loaded.load(Ordering::SeqCst)
    }

    #[cfg(feature = "piper-engine")]
    #[allow(dead_code)]
    fn synthesize_internal(&self, text: &str) -> Result<Vec<f32>, KokoroError> {
        if text.is_empty() {
            return Ok(Vec::new());
        }

        let mut synth = self.synthesizer.lock();
        let piper = synth.as_mut().ok_or(KokoroError::ModelNotLoaded)?;

        let (audio, _sample_rate) = piper
            .create(text, false, None, Some(1.0 / self.speed), None, None)
            .map_err(|e| KokoroError::SynthesisFailed(e.to_string()))?;

        Ok(audio)
    }

    #[cfg(not(feature = "piper-engine"))]
    #[allow(dead_code)]
    fn synthesize_internal(&self, _text: &str) -> Result<Vec<f32>, KokoroError> {
        Ok(Vec::new())
    }

    #[cfg(feature = "piper-engine")]
    async fn synthesize_async(&self, text: String) -> Result<Vec<f32>, KokoroError> {
        if text.is_empty() {
            return Ok(Vec::new());
        }

        let piper = self.synthesizer.clone();
        let speed = self.speed;

        tokio::task::spawn_blocking(move || {
            let mut synth = piper.lock();
            let piper = synth.as_mut().ok_or(KokoroError::ModelNotLoaded)?;

            let (audio, _sample_rate) = piper
                .create(&text, false, None, Some(1.0 / speed), None, None)
                .map_err(|e| KokoroError::SynthesisFailed(e.to_string()))?;

            Ok(audio)
        })
        .await
        .map_err(|e| KokoroError::SynthesisFailed(format!("Task join error: {e}")))?
    }

    #[cfg(not(feature = "piper-engine"))]
    async fn synthesize_async(&self, _text: String) -> Result<Vec<f32>, KokoroError> {
        Ok(Vec::new())
    }
}

impl Default for KokoroTtsEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct KokoroAudioStream {
    data: Vec<f32>,
    sample_rate: u32,
    channels: u8,
    position: usize,
    chunk_size: usize,
    complete: bool,
}

impl KokoroAudioStream {
    pub fn new(data: Vec<f32>, sample_rate: u32, channels: u8) -> Self {
        Self {
            data,
            sample_rate,
            channels,
            position: 0,
            chunk_size: sample_rate as usize / 50,
            complete: false,
        }
    }
}

#[async_trait]
impl AudioStream for KokoroAudioStream {
    async fn next_chunk(&mut self) -> Option<AudioChunk> {
        if self.complete || self.position >= self.data.len() {
            self.complete = true;
            return None;
        }

        let end = (self.position + self.chunk_size).min(self.data.len());
        let chunk_data = self.data[self.position..end].to_vec();
        let is_final = end >= self.data.len();
        self.position = end;
        self.complete = is_final;

        Some(AudioChunk {
            data: chunk_data,
            sample_rate: self.sample_rate,
            channels: self.channels,
            timestamp: chrono::Utc::now(),
            sequence: 0,
            is_final,
        })
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u8 {
        self.channels
    }

    fn is_complete(&self) -> bool {
        self.complete
    }
}

#[async_trait]
impl TtsEngine for KokoroTtsEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn synthesize(
        &self,
        text: &str,
    ) -> Result<AudioChunk, voxy_voice_orchestrator::VoiceOrchestratorError> {
        if !self.is_model_loaded() {
            warn!("Piper TTS model not loaded, returning empty audio");
            return Ok(AudioChunk {
                data: Vec::new(),
                sample_rate: self.sample_rate,
                channels: self.channels,
                timestamp: chrono::Utc::now(),
                sequence: 0,
                is_final: true,
            });
        }

        let audio = self.synthesize_async(text.to_string()).await.map_err(|e| {
            voxy_voice_orchestrator::VoiceOrchestratorError::SynthesisFailed(e.to_string())
        })?;

        Ok(AudioChunk {
            data: audio,
            sample_rate: self.sample_rate,
            channels: self.channels,
            timestamp: chrono::Utc::now(),
            sequence: 0,
            is_final: true,
        })
    }

    async fn synthesize_stream(
        &self,
        text: &str,
    ) -> Result<Box<dyn AudioStream>, voxy_voice_orchestrator::VoiceOrchestratorError> {
        let audio = self.synthesize(text).await?;
        Ok(Box::new(KokoroAudioStream::new(
            audio.data,
            self.sample_rate,
            self.channels,
        )))
    }

    fn list_voices(&self) -> Vec<String> {
        vec![
            "default".to_string(),
            "male-1".to_string(),
            "female-1".to_string(),
            "neutral".to_string(),
            "happy".to_string(),
            "calm".to_string(),
            "excited".to_string(),
            "serious".to_string(),
            "empathetic".to_string(),
            "warning".to_string(),
            "urgent".to_string(),
        ]
    }

    fn is_available(&self) -> bool {
        self.is_model_loaded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kokoro_synthesize_empty() {
        let engine = KokoroTtsEngine::new();
        let result = engine.synthesize("").await.unwrap();
        assert!(result.data.is_empty());
    }

    #[tokio::test]
    async fn test_kokoro_no_model_returns_empty() {
        let engine = KokoroTtsEngine::new();
        let result = engine.synthesize("hello").await.unwrap();
        assert!(result.data.is_empty());
    }

    #[tokio::test]
    async fn test_kokoro_properties() {
        let engine = KokoroTtsEngine::new();
        assert_eq!(engine.name(), "kokoro-tts");
        assert!(!engine.is_available());
        let voices = engine.list_voices();
        assert!(voices.contains(&"default".to_string()));
    }

    #[tokio::test]
    async fn test_kokoro_custom_voice() {
        let engine = KokoroTtsEngine::new()
            .with_voice("female-1")
            .with_speed(1.2)
            .with_pitch(1.1);
        let result = engine.synthesize("hello world").await.unwrap();
        assert!(result.data.is_empty());
    }

    #[test]
    fn test_kokoro_audio_stream() {
        let data = vec![0.1; 1000];
        let stream = KokoroAudioStream::new(data, 16000, 1);
        assert!(!stream.is_complete());
        assert_eq!(stream.sample_rate(), 16000);
        assert_eq!(stream.channels(), 1);
    }

    #[test]
    fn test_kokoro_speed_clamping() {
        let _engine = KokoroTtsEngine::new().with_speed(0.1).with_speed(3.0);
    }

    #[test]
    fn test_kokoro_emotion_default() {
        let engine = KokoroTtsEngine::new();
        assert_eq!(engine.emotion(), "neutral");
    }

    #[tokio::test]
    async fn test_kokoro_emotion_happy() {
        let engine = KokoroTtsEngine::new().with_emotion("happy");
        assert_eq!(engine.emotion(), "happy");
    }

    #[test]
    fn test_kokoro_emotion_unknown_falls_to_neutral() {
        let engine = KokoroTtsEngine::new().with_emotion("unknown-style");
        assert_eq!(engine.emotion(), "neutral");
    }

    #[test]
    fn test_speed_to_length_scale_conversion() {
        let engine = KokoroTtsEngine::new();
        let speed = engine.speed;

        let length_scale = 1.0 / speed;
        assert_eq!(length_scale, 1.0);
    }

    #[test]
    fn test_speed_to_length_scale_half() {
        let engine = KokoroTtsEngine::new().with_speed(0.5);
        let length_scale = 1.0 / engine.speed;
        assert_eq!(length_scale, 2.0);
    }

    #[test]
    fn test_speed_to_length_scale_double() {
        let engine = KokoroTtsEngine::new().with_speed(2.0);
        let length_scale = 1.0 / engine.speed;
        assert_eq!(length_scale, 0.5);
    }
}
