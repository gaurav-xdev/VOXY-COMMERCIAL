use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::cancellation::CancellationManager;
use crate::config::OrchestratorConfig;
use crate::error::{OrchestratorError, Result};
use crate::event::OrchestratorEvent;
use crate::scheduler::TaskSchedulerInternal;
use crate::types::{
    ComponentStatus, JobId, JobState, JobTicket, OrchestratorTask, ScheduleSpec, SystemComponent,
    SystemSnapshot, TaskId, TaskPriority, TaskState,
};

use super::cancellation::DefaultCancellationManager;
use super::execution::{CancellationFlag, PipelineInput, PipelineOutput};
use super::interrupts::DefaultInterruptionManager;
use super::pipeline::{execute_pipeline, StageHandler};
use super::scheduler::PriorityTaskScheduler;

#[async_trait]
pub trait SystemCoordinator: Send + Sync {
    async fn init(&self, config: &OrchestratorConfig) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;

    async fn submit_task(&self, task: OrchestratorTask) -> Result<TaskId>;
    async fn submit_job(&self, tasks: Vec<OrchestratorTask>) -> Result<JobTicket>;
    async fn cancel_task(&self, task_id: &TaskId, reason: &str) -> Result<()>;
    async fn cancel_job(&self, job_id: &JobId, reason: &str) -> Result<()>;

    async fn schedule_task(&self, task: OrchestratorTask, spec: ScheduleSpec) -> Result<TaskId>;
    async fn reschedule_task(&self, task_id: &TaskId, spec: ScheduleSpec) -> Result<()>;

    async fn set_task_priority(&self, task_id: &TaskId, priority: TaskPriority) -> Result<()>;
    async fn get_queued_tasks(
        &self,
        min_priority: Option<TaskPriority>,
    ) -> Result<Vec<OrchestratorTask>>;

    async fn activate_guardian_override(&self, reason: &str) -> Result<()>;
    async fn deactivate_guardian_override(&self, reason: &str) -> Result<()>;
    async fn is_guardian_override_active(&self) -> bool;
    async fn get_active_overrides(&self) -> Result<Vec<String>>;

    async fn pause_system(&self, reason: &str) -> Result<()>;
    async fn resume_system(&self, reason: &str) -> Result<()>;
    async fn is_system_paused(&self) -> bool;
    async fn emergency_override(&self, source: &str, action: &str) -> Result<()>;

    async fn system_snapshot(&self) -> Result<SystemSnapshot>;
    async fn component_status(&self, component: &SystemComponent) -> Result<ComponentStatus>;
    async fn get_active_tasks(&self) -> Result<Vec<OrchestratorTask>>;
    async fn get_task_history(&self, limit: usize) -> Result<Vec<OrchestratorTask>>;

    async fn on_event(&self, handler: Box<dyn Fn(OrchestratorEvent) + Send + Sync>) -> Result<()>;

    async fn submit_background_task(&self, task: OrchestratorTask) -> Result<TaskId>;
    async fn list_background_tasks(&self) -> Result<Vec<OrchestratorTask>>;
}

pub struct DefaultSystemCoordinator {
    scheduler: Arc<PriorityTaskScheduler>,
    cancellation: Arc<DefaultCancellationManager>,
    interrupt: Arc<DefaultInterruptionManager>,
    config: OrchestratorConfig,
    #[allow(clippy::type_complexity)]
    event_handlers: RwLock<Vec<Box<dyn Fn(OrchestratorEvent) + Send + Sync>>>,
    paused: AtomicBool,
    guardian_override: AtomicBool,
    overrides: RwLock<Vec<String>>,
    task_history: RwLock<VecDeque<OrchestratorTask>>,
    jobs: RwLock<HashMap<JobId, JobTicket>>,
    background_tasks: RwLock<Vec<OrchestratorTask>>,
    started_at: DateTime<Utc>,
    pipeline_stages: Vec<Box<dyn StageHandler>>,
}

