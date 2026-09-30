use crate::frames::{
    BotStartedSpeakingFrame, BotStoppedSpeakingFrame, Frame, FrameDirection, TTSAudioRawFrame,
};
use crate::processor::Processor;
use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

#[async_trait]
pub trait VoxrayTtsService: Send + Sync {
    async fn synthesize(&self, text: &str, sample_rate: u32) -> Result<Vec<u8>, String>;
}

/// Fallback / Tone synthesis service for offline testing.
pub struct ToneTtsService;

#[async_trait]
impl VoxrayTtsService for ToneTtsService {
    async fn synthesize(&self, text: &str, sample_rate: u32) -> Result<Vec<u8>, String> {
        if text.is_empty() {
            return Ok(Vec::new());
        }
        // Production audit: ToneTtsService must NEVER emit an audible 440Hz sine wave beep.
        // In offline fallback, return silent PCM rather than generating an annoying test chime.
        let samples = (sample_rate / 10) as usize;
        let pcm = vec![0u8; samples * 2];
        Ok(pcm)
    }
}

/// Windows SAPI TTS — native COM-based speech synthesis.
/// Uses ISpVoice to synthesize speech locally without any API key.
/// Outputs 16-bit PCM at 22050 Hz (SAPI default), resampled to requested rate.
#[cfg(target_os = "windows")]
pub struct WindowsSapiTtsService {
    _initialized: bool,
}

#[cfg(target_os = "windows")]
impl WindowsSapiTtsService {
    pub fn new() -> Result<Self, String> {
        // Verify COM can be initialized (will be done per-call in synthesize)
        Ok(Self { _initialized: true })
    }
}

#[cfg(target_os = "windows")]
#[async_trait]
impl VoxrayTtsService for WindowsSapiTtsService {
    async fn synthesize(&self, text: &str, target_sample_rate: u32) -> Result<Vec<u8>, String> {
        if text.is_empty() {
            return Ok(Vec::new());
        }

        let text = text.to_string();
        let target_sr = target_sample_rate;

        // SAPI COM must run on a dedicated thread (STA apartment)
        let result =
            tokio::task::spawn_blocking(move || sapi_synthesize_blocking(&text, target_sr))
                .await
                .map_err(|e| format!("SAPI task join error: {e}"))?;

        result
    }
}

#[cfg(target_os = "windows")]
fn sapi_synthesize_blocking(text: &str, target_sample_rate: u32) -> Result<Vec<u8>, String> {
    unsafe {
        // Initialize COM for this thread (STA)
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
        };
        let com_hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let com_initialized = com_hr.is_ok();

        let result = sapi_synthesize_inner(text, target_sample_rate);

        if com_initialized {
            CoUninitialize();
        }

        result
    }
}

#[cfg(target_os = "windows")]
fn sapi_synthesize_inner(text: &str, target_sample_rate: u32) -> Result<Vec<u8>, String> {
    use windows::core::*;
    use windows::Win32::Media::Speech::*;

    unsafe {
        // Create SAPI voice
        let voice: ISpVoice = windows::Win32::System::Com::CoCreateInstance(
            &SpVoice,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .map_err(|e| format!("Failed to create ISpVoice: {e}"))?;

        // Create in-memory IStream
        let mem_stream =
            windows::Win32::System::Com::StructuredStorage::CreateStreamOnHGlobal(None, true)
                .map_err(|e| format!("Failed to create in-memory IStream: {e}"))?;

        // Create SpStream for SAPI
        let stream: ISpStream = windows::Win32::System::Com::CoCreateInstance(
            &SpStream,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .map_err(|e| format!("Failed to create SpStream: {e}"))?;

        // Set output format: 22050 Hz, 16-bit mono PCM
        use windows::Win32::Media::Audio::WAVEFORMATEX;
        let wave_format = WAVEFORMATEX {
            wFormatTag: 1, // WAVE_FORMAT_PCM
            nChannels: 1,
            nSamplesPerSec: 22050,
            nAvgBytesPerSec: 22050 * 2,
            nBlockAlign: 2,
            wBitsPerSample: 16,
            cbSize: 0,
        };

        // SPDFID_WaveFormatEx GUID
        let spdfid_wave: windows::core::GUID =
            windows::core::GUID::from_u128(0xC31ADBAE_527F_4FF5_A230_F62BB61FF70C);

        stream
            .SetBaseStream(
                &mem_stream,
                &spdfid_wave as *const _,
                &wave_format as *const _,
            )
            .map_err(|e| format!("Failed to set stream format: {e}"))?;

        // Redirect voice output to memory stream
        let output_stream: ISpStreamFormat = stream
            .cast::<ISpStreamFormat>()
            .map_err(|e| format!("Failed to cast to ISpStreamFormat: {e}"))?;
        voice
            .SetOutput(&output_stream, true)
            .map_err(|e| format!("Failed to set voice output: {e}"))?;

        // Speak synchronously
        let wide_text: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let text_pcwstr = PCWSTR(wide_text.as_ptr());
        voice
            .Speak(text_pcwstr, SPF_DEFAULT.0 as u32, None)
            .map_err(|e| format!("SAPI Speak failed: {e}"))?;

        // Seek to start of in-memory stream
        let seek_origin = windows::Win32::System::Com::STREAM_SEEK_SET;
        mem_stream
            .Seek(0, seek_origin, None)
            .map_err(|e| format!("Failed to seek stream: {e}"))?;

        // Read all data
        let mut all_data = Vec::new();
        loop {
            let mut buf = [0u8; 8192];
            let mut bytes_read = 0u32;
            let hr = mem_stream.Read(
                buf.as_mut_ptr() as *mut _,
                buf.len() as u32,
                Some(&mut bytes_read),
            );
            if bytes_read == 0 || hr.is_err() {
                break;
            }
            all_data.extend_from_slice(&buf[..bytes_read as usize]);
        }

        if all_data.is_empty() {
            return Err("SAPI produced no audio data".to_string());
        }

        tracing::info!(
            "[SAPI:TTS] Synthesized {} bytes ({} samples) at 22050 Hz",
            all_data.len(),
            all_data.len() / 2,
        );

        // Convert from SAPI 22050 Hz to target sample rate if needed
        if target_sample_rate == 22050 || target_sample_rate == 0 {
            Ok(all_data)
        } else {
            // Simple linear interpolation resampling
            let src_samples: Vec<i16> = all_data
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();

            let ratio = target_sample_rate as f64 / 22050.0;
            let dst_len = (src_samples.len() as f64 * ratio) as usize;
            let mut dst = Vec::with_capacity(dst_len * 2);

            for i in 0..dst_len {
                let src_pos = i as f64 / ratio;
                let idx = src_pos as usize;
                let frac = src_pos - idx as f64;

                let s0 = src_samples[idx.min(src_samples.len() - 1)] as f64;
                let s1 = src_samples[(idx + 1).min(src_samples.len() - 1)] as f64;
                let sample = (s0 + (s1 - s0) * frac) as i16;

                dst.extend_from_slice(&sample.to_le_bytes());
            }

            tracing::info!(
                "[SAPI:TTS] Resampled to {} Hz: {} bytes",
                target_sample_rate,
                dst.len(),
            );

            Ok(dst)
        }
    }
}

/// Cloud-based TTS Service (compatible with OpenAI TTS API format / audio/speech)
pub struct CloudOpenAiTtsService {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    voice: String,
}

impl CloudOpenAiTtsService {
    pub fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        voice: impl Into<String>,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
            voice: voice.into(),
        }
    }
}

