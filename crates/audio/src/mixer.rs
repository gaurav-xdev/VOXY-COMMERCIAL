use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

use crate::error::Result;

/// Channel identifier for the mixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MixerChannel {
    Voxy,
    Music,
    Game,
    Discord,
    Notifications,
    Browser,
    System,
    Loopback,
    Custom(u32),
}

impl MixerChannel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Voxy => "voxy",
            Self::Music => "music",
            Self::Game => "game",
            Self::Discord => "discord",
            Self::Notifications => "notifications",
            Self::Browser => "browser",
            Self::System => "system",
            Self::Loopback => "loopback",
            Self::Custom(_) => "custom",
        }
    }

    pub fn all() -> &'static [MixerChannel] {
        &[
            Self::Voxy,
            Self::Music,
            Self::Game,
            Self::Discord,
            Self::Notifications,
            Self::Browser,
            Self::System,
            Self::Loopback,
        ]
    }
}

/// Priority level for ducking decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DuckingPriority {
    /// Never duck (emergency sounds, TTS).
    Never = 0,
    /// Duck minimally (voice chat).
    Low = 1,
    /// Duck moderately (games).
    #[default]
    Medium = 2,
    /// Duck significantly (music).
    High = 3,
    /// Duck maximally (background noise).
    Max = 4,
}

/// Per-channel state.
#[derive(Debug, Clone)]
pub struct ChannelState {
    pub gain_db: f32,
    pub is_muted: bool,
    pub priority: DuckingPriority,
    pub duck_amount_db: f32,
    pub fade_samples_remaining: u32,
    pub fade_total_samples: u32,
    pub fade_start_gain: f32,
    pub fade_target_gain: f32,
}

impl Default for ChannelState {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            is_muted: false,
            priority: DuckingPriority::Medium,
            duck_amount_db: 0.0,
            fade_samples_remaining: 0,
            fade_total_samples: 0,
            fade_start_gain: 1.0,
            fade_target_gain: 1.0,
        }
    }
}

/// Audio mixer with per-channel gain, ducking, and priority scheduling.
pub struct AudioMixer {
    channels: RwLock<HashMap<MixerChannel, ChannelState>>,
    master_gain_db: f32,
    sample_rate: u32,
    /// Reference signal level (VOXY output) for ducking decisions.
    reference_level: Arc<AtomicU32>,
    /// Whether ducking is globally enabled.
    ducking_enabled: AtomicBool,
}

impl AudioMixer {
    pub fn new(sample_rate: u32) -> Self {
        let mut channels = HashMap::new();
        for ch in MixerChannel::all() {
            channels.insert(*ch, ChannelState::default());
        }
        Self {
            channels: RwLock::new(channels),
            master_gain_db: 0.0,
            sample_rate,
            reference_level: Arc::new(AtomicU32::new(0)),
            ducking_enabled: AtomicBool::new(true),
        }
    }

    pub fn with_master_gain(mut self, gain_db: f32) -> Self {
        self.master_gain_db = gain_db;
        self
    }

    /// Set gain for a specific channel.
    pub fn set_channel_gain(&self, channel: MixerChannel, gain_db: f32) {
        if let Some(state) = self.channels.write().get_mut(&channel) {
            state.gain_db = gain_db;
        }
    }

    /// Set priority for a channel (affects ducking behavior).
    pub fn set_channel_priority(&self, channel: MixerChannel, priority: DuckingPriority) {
        if let Some(state) = self.channels.write().get_mut(&channel) {
            state.priority = priority;
        }
    }

    /// Mute/unmute a channel.
    pub fn set_muted(&self, channel: MixerChannel, muted: bool) {
        if let Some(state) = self.channels.write().get_mut(&channel) {
            state.is_muted = muted;
        }
    }

    /// Start a smooth fade on a channel.
    pub fn start_fade(&self, channel: MixerChannel, target_gain: f32, duration_ms: u32) {
        let total_samples = (self.sample_rate as u64 * duration_ms as u64 / 1000) as u32;
        if let Some(state) = self.channels.write().get_mut(&channel) {
            let current_gain = db_to_linear(state.gain_db);
            state.fade_start_gain = current_gain;
            state.fade_target_gain = db_to_linear(target_gain);
            state.fade_total_samples = total_samples;
            state.fade_samples_remaining = total_samples;
        }
    }

    /// Update the reference signal level (VOXY output) for ducking.
    pub fn update_reference(&self, peak_level: f32) {
        self.reference_level
            .store((peak_level * 1000.0) as u32, Ordering::Relaxed);
    }

