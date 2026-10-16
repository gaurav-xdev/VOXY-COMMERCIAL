pub mod automation;
pub mod cancellation;
pub mod config;
pub mod coordinator;
pub mod error;
pub mod event;
pub mod execution;
pub mod interrupts;
pub mod pipeline;
pub mod scheduler;
pub mod types;

pub use automation::{
    AutomationBackend, AutomationCapability, ElementInfo, ElementSelector, MouseButton,
    StateVerification, WindowTarget,
};
pub use cancellation::{CancellationManager, DefaultCancellationManager};
pub use config::OrchestratorConfig;
pub use coordinator::{DefaultSystemCoordinator, SystemCoordinator};
pub use error::{OrchestratorError, Result};
pub use event::OrchestratorEvent;
pub use execution::{
    AuditEvent, CancellationFlag, CorrelationId, ExecutionContext, PipelineInput, PipelineOutput,
    PipelineStage, StageTimeline,
};
pub use interrupts::{
    DefaultInterruptionManager, InterruptionEvent, InterruptionManager, InterruptionSeverity,
};
pub use pipeline::{create_pipeline_stages, execute_pipeline, StageHandler};
pub use scheduler::{NoopTaskScheduler, PriorityTaskScheduler, TaskSchedulerInternal};
pub use types::{
    ComponentStatus, JobId, JobState, JobTicket, OrchestratorTask, ScheduleSpec, SystemComponent,
    SystemSnapshot, TaskId, TaskPriority, TaskState,
};

pub mod prelude {
    pub use crate::automation::AutomationBackend;
    pub use crate::cancellation::{CancellationManager, DefaultCancellationManager};
    pub use crate::coordinator::{DefaultSystemCoordinator, SystemCoordinator};
    pub use crate::error::Result;
    pub use crate::execution::{
        AuditEvent, CancellationFlag, CorrelationId, ExecutionContext, PipelineInput,
        PipelineOutput, PipelineStage, StageTimeline,
    };
    pub use crate::interrupts::{InterruptionManager, InterruptionSeverity};
    pub use crate::pipeline::{create_pipeline_stages, execute_pipeline, StageHandler};
    pub use crate::scheduler::{PriorityTaskScheduler, TaskSchedulerInternal};
    pub use crate::types::{
        ComponentStatus, JobId, JobState, JobTicket, OrchestratorTask, ScheduleSpec,
        SystemComponent, SystemSnapshot, TaskId, TaskPriority, TaskState,
    };
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::Utc;

    use super::*;

    // ---- Existing tests (preserved) ----

    #[test]
    fn test_task_id_creation_and_display() {
        let id = TaskId("task-1".into());
        assert_eq!(id.0, "task-1");
        assert_eq!(format!("{}", id), "task-1");
    }

    #[test]
    fn test_job_id_creation() {
        let id = JobId("job-1".into());
        assert_eq!(id.0, "job-1");
        assert_eq!(format!("{}", id), "job-1");
    }

    #[test]
    fn test_system_component_variants() {
        let variants = vec![
            (SystemComponent::Conversation, "Conversation"),
            (SystemComponent::Memory, "Memory"),
            (SystemComponent::Learning, "Learning"),
            (SystemComponent::WorldModel, "WorldModel"),
            (SystemComponent::Guardian, "Guardian"),
            (SystemComponent::Automation, "Automation"),
            (SystemComponent::Vision, "Vision"),
            (SystemComponent::Providers, "Providers"),
            (SystemComponent::Plugins, "Plugins"),
            (SystemComponent::Home, "Home"),
            (SystemComponent::Hardware, "Hardware"),
            (SystemComponent::Personality, "Personality"),
            (SystemComponent::Executor, "Executor"),
            (SystemComponent::Planner, "Planner"),
            (SystemComponent::Reflection, "Reflection"),
            (SystemComponent::Cognition, "Cognition"),
            (SystemComponent::Orchestrator, "Orchestrator"),
        ];
        for (comp, name) in variants {
            assert_eq!(format!("{}", comp), name);
        }
    }

    #[test]
    fn test_component_status_healthy() {
        let status = ComponentStatus::Healthy;
        assert!(status.is_healthy());
        assert!(!status.is_degraded());
        assert!(!status.is_unhealthy());
    }

    #[test]
    fn test_component_status_degraded() {
        let status = ComponentStatus::Degraded("high latency".into());
        assert!(!status.is_healthy());
        assert!(status.is_degraded());
        assert!(!status.is_unhealthy());
    }

