use std::collections::HashMap;
use std::fmt;

use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JobId(pub String);

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SystemComponent {
    Conversation,
    Memory,
    Learning,
    WorldModel,
    Guardian,
    Automation,
    Vision,
    Providers,
    Plugins,
    Home,
    Hardware,
    Personality,
    Executor,
    Planner,
    Reflection,
    Cognition,
    Orchestrator,
}

impl fmt::Display for SystemComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversation => write!(f, "Conversation"),
            Self::Memory => write!(f, "Memory"),
            Self::Learning => write!(f, "Learning"),
            Self::WorldModel => write!(f, "WorldModel"),
            Self::Guardian => write!(f, "Guardian"),
            Self::Automation => write!(f, "Automation"),
            Self::Vision => write!(f, "Vision"),
            Self::Providers => write!(f, "Providers"),
            Self::Plugins => write!(f, "Plugins"),
            Self::Home => write!(f, "Home"),
            Self::Hardware => write!(f, "Hardware"),
            Self::Personality => write!(f, "Personality"),
            Self::Executor => write!(f, "Executor"),
            Self::Planner => write!(f, "Planner"),
            Self::Reflection => write!(f, "Reflection"),
            Self::Cognition => write!(f, "Cognition"),
            Self::Orchestrator => write!(f, "Orchestrator"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
    Starting,
    Stopping,
    Paused,
}

impl ComponentStatus {
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy)
    }

    pub fn is_degraded(&self) -> bool {
        matches!(self, Self::Degraded(_))
    }

    pub fn is_unhealthy(&self) -> bool {
        matches!(self, Self::Unhealthy(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    Background = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Critical = 4,
}

impl TaskPriority {
    pub fn value(&self) -> u8 {
        match self {
            Self::Background => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Critical => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskState {
    Pending,
    Queued,
    Running,
    Paused,
    Completed,
    Failed(String),
    Cancelled,
    Interrupted,
}

impl fmt::Display for TaskState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Queued => write!(f, "Queued"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed(e) => write!(f, "Failed({})", e),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Interrupted => write!(f, "Interrupted"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrchestratorTask {
    pub id: TaskId,
    pub job_id: Option<JobId>,
    pub name: String,
    pub description: String,
    pub task_type: String,
    pub priority: TaskPriority,
    pub state: TaskState,
    pub component: SystemComponent,
    pub dependencies: Vec<TaskId>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub timeout_seconds: u64,
    pub retry_count: u32,
    pub max_retries: u32,
    pub metadata: HashMap<String, String>,
    pub cancellation_token: Option<String>,
    pub context: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub enum ScheduleSpec {
    Immediate,
    Delayed { seconds: u64 },
    Cron { expression: String },
    Interval { seconds: u64 },
}

#[derive(Debug, Clone)]
pub struct JobTicket {
    pub id: JobId,
    pub tasks: Vec<TaskId>,
    pub state: JobState,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum JobState {
    Pending,
    Running,
    Completed { success: bool },
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub component_statuses: HashMap<SystemComponent, ComponentStatus>,
    pub active_tasks: usize,
    pub queued_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub is_guardian_override_active: bool,
    pub is_system_paused: bool,
    pub uptime_seconds: u64,
    pub timestamp: DateTime<Utc>,
}