    /// Enable/disable ducking globally.
    pub fn set_ducking_enabled(&self, enabled: bool) {
        self.ducking_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Mix all channels into a single output buffer.
    /// Each channel's `channel_data` is (channel_id, samples).
    pub fn mix(&self, channel_data: &[(MixerChannel, &[f32])], output: &mut [f32]) -> Result<()> {
        if output.is_empty() {
            return Ok(());
        }

        let output_len = output.len();
        output.fill(0.0);

        let master_linear = db_to_linear(self.master_gain_db);
        let reference = self.reference_level.load(Ordering::Relaxed) as f32 / 1000.0;
        let ducking_on = self.ducking_enabled.load(Ordering::Relaxed);

        for (channel_id, samples) in channel_data {
            let mut channels = self.channels.write();
            let state = match channels.get_mut(channel_id) {
                Some(s) => s,
                None => continue,
            };

            if state.is_muted {
                // Still process fade even when muted (for smooth unmute)
                Self::advance_fade(state);
                continue;
            }

            // Calculate effective gain
            let mut gain_db = state.gain_db;

            // Apply ducking if this channel should be ducked
            if ducking_on && state.priority != DuckingPriority::Never && reference > 0.01 {
                let duck_db = Self::compute_duck_amount(state.priority, reference);
                gain_db -= duck_db;
                state.duck_amount_db = duck_db;
            }

            let mut gain_linear = db_to_linear(gain_db) * master_linear;

            // Apply fade
            gain_linear *= Self::apply_fade(state);

            // Mix into output
            let mix_len = output_len.min(samples.len());
            for i in 0..mix_len {
                output[i] += samples[i] * gain_linear;
            }
        }

        // Clip to prevent distortion
        for sample in output.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }

        Ok(())
    }

    /// Mix from a HashMap of channel buffers.
    pub fn mix_map(
        &self,
        channel_data: &HashMap<MixerChannel, Vec<f32>>,
        output: &mut [f32],
    ) -> Result<()> {
        let pairs: Vec<(MixerChannel, &[f32])> = channel_data
            .iter()
            .map(|(k, v)| (*k, v.as_slice()))
            .collect();
        self.mix(&pairs, output)
    }

    fn compute_duck_amount(priority: DuckingPriority, reference_level: f32) -> f32 {
        let intensity = (reference_level * 10.0).min(1.0);
        match priority {
            DuckingPriority::Never => 0.0,
            DuckingPriority::Low => intensity * 3.0,
            DuckingPriority::Medium => intensity * 8.0,
            DuckingPriority::High => intensity * 15.0,
            DuckingPriority::Max => intensity * 25.0,
        }
    }

    fn apply_fade(state: &mut ChannelState) -> f32 {
        if state.fade_samples_remaining == 0 {
            return 1.0;
        }

        let total = state.fade_total_samples as f32;
        let remaining = state.fade_samples_remaining as f32;
        let progress = 1.0 - (remaining / total);

        let gain =
            state.fade_start_gain + (state.fade_target_gain - state.fade_start_gain) * progress;
        state.fade_samples_remaining = state.fade_samples_remaining.saturating_sub(1);

        if state.fade_samples_remaining == 0 {
            state.fade_start_gain = state.fade_target_gain;
        }

        gain
    }

    fn advance_fade(state: &mut ChannelState) {
        if state.fade_samples_remaining > 0 {
            state.fade_samples_remaining = state.fade_samples_remaining.saturating_sub(1);
            if state.fade_samples_remaining == 0 {
                state.fade_start_gain = state.fade_target_gain;
            }
        }
    }

    pub fn get_channel_state(&self, channel: MixerChannel) -> Option<ChannelState> {
        self.channels.read().get(&channel).cloned()
    }

