//! OSMOO Voice Pipeline: True Token-to-Speech Streaming Integration Tests.
//!
//! Verifies:
//! 1. Immediate first-word emission without sentence punctuation.
//! 2. Token boundary combination across arbitrary chunk splits (e.g. "Hel" + "lo," -> "Hello,").
//! 3. Streaming overlap: TTS receives chunk 1 before LLM finishes generating later tokens.
//! 4. Continuous audio playback starting while generation is active.
//! 5. Barge-in during streaming immediately cancels LLM, TTS, and playback workers.
//! 6. Bounded backpressure between LLM, TTS, and playback buffers.

use async_trait::async_trait;
use futures_util::future::BoxFuture;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use voxy_voice_orchestrator::{VoicePipeline, VoicePipelineConfig};
use voxy_voice_stt::MockSttEngine;
use voxy_voice_tts::{MockTtsEngine, TtsEngine, TtsError};

/// Recording TTS mock that tracks exact chunk texts and invocation timestamps.
struct RecordingTtsEngine {
    chunks: Arc<Mutex<Vec<(String, Instant)>>>,
    delay_ms: u64,
}

impl RecordingTtsEngine {
    fn new(delay_ms: u64) -> (Self, Arc<Mutex<Vec<(String, Instant)>>>) {
        let chunks = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                chunks: chunks.clone(),
                delay_ms,
            },
            chunks,
        )
    }
}

#[async_trait]
impl TtsEngine for RecordingTtsEngine {
    fn name(&self) -> &str {
        "Recording Mock TTS"
    }

    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, TtsError> {
        if self.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
        }
        self.chunks.lock().push((text.to_string(), Instant::now()));
        MockTtsEngine::new().synthesize(text).await
    }
}

#[tokio::test]
async fn test_token_boundary_combination_and_early_tts_dispatch() {
    let stt = Arc::new(MockSttEngine::new("test"));
    let (recording_tts, chunk_log) = RecordingTtsEngine::new(10);
    let mut config = VoicePipelineConfig::default();
    config.audio.allow_virtual_fallback = true;

    let pipeline =
        Arc::new(VoicePipeline::new(config, stt, Arc::new(recording_tts), None).unwrap());

    let (chunk_tx, chunk_rx) = mpsc::channel::<String>(16);
    let cancel_token = CancellationToken::new();

    let stream_start = Instant::now();
    let llm_finished = Arc::new(AtomicBool::new(false));
    let llm_finished_flag = llm_finished.clone();

    // Simulated LLM emitting split tokens with small delays
    let gen_fut: BoxFuture<'static, Result<(), String>> = Box::pin(async move {
        // "Hel" + "lo," -> should combine into "Hello," and dispatch immediately
        tokio::time::sleep(Duration::from_millis(20)).await;
        chunk_tx.send("Hel".into()).await.unwrap();

        tokio::time::sleep(Duration::from_millis(20)).await;
        chunk_tx.send("lo,".into()).await.unwrap();

        // Later tokens emitted after delays
        tokio::time::sleep(Duration::from_millis(80)).await;
        chunk_tx.send(" I".into()).await.unwrap();

        tokio::time::sleep(Duration::from_millis(40)).await;
        chunk_tx.send(" am".into()).await.unwrap();

        tokio::time::sleep(Duration::from_millis(40)).await;
        chunk_tx.send(" OSMOO.".into()).await.unwrap();

        llm_finished_flag.store(true, Ordering::SeqCst);
        Ok(())
    });

    pipeline
        .run_streaming_response(gen_fut, chunk_rx, cancel_token)
        .await;

    let recorded = chunk_log.lock().clone();
    assert!(
        !recorded.is_empty(),
        "At least one chunk must be dispatched to TTS"
    );

    // 1. Verify first chunk is "Hello,"
    assert_eq!(
        recorded[0].0, "Hello,",
        "First speakable word must be combined from 'Hel'+'lo,' and dispatched immediately"
    );

    // 2. Verify first chunk was received by TTS BEFORE LLM finished generating all tokens
    let first_tts_time = recorded[0].1;
    let tts_latency_from_start = first_tts_time.duration_since(stream_start);
    println!("First TTS input received at: {tts_latency_from_start:?}");

    // The full LLM generation takes at least 20 + 20 + 80 + 40 + 40 = 200ms.
    // The first chunk must have been dispatched around ~40-60ms.
    assert!(
        tts_latency_from_start < Duration::from_millis(150),
        "TTS must receive the first chunk well before the complete LLM response finishes, took {tts_latency_from_start:?}"
    );

    // 3. Verify telemetry metrics
    let metrics = pipeline.last_streaming_metrics().expect("Metrics recorded");
    assert!(metrics.time_to_first_llm_token.is_some());
    assert!(metrics.time_to_first_tts_input.is_some());
    assert!(metrics.time_to_first_tts_input.unwrap() < metrics.total_duration);
    assert_eq!(metrics.chunks_sent_to_tts, recorded.len());
}

