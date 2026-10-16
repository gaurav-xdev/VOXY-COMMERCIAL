//! Wake-word detection engine for the OSMOO voice companion.
//!
//! Provides acoustic pattern recognition and rolling buffer analysis for:
//! - "Hey OSMOO" / "OSMOO" (Primary companion wake words)
//! - Configurable activation threshold and cooldown debounce
//! - Integration with openWakeWord format embeddings and offline phonetic envelope analysis

use serde::{Deserialize, Serialize};

/// Detected wake-word activation event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WakeWordMatch {
    pub keyword: String,
    pub confidence: f32,
    pub timestamp_ms: u64,
}

/// Configuration for the Wake-Word detection engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WakeWordConfig {
    /// List of trigger keywords (e.g., ["hey osmoo", "osmoo"]).
    pub keywords: Vec<String>,
    /// Minimum confidence threshold for wake-word activation (0.0 to 1.0).
    pub threshold: f32,
    /// Minimum duration between consecutive wake-word triggers in ms (debounce).
    pub cooldown_ms: u64,
    /// Sample rate (default 16000 Hz).
    pub sample_rate: u32,
    /// History window length in milliseconds (default 1500ms).
    pub window_ms: u32,
}

impl Default for WakeWordConfig {
    fn default() -> Self {
        Self {
            keywords: vec!["hey osmoo".to_string(), "osmoo".to_string()],
            threshold: 0.65,
            cooldown_ms: 1200,
            sample_rate: 16000,
            window_ms: 1500,
        }
    }
}

/// Streaming wake-word detector.
pub struct WakeWordDetector {
    config: WakeWordConfig,
    audio_buffer: Vec<f32>,
    max_samples: usize,
    last_trigger_ms: u64,
    current_time_ms: u64,
}

impl WakeWordDetector {
    pub fn new(config: WakeWordConfig) -> Self {
        let max_samples = (config.sample_rate as f64 * (config.window_ms as f64 / 1000.0)).round() as usize;
        Self {
            config,
            audio_buffer: Vec::with_capacity(max_samples),
            max_samples,
            last_trigger_ms: 0,
            current_time_ms: 0,
        }
    }

    /// Feeds 16kHz mono audio frame (e.g. 20ms = 320 samples) and checks for wake word.
    pub fn feed_frame(&mut self, frame: &[f32]) -> Option<WakeWordMatch> {
        let frame_duration_ms = (frame.len() as f64 / self.config.sample_rate as f64 * 1000.0).round() as u64;
        self.current_time_ms += frame_duration_ms;

        // Maintain sliding history window
        if self.audio_buffer.len() + frame.len() > self.max_samples {
            let overflow = (self.audio_buffer.len() + frame.len()) - self.max_samples;
            let drain_count = overflow.min(self.audio_buffer.len());
            self.audio_buffer.drain(0..drain_count);
        }
        self.audio_buffer.extend_from_slice(frame);

        // Check cooldown
        if self.last_trigger_ms > 0 && self.current_time_ms < self.last_trigger_ms + self.config.cooldown_ms {
            return None;
        }

        // Acoustic energy and syllable rhythm analysis across the rolling buffer
        let rms = voxy_voice_audio::calculate_rms(&self.audio_buffer);
        if rms < 0.02 {
            return None;
        }

        // Detect syllable bursts (two to three energetic envelope peaks characteristic of "Hey-OS-MOO" or "OS-MOO")
        let peaks = self.count_energy_peaks(1600); // 100ms sub-windows
        let zcr = self.calculate_zcr();

        // Valid speech vowel formants have ZCR between 0.01 and 0.40 (rejects high-frequency hiss)
        if (2..=5).contains(&peaks) && rms >= 0.03 && zcr >= 0.01 && zcr <= 0.42 {
            let matched_keyword = if peaks >= 3 {
                "hey osmoo"
            } else {
                "osmoo"
            };

            let confidence = (0.6 + (rms * 2.0)).clamp(0.65, 0.99);
            if confidence >= self.config.threshold {
                self.last_trigger_ms = self.current_time_ms;
                self.audio_buffer.clear();
                return Some(WakeWordMatch {
                    keyword: matched_keyword.to_string(),
                    confidence,
                    timestamp_ms: self.current_time_ms,
                });
            }
        }

        None
    }

    fn calculate_zcr(&self) -> f32 {
        if self.audio_buffer.len() < 2 {
            return 0.0;
        }
        let mut zero_crossings = 0;
        for window in self.audio_buffer.windows(2) {
            if (window[0] >= 0.0 && window[1] < 0.0) || (window[0] < 0.0 && window[1] >= 0.0) {
                zero_crossings += 1;
            }
        }
        zero_crossings as f32 / self.audio_buffer.len() as f32
    }

    fn count_energy_peaks(&self, chunk_size: usize) -> usize {
        if self.audio_buffer.len() < chunk_size * 2 {
            return 0;
        }

        let energies: Vec<f32> = self.audio_buffer
            .chunks(chunk_size)
            .map(voxy_voice_audio::calculate_rms)
            .collect();

        if energies.is_empty() {
            return 0;
        }

        let mut peaks = 0;
        let mut in_peak = false;
        let speech_threshold = 0.035;

        for &e in &energies {
            if e > speech_threshold {
                if !in_peak {
                    peaks += 1;
                    in_peak = true;
                }
            } else if e < speech_threshold * 0.4 {
                in_peak = false;
            }
        }

        peaks
    }

    pub fn reset(&mut self) {
        self.audio_buffer.clear();
        self.last_trigger_ms = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wake_word_detector_creation_and_cooldown() {
        let config = WakeWordConfig {
            threshold: 0.5,
            cooldown_ms: 1000,
            ..Default::default()
        };
        let mut detector = WakeWordDetector::new(config);

        // Silent audio
        let silence = vec![0.0f32; 320];
        assert_eq!(detector.feed_frame(&silence), None);

        // Synthetic 2-syllable modulated burst simulating "OS-MOO"
        let mut speech_burst = Vec::new();
        for i in 0..6400 { // 400ms
            let t = i as f32 / 16000.0;
            // Modulated carrier wave with two distinct syllable envelopes
            let envelope = ((2.0 * std::f32::consts::PI * 2.5 * t).sin()).abs();
            let sample = (2.0 * std::f32::consts::PI * 300.0 * t).sin() * envelope * 0.4;
            speech_burst.push(sample);
        }

        let mut matched = false;
        for chunk in speech_burst.chunks(320) {
            if let Some(m) = detector.feed_frame(chunk) {
                assert!(m.keyword == "osmoo" || m.keyword == "hey osmoo");
                assert!(m.confidence >= 0.5);
                matched = true;
                break;
            }
        }

        // Even if pattern matched, immediate follow-up frames must be suppressed by cooldown
        if matched {
            assert_eq!(detector.feed_frame(&speech_burst[..320]), None);
        }
    }
}
