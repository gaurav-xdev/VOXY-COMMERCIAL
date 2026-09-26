use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
#[cfg(feature = "whisper-engine")]
use tracing::info;
use tracing::warn;
use voxy_voice_orchestrator::{AudioChunk, AudioStream, SttEngine};

#[cfg(feature = "sherpa-engine")]
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig, OnlineStream, OnlineTransducerModelConfig, OnlineModelConfig};

pub use voxy_provider_core as core_traits;

/// Number of compute threads for a single whisper inference.
///
/// Defaults to 8 (measured ~1.3s STT vs ~2.0s at 4 threads on the validation
/// hardware) while keeping peak whisper compute bounded so the async runtime
/// is not starved on shared cores. Tunable via `VOXY_WHISPER_THREADS` for
/// hardware-specific tuning (e.g. small/low-power machines may prefer 4);
/// clamped to available cores and to the whisper-rs accepted range.
pub fn whisper_threads() -> i32 {
    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let requested = std::env::var("VOXY_WHISPER_THREADS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(8);
    // Minimum 2 threads: n_threads=1 triggers a whisper.cpp hang
    // (infinite seek loop in decoder). Verified in Phase 26 testing.
    requested.max(2).min(available.max(2)).min(16) as i32
}

#[derive(Debug, thiserror::Error)]
pub enum WhisperError {
    #[error("Model not loaded")]
    ModelNotLoaded,
    #[error("Model file not found: {0}")]
    ModelNotFound(String),
    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),
    #[error("Audio error: {0}")]
    AudioError(String),
}

pub struct WhisperSttEngine {
    name: String,
    model_path: Option<PathBuf>,
    #[cfg(feature = "whisper-engine")]
    context: Arc<Mutex<Option<whisper_rs::WhisperContext>>>,
    #[cfg(feature = "whisper-engine")]
    cached_state: Arc<Mutex<Option<whisper_rs::WhisperState>>>,
    buffer: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u8,
    max_buffer_seconds: f64,
    last_text: Arc<Mutex<String>>,
    is_loaded: AtomicBool,
    language: String,
    translate: bool,
    #[cfg(feature = "whisper-engine")]
    no_timestamps: bool,
}

impl WhisperSttEngine {
    pub fn new() -> Self {
        Self {
            name: "whisper-stt".into(),
            model_path: None,
            #[cfg(feature = "whisper-engine")]
            context: Arc::new(Mutex::new(None)),
            #[cfg(feature = "whisper-engine")]
            cached_state: Arc::new(Mutex::new(None)),
            buffer: Arc::new(Mutex::new(Vec::new())),
            sample_rate: 16000,
            channels: 1,
            max_buffer_seconds: 30.0,
            last_text: Arc::new(Mutex::new(String::new())),
            is_loaded: AtomicBool::new(false),
            language: "en".to_string(),
            translate: false,
            #[cfg(feature = "whisper-engine")]
            no_timestamps: true,
        }
    }

    pub fn with_model_path(mut self, path: PathBuf) -> Self {
        self.model_path = Some(path);
        self
    }

    pub fn with_language(mut self, lang: &str) -> Self {
        self.language = lang.to_string();
        self
    }

    pub fn with_translate(mut self, translate: bool) -> Self {
        self.translate = translate;
        self
    }

    #[cfg(feature = "whisper-engine")]
    pub fn load_model(&self) -> Result<(), WhisperError> {
        let path = self
            .model_path
            .as_ref()
            .ok_or(WhisperError::ModelNotLoaded)?;

        if !path.exists() {
            return Err(WhisperError::ModelNotFound(path.display().to_string()));
        }

        let ctx = whisper_rs::WhisperContext::new_with_params(
            path.to_str()
                .ok_or_else(|| WhisperError::ModelNotFound("Invalid model path".to_string()))?,
            whisper_rs::WhisperContextParameters::default(),
        )
        .map_err(|e| WhisperError::TranscriptionFailed(e.to_string()))?;

        let state = ctx
            .create_state()
            .map_err(|e| WhisperError::TranscriptionFailed(e.to_string()))?;

        *self.context.lock() = Some(ctx);
        *self.cached_state.lock() = Some(state);
        self.is_loaded.store(true, Ordering::SeqCst);

        info!("Whisper model loaded from {}", path.display());
        Ok(())
    }

