//! PHASE 24 synthetic end-to-end validation harness.
//!
//! Drives the REAL production `VoicePipeline` (VAD -> endpointing -> partial/final
//! STT -> streaming LLM -> sentence-chunked TTS -> playback write -> barge-in) with
//! synthetic utterances synthesized by the REAL Piper engine, transcribed by the REAL
//! whisper engine, and answered by the REAL LLM provider. A synthetic input stream
//! stands in for the microphone and a recording output stream stands in for the
//! speaker. This is TEST/VALIDATION infrastructure only -- it is not a second
//! production voice path and does not modify the production pipeline logic.
//!
//! Measurements are labeled "REAL MODEL / SYNTHETIC AUDIO" and are NOT equivalent to
//! human-speech validation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::Mutex;
use tokio::sync::mpsc;

use voxy_audio::config::{AudioRuntimeConfig, AudioStreamConfig};
use voxy_audio::device::{AudioDeviceInfo, AudioDeviceManager};
use voxy_audio::error::{AudioError, Result as AudioResult};
use voxy_audio::stream::{
    AudioInputStream, AudioOutputStream, AudioPacket, AudioPacketStream,
};
use voxy_hardware::DeviceType;
use voxy_provider_core::LlmChunk;
use voxy_voice::{VoiceConfig, VoicePipeline};
use voxy_voice_orchestrator::{PartialTranscript, TtsEngine};

const INPUT_SR: u32 = 16000;
const INPUT_CH: u8 = 1;
const FRAME_SAMPLES: usize = 480 * INPUT_CH as usize; // 480 frames; 30 ms at 16 kHz mono
const LEAD_SILENCE_MS: u64 = 600;
const TRAIL_SILENCE_MS: u64 = 1600;
const RESPONSE_TIMEOUT_MS: u64 = 30_000;
const QUIESCE_MS: u64 = 1200;

/// A scripted utterance slot served by the synthetic input stream.
struct Utterance {
    label: String,
    samples: Vec<f32>,
    onset_wall_ms: Option<i64>,
}

struct ScriptState {
    current: Option<Utterance>,
    pos: usize,
    lead_samples_left: usize,
    trail_samples_left: usize,
    serving_speech: bool,
    /// After this many total speech samples served, start returning read errors
    /// (failure-injection knob; `None` disables).
    fail_after_samples: Option<usize>,
    samples_served: usize,
    round: u64,
    read_calls: usize,
}

impl ScriptState {
    fn new() -> Self {
        Self {
            current: None,
            pos: 0,
            lead_samples_left: 0,
            trail_samples_left: 0,
            serving_speech: false,
            fail_after_samples: None,
            samples_served: 0,
            round: 0,
            read_calls: 0,
        }
    }

    fn set_utterance(&mut self, label: String, samples: Vec<f32>) {
        self.lead_samples_left = (LEAD_SILENCE_MS * INPUT_SR as u64 * INPUT_CH as u64) as usize / 1000;
        self.trail_samples_left = (TRAIL_SILENCE_MS * INPUT_SR as u64 * INPUT_CH as u64) as usize / 1000;
        self.current = Some(Utterance {
            label,
            samples,
            onset_wall_ms: None,
        });
        self.pos = 0;
        self.serving_speech = false;
        self.round += 1;
    }
}

/// Serves the current utterance (or silence) as {INPUT_SR} Hz mono samples.
fn serve(state: &mut ScriptState, n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; n];
    if state.current.is_none() {
        return out;
    }
    if state.fail_after_samples.is_some() && state.samples_served >= state.fail_after_samples.unwrap() {
        // Caller checks this flag and returns an error.
        state.current = None;
        return out;
    }
    if state.lead_samples_left > 0 {
        let take = state.lead_samples_left.min(n);
        state.lead_samples_left -= take;
        if state.lead_samples_left == 0 {
            state.serving_speech = true;
        }
        return out;
    }
    if state.serving_speech {
        let u = state.current.as_mut().unwrap();
        let remaining = u.samples.len() - state.pos;
        let take = remaining.min(n);
        out[..take].copy_from_slice(&u.samples[state.pos..state.pos + take]);
        state.pos += take;
        state.samples_served += take;
        if u.onset_wall_ms.is_none() {
            u.onset_wall_ms = Some(Utc::now().timestamp_millis());
            tracing::info!("[P24:T0] label='{}' T0_onset_wall={}", u.label, u.onset_wall_ms.unwrap());
        }
        if state.pos >= u.samples.len() {
            state.serving_speech = false;
        }
        return out;
    }
    if state.trail_samples_left > 0 {
        let take = state.trail_samples_left.min(n);
        state.trail_samples_left -= take;
        if state.trail_samples_left == 0 {
            state.current = None;
        }
    }
    out
}

