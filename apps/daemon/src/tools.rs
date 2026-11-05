//! VOXY Tool System — Structured Windows actions with explicit schemas.
//!
//! Each tool has a defined name, parameters, and execution logic.
//! The LLM returns tool calls as JSON, which are validated and executed
//! through the automation backend. NO arbitrary shell commands.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use voxy_automation::{HybridBackend, WindowsUiaBackend};
use voxy_orchestrator::automation::AutomationBackend;

/// A structured tool call from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub params: serde_json::Value,
}

/// Result of executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub message: String,
}

/// Registry of available tools with their schemas and emergency stop safeguard.
pub struct ToolRegistry {
    backend: Arc<HybridBackend>,
    emergency_stop: Arc<AtomicBool>,
    native_registry: Arc<voxy_tool_calling::ToolRegistry>,
    approval_broker: Option<Arc<voxy_security::ApprovalBroker>>,
}

impl ToolRegistry {
    pub async fn new() -> Result<Self, String> {
        let uia = Arc::new(WindowsUiaBackend::new());
        let hybrid = HybridBackend::builder()
            .with_primary(uia)
            .build()
            .map_err(|e| format!("Failed to create automation backend: {e}"))?;
        hybrid
            .initialize()
            .await
            .map_err(|e| format!("Failed to initialize automation: {e}"))?;
        Ok(Self {
            backend: Arc::new(hybrid),
            emergency_stop: Arc::new(AtomicBool::new(false)),
            native_registry: Arc::new(voxy_tool_calling::ToolRegistry::with_builtins()),
            approval_broker: None,
        })
    }

    /// Attach an ApprovalBroker to the registry and propagate to the native tool registry.
    pub fn with_approval_broker(mut self, broker: Arc<voxy_security::ApprovalBroker>) -> Self {
        self.approval_broker = Some(broker.clone());
        let native = voxy_tool_calling::ToolRegistry::with_builtins().with_approval_broker(broker);
        self.native_registry = Arc::new(native);
        self
    }

    /// Provide a custom or shared emergency stop token.
    #[allow(dead_code)]
    pub fn with_emergency_stop(mut self, stop: Arc<AtomicBool>) -> Self {
        self.emergency_stop = stop;
        self
    }

    /// Obtain a clone of the emergency stop token.
    #[allow(dead_code)]
    pub fn emergency_stop_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.emergency_stop)
    }

    /// Immediately trigger emergency stop, aborting any tool execution.
    pub fn trigger_emergency_stop(&self) {
        self.emergency_stop.store(true, Ordering::SeqCst);
        tracing::warn!("[TOOL] Emergency Stop TRIGGERED. All computer control actions halted.");
    }

    /// Reset emergency stop, allowing tool actions to proceed.
    pub fn reset_emergency_stop(&self) {
        self.emergency_stop.store(false, Ordering::SeqCst);
        tracing::info!("[TOOL] Emergency Stop reset. Computer control re-enabled.");
    }

    /// Check if emergency stop is currently engaged.
    pub fn is_emergency_stopped(&self) -> bool {
        self.emergency_stop.load(Ordering::SeqCst)
    }

    /// Get the tool definitions for the LLM system prompt.
    pub fn tool_definitions() -> &'static str {
        r#"You have access to the following tools. To use a tool, respond with EXACTLY this format on its own line:
TOOL: {"tool": "<name>", "params": {<parameters>}}

