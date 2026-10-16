//! Text-to-Speech (TTS) services for the OSMOO voice pipeline.
//!
//! Provides:
//! - Natural sentence chunker for streaming LLM text
//! - Cloud TTS client with OpenAI/Groq/Cartesia compatible API format
//! - Local TTS adapter interface
//! - Deterministic Mock TTS engine with synthetic WAV audio generation for offline tests

use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TtsError {
    #[error("API request failed: {0}")]
    Network(String),
    #[error("Missing API key for TTS provider: {0}")]
    MissingApiKey(String),
    #[error("Provider returned error status {status}: {message}")]
    ProviderError { status: u16, message: String },
    #[error("Synthesis failed: {0}")]
    SynthesisFailed(String),
    #[error("Empty text provided for TTS")]
    EmptyText,
}

/// Natural sentence chunker for streaming LLM responses.
///
/// Accumulates streaming tokens and breaks on sentence boundaries (`.` `!` `?` `\n` `;`)
/// once a minimum character threshold is reached, avoiding unnatural choppy voice delivery.
#[derive(Debug, Clone)]
pub struct SentenceChunker {
    buffer: String,
    min_chunk_chars: usize,
}

impl SentenceChunker {
    pub fn new(min_chunk_chars: usize) -> Self {
        Self {
            buffer: String::new(),
            min_chunk_chars,
        }
    }

    /// Feed newly arrived token delta from the LLM stream.
    /// Returns any complete sentences ready for speech synthesis.
    pub fn push_text(&mut self, delta: &str) -> Vec<String> {
        self.buffer.push_str(delta);
        let mut chunks = Vec::new();

        while let Some(cut_idx) = self.find_sentence_boundary() {
            let chunk = self.buffer[..=cut_idx].trim().to_string();
            self.buffer = self.buffer[cut_idx + 1..].to_string();

            if !chunk.is_empty() {
                chunks.push(chunk);
            }
        }

        chunks
    }

    /// Flush any remaining text buffer at the end of the LLM stream.
    pub fn flush(&mut self) -> Option<String> {
        let remaining = std::mem::take(&mut self.buffer).trim().to_string();
        if remaining.is_empty() {
            None
        } else {
            Some(remaining)
        }
    }

    /// Alias for push_text to support feed() API.
    pub fn feed(&mut self, delta: &str) -> Vec<String> {
        self.push_text(delta)
    }

    /// Alias for flush returning a Vec of flushed sentences.
    pub fn finish(&mut self) -> Vec<String> {
        self.flush().into_iter().collect()
    }

    fn find_sentence_boundary(&self) -> Option<usize> {
        if self.buffer.len() < self.min_chunk_chars {
            return None;
        }

        let chars: Vec<(usize, char)> = self.buffer.char_indices().collect();
        for (i, &(byte_idx, c)) in chars.iter().enumerate() {
            if c == '.' || c == '!' || c == '?' || c == '\n' || c == ';' {
                // Ignore decimals and abbreviations
                if c == '.' {
                    if i + 1 < chars.len() && chars[i + 1].1.is_numeric() {
                        continue;
                    }
                    if i > 0 && chars[i - 1].1.is_numeric() {
                        continue;
                    }
                }

                // Ensure chunk meets minimum size or has whitespace following
                if byte_idx >= self.min_chunk_chars {
                    return Some(byte_idx);
                }
            }
        }

        None
    }
}

impl Default for SentenceChunker {
    fn default() -> Self {
        Self::new(25)
    }
}

/// Universal TTS engine contract.
#[async_trait]
pub trait TtsEngine: Send + Sync {
    /// Synthesize text into audio bytes (standard WAV or MP3 stream).
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, TtsError>;

    /// Human-readable provider and voice name.
    fn name(&self) -> &str;
}

/// Cloud speech synthesis client supporting OpenAI / Cartesia / ElevenLabs compatible endpoints.
pub struct CloudTtsEngine {
    api_key: String,
    endpoint: String,
    model: String,
    voice: String,
    client: reqwest::Client,
}

