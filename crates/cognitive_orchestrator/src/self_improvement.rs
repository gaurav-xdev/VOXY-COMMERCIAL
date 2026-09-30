//! Self-Improvement Engine — analyzes mistakes, slow ops, user corrections.
//!
//! Continuously learns from errors and generates optimization suggestions.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InsightId(pub String);

impl Default for InsightId {
    fn default() -> Self {
        Self::new()
    }
}

impl InsightId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// Type of improvement insight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsightType {
    /// User corrected VOXY's output.
    UserCorrection,
    /// An operation took too long.
    SlowOperation,
    /// A repeated command pattern.
    RepeatedPattern,
    /// A failure that could be prevented.
    PreventableFailure,
    /// Resource usage optimization.
    ResourceOptimization,
    /// Workflow improvement suggestion.
    WorkflowImprovement,
}

/// An improvement insight discovered by the engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub id: InsightId,
    pub insight_type: InsightType,
    pub title: String,
    pub description: String,
    pub evidence: Vec<String>,
    pub suggested_action: String,
    pub confidence: f64,
    pub impact_score: f64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: InsightStatus,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InsightStatus {
    New,
    UnderReview,
    Accepted,
    Implemented,
    Rejected,
}

/// A performance metric snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub operation: String,
    pub duration_ms: u64,
    pub success: bool,
    pub error: Option<String>,
    pub context: HashMap<String, String>,
}

/// Pattern detected in user behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorPattern {
    pub pattern_type: String,
    pub frequency: u32,
    pub examples: Vec<String>,
    pub suggestion: String,
    pub confidence: f64,
}

// ============================================================================
// Self-Improvement Engine
// ============================================================================

/// Analyzes performance and generates improvement insights.
pub struct SelfImprovementEngine {
    insights: Vec<Insight>,
    performance_log: Vec<PerformanceSnapshot>,
    correction_count: HashMap<String, u32>,
    max_insights: usize,
    max_performance_log: usize,
    slow_threshold_ms: u64,
}

impl SelfImprovementEngine {
    pub fn new(max_insights: usize, max_performance_log: usize, slow_threshold_ms: u64) -> Self {
        Self {
            insights: Vec::new(),
            performance_log: Vec::new(),
            correction_count: HashMap::new(),
            max_insights,
            max_performance_log,
            slow_threshold_ms,
        }
    }

    pub fn default_engine() -> Self {
        Self::new(500, 5000, 5000)
    }

    /// Record a performance snapshot.
    pub fn record_operation(&mut self, snapshot: PerformanceSnapshot) {
        // Check for slow operation
        if snapshot.duration_ms > self.slow_threshold_ms {
            self.add_insight(Insight {
                id: InsightId::new(),
                insight_type: InsightType::SlowOperation,
                title: format!("Slow operation: {}", snapshot.operation),
                description: format!(
                    "Operation '{}' took {}ms (threshold: {}ms)",
                    snapshot.operation, snapshot.duration_ms, self.slow_threshold_ms
                ),
                evidence: vec![format!(
                    "Duration: {}ms, Success: {}",
                    snapshot.duration_ms, snapshot.success
                )],
                suggested_action: format!("Consider optimizing '{}'", snapshot.operation),
                confidence: 0.8,
                impact_score: (snapshot.duration_ms as f64 / self.slow_threshold_ms as f64)
                    .min(1.0),
                created_at: chrono::Utc::now(),
                status: InsightStatus::New,
                tags: vec!["performance".to_string()],
            });
        }

        // Check for failures
        if !snapshot.success {
            if let Some(error) = &snapshot.error {
                self.add_insight(Insight {
                    id: InsightId::new(),
                    insight_type: InsightType::PreventableFailure,
                    title: format!("Operation failed: {}", snapshot.operation),
                    description: format!(
                        "Operation '{}' failed with: {}",
                        snapshot.operation, error
                    ),
                    evidence: vec![error.clone()],
                    suggested_action: "Investigate root cause and add error handling".to_string(),
                    confidence: 0.7,
                    impact_score: 0.6,
                    created_at: chrono::Utc::now(),
                    status: InsightStatus::New,
                    tags: vec!["error".to_string()],
                });
            }
        }

        self.performance_log.push(snapshot);
        if self.performance_log.len() > self.max_performance_log {
            self.performance_log
                .drain(0..self.performance_log.len() - self.max_performance_log);
        }
    }

