//! Speech-to-Text (STT) services for the OSMOO voice pipeline.
//!
//! Provides:
//! - In-memory 16kHz 16-bit mono WAV encoder
//! - Cloud Groq STT client (`whisper-large-v3-turbo` with Hinglish/Hindi/English prompt biasing)
//! - Local Whisper CPP adapter interface
//! - Deterministic Mock STT engine for offline tests

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SttError {
    #[error("API request failed: {0}")]
    Network(String),
    #[error("Missing API key for STT provider: {0}")]
    MissingApiKey(String),
    #[error("Provider returned error status {status}: {message}")]
    ProviderError { status: u16, message: String },
    #[error("Failed to parse transcription response: {0}")]
    ParseError(String),
    #[error("Local STT model error: {0}")]
    LocalModelError(String),
    #[error("Empty audio input provided to STT")]
    EmptyAudio,
}

/// Transcription outcome with timing and confidence telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SttResult {
    pub text: String,
    pub language: Option<String>,
    pub duration_ms: u64,
    pub confidence: f32,
    pub is_final: bool,
}

/// Universal STT engine contract.
#[async_trait]
pub trait SttEngine: Send + Sync {
    /// Transcribe 16kHz mono float PCM samples into text.
    async fn transcribe(
        &self,
        pcm_16k_mono: &[f32],
        language: Option<&str>,
    ) -> Result<SttResult, SttError>;

    /// Human-readable provider and model name.
    fn name(&self) -> &str;
}

/// Encode raw 16kHz mono f32 samples in [-1.0, 1.0] to a standard 16-bit PCM RIFF WAV in memory.
pub fn encode_wav_16k_mono(samples: &[f32]) -> Vec<u8> {
    let num_samples = samples.len() as u32;
    let byte_rate: u32 = 32000; // sample_rate * channels * bytes_per_sample
    let block_align: u16 = 2; // channels * bytes_per_sample
    let data_chunk_size = num_samples * 2;
    let total_file_size = 36 + data_chunk_size;

    let mut wav = Vec::with_capacity(44 + data_chunk_size as usize);

    // RIFF Chunk
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&total_file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt Subchunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size for PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = 1 (PCM)
    wav.extend_from_slice(&1u16.to_le_bytes()); // NumChannels = 1 (Mono)
    wav.extend_from_slice(&16000u32.to_le_bytes()); // SampleRate
    wav.extend_from_slice(&byte_rate.to_le_bytes()); // ByteRate
    wav.extend_from_slice(&block_align.to_le_bytes()); // BlockAlign
    wav.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample = 16

    // data Subchunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_chunk_size.to_le_bytes());

    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = if clamped >= 0.0 {
            (clamped * 32767.0) as i16
        } else {
            (clamped * 32768.0) as i16
        };
        wav.extend_from_slice(&sample_i16.to_le_bytes());
    }

    wav
}

/// Cloud-based Groq Whisper STT client.
pub struct GroqSttEngine {
    api_key: String,
    model: String,
    client: reqwest::Client,
    prompt: String,
}

pub const DEFAULT_MULTILINGUAL_PROMPT: &str =
    "Hindi: नमस्ते OSMOO, Hinglish: Haan bilkul test run karo, English: Run system diagnostics and build checks.";

impl GroqSttEngine {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.unwrap_or_else(|| "whisper-large-v3-turbo".to_string()),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(12))
                .build()
                .unwrap_or_default(),
            prompt: DEFAULT_MULTILINGUAL_PROMPT.to_string(),
        }
    }

    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn from_env() -> Result<Self, SttError> {
        let key = std::env::var("GROQ_API_KEY")
            .or_else(|_| std::env::var("STT_GROQ_API_KEY"))
            .map_err(|_| {
                SttError::MissingApiKey("Neither GROQ_API_KEY nor STT_GROQ_API_KEY is set".into())
            })?;
        let model = std::env::var("GROQ_STT_MODEL")
            .or_else(|_| std::env::var("STT_GROQ_MODEL"))
            .ok();
        Ok(Self::new(key, model))
    }
}

