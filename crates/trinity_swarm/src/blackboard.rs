use crate::types::{AgentId, AgentRole};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// An artifact produced by an agent during swarm execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmArtifact {
    pub id: String,
    pub name: String,
    pub created_by: AgentRole,
    pub path_or_uri: String,
    pub content_preview: String,
    pub created_at: DateTime<Utc>,
}

/// A structured context handoff envelope between two agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffEnvelope {
    pub handoff_id: String,
    pub from_agent: AgentId,
    pub from_role: AgentRole,
    pub to_role: AgentRole,
    pub task_id: String,
    pub summary: String,
    pub artifact_ids: Vec<String>,
    pub instructions: String,
    pub created_at: DateTime<Utc>,
}

/// Shared memory and artifact blackboard for SwarmClaw multi-agent collaboration.
#[derive(Debug, Clone)]
pub struct SwarmBlackboard {
    artifacts: Arc<RwLock<HashMap<String, SwarmArtifact>>>,
    handoffs: Arc<RwLock<Vec<HandoffEnvelope>>>,
    shared_context: Arc<RwLock<HashMap<String, String>>>,
}

impl SwarmBlackboard {
    pub fn new() -> Self {
        Self {
            artifacts: Arc::new(RwLock::new(HashMap::new())),
            handoffs: Arc::new(RwLock::new(Vec::new())),
            shared_context: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an artifact produced by an agent.
    pub fn register_artifact(
        &self,
        name: impl Into<String>,
        created_by: AgentRole,
        path_or_uri: impl Into<String>,
        content_preview: impl Into<String>,
    ) -> String {
        let id = format!("art-{}", uuid::Uuid::new_v4());
        let artifact = SwarmArtifact {
            id: id.clone(),
            name: name.into(),
            created_by,
            path_or_uri: path_or_uri.into(),
            content_preview: content_preview.into(),
            created_at: Utc::now(),
        };
        self.artifacts.write().insert(id.clone(), artifact);
        id
    }

    /// Retrieve an artifact by ID.
    pub fn get_artifact(&self, id: &str) -> Option<SwarmArtifact> {
        self.artifacts.read().get(id).cloned()
    }

    /// Retrieve all registered artifacts.
    pub fn all_artifacts(&self) -> Vec<SwarmArtifact> {
        self.artifacts.read().values().cloned().collect()
    }

    /// Post a structured handoff envelope to the blackboard.
    pub fn post_handoff(&self, handoff: HandoffEnvelope) {
        self.handoffs.write().push(handoff);
    }

    /// Retrieve latest handoffs destined for a specific role.
    pub fn get_handoffs_for_role(&self, role: AgentRole) -> Vec<HandoffEnvelope> {
        self.handoffs
            .read()
            .iter()
            .filter(|h| h.to_role == role)
            .cloned()
            .collect()
    }

    /// Set a key-value pair in shared context.
    pub fn set_context(&self, key: impl Into<String>, value: impl Into<String>) {
        self.shared_context.write().insert(key.into(), value.into());
    }

    /// Get a value from shared context.
    pub fn get_context(&self, key: &str) -> Option<String> {
        self.shared_context.read().get(key).cloned()
    }

    /// Clear all blackboard contents for fresh mission execution.
    pub fn clear(&self) {
        self.artifacts.write().clear();
        self.handoffs.write().clear();
        self.shared_context.write().clear();
    }
}

impl Default for SwarmBlackboard {
    fn default() -> Self {
        Self::new()
    }
}
