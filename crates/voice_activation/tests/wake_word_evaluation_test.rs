use voxy_voice_activation::{WakeWordConfig, WakeWordDetector};

#[test]
fn test_wake_word_evaluation_against_noise_silence_and_speech() {
    let mut detector = WakeWordDetector::new(WakeWordConfig::default());

    // 1. Test Silence (100 frames = 2 seconds of zero audio)
    let silence_frame = vec![0.0f32; 320];
    let mut silence_activations = 0;
    for _ in 0..100 {
        if detector.feed_frame(&silence_frame).is_some() {
            silence_activations += 1;
        }
    }
    assert_eq!(silence_activations, 0, "Silence should never trigger wake word");

    // 2. Test Low-Level Background Noise (100 frames of random noise with RMS < 0.02)
    let noise_frame: Vec<f32> = (0..320).map(|i| ((i % 7) as f32 - 3.0) * 0.002).collect();
    let mut noise_activations = 0;
    for _ in 0..100 {
        if detector.feed_frame(&noise_frame).is_some() {
            noise_activations += 1;
        }
    }
    assert_eq!(noise_activations, 0, "Background noise below threshold should never trigger wake word");

    // 3. Test Unrelated Monotone Speech / Hum (constant sine wave 200Hz, no syllable rhythm)
    let hum_frame: Vec<f32> = (0..320)
        .map(|i| (i as f32 * 200.0 * 2.0 * std::f32::consts::PI / 16000.0).sin() * 0.1)
        .collect();
    let mut hum_activations = 0;
    for _ in 0..5 {
        if detector.feed_frame(&hum_frame).is_some() {
            hum_activations += 1;
        }
    }
    println!("Hum activations: {}", hum_activations);

    // 4. Test Target Syllabic Rhythm ("Hey-OS-MOO": ~900ms utterance with 3 bursts)
    let mut osmoo_detector = WakeWordDetector::new(WakeWordConfig::default());
    let mut target_activations = 0;

    // Syllable 1 ("Hey"): 200ms (10 frames)
    for _ in 0..10 {
        let burst: Vec<f32> = (0..320)
            .map(|i| (i as f32 * 300.0 * 2.0 * std::f32::consts::PI / 16000.0).sin() * 0.15)
            .collect();
        if osmoo_detector.feed_frame(&burst).is_some() {
            target_activations += 1;
        }
    }
    // Gap: 100ms (5 frames silence)
    for _ in 0..5 {
        if osmoo_detector.feed_frame(&silence_frame).is_some() {
            target_activations += 1;
        }
    }
    // Syllable 2 ("OS"): 200ms (10 frames)
    for _ in 0..10 {
        let burst: Vec<f32> = (0..320)
            .map(|i| (i as f32 * 450.0 * 2.0 * std::f32::consts::PI / 16000.0).sin() * 0.15)
            .collect();
        if osmoo_detector.feed_frame(&burst).is_some() {
            target_activations += 1;
        }
    }
    // Gap: 100ms (5 frames silence)
    for _ in 0..5 {
        if osmoo_detector.feed_frame(&silence_frame).is_some() {
            target_activations += 1;
        }
    }
    // Syllable 3 ("MOO"): 300ms (15 frames)
    for _ in 0..15 {
        let burst: Vec<f32> = (0..320)
            .map(|i| (i as f32 * 250.0 * 2.0 * std::f32::consts::PI / 16000.0).sin() * 0.15)
            .collect();
        if osmoo_detector.feed_frame(&burst).is_some() {
            target_activations += 1;
        }
    }

    println!("Target cadence activations: {}", target_activations);
    assert!(target_activations > 0, "Target 3-syllable cadence should trigger wake word");
}
