//! Deepgram Aura Text-to-Speech provider adapter.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::HeaderMap;
use serde_json::json;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, TTSCapabilities, TTSProvider, TTSStream, VoiceLanguage,
};

const DEEPGRAM_TTS_ENDPOINT: &str = "https://api.deepgram.com/v1/speak";
// $0.015 per 1,000 characters
const COST_PER_CHARACTER: f64 = 0.000015;

pub struct DeepgramTTSProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    capabilities: TTSCapabilities,
}

impl DeepgramTTSProvider {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        let capabilities = TTSCapabilities {
            streaming: true,
            supported_languages: vec![VoiceLanguage::English],
            emotional_styles: false,
            speed_control: false,
            pitch_control: false,
            typical_first_byte_ms: 95,
            cost_per_char_usd: COST_PER_CHARACTER,
        };

        Self {
            api_key: api_key.into(),
            model: model.unwrap_or_else(|| "aura-asteria-en".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("TTS_DEEPGRAM_API_KEY")
            .or_else(|_| std::env::var("DEEPGRAM_API_KEY"))
            .or_else(|_| std::env::var("VOXY_API_KEYS_DEEPGRAM"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let model = std::env::var("TTS_DEEPGRAM_MODEL").ok();
        Some(Self::new(key, model))
    }
}

#[async_trait]
impl TTSProvider for DeepgramTTSProvider {
    fn id(&self) -> &'static str {
        "deepgram-aura-tts"
    }

    fn name(&self) -> &'static str {
        "Deepgram Aura TTS"
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
            return Err(ProviderError::AuthError("Empty Deepgram API key".into()));
        }

        let url = format!(
            "{}?model={}&encoding=linear16&sample_rate=16000&container=none",
            DEEPGRAM_TTS_ENDPOINT, self.model
        );

        let body = json!({ "text": text });

        debug!(
            provider = self.id(),
            model = self.model,
            "Calling Deepgram Aura TTS"
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Token {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(Duration::from_secs(20))
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
                reason: "Deepgram Aura 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "Deepgram TTS authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Deepgram Aura TTS failure");
            return Err(ProviderError::SynthesisFailed(format!(
                "Deepgram Aura error ({}): {}",
                status, err_txt
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            ProviderError::SynthesisFailed(format!("Failed to read audio bytes: {}", e))
        })?;

        Ok(AudioData::new(bytes.to_vec(), 16000, 1))
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        _language: Option<VoiceLanguage>,
    ) -> Result<TTSStream, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::AuthError("Empty Deepgram API key".into()));
        }

        let url = format!(
            "{}?model={}&encoding=linear16&sample_rate=16000&container=none",
            DEEPGRAM_TTS_ENDPOINT, self.model
        );

        let body = json!({ "text": text });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Token {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_txt = response.text().await.unwrap_or_default();
            return Err(ProviderError::SynthesisFailed(format!(
                "Deepgram Aura stream error ({}): {}",
                status, err_txt
            )));
        }

        let byte_stream = response.bytes_stream();
        let mapped = byte_stream.map(|chunk_res| {
            chunk_res
                .map(|chunk| AudioData::new(chunk.to_vec(), 16000, 1))
                .map_err(|e| ProviderError::SynthesisFailed(e.to_string()))
        });

        Ok(Box::pin(mapped))
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
