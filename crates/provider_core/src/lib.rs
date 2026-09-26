pub mod capability;
pub mod config;
pub mod discovery;
pub mod error;
pub mod health;
pub mod registry;

pub use capability::*;
pub use config::*;
pub use discovery::*;
pub use error::{ProviderError, Result};
pub use health::*;
pub use registry::*;

/// A single token/chunk from a streaming LLM response.
#[derive(Debug, Clone)]
pub struct LlmChunk {
    pub text: String,
    pub done: bool,
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String>;

    /// Streaming completion. Yields text chunks as they are generated.
    /// Default implementation falls back to `complete()` and yields the full response at once.
    async fn complete_streaming(
        &self,
        prompt: &str,
        tx: tokio::sync::mpsc::Sender<LlmChunk>,
    ) -> Result<()> {
        let full = self.complete(prompt).await?;
        let _ = tx.send(LlmChunk { text: full, done: true }).await;
        Ok(())
    }

    /// Whether this provider supports true streaming (token-by-token).
    fn supports_streaming(&self) -> bool {
        false
    }

    async fn health(&self) -> Result<bool> {
        Ok(true)
    }
    fn available_models(&self) -> Vec<String>;
    fn name(&self) -> &str;
}

#[async_trait::async_trait]
pub trait SttProvider: Send + Sync {
    async fn transcribe(&self, audio: &[u8]) -> Result<String>;
    fn supported_languages(&self) -> Vec<String>;
    fn name(&self) -> &str;
}

#[async_trait::async_trait]
pub trait TtsProvider: Send + Sync {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>>;
    fn list_voices(&self) -> Vec<String>;
    fn name(&self) -> &str;
}

#[async_trait::async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str;
}

#[async_trait::async_trait]
pub trait VisionProvider: Send + Sync {
    async fn analyze(&self, image: &[u8]) -> Result<String>;
    fn available_models(&self) -> Vec<String>;
    fn name(&self) -> &str;
}
