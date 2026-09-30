use std::fmt;
use std::future::Future;

use async_trait::async_trait;
use uuid::Uuid;

use crate::config::RuntimeConfig;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum ScheduleSpec {
    Immediate,
    Delay {
        seconds: u64,
    },
    Cron {
        expression: String,
    },
    Interval {
        seconds: u64,
    },
    Periodic {
        initial_delay_seconds: u64,
        interval_seconds: u64,
        max_repetitions: Option<u64>,
    },
}

#[async_trait]
pub trait TaskHandle: Send + Sync {
    fn task_id(&self) -> &Uuid;
    fn name(&self) -> &str;
    fn status(&self) -> TaskStatus;
    async fn cancel(&self) -> Result<()>;
    async fn wait_for_completion(&self) -> Result<TaskStatus>;
    fn progress(&self) -> Option<f64>;
    fn created_at(&self) -> &chrono::DateTime<chrono::Utc>;
    fn priority(&self) -> TaskPriority;
}

#[async_trait]
pub trait TaskScheduler: Send + Sync {
    async fn submit(
        &self,
        task: Box<dyn ScheduledTask>,
        priority: TaskPriority,
    ) -> Result<Box<dyn TaskHandle>>;
    async fn schedule(
        &self,
        task: Box<dyn ScheduledTask>,
        spec: ScheduleSpec,
    ) -> Result<Box<dyn TaskHandle>>;
    async fn cancel(&self, task_id: &Uuid) -> Result<()>;
    async fn status(&self, task_id: &Uuid) -> Option<TaskStatus>;
    async fn list_tasks(&self) -> Result<Vec<Box<dyn TaskHandle>>>;
    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
    fn is_paused(&self) -> bool;
    async fn active_task_count(&self) -> usize;
    async fn queued_task_count(&self) -> usize;
}

#[async_trait]
pub trait ScheduledTask: Send + Sync {
    fn name(&self) -> &str;
    async fn execute(&self, handle: Box<dyn TaskHandle>) -> Result<()>;
    fn timeout_seconds(&self) -> Option<u64>;
}

#[async_trait]
pub trait WorkerPool: Send + Sync {
    async fn execute<F, T>(&self, task: F) -> Result<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static;
    async fn execute_async<F, T>(&self, task: F) -> Result<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;
    fn active_workers(&self) -> usize;
    fn idle_workers(&self) -> usize;
    fn max_workers(&self) -> usize;
    async fn resize(&self, new_size: usize) -> Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Created,
    Initializing,
    Running,
    Paused,
    Stopping,
    Stopped,
    Failed,
}

impl fmt::Display for RuntimeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Created => write!(f, "Created"),
            Self::Initializing => write!(f, "Initializing"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Stopping => write!(f, "Stopping"),
            Self::Stopped => write!(f, "Stopped"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

#[async_trait]
pub trait RuntimeLifecycle: Send + Sync {
    async fn init(&self, config: &RuntimeConfig) -> Result<()>;
    async fn start(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
    async fn restart(&self) -> Result<()>;
    fn state(&self) -> RuntimeState;
    fn is_running(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_priority_variants() {
        assert!(TaskPriority::Low < TaskPriority::Normal);
        assert!(TaskPriority::Normal < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Critical);
    }

    #[test]
    fn test_task_status_variants() {
        let statuses = [
            TaskStatus::Pending,
            TaskStatus::Running,
            TaskStatus::Completed,
            TaskStatus::Failed("err".into()),
            TaskStatus::Cancelled,
        ];
        assert_eq!(statuses.len(), 5);
        match &statuses[3] {
            TaskStatus::Failed(msg) => assert_eq!(msg, "err"),
            _ => panic!("expected Failed"),
        }
    }

    #[test]
    fn test_schedule_spec_variants() {
        let specs = [
            ScheduleSpec::Immediate,
            ScheduleSpec::Delay { seconds: 10 },
            ScheduleSpec::Cron {
                expression: "* * * * *".into(),
            },
            ScheduleSpec::Interval { seconds: 5 },
            ScheduleSpec::Periodic {
                initial_delay_seconds: 0,
                interval_seconds: 60,
                max_repetitions: None,
            },
        ];
        assert_eq!(specs.len(), 5);
    }

    #[test]
    fn test_runtime_state_transitions() {
        let states = [
            RuntimeState::Created,
            RuntimeState::Initializing,
            RuntimeState::Running,
            RuntimeState::Paused,
            RuntimeState::Stopping,
            RuntimeState::Stopped,
            RuntimeState::Failed,
        ];
        assert_eq!(states.len(), 7);
    }

    #[test]
    fn test_runtime_state_display() {
        assert_eq!(format!("{}", RuntimeState::Created), "Created");
        assert_eq!(format!("{}", RuntimeState::Running), "Running");
        assert_eq!(format!("{}", RuntimeState::Stopped), "Stopped");
        assert_eq!(format!("{}", RuntimeState::Failed), "Failed");
    }
}