    #[cfg(not(feature = "whisper-engine"))]
    pub fn load_model(&self) -> Result<(), WhisperError> {
        let path = self
            .model_path
            .as_ref()
            .ok_or(WhisperError::ModelNotLoaded)?;

        if !path.exists() {
            return Err(WhisperError::ModelNotFound(path.display().to_string()));
        }

        warn!(
            "whisper-engine feature disabled; model at {} not loaded",
            path.display()
        );
        Ok(())
    }

    pub fn is_model_loaded(&self) -> bool {
        self.is_loaded.load(Ordering::SeqCst)
    }

    /// Runs one inference over a short silence buffer so the first real
    /// utterance does not pay the one-time model warm-up cost (allocations,
    /// thread-pool bring-up). Mirrors the reference "noise warm" pattern
    /// (e.g. isair/jarvis). Best-effort: failures are logged, not fatal.
    pub fn warmup(&self) -> Result<(), WhisperError> {
        #[cfg(feature = "whisper-engine")]
        {
            let audio: Vec<f32> = vec![0.0; 16000 / 2]; // 0.5 s of silence
            let start = std::time::Instant::now();
            let text = self.transcribe_audio(&audio, 16000)?;
            if !text.is_empty() {
                tracing::debug!("Whisper warm-up produced text on silence: {text:?}");
            }
            info!("Whisper warm-up inference complete in {:?}", start.elapsed());
        }
        Ok(())
    }

    pub fn clear_buffer(&self) {
        self.buffer.lock().clear();
        self.last_text.lock().clear();
    }

    pub fn buffered_duration_ms(&self) -> f64 {
        let buf = self.buffer.lock();
        if self.sample_rate == 0 || self.channels == 0 {
            return 0.0;
        }
        (buf.len() as f64 / (self.sample_rate as f64 * self.channels as f64)) * 1000.0
    }

    fn max_buffer_samples(&self) -> usize {
        (self.sample_rate as f64 * self.max_buffer_seconds) as usize * self.channels as usize
    }

