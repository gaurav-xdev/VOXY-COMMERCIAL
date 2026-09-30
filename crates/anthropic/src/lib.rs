pub mod client;

use async_trait::async_trait;
use std::time::Duration;
use tokio::sync::mpsc;

use voxy_provider_core::{LlmChunk, LlmProvider, ProviderError, Result};

use crate::client::AnthropicClient;

const DEFAULT_MODEL: &str = "claude-sonnet-4-20250514";

pub struct AnthropicProvider {
    client: AnthropicClient,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        let timeout = Duration::from_secs(120);
        let model = DEFAULT_MODEL.to_string();
        let client = AnthropicClient::new(
            api_key.into(),
            "https://api.anthropic.com".into(),
            model.clone(),
            timeout,
        );
        Self { client, model }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_base_url(self, base_url: impl Into<String>) -> Self {
        // Rebuild client with new base_url
        let client = AnthropicClient::new(
            self.client.api_key().to_string(),
            base_url.into(),
            self.model.clone(),
            self.client.timeout(),
        );
        Self { client, ..self }
    }

    pub fn with_timeout(self, timeout: Duration) -> Self {
        let client = AnthropicClient::new(
            self.client.api_key().to_string(),
            self.client.base_url().to_string(),
            self.model.clone(),
            timeout,
        );
        Self { client, ..self }
    }

    pub async fn health(&self) -> std::result::Result<bool, ProviderError> {
        self.client.health().await
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, prompt: &str) -> Result<String> {
        self.client.send_message(prompt).await
    }

    async fn complete_streaming(&self, prompt: &str, tx: mpsc::Sender<LlmChunk>) -> Result<()> {
        self.client.chat_completion_streaming(prompt, tx).await
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn health(&self) -> Result<bool> {
        self.health()
            .await
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))
    }

    fn available_models(&self) -> Vec<String> {
        vec![
            "claude-sonnet-4-20250514".into(),
            "claude-3-5-sonnet-20241022".into(),
            "claude-3-5-haiku-20241022".into(),
            "claude-3-opus-20240229".into(),
        ]
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_creation() {
        let provider = AnthropicProvider::new("sk-ant-test");
        assert_eq!(provider.name(), "anthropic");
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_anthropic_with_custom_model() {
        let provider = AnthropicProvider::new("sk-ant-test").with_model("claude-3-opus-20240229");
        assert_eq!(provider.model(), "claude-3-opus-20240229");
    }

    #[test]
    fn test_anthropic_available_models() {
        let provider = AnthropicProvider::new("sk-ant-test");
        let models = provider.available_models();
        assert!(models.contains(&"claude-sonnet-4-20250514".to_string()));
        assert!(models.contains(&"claude-3-5-sonnet-20241022".to_string()));
    }

    #[tokio::test]
    async fn test_anthropic_complete_no_api_key() {
        let provider = AnthropicProvider::new("");
        let result = provider.complete("Hello").await;
        assert!(result.is_err());
    }
}
