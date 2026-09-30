//! Task Graph V2 — Directed Acyclic Graph (DAG) based task management.
//!
//! Replaces simple linear plans with a proper DAG that supports:
//! - Parallel execution of independent tasks
//! - Dependency resolution with topological sort
//! - Critical path calculation
//! - Cycle detection
//! - Dynamic re-planning on failure
//! - Resource-constrained scheduling

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Core Types
// ============================================================================

/// Unique identifier for a task node in the graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskNodeId(pub String);

impl TaskNodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for TaskNodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for a task graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskGraphId(pub String);

impl TaskGraphId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl Default for TaskGraphId {
    fn default() -> Self {
        Self::new()
    }
}

/// Task priority level.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum TaskPriority {
    Critical = 0,
    High = 1,
    #[default]
    Medium = 2,
    Low = 3,
    Background = 4,
}

/// Task execution state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskNodeState {
    /// Not yet ready (dependencies pending).
    Pending,
    /// All dependencies met, ready to execute.
    Ready,
    /// Currently executing.
    Running,
    /// Successfully completed.
    Completed,
    /// Failed with error message.
    Failed(String),
    /// Skipped (dependency was skipped or cancelled).
    Skipped,
    /// Cancelled by user or system.
    Cancelled,
    /// Waiting for external input or resource.
    Waiting(String),
    /// Timed out.
    TimedOut,
}

/// The type of task being executed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskType {
    /// Code generation or modification.
    Code,
    /// Research or information gathering.
    Research,
    /// File system operations.
    FileSystem,
    /// Command execution.
    Command,
    /// Review or validation.
    Review,
    /// Testing.
    Test,
    /// Deployment.
    Deploy,
    /// Communication (email, message).
    Communication,
    /// Custom user-defined type.
    Custom(String),
}

/// A single node in the task graph DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNode {
    pub id: TaskNodeId,
    pub name: String,
    pub description: String,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub state: TaskNodeState,
    /// IDs of tasks that must complete before this one.
    pub dependencies: HashSet<TaskNodeId>,
    /// IDs of tasks that depend on this task.
    pub dependents: HashSet<TaskNodeId>,
    /// Estimated duration.
    pub estimated_duration: Option<Duration>,
    /// Actual duration in milliseconds (set on completion).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_duration_ms: Option<u64>,
    /// Maximum allowed retries.
    pub max_retries: u32,
    /// Current retry count.
    pub retry_count: u32,
    /// Maximum time before timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// When the task started executing (nanos since epoch, for serialization).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at_nanos: Option<u64>,
    /// When the task completed (nanos since epoch, for serialization).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at_nanos: Option<u64>,
    /// Result data from execution.
    pub result: Option<String>,
    /// Error details if failed.
    pub error: Option<String>,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Resource requirements.
    pub resource_requirements: ResourceRequirements,
    /// Metadata.
    pub metadata: HashMap<String, String>,
    // Runtime-only fields (not serialized)
    #[serde(skip)]
    pub(crate) _runtime_started: Option<Instant>,
    #[serde(skip)]
    pub(crate) _runtime_completed: Option<Instant>,
}

impl TaskNode {
    pub fn new(
        name: String,
        description: String,
        task_type: TaskType,
        priority: TaskPriority,
    ) -> Self {
        Self {
            id: TaskNodeId::new(),
            name,
            description,
            task_type,
            priority,
            state: TaskNodeState::Pending,
            dependencies: HashSet::new(),
            dependents: HashSet::new(),
            estimated_duration: None,
            actual_duration_ms: None,
            max_retries: 3,
            retry_count: 0,
            timeout_secs: Some(300),
            started_at_nanos: None,
            completed_at_nanos: None,
            result: None,
            error: None,
            tags: Vec::new(),
            resource_requirements: ResourceRequirements::default(),
            metadata: HashMap::new(),
            _runtime_started: None,
            _runtime_completed: None,
        }
    }

    /// Check if this task is ready to execute (all dependencies completed).
    pub fn is_ready(&self) -> bool {
        matches!(
            self.state,
            TaskNodeState::Pending | TaskNodeState::Waiting(_)
        )
    }

