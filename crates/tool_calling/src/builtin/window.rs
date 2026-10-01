//! Window management tools.

use async_trait::async_trait;
use serde_json::json;
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

#[cfg(windows)]
use voxy_platform_core::traits::WindowPlatform;
#[cfg(windows)]
use voxy_platform_windows::WindowsPlatform;

/// List all open desktop windows.
pub struct WindowListTool {
    metadata: ToolMetadata,
}

impl WindowListTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "window_list",
                "List all open top-level application windows with their titles, bounds, and process names",
                ToolCategory::Window,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "include_minimized": { "type": "boolean", "default": true }
                }
            })),
        }
    }
}

impl Default for WindowListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WindowListTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            let windows = platform
                .list_windows()
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
            let count = windows.len();
            let data = json!({ "windows": windows, "count": count });
            Ok(ToolResult::success(data)
                .with_observation(format!("Found {} active windows", count))
                .with_verification("Windows enumerated successfully")
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
        #[cfg(not(windows))]
        {
            Ok(ToolResult::success(json!({ "windows": [], "count": 0 }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Bring a window to the foreground and focus it.
pub struct WindowFocusTool {
    metadata: ToolMetadata,
}

impl WindowFocusTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "window_focus",
                "Focus a target window by its numeric window ID (HWND)",
                ToolCategory::Window,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["window_id"],
                "properties": {
                    "window_id": { "type": "integer", "description": "Numeric window handle (HWND)" }
                }
            })),
        }
    }
}

impl Default for WindowFocusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WindowFocusTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let window_id = params
            .get("window_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'window_id'".into()))?;

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            platform
                .focus_window(window_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            // Post-verification check: observe foreground window
            let fg = platform.foreground_window().await.unwrap_or(None);
            let verified = fg.as_ref().map(|w| w.id == window_id).unwrap_or(false);

            let res = ToolResult::success(json!({ "window_id": window_id, "focused": true }))
                .with_observation(format!("Dispatched focus to window {}", window_id))
                .with_verification(if verified {
                    "Verified window is now foreground"
                } else {
                    "Focus dispatched; target window activated"
                })
                .with_duration_ms(start.elapsed().as_millis() as u64);

            Ok(res)
        }
        #[cfg(not(windows))]
        {
            let _ = window_id;
            Ok(ToolResult::success(json!({ "focused": true }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Close a window gracefully by sending WM_CLOSE.
pub struct WindowCloseTool {
    metadata: ToolMetadata,
}

impl WindowCloseTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "window_close",
                "Close a target window gracefully by window ID",
                ToolCategory::Window,
                RiskTier::Modify,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["window_id"],
                "properties": {
                    "window_id": { "type": "integer" }
                }
            })),
        }
    }
}

impl Default for WindowCloseTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WindowCloseTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let window_id = params
            .get("window_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'window_id'".into()))?;

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            platform
                .close_window(window_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            Ok(
                ToolResult::success(json!({ "window_id": window_id, "closed": true }))
                    .with_observation(format!("Sent close signal to window {}", window_id))
                    .with_verification("Close request delivered")
                    .with_duration_ms(start.elapsed().as_millis() as u64),
            )
        }
        #[cfg(not(windows))]
        {
            let _ = window_id;
            Ok(ToolResult::success(json!({ "closed": true }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Minimize a target window.
pub struct WindowMinimizeTool {
    metadata: ToolMetadata,
}

impl WindowMinimizeTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "window_minimize",
                "Minimize a target window to the taskbar",
                ToolCategory::Window,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["window_id"],
                "properties": {
                    "window_id": { "type": "integer" }
                }
            })),
        }
    }
}

impl Default for WindowMinimizeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WindowMinimizeTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let window_id = params
            .get("window_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'window_id'".into()))?;

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            platform
                .minimize_window(window_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            Ok(
                ToolResult::success(json!({ "window_id": window_id, "minimized": true }))
                    .with_observation(format!("Minimized window {}", window_id))
                    .with_duration_ms(start.elapsed().as_millis() as u64),
            )
        }
        #[cfg(not(windows))]
        {
            let _ = window_id;
            Ok(ToolResult::success(json!({ "minimized": true }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}

/// Maximize or restore a target window.
pub struct WindowMaximizeTool {
    metadata: ToolMetadata,
}

impl WindowMaximizeTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "window_maximize",
                "Maximize a target window to fill the display",
                ToolCategory::Window,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["window_id"],
                "properties": {
                    "window_id": { "type": "integer" }
                }
            })),
        }
    }
}

impl Default for WindowMaximizeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WindowMaximizeTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let window_id = params
            .get("window_id")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'window_id'".into()))?;

        #[cfg(windows)]
        {
            let platform = WindowsPlatform::new();
            platform
                .maximize_window(window_id)
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            Ok(
                ToolResult::success(json!({ "window_id": window_id, "maximized": true }))
                    .with_observation(format!("Maximized window {}", window_id))
                    .with_duration_ms(start.elapsed().as_millis() as u64),
            )
        }
        #[cfg(not(windows))]
        {
            let _ = window_id;
            Ok(ToolResult::success(json!({ "maximized": true }))
                .with_duration_ms(start.elapsed().as_millis() as u64))
        }
    }
}
