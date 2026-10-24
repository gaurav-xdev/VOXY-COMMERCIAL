//! Universal Coding Agent Adapter and Execution Harness.
//!
//! Provides a capability-negotiated, provider-neutral execution interface for
//! autonomous coding agents (Claude Code, OpenAI Codex CLI, and native subagents).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::patch_engine::PatchError;
use crate::sandbox_runner::{RunnerError, SandboxedRunner};

#[derive(Error, Debug)]
pub enum AgentAdapterError {
    #[error("Agent CLI binary '{0}' was not found in PATH or configured location")]
    AgentNotInstalled(String),
    #[error("Task execution timed out after {0:?}")]
    Timeout(Duration),
    #[error("Workspace security violation: path '{0}' is outside repository root '{1}'")]
    PathBoundaryViolation(PathBuf, PathBuf),
    #[error("Security violation: patch contains prohibited secret or credential: {0}")]
    SecretDetected(String),
    #[error("Runner error: {0}")]
    Runner(#[from] RunnerError),
    #[error("Patch error: {0}")]
    Patch(#[from] PatchError),
    #[error("Execution failed with code {code:?}: {stderr}")]
    ExecutionFailed {
        code: Option<i32>,
        stderr: String,
        stdout: String,
    },
}

/// Agent execution capabilities reported by discovery.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentCapabilities {
    pub supports_streaming: bool,
    pub supports_dry_run: bool,
    pub supports_isolated_workspace: bool,
    pub supports_cancellation: bool,
    pub supports_native_tool_use: bool,
}

/// Status of an external coding agent CLI on the local system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentAvailability {
    Available {
        version: String,
        executable_path: PathBuf,
    },
    NotInstalled,
    Blocked {
        reason: String,
    },
}

/// A bounded task definition dispatched to a coding agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodingTask {
    pub task_id: String,
    pub prompt: String,
    pub workspace_root: PathBuf,
    pub timeout: Duration,
    pub dry_run: bool,
}

/// Result of an agent execution task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    pub task_id: String,
    pub success: bool,
    pub duration: Duration,
    pub output_summary: String,
    pub exit_code: Option<i32>,
}

/// Universal trait for coding agent adapters.
#[async_trait]
pub trait CodingAgentAdapter: Send + Sync {
    /// Unique identifier for this adapter (e.g. "claude-code", "codex-cli", "antigravity").
    fn agent_name(&self) -> &str;

    /// Probe whether the required CLI or binary is installed and executable.
    fn check_availability(&self) -> AgentAvailability;

    /// Declared capabilities of this adapter.
    fn capabilities(&self) -> AgentCapabilities;

    /// Execute a task within the bounded workspace with least-privilege guarantees.
    async fn execute_task(
        &self,
        task: &CodingTask,
    ) -> Result<TaskExecutionResult, AgentAdapterError>;
}

/// Helper to search the system PATH for a binary name with common extensions.
fn find_in_path(candidates: &[&str]) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        for candidate in candidates {
            let full = dir.join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

/// Adapter for Anthropic Claude Code CLI (`claude`).
pub struct ClaudeCodeAdapter {
    custom_binary_path: Option<PathBuf>,
}

impl ClaudeCodeAdapter {
    pub fn new(custom_binary_path: Option<PathBuf>) -> Self {
        Self { custom_binary_path }
    }

    fn resolve_binary(&self) -> Option<PathBuf> {
        if let Some(ref p) = self.custom_binary_path {
            if p.is_file() {
                return Some(p.clone());
            } else {
                return None;
            }
        }
        find_in_path(&[
            "claude.cmd",
            "claude.exe",
            "claude.bat",
            "claude.ps1",
            "claude",
        ])
    }
}

#[async_trait]
impl CodingAgentAdapter for ClaudeCodeAdapter {
    fn agent_name(&self) -> &str {
        "claude-code"
    }

    fn check_availability(&self) -> AgentAvailability {
        match self.resolve_binary() {
            Some(path) => {
                let output = std::process::Command::new(&path).arg("--version").output();
                match output {
                    Ok(out) if out.status.success() => {
                        let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        AgentAvailability::Available {
                            version,
                            executable_path: path,
                        }
                    }
                    _ => AgentAvailability::Blocked {
                        reason: "Binary exists but failed to respond to --version".to_string(),
                    },
                }
            }
            None => AgentAvailability::NotInstalled,
        }
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supports_streaming: true,
            supports_dry_run: true,
            supports_isolated_workspace: true,
            supports_cancellation: true,
            supports_native_tool_use: true,
        }
    }

    async fn execute_task(
        &self,
        task: &CodingTask,
    ) -> Result<TaskExecutionResult, AgentAdapterError> {
        let binary = self
            .resolve_binary()
            .ok_or_else(|| AgentAdapterError::AgentNotInstalled("claude".to_string()))?;

        if !task.workspace_root.exists() {
            return Err(AgentAdapterError::PathBoundaryViolation(
                task.workspace_root.clone(),
                task.workspace_root.clone(),
            ));
        }

        let start = Instant::now();
        let mut args: Vec<String> = vec!["-p".to_string(), task.prompt.clone()];
        if task.dry_run {
            args.push("--dry-run".to_string());
        }

        let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let runner = SandboxedRunner::new(&task.workspace_root);
        let output = runner
            .run_command(
                binary.to_str().unwrap_or("claude"),
                &str_args,
                None,
                task.timeout,
            )
            .await?;

        let success = output.was_success;
        Ok(TaskExecutionResult {
            task_id: task.task_id.clone(),
            success,
            duration: start.elapsed(),
            output_summary: if success {
                output.stdout
            } else {
                output.stderr
            },
            exit_code: output.exit_code,
        })
    }
}

