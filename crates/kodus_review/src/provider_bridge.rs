use crate::security::KodusSecurityBoundary;
use std::sync::Arc;
use thiserror::Error;
use voxy_model_router::{ModelRouter, RouterError};
use voxy_provider_core::ProviderCapability;

#[derive(Debug, Error)]
pub enum ProviderBridgeError {
    #[error("No LLM provider available in OSMOO router: {0}")]
    NoProviderAvailable(String),

    #[error("Router execution error: {0}")]
    ExecutionFailed(#[from] RouterError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Bridge connecting the Kodus review engine directly to OSMOO's existing configured LLM providers.
/// Reuses all credentials already in the user's settings without prompting or duplicating keys.
#[derive(Clone)]
pub struct KodusProviderBridge {
    model_router: Arc<ModelRouter>,
    security: KodusSecurityBoundary,
}

impl KodusProviderBridge {
    pub fn new(model_router: Arc<ModelRouter>, security: KodusSecurityBoundary) -> Self {
        Self {
            model_router,
            security,
        }
    }

    /// Access the underlying model router.
    pub fn router(&self) -> &Arc<ModelRouter> {
        &self.model_router
    }

    /// Execute a review completion through the user's configured provider with automatic fallback.
    pub async fn complete_review_prompt(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<(String, String), ProviderBridgeError> {
        // Redact any accidental credential leaks from prompts before dispatch
        let safe_system = self.security.redact_secrets(system_prompt);
        let safe_user = self.security.redact_secrets(user_prompt);

        let combined_prompt = format!(
            "<system>\n{}\n</system>\n\n<user>\n{}\n</user>",
            safe_system, safe_user
        );

        // Leverage OSMOO's ModelRouter which handles active provider selection,
        // user settings priority, and circuit-breaker fallback.
        let raw_response = self
            .model_router
            .route_complete(&combined_prompt)
            .await
            .map_err(ProviderBridgeError::ExecutionFailed)?;

        // Ensure review findings are scrubbed of any exposed keys
        let clean_content = self.security.redact_secrets(&raw_response);

        let provider_name = self
            .model_router
            .select_provider(&ProviderCapability::Llm)
            .await
            .map(|p| p.name)
            .unwrap_or_else(|_| "OSMOO-Active-Provider".to_string());

        Ok((clean_content, provider_name))
    }
}