    #[cfg(feature = "whisper-engine")]
    fn transcribe_audio(&self, audio: &[f32], _sample_rate: u32) -> Result<String, WhisperError> {
        let mut state_guard = self.cached_state.lock();
        let state = state_guard.as_mut().ok_or(WhisperError::ModelNotLoaded)?;

        let mut params =
            whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });

        let n_threads = crate::whisper_threads();
        params.set_n_threads(n_threads);
        params.set_translate(self.translate);
        params.set_no_timestamps(self.no_timestamps);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);

        state
            .full(params, audio)
            .map_err(|e| WhisperError::TranscriptionFailed(e.to_string()))?;

        let num_segments = state.full_n_segments();

        let mut text = String::new();
        for i in 0..num_segments {
            if let Some(segment) = state.get_segment(i) {
                if let Ok(segment_text) = segment.to_str_lossy() {
                    text.push_str(&segment_text);
                }
            }
        }

        Ok(text.trim().to_string())
    }

    #[cfg(not(feature = "whisper-engine"))]
    #[allow(dead_code)]
    fn transcribe_audio(&self, _audio: &[f32], _sample_rate: u32) -> Result<String, WhisperError> {
        Ok(String::new())
    }

    #[cfg(feature = "whisper-engine")]
    async fn transcribe_audio_async(
        &self,
        audio: Vec<f32>,
        _sample_rate: u32,
    ) -> Result<String, WhisperError> {
        let cached_state = self.cached_state.clone();
        let translate = self.translate;
        let no_timestamps = self.no_timestamps;

        tokio::task::spawn_blocking(move || {
            let mut guard = cached_state.lock();
            let state = guard.as_mut().ok_or(WhisperError::ModelNotLoaded)?;

            let mut params =
                whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
            let n_threads = crate::whisper_threads();
            params.set_n_threads(n_threads);
            params.set_translate(translate);
            params.set_no_timestamps(no_timestamps);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_nst(true);

            state
                .full(params, &audio)
                .map_err(|e| WhisperError::TranscriptionFailed(e.to_string()))?;

            let num_segments = state.full_n_segments();

            let mut text = String::new();
            for i in 0..num_segments {
                if let Some(segment) = state.get_segment(i) {
                    if let Ok(segment_text) = segment.to_str_lossy() {
                        text.push_str(&segment_text);
}

#[cfg(feature = "sherpa-engine")]
pub struct SherpaStreamingSttEngine {
    name: String,
    model_path: Option<PathBuf>,
    recognizer: Option<OnlineRecognizer>,
    sample_rate: u32,
    channels: u8,
    is_loaded: AtomicBool,
}

#[cfg(feature = "sherpa-engine")]
pub use SherpaStreamingSttEngine;

#[cfg(feature = "sherpa-engine")]
pub use SherpaError;

#[cfg(feature = "sherpa-engine")]
impl SherpaStreamingSttEngine {
    pub fn new() -> Self {
        Self {
            name: "sherpa-streaming-stt".into(),
            model_path: None,
            recognizer: None,
            sample_rate: 16000,
            channels: 1,
            is_loaded: AtomicBool::new(false),
        }
    }

    pub fn with_model_path(mut self, path: PathBuf) -> Self {
        self.model_path = Some(path);
        self
    }

    pub fn load_model(&mut self) -> Result<(), SherpaError> {
        let model_dir = self.model_path.as_ref().ok_or(SherpaError::ModelNotLoaded)?;

        let encoder = model_dir.join("encoder-epoch-99-avg-1-chunk-16-left-128.int8.onnx");
        let decoder = model_dir.join("decoder-epoch-99-avg-1-chunk-16-left-128.int8.onnx");
        let joiner = model_dir.join("joiner-epoch-99-avg-1-chunk-128.int8.onnx");
        let tokens = model_dir.join("tokens.txt");

        for path in [&encoder, &decoder, &joiner] {
            if !path.exists() {
                return Err(SherpaError::ModelNotFound(path.display().to_string()));
            }
        }

        let mut config = OnlineRecognizerConfig::default();
        config.model_config.transducer.encoder = Some(encoder.to_string_lossy().to_string());
        config.model_config.transducer.decoder = Some(decoder.to_string_lossy().to_string());
        config.model_config.transducer.joiner = Some(joiner.to_string_lossy().to_string());
        config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
        config.model_config.num_threads = whisper_threads();
        config.enable_endpoint = true;
        config.decoding_method = Some("greedy_search".to_string());

        let recognizer = OnlineRecognizer::create(&config)
            .ok_or_else(|| SherpaError::TranscriptionFailed("Failed to create OnlineRecognizer".into()))?;

        self.recognizer = Some(recognizer);
        self.is_loaded.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SherpaError {
    #[error("Model not loaded")]
    ModelNotLoaded,
    #[error("Model file not found: {0}")]
    ModelNotFound(String),
    #[error("Transcription failed: {0}")]
    TranscriptionFailed(String),
}

#[cfg(feature = "sherpa-engine")]
#[async_trait]
impl SttEngine for SherpaStreamingSttEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn transcribe(
        &self,
        audio: &AudioChunk,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        if audio.data.is_empty() || !self.is_model_loaded() {
            return Ok(String::new());
        }

        let recognizer = self.recognizer.as_ref().ok_or_else(|| {
            voxy_voice_orchestrator::VoiceOrchestratorError::PipelineError("Sherpa recognizer not initialized".into())
        })?;

        let normalized = normalize_to_engine(&audio.data, audio.sample_rate, audio.channels, self.sample_rate);
        let mut stream = recognizer.create_stream();

        stream.accept_waveform(self.sample_rate as i32, &normalized);
        if audio.is_final {
            stream.input_finished();
        }

        while recognizer.is_ready(&mut stream) {
            recognizer.decode(&mut stream);
        }

        let result = recognizer.get_result(&mut stream).ok_or_else(|| {
            voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed("No result from Sherpa recognizer".into())
        })?;

        Ok(result.text.trim().to_string())
    }

    async fn transcribe_stream(
        &self,
        mut stream: Box<dyn AudioStream>,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        let recognizer = self.recognizer.as_ref().ok_or_else(|| {
            voxy_voice_orchestrator::VoiceOrchestratorError::PipelineError("Sherpa recognizer not initialized".into())
        })?;

        let mut stream_obj = recognizer.create_stream();
        let mut accumulated = Vec::new();
        let sr = stream.sample_rate();

        while let Some(chunk) = stream.next_chunk().await {
            accumulated.extend_from_slice(&chunk.data);
        }

        let normalized = normalize_to_engine(&accumulated, sr, 1, self.sample_rate);
        let mut stream_obj = recognizer.create_stream();
        stream_obj.accept_waveform(self.sample_rate as i32, &normalized);
        stream_obj.input_finished();

        while recognizer.is_ready(&mut stream_obj) {
            recognizer.decode(&mut stream_obj);
        }

        let result = recognizer.get_result(&mut stream_obj).ok_or_else(|| {
            voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed("No result from Sherpa recognizer".into())
        })?;

        Ok(result.text.trim().to_string())
    }

    async fn transcribe_partial(
        &self,
        audio: &AudioChunk,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        if audio.data.is_empty() || !self.is_model_loaded() {
            return Ok(String::new());
        }
        let rms = compute_rms(&audio.data);
        if rms < 0.01 {
            return Ok(String::new());
        }
        let normalized = normalize_to_engine(&audio.data, audio.sample_rate, audio.channels, self.sample_rate);
        let recognizer = self.recognizer.as_ref().ok_or_else(|| {
            voxy_voice_orchestrator::VoiceOrchestratorError::PipelineError("Sherpa recognizer not initialized".into())
        })?;
        let mut stream = recognizer.create_stream();
        stream.accept_waveform(self.sample_rate as i32, &normalize_to_engine(&audio.data, audio.sample_rate, audio.channels, self.sample_rate));
        while recognizer.is_ready(&mut stream) {
            recognizer.decode(&mut stream);
        }
        let result = recognizer.get_result(&mut stream).ok_or_else(|| {
            voxy_voice_orchestrator::VoiceOrchestratorError::TranscriptionFailed("No result from Sherpa recognizer".into())
        })?;
        Ok(result.text.trim().to_string())
    }

    fn supports_partial_transcription(&self) -> bool {
        true
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["en".to_string()]
    }

    fn is_available(&self) -> bool {
        self.is_model_loaded()
    }
}

impl SherpaStreamingSttEngine {
    fn is_model_loaded(&self) -> bool {
        self.is_loaded.load(Ordering::SeqCst)
    }
}
                }
            }

            Ok(text.trim().to_string())
        })
        .await
        .map_err(|e| WhisperError::TranscriptionFailed(format!("Task join error: {e}")))?
    }

    #[cfg(not(feature = "whisper-engine"))]
    async fn transcribe_audio_async(
        &self,
        _audio: Vec<f32>,
        _sample_rate: u32,
    ) -> Result<String, WhisperError> {
        Ok(String::new())
    }

#[cfg(feature = "whisper-engine")]
    async fn decode_partial(&self, audio: Vec<f32>) -> Result<String, WhisperError> {
        let context = self.context.clone();
        let translate = self.translate;
        let no_timestamps = self.no_timestamps;

        tokio::task::spawn_blocking(move || {
            let guard = context.lock();
            let ctx = guard.as_ref().ok_or(WhisperError::ModelNotLoaded)?;

            let mut state = ctx
                .create_state()
                .map_err(|e| WhisperError::TranscriptionFailed(e.to_string()))?;

            let mut params =
                whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
            let n_threads = crate::whisper_threads();
            params.set_n_threads(n_threads);
            params.set_translate(translate);
            params.set_no_timestamps(no_timestamps);
            params.set_print_special(false);
            params.set_print_progress(false);
            params.set_print_realtime(false);
            params.set_print_timestamps(false);
            params.set_suppress_blank(true);
            params.set_suppress_nst(true);

            state
                .full(params, &audio)
                .map_err(|e| WhisperError::TranscriptionFailed(e.to_string()))?;

            let num_segments = state.full_n_segments();
            let mut text = String::new();
            for i in 0..num_segments {
                if let Some(segment) = state.get_segment(i) {
                    if let Ok(segment_text) = segment.to_str_lossy() {
                        text.push_str(&segment_text);
                    }
                }
            }
            Ok(text.trim().to_string())
        })
        .await
        .map_err(|e| WhisperError::TranscriptionFailed(format!("Task join error: {e}")))?
    }

#[cfg(not(feature = "whisper-engine"))]
    async fn decode_partial(&self, _audio: Vec<f32>) -> Result<String, WhisperError> {
        Ok(String::new())
    }
}

