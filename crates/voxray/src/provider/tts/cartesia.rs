//! Cartesia Sonic Text-to-Speech provider adapter.
//!
//! Connects to the official Cartesia Sonic TTS API:
//! POST https://api.cartesia.ai/tts/bytes
//! Headers:
//!   - X-API-Key: <api_key>
//!   - Cartesia-Version: 2024-06-10
//!   - Content-Type: application/json

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::json;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, TTSCapabilities, TTSProvider, TTSStream, VoiceLanguage,
};

pub const DEFAULT_CARTESIA_ENDPOINT: &str = "https://api.cartesia.ai/tts/bytes";
pub const DEFAULT_CARTESIA_VERSION: &str = "2024-06-10";
pub const DEFAULT_CARTESIA_MODEL: &str = "sonic-english";
pub const DEFAULT_CARTESIA_VOICE_ID: &str = "a0e99841-438c-4a64-b679-ae501e7d6091";
pub const DEFAULT_SAMPLE_RATE: u32 = 16000;
// Cartesia pricing: ~$0.075 per 1,000 characters = $0.000075 / char
pub const COST_PER_CHARACTER: f64 = 0.000075;

pub struct CartesiaTTSProvider {
    api_key: String,
    voice_id: String,
    model_id: String,
    endpoint: String,
    timeout: Duration,
    client: reqwest::Client,
    capabilities: TTSCapabilities,
}

impl CartesiaTTSProvider {
    pub fn new(
        api_key: impl Into<String>,
        voice_id: Option<String>,
        model_id: Option<String>,
    ) -> Self {
        Self::with_endpoint(api_key, voice_id, model_id, None, None)
    }

    pub fn with_endpoint(
        api_key: impl Into<String>,
        voice_id: Option<String>,
        model_id: Option<String>,
        endpoint: Option<String>,
        timeout: Option<Duration>,
    ) -> Self {
        let request_timeout = timeout.unwrap_or(Duration::from_secs(15));
        let client = reqwest::Client::builder()
            .timeout(request_timeout)
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
            typical_first_byte_ms: 100, // Ultra-low ~90-120ms first byte
            cost_per_char_usd: COST_PER_CHARACTER,
        };

