//! Microsoft Azure Speech-to-Text provider adapter.

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, STTCapabilities, STTProvider, Transcript,
    VoiceLanguage,
};

// ~$1.00 per hour = $0.000278 per second
const COST_PER_AUDIO_SECOND: f64 = 0.000278;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AzureSTTResponse {
    #[allow(dead_code)]
    recognition_status: String,
    display_text: Option<String>,
}

pub struct AzureSTTProvider {
    api_key: String,
    region: String,
    client: reqwest::Client,
    capabilities: STTCapabilities,
}

impl AzureSTTProvider {
    pub fn new(api_key: impl Into<String>, region: Option<String>) -> Self {
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
            ],
            multilingual: true,
            custom_vocab: true,
            max_audio_duration_secs: 60,
            typical_latency_ms: 280,
            cost_per_second_usd: COST_PER_AUDIO_SECOND,
        };

        Self {
            api_key: api_key.into(),
            region: region.unwrap_or_else(|| "centralindia".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STT_AZURE_API_KEY")
            .or_else(|_| std::env::var("AZURE_SPEECH_KEY"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let region = std::env::var("STT_AZURE_REGION")
            .or_else(|_| std::env::var("AZURE_SPEECH_REGION"))
            .ok();
        Some(Self::new(key, region))
    }
}

#[async_trait]
impl STTProvider for AzureSTTProvider {
    fn id(&self) -> &'static str {
        "azure-speech-stt"
    }

    fn name(&self) -> &'static str {
        "Microsoft Azure Speech-to-Text"
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
            return Err(ProviderError::AuthError("Empty Azure Speech key".into()));
        }

        let lang_code = match language_hint {
            Some(VoiceLanguage::Hindi) => "hi-IN",
            Some(VoiceLanguage::Hinglish) => "hi-IN",
            Some(VoiceLanguage::English) => "en-IN",
            _ => "en-IN",
        };

        let url = format!(
            "https://{}.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1?language={}&format=detailed",
            self.region, lang_code
        );

        debug!(provider = self.id(), "Submitting audio to Azure Speech STT");

        let response = self
            .client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", "audio/wav; codecs=audio/pcm; samplerate=16000")
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
                reason: "Azure Speech 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "Azure Speech STT authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Azure Speech STT error");
            return Err(ProviderError::TranscriptionFailed(format!(
                "Azure Speech error ({}): {}",
                status, err_txt
            )));
        }

        let resp: AzureSTTResponse = response.json().await.map_err(|e| {
            ProviderError::TranscriptionFailed(format!("Failed to parse Azure response: {}", e))
        })?;

        let text = resp.display_text.unwrap_or_default();

        Ok(Transcript {
            text: text.trim().to_string(),
            confidence: Some(0.95),
            language: language_hint,
            latency_ms: 280,
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
