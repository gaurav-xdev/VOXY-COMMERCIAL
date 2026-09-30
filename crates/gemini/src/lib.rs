use async_trait::async_trait;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

use voxy_provider_core::{LlmChunk, LlmProvider, ProviderError, Result};

const DEFAULT_MODEL: &str = "gemini-2.0-flash";

pub struct GeminiProvider {
    api_key: zeroize::Zeroizing<String>,
    model: String,
    timeout: Duration,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        let timeout = Duration::from_secs(120);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to create HTTP client with custom config, using default");
                reqwest::Client::new()
            });
        Self {
            api_key: zeroize::Zeroizing::new(api_key.into()),
            model: DEFAULT_MODEL.into(),
            timeout,
            client,
        }
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self.client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to create HTTP client with custom timeout, using default");
                reqwest::Client::new()
            });
        self
    }

    fn build_url(&self) -> String {
        format!(
            "https://generativelanguage.googleapis.com/v1/models/{}:generateContent",
            self.model
        )
    }

    async fn send_message(&self, prompt: &str) -> Result<String> {
        let body = serde_json::json!({
            "contents": [
                {
                    "parts": [
                        { "text": prompt }
                    ]
                }
            ]
        });

        let resp = self
            .client
            .post(self.build_url())
            .header("x-goog-api-key", self.api_key.as_str())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::RequestFailed("Gemini request timed out".into())
                } else if e.is_connect() {
                    ProviderError::ConnectionFailed(format!("Cannot connect to Gemini API: {}", e))
                } else {
                    ProviderError::RequestFailed(e.to_string())
                }
            })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(match status.as_u16() {
                401 | 403 => {
                    ProviderError::AuthenticationFailed(format!("HTTP {}: {}", status, text))
                }
                429 => ProviderError::RateLimited,
                404 => ProviderError::ModelNotFound(text),
                _ => ProviderError::RequestFailed(format!("HTTP {}: {}", status, text)),
            });
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        let text = data["candidates"]
            .as_array()
            .and_then(|candidates| candidates.first())
            .and_then(|c| c["content"]["parts"].as_array())
            .and_then(|parts| parts.first())
            .and_then(|part| part["text"].as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                let block_reason = data["promptFeedback"]["blockReason"]
                    .as_str()
                    .map(|r| format!(" (blocked: {})", r))
                    .unwrap_or_default();
                ProviderError::InvalidResponse(format!("No valid response content{}", block_reason))
            })?;

        Ok(text)
    }

    pub async fn chat_completion_streaming(
        &self,
        prompt: &str,
        tx: mpsc::Sender<LlmChunk>,
    ) -> Result<()> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1/models/{}:streamGenerateContent?alt=sse",
            self.model
        );

        let body = serde_json::json!({
            "contents": [
                {
                    "parts": [
                        { "text": prompt }
                    ]
                }
            ]
        });

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", self.api_key.as_str())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::RequestFailed("Gemini streaming request timed out".into())
                } else if e.is_connect() {
                    ProviderError::ConnectionFailed(format!("Cannot connect to Gemini API: {}", e))
                } else {
                    ProviderError::RequestFailed(e.to_string())
                }
            })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(match status.as_u16() {
                401 | 403 => {
                    ProviderError::AuthenticationFailed(format!("HTTP {}: {}", status, text))
                }
                429 => ProviderError::RateLimited,
                404 => ProviderError::ModelNotFound(text),
                _ => ProviderError::RequestFailed(format!("HTTP {}: {}", status, text)),
            });
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result
                .map_err(|e| ProviderError::RequestFailed(format!("Stream read error: {}", e)))?;

            buffer.extend_from_slice(&chunk);

            while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim();

                if let Some(json_str) = line.strip_prefix("data: ") {
                    if json_str == "[DONE]" {
                        let _ = tx
                            .send(LlmChunk {
                                text: String::new(),
                                done: true,
                            })
                            .await;
                        return Ok(());
                    }

                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(json_str) {
                        if let Some(text) = data["candidates"]
                            .as_array()
                            .and_then(|candidates| candidates.first())
                            .and_then(|c| c["content"]["parts"].as_array())
                            .and_then(|parts| parts.first())
                            .and_then(|part| part["text"].as_str())
                        {
                            if !text.is_empty() {
                                let _ = tx
                                    .send(LlmChunk {
                                        text: text.to_string(),
                                        done: false,
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        let _ = tx
            .send(LlmChunk {
                text: String::new(),
                done: true,
            })
            .await;
        Ok(())
    }

    pub async fn health(&self) -> std::result::Result<bool, ProviderError> {
        if self.api_key.is_empty() {
            return Ok(false);
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| ProviderError::ConnectionFailed("Failed to build client".into()))?;

        let url = "https://generativelanguage.googleapis.com/v1/models";
        match client
            .get(url)
            .header("x-goog-api-key", self.api_key.as_str())
            .send()
            .await
        {
            Ok(r) => Ok(r.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    async fn complete(&self, prompt: &str) -> Result<String> {
        self.send_message(prompt).await
    }

    async fn complete_streaming(&self, prompt: &str, tx: mpsc::Sender<LlmChunk>) -> Result<()> {
        self.chat_completion_streaming(prompt, tx).await
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
            "gemini-2.0-flash".into(),
            "gemini-2.0-flash-lite".into(),
            "gemini-2.5-pro-exp-03-25".into(),
            "gemini-1.5-pro".into(),
            "gemini-1.5-flash".into(),
        ]
    }

    fn name(&self) -> &str {
        "gemini"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_provider_creation() {
        let provider = GeminiProvider::new("test-key");
        assert_eq!(provider.name(), "gemini");
        assert_eq!(provider.model(), DEFAULT_MODEL);
    }

    #[test]
    fn test_gemini_with_custom_model() {
        let provider = GeminiProvider::new("test-key").with_model("gemini-1.5-pro");
        assert_eq!(provider.model(), "gemini-1.5-pro");
    }

    #[test]
    fn test_gemini_available_models() {
        let provider = GeminiProvider::new("test-key");
        let models = provider.available_models();
        assert!(models.contains(&"gemini-2.0-flash".to_string()));
    }

    #[tokio::test]
    async fn test_gemini_complete_no_api_key() {
        let provider = GeminiProvider::new("");
        let result = provider.complete("Hello").await;
        assert!(result.is_err());
    }
}
