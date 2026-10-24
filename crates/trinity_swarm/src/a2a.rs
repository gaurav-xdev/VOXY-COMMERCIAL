use crate::types::{AgentId, AgentRole};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use uuid::Uuid;

/// Message types supported by Trinity's Agent-to-Agent protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum A2AMessageType {
    TaskProposal,
    TaskAccepted,
    ProgressUpdate,
    Handoff,
    ArtifactSubmission,
    ReviewRequest,
    ReviewVerdict,
    CanaryCheck,
    FailureAlert,
    TaskCompleted,
}

/// A2A Message Envelope following Trinity specifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    pub message_id: String,
    pub conversation_id: String,
    pub from_agent: AgentId,
    pub from_role: AgentRole,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_agent: Option<AgentId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_role: Option<AgentRole>,
    pub message_type: A2AMessageType,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

impl A2AMessage {
    pub fn new(
        conversation_id: impl Into<String>,
        from_agent: AgentId,
        from_role: AgentRole,
        message_type: A2AMessageType,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            message_id: format!("a2a-{}", Uuid::new_v4()),
            conversation_id: conversation_id.into(),
            from_agent,
            from_role,
            to_agent: None,
            target_role: None,
            message_type,
            payload,
            timestamp: Utc::now(),
        }
    }

    pub fn to_agent(mut self, target: AgentId) -> Self {
        self.to_agent = Some(target);
        self
    }

    pub fn target_role(mut self, role: AgentRole) -> Self {
        self.target_role = Some(role);
        self
    }
}

/// In-memory concurrent message router for Trinity A2A communication.
#[derive(Debug, Clone)]
pub struct A2AMessageBus {
    history: Arc<RwLock<VecDeque<A2AMessage>>>,
    max_history: usize,
}

impl A2AMessageBus {
    pub fn new(max_history: usize) -> Self {
        Self {
            history: Arc::new(RwLock::new(VecDeque::with_capacity(max_history))),
            max_history,
        }
    }

    /// Dispatch a message into the bus.
    pub fn publish(&self, message: A2AMessage) {
        let mut history = self.history.write();
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(message);
    }

    /// Retrieve all messages addressed to a specific agent or role.
    pub fn poll_for_agent(&self, agent_id: &AgentId, role: AgentRole) -> Vec<A2AMessage> {
        let history = self.history.read();
        history
            .iter()
            .filter(|msg| {
                // Direct match on agent ID
                if let Some(target) = &msg.to_agent {
                    if target == agent_id {
                        return true;
                    }
                }
                // Match on target role
                if let Some(target_role) = &msg.target_role {
                    if *target_role == role {
                        return true;
                    }
                }
                // Broadcast messages (no specific target agent or role)
                msg.to_agent.is_none() && msg.target_role.is_none()
            })
            .cloned()
            .collect()
    }

    /// Retrieve full conversation history.
    pub fn get_conversation_history(&self, conversation_id: &str) -> Vec<A2AMessage> {
        let history = self.history.read();
        history
            .iter()
            .filter(|msg| msg.conversation_id == conversation_id)
            .cloned()
            .collect()
    }

    /// Total messages in bus.
    pub fn count(&self) -> usize {
        self.history.read().len()
    }
}

impl Default for A2AMessageBus {
    fn default() -> Self {
        Self::new(1000)
    }
}
