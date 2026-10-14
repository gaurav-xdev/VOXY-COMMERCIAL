use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

use voxy_brain::types::{BrainInput, SessionId as BrainSessionId};
use voxy_brain::UnifiedBrainEngine;
use voxy_companion::types::UserPresence;
use voxy_voice::VoicePipeline;

use crate::config::VoiceRuntimeConfig;
use crate::echo::EchoCanceller;
use crate::error::{Result, VoiceRuntimeError};
use crate::streaming::StreamingManager;
use crate::turn::TurnDetector;
use crate::types::*;

pub struct VoiceRuntimeEngine {
    config: VoiceRuntimeConfig,
    state: RwLock<VoiceRuntimeState>,
    voice_pipeline: Arc<VoicePipeline>,
    brain: Arc<UnifiedBrainEngine>,
    turn_detector: Arc<TurnDetector>,
    echo_canceller: Arc<EchoCanceller>,
    streaming: Arc<StreamingManager>,
    shutdown_flag: Arc<AtomicBool>,
    is_initialized: Arc<AtomicBool>,
    session_id: Arc<RwLock<VoiceSessionId>>,
    #[allow(dead_code)]
    current_turn_id: Arc<RwLock<Option<VoiceTurnId>>>,
    last_barge_in: Arc<AtomicU64>,
    latency: Arc<RwLock<LatencyBreakdown>>,
}

impl VoiceRuntimeEngine {
    pub fn new(config: VoiceRuntimeConfig, brain: UnifiedBrainEngine) -> Self {
        let streaming = Arc::new(StreamingManager::new(
            config.streaming.event_buffer_size,
            config.streaming.partial_transcription_interval_ms,
        ));
        let echo_canceller = Arc::new(EchoCanceller::new(
            config.echo_cancellation_enabled,
            config.echo_cancellation_tail_ms,
            config.voice.audio.input.sample_rate,
        ));
        let turn_detector = Arc::new(TurnDetector::new(config.turn_detection.clone()));
        let voice_pipeline = Arc::new(VoicePipeline::new(config.voice.clone()));

        Self {
            config,
            state: RwLock::new(VoiceRuntimeState::Idle),
            voice_pipeline,
            brain: Arc::new(brain),
            turn_detector,
            echo_canceller,
            streaming,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            is_initialized: Arc::new(AtomicBool::new(false)),
            session_id: Arc::new(RwLock::new(VoiceSessionId::new())),
            current_turn_id: Arc::new(RwLock::new(None)),
            last_barge_in: Arc::new(AtomicU64::new(0)),
            latency: Arc::new(RwLock::new(LatencyBreakdown::default())),
        }
    }

    pub async fn init(&self) -> Result<()> {
        if self.is_initialized.load(Ordering::SeqCst) {
            return Err(VoiceRuntimeError::AlreadyInitialized);
        }

        self.voice_pipeline
            .initialize()
            .await
            .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;

        self.brain
            .init()
            .await
            .map_err(|e| VoiceRuntimeError::BrainError(e.to_string()))?;

        let streaming_clone = Arc::clone(&self.streaming);
        self.voice_pipeline
            .on_event(Box::new(move |event| {
                for se in voice_event_to_stream(&event) {
                    streaming_clone.emit(se);
                }
            }))
            .await
            .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;

        let brain_clone = Arc::clone(&self.brain);
        let streaming_clone2 = Arc::clone(&self.streaming);
        let session_id_clone = Arc::clone(&self.session_id);
        self.voice_pipeline
            .set_response_handler(Box::new(move |text| {
                let brain = Arc::clone(&brain_clone);
                let streaming = Arc::clone(&streaming_clone2);
                let session_id_lock = Arc::clone(&session_id_clone);
                Box::pin(async move {
                    let sid = session_id_lock.read().await.clone();
                    match process_text_through_brain(&brain, &text, &sid).await {
                        Ok(output) => output.response_text.unwrap_or_default(),
                        Err(e) => {
                            error!("Brain processing failed: {}", e);
                            streaming.emit(VoiceStreamEvent::Error {
                                message: e.to_string(),
                            });
                            String::new()
                        }
                    }
                })
            }))
            .await;

        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::Idle;
        }
        self.is_initialized.store(true, Ordering::SeqCst);

