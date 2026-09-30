//! Autonomous Workflows — scheduled/recurring tasks that run without user input.
//!
//! Enables VOXY to independently:
//! - Run morning routines (check GitHub, read issues, generate reports)
//! - Monitor systems and trigger alerts
//! - Execute recurring maintenance tasks
//! - Continue pending work automatically

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkflowId(pub String);

impl WorkflowId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for WorkflowId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StepId(pub String);

impl Default for StepId {
    fn default() -> Self {
        Self::new()
    }
}

impl StepId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

/// How often a workflow should run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Schedule {
    /// Run once at a specific time.
    Once(chrono::DateTime<chrono::Utc>),
    /// Run every N seconds.
    Interval(Duration),
    /// Run daily at a specific time (hour, minute).
    Daily { hour: u32, minute: u32 },
    /// Run weekly on specific weekdays.
    Weekly {
        weekdays: Vec<chrono::Weekday>,
        hour: u32,
        minute: u32,
    },
    /// Run when a condition is met.
    Conditional(String),
    /// Run immediately and repeat.
    Immediate,
}

/// Current state of a workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowState {
    /// Not yet started.
    Pending,
    /// Currently running.
    Running,
    /// Paused by user or system.
    Paused,
    /// Waiting for next scheduled run.
    Waiting,
    /// Completed successfully.
    Completed,
    /// Failed with error.
    Failed(String),
    /// Cancelled.
    Cancelled,
}

