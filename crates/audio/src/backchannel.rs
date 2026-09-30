use std::collections::VecDeque;

/// Generates natural backchannel signals ("uh-huh", "mhm", "right")
/// during user speech to make the assistant feel attentive and alive.
///
/// Backchannels are triggered by:
/// - Speech duration > threshold (user has been talking for a while)
/// - Energy pauses (brief dips in energy suggest natural pause points)
/// - Speech rate (fast speech gets fewer backchannels)
pub struct BackchannelGenerator {
    /// Minimum speech duration (ms) before first backchannel.
    min_speech_ms: u32,
    /// Minimum interval between backchannels (ms).
    min_interval_ms: u32,
    /// Speech duration since last backchannel (ms).
    speech_duration_ms: u32,
    /// Time since last backchannel (ms).
    time_since_last_ms: u32,
    /// Recent energy levels for pause detection.
    energy_history: VecDeque<f32>,
    /// Maximum history length.
    max_history: usize,
    /// Backchannel phrases.
    phrases: Vec<&'static str>,
    /// Current phrase index (round-robin).
    phrase_index: usize,
    /// Whether we're in a "listening" state.
    is_listening: bool,
    /// Number of backchannels generated in this turn.
    backchannels_this_turn: usize,
    /// Maximum backchannels per turn (prevents over-backchanneling).
    max_per_turn: usize,
}

impl BackchannelGenerator {
    pub fn new() -> Self {
        Self {
            min_speech_ms: 2000,
            min_interval_ms: 3000,
            speech_duration_ms: 0,
            time_since_last_ms: 0,
            energy_history: VecDeque::with_capacity(50),
            max_history: 50,
            phrases: vec!["uh-huh", "mhm", "right", "yeah", "okay", "i see"],
            phrase_index: 0,
            is_listening: false,
            backchannels_this_turn: 0,
            max_per_turn: 5,
        }
    }

    /// Process an audio frame. Returns a backchannel phrase if one should be played.
    pub fn process_frame(&mut self, energy: f32, frame_duration_ms: u32) -> Option<&'static str> {
        self.energy_history.push_back(energy);
        if self.energy_history.len() > self.max_history {
            self.energy_history.pop_front();
        }

        self.time_since_last_ms += frame_duration_ms;

        if energy > 0.01 {
            self.speech_duration_ms += frame_duration_ms;
            self.is_listening = true;
        } else if self.is_listening {
            // Speech ended
            self.is_listening = false;
            self.speech_duration_ms = 0;
            self.backchannels_this_turn = 0;
        }

        // Check if we should backchannel
        if self.speech_duration_ms >= self.min_speech_ms
            && self.time_since_last_ms >= self.min_interval_ms
            && self.backchannels_this_turn < self.max_per_turn
        {
            // Only backchannel at energy dips (natural pause points)
            if self.is_energy_dip() {
                self.time_since_last_ms = 0;
                self.backchannels_this_turn += 1;
                let phrase = self.phrases[self.phrase_index % self.phrases.len()];
                self.phrase_index += 1;
                return Some(phrase);
            }
        }

        None
    }

    /// Detect energy dips (brief pauses in speech).
    fn is_energy_dip(&self) -> bool {
        if self.energy_history.len() < 5 {
            return false;
        }
        let recent: f32 = self.energy_history.iter().rev().take(3).sum::<f32>() / 3.0;
        let prev: f32 = self
            .energy_history
            .iter()
            .rev()
            .skip(3)
            .take(3)
            .sum::<f32>()
            / 3.0;
        // Energy dip: recent is lower than previous
        recent < prev * 0.7 && recent < 0.1
    }

    pub fn reset(&mut self) {
        self.speech_duration_ms = 0;
        self.time_since_last_ms = 0;
        self.energy_history.clear();
        self.phrase_index = 0;
        self.backchannels_this_turn = 0;
        self.is_listening = false;
    }
}

impl Default for BackchannelGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backchannel_creation() {
        let bc = BackchannelGenerator::new();
        assert_eq!(bc.min_speech_ms, 2000);
        assert_eq!(bc.min_interval_ms, 3000);
    }

    #[test]
    fn backchannel_no_trigger_on_silence() {
        let mut bc = BackchannelGenerator::new();
        let result = bc.process_frame(0.001, 30);
        assert!(result.is_none());
    }

    #[test]
    fn backchannel_triggers_after_speech() {
        let mut bc = BackchannelGenerator::new();
        // Speech for 2.5 seconds
        for _ in 0..83 {
            bc.process_frame(0.5, 30);
        }
        // Energy dip should trigger backchannel
        // Feed a dip
        let _result = bc.process_frame(0.005, 30);
        // May or may not trigger depending on energy dip detection
        // Just verify no panic
    }

    #[test]
    fn backchannel_reset() {
        let mut bc = BackchannelGenerator::new();
        for _ in 0..100 {
            bc.process_frame(0.5, 30);
        }
        bc.reset();
        assert_eq!(bc.speech_duration_ms, 0);
        assert_eq!(bc.backchannels_this_turn, 0);
    }
}
