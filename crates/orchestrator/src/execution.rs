use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

pub type CorrelationId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineStage {
    Conversation,
    Cognition,
    Planning,
    ToolSelection,
    GuardianCheck,
    WorldModelUpdate,
    Execution,
    Verification,
    Reflection,
    MemoryStorage,
    Learning,
}

impl PipelineStage {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Conversation => "Conversation",
            Self::Cognition => "Cognition",
            Self::Planning => "Planning",
            Self::ToolSelection => "ToolSelection",
            Self::GuardianCheck => "GuardianCheck",
            Self::WorldModelUpdate => "WorldModelUpdate",
            Self::Execution => "Execution",
            Self::Verification => "Verification",
            Self::Reflection => "Reflection",
            Self::MemoryStorage => "MemoryStorage",
            Self::Learning => "Learning",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub id: String,
    pub correlation_id: CorrelationId,
    pub stage: PipelineStage,
    pub event_type: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct StageTimeline {
    pub stage: PipelineStage,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub correlation_id: CorrelationId,
    pub timeline: Vec<StageTimeline>,
    pub audit_events: Vec<AuditEvent>,
    pub session_id: Option<String>,
    pub user_input: Option<String>,
    pub metadata: HashMap<String, String>,
    pub started_at: DateTime<Utc>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            timeline: Vec::new(),
            audit_events: Vec::new(),
            session_id: None,
            user_input: None,
            metadata: HashMap::new(),
            started_at: Utc::now(),
        }
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_user_input(mut self, text: impl Into<String>) -> Self {
        self.user_input = Some(text.into());
        self
    }

    pub fn add_audit_event(&mut self, stage: PipelineStage, event_type: &str, message: &str) {
        self.audit_events.push(AuditEvent {
            id: Uuid::new_v4().to_string(),
            correlation_id: self.correlation_id,
            stage,
            event_type: event_type.to_string(),
            message: message.to_string(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        });
    }

    pub fn start_stage(&mut self, stage: PipelineStage) {
        self.timeline.push(StageTimeline {
            stage,
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            success: false,
            error: None,
        });
        self.add_audit_event(
            stage,
            "stage_started",
            &format!("Stage {} started", stage.name()),
        );
    }

    pub fn complete_stage(&mut self, stage: PipelineStage, success: bool, error: Option<String>) {
        if let Some(entry) = self
            .timeline
            .iter_mut()
            .rev()
            .find(|e| e.stage == stage && e.completed_at.is_none())
        {
            entry.completed_at = Some(Utc::now());
            entry.duration_ms = entry
                .completed_at
                .map(|c| (c - entry.started_at).num_milliseconds() as u64);
            entry.success = success;
            entry.error = error.clone();
        }
        let event_type = if success {
            "stage_completed"
        } else {
            "stage_failed"
        };
        self.add_audit_event(
            stage,
            event_type,
            &format!(
                "Stage {} {}",
                stage.name(),
                if success { "completed" } else { "failed" }
            ),
        );
    }

    pub fn stage_duration_ms(&self, stage: PipelineStage) -> Option<u64> {
        self.timeline
            .iter()
            .rev()
            .find(|e| e.stage == stage)
            .and_then(|e| e.duration_ms)
    }

    pub fn total_duration_ms(&self) -> i64 {
        (Utc::now() - self.started_at).num_milliseconds()
    }

    pub fn is_stage_completed(&self, stage: PipelineStage) -> bool {
        self.timeline
            .iter()
            .any(|e| e.stage == stage && e.completed_at.is_some())
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

pub struct CancellationFlag {
    cancelled: AtomicBool,
    reason: RwLock<Option<String>>,
}

impl CancellationFlag {
    pub fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            reason: RwLock::new(None),
        }
    }

    pub fn cancel(&self, reason: &str) {
        self.cancelled.store(true, Ordering::SeqCst);
        *self.reason.write() = Some(reason.to_string());
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn reason(&self) -> Option<String> {
        self.reason.read().clone()
    }

    pub fn check(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::SeqCst) {
            Err(self
                .reason
                .read()
                .clone()
                .unwrap_or_else(|| "Cancelled".to_string()))
        } else {
            Ok(())
        }
    }
}

impl Default for CancellationFlag {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PipelineInput {
    pub text: Option<String>,
    pub audio: Option<Vec<f32>>,
    pub session_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

pub struct PipelineOutput {
    pub correlation_id: CorrelationId,
    pub success: bool,
    pub response_text: Option<String>,
    pub timeline: Vec<StageTimeline>,
    pub audit_events: Vec<AuditEvent>,
    pub error: Option<String>,
    pub total_duration_ms: i64,
}