impl DefaultSystemCoordinator {
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            scheduler: Arc::new(PriorityTaskScheduler::new()),
            cancellation: Arc::new(DefaultCancellationManager::new()),
            interrupt: Arc::new(DefaultInterruptionManager::new()),
            event_handlers: RwLock::new(Vec::new()),
            paused: AtomicBool::new(false),
            guardian_override: AtomicBool::new(false),
            overrides: RwLock::new(Vec::new()),
            task_history: RwLock::new(VecDeque::new()),
            jobs: RwLock::new(HashMap::new()),
            background_tasks: RwLock::new(Vec::new()),
            started_at: Utc::now(),
            pipeline_stages: Vec::new(),
            config,
        }
    }

    pub fn with_stages(mut self, stages: Vec<Box<dyn StageHandler>>) -> Self {
        self.pipeline_stages = stages;
        self
    }

    pub fn register_stage(&mut self, handler: Box<dyn StageHandler>) {
        self.pipeline_stages.push(handler);
    }

    pub async fn execute_pipeline(&self, input: PipelineInput) -> PipelineOutput {
        let cancellation = CancellationFlag::new();

        self.emit_event(OrchestratorEvent::TaskStarted {
            task_id: uuid_cor_id(),
        });

        let output = execute_pipeline(
            input,
            &self.pipeline_stages[..],
            &cancellation,
            self.config.task_timeout_seconds,
            self.config.max_retries_per_task,
        )
        .await;

        self.emit_event(OrchestratorEvent::TaskCompleted {
            task_id: uuid_cor_id(),
            success: output.success,
            duration_ms: output.total_duration_ms as u64,
        });

        output
    }

    fn record_task_history(&self, task: &OrchestratorTask) {
        let mut history = self.task_history.write();
        history.push_back(task.clone());
        if history.len() > 1000 {
            history.pop_front();
        }
    }

    fn emit_event(&self, event: OrchestratorEvent) {
        let handlers = self.event_handlers.read();
        let len = handlers.len();
        if len == 0 {
            return;
        }
        for handler in handlers.iter().take(len - 1) {
            handler(event.clone());
        }
        handlers.last().unwrap()(event);
    }
}

fn uuid_cor_id() -> String {
    Uuid::new_v4().to_string()
}

#[async_trait]
impl SystemCoordinator for DefaultSystemCoordinator {
    async fn init(&self, _config: &OrchestratorConfig) -> Result<()> {
        self.scheduler
            .clear()
            .await
            .map_err(|e| OrchestratorError::SchedulingError(e.to_string()))?;
        self.cancellation.clear().await;
        self.interrupt.clear().await;
        self.paused.store(false, Ordering::SeqCst);
        self.guardian_override.store(false, Ordering::SeqCst);
        self.overrides.write().clear();
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        self.emit_event(OrchestratorEvent::SystemShuttingDown {
            reason: "coordinator shutdown".to_string(),
        });
        self.scheduler
            .clear()
            .await
            .map_err(|e| OrchestratorError::SchedulingError(e.to_string()))?;
        self.cancellation.cancel_all("system shutdown").await?;
        self.interrupt.clear().await;
        Ok(())
    }

    async fn submit_task(&self, task: OrchestratorTask) -> Result<TaskId> {
        if self.paused.load(Ordering::SeqCst) {
            return Err(OrchestratorError::SchedulingError(
                "system is paused".to_string(),
            ));
        }
        let task_id = task.id.clone();
        self.cancellation
            .register_cancellation_token(&task_id)
            .await?;
        self.scheduler.enqueue(task.clone()).await?;
        self.record_task_history(&task);
        self.emit_event(OrchestratorEvent::TaskScheduled {
            task_id: task_id.0.clone(),
            task_type: task.task_type.clone(),
            priority: task.priority.value(),
        });
        Ok(task_id)
    }

