//! ElevenLabs Text-to-Speech provider adapter.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::HeaderMap;
use serde_json::json;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, TTSCapabilities, TTSProvider, TTSStream, VoiceLanguage,
};

const ELEVENLABS_TTS_BASE_URL: &str = "https://api.elevenlabs.io/v1/text-to-speech";
const DEFAULT_VOICE_ID: &str = "21m00Tcm4TlvDq8ikWAM";
// ~$0.15 per 1,000 characters (turbo)
const COST_PER_CHARACTER: f64 = 0.00015;

pub struct ElevenLabsTTSProvider {
    api_key: String,
    voice_id: String,
    model_id: String,
    client: reqwest::Client,
    capabilities: TTSCapabilities,
}

impl ElevenLabsTTSProvider {
    pub fn new(
        api_key: impl Into<String>,
        voice_id: Option<String>,
        model_id: Option<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let capabilities = TTSCapabilities {
            streaming: true,
            supported_languages: vec![
                VoiceLanguage::English,
                VoiceLanguage::Hindi,
                VoiceLanguage::Hinglish,
                VoiceLanguage::Other("multilingual".into()),
            ],
            emotional_styles: true,
            speed_control: true,
            pitch_control: false,
            typical_first_byte_ms: 180,
            cost_per_char_usd: COST_PER_CHARACTER,
        };

        Self {
            api_key: api_key.into(),
            voice_id: voice_id.unwrap_or_else(|| DEFAULT_VOICE_ID.to_string()),
            model_id: model_id.unwrap_or_else(|| "eleven_turbo_v2_5".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("TTS_ELEVENLABS_API_KEY")
            .or_else(|_| std::env::var("VOXY_API_KEYS_ELEVENLABS"))
            .or_else(|_| std::env::var("ELEVENLABS_API_KEY"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let voice = std::env::var("TTS_ELEVENLABS_VOICE_ID").ok();
        let model = std::env::var("TTS_ELEVENLABS_MODEL_ID").ok();
        Some(Self::new(key, voice, model))
    }

    fn raw_pcm16_to_audiodata(bytes: &[u8], sample_rate: u32) -> AudioData {
        let samples: Vec<f32> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();
        AudioData::from_pcm(samples, sample_rate, 1)
    }
}

#[async_trait]
impl TTSProvider for ElevenLabsTTSProvider {
    fn id(&self) -> &'static str {
        "elevenlabs-tts"
    }

    fn name(&self) -> &'static str {
        "ElevenLabs Neural TTS"
    }

    fn capabilities(&self) -> &TTSCapabilities {
        &self.capabilities
    }

    async fn synthesize(
        &self,
        text: &str,
        _language: Option<VoiceLanguage>,
    ) -> Result<AudioData, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::AuthError("Empty ElevenLabs API key".into()));
        }

        let url = format!(
            "{}/{}?output_format=pcm_16000",
            ELEVENLABS_TTS_BASE_URL, self.voice_id
        );

        let body = json!({
            "text": text,
            "model_id": self.model_id,
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.75,
                "use_speaker_boost": true
            }
        });

        debug!(
            provider = self.id(),
            text_len = text.len(),
            "Calling ElevenLabs TTS"
        );

        let response = self
            .client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(Duration::from_secs(30))
                } else {
                    ProviderError::NetworkError(e.to_string())
                }
            })?;

        let status = response.status();
        let headers: HeaderMap = response.headers().clone();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = headers
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(Duration::from_secs);
            return Err(ProviderError::RateLimited {
                retry_after,
                reason: "ElevenLabs TTS 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "ElevenLabs TTS authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "ElevenLabs TTS failure");
            return Err(ProviderError::SynthesisFailed(format!(
                "ElevenLabs error ({}): {}",
                status, err_txt
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            ProviderError::SynthesisFailed(format!("Failed to read audio bytes: {}", e))
        })?;

        Ok(Self::raw_pcm16_to_audiodata(&bytes, 16000))
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        _language: Option<VoiceLanguage>,
    ) -> Result<TTSStream, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::AuthError("Empty ElevenLabs API key".into()));
        }

        let url = format!(
            "{}/{}/stream?output_format=pcm_16000",
            ELEVENLABS_TTS_BASE_URL, self.voice_id
        );

        let body = json!({
            "text": text,
            "model_id": self.model_id,
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.75
            }
        });

        let response = self
            .client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_txt = response.text().await.unwrap_or_default();
            return Err(ProviderError::SynthesisFailed(format!(
                "ElevenLabs stream error ({}): {}",
                status, err_txt
            )));
        }

        let byte_stream = response.bytes_stream();
        let mapped_stream = byte_stream.map(|chunk_res| {
            chunk_res
                .map(|chunk| Self::raw_pcm16_to_audiodata(&chunk, 16000))
                .map_err(|e| ProviderError::SynthesisFailed(e.to_string()))
        });

        Ok(Box::pin(mapped_stream))
    }

    fn estimate_cost(&self, character_count: usize) -> EstimatedCost {
        let chars = character_count as f64;
        let cost = chars * COST_PER_CHARACTER;
        EstimatedCost {
            amount_usd: cost,
            currency: "USD".into(),
            estimated_cost_usd: cost,
            billable_units: chars,
            unit_type: "characters".into(),
        }
    }
}
