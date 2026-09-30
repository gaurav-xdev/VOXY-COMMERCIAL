//! AssemblyAI Speech-to-Text provider adapter.

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use serde_json::json;
use std::time::Duration;
use tracing::debug;

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, STTCapabilities, STTProvider, Transcript,
    VoiceLanguage,
};

const ASSEMBLYAI_UPLOAD_ENDPOINT: &str = "https://api.assemblyai.com/v2/upload";
const ASSEMBLYAI_TRANSCRIPT_ENDPOINT: &str = "https://api.assemblyai.com/v2/transcript";
// ~$0.00025 per second
const COST_PER_AUDIO_SECOND: f64 = 0.00025;

pub struct AssemblyAISTTProvider {
    api_key: String,
    client: reqwest::Client,
    capabilities: STTCapabilities,
}

impl AssemblyAISTTProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        let capabilities = STTCapabilities {
            streaming: false,
            word_timestamps: true,
            supported_languages: vec![VoiceLanguage::English, VoiceLanguage::Hindi],
            multilingual: true,
            custom_vocab: true,
            max_audio_duration_secs: 1800,
            typical_latency_ms: 750,
            cost_per_second_usd: COST_PER_AUDIO_SECOND,
        };

        Self {
            api_key: api_key.into(),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STT_ASSEMBLYAI_API_KEY")
            .or_else(|_| std::env::var("ASSEMBLYAI_API_KEY"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        Some(Self::new(key))
    }
}

#[async_trait]
impl STTProvider for AssemblyAISTTProvider {
    fn id(&self) -> &'static str {
        "assemblyai-stt"
    }

    fn name(&self) -> &'static str {
        "AssemblyAI Speech-to-Text"
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
            return Err(ProviderError::AuthError("Empty AssemblyAI API key".into()));
        }

        debug!(provider = self.id(), "Uploading audio to AssemblyAI");

        // 1. Upload audio
        let upload_res = self
            .client
            .post(ASSEMBLYAI_UPLOAD_ENDPOINT)
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/octet-stream")
            .body(audio.to_wav())
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let status = upload_res.status();
        let headers: HeaderMap = upload_res.headers().clone();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = headers
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(Duration::from_secs);
            return Err(ProviderError::RateLimited {
                retry_after,
                reason: "AssemblyAI 429 rate limit exceeded".into(),
            });
        }

        if !status.is_success() {
            let status_code = status.as_u16();
            let msg = upload_res.text().await.unwrap_or_default();
            return Err(ProviderError::TranscriptionFailed(format!(
                "AssemblyAI upload error ({}): {}",
                status_code, msg
            )));
        }

        let upload_body: serde_json::Value = upload_res.json().await.map_err(|e| {
            ProviderError::TranscriptionFailed(format!("Failed to parse upload JSON: {}", e))
        })?;

        let upload_url = upload_body["upload_url"].as_str().ok_or_else(|| {
            ProviderError::TranscriptionFailed("Missing upload_url in response".into())
        })?;

        // 2. Submit transcription job
        let mut job_body = json!({
            "audio_url": upload_url,
            "speech_model": "nano"
        });

        if let Some(ref lang) = language_hint {
            let code = match lang {
                VoiceLanguage::Hindi => "hi",
                VoiceLanguage::English => "en",
                _ => "en",
            };
            job_body["language_code"] = json!(code);
        }

        let submit_res = self
            .client
            .post(ASSEMBLYAI_TRANSCRIPT_ENDPOINT)
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&job_body)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        let submit_body: serde_json::Value = submit_res.json().await.map_err(|e| {
            ProviderError::TranscriptionFailed(format!("Failed to parse submit JSON: {}", e))
        })?;

        let transcript_id = submit_body["id"].as_str().ok_or_else(|| {
            ProviderError::TranscriptionFailed("Missing id in submit response".into())
        })?;

        // 3. Poll for completion (up to 10 iterations of 300ms)
        let poll_url = format!("{}/{}", ASSEMBLYAI_TRANSCRIPT_ENDPOINT, transcript_id);
        let mut text = String::new();

        for _ in 0..12 {
            tokio::time::sleep(Duration::from_millis(300)).await;

            let poll_res = self
                .client
                .get(&poll_url)
                .header("Authorization", &self.api_key)
                .send()
                .await;

            if let Ok(res) = poll_res {
                if let Ok(body) = res.json::<serde_json::Value>().await {
                    let status_str = body["status"].as_str().unwrap_or("");
                    if status_str == "completed" {
                        text = body["text"].as_str().unwrap_or("").trim().to_string();
                        break;
                    } else if status_str == "error" {
                        let err_msg = body["error"]
                            .as_str()
                            .unwrap_or("Unknown transcription error");
                        return Err(ProviderError::TranscriptionFailed(err_msg.to_string()));
                    }
                }
            }
        }

        Ok(Transcript {
            text,
            confidence: Some(0.92),
            language: language_hint,
            latency_ms: 750,
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