    async fn submit_job(&self, tasks: Vec<OrchestratorTask>) -> Result<JobTicket> {
        let job_id = JobId(Uuid::new_v4().to_string());
        let mut task_ids = Vec::new();
        for task in tasks {
            let tid = self.scheduler.enqueue(task).await?;
            task_ids.push(tid);
        }
        let ticket = JobTicket {
            id: job_id.clone(),
            tasks: task_ids,
            state: JobState::Pending,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        self.jobs.write().insert(job_id, ticket.clone());
        Ok(ticket)
    }

    async fn cancel_task(&self, task_id: &TaskId, reason: &str) -> Result<()> {
        self.cancellation.cancel_task(task_id, reason).await?;
        self.scheduler.remove(task_id).await?;
        self.emit_event(OrchestratorEvent::TaskCancelled {
            task_id: task_id.0.clone(),
            reason: reason.to_string(),
        });
        Ok(())
    }

    async fn cancel_job(&self, job_id: &JobId, reason: &str) -> Result<()> {
        let task_ids: Vec<TaskId> = {
            let jobs = self.jobs.read();
            jobs.get(job_id)
                .map(|t| t.tasks.clone())
                .unwrap_or_default()
        };
        for task_id in &task_ids {
            let _ = self.cancellation.cancel_task(task_id, reason).await;
            let _ = self.scheduler.remove(task_id).await;
        }
        Ok(())
    }

    async fn schedule_task(&self, task: OrchestratorTask, spec: ScheduleSpec) -> Result<TaskId> {
        let task_id = task.id.clone();
        match spec {
            ScheduleSpec::Immediate => {
                self.scheduler.enqueue(task).await?;
            }
            ScheduleSpec::Delayed { seconds } => {
                let scheduler = self.scheduler.clone();
                let cancellation = self.cancellation.clone();
                let tid = task_id.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(seconds)).await;
                    if !cancellation.is_cancelled(&tid).await {
                        let _ = scheduler.enqueue(task).await;
                    }
                });
            }
            ScheduleSpec::Cron { .. } | ScheduleSpec::Interval { .. } => {
                return Err(OrchestratorError::SchedulingError(
                    "cron/interval not implemented in DefaultSystemCoordinator".to_string(),
                ));
            }
        }
        Ok(task_id)
    }

    async fn reschedule_task(&self, _task_id: &TaskId, _spec: ScheduleSpec) -> Result<()> {
        Err(OrchestratorError::SchedulingError(
            "reschedule not implemented".to_string(),
        ))
    }

    async fn set_task_priority(&self, task_id: &TaskId, priority: TaskPriority) -> Result<()> {
        self.scheduler.reorder(task_id, priority).await
    }

    async fn get_queued_tasks(
        &self,
        min_priority: Option<TaskPriority>,
    ) -> Result<Vec<OrchestratorTask>> {
        self.scheduler.get_queue(min_priority).await
    }

    async fn activate_guardian_override(&self, reason: &str) -> Result<()> {
        if !self.config.enable_guardian_override {
            return Err(OrchestratorError::GuardianOverride(
                "guardian override disabled".to_string(),
            ));
        }
        self.guardian_override.store(true, Ordering::SeqCst);
        self.overrides.write().push(reason.to_string());
        let affected = self.scheduler.queue_length().await;
        self.emit_event(OrchestratorEvent::GuardianOverrideActivated {
            reason: reason.to_string(),
            affected_tasks: affected,
        });
        Ok(())
    }

    async fn deactivate_guardian_override(&self, reason: &str) -> Result<()> {
        self.guardian_override.store(false, Ordering::SeqCst);
        self.emit_event(OrchestratorEvent::GuardianOverrideDeactivated {
            reason: reason.to_string(),
        });
        Ok(())
    }

    async fn is_guardian_override_active(&self) -> bool {
        self.guardian_override.load(Ordering::SeqCst)
    }

    async fn get_active_overrides(&self) -> Result<Vec<String>> {
        Ok(self.overrides.read().clone())
    }

    async fn pause_system(&self, reason: &str) -> Result<()> {
        self.paused.store(true, Ordering::SeqCst);
        self.emit_event(OrchestratorEvent::SystemPaused {
            reason: reason.to_string(),
        });
        Ok(())
    }

    async fn resume_system(&self, reason: &str) -> Result<()> {
        self.paused.store(false, Ordering::SeqCst);
        self.emit_event(OrchestratorEvent::SystemResumed {
            reason: reason.to_string(),
        });
        Ok(())
    }

    async fn is_system_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    async fn emergency_override(&self, source: &str, action: &str) -> Result<()> {
        if !self.config.enable_emergency_override {
            return Err(OrchestratorError::GuardianOverride(
                "emergency override disabled".to_string(),
            ));
        }
        self.emit_event(OrchestratorEvent::EmergencyOverride {
            source: source.to_string(),
            action: action.to_string(),
        });
        if action == "shutdown" || action == "pause" {
            self.paused.store(true, Ordering::SeqCst);
            self.scheduler
                .clear()
                .await
                .map_err(|e| OrchestratorError::SchedulingError(e.to_string()))?;
            self.cancellation
                .cancel_all(&format!("emergency: {}", action))
                .await?;
        }
        Ok(())
    }

    async fn system_snapshot(&self) -> Result<SystemSnapshot> {
        let mut statuses = HashMap::new();
        statuses.insert(SystemComponent::Orchestrator, ComponentStatus::Healthy);
        if self.paused.load(Ordering::SeqCst) {
            statuses.insert(SystemComponent::Orchestrator, ComponentStatus::Paused);
        }

        let (completed, failed) = {
            let history = self.task_history.read();
            let c = history
                .iter()
                .filter(|t| t.state == TaskState::Completed)
                .count();
            let f = history
                .iter()
                .filter(|t| matches!(t.state, TaskState::Failed(_)))
                .count();
            (c, f)
        };

        Ok(SystemSnapshot {
            component_statuses: statuses,
            active_tasks: self.scheduler.queue_length().await,
            queued_tasks: self.scheduler.queue_length().await,
            completed_tasks: completed,
            failed_tasks: failed,
            is_guardian_override_active: self.guardian_override.load(Ordering::SeqCst),
            is_system_paused: self.paused.load(Ordering::SeqCst),
            uptime_seconds: (Utc::now() - self.started_at).num_seconds() as u64,
            timestamp: Utc::now(),
        })
    }

    async fn component_status(&self, _component: &SystemComponent) -> Result<ComponentStatus> {
        if self.paused.load(Ordering::SeqCst) {
            return Ok(ComponentStatus::Paused);
        }
        Ok(ComponentStatus::Healthy)
    }

    async fn get_active_tasks(&self) -> Result<Vec<OrchestratorTask>> {
        self.scheduler.get_queue(None).await
    }

    async fn get_task_history(&self, limit: usize) -> Result<Vec<OrchestratorTask>> {
        let history = self.task_history.read();
        let len = history.len();
        let start = len.saturating_sub(limit);
        Ok(history.iter().skip(start).cloned().collect())
    }

    async fn on_event(&self, handler: Box<dyn Fn(OrchestratorEvent) + Send + Sync>) -> Result<()> {
        self.event_handlers.write().push(handler);
        Ok(())
    }

    async fn submit_background_task(&self, task: OrchestratorTask) -> Result<TaskId> {
        if !self.config.enable_background_tasks {
            return Err(OrchestratorError::SchedulingError(
                "background tasks disabled".to_string(),
            ));
        }
        let task_id = task.id.clone();
        self.background_tasks.write().push(task);
        self.emit_event(OrchestratorEvent::BackgroundTaskStarted {
            task_id: task_id.0.clone(),
        });
        Ok(task_id)
    }

    async fn list_background_tasks(&self) -> Result<Vec<OrchestratorTask>> {
        Ok(self.background_tasks.read().clone())
    }
}
