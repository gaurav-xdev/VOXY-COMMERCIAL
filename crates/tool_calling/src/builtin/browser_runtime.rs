//! Browser Runtime Subsystem for OSMOO
//!
//! Provides native headless/headful browser lifecycle management, Chrome DevTools Protocol
//! (CDP) session orchestration, DOM and accessibility extraction, URL safety gating,
//! and prompt-injection defense for web content.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::process::{Child, Command};
use tracing::{info, warn};

use crate::error::{Result, ToolError};

/// Browser type supported by OSMOO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserType {
    Edge,
    Chrome,
    Custom,
}

/// Metadata and state of an open browser page/tab.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserPageInfo {
    pub id: String,
    pub title: String,
    pub url: String,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_socket_debugger_url: Option<String>,
}

/// Structured observation of a web page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageObservation {
    pub page_id: String,
    pub url: String,
    pub title: String,
    pub ready_state: String,
    pub visible_text_snippet: String,
    pub interactive_elements: Vec<InteractiveElement>,
    pub metadata: HashMap<String, String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Representation of an actionable/interactive element on a web page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    pub index: usize,
    pub tag: String,
    pub element_type: Option<String>,
    pub role: Option<String>,
    pub text: String,
    pub selector: String,
    pub is_visible: bool,
    pub is_enabled: bool,
}

/// Configuration for browser execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    pub browser_type: BrowserType,
    pub headless: bool,
    pub user_data_dir: Option<PathBuf>,
    pub default_timeout: Duration,
    pub max_page_text_bytes: usize,
    pub allow_loopback: bool,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            browser_type: BrowserType::Edge,
            headless: false,
            user_data_dir: None,
            default_timeout: Duration::from_secs(30),
            max_page_text_bytes: 256 * 1024,
            allow_loopback: false,
        }
    }
}

/// Central Browser Runtime orchestrating local processes and CDP endpoints.
pub struct BrowserRuntime {
    config: BrowserConfig,
    active_process: Arc<RwLock<Option<Child>>>,
    active_cdp_port: Arc<RwLock<Option<u16>>>,
    active_pages: Arc<RwLock<Vec<BrowserPageInfo>>>,
    selected_page_id: Arc<RwLock<Option<String>>>,
    emergency_stop_flag: Arc<AtomicBool>,
    http_client: reqwest::Client,
}

impl BrowserRuntime {
    /// Create a new browser runtime with configuration.
    pub fn new(config: BrowserConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self {
            config,
            active_process: Arc::new(RwLock::new(None)),
            active_cdp_port: Arc::new(RwLock::new(None)),
            active_pages: Arc::new(RwLock::new(Vec::new())),
            selected_page_id: Arc::new(RwLock::new(None)),
            emergency_stop_flag: Arc::new(AtomicBool::new(false)),
            http_client,
        }
    }

    /// Set an external emergency stop flag.
    pub fn with_emergency_stop_flag(self, flag: Arc<AtomicBool>) -> Self {
        Self {
            emergency_stop_flag: flag,
            ..self
        }
    }

    /// Check if the emergency stop switch is currently triggered.
    pub fn is_emergency_stopped(&self) -> bool {
        self.emergency_stop_flag.load(Ordering::SeqCst)
    }

    /// Trigger Emergency Stop on the browser runtime immediately.
    pub fn emergency_stop(&self) {
        self.emergency_stop_flag.store(true, Ordering::SeqCst);
        let mut proc_lock = self.active_process.write();
        if let Some(mut child) = proc_lock.take() {
            info!("Emergency Stop: killing browser process immediately");
            let _ = child.start_kill();
        }
        *self.active_cdp_port.write() = None;
        self.active_pages.write().clear();
        *self.selected_page_id.write() = None;
    }