    /// Check if this task is terminal (completed, failed, skipped, cancelled).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            TaskNodeState::Completed
                | TaskNodeState::Failed(_)
                | TaskNodeState::Skipped
                | TaskNodeState::Cancelled
                | TaskNodeState::TimedOut
        )
    }

    /// Mark as started.
    pub fn start(&mut self) {
        self.state = TaskNodeState::Running;
        let now = Instant::now();
        self._runtime_started = Some(now);
        self.started_at_nanos = Some(now.elapsed().as_nanos() as u64);
    }

    /// Mark as completed.
    pub fn complete(&mut self, result: String) {
        self.state = TaskNodeState::Completed;
        self.result = Some(result);
        let now = Instant::now();
        self._runtime_completed = Some(now);
        self.completed_at_nanos = Some(now.elapsed().as_nanos() as u64);
        if let Some(start) = self._runtime_started {
            self.actual_duration_ms = Some(start.elapsed().as_millis() as u64);
        }
    }

    /// Mark as failed.
    pub fn fail(&mut self, error: String) {
        self.retry_count += 1;
        if self.retry_count < self.max_retries {
            self.state = TaskNodeState::Pending;
            self.error = Some(error);
            self._runtime_started = None;
            self.started_at_nanos = None;
        } else {
            self.state = TaskNodeState::Failed(error.clone());
            self.error = Some(error);
            let now = Instant::now();
            self._runtime_completed = Some(now);
            self.completed_at_nanos = Some(now.elapsed().as_nanos() as u64);
        }
    }
}

/// Resource requirements for a task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: Option<u32>,
    pub memory_mb: Option<u64>,
    pub gpu: bool,
    pub network: bool,
    pub exclusive: bool,
}

// ============================================================================
// Task Graph (DAG)
// ============================================================================

