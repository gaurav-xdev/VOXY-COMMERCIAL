#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use voxy_provider_core::{LlmChunk, LlmProvider};
use voxy_voice::VoicePipeline;

// ─── Timing Collection ───────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct UtteranceTimings {
    pub iteration: usize,
    pub t0_to_t3_ms: f64,
    pub t3_to_t5_ms: f64,
    pub t5_to_t6_ms: f64,
    pub t6_to_t7_ms: f64,
    pub t7_to_t8_ms: f64,
    pub t0_to_t8_total_ms: f64,
    pub transcription: String,
    pub llm_response_len: usize,
}

#[derive(Debug, Clone)]
pub struct TimingStats {
    pub name: &'static str,
    values: Vec<f64>,
}

impl TimingStats {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            values: Vec::with_capacity(64),
        }
    }

    pub fn record(&mut self, ms: f64) {
        self.values.push(ms);
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }

    pub fn min_ms(&self) -> f64 {
        self.values.iter().cloned().fold(f64::MAX, f64::min)
    }

    pub fn max_ms(&self) -> f64 {
        self.values.iter().cloned().fold(f64::MIN, f64::max)
    }

    pub fn avg_ms(&self) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        self.values.iter().sum::<f64>() / self.values.len() as f64
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() {
            return 0.0;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

// ─── Hardware Info ────────────────────────────────────────────────────────

pub fn detect_simd() -> Vec<String> {
    let mut caps = Vec::new();
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            caps.push("AVX2".to_string());
        }
        if is_x86_feature_detected!("avx512f") {
            caps.push("AVX-512F".to_string());
        }
        if is_x86_feature_detected!("sse2") {
            caps.push("SSE2".to_string());
        }
        if is_x86_feature_detected!("fma") {
            caps.push("FMA".to_string());
        }
        if is_x86_feature_detected!("aes") {
            caps.push("AES-NI".to_string());
        }
    }
    if caps.is_empty() {
        caps.push("none detected".to_string());
    }
    caps
}

pub fn detect_hardware_info() -> Vec<String> {
    let mut info = Vec::new();
    info.push(format!(
        "Logical cores: {}",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    ));
    info.push(format!("SIMD: {}", detect_simd().join(", ")));
    info.push(format!("Platform: {}", std::env::consts::OS));
    info.push(format!("Arch: {}", std::env::consts::ARCH));
    info
}

// ─── Benchmark Config ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct E2eBenchmarkConfig {
    pub iterations: usize,
    pub whisper_model_path: String,
    pub piper_model_path: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub silence_timeout_ms: u64,
}

impl Default for E2eBenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 5,
            whisper_model_path: "models/ggml-base.en.bin".into(),
            piper_model_path: "models/en_US-lessac-medium.onnx".into(),
            ollama_url: "http://127.0.0.1:11434".into(),
            ollama_model: "voxy-fast:latest".into(),
            silence_timeout_ms: 100,
        }
    }
}

// ─── Benchmark Runner ────────────────────────────────────────────────────

