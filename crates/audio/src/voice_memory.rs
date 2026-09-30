use std::collections::HashMap;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

/// Voice profile learned from the user over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceProfile {
    /// Average speaking speed in words per minute.
    pub speaking_wpm: f64,
    /// Detected accent/dialect hint.
    pub accent: String,
    /// Preferred language code.
    pub preferred_language: String,
    /// Typical background noise level in dB.
    pub background_noise_db: f32,
    /// Preferred microphone gain in dB.
    pub preferred_mic_gain_db: f32,
    /// Speaking volume tendency (quiet/normal/loud).
    pub volume_tendency: VolumeTendency,
    /// Number of conversations used for learning.
    pub conversation_count: u32,
    /// Average pause duration between sentences in ms.
    pub avg_pause_ms: f64,
    /// Whether the user tends to interrupt VOXY.
    pub interrupts_frequently: bool,
}

impl Default for VoiceProfile {
    fn default() -> Self {
        Self {
            speaking_wpm: 150.0,
            accent: "en-US".into(),
            preferred_language: "en".into(),
            background_noise_db: -40.0,
            preferred_mic_gain_db: 0.0,
            volume_tendency: VolumeTendency::Normal,
            conversation_count: 0,
            avg_pause_ms: 500.0,
            interrupts_frequently: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeTendency {
    Quiet,
    Normal,
    Loud,
}

impl std::fmt::Display for VolumeTendency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quiet => write!(f, "quiet"),
            Self::Normal => write!(f, "normal"),
            Self::Loud => write!(f, "loud"),
        }
    }
}

/// Voice memory that learns user characteristics over time.
pub struct VoiceMemory {
    profiles: RwLock<HashMap<String, VoiceProfile>>,
    /// Rolling window of recent speaking rates.
    recent_wpm: RwLock<Vec<f64>>,
    /// Rolling window of recent volume levels.
    recent_volume: RwLock<Vec<f32>>,
    /// Rolling window of pause durations.
    recent_pauses: RwLock<Vec<f64>>,
    /// Count of interruptions detected.
    interruption_count: RwLock<u32>,
    max_samples: usize,
}

impl VoiceMemory {
    pub fn new() -> Self {
        Self {
            profiles: RwLock::new(HashMap::new()),
            recent_wpm: RwLock::new(Vec::with_capacity(100)),
            recent_volume: RwLock::new(Vec::with_capacity(100)),
            recent_pauses: RwLock::new(Vec::with_capacity(100)),
            interruption_count: RwLock::new(0),
            max_samples: 100,
        }
    }

    /// Get or create a profile for a user.
    pub fn get_or_create_profile(&self, user_id: &str) -> VoiceProfile {
        let profiles = self.profiles.read();
        profiles.get(user_id).cloned().unwrap_or_default()
    }

    /// Update the profile based on new conversation data.
    pub fn record_conversation(
        &self,
        user_id: &str,
        speaking_duration_secs: f64,
        word_count: usize,
        volume_db: f32,
        pause_ms: f64,
        was_interrupted: bool,
    ) {
        // Update rolling windows
        if speaking_duration_secs > 0.0 && word_count > 0 {
            let wpm = (word_count as f64 / speaking_duration_secs) * 60.0;
            let mut wpm_window = self.recent_wpm.write();
            if wpm_window.len() >= self.max_samples {
                wpm_window.remove(0);
            }
            wpm_window.push(wpm);
        }

        {
            let mut vol_window = self.recent_volume.write();
            if vol_window.len() >= self.max_samples {
                vol_window.remove(0);
            }
            vol_window.push(volume_db);
        }

        {
            let mut pause_window = self.recent_pauses.write();
            if pause_window.len() >= self.max_samples {
                pause_window.remove(0);
            }
            pause_window.push(pause_ms);
        }

        if was_interrupted {
            *self.interruption_count.write() += 1;
        }

        // Update profile
        let mut profiles = self.profiles.write();
        let profile = profiles.entry(user_id.to_string()).or_default();

        profile.conversation_count += 1;

        // Running average update
        let n = profile.conversation_count as f64;
        let wpm_window = self.recent_wpm.read();
        if !wpm_window.is_empty() {
            let avg_wpm = wpm_window.iter().sum::<f64>() / wpm_window.len() as f64;
            profile.speaking_wpm = profile.speaking_wpm * ((n - 1.0) / n) + avg_wpm / n;
        }

        let vol_window = self.recent_volume.read();
        if !vol_window.is_empty() {
            let avg_vol = vol_window.iter().sum::<f32>() / vol_window.len() as f32;
            profile.background_noise_db = avg_vol;
            profile.volume_tendency = if avg_vol < -50.0 {
                VolumeTendency::Quiet
            } else if avg_vol > -20.0 {
                VolumeTendency::Loud
            } else {
                VolumeTendency::Normal
            };
        }

        let pause_window = self.recent_pauses.read();
        if !pause_window.is_empty() {
            let avg_pause = pause_window.iter().sum::<f64>() / pause_window.len() as f64;
            profile.avg_pause_ms = profile.avg_pause_ms * ((n - 1.0) / n) + avg_pause / n;
        }

        let int_count = *self.interruption_count.read();
        profile.interrupts_frequently = int_count as f64 / n > 0.3;
    }

