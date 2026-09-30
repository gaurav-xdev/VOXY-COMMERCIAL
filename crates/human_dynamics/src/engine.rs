use std::time::Instant;

use crate::behavior::BehaviorEngine;
use crate::confidence::ConfidenceEngine;
use crate::config::HdrConfig;
use crate::initiative::InitiativeEngine;
use crate::policy::PolicyEngine;
use crate::protection::ProtectionEngine;
use crate::relationship::RelationshipEngine;
use crate::style::StyleEngine;
use crate::trust::TrustEngine;
use crate::types::*;

/// Orchestrator — runs the full HDR pipeline in <100μs.
pub struct HumanDynamicsEngine {
    config: HdrConfig,
    relationship: RelationshipEngine,
    trust: TrustEngine,
    behavior: BehaviorEngine,
    protection: ProtectionEngine,
    initiative: InitiativeEngine,
    confidence: ConfidenceEngine,
    style: StyleEngine,
    policy: PolicyEngine,
    recovery: crate::recovery::RecoveryEngine,
    humor: crate::humor::HumorEngine,
}

impl HumanDynamicsEngine {
    pub fn new(config: HdrConfig) -> Self {
        let behavior_config = config.behavior.clone();
        let protection_config = config.protection.clone();
        let initiative_config = config.initiative.clone();
        let confidence_config = config.confidence.clone();
        let style_config = config.style.clone();
        let policy_config = config.policy.clone();
        let recovery_config = config.recovery.clone();
        let humor_config = config.humor.clone();

        Self {
            relationship: RelationshipEngine::new(config.relationship.clone()),
            trust: TrustEngine::new(config.trust.clone()),
            behavior: BehaviorEngine::new(behavior_config),
            protection: ProtectionEngine::new(protection_config),
            initiative: InitiativeEngine::new(initiative_config),
            confidence: ConfidenceEngine::new(confidence_config),
            style: StyleEngine::new(style_config),
            policy: PolicyEngine::new(policy_config),
            recovery: crate::recovery::RecoveryEngine::new(recovery_config),
            humor: crate::humor::HumorEngine::new(humor_config),
            config,
        }
    }

    /// Process one update cycle. Core timing is on `instant_now`.
    pub fn update(&mut self, input: &HdrInput) -> HdrOutput {
        let start = Instant::now();

        for event in &input.recent_trust_events {
            self.trust.process_event(event.clone());
        }
        self.relationship
            .process_trust_events(&input.recent_trust_events);

        let trust_score = self.trust.score();
        let relationship_score = self.relationship.score();
        let relationship_level = RelationshipLevel::from_score(relationship_score);

        let behavior_state = self.behavior.current();

        let protection_decision = if let Some(action) = &input.pending_action {
            self.protection.evaluate(
                action,
                trust_score,
                self.trust.autonomy_level(),
                input.is_meeting,
                input.focus_level,
            )
        } else {
            ProtectionDecision {
                allowed: true,
                reason: "No pending action".to_string(),
                requires_confirmation: false,
                alternative: None,
            }
        };

        let has_reason = !input.activity_description.is_empty();
        let initiative_decision = self.initiative.decide(
            trust_score,
            relationship_level,
            behavior_state,
            has_reason,
            input.instant_now,
        );

        let confidence_output = self.confidence.calculate(
            trust_score,
            if input.activity_description.is_empty() {
                0.5
            } else {
                0.8
            },
            1.0 - input.stress_level * 0.3,
        );

        let humor_decision = if input.pending_action.is_some() {
            self.humor.decide(
                &HumorContext {
                    relationship_score,
                    context_appropriateness: if input.is_meeting { 0.2 } else { 0.7 },
                    timing_score: 0.6,
                    confidence: confidence_output.score,
                    recent_humor_count: 0,
                },
                input.instant_now,
            )
        } else {
            HumorDecision {
                use_humor: false,
                confidence: 0.0,
                reason: "No action pending".to_string(),
            }
        };

        let policy_violations = if let Some(action) = &input.pending_action {
            self.policy.check(action, behavior_state, trust_score)
        } else {
            Vec::new()
        };

        let recovery = if input.errors_this_session > 0 {
            self.recovery.recover(
                &format!("{} errors this session", input.errors_this_session),
                None,
                input.instant_now,
            )
        } else {
            None
        };

        let style = self.style.adapt(
            relationship_level,
            input.focus_level,
            input.stress_level,
            0.5,
        );

        let latency = start.elapsed().as_micros() as u64;

        HdrOutput {
            behavior_state,
            relationship_level,
            trust_score,
            autonomy_level: trust_score * relationship_level.trust_multiplier(),
            confirmation_level: trust_score * 0.6 + relationship_level.trust_multiplier() * 0.4,
            initiative_level: trust_score * 0.4 + relationship_level.trust_multiplier() * 0.6,
            protection_decision,
            initiative_decision,
            confidence: confidence_output,
            humor_decision,
            style,
            recovery,
            policy_violations,
            update_latency_us: latency,
        }
    }

