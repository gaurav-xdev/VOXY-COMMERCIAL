//! Chatterbox Turbo Hinglish TTS provider adapter.

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde_json::json;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, TTSCapabilities, TTSProvider,
    VoiceLanguage,
};

const DEFAULT_CHATTERBOX_ENDPOINT: &str = "http://127.0.0.1:8001/v1/audio/speech";

pub struct ChatterboxTTSProvider {
    endpoint: String,
    api_key: Option<String>,
    client: reqwest::Client,
    capabilities: TTSCapabilities,
}

impl ChatterboxTTSProvider {
    pub fn new(endpoint: Option<String>, api_key: Option<String>) -> Self {
        let ep = endpoint.unwrap_or_else(|| DEFAULT_CHATTERBOX_ENDPOINT.to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        let capabilities = TTSCapabilities {
            streaming: false,
            supported_languages: vec![
                VoiceLanguage::Hindi,
                VoiceLanguage::Hinglish,
                VoiceLanguage::English,
            ],
            emotional_styles: true,
            speed_control: true,
            pitch_control: true,
            typical_first_byte_ms: 150,
            cost_per_char_usd: 0.000005,
        };

        Self {
            endpoint: ep,
            api_key,
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let ep = std::env::var("TTS_CHATTERBOX_ENDPOINT")
            .or_else(|_| std::env::var("CHATTERBOX_ENDPOINT"))
            .ok();
        let key = std::env::var("TTS_CHATTERBOX_API_KEY")
            .or_else(|_| std::env::var("CHATTERBOX_API_KEY"))
            .ok();

        if ep.is_none() && key.is_none() {
            return None;
        }

        Some(Self::new(ep, key))
    }

    fn decode_wav_or_raw_pcm(bytes: &[u8], sample_rate: u32) -> AudioData {
        let pcm_bytes = if bytes.len() >= 44 && &bytes[0..4] == b"RIFF" {
            bytes[44..].to_vec()
        } else {
            bytes.to_vec()
        };

        AudioData::new(pcm_bytes, sample_rate, 1)
    }
}

#[async_trait]
impl TTSProvider for ChatterboxTTSProvider {
    fn id(&self) -> &'static str {
        "chatterbox-turbo"
    }

    fn name(&self) -> &'static str {
        "Chatterbox Turbo Hinglish TTS"
    }

    fn capabilities(&self) -> &TTSCapabilities {
        &self.capabilities
    }

    async fn synthesize(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<AudioData, ProviderError> {
        let lang_str = match language {
            Some(VoiceLanguage::Hindi) => "hi",
            Some(VoiceLanguage::Hinglish) => "hinglish",
            Some(VoiceLanguage::English) => "en",
            _ => "hinglish",
        };

        let body = json!({
            "text": text,
            "language": lang_str,
            "response_format": "wav"
        });

        debug!(provider = self.id(), endpoint = %self.endpoint, "Calling Chatterbox Turbo");

        let mut req = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&body);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().await.map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout(Duration::from_secs(15))
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
                reason: "Chatterbox rate limit exceeded".into(),
            });
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Chatterbox TTS failure");
            return Err(ProviderError::SynthesisFailed(format!(
                "Chatterbox error ({}): {}",
                status, err_txt
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::SynthesisFailed(format!("Failed to read audio bytes: {}", e)))?;

        Ok(Self::decode_wav_or_raw_pcm(&bytes, 24000))
    }

    fn estimate_cost(&self, character_count: usize) -> EstimatedCost {
        let chars = character_count as f64;
        let cost = chars * self.capabilities.cost_per_char_usd;
        EstimatedCost {
            amount_usd: cost,
            currency: "USD".into(),
            estimated_cost_usd: cost,
            billable_units: chars,
            unit_type: "characters".into(),
        }
    }
}
