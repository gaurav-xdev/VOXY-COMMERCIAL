//! Multi-Agent Orchestrator — specialized agents that collaborate.
//!
//! Agents: Planner, Researcher, Coder, Desktop, Browser, QA, Security,
//! Memory, Reviewer. Planner assigns work. Reviewer verifies. QA tests.
//! Security checks. Memory stores.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub String);

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskAssignmentId(pub String);

/// Specialized agent types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    Planner,
    Researcher,
    Coder,
    Desktop,
    Browser,
    QA,
    Security,
    Memory,
    Reviewer,
}

impl AgentRole {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Planner => "Planner",
            Self::Researcher => "Researcher",
            Self::Coder => "Coder",
            Self::Desktop => "Desktop",
            Self::Browser => "Browser",
            Self::QA => "QA",
            Self::Security => "Security",
            Self::Memory => "Memory",
            Self::Reviewer => "Reviewer",
        }
    }

    pub fn can_handle(&self, task_type: &str) -> bool {
        match self {
            Self::Planner => task_type == "plan" || task_type == "decompose",
            Self::Researcher => task_type == "research" || task_type == "analyze",
            Self::Coder => {
                task_type == "code"
                    || task_type == "implement"
                    || task_type == "patch"
                    || task_type == "build"
                    || task_type == "diagnostics"
                    || task_type == "harness"
            }
            Self::Desktop => task_type == "desktop" || task_type == "ui",
            Self::Browser => task_type == "browser" || task_type == "web",
            Self::QA => task_type == "test" || task_type == "validate",
            Self::Security => task_type == "security" || task_type == "audit",
            Self::Memory => task_type == "memory" || task_type == "store",
            Self::Reviewer => task_type == "review" || task_type == "verify",
        }
    }
}

/// Agent status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Working,
    Error(String),
    Offline,
}

/// Information about an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: AgentId,
    pub role: AgentRole,
    pub name: String,
    pub status: AgentStatus,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub current_task: Option<TaskAssignmentId>,
    pub capabilities: Vec<String>,
    pub metadata: HashMap<String, String>,
}

/// A task to be assigned to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: TaskAssignmentId,
    pub task_type: String,
    pub description: String,
    pub input: HashMap<String, String>,
    pub required_role: Option<AgentRole>,
    pub priority: u32,
    pub timeout_secs: Option<u64>,
    pub result: Option<String>,
    pub status: TaskAssignmentStatus,
    pub assigned_agent: Option<AgentId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskAssignmentStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// Communication between agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub from: AgentId,
    pub to: AgentId,
    pub message_type: MessageType,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    TaskAssignment,
    TaskResult,
    RequestHelp,
    ProvideHelp,
    StatusUpdate,
    ReviewRequest,
    ReviewResult,
    SecurityCheck,
    MemoryStore,
}

// ============================================================================
// Orchestrator
// ============================================================================

/// Orchestrates multiple specialized agents.
pub struct MultiAgentOrchestrator {
    agents: HashMap<AgentId, AgentInfo>,
    task_queue: Vec<AgentTask>,
    completed_tasks: Vec<AgentTask>,
    message_log: Vec<AgentMessage>,
    max_agents: usize,
    max_queue: usize,
}

impl MultiAgentOrchestrator {
    pub fn new(max_agents: usize, max_queue: usize) -> Self {
        Self {
            agents: HashMap::new(),
            task_queue: Vec::new(),
            completed_tasks: Vec::new(),
            message_log: Vec::new(),
            max_agents,
            max_queue,
        }
    }

    pub fn default_orchestrator() -> Self {
        Self::new(20, 100)
    }

    /// Create and register a default set of agents.
    pub fn with_default_agents() -> Self {
        let mut orch = Self::default_orchestrator();
        let roles = vec![
            AgentRole::Planner,
            AgentRole::Researcher,
            AgentRole::Coder,
            AgentRole::Desktop,
            AgentRole::Browser,
            AgentRole::QA,
            AgentRole::Security,
            AgentRole::Memory,
            AgentRole::Reviewer,
        ];

        for role in roles {
            let agent = AgentInfo {
                id: AgentId::new(),
                role: role.clone(),
                name: role.name().to_string(),
                status: AgentStatus::Idle,
                tasks_completed: 0,
                tasks_failed: 0,
                current_task: None,
                capabilities: vec![role.name().to_lowercase()],
                metadata: HashMap::new(),
            };
            orch.agents.insert(agent.id.clone(), agent);
        }

        orch
    }

