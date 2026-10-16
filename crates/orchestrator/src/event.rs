use std::fmt;

pub enum OrchestratorEvent {
    TaskScheduled {
        task_id: String,
        task_type: String,
        priority: u8,
    },
    TaskStarted {
        task_id: String,
    },
    TaskCompleted {
        task_id: String,
        success: bool,
        duration_ms: u64,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },
    TaskCancelled {
        task_id: String,
        reason: String,
    },
    TaskInterrupted {
        task_id: String,
        source: String,
    },
    GuardianOverrideActivated {
        reason: String,
        affected_tasks: usize,
    },
    GuardianOverrideDeactivated {
        reason: String,
    },
    EmergencyOverride {
        source: String,
        action: String,
    },
    SystemPaused {
        reason: String,
    },
    SystemResumed {
        reason: String,
    },
    SystemShuttingDown {
        reason: String,
    },
    BackgroundTaskStarted {
        task_id: String,
    },
    BackgroundTaskCompleted {
        task_id: String,
    },
    PipelineStageStarted {
        correlation_id: String,
        stage: String,
    },
    PipelineStageCompleted {
        correlation_id: String,
        stage: String,
        success: bool,
        duration_ms: u64,
    },
    PipelineStageFailed {
        correlation_id: String,
        stage: String,
        error: String,
    },
    PipelineCompleted {
        correlation_id: String,
        success: bool,
        total_duration_ms: u64,
        stages_completed: usize,
        stages_failed: usize,
    },
    AuditEvent {
        correlation_id: String,
        stage: String,
        event_type: String,
        message: String,
    },
}

impl fmt::Display for OrchestratorEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskScheduled {
                task_id,
                task_type,
                priority,
            } => write!(
                f,
                "Task scheduled: {} (type: {}, priority: {})",
                task_id, task_type, priority
            ),
            Self::TaskStarted { task_id } => {
                write!(f, "Task started: {}", task_id)
            }
            Self::TaskCompleted {
                task_id,
                success,
                duration_ms,
            } => write!(
                f,
                "Task completed: {} (success: {}, duration: {}ms)",
                task_id, success, duration_ms
            ),
            Self::TaskFailed { task_id, error } => {
                write!(f, "Task failed: {} (error: {})", task_id, error)
            }
            Self::TaskCancelled { task_id, reason } => {
                write!(f, "Task cancelled: {} (reason: {})", task_id, reason)
            }
            Self::TaskInterrupted { task_id, source } => {
                write!(f, "Task interrupted: {} (source: {})", task_id, source)
            }
            Self::GuardianOverrideActivated {
                reason,
                affected_tasks,
            } => write!(
                f,
                "Guardian override activated (reason: {}, affected tasks: {})",
                reason, affected_tasks
            ),
            Self::GuardianOverrideDeactivated { reason } => {
                write!(f, "Guardian override deactivated (reason: {})", reason)
            }
            Self::EmergencyOverride { source, action } => {
                write!(f, "Emergency override from {}: {}", source, action)
            }
            Self::SystemPaused { reason } => {
                write!(f, "System paused: {}", reason)
            }
            Self::SystemResumed { reason } => {
                write!(f, "System resumed: {}", reason)
            }
            Self::SystemShuttingDown { reason } => {
                write!(f, "System shutting down: {}", reason)
            }
            Self::BackgroundTaskStarted { task_id } => {
                write!(f, "Background task started: {}", task_id)
            }
            Self::BackgroundTaskCompleted { task_id } => {
                write!(f, "Background task completed: {}", task_id)
            }
            Self::PipelineStageStarted {
                correlation_id,
                stage,
            } => write!(
                f,
                "Pipeline stage started: {} (correlation: {})",
                stage, correlation_id
            ),
            Self::PipelineStageCompleted {
                correlation_id,
                stage,
                success,
                duration_ms,
            } => write!(
                f,
                "Pipeline stage completed: {} (success: {}, duration: {}ms, correlation: {})",
                stage, success, duration_ms, correlation_id
            ),
            Self::PipelineStageFailed {
                correlation_id,
                stage,
                error,
            } => write!(
                f,
                "Pipeline stage failed: {} (error: {}, correlation: {})",
                stage, error, correlation_id
            ),
            Self::PipelineCompleted {
                correlation_id,
                success,
                total_duration_ms,
                stages_completed,
                stages_failed,
            } => write!(
                f,
                "Pipeline completed: {} (success: {}, duration: {}ms, completed: {}, failed: {})",
                correlation_id, success, total_duration_ms, stages_completed, stages_failed
            ),
            Self::AuditEvent {
                correlation_id,
                stage,
                event_type,
                message,
            } => write!(
                f,
                "Audit [{}] {}: {} - {}",
                stage, event_type, message, correlation_id
            ),
        }
    }
}