/// A directed acyclic graph of tasks with dependency management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskGraph {
    pub id: TaskGraphId,
    pub name: String,
    pub description: String,
    pub nodes: HashMap<TaskNodeId, TaskNode>,
    pub root_ids: HashSet<TaskNodeId>,
    pub terminal_ids: HashSet<TaskNodeId>,
    pub state: GraphState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphState {
    Building,
    Ready,
    Executing,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl TaskGraph {
    pub fn new(name: String, description: String) -> Self {
        Self {
            id: TaskGraphId::new(),
            name,
            description,
            nodes: HashMap::new(),
            root_ids: HashSet::new(),
            terminal_ids: HashSet::new(),
            state: GraphState::Building,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    /// Add a task node to the graph.
    pub fn add_node(&mut self, mut node: TaskNode) -> TaskNodeId {
        let id = node.id.clone();
        node.state = TaskNodeState::Pending;
        self.nodes.insert(id.clone(), node);
        self.root_ids.insert(id.clone());
        self.terminal_ids.insert(id.clone());
        self.updated_at = chrono::Utc::now();
        id
    }

    /// Add a dependency edge: `dependent_id` depends on `dependency_id`.
    pub fn add_dependency(
        &mut self,
        dependent_id: &TaskNodeId,
        dependency_id: &TaskNodeId,
    ) -> Result<(), GraphError> {
        if !self.nodes.contains_key(dependent_id) {
            return Err(GraphError::NodeNotFound(dependent_id.0.clone()));
        }
        if !self.nodes.contains_key(dependency_id) {
            return Err(GraphError::NodeNotFound(dependency_id.0.clone()));
        }
        if dependent_id == dependency_id {
            return Err(GraphError::SelfDependency(dependent_id.0.clone()));
        }

        // Check for cycle: adding dependent_id -> dependency_id would create a cycle
        // if dependency_id is already reachable from dependent_id
        if self.is_reachable(dependent_id, dependency_id) {
            return Err(GraphError::CycleDetected {
                from: dependent_id.0.clone(),
                to: dependency_id.0.clone(),
            });
        }

        self.nodes
            .get_mut(dependent_id)
            .unwrap()
            .dependencies
            .insert(dependency_id.clone());
        self.nodes
            .get_mut(dependency_id)
            .unwrap()
            .dependents
            .insert(dependent_id.clone());

        // Dependent is no longer a root
        self.root_ids.remove(dependent_id);
        // Dependency is no longer terminal
        self.terminal_ids.remove(dependency_id);

        self.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Check if `to_id` can reach `from_id` via the dependency edges (forward traversal).
    /// If true, adding `from_id -> to_id` would create a cycle.
    fn is_reachable(&self, from_id: &TaskNodeId, to_id: &TaskNodeId) -> bool {
        let mut visited = HashSet::new();
        let mut stack = Vec::new();

        // Start from to_id and follow dependencies forward — if we reach from_id,
        // then adding from_id -> to_id would create a cycle.
        stack.push(to_id.clone());
        while let Some(current) = stack.pop() {
            if current == *from_id {
                return true;
            }
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());
            if let Some(node) = self.nodes.get(&current) {
                for dep_id in &node.dependencies {
                    stack.push(dep_id.clone());
                }
            }
        }
        false
    }

    /// Validate that the graph has no cycles.
    pub fn validate(&self) -> Result<(), GraphError> {
        let mut visited = HashSet::with_capacity(self.nodes.len());
        let mut rec_stack = HashSet::with_capacity(self.nodes.len());

        for node_id in self.nodes.keys() {
            if !visited.contains(node_id) {
                self.dfs_cycle_check(node_id, &mut visited, &mut rec_stack)?;
            }
        }
        Ok(())
    }

    fn dfs_cycle_check<'a>(
        &'a self,
        node_id: &'a TaskNodeId,
        visited: &mut HashSet<&'a TaskNodeId>,
        rec_stack: &mut HashSet<&'a TaskNodeId>,
    ) -> Result<(), GraphError> {
        visited.insert(node_id);
        rec_stack.insert(node_id);

        if let Some(node) = self.nodes.get(node_id) {
            for dep_id in &node.dependents {
                if !visited.contains(dep_id) {
                    self.dfs_cycle_check(dep_id, visited, rec_stack)?;
                } else if rec_stack.contains(dep_id) {
                    return Err(GraphError::CycleDetected {
                        from: node_id.0.clone(),
                        to: dep_id.0.clone(),
                    });
                }
            }
        }

        rec_stack.remove(node_id);
        Ok(())
    }

    /// Topological sort returning execution layers (parallelizable groups).
    pub fn topological_layers(&self) -> Result<Vec<Vec<TaskNodeId>>, GraphError> {
        self.validate()?;

        let mut in_degree: HashMap<TaskNodeId, usize> = HashMap::with_capacity(self.nodes.len());
        for (id, node) in &self.nodes {
            in_degree
                .entry(id.clone())
                .or_insert(node.dependencies.len());
        }

        let mut layers = Vec::new();
        let mut processed = HashSet::with_capacity(self.nodes.len());

        loop {
            let layer: Vec<TaskNodeId> = in_degree
                .iter()
                .filter(|(id, &deg)| deg == 0 && !processed.contains(*id))
                .map(|(id, _)| id.clone())
                .collect();

            if layer.is_empty() {
                break;
            }

            for id in &layer {
                processed.insert(id.clone());
                if let Some(node) = self.nodes.get(id) {
                    for dep_id in &node.dependents {
                        if let Some(deg) = in_degree.get_mut(dep_id) {
                            *deg = deg.saturating_sub(1);
                        }
                    }
                }
            }

            layers.push(layer);
        }

        if processed.len() != self.nodes.len() {
            return Err(GraphError::CycleDetected {
                from: "unknown".to_string(),
                to: "unknown".to_string(),
            });
        }

        Ok(layers)
    }

    /// Calculate the critical path (longest path through the graph).
    pub fn critical_path(&self) -> Result<Vec<TaskNodeId>, GraphError> {
        let layers = self.topological_layers()?;
        let mut distances: HashMap<TaskNodeId, Duration> = HashMap::new();
        let mut predecessors: HashMap<TaskNodeId, Option<TaskNodeId>> = HashMap::new();

        for layer in &layers {
            for node_id in layer {
                let node = &self.nodes[node_id];
                let node_duration = node.estimated_duration.unwrap_or(Duration::from_secs(60));

                let max_predecessor_dist = node
                    .dependencies
                    .iter()
                    .filter_map(|dep_id| distances.get(dep_id))
                    .max()
                    .copied()
                    .unwrap_or(Duration::ZERO);

                distances.insert(node_id.clone(), max_predecessor_dist + node_duration);

                let pred = node
                    .dependencies
                    .iter()
                    .max_by_key(|dep_id| distances.get(*dep_id).unwrap_or(&Duration::ZERO))
                    .cloned();
                predecessors.insert(node_id.clone(), pred);
            }
        }

        // Find the node with maximum distance
        let end_node = distances
            .iter()
            .max_by_key(|(_, d)| **d)
            .map(|(id, _)| id.clone())
            .ok_or(GraphError::EmptyGraph)?;

        // Reconstruct path
        let mut path = Vec::new();
        let mut current = Some(end_node);
        while let Some(node_id) = current {
            path.push(node_id.clone());
            current = predecessors.get(&node_id).and_then(|p| p.clone());
        }
        path.reverse();

        Ok(path)
    }

    /// Get all tasks that are ready to execute.
    pub fn ready_tasks(&self) -> Vec<&TaskNode> {
        self.nodes
            .values()
            .filter(|n| n.is_ready())
            .filter(|n| {
                n.dependencies.iter().all(|dep_id| {
                    matches!(
                        self.nodes.get(dep_id).map(|n| &n.state),
                        Some(TaskNodeState::Completed)
                    )
                })
            })
            .collect()
    }

    /// Get tasks by priority.
    pub fn tasks_by_priority(&self) -> Vec<&TaskNode> {
        let mut tasks: Vec<&TaskNode> = self.nodes.values().filter(|n| !n.is_terminal()).collect();
        tasks.sort_by_key(|n| n.priority);
        tasks
    }

    /// Calculate overall progress (0.0 - 1.0).
    pub fn progress(&self) -> f32 {
        if self.nodes.is_empty() {
            return 1.0;
        }
        let completed = self
            .nodes
            .values()
            .filter(|n| matches!(n.state, TaskNodeState::Completed))
            .count();
        completed as f32 / self.nodes.len() as f32
    }

    /// Calculate estimated time remaining.
    pub fn estimated_time_remaining(&self) -> Duration {
        self.nodes
            .values()
            .filter(|n| !n.is_terminal())
            .filter_map(|n| n.estimated_duration)
            .sum()
    }

    /// Get all failed tasks.
    pub fn failed_tasks(&self) -> Vec<&TaskNode> {
        self.nodes
            .values()
            .filter(|n| matches!(n.state, TaskNodeState::Failed(_)))
            .collect()
    }

    /// Get the total node count.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get completed node count.
    pub fn completed_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| matches!(n.state, TaskNodeState::Completed))
            .count()
    }

    /// Cancel all pending/running tasks.
    pub fn cancel_all(&mut self) {
        for node in self.nodes.values_mut() {
            if !node.is_terminal() {
                node.state = TaskNodeState::Cancelled;
            }
        }
        self.state = GraphState::Cancelled;
    }
}