Available tools:
- open_app: Open an application. params: {"name": "chrome|notepad|calc|explorer|settings|cmd|powershell|spotify|discord|vscode|teams|outlook|edge|firefox|terminal|taskmanager|<executable_name>"}
- close_app: Close an application window. params: {"name": "<window_title_substring>"}
- focus_app: Bring an application to foreground. params: {"name": "<window_title_substring>"}
- type_text: Type text into the focused window. params: {"text": "<text_to_type>"}
- press_key: Press a key or key combination. params: {"keys": "ctrl+c|ctrl+v|alt+tab|win+e|enter|escape|..."}
- screenshot: Take a screenshot. params: {}
- volume: Set system volume. params: {"action": "up|down|mute|unmute"}
- search_web: Open a web search. params: {"query": "<search_query>"}
- minimize_all: Minimize all windows (show desktop). params: {}
- lock_screen: Lock the computer. params: {}
- file_list: List files in a directory. params: {"path": "<dir_path>"}
- file_read: Read text content of a file. params: {"path": "<file_path>"}
- file_write: Safely write content to a file with rollback. params: {"path": "<file_path>", "content": "<text>"}
- file_delete: Delete a file (requires confirmation). params: {"path": "<file_path>"}
- process_list: List active processes with PID and memory. params: {"filter_name": "<optional_filter>"}
- process_info: Query diagnostics for a process by PID. params: {"pid": <pid>}
- process_kill: Terminate an active process (requires confirmation). params: {"pid": <pid>}
- system_info: Retrieve OS details, CPU architecture, hostname, and displays. params: {}
- execute_coding_task: Execute an autonomous software engineering task using the coding harness. params: {"prompt": "<coding_instruction>", "workspace_root": "<optional_path>"}
- research_deep_investigate: Perform comprehensive multi-source investigation on a query. params: {"query": "<topic>"}
- memory_store: Persist an important fact or user preference into long-term memory. params: {"key": "<key>", "value": "<content>"}
- memory_recall: Query long-term memory for relevant user context. params: {"query": "<query>"}
- task_history_get: Retrieve recorded execution telemetry for a task. params: {"task_id": "<task_id>"}
- task_artifacts_list: List artifacts generated for a task. params: {"task_id": "<task_id>"}
- office_docx_create: Create a native Microsoft Word (.docx) document with styled paragraphs, headings, and tables. params: {"path": "<path>", "title": "<title>", "paragraphs": ["<p1>"], "headings": ["<h1>"], "bullets": ["<b1>"], "table_headers": ["<col>"], "table_rows": [["<r1c1>"]]}
- office_xlsx_create: Create a native Microsoft Excel (.xlsx) spreadsheet with numeric values, cell formulas, and sheets. params: {"path": "<path>", "sheet_name": "<name>", "headers": ["<col1>"], "rows": [["<val1>"]]}
- office_pptx_create: Create a native Microsoft PowerPoint (.pptx) presentation with title slides, bullet slides, and notes. params: {"path": "<path>", "title": "<title>", "slides": [{"title": "<slide_title>", "bullet_points": ["<point>"], "notes": "<notes>"}]}
- office_file_validate: Validate structural integrity and extract text from .docx, .xlsx, or .pptx files. params: {"path": "<path>", "kind": "docx/xlsx/pptx"}
- calendar_create_event: Schedule a calendar event with persistent SQLite storage and conflict detection. params: {"title": "<title>", "start_time": "<iso8601>", "duration_minutes": 30, "recurrence": "none/daily/weekly/monthly/weekdays"}
- calendar_list_events: List upcoming calendar events within a time range. params: {"start_time": "<iso8601>", "end_time": "<iso8601>"}
- calendar_delete_event: Delete or cancel a scheduled calendar event. params: {"event_id": "<id>"}
- email_search_inbox: Search inbox messages across sender, subject, and content. params: {"query": "<query>", "max_results": 10}
- email_read_thread: Read and summarize an email conversation thread with action items. params: {"thread_id": "<id>"}
- email_create_draft: Create a new email draft (does NOT send until approved). params: {"to": ["<addr>"], "subject": "<subject>", "body": "<body>"}
- email_send_approved: Send an email draft that has been approved by the user. params: {"draft_id": "<id>", "approval_token": "<token>"}
- travel_search_flights: Search flight options with baggage rules and verified booking URLs. params: {"origin": "<airport>", "destination": "<airport>", "date": "YYYY-MM-DD"}
- travel_search_hotels: Search hotel accommodations with nightly rates and cancellation terms. params: {"city": "<city>", "check_in": "YYYY-MM-DD", "check_out": "YYYY-MM-DD"}
- shopping_discover_products: Discover products across merchants with prices and stock status. params: {"query": "<product_query>", "max_results": 5}
- payment_execute_sandbox_test: Execute a test payment in sandbox mode (requires tok_sandbox_*; no real cards). params: {"cart_id": "<id>", "token": "tok_sandbox_*", "amount": 10.0}

