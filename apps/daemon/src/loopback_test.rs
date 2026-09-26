use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Mutex, Notify};
use tracing::info;
use voxy_audio::{AudioDeviceManager, WasapiDeviceManager};
#[cfg(feature = "piper-engine")]
use voxy_kokoro::KokoroTtsEngine;
use voxy_provider_core::LlmProvider;
use voxy_voice::pipeline::VoicePipeline;
use voxy_voice::VoiceConfig;

struct IterationTimings {
    t_play_start: Option<Instant>,
    t_mic_speech: Option<Instant>,
    t_stt_start: Option<Instant>,
    t_stt_done: Option<Instant>,
    t_llm_start: Option<Instant>,
    t_llm_first_token: Option<Instant>,
    t_first_sentence: Option<Instant>,
    t_tts_start: Option<Instant>,
    t_tts_first_audio: Option<Instant>,
    t_response_done: Option<Instant>,
    stt_text: String,
    llm_response: String,
}

struct TimingSample {
    name: &'static str,
    values: Vec<f64>,
}

impl TimingSample {
    fn new(name: &'static str) -> Self {
        Self { name, values: Vec::new() }
    }
    fn record(&mut self, ms: f64) { self.values.push(ms); }
    fn avg(&self) -> f64 { if self.values.is_empty() { 0.0 } else { self.values.iter().sum::<f64>() / self.values.len() as f64 } }
    fn min(&self) -> f64 { self.values.iter().cloned().fold(f64::MAX, f64::min) }
    fn max(&self) -> f64 { self.values.iter().cloned().fold(f64::MIN, f64::max) }
    fn p50(&self) -> f64 { self.percentile(50.0) }
    fn p95(&self) -> f64 { self.percentile(95.0) }
    fn percentile(&self, p: f64) -> f64 {
        if self.values.is_empty() { return 0.0; }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

pub async fn run_loopback_test(iterations: usize) {
    info!("═══════════════════════════════════════════════════════");
    info!("  VOXY LOOPBACK LATENCY TEST — {} iterations", iterations);
    info!("═══════════════════════════════════════════════════════");
    info!("Test phrase: \"How are you?\"");
    info!("This test plays TTS through speakers, captures via mic,");
    info!("and measures the full pipeline latency.");

    let config = VoiceConfig {
        auto_start_capture: true,
        wake_word_enabled: false,
        vad_enabled: true,
        vad_threshold: 0.05,
        orchestrator: voxy_voice_orchestrator::VoiceOrchestratorConfig {
            silence_timeout_ms: 100,
            ..Default::default()
        },
        ..Default::default()
    };

    let audio_mgr = Box::new(WasapiDeviceManager::new());
    let pipeline = Arc::new(VoicePipeline::with_audio_mgr(config.clone(), audio_mgr));

    if let Err(e) = pipeline.initialize().await {
        tracing::error!("Pipeline init failed: {e}");
        return;
    }
    if let Err(e) = pipeline.with_default_engines().await {
        tracing::error!("Default engines failed: {e}");
        return;
    }

    // Load Whisper STT
    #[cfg(feature = "whisper-engine")]
    {
        let whisper = voxy_whisper::WhisperSttEngine::new()
            .with_model_path("models/ggml-base.en.bin".into());
        match whisper.load_model() {
            Ok(()) => {
                info!("[LOOPBACK] Whisper model loaded");
                if let Err(e) = pipeline.set_stt_engine(Box::new(whisper)).await {
                    tracing::error!("Set STT failed: {e}");
                }
            }
            Err(e) => {
                tracing::error!("[LOOPBACK] Whisper load failed: {e}");
                return;
            }
        }
    }

    // Load Piper TTS
    #[cfg(feature = "piper-engine")]
    {
        let tts = KokoroTtsEngine::new()
            .with_voice("default")
            .with_speed(0.55)
            .with_pitch(1.0)
            .with_model_path("models/en_US-lessac-medium.onnx".into());
        match tts.load_model() {
            Ok(()) => info!("[LOOPBACK] Piper TTS loaded"),
            Err(e) => {
                tracing::error!("[LOOPBACK] Piper load failed: {e}");
                return;
            }
        }
        if let Err(e) = pipeline.set_tts_engine(Box::new(tts)).await {
            tracing::error!("Set TTS failed: {e}");
        }
    }

    // Create LLM provider
    let llm = create_loopback_llm_provider();
    info!("[LOOPBACK] LLM provider: {}", llm.name());

    // Timing state shared across the streaming handler
    let timings: Arc<Mutex<Vec<IterationTimings>>> = Arc::new(Mutex::new(Vec::new()));
    let iteration_done = Arc::new(Notify::new());
    let iteration_active = Arc::new(AtomicBool::new(false));
    let current_llm_response: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));

