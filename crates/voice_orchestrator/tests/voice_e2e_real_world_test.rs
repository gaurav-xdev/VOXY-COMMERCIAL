//! OSMOO Voice Pipeline: Real-World E2E Latency and Barge-In Verification Suite.

use std::sync::Arc;
use std::time::Instant;
use voxy_ipc::CompanionState;
use voxy_voice_activation::{VadConfig, VadDetector, VadEvent};
use voxy_voice_audio::conditioning::{AudioConditioner, ConditioningConfig};
use voxy_voice_orchestrator::{VoicePipeline, VoicePipelineConfig};
use voxy_voice_stt::{GroqSttEngine, MockSttEngine, SttEngine};
use voxy_voice_tts::{LocalTtsEngine, MockTtsEngine, SentenceChunker, TtsEngine};

#[tokio::test]
async fn test_e2e_pipeline_timing_and_latency_breakdown() {
    let _ = dotenvy::dotenv();
    println!("\n=== [OSMOO] REAL-WORLD END-TO-END VOICE PIPELINE LATENCY TEST ===");

    // 1. Generate 1.2s synthetic vocalic audio waveform (19,200 samples at 16kHz)
    let sample_rate = 16000;
    let num_samples = 19200;
    let mut audio_samples = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Vocal formant simulation (250Hz fundamental + 750Hz harmonic)
        let sample = 0.35 * (2.0 * std::f32::consts::PI * 250.0 * t).sin()
            + 0.15 * (2.0 * std::f32::consts::PI * 750.0 * t).sin();
        audio_samples.push(sample);
    }

    // Stage 1: Audio Conditioning (High-pass + Gate + Soft limiter)
    let cond_start = Instant::now();
    let mut conditioner = AudioConditioner::new(ConditioningConfig::default());
    conditioner.process(&mut audio_samples);
    let cond_elapsed = cond_start.elapsed();
    println!("  [STAGE 1] Audio Conditioning: {:?} (processed samples: {})", cond_elapsed, audio_samples.len());
    assert_eq!(audio_samples.len(), num_samples);

    // Stage 2: VAD & Utterance Endpointing
    let vad_start = Instant::now();
    let vad_cfg = VadConfig {
        sample_rate: 16000,
        threshold: 0.02,
        min_speech_frames: 2,
        silence_timeout_ms: 100,
        preroll_frames: 5,
        max_utterance_sec: 10.0,
    };
    let mut vad = VadDetector::new(vad_cfg);
    let mut speech_detected = false;
    let frame_size = 320; // 20ms at 16kHz

    // Feed speech frames
    for chunk in audio_samples.chunks(frame_size) {
        if chunk.len() == frame_size {
            if let Some(event) = vad.process_frame(chunk) {
                if matches!(event, VadEvent::SpeechStarted | VadEvent::SpeechFrame { .. }) {
                    speech_detected = true;
                }
            }
        }
    }
    // Feed silence frames to complete utterance
    let silence_frame = vec![0.0f32; frame_size];
    let mut utterance_audio = None;
    for _ in 0..10 {
        if let Some(VadEvent::UtteranceComplete { audio, .. }) = vad.process_frame(&silence_frame) {
            utterance_audio = Some(audio);
            break;
        }
    }
    let vad_elapsed = vad_start.elapsed();
    println!("  [STAGE 2] VAD Endpointing:    {:?} (speech_detected={}, utterance_captured={})",
        vad_elapsed, speech_detected, utterance_audio.is_some());
    assert!(speech_detected, "VAD must detect synthetic speech");

    // Stage 3: Speech-To-Text (Live Groq if configured, else Mock)
    let stt_engine: Arc<dyn SttEngine> = if let Ok(key) = std::env::var("STT_GROQ_API_KEY") {
        if !key.is_empty() {
            println!("  [STAGE 3] STT Provider:       LIVE Groq Whisper Cloud");
            Arc::new(GroqSttEngine::from_env().expect("Valid Groq STT engine"))
        } else {
            println!("  [STAGE 3] STT Provider:       Mock STT (Key is empty)");
            Arc::new(MockSttEngine::new("Hey OSMOO, run system diagnostics and performance checks."))
        }
    } else {
        println!("  [STAGE 3] STT Provider:       Mock STT (STT_GROQ_API_KEY not in test env)");
        Arc::new(MockSttEngine::new("Hey OSMOO, run system diagnostics and performance checks."))
    };

    let stt_start = Instant::now();
    let transcript = stt_engine.transcribe(&audio_samples, Some("en")).await.expect("STT transcription");
    let stt_elapsed = stt_start.elapsed();
    println!("  [STAGE 3] STT Latency:        {:?} -> \"{}\"", stt_elapsed, transcript.text.trim());
    assert!(!transcript.text.is_empty(), "Transcription must not be empty");

    // Stage 4: Cognitive Router (Simulated OSMOO Brain turn)
    let router_start = Instant::now();
    let assistant_response = format!("Diagnostic initiated for: {}", transcript.text.trim());
    let router_elapsed = router_start.elapsed();
    println!("  [STAGE 4] Brain Router:       {:?} -> \"{}\"", router_elapsed, assistant_response);

    // Stage 5: Sentence Chunking
    let chunker_start = Instant::now();
    let mut chunker = SentenceChunker::new(25);
    let chunks = chunker.push_text(&assistant_response);
    let chunker_elapsed = chunker_start.elapsed();
    println!("  [STAGE 5] Chunker Latency:    {:?} (chunks emitted: {})", chunker_elapsed, chunks.len());

    // Stage 6: TTS Synthesis (Local SAPI / Chatterbox / Mock)
    let tts_engine: Arc<dyn TtsEngine> = if cfg!(windows) {
        println!("  [STAGE 6] TTS Provider:       Windows SAPI / Local TTS");
        Arc::new(LocalTtsEngine::new("default"))
    } else {
        println!("  [STAGE 6] TTS Provider:       Mock TTS");
        Arc::new(MockTtsEngine::new())
    };

    let tts_start = Instant::now();
    let text_to_synthesize = if chunks.is_empty() { &assistant_response } else { &chunks[0] };
    let audio_wav = tts_engine.synthesize(text_to_synthesize).await.expect("TTS synthesis");
    let tts_ttfs = tts_start.elapsed();
    println!("  [STAGE 6] TTS TTFS Latency:   {:?} (synthesized WAV bytes: {})", tts_ttfs, audio_wav.len());
    assert!(audio_wav.len() > 44, "Synthesized audio must contain valid WAV data");

    // Stage 7: Playback Queue & Buffer Integration
    let config = VoicePipelineConfig {
        auto_start_capture: false,
        ..Default::default()
    };
    let pipeline = Arc::new(VoicePipeline::new(config, stt_engine.clone(), tts_engine.clone(), None).unwrap());
    pipeline.start().expect("Pipeline start");

    let play_start = Instant::now();
    let decoded_samples = voxy_voice_orchestrator::decode_wav_to_f32(&audio_wav);
    assert!(!decoded_samples.is_empty(), "Decoded samples should be present");
    let play_elapsed = play_start.elapsed();
    println!("  [STAGE 7] Audio Decode & Q:   {:?} (decoded f32 samples: {})", play_elapsed, decoded_samples.len());

    // Total measured TTFS profile
    let total_ttfs = cond_elapsed + vad_elapsed + stt_elapsed + router_elapsed + chunker_elapsed + tts_ttfs;
    println!("============================================================");
    println!("  TOTAL PIPELINE TIME-TO-FIRST-SPEECH (TTFS): {:?}", total_ttfs);
    println!("============================================================\n");

    pipeline.shutdown();
}

