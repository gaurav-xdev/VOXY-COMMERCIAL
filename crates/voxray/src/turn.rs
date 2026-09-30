use async_trait::async_trait;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::frames::{
    AudioRawFrame, Frame, FrameDirection, UserStartedSpeakingFrame, UserStoppedSpeakingFrame,
};
use crate::processor::Processor;

pub struct TurnProcessorConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub energy_threshold: f32,
    pub silence_timeout: Duration,
    pub min_speech_duration: Duration,
    pub max_turn_duration: Duration,
}

impl Default for TurnProcessorConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            energy_threshold: 0.015,
            silence_timeout: Duration::from_millis(600),
            min_speech_duration: Duration::from_millis(200),
            max_turn_duration: Duration::from_secs(12),
        }
    }
}

/// TurnProcessor handles Voxray VAD, speech chunk accumulation, and turn detection.
pub struct TurnProcessor {
    config: Mutex<TurnProcessorConfig>,
    in_speech: Arc<AtomicBool>,
    buffer: Arc<Mutex<Vec<u8>>>,
    speech_start: Arc<Mutex<Option<Instant>>>,
    last_speech_time: Arc<Mutex<Option<Instant>>>,
}

impl TurnProcessor {
    pub fn new(config: TurnProcessorConfig) -> Self {
        Self {
            config: Mutex::new(config),
            in_speech: Arc::new(AtomicBool::new(false)),
            buffer: Arc::new(Mutex::new(Vec::with_capacity(32000))),
            speech_start: Arc::new(Mutex::new(None)),
            last_speech_time: Arc::new(Mutex::new(None)),
        }
    }

    fn calculate_rms(audio_bytes: &[u8]) -> f32 {
        if audio_bytes.len() < 2 {
            return 0.0;
        }
        let sample_count = audio_bytes.len() / 2;
        let mut sum_sq = 0.0f64;
        for i in 0..sample_count {
            let sample = i16::from_le_bytes([audio_bytes[i * 2], audio_bytes[i * 2 + 1]]);
            let norm = sample as f64 / 32768.0;
            sum_sq += norm * norm;
        }
        (sum_sq / sample_count as f64).sqrt() as f32
    }
}

#[async_trait]
impl Processor for TurnProcessor {
    fn name(&self) -> &str {
        "TurnProcessor"
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
            Frame::VADParamsUpdate(v) => {
                {
                    let mut cfg = self.config.lock();
                    if let Some(th) = v.threshold {
                        cfg.energy_threshold = th;
                    }
                    if let Some(sec) = v.stop_secs {
                        cfg.silence_timeout = Duration::from_secs_f64(sec);
                    }
                }
                let _ = out_tx.send(Frame::VADParamsUpdate(v)).await;
            }
            Frame::AudioRaw(audio) => {
                let rms = Self::calculate_rms(&audio.audio);
                let (thresh, silence_timeout, max_duration) = {
                    let cfg = self.config.lock();
                    (
                        cfg.energy_threshold,
                        cfg.silence_timeout,
                        cfg.max_turn_duration,
                    )
                };

                let now = Instant::now();
                let is_voice = rms >= thresh;

                if is_voice {
                    {
                        let mut last_sp = self.last_speech_time.lock();
                        *last_sp = Some(now);
                    }

                    if !self.in_speech.load(Ordering::SeqCst) {
                        self.in_speech.store(true, Ordering::SeqCst);
                        {
                            let mut sp_start = self.speech_start.lock();
                            *sp_start = Some(now);
                        }
                        {
                            let mut buf = self.buffer.lock();
                            buf.clear();
                            buf.extend_from_slice(&audio.audio);
                        }

                        // Broadcast that user started speaking immediately
                        let _ = out_tx
                            .send(Frame::UserStartedSpeaking(
                                UserStartedSpeakingFrame::default(),
                            ))
                            .await;
                    } else {
                        let mut buf = self.buffer.lock();
                        buf.extend_from_slice(&audio.audio);
                    }
                } else if self.in_speech.load(Ordering::SeqCst) {
                    let mut should_emit_turn = false;
                    let mut speech_data = Vec::new();

                    {
                        let mut buf = self.buffer.lock();
                        buf.extend_from_slice(&audio.audio);

                        let last_sp = *self.last_speech_time.lock();
                        let sp_start = *self.speech_start.lock();

                        let silence_elapsed =
                            last_sp.map(|t| now.duration_since(t)).unwrap_or_default();
                        let total_elapsed =
                            sp_start.map(|t| now.duration_since(t)).unwrap_or_default();

                        // Turn complete condition
                        if silence_elapsed >= silence_timeout || total_elapsed >= max_duration {
                            self.in_speech.store(false, Ordering::SeqCst);
                            speech_data = buf.clone();
                            buf.clear();
                            should_emit_turn = true;
                        }
                    }

                    if should_emit_turn {
                        // Emit UserStoppedSpeaking
                        let _ = out_tx
                            .send(Frame::UserStoppedSpeaking(
                                UserStoppedSpeakingFrame::default(),
                            ))
                            .await;

                        // Emit aggregated turn audio downstream for STT
                        let turn_audio =
                            AudioRawFrame::new(speech_data, audio.sample_rate, audio.num_channels);
                        let _ = out_tx.send(Frame::AudioRaw(turn_audio)).await;
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
