//! Goal Engine V2 — persistent goals with priority, dependencies, and estimation.
//!
//! Extends the existing GoalManager with:
//! - Goal priority levels
//! - Goal dependencies (blocking relationships)
//! - Estimated completion dates
//! - Current state tracking
//! - Goal hierarchy (sub-goals)
//! - Auto-progress calculation from milestones

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GoalV2Id(pub String);

impl Default for GoalV2Id {
    fn default() -> Self {
        Self::new()
    }
}

impl GoalV2Id {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// Goal priority level.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum GoalPriority {
    Critical = 0,
    High = 1,
    #[default]
    Medium = 2,
    Low = 3,
    Backlog = 4,
}

/// Goal state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalV2State {
    Draft,
    Active,
    Blocked,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

/// Current progress details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProgress {
    pub current: f32,
    pub target: f32,
    pub milestone_progress: f32,
    pub subgoal_progress: f32,
    pub estimated_remaining_hours: Option<f64>,
}

/// A sub-goal within a goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGoal {
    pub id: GoalV2Id,
    pub title: String,
    pub completed: bool,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// A persistent goal with full metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalV2 {
    pub id: GoalV2Id,
    pub title: String,
    pub description: String,
    pub priority: GoalPriority,
    pub state: GoalV2State,
    pub progress: GoalProgress,
    pub parent_id: Option<GoalV2Id>,
    pub sub_goals: Vec<SubGoal>,
    pub dependencies: Vec<GoalV2Id>,
    pub dependents: Vec<GoalV2Id>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
    pub estimated_completion: Option<chrono::DateTime<chrono::Utc>>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub metadata: HashMap<String, String>,
}

/// Event when a goal changes state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalV2Event {
    pub goal_id: GoalV2Id,
    pub event_type: GoalV2EventType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GoalV2EventType {
    Created,
    Started,
    ProgressUpdated { old: f32, new: f32 },
    Blocked { by: GoalV2Id },
    Unblocked,
    Completed,
    Failed { reason: String },
    Cancelled,
    SubGoalCompleted { subgoal_id: GoalV2Id },
}

// ============================================================================
// Goal Engine V2
// ============================================================================

/// Persistent goal management with priority, dependencies, and estimation.
pub struct GoalEngineV2 {
    goals: HashMap<GoalV2Id, GoalV2>,
    event_log: Vec<GoalV2Event>,
    max_goals: usize,
    max_event_log: usize,
}

impl GoalEngineV2 {
    pub fn new(max_goals: usize, max_event_log: usize) -> Self {
        Self {
            goals: HashMap::with_capacity(max_goals),
            event_log: Vec::with_capacity(max_event_log.min(1000)),
            max_goals,
            max_event_log,
        }
    }

    pub fn default_engine() -> Self {
        Self::new(200, 5000)
    }

