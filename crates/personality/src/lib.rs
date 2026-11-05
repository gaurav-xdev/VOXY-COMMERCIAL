pub mod behavior_spec;
pub mod config;
pub mod error;
pub mod event;
pub mod traits;

pub use behavior_spec::{InteractionContext, OsmooBehaviorSpecification};
pub use config::PersonalityConfig;
pub use error::{PersonalityError, Result};
pub use event::PersonalityEvent;
pub use traits::*;

pub mod prelude {
    pub use crate::config::PersonalityConfig;
    pub use crate::error::{PersonalityError, Result};
    pub use crate::event::PersonalityEvent;
    pub use crate::traits::{
        CharacterTrait, CommunicationStyle, MoodState, PersonalityManager, PersonalityProfile,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::PersonalityError;
    use crate::traits::{CharacterTrait, CommunicationStyle, MoodState};
    use std::collections::HashMap;

    #[test]
    fn test_character_trait_creation() {
        let t = CharacterTrait::Warmth(0.8);
        if let CharacterTrait::Warmth(v) = t {
            assert!((v - 0.8).abs() < f64::EPSILON);
        } else {
            panic!("Expected Warmth");
        }
    }

    #[test]
    fn test_character_trait_bounds() {
        let t = CharacterTrait::Humor(0.0);
        if let CharacterTrait::Humor(v) = t {
            assert!(v >= 0.0);
        }
        let t2 = CharacterTrait::Humor(1.0);
        if let CharacterTrait::Humor(v) = t2 {
            assert!(v <= 1.0);
        }
    }

    #[test]
    fn test_character_trait_custom() {
        let t = CharacterTrait::Custom("curiosity".to_string(), 0.75);
        if let CharacterTrait::Custom(name, v) = t {
            assert_eq!(name, "curiosity");
            assert!((v - 0.75).abs() < f64::EPSILON);
        } else {
            panic!("Expected Custom");
        }
    }

    #[test]
    fn test_character_trait_equality() {
        assert_eq!(CharacterTrait::Warmth(0.5), CharacterTrait::Warmth(0.5));
        assert_ne!(CharacterTrait::Warmth(0.5), CharacterTrait::Warmth(0.6));
    }

    #[test]
    fn test_mood_state_variants() {
        let moods = vec![
            MoodState::Neutral,
            MoodState::Happy,
            MoodState::Sad,
            MoodState::Anxious,
            MoodState::Excited,
            MoodState::Calm,
            MoodState::Frustrated,
            MoodState::Playful,
            MoodState::Serious,
            MoodState::Tired,
            MoodState::Custom("melancholy".to_string()),
        ];
        assert_eq!(moods.len(), 11);
    }

    #[test]
    fn test_mood_state_equality() {
        assert_eq!(MoodState::Neutral, MoodState::Neutral);
        assert_ne!(MoodState::Happy, MoodState::Sad);
    }

    #[test]
    fn test_communication_style_display() {
        assert_eq!(format!("{}", CommunicationStyle::Casual), "Casual");
        assert_eq!(format!("{}", CommunicationStyle::Formal), "Formal");
        assert_eq!(
            format!("{}", CommunicationStyle::Professional),
            "Professional"
        );
        assert_eq!(format!("{}", CommunicationStyle::Friendly), "Friendly");
        assert_eq!(
            format!("{}", CommunicationStyle::Authoritative),
            "Authoritative"
        );
        assert_eq!(format!("{}", CommunicationStyle::Empathetic), "Empathetic");
        assert_eq!(format!("{}", CommunicationStyle::Playful), "Playful");
        assert_eq!(
            format!("{}", CommunicationStyle::Custom("quirky".to_string())),
            "Custom(quirky)"
        );
    }

    #[test]
    fn test_mood_state_display() {
        assert_eq!(format!("{}", MoodState::Neutral), "Neutral");
        assert_eq!(
            format!("{}", MoodState::Custom("sleepy".to_string())),
            "Custom(sleepy)"
        );
    }

    #[test]
    fn test_personality_config_default() {
        let cfg = PersonalityConfig::default();
        assert_eq!(cfg.profile_id, "default");
        assert_eq!(cfg.profile_name, "Default Profile");
        assert!(cfg.traits.is_empty());
        assert_eq!(cfg.default_mood, MoodState::Neutral);
        assert!(cfg.allow_mood_transitions);
        assert_eq!(cfg.mood_transition_interval_seconds, 300);
        assert_eq!(cfg.communication_style, CommunicationStyle::Casual);
    }

    #[test]
    fn test_personality_config_custom() {
        let mut traits = HashMap::new();
        traits.insert("warmth".to_string(), 0.8);
        let cfg = PersonalityConfig {
            profile_id: "custom1".to_string(),
            profile_name: "Custom".to_string(),
            traits,
            default_mood: MoodState::Happy,
            allow_mood_transitions: false,
            mood_transition_interval_seconds: 600,
            communication_style: CommunicationStyle::Formal,
        };
        assert_eq!(cfg.profile_id, "custom1");
        assert_eq!(cfg.traits.get("warmth"), Some(&0.8));
        assert_eq!(cfg.default_mood, MoodState::Happy);
    }

    #[test]
    fn test_personality_error_display() {
        let err = PersonalityError::InvalidConfig("missing traits".to_string());
        assert_eq!(
            format!("{}", err),
            "Invalid personality configuration: missing traits"
        );

        let err = PersonalityError::ProfileNotFound("profile_1".to_string());
        assert_eq!(format!("{}", err), "Profile not found: profile_1");

        let err = PersonalityError::TraitValueOutOfRange("value > 1.0".to_string());
        assert_eq!(format!("{}", err), "Trait value out of range: value > 1.0");
    }

    #[test]
    fn test_personality_error_error_trait() {
        use std::error::Error;
        let err = PersonalityError::InvalidConfig("test".to_string());
        assert!(err.source().is_none());
    }
}
