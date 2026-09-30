use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::attention::{
    AttentionFactors, AttentionRecommendation, AttentionScore, AttentionSystem,
};
use crate::confidence::ConfidenceEstimator;
use crate::config::CognitionConfig;
use crate::context::{AssembledContext, ContextAssembler, ContextAssemblyInput, ContextSource};
use crate::diagnostics::{ContextSummary, Diagnostics, DiagnosticsSnapshot, LatencyDiagnostics};
use crate::error::Result;
use crate::goals::{
    Goal, GoalDecomposer, GoalDecomposition, GoalPriority, GoalState, GoalTrigger,
    PrioritizationResult,
};
use crate::intent::{IntentAnalysis, IntentAnalyzer, IntentInput, IntentType};
use crate::orchestration::{CognitiveEngine, CognitiveResult, DecisionOutput};
use crate::planner::{
    Plan, PlanAction, PlanState, PlanStep, Planner, PlanningContext, StepState, StepType,
};
use crate::reasoning::{Reasoner, ReasoningInput, ReasoningOutput, ReasoningStep};
use crate::recovery::{RecoveryManager, RecoveryPlan, RecoveryStrategy};
use crate::reflection::{
    ContextChange, InsightSource, OutcomeComparison, ReflectionEngine, ReflectionInput,
    ReflectionInsight, ReflectionReport,
};
use crate::self_monitoring::{HealthStatus, LatencyRecord, SelfMonitor, SystemSnapshot};
use crate::tools::{ToolDescription, ToolSelection, ToolSelectionInput, ToolSelector};
use crate::types::{
    ActionId, CognitiveState, ConfidenceScore, ContextId, GoalId, IntentId, PlanId, ReflectionId,
    StepId, Urgency,
};
use crate::validation::{ActionValidator, ValidationResult, ValidationStatus};

// ---------------------------------------------------------------------------
// InMemoryIntentAnalyzer
// ---------------------------------------------------------------------------

pub struct InMemoryIntentAnalyzer;

#[async_trait]
impl IntentAnalyzer for InMemoryIntentAnalyzer {
    async fn analyze(&self, input: &IntentInput) -> Result<IntentAnalysis> {
        let text = input.raw_text.trim().to_lowercase();
        let intent_type = self.classify_intent(&text);
        let confidence = self.estimate_confidence(&text, &intent_type);
        let requires_planning = matches!(
            intent_type,
            IntentType::Command
                | IntentType::Creation
                | IntentType::Modification
                | IntentType::Navigation
        );
        let requires_reasoning = matches!(intent_type, IntentType::Query | IntentType::Learning);
        Ok(IntentAnalysis {
            intent_id: IntentId(Uuid::new_v4().to_string()),
            intent_type,
            confidence: ConfidenceScore::new(confidence)?,
            primary_action: self.extract_action(&text),
            parameters: self.extract_parameters(&text),
            requires_planning,
            requires_reasoning,
            urgency: self.classify_urgency(&text),
            alternate_interpretations: vec![],
            timestamp: Utc::now(),
        })
    }

    async fn analyze_streaming(
        &self,
        input: &IntentInput,
        _partial: bool,
    ) -> Result<IntentAnalysis> {
        self.analyze(input).await
    }
}

impl InMemoryIntentAnalyzer {
    fn classify_intent(&self, text: &str) -> IntentType {
        if text.starts_with("create")
            || text.starts_with("make")
            || text.starts_with("build")
            || text.starts_with("new")
        {
            IntentType::Creation
        } else if text.starts_with("modify")
            || text.starts_with("change")
            || text.starts_with("update")
            || text.starts_with("edit")
        {
            IntentType::Modification
        } else if text.starts_with("delete")
            || text.starts_with("remove")
            || text.starts_with("destroy")
        {
            IntentType::Deletion
        } else if text.starts_with("navigate")
            || text.starts_with("go")
            || text.starts_with("open")
            || text.starts_with("show")
        {
            IntentType::Navigation
        } else if text.starts_with("call")
            || text.starts_with("message")
            || text.starts_with("email")
            || text.starts_with("send")
        {
            IntentType::Communication
        } else if text.starts_with("play")
            || text.starts_with("watch")
            || text.starts_with("listen")
        {
            IntentType::Entertainment
        } else if text.starts_with("what")
            || text.starts_with("how")
            || text.starts_with("why")
            || text.starts_with("when")
            || text.starts_with("where")
            || text.starts_with("who")
        {
            IntentType::Query
        } else if text.starts_with("learn")
            || text.starts_with("study")
            || text.starts_with("explain")
        {
            IntentType::Learning
        } else if text.contains("do")
            || text.contains("run")
            || text.contains("execute")
            || text.contains("perform")
        {
            IntentType::Command
        } else {
            IntentType::Query
        }
    }

    fn estimate_confidence(&self, text: &str, intent_type: &IntentType) -> f64 {
        let base = match intent_type {
            IntentType::Query => 0.85,
            IntentType::Command => 0.75,
            IntentType::Creation => 0.80,
            IntentType::Modification => 0.80,
            IntentType::Deletion => 0.85,
            IntentType::Navigation => 0.70,
            IntentType::Communication => 0.75,
            IntentType::Entertainment => 0.70,
            IntentType::Productivity => 0.65,
            IntentType::Learning => 0.70,
            IntentType::Custom(_) => 0.50,
        };
        let length_factor = (text.len() as f64).clamp(5.0, 200.0) / 200.0;
        (base * 0.7 + length_factor * 0.3).clamp(0.0, 1.0)
    }

    fn extract_action(&self, text: &str) -> String {
        text.split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_string()
    }

    fn extract_parameters(&self, _text: &str) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }

    fn classify_urgency(&self, text: &str) -> Urgency {
        let lower = text.to_lowercase();
        if lower.contains("urgent")
            || lower.contains("immediately")
            || lower.contains("asap")
            || lower.contains("emergency")
        {
            Urgency::Critical
        } else if lower.contains("soon") || lower.contains("quickly") || lower.contains("hurry") {
            Urgency::High
        } else if lower.contains("later")
            || lower.contains("eventually")
            || lower.contains("sometime")
        {
            Urgency::Low
        } else {
            Urgency::Medium
        }
    }
}

// ---------------------------------------------------------------------------
// InMemoryGoalDecomposer — context-aware goal management
// ---------------------------------------------------------------------------

pub struct InMemoryGoalDecomposer {
    config: CognitionConfig,
    goal_counter: AtomicUsize,
    active_goals: RwLock<Vec<Goal>>,
}

impl InMemoryGoalDecomposer {
    pub fn new(config: CognitionConfig) -> Self {
        Self {
            config,
            goal_counter: AtomicUsize::new(0),
            active_goals: RwLock::new(Vec::new()),
        }
    }

    fn next_goal_id(&self) -> GoalId {
        GoalId(format!(
            "goal-{}",
            self.goal_counter.fetch_add(1, Ordering::SeqCst)
        ))
    }
}

#[async_trait]
impl GoalDecomposer for InMemoryGoalDecomposer {
    async fn decompose(&self, intent: &IntentAnalysis) -> Result<GoalDecomposition> {
        let max_goals = self.config.max_goals_per_intent;
        let mut goals = Vec::new();
        let mut deps = Vec::new();

        let primary_goal_id = self.next_goal_id();
        let primary_goal = Goal {
            id: primary_goal_id.clone(),
            description: format!("{}: {}", intent.intent_type, intent.primary_action),
            priority: GoalPriority::High,
            dependencies: vec![],
            state: GoalState::Pending,
            acceptance_criteria: vec!["Completed successfully".to_string()],
            metadata: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            context_snapshot: None,
            paused_by_context: false,
        };
        goals.push(primary_goal);

        if intent.requires_planning && goals.len() < max_goals {
            let sub_id = self.next_goal_id();
            let sub = Goal {
                id: sub_id.clone(),
                description: "Plan execution".to_string(),
                priority: GoalPriority::Medium,
                dependencies: vec![primary_goal_id.clone()],
                state: GoalState::Pending,
                acceptance_criteria: vec!["Plan executed".to_string()],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                context_snapshot: None,
                paused_by_context: false,
            };
            deps.push((primary_goal_id.clone(), sub_id));
            goals.push(sub);
        }

        if intent.requires_reasoning && goals.len() < max_goals {
            let rid = self.next_goal_id();
            let rgoal = Goal {
                id: rid.clone(),
                description: "Reason about results".to_string(),
                priority: GoalPriority::Low,
                dependencies: vec![primary_goal_id.clone()],
                state: GoalState::Pending,
                acceptance_criteria: vec!["Reasoning complete".to_string()],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                context_snapshot: None,
                paused_by_context: false,
            };
            deps.push((primary_goal_id, rid));
            goals.push(rgoal);
        }

        let complexity = if max_goals > 0 {
            goals.len() as f64 / max_goals as f64
        } else {
            1.0
        };

        // Store goals as active
        *self.active_goals.write() = goals.clone();

        Ok(GoalDecomposition {
            intent_id: intent.intent_id.clone(),
            goals,
            dependency_graph: deps,
            estimated_complexity: complexity,
            timestamp: Utc::now(),
        })
    }