    /// Create a new goal.
    pub fn create_goal(
        &mut self,
        title: String,
        description: String,
        priority: GoalPriority,
        deadline: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<GoalV2Id, GoalEngineError> {
        if self.goals.len() >= self.max_goals {
            return Err(GoalEngineError::CapacityReached(self.max_goals));
        }

        let now = chrono::Utc::now();
        let id = GoalV2Id::new();
        let goal_id_for_event = id.clone();
        let goal = GoalV2 {
            id: id.clone(),
            title,
            description,
            priority,
            state: GoalV2State::Draft,
            progress: GoalProgress {
                current: 0.0,
                target: 1.0,
                milestone_progress: 0.0,
                subgoal_progress: 0.0,
                estimated_remaining_hours: None,
            },
            parent_id: None,
            sub_goals: Vec::new(),
            dependencies: Vec::new(),
            dependents: Vec::new(),
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            deadline,
            estimated_completion: None,
            started_at: None,
            completed_at: None,
            metadata: HashMap::new(),
        };

        self.goals.insert(id.clone(), goal);
        self.emit_event(GoalV2Event {
            goal_id: goal_id_for_event,
            event_type: GoalV2EventType::Created,
            timestamp: now,
            details: None,
        });

        Ok(id)
    }

    /// Start a goal.
    pub fn start_goal(&mut self, id: &GoalV2Id) -> Result<(), GoalEngineError> {
        let blocked;

        {
            let goal = self
                .goals
                .get(id)
                .ok_or_else(|| GoalEngineError::NotFound(id.0.clone()))?;

            if goal.state != GoalV2State::Draft {
                return Err(GoalEngineError::InvalidState(format!(
                    "Cannot start goal in state {:?}",
                    goal.state
                )));
            }

            blocked = goal.dependencies.iter().any(|dep_id| {
                self.goals
                    .get(dep_id)
                    .map(|g| g.state != GoalV2State::Completed)
                    .unwrap_or(true)
            });
        }

        {
            let goal = self
                .goals
                .get_mut(id)
                .ok_or_else(|| GoalEngineError::NotFound(id.0.clone()))?;

            if blocked {
                goal.state = GoalV2State::Blocked;
                return Ok(());
            }

            goal.state = GoalV2State::Active;
            goal.started_at = Some(chrono::Utc::now());
            goal.updated_at = chrono::Utc::now();
        }

        self.emit_event(GoalV2Event {
            goal_id: id.clone(),
            event_type: GoalV2EventType::Started,
            timestamp: chrono::Utc::now(),
            details: None,
        });

        Ok(())
    }

    /// Update goal progress.
    pub fn update_progress(&mut self, id: &GoalV2Id, progress: f32) -> Result<(), GoalEngineError> {
        let should_complete;
        let dependents;
        let old_progress;

        {
            let goal = self
                .goals
                .get_mut(id)
                .ok_or_else(|| GoalEngineError::NotFound(id.0.clone()))?;

            old_progress = goal.progress.current;
            goal.progress.current = progress.clamp(0.0, 1.0);
            goal.updated_at = chrono::Utc::now();

            should_complete = progress >= 1.0;
            if should_complete {
                goal.state = GoalV2State::Completed;
                goal.completed_at = Some(chrono::Utc::now());
                dependents = goal.dependents.clone();
            } else {
                dependents = Vec::new();
            }
        }

        self.emit_event(GoalV2Event {
            goal_id: id.clone(),
            event_type: GoalV2EventType::ProgressUpdated {
                old: old_progress,
                new: progress,
            },
            timestamp: chrono::Utc::now(),
            details: None,
        });

        if should_complete {
            self.emit_event(GoalV2Event {
                goal_id: id.clone(),
                event_type: GoalV2EventType::Completed,
                timestamp: chrono::Utc::now(),
                details: None,
            });

            for dep_id in dependents {
                self.unblock_if_ready(&dep_id);
            }
        }

        Ok(())
    }

    /// Add a sub-goal.
    pub fn add_subgoal(
        &mut self,
        goal_id: &GoalV2Id,
        title: String,
    ) -> Result<GoalV2Id, GoalEngineError> {
        let goal = self
            .goals
            .get_mut(goal_id)
            .ok_or_else(|| GoalEngineError::NotFound(goal_id.0.clone()))?;

        let sub_id = GoalV2Id::new();
        goal.sub_goals.push(SubGoal {
            id: sub_id.clone(),
            title,
            completed: false,
            completed_at: None,
        });
        goal.updated_at = chrono::Utc::now();
        Ok(sub_id)
    }

    /// Complete a sub-goal.
    pub fn complete_subgoal(
        &mut self,
        goal_id: &GoalV2Id,
        subgoal_id: &GoalV2Id,
    ) -> Result<(), GoalEngineError> {
        let goal = self
            .goals
            .get_mut(goal_id)
            .ok_or_else(|| GoalEngineError::NotFound(goal_id.0.clone()))?;

        let sub = goal
            .sub_goals
            .iter_mut()
            .find(|s| s.id == *subgoal_id)
            .ok_or_else(|| GoalEngineError::SubGoalNotFound(subgoal_id.0.clone()))?;

        sub.completed = true;
        sub.completed_at = Some(chrono::Utc::now());

        // Recalculate subgoal progress
        let total = goal.sub_goals.len();
        let completed = goal.sub_goals.iter().filter(|s| s.completed).count();
        goal.progress.subgoal_progress = if total > 0 {
            completed as f32 / total as f32
        } else {
            0.0
        };

        // Auto-calculate overall progress
        goal.progress.current =
            (goal.progress.milestone_progress + goal.progress.subgoal_progress) / 2.0;

        goal.updated_at = chrono::Utc::now();

        self.emit_event(GoalV2Event {
            goal_id: goal_id.clone(),
            event_type: GoalV2EventType::SubGoalCompleted {
                subgoal_id: subgoal_id.clone(),
            },
            timestamp: chrono::Utc::now(),
            details: None,
        });

        Ok(())
    }

    /// Add a dependency: `goal_id` depends on `dependency_id`.
    pub fn add_dependency(
        &mut self,
        goal_id: &GoalV2Id,
        dependency_id: &GoalV2Id,
    ) -> Result<(), GoalEngineError> {
        if goal_id == dependency_id {
            return Err(GoalEngineError::SelfDependency);
        }

        {
            let goal = self
                .goals
                .get_mut(goal_id)
                .ok_or_else(|| GoalEngineError::NotFound(goal_id.0.clone()))?;
            goal.dependencies.push(dependency_id.clone());
            goal.updated_at = chrono::Utc::now();
        }

        {
            let dep = self
                .goals
                .get_mut(dependency_id)
                .ok_or_else(|| GoalEngineError::NotFound(dependency_id.0.clone()))?;
            dep.dependents.push(goal_id.clone());
            dep.updated_at = chrono::Utc::now();
        }

        Ok(())
    }

    /// Pause a goal.
    pub fn pause_goal(&mut self, id: &GoalV2Id) -> Result<(), GoalEngineError> {
        let goal = self
            .goals
            .get_mut(id)
            .ok_or_else(|| GoalEngineError::NotFound(id.0.clone()))?;

        if goal.state != GoalV2State::Active {
            return Err(GoalEngineError::InvalidState(format!(
                "Cannot pause goal in state {:?}",
                goal.state
            )));
        }

        goal.state = GoalV2State::Paused;
        goal.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Resume a goal.
    pub fn resume_goal(&mut self, id: &GoalV2Id) -> Result<(), GoalEngineError> {
        let goal = self
            .goals
            .get_mut(id)
            .ok_or_else(|| GoalEngineError::NotFound(id.0.clone()))?;

        if goal.state != GoalV2State::Paused {
            return Err(GoalEngineError::InvalidState(format!(
                "Cannot resume goal in state {:?}",
                goal.state
            )));
        }

        goal.state = GoalV2State::Active;
        goal.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Cancel a goal.
    pub fn cancel_goal(&mut self, id: &GoalV2Id) -> Result<(), GoalEngineError> {
        {
            let goal = self
                .goals
                .get_mut(id)
                .ok_or_else(|| GoalEngineError::NotFound(id.0.clone()))?;

            goal.state = GoalV2State::Cancelled;
            goal.updated_at = chrono::Utc::now();
        }

        self.emit_event(GoalV2Event {
            goal_id: id.clone(),
            event_type: GoalV2EventType::Cancelled,
            timestamp: chrono::Utc::now(),
            details: None,
        });

        Ok(())
    }

    /// Get a goal by ID.
    pub fn get(&self, id: &GoalV2Id) -> Option<&GoalV2> {
        self.goals.get(id)
    }

    /// Get all active goals.
    pub fn active_goals(&self) -> Vec<&GoalV2> {
        self.goals
            .values()
            .filter(|g| g.state == GoalV2State::Active)
            .collect()
    }

    /// Get goals by priority.
    pub fn goals_by_priority(&self) -> Vec<&GoalV2> {
        let mut goals: Vec<&GoalV2> = self.goals.values().collect();
        goals.sort_by_key(|g| g.priority);
        goals
    }

    /// Get blocked goals.
    pub fn blocked_goals(&self) -> Vec<&GoalV2> {
        self.goals
            .values()
            .filter(|g| g.state == GoalV2State::Blocked)
            .collect()
    }

    /// Get all goals.
    pub fn all(&self) -> Vec<&GoalV2> {
        self.goals.values().collect()
    }

    /// Get event log.
    pub fn event_log(&self) -> &[GoalV2Event] {
        &self.event_log
    }

    fn unblock_if_ready(&mut self, goal_id: &GoalV2Id) {
        let should_unblock;

        {
            if let Some(goal) = self.goals.get(goal_id) {
                if goal.state != GoalV2State::Blocked {
                    return;
                }
                should_unblock = goal.dependencies.iter().all(|dep_id| {
                    self.goals
                        .get(dep_id)
                        .map(|g| g.state == GoalV2State::Completed)
                        .unwrap_or(true)
                });
            } else {
                return;
            }
        }

        if should_unblock {
            if let Some(goal) = self.goals.get_mut(goal_id) {
                goal.state = GoalV2State::Active;
                goal.updated_at = chrono::Utc::now();
            }
            self.emit_event(GoalV2Event {
                goal_id: goal_id.clone(),
                event_type: GoalV2EventType::Unblocked,
                timestamp: chrono::Utc::now(),
                details: None,
            });
        }
    }

    fn emit_event(&mut self, event: GoalV2Event) {
        self.event_log.push(event);
        if self.event_log.len() > self.max_event_log {
            let excess = self.event_log.len() - self.max_event_log;
            self.event_log.drain(..excess);
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum GoalEngineError {
    #[error("Goal not found: {0}")]
    NotFound(String),

    #[error("Sub-goal not found: {0}")]
    SubGoalNotFound(String),

    #[error("Self-dependency not allowed")]
    SelfDependency,

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Capacity reached: {0} goals maximum")]
    CapacityReached(usize),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_creation() {
        let _engine = GoalEngineV2::default_engine();
    }

    #[test]
    fn create_goal() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Build VOXY".to_string(),
                "AI OS".to_string(),
                GoalPriority::Critical,
                None,
            )
            .unwrap();
        assert!(engine.get(&id).is_some());
    }

    #[test]
    fn start_goal() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        engine.start_goal(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, GoalV2State::Active);
    }

    #[test]
    fn update_progress() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        engine.start_goal(&id).unwrap();
        engine.update_progress(&id, 0.5).unwrap();
        assert_eq!(engine.get(&id).unwrap().progress.current, 0.5);
    }

    #[test]
    fn complete_goal() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        engine.start_goal(&id).unwrap();
        engine.update_progress(&id, 1.0).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, GoalV2State::Completed);
    }

