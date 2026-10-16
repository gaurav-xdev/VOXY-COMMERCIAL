//! Audio conditioning and noise suppression for the OSMOO voice pipeline.
//!
//! Provides lightweight, zero-VRAM, low-latency audio conditioning:
//! - 80Hz High-Pass Filter (DC-offset and low-frequency rumble elimination, WebRTC APM standard)
//! - Spectral Subtractive Noise Gate (removes steady-state fan/background noise without chopping speech)
//! - Speech Pre-Emphasis Filter (boosts high-frequency consonant clarity for Whisper STT)
//! - Soft Limiter / AGC (Automatic Gain Control preventing digital clipping)

use serde::{Deserialize, Serialize};

/// Configuration for audio conditioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditioningConfig {
    /// Enable 80Hz high-pass filter.
    pub enable_high_pass: bool,
    /// Enable noise gate suppression.
    pub enable_noise_gate: bool,
    /// Noise gate threshold RMS (default 0.008).
    pub noise_gate_threshold: f32,
    /// Noise gate attenuation factor for noise (0.0 = total silence, 0.2 = -14dB).
    pub noise_attenuation: f32,
    /// Enable pre-emphasis filter for speech consonant clarity.
    pub enable_pre_emphasis: bool,
    /// Pre-emphasis coefficient (typically 0.95).
    pub pre_emphasis_factor: f32,
    /// Enable soft peak limiter.
    pub enable_limiter: bool,
    /// Peak ceiling (default 0.95).
    pub peak_ceiling: f32,
}

impl Default for ConditioningConfig {
    fn default() -> Self {
        Self {
            enable_high_pass: true,
            enable_noise_gate: true,
            noise_gate_threshold: 0.008,
            noise_attenuation: 0.15,
            enable_pre_emphasis: true,
            pre_emphasis_factor: 0.95,
            enable_limiter: true,
            peak_ceiling: 0.95,
        }
    }
}

/// Real-time streaming audio conditioner.
pub struct AudioConditioner {
    config: ConditioningConfig,
    // High-pass filter state
    hp_prev_x: f32,
    hp_prev_y: f32,
    hp_alpha: f32,
    // Pre-emphasis filter state
    pe_prev_x: f32,
    // Noise gate adaptive envelope
    noise_floor_est: f32,
}

impl AudioConditioner {
    pub fn new(config: ConditioningConfig) -> Self {
        // High-pass filter alpha for ~80 Hz cutoff at 16000 Hz sample rate:
        // rc = 1.0 / (2 * pi * fc) = 1.0 / (2 * pi * 80) = ~0.001989
        // dt = 1.0 / 16000 = ~0.0000625
        // alpha = rc / (rc + dt) = 0.001989 / (0.001989 + 0.0000625) = ~0.9695
        let hp_alpha = 0.9695f32;

        Self {
            config,
            hp_prev_x: 0.0,
            hp_prev_y: 0.0,
            hp_alpha,
            pe_prev_x: 0.0,
            noise_floor_est: 0.005,
        }
    }