        info!("VoiceRuntimeEngine initialized");
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::ShuttingDown;
        }
        self.shutdown_flag.store(true, Ordering::SeqCst);
        self.voice_pipeline.interrupt_tts().await;
        self.voice_pipeline.stop_listening().await;
        if self.voice_pipeline.is_running() {
            if let Err(e) = self.voice_pipeline.stop_capture().await {
                warn!("stop_capture during shutdown: {e}");
            }
        }
        self.voice_pipeline
            .shutdown()
            .await
            .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;
        self.brain
            .shutdown()
            .await
            .map_err(|e| VoiceRuntimeError::BrainError(e.to_string()))?;
        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::Shutdown;
        }
        self.is_initialized.store(false, Ordering::SeqCst);
        info!("VoiceRuntimeEngine shutdown complete");
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        if self.voice_pipeline.is_running() {
            return Err(VoiceRuntimeError::AlreadyRunning);
        }
        self.voice_pipeline
            .start_capture()
            .await
            .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;
        self.voice_pipeline
            .with_default_engines()
            .await
            .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;
        self.voice_pipeline
            .start_listening()
            .await
            .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;
        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::Listening;
        }
        info!("VoiceRuntimeEngine started");
        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        self.voice_pipeline.interrupt_tts().await;
        self.voice_pipeline.stop_listening().await;
        if self.voice_pipeline.is_running() {
            self.voice_pipeline
                .stop_capture()
                .await
                .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;
        }
        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::Idle;
        }
        Ok(())
    }

    pub async fn process_audio_frame(
        &self,
        mut audio_data: Vec<f32>,
        sample_rate: u32,
        channels: u8,
    ) -> Result<()> {
        let frame_start = Instant::now();
        {
            let state = self.state.read().await;
            if *state == VoiceRuntimeState::ShuttingDown || *state == VoiceRuntimeState::Shutdown {
                return Err(VoiceRuntimeError::NotRunning);
            }
        }
        self.echo_canceller.process_capture(&mut audio_data);
        let packet = voxy_audio::AudioPacket::new(audio_data, sample_rate, channels);
        self.voice_pipeline
            .process_audio(packet)
            .await
            .map_err(|e| VoiceRuntimeError::CaptureError(e.to_string()))?;
        let total_us = frame_start.elapsed().as_micros() as u64;
        {
            let mut latency = self.latency.write().await;
            latency.total_us = total_us;
        }
        Ok(())
    }

    pub async fn speak(&self, text: &str) -> Result<()> {
        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::Speaking;
        }
        self.streaming.emit(VoiceStreamEvent::SynthesisStarted {
            text: text.to_string(),
        });
        self.voice_pipeline
            .speak(text)
            .await
            .map_err(|e| VoiceRuntimeError::SynthesisError(e.to_string()))?;
        self.streaming
            .emit(VoiceStreamEvent::SynthesisCompleted { duration_ms: 0 });
        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::Listening;
        }
        Ok(())
    }

    pub async fn interrupt(&self) -> Result<()> {
        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        self.last_barge_in.store(now_ms, Ordering::Relaxed);
        self.streaming
            .emit(VoiceStreamEvent::BargeInDetected { tts_playback_ms: 0 });
        self.voice_pipeline.interrupt_tts().await;
        if self.config.barge_in.propagate_to_brain {
            let session_id = self.session_id.read().await.clone();
            let brain_session = BrainSessionId(session_id.0.clone());
            if let Err(e) = self.brain.cancel_turn(&brain_session).await {
                warn!("Failed to propagate barge-in to brain: {e}");
            }
        }
        {
            let mut state = self.state.write().await;
            *state = VoiceRuntimeState::Interrupted;
        }
        Ok(())
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<VoiceStreamEvent> {
        self.streaming.subscribe()
    }

    pub async fn state(&self) -> VoiceRuntimeState {
        self.state.read().await.clone()
    }

    pub async fn session_id(&self) -> VoiceSessionId {
        self.session_id.read().await.clone()
    }

    pub async fn set_session_id(&self, id: VoiceSessionId) {
        *self.session_id.write().await = id;
    }

    pub fn is_running(&self) -> bool {
        self.voice_pipeline.is_running()
    }

    pub fn is_speaking(&self) -> bool {
        self.voice_pipeline.is_speaking()
    }

    pub fn is_in_turn(&self) -> bool {
        self.turn_detector.is_in_turn()
    }

    pub async fn latency(&self) -> LatencyBreakdown {
        self.latency.read().await.clone()
    }

    pub fn event_count(&self) -> u64 {
        self.streaming.event_count()
    }

    pub fn config(&self) -> &VoiceRuntimeConfig {
        &self.config
    }
}

