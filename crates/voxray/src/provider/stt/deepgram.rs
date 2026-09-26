//! Deepgram Nova-2 Speech-to-Text provider adapter.

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, STTCapabilities, STTProvider, Transcript,
    VoiceLanguage,
};

const DEEPGRAM_STT_ENDPOINT: &str = "https://api.deepgram.com/v1/listen";
// ~$0.0043 per minute ($0.000072 per sec)
const COST_PER_AUDIO_SECOND: f64 = 0.000072;

#[derive(Debug, Deserialize)]
struct DeepgramResponse {
    results: DeepgramResults,
}

#[derive(Debug, Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(Debug, Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Debug, Deserialize)]
struct DeepgramAlternative {
    transcript: String,
    confidence: f32,
}

pub struct DeepgramSTTProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    capabilities: STTCapabilities,
}

impl DeepgramSTTProvider {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        let capabilities = STTCapabilities {
            streaming: true,
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
            typical_latency_ms: 180,
            cost_per_second_usd: COST_PER_AUDIO_SECOND,
        };

        Self {
            api_key: api_key.into(),
            model: model.unwrap_or_else(|| "nova-2".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STT_DEEPGRAM_API_KEY")
            .or_else(|_| std::env::var("DEEPGRAM_API_KEY"))
            .or_else(|_| std::env::var("VOXY_API_KEYS_DEEPGRAM"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let model = std::env::var("STT_DEEPGRAM_MODEL").ok();
        Some(Self::new(key, model))
    }
}

#[async_trait]
impl STTProvider for DeepgramSTTProvider {
    fn id(&self) -> &'static str {
        "deepgram-nova-2"
    }

    fn name(&self) -> &'static str {
        "Deepgram Nova-2"
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
            return Err(ProviderError::AuthError("Empty Deepgram API key".into()));
        }

        let mut url = format!(
            "{}?model={}&smart_format=true&punctuate=true",
            DEEPGRAM_STT_ENDPOINT, self.model
        );

        if let Some(ref lang) = language_hint {
            let code = match lang {
                VoiceLanguage::English => "en",
                VoiceLanguage::Hindi => "hi",
                VoiceLanguage::Hinglish => "hi-Latn",
                VoiceLanguage::Other(ref c) => c.as_str(),
                VoiceLanguage::Custom(ref c) => c.as_str(),
                _ => "en",
            };
            url.push_str(&format!("&language={}", code));
        }

        debug!(provider = self.id(), "Submitting audio to Deepgram STT");

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Token {}", self.api_key))
            .header("Content-Type", "audio/wav")
            .body(audio.to_wav())
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
                reason: "Deepgram STT 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "Deepgram STT authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Deepgram STT error");
            return Err(ProviderError::TranscriptionFailed(format!(
                "Deepgram STT error ({}): {}",
                status, err_txt
            )));
        }

        let resp: DeepgramResponse = response.json().await.map_err(|e| {
            ProviderError::TranscriptionFailed(format!("Failed to parse Deepgram response: {}", e))
        })?;

        let alt = resp
            .results
            .channels
            .into_iter()
            .next()
            .and_then(|c| c.alternatives.into_iter().next())
            .unwrap_or(DeepgramAlternative {
                transcript: String::new(),
                confidence: 0.0,
            });

        Ok(Transcript {
            text: alt.transcript.trim().to_string(),
            confidence: Some(alt.confidence),
            language: language_hint,
            latency_ms: 180,
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
