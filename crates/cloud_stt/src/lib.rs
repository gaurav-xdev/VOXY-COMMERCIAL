use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, warn};
use voxy_voice_orchestrator::{AudioChunk, AudioStream, SttEngine};

#[derive(Debug, thiserror::Error)]
pub enum CloudSttError {
    #[error("HTTP request failed: {0}")]
    HttpRequest(String),
    #[error("API error ({status}): {body}")]
    ApiError { status: u16, body: String },
    #[error("Audio conversion failed: {0}")]
    AudioConversion(String),
    #[error("No API key configured")]
    NoApiKey,
}

pub struct CloudSttConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl Clone for CloudSttConfig {
    fn clone(&self) -> Self {
        Self {
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
        }
    }
}

impl Default for CloudSttConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openai.com".into(),
            model: "whisper-1".into(),
        }
    }
}

pub struct CloudSttEngine {
    name: String,
    config: CloudSttConfig,
    client: Client,
}

impl CloudSttEngine {
    pub fn new(config: CloudSttConfig) -> Self {
        Self {
            name: "cloud-stt".into(),
            config,
            client: Client::new(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    fn f32_to_wav(data: &[f32], sample_rate: u32) -> Result<Vec<u8>, CloudSttError> {
        if data.is_empty() {
            return Err(CloudSttError::AudioConversion("empty audio data".into()));
        }

        let byte_rate = sample_rate * 2;
        let block_align = 2u16;
        let data_size = (data.len() * 2) as u32;
        let header_size = 44u32;

        let mut wav = Vec::with_capacity((header_size + data_size) as usize);

        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(header_size + data_size - 8).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());

        for &sample in data {
            let clamped = sample.clamp(-1.0, 1.0);
            let pcm = (clamped * i16::MAX as f32) as i16;
            wav.extend_from_slice(&pcm.to_le_bytes());
        }

        Ok(wav)
    }
}

#[async_trait]
impl SttEngine for CloudSttEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn transcribe(
        &self,
        audio: &AudioChunk,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        if audio.data.is_empty() {
            return Ok(String::new());
        }

        if self.config.api_key.is_empty() {
            return Err(
                voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(
                    CloudSttError::NoApiKey.to_string(),
                ),
            );
        }

        let wav_bytes = Self::f32_to_wav(&audio.data, audio.sample_rate).map_err(|e| {
            voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(e.to_string())
        })?;

        let url = format!("{}/v1/audio/transcriptions", self.config.base_url);

        let part = reqwest::multipart::Part::bytes(wav_bytes)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| {
                voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(e.to_string())
            })?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.config.model.clone())
            .text("response_format", "text");