#[async_trait]
impl VoxrayTtsService for CloudOpenAiTtsService {
    async fn synthesize(&self, text: &str, _sample_rate: u32) -> Result<Vec<u8>, String> {
        if text.is_empty() {
            return Ok(Vec::new());
        }

        if self.api_key.is_empty() {
            return Err("No API key configured for Cloud TTS".to_string());
        }

        let url = format!("{}/v1/audio/speech", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "input": text,
            "voice": self.voice,
            "response_format": "pcm",
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response.text().await.unwrap_or_default();
            return Err(format!("Cloud TTS API error {}: {}", status, err_body));
        }

        let bytes = response.bytes().await.map_err(|e| e.to_string())?;
        Ok(bytes.to_vec())
    }
}

/// TTSProcessor consumes TTSSpeakFrame / LLMTextFrame, batches by sentence, and produces TTSAudioRawFrame.
pub struct TTSProcessor {
    service: Arc<dyn VoxrayTtsService>,
    sample_rate: u32,
    buffer: Arc<Mutex<String>>,
    bot_speaking: Arc<AtomicBool>,
}

impl TTSProcessor {
    pub fn new(service: Arc<dyn VoxrayTtsService>, sample_rate: u32) -> Self {
        Self {
            service,
            sample_rate,
            buffer: Arc::new(Mutex::new(String::new())),
            bot_speaking: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_speaking(&self) -> bool {
        self.bot_speaking.load(Ordering::SeqCst)
    }

    async fn speak_text(&self, text: &str, out_tx: &mpsc::Sender<Frame>) -> Result<(), String> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Ok(());
        }

        tracing::info!("[VOXRAY:TTS] Synthesizing: '{}'", trimmed);
        match self.service.synthesize(trimmed, self.sample_rate).await {
            Ok(audio_bytes) => {
                if !audio_bytes.is_empty() {
                    if !self.bot_speaking.swap(true, Ordering::SeqCst) {
                        let _ = out_tx
                            .send(Frame::BotStartedSpeaking(BotStartedSpeakingFrame::default()))
                            .await;
                    }
                    let out_frame = TTSAudioRawFrame::new(audio_bytes, self.sample_rate, 1);
                    let _ = out_tx.send(Frame::TTSAudioRaw(out_frame)).await;
                    let _ = out_tx
                        .send(Frame::BotStoppedSpeaking(BotStoppedSpeakingFrame::default()))
                        .await;
                    self.bot_speaking.store(false, Ordering::SeqCst);
                }
            }
            Err(e) => {
                tracing::error!("[VOXRAY:TTS] Synthesis error: {}", e);
                self.bot_speaking.store(false, Ordering::SeqCst);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Processor for TTSProcessor {
    fn name(&self) -> &str {
        "TTSProcessor"
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
            Frame::TTSSpeak(s) => {
                self.speak_text(&s.text, out_tx).await?;
            }
            Frame::LLMText(t) => {
                let sentence_to_speak = {
                    let mut buf = self.buffer.lock();
                    buf.push_str(&t.text);

                    // Check for sentence boundary: '.', '!', '?', '\n'
                    if let Some(pos) = buf.find(['.', '!', '?', '\n']) {
                        let sentence: String = buf.drain(..=pos).collect();
                        Some(sentence)
                    } else {
                        None
                    }
                };

                if let Some(sentence) = sentence_to_speak {
                    self.speak_text(&sentence, out_tx).await?;
                }
            }
            Frame::Interruption(_) | Frame::UserStartedSpeaking(_) => {
                // Clear pending TTS buffer immediately
                self.buffer.lock().clear();
                self.bot_speaking.store(false, Ordering::SeqCst);
                let _ = out_tx.send(frame).await;
            }
            other => {
                let _ = out_tx.send(other).await;
            }
        }

        Ok(())
    }
}