    async fn refine_goal(&self, goal: &Goal, context: &AssembledContext) -> Result<Goal> {
        let mut refined = goal.clone();
        refined.updated_at = Utc::now();
        refined.context_snapshot = Some(context.clone());

        // Use context to refine goal description
        if let Some(fusion_data) = &context.fusion_data {
            if let Some(activity) = fusion_data.get("activity") {
                refined
                    .metadata
                    .insert("activity_context".to_string(), activity.to_string());
            }
        }

        Ok(refined)
    }

    async fn prioritize(&self, goals: Vec<Goal>) -> Result<Vec<(Goal, GoalPriority)>> {
        Ok(goals
            .into_iter()
            .map(|g| {
                let p = g.priority;
                (g, p)
            })
            .collect())
    }

    async fn reprioritize(
        &self,
        goals: Vec<Goal>,
        context: &AssembledContext,
    ) -> Result<PrioritizationResult> {
        let mut prioritized = Vec::new();
        let mut paused = Vec::new();
        let mut activated = Vec::new();
        let mut blocked = Vec::new();

        for mut goal in goals {
            // Check for context-based priority changes
            if let Some(fusion_data) = &context.fusion_data {
                // Example: if battery is low, pause resource-heavy goals
                if let Some(battery) = fusion_data.get("battery_level") {
                    if let Some(level) = battery.as_f64() {
                        if level < 0.1 && goal.priority != GoalPriority::Critical {
                            goal.state = GoalState::Blocked("Low battery — paused".to_string());
                            goal.paused_by_context = true;
                            paused.push(goal.id.clone());
                            blocked.push((goal.id.clone(), "Low battery".to_string()));
                            continue;
                        }
                    }
                }

                // Example: if meeting is active, pause non-meeting goals
                if let Some(activity) = fusion_data.get("activity") {
                    if let Some(activity_type) = activity.get("type") {
                        if activity_type == "meeting"
                            && (goal.description.contains("code")
                                || goal.description.contains("build"))
                        {
                            goal.state = GoalState::Blocked("Meeting in progress".to_string());
                            goal.paused_by_context = true;
                            paused.push(goal.id.clone());
                            continue;
                        }
                    }
                }
            }

            if goal.paused_by_context {
                // Resume paused goals if context is now favorable
                goal.state = GoalState::Active;
                goal.paused_by_context = false;
                activated.push(goal.id.clone());
            }

            prioritized.push((goal.clone(), goal.priority));
        }

        Ok(PrioritizationResult {
            goals: prioritized,
            paused,
            activated,
            blocked,
        })
    }

