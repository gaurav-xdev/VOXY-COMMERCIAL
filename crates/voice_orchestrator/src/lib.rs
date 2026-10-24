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
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use voxy_ipc::{CompanionState, DaemonMessage};
use voxy_voice_activation::{ActivationMode, VadConfig, VadDetector, VadEvent, WakeWordConfig};
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
    pub wake_word: Option<WakeWordConfig>,
    pub pipecat: Option<PipecatConfig>,
    pub auto_start_capture: bool,
    pub voice_speed: f32,
}

impl Default for VoicePipelineConfig {
    fn default() -> Self {
        Self {
            audio: AudioConfig::default(),
            vad: VadConfig::default(),
            wake_word: Some(WakeWordConfig::default()),
            pipecat: None,
            auto_start_capture: true,
            voice_speed: 1.0,
        }
    }
}

pub type ResponseHandlerFn =
    Box<dyn Fn(String) -> BoxFuture<'static, Result<String, String>> + Send + Sync>;

/// Streaming response handler: receives the user transcript and a chunk sink.
/// Implementations push partial LLM text into the sink as it is generated so
/// sentence-aware TTS can start before the full response exists.
pub type StreamResponseHandlerFn = Box<
    dyn Fn(String, mpsc::Sender<String>) -> BoxFuture<'static, Result<(), String>> + Send + Sync,
>;

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
    streaming_response_handler: Arc<Mutex<Option<StreamResponseHandlerFn>>>,
    ipc_server: Option<Arc<voxy_ipc::VoxyIpcServer>>,
    pipecat_supervisor: Arc<Mutex<Option<Arc<PipecatSupervisor>>>>,
    is_running: Arc<AtomicBool>,
    /// Monotonically increasing turn ID for race protection and stale event invalidation.
    turn_id: Arc<AtomicU64>,
    /// True while the companion is in sleep mode (only wake-word input accepted).
    sleeping: Arc<AtomicBool>,
    /// Voice speed multiplier in milli-units (1000 = natural 1.0×).
    voice_speed_milli: Arc<AtomicU32>,
    /// Recorded reason the pipeline runs degraded (e.g. playback init failure), if any.
    degraded_reason: Arc<Mutex<Option<String>>>,
}

