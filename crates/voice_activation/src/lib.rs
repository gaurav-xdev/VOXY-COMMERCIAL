//! Voice Activity Detection (VAD), utterance endpointing, push-to-talk, and wake controls.
//!
//! Implements:
//! - Pre-roll audio buffer to prevent truncated initial words
//! - State machine with configurable speech onset and trailing silence thresholds
//! - Real-time energy telemetry
//! - Push-to-talk manual override and microphone mute state

use std::collections::VecDeque;
use serde::{Deserialize, Serialize};

/// Configuration for Voice Activity Detection and endpointing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VadConfig {
    /// Minimum RMS energy to qualify a 20ms frame as speech (0.0 to 1.0).
    pub threshold: f32,
    /// Consecutive active frames required to declare speech onset (prevents click triggers).
    pub min_speech_frames: usize,
    /// Trailing silence duration in milliseconds to declare end of utterance.
    pub silence_timeout_ms: u64,
    /// Number of pre-roll frames retained (prevents chopping initial consonants).
    pub preroll_frames: usize,
    /// Maximum allowed utterance duration in seconds before forcing endpoint.
    pub max_utterance_sec: f32,
    /// Sample rate (default 16000 Hz).
    pub sample_rate: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            threshold: 0.025,
            min_speech_frames: 3,
            silence_timeout_ms: 600,
            preroll_frames: 10, // ~200ms at 20ms/frame
            max_utterance_sec: 25.0,
            sample_rate: 16000,
        }
    }
}

pub mod wake_word;
pub use wake_word::{WakeWordConfig, WakeWordDetector, WakeWordMatch};

/// Events emitted by the VAD state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum VadEvent {
    /// Wake word detected (e.g. "Hey OSMOO").
    WakeWordDetected { keyword: String, confidence: f32 },
    /// User began speaking.
    SpeechStarted,
    /// In-progress speech frame.
    SpeechFrame { rms: f32 },
    /// User finished speaking and utterance is ready for STT.
    UtteranceComplete {
        audio: Vec<f32>,
        duration_ms: u64,
    },
}

/// Operational activation modes for the voice companion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ActivationMode {
    /// Continuous listening with automatic VAD.
    #[default]
    AutoVad,
    /// Wake word activation required before opening voice input.
    WakeWord,
    /// Push-to-talk mode: listening only while manually held.
    PushToTalk,
    /// Muted: capture active but audio discarded and VAD disabled.
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VadState {
    Silence,
    OnsetPending,
    InSpeech,
}

/// Stateful VAD and endpoint detector.
pub struct VadDetector {
    config: VadConfig,
    state: VadState,
    mode: ActivationMode,
    is_ptt_active: bool,
    preroll_buffer: VecDeque<Vec<f32>>,
    accumulated_audio: Vec<f32>,
    onset_count: usize,
    silence_frames_count: usize,
    max_silence_frames: usize,
    max_samples: usize,
    wake_word_detector: Option<WakeWordDetector>,
}

impl VadDetector {
    pub fn new(config: VadConfig) -> Self {
        let frame_ms = 20.0;
        let max_silence_frames = (config.silence_timeout_ms as f64 / frame_ms).round() as usize;
        let max_samples = (config.max_utterance_sec * config.sample_rate as f32) as usize;

        Self {
            config,
            state: VadState::Silence,
            mode: ActivationMode::AutoVad,
            is_ptt_active: false,
            preroll_buffer: VecDeque::new(),
            accumulated_audio: Vec::new(),
            onset_count: 0,
            silence_frames_count: 0,
            max_silence_frames,
            max_samples,
            wake_word_detector: None,
        }
    }

    pub fn with_wake_word(mut self, config: WakeWordConfig) -> Self {
        self.wake_word_detector = Some(WakeWordDetector::new(config));
        self
    }

    pub fn set_wake_word_config(&mut self, config: WakeWordConfig) {
        self.wake_word_detector = Some(WakeWordDetector::new(config));
    }

    pub fn set_mode(&mut self, mode: ActivationMode) {
        if self.mode != mode {
            self.mode = mode;
            self.reset();
        }
    }

    pub fn mode(&self) -> ActivationMode {
        self.mode
    }

    pub fn set_push_to_talk(&mut self, active: bool) {
        self.is_ptt_active = active;
    }

    pub fn reset(&mut self) {
        self.state = VadState::Silence;
        self.preroll_buffer.clear();
        self.accumulated_audio.clear();
        self.onset_count = 0;
        self.silence_frames_count = 0;
        if let Some(ref mut ww) = self.wake_word_detector {
            ww.reset();
        }
    }

