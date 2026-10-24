use crate::types::AgentRole;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Specific capabilities granted to agents based on their Trinity canon role.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmCapability {
    /// Can decompose missions into tasks and manage DAG lifecycle.
    PlanAndSupervise,
    /// Can produce system architecture and interface designs.
    ArchitecturalDesign,
    /// Can execute tools, write code, and produce file artifacts.
    CodeImplementation,
    /// Can perform automated code reviews and Kodus AI audits.
    CodeReview,
    /// Can execute test suites and canary assertions.
    CanaryTesting,
    /// Can trigger context handoffs to other roles.
    ContextHandoff,
}

/// Role definition and permission manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleManifest {
    pub role: AgentRole,
    pub capabilities: HashSet<SwarmCapability>,
    pub max_concurrent_tasks: usize,
}

impl RoleManifest {
    pub fn for_role(role: AgentRole) -> Self {
        let mut caps = HashSet::new();
        caps.insert(SwarmCapability::ContextHandoff);

        let max_tasks = match role {
            AgentRole::Supervisor => {
                caps.insert(SwarmCapability::PlanAndSupervise);
                1
            }
            AgentRole::Architect => {
                caps.insert(SwarmCapability::ArchitecturalDesign);
                2
            }
            AgentRole::SpecialistWorker => {
                caps.insert(SwarmCapability::CodeImplementation);
                4
            }
            AgentRole::Reviewer => {
                caps.insert(SwarmCapability::CodeReview);
                2
            }
            AgentRole::QaTester => {
                caps.insert(SwarmCapability::CanaryTesting);
                2
            }
        };

        Self {
            role,
            capabilities: caps,
            max_concurrent_tasks: max_tasks,
        }
    }

    pub fn has_capability(&self, capability: &SwarmCapability) -> bool {
        self.capabilities.contains(capability)
    }
}
