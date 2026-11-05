use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum HarnessError {
    #[error("Security violation: {0}")]
    Security(String),

    #[error("Path violation: {0}")]
    PathViolation(String),

    #[error("File edit error: {0}")]
    EditError(String),

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Execution timeout: {0}")]
    Timeout(String),

    #[error("Command execution error: {0}")]
    CommandError(String),

    #[error("Planning error: {0}")]
    PlanningError(String),

    #[error("Routing error: {0}")]
    RoutingError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, HarnessError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditInput {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditResult {
    pub file_path: String,
    pub applied: bool,
    pub occurrences: usize,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadInput {
    pub file_path: String,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResult {
    pub file_path: String,
    pub content: String,
    pub total_lines: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteInput {
    pub file_path: String,
    pub content: String,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteResult {
    pub file_path: String,
    pub bytes_written: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub active_form: Option<String>,
    pub dependencies: Vec<String>,
    pub affected_files: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub plan_id: Uuid,
    pub title: String,
    pub description: String,
    pub steps: Vec<TaskStep>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecutionInput {
    pub command: String,
    pub cwd: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub description: Option<String>,
    #[serde(default)]
    pub run_in_background: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessExecutionInput {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub timeout_ms: Option<u64>,
    pub description: Option<String>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default = "default_true")]
    pub require_job_object: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodingEvent {
    PlanCreated(TaskPlan),
    StepStarted {
        step_id: String,
        active_form: String,
    },
    StepCompleted {
        step_id: String,
    },
    StepFailed {
        step_id: String,
        error: String,
    },
    FileModified {
        file_path: String,
        diff: String,
    },
    CommandExecuted {
        command: String,
        exit_code: i32,
        duration_ms: u64,
    },
    ReviewComment {
        severity: String,
        file_path: String,
        message: String,
    },
    ApprovalRequested {
        request_id: String,
        tool: String,
        reason: String,
    },
    ApprovalResolved {
        request_id: String,
        approved: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingSession {
    pub session_id: Uuid,
    pub workspace_root: PathBuf,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}
