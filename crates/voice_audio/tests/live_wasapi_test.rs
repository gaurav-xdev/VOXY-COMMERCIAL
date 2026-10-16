use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use voxy_voice_audio::{AudioConfig, AudioRuntime};

#[tokio::test]
async fn test_live_wasapi_capture_and_playback() {
    let config = AudioConfig::default();
    let runtime =
        AudioRuntime::new(config).expect("AudioRuntime should initialize with Windows host");

    // Test playback initialization
    let playback = runtime
        .start_playback()
        .expect("Audio playback should start");
    assert!(!playback.is_playing());

    // Queue 100ms 440Hz tone
    let mut tone = Vec::with_capacity(1600);
    for i in 0..1600 {
        let t = i as f32 / 16000.0;
        tone.push((t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.1);
    }
    playback.queue_samples(&tone);
    assert!(playback.is_playing() || playback.buffer_len() > 0);

    // Test barge-in clear
    playback.clear_playback();
    assert_eq!(playback.buffer_len(), 0);

    // Test capture initialization
    let frames_captured = Arc::new(AtomicUsize::new(0));
    let frames_clone = frames_captured.clone();

    let capture = runtime.start_capture(move |frame, rms| {
        assert_eq!(
            frame.len(),
            320,
            "Normalized frame must be 320 samples (20ms @ 16kHz)"
        );
        assert!((0.0..=1.0).contains(&rms), "RMS must be bounded [0, 1]");
        frames_clone.fetch_add(1, Ordering::SeqCst);
    });

    match capture {
        Ok(handle) => {
            println!(
                "Physical WASAPI audio capture handle opened: active={}",
                handle.is_active()
            );
            tokio::time::sleep(Duration::from_millis(300)).await;
            let count = frames_captured.load(Ordering::SeqCst);
            println!("Captured {} frames from WASAPI in 300ms", count);
            handle.stop();
            assert!(!handle.is_active());
        }
        Err(e) => {
            println!(
                "Capture returned error (e.g. headless/no microphone): {}",
                e
            );
        }
    }
}