impl fmt::Debug for OrchestratorEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskScheduled {
                task_id,
                task_type,
                priority,
            } => f
                .debug_struct("TaskScheduled")
                .field("task_id", task_id)
                .field("task_type", task_type)
                .field("priority", priority)
                .finish(),
            Self::TaskStarted { task_id } => f
                .debug_struct("TaskStarted")
                .field("task_id", task_id)
                .finish(),
            Self::TaskCompleted {
                task_id,
                success,
                duration_ms,
            } => f
                .debug_struct("TaskCompleted")
                .field("task_id", task_id)
                .field("success", success)
                .field("duration_ms", duration_ms)
                .finish(),
            Self::TaskFailed { task_id, error } => f
                .debug_struct("TaskFailed")
                .field("task_id", task_id)
                .field("error", error)
                .finish(),
            Self::TaskCancelled { task_id, reason } => f
                .debug_struct("TaskCancelled")
                .field("task_id", task_id)
                .field("reason", reason)
                .finish(),
            Self::TaskInterrupted { task_id, source } => f
                .debug_struct("TaskInterrupted")
                .field("task_id", task_id)
                .field("source", source)
                .finish(),
            Self::GuardianOverrideActivated {
                reason,
                affected_tasks,
            } => f
                .debug_struct("GuardianOverrideActivated")
                .field("reason", reason)
                .field("affected_tasks", affected_tasks)
                .finish(),
            Self::GuardianOverrideDeactivated { reason } => f
                .debug_struct("GuardianOverrideDeactivated")
                .field("reason", reason)
                .finish(),
            Self::EmergencyOverride { source, action } => f
                .debug_struct("EmergencyOverride")
                .field("source", source)
                .field("action", action)
                .finish(),
            Self::SystemPaused { reason } => f
                .debug_struct("SystemPaused")
                .field("reason", reason)
                .finish(),
            Self::SystemResumed { reason } => f
                .debug_struct("SystemResumed")
                .field("reason", reason)
                .finish(),
            Self::SystemShuttingDown { reason } => f
                .debug_struct("SystemShuttingDown")
                .field("reason", reason)
                .finish(),
            Self::BackgroundTaskStarted { task_id } => f
                .debug_struct("BackgroundTaskStarted")
                .field("task_id", task_id)
                .finish(),
            Self::BackgroundTaskCompleted { task_id } => f
                .debug_struct("BackgroundTaskCompleted")
                .field("task_id", task_id)
                .finish(),
            Self::PipelineStageStarted {
                correlation_id,
                stage,
            } => f
                .debug_struct("PipelineStageStarted")
                .field("correlation_id", correlation_id)
                .field("stage", stage)
                .finish(),
            Self::PipelineStageCompleted {
                correlation_id,
                stage,
                success,
                duration_ms,
            } => f
                .debug_struct("PipelineStageCompleted")
                .field("correlation_id", correlation_id)
                .field("stage", stage)
                .field("success", success)
                .field("duration_ms", duration_ms)
                .finish(),
            Self::PipelineStageFailed {
                correlation_id,
                stage,
                error,
            } => f
                .debug_struct("PipelineStageFailed")
                .field("correlation_id", correlation_id)
                .field("stage", stage)
                .field("error", error)
                .finish(),
            Self::PipelineCompleted {
                correlation_id,
                success,
                total_duration_ms,
                stages_completed,
                stages_failed,
            } => f
                .debug_struct("PipelineCompleted")
                .field("correlation_id", correlation_id)
                .field("success", success)
                .field("total_duration_ms", total_duration_ms)
                .field("stages_completed", stages_completed)
                .field("stages_failed", stages_failed)
                .finish(),
            Self::AuditEvent {
                correlation_id,
                stage,
                event_type,
                message,
            } => f
                .debug_struct("AuditEvent")
                .field("correlation_id", correlation_id)
                .field("stage", stage)
                .field("event_type", event_type)
                .field("message", message)
                .finish(),
        }
    }
}

