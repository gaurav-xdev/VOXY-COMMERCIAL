use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;
use crate::frames::{Frame, FrameDirection, TranscriptionFrame};
use crate::processor::Processor;

#[async_trait]
pub trait VoxraySttService: Send + Sync {
    async fn transcribe(&self, audio_pcm: &[u8], sample_rate: u32, channels: u16) -> Result<String, String>;
}

/// Mock / Default STT service for test and offline environments.
pub struct EchoSttService;

#[async_trait]
impl VoxraySttService for EchoSttService {
    async fn transcribe(&self, audio_pcm: &[u8], _sample_rate: u32, _channels: u16) -> Result<String, String> {
        if audio_pcm.is_empty() {
            return Ok(String::new());
        }
        Ok("hello voxy".to_string())
    }
}

/// Cloud-based STT Service (compatible with OpenAI Whisper API, Groq Whisper, etc.)
pub struct CloudOpenAiSttService {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl CloudOpenAiSttService {
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
        }
    }

    fn pcm_to_wav(pcm_bytes: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
        let byte_rate = sample_rate * channels as u32 * 2;
        let block_align = channels * 2;
        let data_size = pcm_bytes.len() as u32;
        let header_size = 44u32;

        let mut wav = Vec::with_capacity((header_size + data_size) as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(header_size + data_size - 8).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes()); // 16 bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());
        wav.extend_from_slice(pcm_bytes);
        wav
    }
}

#[async_trait]
impl VoxraySttService for CloudOpenAiSttService {
    async fn transcribe(&self, audio_pcm: &[u8], sample_rate: u32, channels: u16) -> Result<String, String> {
        if audio_pcm.is_empty() {
            return Ok(String::new());
        }

        if self.api_key.is_empty() {
            return Err("No API key configured for Cloud STT".to_string());
        }

        let wav_data = Self::pcm_to_wav(audio_pcm, sample_rate, channels);
        let url = format!("{}/v1/audio/transcriptions", self.base_url.trim_end_matches('/'));

        let part = reqwest::multipart::Part::bytes(wav_data)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| e.to_string())?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", self.model.clone())
            .text("response_format", "text");

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Cloud STT API error {}: {}", status, body));
        }

        let text = response.text().await.map_err(|e| e.to_string())?;
        Ok(text.trim().to_string())
    }
}

/// STTProcessor consumes AudioRawFrame segments and pushes TranscriptionFrame downstream.
pub struct STTProcessor {
    service: Arc<dyn VoxraySttService>,
    min_buffer_bytes: usize,
}

impl STTProcessor {
    pub fn new(service: Arc<dyn VoxraySttService>) -> Self {
        Self {
            service,
            min_buffer_bytes: 3200, // at least 100ms of 16kHz mono audio
        }
    }
}

#[async_trait]
impl Processor for STTProcessor {
    fn name(&self) -> &str {
        "STTProcessor"
    }

    async fn process_frame(
        &self,
        frame: Frame,
        direction: FrameDirection,
        out_tx: &mpsc::Sender<Frame>,
    ) -> Result<(), String> {
        if direction != FrameDirection::Downstream {
            let _ = out_tx.send(frame).await;
            return Ok(());
        }

        match frame {
            Frame::AudioRaw(audio) => {
                if audio.audio.len() < self.min_buffer_bytes {
                    return Ok(());
                }

                match self.service.transcribe(&audio.audio, audio.sample_rate, audio.num_channels).await {
                    Ok(text) => {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            tracing::info!("[VOXRAY:STT] Transcribed: '{}'", trimmed);
                            let tf = TranscriptionFrame::new(trimmed, true);
                            let _ = out_tx.send(Frame::Transcription(tf)).await;
                        }
                    }
                    Err(e) => {
                        tracing::error!("[VOXRAY:STT] Transcription error: {}", e);
                    }
                }
            }
            other => {
                let _ = out_tx.send(other).await;
            }
        }

        Ok(())
    }
}