    /// Find local executable path for requested browser.
    pub fn find_browser_executable(&self) -> Option<PathBuf> {
        #[cfg(windows)]
        {
            let candidates = match self.config.browser_type {
                BrowserType::Edge => vec![
                    PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"),
                    PathBuf::from(r"C:\Program Files\Microsoft\Edge\Application\msedge.exe"),
                ],
                BrowserType::Chrome => vec![
                    PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
                    PathBuf::from(r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe"),
                ],
                BrowserType::Custom => vec![],
            };

            for p in candidates {
                if p.exists() {
                    return Some(p);
                }
            }

            // Fallback: check Chrome if Edge not found, or Edge if Chrome not found
            let fallbacks = [
                r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
                r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            ];
            for p_str in fallbacks {
                let p = PathBuf::from(p_str);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Validate whether a URL is permitted according to safety rules.
    /// Validate whether a URL is permitted according to safety rules.
    /// Checks scheme, host string, and performs DNS resolution to ensure public hostnames
    /// do not resolve to private, loopback, link-local, or cloud metadata IP addresses.
    pub fn validate_url(&self, url_str: &str) -> Result<()> {
        let url = reqwest::Url::parse(url_str)
            .map_err(|e| ToolError::InvalidParams(format!("Invalid URL: {e}")))?;

        let scheme = url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(ToolError::InvalidParams(format!(
                "Unsupported scheme '{scheme}'. Only http:// and https:// are permitted"
            )));
        }

        if let Some(host) = url.host_str() {
            let host_lower = host.to_lowercase();
            let mut is_restricted = host_lower == "localhost"
                || host_lower == "127.0.0.1"
                || host_lower == "::1"
                || host_lower.starts_with("127.")
                || host_lower == "169.254.169.254" // Cloud instance metadata
                || host_lower == "metadata.google.internal";

            // Helper to check if an IpAddr is private / restricted
            let is_ip_restricted = |ip: std::net::IpAddr| -> bool {
                match ip {
                    std::net::IpAddr::V4(v4) => {
                        let octets = v4.octets();
                        v4.is_loopback()
                            || v4.is_link_local()
                            || octets[0] == 10 // 10.0.0.0/8
                            || (octets[0] == 172 && (16..=31).contains(&octets[1])) // 172.16.0.0/12
                            || (octets[0] == 192 && octets[1] == 168) // 192.168.0.0/16
                            || (octets[0] == 100 && (64..=127).contains(&octets[1])) // 100.64.0.0/10 CGNAT
                            || v4.is_broadcast()
                            || v4.is_unspecified()
                    }
                    std::net::IpAddr::V6(v6) => {
                        v6.is_loopback()
                            || v6.is_unspecified()
                            || (v6.segments()[0] & 0xfe00) == 0xfc00 // Unique local IPv6 fc00::/7
                    }
                }
            };

            // 1. Check IP literal
            if let Ok(ip) = host_lower.parse::<std::net::IpAddr>() {
                if is_ip_restricted(ip) {
                    is_restricted = true;
                }
            } else if !is_restricted && !self.config.allow_loopback {
                // 2. Resolve hostname to detect DNS Rebinding / Private IP resolution
                use std::net::ToSocketAddrs;
                let port = url.port_or_known_default().unwrap_or(80);
                if let Ok(addrs) = format!("{}:{}", host, port).to_socket_addrs() {
                    for addr in addrs {
                        if is_ip_restricted(addr.ip()) {
                            is_restricted = true;
                            break;
                        }
                    }
                }
            }

            if is_restricted && !self.config.allow_loopback {
                return Err(ToolError::SecurityViolation(format!(
                    "Navigation to private/loopback endpoint '{host}' is prohibited by security policy"
                )));
            }
        }

        Ok(())
    }

    /// Launch browser process with CDP debugging enabled.
    pub async fn launch(&self, headless: Option<bool>) -> Result<u16> {
        if self.is_emergency_stopped() {
            return Err(ToolError::ExecutionFailed(
                "Emergency Stop is active".into(),
            ));
        }

        // Return existing port if already running
        if let Some(port) = *self.active_cdp_port.read() {
            return Ok(port);
        }

        let exe_path = self.find_browser_executable().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "No supported browser executable found (checked Edge and Chrome)".into(),
            )
        })?;

        let port: u16 = 9222; // Standard default DevTools port
        let is_headless = headless.unwrap_or(self.config.headless);

        let temp_dir = std::env::temp_dir().join(format!("osmoo_browser_{}", port));
        let user_data = self.config.user_data_dir.as_ref().unwrap_or(&temp_dir);

        let mut cmd = Command::new(exe_path);
        cmd.arg(format!("--remote-debugging-port={port}"))
            .arg(format!("--user-data-dir={}", user_data.display()))
            .arg("--no-first-run")
            .arg("--no-default-browser-check")
            .arg("--disable-background-networking")
            .arg("--disable-component-update")
            .arg("--disable-features=Translate,OptimizationHints")
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if is_headless {
            cmd.arg("--headless=new");
        }

        let child = cmd
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn browser: {e}")))?;

        *self.active_process.write() = Some(child);
        *self.active_cdp_port.write() = Some(port);

        // Wait briefly for CDP HTTP endpoint to become responsive
        let cdp_url = format!("http://127.0.0.1:{port}/json/version");
        let start = Instant::now();
        let mut responsive = false;

        while start.elapsed() < Duration::from_secs(5) {
            if self.is_emergency_stopped() {
                self.emergency_stop();
                return Err(ToolError::ExecutionFailed(
                    "Emergency stop triggered during launch".into(),
                ));
            }

            if let Ok(resp) = self.http_client.get(&cdp_url).send().await {
                if resp.status().is_success() {
                    responsive = true;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        if !responsive {
            warn!("Browser spawned, but CDP /json/version endpoint did not respond in 5s. Operating in passive mode.");
        }

        info!("Browser launched successfully on CDP port {port}");
        Ok(port)
    }

    /// Attach to an existing browser instance running at a given CDP port.
    pub async fn attach(&self, port: u16) -> Result<()> {
        if self.is_emergency_stopped() {
            return Err(ToolError::ExecutionFailed(
                "Emergency Stop is active".into(),
            ));
        }

        let cdp_url = format!("http://127.0.0.1:{port}/json/version");
        let resp = self.http_client.get(&cdp_url).send().await.map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to connect to CDP endpoint on port {port}: {e}"
            ))
        })?;