        Self {
            api_key: api_key.into(),
            voice_id: voice_id.unwrap_or_else(|| DEFAULT_CARTESIA_VOICE_ID.to_string()),
            model_id: model_id.unwrap_or_else(|| DEFAULT_CARTESIA_MODEL.to_string()),
            endpoint: endpoint.unwrap_or_else(|| DEFAULT_CARTESIA_ENDPOINT.to_string()),
            timeout: request_timeout,
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        // Check if explicitly disabled via CARTESIA_ENABLED=false
        if let Ok(enabled) = std::env::var("CARTESIA_ENABLED") {
            let val = enabled.trim().to_lowercase();
            if val == "false" || val == "0" || val == "no" {
                tracing::info!("Cartesia TTS is disabled via CARTESIA_ENABLED=false");
                return None;
            }
        }

        let key = std::env::var("CARTESIA_API_KEY")
            .or_else(|_| std::env::var("TTS_CARTESIA_API_KEY"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let voice = std::env::var("CARTESIA_VOICE_ID")
            .or_else(|_| std::env::var("TTS_CARTESIA_VOICE_ID"))
            .ok();
        let model = std::env::var("CARTESIA_MODEL_ID")
            .or_else(|_| std::env::var("TTS_CARTESIA_MODEL_ID"))
            .ok();
        let endpoint = std::env::var("CARTESIA_ENDPOINT").ok();

        Some(Self::with_endpoint(key, voice, model, endpoint, None))
    }

    pub fn voice_id(&self) -> &str {
        &self.voice_id
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn raw_pcm16_to_audiodata(bytes: &[u8], sample_rate: u32) -> AudioData {
        let samples: Vec<f32> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();
        AudioData::from_pcm(samples, sample_rate, 1)
    }

    fn build_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();
        let mut key_val = HeaderValue::from_str(&self.api_key)
            .map_err(|_| ProviderError::InvalidCredentials("Invalid Cartesia API key format".into()))?;
        key_val.set_sensitive(true);

        headers.insert("X-API-Key", key_val);
        headers.insert(
            "Cartesia-Version",
            HeaderValue::from_static(DEFAULT_CARTESIA_VERSION),
        );
        headers.insert(
            "Content-Type",
            HeaderValue::from_static("application/json"),
        );

        Ok(headers)
    }

    fn build_payload(&self, text: &str, language: Option<VoiceLanguage>) -> serde_json::Value {
        let mut body = json!({
            "model_id": self.model_id,
            "transcript": text,
            "voice": {
                "mode": "id",
                "id": self.voice_id
            },
            "output_format": {
                "container": "raw",
                "encoding": "pcm_s16le",
                "sample_rate": DEFAULT_SAMPLE_RATE
            }
        });

        // Add language code if provided and using multilingual model
        if let Some(lang) = language {
            let code = match lang {
                VoiceLanguage::Hindi => Some("hi"),
                VoiceLanguage::Hinglish => Some("hi"),
                VoiceLanguage::English => Some("en"),
                VoiceLanguage::Other(ref c) if c.len() <= 5 => Some(c.as_str()),
                _ => None,
            };
            if let Some(c) = code {
                if let Some(obj) = body.as_object_mut() {
                    obj.insert("language".into(), json!(c));
                }
            }
        }

        body
    }

    async fn send_request_with_retry(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<reqwest::Response, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::InvalidCredentials("Empty Cartesia API key".into()));
        }

        let headers = self.build_headers()?;
        let body = self.build_payload(text, language);

        // Sanitize logging: do not log credentials
        debug!(
            provider = "cartesia-tts",
            endpoint = %self.endpoint,
            model = %self.model_id,
            voice = %self.voice_id,
            text_len = text.len(),
            "Sending request to Cartesia Sonic TTS"
        );

        let max_retries = 2;
        let mut attempt = 0;

        loop {
            attempt += 1;
            let req = self
                .client
                .post(&self.endpoint)
                .headers(headers.clone())
                .json(&body);

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();

                    // Rate limit: 429
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        let retry_after = resp
                            .headers()
                            .get("retry-after")
                            .and_then(|h| h.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(Duration::from_secs);
                        return Err(ProviderError::RateLimited {
                            retry_after,
                            reason: "Cartesia TTS 429 rate limit exceeded".into(),
                        });
                    }

                    // Auth failure: 401 or 403
                    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                        return Err(ProviderError::InvalidCredentials(
                            "Cartesia TTS authentication failed: invalid or unauthorized API key (401/403)".into(),
                        ));
                    }

                    // Transient server errors (500, 502, 503, 504) -> retry if attempts remain
                    if status.is_server_error() && attempt <= max_retries {
                        warn!(
                            attempt,
                            status = %status,
                            "Cartesia TTS transient server error; retrying with backoff"
                        );
                        tokio::time::sleep(Duration::from_millis(150 * attempt as u64)).await;
                        continue;
                    }

                    // Other errors (e.g. 400 Bad Request or persistent 5xx)
                    if !status.is_success() {
                        let err_txt = resp.text().await.unwrap_or_default();
                        warn!(status = %status, error = %err_txt, "Cartesia TTS request failed");
                        if status.is_server_error() {
                            return Err(ProviderError::ServerError {
                                status: status.as_u16(),
                                message: format!("Cartesia TTS server error: {}", err_txt),
                            });
                        } else {
                            return Err(ProviderError::SynthesisFailed(format!(
                                "Cartesia TTS error ({}): {}",
                                status, err_txt
                            )));
                        }
                    }

                    return Ok(resp);
                }
                Err(e) => {
                    if e.is_timeout() {
                        return Err(ProviderError::Timeout(self.timeout));
                    }
                    if attempt <= max_retries {
                        warn!(attempt, error = %e, "Cartesia TTS network error; retrying");
                        tokio::time::sleep(Duration::from_millis(150 * attempt as u64)).await;
                        continue;
                    }
                    return Err(ProviderError::NetworkError(format!(
                        "Cartesia network error: {}",
                        e
                    )));
                }
            }
        }
    }
}

#[async_trait]
impl TTSProvider for CartesiaTTSProvider {
    fn id(&self) -> &'static str {
        "cartesia-tts"
    }

    fn name(&self) -> &'static str {
        "Cartesia Sonic TTS"
    }

    fn capabilities(&self) -> &TTSCapabilities {
        &self.capabilities
    }

    async fn synthesize(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<AudioData, ProviderError> {
        let response = self.send_request_with_retry(text, language).await?;
        let bytes = response.bytes().await.map_err(|e| {
            ProviderError::SynthesisFailed(format!("Failed to read Cartesia audio bytes: {}", e))
        })?;

        Ok(Self::raw_pcm16_to_audiodata(&bytes, DEFAULT_SAMPLE_RATE))
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<TTSStream, ProviderError> {
        let response = self.send_request_with_retry(text, language).await?;
        let byte_stream = response.bytes_stream();

        let mapped_stream = byte_stream.map(|chunk_res| {
            chunk_res
                .map(|chunk| Self::raw_pcm16_to_audiodata(&chunk, DEFAULT_SAMPLE_RATE))
                .map_err(|e| ProviderError::SynthesisFailed(format!("Cartesia stream error: {}", e)))
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