    #[test]
    fn add_subgoal() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        let _sub_id = engine.add_subgoal(&id, "Sub1".to_string()).unwrap();
        assert_eq!(engine.get(&id).unwrap().sub_goals.len(), 1);
    }

    #[test]
    fn complete_subgoal() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        engine.add_subgoal(&id, "S1".to_string()).unwrap();
        engine.add_subgoal(&id, "S2".to_string()).unwrap();
        let sub_id = engine.get(&id).unwrap().sub_goals[0].id.clone();
        engine.complete_subgoal(&id, &sub_id).unwrap();
        assert_eq!(engine.get(&id).unwrap().progress.subgoal_progress, 0.5);
    }

    #[test]
    fn add_dependency() {
        let mut engine = GoalEngineV2::default_engine();
        let id1 = engine
            .create_goal("A".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        let id2 = engine
            .create_goal("B".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        engine.add_dependency(&id2, &id1).unwrap();
        assert!(engine.get(&id2).unwrap().dependencies.contains(&id1));
        assert!(engine.get(&id1).unwrap().dependents.contains(&id2));
    }

    #[test]
    fn blocked_goal() {
        let mut engine = GoalEngineV2::default_engine();
        let id1 = engine
            .create_goal("A".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        let id2 = engine
            .create_goal("B".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        engine.add_dependency(&id2, &id1).unwrap();
        engine.start_goal(&id2).unwrap();
        assert_eq!(engine.get(&id2).unwrap().state, GoalV2State::Blocked);
    }

    #[test]
    fn unblock_on_dependency_complete() {
        let mut engine = GoalEngineV2::default_engine();
        let id1 = engine
            .create_goal("A".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        let id2 = engine
            .create_goal("B".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        engine.add_dependency(&id2, &id1).unwrap();
        engine.start_goal(&id1).unwrap();
        engine.start_goal(&id2).unwrap();
        assert_eq!(engine.get(&id2).unwrap().state, GoalV2State::Blocked);
        engine.update_progress(&id1, 1.0).unwrap();
        assert_eq!(engine.get(&id2).unwrap().state, GoalV2State::Active);
    }

    #[test]
    fn pause_resume_goal() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        engine.start_goal(&id).unwrap();
        engine.pause_goal(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, GoalV2State::Paused);
        engine.resume_goal(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, GoalV2State::Active);
    }

    #[test]
    fn cancel_goal() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        engine.cancel_goal(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, GoalV2State::Cancelled);
    }

    #[test]
    fn goals_by_priority() {
        let mut engine = GoalEngineV2::default_engine();
        engine
            .create_goal("Low".to_string(), "".to_string(), GoalPriority::Low, None)
            .unwrap();
        engine
            .create_goal(
                "Critical".to_string(),
                "".to_string(),
                GoalPriority::Critical,
                None,
            )
            .unwrap();
        let goals = engine.goals_by_priority();
        assert_eq!(goals[0].priority, GoalPriority::Critical);
    }

    #[test]
    fn active_goals() {
        let mut engine = GoalEngineV2::default_engine();
        let id1 = engine
            .create_goal("A".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        let _id2 = engine
            .create_goal("B".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        engine.start_goal(&id1).unwrap();
        assert_eq!(engine.active_goals().len(), 1);
    }

    #[test]
    fn blocked_goals() {
        let mut engine = GoalEngineV2::default_engine();
        let id1 = engine
            .create_goal("A".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        let id2 = engine
            .create_goal("B".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        engine.add_dependency(&id2, &id1).unwrap();
        engine.start_goal(&id2).unwrap();
        assert_eq!(engine.blocked_goals().len(), 1);
    }

    #[test]
    fn self_dependency_error() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal("A".to_string(), "".to_string(), GoalPriority::Medium, None)
            .unwrap();
        let result = engine.add_dependency(&id, &id);
        assert!(result.is_err());
    }

    #[test]
    fn not_found_error() {
        let mut engine = GoalEngineV2::default_engine();
        let result = engine.start_goal(&GoalV2Id::new());
        assert!(result.is_err());
    }

    #[test]
    fn event_log() {
        let mut engine = GoalEngineV2::default_engine();
        let id = engine
            .create_goal(
                "Test".to_string(),
                "".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        engine.start_goal(&id).unwrap();
        assert!(!engine.event_log().is_empty());
    }
}