    /// Get the recommended VAD threshold based on the user's profile.
    pub fn recommended_vad_threshold(&self, user_id: &str) -> f32 {
        let profile = self.get_or_create_profile(user_id);
        let base = 10.0f32.powf((profile.background_noise_db + 6.0) / 20.0);
        match profile.volume_tendency {
            VolumeTendency::Quiet => base * 0.5,
            VolumeTendency::Normal => base,
            VolumeTendency::Loud => base * 1.5,
        }
    }

    /// Get all profiles.
    pub fn all_profiles(&self) -> HashMap<String, VoiceProfile> {
        self.profiles.read().clone()
    }

    pub fn conversation_count(&self, user_id: &str) -> u32 {
        self.profiles
            .read()
            .get(user_id)
            .map(|p| p.conversation_count)
            .unwrap_or(0)
    }

    pub fn reset(&self) {
        self.profiles.write().clear();
        self.recent_wpm.write().clear();
        self.recent_volume.write().clear();
        self.recent_pauses.write().clear();
        *self.interruption_count.write() = 0;
    }
}

impl Default for VoiceMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_creation() {
        let mem = VoiceMemory::new();
        let profile = mem.get_or_create_profile("user-1");
        assert_eq!(profile.conversation_count, 0);
        assert_eq!(profile.speaking_wpm, 150.0);
    }

    #[test]
    fn memory_record_conversation() {
        let mem = VoiceMemory::new();
        mem.record_conversation("user-1", 60.0, 150, -35.0, 400.0, false);
        let profile = mem.get_or_create_profile("user-1");
        assert_eq!(profile.conversation_count, 1);
        assert!(profile.speaking_wpm > 0.0);
    }

    #[test]
    fn memory_running_average() {
        let mem = VoiceMemory::new();
        mem.record_conversation("u", 60.0, 150, -35.0, 400.0, false);
        mem.record_conversation("u", 60.0, 300, -35.0, 400.0, false);
        let profile = mem.get_or_create_profile("u");
        assert_eq!(profile.conversation_count, 2);
        // WPM should be averaged
        assert!(profile.speaking_wpm > 100.0);
    }

    #[test]
    fn memory_volume_tendency() {
        let mem = VoiceMemory::new();
        // Quiet user only — fills window with quiet samples
        for _ in 0..5 {
            mem.record_conversation("quiet", 60.0, 100, -60.0, 500.0, false);
        }
        let profile = mem.get_or_create_profile("quiet");
        assert_eq!(profile.volume_tendency, VolumeTendency::Quiet);

        // Loud user — create a new memory so the window is clean
        let mem2 = VoiceMemory::new();
        for _ in 0..5 {
            mem2.record_conversation("loud", 60.0, 100, -10.0, 300.0, false);
        }
        let profile = mem2.get_or_create_profile("loud");
        assert_eq!(profile.volume_tendency, VolumeTendency::Loud);
    }

    #[test]
    fn memory_interruption_tracking() {
        let mem = VoiceMemory::new();
        for _ in 0..10 {
            mem.record_conversation("u", 60.0, 100, -35.0, 400.0, true);
        }
        let profile = mem.get_or_create_profile("u");
        assert!(profile.interrupts_frequently);
    }

    #[test]
    fn memory_recommended_vad() {
        let mem = VoiceMemory::new();
        let threshold = mem.recommended_vad_threshold("new-user");
        assert!(threshold > 0.0);
        assert!(threshold < 1.0);
    }

    #[test]
    fn memory_reset() {
        let mem = VoiceMemory::new();
        mem.record_conversation("u", 60.0, 100, -35.0, 400.0, false);
        mem.reset();
        assert_eq!(mem.conversation_count("u"), 0);
    }

    #[test]
    fn memory_multiple_users() {
        let mem = VoiceMemory::new();
        mem.record_conversation("alice", 60.0, 150, -35.0, 400.0, false);
        mem.record_conversation("bob", 60.0, 200, -30.0, 300.0, false);
        assert_eq!(mem.conversation_count("alice"), 1);
        assert_eq!(mem.conversation_count("bob"), 1);
    }

    #[test]
    fn memory_profile_serialization() {
        let profile = VoiceProfile::default();
        let json = serde_json::to_string(&profile).unwrap();
        assert!(json.contains("speaking_wpm"));
        let _deserialized: VoiceProfile = serde_json::from_str(&json).unwrap();
    }
}