pub async fn run_e2e_benchmark(config: E2eBenchmarkConfig) {
    let hw = detect_hardware_info();

    println!();
    println!("=================================================================");
    println!("  VOXY End-to-End Real Microphone Benchmark");
    println!("=================================================================");
    println!();
    for h in &hw {
        println!("  Hardware: {}", h);
    }
    println!("  Iterations: {}", config.iterations);
    println!("  Whisper: {}", config.whisper_model_path);
    println!("  Piper TTS: {}", config.piper_model_path);
    println!("  LLM: {} @ {}", config.ollama_model, config.ollama_url);
    println!();
    println!("  Instructions:");
    println!("  1. When prompted, speak a test phrase into your microphone.");
    println!("  2. The pipeline captures speech via WASAPI, runs VAD, STT,");
    println!("     LLM streaming, sentence-chunked TTS, and plays audio.");
    println!("  3. T0-T8 timings are measured for each stage.");
    println!("  4. After all iterations, summary statistics are printed.");
    println!("=================================================================");
    println!();
    println!("Initializing pipeline with WASAPI audio devices...");
    println!();

    // ── Build pipeline with real WASAPI audio device manager ──────────
    let voice_config = voxy_voice::VoiceConfig {
        auto_start_capture: true,
        wake_word: "hey voxy".into(),
        wake_word_enabled: false,
        vad_enabled: true,
        vad_threshold: 0.05,
        orchestrator: voxy_voice_orchestrator::VoiceOrchestratorConfig {
            silence_timeout_ms: config.silence_timeout_ms,
            ..Default::default()
        },
        ..Default::default()
    };

    let audio_mgr = Box::new(voxy_audio::WasapiDeviceManager::new());
    let pipeline = Arc::new(VoicePipeline::with_audio_mgr(voice_config.clone(), audio_mgr));

    if let Err(e) = pipeline.initialize().await {
        tracing::error!("[E2E-BENCH] Pipeline init failed: {e}");
        return;
    }
    if let Err(e) = pipeline.with_default_engines().await {
        tracing::error!("[E2E-BENCH] Default engines failed: {e}");
        return;
    }

    println!("  Audio devices opened (WASAPI)");

    // ── Load Whisper STT model ───────────────────────────────────────
    #[cfg(feature = "whisper-engine")]
    {
        println!("  Loading Whisper model: {}...", config.whisper_model_path);
        let whisper = voxy_whisper::WhisperSttEngine::new()
            .with_model_path(config.whisper_model_path.clone().into());
        match whisper.load_model() {
            Ok(()) => {
                println!("  Whisper model loaded successfully");
                pipeline.set_stt_engine(Box::new(whisper)).await.unwrap();
            }
            Err(e) => {
                tracing::error!("[E2E-BENCH] Whisper load failed: {e}");
                println!("  WARNING: Whisper model failed to load. STT will be empty.");
                pipeline
                    .set_stt_engine(Box::new(voxy_whisper::WhisperSttEngine::new()))
                    .await
                    .unwrap();
            }
        }
    }
    #[cfg(not(feature = "whisper-engine"))]
    {
        println!("  WARNING: whisper-engine feature disabled");
        pipeline
            .set_stt_engine(Box::new(voxy_whisper::WhisperSttEngine::new()))
            .await
            .unwrap();
    }

    // ── Load Piper TTS model ─────────────────────────────────────────
    #[cfg(feature = "piper-engine")]
    {
        println!("  Loading Piper TTS model: {}...", config.piper_model_path);
        let tts = voxy_kokoro::KokoroTtsEngine::new()
            .with_voice("default")
            .with_speed(0.55)
            .with_pitch(1.0)
            .with_model_path(config.piper_model_path.clone().into());
        match tts.load_model() {
            Ok(()) => {
                println!("  Piper TTS model loaded successfully");
                pipeline.set_tts_engine(Box::new(tts)).await.unwrap();
            }
            Err(e) => {
                tracing::error!("[E2E-BENCH] Piper load failed: {e}");
                println!("  WARNING: Piper model failed to load. TTS will be empty.");
                pipeline
                    .set_tts_engine(Box::new(voxy_kokoro::KokoroTtsEngine::new()))
                    .await
                    .unwrap();
            }
        }
    }
    #[cfg(not(feature = "piper-engine"))]
    {
        println!("  WARNING: piper-engine feature disabled");
        pipeline
            .set_tts_engine(Box::new(voxy_kokoro::KokoroTtsEngine::new()))
            .await
            .unwrap();
    }

    // ── Create LLM provider ──────────────────────────────────────────
    let llm = create_benchmark_llm_provider(&config);
    println!("  LLM provider: {}", llm.name());

    // ── Shared state for timing collection ────────────────────────────
    let timings: Arc<tokio::sync::Mutex<Vec<UtteranceTimings>>> =
        Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let iteration_counter = Arc::new(AtomicUsize::new(0));
    let bench_running = Arc::new(AtomicBool::new(true));

    // ── Timing state for current utterance ────────────────────────────
    let cur_t0 = Arc::new(tokio::sync::Mutex::new(None::<Instant>));
    let cur_t3 = Arc::new(tokio::sync::Mutex::new(None::<Instant>));
    let cur_t5 = Arc::new(tokio::sync::Mutex::new(None::<Instant>));
    let cur_t6 = Arc::new(tokio::sync::Mutex::new(None::<Instant>));
    let cur_t7 = Arc::new(tokio::sync::Mutex::new(None::<Instant>));
    let cur_t8 = Arc::new(tokio::sync::Mutex::new(None::<Instant>));
    let cur_transcript = Arc::new(tokio::sync::Mutex::new(String::new()));
    let cur_llm_len = Arc::new(tokio::sync::Mutex::new(0usize));

    // ── Install streaming response handler with LLM timing ───────────
    {
        let timings_clone = timings.clone();
        let llm_ref: Arc<dyn LlmProvider> = llm.clone();
        let iter_counter = iteration_counter.clone();
        let bench_running_clone = bench_running.clone();
        let cur_t0 = cur_t0.clone();
        let cur_t3 = cur_t3.clone();
        let cur_t5 = cur_t5.clone();
        let cur_t6 = cur_t6.clone();
        let cur_t7 = cur_t7.clone();
        let cur_t8 = cur_t8.clone();
        let cur_transcript = cur_transcript.clone();
        let cur_llm_len = cur_llm_len.clone();

        let streaming_handler = Arc::new(
            move |text: String,
                  tx: tokio::sync::mpsc::Sender<String>|
             -> std::pin::Pin<
                Box<dyn std::future::Future<Output = ()> + Send>,
            > {
                let llm = llm_ref.clone();
                let timings = timings_clone.clone();
                let iter = iter_counter.clone();
                let _bench_running = bench_running_clone.clone();
                let cur_t0 = cur_t0.clone();
                let cur_t3 = cur_t3.clone();
                let cur_t5 = cur_t5.clone();
                let cur_t6 = cur_t6.clone();
                let cur_t7 = cur_t7.clone();
                let cur_t8 = cur_t8.clone();
                let cur_transcript = cur_transcript.clone();
                let cur_llm_len = cur_llm_len.clone();
                Box::pin(async move {
                    let iter_val = iter.load(Ordering::Relaxed);

                    // T5: LLM streaming begins
                    let t5 = Instant::now();
                    *cur_t5.lock().await = Some(t5);
                    tracing::info!(
                        "[E2E-BENCH] Iteration {} T5 LLM streaming begins",
                        iter_val
                    );

                    // Call LLM streaming
                    let (llm_tx, mut llm_rx) =
                        tokio::sync::mpsc::channel::<LlmChunk>(16);

                    let llm_clone = llm.clone();
                    let prompt = format!(
                        "You are a helpful voice assistant named VOXY. Reply concisely in one or two sentences.\n\nUser: {}",
                        text
                    );
                    tokio::spawn(async move {
                        let _ = llm_clone.complete_streaming(&prompt, llm_tx).await;
                    });

                    // T6: Track first token
                    let mut t6_recorded = false;
                    let mut accumulated = String::new();
                    let mut sentence_buffer = String::new();

                    while let Some(chunk) = llm_rx.recv().await {
                        if chunk.done {
                            // Flush remaining text
                            if !sentence_buffer.trim().is_empty() {
                                let remaining = sentence_buffer.clone();
                                if !remaining.is_empty() {
                                    let _ = tx.send(remaining).await;
                                }
                            }
                            break;
                        }

                        if !t6_recorded && !chunk.text.is_empty() {
                            let t6_ms = t5.elapsed().as_secs_f64() * 1000.0;
                            *cur_t6.lock().await = Some(Instant::now());
                            tracing::info!(
                                "[E2E-BENCH] Iteration {} T6 LLM first token: {:.1}ms",
                                iter_val, t6_ms
                            );
                            t6_recorded = true;
                        }

                        sentence_buffer.push_str(&chunk.text);

                        // Split on sentence boundaries: . ! ? \n
                        while let Some(pos) = sentence_buffer
                            .char_indices()
                            .find(|&(_, c)| c == '.' || c == '!' || c == '?' || c == '\n')
                            .map(|(i, _)| i)
                        {
                            let end = pos + 1;
                            let sentence: String = sentence_buffer.drain(..end).collect();
                            let trimmed = sentence.trim().to_string();
                            if !trimmed.is_empty() {
                                tracing::info!(
                                    "[E2E-BENCH] Iteration {} T7 Sentence chunk: '{}'",
                                    iter_val, trimmed
                                );
                                let _ = tx.send(trimmed).await;
                            }
                        }

                        accumulated.push_str(&chunk.text);
                    }

                    // Store LLM response length
                    *cur_llm_len.lock().await = accumulated.len();
                    tracing::info!(
                        "[E2E-BENCH] Iteration {} LLM response: {} chars",
                        iter_val,
                        accumulated.len()
                    );

                    // Collect all timing data for this utterance
                    let t0_instant = cur_t0.lock().await.take();
                    let t3_instant = cur_t3.lock().await.take();
                    let t5_instant = cur_t5.lock().await.take();
                    let t6_instant = cur_t6.lock().await.take();
                    let t7_instant = cur_t7.lock().await.take();
                    let t8_instant = cur_t8.lock().await.take();
                    let transcription = std::mem::take(&mut *cur_transcript.lock().await);
                    let llm_len = *cur_llm_len.lock().await;

                    // Compute stage durations
                    let t0_to_t3 = match (t0_instant, t3_instant) {
                        (Some(start), Some(end)) => {
                            end.duration_since(start).as_secs_f64() * 1000.0
                        }
                        _ => 0.0,
                    };
                    let t3_to_t5 = match (t3_instant, t5_instant) {
                        (Some(start), Some(end)) => {
                            end.duration_since(start).as_secs_f64() * 1000.0
                        }
                        _ => 0.0,
                    };
                    let t5_to_t6 = match (t5_instant, t6_instant) {
                        (Some(start), Some(end)) => {
                            end.duration_since(start).as_secs_f64() * 1000.0
                        }
                        _ => 0.0,
                    };
                    let t6_to_t7 = match (t6_instant, t7_instant) {
                        (Some(start), Some(end)) => {
                            end.duration_since(start).as_secs_f64() * 1000.0
                        }
                        _ => 0.0,
                    };
                    let t7_to_t8 = match (t7_instant, t8_instant) {
                        (Some(start), Some(end)) => {
                            end.duration_since(start).as_secs_f64() * 1000.0
                        }
                        _ => 0.0,
                    };
                    let t0_to_t8_total = match (t0_instant, t8_instant) {
                        (Some(start), Some(end)) => {
                            end.duration_since(start).as_secs_f64() * 1000.0
                        }
                        _ => t0_to_t3 + t3_to_t5 + t5_to_t6 + t6_to_t7 + t7_to_t8,
                    };

                    let utterance = UtteranceTimings {
                        iteration: iter_val,
                        t0_to_t3_ms: t0_to_t3,
                        t3_to_t5_ms: t3_to_t5,
                        t5_to_t6_ms: t5_to_t6,
                        t6_to_t7_ms: t6_to_t7,
                        t7_to_t8_ms: t7_to_t8,
                        t0_to_t8_total_ms: t0_to_t8_total,
                        transcription,
                        llm_response_len: llm_len,
                    };

                    println!(
                        "  [Utterance {}/{}] {:.0}ms total (STT:{:.0} LLM:{:.0} TTS:{:.0})",
                        iter_val,
                        iter.load(Ordering::Relaxed),
                        utterance.t0_to_t8_total_ms,
                        utterance.t0_to_t3_ms,
                        utterance.t5_to_t6_ms + utterance.t6_to_t7_ms,
                        utterance.t7_to_t8_ms,
                    );

                    let mut timings_guard = timings.lock().await;
                    timings_guard.push(utterance);
                })
            },
        );

        pipeline
            .set_streaming_response_handler(streaming_handler)
            .await;
    }

    // ── Start listening ───────────────────────────────────────────────
    pipeline.start_capture().await.unwrap();
    pipeline.start_listening().await.unwrap();

    // Re-register event handler after start_listening (start_listening
    // calls on_event internally via the listening loop which reads it)
    // Actually, looking at the pipeline code, on_event is just stored
    // and called from the listening loop. start_listening doesn't call
    // on_event. So our handler should still be active.
    // But to be safe, re-register it.
    {
        let cur_t0 = cur_t0.clone();
        let cur_t3 = cur_t3.clone();
        let cur_t7 = cur_t7.clone();
        let cur_t8 = cur_t8.clone();
        let cur_transcript = cur_transcript.clone();

        pipeline
            .on_event(Box::new(move |event| {
                let cur_t0 = cur_t0.clone();
                let cur_t3 = cur_t3.clone();
                let cur_t7 = cur_t7.clone();
                let cur_t8 = cur_t8.clone();
                let cur_transcript = cur_transcript.clone();
                tokio::spawn(async move {
                    match event {
                        voxy_voice::VoiceEvent::VoiceActivityStarted => {
                            let mut t0 = cur_t0.lock().await;
                            if t0.is_none() {
                                *t0 = Some(Instant::now());
                            }
                        }
                        voxy_voice::VoiceEvent::TranscriptionResult {
                            text,
                            is_final: true,
                            ..
                        } => {
                            let mut t3 = cur_t3.lock().await;
                            *t3 = Some(Instant::now());
                            let mut t = cur_transcript.lock().await;
                            *t = text;
                        }
                        voxy_voice::VoiceEvent::SynthesisStarted { text: _ } => {
                            let mut t7 = cur_t7.lock().await;
                            *t7 = Some(Instant::now());
                        }
                        voxy_voice::VoiceEvent::SynthesisCompleted { duration_ms: _ } => {
                            let mut t8 = cur_t8.lock().await;
                            *t8 = Some(Instant::now());
                        }
                        _ => {}
                    }
                });
            }))
            .await
            .unwrap();
    }

    println!();
    println!("  Pipeline started. Microphone is active.");
    println!();
    println!("  Say a test phrase when ready. The benchmark will capture");
    println!("  up to {} utterances.", config.iterations);
    println!();
    println!("  Press Ctrl+C to stop early and print results.");
    println!();

    // ── Ctrl+C handler ───────────────────────────────────────────────
    {
        let running = bench_running.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            println!("\n  [Ctrl+C detected — stopping benchmark]");
            running.store(false, Ordering::Relaxed);
        });
    }

    // ── Wait for benchmark to complete ────────────────────────────────
    let bench_start = Instant::now();
    while bench_running.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(200)).await;
        let iter_val = iteration_counter.load(Ordering::Relaxed);
        if iter_val >= config.iterations {
            println!();
            println!(
                "  All {} utterances collected.",
                config.iterations
            );
            break;
        }
    }

    let total_duration = bench_start.elapsed();

    // ── Shutdown pipeline ─────────────────────────────────────────────
    pipeline.stop_listening().await;
    pipeline.stop_capture().await.unwrap_or(());

    println!("  Pipeline stopped.");

    // ── Print Results ─────────────────────────────────────────────────
    let collected = timings.lock().await;
    print_results(&collected, &config, total_duration, &hw);
}