    // Set up streaming response handler with timing instrumentation
    {
        let llm = llm.clone();
        let timings = timings.clone();
        let iteration_done = iteration_done.clone();
        let iteration_active = iteration_active.clone();
        let current_llm_response = current_llm_response.clone();

        let streaming_handler = Arc::new(
            move |text: String,
                  tx: mpsc::Sender<String>|
             -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
                let llm = llm.clone();
                let timings = timings.clone();
                let iteration_done = iteration_done.clone();
                let iteration_active = iteration_active.clone();
                let current_llm_response = current_llm_response.clone();
                Box::pin(async move {
                    if text.is_empty() || text == "short sound" {
                        let _ = tx.send(String::new()).await;
                        return;
                    }

                    info!("[LOOPBACK] STT recognized: '{}'", text);

                    // Record STT done
                    {
                        let mut t = timings.lock().await;
                        if let Some(current) = t.last_mut() {
                            current.t_stt_done = Some(Instant::now());
                            current.stt_text = text.clone();
                        }
                    }

                    // Build prompt
                    let prompt = format!(
                        "You are a helpful voice assistant named VOXY. Reply concisely in 1-2 sentences.\n\nUser: {text}\nAssistant:"
                    );

                    // Record LLM start
                    {
                        let mut t = timings.lock().await;
                        if let Some(current) = t.last_mut() {
                            current.t_llm_start = Some(Instant::now());
                        }
                    }

                    let (llm_tx, mut llm_rx) = mpsc::channel::<voxy_provider_core::LlmChunk>(16);
                    let llm_clone = llm.clone();
                    let prompt_clone = prompt.clone();
                    tokio::spawn(async move {
                        let _ = llm_clone.complete_streaming(&prompt_clone, llm_tx).await;
                    });

                    let mut accumulated = String::new();
                    let mut sentence_buffer = String::new();
                    let mut first_token_done = false;

                    while let Some(chunk) = llm_rx.recv().await {
                        if chunk.done {
                            if !sentence_buffer.trim().is_empty() {
                                let remaining = sentence_buffer.trim().to_string();
                                if !remaining.is_empty() {
                                    let _ = tx.send(remaining).await;
                                }
                            }
                            break;
                        }

                        if !first_token_done && !chunk.text.is_empty() {
                            first_token_done = true;
                            let mut t = timings.lock().await;
                            if let Some(current) = t.last_mut() {
                                current.t_llm_first_token = Some(Instant::now());
                            }
                        }

                        sentence_buffer.push_str(&chunk.text);

                        while let Some(pos) = sentence_buffer
                            .char_indices()
                            .find(|&(_, c)| c == '.' || c == '!' || c == '?' || c == '\n')
                            .map(|(i, _)| i)
                        {
                            let end = pos + 1;
                            let sentence: String = sentence_buffer.drain(..end).collect();
                            let trimmed = sentence.trim().to_string();
                            if !trimmed.is_empty() {
                                info!("[LOOPBACK] Sentence to TTS: '{}'", trimmed);
                                {
                                    let mut t = timings.lock().await;
                                    if let Some(current) = t.last_mut() {
                                        if current.t_first_sentence.is_none() {
                                            current.t_first_sentence = Some(Instant::now());
                                        }
                                    }
                                }
                                let _ = tx.send(trimmed).await;
                            }
                        }

                        accumulated.push_str(&chunk.text);
                    }

                    info!("[LOOPBACK] LLM full response: '{}'", accumulated);
                    {
                        let mut resp = current_llm_response.lock().await;
                        *resp = accumulated.clone();
                    }

                    // Mark iteration done
                    {
                        let mut t = timings.lock().await;
                        if let Some(current) = t.last_mut() {
                            current.t_response_done = Some(Instant::now());
                            current.llm_response = accumulated;
                        }
                    }
                    iteration_active.store(false, Ordering::Relaxed);
                    iteration_done.notify_one();
                })
            },
        );