    /// Register a new agent.
    pub fn register_agent(&mut self, agent: AgentInfo) -> Result<AgentId, OrchestratorError> {
        if self.agents.len() >= self.max_agents {
            return Err(OrchestratorError::CapacityReached(self.max_agents));
        }
        let id = agent.id.clone();
        self.agents.insert(id.clone(), agent);
        Ok(id)
    }

    /// Submit a task for execution.
    pub fn submit_task(&mut self, task: AgentTask) -> Result<TaskAssignmentId, OrchestratorError> {
        if self.task_queue.len() >= self.max_queue {
            return Err(OrchestratorError::QueueFull(self.max_queue));
        }
        let id = task.id.clone();
        self.task_queue.push(task);
        Ok(id)
    }

    /// Find the best agent for a task.
    pub fn find_agent(&self, task: &AgentTask) -> Option<&AgentInfo> {
        self.agents
            .values()
            .filter(|a| a.status == AgentStatus::Idle)
            .filter(|a| {
                task.required_role
                    .as_ref()
                    .map(|r| a.role == *r)
                    .unwrap_or(true)
            })
            .filter(|a| a.role.can_handle(&task.task_type))
            .max_by_key(|a| a.tasks_completed)
    }

    /// Assign the next task to an agent.
    pub fn assign_next(&mut self) -> Option<(AgentId, AgentTask)> {
        // Find the next pending task
        let task_idx = self
            .task_queue
            .iter()
            .position(|t| t.status == TaskAssignmentStatus::Pending)?;
        let task = &self.task_queue[task_idx];

        // Find the best agent
        let agent_id = self.find_agent(task)?.id.clone();

        // Assign
        let mut task = self.task_queue.remove(task_idx);
        task.status = TaskAssignmentStatus::Assigned;
        task.assigned_agent = Some(agent_id.clone());

        if let Some(agent) = self.agents.get_mut(&agent_id) {
            agent.status = AgentStatus::Working;
            agent.current_task = Some(task.id.clone());
        }

        self.task_queue.push(task.clone());
        Some((agent_id, task))
    }

    /// Complete a task.
    pub fn complete_task(
        &mut self,
        task_id: &TaskAssignmentId,
        result: String,
    ) -> Result<(), OrchestratorError> {
        let task_idx = self
            .task_queue
            .iter()
            .position(|t| t.id == *task_id)
            .ok_or_else(|| OrchestratorError::TaskNotFound(task_id.0.clone()))?;

        let mut task = self.task_queue.remove(task_idx);
        task.status = TaskAssignmentStatus::Completed;
        task.result = Some(result);

        // Update agent stats
        if let Some(agent_id) = &task.assigned_agent {
            if let Some(agent) = self.agents.get_mut(agent_id) {
                agent.tasks_completed += 1;
                agent.status = AgentStatus::Idle;
                agent.current_task = None;
            }
        }

        self.completed_tasks.push(task);
        Ok(())
    }

    /// Fail a task.
    pub fn fail_task(
        &mut self,
        task_id: &TaskAssignmentId,
        error: String,
    ) -> Result<(), OrchestratorError> {
        let task_idx = self
            .task_queue
            .iter()
            .position(|t| t.id == *task_id)
            .ok_or_else(|| OrchestratorError::TaskNotFound(task_id.0.clone()))?;

        let mut task = self.task_queue.remove(task_idx);
        task.status = TaskAssignmentStatus::Failed(error);

        if let Some(agent_id) = &task.assigned_agent {
            if let Some(agent) = self.agents.get_mut(agent_id) {
                agent.tasks_failed += 1;
                agent.status = AgentStatus::Idle;
                agent.current_task = None;
            }
        }

        self.completed_tasks.push(task);
        Ok(())
    }

    /// Get all agents.
    pub fn agents(&self) -> Vec<&AgentInfo> {
        self.agents.values().collect()
    }

    /// Get pending tasks.
    pub fn pending_tasks(&self) -> Vec<&AgentTask> {
        self.task_queue
            .iter()
            .filter(|t| t.status == TaskAssignmentStatus::Pending)
            .collect()
    }

    /// Get completed tasks.
    pub fn completed_tasks(&self) -> &[AgentTask] {
        &self.completed_tasks
    }

    /// Send a message between agents.
    pub fn send_message(&mut self, message: AgentMessage) {
        self.message_log.push(message);
    }