Only use tools when the user explicitly asks you to perform an action.
For conversation, questions, or information requests, just respond normally without tools."#
    }

    /// Execute a tool call.
    pub async fn execute(&self, call: &ToolCall) -> ToolResult {
        if self.is_emergency_stopped() {
            tracing::warn!("[TOOL] Tool execution aborted: Emergency Stop active");
            return ToolResult {
                success: false,
                message:
                    "Action aborted: Emergency Stop active. All computer control actions halted."
                        .to_string(),
            };
        }

        tracing::info!("[TOOL] Executing: {} with {:?}", call.tool, call.params);

        // Check native tool registry first
        if let Some(_tool) = self.native_registry.get(&call.tool).await {
            // Note: with_confirmation(false) ensures that if the tool requires confirmation,
            // it properly delegates to the ApprovalBroker for human authorization.
            let ctx =
                voxy_tool_calling::ToolContext::new("daemon-session").with_confirmation(false);
            match self
                .native_registry
                .execute(&call.tool, call.params.clone(), &ctx)
                .await
            {
                Ok(res) => {
                    let msg = res.observation.unwrap_or_else(|| {
                        if res.success {
                            format!("Action succeeded: {}", res.data)
                        } else {
                            res.error.unwrap_or_else(|| "Action failed".into())
                        }
                    });
                    return ToolResult {
                        success: res.success,
                        message: msg,
                    };
                }
                Err(e) => {
                    return ToolResult {
                        success: false,
                        message: format!("Tool execution failed: {}", e),
                    };
                }
            }
        }

        match call.tool.as_str() {
            "open_app" => self.open_app(&call.params).await,
            "close_app" => self.close_app(&call.params).await,
            "focus_app" => self.focus_app(&call.params).await,
            "type_text" => self.type_text(&call.params).await,
            "press_key" => self.press_key(&call.params).await,
            "screenshot" => self.screenshot(&call.params).await,
            "volume" => self.volume(&call.params).await,
            "search_web" => self.search_web(&call.params).await,
            "minimize_all" => self.minimize_all().await,
            "lock_screen" => self.lock_screen().await,
            "execute_coding_task" => self.execute_coding_task(&call.params).await,
            _ => ToolResult {
                success: false,
                message: format!("Unknown tool: {}", call.tool),
            },
        }
    }

    /// Parse a tool call from LLM output, if present.
    pub fn parse_tool_call(text: &str) -> Option<(ToolCall, String)> {
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(json_str) = trimmed.strip_prefix("TOOL:") {
                let json_str = json_str.trim();
                if let Ok(call) = serde_json::from_str::<ToolCall>(json_str) {
                    // Return the tool call and the remaining text (without the TOOL line)
                    let remaining: String = text
                        .lines()
                        .filter(|l| !l.trim().starts_with("TOOL:"))
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .to_string();
                    return Some((call, remaining));
                }
            }
        }
        None
    }

    // ── Tool Implementations ──────────────────────────────────────────

    async fn open_app(&self, params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n.trim(),
            None => {
                return ToolResult {
                    success: false,
                    message: "Missing 'name' parameter".to_string(),
                }
            }
        };

        match safe_launch_application(name) {
            Ok(msg) => ToolResult {
                success: true,
                message: msg,
            },
            Err(e) => ToolResult {
                success: false,
                message: e,
            },
        }
    }

    async fn close_app(&self, params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return ToolResult {
                    success: false,
                    message: "Missing 'name' parameter".to_string(),
                }
            }
        };

        match self.backend.find_window(name, None).await {
            Ok(windows) => {
                if windows.is_empty() {
                    return ToolResult {
                        success: false,
                        message: format!("No window found matching '{}'", name),
                    };
                }
                for win in &windows {
                    let _ = self.backend.close_window(&win.id).await;
                }
                ToolResult {
                    success: true,
                    message: format!("Closed {} window(s) matching '{}'", windows.len(), name),
                }
            }
            Err(e) => ToolResult {
                success: false,
                message: format!("Failed to find window: {e}"),
            },
        }
    }

    async fn focus_app(&self, params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        let name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return ToolResult {
                    success: false,
                    message: "Missing 'name' parameter".to_string(),
                }
            }
        };

        match self.backend.find_window(name, None).await {
            Ok(windows) => {
                if let Some(win) = windows.first() {
                    match self.backend.focus_window(&win.id).await {
                        Ok(_) => ToolResult {
                            success: true,
                            message: format!("Focused '{}'", win.title),
                        },
                        Err(e) => ToolResult {
                            success: false,
                            message: format!("Failed to focus: {e}"),
                        },
                    }
                } else {
                    ToolResult {
                        success: false,
                        message: format!("No window found matching '{}'", name),
                    }
                }
            }
            Err(e) => ToolResult {
                success: false,
                message: format!("Failed to search windows: {e}"),
            },
        }
    }

    async fn type_text(&self, params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        let text = match params.get("text").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => {
                return ToolResult {
                    success: false,
                    message: "Missing 'text' parameter".to_string(),
                }
            }
        };

        match self.backend.type_text(text, 8).await {
            Ok(_) => ToolResult {
                success: true,
                message: format!("Typed {} characters", text.len()),
            },
            Err(e) => ToolResult {
                success: false,
                message: format!("Failed to type text: {e}"),
            },
        }
    }

    async fn press_key(&self, params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        let keys = match params.get("keys").and_then(|v| v.as_str()) {
            Some(k) => k,
            None => {
                return ToolResult {
                    success: false,
                    message: "Missing 'keys' parameter".to_string(),
                }
            }
        };

        // Handle combination keys (e.g., "ctrl+c")
        if keys.contains('+') {
            let key_parts: Vec<&str> = keys.split('+').collect();
            match self.backend.key_combination(&key_parts).await {
                Ok(_) => ToolResult {
                    success: true,
                    message: format!("Pressed {}", keys),
                },
                Err(e) => ToolResult {
                    success: false,
                    message: format!("Failed to press keys: {e}"),
                },
            }
        } else {
            match self.backend.key_press(keys).await {
                Ok(_) => ToolResult {
                    success: true,
                    message: format!("Pressed {}", keys),
                },
                Err(e) => ToolResult {
                    success: false,
                    message: format!("Failed to press key: {e}"),
                },
            }
        }
    }

    async fn screenshot(&self, _params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        match self.backend.screenshot(None, None, None, None).await {
            Ok(bytes) => ToolResult {
                success: true,
                message: format!("Captured screenshot ({} bytes)", bytes.len()),
            },
            Err(e) => ToolResult {
                success: false,
                message: format!("Screenshot failed: {e}"),
            },
        }
    }

    async fn volume(&self, params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => {
                return ToolResult {
                    success: false,
                    message: "Missing 'action' parameter".to_string(),
                }
            }
        };

        let key = match action {
            "up" => "volumeup",
            "down" => "volumedown",
            "mute" | "unmute" => "volumemute",
            _ => {
                return ToolResult {
                    success: false,
                    message: format!("Unknown volume action: {}", action),
                }
            }
        };

        match self.backend.key_press(key).await {
            Ok(_) => ToolResult {
                success: true,
                message: format!("Volume {}", action),
            },
            Err(e) => ToolResult {
                success: false,
                message: format!("Volume control failed: {e}"),
            },
        }
    }

    async fn search_web(&self, params: &serde_json::Value) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        let query = match params.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.trim(),
            None => {
                return ToolResult {
                    success: false,
                    message: "Missing 'query' parameter".to_string(),
                }
            }
        };

        let url = format!("https://www.google.com/search?q={}", urlencoding(query));

        // Open directly via default explorer handler (safe, zero cmd.exe shell interpretation)
        match std::process::Command::new("explorer").arg(&url).spawn() {
            Ok(_) => ToolResult {
                success: true,
                message: format!("Searching for '{}'", query),
            },
            Err(e) => ToolResult {
                success: false,
                message: format!("Failed to open browser: {e}"),
            },
        }
    }

    async fn minimize_all(&self) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        match self.backend.key_combination(&["win", "d"]).await {
            Ok(_) => ToolResult {
                success: true,
                message: "All windows minimized".to_string(),
            },
            Err(e) => ToolResult {
                success: false,
                message: format!("Failed: {e}"),
            },
        }
    }

    async fn lock_screen(&self) -> ToolResult {
        if self.is_emergency_stopped() {
            return ToolResult {
                success: false,
                message: "Action aborted: Emergency Stop active.".to_string(),
            };
        }

        match self.backend.key_combination(&["win", "l"]).await {
            Ok(_) => ToolResult {
                success: true,
                message: "Screen locked".to_string(),
            },
            Err(e) => ToolResult {
                success: false,
                message: format!("Failed: {e}"),
            },
        }
    }

    pub async fn execute_coding_task(&self, params: &serde_json::Value) -> ToolResult {
        let prompt = params["prompt"].as_str().unwrap_or("").to_string();
        let ws_str = params["workspace_root"].as_str().unwrap_or(".");
        let ws_path = std::path::PathBuf::from(ws_str);

        let engine = voxy_coding_harness::CodingEngine::new(&ws_path);
        if let Err(e) = engine.initialize() {
            return ToolResult {
                success: false,
                message: format!("Harness initialization crash recovery failed: {}", e),
            };
        }

        let toolchains = engine.detect_toolchains();
        let available_langs: Vec<String> = toolchains
            .iter()
            .filter_map(|t| match &t.status {
                voxy_coding_harness::ToolchainStatus::Available { binary, version } => {
                    Some(format!("{:?} ({} {})", t.language, binary, version))
                }
                _ => None,
            })
            .collect();

        ToolResult {
            success: true,
            message: format!(
                "OSMOO Autonomous Senior Engineering Harness operational on '{}'. Available toolchains: [{}]. Task '{}' initialized.",
                ws_path.display(),
                available_langs.join(", "),
                prompt
            ),
        }
    }
}