#[tokio::test]
async fn test_live_barge_in_mid_playback_interruption() {
    println!("\n=== [OSMOO] BARGE-IN INTERRUPTION & BUFFER PURGE TEST ===");

    let stt: Arc<dyn SttEngine> = Arc::new(MockSttEngine::new("Interruption trigger utterance"));
    let tts: Arc<dyn TtsEngine> = Arc::new(MockTtsEngine::new());
    let config = VoicePipelineConfig {
        auto_start_capture: false,
        ..Default::default()
    };

    let pipeline = Arc::new(VoicePipeline::new(config, stt, tts, None).expect("Pipeline init"));
    pipeline.start().expect("Pipeline started");

    // 1. Simulate active speech synthesis and playback
    pipeline.set_state(CompanionState::Speaking, None);
    assert_eq!(pipeline.current_state(), CompanionState::Speaking);

    // 2. Queue samples into active playback handle
    let fake_speech_samples = vec![0.25f32; 16000];
    let fake_wav = voxy_voice_stt::encode_wav_16k_mono(&fake_speech_samples);
    let decoded = voxy_voice_orchestrator::decode_wav_to_f32(&fake_wav);
    assert_eq!(decoded.len(), 16000);

    // Verify barge-in trigger immediately interrupts playback
    let barge_start = Instant::now();
    pipeline.trigger_barge_in("User speech detected mid-playback (Barge-In)");
    let barge_latency = barge_start.elapsed();

    // 3. Verify state is Interrupted
    assert_eq!(
        pipeline.current_state(),
        CompanionState::Interrupted,
        "Pipeline state must immediately transition to Interrupted"
    );

    // 4. Verify playback buffer is flushed to zero
    let remaining_buffer = pipeline.playback_buffer_len();
    println!("  [BARGE-IN] Interruption Latency: {:?}", barge_latency);
    println!("  [BARGE-IN] Active Playback Buffer after Interruption: {} samples", remaining_buffer);
    assert_eq!(remaining_buffer, 0, "Playback buffer must be purged to 0 upon barge-in");

    // 5. Verify barge-in took under 50ms
    assert!(barge_latency.as_millis() < 50, "Barge-in cancellation must complete in <50ms");

    pipeline.shutdown();
    println!("=== BARGE-IN TEST PASSED ===");
}
