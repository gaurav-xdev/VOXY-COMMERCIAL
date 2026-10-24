use crate::error::{Result, SwarmError};
use crate::security::SwarmSecurityBoundary;
use crate::types::AgentRole;
use std::sync::Arc;
use voxy_model_router::ModelRouter;

/// Bridge connecting Trinity & SwarmClaw agents directly to OSMOO's existing
/// ModelRouter and user-configured credentials without any duplicate API keys.
#[derive(Clone)]
pub struct SwarmProviderBridge {
    model_router: Arc<ModelRouter>,
    security: SwarmSecurityBoundary,
}

impl SwarmProviderBridge {
    pub fn new(model_router: Arc<ModelRouter>, security: SwarmSecurityBoundary) -> Self {
        Self {
            model_router,
            security,
        }
    }

    /// Execute an LLM query tailored to a specific Trinity canon agent role.
    pub async fn execute_agent_prompt(
        &self,
        role: AgentRole,
        task_instruction: &str,
        context: &str,
    ) -> Result<String> {
        let system_prompt = role.default_system_prompt();
        let safe_task = self.security.redact_secrets(task_instruction);
        let safe_context = self.security.redact_secrets(context);

        let full_prompt = format!(
            "<system>\n### ROLE SYSTEM PROMPT\n{}\n</system>\n\n<context>\n### SHARED SWARM CONTEXT\n{}\n</context>\n\n<task>\n### TASK ASSIGNMENT\n{}\n\n### INSTRUCTIONS\nProvide a clear, structured response adhering to your role and capabilities.\n</task>",
            system_prompt,
            if safe_context.is_empty() { "No prior context." } else { &safe_context },
            safe_task
        );

        let response = self
            .model_router
            .route_complete(&full_prompt)
            .await
            .map_err(|e| SwarmError::ProviderError(format!("ModelRouter query failed: {e}")))?;

        // Ensure no leaked credentials in agent responses
        let sanitized = self.security.redact_secrets(&response);
        Ok(sanitized)
    }

    /// Retrieve active ModelRouter reference.
    pub fn model_router(&self) -> &Arc<ModelRouter> {
        &self.model_router
    }

    /// Retrieve security boundary.
    pub fn security(&self) -> &SwarmSecurityBoundary {
        &self.security
    }
}
