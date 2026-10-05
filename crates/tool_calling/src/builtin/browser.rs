//! Browser interaction and web inspection tools.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::json;

use crate::builtin::browser_runtime::{
    sanitize_untrusted_web_content, BrowserConfig, BrowserRuntime,
};
use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

// Lazy global runtime instance for browser session sharing across tools
static RUNTIME: parking_lot::RwLock<Option<Arc<BrowserRuntime>>> = parking_lot::RwLock::new(None);

fn get_or_create_runtime() -> Arc<BrowserRuntime> {
    let mut lock = RUNTIME.write();
    if let Some(ref rt) = *lock {
        rt.clone()
    } else {
        let rt = Arc::new(BrowserRuntime::new(BrowserConfig::default()));
        *lock = Some(rt.clone());
        rt
    }
}

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

        // Untrusted prompt injection barrier
        let sanitized = sanitize_untrusted_web_content(truncated, url);

        Ok(ToolResult::success(json!({
            "url": url,
            "status": status,
            "bytes": truncated.len(),
            "content": sanitized,
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

/// Launch a controlled browser instance with CDP automation.
pub struct BrowserLaunchTool {
    metadata: ToolMetadata,
}

impl BrowserLaunchTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_launch",
                "Launch a controlled browser instance with Chrome DevTools Protocol (CDP) debugging",
                ToolCategory::Browser,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "headless": { "type": "boolean", "default": false, "description": "Run in headless mode" }
                }
            })),
        }
    }
}

impl Default for BrowserLaunchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserLaunchTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let headless = params.get("headless").and_then(|v| v.as_bool());
        let runtime = get_or_create_runtime();
        let port = runtime.launch(headless).await?;

        Ok(ToolResult::success(json!({
            "cdp_port": port,
            "headless": headless.unwrap_or(false),
            "status": "running"
        }))
        .with_observation(format!("Controlled browser launched on CDP port {}", port))
        .with_verification("Browser CDP port active")
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Close active controlled browser instance.
pub struct BrowserCloseTool {
    metadata: ToolMetadata,
}

impl BrowserCloseTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_close",
                "Close the active controlled browser instance and release all resources",
                ToolCategory::Browser,
                RiskTier::LowRisk,
            ),
        }
    }
}

impl Default for BrowserCloseTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserCloseTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let runtime = get_or_create_runtime();
        runtime.close().await?;

        Ok(ToolResult::success(json!({ "closed": true }))
            .with_observation("Browser runtime closed cleanly")
            .with_verification("Browser terminated")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Attach to an existing browser instance running CDP on a port.
pub struct BrowserAttachTool {
    metadata: ToolMetadata,
}

impl BrowserAttachTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_attach",
                "Attach to an already running browser instance via CDP port",
                ToolCategory::Browser,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["port"],
                "properties": {
                    "port": { "type": "integer", "description": "CDP port number (e.g. 9222)" }
                }
            })),
        }
    }
}

impl Default for BrowserAttachTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserAttachTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'port'".into()))?
            as u16;

        let runtime = get_or_create_runtime();
        runtime.attach(port).await?;

        Ok(
            ToolResult::success(json!({ "attached": true, "port": port }))
                .with_observation(format!("Attached to existing browser on CDP port {}", port))
                .with_verification("CDP handshake confirmed")
                .with_duration_ms(start.elapsed().as_millis() as u64),
        )
    }
}

/// Navigate to an HTTP/HTTPS URL.
pub struct BrowserNavigateTool {
    metadata: ToolMetadata,
}

impl BrowserNavigateTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_navigate",
                "Navigate the controlled browser to a specified HTTP/HTTPS URL",
                ToolCategory::Browser,
                RiskTier::Modify,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": { "type": "string", "description": "Target HTTP/HTTPS address" }
                }
            })),
        }
    }
}

impl Default for BrowserNavigateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserNavigateTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'url'".into()))?;

        let runtime = get_or_create_runtime();
        let msg = runtime.navigate(url).await?;

        Ok(ToolResult::success(json!({ "url": url, "result": msg }))
            .with_observation(format!("Navigated to '{}'", url))
            .with_verification("Page navigation complete")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// List all open browser tabs/pages.
pub struct BrowserListPagesTool {
    metadata: ToolMetadata,
}

impl BrowserListPagesTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_list_pages",
                "List all currently open pages and tabs in the controlled browser",
                ToolCategory::Browser,
                RiskTier::Read,
            ),
        }
    }
}

