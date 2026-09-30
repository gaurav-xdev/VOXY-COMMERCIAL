//! Microsoft Azure Neural TTS provider adapter.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::HeaderMap;
use std::time::Duration;
use tracing::{debug, warn};

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, TTSCapabilities, TTSProvider, TTSStream, VoiceLanguage,
};

// Azure Neural TTS pricing ~$16 per 1M characters ($0.000016 per char)
const COST_PER_CHARACTER: f64 = 0.000016;

pub struct AzureTTSProvider {
    api_key: String,
    region: String,
    english_voice: String,
    hindi_voice: String,
    client: reqwest::Client,
    capabilities: TTSCapabilities,
}

impl AzureTTSProvider {
    pub fn new(
        api_key: impl Into<String>,
        region: Option<String>,
        english_voice: Option<String>,
        hindi_voice: Option<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_default();

        let capabilities = TTSCapabilities {
            streaming: true,
            supported_languages: vec![
                VoiceLanguage::English,
                VoiceLanguage::Hindi,
                VoiceLanguage::Hinglish,
            ],
            emotional_styles: true,
            speed_control: true,
            pitch_control: true,
            typical_first_byte_ms: 120,
            cost_per_char_usd: COST_PER_CHARACTER,
        };

        Self {
            api_key: api_key.into(),
            region: region.unwrap_or_else(|| "centralindia".to_string()),
            english_voice: english_voice.unwrap_or_else(|| "en-IN-NeerjaNeural".to_string()),
            hindi_voice: hindi_voice.unwrap_or_else(|| "hi-IN-SwaraNeural".to_string()),
            client,
            capabilities,
        }
    }

    pub fn from_env() -> Option<Self> {
        let key = std::env::var("TTS_AZURE_API_KEY")
            .or_else(|_| std::env::var("AZURE_SPEECH_KEY"))
            .ok()?;
        if key.trim().is_empty() {
            return None;
        }

        let region = std::env::var("TTS_AZURE_REGION")
            .or_else(|_| std::env::var("AZURE_SPEECH_REGION"))
            .ok();
        let en_voice = std::env::var("TTS_AZURE_VOICE_EN").ok();
        let hi_voice = std::env::var("TTS_AZURE_VOICE_HI").ok();

        Some(Self::new(key, region, en_voice, hi_voice))
    }

    fn select_voice(&self, lang: Option<VoiceLanguage>) -> (&str, &str) {
        match lang {
            Some(VoiceLanguage::Hindi) | Some(VoiceLanguage::Hinglish) => {
                ("hi-IN", self.hindi_voice.as_str())
            }
            _ => ("en-IN", self.english_voice.as_str()),
        }
    }

    fn build_ssml(&self, text: &str, lang: Option<VoiceLanguage>) -> String {
        let (xml_lang, voice_name) = self.select_voice(lang);
        let escaped_text = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;");

        format!(
            "<speak version='1.0' xml:lang='{xml_lang}'><voice xml:lang='{xml_lang}' name='{voice_name}'>{escaped_text}</voice></speak>"
        )
    }

    fn raw_pcm16_to_audiodata(bytes: &[u8], sample_rate: u32) -> AudioData {
        let samples: Vec<f32> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
            .collect();
        AudioData::from_pcm(samples, sample_rate, 1)
    }
}

#[async_trait]
impl TTSProvider for AzureTTSProvider {
    fn id(&self) -> &'static str {
        "azure-neural-tts"
    }

    fn name(&self) -> &'static str {
        "Microsoft Azure Neural TTS"
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
            return Err(ProviderError::AuthError("Empty Azure TTS API key".into()));
        }

        let url = format!(
            "https://{}.tts.speech.microsoft.com/cognitiveservices/v1",
            self.region
        );
        let ssml = self.build_ssml(text, language);

        debug!(
            provider = self.id(),
            text_len = text.len(),
            "Calling Azure Neural TTS"
        );

        let response = self
            .client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", "raw-16khz-16bit-mono-pcm")
            .header("User-Agent", "VOXY-AudioEngine")
            .body(ssml)
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
                reason: "Azure Neural TTS 429 rate limit exceeded".into(),
            });
        }

        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(ProviderError::AuthError(
                "Azure TTS authentication failed (401/403)".into(),
            ));
        }

        if !status.is_success() {
            let err_txt = response.text().await.unwrap_or_default();
            warn!(status = %status, error = %err_txt, "Azure TTS failure");
            return Err(ProviderError::SynthesisFailed(format!(
                "Azure TTS error ({}): {}",
                status, err_txt
            )));
        }

        let bytes = response.bytes().await.map_err(|e| {
            ProviderError::SynthesisFailed(format!("Failed to read audio bytes: {}", e))
        })?;

        Ok(Self::raw_pcm16_to_audiodata(&bytes, 16000))
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<TTSStream, ProviderError> {
        if self.api_key.trim().is_empty() {
            return Err(ProviderError::AuthError("Empty Azure TTS API key".into()));
        }

        let url = format!(
            "https://{}.tts.speech.microsoft.com/cognitiveservices/v1",
            self.region
        );
        let ssml = self.build_ssml(text, language);

        let response = self
            .client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.api_key)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", "raw-16khz-16bit-mono-pcm")
            .header("User-Agent", "VOXY-AudioEngine")
            .body(ssml)
            .send()
            .await
            .map_err(|e| ProviderError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_txt = response.text().await.unwrap_or_default();
            return Err(ProviderError::SynthesisFailed(format!(
                "Azure TTS stream error ({}): {}",
                status, err_txt
            )));
        }

        let byte_stream = response.bytes_stream();
        let mapped = byte_stream.map(|chunk_res| {
            chunk_res
                .map(|chunk| Self::raw_pcm16_to_audiodata(&chunk, 16000))
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