// ─── Print Results ────────────────────────────────────────────────────────

fn print_results(
    timings: &[UtteranceTimings],
    config: &E2eBenchmarkConfig,
    total_duration: Duration,
    hw_info: &[String],
) {
    println!();
    println!("=================================================================");
    println!("  E2E Benchmark Results");
    println!("=================================================================");
    println!();
    println!("  Completed in {:.1}s", total_duration.as_secs_f64());
    println!("  Utterances: {}", timings.len());
    println!();

    if timings.is_empty() {
        println!("  No utterances captured.");
        println!("  Ensure your microphone is working and speak clearly.");
        println!();
        return;
    }

    // Build stats for each timing stage
    let mut stats_t0_t3 = TimingStats::new("T0→T3 STT");
    let mut stats_t3_t5 = TimingStats::new("T3→T5 LLM-prep");
    let mut stats_t5_t6 = TimingStats::new("T5→T6 TTFT");
    let mut stats_t6_t7 = TimingStats::new("T6→T7 LLM-gen");
    let mut stats_t7_t8 = TimingStats::new("T7→T8 TTS");
    let mut stats_total = TimingStats::new("T0→T8 Total");

    for t in timings {
        stats_t0_t3.record(t.t0_to_t3_ms);
        stats_t3_t5.record(t.t3_to_t5_ms);
        stats_t5_t6.record(t.t5_to_t6_ms);
        stats_t6_t7.record(t.t6_to_t7_ms);
        stats_t7_t8.record(t.t7_to_t8_ms);
        stats_total.record(t.t0_to_t8_total_ms);
    }

    println!("  Stage Latency Summary (ms):");
    println!("  ┌──────────────────────┬───────┬────────┬────────┬────────┬────────┬────────┐");
    println!("  │ Stage                │ Count │ Avg    │ Min    │ P50    │ P95    │ Max    │");
    println!("  ├──────────────────────┼───────┼────────┼────────┼────────┼────────┼────────┤");

    for stats in [
        &stats_t0_t3,
        &stats_t3_t5,
        &stats_t5_t6,
        &stats_t6_t7,
        &stats_t7_t8,
        &stats_total,
    ] {
        println!(
            "  │ {:<20} │ {:>5} │ {:>6.1} │ {:>6.1} │ {:>6.1} │ {:>6.1} │ {:>6.1} │",
            stats.name,
            stats.count(),
            stats.avg_ms(),
            stats.min_ms(),
            stats.percentile(50.0),
            stats.percentile(95.0),
            stats.max_ms()
        );
    }
    println!("  └──────────────────────┴───────┴────────┴────────┴────────┴────────┴────────┘");
    println!();

    // Per-utterance detail
    println!("  Per-Utterance Detail:");
    println!("  ┌─────┬──────────┬──────────┬──────────┬──────────┬──────────┬──────────┐");
    println!("  │  #  │ STT      │ Pre-LLM  │ TTFT     │ LLM-gen  │ TTS      │ Total    │");
    println!("  ├─────┼──────────┼──────────┼──────────┼──────────┼──────────┼──────────┤");
    for t in timings {
        println!(
            "  │ {:>3} │ {:>6.1}ms │ {:>6.1}ms │ {:>6.1}ms │ {:>6.1}ms │ {:>6.1}ms │ {:>6.1}ms │",
            t.iteration,
            t.t0_to_t3_ms,
            t.t3_to_t5_ms,
            t.t5_to_t6_ms,
            t.t6_to_t7_ms,
            t.t7_to_t8_ms,
            t.t0_to_t8_total_ms,
        );
    }
    println!("  └─────┴──────────┴──────────┴──────────┴──────────┴──────────┴──────────┘");
    println!();

    // Average breakdown
    let avg_total = stats_total.avg_ms();
    let avg_stt = stats_t0_t3.avg_ms();
    let avg_t5_t6 = stats_t5_t6.avg_ms();
    let avg_t6_t7 = stats_t6_t7.avg_ms();
    let avg_tts = stats_t7_t8.avg_ms();

    println!("  Average Stage Breakdown:");
    if avg_total > 0.0 {
        println!(
            "    T0→T3 STT:      {:>6.1}ms ({:>4.0}%)",
            avg_stt,
            avg_stt / avg_total * 100.0
        );
        println!(
            "    T5→T6 TTFT:     {:>6.1}ms ({:>4.0}%)",
            avg_t5_t6,
            avg_t5_t6 / avg_total * 100.0
        );
        println!(
            "    T6→T7 LLM-gen:  {:>6.1}ms ({:>4.0}%)",
            avg_t6_t7,
            avg_t6_t7 / avg_total * 100.0
        );
        println!(
            "    T7→T8 TTS:      {:>6.1}ms ({:>4.0}%)",
            avg_tts,
            avg_tts / avg_total * 100.0
        );
    }
    println!(
        "    T0→T8 Total:    {:>6.1}ms",
        avg_total
    );
    println!();

    // Bottleneck identification
    let max_stage = [
        ("STT", avg_stt),
        ("TTFT", avg_t5_t6),
        ("LLM-gen", avg_t6_t7),
        ("TTS", avg_tts),
    ]
    .iter()
    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    .map(|(name, _)| *name)
    .unwrap_or("unknown");

    println!("  Bottleneck: {} stage dominates latency", max_stage);
    println!();

    // Transcriptions
    println!("  Transcriptions:");
    for t in timings {
        let transcript = if t.transcription.len() > 60 {
            format!("{}...", &t.transcription[..60])
        } else if t.transcription.is_empty() {
            "(empty)".to_string()
        } else {
            t.transcription.clone()
        };
        println!("    [{:>2}] \"{}\"", t.iteration, transcript);
    }
    println!();

    // Hardware info
    println!("  System Information:");
    for h in hw_info {
        println!("    {}", h);
    }
    println!(
        "    LLM: {} @ {}",
        config.ollama_model, config.ollama_url
    );
    println!("    Whisper: {}", config.whisper_model_path);
    println!("    TTS: {}", config.piper_model_path);
    println!();

    // Full timing detail via pipeline logs
    println!("  Full T0-T8 timing details are logged with [VOICE:TIMING] prefix.");
    println!("  Run with RUST_LOG=info voxy-daemon --e2e-benchmark to see logs.");
    println!();
}

