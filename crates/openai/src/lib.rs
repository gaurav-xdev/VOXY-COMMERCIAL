pub mod client;
pub mod config;

use async_trait::async_trait;

use voxy_provider_core::{EmbeddingProvider, LlmChunk, LlmProvider, ProviderError, Result};

use self::client::OpenAIClient;
pub use self::config::OpenAIConfig;

pub struct OpenAIProvider {
    client: OpenAIClient,
    model: String,
    models: Vec<String>,
}

impl OpenAIProvider {
    pub fn new(config: OpenAIConfig) -> Self {
        let model = config
            .default_model
            .clone()
            .unwrap_or_else(|| "gpt-4o-mini".into());
        let models = vec![model.clone()];
        Self {
            client: OpenAIClient::new(config),
            model,
            models,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self.models = vec![self.model.clone()];
        self
    }

    pub fn client(&self) -> &OpenAIClient {
        &self.client
    }

    pub async fn refresh_models(&mut self) {
        if let Ok(remote_models) = self.client.list_models().await {
            self.models = remote_models.into_iter().map(|m| m.id).collect();
        }
    }

    pub async fn chat(
        &self,
        messages: Vec<client::ChatMessage>,
    ) -> std::result::Result<String, ProviderError> {
        let resp = self
            .client
            .chat_completion(&self.model, messages, None)
            .await?;
        Ok(resp
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default())
    }

    pub async fn health(&self) -> std::result::Result<bool, ProviderError> {
        self.client.health_check().await
    }
}

#[async_trait]
impl LlmProvider for OpenAIProvider {
    async fn complete(&self, prompt: &str) -> Result<String> {
        let messages = vec![client::ChatMessage::user(prompt)];
        self.chat(messages).await
    }

    async fn complete_streaming(
        &self,
        prompt: &str,
        tx: tokio::sync::mpsc::Sender<LlmChunk>,
    ) -> Result<()> {
        let messages = vec![client::ChatMessage::user(prompt)];
        self.client
            .chat_completion_streaming(&self.model, messages, None, tx)
            .await
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
        self.models.clone()
    }

    fn name(&self) -> &str {
        "openai"
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAIProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let model = if self.model.contains("embedding") {
            &self.model
        } else {
            "text-embedding-3-small"
        };
        let resp = self.client.embeddings(model, text).await?;
        let embedding = resp
            .data
            .first()
            .ok_or_else(|| ProviderError::InvalidResponse("No embedding data".into()))?;
        Ok(embedding.embedding.iter().map(|&v| v as f32).collect())
    }

    fn dimensions(&self) -> usize {
        1536
    }

    fn name(&self) -> &str {
        "openai"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_config_creates() {
        let config = OpenAIConfig::openai("sk-test");
        assert_eq!(*config.api_key, "sk-test");
        assert_eq!(config.base_url, "https://api.openai.com");
    }

    #[test]
    fn test_openai_config_debug_redacts_api_key() {
        let config = OpenAIConfig::openai("sk-super-secret");
        let debug = format!("{:?}", config);
        assert!(
            !debug.contains("sk-super-secret"),
            "Debug must not leak the API key"
        );
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn test_openai_config_groq() {
        let config = OpenAIConfig::groq("gsk-test");
        assert_eq!(config.base_url, "https://api.groq.com/openai");
    }

    #[test]
    fn test_openai_config_openrouter() {
        let config = OpenAIConfig::openrouter("sk-test");
        assert_eq!(config.base_url, "https://openrouter.ai/api");
    }

    #[test]
    fn test_openai_config_deepseek() {
        let config = OpenAIConfig::deepseek("sk-test");
        assert_eq!(config.base_url, "https://api.deepseek.com");
    }

    #[test]
    fn test_openai_config_xai() {
        let config = OpenAIConfig::xai("sk-test");
        assert_eq!(config.base_url, "https://api.x.ai");
    }

    #[test]
    fn test_openai_config_mistral() {
        let config = OpenAIConfig::mistral("sk-test");
        assert_eq!(config.base_url, "https://api.mistral.ai");
    }

    #[test]
    fn test_openai_config_together() {
        let config = OpenAIConfig::together("sk-test");
        assert_eq!(config.base_url, "https://api.together.xyz");
    }

    #[test]
    fn test_openai_config_fireworks() {
        let config = OpenAIConfig::fireworks("sk-test");
        assert_eq!(config.base_url, "https://api.fireworks.ai/inference");
    }

    #[test]
    fn test_openai_config_cerebras() {
        let config = OpenAIConfig::cerebras("sk-test");
        assert_eq!(config.base_url, "https://api.cerebras.ai");
    }

    #[test]
    fn test_openai_config_azure() {
        let config = OpenAIConfig::azure("key", "my-resource", "gpt-4");
        assert_eq!(
            config.base_url,
            "https://my-resource.openai.azure.com/openai/deployments/gpt-4"
        );
    }

    #[test]
    fn test_openai_default_config() {
        let config = OpenAIConfig::default();
        assert_eq!(config.base_url, "http://127.0.0.1:1234");
    }

    #[test]
    fn test_openai_provider_creation() {
        use voxy_provider_core::LlmProvider;
        let config = OpenAIConfig::openai("sk-test");
        let provider = OpenAIProvider::new(config);
        assert_eq!(LlmProvider::name(&provider), "openai");
        assert!(!provider.available_models().is_empty());
    }

    #[tokio::test]
    async fn test_openai_complete_connection_error() {
        let config = OpenAIConfig::new("sk-test", "http://127.0.0.1:1");
        let provider = OpenAIProvider::new(config);
        let result = provider.complete("Hello").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_chat_message_creations() {
        let sys = client::ChatMessage::system("You are helpful");
        assert_eq!(sys.role, "system");
        let user = client::ChatMessage::user("Hi");
        assert_eq!(user.role, "user");
        let assistant = client::ChatMessage::assistant("Hello");
        assert_eq!(assistant.role, "assistant");
    }

    #[test]
    fn test_chat_message_with_images() {
        let msg = client::ChatMessage::user_with_images(
            "Describe this",
            vec!["data:image/png;base64,..."],
        );
        assert_eq!(msg.role, "user");
        if let serde_json::Value::Array(content) = &msg.content {
            assert_eq!(content.len(), 2);
        } else {
            panic!("Expected array content for image messages");
        }
    }
}