struct Recorder {
    start: Instant,
    writes: Vec<WriteEntry>,
}

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct WriteEntry {
    wall_ms: i64,
    elapsed_ms: f64,
    duration_ms: f64,
    samples: usize,
}

impl Recorder {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            writes: Vec::new(),
        }
    }

    fn record(&mut self, packet: &AudioPacket) {
        self.writes.push(WriteEntry {
            wall_ms: Utc::now().timestamp_millis(),
            elapsed_ms: self.start.elapsed().as_secs_f64() * 1000.0,
            duration_ms: packet.duration_ms,
            samples: packet.data.len(),
        });
    }
}

/// AudioDeviceManager providing a scripted input stream and a recording output stream.
pub struct SyntheticDeviceManager {
    script: Arc<Mutex<ScriptState>>,
    recorder: Arc<Mutex<Recorder>>,
    initialized: Arc<AtomicBool>,
}

impl Clone for SyntheticDeviceManager {
    fn clone(&self) -> Self {
        Self {
            script: self.script.clone(),
            recorder: self.recorder.clone(),
            initialized: self.initialized.clone(),
        }
    }
}

impl SyntheticDeviceManager {
    pub fn new() -> Self {
        Self {
            script: Arc::new(Mutex::new(ScriptState::new())),
            recorder: Arc::new(Mutex::new(Recorder::new())),
            initialized: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn set_utterance(&self, label: String, samples: Vec<f32>) {
        self.script.lock().set_utterance(label, samples);
    }

    #[allow(dead_code)]
    pub fn set_fail_after_samples(&self, n: usize) {
        self.script.lock().fail_after_samples = Some(n);
    }

    pub fn round(&self) -> u64 {
        self.script.lock().round
    }
}

fn fake_device(kind: &str) -> AudioDeviceInfo {
    AudioDeviceInfo {
        id: format!("synthetic-{kind}"),
        name: format!("Synthetic {kind} (Phase 24 harness)"),
        device_type: if kind == "input" {
            DeviceType::Microphone
        } else {
            DeviceType::Speaker
        },
        status: voxy_hardware::DeviceStatus::Available,
        supported_sample_rates: vec![16000, 22050, 48000],
        supported_channels: vec![1, 2],
        is_default: true,
    }
}

#[async_trait]
impl AudioDeviceManager for SyntheticDeviceManager {
    async fn initialize(&self, _config: &AudioRuntimeConfig) -> AudioResult<()> {
        if self.initialized.load(Ordering::SeqCst) {
            return Err(AudioError::AlreadyInitialized);
        }
        self.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn shutdown(&self) -> AudioResult<()> {
        self.initialized.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn list_inputs(&self) -> AudioResult<Vec<AudioDeviceInfo>> {
        Ok(vec![fake_device("input")])
    }

    async fn list_outputs(&self) -> AudioResult<Vec<AudioDeviceInfo>> {
        Ok(vec![fake_device("output")])
    }

    async fn default_input(&self) -> AudioResult<AudioDeviceInfo> {
        Ok(fake_device("input"))
    }

    async fn default_output(&self) -> AudioResult<AudioDeviceInfo> {
        Ok(fake_device("output"))
    }

    async fn open_input(&self, _config: &AudioStreamConfig) -> AudioResult<Box<dyn AudioInputStream>> {
        Ok(Box::new(SyntheticInputStream {
            script: self.script.clone(),
            sample_rate: INPUT_SR,
            channels: INPUT_CH,
            open: AtomicBool::new(true),
        }))
    }

    async fn open_output(&self, _config: &AudioStreamConfig) -> AudioResult<Box<dyn AudioOutputStream>> {
        Ok(Box::new(SyntheticOutputStream {
            recorder: self.recorder.clone(),
            sample_rate: INPUT_SR,
            channels: INPUT_CH,
            open: AtomicBool::new(true),
        }))
    }

    async fn get_device(&self, id: &str) -> AudioResult<AudioDeviceInfo> {
        if id.contains("input") {
            Ok(fake_device("input"))
        } else if id.contains("output") {
            Ok(fake_device("output"))
        } else {
            Err(AudioError::DeviceNotFound(id.to_string()))
        }
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }
}

struct SyntheticInputStream {
    script: Arc<Mutex<ScriptState>>,
    sample_rate: u32,
    channels: u8,
    open: AtomicBool,
}

#[async_trait]
impl AudioInputStream for SyntheticInputStream {
    async fn open(&mut self, config: &AudioStreamConfig) -> AudioResult<()> {
        self.sample_rate = config.sample_rate;
        self.channels = config.channels;
        self.open.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn close(&mut self) -> AudioResult<()> {
        self.open.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn read(&mut self, frames: usize) -> AudioResult<AudioPacket> {
        let (data, seq, read_calls, round, label) = {
            let mut guard = self.script.lock();
            if guard.fail_after_samples.is_some() && guard.samples_served >= guard.fail_after_samples.unwrap() {
                guard.fail_after_samples = None;
                guard.current = None;
                return Err(AudioError::StreamError("synthetic read failure injected".into()));
            }
            let n = frames * self.channels as usize;
            let data = serve(&mut guard, n);
            guard.read_calls += 1;
            let rc = guard.read_calls;
            let r = guard.round;
            let lbl = guard.current.as_ref().map(|u| u.label.clone()).unwrap_or_else(|| "-".into());
            (data, guard.samples_served as u64, rc, r, lbl)
        };
        if read_calls % 100 == 0 {
            tracing::info!("[P24:READ] calls={} round={} label='{}' sr={}ch={}", read_calls, round, label, self.sample_rate, self.channels);
        }
        let frame_ms = (frames as u64 * 1000) / self.sample_rate.max(1) as u64;
        if frame_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(frame_ms)).await;
        }
        let mut packet = AudioPacket::new(data, self.sample_rate, self.channels);
        packet.sequence = seq;
        Ok(packet)
    }

    async fn stream(&mut self) -> Box<dyn AudioPacketStream> {
        Box::new(SyntheticPacketStream {
            script: self.script.clone(),
            sample_rate: self.sample_rate,
            channels: self.channels,
        })
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u8 {
        self.channels
    }

    fn latency_ms(&self) -> f64 {
        0.0
    }

    fn is_open(&self) -> bool {
        self.open.load(Ordering::SeqCst)
    }

    fn device_id(&self) -> Option<&str> {
        Some("synthetic-input")
    }
}

struct SyntheticPacketStream {
    script: Arc<Mutex<ScriptState>>,
    sample_rate: u32,
    channels: u8,
}

#[async_trait]
impl AudioPacketStream for SyntheticPacketStream {
    async fn next(&mut self) -> Option<AudioPacket> {
        let n = FRAME_SAMPLES;
        let data = {
            let mut guard = self.script.lock();
            serve(&mut guard, n)
        };
        Some(AudioPacket::new(data, self.sample_rate, self.channels))
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u8 {
        self.channels
    }

    fn is_complete(&self) -> bool {
        false
    }
}

struct SyntheticOutputStream {
    recorder: Arc<Mutex<Recorder>>,
    sample_rate: u32,
    channels: u8,
    open: AtomicBool,
}

#[async_trait]
impl AudioOutputStream for SyntheticOutputStream {
    async fn open(&mut self, config: &AudioStreamConfig) -> AudioResult<()> {
        self.sample_rate = config.sample_rate;
        self.channels = config.channels;
        self.open.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn close(&mut self) -> AudioResult<()> {
        self.open.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn write(&mut self, packet: &AudioPacket) -> AudioResult<()> {
        self.recorder.lock().record(packet);
        Ok(())
    }

    async fn play(&mut self, mut stream: Box<dyn AudioPacketStream>) -> AudioResult<()> {
        while let Some(packet) = stream.next().await {
            self.recorder.lock().record(&packet);
        }
        Ok(())
    }

    async fn flush(&mut self) -> AudioResult<()> {
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u8 {
        self.channels
    }

    fn latency_ms(&self) -> f64 {
        0.0
    }

    fn is_open(&self) -> bool {
        self.open.load(Ordering::SeqCst)
    }

    fn device_id(&self) -> Option<&str> {
        Some("synthetic-output")
    }
}

fn linear_resample(data: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return data.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (data.len() as f64 / ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = (i as f64) * ratio;
        let idx = src.floor() as usize;
        let frac = src - idx as f64;
        let a = data.get(idx).copied().unwrap_or(0.0);
        let b = data.get(idx + 1).copied().unwrap_or(a);
        out.push((a as f64 * (1.0 - frac) + b as f64 * frac) as f32);
    }
    out
}

/// Piper inserts long robotic inter-word pauses that the real VAD would not
/// normally see in human speech. The VAD (EnergyVadDetector) treats any
/// 30ms/480-sample chunk whose RMS is below the VAD threshold as silence and
/// finalizes the utterance after 3 consecutive silent chunks (90ms). Sample-
/// level clamping is insufficient because long "mostly quiet but spiky"
/// regions (piper fade-outs, breathy gaps) keep RMS below threshold without a
/// contiguous run of near-zero samples.
///
/// This chunk-level squeeze keeps only the middle 30ms chunk of every run of
/// 2+ consecutive low-RMS chunks, so no quiet run longer than ~30ms survives
/// and the utterance is captured as a single phrase (matching human cadence).
/// Content is unchanged; only inter-word silence/fade regions are shortened.
fn squeeze_gaps(mono: &[f32], _sr: u32, _max_gap_ms: u64, _keep_ms: u64) -> Vec<f32> {
    const FRAME: usize = 480;
    const VAD_THRESHOLD: f32 = 0.05;
    let chunks: Vec<&[f32]> = mono.chunks(FRAME).collect();
    let n = chunks.len();
    if n == 0 {
        return mono.to_vec();
    }
    let rms: Vec<f32> = chunks
        .iter()
        .map(|c| {
            let ss: f64 = c.iter().map(|&s| (s as f64) * (s as f64)).sum();
            (ss / c.len() as f64).sqrt() as f32
        })
        .collect();
    let mut out: Vec<f32> = Vec::with_capacity(mono.len());
    let mut i = 0usize;
    while i < n {
        if rms[i] < VAD_THRESHOLD {
            let start = i;
            while i < n && rms[i] < VAD_THRESHOLD {
                i += 1;
            }
            let run = i - start;
            if run >= 2 {
                out.extend_from_slice(chunks[start + run / 2]);
            } else {
                out.extend_from_slice(chunks[start]);
            }
        } else {
            out.extend_from_slice(chunks[i]);
            i += 1;
        }
    }
    out
}

/// Feeds the same 480-sample chunks the capture loop would feed the VAD and
/// logs when the VAD would drop out of speech, to ground-truth fragmentation.
async fn debug_vad_behavior(samples: &[f32], sr: u32, label: &str) {
    use voxy_voice_orchestrator::VadDetector;
    let vad = voxy_voice::detection::EnergyVadDetector::new(0.05, sr)
        .with_min_speech_frames(3)
        .with_silence_frames_for_end(3);
    let mut onset = None;
    let mut off_frame = 0usize;
    for (i, c) in samples.chunks(480).enumerate() {
        let chunk = voxy_voice_orchestrator::AudioChunk {
            data: c.to_vec(),
            sample_rate: sr,
            channels: 1,
            timestamp: Utc::now(),
            sequence: i as u64,
            is_final: false,
        };
        let voice = vad.is_voice(&chunk).await.unwrap_or(false);
        if voice && onset.is_none() {
            onset = Some(i);
        }
        if voice {
            off_frame = 0;
        } else if onset.is_some() {
            off_frame += 1;
            if off_frame == 3 {
                tracing::info!(
                    "[P24:VAD] '{}' IN-SPEECH frames {}..{} then dropped at frame {} (gap_ms ~{})",
                    label,
                    onset.unwrap(),
                    i - 2,
                    i,
                    i * 30
                );
                break;
            }
        }
    }
}

fn max_silence_gap_ms(mono: &[f32], sr: u32) -> u64 {
    let mut max_gap = 0usize;
    let mut cur = 0usize;
    for s in mono {
        if s.abs() < 0.05 {
            cur += 1;
            if cur > max_gap {
                max_gap = cur;
            }
        } else {
            cur = 0;
        }
    }
    max_gap as u64 * 1000 / sr as u64
}

fn frame_rms_stats(mono: &[f32], sr: u32) -> (f32, f32, usize) {
    let frame = (sr / 1000 * 30) as usize;
    let mut min = f32::MAX;
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for c in mono.chunks(frame.max(1)) {
        if c.is_empty() {
            continue;
        }
        let ss: f64 = c.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (ss / c.len() as f64).sqrt() as f32;
        if rms < min {
            min = rms;
        }
        sum += rms as f64;
        n += 1;
    }
    (min, (sum / n.max(1) as f64) as f32, n)
}

fn to_channel_layout(mono: &[f32]) -> Vec<f32> {
    match INPUT_CH {
        1 => mono.to_vec(),
        _ => {
            let mut out = Vec::with_capacity(mono.len() * INPUT_CH as usize);
            for &s in mono {
                for _ in 0..INPUT_CH {
                    out.push(s);
                }
            }
            out
        }
    }
}

/// Build a lean, production-faithful streaming response handler that uses the
/// REAL LLM provider (create_llm_provider: Groq via env) and the REAL
/// SentenceChunker, sending sentences over the pipeline's TTS channel.
fn build_streaming_handler(
    llm: Arc<dyn voxy_provider_core::LlmProvider>,
) -> Arc<dyn Fn(String, mpsc::Sender<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>
{
    let llm = llm.clone();
    Arc::new(move |text: String, tx: mpsc::Sender<String>| {
        let llm = llm.clone();
        Box::pin(async move {
            let sanitized = text.trim().to_string();
            if sanitized.is_empty() || sanitized == "short sound" {
                let _ = tx.send(String::new()).await;
                return;
            }
            let prompt = format!(
                "You are VOXY, a concise Windows voice assistant running as a validation harness. \
                 Answer naturally. If the user asks you to open or launch an app, name it. \
                 Keep responses short unless the question demands detail.\n\nUser: {sanitized}\nAssistant:"
            );
            let (llm_tx, mut llm_rx) = mpsc::channel::<LlmChunk>(16);
            let llm_clone = llm.clone();
            let prompt_clone = prompt.clone();
            tokio::spawn(async move {
                if let Err(e) = llm_clone.complete_streaming(&prompt_clone, llm_tx).await {
                    tracing::warn!("[P24] LLM streaming failed: {e}");
                }
            });
            let mut chunker = voxy_voice_stream::SentenceChunker::new(20);
            let mut chars = 0usize;
            while let Some(chunk) = llm_rx.recv().await {
                if chunk.done {
                    for sentence in chunker.finish() {
                        tracing::info!("[VOICE:STREAMING] Sending sentence to TTS: '{sentence}'");
                        let _ = tx.send(sentence).await;
                    }
                    break;
                }
                chars += chunk.text.len();
                for sentence in chunker.feed(&chunk.text) {
                    tracing::info!("[VOICE:STREAMING] Sending sentence to TTS: '{sentence}'");
                    let _ = tx.send(sentence).await;
                }
            }
            tracing::info!("[P24] LLM handler ended ({} chars)", chars);
        })
    })
}

struct RoundResult {
    round: u64,
    label: String,
    t0: i64,
    t2: i64,
    first_write_wall_ms: i64,
    writes: usize,
    audio_written_ms: f64,
    failure: Option<String>,
}

/// Wait until the pipeline's TTS playback for the current round has started and
/// then gone quiet (or until timeout). Returns the first-write wall clock.
async fn wait_for_response(
    recorder: Arc<Mutex<Recorder>>,
    base_writes: usize,
) -> Result<(i64, usize, f64), String> {
    let start = Instant::now();
    let mut saw_write = false;
    loop {
        let writes = recorder.lock().writes.clone();
        let count = writes.len();
        if count > base_writes {
            saw_write = true;
        }
        if saw_write {
            let last = writes.last().cloned();
            if let Some(w) = last {
                let last_elapsed = w.elapsed_ms;
                let now_elapsed = recorder.lock().start.elapsed().as_secs_f64() * 1000.0;
                if now_elapsed - last_elapsed > QUIESCE_MS as f64 {
                    let first = writes.get(base_writes).cloned().unwrap();
                    return Ok((first.wall_ms, count - base_writes, w.duration_ms));
                }
            }
        }
        if start.elapsed().as_millis() as u64 > RESPONSE_TIMEOUT_MS {
            if saw_write {
                let first = writes.get(base_writes).cloned().unwrap_or(WriteEntry {
                    wall_ms: 0,
                    elapsed_ms: 0.0,
                    duration_ms: 0.0,
                    samples: 0,
                });
                return Ok((first.wall_ms, count.saturating_sub(base_writes), 0.0));
            }
            return Err(format!(
                "no playback write within {} ms (LLM/TTS failed or timed out)",
                RESPONSE_TIMEOUT_MS
            ));
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

struct PartialCollector {
    first_partial_wall_ms: Mutex<Option<i64>>,
}

pub async fn run_p24_e2e() {
    // No loudspeaker→mic echo path exists in this synthetic setup, so the
    // adaptive echo canceller would only inject adaptation residual into
    // captured silence. Disable it to emulate a real mic with no nearby
    // speaker (documented in the Phase 24 report).
    std::env::set_var("VOXY_DISABLE_ECHO_CANCELLER", "1");
    tracing::info!("[P24] ===== PHASE 24 synthetic e2e harness start =====");
    tracing::info!(
        "[P24] mode: REAL MODEL / SYNTHETIC AUDIO — input {}Hz/{}ch synthetic, output recording",
        INPUT_SR,
        INPUT_CH
    );

    // ── Real engines ──────────────────────────────────────────────
    let stt_model = std::env::var("VOXY_P24_STT_MODEL").unwrap_or_else(|_| "models/ggml-base.en.bin".into());
    let whisper = voxy_whisper::WhisperSttEngine::new()
        .with_model_path(stt_model.into());
    if let Err(e) = whisper.load_model() {
        tracing::error!("[P24] whisper load failed: {e}");
        return;
    }
    whisper.warmup().ok();
    tracing::info!("[P24] whisper ready (REAL MODEL)");

    let tts_model = std::env::var("VOXY_P24_TTS_MODEL").unwrap_or_else(|_| "models/en_US-lessac-medium.onnx".into());
    let tts = voxy_kokoro::KokoroTtsEngine::new()
        .with_voice("default")
        .with_speed(0.55)
        .with_pitch(1.0)
        .with_model_path(tts_model.into());
    if let Err(e) = tts.load_model() {
        tracing::error!("[P24] piper load failed: {e}");
        return;
    }
    tracing::info!("[P24] piper ready (REAL MODEL)");

    // ── Utterance set (English only; Hindi/Hinglish UNVERIFIED — whisper base.en) ──
    const UTTERANCES: &[(&str, &str)] = &[
        ("A", "Hello Voxy."),
        ("A", "Open my browser."),
        ("A", "Play some music."),
        ("A", "Stop."),
        ("A", "Close the window."),
        ("H", "What time is it?"),
        ("A", "Open notepad."),
        ("H", "What is the capital of France?"),
        ("B", "Search for Minecraft tutorials."),
        ("B", "Tell me today's weather."),
        ("B", "How are you doing today?"),
        ("B", "Explain this to me."),
        ("B", "Calculate twenty five multiplied by eighteen."),
        ("H", "What is two plus two?"),
        ("H", "Can you tell me a joke?"),
        ("H", "Where is the nearest coffee shop?"),
        ("A", "Please open calculator."),
        ("B", "Give me a summary of today's news."),
        ("B", "Set a timer for five minutes."),
        ("H", "Who wrote Pride and Prejudice?"),
        ("H", "What is the weather forecast for tomorrow?"),
        ("B", "Please play some relaxing music."),
        ("B", "Search the web for cheap flights to Paris."),
        ("C", "Explain in detail how the water cycle works."),
        ("C", "Tell me everything you know about the history of ancient Rome."),
        ("C", "Describe the process of photosynthesis in plants step by step."),
        ("C", "What are the main differences between dogs and cats as pets?"),
        ("H", "What is the largest planet in our solar system?"),
        ("B", "Recommend a good book to read this weekend."),
        ("A", "Stop the music."),
    ];

    // Pre-synthesize all utterances once with the REAL Piper engine (before the
    // engine is moved into the pipeline).
    let mut synth: Vec<(String, Vec<f32>)> = Vec::with_capacity(UTTERANCES.len());
    for (cat, text) in UTTERANCES {
        match tts.synthesize(text).await {
            Ok(chunk) => {
                let mono48 = linear_resample(&chunk.data, chunk.sample_rate, INPUT_SR);
                let squeezed = squeeze_gaps(&mono48, INPUT_SR, 60, 25);
                debug_vad_behavior(&squeezed, INPUT_SR, text).await;
                let (min_rms, avg_rms, nf) = frame_rms_stats(&squeezed, INPUT_SR);
                tracing::info!("[P24] gap raw={}ms squeezed={}ms rms_min={:.3} rms_avg={:.3} nframes={} for '{}'", max_silence_gap_ms(&mono48, INPUT_SR), max_silence_gap_ms(&squeezed, INPUT_SR), min_rms, avg_rms, nf, text);
                let stereo = to_channel_layout(&squeezed);
                let label = format!("{cat}:{text}");
                tracing::info!("[P24] synthesized '{}' -> {} samples {}Hz {}-ch", text, stereo.len(), INPUT_SR, INPUT_CH);
                synth.push((label, stereo));
            }
            Err(e) => {
                tracing::error!("[P24] piper synth failed for '{text}': {e}");
            }
        }
    }
    if synth.len() < 30 {
        tracing::error!("[P24] only {} utterances synthesized; aborting", synth.len());
        return;
    }
    let stop_samples = match tts.synthesize("Stop.").await {
        Ok(c) => {
            let m = linear_resample(&c.data, c.sample_rate, INPUT_SR);
            to_channel_layout(&squeeze_gaps(&m, INPUT_SR, 60, 25))
        }
        Err(e) => {
            tracing::error!("[P24] failed to synthesize barge-in 'Stop.': {e}");
            Vec::new()
        }
    };
    let recovery_samples = match tts.synthesize("Hello again.").await {
        Ok(c) => {
            let m = linear_resample(&c.data, c.sample_rate, INPUT_SR);
            to_channel_layout(&squeeze_gaps(&m, INPUT_SR, 60, 25))
        }
        Err(e) => {
            tracing::error!("[P24] failed to synthesize recovery phrase: {e}");
            Vec::new()
        }
    };
    tracing::info!("[P24] {} utterances + barge-in audio synthesized", synth.len());

    const BARGE_LONGS: &[&str] = &[
        "Tell me everything you know about the history of ancient Rome.",
        "Describe the process of photosynthesis in plants step by step.",
        "Explain in detail how the water cycle works.",
    ];
    let mut barge_long: Vec<(String, Vec<f32>)> = Vec::with_capacity(10);
    for (i, t) in BARGE_LONGS.iter().cycle().take(10).enumerate() {
        let label = format!("BARGELONG{i}:{t}");
        match tts.synthesize(t).await {
            Ok(c) => {
                let m = linear_resample(&c.data, c.sample_rate, INPUT_SR);
                barge_long.push((label, to_channel_layout(&squeeze_gaps(&m, INPUT_SR, 60, 25))));
            }
            Err(e) => {
                tracing::error!("[P24] barge long synth failed: {e}");
            }
        }
    }

    let llm = crate::create_llm_provider();
    tracing::info!("[P24] LLM provider ready");

    // ── Synthetic devices ─────────────────────────────────────────
    let mgr = SyntheticDeviceManager::new();
    if let Ok(n) = std::env::var("VOXY_P24_FAIL_AFTER_SAMPLES") {
        match n.parse::<usize>() {
            Ok(n) => {
                mgr.set_fail_after_samples(n);
                tracing::warn!("[P24] read-failure injection armed after {n} samples");
            }
            Err(_) => {}
        }
    }
    let recorder = mgr.recorder.clone();

    let mut config = VoiceConfig::default();
    config.auto_start_capture = true;
    config.wake_word_enabled = false;
    config.vad_threshold = 0.05;
    config.orchestrator.silence_timeout_ms = 100;
    config.audio.input.sample_rate = INPUT_SR;
    config.audio.input.channels = INPUT_CH;
    config.audio.output.sample_rate = INPUT_SR;
    config.audio.output.channels = INPUT_CH;

    let pipeline = Arc::new(VoicePipeline::with_audio_mgr(config.clone(), Box::new(mgr.clone())));
    pipeline.initialize().await.expect("pipeline init");
    pipeline.with_default_engines().await.expect("default engines");
    let _ = pipeline.set_stt_engine(Box::new(whisper)).await;
    let _ = pipeline.set_tts_engine(Box::new(tts)).await;
    let _ = pipeline.set_streaming_response_handler(build_streaming_handler(llm)).await;

    let partial = Arc::new(PartialCollector {
        first_partial_wall_ms: Mutex::new(None),
    });
    {
        let (partial_tx, mut partial_rx) = mpsc::channel::<PartialTranscript>(16);
        pipeline.set_partial_transcript_handler(partial_tx).await;
        let pcol = partial.clone();
        tokio::spawn(async move {
            while let Some(pt) = partial_rx.recv().await {
                let text = pt.text.trim().to_string();
                if !text.is_empty() && pcol.first_partial_wall_ms.lock().is_none() {
                    tracing::info!("[VOICE:STREAMING] First partial: '{text}'");
                    *pcol.first_partial_wall_ms.lock() = Some(Utc::now().timestamp_millis());
                }
            }
        });
    }

    pipeline.start_capture().await.expect("start capture");
    pipeline.initialize_v2().await.expect("initialize v2");
    pipeline.start_listening().await.expect("start listening");
    tracing::info!("[P24] pipeline listening (REAL production VoicePipeline)");

    let mut results: Vec<RoundResult> = Vec::new();
    let mut base_writes = recorder.lock().writes.len();
    let max_rounds = std::env::var("VOXY_P24_MAX_ROUNDS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(synth.len());

    for (label, samples) in synth.iter().enumerate().cycle().take(max_rounds).map(|(_, x)| x) {
        *partial.first_partial_wall_ms.lock() = None;
        mgr.set_utterance(label.clone(), samples.clone());
        let round = mgr.round();

        // Capture the speech-onset wall clock (T0) as serve() populates it;
        // by the time the response write arrives the utterance is consumed.
        let wait_fut = wait_for_response(recorder.clone(), base_writes);
        tokio::pin!(wait_fut);
        let mut t0 = 0i64;
        let result = loop {
            tokio::select! {
                r = &mut wait_fut => break r,
                _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                    if t0 == 0 {
                        t0 = mgr.script.lock().current.as_ref()
                            .and_then(|u| u.onset_wall_ms)
                            .unwrap_or(0);
                    }
                }
            }
        };

        match result {
            Ok((first_write_wall, writes, audio_ms)) => {
                let t2 = partial.first_partial_wall_ms.lock().unwrap_or(0);
                tracing::info!(
                    "[P24ROUND] r={} label='{}' t0={} t2={} first_write={} writes={} audio_ms={:.0}",
                    round,
                    label,
                    t0,
                    t2,
                    first_write_wall,
                    writes,
                    audio_ms
                );
                results.push(RoundResult {
                    round,
                    label: label.clone(),
                    t0,
                    t2,
                    first_write_wall_ms: first_write_wall,
                    writes,
                    audio_written_ms: audio_ms,
                    failure: None,
                });
            }
            Err(e) => {
                tracing::warn!("[P24ROUND] r={} label='{}' FAILED: {}", round, label, e);
                results.push(RoundResult {
                    round,
                    label: label.clone(),
                    t0,
                    t2: 0,
                    first_write_wall_ms: 0,
                    writes: 0,
                    audio_written_ms: 0.0,
                    failure: Some(e),
                });
            }
        }
        base_writes = recorder.lock().writes.len();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // ── Barge-in rounds ───────────────────────────────────────────
    let barge_count = std::env::var("VOXY_P24_BARGE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10)
        .min(barge_long.len());
    for (i, (long_label, long_samples)) in barge_long.iter().enumerate().take(barge_count) {

        *partial.first_partial_wall_ms.lock() = None;
        mgr.set_utterance(long_label.clone(), long_samples.clone());
        let round = mgr.round();
        base_writes = recorder.lock().writes.len();

        // Wait for playback to actually start.
        let play_start = Instant::now();
        let mut playback_started = false;
        while (play_start.elapsed().as_millis() as u64) < RESPONSE_TIMEOUT_MS {
            if recorder.lock().writes.len() > base_writes {
                playback_started = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        if !playback_started {
            tracing::warn!("[P24BARGE] round {} long response never produced audio", round);
            continue;
        }

        // Let a little of the response play, then inject "Stop."
        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        let writes_before_barge = recorder.lock().writes.len();
        mgr.set_utterance(format!("B-stop-{i}"), stop_samples.clone());
        let barge_round = mgr.round();
        let inject_wall_ms = Utc::now().timestamp_millis();

        // Wait for interruption to complete (writes quiesce again).
        let b_start = Instant::now();
        let mut interrupted = false;
        let mut last_wall = 0i64;
        while (b_start.elapsed().as_millis() as u64) < RESPONSE_TIMEOUT_MS {
            let writes = recorder.lock().writes.len();
            if writes > writes_before_barge {
                interrupted = true;
                let last = recorder.lock().writes.last().cloned();
                last_wall = last.map(|w| w.wall_ms).unwrap_or(0);
            }
            if interrupted {
                let now_elapsed = recorder.lock().start.elapsed().as_secs_f64() * 1000.0;
                let last_elapsed = recorder.lock().writes.last().map(|w| w.elapsed_ms).unwrap_or(0.0);
                if now_elapsed - last_elapsed > QUIESCE_MS as f64 {
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        tracing::info!(
            "[P24BARGE] r={} injected at {}ms wall, interruption observed={}, last write={}ms wall",
            barge_round,
            inject_wall_ms,
            interrupted,
            last_wall
        );

        // Next-turn recovery: feed a fresh short utterance and expect a new response.
        let recovery_label = format!("B-recover-{i}");
        let rec_samples = recovery_samples.clone();
        base_writes = recorder.lock().writes.len();
        mgr.set_utterance(recovery_label, rec_samples);
        match wait_for_response(recorder.clone(), base_writes).await {
            Ok((_, writes, _)) => {
                tracing::info!("[P24BARGE] round {} next-turn recovery OK ({} writes)", barge_round, writes);
            }
            Err(e) => {
                tracing::error!("[P24BARGE] round {} next-turn recovery FAILED: {e}", barge_round);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }

    pipeline.stop_listening().await;
    let _ = pipeline.stop_capture().await;
    let _ = pipeline.shutdown().await;

    tracing::info!("[P24] ===== harness complete: {} latency rounds, {} barge-in rounds =====", results.len(), barge_long.len());
    for r in &results {
        tracing::info!(
            "[P24SUMMARY] round={} label='{}' ok={} t0={} t2={} first_write={} writes={} audio_ms={:.0}",
            r.round,
            r.label,
            r.failure.is_none(),
            r.t0,
            r.t2,
            r.first_write_wall_ms,
            r.writes,
            r.audio_written_ms
        );
    }
}