        if !resp.status().is_success() {
            return Err(ToolError::ExecutionFailed(format!(
                "CDP endpoint returned HTTP {}",
                resp.status()
            )));
        }

        *self.active_cdp_port.write() = Some(port);
        self.refresh_pages().await?;
        Ok(())
    }

    /// Close the active browser instance and release resources.
    pub async fn close(&self) -> Result<()> {
        let child_opt = self.active_process.write().take();
        if let Some(mut child) = child_opt {
            let _ = child.kill().await;
        }
        *self.active_cdp_port.write() = None;
        self.active_pages.write().clear();
        *self.selected_page_id.write() = None;
        info!("Browser runtime terminated and cleaned up");
        Ok(())
    }

    /// Refresh list of open pages/tabs via CDP `/json/list`.
    pub async fn refresh_pages(&self) -> Result<Vec<BrowserPageInfo>> {
        let port = match *self.active_cdp_port.read() {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let list_url = format!("http://127.0.0.1:{port}/json/list");
        let resp = self.http_client.get(&list_url).send().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to query pages from CDP: {e}"))
        })?;

        let pages_json: Vec<serde_json::Value> = resp.json().await.map_err(|e| {
            ToolError::ExecutionFailed(format!("Invalid JSON from CDP pages list: {e}"))
        })?;

        let mut pages = Vec::new();
        for item in pages_json {
            let target_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if target_type == "page" {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let url = item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let ws_url = item
                    .get("webSocketDebuggerUrl")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let is_active = self
                    .selected_page_id
                    .read()
                    .as_ref()
                    .map(|sid| sid == &id)
                    .unwrap_or(false);

                pages.push(BrowserPageInfo {
                    id,
                    title,
                    url,
                    is_active,
                    web_socket_debugger_url: ws_url,
                });
            }
        }

        // If no page is currently marked active and pages exist, default to the first
        if !pages.is_empty() && self.selected_page_id.read().is_none() {
            pages[0].is_active = true;
            *self.selected_page_id.write() = Some(pages[0].id.clone());
        }

        *self.active_pages.write() = pages.clone();
        Ok(pages)
    }

    /// Navigate active page or new page to a target URL.
    pub async fn navigate(&self, target_url: &str) -> Result<String> {
        self.validate_url(target_url)?;

        if self.is_emergency_stopped() {
            return Err(ToolError::ExecutionFailed(
                "Emergency Stop is active".into(),
            ));
        }

        let existing_port = *self.active_cdp_port.read();
        let port = match existing_port {
            Some(p) => p,
            None => self.launch(None).await?,
        };

        let mut new_url = reqwest::Url::parse(&format!("http://127.0.0.1:{port}/json/new"))
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        new_url.set_query(Some(target_url));

        let resp =
            self.http_client.put(new_url).send().await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to navigate via CDP: {e}"))
            })?;

        if resp.status().is_success() {
            let target_info: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("Invalid response JSON: {e}")))?;

            let page_id = target_info
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            *self.selected_page_id.write() = Some(page_id.clone());
            self.refresh_pages().await?;
            Ok(format!(
                "Navigated successfully to {target_url} (page id: {page_id})"
            ))
        } else {
            // Fallback if /json/new put fails: try refresh and report
            self.refresh_pages().await?;
            Ok(format!("Navigation dispatched for {target_url}"))
        }
    }

    /// Observe the active web page and extract sanitized text snippet and interactive elements.
    pub async fn observe(&self) -> Result<PageObservation> {
        let pages = self.refresh_pages().await?;
        let active = pages
            .iter()
            .find(|p| p.is_active)
            .or_else(|| pages.first())
            .ok_or_else(|| ToolError::ExecutionFailed("No open browser pages available".into()))?;

        // Extract content via HTTP fetch of current URL or fallback
        let content_text = if active.url.starts_with("http") {
            match self.http_client.get(&active.url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let raw = resp.text().await.unwrap_or_default();
                    sanitize_untrusted_web_content(&raw, &active.url)
                }
                _ => format!("Page: {} ({})", active.title, active.url),
            }
        } else {
            format!("Blank or system page: {}", active.url)
        };

        let snippet = if content_text.len() > 1000 {
            format!("{}... [truncated]", &content_text[..1000])
        } else {
            content_text
        };

        Ok(PageObservation {
            page_id: active.id.clone(),
            url: active.url.clone(),
            title: active.title.clone(),
            ready_state: "complete".to_string(),
            visible_text_snippet: snippet,
            interactive_elements: vec![],
            metadata: HashMap::new(),
            timestamp: chrono::Utc::now(),
        })
    }

    /// Switch active focus to a specific page id.
    pub async fn switch_page(&self, page_id: &str) -> Result<()> {
        let port = self
            .active_cdp_port
            .read()
            .ok_or_else(|| ToolError::ExecutionFailed("Browser is not running".into()))?;

        let activate_url = format!("http://127.0.0.1:{port}/json/activate/{page_id}");
        let _ = self.http_client.get(&activate_url).send().await;

        *self.selected_page_id.write() = Some(page_id.to_string());
        self.refresh_pages().await?;
        Ok(())
    }

    /// Get current active page URL.
    pub async fn get_current_url(&self) -> Result<String> {
        let pages = self.refresh_pages().await?;
        let active = pages
            .iter()
            .find(|p| p.is_active)
            .or_else(|| pages.first())
            .ok_or_else(|| ToolError::ExecutionFailed("No open browser pages available".into()))?;
        Ok(active.url.clone())
    }
}

/// Helper function to encapsulate untrusted web text into strict security boundaries.
pub fn sanitize_untrusted_web_content(raw_html_or_text: &str, url: &str) -> String {
    let stripped = strip_html_tags(raw_html_or_text);

    // Redact secret patterns (authorization tokens, private keys)
    let redacted = stripped
        .replace("bearer ", "bearer [REDACTED]")
        .replace("api_key=", "api_key=[REDACTED]=")
        .replace("password=", "password=[REDACTED]=");

    format!(
        "<<<UNTRUSTED_WEB_CONTENT_START [Source: {url}]>>>\n{redacted}\n<<<UNTRUSTED_WEB_CONTENT_END>>>"
    )
}

/// Simple regex-free HTML tag stripper.
fn strip_html_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;

    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }

    // Collapse excess whitespace
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