// ─── LLM Provider Factory ────────────────────────────────────────────────

fn create_benchmark_llm_provider(config: &E2eBenchmarkConfig) -> Arc<dyn LlmProvider> {
    let provider = std::env::var("VOXY_LLM_PROVIDER")
        .unwrap_or_else(|_| "ollama".into())
        .to_lowercase();

    match provider.as_str() {
        "openai" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI").unwrap_or_default();
            if api_key.is_empty() {
                tracing::warn!("[E2E-BENCH] VOXY_API_KEYS_OPENAI not set, using Ollama");
                return create_benchmark_ollama_provider(config);
            }
            let model = std::env::var("VOXY_OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".into());
            let config_llm = voxy_openai::OpenAIConfig::openai(api_key);
            Arc::new(
                voxy_openai::OpenAIProvider::new(config_llm).with_model(&model),
            )
        }
        "anthropic" => {
            let api_key = std::env::var("VOXY_API_KEYS_ANTHROPIC").unwrap_or_default();
            if api_key.is_empty() {
                tracing::warn!("[E2E-BENCH] VOXY_API_KEYS_ANTHROPIC not set, using Ollama");
                return create_benchmark_ollama_provider(config);
            }
            let model = std::env::var("VOXY_ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            Arc::new(voxy_anthropic::AnthropicProvider::new(api_key).with_model(&model))
        }
        _ => create_benchmark_ollama_provider(config),
    }
}

fn create_benchmark_ollama_provider(config: &E2eBenchmarkConfig) -> Arc<dyn LlmProvider> {
    let provider = voxy_ollama::OllamaProvider::new(&config.ollama_url, &config.ollama_model)
        .map_err(|e| {
            tracing::warn!("[E2E-BENCH] Failed to create Ollama client: {e}");
            e
        })
        .unwrap_or_else(|_| voxy_ollama::OllamaProvider::default());
    Arc::new(provider)
}
