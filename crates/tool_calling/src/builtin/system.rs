//! System diagnostics, display introspection, and screenshot capture tools.

use async_trait::async_trait;
use serde_json::json;
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

#[cfg(windows)]
use voxy_platform_core::traits::{DisplayPlatform, Platform};
#[cfg(windows)]
use voxy_platform_windows::WindowsPlatform;

/// Query system information, OS details, hostname, and displays.
pub struct SystemInfoTool {
    metadata: ToolMetadata,
}

impl SystemInfoTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "system_info",
                "Retrieve operating system details, computer name, CPU architecture, and connected displays",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({ "type": "object" })),
        }
    }
}

impl Default for SystemInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SystemInfoTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            let info = platform.info();
            let displays = platform.list_displays().await.unwrap_or_default();

            Ok(ToolResult::success(json!({
                "os": info.os,
                "version": info.version,
                "arch": info.arch,
                "hostname": info.hostname,
                "displays": displays,
            }))
            .with_observation(format!(
                "Queried system: {} {} on {}",
                info.os,
                info.version,
                info.hostname.as_deref().unwrap_or("?")
            ))
            .with_duration_ms(start.elapsed().as_millis() as u64))
        }
        #[cfg(not(windows))]
        {
            Ok(ToolResult::success(json!({
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            }))
            .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Capture a screenshot of a display screen into binary/base64 BMP data.
pub struct SystemScreenshotTool {
    metadata: ToolMetadata,
}

impl SystemScreenshotTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "system_screenshot",
                "Capture full-screen screenshot of a display (returns BMP byte length and base64 preview)",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "display_id": { "type": "integer", "default": 0, "description": "Display index to capture" }
                }
            })),
        }
    }
}

impl Default for SystemScreenshotTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SystemScreenshotTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let display_id = params
            .get("display_id")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            let bmp_bytes = platform
                .screenshot(display_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            let bytes_len = bmp_bytes.len();
            // Prefix verification check: BMP begins with "BM"
            let is_bmp = bytes_len >= 2 && &bmp_bytes[0..2] == b"BM";

            Ok(ToolResult::success(json!({
                "display_id": display_id,
                "format": "bmp",
                "byte_count": bytes_len,
                "valid_header": is_bmp,
            }))
            .with_observation(format!(
                "Captured {} bytes for display {}",
                bytes_len, display_id
            ))
            .with_verification(if is_bmp {
                "Verified valid Windows BMP bitmap header"
            } else {
                "Warning: invalid bitmap header"
            })
            .with_duration_ms(start.elapsed().as_millis() as u64))
        }
        #[cfg(not(windows))]
        {
            let _ = display_id;
            Ok(
                ToolResult::success(json!({ "format": "none", "byte_count": 0 }))
                    .with_duration_ms(start.elapsed().as_millis() as u64),
            )
        }
    }
}