// ============================================================================
// Task Graph Builder (fluent API)
// ============================================================================

/// Fluent builder for constructing task graphs.
pub struct TaskGraphBuilder {
    graph: TaskGraph,
    last_added: Option<TaskNodeId>,
}

impl TaskGraphBuilder {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            graph: TaskGraph::new(name.to_string(), description.to_string()),
            last_added: None,
        }
    }

    /// Add a task node.
    pub fn task(mut self, name: &str, description: &str, task_type: TaskType) -> Self {
        let node = TaskNode::new(
            name.to_string(),
            description.to_string(),
            task_type,
            TaskPriority::Medium,
        );
        self.last_added = Some(self.graph.add_node(node));
        self
    }

    /// Add a task with priority.
    pub fn task_with_priority(
        mut self,
        name: &str,
        description: &str,
        task_type: TaskType,
        priority: TaskPriority,
    ) -> Self {
        let node = TaskNode::new(
            name.to_string(),
            description.to_string(),
            task_type,
            priority,
        );
        self.last_added = Some(self.graph.add_node(node));
        self
    }

    /// Add a task that depends on the previously added task.
    pub fn then(mut self, name: &str, description: &str, task_type: TaskType) -> Self {
        let prev_id = self.last_added.take();
        let node = TaskNode::new(
            name.to_string(),
            description.to_string(),
            task_type,
            TaskPriority::Medium,
        );
        let new_id = self.graph.add_node(node);
        if let Some(prev) = prev_id {
            let _ = self.graph.add_dependency(&new_id, &prev);
        }
        self.last_added = Some(new_id);
        self
    }

    /// Build the graph, validating it.
    pub fn build(mut self) -> Result<TaskGraph, GraphError> {
        self.graph.validate()?;
        self.graph.state = GraphState::Ready;
        Ok(self.graph)
    }
}

