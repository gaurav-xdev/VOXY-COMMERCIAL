//! Sandboxed Command Execution with Timeout & Emergency Stop Token.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("Execution timed out after {0:?}")]
    Timeout(Duration),

    #[error("Execution aborted by emergency stop")]
    EmergencyStopAborted,

    #[error("Command execution failed: {0}")]
    ProcessFailed(#[from] std::io::Error),

    #[error("Security error: working directory escapes repository boundary")]
    InvalidWorkingDir,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub was_success: bool,
}

pub struct SandboxedRunner {
    repo_root: PathBuf,
    emergency_stop: Arc<AtomicBool>,
}

impl SandboxedRunner {
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
            emergency_stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_emergency_stop(mut self, stop: Arc<AtomicBool>) -> Self {
        self.emergency_stop = stop;
        self
    }

    pub fn emergency_stop_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.emergency_stop)
    }

    /// Triggers an immediate emergency stop, halting current and subsequent executions.
    pub fn trigger_emergency_stop(&self) {
        self.emergency_stop.store(true, Ordering::SeqCst);
    }

    /// Resets the emergency stop state.
    pub fn reset_emergency_stop(&self) {
        self.emergency_stop.store(false, Ordering::SeqCst);
    }

    /// Executes a command in the repository workspace with timeout and stop checks.
    pub async fn run_command(
        &self,
        program: &str,
        args: &[&str],
        relative_cwd: Option<&Path>,
        timeout_duration: Duration,
    ) -> Result<CommandOutput, RunnerError> {
        if self.emergency_stop.load(Ordering::SeqCst) {
            return Err(RunnerError::EmergencyStopAborted);
        }

        let working_dir = match relative_cwd {
            Some(rel) => {
                if rel.is_absolute() {
                    return Err(RunnerError::InvalidWorkingDir);
                }
                for comp in rel.components() {
                    match comp {
                        std::path::Component::Prefix(_)
                        | std::path::Component::RootDir
                        | std::path::Component::ParentDir => {
                            return Err(RunnerError::InvalidWorkingDir);
                        }
                        _ => {}
                    }
                }
                let full = self.repo_root.join(rel);
                if let Ok(canon_full) = full.canonicalize() {
                    if let Ok(canon_root) = self.repo_root.canonicalize() {
                        if !canon_full.starts_with(&canon_root) {
                            return Err(RunnerError::InvalidWorkingDir);
                        }
                    }
                } else if !full.starts_with(&self.repo_root) {
                    return Err(RunnerError::InvalidWorkingDir);
                }
                full
            }
            None => self.repo_root.clone(),
        };

        let start = std::time::Instant::now();

        let mut child = Command::new(program)
            .args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();

        let stdout_task = tokio::spawn(async move {
            let mut out = String::new();
            if let Some(ref mut p) = stdout_pipe {
                let _ = p.read_to_string(&mut out).await;
            }
            out
        });

        let stderr_task = tokio::spawn(async move {
            let mut err = String::new();
            if let Some(ref mut p) = stderr_pipe {
                let _ = p.read_to_string(&mut err).await;
            }
            err
        });

        let stop_token = self.emergency_stop.clone();

        let wait_future = async {
            // Periodic poll check for emergency stop
            loop {
                if stop_token.load(Ordering::SeqCst) {
                    let _ = child.kill().await;
                    return Err(RunnerError::EmergencyStopAborted);
                }

                match child.try_wait() {
                    Ok(Some(status)) => return Ok(status),
                    Ok(None) => tokio::time::sleep(Duration::from_millis(50)).await,
                    Err(e) => return Err(RunnerError::ProcessFailed(e)),
                }
            }
        };

        let status_res = tokio::time::timeout(timeout_duration, wait_future).await;

        let status = match status_res {
            Ok(inner) => inner?,
            Err(_) => {
                let _ = child.kill().await;
                return Err(RunnerError::Timeout(timeout_duration));
            }
        };

        let stdout = stdout_task.await.unwrap_or_default();
        let stderr = stderr_task.await.unwrap_or_default();

        let duration = start.elapsed();

        Ok(CommandOutput {
            exit_code: status.code(),
            stdout,
            stderr,
            duration,
            was_success: status.success(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_harness_emergency_stop() {
        let dir = tempdir().unwrap();
        let runner = SandboxedRunner::new(dir.path());

        // Normal command succeeds
        #[cfg(target_os = "windows")]
        let res = runner
            .run_command(
                "cmd.exe",
                &["/c", "echo Hello Harness"],
                None,
                Duration::from_secs(5),
            )
            .await
            .unwrap();

        #[cfg(not(target_os = "windows"))]
        let res = runner
            .run_command("echo", &["Hello Harness"], None, Duration::from_secs(5))
            .await
            .unwrap();

        assert!(res.was_success);
        assert!(res.stdout.contains("Hello Harness"));

        // Trigger emergency stop
        runner.trigger_emergency_stop();

        #[cfg(target_os = "windows")]
        let blocked = runner
            .run_command(
                "cmd.exe",
                &["/c", "echo after stop"],
                None,
                Duration::from_secs(5),
            )
            .await;

        #[cfg(not(target_os = "windows"))]
        let blocked = runner
            .run_command("echo", &["after stop"], None, Duration::from_secs(5))
            .await;

        assert!(matches!(blocked, Err(RunnerError::EmergencyStopAborted)));

        // Reset and command works again
        runner.reset_emergency_stop();

        #[cfg(target_os = "windows")]
        let unblocked = runner
            .run_command(
                "cmd.exe",
                &["/c", "echo working again"],
                None,
                Duration::from_secs(5),
            )
            .await
            .unwrap();

        assert!(unblocked.was_success);
    }
}