    #[test]
    fn test_component_status_unhealthy() {
        let status = ComponentStatus::Unhealthy("crashed".into());
        assert!(!status.is_healthy());
        assert!(!status.is_degraded());
        assert!(status.is_unhealthy());
    }

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Background < TaskPriority::Low);
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Critical);
        assert!(TaskPriority::Critical > TaskPriority::Normal);
    }

    #[test]
    fn test_task_priority_value() {
        assert_eq!(TaskPriority::Background.value(), 0);
        assert_eq!(TaskPriority::Low.value(), 1);
        assert_eq!(TaskPriority::Normal.value(), 2);
        assert_eq!(TaskPriority::High.value(), 3);
        assert_eq!(TaskPriority::Critical.value(), 4);
    }

    #[test]
    fn test_task_state_pending() {
        let state = TaskState::Pending;
        assert_eq!(format!("{}", state), "Pending");
    }

    #[test]
    fn test_task_state_queued() {
        let state = TaskState::Queued;
        assert_eq!(format!("{}", state), "Queued");
    }

    #[test]
    fn test_task_state_running() {
        let state = TaskState::Running;
        assert_eq!(format!("{}", state), "Running");
    }

    #[test]
    fn test_task_state_paused() {
        let state = TaskState::Paused;
        assert_eq!(format!("{}", state), "Paused");
    }

    #[test]
    fn test_task_state_completed() {
        let state = TaskState::Completed;
        assert_eq!(format!("{}", state), "Completed");
    }

    #[test]
    fn test_task_state_failed() {
        let state = TaskState::Failed("timeout".into());
        assert_eq!(format!("{}", state), "Failed(timeout)");
    }

    #[test]
    fn test_task_state_cancelled() {
        let state = TaskState::Cancelled;
        assert_eq!(format!("{}", state), "Cancelled");
    }

    #[test]
    fn test_task_state_interrupted() {
        let state = TaskState::Interrupted;
        assert_eq!(format!("{}", state), "Interrupted");
    }

    #[test]
    fn test_orchestrator_task_creation() {
        let task = OrchestratorTask {
            id: TaskId("t1".into()),
            job_id: Some(JobId("j1".into())),
            name: "test-task".into(),
            description: "A test task".into(),
            task_type: "general".into(),
            priority: TaskPriority::High,
            state: TaskState::Pending,
            component: SystemComponent::Executor,
            dependencies: vec![TaskId("dep1".into())],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            timeout_seconds: 60,
            retry_count: 0,
            max_retries: 3,
            metadata: {
                let mut m = HashMap::new();
                m.insert("key".into(), "value".into());
                m
            },
            cancellation_token: None,
            context: HashMap::new(),
        };
        assert_eq!(task.id.0, "t1");
        assert_eq!(task.name, "test-task");
        assert_eq!(task.priority, TaskPriority::High);
        assert!(task.started_at.is_none());
    }

    #[test]
    fn test_schedule_spec_immediate() {
        let spec = ScheduleSpec::Immediate;
        assert!(matches!(spec, ScheduleSpec::Immediate));
    }

    #[test]
    fn test_schedule_spec_delayed() {
        let spec = ScheduleSpec::Delayed { seconds: 30 };
        assert!(matches!(spec, ScheduleSpec::Delayed { seconds: 30 }));
    }

    #[test]
    fn test_schedule_spec_cron() {
        let spec = ScheduleSpec::Cron {
            expression: "0 0 * * * *".into(),
        };
        assert!(matches!(spec, ScheduleSpec::Cron { .. }));
    }

    #[test]
    fn test_schedule_spec_interval() {
        let spec = ScheduleSpec::Interval { seconds: 60 };
        assert!(matches!(spec, ScheduleSpec::Interval { seconds: 60 }));
    }

    #[test]
    fn test_job_ticket_creation() {
        let ticket = JobTicket {
            id: JobId("jt1".into()),
            tasks: vec![TaskId("t1".into()), TaskId("t2".into())],
            state: JobState::Pending,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        };
        assert_eq!(ticket.id.0, "jt1");
        assert_eq!(ticket.tasks.len(), 2);
        assert!(matches!(ticket.state, JobState::Pending));
    }

    #[test]
    fn test_job_state_pending() {
        assert!(matches!(JobState::Pending, JobState::Pending));
    }

    #[test]
    fn test_job_state_running() {
        assert!(matches!(JobState::Running, JobState::Running));
    }

    #[test]
    fn test_job_state_completed() {
        assert!(matches!(
            JobState::Completed { success: true },
            JobState::Completed { .. }
        ));
    }

    #[test]
    fn test_job_state_failed() {
        assert!(matches!(
            JobState::Failed("err".into()),
            JobState::Failed(_)
        ));
    }

    #[test]
    fn test_job_state_cancelled() {
        assert!(matches!(JobState::Cancelled, JobState::Cancelled));
    }

    #[test]
    fn test_system_snapshot_creation() {
        let snapshot = SystemSnapshot {
            component_statuses: {
                let mut m = HashMap::new();
                m.insert(SystemComponent::Conversation, ComponentStatus::Healthy);
                m
            },
            active_tasks: 5,
            queued_tasks: 10,
            completed_tasks: 100,
            failed_tasks: 2,
            is_guardian_override_active: false,
            is_system_paused: false,
            uptime_seconds: 3600,
            timestamp: Utc::now(),
        };
        assert_eq!(snapshot.active_tasks, 5);
        assert_eq!(snapshot.queued_tasks, 10);
        assert!(!snapshot.is_guardian_override_active);
    }

    #[test]
    fn test_mouse_button_variants() {
        assert_eq!(MouseButton::Left, MouseButton::Left);
        assert_eq!(MouseButton::Right, MouseButton::Right);
        assert_eq!(MouseButton::Middle, MouseButton::Middle);
        assert_eq!(MouseButton::X1, MouseButton::X1);
        assert_eq!(MouseButton::X2, MouseButton::X2);
        assert_ne!(MouseButton::Left, MouseButton::Right);
    }

    #[test]
    fn test_window_target_creation() {
        let target = WindowTarget {
            id: "win1".into(),
            title: "Test Window".into(),
            class_name: Some("WindowClass".into()),
            process_id: Some(1234),
            bounds: voxy_shared::types::Rect::new(0, 0, 800, 600),
            is_visible: true,
            is_focused: false,
        };
        assert_eq!(target.title, "Test Window");
        assert!(target.is_visible);
        assert!(!target.is_focused);
    }

    #[test]
    fn test_element_selector_creation() {
        let selector = ElementSelector {
            automation_id: Some("btn1".into()),
            name: Some("Submit".into()),
            class_name: None,
            control_type: Some("Button".into()),
            text: None,
            index: Some(0),
        };
        assert_eq!(selector.automation_id, Some("btn1".into()));
        assert_eq!(selector.name, Some("Submit".into()));
        assert_eq!(selector.index, Some(0));

        let empty = ElementSelector {
            automation_id: None,
            name: None,
            class_name: None,
            control_type: None,
            text: None,
            index: None,
        };
        assert!(empty.automation_id.is_none());
    }

    #[test]
    fn test_element_info_creation() {
        let info = ElementInfo {
            id: "el1".into(),
            name: "OK Button".into(),
            control_type: "Button".into(),
            bounds: voxy_shared::types::Rect::new(10, 10, 100, 30),
            is_enabled: true,
            is_visible: true,
            text: Some("OK".into()),
            children: vec![],
        };
        assert_eq!(info.name, "OK Button");
        assert!(info.is_enabled);
        assert_eq!(info.text, Some("OK".into()));
    }

    #[test]
    fn test_state_verification_creation() {
        let verification = StateVerification {
            window_title: Some("Main".into()),
            element_present: None,
            text_visible: Some("Hello".into()),
            timeout_ms: 5000,
        };
        assert_eq!(verification.window_title, Some("Main".into()));
        assert_eq!(verification.text_visible, Some("Hello".into()));
    }

    #[test]
    fn test_automation_capability_variants() {
        let all = vec![
            AutomationCapability::Mouse,
            AutomationCapability::Keyboard,
            AutomationCapability::ScreenCapture,
            AutomationCapability::WindowManagement,
            AutomationCapability::ElementDetection,
            AutomationCapability::Ocr,
            AutomationCapability::StateVerification,
            AutomationCapability::Recovery,
            AutomationCapability::Hybrid,
        ];
        assert_eq!(all.len(), 9);
        assert!(all.contains(&AutomationCapability::Mouse));
        assert!(all.contains(&AutomationCapability::Hybrid));
    }

    #[test]
    fn test_interruption_event_creation() {
        let event = InterruptionEvent {
            id: "int1".into(),
            source: "voice".into(),
            task_id: Some(TaskId("t1".into())),
            reason: "user interrupt".into(),
            severity: InterruptionSeverity::Warning,
            timestamp: Utc::now(),
            metadata: {
                let mut m = HashMap::new();
                m.insert("key".into(), "val".into());
                m
            },
        };
        assert_eq!(event.id, "int1");
        assert_eq!(event.severity, InterruptionSeverity::Warning);
        assert_eq!(event.reason, "user interrupt");
    }

    #[test]
    fn test_interruption_severity_ordering() {
        assert!(InterruptionSeverity::Info < InterruptionSeverity::Warning);
        assert!(InterruptionSeverity::Warning < InterruptionSeverity::Critical);
        assert!(InterruptionSeverity::Critical < InterruptionSeverity::Emergency);
        assert!(InterruptionSeverity::Emergency > InterruptionSeverity::Info);
    }

    #[test]
    fn test_trait_object_safe_automation_backend() {
        fn _check(_v: &dyn AutomationBackend) {}
    }

    #[test]
    fn test_trait_object_safe_system_coordinator() {
        fn _check(_v: &dyn SystemCoordinator) {}
    }

    #[test]
    fn test_trait_object_safe_interruption_manager() {
        fn _check(_v: &dyn InterruptionManager) {}
    }

    #[test]
    fn test_trait_object_safe_cancellation_manager() {
        fn _check(_v: &dyn CancellationManager) {}
    }

    #[test]
    fn test_trait_object_safe_task_scheduler() {
        fn _check(_v: &dyn TaskSchedulerInternal) {}
    }

    #[test]
    fn test_orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        assert_eq!(config.max_concurrent_tasks, 50);
        assert_eq!(config.max_priority_levels, 5);
        assert!(config.enable_guardian_override);
        assert!(config.enable_emergency_override);
        assert!(config.enable_background_tasks);
        assert_eq!(config.task_timeout_seconds, 300);
        assert_eq!(config.scheduler_tick_ms, 100);
        assert_eq!(config.max_retries_per_task, 3);
        assert_eq!(config.interruption_cooldown_ms, 500);
        assert_eq!(config.health_check_interval_seconds, 30);
        assert_eq!(config.pipeline_timeout_seconds, 600);
        assert!(config.enable_audit_trail);
        assert!(config.enable_correlation_tracking);
    }

    #[test]
    fn test_orchestrator_event_display_task_scheduled() {
        let event = OrchestratorEvent::TaskScheduled {
            task_id: "t1".into(),
            task_type: "inference".into(),
            priority: 3,
        };
        let s = format!("{}", event);
        assert!(s.contains("Task scheduled"));
        assert!(s.contains("t1"));
        assert!(s.contains("inference"));
    }

    #[test]
    fn test_orchestrator_event_display_task_completed() {
        let event = OrchestratorEvent::TaskCompleted {
            task_id: "t1".into(),
            success: true,
            duration_ms: 1500,
        };
        let s = format!("{}", event);
        assert!(s.contains("Task completed"));
        assert!(s.contains("true"));
        assert!(s.contains("1500ms"));
    }

    #[test]
    fn test_orchestrator_event_display_guardian_override() {
        let event = OrchestratorEvent::GuardianOverrideActivated {
            reason: "safety check".into(),
            affected_tasks: 3,
        };
        let s = format!("{}", event);
        assert!(s.contains("Guardian override activated"));
        assert!(s.contains("safety check"));
        assert!(s.contains("3"));
    }



    #[test]
    fn test_orchestrator_event_display_emergency_override() {
        let event = OrchestratorEvent::EmergencyOverride {
            source: "guardian".into(),
            action: "shutdown".into(),
        };
        let s = format!("{}", event);
        assert!(s.contains("Emergency override"));
        assert!(s.contains("guardian"));
        assert!(s.contains("shutdown"));
    }

    #[test]
    fn test_orchestrator_event_display_system_paused() {
        let event = OrchestratorEvent::SystemPaused {
            reason: "maintenance".into(),
        };
        assert_eq!(format!("{}", event), "System paused: maintenance");
    }

    #[test]
    fn test_orchestrator_event_display_system_shutting_down() {
        let event = OrchestratorEvent::SystemShuttingDown {
            reason: "upgrade".into(),
        };
        assert_eq!(format!("{}", event), "System shutting down: upgrade");
    }

    #[test]
    fn test_orchestrator_error_display() {
        let err = OrchestratorError::InvalidConfig("missing field".into());
        assert_eq!(format!("{}", err), "Invalid config: missing field");

        let err = OrchestratorError::TaskError("execution failed".into());
        assert_eq!(format!("{}", err), "Task error: execution failed");

        let err = OrchestratorError::Timeout("operation timed out".into());
        assert_eq!(format!("{}", err), "Timeout: operation timed out");
    }

    #[test]
    fn test_orchestrator_error_scheduling() {
        let err = OrchestratorError::SchedulingError("queue full".into());
        assert_eq!(format!("{}", err), "Scheduling error: queue full");
    }

    #[test]
    fn test_orchestrator_error_shutdown() {
        let err = OrchestratorError::ShutdownError("in progress".into());
        assert_eq!(format!("{}", err), "Shutdown error: in progress");
    }

    #[test]
    fn test_task_id_hash_and_eq() {
        let a = TaskId("test".into());
        let b = TaskId("test".into());
        let c = TaskId("other".into());
        assert_eq!(a, b);
        assert_ne!(a, c);

        let mut map = HashMap::new();
        map.insert(a.clone(), 1);
        assert_eq!(map.get(&b), Some(&1));
        assert_eq!(map.get(&c), None);
    }

    #[test]
    fn test_job_id_hash_and_eq() {
        let a = JobId("j1".into());
        let b = JobId("j1".into());
        assert_eq!(a, b);
    }

    #[test]
    fn test_system_component_debug() {
        let comp = SystemComponent::Vision;
        let s = format!("{:?}", comp);
        assert_eq!(s, "Vision");
    }

    #[test]
    fn test_component_status_starting() {
        let status = ComponentStatus::Starting;
        assert!(!status.is_healthy());
        assert!(!status.is_degraded());
        assert!(!status.is_unhealthy());
    }

    #[test]
    fn test_component_status_stopping() {
        let status = ComponentStatus::Stopping;
        assert!(!status.is_healthy());
    }

    #[test]
    fn test_orchestrator_task_default_fields() {
        let task = OrchestratorTask {
            id: TaskId("t2".into()),
            job_id: None,
            name: String::new(),
            description: String::new(),
            task_type: "test".into(),
            priority: TaskPriority::Normal,
            state: TaskState::Pending,
            component: SystemComponent::Orchestrator,
            dependencies: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            timeout_seconds: 300,
            retry_count: 0,
            max_retries: 0,
            metadata: HashMap::new(),
            cancellation_token: None,
            context: HashMap::new(),
        };
        assert_eq!(task.task_type, "test");
        assert_eq!(task.max_retries, 0);
        assert!(task.job_id.is_none());
    }

    #[test]
    fn test_orchestrator_error_pipeline() {
        let err = OrchestratorError::PipelineError("stage failed".into());
        assert_eq!(format!("{}", err), "Pipeline error: stage failed");
    }

    #[test]
    fn test_orchestrator_error_stage() {
        let err = OrchestratorError::StageError("timeout".into());
        assert_eq!(format!("{}", err), "Stage error: timeout");
    }

    // ---- New integration tests for concrete implementations ----

    #[test]
    fn test_execution_context_creation() {
        let ctx = ExecutionContext::new();
        assert_eq!(ctx.timeline.len(), 0);
        assert!(ctx.session_id.is_none());
        assert!(ctx.user_input.is_none());
    }

    #[test]
    fn test_execution_context_with_session() {
        let ctx = ExecutionContext::new().with_session_id("test-session");
        assert_eq!(ctx.session_id, Some("test-session".to_string()));
    }

    #[test]
    fn test_execution_context_stage_tracking() {
        let mut ctx = ExecutionContext::new();
        ctx.start_stage(PipelineStage::Cognition);
        assert_eq!(ctx.timeline.len(), 1);
        assert_eq!(ctx.timeline[0].stage, PipelineStage::Cognition);
        assert!(ctx.timeline[0].completed_at.is_none());

        ctx.complete_stage(PipelineStage::Cognition, true, None);
        assert!(ctx.timeline[0].completed_at.is_some());
        assert!(ctx.timeline[0].success);
        assert!(ctx.timeline[0].duration_ms.is_some());
        assert!(ctx.is_stage_completed(PipelineStage::Cognition));
    }

    #[test]
    fn test_execution_context_audit_events() {
        let mut ctx = ExecutionContext::new();
        ctx.add_audit_event(PipelineStage::Cognition, "test", "test message");
        assert_eq!(ctx.audit_events.len(), 1);
        assert_eq!(ctx.audit_events[0].event_type, "test");
    }

    #[test]
    fn test_cancellation_flag() {
        let flag = CancellationFlag::new();
        assert!(!flag.is_cancelled());
        assert!(flag.check().is_ok());

        flag.cancel("test reason");
        assert!(flag.is_cancelled());
        assert!(flag.check().is_err());
        assert_eq!(flag.reason(), Some("test reason".to_string()));
    }

    #[test]
    fn test_pipeline_stage_name() {
        assert_eq!(PipelineStage::Conversation.name(), "Conversation");
        assert_eq!(PipelineStage::Cognition.name(), "Cognition");
        assert_eq!(PipelineStage::Execution.name(), "Execution");
        assert_eq!(PipelineStage::MemoryStorage.name(), "MemoryStorage");
        assert_eq!(PipelineStage::Learning.name(), "Learning");
    }

    #[test]
    fn test_priority_scheduler_enqueue_dequeue() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = PriorityTaskScheduler::new();
            let task = make_test_task(TaskPriority::High);

            let task_id = scheduler.enqueue(task.clone()).await.unwrap();
            assert_eq!(task_id.0, "test-task");
            assert_eq!(scheduler.queue_length().await, 1);
            assert!(!scheduler.is_empty().await);

            let dequeued = scheduler.dequeue(None).await.unwrap();
            assert!(dequeued.is_some());
            assert_eq!(dequeued.unwrap().id.0, "test-task");
            assert!(scheduler.is_empty().await);
        });
    }

    #[test]
    fn test_priority_scheduler_priority_ordering() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = PriorityTaskScheduler::new();

            let low = make_test_task_with_id(TaskPriority::Low, "low");
            let high = make_test_task_with_id(TaskPriority::High, "high");
            let crit = make_test_task_with_id(TaskPriority::Critical, "critical");

            scheduler.enqueue(low).await.unwrap();
            scheduler.enqueue(high).await.unwrap();
            scheduler.enqueue(crit).await.unwrap();
            assert_eq!(scheduler.queue_length().await, 3);

            let first = scheduler.dequeue(None).await.unwrap().unwrap();
            assert_eq!(first.id.0, "critical");

            let second = scheduler.dequeue(None).await.unwrap().unwrap();
            assert_eq!(second.id.0, "high");

            let third = scheduler.dequeue(None).await.unwrap().unwrap();
            assert_eq!(third.id.0, "low");
        });
    }

    #[test]
    fn test_priority_scheduler_remove_and_reorder() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = PriorityTaskScheduler::new();
            let task = make_test_task(TaskPriority::Normal);
            let tid = task.id.clone();

            scheduler.enqueue(task).await.unwrap();
            assert_eq!(scheduler.queue_length().await, 1);

            let result = scheduler.reorder(&tid, TaskPriority::Critical).await;
            assert!(result.is_ok());

            scheduler.remove(&tid).await.unwrap();
            assert!(scheduler.is_empty().await);
        });
    }

    #[test]
    fn test_priority_scheduler_clear() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = PriorityTaskScheduler::new();
            scheduler
                .enqueue(make_test_task(TaskPriority::Low))
                .await
                .unwrap();
            scheduler
                .enqueue(make_test_task_with_id(TaskPriority::High, "h1"))
                .await
                .unwrap();
            assert_eq!(scheduler.queue_length().await, 2);

            scheduler.clear().await.unwrap();
            assert!(scheduler.is_empty().await);
            assert_eq!(scheduler.queue_length().await, 0);
        });
    }

    #[test]
    fn test_priority_scheduler_dequeue_with_max_priority() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = PriorityTaskScheduler::new();
            scheduler
                .enqueue(make_test_task_with_id(TaskPriority::Critical, "c1"))
                .await
                .unwrap();
            scheduler
                .enqueue(make_test_task_with_id(TaskPriority::Normal, "n1"))
                .await
                .unwrap();

            let dq = scheduler
                .dequeue(Some(TaskPriority::Normal))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(dq.id.0, "n1");
        });
    }

    #[test]
    fn test_priority_scheduler_get_queue() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = PriorityTaskScheduler::new();
            scheduler
                .enqueue(make_test_task_with_id(TaskPriority::Low, "l1"))
                .await
                .unwrap();
            scheduler
                .enqueue(make_test_task_with_id(TaskPriority::High, "h1"))
                .await
                .unwrap();

            let queue = scheduler.get_queue(Some(TaskPriority::High)).await.unwrap();
            assert_eq!(queue.len(), 1);
            assert_eq!(queue[0].id.0, "h1");
        });
    }

    #[test]
    fn test_priority_scheduler_peek() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = PriorityTaskScheduler::new();
            assert!(scheduler.peek().await.unwrap().is_none());

            scheduler
                .enqueue(make_test_task_with_id(TaskPriority::Critical, "c1"))
                .await
                .unwrap();
            let peeked = scheduler.peek().await.unwrap();
            assert!(peeked.is_some());
            assert_eq!(peeked.unwrap().id.0, "c1");
            assert_eq!(scheduler.queue_length().await, 1);
        });
    }

    #[test]
    fn test_default_cancellation_manager() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mgr = DefaultCancellationManager::new();
            let task_id = TaskId("test-task".into());

            let token = mgr.register_cancellation_token(&task_id).await.unwrap();
            assert!(!token.is_empty());

            assert!(!mgr.is_cancelled(&task_id).await);
            mgr.cancel_task(&task_id, "test cancel").await.unwrap();
            assert!(mgr.is_cancelled(&task_id).await);

            let count = mgr.cancel_all("bulk").await.unwrap();
            assert!(count > 0);
        });
    }

    #[test]
    fn test_default_interruption_manager() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mgr = DefaultInterruptionManager::new();
            let task_id = TaskId("test-task".into());

            let event = InterruptionEvent {
                id: "int-1".into(),
                source: "voice".into(),
                task_id: Some(task_id.clone()),
                reason: "barge in".into(),
                severity: InterruptionSeverity::Warning,
                timestamp: Utc::now(),
                metadata: HashMap::new(),
            };

            mgr.emit_interruption(event).await.unwrap();
            assert!(mgr.is_interrupted(&task_id).await);

            let active = mgr.get_active_interruptions().await.unwrap();
            assert_eq!(active.len(), 1);

            mgr.clear_interruptions("voice").await.unwrap();
            let cleared = mgr.get_active_interruptions().await.unwrap();
            assert_eq!(cleared.len(), 0);
        });
    }

    #[test]
    fn test_interruption_handler_registration() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mgr = DefaultInterruptionManager::new();
            let handled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let handled_clone = handled.clone();

            mgr.register_handler(
                "test",
                Box::new(move |_event| {
                    handled_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }),
            )
            .await
            .unwrap();

            let event = InterruptionEvent {
                id: "int-2".into(),
                source: "test".into(),
                task_id: None,
                reason: "test".into(),
                severity: InterruptionSeverity::Info,
                timestamp: Utc::now(),
                metadata: HashMap::new(),
            };

            mgr.emit_interruption(event).await.unwrap();
            assert!(handled.load(std::sync::atomic::Ordering::SeqCst));
        });
    }

    #[test]
    fn test_default_system_coordinator_creation() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let config = OrchestratorConfig::default();
            let coordinator = DefaultSystemCoordinator::new(config);
            assert!(!coordinator.is_system_paused().await);
        });
    }

    #[test]
    fn test_default_system_coordinator_submit_task() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let task = make_test_task(TaskPriority::Normal);
            let task_id = coordinator.submit_task(task).await.unwrap();
            assert_eq!(task_id.0, "test-task");

            let queued = coordinator.get_queued_tasks(None).await.unwrap();
            assert_eq!(queued.len(), 1);
        });
    }

    #[test]
    fn test_default_system_coordinator_cancel_task() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let task = make_test_task(TaskPriority::Normal);
            let task_id = task.id.clone();
            coordinator.submit_task(task).await.unwrap();
            coordinator.cancel_task(&task_id, "test").await.unwrap();
            let queued = coordinator.get_queued_tasks(None).await.unwrap();
            assert_eq!(queued.len(), 0);
        });
    }

    #[test]
    fn test_default_system_coordinator_pause_resume() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            assert!(!coordinator.is_system_paused().await);

            coordinator.pause_system("test").await.unwrap();
            assert!(coordinator.is_system_paused().await);

            coordinator.resume_system("test").await.unwrap();
            assert!(!coordinator.is_system_paused().await);
        });
    }

    #[test]
    fn test_default_system_coordinator_submit_when_paused() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            coordinator.pause_system("test").await.unwrap();
            let task = make_test_task(TaskPriority::Normal);
            let result = coordinator.submit_task(task).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_default_system_coordinator_submit_job() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let tasks = vec![
                make_test_task(TaskPriority::Normal),
                make_test_task_with_id(TaskPriority::High, "task-2"),
            ];
            let ticket = coordinator.submit_job(tasks).await.unwrap();
            assert_eq!(ticket.tasks.len(), 2);
            assert!(matches!(ticket.state, JobState::Pending));
        });
    }

    #[test]
    fn test_default_system_coordinator_guardian_override() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());

            coordinator
                .activate_guardian_override("safety")
                .await
                .unwrap();
            assert!(coordinator.is_guardian_override_active().await);
            let overrides = coordinator.get_active_overrides().await.unwrap();
            assert!(!overrides.is_empty());

            coordinator
                .deactivate_guardian_override("resolved")
                .await
                .unwrap();
            assert!(!coordinator.is_guardian_override_active().await);
        });
    }

    #[test]
    fn test_default_system_coordinator_emergency_override() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let result = coordinator.emergency_override("guardian", "shutdown").await;
            assert!(result.is_ok());
            assert!(coordinator.is_system_paused().await);
        });
    }



    #[test]
    fn test_default_system_coordinator_snapshot() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let snapshot = coordinator.system_snapshot().await.unwrap();
            assert!(!snapshot.is_system_paused);
        });
    }

    #[test]
    fn test_default_system_coordinator_background_task() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let task = make_test_task(TaskPriority::Background);
            let task_id = coordinator.submit_background_task(task).await.unwrap();
            assert_eq!(task_id.0, "test-task");

            let bg_tasks = coordinator.list_background_tasks().await.unwrap();
            assert_eq!(bg_tasks.len(), 1);
        });
    }

    #[test]
    fn test_default_system_coordinator_init() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let result = coordinator.init(&OrchestratorConfig::default()).await;
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_default_system_coordinator_shutdown() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let result = coordinator.shutdown().await;
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_default_system_coordinator_event_handler() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            let event_received = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let event_clone = event_received.clone();

            coordinator
                .on_event(Box::new(move |_event| {
                    event_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }))
                .await
                .unwrap();

            coordinator.pause_system("test").await.unwrap();
            assert!(event_received.load(std::sync::atomic::Ordering::SeqCst));
        });
    }

    #[test]
    fn test_default_system_coordinator_task_history() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let coordinator = DefaultSystemCoordinator::new(OrchestratorConfig::default());
            coordinator
                .submit_task(make_test_task(TaskPriority::Normal))
                .await
                .unwrap();
            coordinator
                .submit_task(make_test_task_with_id(TaskPriority::Low, "task-2"))
                .await
                .unwrap();

            let history = coordinator.get_task_history(10).await.unwrap();
            assert_eq!(history.len(), 2);
        });
    }

    #[test]
    fn test_noop_task_scheduler() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let scheduler = NoopTaskScheduler;
            let task = make_test_task(TaskPriority::Normal);
            let id = scheduler.enqueue(task).await.unwrap();
            assert_eq!(id.0, "test-task");

            assert!(scheduler.is_empty().await);
            assert_eq!(scheduler.queue_length().await, 0);
            assert!(scheduler.peek().await.unwrap().is_none());
            assert!(scheduler.dequeue(None).await.unwrap().is_none());
            assert!(scheduler.remove(&id).await.is_ok());
            assert!(scheduler.clear().await.is_ok());
        });
    }

    #[test]
    fn test_pipeline_stages_creation() {
        let stages = create_pipeline_stages(Vec::new());
        assert!(stages.is_empty());
    }

    #[test]
    fn test_pipeline_output_creation() {
        let output = PipelineOutput {
            correlation_id: uuid::Uuid::new_v4(),
            success: true,
            response_text: Some("hello".into()),
            timeline: Vec::new(),
            audit_events: Vec::new(),
            error: None,
            total_duration_ms: 100,
        };
        assert!(output.success);
        assert_eq!(output.response_text, Some("hello".into()));
        assert!(output.error.is_none());
    }

    fn make_test_task(priority: TaskPriority) -> OrchestratorTask {
        OrchestratorTask {
            id: TaskId("test-task".into()),
            job_id: None,
            name: "test".into(),
            description: "test".into(),
            task_type: "test".into(),
            priority,
            state: TaskState::Pending,
            component: SystemComponent::Orchestrator,
            dependencies: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            timeout_seconds: 60,
            retry_count: 0,
            max_retries: 0,
            metadata: HashMap::new(),
            cancellation_token: None,
            context: HashMap::new(),
        }
    }

    fn make_test_task_with_id(priority: TaskPriority, id: &str) -> OrchestratorTask {
        OrchestratorTask {
            id: TaskId(id.into()),
            job_id: None,
            name: id.into(),
            description: "test".into(),
            task_type: "test".into(),
            priority,
            state: TaskState::Pending,
            component: SystemComponent::Orchestrator,
            dependencies: vec![],
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            timeout_seconds: 60,
            retry_count: 0,
            max_retries: 0,
            metadata: HashMap::new(),
            cancellation_token: None,
            context: HashMap::new(),
        }
    }
}