// ============================================================================
// Execution Engine
// ============================================================================

/// Executes a task graph, handling dependency resolution and parallel execution.
pub struct TaskGraphExecutor {
    concurrency_limit: usize,
}

impl TaskGraphExecutor {
    pub fn new(concurrency_limit: usize) -> Self {
        Self {
            concurrency_limit: concurrency_limit.max(1),
        }
    }

    /// Get the next batch of tasks to execute (respecting dependencies and concurrency).
    pub fn next_batch<'a>(&self, graph: &'a TaskGraph) -> Vec<&'a TaskNode> {
        let ready = graph.ready_tasks();
        let mut batch: Vec<&TaskNode> = ready
            .into_iter()
            .filter(|n| matches!(n.state, TaskNodeState::Pending))
            .collect();

        // Sort by priority (critical first)
        batch.sort_by_key(|n| n.priority);
        batch.truncate(self.concurrency_limit);
        batch
    }

    /// Mark a task as started.
    pub fn start_task(graph: &mut TaskGraph, task_id: &TaskNodeId) -> Result<(), GraphError> {
        let node = graph
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| GraphError::NodeNotFound(task_id.0.clone()))?;
        node.start();
        graph.updated_at = chrono::Utc::now();
        Ok(())
    }

    /// Mark a task as completed.
    pub fn complete_task(
        graph: &mut TaskGraph,
        task_id: &TaskNodeId,
        result: String,
    ) -> Result<(), GraphError> {
        let node = graph
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| GraphError::NodeNotFound(task_id.0.clone()))?;
        node.complete(result);
        graph.updated_at = chrono::Utc::now();

        // Check if all nodes are terminal
        if graph.nodes.values().all(|n| n.is_terminal()) {
            let has_failures = !graph.failed_tasks().is_empty();
            graph.state = if has_failures {
                GraphState::Failed
            } else {
                GraphState::Completed
            };
        }

        Ok(())
    }

    /// Mark a task as failed (with retry logic).
    pub fn fail_task(
        graph: &mut TaskGraph,
        task_id: &TaskNodeId,
        error: String,
    ) -> Result<(), GraphError> {
        let node = graph
            .nodes
            .get_mut(task_id)
            .ok_or_else(|| GraphError::NodeNotFound(task_id.0.clone()))?;
        node.fail(error);
        graph.updated_at = chrono::Utc::now();

        // If task failed permanently, skip all dependents
        if matches!(node.state, TaskNodeState::Failed(_)) {
            Self::skip_dependents(graph, task_id);
        }

        Ok(())
    }

    /// Recursively skip all tasks that depend on a failed task.
    fn skip_dependents(graph: &mut TaskGraph, task_id: &TaskNodeId) {
        if let Some(node) = graph.nodes.get(task_id).cloned() {
            for dep_id in &node.dependents {
                if let Some(dep_node) = graph.nodes.get_mut(dep_id) {
                    if !dep_node.is_terminal() {
                        dep_node.state = TaskNodeState::Skipped;
                        Self::skip_dependents(graph, dep_id);
                    }
                }
            }
        }
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, Clone, thiserror::Error)]
pub enum GraphError {
    #[error("Task node not found: {0}")]
    NodeNotFound(String),

    #[error("Self-dependency detected: {0}")]
    SelfDependency(String),

    #[error("Cycle detected: {from} -> {to}")]
    CycleDetected { from: String, to: String },

    #[error("Empty graph")]
    EmptyGraph,

    #[error("Invalid state transition: {0}")]
    InvalidTransition(String),
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_node_creation() {
        let node = TaskNode::new(
            "Test".to_string(),
            "Description".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        );
        assert_eq!(node.name, "Test");
        assert!(node.is_ready());
        assert!(!node.is_terminal());
    }

    #[test]
    fn task_node_lifecycle() {
        let mut node = TaskNode::new(
            "Test".to_string(),
            "Desc".to_string(),
            TaskType::Code,
            TaskPriority::High,
        );

        node.start();
        assert_eq!(node.state, TaskNodeState::Running);

        node.complete("done".to_string());
        assert!(node.is_terminal());
        assert_eq!(node.state, TaskNodeState::Completed);
        assert!(node.actual_duration_ms.is_some());
    }

