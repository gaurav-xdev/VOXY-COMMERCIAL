use std::collections::VecDeque;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Calibration profile stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub noise_floor_db: f32,
    pub echo_level_db: f32,
    pub mic_gain_db: f32,
    pub speaker_delay_ms: f32,
    pub optimal_vad_threshold: f32,
    pub optimal_wake_threshold: f32,
    pub room_impulse_peak_db: f32,
    pub calibrated_at: Option<String>,
}

impl Default for CalibrationProfile {
    fn default() -> Self {
        Self {
            noise_floor_db: -40.0,
            echo_level_db: -30.0,
            mic_gain_db: 0.0,
            speaker_delay_ms: 0.0,
            optimal_vad_threshold: 0.05,
            optimal_wake_threshold: 0.3,
            room_impulse_peak_db: -20.0,
            calibrated_at: None,
        }
    }
}

/// Self-calibration engine that learns room acoustics on first launch.
pub struct SelfCalibrator {
    profile: RwLock<CalibrationProfile>,
    /// Rolling buffer of input levels for noise floor estimation.
    input_levels_db: RwLock<VecDeque<f32>>,
    /// Rolling buffer of echo levels.
    echo_levels_db: RwLock<VecDeque<f32>>,
    /// Whether calibration has been completed.
    is_calibrated: RwLock<bool>,
    /// Number of samples needed for calibration.
    calibration_samples: usize,
}

impl SelfCalibrator {
    pub fn new() -> Self {
        Self {
            profile: RwLock::new(CalibrationProfile::default()),
            input_levels_db: RwLock::new(VecDeque::with_capacity(1000)),
            echo_levels_db: RwLock::new(VecDeque::with_capacity(1000)),
            is_calibrated: RwLock::new(false),
            calibration_samples: 500,
        }
    }

    pub fn with_calibration_samples(mut self, samples: usize) -> Self {
        self.calibration_samples = samples;
        self
    }

    /// Feed an audio frame during calibration period.
    pub fn feed_input_frame(&self, samples: &[f32]) {
        let rms = compute_rms(samples);
        let db = if rms > 0.0 {
            20.0 * rms.log10()
        } else {
            -100.0
        };

        let mut levels = self.input_levels_db.write();
        levels.push_back(db);
        if levels.len() > self.calibration_samples {
            levels.pop_front();
        }
    }

    /// Feed the echo reference signal (what VOXY is currently playing).
    pub fn feed_echo_reference(&self, samples: &[f32]) {
        let rms = compute_rms(samples);
        let db = if rms > 0.0 {
            20.0 * rms.log10()
        } else {
            -100.0
        };

        let mut levels = self.echo_levels_db.write();
        levels.push_back(db);
        if levels.len() > self.calibration_samples {
            levels.pop_front();
        }
    }

    /// Check if enough data has been collected.
    pub fn is_ready(&self) -> bool {
        self.input_levels_db.read().len() >= self.calibration_samples
    }

    /// Run calibration and compute the profile.
    pub fn calibrate(&self) -> Result<CalibrationProfile> {
        let input_levels = self.input_levels_db.read();
        let echo_levels = self.echo_levels_db.read();

        if input_levels.is_empty() {
            return Ok(self.profile.read().clone());
        }

        // Noise floor: median of input levels when no echo is present
        let mut sorted_input: Vec<f32> = input_levels.iter().copied().collect();
        sorted_input.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let noise_floor = sorted_input[sorted_input.len() / 10]; // 10th percentile

        // Echo level: when VOXY is playing, how much leaks into the mic
        let echo_db = if !echo_levels.is_empty() {
            let max_echo = echo_levels
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            let _avg_echo: f32 = echo_levels.iter().sum::<f32>() / echo_levels.len() as f32;
            // Use max as peak echo level
            max_echo
        } else {
            -100.0
        };

        // Optimal VAD threshold: noise floor + 6dB margin
        let optimal_vad = 10.0f32.powf((noise_floor + 6.0) / 20.0);

        // Optimal wake word threshold: slightly above noise floor
        let optimal_wake = 10.0f32.powf((noise_floor + 10.0) / 20.0);

        let profile = CalibrationProfile {
            noise_floor_db: noise_floor,
            echo_level_db: echo_db,
            mic_gain_db: 0.0,      // Will be adjusted by user
            speaker_delay_ms: 0.0, // Will be measured with ping
            optimal_vad_threshold: optimal_vad.clamp(0.001, 1.0),
            optimal_wake_threshold: optimal_wake.clamp(0.01, 1.0),
            room_impulse_peak_db: echo_db,
            calibrated_at: Some(chrono::Utc::now().to_rfc3339()),
        };

        *self.profile.write() = profile.clone();
        *self.is_calibrated.write() = true;

        Ok(profile)
    }