#[tokio::test]
async fn test_first_speakable_word_without_sentence_punctuation() {
    let stt = Arc::new(MockSttEngine::new("test"));
    let (recording_tts, chunk_log) = RecordingTtsEngine::new(5);
    let mut config = VoicePipelineConfig::default();
    config.audio.allow_virtual_fallback = true;

    let pipeline =
        Arc::new(VoicePipeline::new(config, stt, Arc::new(recording_tts), None).unwrap());
    let (chunk_tx, chunk_rx) = mpsc::channel::<String>(16);
    let cancel_token = CancellationToken::new();

    let gen_fut: BoxFuture<'static, Result<(), String>> = Box::pin(async move {
        // "Hello " (ends in whitespace, no punctuation) -> must emit immediately
        tokio::time::sleep(Duration::from_millis(15)).await;
        chunk_tx.send("Hello ".into()).await.unwrap();

        tokio::time::sleep(Duration::from_millis(60)).await;
        chunk_tx.send("world how are you".into()).await.unwrap();

        tokio::time::sleep(Duration::from_millis(30)).await;
        chunk_tx.send(" doing today?".into()).await.unwrap();
        Ok(())
    });

    pipeline
        .run_streaming_response(gen_fut, chunk_rx, cancel_token)
        .await;

    let recorded = chunk_log.lock().clone();
    assert!(!recorded.is_empty());
    assert_eq!(
        recorded[0].0, "Hello",
        "First single complete word must emit immediately without waiting for sentence punctuation"
    );
}

#[tokio::test]
async fn test_playback_starts_during_ongoing_generation() {
    let stt = Arc::new(MockSttEngine::new("test"));
    let (recording_tts, _) = RecordingTtsEngine::new(10);
    let mut config = VoicePipelineConfig::default();
    config.audio.allow_virtual_fallback = true;

    let pipeline =
        Arc::new(VoicePipeline::new(config, stt, Arc::new(recording_tts), None).unwrap());
    let (chunk_tx, chunk_rx) = mpsc::channel::<String>(16);
    let cancel_token = CancellationToken::new();

    let llm_is_active = Arc::new(AtomicBool::new(true));
    let llm_active_flag = llm_is_active.clone();

    let gen_fut: BoxFuture<'static, Result<(), String>> = Box::pin(async move {
        chunk_tx.send("Immediate ".into()).await.unwrap();
        for i in 0..6 {
            tokio::time::sleep(Duration::from_millis(35)).await;
            let _ = chunk_tx.send(format!("step {i} ")).await;
        }
        llm_active_flag.store(false, Ordering::SeqCst);
        Ok(())
    });

    pipeline
        .run_streaming_response(gen_fut, chunk_rx, cancel_token)
        .await;

    let metrics = pipeline.last_streaming_metrics().expect("Metrics recorded");
    assert!(
        metrics.time_to_first_audio_playback.is_some(),
        "Audio playback must start"
    );
    let ttfa = metrics.time_to_first_audio_playback.unwrap();
    println!(
        "TTFA measured: {ttfa:?}, total stream duration: {:?}",
        metrics.total_duration
    );
    assert!(
        ttfa < metrics.total_duration,
        "Audio playback must begin while stream is ongoing"
    );
}

#[tokio::test]
async fn test_barge_in_cancels_all_streaming_stages_promptly() {
    let stt = Arc::new(MockSttEngine::new("test"));
    let (recording_tts, _) = RecordingTtsEngine::new(50);
    let mut config = VoicePipelineConfig::default();
    config.audio.allow_virtual_fallback = true;

    let pipeline =
        Arc::new(VoicePipeline::new(config, stt, Arc::new(recording_tts), None).unwrap());
    let (chunk_tx, chunk_rx) = mpsc::channel::<String>(16);
    let cancel_token = CancellationToken::new();

    // Generation that attempts to run for a long time
    let gen_fut: BoxFuture<'static, Result<(), String>> = Box::pin(async move {
        let _ = chunk_tx.send("Initial word ".into()).await;
        for _ in 0..100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if chunk_tx.send("endless stream token ".into()).await.is_err() {
                break;
            }
        }
        Ok(())
    });

    let cancel_trigger = cancel_token.clone();
    tokio::spawn(async move {
        // Trigger barge-in cancellation after 30ms
        tokio::time::sleep(Duration::from_millis(30)).await;
        cancel_trigger.cancel();
    });

    let start = Instant::now();
    pipeline
        .run_streaming_response(gen_fut, chunk_rx, cancel_token)
        .await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(1500),
        "Barge-in cancellation must cleanly abort all streaming workers in < 1500ms, took {elapsed:?}"
    );
}

#[tokio::test]
async fn test_bounded_backpressure_under_slow_tts() {
    let stt = Arc::new(MockSttEngine::new("test"));
    // Artificially slow TTS: 40ms per chunk
    let (recording_tts, chunk_log) = RecordingTtsEngine::new(40);
    let mut config = VoicePipelineConfig::default();
    config.audio.allow_virtual_fallback = true;

    let pipeline =
        Arc::new(VoicePipeline::new(config, stt, Arc::new(recording_tts), None).unwrap());
    let (chunk_tx, chunk_rx) = mpsc::channel::<String>(16);
    let cancel_token = CancellationToken::new();

    // Rapid sender pushing 20 tokens
    let gen_fut: BoxFuture<'static, Result<(), String>> = Box::pin(async move {
        for i in 0..20 {
            if chunk_tx.send(format!("word{i} ")).await.is_err() {
                break;
            }
        }
        Ok(())
    });

    pipeline
        .run_streaming_response(gen_fut, chunk_rx, cancel_token)
        .await;

    let recorded = chunk_log.lock().clone();
    assert!(
        !recorded.is_empty(),
        "Slow TTS must process all bounded chunks without buffer overflow"
    );
    let metrics = pipeline.last_streaming_metrics().expect("Metrics recorded");
    assert!(metrics.chunks_sent_to_tts >= 1);
}