    /// Process a single 16kHz mono audio chunk (e.g. 20ms = 320 samples).
    pub fn process_frame(&mut self, frame: &[f32]) -> Option<VadEvent> {
        if self.mode == ActivationMode::Muted {
            return None;
        }

        // Check wake word if configured
        if let Some(ref mut ww) = self.wake_word_detector {
            if let Some(m) = ww.feed_frame(frame) {
                if self.mode == ActivationMode::WakeWord || self.state == VadState::Silence {
                    self.state = VadState::InSpeech;
                    self.accumulated_audio.clear();
                    self.accumulated_audio.extend_from_slice(frame);
                    self.silence_frames_count = 0;
                    self.onset_count = 0;
                    return Some(VadEvent::WakeWordDetected {
                        keyword: m.keyword,
                        confidence: m.confidence,
                    });
                }
            }
        }

        if self.mode == ActivationMode::WakeWord && self.state == VadState::Silence {
            return None;
        }

        let rms = voxy_voice_audio::calculate_rms(frame);
        let is_active_voice = if self.mode == ActivationMode::PushToTalk {
            self.is_ptt_active
        } else {
            rms >= self.config.threshold
        };

        match self.state {
            VadState::Silence => {
                // Keep rolling pre-roll buffer
                self.preroll_buffer.push_back(frame.to_vec());
                if self.preroll_buffer.len() > self.config.preroll_frames {
                    self.preroll_buffer.pop_front();
                }

                if is_active_voice {
                    self.onset_count += 1;
                    if self.onset_count >= self.config.min_speech_frames {
                        self.state = VadState::InSpeech;
                        self.accumulated_audio.clear();

                        // Drain pre-roll into speech buffer
                        for preroll_chunk in self.preroll_buffer.drain(..) {
                            self.accumulated_audio.extend_from_slice(&preroll_chunk);
                        }
                        self.accumulated_audio.extend_from_slice(frame);
                        self.silence_frames_count = 0;
                        self.onset_count = 0;
                        return Some(VadEvent::SpeechStarted);
                    } else {
                        self.state = VadState::OnsetPending;
                    }
                }
                None
            }

            VadState::OnsetPending => {
                self.preroll_buffer.push_back(frame.to_vec());
                if self.preroll_buffer.len() > self.config.preroll_frames {
                    self.preroll_buffer.pop_front();
                }

                if is_active_voice {
                    self.onset_count += 1;
                    if self.onset_count >= self.config.min_speech_frames {
                        self.state = VadState::InSpeech;
                        self.accumulated_audio.clear();
                        for preroll_chunk in self.preroll_buffer.drain(..) {
                            self.accumulated_audio.extend_from_slice(&preroll_chunk);
                        }
                        self.accumulated_audio.extend_from_slice(frame);
                        self.silence_frames_count = 0;
                        self.onset_count = 0;
                        return Some(VadEvent::SpeechStarted);
                    }
                } else {
                    // False trigger / brief spike
                    self.onset_count = 0;
                    self.state = VadState::Silence;
                }
                None
            }

            VadState::InSpeech => {
                self.accumulated_audio.extend_from_slice(frame);

                if is_active_voice {
                    self.silence_frames_count = 0;
                    Some(VadEvent::SpeechFrame { rms })
                } else if self.mode == ActivationMode::PushToTalk {
                    // Instant endpointing on PTT release
                    let final_audio = std::mem::take(&mut self.accumulated_audio);
                    let duration_ms =
                        (final_audio.len() as f64 / self.config.sample_rate as f64 * 1000.0) as u64;

                    self.state = VadState::Silence;
                    self.silence_frames_count = 0;
                    self.onset_count = 0;

                    Some(VadEvent::UtteranceComplete {
                        audio: final_audio,
                        duration_ms,
                    })
                } else {
                    self.silence_frames_count += 1;
                    if self.silence_frames_count >= self.max_silence_frames
                        || self.accumulated_audio.len() >= self.max_samples
                    {
                        // Endpoint reached!
                        let final_audio = std::mem::take(&mut self.accumulated_audio);
                        let duration_ms =
                            (final_audio.len() as f64 / self.config.sample_rate as f64 * 1000.0) as u64;

                        self.state = VadState::Silence;
                        self.silence_frames_count = 0;
                        self.onset_count = 0;

                        Some(VadEvent::UtteranceComplete {
                            audio: final_audio,
                            duration_ms,
                        })
                    } else {
                        Some(VadEvent::SpeechFrame { rms })
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_silence_to_speech_to_utterance() {
        let config = VadConfig {
            threshold: 0.1,
            min_speech_frames: 2,
            silence_timeout_ms: 40, // 2 frames @ 20ms
            preroll_frames: 2,
            max_utterance_sec: 1.0,
            sample_rate: 16000,
        };

        let mut vad = VadDetector::new(config);

        let silence_frame = vec![0.01; 320];
        let speech_frame = vec![0.3; 320];

        // 1. Send silence
        assert_eq!(vad.process_frame(&silence_frame), None);

        // 2. Onset 1
        assert_eq!(vad.process_frame(&speech_frame), None);

        // 3. Onset 2 -> SpeechStarted!
        assert_eq!(vad.process_frame(&speech_frame), Some(VadEvent::SpeechStarted));

        // 4. Continued speech
        match vad.process_frame(&speech_frame) {
            Some(VadEvent::SpeechFrame { rms }) => assert!(rms >= 0.1),
            other => panic!("Expected SpeechFrame, got {:?}", other),
        }

        // 5. Silence frame 1
        match vad.process_frame(&silence_frame) {
            Some(VadEvent::SpeechFrame { .. }) => {}
            other => panic!("Expected SpeechFrame during silence window, got {:?}", other),
        }

        // 6. Silence frame 2 -> UtteranceComplete!
        match vad.process_frame(&silence_frame) {
            Some(VadEvent::UtteranceComplete { audio, duration_ms }) => {
                assert!(!audio.is_empty());
                assert!(duration_ms > 0);
            }
            other => panic!("Expected UtteranceComplete, got {:?}", other),
        }
    }

    #[test]
    fn test_push_to_talk_mode() {
        let mut vad = VadDetector::new(VadConfig {
            min_speech_frames: 1,
            silence_timeout_ms: 20,
            ..Default::default()
        });
        vad.set_mode(ActivationMode::PushToTalk);

        let quiet_frame = vec![0.005; 320];

        // Frame without PTT button
        assert_eq!(vad.process_frame(&quiet_frame), None);

        // Press PTT
        vad.set_push_to_talk(true);
        assert_eq!(vad.process_frame(&quiet_frame), Some(VadEvent::SpeechStarted));

        // Release PTT
        vad.set_push_to_talk(false);
        match vad.process_frame(&quiet_frame) {
            Some(VadEvent::UtteranceComplete { .. }) => {}
            other => panic!("Expected UtteranceComplete on PTT release, got {:?}", other),
        }
    }
}