/// A single step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: StepId,
    pub name: String,
    pub action: String,
    pub parameters: HashMap<String, String>,
    pub timeout_secs: Option<u64>,
    pub retry_count: u32,
    pub max_retries: u32,
    pub state: WorkflowStepState,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowStepState {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// A complete workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: WorkflowId,
    pub name: String,
    pub description: String,
    pub schedule: Schedule,
    pub steps: Vec<WorkflowStep>,
    pub state: WorkflowState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub last_run: Option<chrono::DateTime<chrono::Utc>>,
    pub next_run: Option<chrono::DateTime<chrono::Utc>>,
    pub run_count: u64,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl Workflow {
    pub fn new(name: String, description: String) -> Self {
        Self {
            id: WorkflowId::new(),
            name,
            description,
            schedule: Schedule::Immediate,
            steps: Vec::new(),
            state: WorkflowState::Pending,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_run: None,
            next_run: None,
            run_count: 0,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

/// Event emitted when a workflow changes state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub workflow_id: WorkflowId,
    pub event_type: WorkflowEventType,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowEventType {
    Started,
    StepStarted { step_id: StepId },
    StepCompleted { step_id: StepId },
    StepFailed { step_id: StepId, error: String },
    Completed,
    Failed { error: String },
    Paused,
    Resumed,
    Cancelled,
}

// ============================================================================
// Workflow Engine
// ============================================================================

/// Manages and executes autonomous workflows.
pub struct WorkflowEngine {
    workflows: HashMap<WorkflowId, Workflow>,
    event_log: Vec<WorkflowEvent>,
    max_workflows: usize,
    max_event_log: usize,
    #[allow(dead_code)]
    last_tick: Instant,
}

impl WorkflowEngine {
    pub fn new(max_workflows: usize, max_event_log: usize) -> Self {
        Self {
            workflows: HashMap::new(),
            event_log: Vec::new(),
            max_workflows,
            max_event_log,
            last_tick: Instant::now(),
        }
    }

    pub fn default_engine() -> Self {
        Self::new(100, 1000)
    }

    /// Register a new workflow.
    pub fn register(&mut self, workflow: Workflow) -> Result<WorkflowId, WorkflowError> {
        if self.workflows.len() >= self.max_workflows {
            return Err(WorkflowError::CapacityReached(self.max_workflows));
        }
        let id = workflow.id.clone();
        self.workflows.insert(id.clone(), workflow);
        Ok(id)
    }

    /// Get a workflow by ID.
    pub fn get(&self, id: &WorkflowId) -> Option<&Workflow> {
        self.workflows.get(id)
    }

    /// Get all workflows.
    pub fn all(&self) -> Vec<&Workflow> {
        self.workflows.values().collect()
    }

    /// Get workflows that are due to run.
    pub fn due_workflows(&self) -> Vec<&Workflow> {
        let now = chrono::Utc::now();
        self.workflows
            .values()
            .filter(|w| {
                matches!(w.state, WorkflowState::Pending | WorkflowState::Waiting)
                    && w.next_run
                        .map(|nr| nr <= now)
                        .unwrap_or(matches!(w.schedule, Schedule::Immediate))
            })
            .collect()
    }

    /// Start a workflow execution.
    pub fn start(&mut self, id: &WorkflowId) -> Result<(), WorkflowError> {
        let workflow = self
            .workflows
            .get_mut(id)
            .ok_or_else(|| WorkflowError::NotFound(id.0.clone()))?;

        if workflow.state == WorkflowState::Running {
            return Err(WorkflowError::AlreadyRunning(id.0.clone()));
        }

        workflow.state = WorkflowState::Running;
        workflow.last_run = Some(chrono::Utc::now());

        // Reset all steps to pending
        for step in &mut workflow.steps {
            step.state = WorkflowStepState::Pending;
            step.result = None;
            step.error = None;
        }

        self.emit_event(WorkflowEvent {
            workflow_id: id.clone(),
            event_type: WorkflowEventType::Started,
            timestamp: chrono::Utc::now(),
            details: None,
        });

        Ok(())
    }

    /// Complete a step in a workflow.
    pub fn complete_step(
        &mut self,
        workflow_id: &WorkflowId,
        step_id: &StepId,
        result: String,
    ) -> Result<(), WorkflowError> {
        let (all_done, _new_state) = {
            let workflow = self
                .workflows
                .get_mut(workflow_id)
                .ok_or_else(|| WorkflowError::NotFound(workflow_id.0.clone()))?;

            let step = workflow
                .steps
                .iter_mut()
                .find(|s| s.id == *step_id)
                .ok_or_else(|| WorkflowError::StepNotFound(step_id.0.clone()))?;

            step.state = WorkflowStepState::Completed;
            step.result = Some(result);

            let all_done = workflow.steps.iter().all(|s| {
                s.state == WorkflowStepState::Completed || s.state == WorkflowStepState::Skipped
            });

            if all_done {
                workflow.state = WorkflowState::Completed;
                workflow.run_count += 1;
            }

            (all_done, workflow.state.clone())
        };

        self.emit_event(WorkflowEvent {
            workflow_id: workflow_id.clone(),
            event_type: WorkflowEventType::StepStarted {
                step_id: step_id.clone(),
            },
            timestamp: chrono::Utc::now(),
            details: None,
        });

        if all_done {
            self.emit_event(WorkflowEvent {
                workflow_id: workflow_id.clone(),
                event_type: WorkflowEventType::Completed,
                timestamp: chrono::Utc::now(),
                details: None,
            });
        }

        Ok(())
    }

    /// Fail a step in a workflow.
    pub fn fail_step(
        &mut self,
        workflow_id: &WorkflowId,
        step_id: &StepId,
        error: String,
    ) -> Result<(), WorkflowError> {
        let should_emit_fail = {
            let workflow = self
                .workflows
                .get_mut(workflow_id)
                .ok_or_else(|| WorkflowError::NotFound(workflow_id.0.clone()))?;

            let step = workflow
                .steps
                .iter_mut()
                .find(|s| s.id == *step_id)
                .ok_or_else(|| WorkflowError::StepNotFound(step_id.0.clone()))?;

            step.error = Some(error.clone());

            if step.retry_count < step.max_retries {
                step.retry_count += 1;
                step.state = WorkflowStepState::Pending; // Retry
                false
            } else {
                step.state = WorkflowStepState::Failed;
                workflow.state = WorkflowState::Failed(error.clone());
                true
            }
        };

        if should_emit_fail {
            self.emit_event(WorkflowEvent {
                workflow_id: workflow_id.clone(),
                event_type: WorkflowEventType::Failed { error },
                timestamp: chrono::Utc::now(),
                details: None,
            });
        }

        Ok(())
    }

    /// Pause a workflow.
    pub fn pause(&mut self, id: &WorkflowId) -> Result<(), WorkflowError> {
        let workflow = self
            .workflows
            .get_mut(id)
            .ok_or_else(|| WorkflowError::NotFound(id.0.clone()))?;

        if workflow.state == WorkflowState::Running {
            workflow.state = WorkflowState::Paused;
            self.emit_event(WorkflowEvent {
                workflow_id: id.clone(),
                event_type: WorkflowEventType::Paused,
                timestamp: chrono::Utc::now(),
                details: None,
            });
            Ok(())
        } else {
            let state = workflow.state.clone();
            Err(WorkflowError::InvalidState(format!(
                "Cannot pause workflow in state {:?}",
                state
            )))
        }
    }

    /// Resume a paused workflow.
    pub fn resume(&mut self, id: &WorkflowId) -> Result<(), WorkflowError> {
        let workflow = self
            .workflows
            .get_mut(id)
            .ok_or_else(|| WorkflowError::NotFound(id.0.clone()))?;

        if workflow.state == WorkflowState::Paused {
            workflow.state = WorkflowState::Running;
            self.emit_event(WorkflowEvent {
                workflow_id: id.clone(),
                event_type: WorkflowEventType::Resumed,
                timestamp: chrono::Utc::now(),
                details: None,
            });
            Ok(())
        } else {
            let state = workflow.state.clone();
            Err(WorkflowError::InvalidState(format!(
                "Cannot resume workflow in state {:?}",
                state
            )))
        }
    }

    /// Cancel a workflow.
    pub fn cancel(&mut self, id: &WorkflowId) -> Result<(), WorkflowError> {
        {
            let workflow = self
                .workflows
                .get_mut(id)
                .ok_or_else(|| WorkflowError::NotFound(id.0.clone()))?;

            workflow.state = WorkflowState::Cancelled;
        }

        self.emit_event(WorkflowEvent {
            workflow_id: id.clone(),
            event_type: WorkflowEventType::Cancelled,
            timestamp: chrono::Utc::now(),
            details: None,
        });
        Ok(())
    }

    /// Remove a workflow.
    pub fn remove(&mut self, id: &WorkflowId) -> Result<Workflow, WorkflowError> {
        self.workflows
            .remove(id)
            .ok_or_else(|| WorkflowError::NotFound(id.0.clone()))
    }

    /// Get the event log.
    pub fn event_log(&self) -> &[WorkflowEvent] {
        &self.event_log
    }

    fn emit_event(&mut self, event: WorkflowEvent) {
        self.event_log.push(event);
        if self.event_log.len() > self.max_event_log {
            self.event_log
                .drain(0..self.event_log.len() - self.max_event_log);
        }
    }
}

// ============================================================================
// Builder
// ============================================================================

pub struct WorkflowBuilder {
    workflow: Workflow,
}

impl WorkflowBuilder {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            workflow: Workflow {
                id: WorkflowId::new(),
                name: name.to_string(),
                description: description.to_string(),
                schedule: Schedule::Immediate,
                steps: Vec::new(),
                state: WorkflowState::Pending,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                last_run: None,
                next_run: None,
                run_count: 0,
                tags: Vec::new(),
                metadata: HashMap::new(),
            },
        }
    }

    pub fn schedule(mut self, schedule: Schedule) -> Self {
        self.workflow.schedule = schedule;
        self
    }

    pub fn step(mut self, name: &str, action: &str) -> Self {
        self.workflow.steps.push(WorkflowStep {
            id: StepId::new(),
            name: name.to_string(),
            action: action.to_string(),
            parameters: HashMap::new(),
            timeout_secs: Some(300),
            retry_count: 0,
            max_retries: 3,
            state: WorkflowStepState::Pending,
            result: None,
            error: None,
        });
        self
    }

    pub fn build(self) -> Workflow {
        self.workflow
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum WorkflowError {
    #[error("Workflow not found: {0}")]
    NotFound(String),

    #[error("Step not found: {0}")]
    StepNotFound(String),

    #[error("Workflow already running: {0}")]
    AlreadyRunning(String),

    #[error("Capacity reached: {0} workflows maximum")]
    CapacityReached(usize),

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_engine_creation() {
        let engine = WorkflowEngine::default_engine();
        assert_eq!(engine.all().len(), 0);
    }

    #[test]
    fn register_workflow() {
        let mut engine = WorkflowEngine::default_engine();
        let wf = Workflow::new("Morning Routine".to_string(), "Daily tasks".to_string());
        let id = engine.register(wf).unwrap();
        assert!(engine.get(&id).is_some());
    }

    #[test]
    fn capacity_limit() {
        let mut engine = WorkflowEngine::new(2, 100);
        engine
            .register(Workflow::new("W1".to_string(), "".to_string()))
            .unwrap();
        engine
            .register(Workflow::new("W2".to_string(), "".to_string()))
            .unwrap();
        let result = engine.register(Workflow::new("W3".to_string(), "".to_string()));
        assert!(result.is_err());
    }

    #[test]
    fn start_workflow() {
        let mut engine = WorkflowEngine::default_engine();
        let wf = Workflow::new("Test".to_string(), "".to_string());
        let id = engine.register(wf).unwrap();
        engine.start(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, WorkflowState::Running);
    }

    #[test]
    fn complete_workflow() {
        let mut engine = WorkflowEngine::default_engine();
        let mut wf = Workflow::new("Test".to_string(), "".to_string());
        wf.steps.push(WorkflowStep {
            id: StepId::new(),
            name: "Step 1".to_string(),
            action: "do_thing".to_string(),
            parameters: HashMap::new(),
            timeout_secs: None,
            retry_count: 0,
            max_retries: 0,
            state: WorkflowStepState::Pending,
            result: None,
            error: None,
        });
        let id = engine.register(wf).unwrap();
        engine.start(&id).unwrap();

        let step_id = engine.get(&id).unwrap().steps[0].id.clone();
        engine
            .complete_step(&id, &step_id, "done".to_string())
            .unwrap();

        assert_eq!(engine.get(&id).unwrap().state, WorkflowState::Completed);
    }

    #[test]
    fn pause_resume_workflow() {
        let mut engine = WorkflowEngine::default_engine();
        let wf = Workflow::new("Test".to_string(), "".to_string());
        let id = engine.register(wf).unwrap();
        engine.start(&id).unwrap();
        engine.pause(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, WorkflowState::Paused);
        engine.resume(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, WorkflowState::Running);
    }

    #[test]
    fn cancel_workflow() {
        let mut engine = WorkflowEngine::default_engine();
        let wf = Workflow::new("Test".to_string(), "".to_string());
        let id = engine.register(wf).unwrap();
        engine.start(&id).unwrap();
        engine.cancel(&id).unwrap();
        assert_eq!(engine.get(&id).unwrap().state, WorkflowState::Cancelled);
    }

    #[test]
    fn workflow_builder() {
        let wf = WorkflowBuilder::new("Morning", "Daily tasks")
            .schedule(Schedule::Immediate)
            .step("Check GitHub", "check_github")
            .step("Read Issues", "read_issues")
            .step("Generate Report", "generate_report")
            .build();

        assert_eq!(wf.steps.len(), 3);
        assert_eq!(wf.name, "Morning");
    }

    #[test]
    fn remove_workflow() {
        let mut engine = WorkflowEngine::default_engine();
        let wf = Workflow::new("Test".to_string(), "".to_string());
        let id = engine.register(wf).unwrap();
        let removed = engine.remove(&id).unwrap();
        assert_eq!(removed.name, "Test");
        assert!(engine.get(&id).is_none());
    }

    #[test]
    fn not_found_error() {
        let mut engine = WorkflowEngine::default_engine();
        let result = engine.start(&WorkflowId::new());
        assert!(result.is_err());
    }

    #[test]
    fn already_running_error() {
        let mut engine = WorkflowEngine::default_engine();
        let wf = Workflow::new("Test".to_string(), "".to_string());
        let id = engine.register(wf).unwrap();
        engine.start(&id).unwrap();
        let result = engine.start(&id);
        assert!(result.is_err());
    }

    #[test]
    fn step_retry() {
        let mut engine = WorkflowEngine::default_engine();
        let mut wf = Workflow::new("Test".to_string(), "".to_string());
        wf.steps.push(WorkflowStep {
            id: StepId::new(),
            name: "Step 1".to_string(),
            action: "do_thing".to_string(),
            parameters: HashMap::new(),
            timeout_secs: None,
            retry_count: 0,
            max_retries: 2,
            state: WorkflowStepState::Pending,
            result: None,
            error: None,
        });
        let id = engine.register(wf).unwrap();
        engine.start(&id).unwrap();

        let step_id = engine.get(&id).unwrap().steps[0].id.clone();
        engine
            .fail_step(&id, &step_id, "error".to_string())
            .unwrap();

        // Should be retrying (pending)
        let wf = engine.get(&id).unwrap();
        assert_eq!(wf.state, WorkflowState::Running);
        assert_eq!(wf.steps[0].retry_count, 1);
    }

    #[test]
    fn event_log() {
        let mut engine = WorkflowEngine::default_engine();
        let wf = Workflow::new("Test".to_string(), "".to_string());
        let id = engine.register(wf).unwrap();
        engine.start(&id).unwrap();
        assert!(!engine.event_log().is_empty());
    }
}
