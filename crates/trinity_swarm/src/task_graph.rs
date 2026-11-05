use crate::error::{Result, SwarmError};
use crate::types::{SwarmTask, TaskResult, TaskStatus};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Thread-safe Directed Acyclic Graph (DAG) for Trinity task execution.
#[derive(Debug, Clone)]
pub struct TaskGraph {
    tasks: Arc<RwLock<HashMap<String, SwarmTask>>>,
}

impl TaskGraph {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a task to the DAG.
    pub fn add_task(&self, task: SwarmTask) {
        self.tasks.write().insert(task.id.clone(), task);
    }

    /// Add multiple tasks.
    pub fn add_tasks(&self, tasks: Vec<SwarmTask>) {
        let mut map = self.tasks.write();
        for task in tasks {
            map.insert(task.id.clone(), task);
        }
    }

    /// Retrieve a task by ID.
    pub fn get_task(&self, task_id: &str) -> Option<SwarmTask> {
        self.tasks.read().get(task_id).cloned()
    }

    /// Retrieve all tasks.
    pub fn all_tasks(&self) -> Vec<SwarmTask> {
        self.tasks.read().values().cloned().collect()
    }

    /// Total number of tasks.
    pub fn len(&self) -> usize {
        self.tasks.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.tasks.read().is_empty()
    }

    /// Validate that the task graph contains no cycles and all dependencies exist.
    pub fn validate_dag(&self) -> Result<()> {
        let tasks = self.tasks.read();

        // 1. Verify that all referenced dependencies exist
        for (id, task) in tasks.iter() {
            for dep in &task.depends_on {
                if !tasks.contains_key(dep) {
                    return Err(SwarmError::TaskNotFound(format!(
                        "Task '{id}' depends on missing task '{dep}'"
                    )));
                }
            }
        }

        // 2. Cycle detection via Kahn's algorithm (topological sort)
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

        for (id, task) in tasks.iter() {
            in_degree.entry(id.as_str()).or_insert(0);
            for dep in &task.depends_on {
                adj.entry(dep.as_str()).or_default().push(id.as_str());
                *in_degree.entry(id.as_str()).or_insert(0) += 1;
            }
        }

        let mut queue = VecDeque::new();
        for (id, deg) in in_degree.iter() {
            if *deg == 0 {
                queue.push_back(*id);
            }
        }

        let mut visited_count = 0;
        while let Some(u) = queue.pop_front() {
            visited_count += 1;
            if let Some(neighbors) = adj.get(u) {
                for &v in neighbors {
                    if let Some(deg) = in_degree.get_mut(v) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(v);
                        }
                    }
                }
            }
        }

        if visited_count < tasks.len() {
            return Err(SwarmError::CyclicDependency(format!(
                "Cycle detected in task graph: visited {visited_count} of {} tasks",
                tasks.len()
            )));
        }

        Ok(())
    }

    /// Retrieve all tasks that are ready for immediate execution:
    /// - Current status is Pending or Ready
    /// - All prerequisite dependency tasks are Completed
    pub fn get_ready_tasks(&self) -> Vec<SwarmTask> {
        let mut tasks = self.tasks.write();

        // Collect completed task IDs
        let completed: HashSet<String> = tasks
            .values()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect();

        let mut ready = Vec::new();

        for task in tasks.values_mut() {
            if task.status == TaskStatus::Pending || task.status == TaskStatus::Ready {
                let all_deps_met = task.depends_on.iter().all(|dep| completed.contains(dep));
                if all_deps_met {
                    task.status = TaskStatus::Ready;
                    ready.push(task.clone());
                }
            }
        }

        // Sort by priority descending
        ready.sort_by(|a, b| b.priority.cmp(&a.priority));
        ready
    }

    /// Update status of a specific task.
    /// If marked as Failed, cascades Blocked status to dependent tasks.
    pub fn update_task_status(&self, task_id: &str, status: TaskStatus) -> Result<()> {
        let mut tasks = self.tasks.write();

        let task = tasks.get_mut(task_id).ok_or_else(|| {
            SwarmError::TaskNotFound(format!("Cannot update non-existent task '{task_id}'"))
        })?;
        task.status = status;

        // Cascade failure to dependent downstream tasks
        if status == TaskStatus::Failed || status == TaskStatus::Cancelled {
            let mut to_block = vec![task_id.to_string()];
            while let Some(parent_id) = to_block.pop() {
                for t in tasks.values_mut() {
                    if t.depends_on.contains(&parent_id)
                        && (t.status == TaskStatus::Pending || t.status == TaskStatus::Ready)
                    {
                        t.status = TaskStatus::Blocked;
                        to_block.push(t.id.clone());
                    }
                }
            }
        }

        Ok(())
    }

    /// Attach result to an executed task and update its status.
    pub fn set_task_result(&self, task_id: &str, result: TaskResult) -> Result<()> {
        let mut tasks = self.tasks.write();
        let task = tasks.get_mut(task_id).ok_or_else(|| {
            SwarmError::TaskNotFound(format!(
                "Cannot set result on non-existent task '{task_id}'"
            ))
        })?;

        task.status = if result.success {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        task.result = Some(result);
        Ok(())
    }

    /// Check if the graph has reached a terminal state (no running or ready tasks).
    pub fn is_finished(&self) -> bool {
        let tasks = self.tasks.read();
        tasks.values().all(|t| {
            matches!(
                t.status,
                TaskStatus::Completed
                    | TaskStatus::Failed
                    | TaskStatus::Blocked
                    | TaskStatus::Cancelled
            )
        })
    }

    /// Returns true if all tasks completed successfully.
    pub fn all_succeeded(&self) -> bool {
        let tasks = self.tasks.read();
        !tasks.is_empty() && tasks.values().all(|t| t.status == TaskStatus::Completed)
    }

    /// Cancel all non-terminal tasks.
    pub fn cancel_all(&self) {
        let mut tasks = self.tasks.write();
        for task in tasks.values_mut() {
            if matches!(
                task.status,
                TaskStatus::Pending | TaskStatus::Ready | TaskStatus::Running
            ) {
                task.status = TaskStatus::Cancelled;
            }
        }
    }
}

impl Default for TaskGraph {
    fn default() -> Self {
        Self::new()
    }
}