impl VoicePipeline {
    pub fn new(
        config: VoicePipelineConfig,
        stt: Arc<dyn SttEngine>,
        tts: Arc<dyn TtsEngine>,
        ipc_server: Option<Arc<voxy_ipc::VoxyIpcServer>>,
    ) -> Result<Self, PipelineError> {
        let audio_runtime = Arc::new(AudioRuntime::new(config.audio.clone())?);
        let mut vad_detector = VadDetector::new(config.vad.clone());
        if let Some(ref ww) = config.wake_word {
            vad_detector.set_wake_word_config(ww.clone());
        }
        let vad = Arc::new(Mutex::new(vad_detector));

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

        // Seed the authoritative speed atomic from the declared config.
        let initial_speed_milli = (config.voice_speed * 1000.0) as u32;
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
            streaming_response_handler: Arc::new(Mutex::new(None)),
            ipc_server,
            pipecat_supervisor: Arc::new(Mutex::new(pipecat_supervisor)),
            is_running: Arc::new(AtomicBool::new(false)),
            turn_id: Arc::new(AtomicU64::new(0)),
            sleeping: Arc::new(AtomicBool::new(false)),
            voice_speed_milli: Arc::new(AtomicU32::new(initial_speed_milli)),
            degraded_reason: Arc::new(Mutex::new(None)),
        })
    }

    /// Monotonically increasing current turn ID.
    pub fn current_turn_id(&self) -> u64 {
        self.turn_id.load(Ordering::SeqCst)
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

    /// Register a streaming response handler. When present, it is preferred over
    /// the legacy whole-response handler so TTS can start on first sentence chunks.
    pub fn set_streaming_response_handler(&self, handler: StreamResponseHandlerFn) {
        *self.streaming_response_handler.lock() = Some(handler);
    }

    /// Reason the pipeline is running degraded (e.g. playback device failure), if any.
    pub fn degraded_reason(&self) -> Option<String> {
        self.degraded_reason.lock().clone()
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

            // Invalidate current turn ID monotonically so in-flight tasks ignore completion
            self.turn_id.fetch_add(1, Ordering::SeqCst);

            // 1. Abort in-flight LLM/TTS generation
            if let Some(cancel) = self.active_cancellation.lock().take() {
                cancel.cancel();
            }

            // 2. Clear audio playback ring buffer and update AEC reference
            if let Some(ref pb) = *self.playback_handle.lock() {
                pb.clear_playback();
            }
            if let Some(ref cap) = *self.capture_handle.lock() {
                cap.set_speaker_playing(false);
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

    /// Dynamically enable or update wake-word detection configuration.
    pub fn enable_wake_word(&self, config: WakeWordConfig) {
        self.vad.lock().set_wake_word_config(config);
    }

    /// Disable wake-word detection.
    pub fn disable_wake_word(&self) {
        self.vad.lock().disable_wake_word();
    }

    /// Check if wake-word detection is currently enabled.
    pub fn has_wake_word(&self) -> bool {
        self.vad.lock().has_wake_word()
    }

    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    pub fn is_initialized(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Real sleep-state: true while the companion is in sleep mode and only
    /// wake-word input is accepted (never a hardcoded false).
    pub fn is_sleeping(&self) -> bool {
        self.sleeping.load(Ordering::SeqCst)
    }

    /// Enter/leave sleep mode. In sleep mode the pipeline accepts only wake-word
    /// activation; ordinary speech is not transcribed or answered.
    pub fn set_sleeping(&self, sleeping: bool) {
        tracing::info!(sleeping, "[VOICE] Sleep mode toggled");
        self.sleeping.store(sleeping, Ordering::SeqCst);
        if sleeping {
            self.set_state(CompanionState::Idle, Some("Sleep mode".into()));
        }
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

    /// Apply a voice speed multiplier. Stored as authoritative pipeline state and
    /// forwarded to the TTS engine; reports whether the engine honored it.
    pub fn set_voice_speed(&self, speed: f32) {
        let clamped = speed.clamp(0.5, 3.0);
        self.voice_speed_milli
            .store((clamped * 1000.0) as u32, Ordering::SeqCst);
        let honored = self.tts.set_speed(clamped);
        if honored {
            tracing::info!(speed = clamped, "[VOICE] Voice speed applied to TTS engine");
        } else {
            tracing::warn!(
                speed = clamped,
                engine = %self.tts.name(),
                "[VOICE] TTS engine does not support speed control; stored for capable engines"
            );
        }
    }

    /// Current voice speed multiplier (1.0 = natural).
    pub fn voice_speed(&self) -> f32 {
        self.voice_speed_milli.load(Ordering::SeqCst) as f32 / 1000.0
    }

    /// Initialize audio streams and start the capture loop.
    ///
    /// Failure policy (never fake-running):
    /// - Capture init failure is fatal: the error is propagated and `is_running`
    ///   stays false. The pipeline never reports success without a mic stream.
    /// - Playback init failure is degraded: capture continues, and the reason is
    ///   recorded and observable via `degraded_reason()`.
    ///
    /// The VAD state machine runs directly on the capture thread (cheap, ordered,
    /// no reordering/backlog). Only heavy utterance work (STT → LLM → TTS) is
    /// dispatched to the async runtime, once per completed utterance.
    pub fn start(self: &Arc<Self>) -> Result<(), PipelineError> {
        match self.audio_runtime.start_playback() {
            Ok(playback) => {
                *self.playback_handle.lock() = Some(playback);
                *self.degraded_reason.lock() = None;
            }
            Err(e) => {
                *self.degraded_reason.lock() = Some(format!("playback init failed: {e}"));
                tracing::warn!(
                    "[VOICE] Audio playback init failed (degraded; capture continues): {e}"
                );
            }
        }

        let this = Arc::clone(self);
        let vad = Arc::clone(&self.vad);
        let ipc = self.ipc_server.clone();
        let sleeping = Arc::clone(&self.sleeping);
        // Dispatch handle for utterance-complete events; tolerate being called
        // outside a tokio runtime (e.g. some tests) by dropping events honestly.
        let runtime_handle = tokio::runtime::Handle::try_current().ok();

        if self.config.auto_start_capture {
            let result = self
                .audio_runtime
                .start_capture(move |frame: Vec<f32>, rms: f32| {
                    // Broadcast energy telemetry synchronously (cheap broadcast channel).
                    if let Some(ref ipc) = ipc {
                        ipc.broadcast_audio_energy(rms);
                    }

                    // VAD runs on the audio thread: ordered, bounded, no per-frame spawn.
                    let event = { vad.lock().process_frame(&frame) };
                    let Some(event) = event else { return };

                    // Wake-word and speech events are handled inline (cheap, ordered);
                    // heavy async work is dispatched only per completed utterance.
                    match event {
                        VadEvent::WakeWordDetected {
                            keyword,
                            confidence,
                        } => {
                            if sleeping.load(Ordering::SeqCst) {
                                sleeping.store(false, Ordering::SeqCst);
                                tracing::info!(%keyword, "[VOICE] Wake word in sleep mode; waking");
                            }
                            this.trigger_barge_in("Wake word activation");
                            this.set_state(
                                CompanionState::Listening,
                                Some(format!("Wake word: {keyword}")),
                            );
                            tracing::debug!(%keyword, %confidence, "[VOICE] Wake word path active");
                        }
                        VadEvent::SpeechStarted => {
                            if sleeping.load(Ordering::SeqCst) {
                                return; // sleeping: ignore speech until wake word
                            }
                            // If user speaks while assistant is speaking or thinking -> BARGE-IN
                            this.trigger_barge_in("User speech onset");
                            this.set_state(CompanionState::Listening, None);
                        }
                        VadEvent::SpeechFrame { .. } => {
                            if sleeping.load(Ordering::SeqCst) {
                                return;
                            }
                            if this.current_state() != CompanionState::Listening
                                && this.current_state() != CompanionState::Speaking
                            {
                                this.set_state(CompanionState::Listening, None);
                            }
                        }
                        VadEvent::UtteranceComplete { audio, duration_ms } => {
                            if sleeping.load(Ordering::SeqCst) {
                                tracing::debug!("[VOICE] Utterance ignored while sleeping");
                                return;
                            }
                            match &runtime_handle {
                                Some(handle) => {
                                    let p = this.clone();
                                    handle.spawn(async move {
                                        p.on_utterance_complete(audio, duration_ms).await;
                                    });
                                }
                                None => {
                                    tracing::warn!(
                                    "[VOICE] No async runtime available; utterance dropped ({ms} ms)",
                                    ms = duration_ms
                                );
                                }
                            }
                        }
                    }
                });

            let capture = result.map_err(|e| {
                tracing::error!(
                    "[VOICE] Audio capture initialization failed: {e} — pipeline NOT started"
                );
                PipelineError::Audio(e)
            })?;
            *self.capture_handle.lock() = Some(capture);
        } else {
            tracing::info!("[VOICE] auto_start_capture is false; capture stream deferred");
        }

        self.is_running.store(true, Ordering::SeqCst);
        self.set_state(CompanionState::Idle, None);

        Ok(())
    }

    /// Handle a completed user utterance: STT → transcript event → LLM/TTS dispatch.
    async fn on_utterance_complete(&self, audio: Vec<f32>, duration_ms: u64) {
        self.set_state(CompanionState::Thinking, Some("Transcribing".into()));

        if audio.is_empty() {
            self.set_state(CompanionState::Idle, None);
            return;
        }

        let stt_res = self.stt.transcribe(&audio, None).await;
        match stt_res {
            Ok(result) => {
                let text = result.text.trim().to_string();
                if text.is_empty() {
                    self.set_state(CompanionState::Idle, None);
                    return;
                }

                tracing::info!(
                    utt_ms = duration_ms,
                    stt_engine = %self.stt.name(),
                    "[VOICE] User utterance recognized"
                );
                if let Some(ref ipc) = self.ipc_server {
                    ipc.set_transcript(text.clone(), true, result.confidence)
                        .await;
                }

                self.dispatch_llm_and_tts(text).await;
            }
            Err(e) => {
                tracing::error!("[VOICE] STT transcription failed: {e}");
                self.set_state(CompanionState::Error, Some(e.to_string()));
                if let Some(ref ipc) = self.ipc_server {
                    ipc.broadcast(DaemonMessage::ErrorNotification {
                        code: "STT_ERROR".into(),
                        message: e.to_string(),
                    });
                }
            }
        }
    }

    /// Process incoming 16kHz mono audio frame through VAD (public API retained
    /// for direct callers/tests). VAD events are handled by the same logic the
    /// capture-thread path uses; utterance completion runs through
    /// [`on_utterance_complete`](Self::on_utterance_complete).
    pub async fn process_frame(&self, frame: Vec<f32>, rms: f32) {
        if let Some(ref ipc) = self.ipc_server {
            ipc.broadcast_audio_energy(rms);
        }

        let vad_event = self.vad.lock().process_frame(&frame);

        match vad_event {
            Some(VadEvent::WakeWordDetected {
                keyword,
                confidence,
            }) => {
                if self.is_sleeping() {
                    self.set_sleeping(false);
                }
                tracing::info!(
                    keyword,
                    confidence,
                    "[VOICE] Wake word detected; activating listening session"
                );
                self.trigger_barge_in("Wake word activation");
                self.set_state(
                    CompanionState::Listening,
                    Some(format!("Wake word: {keyword}")),
                );
            }
            Some(VadEvent::SpeechStarted) => {
                // If user speaks while assistant is speaking or thinking -> BARGE-IN!
                self.trigger_barge_in("User speech onset");
                self.set_state(CompanionState::Listening, None);
            }
            Some(VadEvent::SpeechFrame { rms: _ }) => {
                if self.current_state() != CompanionState::Listening {
                    self.set_state(CompanionState::Listening, None);
                }
            }
            Some(VadEvent::UtteranceComplete { audio, duration_ms }) => {
                self.on_utterance_complete(audio, duration_ms).await;
            }
            None => {}
        }
    }

    /// Dispatch transcript through the OSMOO LLM router.
    ///
    /// Preferred path (real streaming): a registered streaming handler receives a
    /// chunk sink and pushes partial LLM text; sentence-aware TTS begins as soon
    /// as the first coherent chunk is available instead of waiting for the full
    /// response. Legacy whole-response handler is used as fallback.
    pub async fn dispatch_llm_and_tts(&self, text: String) {
        let turn = self.turn_id.fetch_add(1, Ordering::SeqCst) + 1;
        self.set_state(CompanionState::Thinking, Some("Reasoning".into()));

        let cancel_token = CancellationToken::new();
        *self.active_cancellation.lock() = Some(cancel_token.clone());

        let streaming_dispatch = {
            let guard = self.streaming_response_handler.lock();
            guard.as_ref().map(|h| {
                let (tx, rx) = mpsc::channel::<String>(64);
                let fut = h(text.clone(), tx);
                (fut, rx)
            })
        };

        if let Some((handler_fut, chunk_rx)) = streaming_dispatch {
            self.run_streaming_response(handler_fut, chunk_rx, cancel_token.clone())
                .await;
            return;
        }

        let legacy_handler_present = { self.response_handler.lock().is_some() };

        if legacy_handler_present {
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
                }
                res = response_future => {
                    // Stale check: if a new turn occurred, ignore old response
                    if self.turn_id.load(Ordering::SeqCst) != turn {
                        tracing::info!("[VOICE] Discarding stale LLM response from prior turn {turn}");
                        return;
                    }
                    match res {
                        Ok(reply_text) => {
                            if !cancel_token.is_cancelled() {
                                self.play_tts_stream(&reply_text, cancel_token).await;
                            }
                        }
                        Err(err) => {
                            tracing::error!("[VOICE] LLM generation error: {}", err);
                            self.set_state(CompanionState::Error, Some(err));
                        }
                    }
                }
            }
        } else {
            // Honest behavior when no router is wired: report the state instead of
            // echoing a fabricated acknowledgment as if it were a real reply.
            tracing::warn!("[VOICE] No response handler wired; skipping LLM/TTS dispatch");
            self.set_state(
                CompanionState::Error,
                Some("No conversation handler is wired".into()),
            );
        }
    }

    /// Run the streaming handler and feed sentence chunks to TTS as they arrive.
    async fn run_streaming_response(
        &self,
        fut: BoxFuture<'static, Result<(), String>>,
        mut chunk_rx: mpsc::Receiver<String>,
        cancel_token: CancellationToken,
    ) {
        self.set_state(CompanionState::Thinking, Some("Reasoning".into()));

        let speak_cancel = cancel_token.clone();
        let mut chunker = SentenceChunker::new(20);
        let speak = async move {
            loop {
                tokio::select! {
                    _ = speak_cancel.cancelled() => {
                        tracing::info!("[VOICE] Streaming response cancelled by barge-in");
                        break;
                    }
                    item = chunk_rx.recv() => {
                        match item {
                            None => break,
                            Some(chunk) => {
                                if speak_cancel.is_cancelled() {
                                    break;
                                }
                                if self.current_state() != CompanionState::Speaking {
                                    self.set_state(CompanionState::Speaking, None);
                                }
                                for s in chunker.push_text(&chunk) {
                                    self.synthesize_and_queue(&s, &speak_cancel).await;
                                }
                            }
                        }
                    }
                }
            }
            if !speak_cancel.is_cancelled() {
                if let Some(remaining) = chunker.flush() {
                    self.synthesize_and_queue(&remaining, &speak_cancel).await;
                }
            }
        };

        let gen_cancel = cancel_token.clone();
        let generate = async {
            tokio::select! {
                _ = gen_cancel.cancelled() => {
                    tracing::info!("[VOICE] Streaming LLM generation future aborted by barge-in cancellation");
                }
                res = fut => {
                    if let Err(err) = res {
                        tracing::error!("[VOICE] Streaming LLM generation error: {err}");
                        if !gen_cancel.is_cancelled() {
                            self.set_state(CompanionState::Error, Some(err));
                        }
                    }
                }
            }
        };

        tokio::join!(speak, generate);

        if let Some(ref cap) = *self.capture_handle.lock() {
            cap.set_speaker_playing(false);
        }

        if !cancel_token.is_cancelled() {
            self.set_state(CompanionState::Idle, None);
        }
    }

    /// Synthesize one sentence chunk and queue it to the playback device,
    /// resampling to the device's actual output rate.
    async fn synthesize_and_queue(&self, chunk: &str, cancel_token: &CancellationToken) {
        if cancel_token.is_cancelled() {
            return;
        }
        match self.tts.synthesize(chunk).await {
            Ok(audio_wav) => {
                if cancel_token.is_cancelled() {
                    return;
                }
                if let Some(ref pb) = *self.playback_handle.lock() {
                    match decode_wav_checked(&audio_wav) {
                        Ok(wav_audio) => {
                            let out_rate = pb.sample_rate();
                            let samples = if wav_audio.sample_rate == out_rate {
                                wav_audio.samples_mono
                            } else {
                                voxy_voice_audio::normalize_to_mono_16k(
                                    &wav_audio.samples_mono,
                                    wav_audio.sample_rate,
                                    1,
                                    out_rate,
                                )
                            };
                            if let Some(ref cap) = *self.capture_handle.lock() {
                                cap.set_speaker_playing(true);
                            }
                            pb.queue_samples(&samples);
                        }
                        Err(e) => {
                            tracing::error!("[VOICE] TTS WAV decode failed: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("[VOICE] TTS synthesis error: {e}");
            }
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
            self.synthesize_and_queue(&chunk, &cancel_token).await;
        }

        if let Some(remaining) = chunker.flush() {
            if !cancel_token.is_cancelled() {
                self.synthesize_and_queue(&remaining, &cancel_token).await;
            }
        }

        if let Some(ref cap) = *self.capture_handle.lock() {
            cap.set_speaker_playing(false);
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

/// Parsed WAV content: format metadata plus mono mixdown samples.
#[derive(Debug, Clone, PartialEq)]
pub struct WavAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub samples_mono: Vec<f32>,
}

/// Parse a RIFF/WAVE byte buffer without assuming a fixed header size.
///
/// Walks the actual chunk list (supports fmt sizes 16/18/40, various channel
/// counts and rates) and decodes PCM 8/16/24/32-bit and IEEE float32. Fails with
/// a real error on truncated/unsupported files instead of guessing offsets.
pub fn parse_wav_bytes(wav_bytes: &[u8]) -> Result<WavAudio, String> {
    if wav_bytes.len() < 12 {
        return Err("WAV too small to contain a RIFF header".to_string());
    }
    if &wav_bytes[0..4] != b"RIFF" || &wav_bytes[8..12] != b"WAVE" {
        return Err("not a RIFF/WAVE file".to_string());
    }

    let mut offset = 12usize;
    let mut audio_format: Option<u16> = None;
    let mut channels: Option<u16> = None;
    let mut sample_rate: Option<u32> = None;
    let mut bits_per_sample: Option<u16> = None;
    let mut data: Option<&[u8]> = None;

    while offset + 8 <= wav_bytes.len() {
        let chunk_id = &wav_bytes[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            wav_bytes[offset + 4],
            wav_bytes[offset + 5],
            wav_bytes[offset + 6],
            wav_bytes[offset + 7],
        ]) as usize;
        let chunk_data_end = offset + 8 + chunk_size;
        if chunk_data_end > wav_bytes.len() {
            return Err(format!(
                "WAV chunk '{:?}' truncated (claims {} bytes at offset {})",
                chunk_id, chunk_size, offset
            ));
        }
        let chunk_data = &wav_bytes[offset + 8..chunk_data_end];

        match chunk_id {
            b"fmt " => {
                if chunk_data.len() < 16 {
                    return Err("fmt chunk smaller than 16 bytes".to_string());
                }
                audio_format = Some(u16::from_le_bytes([chunk_data[0], chunk_data[1]]));
                channels = Some(u16::from_le_bytes([chunk_data[2], chunk_data[3]]));
                sample_rate = Some(u32::from_le_bytes([
                    chunk_data[4],
                    chunk_data[5],
                    chunk_data[6],
                    chunk_data[7],
                ]));
                bits_per_sample = Some(u16::from_le_bytes([chunk_data[14], chunk_data[15]]));
            }
            b"data" => {
                data = Some(chunk_data);
            }
            _ => {
                // Unknown chunks (LIST, fact, bext, etc.) are skipped.
            }
        }

        // Chunks are word-aligned.
        offset = chunk_data_end + (chunk_size & 1);
    }

    let (audio_format, channels, sample_rate, bits, data) =
        match (audio_format, channels, sample_rate, bits_per_sample, data) {
            (Some(f), Some(c), Some(sr), Some(b), Some(d)) => (f, c, sr, b, d),
            _ => return Err("WAV missing fmt or data chunk".to_string()),
        };

    if channels == 0 {
        return Err("WAV declares zero channels".to_string());
    }

    let mut mono: Vec<f32> = Vec::with_capacity(data.len() / 4);
    match audio_format {
        1 => match bits {
            8 => {
                for chunk in data.chunks_exact(channels as usize) {
                    let frame: f32 = chunk
                        .iter()
                        .map(|&s| (s as f32 - 128.0) / 128.0)
                        .sum::<f32>()
                        / channels as f32;
                    mono.push(frame);
                }
            }
            16 => {
                let bytes_per_frame = channels as usize * 2;
                for frame_bytes in data.chunks_exact(bytes_per_frame) {
                    let frame: f32 = frame_bytes
                        .chunks_exact(2)
                        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                        .sum::<f32>()
                        / channels as f32;
                    mono.push(frame);
                }
            }
            24 => {
                let bytes_per_frame = channels as usize * 3;
                for frame_bytes in data.chunks_exact(bytes_per_frame) {
                    let frame: f32 = frame_bytes
                        .chunks_exact(3)
                        .map(|b| {
                            let v = (b[0] as i32) | ((b[1] as i32) << 8) | ((b[2] as i32) << 16);
                            let v = if v & 0x80_0000 != 0 {
                                v | !0xFF_FFFF
                            } else {
                                v
                            };
                            v as f32 / 8388608.0
                        })
                        .sum::<f32>()
                        / channels as f32;
                    mono.push(frame);
                }
            }
            32 => {
                let bytes_per_frame = channels as usize * 4;
                for frame_bytes in data.chunks_exact(bytes_per_frame) {
                    let frame: f32 = frame_bytes
                        .chunks_exact(4)
                        .map(|b| {
                            let v = i32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                            v as f32 / i32::MAX as f32
                        })
                        .sum::<f32>()
                        / channels as f32;
                    mono.push(frame);
                }
            }
            other => return Err(format!("unsupported PCM bit depth: {other}")),
        },
        3 => match bits {
            32 => {
                let bytes_per_frame = channels as usize * 4;
                for frame_bytes in data.chunks_exact(bytes_per_frame) {
                    let frame: f32 = frame_bytes
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .sum::<f32>()
                        / channels as f32;
                    mono.push(frame);
                }
            }
            other => return Err(format!("unsupported IEEE float bit depth: {other}")),
        },
        other => return Err(format!("unsupported WAV audio format tag: {other}")),
    }

    Ok(WavAudio {
        sample_rate,
        channels,
        bits_per_sample: bits,
        samples_mono: mono,
    })
}

/// Decode a WAV to mono f32 samples with real format awareness.
/// Returns the parsed metadata alongside the samples.
pub fn decode_wav_checked(wav_bytes: &[u8]) -> Result<WavAudio, String> {
    parse_wav_bytes(wav_bytes)
}

/// Helper to decode WAV bytes into normalized mono f32 samples.
/// Kept for compatibility; use [`decode_wav_checked`] for real error handling.
pub fn decode_wav_to_f32(wav_bytes: &[u8]) -> Vec<f32> {
    match parse_wav_bytes(wav_bytes) {
        Ok(audio) => audio.samples_mono,
        Err(e) => {
            tracing::warn!(error = %e, "[VOICE] WAV decode failed; returning no samples");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxy_voice_stt::MockSttEngine;
    use voxy_voice_tts::MockTtsEngine;

    #[tokio::test]
    async fn test_pipeline_creation_and_state_flow() {
        // 1. In production mode (allow_virtual_fallback = false), start() correctly fails closed if mic access is denied
        let mut prod_config = VoicePipelineConfig::default();
        prod_config.audio.allow_virtual_fallback = false;
        let stt = Arc::new(MockSttEngine::new("System check status"));
        let tts = Arc::new(MockTtsEngine::new());

        let prod_pipeline =
            Arc::new(VoicePipeline::new(prod_config, stt.clone(), tts.clone(), None).unwrap());
        let start_res = prod_pipeline.start();
        // If mic access is denied or not present, it must return PipelineError::Audio(InputStreamError)
        // If mic is available, start_res is Ok(())
        match start_res {
            Ok(_) => {
                assert!(prod_pipeline.is_running.load(Ordering::SeqCst));
                prod_pipeline.shutdown();
            }
            Err(PipelineError::Audio(voxy_voice_audio::AudioError::InputStreamError(_))) => {
                // Expected fail-closed behavior on systems with no mic permission!
            }
            Err(e) => panic!("Unexpected error: {e}"),
        }

        // 2. In simulation/test mode (allow_virtual_fallback = true), pipeline starts cleanly without physical mic
        let mut sim_config = VoicePipelineConfig::default();
        sim_config.audio.allow_virtual_fallback = true;
        let sim_pipeline = Arc::new(VoicePipeline::new(sim_config, stt, tts, None).unwrap());
        assert_eq!(sim_pipeline.current_state(), CompanionState::Idle);

        sim_pipeline.start().unwrap();
        assert_eq!(sim_pipeline.current_state(), CompanionState::Idle);
        assert_eq!(sim_pipeline.current_turn_id(), 0);

        // Simulate barge-in trigger and turn invalidation
        sim_pipeline.set_state(CompanionState::Speaking, None);
        assert_eq!(sim_pipeline.current_state(), CompanionState::Speaking);

        sim_pipeline.trigger_barge_in("User interrupt test");
        assert_eq!(sim_pipeline.current_state(), CompanionState::Interrupted);
        assert_eq!(sim_pipeline.current_turn_id(), 1);

        sim_pipeline.shutdown();
        assert_eq!(sim_pipeline.current_state(), CompanionState::Idle);
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

    #[tokio::test]
    async fn test_streaming_response_aborts_hanging_future_on_barge_in() {
        let stt = Arc::new(MockSttEngine::new("test"));
        let tts = Arc::new(MockTtsEngine::new());
        let mut sim_config = VoicePipelineConfig::default();
        sim_config.audio.allow_virtual_fallback = true;
        let pipeline = Arc::new(VoicePipeline::new(sim_config, stt, tts, None).unwrap());

        let (_tx, rx) = mpsc::channel(16);
        let cancel_token = CancellationToken::new();

        // Future that hangs indefinitely unless dropped/cancelled
        let hanging_fut: BoxFuture<'static, Result<(), String>> = Box::pin(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Ok(())
        });

        let cancel_clone = cancel_token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            cancel_clone.cancel();
        });

        let start = std::time::Instant::now();
        pipeline
            .run_streaming_response(hanging_fut, rx, cancel_token)
            .await;
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_millis(1500),
            "Streaming response must abort promptly on cancellation, took {elapsed:?}"
        );
    }
}
