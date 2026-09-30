//! ElevenLabs Scribe Speech-to-Text provider adapter.

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

const ELEVENLABS_STT_ENDPOINT: &str = "https://api.elevenlabs.io/v1/speech-to-text";
// ~$0.0048 per minute ($0.00008 per sec) for Scribe
const COST_PER_AUDIO_SECOND: f64 = 0.00008;

#[derive(Debug, Deserialize)]
struct ElevenLabsTranscriptionResponse {
    text: String,
    #[serde(default)]
    language_code: Option<String>,
}

pub struct ElevenLabsSTTProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    capabilities: STTCapabilities,
}

impl ElevenLabsSTTProvider {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        let key = api_key.into();
        let model = model.unwrap_or_else(|| "scribe_v1".to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
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
            custom_vocab: false,
            max_audio_duration_secs: 1800,
            typical_latency_ms: 550,
            cost_per_second_usd: COST_PER_AUDIO_SECOND,
        };

        Self {
            api_key: key,
            model,
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STT_ELEVENLABS_API_KEY")
            .or_else(|_| std::env::var("VOXY_API_KEYS_ELEVENLABS"))
            .or_else(|_| std::env::var("ELEVENLABS_API_KEY"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let model = std::env::var("STT_ELEVENLABS_MODEL").ok();
        Some(Self::new(key, model))
    }
}

#[async_trait]
impl STTProvider for ElevenLabsSTTProvider {
    fn id(&self) -> &'static str {
        "elevenlabs-scribe"
    }

    fn name(&self) -> &'static str {
        "ElevenLabs Scribe Speech-to-Text"
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
            return Err(ProviderError::AuthError("Empty ElevenLabs API key".into()));
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
            .text("model_id", self.model.clone());

        if let Some(lang) = language_hint {
            let code = match lang {
                VoiceLanguage::English => Some("eng"),
                VoiceLanguage::Hindi => Some("hin"),
                VoiceLanguage::Hinglish => Some("hin"),
                VoiceLanguage::Other(ref c) => Some(c.as_str()),
                VoiceLanguage::Custom(ref c) => Some(c.as_str()),
                _ => None,
            };
            if let Some(c) = code {
                form = form.text("language_code", c.to_string());
            }
        }

        debug!(
            provider = self.id(),
            "Submitting audio to ElevenLabs Scribe STT"
        );

        let response = self
            .client
            .post(ELEVENLABS_STT_ENDPOINT)
            .header("xi-api-key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::Timeout(Duration::from_secs(25))
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
                reason: "ElevenLabs STT 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "ElevenLabs STT authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %error_text, "ElevenLabs STT error response");
            return Err(ProviderError::TranscriptionFailed(format!(
                "ElevenLabs API error ({}): {}",
                status, error_text
            )));
        }

        let resp: ElevenLabsTranscriptionResponse = response.json().await.map_err(|e| {
            ProviderError::TranscriptionFailed(format!(
                "Failed to parse ElevenLabs response: {}",
                e
            ))
        })?;

        let lang = resp.language_code.map(|l| match l.to_lowercase().as_str() {
            "en" | "eng" => VoiceLanguage::English,
            "hi" | "hin" => VoiceLanguage::Hindi,
            other => VoiceLanguage::Other(other.to_string()),
        });

        Ok(Transcript {
            text: resp.text.trim().to_string(),
            is_final: true,
            confidence: Some(0.95),
            language: lang,
            latency_ms: 550,
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