impl Default for WhisperSttEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SttEngine for WhisperSttEngine {
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

        {
            let mut buf = self.buffer.lock();
            let max_samples = self.max_buffer_samples();
            let normalized =
                normalize_to_engine(&audio.data, audio.sample_rate, audio.channels, self.sample_rate);
            let overflow = buf
                .len()
                .saturating_add(normalized.len())
                .saturating_sub(max_samples);
            if overflow > 0 {
                let end = overflow.min(buf.len());
                buf.drain(..end);
            }
            buf.extend_from_slice(&normalized);
        }

        if !self.is_model_loaded() {
            return Ok(String::new());
        }

        if audio.is_final {
            let accumulated = {
                let mut buf = self.buffer.lock();
                std::mem::take(&mut *buf)
            };

            let rms = compute_rms(&accumulated);
            if rms < 0.01 {
                return Ok(String::new());
            }

            let result = self
                .transcribe_audio_async(accumulated, audio.sample_rate)
                .await;
            match result {
                Ok(text) => {
                    let rest = {
                        let last = self.last_text.lock();
                        apply_session_carry(&last, &text)
                    };
                    *self.last_text.lock() = text;
                    Ok(rest)
                }
                Err(e) => {
                    warn!("Transcription error: {}", e);
                    Ok(String::new())
                }
            }
        } else {
            Ok(String::new())
        }
    }

    async fn transcribe_stream(
        &self,
        mut stream: Box<dyn AudioStream>,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        let mut accumulated = Vec::new();
        let sr = stream.sample_rate();

        while let Some(chunk) = stream.next_chunk().await {
            accumulated.extend_from_slice(&chunk.data);
        }

        let accumulated = normalize_to_engine(&accumulated, sr, 1, self.sample_rate);
        self.buffer.lock().clear();

        let rms = compute_rms(&accumulated);
        if rms < 0.01 {
            return Ok(String::new());
        }

        match self.transcribe_audio_async(accumulated, sr).await {
            Ok(text) => Ok(text),
            Err(e) => {
                warn!("Stream transcription error: {}", e);
                Ok(String::new())
            }
        }
    }

    async fn transcribe_partial(
        &self,
        audio: &AudioChunk,
    ) -> Result<String, voxy_voice_orchestrator::VoiceOrchestratorError> {
        if audio.data.is_empty() || !self.is_model_loaded() {
            return Ok(String::new());
        }
        let rms = compute_rms(&audio.data);
        if rms < 0.01 {
            return Ok(String::new());
        }
        let normalized =
            normalize_to_engine(&audio.data, audio.sample_rate, audio.channels, self.sample_rate);
        match self.decode_partial(normalized).await {
            Ok(text) => Ok(text),
            Err(e) => {
                warn!("Partial transcription error: {}", e);
                Ok(String::new())
            }
        }
    }

    fn supports_partial_transcription(&self) -> bool {
        true
    }

    fn supported_languages(&self) -> Vec<String> {
        vec![
            "en".to_string(),
            "es".to_string(),
            "fr".to_string(),
            "de".to_string(),
            "it".to_string(),
            "pt".to_string(),
            "ru".to_string(),
            "ja".to_string(),
            "ko".to_string(),
            "zh".to_string(),
        ]
    }

    fn is_available(&self) -> bool {
        self.is_model_loaded()
    }
}

