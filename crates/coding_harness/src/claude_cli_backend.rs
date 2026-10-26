use crate::process_runner::ProcessRunner;
use crate::types::{CommandExecutionInput, HarnessError, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Safe adapter for delegating coding tasks to an installed Claude Code CLI backend.
/// Designed according to REA findings: non-interactive invocation via `-p`, session retention,
/// and execution within Windows Job Objects to ensure clean child termination.
#[derive(Debug, Clone)]
pub struct ClaudeCliBackend {
    binary_path: PathBuf,
    working_dir: PathBuf,
    session_id: Option<String>,
    timeout: Duration,
}

impl ClaudeCliBackend {
    /// Detect if Claude Code CLI is available on the system PATH or common install locations.
    pub fn detect(working_dir: &Path) -> Option<Self> {
        let candidates = if cfg!(windows) {
            vec!["claude.cmd", "claude.exe", "claude"]
        } else {
            vec!["claude"]
        };

        if let Some(path_var) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_var) {
                for candidate in &candidates {
                    let full = dir.join(candidate);
                    if full.is_file() {
                        return Some(Self {
                            binary_path: full,
                            working_dir: working_dir.to_path_buf(),
                            session_id: None,
                            timeout: Duration::from_secs(120),
                        });
                    }
                }
            }
        }

        // Check common npm global install paths on Windows
        if cfg!(windows) {
            if let Ok(app_data) = std::env::var("APPDATA") {
                let npm_claude = PathBuf::from(app_data).join("npm").join("claude.cmd");
                if npm_claude.exists() {
                    return Some(Self {
                        binary_path: npm_claude,
                        working_dir: working_dir.to_path_buf(),
                        session_id: None,
                        timeout: Duration::from_secs(120),
                    });
                }
            }
        }

        None
    }

    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = Some(session_id);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Query the installed Claude Code CLI version (e.g. "2.1.215 (Claude Code)").
    pub async fn get_version(&self) -> Result<String> {
        let cmd = if cfg!(windows) {
            format!("& \"{}\" --version", self.binary_path.display())
        } else {
            format!("\"{}\" --version", self.binary_path.display())
        };

        let input = CommandExecutionInput {
            command: cmd,
            cwd: Some(self.working_dir.clone()),
            timeout_ms: Some(10_000),
            description: Some("Query Claude Code version".into()),
            run_in_background: false,
        };

        let result = ProcessRunner::run_command(&input, &self.working_dir).await?;
        if result.exit_code == 0 {
            Ok(result.stdout.trim().to_string())
        } else {
            Err(HarnessError::CommandError(format!(
                "Failed to query Claude Code version: {}",
                result.stderr.trim()
            )))
        }
    }

    /// Execute a non-interactive coding task via the Claude Code CLI.
    /// Uses bounded execution and process tree kill on timeout.
    pub async fn execute_task(&self, prompt: &str) -> Result<String> {
        let prefix = if cfg!(windows) { "& " } else { "" };
        let mut cmd_str = format!("{}\"{}\" -p \"{}\"", prefix, self.binary_path.display(), prompt.replace('"', "\\\""));
        if let Some(ref sid) = self.session_id {
            cmd_str.push_str(&format!(" --session-id \"{}\"", sid));
        }

        let input = CommandExecutionInput {
            command: cmd_str,
            cwd: Some(self.working_dir.clone()),
            timeout_ms: Some(self.timeout.as_millis() as u64),
            description: Some(format!("Execute Claude CLI task: {}", prompt)),
            run_in_background: false,
        };

        let result = ProcessRunner::run_command(&input, &self.working_dir).await?;
        if result.exit_code == 0 {
            Ok(result.stdout)
        } else {
            Err(HarnessError::CommandError(format!(
                "Claude CLI task exited with code {}: {}",
                result.exit_code,
                result.stderr.trim()
            )))
        }
    }

    pub fn binary_path(&self) -> &Path {
        &self.binary_path
    }
}
