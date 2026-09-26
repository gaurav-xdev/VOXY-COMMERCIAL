//! Groq Whisper Large V3 Turbo STT provider adapter.

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, STTCapabilities, STTProvider, Transcript,
    VoiceLanguage,
};

const GROQ_STT_ENDPOINT: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
// ~$0.04 per hour = $0.000011 per second
const COST_PER_AUDIO_SECOND: f64 = 0.000011;

#[derive(Debug, Deserialize)]
struct GroqTranscriptionResponse {
    text: String,
}

pub struct GroqSTTProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    capabilities: STTCapabilities,
}

impl GroqSTTProvider {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();

        let capabilities = STTCapabilities {
            streaming: false,
            word_timestamps: true,
            supported_languages: vec![
                VoiceLanguage::English,
                VoiceLanguage::Hindi,
                VoiceLanguage::Hinglish,
                VoiceLanguage::Other("multilingual".into()),
            ],
            multilingual: true,
            custom_vocab: true,
            max_audio_duration_secs: 1800,
            typical_latency_ms: 220,
            cost_per_second_usd: COST_PER_AUDIO_SECOND,
        };

        Self {
            api_key: api_key.into(),
            model: model.unwrap_or_else(|| "whisper-large-v3-turbo".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STT_GROQ_API_KEY")
            .or_else(|_| std::env::var("VOXY_API_KEYS_GROQ"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let model = std::env::var("STT_GROQ_MODEL").ok();
        Some(Self::new(key, model))
    }
}

#[async_trait]
impl STTProvider for GroqSTTProvider {
    fn id(&self) -> &'static str {
        "groq-whisper-turbo"
    }

    fn name(&self) -> &'static str {
        "Groq Whisper Large V3 Turbo"
    }

    fn capabilities(&self) -> &STTCapabilities {
        &self.capabilities
    }

    async fn transcribe(
        &self,
        audio: AudioData,
        language_hint: Option<VoiceLanguage>,
    ) -> Result<Transcript, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::AuthError("Empty Groq API key".into()));
        }

        let wav_bytes = audio.to_wav();
        let mut form = Form::new()
            .part(
                "file",
                Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| ProviderError::TranscriptionFailed(e.to_string()))?,
            )
            .text("model", self.model.clone())
            .text("response_format", "json");

        if let Some(ref lang) = language_hint {
            let code = match lang {
                VoiceLanguage::English => Some("en"),
                VoiceLanguage::Hindi => Some("hi"),
                VoiceLanguage::Hinglish => Some("hi"),
                VoiceLanguage::Other(ref c) => Some(c.as_str()),
                VoiceLanguage::Custom(ref c) => Some(c.as_str()),
                VoiceLanguage::Auto | VoiceLanguage::AutoDetect => None,
            };
            if let Some(c) = code {
                form = form.text("language", c.to_string());
            }
        }

        debug!(provider = self.id(), "Submitting audio to Groq Whisper STT");

        let response = self
            .client
            .post(GROQ_STT_ENDPOINT)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
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
                reason: "Groq Whisper STT 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "Groq STT authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Groq STT error");
            return Err(ProviderError::TranscriptionFailed(format!(
                "Groq STT error ({}): {}",
                status, err_txt
            )));
        }

        let resp: GroqTranscriptionResponse = response.json().await.map_err(|e| {
            ProviderError::TranscriptionFailed(format!("Failed to parse Groq response: {}", e))
        })?;

        Ok(Transcript {
            text: resp.text.trim().to_string(),
            confidence: Some(0.95),
            language: language_hint,
            latency_ms: 220,
            is_final: true,
            provider: self.id().to_string(),
            duration_secs: audio.duration_secs(),
            words: None,
        })
    }

    fn estimate_cost(&self, audio_duration_secs: f64) -> EstimatedCost {
        let cost = audio_duration_secs * COST_PER_AUDIO_SECOND;
        EstimatedCost {
            amount_usd: cost,
            currency: "USD".into(),
            estimated_cost_usd: cost,
            billable_units: audio_duration_secs,
            unit_type: "seconds".into(),
        }
    }
}