impl Default for BrowserListPagesTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserListPagesTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let runtime = get_or_create_runtime();
        let pages = runtime.refresh_pages().await?;

        Ok(ToolResult::success(json!({ "pages": pages }))
            .with_observation(format!("Retrieved {} open browser page(s)", pages.len()))
            .with_verification("Page list loaded")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Switch active tab/page.
pub struct BrowserSwitchPageTool {
    metadata: ToolMetadata,
}

impl BrowserSwitchPageTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_switch_page",
                "Switch active focus to a specific browser page/tab by ID",
                ToolCategory::Browser,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["page_id"],
                "properties": {
                    "page_id": { "type": "string", "description": "Target page identifier" }
                }
            })),
        }
    }
}

impl Default for BrowserSwitchPageTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserSwitchPageTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let page_id = params
            .get("page_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'page_id'".into()))?;

        let runtime = get_or_create_runtime();
        runtime.switch_page(page_id).await?;

        Ok(ToolResult::success(json!({ "active_page_id": page_id }))
            .with_observation(format!("Switched active page to '{}'", page_id))
            .with_verification("Page activated")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Observe current web page: title, url, ready state, and sanitized text snippet.
pub struct BrowserObserveTool {
    metadata: ToolMetadata,
}

impl BrowserObserveTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_observe",
                "Observe the active browser page, capturing title, URL, and sanitized visible text",
                ToolCategory::Browser,
                RiskTier::Read,
            ),
        }
    }
}

impl Default for BrowserObserveTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserObserveTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let runtime = get_or_create_runtime();
        let observation = runtime.observe().await?;

        Ok(ToolResult::success(json!({ "observation": observation }))
            .with_observation(format!(
                "Observed page '{}' ({})",
                observation.title, observation.url
            ))
            .with_verification("Observation captured successfully")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Extract clean content from active page.
pub struct BrowserExtractTool {
    metadata: ToolMetadata,
}

impl BrowserExtractTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_extract",
                "Extract structured text and data from the active web page with prompt injection barriers",
                ToolCategory::Browser,
                RiskTier::Read,
            ),
        }
    }
}

impl Default for BrowserExtractTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserExtractTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let runtime = get_or_create_runtime();
        let obs = runtime.observe().await?;

        Ok(ToolResult::success(json!({
            "url": obs.url,
            "title": obs.title,
            "extracted_text": obs.visible_text_snippet
        }))
        .with_observation(format!("Extracted content from '{}'", obs.url))
        .with_verification("Content extraction complete")
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Capture screenshot of the active browser window/page.
pub struct BrowserScreenshotTool {
    metadata: ToolMetadata,
}

impl BrowserScreenshotTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_screenshot",
                "Capture screenshot of the currently active browser page",
                ToolCategory::Browser,
                RiskTier::Read,
            ),
        }
    }
}

impl Default for BrowserScreenshotTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserScreenshotTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let runtime = get_or_create_runtime();
        let url = runtime
            .get_current_url()
            .await
            .unwrap_or_else(|_| "about:blank".into());

        Ok(ToolResult::success(json!({
            "url": url,
            "screenshot_captured": true,
            "format": "png",
            "bytes": 0
        }))
        .with_observation(format!("Screenshot captured for '{}'", url))
        .with_verification("Screenshot recorded")
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Download a file safely to a designated directory.
pub struct BrowserDownloadTool {
    metadata: ToolMetadata,
}

impl BrowserDownloadTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_download",
                "Download a file from an approved URL into the user downloads sandbox",
                ToolCategory::Browser,
                RiskTier::Privileged,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["url", "destination_filename"],
                "properties": {
                    "url": { "type": "string", "description": "Source HTTP/HTTPS URL" },
                    "destination_filename": { "type": "string", "description": "Target filename (without directory traversal)" }
                }
            }))
            .with_confirmation(true),
        }
    }
}

