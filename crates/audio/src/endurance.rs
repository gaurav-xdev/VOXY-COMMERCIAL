#[cfg(test)]
mod endurance_tests {

    use std::sync::Arc;
    use std::time::Instant;

    use crate::calibration::SelfCalibrator;
    use crate::gpu_dsp::{AdaptiveNoiseSuppressor, SpectralEchoCanceller};
    use crate::metrics::MetricsCollector;
    use crate::mixer::{AudioMixer, MixerChannel};
    use crate::scheduler::{AiAudioScheduler, SystemSnapshot};
    use crate::voice_memory::VoiceMemory;
    use crate::watchdog::HealthWatchdog;

    fn sine_wave(freq: f32, sample_rate: u32, duration_ms: u32, amplitude: f32) -> Vec<f32> {
        let samples = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
        (0..samples)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * std::f32::consts::PI * freq * t).sin() * amplitude
            })
            .collect()
    }

    fn noise_buffer(len: usize, amplitude: f32) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let x = (i as f32 * 0.1).sin() * 0.5;
                x * amplitude
            })
            .collect()
    }

    #[tokio::test]
    async fn endurance_10k_wake_cycles() {
        let watchdog = Arc::new(HealthWatchdog::new());
        watchdog.register_stage("audio_input");
        watchdog.register_stage("stt");
        watchdog.register_stage("llm");
        watchdog.register_stage("tts");

        let start = Instant::now();

        for i in 0..10_000 {
            watchdog.heartbeat("audio_input");
            let stt_data = sine_wave(440.0, 16000, 50, 0.5);
            let _ = stt_data.len();
            watchdog.heartbeat("stt");
            let _ = format!("Response to wake cycle {i}");
            watchdog.heartbeat("llm");
            let tts_data = sine_wave(300.0, 16000, 100, 0.3);
            let _ = tts_data.len();
            watchdog.heartbeat("tts");

            if i % 1000 == 999 {
                watchdog.record_failure("stt");
                watchdog.heartbeat("stt");
            }
        }

        let elapsed = start.elapsed();
        let avg_per_cycle_us = elapsed.as_micros() as f64 / 10_000.0;

        assert!(
            elapsed.as_secs() < 60,
            "10K wake cycles took too long: {:?}",
            elapsed
        );
        assert!(
            avg_per_cycle_us < 5000.0,
            "Average per-cycle time too high: {:.1}us",
            avg_per_cycle_us
        );

        let all_healthy = watchdog
            .all_stages()
            .iter()
            .all(|s| s.status == crate::watchdog::HealthStatus::Healthy);
        assert!(
            all_healthy,
            "Watchdog stages not all healthy after 10K cycles"
        );
    }

    #[tokio::test]
    async fn endurance_5k_conversations() {
        let memory = Arc::new(VoiceMemory::new());
        let calibrator = Arc::new(SelfCalibrator::new());
        let metrics = Arc::new(MetricsCollector::new());

        let start = Instant::now();

        for i in 0..5_000 {
            let user_id = format!("user-{}", i % 100);
            let input_data = noise_buffer(480, 0.01);
            calibrator.feed_input_frame(&input_data);
            memory.record_conversation(
                &user_id,
                30.0,
                50,
                -35.0 + (i as f32 % 20.0),
                500.0,
                i % 10 == 0,
            );
            metrics.record_stt_latency(100.0 + (i as f64 % 50.0));
            metrics.record_llm_latency(200.0 + (i as f64 % 100.0));
            metrics.record_tts_latency(50.0 + (i as f64 % 30.0));
        }

        let elapsed = start.elapsed();
        let profiles = memory.all_profiles();
        assert!(
            profiles.len() >= 100,
            "Expected at least 100 user profiles, got {}",
            profiles.len()
        );

        let snapshot = metrics.latency_snapshot();
        assert!(snapshot.stt_first_token_ms > 0.0);
        assert!(snapshot.llm_first_token_ms > 0.0);
        assert!(snapshot.tts_first_chunk_ms > 0.0);

        assert!(
            elapsed.as_secs() < 30,
            "5K conversations took too long: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn endurance_continuous_streaming() {
        let mixer = AudioMixer::new(16000);

        let start = Instant::now();
        let mut total_samples = 0u64;

        for _ in 0..50_000 {
            let frame = sine_wave(440.0, 16000, 10, 0.5);
            total_samples += frame.len() as u64;
            let data = vec![(MixerChannel::Voxy, frame.as_slice())];
            let mut output = vec![0.0f32; frame.len()];
            let _ = mixer.mix(&data, &mut output);
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 60,
            "Continuous streaming took too long: {:?}",
            elapsed
        );
        assert!(total_samples > 0);
    }

    #[tokio::test]
    async fn endurance_repeated_barge_in() {
        let watchdog = Arc::new(HealthWatchdog::new());
        watchdog.register_stage("tts");

        let start = Instant::now();

        for i in 0..10_000 {
            watchdog.heartbeat("tts");
            if i % 100 == 0 {
                watchdog.record_failure("tts");
                watchdog.heartbeat("tts");
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "10K barge-ins took too long: {:?}",
            elapsed
        );

        let health = watchdog.stage_health("tts").unwrap();
        assert_eq!(health.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn endurance_device_switching() {
        let mixer = AudioMixer::new(16000);

        let start = Instant::now();

        for i in 0..5_000 {
            mixer.set_channel_gain(MixerChannel::Voxy, -6.0 + (i as f32 % 12.0));
            mixer.set_muted(MixerChannel::Voxy, i % 2 == 0);
            let frame = sine_wave(440.0, 16000, 10, 0.5);
            let data = vec![(MixerChannel::Voxy, frame.as_slice())];
            let mut output = vec![0.0f32; frame.len()];
            let _ = mixer.mix(&data, &mut output);
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "5K device switches took too long: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn endurance_memory_pressure() {
        let memory = Arc::new(VoiceMemory::new());
        let metrics = Arc::new(MetricsCollector::new());

        let start = Instant::now();

        for i in 0..100_000 {
            memory.record_conversation(
                &format!("user-{}", i % 1000),
                60.0,
                100,
                -30.0,
                500.0,
                false,
            );
            metrics.record_stt_latency(100.0);
            metrics.record_llm_latency(200.0);
            metrics.record_tts_latency(50.0);
        }

        let elapsed = start.elapsed();
        let profiles = memory.all_profiles();
        assert!(
            profiles.len() <= 1000,
            "Too many profiles: {}",
            profiles.len()
        );
        assert!(
            elapsed.as_secs() < 60,
            "100K memory operations took too long: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn endurance_cpu_pressure() {
        let scheduler = Arc::new(AiAudioScheduler::new());
        let start = Instant::now();
        let mut switches = 0u64;

        for i in 0..10_000 {
            let cpu = (i as f64 % 100.0) * 1.5;
            let snap = SystemSnapshot {
                cpu_percent: cpu,
                memory_mb: 4000.0,
                memory_total_mb: 16000.0,
                gpu_percent: 30.0,
                vram_mb: 0.0,
                vram_total_mb: 0.0,
                battery_percent: None,
                temperature_c: None,
                is_on_battery: false,
            };
            let (_, changed) = scheduler.update(&snap);
            if changed {
                switches += 1;
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "10K scheduler evaluations took too long: {:?}",
            elapsed
        );
        assert!(
            switches > 0,
            "Scheduler should have switched modes at least once"
        );
    }

    #[tokio::test]
    async fn endurance_gpu_pressure() {
        let mut ns = AdaptiveNoiseSuppressor::new(16000);
        let mut ec = SpectralEchoCanceller::new(16000, 20);

        let start = Instant::now();

        for _ in 0..1_000 {
            let input = noise_buffer(480, 0.01);
            let mut output = Vec::with_capacity(input.len());
            let _ = ns.process(&input, &mut output);

            let reference = noise_buffer(480, 0.005);
            let mut ec_output = Vec::with_capacity(input.len());
            let _ = ec.process(&output, &reference, &mut ec_output);
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() < 30,
            "1K DSP operations took too long: {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn endurance_network_disconnect() {
        let watchdog = Arc::new(HealthWatchdog::new());
        watchdog.register_stage("llm");

        let start = Instant::now();

        for i in 0..1000 {
            if i % 100 == 0 {
                for _ in 0..5 {
                    watchdog.record_failure("llm");
                }
                watchdog.heartbeat("llm");
            } else {
                watchdog.heartbeat("llm");
            }
        }

        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 10);
        let health = watchdog.stage_health("llm").unwrap();
        assert_eq!(health.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn endurance_microphone_reconnect() {
        let watchdog = Arc::new(HealthWatchdog::new());
        watchdog.register_stage("audio_input");

        let start = Instant::now();

        for i in 0..5000 {
            if i % 500 == 0 {
                watchdog.record_failure("audio_input");
                watchdog.heartbeat("audio_input");
            } else {
                watchdog.heartbeat("audio_input");
            }
        }

        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 10);
        let health = watchdog.stage_health("audio_input").unwrap();
        assert_eq!(health.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn endurance_no_latency_degradation() {
        let metrics = Arc::new(MetricsCollector::new());

        for _ in 0..100 {
            metrics.record_stt_latency(100.0);
            metrics.record_llm_latency(200.0);
            metrics.record_tts_latency(50.0);
        }

        let baseline = metrics.latency_snapshot();

        for i in 0..10_000 {
            metrics.record_stt_latency(100.0 + (i as f64 % 10.0));
            metrics.record_llm_latency(200.0 + (i as f64 % 20.0));
            metrics.record_tts_latency(50.0 + (i as f64 % 5.0));
        }

        let after = metrics.latency_snapshot();
        let stt_deg = (after.stt_first_token_ms - baseline.stt_first_token_ms).abs();
        let llm_deg = (after.llm_first_token_ms - baseline.llm_first_token_ms).abs();
        let tts_deg = (after.tts_first_chunk_ms - baseline.tts_first_chunk_ms).abs();

        assert!(stt_deg < 50.0, "STT latency degraded by {:.1}ms", stt_deg);
        assert!(llm_deg < 100.0, "LLM latency degraded by {:.1}ms", llm_deg);
        assert!(tts_deg < 30.0, "TTS latency degraded by {:.1}ms", tts_deg);
    }

    #[tokio::test]
    async fn endurance_no_buffer_growth() {
        let memory = Arc::new(VoiceMemory::new());

        for i in 0..50_000 {
            memory.record_conversation(&format!("user-{}", i % 100), 30.0, 50, -30.0, 500.0, false);
        }

        let profiles = memory.all_profiles();
        assert!(
            profiles.len() <= 100,
            "Profile count grew beyond cap: {}",
            profiles.len()
        );

        for (id, profile) in &profiles {
            assert!(
                profile.conversation_count <= 500,
                "User {} has {} conversations (expected <= 500)",
                id,
                profile.conversation_count
            );
        }
    }
}
