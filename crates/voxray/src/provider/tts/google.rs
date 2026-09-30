//! Google Cloud Text-to-Speech provider adapter.

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, TTSCapabilities, TTSProvider, VoiceLanguage,
};

const GOOGLE_TTS_ENDPOINT: &str = "https://texttospeech.googleapis.com/v1/text:synthesize";
// Journey/Neural2 voice pricing ~$16 per 1M characters
const COST_PER_CHARACTER: f64 = 0.000016;

#[derive(Serialize)]
struct GoogleTTSInput<'a> {
    text: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTTSVoice<'a> {
    language_code: &'a str,
    name: &'a str,
    ssml_gender: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTTSAudioConfig {
    audio_encoding: &'static str,
    sample_rate_hertz: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTTSRequest<'a> {
    input: GoogleTTSInput<'a>,
    voice: GoogleTTSVoice<'a>,
    audio_config: GoogleTTSAudioConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleTTSResponse {
    audio_content: String,
}

pub struct GoogleTTSProvider {
    api_key: String,
    english_voice: String,
    hindi_voice: String,
    client: reqwest::Client,
    capabilities: TTSCapabilities,
}

impl GoogleTTSProvider {
    pub fn new(
        api_key: impl Into<String>,
        english_voice: Option<String>,
        hindi_voice: Option<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .unwrap_or_default();

        let capabilities = TTSCapabilities {
            streaming: false,
            supported_languages: vec![
                VoiceLanguage::English,
                VoiceLanguage::Hindi,
                VoiceLanguage::Hinglish,
            ],
            emotional_styles: true,
            speed_control: true,
            pitch_control: true,
            typical_first_byte_ms: 220,
            cost_per_char_usd: COST_PER_CHARACTER,
        };

        Self {
            api_key: api_key.into(),
            english_voice: english_voice.unwrap_or_else(|| "en-IN-Journey-F".to_string()),
            hindi_voice: hindi_voice.unwrap_or_else(|| "hi-IN-Neural2-A".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("TTS_GOOGLE_API_KEY")
            .or_else(|_| std::env::var("GOOGLE_TTS_API_KEY"))
            .or_else(|_| std::env::var("VOXY_API_KEYS_GOOGLE"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let en_voice = std::env::var("TTS_GOOGLE_VOICE_EN").ok();
        let hi_voice = std::env::var("TTS_GOOGLE_VOICE_HI").ok();
        Some(Self::new(key, en_voice, hi_voice))
    }

    fn select_voice(&self, lang: Option<VoiceLanguage>) -> (&str, &str) {
        match lang {
            Some(VoiceLanguage::Hindi) | Some(VoiceLanguage::Hinglish) => {
                ("hi-IN", self.hindi_voice.as_str())
            }
            _ => ("en-IN", self.english_voice.as_str()),
        }
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
impl TTSProvider for GoogleTTSProvider {
    fn id(&self) -> &'static str {
        "google-cloud-tts"
    }

    fn name(&self) -> &'static str {
        "Google Cloud Text-to-Speech (Neural2/Journey)"
    }

    fn capabilities(&self) -> &TTSCapabilities {
        &self.capabilities
    }

    async fn synthesize(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<AudioData, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::AuthError("Empty Google TTS API key".into()));
        }

        let (lang_code, voice_name) = self.select_voice(language);
        let url = format!("{}?key={}", GOOGLE_TTS_ENDPOINT, self.api_key);

        let req_body = GoogleTTSRequest {
            input: GoogleTTSInput { text },
            voice: GoogleTTSVoice {
                language_code: lang_code,
                name: voice_name,
                ssml_gender: "FEMALE",
            },
            audio_config: GoogleTTSAudioConfig {
                audio_encoding: "LINEAR16",
                sample_rate_hertz: 16000,
            },
        };

        debug!(
            provider = self.id(),
            voice = voice_name,
            "Calling Google Cloud TTS"
        );

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&req_body)
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
                reason: "Google TTS 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "Google TTS authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Google TTS failure");
            return Err(ProviderError::SynthesisFailed(format!(
                "Google TTS error ({}): {}",
                status, err_txt
            )));
        }

        let resp: GoogleTTSResponse = response.json().await.map_err(|e| {
            ProviderError::SynthesisFailed(format!("Failed to parse Google TTS response: {}", e))
        })?;

        let decoded = BASE64.decode(&resp.audio_content).map_err(|e| {
            ProviderError::SynthesisFailed(format!("Failed to base64 decode audioContent: {}", e))
        })?;

        Ok(Self::decode_wav_or_raw_pcm(&decoded, 16000))
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