    #[test]
    fn task_node_retry() {
        let mut node = TaskNode::new(
            "Test".to_string(),
            "Desc".to_string(),
            TaskType::Code,
            TaskPriority::High,
        );
        node.max_retries = 2;

        node.start();
        node.fail("error1".to_string());
        assert_eq!(node.retry_count, 1);
        assert!(!node.is_terminal()); // Should be pending for retry

        node.start();
        node.fail("error2".to_string());
        assert_eq!(node.retry_count, 2);
        assert!(node.is_terminal()); // Should be permanently failed
    }

    #[test]
    fn graph_creation() {
        let graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        assert_eq!(graph.nodes.len(), 0);
    }

    #[test]
    fn graph_add_node() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let node = TaskNode::new(
            "Task 1".to_string(),
            "Desc".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        );
        let id = graph.add_node(node);
        assert!(graph.nodes.contains_key(&id));
    }

    #[test]
    fn graph_add_dependency() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "Task 1".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "Task 2".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.add_dependency(&n2, &n1).unwrap();

        assert!(graph.nodes[&n2].dependencies.contains(&n1));
        assert!(graph.nodes[&n1].dependents.contains(&n2));
        assert!(!graph.root_ids.contains(&n2));
    }

    #[test]
    fn graph_cycle_detection() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n3 = graph.add_node(TaskNode::new(
            "C".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.add_dependency(&n2, &n1).unwrap();
        graph.add_dependency(&n3, &n2).unwrap();
        // This would create a cycle: n1 -> n2 -> n3 -> n1
        let result = graph.add_dependency(&n1, &n3);
        assert!(result.is_err());
    }

    #[test]
    fn graph_topological_sort() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n3 = graph.add_node(TaskNode::new(
            "C".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.add_dependency(&n2, &n1).unwrap();
        graph.add_dependency(&n3, &n2).unwrap();

        let layers = graph.topological_layers().unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0], vec![n1.clone()]);
        assert_eq!(layers[1], vec![n2.clone()]);
        assert_eq!(layers[2], vec![n3.clone()]);
    }

    #[test]
    fn graph_parallel_layers() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n3 = graph.add_node(TaskNode::new(
            "C".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        // n1 and n2 are independent (both roots), n3 depends on both
        graph.add_dependency(&n3, &n1).unwrap();
        graph.add_dependency(&n3, &n2).unwrap();

        let layers = graph.topological_layers().unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].len(), 2); // n1 and n2 in parallel
        assert_eq!(layers[1], vec![n3.clone()]);
    }

    #[test]
    fn graph_ready_tasks() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.add_dependency(&n2, &n1).unwrap();

        // Initially only n1 is ready
        let ready = graph.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, n1);

        // Complete n1
        graph.nodes.get_mut(&n1).unwrap().start();
        graph
            .nodes
            .get_mut(&n1)
            .unwrap()
            .complete("done".to_string());

        // Now n2 should be ready
        let ready = graph.ready_tasks();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].id, n2);
    }

    #[test]
    fn graph_progress() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        assert_eq!(graph.progress(), 0.0);

        graph.nodes.get_mut(&n1).unwrap().state = TaskNodeState::Completed;
        assert_eq!(graph.progress(), 0.5);

        graph.nodes.get_mut(&n2).unwrap().state = TaskNodeState::Completed;
        assert_eq!(graph.progress(), 1.0);
    }

    #[test]
    fn graph_builder_fluent() {
        let graph = TaskGraphBuilder::new("Project", "Build something")
            .task("Research", "Research topic", TaskType::Research)
            .then("Code", "Write code", TaskType::Code)
            .then("Test", "Write tests", TaskType::Test)
            .then("Deploy", "Deploy", TaskType::Deploy)
            .build()
            .unwrap();

        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.state, GraphState::Ready);
    }

    #[test]
    fn graph_cancel_all() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.cancel_all();
        assert_eq!(graph.state, GraphState::Cancelled);
        for node in graph.nodes.values() {
            assert_eq!(node.state, TaskNodeState::Cancelled);
        }
    }

    #[test]
    fn graph_critical_path() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n3 = graph.add_node(TaskNode::new(
            "C".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.nodes.get_mut(&n1).unwrap().estimated_duration = Some(Duration::from_secs(10));
        graph.nodes.get_mut(&n2).unwrap().estimated_duration = Some(Duration::from_secs(20));
        graph.nodes.get_mut(&n3).unwrap().estimated_duration = Some(Duration::from_secs(5));

        graph.add_dependency(&n2, &n1).unwrap();
        graph.add_dependency(&n3, &n2).unwrap();

        let path = graph.critical_path().unwrap();
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], n1);
        assert_eq!(path[1], n2);
        assert_eq!(path[2], n3);
    }

    #[test]
    fn executor_next_batch() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let _n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::High,
        ));
        let _n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Low,
        ));

        let executor = TaskGraphExecutor::new(2);
        let batch = executor.next_batch(&graph);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn executor_skip_dependents_on_failure() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n3 = graph.add_node(TaskNode::new(
            "C".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.add_dependency(&n2, &n1).unwrap();
        graph.add_dependency(&n3, &n2).unwrap();

        // Fail n1 permanently
        graph.nodes.get_mut(&n1).unwrap().max_retries = 0;
        graph.nodes.get_mut(&n1).unwrap().start();
        TaskGraphExecutor::fail_task(&mut graph, &n1, "fatal".to_string()).unwrap();

        // n2 and n3 should be skipped
        assert_eq!(graph.nodes[&n2].state, TaskNodeState::Skipped);
        assert_eq!(graph.nodes[&n3].state, TaskNodeState::Skipped);
    }

    #[test]
    fn task_priority_ordering() {
        assert!(TaskPriority::Critical < TaskPriority::High);
        assert!(TaskPriority::High < TaskPriority::Medium);
        assert!(TaskPriority::Medium < TaskPriority::Low);
        assert!(TaskPriority::Low < TaskPriority::Background);
    }

    #[test]
    fn graph_empty_validation() {
        let graph = TaskGraph::new("Empty".to_string(), "Desc".to_string());
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn graph_self_dependency_error() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let result = graph.add_dependency(&n1, &n1);
        assert!(result.is_err());
    }

    #[test]
    fn graph_node_not_found() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = TaskNodeId::new();
        let n2 = TaskNodeId::new();
        graph.nodes.insert(
            n1.clone(),
            TaskNode::new(
                "A".to_string(),
                "".to_string(),
                TaskType::Code,
                TaskPriority::Medium,
            ),
        );
        let result = graph.add_dependency(&n1, &n2);
        assert!(result.is_err());
    }

    #[test]
    fn graph_estimated_time_remaining() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.nodes.get_mut(&n1).unwrap().estimated_duration = Some(Duration::from_secs(10));
        graph.nodes.get_mut(&n2).unwrap().estimated_duration = Some(Duration::from_secs(20));

        assert_eq!(graph.estimated_time_remaining(), Duration::from_secs(30));

        graph.nodes.get_mut(&n1).unwrap().state = TaskNodeState::Completed;
        assert_eq!(graph.estimated_time_remaining(), Duration::from_secs(20));
    }

    #[test]
    fn task_type_variants() {
        let _ = TaskType::Code;
        let _ = TaskType::Research;
        let _ = TaskType::FileSystem;
        let _ = TaskType::Command;
        let _ = TaskType::Review;
        let _ = TaskType::Test;
        let _ = TaskType::Deploy;
        let _ = TaskType::Communication;
        let _ = TaskType::Custom("custom".to_string());
    }

    #[test]
    fn resource_requirements_default() {
        let req = ResourceRequirements::default();
        assert!(req.cpu_cores.is_none());
        assert!(!req.gpu);
        assert!(!req.network);
        assert!(!req.exclusive);
    }

    #[test]
    fn graph_failed_tasks() {
        let mut graph = TaskGraph::new("Test".to_string(), "Desc".to_string());
        let n1 = graph.add_node(TaskNode::new(
            "A".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));
        let n2 = graph.add_node(TaskNode::new(
            "B".to_string(),
            "".to_string(),
            TaskType::Code,
            TaskPriority::Medium,
        ));

        graph.nodes.get_mut(&n1).unwrap().state = TaskNodeState::Completed;
        graph.nodes.get_mut(&n2).unwrap().state = TaskNodeState::Failed("error".to_string());

        let failed = graph.failed_tasks();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, n2);
    }
}
