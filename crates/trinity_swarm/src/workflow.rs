use crate::coordinator::SwarmCoordinator;
use crate::error::Result;
use crate::types::SwarmExecutionReceipt;
use std::sync::Arc;

pub const DEFAULT_MAX_SWARM_LOOPS: usize = 3;

/// Autonomous multi-agent collaboration workflow.
/// Executes Goal -> Supervisor Plan -> Architect Spec -> Specialist Implement -> Review -> QA Canary,
/// with bounded retries if review or canary verification flags issues.
pub struct SwarmCollaborationWorkflow {
    coordinator: Arc<SwarmCoordinator>,
    max_loops: usize,
}

impl SwarmCollaborationWorkflow {
    pub fn new(coordinator: Arc<SwarmCoordinator>) -> Self {
        Self {
            coordinator,
            max_loops: DEFAULT_MAX_SWARM_LOOPS,
        }
    }

    pub fn with_max_loops(mut self, max_loops: usize) -> Self {
        self.max_loops = max_loops.max(1);
        self
    }

    /// Execute the collaborative workflow until goal is achieved or max iterations reached.
    pub async fn run(
        &self,
        goal: &str,
        caller: &str,
        token: &str,
    ) -> Result<SwarmExecutionReceipt> {
        let mut iteration = 0;
        let mut last_receipt = None;

        while iteration < self.max_loops {
            iteration += 1;
            tracing::info!(iteration, goal, "Executing Trinity/SwarmClaw collaboration round");

            let receipt = self.coordinator.execute_mission(goal, caller, token).await?;
            if receipt.success {
                return Ok(receipt);
            }

            last_receipt = Some(receipt);
        }

        // Return final receipt even if not all tasks completed
        last_receipt.ok_or_else(|| {
            crate::error::SwarmError::TaskExecutionFailed {
                task_id: "mission".into(),
                reason: "Swarm loop terminated without receipt".into(),
            }
        })
    }

    pub fn coordinator(&self) -> &Arc<SwarmCoordinator> {
        &self.coordinator
    }
}