impl Clone for OrchestratorEvent {
    fn clone(&self) -> Self {
        match self {
            Self::TaskScheduled {
                task_id,
                task_type,
                priority,
            } => Self::TaskScheduled {
                task_id: task_id.clone(),
                task_type: task_type.clone(),
                priority: *priority,
            },
            Self::TaskStarted { task_id } => Self::TaskStarted {
                task_id: task_id.clone(),
            },
            Self::TaskCompleted {
                task_id,
                success,
                duration_ms,
            } => Self::TaskCompleted {
                task_id: task_id.clone(),
                success: *success,
                duration_ms: *duration_ms,
            },
            Self::TaskFailed { task_id, error } => Self::TaskFailed {
                task_id: task_id.clone(),
                error: error.clone(),
            },
            Self::TaskCancelled { task_id, reason } => Self::TaskCancelled {
                task_id: task_id.clone(),
                reason: reason.clone(),
            },
            Self::TaskInterrupted { task_id, source } => Self::TaskInterrupted {
                task_id: task_id.clone(),
                source: source.clone(),
            },
            Self::GuardianOverrideActivated {
                reason,
                affected_tasks,
            } => Self::GuardianOverrideActivated {
                reason: reason.clone(),
                affected_tasks: *affected_tasks,
            },
            Self::GuardianOverrideDeactivated { reason } => Self::GuardianOverrideDeactivated {
                reason: reason.clone(),
            },
            Self::EmergencyOverride { source, action } => Self::EmergencyOverride {
                source: source.clone(),
                action: action.clone(),
            },
            Self::SystemPaused { reason } => Self::SystemPaused {
                reason: reason.clone(),
            },
            Self::SystemResumed { reason } => Self::SystemResumed {
                reason: reason.clone(),
            },
            Self::SystemShuttingDown { reason } => Self::SystemShuttingDown {
                reason: reason.clone(),
            },
            Self::BackgroundTaskStarted { task_id } => Self::BackgroundTaskStarted {
                task_id: task_id.clone(),
            },
            Self::BackgroundTaskCompleted { task_id } => Self::BackgroundTaskCompleted {
                task_id: task_id.clone(),
            },
            Self::PipelineStageStarted {
                correlation_id,
                stage,
            } => Self::PipelineStageStarted {
                correlation_id: correlation_id.clone(),
                stage: stage.clone(),
            },
            Self::PipelineStageCompleted {
                correlation_id,
                stage,
                success,
                duration_ms,
            } => Self::PipelineStageCompleted {
                correlation_id: correlation_id.clone(),
                stage: stage.clone(),
                success: *success,
                duration_ms: *duration_ms,
            },
            Self::PipelineStageFailed {
                correlation_id,
                stage,
                error,
            } => Self::PipelineStageFailed {
                correlation_id: correlation_id.clone(),
                stage: stage.clone(),
                error: error.clone(),
            },
            Self::PipelineCompleted {
                correlation_id,
                success,
                total_duration_ms,
                stages_completed,
                stages_failed,
            } => Self::PipelineCompleted {
                correlation_id: correlation_id.clone(),
                success: *success,
                total_duration_ms: *total_duration_ms,
                stages_completed: *stages_completed,
                stages_failed: *stages_failed,
            },
            Self::AuditEvent {
                correlation_id,
                stage,
                event_type,
                message,
            } => Self::AuditEvent {
                correlation_id: correlation_id.clone(),
                stage: stage.clone(),
                event_type: event_type.clone(),
                message: message.clone(),
            },
        }
    }
}
