//! Google Cloud Speech-to-Text / Chirp provider adapter.

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, STTCapabilities, STTProvider, Transcript,
    VoiceLanguage,
};

const GOOGLE_STT_ENDPOINT: &str = "https://speech.googleapis.com/v1/speech:recognize";
// ~$0.016 per minute ($0.000267 per sec)
const COST_PER_AUDIO_SECOND: f64 = 0.000267;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleSTTConfig<'a> {
    encoding: &'static str,
    sample_rate_hertz: u32,
    language_code: &'a str,
    enable_automatic_punctuation: bool,
    model: &'a str,
}

#[derive(Serialize)]
struct GoogleSTTAudio {
    content: String,
}

#[derive(Serialize)]
struct GoogleSTTRequest<'a> {
    config: GoogleSTTConfig<'a>,
    audio: GoogleSTTAudio,
}

#[derive(Deserialize)]
struct GoogleSTTResponse {
    #[serde(default)]
    results: Vec<GoogleSTTResult>,
}

#[derive(Deserialize)]
struct GoogleSTTResult {
    #[serde(default)]
    alternatives: Vec<GoogleSTTAlternative>,
}

#[derive(Deserialize)]
struct GoogleSTTAlternative {
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    confidence: f32,
}

pub struct GoogleSTTProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
    capabilities: STTCapabilities,
}

impl GoogleSTTProvider {
    pub fn new(api_key: impl Into<String>, model: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        let capabilities = STTCapabilities {
            streaming: false,
            word_timestamps: true,
            supported_languages: vec![
                VoiceLanguage::English,
                VoiceLanguage::Hindi,
                VoiceLanguage::Hinglish,
            ],
            multilingual: true,
            custom_vocab: true,
            max_audio_duration_secs: 60,
            typical_latency_ms: 350,
            cost_per_second_usd: COST_PER_AUDIO_SECOND,
        };

        Self {
            api_key: api_key.into(),
            model: model.unwrap_or_else(|| "chirp".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STT_GOOGLE_API_KEY")
            .or_else(|_| std::env::var("GOOGLE_STT_API_KEY"))
            .or_else(|_| std::env::var("VOXY_API_KEYS_GOOGLE"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let model = std::env::var("STT_GOOGLE_MODEL").ok();
        Some(Self::new(key, model))
    }
}

#[async_trait]
impl STTProvider for GoogleSTTProvider {
    fn id(&self) -> &'static str {
        "google-cloud-speech"
    }

    fn name(&self) -> &'static str {
        "Google Cloud Speech / Chirp"
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
            return Err(ProviderError::AuthError("Empty Google STT API key".into()));
        }

        let lang_code = match language_hint {
            Some(VoiceLanguage::Hindi) => "hi-IN",
            Some(VoiceLanguage::Hinglish) => "hi-IN",
            Some(VoiceLanguage::English) => "en-IN",
            _ => "en-IN",
        };

        let b64_audio = BASE64.encode(&audio.pcm_bytes);
        let req = GoogleSTTRequest {
            config: GoogleSTTConfig {
                encoding: "LINEAR16",
                sample_rate_hertz: audio.sample_rate,
                language_code: lang_code,
                enable_automatic_punctuation: true,
                model: &self.model,
            },
            audio: GoogleSTTAudio { content: b64_audio },
        };

        let url = format!("{}?key={}", GOOGLE_STT_ENDPOINT, self.api_key);
        debug!(provider = self.id(), "Submitting audio to Google Cloud STT");

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&req)
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
                reason: "Google Cloud STT 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "Google Cloud STT authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Google STT error");
            return Err(ProviderError::TranscriptionFailed(format!(
                "Google STT error ({}): {}",
                status, err_txt
            )));
        }

        let resp: GoogleSTTResponse = response.json().await.map_err(|e| {
            ProviderError::TranscriptionFailed(format!(
                "Failed to parse Google STT response: {}",
                e
            ))
        })?;

        let alt = resp
            .results
            .into_iter()
            .next()
            .and_then(|r| r.alternatives.into_iter().next())
            .unwrap_or(GoogleSTTAlternative {
                transcript: String::new(),
                confidence: 0.0,
            });

        Ok(Transcript {
            text: alt.transcript.trim().to_string(),
            confidence: Some(alt.confidence),
            language: language_hint,
            latency_ms: 350,
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