    /// Record a user correction.
    pub fn record_correction(&mut self, original: &str, corrected: &str, context: &str) {
        let key = format!("{}:{}", original, corrected);
        *self.correction_count.entry(key.clone()).or_insert(0) += 1;

        let count = self.correction_count[&key];
        if count >= 2 {
            self.add_insight(Insight {
                id: InsightId::new(),
                insight_type: InsightType::UserCorrection,
                title: format!("Repeated correction: {} -> {}", original, corrected),
                description: format!(
                    "User has corrected '{}' to '{}' {} times in context: {}",
                    original, corrected, count, context
                ),
                evidence: vec![
                    format!("Original: {}", original),
                    format!("Corrected: {}", corrected),
                    format!("Context: {}", context),
                    format!("Count: {}", count),
                ],
                suggested_action: format!(
                    "Remember that '{}' should be '{}' in this context",
                    original, corrected
                ),
                confidence: (count as f64 / 10.0).min(0.95),
                impact_score: 0.5 + (count as f64 * 0.05).min(0.3),
                created_at: chrono::Utc::now(),
                status: InsightStatus::New,
                tags: vec!["correction".to_string(), "learning".to_string()],
            });
        }
    }

    /// Analyze performance log for patterns.
    pub fn analyze_patterns(&self) -> Vec<BehaviorPattern> {
        let mut patterns = Vec::new();

        // Find repeated operations
        let mut op_counts: HashMap<String, u32> = HashMap::new();
        for snapshot in &self.performance_log {
            *op_counts.entry(snapshot.operation.clone()).or_insert(0) += 1;
        }

        for (op, count) in &op_counts {
            if *count >= 5 {
                patterns.push(BehaviorPattern {
                    pattern_type: "repeated_operation".to_string(),
                    frequency: *count,
                    examples: vec![op.clone()],
                    suggestion: format!("Consider caching or optimizing '{}'", op),
                    confidence: (*count as f64 / 100.0).min(0.9),
                });
            }
        }

        // Find slow operations
        let mut slow_ops: HashMap<String, Vec<u64>> = HashMap::new();
        for snapshot in &self.performance_log {
            if snapshot.duration_ms > self.slow_threshold_ms {
                slow_ops
                    .entry(snapshot.operation.clone())
                    .or_default()
                    .push(snapshot.duration_ms);
            }
        }

        for (op, durations) in &slow_ops {
            let avg = durations.iter().sum::<u64>() as f64 / durations.len() as f64;
            patterns.push(BehaviorPattern {
                pattern_type: "slow_operation".to_string(),
                frequency: durations.len() as u32,
                examples: vec![format!("Avg: {:.0}ms", avg)],
                suggestion: format!("Optimize '{}' (avg {:.0}ms)", op, avg),
                confidence: 0.8,
            });
        }

        patterns
    }

    /// Get all insights.
    pub fn insights(&self) -> &[Insight] {
        &self.insights
    }

    /// Get new (unprocessed) insights.
    pub fn new_insights(&self) -> Vec<&Insight> {
        self.insights
            .iter()
            .filter(|i| i.status == InsightStatus::New)
            .collect()
    }

    /// Get high-impact insights.
    pub fn high_impact_insights(&self) -> Vec<&Insight> {
        self.insights
            .iter()
            .filter(|i| i.impact_score >= 0.7)
            .collect()
    }

    /// Update insight status.
    pub fn update_insight_status(
        &mut self,
        id: &InsightId,
        status: InsightStatus,
    ) -> Result<(), ImprovementError> {
        let insight = self
            .insights
            .iter_mut()
            .find(|i| i.id == *id)
            .ok_or_else(|| ImprovementError::InsightNotFound(id.0.clone()))?;
        insight.status = status;
        Ok(())
    }