    pub fn all_channel_states(&self) -> HashMap<MixerChannel, ChannelState> {
        self.channels.read().clone()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

fn db_to_linear(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixer_creation() {
        let mixer = AudioMixer::new(16000);
        assert_eq!(mixer.sample_rate(), 16000);
        assert!(mixer.channels.read().contains_key(&MixerChannel::Voxy));
        assert!(mixer.channels.read().contains_key(&MixerChannel::Music));
    }

    #[test]
    fn mixer_silent_channels() {
        let mixer = AudioMixer::new(16000);
        let mut output = vec![0.0; 480];
        let result = mixer.mix(&[], &mut output);
        assert!(result.is_ok());
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn mixer_single_channel_passthrough() {
        let mixer = AudioMixer::new(16000);
        let input = vec![0.5, -0.5, 0.25, -0.25];
        let mut output = vec![0.0; 4];
        let data = vec![(MixerChannel::Voxy, input.as_slice())];
        mixer.mix(&data, &mut output).unwrap();
        for (i, &o) in output.iter().enumerate() {
            assert!(
                (o - input[i]).abs() < 1e-6,
                "sample {i}: {o} != {}",
                input[i]
            );
        }
    }

    #[test]
    fn mixer_two_channels_add() {
        let mixer = AudioMixer::new(16000);
        let ch1 = vec![0.3, 0.3];
        let ch2 = vec![0.2, 0.2];
        let mut output = vec![0.0; 2];
        let data = vec![
            (MixerChannel::Voxy, ch1.as_slice()),
            (MixerChannel::Music, ch2.as_slice()),
        ];
        mixer.mix(&data, &mut output).unwrap();
        assert!((output[0] - 0.5).abs() < 1e-6);
        assert!((output[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn mixer_mute_channel() {
        let mixer = AudioMixer::new(16000);
        mixer.set_muted(MixerChannel::Music, true);
        let ch1 = vec![0.3, 0.3];
        let ch2 = vec![0.5, 0.5];
        let mut output = vec![0.0; 2];
        let data = vec![
            (MixerChannel::Voxy, ch1.as_slice()),
            (MixerChannel::Music, ch2.as_slice()),
        ];
        mixer.mix(&data, &mut output).unwrap();
        assert!((output[0] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn mixer_gain_applies() {
        let mixer = AudioMixer::new(16000);
        mixer.set_channel_gain(MixerChannel::Voxy, 6.0);
        let input = vec![0.5, 0.5];
        let mut output = vec![0.0; 2];
        let data = vec![(MixerChannel::Voxy, input.as_slice())];
        mixer.mix(&data, &mut output).unwrap();
        let expected = 0.5 * 10.0f32.powf(6.0 / 20.0);
        assert!((output[0] - expected).abs() < 1e-4);
    }

    #[test]
    fn mixer_clipping() {
        let mixer = AudioMixer::new(16000);
        mixer.set_channel_gain(MixerChannel::Voxy, 20.0);
        let input = vec![0.9, 0.9];
        let mut output = vec![0.0; 2];
        let data = vec![(MixerChannel::Voxy, input.as_slice())];
        mixer.mix(&data, &mut output).unwrap();
        assert!(output[0] <= 1.0);
        assert!(output[0] >= -1.0);
    }

    #[test]
    fn mixer_empty_output() {
        let mixer = AudioMixer::new(16000);
        let mut output = vec![];
        let result = mixer.mix(&[], &mut output);
        assert!(result.is_ok());
    }

    #[test]
    fn mixer_priority_setting() {
        let mixer = AudioMixer::new(16000);
        mixer.set_channel_priority(MixerChannel::Discord, DuckingPriority::Low);
        let state = mixer.get_channel_state(MixerChannel::Discord).unwrap();
        assert_eq!(state.priority, DuckingPriority::Low);
    }

    #[test]
    fn mixer_duck_amount_calculation() {
        let low = AudioMixer::compute_duck_amount(DuckingPriority::Low, 0.5);
        let high = AudioMixer::compute_duck_amount(DuckingPriority::High, 0.5);
        let never = AudioMixer::compute_duck_amount(DuckingPriority::Never, 0.5);
        assert!(high > low);
        assert_eq!(never, 0.0);
    }

    #[test]
    fn mixer_fade_produces_intermediate_values() {
        let mixer = AudioMixer::new(16000);
        mixer.start_fade(MixerChannel::Voxy, -6.0, 100);
        let state = mixer.get_channel_state(MixerChannel::Voxy).unwrap();
        assert!(state.fade_samples_remaining > 0);
        assert!((state.fade_start_gain - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mixer_channel_all_variants() {
        let all = MixerChannel::all();
        assert_eq!(all.len(), 8);
        assert!(all.contains(&MixerChannel::Voxy));
        assert!(all.contains(&MixerChannel::Loopback));
    }

    #[test]
    fn mixer_ducking_toggle() {
        let mixer = AudioMixer::new(16000);
        mixer.set_ducking_enabled(false);
        assert!(!mixer.ducking_enabled.load(Ordering::Relaxed));
        mixer.set_ducking_enabled(true);
        assert!(mixer.ducking_enabled.load(Ordering::Relaxed));
    }

    #[test]
    fn mixer_db_to_linear_conversion() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_linear(6.0) - 2.0).abs() < 0.1);
        assert!((db_to_linear(-6.0) - 0.5).abs() < 0.1);
    }
}