        pipeline.set_streaming_response_handler(streaming_handler).await;
    }

    // Start listening
    if let Err(e) = pipeline.start_capture().await {
        tracing::error!("Start capture failed: {e}");
        return;
    }
    if let Err(e) = pipeline.start_listening().await {
        tracing::error!("Start listening failed: {e}");
        return;
    }

    info!("[LOOPBACK] Pipeline started. Beginning test iterations...");
    tokio::time::sleep(Duration::from_secs(2)).await;

    for i in 0..iterations {
        info!("───────────────────────────────────────────────────────");
        info!("  ITERATION {}/{}", i + 1, iterations);
        info!("───────────────────────────────────────────────────────");

        // Record T0 = play start
        {
            let mut t = timings.lock().await;
            t.push(IterationTimings {
                t_play_start: Some(Instant::now()),
                t_mic_speech: None,
                t_stt_start: None,
                t_stt_done: None,
                t_llm_start: None,
                t_llm_first_token: None,
                t_first_sentence: None,
                t_tts_start: None,
                t_tts_first_audio: None,
                t_response_done: None,
                stt_text: String::new(),
                llm_response: String::new(),
            });
        }
        iteration_active.store(true, Ordering::Relaxed);

        // Synthesize and play "How are you?" through a separate output device
        {
            let tts_guard = pipeline.tts_engine_handle().read().await;
            if let Some(ref tts) = *tts_guard {
                match tts.synthesize_stream("How are you?").await {
                    Ok(mut stream) => {
                        info!("[LOOPBACK] TTS stream ready, playing through speaker...");
                        let test_audio_mgr = WasapiDeviceManager::new();
                        match test_audio_mgr.open_output(&config.audio.output).await {
                            Ok(mut test_output) => {
                                let mut first_chunk = true;
                                while let Some(chunk) = stream.next_chunk().await {
                                    if first_chunk {
                                        info!("[LOOPBACK] T0: First TTS audio chunk ready for playback");
                                        first_chunk = false;
                                    }
                                    let pkt = voxy_audio::AudioPacket::new(
                                        chunk.data,
                                        chunk.sample_rate,
                                        config.audio.output.channels,
                                    );
                                    let _ = test_output.write(&pkt).await;
                                }
                                info!("[LOOPBACK] T0: Playback of 'How are you?' complete");
                                let _ = test_output.close().await;
                            }
                            Err(e) => {
                                tracing::error!("[LOOPBACK] Failed to open test output: {e}");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("[LOOPBACK] TTS synthesis failed: {e}");
                        continue;
                    }
                }
            } else {
                tracing::error!("[LOOPBACK] No TTS engine");
                continue;
            }
        }

        // Wait for iteration to complete (LLM response done + TTS playback)
        match tokio::time::timeout(Duration::from_secs(30), iteration_done.notified()).await {
            Ok(()) => info!("[LOOPBACK] Iteration {} completed", i + 1),
            Err(_) => {
                tracing::warn!("[LOOPBACK] Iteration {} timed out after 30s", i + 1);
                iteration_active.store(false, Ordering::Relaxed);
            }
        }

        // Additional wait for response TTS to finish playing
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    // Shutdown pipeline
    let _ = pipeline.stop_listening().await;
    let _ = pipeline.stop_capture().await;
    let _ = pipeline.shutdown().await;

    // Print results
    let all_timings = timings.lock().await;
    print_results(&all_timings);
}

fn print_results(all_timings: &[IterationTimings]) {
    let mut stt_latencies = TimingSample::new("STT");
    let mut llm_ttfts = TimingSample::new("LLM TTFT");
    let mut first_sentence_latencies = TimingSample::new("First Sentence");
    let mut tts_latencies = TimingSample::new("TTS First Audio");
    let mut ttfa_latencies = TimingSample::new("TTFA");
    let mut total_latencies = TimingSample::new("Total");

    println!();
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("  VOXY LOOPBACK LATENCY TEST — RESULTS");
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!();
    println!("  Test phrase: \"How are you?\"");
    println!("  Iterations: {}", all_timings.len());
    println!();

    println!("  {:>4} │ {:>8} │ {:>10} │ {:>12} │ {:>8} │ {:>8} │ {:>8} │ {:>6}",
        "RUN", "STT", "LLM TTFT", "First Sent", "TTS 1st", "TTFA", "Total", "Recognized");
    println!("  {:─>4}─┼─{:─>8}─┼─{:─>10}─┼─{:─>12}─┼─{:─>8}─┼─{:─>8}─┼─{:─>8}─┼─{:─>6}",
        "────", "────────", "──────────", "────────────", "────────", "────────", "────────", "──────");

    for (i, t) in all_timings.iter().enumerate() {
        let stt_ms = match (t.t_stt_start, t.t_stt_done) {
            (Some(s), Some(e)) => e.duration_since(s).as_secs_f64() * 1000.0,
            _ => 0.0,
        };
        let llm_ttft_ms = match (t.t_llm_start, t.t_llm_first_token) {
            (Some(s), Some(e)) => e.duration_since(s).as_secs_f64() * 1000.0,
            _ => 0.0,
        };
        let first_sent_ms = match (t.t_llm_start, t.t_first_sentence) {
            (Some(s), Some(e)) => e.duration_since(s).as_secs_f64() * 1000.0,
            _ => 0.0,
        };
        let tts_ms = match (t.t_tts_start, t.t_tts_first_audio) {
            (Some(s), Some(e)) => e.duration_since(s).as_secs_f64() * 1000.0,
            _ => 0.0,
        };
        let ttfa_ms = match (t.t_play_start, t.t_tts_first_audio) {
            (Some(s), Some(e)) => e.duration_since(s).as_secs_f64() * 1000.0,
            _ => 0.0,
        };
        let total_ms = match (t.t_play_start, t.t_response_done) {
            (Some(s), Some(e)) => e.duration_since(s).as_secs_f64() * 1000.0,
            _ => 0.0,
        };

        stt_latencies.record(stt_ms);
        llm_ttfts.record(llm_ttft_ms);
        first_sentence_latencies.record(first_sent_ms);
        tts_latencies.record(tts_ms);
        ttfa_latencies.record(ttfa_ms);
        total_latencies.record(total_ms);

        let stt_display = if t.stt_text.is_empty() { "?".to_string() } else { t.stt_text.chars().take(6).collect() };

        println!("  {:>4} │ {:>7.1}ms │ {:>9.1}ms │ {:>11.1}ms │ {:>7.1}ms │ {:>7.1}ms │ {:>7.1}ms │ {:>6}",
            i + 1, stt_ms, llm_ttft_ms, first_sent_ms, tts_ms, ttfa_ms, total_ms, stt_display);
    }

    println!();
    println!("  ── Summary ─────────────────────────────────────────────────────────────────");
    println!();
    println!("  {:>20} │ {:>8} │ {:>8} │ {:>8} │ {:>8} │ {:>8}",
        "Metric", "Avg", "Min", "Max", "P50", "P95");
    println!("  {:─>20}─┼─{:─>8}─┼─{:─>8}─┼─{:─>8}─┼─{:─>8}─┼─{:─>8}",
        "────────────────────", "────────", "────────", "────────", "────────", "────────");

    for sample in &[&stt_latencies, &llm_ttfts, &first_sentence_latencies, &tts_latencies, &ttfa_latencies, &total_latencies] {
        println!("  {:>20} │ {:>7.1}ms │ {:>7.1}ms │ {:>7.1}ms │ {:>7.1}ms │ {:>7.1}ms",
            sample.name, sample.avg(), sample.min(), sample.max(), sample.p50(), sample.p95());
    }

    println!();

    // Identify bottleneck
    let mut bottlenecks: Vec<(&str, f64)> = vec![
        ("STT", stt_latencies.avg()),
        ("LLM TTFT", llm_ttfts.avg()),
        ("TTS First Audio", tts_latencies.avg()),
    ];
    bottlenecks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((name, ms)) = bottlenecks.first() {
        println!("  LARGEST BOTTLENECK: {} at {:.1}ms avg", name, ms);
    }

    // Transcription accuracy check
    let mut recognized = Vec::new();
    for t in all_timings.iter() {
        if !t.stt_text.is_empty() {
            recognized.push(t.stt_text.clone());
        }
    }
    if !recognized.is_empty() {
        println!();
        println!("  TRANSCRIPTION RESULTS:");
        for (i, text) in recognized.iter().enumerate() {
            println!("    Run {}: \"{}\"", i + 1, text);
        }
    }

    println!();
    println!("═══════════════════════════════════════════════════════════════════════════════");
}

fn create_loopback_llm_provider() -> Arc<dyn LlmProvider> {
    let provider = std::env::var("VOXY_LLM_PROVIDER")
        .unwrap_or_else(|_| "groq".into())
        .to_lowercase();

    match provider.as_str() {
        "groq" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI").unwrap_or_default();
            let model = std::env::var("VOXY_GROQ_MODEL")
                .unwrap_or_else(|_| "llama-3.3-70b-versatile".into());
            if api_key.is_empty() {
                tracing::warn!("[LOOPBACK] No API key, falling back to Ollama");
                return create_loopback_ollama();
            }
            info!("[LOOPBACK] Using Groq: model={}", model);
            Arc::new(voxy_openai::OpenAIProvider::new(
                voxy_openai::OpenAIConfig::groq(api_key),
            ).with_model(&model))
        }
        "ollama" => create_loopback_ollama(),
        "openai" => {
            let api_key = std::env::var("VOXY_API_KEYS_OPENAI").unwrap_or_default();
            let model = std::env::var("VOXY_OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4o-mini".into());
            if api_key.is_empty() { return create_loopback_ollama(); }
            info!("[LOOPBACK] Using OpenAI: model={}", model);
            Arc::new(voxy_openai::OpenAIProvider::new(
                voxy_openai::OpenAIConfig::openai(api_key),
            ).with_model(&model))
        }
        "anthropic" => {
            let api_key = std::env::var("VOXY_API_KEYS_ANTHROPIC").unwrap_or_default();
            let model = std::env::var("VOXY_ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
            if api_key.is_empty() { return create_loopback_ollama(); }
            info!("[LOOPBACK] Using Anthropic: model={}", model);
            Arc::new(voxy_anthropic::AnthropicProvider::new(api_key).with_model(&model))
        }
        "gemini" => {
            let api_key = std::env::var("VOXY_API_KEYS_GEMINI").unwrap_or_default();
            let model = std::env::var("VOXY_GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-2.0-flash".into());
            if api_key.is_empty() { return create_loopback_ollama(); }
            info!("[LOOPBACK] Using Gemini: model={}", model);
            Arc::new(voxy_gemini::GeminiProvider::new(api_key).with_model(&model))
        }
        _ => create_loopback_ollama(),
    }
}

fn create_loopback_ollama() -> Arc<dyn LlmProvider> {
    let url = std::env::var("VOXY_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let model = std::env::var("VOXY_OLLAMA_MODEL")
        .unwrap_or_else(|_| "voxy-fast:latest".into());
    info!("[LOOPBACK] Using Ollama: model={} @ {}", model, url);
    Arc::new(
        voxy_ollama::OllamaProvider::new(&url, &model)
            .map_err(|e| tracing::warn!("Ollama init: {e}"))
            .unwrap_or_else(|_| voxy_ollama::OllamaProvider::default()),
    )
}
