use std::time::Instant;

use voxy_kokoro::KokoroTtsEngine;
use voxy_provider_core::LlmChunk;
use voxy_provider_core::LlmProvider;
use voxy_voice_orchestrator::AudioChunk;
use voxy_voice_orchestrator::SttEngine;
use voxy_voice_orchestrator::TtsEngine;
use voxy_whisper::WhisperSttEngine;

fn linear_resample(data: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to {
        return data.to_vec();
    }
    let ratio = from as f64 / to as f64;
    let out_len = (data.len() as f64 * (to as f64 / from as f64)) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = (i as f64) * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = data.get(idx).copied().unwrap_or(0.0);
        let b = data.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

fn to_stereo_interleaved(mono: &[f32]) -> Vec<f32> {
    let mut out = Vec::with_capacity(mono.len() * 2);
    for &s in mono {
        out.push(s);
        out.push(s);
    }
    out
}

fn chunk(data: Vec<f32>, sample_rate: u32, channels: u8) -> AudioChunk {
    AudioChunk {
        data,
        sample_rate,
        channels,
        timestamp: chrono::Utc::now(),
        sequence: 0,
        is_final: true,
    }
}

async fn run_stt(
    whisper: &WhisperSttEngine,
    label: &str,
    samples: Vec<f32>,
    sr: u32,
    ch: u8,
) -> (String, u128) {
    whisper.clear_buffer();
    let t = Instant::now();
    let text = match whisper.transcribe(&chunk(samples, sr, ch)).await {
        Ok(t) => t,
        Err(e) => format!("<error: {e}>"),
    };
    let ms = t.elapsed().as_millis();
    println!("  [STT] {label}: '{}' ({}ms)", text, ms);
    (text, ms)
}

/// Real-model end-to-end probe (no microphone required):
/// piper TTS synthesizes a known phrase, then whisper STT transcribes it at
/// several formats to measure REAL synthesis/transcription latency and
/// accuracy on the actual models and hardware.
pub async fn run_model_e2e() {
    let phrase = "Hello, how are you doing today?";

    let whisper = WhisperSttEngine::new()
        .with_model_path("models/ggml-base.en.bin".into());
    if let Err(e) = whisper.load_model() {
        tracing::error!("[MODEL-E2E] Whisper load failed: {e}");
        return;
    }

    let tts = KokoroTtsEngine::new()
        .with_voice("default")
        .with_speed(0.55)
        .with_pitch(1.0)
        .with_model_path("models/en_US-lessac-medium.onnx".into());
    if let Err(e) = tts.load_model() {
        tracing::error!("[MODEL-E2E] Piper load failed: {e}");
        return;
    }

    println!();
    println!("================================================================");
    println!("  VOXY real-model E2E probe (piper STT+whisper TTS, no mic)");
    println!("  Phrase: \"{phrase}\"");
    println!("================================================================");

    let t_synth = Instant::now();
    let audio = match tts.synthesize(phrase).await {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("[MODEL-E2E] Piper synthesize failed: {e}");
            return;
        }
    };
    let synth_ms = t_synth.elapsed().as_millis();
    println!();
    println!("  [TTS] piper synthesize '{}': {}ms, {} samples", phrase, synth_ms, audio.data.len());

    let tts_sr = audio.sample_rate;
    let tts_ch = audio.channels;

    let native_22050 = run_stt(&whisper, "piper native mono", audio.data.clone(), tts_sr, tts_ch).await;
    let clean_16000 = run_stt(&whisper, "16kHz mono (correct whisper input)", linear_resample(&audio.data, tts_sr, 16000), 16000, 1).await;
    let device_48000 = run_stt(
        &whisper,
        "48kHz stereo (current real-device feed)",
        to_stereo_interleaved(&linear_resample(&audio.data, tts_sr, 48000)),
        48000,
        2,
    )
    .await;

    let expected = phrase.trim().to_lowercase();
    let score = |t: &str| {
        let s = t.trim().to_lowercase();
        if s.is_empty() {
            "MISS (empty)"
        } else if s.contains(&expected) || expected.contains(&s) {
            "MATCH"
        } else {
            "PARTIAL/DIFF"
        }
    };

    println!();
    println!("  Expected (lowercased): \"{expected}\"");
    println!("  STT accuracy:");
    println!("    native 22050 mono   -> {} ({})", score(&native_22050.0), native_22050.1);
    println!("    16k mono            -> {} ({})", score(&clean_16000.0), clean_16000.1);
    println!("    48k stereo (device) -> {} ({})", score(&device_48000.0), device_48000.1);
    println!();

    // ── Real local LLM streaming latency (Ollama) ──────────────────────────
    println!("  [LLM] Ollama voxy-fast:latest streaming probe...");
    let llm = std::sync::Arc::new(
        voxy_ollama::OllamaProvider::new("http://127.0.0.1:11434", "voxy-fast:latest")
            .map_err(|e| tracing::warn!("[MODEL-E2E] Ollama client failed: {e}"))
            .unwrap_or_else(|_| voxy_ollama::OllamaProvider::default()),
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel::<LlmChunk>(16);
    let prompt = "Reply in one short sentence to: what is the weather like today?";
    let llm_clone = llm.clone();
    let t_llm = Instant::now();
    tokio::spawn(async move {
        let _ = llm_clone.complete_streaming(prompt, tx).await;
    });
    let mut first_token_ms: Option<u128> = None;
    let mut total = String::new();
    while let Some(c) = rx.recv().await {
        if first_token_ms.is_none() && !c.text.is_empty() {
            first_token_ms = Some(t_llm.elapsed().as_millis());
        }
        total.push_str(&c.text);
        if c.done {
            break;
        }
    }
    let llm_total_ms = t_llm.elapsed().as_millis();
    println!(
        "  [LLM] first token: {}ms, full response: {}ms, chars: {}",
        first_token_ms.map(|v| v.to_string()).unwrap_or_else(|| "n/a".into()),
        llm_total_ms,
        total.len()
    );
    println!(
        "  [LLM] response sample: {}",
        total.chars().take(120).collect::<String>()
    );
    println!("================================================================");
}