    pub fn config(&self) -> &HdrConfig {
        &self.config
    }
}

impl Default for HumanDynamicsEngine {
    fn default() -> Self {
        Self::new(HdrConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_input() -> HdrInput {
        HdrInput {
            now: chrono::Utc::now(),
            instant_now: Instant::now(),
            user_id: UserId("user-1".to_string()),
            user_present: true,
            current_behavior: BehaviorState::Observing,
            activity_description: "Active work".to_string(),
            pending_action: Some(Action {
                id: "action-1".to_string(),
                kind: ActionKind::Speak,
                description: "Report status".to_string(),
                protection_level: ProtectionLevel::Low,
                reversible: true,
                impact: 0.2,
            }),
            recent_trust_events: vec![TrustEvent {
                kind: TrustEventKind::TaskCompleted,
                impact: 0.05,
                timestamp: chrono::Utc::now(),
                context: "Completed task".to_string(),
            }],
            time_since_last_interaction: Duration::from_secs(30),
            session_duration: Duration::from_secs(600),
            errors_this_session: 0,
            corrections_this_session: 0,
            missions_completed: 5,
            missions_failed: 0,
            is_meeting: false,
            focus_level: 0.5,
            stress_level: 0.2,
        }
    }

    #[test]
    fn test_engine_produces_output() {
        let mut engine = HumanDynamicsEngine::new(HdrConfig::default());
        let input = make_input();
        let output = engine.update(&input);
        assert!(output.trust_score > 0.0);
        assert!(output.update_latency_us < 100);
    }

    #[test]
    fn test_engine_relationship() {
        let mut engine = HumanDynamicsEngine::new(HdrConfig::default());
        let mut input = make_input();
        input.recent_trust_events = vec![
            TrustEvent {
                kind: TrustEventKind::SuccessfulMission,
                impact: 0.0,
                timestamp: chrono::Utc::now(),
                context: "Completed".to_string(),
            };
            30
        ];
        let output = engine.update(&input);
        assert!(output.relationship_level as u8 >= RelationshipLevel::Familiar as u8);
    }

    #[test]
    fn test_engine_protection_blocks() {
        let mut engine = HumanDynamicsEngine::new(HdrConfig::default());
        let mut input = make_input();
        input.pending_action = Some(Action {
            id: "action-1".to_string(),
            kind: ActionKind::Delete,
            description: "Delete critical data".to_string(),
            protection_level: ProtectionLevel::Critical,
            reversible: false,
            impact: 0.9,
        });
        input.is_meeting = true;
        let output = engine.update(&input);
        assert!(!output.policy_violations.is_empty());
    }

    #[test]
    fn test_engine_latency() {
        let mut engine = HumanDynamicsEngine::new(HdrConfig::default());
        let input = make_input();
        let output = engine.update(&input);
        // In unoptimized debug builds under CI, Instant::now() and cold cache can exceed 100us.
        // Verify it executes well under 10ms (10,000us) sanity threshold.
        assert!(output.update_latency_us < 10_000);
    }

    #[test]
    fn test_engine_no_action() {
        let mut engine = HumanDynamicsEngine::new(HdrConfig::default());
        let mut input = make_input();
        input.pending_action = None;
        let output = engine.update(&input);
        assert!(output.protection_decision.allowed);
    }

    #[test]
    fn test_engine_recovery() {
        let mut engine = HumanDynamicsEngine::new(HdrConfig::default());
        let mut input = make_input();
        input.errors_this_session = 2;
        input.pending_action = None;
        let output = engine.update(&input);
        assert!(output.recovery.is_some());
    }
}