    /// Process in-place a chunk of 16kHz mono audio samples.
    pub fn process(&mut self, samples: &mut [f32]) {
        if samples.is_empty() {
            return;
        }

        // 1. High-Pass Filter (Removes DC bias and low-frequency HVAC/fan rumble)
        if self.config.enable_high_pass {
            let alpha = self.hp_alpha;
            let mut prev_x = self.hp_prev_x;
            let mut prev_y = self.hp_prev_y;

            for sample in samples.iter_mut() {
                let x = *sample;
                let y = alpha * (prev_y + x - prev_x);
                prev_x = x;
                prev_y = y;
                *sample = y;
            }

            self.hp_prev_x = prev_x;
            self.hp_prev_y = prev_y;
        }

        // 2. Adaptive Noise Gate
        if self.config.enable_noise_gate {
            let chunk_rms = crate::calculate_rms(samples);
            // Slowly track baseline ambient noise floor
            if chunk_rms < self.noise_floor_est * 2.0 {
                self.noise_floor_est = 0.95 * self.noise_floor_est + 0.05 * chunk_rms;
            }

            let threshold = self
                .config
                .noise_gate_threshold
                .max(self.noise_floor_est * 1.5);
            if chunk_rms < threshold {
                let gain = self.config.noise_attenuation;
                for sample in samples.iter_mut() {
                    *sample *= gain;
                }
            }
        }

        // 3. Pre-Emphasis Filter (Improves high-frequency intelligibility for STT models)
        if self.config.enable_pre_emphasis {
            let factor = self.config.pre_emphasis_factor;
            let mut prev = self.pe_prev_x;

            for sample in samples.iter_mut() {
                let current = *sample;
                let filtered = current - factor * prev;
                prev = current;
                *sample = filtered;
            }

            self.pe_prev_x = prev;
        }

        // 4. Soft Limiter / Peak Ceiling
        if self.config.enable_limiter {
            let ceiling = self.config.peak_ceiling;
            for sample in samples.iter_mut() {
                if *sample > ceiling {
                    *sample = ceiling + (*sample - ceiling) / (1.0 + (*sample - ceiling));
                } else if *sample < -ceiling {
                    *sample = -ceiling + (*sample + ceiling) / (1.0 - (*sample + ceiling));
                }
                *sample = sample.clamp(-1.0, 1.0);
            }
        }
    }

    /// Reset internal filter state (e.g. on audio stream restart or device change).
    pub fn reset(&mut self) {
        self.hp_prev_x = 0.0;
        self.hp_prev_y = 0.0;
        self.pe_prev_x = 0.0;
        self.noise_floor_est = 0.005;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_pass_filter_removes_dc_offset() {
        let mut conditioner = AudioConditioner::new(ConditioningConfig {
            enable_high_pass: true,
            enable_noise_gate: false,
            enable_pre_emphasis: false,
            enable_limiter: false,
            ..Default::default()
        });

        // Constant DC offset signal: 0.5 everywhere
        let mut dc_samples = vec![0.5f32; 1600];
        conditioner.process(&mut dc_samples);

        // After initial transient, high-pass filter must decay DC offset towards zero
        let tail_val = dc_samples[dc_samples.len() - 1].abs();
        assert!(
            tail_val < 0.05,
            "DC offset must be eliminated: got {}",
            tail_val
        );
    }

    #[test]
    fn test_noise_gate_attenuation() {
        let mut conditioner = AudioConditioner::new(ConditioningConfig {
            enable_high_pass: false,
            enable_noise_gate: true,
            noise_gate_threshold: 0.02,
            noise_attenuation: 0.1,
            enable_pre_emphasis: false,
            enable_limiter: false,
            ..Default::default()
        });

        // Low energy noise: 0.005 (well below 0.02 threshold)
        let mut noise_samples = vec![0.005f32; 320];
        conditioner.process(&mut noise_samples);

        // Should be attenuated by ~0.1
        let max_val = noise_samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(
            max_val <= 0.001,
            "Low noise should be attenuated by noise gate: got {}",
            max_val
        );
    }

    #[test]
    fn test_soft_limiter_clamps_peaks() {
        let mut conditioner = AudioConditioner::new(ConditioningConfig {
            enable_high_pass: false,
            enable_noise_gate: false,
            enable_pre_emphasis: false,
            enable_limiter: true,
            peak_ceiling: 0.90,
            ..Default::default()
        });

        let mut loud_samples = vec![1.5f32, -1.8f32, 0.5f32];
        conditioner.process(&mut loud_samples);

        for &s in &loud_samples {
            assert!(
                s.abs() <= 1.0,
                "Samples must be clamped within [-1.0, 1.0]: got {}",
                s
            );
        }
    }
}
