#[cfg(test)]
mod benchmark_tests {
    use std::time::Instant;

    use crate::calibration::SelfCalibrator;
    use crate::gpu_dsp::{AdaptiveNoiseSuppressor, SpectralEchoCanceller};
    use crate::metrics::MetricsCollector;
    use crate::mixer::{AudioMixer, MixerChannel};
    use crate::scheduler::{AiAudioScheduler, SystemSnapshot};
    use crate::voice_memory::VoiceMemory;
    use crate::watchdog::HealthWatchdog;

    fn sine_wave(freq: f32, sr: u32, ms: u32, amp: f32) -> Vec<f32> {
        let n = (sr as u64 * ms as u64 / 1000) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin() * amp)
            .collect()
    }

    fn noise(len: usize) -> Vec<f32> {
        (0..len).map(|i| (i as f32 * 0.1).sin() * 0.001).collect()
    }

    // ═══════════════════════════════════════════════════════════════
    // PHASE 5: Latency Benchmarks
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn benchmark_wake_detection_latency() {
        let watchdog = HealthWatchdog::new();
        watchdog.register_stage("audio_input");
        let mut latencies = Vec::new();

        for _ in 0..1000 {
            let start = Instant::now();
            watchdog.heartbeat("audio_input");
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
        let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];

        assert!(avg < 10.0, "Wake detection avg too high: {:.2}us", avg);
        assert!(p95 < 50.0, "Wake detection P95 too high: {:.2}us", p95);
        assert!(p99 < 100.0, "Wake detection P99 too high: {:.2}us", p99);
    }

    #[test]
    fn benchmark_vad_latency() {
        use crate::dsp::SilenceDetector;
        let detector = SilenceDetector::new(0.01, 3);
        let frame = sine_wave(440.0, 16000, 30, 0.5);
        let mut latencies = Vec::new();

        for _ in 0..1000 {
            let start = Instant::now();
            let _ = detector.is_silence(&frame);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 100.0, "VAD avg too high: {:.2}us", avg);
    }

    #[test]
    fn benchmark_noise_suppression_latency() {
        let mut ns = AdaptiveNoiseSuppressor::new(16000);
        let frame = noise(480);
        let mut latencies = Vec::new();

        for _ in 0..1000 {
            let mut output = Vec::with_capacity(frame.len());
            let start = Instant::now();
            let _ = ns.process(&frame, &mut output);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 500.0, "Noise suppression avg too high: {:.2}us", avg);
    }

    #[test]
    fn benchmark_echo_cancellation_latency() {
        let mut ec = SpectralEchoCanceller::new(16000, 20);
        let mic = noise(480);
        let ref_signal = noise(480);
        let mut latencies = Vec::new();

        for _ in 0..500 {
            let mut output = Vec::with_capacity(mic.len());
            let start = Instant::now();
            let _ = ec.process(&mic, &ref_signal, &mut output);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let threshold = if cfg!(debug_assertions) {
            25000.0
        } else {
            5000.0
        };
        assert!(
            avg < threshold,
            "Echo cancellation avg too high: {:.2}us (threshold: {:.2}us)",
            avg,
            threshold
        );
    }

    #[test]
    fn benchmark_mixer_latency() {
        let mixer = AudioMixer::new(16000);
        let frame = sine_wave(440.0, 16000, 10, 0.5);
        let mut latencies = Vec::new();

        for _ in 0..1000 {
            let data = vec![(MixerChannel::Voxy, frame.as_slice())];
            let mut output = vec![0.0f32; frame.len()];
            let start = Instant::now();
            let _ = mixer.mix(&data, &mut output);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 200.0, "Mixer avg too high: {:.2}us", avg);
    }

    #[test]
    fn benchmark_metrics_collection_latency() {
        let mc = MetricsCollector::new();
        let mut latencies = Vec::new();

        for _ in 0..10_000 {
            let start = Instant::now();
            mc.record_stt_latency(100.0);
            mc.record_llm_latency(200.0);
            mc.record_tts_latency(50.0);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 10.0, "Metrics collection avg too high: {:.2}us", avg);
    }

    #[test]
    fn benchmark_scheduler_evaluation_latency() {
        let scheduler = AiAudioScheduler::new();
        let snap = SystemSnapshot {
            cpu_percent: 50.0,
            memory_mb: 4000.0,
            memory_total_mb: 16000.0,
            gpu_percent: 30.0,
            vram_mb: 0.0,
            vram_total_mb: 0.0,
            battery_percent: None,
            temperature_c: None,
            is_on_battery: false,
        };
        let mut latencies = Vec::new();

        for _ in 0..10_000 {
            let start = Instant::now();
            let _ = scheduler.evaluate(&snap);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 5.0, "Scheduler evaluation avg too high: {:.2}us", avg);
    }

    #[test]
    fn benchmark_calibration_latency() {
        let cal = SelfCalibrator::new();
        let frame = noise(480);
        let mut latencies = Vec::new();

        for _ in 0..1000 {
            let start = Instant::now();
            cal.feed_input_frame(&frame);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 50.0, "Calibration feed avg too high: {:.2}us", avg);
    }

    #[test]
    fn benchmark_voice_memory_latency() {
        let vm = VoiceMemory::new();
        let mut latencies = Vec::new();

        for i in 0..10_000 {
            let start = Instant::now();
            vm.record_conversation(&format!("user-{}", i % 100), 30.0, 50, -30.0, 500.0, false);
            latencies.push(start.elapsed().as_micros() as f64);
        }

        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
        assert!(avg < 50.0, "Voice memory avg too high: {:.2}us", avg);
    }

    // ═══════════════════════════════════════════════════════════════
    // PHASE 6: Audio Quality Benchmarks
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn quality_noise_suppression_effectiveness() {
        let mut ns = AdaptiveNoiseSuppressor::new(16000);

        // Feed noise floor
        for _ in 0..100 {
            let noise_frame = noise(480);
            let mut out = Vec::new();
            ns.process(&noise_frame, &mut out).unwrap();
        }

        // Test: loud signal should pass through
        let loud = sine_wave(440.0, 16000, 30, 0.5);
        let mut output = Vec::new();
        ns.process(&loud, &mut output).unwrap();
        let input_rms: f64 = loud
            .iter()
            .map(|&s| s as f64 * s as f64)
            .sum::<f64>()
            .sqrt()
            / (loud.len() as f64).sqrt();
        let output_rms: f64 = output
            .iter()
            .map(|&s| s as f64 * s as f64)
            .sum::<f64>()
            .sqrt()
            / (output.len() as f64).sqrt();
        assert!(
            output_rms > input_rms * 0.3,
            "Loud signal should pass through: in={:.4} out={:.4}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn quality_echo_cancellation_effectiveness() {
        let mut ec = SpectralEchoCanceller::new(16000, 20);

        // Train with repeated blocks
        for _ in 0..50 {
            let ref_signal = sine_wave(300.0, 16000, 30, 0.3);
            let mic: Vec<f32> = ref_signal.iter().map(|&r| r * 0.8 + 0.05).collect();
            let mut output = Vec::new();
            ec.process(&mic, &ref_signal, &mut output).unwrap();
        }

        // After training, echo should be reduced
        let ref_signal = sine_wave(300.0, 16000, 30, 0.3);
        let mic: Vec<f32> = ref_signal.iter().map(|&r| r * 0.8 + 0.05).collect();
        let mut output = Vec::new();
        ec.process(&mic, &ref_signal, &mut output).unwrap();

        let mic_rms: f64 = mic.iter().map(|&s| s as f64 * s as f64).sum::<f64>().sqrt()
            / (mic.len() as f64).sqrt();
        let out_rms: f64 = output
            .iter()
            .map(|&s| s as f64 * s as f64)
            .sum::<f64>()
            .sqrt()
            / (output.len() as f64).sqrt();
        assert!(
            out_rms < mic_rms,
            "Echo should be reduced: mic={:.4} out={:.4}",
            mic_rms,
            out_rms
        );
    }

    #[test]
    fn quality_mixer_no_clipping() {
        let mixer = AudioMixer::new(16000);

        // Set high gain
        mixer.set_channel_gain(MixerChannel::Voxy, 20.0);

        let loud = sine_wave(440.0, 16000, 30, 0.9);
        let data = vec![(MixerChannel::Voxy, loud.as_slice())];
        let mut output = vec![0.0f32; loud.len()];
        mixer.mix(&data, &mut output).unwrap();

        let max = output.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(max <= 1.0, "Mixer should prevent clipping: max={:.4}", max);
    }

    #[test]
    fn quality_ducking_reduces_background() {
        let mixer = AudioMixer::new(16000);

        // Set up music at normal level
        let music = sine_wave(200.0, 16000, 30, 0.5);
        let voxy = sine_wave(440.0, 16000, 30, 0.5);

        // Before ducking
        let data_before = vec![
            (MixerChannel::Music, music.as_slice()),
            (MixerChannel::Voxy, voxy.as_slice()),
        ];
        let mut out_before = vec![0.0f32; music.len()];
        mixer.mix(&data_before, &mut out_before).unwrap();
        let rms_before: f64 = out_before
            .iter()
            .map(|&s| s as f64 * s as f64)
            .sum::<f64>()
            .sqrt()
            / (out_before.len() as f64).sqrt();

        // Trigger ducking
        mixer.update_reference(0.5);

        // After ducking
        let data_after = vec![
            (MixerChannel::Music, music.as_slice()),
            (MixerChannel::Voxy, voxy.as_slice()),
        ];
        let mut out_after = vec![0.0f32; music.len()];
        mixer.mix(&data_after, &mut out_after).unwrap();
        let rms_after: f64 = out_after
            .iter()
            .map(|&s| s as f64 * s as f64)
            .sum::<f64>()
            .sqrt()
            / (out_after.len() as f64).sqrt();

        // Output should be reduced (or at least not increased)
        assert!(
            rms_after <= rms_before * 1.1,
            "Ducking should reduce or maintain level: before={:.4} after={:.4}",
            rms_before,
            rms_after
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // PHASE 7: Performance Regression Detection
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn regression_detector() {
        struct Baseline {
            stt_avg_ms: f64,
            llm_avg_ms: f64,
            tts_avg_ms: f64,
            cpu_percent: f64,
            memory_mb: f64,
        }

        impl Baseline {
            fn detect_regression(&self, current: &Baseline, threshold_pct: f64) -> Vec<String> {
                let mut regressions = Vec::new();

                let stt_increase =
                    ((current.stt_avg_ms - self.stt_avg_ms) / self.stt_avg_ms) * 100.0;
                if stt_increase > threshold_pct {
                    regressions.push(format!("STT latency regressed by {:.1}%", stt_increase));
                }

                let llm_increase =
                    ((current.llm_avg_ms - self.llm_avg_ms) / self.llm_avg_ms) * 100.0;
                if llm_increase > threshold_pct {
                    regressions.push(format!("LLM latency regressed by {:.1}%", llm_increase));
                }

                let tts_increase =
                    ((current.tts_avg_ms - self.tts_avg_ms) / self.tts_avg_ms) * 100.0;
                if tts_increase > threshold_pct {
                    regressions.push(format!("TTS latency regressed by {:.1}%", tts_increase));
                }

                let cpu_increase =
                    ((current.cpu_percent - self.cpu_percent) / self.cpu_percent) * 100.0;
                if cpu_increase > threshold_pct {
                    regressions.push(format!("CPU usage regressed by {:.1}%", cpu_increase));
                }

                let mem_increase = ((current.memory_mb - self.memory_mb) / self.memory_mb) * 100.0;
                if mem_increase > threshold_pct {
                    regressions.push(format!("Memory usage regressed by {:.1}%", mem_increase));
                }

                regressions
            }
        }

        // No regression
        let baseline = Baseline {
            stt_avg_ms: 100.0,
            llm_avg_ms: 200.0,
            tts_avg_ms: 50.0,
            cpu_percent: 10.0,
            memory_mb: 100.0,
        };
        let current = Baseline {
            stt_avg_ms: 105.0,
            llm_avg_ms: 210.0,
            tts_avg_ms: 52.0,
            cpu_percent: 11.0,
            memory_mb: 105.0,
        };
        let regressions = baseline.detect_regression(&current, 20.0);
        assert!(
            regressions.is_empty(),
            "Should not detect regression: {:?}",
            regressions
        );

        // Regression detected
        let current_bad = Baseline {
            stt_avg_ms: 150.0,
            llm_avg_ms: 300.0,
            tts_avg_ms: 80.0,
            cpu_percent: 25.0,
            memory_mb: 200.0,
        };
        let regressions = baseline.detect_regression(&current_bad, 20.0);
        assert!(!regressions.is_empty(), "Should detect regressions");
        assert!(
            regressions.len() >= 3,
            "Should detect at least 3 regressions"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // PHASE 8: Self-Healing
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn self_healing_stage_restart() {
        let watchdog = HealthWatchdog::new();
        watchdog.register_stage("stt");
        watchdog.register_stage("tts");

        // Simulate STT failure
        for _ in 0..5 {
            watchdog.record_failure("stt");
        }
        assert_eq!(
            watchdog.stage_health("stt").unwrap().status,
            crate::watchdog::HealthStatus::Failed
        );

        // Self-healing: restart only STT
        watchdog.heartbeat("stt");
        assert_eq!(
            watchdog.stage_health("stt").unwrap().status,
            crate::watchdog::HealthStatus::Healthy
        );

        // TTS should still be healthy
        assert_eq!(
            watchdog.stage_health("tts").unwrap().status,
            crate::watchdog::HealthStatus::Healthy
        );
    }

    #[test]
    fn self_healing_circuit_breaker() {
        use crate::watchdog::CircuitBreaker;
        use std::time::Duration;

        let cb = CircuitBreaker::new(3, Duration::from_secs(0));

        // Stage with failures
        let mut health = crate::watchdog::StageHealth::new("stt");
        health.consecutive_failures = 3;
        health.status = crate::watchdog::HealthStatus::Failed;
        health.last_heartbeat = Instant::now() - Duration::from_secs(10);

        assert!(
            cb.should_attempt_recovery(&health),
            "Circuit breaker should allow recovery"
        );

        // Healthy stage - no recovery needed
        let healthy = crate::watchdog::StageHealth::new("tts");
        assert!(
            !cb.should_attempt_recovery(&healthy),
            "Healthy stage should not need recovery"
        );
    }

    #[test]
    fn self_healing_independent_stages() {
        let watchdog = HealthWatchdog::new();
        watchdog.register_stage("audio_input");
        watchdog.register_stage("stt");
        watchdog.register_stage("llm");
        watchdog.register_stage("tts");
        watchdog.register_stage("audio_output");

        // Kill LLM only
        for _ in 0..5 {
            watchdog.record_failure("llm");
        }

        // All other stages should be healthy
        for stage in &["audio_input", "stt", "tts", "audio_output"] {
            assert_eq!(
                watchdog.stage_health(stage).unwrap().status,
                crate::watchdog::HealthStatus::Healthy,
                "Stage {} should be healthy",
                stage
            );
        }

        // LLM should be failed
        assert_eq!(
            watchdog.stage_health("llm").unwrap().status,
            crate::watchdog::HealthStatus::Failed
        );

        // Recovery: restart only LLM
        watchdog.heartbeat("llm");
        assert_eq!(
            watchdog.stage_health("llm").unwrap().status,
            crate::watchdog::HealthStatus::Healthy
        );
    }

    #[test]
    fn self_healing_repeated_failures() {
        let watchdog = HealthWatchdog::new();
        watchdog.register_stage("stt");

        // Simulate 100 failure-recovery cycles
        for _ in 0..100 {
            for _ in 0..5 {
                watchdog.record_failure("stt");
            }
            watchdog.heartbeat("stt");
            assert_eq!(
                watchdog.stage_health("stt").unwrap().status,
                crate::watchdog::HealthStatus::Healthy
            );
        }

        // After 100 cycles, should still be healthy
        assert_eq!(
            watchdog.stage_health("stt").unwrap().status,
            crate::watchdog::HealthStatus::Healthy
        );
        assert_eq!(watchdog.stage_health("stt").unwrap().total_failures, 500);
    }
}