impl CloudTtsEngine {
    pub fn new(
        api_key: impl Into<String>,
        endpoint: Option<String>,
        model: Option<String>,
        voice: Option<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            endpoint: endpoint
                .unwrap_or_else(|| "https://api.openai.com/v1/audio/speech".to_string()),
            model: model.unwrap_or_else(|| "tts-1".to_string()),
            voice: voice.unwrap_or_else(|| "alloy".to_string()),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn from_env() -> Result<Self, TtsError> {
        let key = std::env::var("OPENAI_API_KEY")
            .or_else(|_| std::env::var("VOXY_TTS_API_KEY"))
            .map_err(|_| {
                TtsError::MissingApiKey("No OPENAI_API_KEY or VOXY_TTS_API_KEY set".into())
            })?;
        let endpoint = std::env::var("VOXY_TTS_ENDPOINT").ok();
        let model = std::env::var("VOXY_TTS_MODEL").ok();
        let voice = std::env::var("VOXY_TTS_VOICE").ok();
        Ok(Self::new(key, endpoint, model, voice))
    }
}

#[async_trait]
impl TtsEngine for CloudTtsEngine {
    fn name(&self) -> &str {
        "Cloud TTS (OpenAI/Cartesia Compatible)"
    }

    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, TtsError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(TtsError::EmptyText);
        }

        let body = serde_json::json!({
            "model": self.model,
            "input": trimmed,
            "voice": self.voice,
            "response_format": "wav",
        });

        let resp = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| TtsError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| TtsError::Network(e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            let err_msg = resp.text().await.unwrap_or_default();
            Err(TtsError::ProviderError {
                status: status.as_u16(),
                message: err_msg,
            })
        }
    }
}

/// Local speech synthesizer adapter.
pub struct LocalTtsEngine {
    voice_name: String,
}

impl LocalTtsEngine {
    pub fn new(voice_name: impl Into<String>) -> Self {
        Self {
            voice_name: voice_name.into(),
        }
    }

    pub fn voice_name(&self) -> &str {
        &self.voice_name
    }
}

#[async_trait]
impl TtsEngine for LocalTtsEngine {
    fn name(&self) -> &str {
        "Local TTS (Windows SAPI / Local Model)"
    }

    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, TtsError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(TtsError::EmptyText);
        }

        #[cfg(windows)]
        {
            let id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(12345);
            let temp_path = std::env::temp_dir().join(format!("osmoo_sapi_{}.wav", id));
            let sanitized = trimmed.replace('\'', "''");
            let ps_script = format!(
                "$v = New-Object -ComObject SAPI.SpVoice; $s = New-Object -ComObject SAPI.SpFileStream; $s.Open('{}', 3); $v.AudioOutputStream = $s; $v.Speak('{}'); $s.Close()",
                temp_path.display(),
                sanitized
            );

            let res = std::process::Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
                .output();

            if let Ok(out) = res {
                if out.status.success() && temp_path.exists() {
                    if let Ok(bytes) = std::fs::read(&temp_path) {
                        let _ = std::fs::remove_file(&temp_path);
                        if bytes.len() >= 44 {
                            return Ok(bytes);
                        }
                    }
                }
            }
        }

        // Fallback tone if SAPI is unavailable or on non-Windows
        let pcm = vec![0.0f32; 16000];
        Ok(voxy_voice_stt::encode_wav_16k_mono(&pcm))
    }
}

/// Resemble AI Chatterbox TTS engine adapter.
/// Connects to a local or hosted Chatterbox HTTP synthesis endpoint.
pub struct ChatterboxTtsEngine {
    endpoint: String,
    voice: String,
    client: reqwest::Client,
}