/// Apply session-rotation carry: given the previous session text and a new
/// (full-buffer) transcription, return only the NEW portion. This gap-fills
/// audio that was rotated out of the buffer (its text lives in `last`) and
/// prevents the session transcript from being re-reported every utterance.
fn apply_session_carry(last: &str, text: &str) -> String {
    let (carry, rest) = voxy_voice_stream::join_carry(last, text);
    if carry.is_empty() {
        text.trim().to_string()
    } else {
        rest.trim().to_string()
    }
}

fn compute_rms(data: &[f32]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = data.iter().map(|&s| (s as f64) * (s as f64)).sum();
    (sum_sq / data.len() as f64).sqrt()
}

/// Normalize captured audio (arbitrary sample rate / channel count, e.g. the
/// 48kHz stereo that Windows WASAPI falls back to) to the engine's expected
/// 16kHz mono format. whisper-rs expects 16kHz mono; feeding it raw
/// interleaved stereo destroys transcription accuracy (empirically on real
/// models: exact text at 16k mono, garbage at 48k stereo).
fn normalize_to_engine(data: &[f32], sample_rate: u32, channels: u8, target_sr: u32) -> Vec<f32> {
    let ch = channels.max(1) as usize;
    let mono: Vec<f32> = if ch > 1 {
        let mut m = Vec::with_capacity(data.len() / ch);
        for frame in data.chunks_exact(ch) {
            let sum: f32 = frame.iter().sum();
            m.push(sum / ch as f32);
        }
        m
    } else {
        data.to_vec()
    };
    if sample_rate == target_sr {
        mono
    } else {
        linear_resample(&mono, sample_rate, target_sr)
    }
}

