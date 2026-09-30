use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SuggestionType {
    Tip,
    Shortcut,
    Reminder,
    Optimization,
    Celebration,
    Warning,
    Encouragement,
    ContextSwitch,
    BreakReminder,
    Learning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveSuggestion {
    pub id: String,
    pub suggestion_type: SuggestionType,
    pub message: String,
    pub confidence: f64,
    pub priority: f64,
    pub context: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub shown: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CooldownEntry {
    pub suggestion_type: SuggestionType,
    pub last_shown: DateTime<Utc>,
    pub count: usize,
    pub avg_interval_ms: u64,
}

pub struct ProactiveEngine {
    suggestions: Vec<ProactiveSuggestion>,
    cooldowns: HashMap<SuggestionType, CooldownEntry>,
    config: ProactiveConfig,
    annoyance_score: f64,
}

#[derive(Debug, Clone)]
pub struct ProactiveConfig {
    pub cooldown_ms: u64,
    pub max_suggestions_per_hour: usize,
    pub annoyance_threshold: f64,
    pub annoyance_decay_rate: f64,
    pub max_suggestions_per_session: usize,
    pub confidence_threshold: f64,
}

impl Default for ProactiveConfig {
    fn default() -> Self {
        Self {
            cooldown_ms: 300_000,
            max_suggestions_per_hour: 5,
            annoyance_threshold: 0.7,
            annoyance_decay_rate: 0.05,
            max_suggestions_per_session: 20,
            confidence_threshold: 0.6,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SuggestionContext {
    pub current_app: Option<String>,
    pub current_activity: Option<String>,
    pub time_of_day: String,
    pub session_duration_ms: u64,
    pub recent_errors: usize,
    pub recent_completions: usize,
    pub typing_speed: f64,
    pub idle_time_ms: u64,
}

impl ProactiveEngine {
    pub fn new(config: ProactiveConfig) -> Self {
        Self {
            suggestions: Vec::new(),
            cooldowns: HashMap::new(),
            config,
            annoyance_score: 0.0,
        }
    }

    pub fn generate_suggestions(
        &mut self,
        context: &SuggestionContext,
    ) -> Vec<ProactiveSuggestion> {
        if self.annoyance_score >= self.config.annoyance_threshold {
            return vec![];
        }

        let mut candidates = Vec::new();

        candidates.extend(self.generate_tip_suggestions(context));
        candidates.extend(self.generate_shortcut_suggestions(context));
        candidates.extend(self.generate_break_suggestions(context));
        candidates.extend(self.generate_encouragement_suggestions(context));
        candidates.extend(self.generate_learning_suggestions(context));

        candidates.sort_by(|a, b| {
            b.priority
                .partial_cmp(&a.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let filtered: Vec<ProactiveSuggestion> = candidates
            .into_iter()
            .filter(|s| self.should_show(s))
            .take(
                self.config
                    .max_suggestions_per_session
                    .saturating_sub(self.suggestions.len()),
            )
            .collect();

        for suggestion in &filtered {
            self.record_cooldown(suggestion.suggestion_type);
            self.suggestions.push(suggestion.clone());
        }

        filtered
    }

    pub fn mark_shown(&mut self, suggestion_id: &str) {
        if let Some(suggestion) = self.suggestions.iter_mut().find(|s| s.id == suggestion_id) {
            suggestion.shown = true;
            self.annoyance_score = (self.annoyance_score + 0.1).min(1.0);
        }
    }

    pub fn dismissed(&mut self, suggestion_id: &str) {
        if let Some(suggestion) = self.suggestions.iter_mut().find(|s| s.id == suggestion_id) {
            suggestion.shown = true;
            self.annoyance_score = (self.annoyance_score + 0.2).min(1.0);
        }
    }

    pub fn accepted(&mut self, suggestion_id: &str) {
        if let Some(suggestion) = self.suggestions.iter_mut().find(|s| s.id == suggestion_id) {
            suggestion.shown = true;
            self.annoyance_score = (self.annoyance_score - 0.1).max(0.0);
        }
    }

    pub fn decay_annoyance(&mut self) {
        self.annoyance_score = (self.annoyance_score - self.config.annoyance_decay_rate).max(0.0);
    }

    pub fn suggestions(&self) -> &[ProactiveSuggestion] {
        &self.suggestions
    }

    pub fn annoyance_score(&self) -> f64 {
        self.annoyance_score
    }

    pub fn reset_session(&mut self) {
        self.suggestions.clear();
        self.annoyance_score = 0.0;
    }

    fn should_show(&self, suggestion: &ProactiveSuggestion) -> bool {
        if suggestion.confidence < self.config.confidence_threshold {
            return false;
        }

        if let Some(entry) = self.cooldowns.get(&suggestion.suggestion_type) {
            let elapsed = (Utc::now() - entry.last_shown).num_milliseconds() as u64;
            if elapsed < self.config.cooldown_ms {
                return false;
            }
            if entry.count >= self.config.max_suggestions_per_hour {
                return false;
            }
        }

        if let Some(expires) = suggestion.expires_at {
            if Utc::now() > expires {
                return false;
            }
        }

        true
    }

    fn record_cooldown(&mut self, suggestion_type: SuggestionType) {
        let entry = self
            .cooldowns
            .entry(suggestion_type)
            .or_insert_with(|| CooldownEntry {
                suggestion_type,
                last_shown: Utc::now(),
                count: 0,
                avg_interval_ms: self.config.cooldown_ms,
            });

        entry.last_shown = Utc::now();
        entry.count += 1;
    }

    fn generate_tip_suggestions(&self, context: &SuggestionContext) -> Vec<ProactiveSuggestion> {
        let mut suggestions = Vec::new();

        if let Some(app) = &context.current_app {
            if app.contains("code") || app.contains("rust") {
                suggestions.push(ProactiveSuggestion {
                    id: Uuid::new_v4().to_string(),
                    suggestion_type: SuggestionType::Tip,
                    message: "Try Ctrl+Shift+P to open the command palette".to_string(),
                    confidence: 0.7,
                    priority: 0.6,
                    context: HashMap::new(),
                    created_at: Utc::now(),
                    expires_at: Some(Utc::now() + chrono::Duration::minutes(5)),
                    shown: false,
                });
            }
        }

        suggestions
    }

    fn generate_shortcut_suggestions(
        &self,
        context: &SuggestionContext,
    ) -> Vec<ProactiveSuggestion> {
        let mut suggestions = Vec::new();

        if context.typing_speed > 60.0 {
            suggestions.push(ProactiveSuggestion {
                id: Uuid::new_v4().to_string(),
                suggestion_type: SuggestionType::Shortcut,
                message: "You type fast! Try using snippets for common patterns".to_string(),
                confidence: 0.6,
                priority: 0.5,
                context: HashMap::new(),
                created_at: Utc::now(),
                expires_at: None,
                shown: false,
            });
        }

        suggestions
    }

    fn generate_break_suggestions(&self, context: &SuggestionContext) -> Vec<ProactiveSuggestion> {
        let mut suggestions = Vec::new();

        if context.session_duration_ms > 3_600_000 {
            suggestions.push(ProactiveSuggestion {
                id: Uuid::new_v4().to_string(),
                suggestion_type: SuggestionType::BreakReminder,
                message: "You've been coding for over an hour. Time for a break?".to_string(),
                confidence: 0.8,
                priority: 0.7,
                context: HashMap::new(),
                created_at: Utc::now(),
                expires_at: Some(Utc::now() + chrono::Duration::minutes(10)),
                shown: false,
            });
        }

        suggestions
    }

    fn generate_encouragement_suggestions(
        &self,
        context: &SuggestionContext,
    ) -> Vec<ProactiveSuggestion> {
        let mut suggestions = Vec::new();

        if context.recent_completions > 3 {
            suggestions.push(ProactiveSuggestion {
                id: Uuid::new_v4().to_string(),
                suggestion_type: SuggestionType::Celebration,
                message: "Great progress! You've completed several tasks today".to_string(),
                confidence: 0.9,
                priority: 0.8,
                context: HashMap::new(),
                created_at: Utc::now(),
                expires_at: None,
                shown: false,
            });
        }

        if context.recent_errors > 5 {
            suggestions.push(ProactiveSuggestion {
                id: Uuid::new_v4().to_string(),
                suggestion_type: SuggestionType::Encouragement,
                message: "遇到困难了？别担心，每个 bug 都是学习的机会".to_string(),
                confidence: 0.7,
                priority: 0.6,
                context: HashMap::new(),
                created_at: Utc::now(),
                expires_at: None,
                shown: false,
            });
        }

        suggestions
    }

    fn generate_learning_suggestions(
        &self,
        context: &SuggestionContext,
    ) -> Vec<ProactiveSuggestion> {
        let mut suggestions = Vec::new();

        if let Some(activity) = &context.current_activity {
            if activity == "coding" && context.session_duration_ms > 1_800_000 {
                suggestions.push(ProactiveSuggestion {
                    id: Uuid::new_v4().to_string(),
                    suggestion_type: SuggestionType::Learning,
                    message: "Consider taking notes on what you've learned today".to_string(),
                    confidence: 0.6,
                    priority: 0.4,
                    context: HashMap::new(),
                    created_at: Utc::now(),
                    expires_at: None,
                    shown: false,
                });
            }
        }

        suggestions
    }
}

impl Default for ProactiveEngine {
    fn default() -> Self {
        Self::new(ProactiveConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_context() -> SuggestionContext {
        SuggestionContext {
            current_app: Some("VS Code".to_string()),
            current_activity: Some("coding".to_string()),
            time_of_day: "afternoon".to_string(),
            session_duration_ms: 3_600_000,
            recent_errors: 2,
            recent_completions: 5,
            typing_speed: 70.0,
            idle_time_ms: 0,
        }
    }

    #[test]
    fn test_proactive_engine_creation() {
        let engine = ProactiveEngine::new(ProactiveConfig::default());
        assert_eq!(engine.suggestions().len(), 0);
        assert_eq!(engine.annoyance_score(), 0.0);
    }

    #[test]
    fn test_generate_suggestions() {
        let mut engine = ProactiveEngine::default();
        let context = create_test_context();
        let suggestions = engine.generate_suggestions(&context);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_annoyance_threshold() {
        let mut engine = ProactiveEngine {
            annoyance_score: 0.8,
            ..Default::default()
        };
        let context = create_test_context();
        let suggestions = engine.generate_suggestions(&context);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_mark_shown_increases_annoyance() {
        let mut engine = ProactiveEngine::default();
        let context = create_test_context();
        let suggestions = engine.generate_suggestions(&context);
        if let Some(s) = suggestions.first() {
            engine.mark_shown(&s.id);
            assert!(engine.annoyance_score() > 0.0);
        }
    }

    #[test]
    fn test_accepted_decreases_annoyance() {
        let mut engine = ProactiveEngine {
            annoyance_score: 0.5,
            ..Default::default()
        };
        let context = create_test_context();
        let suggestions = engine.generate_suggestions(&context);
        if let Some(s) = suggestions.first() {
            engine.accepted(&s.id);
            assert!(engine.annoyance_score() < 0.5);
        }
    }

    #[test]
    fn test_reset_session() {
        let mut engine = ProactiveEngine::default();
        let context = create_test_context();
        engine.generate_suggestions(&context);
        engine.reset_session();
        assert_eq!(engine.suggestions().len(), 0);
        assert_eq!(engine.annoyance_score(), 0.0);
    }
}