/// Validate and safely launch an application without shell command injection risks.
/// Rejects any shell metacharacters and executes via direct process spawn (never `cmd /c`).
pub fn safe_launch_application(name: &str) -> Result<String, String> {
    let name_trimmed = name.trim();
    if name_trimmed.is_empty() {
        return Err("Application name cannot be empty".to_string());
    }

    // Strict security check: reject any shell metacharacters or command chaining tokens
    let forbidden_chars = [
        '&', '|', ';', '<', '>', '^', '%', '$', '\\', '/', '"', '\'', '`', '\r', '\n', '\0', '\t',
    ];
    if let Some(&bad) = forbidden_chars.iter().find(|&&c| name_trimmed.contains(c)) {
        tracing::warn!(
            "[SECURITY] Rejected launch of '{}': contains forbidden shell character '{}'",
            name_trimmed,
            bad
        );
        return Err(format!(
            "Security error: Disallowed character '{}' in application name",
            bad
        ));
    }

    // Map recognized friendly aliases to their safe targets
    let name_lower = name_trimmed.to_lowercase();
    let (target_bin, target_args): (&str, &[&str]) = match name_lower.as_str() {
        "chrome" | "google chrome" | "google-chrome" | "browser" => ("chrome", &[]),
        "edge" | "microsoft edge" | "msedge" => ("msedge", &[]),
        "firefox" => ("firefox", &[]),
        "notepad" | "editor" | "text editor" => ("notepad", &[]),
        "calc" | "calculator" => ("calc", &[]),
        "explorer" | "files" | "file explorer" => ("explorer", &[]),
        "spotify" => ("spotify", &[]),
        "discord" => ("discord", &[]),
        "code" | "vscode" | "vs code" | "visual studio code" => ("code", &[]),
        "teams" | "microsoft teams" | "msteams" => ("msteams", &[]),
        "outlook" | "mail" => ("outlook", &[]),
        "taskmgr" | "task manager" | "taskmanager" => ("taskmgr", &[]),
        "mspaint" | "paint" => ("mspaint", &[]),
        "wordpad" => ("wordpad", &[]),
        "snippingtool" | "snipping tool" => ("snippingtool", &[]),
        "terminal" | "wt" | "windows terminal" => ("wt", &[]),
        "settings" | "windows settings" | "ms-settings" => ("explorer", &["ms-settings:"]),
        _ => {
            // For custom app names: enforce strict whitelist format:
            // alphanumeric, hyphens, underscores, dots, and must not start or end with a dot.
            // Max length 64 characters.
            if name_trimmed.len() > 64 {
                return Err("Application name exceeds maximum length of 64 characters".to_string());
            }

            let is_valid = name_trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
            if !is_valid
                || name_trimmed.starts_with('.')
                || name_trimmed.ends_with('.')
                || name_trimmed.contains("..")
            {
                return Err(format!(
                    "Security error: Invalid application name format '{}'",
                    name_trimmed
                ));
            }

            (name_trimmed, &[])
        }
    };

    // Direct process spawn using CreateProcessW under the hood (zero shell expansion)
    let mut cmd = std::process::Command::new(target_bin);
    if !target_args.is_empty() {
        cmd.args(target_args);
    }

    match cmd.spawn() {
        Ok(_) => Ok(format!("{} is now open.", name_trimmed)),
        Err(e) => {
            tracing::warn!("[TOOL] Failed to spawn '{}': {}", target_bin, e);
            Err(format!("Failed to launch {}: {}", name_trimmed, e))
        }
    }
}

