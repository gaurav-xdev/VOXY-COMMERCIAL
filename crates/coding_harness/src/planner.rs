use crate::types::{HarnessError, Result, TaskPlan, TaskStatus, TaskStep};
use chrono::Utc;
use uuid::Uuid;

pub struct TaskPlanner;

impl TaskPlanner {
    pub fn create_plan(title: &str, description: &str, steps: Vec<TaskStep>) -> TaskPlan {
        let now = Utc::now();
        TaskPlan {
            plan_id: Uuid::new_v4(),
            title: title.to_string(),
            description: description.to_string(),
            steps,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update_step_status(
        plan: &mut TaskPlan,
        step_id: &str,
        status: TaskStatus,
        error: Option<String>,
    ) -> Result<()> {
        let step = plan
            .steps
            .iter_mut()
            .find(|s| s.id == step_id)
            .ok_or_else(|| HarnessError::PlanningError(format!("Step ID '{}' not found", step_id)))?;

        step.status = status;
        step.error = error;
        plan.updated_at = Utc::now();
        Ok(())
    }

    pub fn get_next_runnable_steps(plan: &TaskPlan) -> Vec<TaskStep> {
        let completed_ids: std::collections::HashSet<&str> = plan
            .steps
            .iter()
            .filter(|s| s.status == TaskStatus::Completed)
            .map(|s| s.id.as_str())
            .collect();

        plan.steps
            .iter()
            .filter(|s| {
                s.status == TaskStatus::Pending
                    && s.dependencies
                        .iter()
                        .all(|dep| completed_ids.contains(dep.as_str()))
            })
            .cloned()
            .collect()
    }

    pub fn is_complete(plan: &TaskPlan) -> bool {
        plan.steps
            .iter()
            .all(|s| matches!(s.status, TaskStatus::Completed | TaskStatus::Skipped))
    }

    pub fn has_failures(plan: &TaskPlan) -> bool {
        plan.steps.iter().any(|s| s.status == TaskStatus::Failed)
    }
}