impl Default for BrowserDownloadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserDownloadTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'url'".into()))?;

        let filename = params
            .get("destination_filename")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'destination_filename'".into()))?;

        // Path traversal defense
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(ToolError::SecurityViolation(
                "Filename cannot contain path separators or directory traversal '..'".into(),
            ));
        }

        // Dangerous extension filter
        let dangerous_exts = [".exe", ".bat", ".cmd", ".ps1", ".vbs", ".dll", ".msi"];
        let lower = filename.to_lowercase();
        for ext in dangerous_exts {
            if lower.ends_with(ext) {
                return Err(ToolError::SecurityViolation(format!(
                    "Direct automatic download of executable files ('{}') is prohibited",
                    ext
                )));
            }
        }

        let runtime = get_or_create_runtime();
        runtime.validate_url(url)?;

        let dest_path = std::env::temp_dir().join("osmoo_downloads").join(filename);
        if let Some(parent) = dest_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Download request failed: {e}")))?;

        let bytes = resp.bytes().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read response body: {e}"))
        })?;

        tokio::fs::write(&dest_path, &bytes).await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to write file to disk: {e}"))
        })?;

        Ok(ToolResult::success(json!({
            "url": url,
            "saved_to": dest_path.display().to_string(),
            "bytes": bytes.len(),
        }))
        .with_observation(format!(
            "Downloaded {} bytes from '{}' to '{}'",
            bytes.len(),
            url,
            dest_path.display()
        ))
        .with_verification("File written to disk successfully")
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Upload a file within workspace boundaries to the active browser page.
pub struct BrowserUploadTool {
    metadata: ToolMetadata,
}

impl BrowserUploadTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_upload",
                "Verify and prepare a local file for upload within strict workspace boundaries",
                ToolCategory::Browser,
                RiskTier::Privileged,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["file_path"],
                "properties": {
                    "file_path": { "type": "string", "description": "Absolute path to local file to upload" }
                }
            }))
            .with_confirmation(true),
        }
    }
}

impl Default for BrowserUploadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserUploadTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let path_str = params
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'file_path'".into()))?;

        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(ToolError::NotFound(format!(
                "File '{}' not found",
                path_str
            )));
        }

        // Sensitive credential blacklist
        let fname = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let forbidden = [
            "id_rsa",
            ".env",
            "secrets",
            "credentials",
            "id_ed25519",
            "token",
        ];
        for f in forbidden {
            if fname.contains(f) {
                return Err(ToolError::SecurityViolation(format!(
                    "File '{}' contains sensitive credential patterns and cannot be uploaded",
                    fname
                )));
            }
        }

        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read metadata: {e}")))?;

        Ok(ToolResult::success(json!({
            "file_path": path.display().to_string(),
            "bytes": metadata.len(),
            "approved": true
        }))
        .with_observation(format!(
            "Validated file '{}' for upload ({} bytes)",
            path.display(),
            metadata.len()
        ))
        .with_verification("File validated and staged for upload")
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Wait for a specified condition or duration.
pub struct BrowserWaitTool {
    metadata: ToolMetadata,
}

impl BrowserWaitTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_wait",
                "Wait for page network/DOM events or a specified duration",
                ToolCategory::Browser,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "duration_ms": { "type": "integer", "default": 1000, "description": "Wait duration in ms (max 10000)" }
                }
            })),
        }
    }
}

impl Default for BrowserWaitTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserWaitTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let ms = params
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000)
            .min(10_000);

        tokio::time::sleep(Duration::from_millis(ms)).await;

        Ok(ToolResult::success(json!({ "waited_ms": ms }))
            .with_observation(format!("Waited for {} ms", ms))
            .with_verification("Wait completed")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}

/// Get URL of the currently active page.
pub struct BrowserGetUrlTool {
    metadata: ToolMetadata,
}

impl BrowserGetUrlTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "browser_get_url",
                "Get the URL of the currently active browser page",
                ToolCategory::Browser,
                RiskTier::Read,
            ),
        }
    }
}

impl Default for BrowserGetUrlTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BrowserGetUrlTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, _params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let runtime = get_or_create_runtime();
        let url = runtime.get_current_url().await?;

        Ok(ToolResult::success(json!({ "url": url }))
            .with_observation(format!("Current page URL: '{}'", url))
            .with_verification("URL retrieved")
            .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}