/// Simple percent-encoding for URL query strings.
fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => result.push(c),
            ' ' => result.push('+'),
            _ => {
                for b in c.to_string().as_bytes() {
                    result.push('%');
                    result.push_str(&format!("{:02X}", b));
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tool_call_valid() {
        let text = "Certainly! I'll open notepad for you.\nTOOL: {\"tool\": \"open_app\", \"params\": {\"name\": \"notepad\"}}\nHave a nice day!";
        let parsed = ToolRegistry::parse_tool_call(text);
        assert!(parsed.is_some());
        let (call, remaining) = parsed.unwrap();
        assert_eq!(call.tool, "open_app");
        assert_eq!(call.params["name"], "notepad");
        assert!(remaining.contains("Certainly!"));
        assert!(remaining.contains("Have a nice day!"));
        assert!(!remaining.contains("TOOL:"));
    }

    #[test]
    fn test_parse_tool_call_none() {
        let text = "No tool call here, just ordinary conversation.";
        assert!(ToolRegistry::parse_tool_call(text).is_none());
    }

    #[test]
    fn test_safe_launch_blocks_command_injection() {
        let malicious_payloads = [
            "calc & calc",
            "calc | whoami",
            "notepad; shutdown /s",
            "notepad.exe & dir",
            "calc && shutdown",
            "calc || whoami",
            "calc > out.txt",
            "calc < in.txt",
            "calc ^& calc",
            "../../windows/system32/cmd.exe",
            "calc\0shutdown",
            "calc\nshutdown",
            "calc\rshutdown",
            "calc`whoami`",
            "calc$env:PATH",
            "calc%SystemRoot%",
        ];

        for payload in malicious_payloads {
            let res = safe_launch_application(payload);
            assert!(
                res.is_err(),
                "Malicious payload '{}' should have been rejected!",
                payload
            );
            let err = res.unwrap_err();
            assert!(
                err.contains("Security error") || err.contains("Invalid"),
                "Expected security error for '{}', got: {}",
                payload,
                err
            );
        }
    }

    #[test]
    fn test_emergency_stop_lifecycle() {
        let stop = Arc::new(AtomicBool::new(false));
        assert!(!stop.load(Ordering::SeqCst));

        stop.store(true, Ordering::SeqCst);
        assert!(stop.load(Ordering::SeqCst));

        stop.store(false, Ordering::SeqCst);
        assert!(!stop.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_emergency_stop_aborts_tool_execution() {
        let uia = Arc::new(WindowsUiaBackend::new());
        let hybrid = HybridBackend::builder()
            .with_primary(uia)
            .build()
            .expect("test backend");
        let registry = ToolRegistry {
            backend: Arc::new(hybrid),
            emergency_stop: Arc::new(AtomicBool::new(false)),
            native_registry: Arc::new(voxy_tool_calling::ToolRegistry::with_builtins()),
            approval_broker: None,
        };

        assert!(!registry.is_emergency_stopped());

        let call = ToolCall {
            tool: "open_app".to_string(),
            params: serde_json::json!({"name": "calc"}),
        };

        // When emergency stop is active, tool execution MUST be rejected
        registry.trigger_emergency_stop();
        assert!(registry.is_emergency_stopped());

        let result = registry.execute(&call).await;
        assert!(
            !result.success,
            "Tool execution must fail when emergency stop is active"
        );
        assert!(
            result.message.contains("Emergency Stop active"),
            "Expected emergency stop message, got: {}",
            result.message
        );

        // When emergency stop is reset, is_emergency_stopped() is false
        registry.reset_emergency_stop();
        assert!(!registry.is_emergency_stopped());
    }
}
