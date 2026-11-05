use crate::process_runner::ProcessRunner;
use crate::types::{HarnessError, ProcessExecutionInput, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Safe adapter for delegating coding tasks to an installed Claude Code CLI backend.
/// Designed according to REA findings: non-interactive invocation via `-p`, session retention,
/// and execution within Windows Job Objects to ensure clean child termination.
///
/// Security: Prompt arguments are passed strictly through structured process arguments (CreateProcessW),
/// completely eliminating shell interpolation, metacharacter expansion, and command injection.
#[derive(Debug, Clone)]
pub struct ClaudeCliBackend {
    binary_path: PathBuf,
    working_dir: PathBuf,
    session_id: Option<String>,
    timeout: Duration,
}

impl ClaudeCliBackend {
    /// Create a backend directly pointing to a verified binary path.
    pub fn new(binary_path: &Path, working_dir: &Path) -> Self {
        Self {
            binary_path: binary_path.to_path_buf(),
            working_dir: working_dir.to_path_buf(),
            session_id: None,
            timeout: Duration::from_secs(120),
        }
    }

    /// Detect if Claude Code CLI is available on the system PATH or common install locations.
    pub fn detect(working_dir: &Path) -> Option<Self> {
        // On Windows, prefer direct .exe if available in npm package to avoid cmd wrapper
        if cfg!(windows) {
            if let Ok(app_data) = std::env::var("APPDATA") {
                let direct_exe = PathBuf::from(&app_data)
                    .join("npm")
                    .join("node_modules")
                    .join("@anthropic-ai")
                    .join("claude-code")
                    .join("bin")
                    .join("claude.exe");
                if direct_exe.is_file() {
                    return Some(Self::new(&direct_exe, working_dir));
                }

                let npm_claude = PathBuf::from(app_data).join("npm").join("claude.cmd");
                if npm_claude.is_file() {
                    return Some(Self::new(&npm_claude, working_dir));
                }
            }
        }

        let candidates = if cfg!(windows) {
            vec!["claude.exe", "claude.cmd", "claude"]
        } else {
            vec!["claude"]
        };

        if let Some(path_var) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_var) {
                for candidate in &candidates {
                    let full = dir.join(candidate);
                    if full.is_file() {
                        return Some(Self::new(&full, working_dir));
                    }
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
        let input = ProcessExecutionInput {
            program: self.binary_path.clone(),
            args: vec!["--version".into()],
            cwd: Some(self.working_dir.clone()),
            timeout_ms: Some(10_000),
            description: Some("Query Claude Code version".into()),
            stdin: None,
            require_job_object: true,
        };

        let result = ProcessRunner::run_process(&input, &self.working_dir).await?;
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
    /// Passes prompt arguments directly through CreateProcessW argument structures,
    /// preventing shell expansion or command injection of untrusted user prompts.
    pub async fn execute_task(&self, prompt: &str) -> Result<String> {
        let mut args = vec!["-p".into(), prompt.to_string()];
        if let Some(ref sid) = self.session_id {
            args.push("--session-id".into());
            args.push(sid.clone());
        }

        let input = ProcessExecutionInput {
            program: self.binary_path.clone(),
            args,
            cwd: Some(self.working_dir.clone()),
            timeout_ms: Some(self.timeout.as_millis() as u64),
            description: Some("Execute Claude CLI task safely".into()),
            stdin: None,
            require_job_object: true,
        };

        let result = ProcessRunner::run_process(&input, &self.working_dir).await?;
        if result.exit_code == 0 {
            Ok(result.stdout)
        } else if result.timed_out {
            Err(HarnessError::Timeout(format!(
                "Claude CLI task timed out after {} ms",
                self.timeout.as_millis()
            )))
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

    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }
}