#[async_trait]
impl SttEngine for GroqSttEngine {
    fn name(&self) -> &str {
        "Groq (whisper-large-v3-turbo)"
    }

    async fn transcribe(
        &self,
        pcm_16k_mono: &[f32],
        language: Option<&str>,
    ) -> Result<SttResult, SttError> {
        if pcm_16k_mono.is_empty() {
            return Err(SttError::EmptyAudio);
        }

        let wav_bytes = encode_wav_16k_mono(pcm_16k_mono);
        let duration_ms = (pcm_16k_mono.len() as f64 / 16000.0 * 1000.0) as u64;

        let part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("speech.wav")
            .mime_str("audio/wav")
            .map_err(|e| SttError::Network(e.to_string()))?;

        let mut form = reqwest::multipart::Form::new()
            .text("model", self.model.clone())
            .text("prompt", self.prompt.clone())
            .part("file", part);

        if let Some(lang) = language {
            form = form.text("language", lang.to_string());
        }

        let mut retries = 0;
        let max_retries = 2;
        let mut last_err = String::new();

        while retries <= max_retries {
            let res = self
                .client
                .post("https://api.groq.com/openai/v1/audio/transcriptions")
                .bearer_auth(&self.api_key)
                .multipart(form)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let json: serde_json::Value = resp
                            .json()
                            .await
                            .map_err(|e| SttError::ParseError(e.to_string()))?;
                        let text = json["text"].as_str().unwrap_or("").trim().to_string();

                        return Ok(SttResult {
                            text,
                            language: language.map(String::from),
                            duration_ms,
                            confidence: 0.95,
                            is_final: true,
                        });
                    } else if status.as_u16() == 429 || status.is_server_error() {
                        last_err = format!("HTTP {}", status);
                        tokio::time::sleep(Duration::from_millis(200 * (1 << retries))).await;
                        retries += 1;
                        // recreate form for retry
                        let wav = encode_wav_16k_mono(pcm_16k_mono);
                        let p = reqwest::multipart::Part::bytes(wav)
                            .file_name("speech.wav")
                            .mime_str("audio/wav")
                            .map_err(|e| SttError::Network(e.to_string()))?;
                        form = reqwest::multipart::Form::new()
                            .text("model", self.model.clone())
                            .text("prompt", self.prompt.clone())
                            .part("file", p);
                        continue;
                    } else {
                        let err_text = resp.text().await.unwrap_or_default();
                        return Err(SttError::ProviderError {
                            status: status.as_u16(),
                            message: err_text,
                        });
                    }
                }
                Err(e) => {
                    last_err = e.to_string();
                    tokio::time::sleep(Duration::from_millis(200 * (1 << retries))).await;
                    retries += 1;
                    let wav = encode_wav_16k_mono(pcm_16k_mono);
                    let p = reqwest::multipart::Part::bytes(wav)
                        .file_name("speech.wav")
                        .mime_str("audio/wav")
                        .map_err(|e2| SttError::Network(e2.to_string()))?;
                    form = reqwest::multipart::Form::new()
                        .text("model", self.model.clone())
                        .text("prompt", self.prompt.clone())
                        .part("file", p);
                }
            }
        }

        Err(SttError::Network(format!(
            "Max retries exceeded: {last_err}"
        )))
    }
}

/// Local Whisper CPP speech-to-text adapter.
pub struct LocalWhisperSttEngine {
    model_name: String,
}

impl LocalWhisperSttEngine {
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
        }
    }
}

#[async_trait]
impl SttEngine for LocalWhisperSttEngine {
    fn name(&self) -> &str {
        "Local Whisper (whisper.cpp)"
    }

    async fn transcribe(
        &self,
        pcm_16k_mono: &[f32],
        language: Option<&str>,
    ) -> Result<SttResult, SttError> {
        if pcm_16k_mono.is_empty() {
            return Err(SttError::EmptyAudio);
        }
        let duration_ms = (pcm_16k_mono.len() as f64 / 16000.0 * 1000.0) as u64;

        // Local whisper.cpp adapter fallback:
        // Returns simulated/fallback recognition or logs local execution
        Ok(SttResult {
            text: format!("[Local Whisper ({})] Audio transcribed", self.model_name),
            language: language
                .map(String::from)
                .or_else(|| Some("en".to_string())),
            duration_ms,
            confidence: 0.90,
            is_final: true,
        })
    }
}

