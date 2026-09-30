//! Decision Engine — evaluates options before every action.
//!
//! Considers risk, benefit, time, confidence, resources, and security
//! before choosing the best course of action.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DecisionId(pub String);

impl Default for DecisionId {
    fn default() -> Self {
        Self::new()
    }
}

impl DecisionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// A possible action to take.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionOption {
    pub id: DecisionId,
    pub name: String,
    pub description: String,
    pub action_type: ActionType,
    pub estimated_time_ms: u64,
    pub resource_cost: ResourceCost,
    pub security_level: SecurityLevel,
    pub reversible: bool,
    pub prerequisites: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    Execute,
    Research,
    Create,
    Modify,
    Delete,
    Communicate,
    Deploy,
    Review,
    Custom(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceCost {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub network_bytes: u64,
    pub disk_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum SecurityLevel {
    #[default]
    None = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

/// Evaluation of a single action option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionEvaluation {
    pub action_id: DecisionId,
    pub risk_score: f64,
    pub benefit_score: f64,
    pub time_score: f64,
    pub confidence: f64,
    pub resource_feasibility: f64,
    pub security_assessment: SecurityAssessment,
    pub overall_score: f64,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAssessment {
    pub level: SecurityLevel,
    pub approved: bool,
    pub concerns: Vec<String>,
}

/// The decision to be made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionContext {
    pub id: DecisionId,
    pub description: String,
    pub goals: Vec<String>,
    pub constraints: Vec<String>,
    pub current_state: HashMap<String, String>,
    pub urgency: Urgency,
    pub max_time_ms: Option<u64>,
    pub preferred_action_type: Option<ActionType>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Urgency {
    Low = 0,
    #[default]
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// The result of a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResult {
    pub context_id: DecisionId,
    pub chosen_action: ActionOption,
    pub evaluation: ActionEvaluation,
    pub all_evaluations: Vec<ActionEvaluation>,
    pub decision_time_ms: u64,
    pub reasoning: String,
}

// ============================================================================
// Decision Engine
// ============================================================================

/// Evaluates options and makes decisions.
pub struct DecisionEngine {
    risk_threshold: f64,
    min_confidence: f64,
    max_security_level: SecurityLevel,
    decision_history: Vec<DecisionResult>,
    max_history: usize,
}

impl DecisionEngine {
    pub fn new(
        risk_threshold: f64,
        min_confidence: f64,
        max_security_level: SecurityLevel,
    ) -> Self {
        Self {
            risk_threshold,
            min_confidence,
            max_security_level,
            decision_history: Vec::new(),
            max_history: 1000,
        }
    }

    pub fn default_engine() -> Self {
        Self::new(0.7, 0.5, SecurityLevel::High)
    }

    /// Evaluate a single action option.
    pub fn evaluate(&self, context: &DecisionContext, action: &ActionOption) -> ActionEvaluation {
        let risk_score = self.calculate_risk(context, action);
        let benefit_score = self.calculate_benefit(context, action);
        let time_score = self.calculate_time_score(context, action);
        let confidence = self.calculate_confidence(context, action);
        let resource_feasibility = self.calculate_resource_feasibility(action);
        let security = self.assess_security(action);

        // Weighted overall score
        let overall = (benefit_score * 0.3)
            + ((1.0 - risk_score) * 0.25)
            + (time_score * 0.2)
            + (confidence * 0.15)
            + (resource_feasibility * 0.1);

        let reasoning = format!(
            "Benefit: {:.2}, Risk: {:.2}, Time: {:.2}, Confidence: {:.2}, Resources: {:.2}, Security: {:?}",
            benefit_score, risk_score, time_score, confidence, resource_feasibility, security.level
        );

        ActionEvaluation {
            action_id: action.id.clone(),
            risk_score,
            benefit_score,
            time_score,
            confidence,
            resource_feasibility,
            security_assessment: security,
            overall_score: overall,
            reasoning,
        }
    }

    /// Make a decision given context and options.
    pub fn decide(
        &mut self,
        context: &DecisionContext,
        options: &[ActionOption],
    ) -> Result<DecisionResult, DecisionError> {
        if options.is_empty() {
            return Err(DecisionError::NoOptionsAvailable);
        }

        let start = std::time::Instant::now();

        let mut evaluations: Vec<ActionEvaluation> = options
            .iter()
            .map(|action| self.evaluate(context, action))
            .collect();

        // Filter out options that exceed risk threshold
        evaluations.retain(|e| e.risk_score <= self.risk_threshold);

        // Filter out options that don't meet minimum confidence
        evaluations.retain(|e| e.confidence >= self.min_confidence);

        // Filter out options that exceed max security level
        evaluations.retain(|e| e.security_assessment.level <= self.max_security_level);

        // If max time is specified, filter out options that take too long
        if let Some(max_time) = context.max_time_ms {
            evaluations.retain(|e| {
                options
                    .iter()
                    .find(|o| o.id == e.action_id)
                    .map(|o| o.estimated_time_ms <= max_time)
                    .unwrap_or(true)
            });
        }

        if evaluations.is_empty() {
            return Err(DecisionError::NoViableOptions);
        }

        // Sort by overall score (descending)
        evaluations.sort_by(|a, b| {
            b.overall_score
                .partial_cmp(&a.overall_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let chosen_eval = evaluations[0].clone();
        let chosen_action = options
            .iter()
            .find(|o| o.id == chosen_eval.action_id)
            .cloned()
            .ok_or(DecisionError::InternalError)?;

        let decision_time = start.elapsed().as_millis() as u64;

        let result = DecisionResult {
            context_id: context.id.clone(),
            chosen_action,
            evaluation: chosen_eval.clone(),
            all_evaluations: evaluations,
            decision_time_ms: decision_time,
            reasoning: chosen_eval.reasoning,
        };

        self.decision_history.push(result.clone());
        if self.decision_history.len() > self.max_history {
            self.decision_history
                .drain(0..self.decision_history.len() - self.max_history);
        }

        Ok(result)
    }

    /// Get decision history.
    pub fn history(&self) -> &[DecisionResult] {
        &self.decision_history
    }

    fn calculate_risk(&self, _context: &DecisionContext, action: &ActionOption) -> f64 {
        let mut risk: f64 = 0.0;

        // Delete actions are high risk
        if action.action_type == ActionType::Delete {
            risk += 0.4;
        }

        // Non-reversible actions are riskier
        if !action.reversible {
            risk += 0.2;
        }

        // Higher security level = higher risk
        risk += match action.security_level {
            SecurityLevel::None => 0.0,
            SecurityLevel::Low => 0.05,
            SecurityLevel::Medium => 0.15,
            SecurityLevel::High => 0.3,
            SecurityLevel::Critical => 0.5,
        };

        risk.clamp(0.0, 1.0)
    }

    fn calculate_benefit(&self, context: &DecisionContext, action: &ActionOption) -> f64 {
        let mut benefit = 0.5; // baseline

        // Higher urgency increases benefit
        benefit += match context.urgency {
            Urgency::Low => 0.0,
            Urgency::Medium => 0.1,
            Urgency::High => 0.2,
            Urgency::Critical => 0.3,
        };

        // Actions that align with goals get bonus
        let goal_alignment = context
            .goals
            .iter()
            .filter(|g| {
                action
                    .description
                    .to_lowercase()
                    .contains(&g.to_lowercase())
            })
            .count() as f64
            * 0.1;
        benefit += goal_alignment.min(0.3);

        benefit.clamp(0.0, 1.0)
    }

    fn calculate_time_score(&self, context: &DecisionContext, action: &ActionOption) -> f64 {
        if let Some(max_time) = context.max_time_ms {
            if action.estimated_time_ms > max_time {
                return 0.0;
            }
            1.0 - (action.estimated_time_ms as f64 / max_time as f64)
        } else {
            // Shorter is better, but not critical
            1.0 - (action.estimated_time_ms as f64 / 60000.0).min(1.0)
        }
    }

    fn calculate_confidence(&self, _context: &DecisionContext, _action: &ActionOption) -> f64 {
        // In a real system, this would use historical data and LLM reasoning
        0.7 // Default confidence
    }

    fn calculate_resource_feasibility(&self, action: &ActionOption) -> f64 {
        let mut score: f64 = 1.0;

        if action.resource_cost.cpu_percent > 80.0 {
            score -= 0.3;
        }
        if action.resource_cost.memory_mb > 1024 {
            score -= 0.2;
        }
        if action.resource_cost.network_bytes > 100 * 1024 * 1024 {
            score -= 0.1;
        }

        score.max(0.0)
    }

    fn assess_security(&self, action: &ActionOption) -> SecurityAssessment {
        let mut concerns = Vec::new();
        let approved = action.security_level <= self.max_security_level;

        if action.action_type == ActionType::Delete {
            concerns.push("Destructive action".to_string());
        }
        if !action.reversible {
            concerns.push("Irreversible action".to_string());
        }
        if action.security_level == SecurityLevel::Critical {
            concerns.push("Critical security level".to_string());
        }

        SecurityAssessment {
            level: action.security_level.clone(),
            approved,
            concerns,
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum DecisionError {
    #[error("No options available")]
    NoOptionsAvailable,

    #[error("No viable options (all filtered by risk/confidence/security)")]
    NoViableOptions,

    #[error("Internal error during decision")]
    InternalError,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_action(name: &str) -> ActionOption {
        ActionOption {
            id: DecisionId::new(),
            name: name.to_string(),
            description: format!("Do {}", name),
            action_type: ActionType::Execute,
            estimated_time_ms: 1000,
            resource_cost: ResourceCost::default(),
            security_level: SecurityLevel::Low,
            reversible: true,
            prerequisites: Vec::new(),
        }
    }

    fn sample_context() -> DecisionContext {
        let mut state = HashMap::new();
        state.insert("mode".to_string(), "normal".to_string());
        DecisionContext {
            id: DecisionId::new(),
            description: "Choose an action".to_string(),
            goals: vec!["complete_task".to_string()],
            constraints: Vec::new(),
            current_state: state,
            urgency: Urgency::Medium,
            max_time_ms: Some(5000),
            preferred_action_type: None,
        }
    }

    #[test]
    fn engine_creation() {
        let _engine = DecisionEngine::default_engine();
    }

    #[test]
    fn evaluate_action() {
        let engine = DecisionEngine::default_engine();
        let context = sample_context();
        let action = sample_action("test");
        let eval = engine.evaluate(&context, &action);
        assert!(eval.overall_score >= 0.0);
        assert!(eval.overall_score <= 1.0);
    }

    #[test]
    fn decide_best_action() {
        let mut engine = DecisionEngine::default_engine();
        let context = sample_context();
        let options = vec![
            sample_action("slow"),
            sample_action("fast"),
            sample_action("medium"),
        ];
        let result = engine.decide(&context, &options).unwrap();
        assert!(!result.all_evaluations.is_empty());
    }

    #[test]
    fn no_options_error() {
        let mut engine = DecisionEngine::default_engine();
        let context = sample_context();
        let result = engine.decide(&context, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn delete_action_high_risk() {
        let engine = DecisionEngine::default_engine();
        let context = sample_context();
        let mut action = sample_action("delete");
        action.action_type = ActionType::Delete;
        action.reversible = false;
        let eval = engine.evaluate(&context, &action);
        assert!(eval.risk_score > 0.3);
    }

    #[test]
    fn critical_security_filtered() {
        let mut engine = DecisionEngine::new(0.9, 0.0, SecurityLevel::Medium);
        let context = sample_context();
        let mut action = sample_action("critical");
        action.security_level = SecurityLevel::Critical;
        let result = engine.decide(&context, &[action]);
        assert!(result.is_err());
    }

    #[test]
    fn decision_history() {
        let mut engine = DecisionEngine::default_engine();
        let context = sample_context();
        let options = vec![sample_action("a")];
        engine.decide(&context, &options).unwrap();
        assert_eq!(engine.history().len(), 1);
    }

    #[test]
    fn resource_feasibility() {
        let engine = DecisionEngine::default_engine();
        let context = sample_context();
        let mut action = sample_action("heavy");
        action.resource_cost.cpu_percent = 90.0;
        action.resource_cost.memory_mb = 2048;
        let eval = engine.evaluate(&context, &action);
        assert!(eval.resource_feasibility < 0.7);
    }

    #[test]
    fn urgency_affects_benefit() {
        let engine = DecisionEngine::default_engine();
        let action = sample_action("test");

        let mut low_ctx = sample_context();
        low_ctx.urgency = Urgency::Low;
        let low_eval = engine.evaluate(&low_ctx, &action);

        let mut high_ctx = sample_context();
        high_ctx.urgency = Urgency::High;
        let high_eval = engine.evaluate(&high_ctx, &action);

        assert!(high_eval.benefit_score > low_eval.benefit_score);
    }
}