    /// Get performance log.
    pub fn performance_log(&self) -> &[PerformanceSnapshot] {
        &self.performance_log
    }

    /// Get correction counts.
    pub fn correction_counts(&self) -> &HashMap<String, u32> {
        &self.correction_count
    }

    fn add_insight(&mut self, insight: Insight) {
        self.insights.push(insight);
        if self.insights.len() > self.max_insights {
            self.insights
                .drain(0..self.insights.len() - self.max_insights);
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum ImprovementError {
    #[error("Insight not found: {0}")]
    InsightNotFound(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_creation() {
        let _engine = SelfImprovementEngine::default_engine();
    }

    #[test]
    fn record_slow_operation() {
        let mut engine = SelfImprovementEngine::default_engine();
        engine.record_operation(PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            operation: "transcribe".to_string(),
            duration_ms: 10000,
            success: true,
            error: None,
            context: HashMap::new(),
        });
        assert!(!engine.insights().is_empty());
        assert_eq!(
            engine.insights()[0].insight_type,
            InsightType::SlowOperation
        );
    }

    #[test]
    fn record_failure() {
        let mut engine = SelfImprovementEngine::default_engine();
        engine.record_operation(PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            operation: "api_call".to_string(),
            duration_ms: 100,
            success: false,
            error: Some("timeout".to_string()),
            context: HashMap::new(),
        });
        assert!(!engine.insights().is_empty());
        assert_eq!(
            engine.insights()[0].insight_type,
            InsightType::PreventableFailure
        );
    }

    #[test]
    fn record_correction_creates_insight() {
        let mut engine = SelfImprovementEngine::default_engine();
        for _ in 0..3 {
            engine.record_correction("voxy", "VOXY", "naming");
        }
        let insights = engine.new_insights();
        assert!(!insights.is_empty());
        assert_eq!(insights[0].insight_type, InsightType::UserCorrection);
    }

    #[test]
    fn correction_count_increases() {
        let mut engine = SelfImprovementEngine::default_engine();
        engine.record_correction("a", "b", "ctx");
        engine.record_correction("a", "b", "ctx");
        assert_eq!(engine.correction_counts()["a:b"], 2);
    }

    #[test]
    fn analyze_patterns() {
        let mut engine = SelfImprovementEngine::default_engine();
        for _ in 0..10 {
            engine.record_operation(PerformanceSnapshot {
                timestamp: chrono::Utc::now(),
                operation: "transcribe".to_string(),
                duration_ms: 100,
                success: true,
                error: None,
                context: HashMap::new(),
            });
        }
        let patterns = engine.analyze_patterns();
        assert!(!patterns.is_empty());
    }

    #[test]
    fn update_insight_status() {
        let mut engine = SelfImprovementEngine::default_engine();
        engine.record_operation(PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            operation: "test".to_string(),
            duration_ms: 10000,
            success: true,
            error: None,
            context: HashMap::new(),
        });
        let id = engine.insights()[0].id.clone();
        engine
            .update_insight_status(&id, InsightStatus::Accepted)
            .unwrap();
        assert_eq!(engine.insights()[0].status, InsightStatus::Accepted);
    }

    #[test]
    fn high_impact_insights() {
        let mut engine = SelfImprovementEngine::default_engine();
        engine.record_operation(PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            operation: "critical_op".to_string(),
            duration_ms: 60000,
            success: true,
            error: None,
            context: HashMap::new(),
        });
        let high = engine.high_impact_insights();
        assert!(!high.is_empty());
    }

    #[test]
    fn fast_operation_no_insight() {
        let mut engine = SelfImprovementEngine::default_engine();
        engine.record_operation(PerformanceSnapshot {
            timestamp: chrono::Utc::now(),
            operation: "fast".to_string(),
            duration_ms: 10,
            success: true,
            error: None,
            context: HashMap::new(),
        });
        assert!(engine.insights().is_empty());
    }

    #[test]
    fn insight_not_found_error() {
        let mut engine = SelfImprovementEngine::default_engine();
        let result = engine.update_insight_status(&InsightId::new(), InsightStatus::Accepted);
        assert!(result.is_err());
    }
}