        debug!("Sending audio to cloud STT ({})", self.config.model);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(
                    CloudSttError::HttpRequest(e.to_string()).to_string(),
                )
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!("Cloud STT API error {}: {}", status, body);
            return Err(
                voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(
                    CloudSttError::ApiError {
                        status: status.as_u16(),
                        body,
                    }
                    .to_string(),
                ),
            );
        }

        let text = response.text().await.map_err(|e| {
            voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed(
                CloudSttError::HttpRequest(e.to_string()).to_string(),
            )
        })?;

        debug!("Cloud STT result: {:?}", text);
        Ok(text.trim().to_string())
    }

    async fn transcribe_stream(
        &self,
        mut stream: Box<dyn AudioStream>,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        let mut accumulated = Vec::new();
        while let Some(chunk) = stream.next_chunk().await {
            accumulated.extend_from_slice(&chunk.data);
        }

        if accumulated.is_empty() {
            return Ok(String::new());
        }

        let sr = 16000;
        let fake_chunk = AudioChunk {
            data: accumulated,
            sample_rate: sr,
            channels: 1,
            timestamp: chrono::Utc::now(),
            sequence: 0,
            is_final: true,
        };

        self.transcribe(&fake_chunk).await
    }

    fn supported_languages(&self) -> Vec<String> {
        vec![
            "en".into(),
            "es".into(),
            "fr".into(),
            "de".into(),
            "it".into(),
            "pt".into(),
            "ru".into(),
            "ja".into(),
            "ko".into(),
            "zh".into(),
            "ar".into(),
            "hi".into(),
            "nl".into(),
            "pl".into(),
            "sv".into(),
            "tr".into(),
        ]
    }

    fn is_available(&self) -> bool {
        !self.config.api_key.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_chunk(data: Vec<f32>, sample_rate: u32, is_final: bool) -> AudioChunk {
        AudioChunk {
            data,
            sample_rate,
            channels: 1,
            timestamp: Utc::now(),
            sequence: 0,
            is_final,
        }
    }

    #[test]
    fn test_f32_to_wav_empty() {
        let result = CloudSttEngine::f32_to_wav(&[], 16000);
        assert!(result.is_err());
    }

    #[test]
    fn test_f32_to_wav_header() {
        let data = vec![0.0; 160];
        let wav = CloudSttEngine::f32_to_wav(&data, 16000).unwrap();
        assert!(wav.starts_with(b"RIFF"));
        assert!(wav.len() >= 44 + 320);
    }

    #[test]
    fn test_f32_to_wav_pcm_conversion() {
        let data = vec![0.5, -0.5, 1.0, -1.0];
        let wav = CloudSttEngine::f32_to_wav(&data, 16000).unwrap();
        let data_offset = 44;
        assert_eq!(wav.len(), data_offset + 8);
        let pcm0 = i16::from_le_bytes([wav[data_offset], wav[data_offset + 1]]);
        let expected = (0.5f32 * i16::MAX as f32) as i16;
        assert_eq!(pcm0, expected);
    }

    #[test]
    fn test_f32_to_wav_clamping() {
        let data = vec![2.0, -2.0];
        let wav = CloudSttEngine::f32_to_wav(&data, 16000).unwrap();
        let data_offset = 44;
        let pcm0 = i16::from_le_bytes([wav[data_offset], wav[data_offset + 1]]);
        let pcm1 = i16::from_le_bytes([wav[data_offset + 2], wav[data_offset + 3]]);
        assert_eq!(pcm0, (1.0 * i16::MAX as f32) as i16);
        assert_eq!(pcm1, -(i16::MAX as f32) as i16);
    }

    #[tokio::test]
    async fn test_cloud_stt_empty_audio() {
        let config = CloudSttConfig {
            api_key: "test".into(),
            ..Default::default()
        };
        let engine = CloudSttEngine::new(config);
        let chunk = make_chunk(vec![], 16000, true);
        let result = engine.transcribe(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_cloud_stt_no_api_key() {
        let engine = CloudSttEngine::new(CloudSttConfig::default());
        let chunk = make_chunk(vec![0.1; 160], 16000, true);
        let result = engine.transcribe(&chunk).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cloud_stt_name() {
        let engine = CloudSttEngine::new(CloudSttConfig::default());
        assert_eq!(engine.name(), "cloud-stt");
    }

    #[tokio::test]
    async fn test_cloud_stt_custom_name() {
        let engine = CloudSttEngine::new(CloudSttConfig::default()).with_name("custom");
        assert_eq!(engine.name(), "custom");
    }

    #[tokio::test]
    async fn test_cloud_stt_is_available() {
        let mut config = CloudSttConfig::default();
        assert!(!CloudSttEngine::new(config.clone()).is_available());
        config.api_key = "sk-test".into();
        assert!(CloudSttEngine::new(config).is_available());
    }

    #[tokio::test]
    async fn test_cloud_stt_supported_languages() {
        let engine = CloudSttEngine::new(CloudSttConfig::default());
        let langs = engine.supported_languages();
        assert!(langs.contains(&"en".to_string()));
        assert!(langs.len() > 10);
    }
}
