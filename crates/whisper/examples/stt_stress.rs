// STT Stress Test â€” Phase 26
// Tests Whisper engine under various concurrency configurations
// Run with: cargo run --example stt_stress --features whisper-engine -- <THREADS> <MODE>
// MODE: partial-only | final-only | sequential | concurrent | rapid

use chrono::Utc;
use std::env;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use voxy_voice_orchestrator::{AudioChunk, SttEngine};
use voxy_whisper::WhisperSttEngine;

// Simple deterministic pseudo-random to avoid extra deps
fn pseudo_random(seed: &mut u64) -> f32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed as f32) / (u64::MAX as f32)
}

const MODEL_PATH: &str = "models/ggml-base.en.bin";

fn model_path() -> String {
    std::env::var("VOXY_WHISPER_MODEL").unwrap_or_else(|_| MODEL_PATH.to_string())
}
const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u8 = 1;

fn generate_speech_audio(duration_ms: u32, seed: &mut u64) -> Vec<f32> {
    // Generate synthetic speech-like audio (tone + noise)
    let samples = (SAMPLE_RATE as f32 * duration_ms as f32 / 1000.0) as usize;
    let mut audio = Vec::with_capacity(samples);
    for i in 0..samples {
        let t = i as f32 / SAMPLE_RATE as f32;
        // Fundamental ~200Hz + harmonics + noise
        let noise = (pseudo_random(seed) - 0.5) * 0.05;
        let signal = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.3
            + (2.0 * std::f32::consts::PI * 400.0 * t).sin() * 0.15
            + (2.0 * std::f32::consts::PI * 800.0 * t).sin() * 0.07
            + noise;
        audio.push(signal);
    }
    audio
}

fn make_chunk(data: Vec<f32>, is_final: bool) -> AudioChunk {
    AudioChunk {
        data,
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
        timestamp: Utc::now(),
        sequence: 0,
        is_final,
    }
}

async fn test_partial_only(engine: Arc<WhisperSttEngine>, iterations: usize) -> Vec<u128> {
    println!("Testing PARTIAL-only ({} iterations)...", iterations);
    let mut latencies = Vec::new();
    let mut seed = 0x123456789ABCDEFu64;
    for i in 0..iterations {
        let audio = generate_speech_audio(500, &mut seed); // 500ms chunks
        let chunk = make_chunk(audio, false);
        let start = Instant::now();
        match engine.transcribe_partial(&chunk).await {
            Ok(_) => latencies.push(start.elapsed().as_millis()),
            Err(e) => eprintln!("Partial error: {}", e),
        }
        if i % 10 == 0 {
            println!("  partial {}/{}", i, iterations);
        }
    }
    latencies
}

async fn test_final_only(engine: Arc<WhisperSttEngine>, iterations: usize) -> Vec<u128> {
    println!("Testing FINAL-only ({} iterations)...", iterations);
    let mut latencies = Vec::new();
    let mut seed = 0x123456789ABCDEFu64;
    for i in 0..iterations {
        let audio = generate_speech_audio(1500, &mut seed); // 1.5s utterances
        let chunk = make_chunk(audio, true);
        let start = Instant::now();
        match engine.transcribe(&chunk).await {
            Ok(_) => latencies.push(start.elapsed().as_millis()),
            Err(e) => eprintln!("Final error: {}", e),
        }
        if i % 10 == 0 {
            println!("  final {}/{}", i, iterations);
        }
    }
    latencies
}

async fn test_sequential(
    engine: Arc<WhisperSttEngine>,
    iterations: usize,
) -> (Vec<u128>, Vec<u128>) {
    println!(
        "Testing SEQUENTIAL (partialâ†’final, {} iterations)...",
        iterations
    );
    let mut partial_latencies = Vec::new();
    let mut final_latencies = Vec::new();
    let mut seed = 0x123456789ABCDEFu64;
    for i in 0..iterations {
        // Simulate partials during utterance
        for _ in 0..3 {
            let audio = generate_speech_audio(500, &mut seed);
            let chunk = make_chunk(audio, false);
            let start = Instant::now();
            let _ = engine.transcribe_partial(&chunk).await;
            partial_latencies.push(start.elapsed().as_millis());
        }
        // Final
        let audio = generate_speech_audio(1500, &mut seed);
        let chunk = make_chunk(audio, true);
        let start = Instant::now();
        let _ = engine.transcribe(&chunk).await;
        final_latencies.push(start.elapsed().as_millis());
        if i % 10 == 0 {
            println!("  sequential {}/{}", i, iterations);
        }
    }
    (partial_latencies, final_latencies)
}