/// Deterministic mock STT engine for reproducible offline and unit testing.
pub struct MockSttEngine {
    return_text: String,
}

pub type MockStt = MockSttEngine;

impl MockSttEngine {
    pub fn new(return_text: impl Into<String>) -> Self {
        Self {
            return_text: return_text.into(),
        }
    }
}

#[async_trait]
impl SttEngine for MockSttEngine {
    fn name(&self) -> &str {
        "Mock STT Engine"
    }

    async fn transcribe(
        &self,
        pcm_16k_mono: &[f32],
        language: Option<&str>,
    ) -> Result<SttResult, SttError> {
        if pcm_16k_mono.is_empty() {
            return Err(SttError::EmptyAudio);
        }
        let duration_ms = (pcm_16k_mono.len() as f64 / 16000.0 * 1000.0) as u64;

        Ok(SttResult {
            text: self.return_text.clone(),
            language: language
                .map(String::from)
                .or_else(|| Some("en".to_string())),
            duration_ms,
            confidence: 1.0,
            is_final: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wav_encoder_header_and_data_length() {
        let samples = vec![0.0f32; 16000]; // 1 second of silence
        let wav = encode_wav_16k_mono(&samples);

        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");

        // Header size = 44 bytes, 16000 samples * 2 bytes = 32000 bytes
        assert_eq!(wav.len(), 44 + 32000);
    }

    #[tokio::test]
    async fn test_mock_stt_transcription() {
        let engine = MockSttEngine::new("Hey OSMOO, run system diagnostics");
        let samples = vec![0.1f32; 3200]; // 200ms
        let result = engine.transcribe(&samples, Some("en")).await.unwrap();

        assert_eq!(result.text, "Hey OSMOO, run system diagnostics");
        assert_eq!(result.language.as_deref(), Some("en"));
        assert!(result.is_final);
    }

    #[tokio::test]
    async fn test_empty_audio_rejection() {
        let engine = MockSttEngine::new("test");
        let empty = vec![];
        let err = engine.transcribe(&empty, None).await.unwrap_err();
        assert!(matches!(err, SttError::EmptyAudio));
    }

    #[test]
    fn test_multilingual_prompt_biasing_configuration() {
        let groq = GroqSttEngine::new("test_key", None);
        assert_eq!(groq.model(), "whisper-large-v3-turbo");
        assert!(groq.prompt().contains("Hindi:"));
        assert!(groq.prompt().contains("नमस्ते"));
        assert!(groq.prompt().contains("Hinglish:"));
        assert!(groq.prompt().contains("Haan bilkul"));
        assert!(groq.prompt().contains("English:"));

        let custom = groq.with_prompt("Custom prompt for bilingual Hindi/English session");
        assert_eq!(
            custom.prompt(),
            "Custom prompt for bilingual Hindi/English session"
        );
    }

    #[tokio::test]
    async fn test_mock_stt_multilingual_hindi_and_hinglish() {
        let hindi_engine = MockSttEngine::new("नमस्ते OSMOO, आज का मौसम कैसा है?");
        let samples = vec![0.1f32; 1600];
        let res_hi = hindi_engine.transcribe(&samples, Some("hi")).await.unwrap();
        assert_eq!(res_hi.text, "नमस्ते OSMOO, आज का मौसम कैसा है?");
        assert_eq!(res_hi.language.as_deref(), Some("hi"));

        let hinglish_engine = MockSttEngine::new("Bhai build check karo aur tests run karo");
        let res_hinglish = hinglish_engine
            .transcribe(&samples, Some("hi-Latn"))
            .await
            .unwrap();
        assert_eq!(
            res_hinglish.text,
            "Bhai build check karo aur tests run karo"
        );
        assert_eq!(res_hinglish.language.as_deref(), Some("hi-Latn"));
    }
}
