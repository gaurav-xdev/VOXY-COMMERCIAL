use std::time::Duration;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use tokio::sync::mpsc;

use voxy_provider_core::{LlmChunk, ProviderError};

use crate::config::OpenAIConfig;

#[derive(Clone)]
pub struct OpenAIClient {
    client: reqwest::Client,
    config: OpenAIConfig,
}

impl OpenAIClient {
    pub fn new(config: OpenAIConfig) -> Self {
        let mut headers = HeaderMap::new();
        if !config.api_key.is_empty() {
            let auth_value = format!("Bearer {}", *config.api_key);
            if let Ok(val) = HeaderValue::from_str(&auth_value) {
                headers.insert(AUTHORIZATION, val);
            }
        }
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(ref org) = config.organization {
            let org_val = org.to_string();
            if let Ok(val) = HeaderValue::from_str(&org_val) {
                headers.insert("OpenAI-Organization", val);
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .build()
            .unwrap_or_else(|e| {
                tracing::error!("Failed to create HTTP client, using default: {e}");
                reqwest::Client::new()
            });

        Self { client, config }
    }

    pub fn config(&self) -> &OpenAIConfig {
        &self.config
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    pub async fn chat_completion(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tool_choice: Option<serde_json::Value>,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
        });

        if let Some(tc) = tool_choice {
            body["tool_choice"] = tc;
        }

        self.post("/v1/chat/completions", body).await
    }

    pub async fn chat_completion_stream(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tool_choice: Option<serde_json::Value>,
    ) -> Result<Vec<ChatCompletionResponse>, ProviderError> {
        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });

        if let Some(tc) = tool_choice {
            body["tool_choice"] = tc;
        }

        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::RequestFailed("Request timed out".into())
                } else if e.is_connect() {
                    ProviderError::ConnectionFailed(e.to_string())
                } else {
                    ProviderError::RequestFailed(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(self.handle_error_response(status, &text));
        }

        let mut responses = Vec::new();
        let mut stream = response.bytes_stream();
        use futures_util::StreamExt;
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError::RequestFailed(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();
                if line.is_empty() {
                    continue;
                }
                if line == "data: [DONE]" {
                    break;
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    match parse_stream_frame(data) {
                        Ok(Some(content)) => {
                            responses.push(ChatCompletionResponse {
                                id: None,
                                choices: vec![Choice {
                                    index: 0,
                                    message: ResponseMessage {
                                        role: None,
                                        content: Some(content),
                                        tool_calls: vec![],
                                    },
                                    finish_reason: None,
                                }],
                                usage: None,
                                model: None,
                            });
                        }
                        Ok(None) => {}
                        Err(msg) => {
                            return Err(ProviderError::RequestFailed(format!(
                                "Stream error: {msg}"
                            )));
                        }
                    }
                }
            }
        }

        Ok(responses)
    }

    pub async fn chat_completion_streaming(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        tool_choice: Option<serde_json::Value>,
        tx: mpsc::Sender<LlmChunk>,
    ) -> Result<(), ProviderError> {
        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
        });

        if let Some(tc) = tool_choice {
            body["tool_choice"] = tc;
        }

        let url = format!("{}/v1/chat/completions", self.config.base_url);
        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    ProviderError::RequestFailed("Request timed out".into())
                } else if e.is_connect() {
                    ProviderError::ConnectionFailed(e.to_string())
                } else {
                    ProviderError::RequestFailed(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(self.handle_error_response(status, &text));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| ProviderError::RequestFailed(e.to_string()))?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();
                if line.is_empty() {
                    continue;
                }
                if line == "data: [DONE]" {
                    let _ = tx
                        .send(LlmChunk {
                            text: String::new(),
                            done: true,
                        })
                        .await;
                    return Ok(());
                }
                if let Some(data) = line.strip_prefix("data: ") {
                    match parse_stream_frame(data) {
                        Ok(Some(content)) => {
                            let _ = tx
                                .send(LlmChunk {
                                    text: content,
                                    done: false,
                                })
                                .await;
                        }
                        Ok(None) => {}
                        Err(msg) => {
                            let _ = tx
                                .send(LlmChunk {
                                    text: String::new(),
                                    done: true,
                                })
                                .await;
                            return Err(ProviderError::RequestFailed(format!(
                                "Stream error: {msg}"
                            )));
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

    pub async fn embeddings(
        &self,
        model: &str,
        input: &str,
    ) -> Result<EmbeddingResponse, ProviderError> {
        let body = serde_json::json!({
            "model": model,
            "input": input,
        });
        self.post("/v1/embeddings", body).await
    }

    pub async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let resp: ModelsResponse = self.get("/v1/models").await?;
        Ok(resp.data)
    }

    pub async fn health_check(&self) -> Result<bool, ProviderError> {
        let url = format!("{}/v1/models", self.config.base_url);
        match self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r) => Ok(r.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> Result<T, ProviderError> {
        let url = format!("{}{}", self.config.base_url, path);
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            let result = self.client.post(&url).json(&body).send().await;

            match result {
                Ok(r) if r.status().is_success() => {
                    return r
                        .json::<T>()
                        .await
                        .map_err(|e| ProviderError::InvalidResponse(format!("JSON parse: {}", e)));
                }
                Ok(r) if r.status().as_u16() == 429 => {
                    last_error = Some(ProviderError::RateLimited);
                    let delay = 1000u64 * (attempt as u64 + 1);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    continue;
                }
                Ok(r) => {
                    let status = r.status().as_u16();
                    let text = r.text().await.unwrap_or_default();
                    return Err(self.handle_error_response(status, &text));
                }
                Err(e) => {
                    last_error = Some(ProviderError::RequestFailed(e.to_string()));
                    let delay = 200u64 * (attempt as u64 + 1);
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or(ProviderError::RequestFailed("Max retries exceeded".into())))
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, ProviderError> {
        let url = format!("{}{}", self.config.base_url, path);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::ConnectionFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(self.handle_error_response(
                response.status().as_u16(),
                &response.text().await.unwrap_or_default(),
            ));
        }

        response
            .json::<T>()
            .await
            .map_err(|e| ProviderError::InvalidResponse(format!("JSON parse: {}", e)))
    }

    fn handle_error_response(&self, status: u16, body: &str) -> ProviderError {
        match status {
            401 | 403 => ProviderError::AuthenticationFailed(format!("HTTP {}: {}", status, body)),
            429 => ProviderError::RateLimited,
            404 => ProviderError::ModelNotFound(body.to_string()),
            500..=599 => ProviderError::RequestFailed(format!("Server error {}: {}", status, body)),
            _ => ProviderError::RequestFailed(format!("HTTP {}: {}", status, body)),
        }
    }
}

/// Parse a single SSE `data:` frame from a streaming chat completion.
///
/// Returns:
/// - `Ok(Some(text))` when the frame carries delta text content,
/// - `Ok(None)` when the frame is a well-formed chunk with no text,
/// - `Err(message)` when the frame is a stream error object or malformed.
fn parse_stream_frame(data: &str) -> std::result::Result<Option<String>, String> {
    if let Ok(parsed) = serde_json::from_str::<StreamChunkResponse>(data) {
        for choice in parsed.choices {
            if let Some(delta) = choice.delta {
                if let Some(content) = delta.content {
                    if !content.is_empty() {
                        return Ok(Some(content));
                    }
                }
            }
        }
        return Ok(None);
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(data) {
        if let Some(err) = value.get("error") {
            return Err(err.to_string());
        }
    }
    Ok(None)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}
impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: serde_json::Value::String(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: serde_json::Value::String(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: serde_json::Value::String(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user_with_images(text: &str, images: Vec<&str>) -> Self {
        let mut content: Vec<serde_json::Value> =
            vec![serde_json::json!({"type": "text", "text": text})];
        for image_url in images {
            content.push(serde_json::json!({
                "type": "image_url",
                "image_url": { "url": image_url }
            }));
        }
        Self {
            role: "user".into(),
            content: serde_json::Value::Array(content),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChatCompletionResponse {
    pub id: Option<String>,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ResponseMessage {
    pub role: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: Option<u32>,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub owned_by: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EmbeddingResponse {
    pub data: Vec<EmbeddingData>,
    pub model: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct EmbeddingData {
    pub embedding: Vec<f64>,
    pub index: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamChunkResponse {
    pub choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: Option<StreamDelta>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stream_frame_extracts_delta_content() {
        let frame = r#"{"choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}"#;
        let result = parse_stream_frame(frame);
        assert_eq!(result, Ok(Some("Hello".to_string())));
    }

    #[test]
    fn parse_stream_frame_ignores_role_only_chunks() {
        let frame =
            r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#;
        let result = parse_stream_frame(frame);
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn parse_stream_frame_detects_error_object() {
        let frame = r#"{"error":{"message":"Rate limit exceeded","type":"rate_limit_error"}}"#;
        let result = parse_stream_frame(frame);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rate_limit"));
    }

    #[test]
    fn parse_stream_frame_ignores_other_frames() {
        // Lines such as comments, keep-alives, or the [DONE] sentinel carry no delta.
        assert_eq!(parse_stream_frame("[DONE]"), Ok(None));
        assert_eq!(parse_stream_frame(""), Ok(None));
    }
}