fn linear_resample(data: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || data.is_empty() {
        return data.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (data.len() as f64 * (to as f64 / from as f64)) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = data.get(idx).copied().unwrap_or(0.0);
        let b = data.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_threads_respects_env_override_and_upper_cap() {
        std::env::set_var("VOXY_WHISPER_THREADS", "8");
        let n = whisper_threads();
        assert!((1..=16).contains(&n), "threads out of range: {n}");
        assert_eq!(n, whisper_threads().min(16));
        std::env::remove_var("VOXY_WHISPER_THREADS");
        let d = whisper_threads();
        assert!((1..=16).contains(&d), "default threads out of range: {d}");
        assert!(d <= 4 || (d as usize) <= std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4));
    }

    #[test]
    fn whisper_threads_clamps_to_two_minimum() {
        std::env::set_var("VOXY_WHISPER_THREADS", "0");
        assert!(whisper_threads() >= 2);
        std::env::set_var("VOXY_WHISPER_THREADS", "1");
        assert_eq!(whisper_threads(), 2);
        std::env::remove_var("VOXY_WHISPER_THREADS");
    }

    #[test]
    fn normalize_48k_stereo_to_16k_mono_preserves_signal() {
        let mut mono16k = Vec::new();
        for i in 0..1600 {
            mono16k.push((i as f32 % 7.0) / 7.0 - 0.5);
        }
        let up48 = linear_resample(&mono16k, 16000, 48000);
        let mut stereo = Vec::with_capacity(up48.len() * 2);
        for &s in &up48 {
            stereo.push(s);
            stereo.push(s);
        }
        let out = normalize_to_engine(&stereo, 48000, 2, 16000);
        assert_eq!(out.len(), mono16k.len());
        for i in 0..mono16k.len() {
            assert!((out[i] - mono16k[i]).abs() < 1e-4, "sample {i} differs");
        }
    }

    #[test]
    fn normalize_downmixes_different_channels() {
        let mono = vec![0.2f32, -0.4, 0.6, -0.8];
        let mut stereo = Vec::new();
        for &s in &mono {
            stereo.push(s);
            stereo.push(s * 0.5);
        }
        let out = normalize_to_engine(&stereo, 16000, 2, 16000);
        for i in 0..mono.len() {
            let expected = (mono[i] + mono[i] * 0.5) / 2.0;
            assert!((out[i] - expected).abs() < 1e-6, "sample {i} differs");
        }
    }
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

    #[tokio::test]
    async fn test_whisper_empty_audio() {
        let engine = WhisperSttEngine::new();
        let chunk = make_chunk(vec![], 16000, false);
        let result = engine.transcribe(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_whisper_silence() {
        let engine = WhisperSttEngine::new();
        let chunk = make_chunk(vec![0.0; 480], 16000, true);
        let result = engine.transcribe(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_whisper_non_final_returns_empty() {
        let engine = WhisperSttEngine::new();
        let data: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0 * 0.5).sin()).collect();
        let chunk = make_chunk(data, 16000, false);
        let result = engine.transcribe(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_whisper_no_model_returns_empty() {
        let engine = WhisperSttEngine::new();
        let data: Vec<f32> = (0..16000)
            .map(|i| (i as f32 / 16000.0 * 0.5).sin())
            .collect();
        let chunk = make_chunk(data, 16000, true);
        let result = engine.transcribe(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_whisper_partial_without_model_returns_empty() {
        let engine = WhisperSttEngine::new();
        let data: Vec<f32> = (0..16000)
            .map(|i| (i as f32 / 16000.0 * 0.5).sin())
            .collect();
        let chunk = make_chunk(data, 16000, false);
        let result = engine.transcribe_partial(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_whisper_partial_silence_returns_empty() {
        let engine = WhisperSttEngine::new();
        let chunk = make_chunk(vec![0.0; 4800], 16000, false);
        let result = engine.transcribe_partial(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_whisper_partial_empty_audio_returns_empty() {
        let engine = WhisperSttEngine::new();
        let chunk = make_chunk(vec![], 16000, false);
        let result = engine.transcribe_partial(&chunk).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_whisper_supports_partial_transcription() {
        let engine = WhisperSttEngine::new();
        assert!(engine.supports_partial_transcription());
    }

    #[tokio::test]
    async fn test_whisper_partial_does_not_consume_internal_buffer() {
        let engine = WhisperSttEngine::new();
        let data: Vec<f32> = (0..4800)
            .map(|i| (i as f32 / 4800.0 * 0.5).sin())
            .collect();
        let chunk = make_chunk(data, 16000, false);
        let _ = engine.transcribe_partial(&chunk).await.unwrap();
        assert!(engine.buffered_duration_ms() < 0.1);
    }

    #[tokio::test]
    async fn test_whisper_name_and_languages() {
        let engine = WhisperSttEngine::new();
        assert_eq!(engine.name(), "whisper-stt");
        assert!(engine.supported_languages().contains(&"en".to_string()));
    }

    #[tokio::test]
    async fn test_whisper_buffer_management() {
        let engine = WhisperSttEngine::new();
        let data: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0 * 0.5).sin()).collect();
        let chunk = make_chunk(data, 16000, false);
        let _ = engine.transcribe(&chunk).await.unwrap();
        assert!(engine.buffered_duration_ms() > 0.0);
        engine.clear_buffer();
        assert!(engine.buffered_duration_ms() < 0.1);
    }

    #[tokio::test]
    async fn test_whisper_config() {
        let engine = WhisperSttEngine::new()
            .with_language("es")
            .with_translate(true);
        assert_eq!(engine.language, "es");
        assert!(engine.translate);
    }

    #[test]
    fn session_carry_returns_new_portion_only() {
        assert_eq!(apply_session_carry("hello world", "hello world what is the weather"), "what is the weather");
    }

    #[test]
    fn session_carry_first_utterance_returns_full() {
        assert_eq!(apply_session_carry("", "hello world"), "hello world");
    }

    #[test]
    fn session_carry_after_rotation_returns_new_speech() {
        assert_eq!(apply_session_carry("hello world what time is it", "what time is it the clock"), "the clock");
    }

    #[test]
    fn session_carry_disjoint_returns_full_text() {
        assert_eq!(apply_session_carry("hello world", "totally different"), "totally different");
    }

    #[test]
    fn clear_buffer_resets_session_carry() {
        let engine = WhisperSttEngine::new();
        engine.last_text.lock().push_str("hello world");
        engine.clear_buffer();
        assert!(engine.last_text.lock().is_empty());
        assert!(engine.buffered_duration_ms() < 0.1);
    }
}