async fn test_concurrent(
    engine: Arc<WhisperSttEngine>,
    iterations: usize,
    concurrent_partials: usize,
) -> (Vec<u128>, Vec<u128>) {
    println!(
        "Testing CONCURRENT (final + {} partials, {} iterations)...",
        concurrent_partials, iterations
    );
    let mut partial_latencies = Vec::new();
    let mut final_latencies = Vec::new();
    let semaphore = Arc::new(Semaphore::new(concurrent_partials));

    for i in 0..iterations {
        let engine_clone = engine.clone();
        let sem = semaphore.clone();
        let mut seed = 0x123456789ABCDEFu64.wrapping_add(i as u64 * 1000);

        // Spawn concurrent partial decoders
        let partial_handles: Vec<_> = (0..concurrent_partials)
            .map(|_| {
                let engine = engine_clone.clone();
                let sem = sem.clone();
                let mut s = seed;
                seed = seed.wrapping_add(0x9E3779B97F4A7C15);
                tokio::spawn(async move {
                    let _permit = sem.acquire().await.unwrap();
                    let audio = generate_speech_audio(500, &mut s);
                    let chunk = make_chunk(audio, false);
                    let start = Instant::now();
                    let _ = engine.transcribe_partial(&chunk).await;
                    start.elapsed().as_millis()
                })
            })
            .collect();

        // Start final decode concurrently
        let audio = generate_speech_audio(1500, &mut seed);
        let chunk = make_chunk(audio, true);
        let final_start = Instant::now();
        let final_result = engine.transcribe(&chunk).await;
        let final_latency = final_start.elapsed().as_millis();
        final_latencies.push(final_latency);

        // Wait for partials
        for h in partial_handles {
            if let Ok(lat) = h.await {
                partial_latencies.push(lat);
            }
        }

        if final_result.is_err() {
            eprintln!("Final decode error at iteration {}", i);
        }
        if i % 5 == 0 {
            println!(
                "  concurrent {}/{} (final: {}ms)",
                i, iterations, final_latency
            );
        }
    }
    (partial_latencies, final_latencies)
}

async fn test_rapid_utterances(engine: Arc<WhisperSttEngine>, iterations: usize) -> Vec<u128> {
    println!(
        "Testing RAPID utterances ({} iterations, no gap)...",
        iterations
    );
    let mut latencies = Vec::new();
    let mut seed = 0x123456789ABCDEFu64;
    for i in 0..iterations {
        let audio = generate_speech_audio(1000, &mut seed);
        let chunk = make_chunk(audio, true);
        let start = Instant::now();
        let _ = engine.transcribe(&chunk).await;
        latencies.push(start.elapsed().as_millis());
        if i % 20 == 0 {
            println!("  rapid {}/{}", i, iterations);
        }
    }
    latencies
}

fn print_stats(name: &str, latencies: &[u128]) {
    if latencies.is_empty() {
        println!("{}: no data", name);
        return;
    }
    let mut sorted = latencies.to_vec();
    sorted.sort();
    let n = sorted.len();
    let sum: u128 = sorted.iter().sum();
    let avg = sum / n as u128;
    let min = sorted[0];
    let max = sorted[n - 1];
    let p50 = sorted[n / 2];
    let p95 = sorted[n * 95 / 100];
    let p99 = sorted[n * 99 / 100];
    println!(
        "{}: n={} min={}ms avg={}ms p50={}ms p95={}ms p99={}ms max={}ms",
        name, n, min, avg, p50, p95, p99, max
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let args: Vec<String> = env::args().collect();
    let threads = args
        .get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(8);
    let mode = args.get(2).map(|s| s.as_str()).unwrap_or("concurrent");
    let iterations = args
        .get(3)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(30);

    println!("=== STT Stress Test ===");
    println!("Threads: {}", threads);
    println!("Mode: {}", mode);
    println!("Iterations: {}", iterations);
    println!("Model: {}", model_path());

    env::set_var("VOXY_WHISPER_THREADS", threads.to_string());

    let engine = Arc::new(
        WhisperSttEngine::new()
            .with_model_path(model_path().into())
            .with_language("en"),
    );
    engine.load_model().expect("Failed to load model");
    engine.warmup().expect("Warmup failed");

    println!("Model loaded, starting tests...\n");

    let (partial_latencies, final_latencies) = match mode {
        "partial-only" => {
            let l = test_partial_only(engine.clone(), iterations).await;
            print_stats("Partial", &l);
            (l, vec![])
        }
        "final-only" => {
            let l = test_final_only(engine.clone(), iterations).await;
            print_stats("Final", &l);
            (vec![], l)
        }
        "sequential" => {
            let (p, f) = test_sequential(engine.clone(), iterations).await;
            print_stats("Partial (seq)", &p);
            print_stats("Final (seq)", &f);
            (p, f)
        }
        "concurrent" => {
            let (p, f) = test_concurrent(engine.clone(), iterations, 3).await;
            print_stats("Partial (concurrent)", &p);
            print_stats("Final (concurrent)", &f);
            (p, f)
        }
        "rapid" => {
            let l = test_rapid_utterances(engine.clone(), iterations).await;
            print_stats("Rapid final", &l);
            (vec![], l)
        }
        _ => {
            eprintln!("Unknown mode: {}", mode);
            std::process::exit(1);
        }
    };

    // Summary
    println!("\n=== SUMMARY ===");
    println!("VOXY_WHISPER_THREADS={}", threads);
    println!("Mode: {}", mode);
    if !partial_latencies.is_empty() {
        print_stats("Partial", &partial_latencies);
    }
    if !final_latencies.is_empty() {
        print_stats("Final", &final_latencies);
    }

    Ok(())
}
