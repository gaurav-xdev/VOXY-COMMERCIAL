//! Authoritative Voice Session State Machine, Barge-In Controller, and Streaming Pipeline Orchestration.
//!
//! Orchestrates the full real-time conversational voice loop:
//! Windows Audio Capture -> Audio Conditioning -> VAD -> STT -> OSMOO LLM Router -> Sentence Chunker -> Streaming TTS -> Playback
//!
//! Implements:
//! - Immediate barge-in cancellation: user utterance halts active TTS playback and in-flight LLM token streaming
//! - Synchronized IPC state broadcasting (Idle, Listening, Thinking, Speaking, Interrupted)
//! - Push-to-talk and microphone mute controls
//! - Modular hot-swappable local and cloud STT/TTS adapters

use futures_util::future::BoxFuture;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use voxy_ipc::CompanionState;
use voxy_voice_activation::{ActivationMode, VadConfig, VadDetector, VadEvent};
use voxy_voice_audio::{AudioCaptureHandle, AudioConfig, AudioPlaybackHandle, AudioRuntime};
use voxy_voice_stt::SttEngine;
use voxy_voice_tts::{SentenceChunker, TtsEngine};

pub mod pipecat;
pub use pipecat::{PipecatConfig, PipecatState, PipecatSupervisor, SupervisorError};

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Audio error: {0}")]
    Audio(#[from] voxy_voice_audio::AudioError),
    #[error("STT error: {0}")]
    Stt(#[from] voxy_voice_stt::SttError),
    #[error("TTS error: {0}")]
    Tts(#[from] voxy_voice_tts::TtsError),
    #[error("Pipecat Supervisor error: {0}")]
    Supervisor(#[from] pipecat::SupervisorError),
    #[error("Pipeline not initialized")]
    NotInitialized,
}

/// Pipeline configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VoicePipelineConfig {
    pub audio: AudioConfig,
    pub vad: VadConfig,
    pub pipecat: Option<PipecatConfig>,
    pub auto_start_capture: bool,
    pub voice_speed: f32,
}

impl Default for VoicePipelineConfig {
    fn default() -> Self {
        Self {
            audio: AudioConfig::default(),
            vad: VadConfig::default(),
            pipecat: None,
            auto_start_capture: true,
            voice_speed: 1.0,
        }
    }
}

pub type ResponseHandlerFn =
    Box<dyn Fn(String) -> BoxFuture<'static, Result<String, String>> + Send + Sync>;

/// Complete unified voice pipeline coordinating audio, VAD, STT, LLM router, and TTS.
pub struct VoicePipeline {
    config: VoicePipelineConfig,
    audio_runtime: Arc<AudioRuntime>,
    vad: Arc<Mutex<VadDetector>>,
    stt: Arc<dyn SttEngine>,
    tts: Arc<dyn TtsEngine>,
    state: Arc<Mutex<CompanionState>>,
    active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    capture_handle: Arc<Mutex<Option<AudioCaptureHandle>>>,
    playback_handle: Arc<Mutex<Option<AudioPlaybackHandle>>>,
    response_handler: Arc<Mutex<Option<ResponseHandlerFn>>>,
    ipc_server: Option<Arc<voxy_ipc::VoxyIpcServer>>,
    pipecat_supervisor: Arc<Mutex<Option<Arc<PipecatSupervisor>>>>,
    is_running: Arc<AtomicBool>,
}

impl VoicePipeline {
    pub fn new(
        config: VoicePipelineConfig,
        stt: Arc<dyn SttEngine>,
        tts: Arc<dyn TtsEngine>,
        ipc_server: Option<Arc<voxy_ipc::VoxyIpcServer>>,
    ) -> Result<Self, PipelineError> {
        let audio_runtime = Arc::new(AudioRuntime::new(config.audio.clone())?);
        let vad = Arc::new(Mutex::new(VadDetector::new(config.vad.clone())));

        let pipecat_supervisor = if let Some(ref pcfg) = config.pipecat {
            match PipecatSupervisor::new(pcfg.clone()) {
                Ok(sup) => Some(Arc::new(sup)),
                Err(e) => {
                    tracing::warn!("[VOICE] Pipecat supervisor init deferred: {e}");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            audio_runtime,
            vad,
            stt,
            tts,
            state: Arc::new(Mutex::new(CompanionState::Idle)),
            active_cancellation: Arc::new(Mutex::new(None)),
            capture_handle: Arc::new(Mutex::new(None)),
            playback_handle: Arc::new(Mutex::new(None)),
            response_handler: Arc::new(Mutex::new(None)),
            ipc_server,
            pipecat_supervisor: Arc::new(Mutex::new(pipecat_supervisor)),
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn config(&self) -> &VoicePipelineConfig {
        &self.config
    }

    pub fn set_pipecat_supervisor(&self, supervisor: Arc<PipecatSupervisor>) {
        *self.pipecat_supervisor.lock() = Some(supervisor);
    }

    pub fn pipecat_supervisor(&self) -> Option<Arc<PipecatSupervisor>> {
        self.pipecat_supervisor.lock().clone()
    }

    pub fn start_pipecat(&self) -> Result<(), PipelineError> {
        if let Some(ref sup) = *self.pipecat_supervisor.lock() {
            sup.start()?;
        }
        Ok(())
    }

    pub fn stop_pipecat(&self) -> Result<(), PipelineError> {
        if let Some(ref sup) = *self.pipecat_supervisor.lock() {
            sup.stop()?;
        }
        Ok(())
    }

    pub fn set_response_handler(&self, handler: ResponseHandlerFn) {
        *self.response_handler.lock() = Some(handler);
    }

    pub fn current_state(&self) -> CompanionState {
        *self.state.lock()
    }

    pub fn set_state(&self, new_state: CompanionState, reason: Option<String>) {
        *self.state.lock() = new_state;
        if let Some(ref ipc) = self.ipc_server {
            let ipc_clone = ipc.clone();
            let reason_clone = reason.clone();
            tokio::spawn(async move {
                ipc_clone.set_voice_state(new_state, reason_clone).await;
            });
        }
    }

    /// Immediate barge-in cancellation: stops playback, aborts in-flight generation, and purges buffers.
    pub fn trigger_barge_in(&self, reason: &str) {
        let current = self.current_state();
        if current == CompanionState::Speaking || current == CompanionState::Thinking {
            tracing::info!("[BARGE-IN] Interruption triggered: {}", reason);

            // 1. Abort in-flight LLM/TTS generation
            if let Some(cancel) = self.active_cancellation.lock().take() {
                cancel.cancel();
            }

            // 2. Clear audio playback ring buffer
            if let Some(ref pb) = *self.playback_handle.lock() {
                pb.clear_playback();
            }

            // 3. Update state and notify clients
            self.set_state(CompanionState::Interrupted, Some(reason.to_string()));

            if let Some(ref ipc) = self.ipc_server {
                ipc.broadcast_interrupted(reason);
            }
        }
    }

    pub fn set_mute(&self, muted: bool) {
        if let Some(ref cap) = *self.capture_handle.lock() {
            cap.set_muted(muted);
        }
        let mode = if muted {
            ActivationMode::Muted
        } else {
            ActivationMode::AutoVad
        };
        self.vad.lock().set_mode(mode);
    }

    pub fn set_push_to_talk(&self, active: bool) {
        self.vad.lock().set_push_to_talk(active);
    }

    pub fn set_activation_mode(&self, mode: ActivationMode) {
        self.vad.lock().set_mode(mode);
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn is_initialized(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn is_sleeping(&self) -> bool {
        false
    }

    pub fn playback_buffer_len(&self) -> usize {
        self.playback_handle
            .lock()
            .as_ref()
            .map(|pb| pb.buffer_len())
            .unwrap_or(0)
    }

    pub fn is_cancellation_active(&self) -> bool {
        self.active_cancellation.lock().is_some()
    }

    pub fn set_voice_speed(&self, speed: f32) {
        tracing::debug!("[VOICE] Voice speed set to {}", speed);
    }

    /// Initialize audio streams and start the capture loop.
    pub fn start(self: &Arc<Self>) -> Result<(), PipelineError> {
        match self.audio_runtime.start_playback() {
            Ok(playback) => *self.playback_handle.lock() = Some(playback),
            Err(e) => tracing::warn!("[VOICE] Audio playback initialization warning: {}", e),
        }

        let this = Arc::clone(self);
        match self.audio_runtime.start_capture(move |frame, rms| {
            let t = Arc::clone(&this);
            tokio::spawn(async move {
                t.process_frame(frame, rms).await;
            });
        }) {
            Ok(capture) => *self.capture_handle.lock() = Some(capture),
            Err(e) => tracing::warn!("[VOICE] Audio capture initialization warning: {}", e),
        }

        self.is_running.store(true, Ordering::SeqCst);
        self.set_state(CompanionState::Idle, None);

        Ok(())
    }

    /// Process incoming 16kHz mono audio frame through VAD, triggering STT and LLM on utterance completion.
    pub async fn process_frame(&self, frame: Vec<f32>, rms: f32) {
        if let Some(ref ipc) = self.ipc_server {
            ipc.broadcast_audio_energy(rms);
        }

        let vad_event = self.vad.lock().process_frame(&frame);

        match vad_event {
            Some(VadEvent::WakeWordDetected { keyword, confidence }) => {
                tracing::info!(keyword, confidence, "[VOICE] Wake word detected; activating listening session");
                self.trigger_barge_in("Wake word activation");
                self.set_state(CompanionState::Listening, Some(format!("Wake word: {keyword}")));
            }
            Some(VadEvent::SpeechStarted) => {
                // If user speaks while assistant is speaking or thinking -> BARGE-IN!
                self.trigger_barge_in("User speech onset");
                self.set_state(CompanionState::Listening, None);
            }
            Some(VadEvent::SpeechFrame { .. }) => {
                if self.current_state() != CompanionState::Listening {
                    self.set_state(CompanionState::Listening, None);
                }
            }
            Some(VadEvent::UtteranceComplete { audio, duration_ms: _ }) => {
                self.set_state(CompanionState::Thinking, Some("Transcribing".into()));

                let stt_res = self.stt.transcribe(&audio, None).await;
                match stt_res {
                    Ok(result) => {
                        let text = result.text.trim().to_string();
                        if text.is_empty() {
                            self.set_state(CompanionState::Idle, None);
                            return;
                        }

                        tracing::info!("[VOICE] User utterance recognized: \"{}\"", text);
                        if let Some(ref ipc) = self.ipc_server {
                            let text_clone = text.clone();
                            let ipc_clone = ipc.clone();
                            tokio::spawn(async move {
                                ipc_clone.set_transcript(text_clone, true, 0.95).await;
                            });
                        }

                        self.dispatch_llm_and_tts(text).await;
                    }
                    Err(e) => {
                        tracing::error!("[VOICE] STT transcription failed: {}", e);
                        self.set_state(CompanionState::Error, Some(e.to_string()));
                    }
                }
            }
            None => {}
        }
    }

    /// Dispatch transcript through the OSMOO LLM router and synthesize response to TTS.
    pub async fn dispatch_llm_and_tts(&self, text: String) {
        self.set_state(CompanionState::Thinking, Some("Reasoning".into()));

        let cancel_token = CancellationToken::new();
        *self.active_cancellation.lock() = Some(cancel_token.clone());

        let handler = {
            let guard = self.response_handler.lock();
            guard.as_ref().map(|_| ())
        };

        if handler.is_some() {
            let response_future = {
                let guard = self.response_handler.lock();
                if let Some(ref cb) = *guard {
                    cb(text.clone())
                } else {
                    return;
                }
            };

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("[VOICE] LLM turn cancelled by barge-in");
                    return;
                }
                res = response_future => {
                    match res {
                        Ok(reply_text) => {
                            if cancel_token.is_cancelled() {
                                return;
                            }
                            self.play_tts_stream(&reply_text, cancel_token).await;
                        }
                        Err(err) => {
                            tracing::error!("[VOICE] LLM generation error: {}", err);
                            self.set_state(CompanionState::Error, Some(err));
                        }
                    }
                }
            }
        } else {
            // Default echo fallback if no external router wired
            self.play_tts_stream(&format!("I heard you say: {}", text), cancel_token).await;
        }
    }

    async fn play_tts_stream(&self, reply_text: &str, cancel_token: CancellationToken) {
        self.set_state(CompanionState::Speaking, None);

        let mut chunker = SentenceChunker::new(20);
        let chunks = chunker.push_text(reply_text);

        for chunk in chunks {
            if cancel_token.is_cancelled() {
                break;
            }

            match self.tts.synthesize(&chunk).await {
                Ok(audio_wav) => {
                    if cancel_token.is_cancelled() {
                        break;
                    }
                    // Queue samples to playback handle
                    if let Some(ref pb) = *self.playback_handle.lock() {
                        let samples = decode_wav_to_f32(&audio_wav);
                        pb.queue_samples(&samples);
                    }
                }
                Err(e) => {
                    tracing::error!("[VOICE] TTS synthesis error: {}", e);
                }
            }
        }

        if let Some(remaining) = chunker.flush() {
            if !cancel_token.is_cancelled() {
                if let Ok(audio_wav) = self.tts.synthesize(&remaining).await {
                    if let Some(ref pb) = *self.playback_handle.lock() {
                        let samples = decode_wav_to_f32(&audio_wav);
                        pb.queue_samples(&samples);
                    }
                }
            }
        }

        if !cancel_token.is_cancelled() {
            self.set_state(CompanionState::Idle, None);
        }
    }

    pub fn shutdown(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        let _ = self.stop_pipecat();
        if let Some(cancel) = self.active_cancellation.lock().take() {
            cancel.cancel();
        }
        if let Some(ref pb) = *self.playback_handle.lock() {
            pb.clear_playback();
        }
        if let Some(ref cap) = *self.capture_handle.lock() {
            cap.stop();
        }
        self.set_state(CompanionState::Idle, Some("Shutdown".into()));
    }
}

/// Helper to decode 16-bit PCM WAV bytes into normalized f32 samples.
pub fn decode_wav_to_f32(wav_bytes: &[u8]) -> Vec<f32> {
    if wav_bytes.len() < 44 {
        return Vec::new();
    }
    // Skip 44-byte RIFF header and decode 16-bit little-endian samples
    let data = &wav_bytes[44..];
    data.chunks_exact(2)
        .map(|chunk| {
            let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample_i16 as f32 / 32768.0
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxy_voice_stt::MockSttEngine;
    use voxy_voice_tts::MockTtsEngine;

    #[tokio::test]
    async fn test_pipeline_creation_and_state_flow() {
        let config = VoicePipelineConfig::default();
        let stt = Arc::new(MockSttEngine::new("System check status"));
        let tts = Arc::new(MockTtsEngine::new());

        let pipeline = Arc::new(VoicePipeline::new(config, stt, tts, None).unwrap());
        assert_eq!(pipeline.current_state(), CompanionState::Idle);

        pipeline.start().unwrap();
        assert_eq!(pipeline.current_state(), CompanionState::Idle);

        // Simulate barge-in trigger
        pipeline.set_state(CompanionState::Speaking, None);
        assert_eq!(pipeline.current_state(), CompanionState::Speaking);

        pipeline.trigger_barge_in("User interrupt test");
        assert_eq!(pipeline.current_state(), CompanionState::Interrupted);

        pipeline.shutdown();
        assert_eq!(pipeline.current_state(), CompanionState::Idle);
    }

    #[tokio::test]
    async fn test_wav_decode_to_f32() {
        let original_samples = vec![0.0f32, 0.5, -0.5, 0.25];
        let wav = voxy_voice_stt::encode_wav_16k_mono(&original_samples);

        let decoded = decode_wav_to_f32(&wav);
        assert_eq!(decoded.len(), original_samples.len());
        for (orig, dec) in original_samples.iter().zip(decoded.iter()) {
            assert!((orig - dec).abs() < 1e-3);
        }
    }
}