/// Adapter for OpenAI Codex CLI (`codex`).
pub struct CodexCliAdapter {
    custom_binary_path: Option<PathBuf>,
}

impl CodexCliAdapter {
    pub fn new(custom_binary_path: Option<PathBuf>) -> Self {
        Self { custom_binary_path }
    }

    fn resolve_binary(&self) -> Option<PathBuf> {
        if let Some(ref p) = self.custom_binary_path {
            if p.is_file() {
                return Some(p.clone());
            } else {
                return None;
            }
        }
        find_in_path(&["codex.exe", "codex.cmd", "codex.bat", "codex"])
    }
}

#[async_trait]
impl CodingAgentAdapter for CodexCliAdapter {
    fn agent_name(&self) -> &str {
        "codex-cli"
    }

    fn check_availability(&self) -> AgentAvailability {
        match self.resolve_binary() {
            Some(path) => {
                let output = std::process::Command::new(&path).arg("--version").output();
                match output {
                    Ok(out) if out.status.success() => {
                        let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        AgentAvailability::Available {
                            version,
                            executable_path: path,
                        }
                    }
                    _ => AgentAvailability::Blocked {
                        reason: "Codex CLI found but failed to report version".to_string(),
                    },
                }
            }
            None => AgentAvailability::NotInstalled,
        }
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            supports_streaming: true,
            supports_dry_run: false,
            supports_isolated_workspace: true,
            supports_cancellation: true,
            supports_native_tool_use: false,
        }
    }

    async fn execute_task(
        &self,
        task: &CodingTask,
    ) -> Result<TaskExecutionResult, AgentAdapterError> {
        let binary = self
            .resolve_binary()
            .ok_or_else(|| AgentAdapterError::AgentNotInstalled("codex".to_string()))?;

        if !task.workspace_root.exists() {
            return Err(AgentAdapterError::PathBoundaryViolation(
                task.workspace_root.clone(),
                task.workspace_root.clone(),
            ));
        }

        let start = Instant::now();
        let args = ["exec", &task.prompt];
        let runner = SandboxedRunner::new(&task.workspace_root);
        let output = runner
            .run_command(
                binary.to_str().unwrap_or("codex"),
                &args,
                None,
                task.timeout,
            )
            .await?;

        let success = output.was_success;
        Ok(TaskExecutionResult {
            task_id: task.task_id.clone(),
            success,
            duration: start.elapsed(),
            output_summary: if success {
                output.stdout
            } else {
                output.stderr
            },
            exit_code: output.exit_code,
        })
    }
}

/// Capability negotiation and adapter registry.
pub struct AgentAdapterRegistry {
    adapters: Vec<Arc<dyn CodingAgentAdapter>>,
}

impl AgentAdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register(Arc::new(ClaudeCodeAdapter::new(None)));
        reg.register(Arc::new(CodexCliAdapter::new(None)));
        reg
    }

    pub fn register(&mut self, adapter: Arc<dyn CodingAgentAdapter>) {
        self.adapters.push(adapter);
    }

    pub fn discover_available(&self) -> Vec<(&str, AgentAvailability)> {
        self.adapters
            .iter()
            .map(|a| (a.agent_name(), a.check_availability()))
            .collect()
    }

    pub fn get_adapter(&self, name: &str) -> Option<Arc<dyn CodingAgentAdapter>> {
        self.adapters
            .iter()
            .find(|a| a.agent_name() == name)
            .cloned()
    }
}

impl Default for AgentAdapterRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_adapter_availability_detection() {
        let adapter = ClaudeCodeAdapter::new(None);
        let avail = adapter.check_availability();
        match avail {
            AgentAvailability::Available {
                version,
                executable_path,
            } => {
                assert!(executable_path.exists());
                assert!(!version.is_empty());
            }
            AgentAvailability::NotInstalled => {}
            AgentAvailability::Blocked { reason } => {
                panic!("Adapter blocked: {reason}");
            }
        }
    }

    #[test]
    fn test_codex_adapter_availability_detection() {
        let adapter = CodexCliAdapter::new(None);
        let avail = adapter.check_availability();
        match avail {
            AgentAvailability::Available {
                version,
                executable_path,
            } => {
                assert!(executable_path.exists());
                assert!(!version.is_empty());
            }
            AgentAvailability::NotInstalled => {}
            AgentAvailability::Blocked { reason } => {
                panic!("Adapter blocked: {reason}");
            }
        }
    }

    #[test]
    fn test_uninstalled_adapter_honest_reporting() {
        let nonexistent_adapter =
            ClaudeCodeAdapter::new(Some(PathBuf::from("C:\\fake\\nonexistent_claude.exe")));
        assert_eq!(
            nonexistent_adapter.check_availability(),
            AgentAvailability::NotInstalled
        );
    }

    #[test]
    fn test_adapter_registry_discovery() {
        let registry = AgentAdapterRegistry::with_defaults();
        let discovered = registry.discover_available();
        assert_eq!(discovered.len(), 2);
        assert!(discovered.iter().any(|(name, _)| *name == "claude-code"));
        assert!(discovered.iter().any(|(name, _)| *name == "codex-cli"));
    }
}