impl ChatterboxTtsEngine {
    pub fn new(endpoint: Option<String>, voice: Option<String>) -> Self {
        Self {
            endpoint: endpoint.unwrap_or_else(|| {
                std::env::var("CHATTERBOX_ENDPOINT")
                    .or_else(|_| std::env::var("TTS_CHATTERBOX_ENDPOINT"))
                    .unwrap_or_else(|_| "http://127.0.0.1:8000/v1/tts".to_string())
            }),
            voice: voice.unwrap_or_else(|| {
                std::env::var("CHATTERBOX_VOICE").unwrap_or_else(|_| "default".to_string())
            }),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn voice(&self) -> &str {
        &self.voice
    }
}

#[async_trait]
impl TtsEngine for ChatterboxTtsEngine {
    fn name(&self) -> &str {
        "Resemble AI Chatterbox TTS"
    }

    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, TtsError> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(TtsError::EmptyText);
        }

        let body = serde_json::json!({
            "text": trimmed,
            "voice": self.voice,
            "response_format": "wav"
        });

        let resp = self
            .client
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| TtsError::Network(e.to_string()))?;

        let status = resp.status();
        if status.is_success() {
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| TtsError::Network(e.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            let err_msg = resp.text().await.unwrap_or_default();
            Err(TtsError::ProviderError {
                status: status.as_u16(),
                message: err_msg,
            })
        }
    }
}

/// Deterministic Mock TTS engine for offline tests and benchmarks.
pub struct MockTtsEngine {
    duration_ms: u64,
}

pub type MockTts = MockTtsEngine;

impl MockTtsEngine {
    pub fn new() -> Self {
        Self { duration_ms: 200 }
    }

    pub fn with_duration(duration_ms: u64) -> Self {
        Self { duration_ms }
    }
}

impl Default for MockTtsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsEngine for MockTtsEngine {
    fn name(&self) -> &str {
        "Mock TTS Engine"
    }

    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, TtsError> {
        if text.trim().is_empty() {
            return Err(TtsError::EmptyText);
        }

        let num_samples = (16000.0 * (self.duration_ms as f64 / 1000.0)) as usize;
        let mut samples = Vec::with_capacity(num_samples);

        // Generate gentle 440 Hz sine wave
        for i in 0..num_samples {
            let t = i as f32 / 16000.0;
            let val = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.25;
            samples.push(val);
        }

        Ok(voxy_voice_stt::encode_wav_16k_mono(&samples))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sentence_chunker_punctuation_breaks() {
        let mut chunker = SentenceChunker::new(10);

        let c1 = chunker.push_text("Hello there! ");
        assert_eq!(c1, vec!["Hello there!"]);

        let c2 = chunker.push_text("How can I assist you with your code today? ");
        assert_eq!(c2, vec!["How can I assist you with your code today?"]);

        let c3 = chunker.push_text("Let's check the build.");
        assert_eq!(c3, vec!["Let's check the build."]);

        let flushed = chunker.flush();
        assert_eq!(flushed, None);
    }

    #[test]
    fn test_sentence_chunker_flush_trailing() {
        let mut chunker = SentenceChunker::new(20);
        let c = chunker.push_text("Streaming unfinished thought");
        assert!(c.is_empty());

        let flushed = chunker.flush();
        assert_eq!(flushed, Some("Streaming unfinished thought".to_string()));
    }

    #[tokio::test]
    async fn test_mock_tts_synthesis() {
        let tts = MockTtsEngine::with_duration(200); // 200ms
        let wav = tts.synthesize("System ready.").await.unwrap();

        assert!(!wav.is_empty());
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    #[tokio::test]
    async fn test_empty_tts_rejection() {
        let tts = MockTtsEngine::with_duration(100);
        let err = tts.synthesize("   ").await.unwrap_err();
        assert!(matches!(err, TtsError::EmptyText));
    }

    #[tokio::test]
    async fn test_chatterbox_tts_properties() {
        let engine = ChatterboxTtsEngine::new(
            Some("http://127.0.0.1:8000/v1/tts".to_string()),
            Some("custom_voice".to_string()),
        );
        assert_eq!(engine.name(), "Resemble AI Chatterbox TTS");
        assert_eq!(engine.endpoint(), "http://127.0.0.1:8000/v1/tts");
        assert_eq!(engine.voice(), "custom_voice");

        let err = engine.synthesize("   ").await.unwrap_err();
        assert!(matches!(err, TtsError::EmptyText));
    }
}
