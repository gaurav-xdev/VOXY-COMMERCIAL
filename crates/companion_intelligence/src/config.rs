use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntelligenceConfig {
    pub emotional: EmotionalConfig,
    pub conversation: ConversationConfig,
    pub memory: MemoryConfig,
    pub proactive: ProactiveConfig,
    pub decision: DecisionConfig,
    pub personality: PersonalityConfig,
    pub presence: PresenceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalConfig {
    pub signal_decay_rate: f64,
    pub emotion_persistence_ms: u64,
    pub confidence_threshold: f64,
    pub max_emotion_history: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationConfig {
    pub max_memory_turns: usize,
    pub topic_window_size: usize,
    pub reference_lookback: usize,
    pub context_merge_threshold: f64,
    pub max_topic_age_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub base_importance: f64,
    pub decay_rate_per_hour: f64,
    pub recall_boost_factor: f64,
    pub project_bonus: f64,
    pub min_importance_to_keep: f64,
    pub max_memories: usize,
    pub consolidation_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveConfig {
    pub min_suggestion_interval_secs: u64,
    pub max_suggestions_per_hour: usize,
    pub cooldown_per_category_secs: u64,
    pub annoyance_threshold: f64,
    pub confidence_threshold: f64,
    pub max_active_suggestions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionConfig {
    pub interrupt_threshold: f64,
    pub silence_threshold: f64,
    pub remind_threshold: f64,
    pub congratulate_threshold: f64,
    pub wait_threshold: f64,
    pub decision_cooldown_ms: u64,
    pub max_decisions_per_minute: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityConfig {
    pub mood_decay_rate: f64,
    pub confidence_update_rate: f64,
    pub curiosity_trigger_threshold: f64,
    pub empathy_response_threshold: f64,
    pub humor_probability: f64,
    pub base_response_length: usize,
    pub thinking_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceConfig {
    pub state_transition_cooldown_ms: u64,
    pub emergency_timeout_secs: u64,
    pub focus_mode_min_duration_secs: u64,
    pub celebration_duration_ms: u64,
}

impl Default for EmotionalConfig {
    fn default() -> Self {
        Self {
            signal_decay_rate: 0.1,
            emotion_persistence_ms: 30_000,
            confidence_threshold: 0.3,
            max_emotion_history: 50,
        }
    }
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            max_memory_turns: 20,
            topic_window_size: 10,
            reference_lookback: 5,
            context_merge_threshold: 0.7,
            max_topic_age_secs: 3600,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            base_importance: 0.5,
            decay_rate_per_hour: 0.02,
            recall_boost_factor: 1.5,
            project_bonus: 0.2,
            min_importance_to_keep: 0.1,
            max_memories: 1000,
            consolidation_interval_secs: 300,
        }
    }
}

impl Default for ProactiveConfig {
    fn default() -> Self {
        Self {
            min_suggestion_interval_secs: 300,
            max_suggestions_per_hour: 3,
            cooldown_per_category_secs: 600,
            annoyance_threshold: 0.6,
            confidence_threshold: 0.7,
            max_active_suggestions: 5,
        }
    }
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            interrupt_threshold: 0.7,
            silence_threshold: 0.3,
            remind_threshold: 0.6,
            congratulate_threshold: 0.8,
            wait_threshold: 0.4,
            decision_cooldown_ms: 5000,
            max_decisions_per_minute: 10,
        }
    }
}

impl Default for PersonalityConfig {
    fn default() -> Self {
        Self {
            mood_decay_rate: 0.05,
            confidence_update_rate: 0.1,
            curiosity_trigger_threshold: 0.6,
            empathy_response_threshold: 0.5,
            humor_probability: 0.2,
            base_response_length: 50,
            thinking_delay_ms: 200,
        }
    }
}

impl Default for PresenceConfig {
    fn default() -> Self {
        Self {
            state_transition_cooldown_ms: 1000,
            emergency_timeout_secs: 300,
            focus_mode_min_duration_secs: 60,
            celebration_duration_ms: 5000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = IntelligenceConfig::default();
        assert_eq!(config.emotional.signal_decay_rate, 0.1);
        assert_eq!(config.conversation.max_memory_turns, 20);
        assert_eq!(config.memory.base_importance, 0.5);
        assert_eq!(config.proactive.max_suggestions_per_hour, 3);
        assert_eq!(config.decision.interrupt_threshold, 0.7);
        assert_eq!(config.personality.thinking_delay_ms, 200);
        assert_eq!(config.presence.celebration_duration_ms, 5000);
    }

    #[test]
    fn test_config_serialization() {
        let config = IntelligenceConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: IntelligenceConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            config.emotional.signal_decay_rate,
            deserialized.emotional.signal_decay_rate
        );
    }
}
