//! Browser interaction and web inspection tools.

use async_trait::async_trait;
use serde_json::json;
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

/// Open an URL in the user's default browser.
pub struct BrowserOpenTool {
    metadata: ToolMetadata,
}

impl BrowserOpenTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_open",
                "Open a specified URL in the default system browser",
                ToolCategory::Browser,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": { "type": "string", "description": "HTTP or HTTPS URL to launch" }
                }
            })),
        }
    }
}

impl Default for BrowserOpenTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserOpenTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'url'".into()))?;

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ToolError::InvalidParams(
                "Only http:// and https:// URLs are permitted".into(),
            ));
        }

        #[cfg(windows)]
        {
            // Windows ShellExecute
            tokio::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .spawn()
                .map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to open browser: {}", e))
                })?;
        }

        Ok(ToolResult::success(json!({ "url": url, "launched": true }))
            .with_observation(format!("Opened browser with URL '{}'", url))
            .with_verification("Browser launch command dispatched")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Fetch web content (HTML/JSON) from a public URL safely.
pub struct BrowserFetchTool {
    metadata: ToolMetadata,
}

impl BrowserFetchTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_fetch",
                "Fetch web page content from an HTTP/HTTPS URL for inspection",
                ToolCategory::Browser,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": { "type": "string" },
                    "max_bytes": { "type": "integer", "default": 262144 }
                }
            })),
        }
    }
}

impl Default for BrowserFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserFetchTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'url'".into()))?;

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ToolError::InvalidParams(
                "Only http:// and https:// URLs are supported".into(),
            ));
        }

        let max_bytes = params
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .unwrap_or(256 * 1024) as usize;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status().as_u16();
        let body = response.text().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read response body: {}", e))
        })?;

        let truncated = if body.len() > max_bytes {
            &body[..max_bytes]
        } else {
            &body
        };

        Ok(ToolResult::success(json!({
            "url": url,
            "status": status,
            "bytes": truncated.len(),
            "content": truncated,
        }))
        .with_observation(format!(
            "Fetched HTTP {} ({} bytes) from '{}'",
            status,
            truncated.len(),
            url
        ))
        .with_verification(if status == 200 {
            "HTTP 200 OK verified"
        } else {
            "HTTP status returned"
        })
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}