    /// Get message log.
    pub fn message_log(&self) -> &[AgentMessage] {
        &self.message_log
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum OrchestratorError {
    #[error("Agent capacity reached: {0}")]
    CapacityReached(usize),

    #[error("Task queue full: {0}")]
    QueueFull(usize),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_creation() {
        let orch = MultiAgentOrchestrator::default_orchestrator();
        assert_eq!(orch.agents().len(), 0);
    }

    #[test]
    fn default_agents() {
        let orch = MultiAgentOrchestrator::with_default_agents();
        assert_eq!(orch.agents().len(), 9);
    }

    #[test]
    fn register_agent() {
        let mut orch = MultiAgentOrchestrator::default_orchestrator();
        let agent = AgentInfo {
            id: AgentId::new(),
            role: AgentRole::Coder,
            name: "Coder".to_string(),
            status: AgentStatus::Idle,
            tasks_completed: 0,
            tasks_failed: 0,
            current_task: None,
            capabilities: vec!["code".to_string()],
            metadata: HashMap::new(),
        };
        orch.register_agent(agent).unwrap();
        assert_eq!(orch.agents().len(), 1);
    }

    #[test]
    fn submit_task() {
        let mut orch = MultiAgentOrchestrator::default_orchestrator();
        let task = AgentTask {
            id: TaskAssignmentId(Uuid::new_v4().to_string()),
            task_type: "code".to_string(),
            description: "Write code".to_string(),
            input: HashMap::new(),
            required_role: None,
            priority: 1,
            timeout_secs: None,
            result: None,
            status: TaskAssignmentStatus::Pending,
            assigned_agent: None,
        };
        orch.submit_task(task).unwrap();
        assert_eq!(orch.pending_tasks().len(), 1);
    }

    #[test]
    fn find_agent_for_task() {
        let orch = MultiAgentOrchestrator::with_default_agents();
        let task = AgentTask {
            id: TaskAssignmentId(Uuid::new_v4().to_string()),
            task_type: "code".to_string(),
            description: "Write code".to_string(),
            input: HashMap::new(),
            required_role: Some(AgentRole::Coder),
            priority: 1,
            timeout_secs: None,
            result: None,
            status: TaskAssignmentStatus::Pending,
            assigned_agent: None,
        };
        let agent = orch.find_agent(&task);
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().role, AgentRole::Coder);
    }

    #[test]
    fn assign_and_complete() {
        let mut orch = MultiAgentOrchestrator::with_default_agents();
        let task = AgentTask {
            id: TaskAssignmentId(Uuid::new_v4().to_string()),
            task_type: "code".to_string(),
            description: "Write code".to_string(),
            input: HashMap::new(),
            required_role: None,
            priority: 1,
            timeout_secs: None,
            result: None,
            status: TaskAssignmentStatus::Pending,
            assigned_agent: None,
        };
        let task_id = task.id.clone();
        orch.submit_task(task).unwrap();

        let (agent_id, _assigned) = orch.assign_next().unwrap();
        orch.complete_task(&task_id, "done".to_string()).unwrap();

        let agent = orch.agents.get(&agent_id).unwrap();
        assert_eq!(agent.tasks_completed, 1);
        assert_eq!(agent.status, AgentStatus::Idle);
    }

    #[test]
    fn agent_role_can_handle() {
        assert!(AgentRole::Coder.can_handle("code"));
        assert!(!AgentRole::Coder.can_handle("security"));
        assert!(AgentRole::Security.can_handle("audit"));
        assert!(AgentRole::QA.can_handle("test"));
        assert!(AgentRole::Browser.can_handle("web"));
    }

    #[test]
    fn message_log() {
        let mut orch = MultiAgentOrchestrator::with_default_agents();
        let agents: Vec<_> = orch.agents().into_iter().cloned().collect();
        let msg = AgentMessage {
            from: agents[0].id.clone(),
            to: agents[1].id.clone(),
            message_type: MessageType::TaskAssignment,
            content: "Do this".to_string(),
            timestamp: chrono::Utc::now(),
        };
        orch.send_message(msg);
        assert_eq!(orch.message_log().len(), 1);
    }

    #[test]
    fn capacity_limit() {
        let mut orch = MultiAgentOrchestrator::new(1, 10);
        let agent = AgentInfo {
            id: AgentId::new(),
            role: AgentRole::Coder,
            name: "A1".to_string(),
            status: AgentStatus::Idle,
            tasks_completed: 0,
            tasks_failed: 0,
            current_task: None,
            capabilities: Vec::new(),
            metadata: HashMap::new(),
        };
        orch.register_agent(agent).unwrap();
        let agent2 = AgentInfo {
            id: AgentId::new(),
            role: AgentRole::QA,
            name: "A2".to_string(),
            status: AgentStatus::Idle,
            tasks_completed: 0,
            tasks_failed: 0,
            current_task: None,
            capabilities: Vec::new(),
            metadata: HashMap::new(),
        };
        let result = orch.register_agent(agent2);
        assert!(result.is_err());
    }
}