    async fn check_context_triggers(
        &self,
        goals: &[Goal],
        context: &AssembledContext,
    ) -> Result<Vec<GoalTrigger>> {
        let mut triggers = Vec::new();

        if let Some(fusion_data) = &context.fusion_data {
            // Check for context changes that should trigger goal actions
            if let Some(battery) = fusion_data.get("battery_level") {
                if let Some(level) = battery.as_f64() {
                    if level < 0.1 {
                        for goal in goals {
                            if goal.state != GoalState::Blocked("".to_string())
                                && goal.priority != GoalPriority::Critical
                            {
                                triggers.push(GoalTrigger::Pause {
                                    goal_id: goal.id.clone(),
                                    reason: "Low battery".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(triggers)
    }

    async fn active_goals(&self) -> Result<Vec<Goal>> {
        Ok(self
            .active_goals
            .read()
            .iter()
            .filter(|g| matches!(g.state, GoalState::Active | GoalState::InProgress))
            .cloned()
            .collect())
    }
}

// ---------------------------------------------------------------------------
// InMemoryPlanner — context-aware planning
// ---------------------------------------------------------------------------

pub struct InMemoryPlanner {
    config: CognitionConfig,
    plan_counter: AtomicUsize,
}

impl InMemoryPlanner {
    pub fn new(config: CognitionConfig) -> Self {
        Self {
            config,
            plan_counter: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Planner for InMemoryPlanner {
    async fn create_plan(
        &self,
        decomposition: &GoalDecomposition,
        planning_context: &PlanningContext,
    ) -> Result<Plan> {
        let max_steps = self.config.max_plan_steps;
        let plan_id = PlanId(format!(
            "plan-{}",
            self.plan_counter.fetch_add(1, Ordering::SeqCst)
        ));
        let mut steps = Vec::new();

        for (step_idx, goal) in decomposition.goals.iter().enumerate() {
            if steps.len() >= max_steps {
                break;
            }
            let sid = StepId(format!("step-{}", step_idx));
            steps.push(PlanStep {
                id: sid,
                description: format!("Execute goal: {}", goal.description),
                step_type: StepType::Atomic,
                dependencies: goal
                    .dependencies
                    .iter()
                    .enumerate()
                    .map(|(i, _)| StepId(format!("step-{}", i)))
                    .collect(),
                required_capabilities: vec![],
                state: StepState::Pending,
                estimated_duration_ms: 1000,
                max_retries: 3,
                metadata: HashMap::new(),
            });
        }

        // Adjust plan based on context confidence
        let adjusted_duration = if planning_context.context_confidence < 0.5 {
            // Low confidence: add buffer time
            decomposition.goals.len() as u64 * 1500
        } else {
            decomposition.goals.len() as u64 * 1000
        };

        Ok(Plan {
            id: plan_id,
            goals: decomposition.goals.iter().map(|g| g.id.clone()).collect(),
            steps,
            state: PlanState::Draft,
            estimated_total_duration_ms: adjusted_duration,
            parallelism_possible: false,
            fallback_plan_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    async fn validate_plan(&self, _plan: &Plan) -> Result<bool> {
        Ok(true)
    }

    async fn optimize_plan(&self, plan: &Plan, planning_context: &PlanningContext) -> Result<Plan> {
        let mut opt = plan.clone();
        opt.updated_at = Utc::now();

        // If context has changed, re-evaluate step dependencies
        if planning_context.context_changed {
            for step in &mut opt.steps {
                if step.state == StepState::Pending {
                    step.state = StepState::Ready;
                }
            }
        }

        Ok(opt)
    }

    async fn create_fallback(&self, plan: &Plan, _reason: &str) -> Result<Plan> {
        let mut fb = plan.clone();
        fb.id = PlanId(format!("fallback-{}", Uuid::new_v4()));
        fb.state = PlanState::Draft;
        fb.updated_at = Utc::now();
        Ok(fb)
    }

    async fn estimate_duration(&self, plan: &Plan) -> Result<u64> {
        Ok(plan.steps.iter().map(|s| s.estimated_duration_ms).sum())
    }

    async fn can_parallelize(&self, _plan: &Plan) -> Result<bool> {
        Ok(false)
    }

    async fn on_context_change(
        &self,
        plan: &Plan,
        planning_context: &PlanningContext,
    ) -> Result<PlanAction> {
        if !planning_context.context_changed {
            return Ok(PlanAction::Continue);
        }

        // Check if context confidence dropped significantly
        if planning_context.context_confidence < 0.3 {
            return Ok(PlanAction::Pause(
                "Context confidence critically low".to_string(),
            ));
        }

        // Check for stale sources
        if !planning_context.stale_sources.is_empty() {
            let stale_steps: Vec<StepId> = plan
                .steps
                .iter()
                .filter(|s| {
                    s.state == StepState::InProgress
                        && planning_context
                            .stale_sources
                            .iter()
                            .any(|src| s.description.contains(src))
                })
                .map(|s| s.id.clone())
                .collect();

            if !stale_steps.is_empty() {
                return Ok(PlanAction::Reschedule(stale_steps));
            }
        }

        Ok(PlanAction::Continue)
    }
}

// ---------------------------------------------------------------------------
// InMemoryReasoner
// ---------------------------------------------------------------------------

pub struct InMemoryReasoner;

#[async_trait]
impl Reasoner for InMemoryReasoner {
    async fn reason(&self, input: &ReasoningInput) -> Result<ReasoningOutput> {
        let conclusion = format!(
            "Evaluated query: {}. Using available context: {} constraints.",
            input.query,
            input.constraints.len()
        );

        Ok(ReasoningOutput {
            conclusion: conclusion.clone(),
            confidence: ConfidenceScore::new(0.75)?,
            steps: vec![ReasoningStep {
                index: 0,
                premise: input.query.clone(),
                inference: conclusion.clone(),
                confidence: 0.75,
                source: "InMemoryReasoner".to_string(),
            }],
            duration_ms: 10,
            context_freshness: input.context.fusion_confidence,
            contributing_sources: input
                .context
                .sources
                .iter()
                .map(|s| format!("{:?}", s))
                .collect(),
        })
    }

    async fn evaluate(&self, claim: &str, context: &AssembledContext) -> Result<ConfidenceScore> {
        let base = if claim.len() > 20 { 0.7 } else { 0.5 };
        // Boost confidence if context has high fusion confidence
        let context_boost = context.fusion_confidence * 0.1;
        ConfidenceScore::new(base + context_boost)
    }

    async fn compare(
        &self,
        options: &[String],
        _criteria: &[String],
    ) -> Result<Vec<(String, f64)>> {
        Ok(options
            .iter()
            .enumerate()
            .map(|(i, opt)| (opt.clone(), 1.0 - (i as f64 * 0.1)))
            .collect())
    }

    async fn reason_with_context(
        &self,
        query: &str,
        context: &AssembledContext,
    ) -> Result<ReasoningOutput> {
        let input = ReasoningInput {
            query: query.to_string(),
            context: context.clone(),
            constraints: vec![],
            max_depth: 5,
        };
        self.reason(&input).await
    }
}

// ---------------------------------------------------------------------------
// InMemoryContextAssembler
// ---------------------------------------------------------------------------

pub struct InMemoryContextAssembler;

#[async_trait]
impl ContextAssembler for InMemoryContextAssembler {
    async fn assemble(&self, input: &ContextAssemblyInput<'_>) -> Result<AssembledContext> {
        let mut sources = vec![ContextSource::WorldModel];
        let mut history = vec![format!(
            "intent: {} ({})",
            input.intent.intent_type, input.intent.primary_action
        )];

        if input.personality.is_some() {
            sources.push(ContextSource::Personality);
        }

        for event in &input.recent_events {
            history.push(format!("{}", event));
        }

        Ok(AssembledContext {
            id: ContextId(Uuid::new_v4().to_string()),
            sources,
            world_snapshot: Some(input.world_snapshot.clone()),
            personality_context: None,
            relevant_history: history,
            constraints: vec![],
            priority_hints: vec![format!("urgency: {:?}", input.intent.urgency)],
            assembly_time_ms: 5,
            timestamp: Utc::now(),
            fusion_data: None,
            fusion_confidence: 0.8,
            source_count: 2,
        })
    }

    async fn refresh(&self, context: &AssembledContext) -> Result<AssembledContext> {
        let mut c = context.clone();
        c.timestamp = Utc::now();
        Ok(c)
    }

    async fn merge(&self, contexts: &[AssembledContext]) -> Result<AssembledContext> {
        let mut merged = contexts.first().cloned().unwrap_or(AssembledContext {
            id: ContextId(Uuid::new_v4().to_string()),
            sources: vec![],
            world_snapshot: None,
            personality_context: None,
            relevant_history: vec![],
            constraints: vec![],
            priority_hints: vec![],
            assembly_time_ms: 0,
            timestamp: Utc::now(),
            fusion_data: None,
            fusion_confidence: 0.0,
            source_count: 0,
        });
        for ctx in contexts.iter().skip(1) {
            merged.relevant_history.extend(ctx.relevant_history.clone());
            merged.constraints.extend(ctx.constraints.clone());
            merged.priority_hints.extend(ctx.priority_hints.clone());
        }
        merged.assembly_time_ms = 10;
        merged.timestamp = Utc::now();
        Ok(merged)
    }
}

// ---------------------------------------------------------------------------
// InMemoryConfidenceEstimator
// ---------------------------------------------------------------------------

pub struct InMemoryConfidenceEstimator;

#[async_trait]
impl ConfidenceEstimator for InMemoryConfidenceEstimator {
    async fn estimate_intent_confidence(&self, intent: &IntentAnalysis) -> Result<ConfidenceScore> {
        Ok(intent.confidence.clone())
    }

    async fn estimate_plan_confidence(&self, plan: &Plan) -> Result<ConfidenceScore> {
        let base = match plan.state {
            PlanState::Draft => 0.5,
            PlanState::Validated => 0.7,
            PlanState::InProgress => 0.6,
            PlanState::Completed { success } => {
                if success {
                    0.9
                } else {
                    0.3
                }
            }
            PlanState::Failed(_) | PlanState::Cancelled | PlanState::RolledBack => 0.2,
        };
        let step_factor = (plan.steps.len() as f64).recip();
        ConfidenceScore::new(base * 0.8 + step_factor * 0.2)
    }

    async fn estimate_step_confidence(
        &self,
        step: &PlanStep,
        _ctx: &AssembledContext,
    ) -> Result<ConfidenceScore> {
        let base = match step.state {
            StepState::Pending | StepState::Ready => 0.6,
            StepState::InProgress => 0.5,
            StepState::Completed => 0.95,
            StepState::Failed(_) => 0.2,
            StepState::Skipped | StepState::Cancelled => 0.3,
        };
        ConfidenceScore::new(base)
    }

    async fn estimate_reasoning_confidence(
        &self,
        output: &ReasoningOutput,
    ) -> Result<ConfidenceScore> {
        Ok(output.confidence.clone())
    }

    async fn estimate_tool_confidence(
        &self,
        tool: &ToolDescription,
        _ctx: &AssembledContext,
    ) -> Result<ConfidenceScore> {
        ConfidenceScore::new(tool.confidence_score.unwrap_or(0.7))
    }

    async fn aggregate_confidence(&self, scores: &[ConfidenceScore]) -> Result<ConfidenceScore> {
        if scores.is_empty() {
            return ConfidenceScore::new(0.5);
        }
        let avg = scores.iter().map(|s| s.value).sum::<f64>() / scores.len() as f64;
        ConfidenceScore::new(avg)
    }
}

// ---------------------------------------------------------------------------
// InMemoryToolSelector
// ---------------------------------------------------------------------------

pub struct InMemoryToolSelector;

#[async_trait]
impl ToolSelector for InMemoryToolSelector {
    async fn select_tool(&self, input: &ToolSelectionInput<'_>) -> Result<ToolSelection> {
        let ranked = self
            .rank_tools(&input.available_tools, input.context)
            .await?;
        let selected = ranked.first().cloned().unwrap_or_else(|| {
            (
                ToolDescription {
                    tool_id: "unknown".to_string(),
                    name: "Unknown".to_string(),
                    description: "No suitable tool found".to_string(),
                    capabilities: vec![],
                    confidence_score: None,
                },
                0.0,
            )
        });

        Ok(ToolSelection {
            selected_tool: selected.0,
            confidence: ConfidenceScore::new(selected.1)?,
            alternatives: ranked.into_iter().skip(1).take(3).collect(),
            selection_rationale: "Selected highest-ranked tool by capability matching".to_string(),
        })
    }

    async fn rank_tools(
        &self,
        tools: &[ToolDescription],
        _ctx: &AssembledContext,
    ) -> Result<Vec<(ToolDescription, f64)>> {
        let mut ranked: Vec<_> = tools
            .iter()
            .map(|t| {
                (
                    t.clone(),
                    t.confidence_score.unwrap_or(0.5) + t.capabilities.len() as f64 * 0.1,
                )
            })
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(ranked)
    }

    async fn validate_tool(&self, tool: &ToolDescription, _step: &PlanStep) -> Result<bool> {
        Ok(tool.confidence_score.unwrap_or(0.0) > 0.3)
    }
}

// ---------------------------------------------------------------------------
// InMemoryActionValidator — SECURITY: real safety/permission checks
// ---------------------------------------------------------------------------

/// Dangerous action patterns that must never be auto-approved.
const DANGEROUS_ACTIONS: &[&str] = &[
    "delete",
    "remove",
    "format",
    "shutdown",
    "reboot",
    "exec",
    "spawn",
    "sudo",
    "kill",
    "modify_system",
    "write_etc",
    "install",
    "uninstall",
];

pub struct InMemoryActionValidator;

#[async_trait]
impl ActionValidator for InMemoryActionValidator {
    async fn validate_action(
        &self,
        action: &PlanStep,
        _context: &AssembledContext,
    ) -> Result<ValidationResult> {
        let step_type = format!("{:?}", action.step_type);
        let description = action.description.to_lowercase();

        let mut risk_level = "low".to_string();
        let mut conditions = Vec::new();
        let mut status = ValidationStatus::Approved;

        // Check for dangerous action patterns
        for pattern in DANGEROUS_ACTIONS {
            if description.contains(pattern) || step_type.to_lowercase().contains(pattern) {
                risk_level = "critical".to_string();
                status = ValidationStatus::RequiresReview(format!(
                    "Dangerous action pattern: {}",
                    pattern
                ));
                conditions.push(format!("Dangerous action pattern detected: {}", pattern));
                break;
            }
        }

        // Check step type risk
        if status == ValidationStatus::Approved
            && (step_type.contains("WriteFile") || step_type.contains("ModifySystem"))
        {
            risk_level = "high".to_string();
            conditions.push(format!("High-risk step type: {}", step_type));
        }

        // Validate required capabilities
        if action.required_capabilities.iter().any(|cap| {
            let s = cap.0.to_lowercase();
            s.contains("delete")
                || s.contains("write")
                || s.contains("execute")
                || s.contains("admin")
        }) {
            risk_level = "high".to_string();
            conditions.push("Step requires high-risk capabilities".to_string());
        }

        Ok(ValidationResult {
            action_id: ActionId(Uuid::new_v4().to_string()),
            status,
            validated_by: vec!["InMemoryActionValidator".to_string()],
            risk_level,
            conditions,
            timestamp: Utc::now(),
        })
    }

    async fn validate_plan(
        &self,
        plan: &Plan,
        context: &AssembledContext,
    ) -> Result<Vec<ValidationResult>> {
        let mut results = Vec::new();
        for step in &plan.steps {
            results.push(self.validate_action(step, context).await?);
        }
        Ok(results)
    }

    async fn check_safety(
        &self,
        action: &PlanStep,
        _context: &AssembledContext,
    ) -> Result<ValidationStatus> {
        let description = action.description.to_lowercase();
        let step_type = format!("{:?}", action.step_type).to_lowercase();

        // Block obviously unsafe actions
        for pattern in DANGEROUS_ACTIONS {
            if description.contains(pattern) || step_type.contains(pattern) {
                return Ok(ValidationStatus::Rejected(format!(
                    "Unsafe action: {}",
                    pattern
                )));
            }
        }

        // Block shell/command injection attempts
        let combined = format!("{} {}", description, step_type);
        if combined.contains(';')
            || combined.contains('|')
            || combined.contains("&&")
            || combined.contains('$')
            || combined.contains('`')
            || combined.contains("$(")
        {
            return Ok(ValidationStatus::Rejected(
                "Shell injection attempt detected".to_string(),
            ));
        }

        Ok(ValidationStatus::Approved)
    }

    async fn check_permissions(
        &self,
        action: &PlanStep,
        _context: &AssembledContext,
    ) -> Result<ValidationStatus> {
        let step_type = format!("{:?}", action.step_type);

        // File write operations require permission check
        if step_type.contains("WriteFile") || step_type.contains("ModifySystem") {
            return Ok(ValidationStatus::RequiresReview(
                "File/system modification requires review".to_string(),
            ));
        }

        // Network operations require permission check
        if step_type.contains("NetworkRequest") {
            return Ok(ValidationStatus::RequiresReview(
                "Network operation requires review".to_string(),
            ));
        }

        Ok(ValidationStatus::Approved)
    }
}

// ---------------------------------------------------------------------------
// InMemoryRecoveryManager
// ---------------------------------------------------------------------------

pub struct InMemoryRecoveryManager;

#[async_trait]
impl RecoveryManager for InMemoryRecoveryManager {
    async fn diagnose(
        &self,
        _plan_id: &PlanId,
        _failed_step: &PlanStep,
        error: &str,
    ) -> Result<Vec<RecoveryStrategy>> {
        let el = error.to_lowercase();
        let mut strategies = Vec::new();
        if el.contains("timeout") || el.contains("retry") {
            strategies.push(RecoveryStrategy::Retry {
                max_attempts: 3,
                backoff_ms: 500,
            });
        }
        if el.contains("permission") || el.contains("denied") {
            strategies.push(RecoveryStrategy::RequestHumanAssistance(
                "Permission required".to_string(),
            ));
        }
        strategies.push(RecoveryStrategy::SkipStep);
        strategies.push(RecoveryStrategy::SimplifyGoal);
        Ok(strategies)
    }

    async fn create_recovery_plan(
        &self,
        strategies: &[RecoveryStrategy],
        _plan: &Plan,
    ) -> Result<RecoveryPlan> {
        let strategy = strategies
            .first()
            .cloned()
            .unwrap_or(RecoveryStrategy::Retry {
                max_attempts: 1,
                backoff_ms: 0,
            });
        let requires_consent = matches!(strategy, RecoveryStrategy::RequestHumanAssistance(_));
        Ok(RecoveryPlan {
            strategy,
            affected_steps: vec![],
            rationale: "Automatic recovery attempt".to_string(),
            estimated_recovery_time_ms: 1000,
            requires_user_consent: requires_consent,
        })
    }

    async fn execute_recovery(&self, _plan: &RecoveryPlan) -> Result<()> {
        Ok(())
    }
    async fn report_failure(&self, _plan_id: &PlanId, _error: &str) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InMemoryReflectionEngine — context-aware reflection
// ---------------------------------------------------------------------------

pub struct InMemoryReflectionEngine;

#[async_trait]
impl ReflectionEngine for InMemoryReflectionEngine {
    async fn reflect(&self, input: &ReflectionInput<'_>) -> Result<ReflectionReport> {
        let mut insights = Vec::new();

        // Analyze execution results
        for (step_id, success, error) in &input.execution_results {
            insights.push(ReflectionInsight {
                category: if *success { "success" } else { "failure" }.to_string(),
                description: format!("Step {} completed with success={}", step_id, success),
                impact: if *success { "positive" } else { "negative" }.to_string(),
                suggestion: if !*success {
                    Some(format!("Review error: {}", error))
                } else {
                    None
                },
                confidence: if *success { 0.9 } else { 0.7 },
                source_type: InsightSource::Execution,
            });
        }

        // Compare expected vs actual outcomes
        let outcome_comparison = match (input.expected_outcome, input.actual_outcome) {
            (Some(expected), Some(actual)) => Some(
                self.compare_outcomes(expected, actual, &input.context_changes)
                    .await?,
            ),
            _ => None,
        };

        // Analyze context impact
        if !input.context_changes.is_empty() {
            for change in &input.context_changes {
                insights.push(ReflectionInsight {
                    category: "context_change".to_string(),
                    description: format!("Context changed: {} — {}", change.trigger, change.impact),
                    impact: "context".to_string(),
                    suggestion: None,
                    confidence: 0.8,
                    source_type: InsightSource::Context,
                });
            }
        }

        let success_rate = if input.execution_results.is_empty() {
            0.0
        } else {
            let successes = input
                .execution_results
                .iter()
                .filter(|(_, s, _)| *s)
                .count() as f64;
            successes / input.execution_results.len() as f64
        };

        Ok(ReflectionReport {
            reflection_id: ReflectionId(Uuid::new_v4().to_string()),
            insights,
            overall_assessment: if success_rate > 0.7 {
                "Goal was achieved with high success rate.".to_string()
            } else if success_rate > 0.3 {
                "Goal was partially achieved; some steps failed.".to_string()
            } else {
                "Goal was not achieved; significant issues encountered.".to_string()
            },
            improvement_suggestions: vec!["Consider breaking down complex steps.".to_string()],
            lessons_learned: vec![format!(
                "Plan '{}' completed with {:.0}% success rate.",
                input.plan.id,
                success_rate * 100.0
            )],
            duration_ms: 15,
            timestamp: Utc::now(),
            outcome_comparison,
            context_impact: input.context_changes.clone(),
        })
    }

    async fn analyze_performance(
        &self,
        _plan: &Plan,
        report: &ReflectionReport,
    ) -> Result<Vec<String>> {
        Ok(report
            .insights
            .iter()
            .map(|i| i.description.clone())
            .collect())
    }

    async fn suggest_improvements(&self, _report: &ReflectionReport) -> Result<Vec<String>> {
        Ok(vec![
            "Add more granular error handling.".to_string(),
            "Implement step-level timeouts.".to_string(),
        ])
    }

    async fn compare_outcomes(
        &self,
        expected: &serde_json::Value,
        actual: &serde_json::Value,
        context_changes: &[ContextChange],
    ) -> Result<OutcomeComparison> {
        let mut differences = Vec::new();
        let mut context_caused = false;

        if expected != actual {
            differences.push("Expected and actual outcomes differ".to_string());
            // Simple heuristic: if context changed during execution, likely context-caused
            context_caused = !context_changes.is_empty();
        }

        let alignment_score = if differences.is_empty() {
            1.0
        } else if context_caused {
            0.5
        } else {
            0.3
        };

        Ok(OutcomeComparison {
            alignment_score,
            differences,
            context_caused,
            root_cause: if context_caused {
                Some("Context changed during execution".to_string())
            } else {
                None
            },
        })
    }

    async fn analyze_failure_causes(
        &self,
        _plan: &Plan,
        execution_results: &[(StepId, bool, String)],
        context: &AssembledContext,
    ) -> Result<Vec<String>> {
        let mut causes = Vec::new();

        for (step_id, success, error) in execution_results {
            if !success {
                causes.push(format!("Step {} failed: {}", step_id, error));
            }
        }

        // Check if context staleness caused failures
        if context.is_stale(300) {
            causes.push("Context was stale during execution".to_string());
        }

        // Check if low confidence caused issues
        if context.fusion_confidence < 0.5 {
            causes.push("Low context confidence may have affected execution".to_string());
        }

        Ok(causes)
    }
}

// ---------------------------------------------------------------------------
// InMemoryAttentionSystem
// ---------------------------------------------------------------------------

pub struct InMemoryAttentionSystem {
    weights: RwLock<HashMap<String, f64>>,
}

impl InMemoryAttentionSystem {
    pub fn new() -> Self {
        let mut weights = HashMap::new();
        weights.insert("conversation_active".to_string(), 0.20);
        weights.insert("foreground_app_importance".to_string(), 0.15);
        weights.insert("voice_activity".to_string(), 0.15);
        weights.insert("notification_pressure".to_string(), 0.10);
        weights.insert("urgency".to_string(), 0.20);
        weights.insert("goal_relevance".to_string(), 0.10);
        weights.insert("context_freshness".to_string(), 0.10);

        Self {
            weights: RwLock::new(weights),
        }
    }
}

impl Default for InMemoryAttentionSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AttentionSystem for InMemoryAttentionSystem {
    async fn score(&self, factors: &AttentionFactors) -> Result<AttentionScore> {
        let weights = self.weights.read();
        let mut breakdown = HashMap::new();

        let score = weights.get("conversation_active").unwrap_or(&0.20)
            * factors.conversation_active
            + weights.get("foreground_app_importance").unwrap_or(&0.15)
                * factors.foreground_app_importance
            + weights.get("voice_activity").unwrap_or(&0.15) * factors.voice_activity
            + weights.get("notification_pressure").unwrap_or(&0.10) * factors.notification_pressure
            + weights.get("urgency").unwrap_or(&0.20) * factors.urgency
            + weights.get("goal_relevance").unwrap_or(&0.10) * factors.goal_relevance
            + weights.get("context_freshness").unwrap_or(&0.10) * factors.context_freshness;

        let score = score.clamp(0.0, 1.0);

        breakdown.insert(
            "conversation_active".to_string(),
            factors.conversation_active,
        );
        breakdown.insert(
            "foreground_app_importance".to_string(),
            factors.foreground_app_importance,
        );
        breakdown.insert("voice_activity".to_string(), factors.voice_activity);
        breakdown.insert(
            "notification_pressure".to_string(),
            factors.notification_pressure,
        );
        breakdown.insert("urgency".to_string(), factors.urgency);
        breakdown.insert("goal_relevance".to_string(), factors.goal_relevance);
        breakdown.insert("context_freshness".to_string(), factors.context_freshness);

        let recommendation = if score > 0.8 {
            AttentionRecommendation::FocusPrimary
        } else if score > 0.5 {
            AttentionRecommendation::Resume
        } else if factors.urgency > 0.7 {
            AttentionRecommendation::SwitchTask
        } else if factors.notification_pressure > 0.8 {
            AttentionRecommendation::Alert
        } else {
            AttentionRecommendation::Idle
        };

        Ok(AttentionScore {
            score,
            breakdown,
            confidence: ConfidenceScore::new(score)?,
            recommendation,
        })
    }

    async fn update_weights(&self, new_weights: HashMap<String, f64>) -> Result<()> {
        *self.weights.write() = new_weights;
        Ok(())
    }

    async fn current_attention(&self) -> Result<AttentionScore> {
        self.score(&AttentionFactors::default()).await
    }

    async fn should_shift(&self, new_factors: &AttentionFactors) -> Result<bool> {
        let score = self.score(new_factors).await?;
        // Shift attention if score is very low or very high
        Ok(score.score < 0.2 || score.score > 0.9)
    }
}

// ---------------------------------------------------------------------------
// InMemorySelfMonitor
// ---------------------------------------------------------------------------

pub struct InMemorySelfMonitor {
    latencies: RwLock<HashMap<String, LatencyRecord>>,
    fusion_count: AtomicUsize,
    cycle_count: AtomicUsize,
    error_count: AtomicUsize,
    staleness_events: AtomicUsize,
    active_goals: AtomicUsize,
}

impl InMemorySelfMonitor {
    pub fn new() -> Self {
        Self {
            latencies: RwLock::new(HashMap::new()),
            fusion_count: AtomicUsize::new(0),
            cycle_count: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
            staleness_events: AtomicUsize::new(0),
            active_goals: AtomicUsize::new(0),
        }
    }
}

impl Default for InMemorySelfMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SelfMonitor for InMemorySelfMonitor {
    async fn record_latency(&self, subsystem: &str, latency: Duration) -> Result<()> {
        let mut latencies = self.latencies.write();
        let record = latencies
            .entry(subsystem.to_string())
            .or_insert_with(|| LatencyRecord::new(subsystem));
        record.record(latency);
        Ok(())
    }

    async fn get_latency(&self, subsystem: &str) -> Result<Option<LatencyRecord>> {
        Ok(self.latencies.read().get(subsystem).cloned())
    }

    async fn all_latencies(&self) -> Result<HashMap<String, LatencyRecord>> {
        Ok(self.latencies.read().clone())
    }

    async fn record_fusion(&self) -> Result<()> {
        self.fusion_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn record_cycle(&self) -> Result<()> {
        self.cycle_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn record_error(&self) -> Result<()> {
        self.error_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn record_staleness(&self) -> Result<()> {
        self.staleness_events.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn set_active_goals(&self, count: usize) -> Result<()> {
        self.active_goals.store(count, Ordering::Relaxed);
        Ok(())
    }

    async fn snapshot(&self) -> Result<SystemSnapshot> {
        let latencies = self.latencies.read().clone();
        let error_count = self.error_count.load(Ordering::Relaxed);
        let health = if error_count > 100 {
            HealthStatus::Critical(format!("{} errors", error_count))
        } else if error_count > 10 {
            HealthStatus::Degraded(format!("{} errors", error_count))
        } else {
            HealthStatus::Healthy
        };

        Ok(SystemSnapshot {
            latencies,
            health,
            fusion_count: self.fusion_count.load(Ordering::Relaxed) as u64,
            cycle_count: self.cycle_count.load(Ordering::Relaxed) as u64,
            error_count: error_count as u64,
            memory_estimate: 0,
            active_goals: self.active_goals.load(Ordering::Relaxed),
            staleness_events: self.staleness_events.load(Ordering::Relaxed) as u64,
        })
    }

    async fn health(&self) -> Result<HealthStatus> {
        let snapshot = self.snapshot().await?;
        Ok(snapshot.health)
    }
}

// ---------------------------------------------------------------------------
// InMemoryDiagnostics
// ---------------------------------------------------------------------------

pub struct InMemoryDiagnostics {
    self_monitor: Arc<InMemorySelfMonitor>,
    attention_system: Arc<InMemoryAttentionSystem>,
}

impl InMemoryDiagnostics {
    pub fn new(
        self_monitor: Arc<InMemorySelfMonitor>,
        attention_system: Arc<InMemoryAttentionSystem>,
    ) -> Self {
        Self {
            self_monitor,
            attention_system,
        }
    }
}

#[async_trait]
impl Diagnostics for InMemoryDiagnostics {
    async fn snapshot(&self) -> Result<DiagnosticsSnapshot> {
        let system_health = self.self_monitor.snapshot().await?;
        let attention = self.attention_system.current_attention().await?;
        let latencies_map = self.self_monitor.all_latencies().await?;

        let mut latencies = HashMap::new();
        for (name, record) in &latencies_map {
            latencies.insert(
                name.clone(),
                LatencyDiagnostics {
                    last_ms: record.last_latency.as_secs_f64() * 1000.0,
                    avg_ms: record.avg_latency.as_secs_f64() * 1000.0,
                    max_ms: record.max_latency.as_secs_f64() * 1000.0,
                    sample_count: record.sample_count,
                },
            );
        }

        Ok(DiagnosticsSnapshot {
            cognitive_state: "Idle".to_string(),
            context: ContextSummary {
                source_count: 0,
                confidence: 0.0,
                sources: vec![],
                freshness_secs: 0,
            },
            fusion_confidence: 0.0,
            active_goals: vec![],
            attention,
            reasoning_stage: None,
            current_decision: None,
            reflection_summary: None,
            system_health,
            latencies,
        })
    }

    async fn cognitive_state(&self) -> Result<String> {
        Ok("Idle".to_string())
    }

    async fn context_summary(&self) -> Result<ContextSummary> {
        Ok(ContextSummary {
            source_count: 0,
            confidence: 0.0,
            sources: vec![],
            freshness_secs: 0,
        })
    }

    async fn fusion_confidence(&self) -> Result<f64> {
        Ok(0.0)
    }

    async fn active_goals(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }

    async fn attention_score(&self) -> Result<AttentionScore> {
        self.attention_system.current_attention().await
    }

    async fn reasoning_stage(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn current_decision(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn reflection_summary(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// InMemoryCognitiveEngine — ties everything together with context
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct InMemoryCognitiveEngine {
    config: CognitionConfig,
    state: RwLock<CognitiveState>,
    intent_analyzer: Arc<dyn IntentAnalyzer>,
    goal_decomposer: Arc<dyn GoalDecomposer>,
    planner: Arc<dyn Planner>,
    reasoner: Arc<dyn Reasoner>,
    context_assembler: Arc<dyn ContextAssembler>,
    confidence_estimator: Arc<dyn ConfidenceEstimator>,
    tool_selector: Arc<dyn ToolSelector>,
    action_validator: Arc<dyn ActionValidator>,
    recovery_manager: Arc<dyn RecoveryManager>,
    reflection_engine: Arc<dyn ReflectionEngine>,
    attention_system: Arc<dyn AttentionSystem>,
    self_monitor: Arc<dyn SelfMonitor>,
}

impl InMemoryCognitiveEngine {
    pub fn new(config: CognitionConfig) -> Self {
        let attention = Arc::new(InMemoryAttentionSystem::new());
        let monitor = Arc::new(InMemorySelfMonitor::new());

        Self {
            config: config.clone(),
            state: RwLock::new(CognitiveState::Idle),
            intent_analyzer: Arc::new(InMemoryIntentAnalyzer),
            goal_decomposer: Arc::new(InMemoryGoalDecomposer::new(config.clone())),
            planner: Arc::new(InMemoryPlanner::new(config.clone())),
            reasoner: Arc::new(InMemoryReasoner),
            context_assembler: Arc::new(InMemoryContextAssembler),
            confidence_estimator: Arc::new(InMemoryConfidenceEstimator),
            tool_selector: Arc::new(InMemoryToolSelector),
            action_validator: Arc::new(InMemoryActionValidator),
            recovery_manager: Arc::new(InMemoryRecoveryManager),
            reflection_engine: Arc::new(InMemoryReflectionEngine),
            attention_system: attention,
            self_monitor: monitor,
        }
    }

    fn set_state(&self, new_state: CognitiveState) {
        *self.state.write() = new_state;
    }
}

#[async_trait]
impl CognitiveEngine for InMemoryCognitiveEngine {
    async fn init(&self, _config: &CognitionConfig) -> Result<()> {
        self.set_state(CognitiveState::Idle);
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        self.set_state(CognitiveState::Completed);
        Ok(())
    }

    async fn process(&self, input: &crate::intent::IntentInput) -> Result<CognitiveResult> {
        let start = Utc::now();
        let cycle_start = std::time::Instant::now();

        // 1. Analyze intent
        self.set_state(CognitiveState::AnalyzingIntent);
        let intent = self.intent_analyzer.analyze(input).await?;
        let intent_confidence = self
            .confidence_estimator
            .estimate_intent_confidence(&intent)
            .await?;
        if !intent_confidence.is_sufficient(self.config.confidence_threshold) {
            return Ok(CognitiveResult {
                intent,
                plan: None,
                context: None,
                result: serde_json::json!({"status": "low_confidence"}),
                confidence: intent_confidence,
                reflection: None,
                duration_ms: (Utc::now() - start).num_milliseconds() as u64,
                success: false,
                errors: vec!["Intent confidence below threshold".to_string()],
                decision: None,
                attention_score: None,
            });
        }

        // 2. Decompose goals
        self.set_state(CognitiveState::DecomposingGoals);
        let decomposition = self.goal_decomposer.decompose(&intent).await?;

        // 3. Assemble context
        self.set_state(CognitiveState::AssemblingContext);
        let ws = voxy_world_model::context::WorldSnapshot {
            desktop: voxy_world_model::desktop::DesktopState {
                windows: vec![],
                active_window_id: None,
                workspaces: vec![],
                focused_app: None,
            },
            environment: voxy_world_model::environment::UserEnvironment::default(),
            devices: vec![],
            tasks: vec![],
            timestamp: Utc::now(),
        };
        let assembly_input = ContextAssemblyInput {
            intent: &intent,
            world_snapshot: &ws,
            personality: None,
            recent_events: vec![],
        };
        let context = self.context_assembler.assemble(&assembly_input).await?;

        // 4. Create plan with context
        self.set_state(CognitiveState::Planning);
        let planning_context = PlanningContext {
            assembled: context.clone(),
            context_changed: false,
            stale_sources: vec![],
            context_confidence: context.fusion_confidence,
        };
        let plan = self
            .planner
            .create_plan(&decomposition, &planning_context)
            .await?;

        // 5. Compute attention
        let attention_score = self
            .attention_system
            .score(&crate::attention::AttentionFactors {
                conversation_active: 0.0,
                foreground_app_importance: 0.5,
                voice_activity: 0.0,
                notification_pressure: 0.0,
                urgency: intent.urgency as u8 as f64 / 3.0,
                goal_relevance: 0.8,
                context_freshness: context.fusion_confidence,
            })
            .await?;

        // 6. Optional reasoning
        if intent.requires_reasoning {
            self.set_state(CognitiveState::Reasoning);
            let reasoning_input = crate::reasoning::ReasoningInput {
                query: input.raw_text.clone(),
                context: context.clone(),
                constraints: vec![],
                max_depth: self.config.max_reasoning_depth,
            };
            let _output = self.reasoner.reason(&reasoning_input).await?;
        }

        // 7. Record cycle
        self.self_monitor.record_cycle().await?;
        self.self_monitor
            .record_latency("cognitive_cycle", cycle_start.elapsed())
            .await?;

        // 8. Create decision
        let decision = DecisionOutput {
            decision: format!("Processed: {}", intent.primary_action),
            confidence: intent_confidence.clone(),
            reason: format!("Intent type: {:?}", intent.intent_type),
            context_summary: format!(
                "Sources: {}, Confidence: {:.2}",
                context.source_count, context.fusion_confidence
            ),
            priority: format!("{:?}", intent.urgency),
            timestamp: Utc::now(),
        };

        self.set_state(CognitiveState::Completed);
        Ok(CognitiveResult {
            intent,
            plan: Some(plan),
            context: Some(context),
            result: serde_json::json!({"status": "processed"}),
            confidence: intent_confidence,
            reflection: None,
            duration_ms: (Utc::now() - start).num_milliseconds() as u64,
            success: true,
            errors: vec![],
            decision: Some(decision),
            attention_score: Some(attention_score),
        })
    }

    async fn process_with_context(
        &self,
        input: &crate::intent::IntentInput,
        assembled: &AssembledContext,
    ) -> Result<CognitiveResult> {
        let start = Utc::now();

        // 1. Analyze intent
        self.set_state(CognitiveState::AnalyzingIntent);
        let intent = self.intent_analyzer.analyze(input).await?;
        let intent_confidence = self
            .confidence_estimator
            .estimate_intent_confidence(&intent)
            .await?;

        // 2. Decompose goals
        self.set_state(CognitiveState::DecomposingGoals);
        let decomposition = self.goal_decomposer.decompose(&intent).await?;

        // 3. Create plan with provided context
        self.set_state(CognitiveState::Planning);
        let planning_context = PlanningContext {
            assembled: assembled.clone(),
            context_changed: false,
            stale_sources: vec![],
            context_confidence: assembled.fusion_confidence,
        };
        let plan = self
            .planner
            .create_plan(&decomposition, &planning_context)
            .await?;

        // 4. Create decision
        let decision = DecisionOutput {
            decision: format!("Processed with context: {}", intent.primary_action),
            confidence: intent_confidence.clone(),
            reason: format!("Intent type: {:?}", intent.intent_type),
            context_summary: format!(
                "Sources: {}, Confidence: {:.2}",
                assembled.source_count, assembled.fusion_confidence
            ),
            priority: format!("{:?}", intent.urgency),
            timestamp: Utc::now(),
        };

        self.set_state(CognitiveState::Completed);
        Ok(CognitiveResult {
            intent,
            plan: Some(plan),
            context: Some(assembled.clone()),
            result: serde_json::json!({"status": "processed_with_context"}),
            confidence: intent_confidence,
            reflection: None,
            duration_ms: (Utc::now() - start).num_milliseconds() as u64,
            success: true,
            errors: vec![],
            decision: Some(decision),
            attention_score: None,
        })
    }

    async fn process_streaming(
        &self,
        input: &crate::intent::IntentInput,
    ) -> Result<CognitiveResult> {
        self.process(input).await
    }

    async fn state(&self) -> CognitiveState {
        self.state.read().clone()
    }
    async fn current_intent(&self) -> Option<IntentId> {
        None
    }
    async fn current_plan(&self) -> Option<PlanId> {
        None
    }

    async fn cancel(&self, _intent_id: &IntentId) -> Result<()> {
        self.set_state(CognitiveState::Idle);
        Ok(())
    }

    async fn pause(&self) -> Result<()> {
        self.set_state(CognitiveState::Idle);
        Ok(())
    }

    async fn resume(&self) -> Result<()> {
        self.set_state(CognitiveState::Idle);
        Ok(())
    }

    async fn diagnostics(&self) -> Result<serde_json::Value> {
        let state = self.state();
        let monitor_snapshot = self.self_monitor.snapshot().await?;
        let attention = self.attention_system.current_attention().await?;

        Ok(serde_json::json!({
            "cognitive_state": format!("{}", state.await),
            "health": format!("{:?}", monitor_snapshot.health),
            "fusion_count": monitor_snapshot.fusion_count,
            "cycle_count": monitor_snapshot.cycle_count,
            "error_count": monitor_snapshot.error_count,
            "active_goals": monitor_snapshot.active_goals,
            "staleness_events": monitor_snapshot.staleness_events,
            "attention_score": attention.score,
            "attention_recommendation": format!("{:?}", attention.recommendation),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_input(text: &str) -> IntentInput {
        IntentInput {
            raw_text: text.to_string(),
            context: None,
            source: "test".to_string(),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_intent_analyzer_query() {
        let a = InMemoryIntentAnalyzer;
        let r = a
            .analyze(&test_input("What is the weather?"))
            .await
            .unwrap();
        assert_eq!(r.intent_type, IntentType::Query);
        assert!(r.confidence.value > 0.5);
    }

    #[tokio::test]
    async fn test_intent_analyzer_navigation() {
        let a = InMemoryIntentAnalyzer;
        let r = a.analyze(&test_input("Open the browser")).await.unwrap();
        assert_eq!(r.intent_type, IntentType::Navigation);
        assert!(r.requires_planning);
    }

    #[tokio::test]
    async fn test_intent_analyzer_creation() {
        let a = InMemoryIntentAnalyzer;
        let r = a
            .analyze(&test_input("Create a new document"))
            .await
            .unwrap();
        assert_eq!(r.intent_type, IntentType::Creation);
    }

    #[tokio::test]
    async fn test_intent_urgency() {
        let a = InMemoryIntentAnalyzer;
        let r = a
            .analyze(&test_input("urgent send email now"))
            .await
            .unwrap();
        assert_eq!(r.urgency, Urgency::Critical);
    }

    #[tokio::test]
    async fn test_goal_decomposer() {
        let d = InMemoryGoalDecomposer::new(CognitionConfig::default());
        let a = InMemoryIntentAnalyzer;
        let intent = a.analyze(&test_input("Build a web app")).await.unwrap();
        let dec = d.decompose(&intent).await.unwrap();
        assert!(!dec.goals.is_empty());
        assert_eq!(dec.intent_id, intent.intent_id);
    }

    #[tokio::test]
    async fn test_planner() {
        let cfg = CognitionConfig::default();
        let p = InMemoryPlanner::new(cfg);
        let a = InMemoryIntentAnalyzer;
        let intent = a.analyze(&test_input("Build a web app")).await.unwrap();
        let d = InMemoryGoalDecomposer::new(CognitionConfig::default());
        let dec = d.decompose(&intent).await.unwrap();
        let _ws = voxy_world_model::context::WorldSnapshot {
            desktop: voxy_world_model::desktop::DesktopState {
                windows: vec![],
                active_window_id: None,
                workspaces: vec![],
                focused_app: None,
            },
            environment: voxy_world_model::environment::UserEnvironment::default(),
            devices: vec![],
            tasks: vec![],
            timestamp: Utc::now(),
        };
        let planning_ctx = PlanningContext {
            assembled: AssembledContext {
                id: ContextId("ctx-1".to_string()),
                sources: vec![],
                world_snapshot: None,
                personality_context: None,
                relevant_history: vec![],
                constraints: vec![],
                priority_hints: vec![],
                assembly_time_ms: 0,
                timestamp: Utc::now(),
                fusion_data: None,
                fusion_confidence: 0.8,
                source_count: 0,
            },
            context_changed: false,
            stale_sources: vec![],
            context_confidence: 0.8,
        };
        let plan = p.create_plan(&dec, &planning_ctx).await.unwrap();
        assert!(!plan.steps.is_empty());
    }

    #[tokio::test]
    async fn test_reasoner() {
        let r = InMemoryReasoner;
        let ctx = AssembledContext {
            id: ContextId("ctx-1".to_string()),
            sources: vec![],
            world_snapshot: None,
            personality_context: None,
            relevant_history: vec![],
            constraints: vec![],
            priority_hints: vec![],
            assembly_time_ms: 0,
            timestamp: Utc::now(),
            fusion_data: None,
            fusion_confidence: 0.8,
            source_count: 0,
        };
        let input = ReasoningInput {
            query: "Why is the sky blue?".to_string(),
            context: ctx,
            constraints: vec![],
            max_depth: 5,
        };
        let output = r.reason(&input).await.unwrap();
        assert!(!output.conclusion.is_empty());
    }

    #[tokio::test]
    async fn test_full_cognitive_engine() {
        let engine = InMemoryCognitiveEngine::new(CognitionConfig::default());
        let result = engine
            .process(&test_input("What is the capital of France?"))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.intent.intent_type, IntentType::Query);
        assert!(result.plan.is_some());
        assert!(result.context.is_some());
        assert!(result.decision.is_some());
    }

    #[tokio::test]
    async fn test_cognitive_engine_low_confidence() {
        let cfg = CognitionConfig {
            confidence_threshold: 1.01,
            ..CognitionConfig::default()
        };
        let engine = InMemoryCognitiveEngine::new(cfg);
        let result = engine.process(&test_input("Hi")).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_context_assembler() {
        let ca = InMemoryContextAssembler;
        let a = InMemoryIntentAnalyzer;
        let intent = a.analyze(&test_input("Show my files")).await.unwrap();
        let ws = voxy_world_model::context::WorldSnapshot {
            desktop: voxy_world_model::desktop::DesktopState {
                windows: vec![],
                active_window_id: None,
                workspaces: vec![],
                focused_app: None,
            },
            environment: voxy_world_model::environment::UserEnvironment::default(),
            devices: vec![],
            tasks: vec![],
            timestamp: Utc::now(),
        };
        let input = ContextAssemblyInput {
            intent: &intent,
            world_snapshot: &ws,
            personality: None,
            recent_events: vec![],
        };
        let ctx = ca.assemble(&input).await.unwrap();
        assert!(!ctx.sources.is_empty());
    }

    #[tokio::test]
    async fn test_reflection_engine() {
        let re = InMemoryReflectionEngine;
        let plan_created_at = Utc::now();
        let plan = Plan {
            id: PlanId("plan-1".to_string()),
            goals: vec![],
            steps: vec![],
            state: PlanState::Completed { success: true },
            estimated_total_duration_ms: 1000,
            parallelism_possible: false,
            fallback_plan_id: None,
            created_at: plan_created_at,
            updated_at: Utc::now(),
        };
        let input = ReflectionInput {
            plan: &plan,
            execution_results: vec![
                (StepId("s1".to_string()), true, "".to_string()),
                (StepId("s2".to_string()), false, "timeout".to_string()),
            ],
            context: &AssembledContext {
                id: ContextId("ctx-1".to_string()),
                sources: vec![],
                world_snapshot: None,
                personality_context: None,
                relevant_history: vec![],
                constraints: vec![],
                priority_hints: vec![],
                assembly_time_ms: 0,
                timestamp: Utc::now(),
                fusion_data: None,
                fusion_confidence: 0.8,
                source_count: 0,
            },
            expected_outcome: None,
            actual_outcome: None,
            context_changes: vec![],
        };
        let report = re.reflect(&input).await.unwrap();
        assert_eq!(report.insights.len(), 2);
    }

    #[tokio::test]
    async fn test_attention_system() {
        let attn = InMemoryAttentionSystem::new();
        let factors = AttentionFactors {
            urgency: 0.8,
            context_freshness: 0.9,
            ..Default::default()
        };
        let score = attn.score(&factors).await.unwrap();
        assert!(score.score > 0.0);
        assert!(score.score <= 1.0);
    }

    #[tokio::test]
    async fn test_self_monitor() {
        let monitor = InMemorySelfMonitor::new();
        monitor
            .record_latency("test", Duration::from_millis(10))
            .await
            .unwrap();
        let record = monitor.get_latency("test").await.unwrap().unwrap();
        assert_eq!(record.sample_count, 1);

        monitor.record_fusion().await.unwrap();
        monitor.record_cycle().await.unwrap();
        let snap = monitor.snapshot().await.unwrap();
        assert_eq!(snap.fusion_count, 1);
        assert_eq!(snap.cycle_count, 1);
    }

    #[tokio::test]
    async fn test_confidence_estimator() {
        let ce = InMemoryConfidenceEstimator;
        let intent = InMemoryIntentAnalyzer
            .analyze(&test_input("Test query"))
            .await
            .unwrap();
        let score = ce.estimate_intent_confidence(&intent).await.unwrap();
        assert!(score.value > 0.0);
    }

    #[tokio::test]
    async fn test_recovery_manager() {
        let rm = InMemoryRecoveryManager;
        let step = PlanStep {
            id: StepId("s1".to_string()),
            description: "Test".to_string(),
            step_type: StepType::Atomic,
            dependencies: vec![],
            required_capabilities: vec![],
            state: StepState::Failed("err".to_string()),
            estimated_duration_ms: 100,
            max_retries: 3,
            metadata: HashMap::new(),
        };
        let strategies = rm
            .diagnose(&PlanId("p1".to_string()), &step, "timeout error")
            .await
            .unwrap();
        assert!(!strategies.is_empty());
    }

    #[tokio::test]
    async fn test_goal_reprioritize() {
        let decomposer = InMemoryGoalDecomposer::new(CognitionConfig::default());
        let ctx = AssembledContext {
            id: ContextId("ctx-1".to_string()),
            sources: vec![],
            world_snapshot: None,
            personality_context: None,
            relevant_history: vec![],
            constraints: vec![],
            priority_hints: vec![],
            assembly_time_ms: 0,
            timestamp: Utc::now(),
            fusion_data: Some(serde_json::json!({"battery_level": 0.05})),
            fusion_confidence: 0.8,
            source_count: 1,
        };
        let goals = vec![
            Goal {
                id: GoalId("g1".to_string()),
                description: "Build web app".to_string(),
                priority: GoalPriority::High,
                dependencies: vec![],
                state: GoalState::Active,
                acceptance_criteria: vec![],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                context_snapshot: None,
                paused_by_context: false,
            },
            Goal {
                id: GoalId("g2".to_string()),
                description: "Critical system check".to_string(),
                priority: GoalPriority::Critical,
                dependencies: vec![],
                state: GoalState::Active,
                acceptance_criteria: vec![],
                metadata: HashMap::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                context_snapshot: None,
                paused_by_context: false,
            },
        ];
        let result = decomposer.reprioritize(goals, &ctx).await.unwrap();
        // High priority goal should be paused due to low battery
        assert!(result.paused.contains(&GoalId("g1".to_string())));
        // Critical priority goal should remain
        assert!(!result.paused.contains(&GoalId("g2".to_string())));
    }

    #[tokio::test]
    async fn test_planner_context_change() {
        let planner = InMemoryPlanner::new(CognitionConfig::default());
        let plan = Plan {
            id: PlanId("p1".to_string()),
            goals: vec![],
            steps: vec![PlanStep {
                id: StepId("s1".to_string()),
                description: "Execute Environment task".to_string(),
                step_type: StepType::Atomic,
                dependencies: vec![],
                required_capabilities: vec![],
                state: StepState::InProgress,
                estimated_duration_ms: 1000,
                max_retries: 3,
                metadata: HashMap::new(),
            }],
            state: PlanState::InProgress,
            estimated_total_duration_ms: 1000,
            parallelism_possible: false,
            fallback_plan_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let ctx = PlanningContext {
            assembled: AssembledContext {
                id: ContextId("ctx-1".to_string()),
                sources: vec![],
                world_snapshot: None,
                personality_context: None,
                relevant_history: vec![],
                constraints: vec![],
                priority_hints: vec![],
                assembly_time_ms: 0,
                timestamp: Utc::now(),
                fusion_data: None,
                fusion_confidence: 0.2,
                source_count: 0,
            },
            context_changed: true,
            stale_sources: vec!["Environment".to_string()],
            context_confidence: 0.2,
        };

        let action = planner.on_context_change(&plan, &ctx).await.unwrap();
        assert_eq!(
            action,
            PlanAction::Pause("Context confidence critically low".to_string())
        );
    }
}
