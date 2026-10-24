use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canon Agent Roles defined by Trinity & SwarmClaw multi-agent topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    /// Strategic coordination, task graph generation, delegation, and final synthesis.
    Supervisor,
    /// High-level architecture, module contracts, interfaces, and technical specification.
    Architect,
    /// Concrete implementation, coding, atomic patching, and tool execution.
    SpecialistWorker,
    /// Automated code, security, and quality audit (bridges to Kodus AI).
    Reviewer,
    /// Validation, test suite execution, regression checks, and canary verification.
    QaTester,
}

impl AgentRole {
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentRole::Supervisor => "Trinity Supervisor",
            AgentRole::Architect => "Swarm Architect",
            AgentRole::SpecialistWorker => "Specialist Worker",
            AgentRole::Reviewer => "Kodus & Swarm Reviewer",
            AgentRole::QaTester => "QA & Canary Tester",
        }
    }

    pub fn default_system_prompt(&self) -> &'static str {
        match self {
            AgentRole::Supervisor => {
                "You are the Trinity Supervisor Agent. Your role is to understand the high-level objective, decompose it into a directed acyclic dependency graph of tasks, assign appropriate roles, supervise execution, handle failures, and synthesize final mission outcomes."
            }
            AgentRole::Architect => {
                "You are the Swarm Architect Agent. Your role is to analyze system constraints, design modular components, define strict API contracts and data models, and establish engineering guidelines for implementation tasks."
            }
            AgentRole::SpecialistWorker => {
                "You are the Specialist Worker Agent. Your role is to execute technical assignments with precision, generate high-quality production code, perform required file operations, and produce verified artifacts."
            }
            AgentRole::Reviewer => {
                "You are the Swarm Code Reviewer Agent powered by Kodus AI. Your role is to rigorously inspect changed files and diffs for bugs, architectural deviations, OWASP vulnerabilities, credential leakage, and edge case failures."
            }
            AgentRole::QaTester => {
                "You are the QA and Canary Tester Agent. Your role is to author test suites, run compilation and unit/integration tests, verify acceptance criteria, and ensure no regressions occur before task completion."
            }
        }
    }
}

/// Agent Instance Identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Task execution state within the Trinity Task Graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Blocked,
    Cancelled,
}

/// Task Priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

/// A discrete unit of work within the Trinity Task Graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub assigned_role: AgentRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_agent_id: Option<AgentId>,
    pub depends_on: Vec<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub required_capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canary_criteria: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<TaskResult>,
    pub retries: u32,
    pub max_retries: u32,
}

impl SwarmTask {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
        assigned_role: AgentRole,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            assigned_role,
            assigned_agent_id: None,
            depends_on: Vec::new(),
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            required_capabilities: Vec::new(),
            canary_criteria: None,
            result: None,
            retries: 0,
            max_retries: 3,
        }
    }

    pub fn with_dependency(mut self, dep_task_id: impl Into<String>) -> Self {
        self.depends_on.push(dep_task_id.into());
        self
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_canary(mut self, canary_criteria: impl Into<String>) -> Self {
        self.canary_criteria = Some(canary_criteria.into());
        self
    }
}

/// Result of an executed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub output: String,
    pub artifacts_produced: Vec<String>,
    pub execution_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// Swarm mission plan decomposed into tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmPlan {
    pub mission_id: String,
    pub goal: String,
    pub tasks: Vec<SwarmTask>,
    pub created_at: DateTime<Utc>,
}

/// Final verified execution receipt returned to OSMOO autonomous operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmExecutionReceipt {
    pub mission_id: String,
    pub goal: String,
    pub success: bool,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub execution_time_ms: u64,
    pub task_results: Vec<TaskResult>,
    pub artifacts: Vec<String>,
    pub summary: String,
}