pub fn voice_event_to_stream(event: &voxy_voice::VoiceEvent) -> Vec<VoiceStreamEvent> {
    match event {
        voxy_voice::VoiceEvent::WakeWordDetected { confidence } => {
            vec![VoiceStreamEvent::WakeWordDetected {
                confidence: *confidence,
                latency_ms: 0,
            }]
        }
        voxy_voice::VoiceEvent::VoiceActivityStarted => {
            vec![VoiceStreamEvent::VoiceActivityStarted {
                timestamp_ms: chrono::Utc::now().timestamp_millis() as u64,
            }]
        }
        voxy_voice::VoiceEvent::VoiceActivityEnded { duration_ms } => {
            vec![VoiceStreamEvent::VoiceActivityEnded {
                duration_ms: *duration_ms,
            }]
        }
        voxy_voice::VoiceEvent::TranscriptionResult {
            text,
            is_final,
            confidence,
        } => vec![VoiceStreamEvent::PartialTranscription {
            text: text.clone(),
            confidence: *confidence,
            is_final: *is_final,
        }],
        voxy_voice::VoiceEvent::TranscriptionError { error } => {
            vec![VoiceStreamEvent::Error {
                message: format!("Transcription error: {error}"),
            }]
        }
        voxy_voice::VoiceEvent::SynthesisStarted { text } => {
            vec![VoiceStreamEvent::SynthesisStarted { text: text.clone() }]
        }
        voxy_voice::VoiceEvent::SynthesisCompleted { duration_ms } => {
            vec![VoiceStreamEvent::SynthesisCompleted {
                duration_ms: *duration_ms,
            }]
        }
        voxy_voice::VoiceEvent::SynthesisError { error } => {
            vec![VoiceStreamEvent::Error {
                message: format!("Synthesis error: {error}"),
            }]
        }
        voxy_voice::VoiceEvent::PipelineStateChanged { .. } => vec![],
        voxy_voice::VoiceEvent::Interrupted { .. } => {
            vec![VoiceStreamEvent::BargeInDetected {
                tts_playback_ms: 0,
            }]
        }
    }
}

async fn process_text_through_brain(
    brain: &UnifiedBrainEngine,
    text: &str,
    session_id: &VoiceSessionId,
) -> std::result::Result<voxy_brain::BrainOutput, voxy_brain::BrainError> {
    let brain_input = BrainInput {
        session_id: BrainSessionId(session_id.0.clone()),
        raw_text: text.to_string(),
        user_presence: UserPresence::Active,
        focus_level: 0.7,
        stress_level: 0.3,
        is_meeting: false,
        time_since_last_interaction: Duration::from_secs(5),
        session_duration: Duration::from_secs(300),
        errors_this_session: 0,
        missions_completed: 0,
        missions_failed: 0,
        metadata: Default::default(),
    };
    brain.process_turn(brain_input).await
}
