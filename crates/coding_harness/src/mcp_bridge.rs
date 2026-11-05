use crate::types::{HarnessError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use voxy_tool_calling::{ToolContext, ToolMetadata, ToolRegistry, ToolResult};

/// Adapter that wraps OSMOO's in-process native tool registry (`voxy_tool_calling::ToolRegistry`).
/// Distinct from external MCP protocol servers.
pub struct NativeToolAdapter {
    registry: Arc<ToolRegistry>,
}

impl NativeToolAdapter {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }

    pub async fn list_available_tools(&self) -> Vec<ToolMetadata> {
        self.registry.list_tools().await
    }

    pub async fn call_tool(
        &self,
        tool_name: &str,
        params: Value,
        session_id: &str,
        confirmed: bool,
    ) -> Result<ToolResult> {
        let ctx = ToolContext::new(session_id).with_confirmation(confirmed);
        self.registry
            .execute(tool_name, params, &ctx)
            .await
            .map_err(|e| {
                HarnessError::CommandError(format!(
                    "Native tool execution error for '{}': {}",
                    tool_name, e
                ))
            })
    }
}

/// Backwards-compatible alias for existing code referencing `McpBridge`.
pub type McpBridge = NativeToolAdapter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub version: Option<String>,
    #[serde(rename = "protocolVersion")]
    pub protocol_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContentItem {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCallResult {
    pub content: Vec<McpContentItem>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
}

/// Genuine Model Context Protocol (MCP) client communicating over JSON-RPC 2.0 stdio transport.
/// Connects to real or controlled external MCP servers, supporting initialization handshake,
/// tool discovery (`tools/list`), tool invocation (`tools/call`), protocol error propagation, and bounded timeouts.
pub struct McpStdioClient {
    command: PathBuf,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    timeout_duration: Duration,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    reader: Option<tokio::io::Lines<BufReader<ChildStdout>>>,
    request_id: AtomicU64,
}

impl McpStdioClient {
    pub fn new(
        command: PathBuf,
        args: Vec<String>,
        cwd: Option<&Path>,
        timeout_duration: Duration,
    ) -> Self {
        Self {
            command,
            args,
            cwd: cwd.map(|p| p.to_path_buf()),
            timeout_duration,
            child: None,
            stdin: None,
            reader: None,
            request_id: AtomicU64::new(1),
        }
    }

    /// Spawn the external MCP server process and establish stdio pipes.
    pub fn start(&mut self) -> Result<()> {
        let mut cmd = Command::new(&self.command);
        for arg in &self.args {
            cmd.arg(arg);
        }
        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| {
            HarnessError::CommandError(format!(
                "Failed to spawn MCP server '{}': {}",
                self.command.display(),
                e
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            HarnessError::CommandError("Failed to open stdin for MCP server".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            HarnessError::CommandError("Failed to open stdout for MCP server".into())
        })?;

        let reader = BufReader::new(stdout).lines();

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.reader = Some(reader);

        Ok(())
    }

    async fn send_request_raw(&mut self, request: Value) -> Result<Value> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| HarnessError::CommandError("MCP server stdin is not active".into()))?;
        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| HarnessError::CommandError("MCP server stdout is not active".into()))?;

        let line = serde_json::to_string(&request)
            .map_err(|e| HarnessError::CommandError(format!("JSON serialization error: {}", e)))?;

        stdin
            .write_all(format!("{}\n", line).as_bytes())
            .await
            .map_err(|e| {
                HarnessError::CommandError(format!("Failed to write to MCP stdin: {}", e))
            })?;
        stdin
            .flush()
            .await
            .map_err(|e| HarnessError::CommandError(format!("Failed to flush MCP stdin: {}", e)))?;

        let expected_id = request.get("id").cloned();
        let read_fut = async {
            loop {
                match reader.next_line().await {
                    Ok(Some(resp_line)) => {
                        let trimmed = resp_line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let val: Value = serde_json::from_str(trimmed).map_err(|e| {
                            HarnessError::CommandError(format!(
                                "Malformed JSON-RPC from MCP server: '{}' ({})",
                                trimmed, e
                            ))
                        })?;
                        // Match response by expected id; skip notifications (no id) or mismatched responses
                        if expected_id.is_none() || val.get("id") == expected_id.as_ref() {
                            return Ok(val);
                        }
                    }
                    Ok(None) => {
                        return Err(HarnessError::CommandError(
                            "MCP server closed stdout unexpectedly".into(),
                        ));
                    }
                    Err(e) => {
                        return Err(HarnessError::CommandError(format!(
                            "Error reading MCP stdout: {}",
                            e
                        )));
                    }
                }
            }
        };

        match timeout(self.timeout_duration, read_fut).await {
            Ok(res) => res,
            Err(_) => Err(HarnessError::Timeout(format!(
                "MCP request timed out after {} ms",
                self.timeout_duration.as_millis()
            ))),
        }
    }

    /// Perform MCP protocol initialization handshake:
    /// Sends `initialize` request and receives server capabilities.
    pub async fn initialize(&mut self) -> Result<McpServerInfo> {
        if self.child.is_none() {
            self.start()?;
        }

        let req_id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": {
                    "name": "osmoo-coding-harness",
                    "version": "0.1.0"
                }
            }
        });

        let resp = self.send_request_raw(init_req).await?;

        if let Some(err) = resp.get("error") {
            return Err(HarnessError::CommandError(format!(
                "MCP initialization error: {}",
                err
            )));
        }

        let result = resp.get("result").ok_or_else(|| {
            HarnessError::CommandError("Missing result in initialize response".into())
        })?;

        let server_info_val = result
            .get("serverInfo")
            .cloned()
            .unwrap_or(serde_json::json!({
                "name": "unknown"
            }));

        let name = server_info_val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let version = server_info_val
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let protocol_version = result
            .get("protocolVersion")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Send notifications/initialized notification
        if let Some(ref mut stdin) = self.stdin {
            let notif = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            });
            let _ = stdin.write_all(format!("{}\n", notif).as_bytes()).await;
            let _ = stdin.flush().await;
        }

        Ok(McpServerInfo {
            name,
            version,
            protocol_version,
        })
    }

    /// Query the MCP server for available tools via `tools/list`.
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolDefinition>> {
        let req_id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let list_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/list",
            "params": {}
        });

        let resp = self.send_request_raw(list_req).await?;

        if let Some(err) = resp.get("error") {
            return Err(HarnessError::CommandError(format!(
                "MCP tools/list error: {}",
                err
            )));
        }

        let result = resp.get("result").ok_or_else(|| {
            HarnessError::CommandError("Missing result in tools/list response".into())
        })?;

        let tools_array = result
            .get("tools")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut defs = Vec::new();
        for t in tools_array {
            if let Ok(def) = serde_json::from_value::<McpToolDefinition>(t) {
                defs.push(def);
            }
        }

        Ok(defs)
    }

    /// Call an MCP tool via `tools/call`.
    pub async fn call_tool(&mut self, name: &str, arguments: Value) -> Result<McpToolCallResult> {
        let req_id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let call_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        });

        let resp = self.send_request_raw(call_req).await?;

        if let Some(err) = resp.get("error") {
            return Err(HarnessError::CommandError(format!(
                "MCP tools/call error from server: {}",
                err
            )));
        }

        let result = resp.get("result").ok_or_else(|| {
            HarnessError::CommandError("Missing result in tools/call response".into())
        })?;

        serde_json::from_value::<McpToolCallResult>(result.clone()).map_err(|e| {
            HarnessError::CommandError(format!(
                "Failed to deserialize tools/call result: {} ({})",
                e, result
            ))
        })
    }
}

impl Drop for McpStdioClient {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
    }
}