    pub fn get_profile(&self) -> CalibrationProfile {
        self.profile.read().clone()
    }

    pub fn is_calibrated(&self) -> bool {
        *self.is_calibrated.read()
    }

    pub fn load_profile(&self, profile: CalibrationProfile) {
        *self.profile.write() = profile;
        *self.is_calibrated.write() = true;
    }

    pub fn reset(&self) {
        self.input_levels_db.write().clear();
        self.echo_levels_db.write().clear();
        *self.is_calibrated.write() = false;
    }
}

impl Default for SelfCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| s as f64 * s as f64).sum();
    (sum_sq / samples.len() as f64).sqrt() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibrator_creation() {
        let cal = SelfCalibrator::new();
        assert!(!cal.is_calibrated());
        assert!(!cal.is_ready());
    }

    #[test]
    fn calibrator_default_profile() {
        let cal = SelfCalibrator::new();
        let profile = cal.get_profile();
        assert_eq!(profile.noise_floor_db, -40.0);
        assert_eq!(profile.optimal_vad_threshold, 0.05);
    }

    #[test]
    fn calibrator_feed_frames() {
        let cal = SelfCalibrator::new().with_calibration_samples(3);
        cal.feed_input_frame(&[0.1, 0.2, 0.3]);
        cal.feed_input_frame(&[0.0, 0.0, 0.0]);
        cal.feed_input_frame(&[0.5, 0.5, 0.5]);
        assert!(cal.is_ready());
    }

    #[test]
    fn calibrator_not_ready_without_enough_data() {
        let cal = SelfCalibrator::new().with_calibration_samples(10);
        cal.feed_input_frame(&[0.1]);
        assert!(!cal.is_ready());
    }

    #[test]
    fn calibrator_run_calibration() {
        let cal = SelfCalibrator::new().with_calibration_samples(5);
        for _ in 0..5 {
            cal.feed_input_frame(&[0.01, 0.01, 0.01]);
        }
        let profile = cal.calibrate().unwrap();
        assert!(cal.is_calibrated());
        assert!(profile.noise_floor_db < 0.0);
        assert!(profile.optimal_vad_threshold > 0.0);
        assert!(profile.calibrated_at.is_some());
    }

    #[test]
    fn calibrator_resets() {
        let cal = SelfCalibrator::new().with_calibration_samples(2);
        cal.feed_input_frame(&[0.1]);
        cal.feed_input_frame(&[0.1]);
        cal.calibrate().unwrap();
        assert!(cal.is_calibrated());
        cal.reset();
        assert!(!cal.is_calibrated());
        assert!(!cal.is_ready());
    }

    #[test]
    fn calibrator_load_profile() {
        let cal = SelfCalibrator::new();
        let profile = CalibrationProfile {
            noise_floor_db: -50.0,
            ..Default::default()
        };
        cal.load_profile(profile);
        assert!(cal.is_calibrated());
        assert_eq!(cal.get_profile().noise_floor_db, -50.0);
    }

    #[test]
    fn calibrator_compute_rms() {
        let rms = compute_rms(&[0.5, -0.5, 0.5, -0.5]);
        assert!((rms - 0.5).abs() < 1e-6);
    }

    #[test]
    fn calibrator_compute_rms_empty() {
        let rms = compute_rms(&[]);
        assert_eq!(rms, 0.0);
    }

    #[test]
    fn calibrator_echo_reference() {
        let cal = SelfCalibrator::new();
        cal.feed_echo_reference(&[0.3, 0.3, 0.3]);
        let levels = cal.echo_levels_db.read();
        assert_eq!(levels.len(), 1);
    }

    #[test]
    fn calibrator_higher_noise_higher_vad() {
        let cal1 = SelfCalibrator::new().with_calibration_samples(3);
        for _ in 0..3 {
            cal1.feed_input_frame(&[0.001, 0.001]);
        }
        let p1 = cal1.calibrate().unwrap();

        let cal2 = SelfCalibrator::new().with_calibration_samples(3);
        for _ in 0..3 {
            cal2.feed_input_frame(&[0.1, 0.1]);
        }
        let p2 = cal2.calibrate().unwrap();

        assert!(p2.optimal_vad_threshold > p1.optimal_vad_threshold);
    }
}
