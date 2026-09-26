use futures_util::StreamExt;
use reqwest::Client;
use tokio::sync::mpsc;
use voxy_provider_core::{LlmChunk, ProviderError, Result};

pub struct AnthropicClient {
    api_key: String,
    base_url: String,
    model: String,
    timeout: std::time::Duration,
    client: Client,
}

impl AnthropicClient {
    pub fn new(
        api_key: String,
        base_url: String,
        model: String,
        timeout: std::time::Duration,
    ) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| {
                tracing::warn!("Failed to create HTTP client with custom config, using default");
                Client::new()
            });
        Self {
            api_key,
            base_url,
            model,
            timeout,
            client,
        }
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn timeout(&self) -> std::time::Duration {
        self.timeout
    }

    fn build_request_body(&self, prompt: &str, stream: bool) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });
        if stream {
            body["stream"] = serde_json::json!(true);
        }
        body
    }

    async fn send_request(&self, body: &serde_json::Value) -> Result<reqwest::Response> {
        let url = format!("{}/v1/messages", self.base_url);
        self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::RequestFailed("Anthropic request timed out".into())
                } else if e.is_connect() {
                    ProviderError::ConnectionFailed(format!(
                        "Cannot connect to Anthropic API at {}",
                        self.base_url
                    ))
                } else {
                    ProviderError::RequestFailed(e.to_string())
                }
            })
    }

    fn handle_error_status(status: reqwest::StatusCode, text: String) -> ProviderError {
        match status.as_u16() {
            401 | 403 => {
                ProviderError::AuthenticationFailed(format!("HTTP {}: {}", status, text))
            }
            429 => ProviderError::RateLimited,
            _ => ProviderError::RequestFailed(format!("HTTP {}: {}", status, text)),
        }
    }

    pub async fn send_message(&self, prompt: &str) -> Result<String> {
        let body = self.build_request_body(prompt, false);
        let resp = self.send_request(&body).await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::handle_error_status(status, text));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        let content = data["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|block| block["text"].as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                ProviderError::InvalidResponse("Missing text content in response".into())
            })?;

        Ok(content)
    }

    pub async fn chat_completion_streaming(
        &self,
        prompt: &str,
        tx: mpsc::Sender<LlmChunk>,
    ) -> Result<()> {
        let body = self.build_request_body(prompt, true);
        let resp = self.send_request(&body).await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(Self::handle_error_status(status, text));
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut current_event_type = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result.map_err(|e| {
                ProviderError::RequestFailed(format!("Stream read error: {}", e))
            })?;

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_pos) = buffer.find('\n') {
                let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                buffer = buffer[newline_pos + 1..].to_string();

                if line.starts_with("event: ") {
                    current_event_type = line[7..].trim().to_string();
                } else if line.starts_with("data: ") {
                    let data_str = line[6..].trim().to_string();
                    self.process_sse_event(&current_event_type, &data_str, &tx)
                        .await?;
                    current_event_type.clear();
                }
            }
        }

        let _ = tx.send(LlmChunk {
            text: String::new(),
            done: true,
        }).await;

        Ok(())
    }

    async fn process_sse_event(
        &self,
        event_type: &str,
        data_str: &str,
        tx: &mpsc::Sender<LlmChunk>,
    ) -> Result<()> {
        if event_type == "content_block_delta" {
            let data: serde_json::Value = serde_json::from_str(data_str)
                .map_err(|e| ProviderError::InvalidResponse(format!("SSE parse error: {}", e)))?;

            if data["delta"]["type"].as_str() == Some("text_delta") {
                if let Some(text) = data["delta"]["text"].as_str() {
                    let _ = tx
                        .send(LlmChunk {
                            text: text.to_string(),
                            done: false,
                        })
                        .await;
                }
            }
        } else if event_type == "message_stop" {
            // Final event — handled by the loop completion
        }

        Ok(())
    }

    pub async fn health(&self) -> std::result::Result<bool, ProviderError> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|_| ProviderError::ConnectionFailed("Failed to build client".into()))?;

        let url = format!("{}/v1/messages", self.base_url);
        match client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": self.model,
                "max_tokens": 1,
                "messages": [{"role": "user", "content": "ping"}]
            }))
            .send()
            .await
        {
            Ok(r) => Ok(is_healthy_status(r.status())),
            Err(_) => Ok(false),
        }
    }
}

/// A provider is considered healthy only when an HTTP probe returns 2xx.
/// Auth failures (401/403), rate limiting (429), and server errors (5xx)
/// must not be reported as healthy.
fn is_healthy_status(status: reqwest::StatusCode) -> bool {
    status.is_success()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_status_is_healthy_only_on_success() {
        assert!(is_healthy_status(reqwest::StatusCode::OK));
        assert!(!is_healthy_status(reqwest::StatusCode::UNAUTHORIZED));
        assert!(!is_healthy_status(reqwest::StatusCode::FORBIDDEN));
        assert!(!is_healthy_status(reqwest::StatusCode::TOO_MANY_REQUESTS));
        assert!(!is_healthy_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR));
        assert!(!is_healthy_status(reqwest::StatusCode::SERVICE_UNAVAILABLE));
    }
}
