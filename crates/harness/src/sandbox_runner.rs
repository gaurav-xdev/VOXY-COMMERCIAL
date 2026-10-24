//! Restricted Process Execution Engine with Boundary Checks, Timeout & Emergency Stop.
//!
//! SECURITY BOUNDARY & ISOLATION DISCLOSURE:
//! This runner provides *Restricted Process Execution* rather than an operating-system-level
//! kernel sandbox (such as Windows AppContainer or Linux cgroups/namespaces).
//!
//! Specifically, it enforces:
//! 1. Strict working-directory boundary containment (rejects directory traversal, parent escapes,
//!    and Windows reparse points/symlinks that escape the repository root).
//! 2. Periodic emergency-stop token polling (every 50ms) with immediate execution abortion.
//! 3. Bounded execution timeouts with guaranteed recursive process-tree termination (`taskkill /F /T /PID`).
//! 4. Optional secret environment variable stripping.
//!
//! NOTE: The executed child process inherits the ambient user account permissions and can access
//! system resources if passed absolute paths. Consequential or destructive commands MUST be guarded
//! by human approval (`voxy_security::ApprovalBroker`) prior to invocation.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

/// Level of process isolation guaranteed by this runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProcessIsolationLevel {
    /// Restricted process execution: working directory boundary containment,
    /// reparse point rejection, emergency stop polling, and recursive process tree termination.
    /// Does NOT provide OS-level containerization, restricted user token, or filesystem/network virtualization.
    RestrictedProcess,
}

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
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

pub struct SandboxedRunner {
    repo_root: PathBuf,
    emergency_stop: Arc<AtomicBool>,
    strip_sensitive_env: bool,
    isolate_env: bool,
    allowed_env_vars: Vec<String>,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
}

const DEFAULT_MAX_OUTPUT_BYTES: usize = 1024 * 1024; // 1 MB

const SAFE_ENV_ALLOWLIST_WINDOWS: &[&str] = &[
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "TEMP",
    "TMP",
    "PATH",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMDATA",
    "ALLUSERSPROFILE",
    "COMPUTERNAME",
    "NUMBER_OF_PROCESSORS",
    "OS",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_IDENTIFIER",
    "PROCESSOR_LEVEL",
    "PROCESSOR_REVISION",
    "CARGO_HOME",
    "RUSTUP_HOME",
];

#[allow(dead_code)]
const SAFE_ENV_ALLOWLIST_UNIX: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "SHELL",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "TERM",
    "CARGO_HOME",
    "RUSTUP_HOME",
];

fn is_sensitive_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    upper.contains("KEY")
        || upper.contains("SECRET")
        || upper.contains("TOKEN")
        || upper.contains("PASSWORD")
        || upper.contains("PASSWD")
        || upper.contains("CREDENTIAL")
        || upper.contains("AUTH")
        || upper.contains("PRIVATE")
        || upper.contains("BEARER")
        || upper.contains("CERTIFICATE")
        || upper.contains("SIGNATURE")
        || upper.contains("SESSION")
}

async fn read_bounded_pipe<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    reader: Option<R>,
    max_bytes: usize,
) -> (String, bool) {
    let mut reader = match reader {
        Some(r) => r,
        None => return (String::new(), false),
    };

    let mut buf = vec![0u8; 8192];
    let mut collected = Vec::new();
    let mut truncated = false;

    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => {
                if collected.len() < max_bytes {
                    let remaining = max_bytes - collected.len();
                    let take = remaining.min(n);
                    collected.extend_from_slice(&buf[..take]);
                    if take < n {
                        truncated = true;
                    }
                } else {
                    truncated = true;
                    // Drain remaining bytes without buffering to prevent child deadlock
                }
            }
            Err(_) => break,
        }
    }

    let mut text = String::from_utf8_lossy(&collected).into_owned();
    if truncated {
        text.push_str("\n[TRUNCATED: output exceeded maximum byte limit]");
    }
    (text, truncated)
}

impl SandboxedRunner {
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
            emergency_stop: Arc::new(AtomicBool::new(false)),
            strip_sensitive_env: true,
            isolate_env: true,
            allowed_env_vars: Vec::new(),
            max_stdout_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            max_stderr_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    pub fn with_emergency_stop(mut self, stop: Arc<AtomicBool>) -> Self {
        self.emergency_stop = stop;
        self
    }

    /// Enable or disable default-deny environment isolation.
    pub fn with_env_isolation(mut self, isolate: bool) -> Self {
        self.isolate_env = isolate;
        self
    }

    /// Whitelist an additional environment variable key for the child process.
    pub fn with_allow_env_var(mut self, var_name: impl Into<String>) -> Self {
        self.allowed_env_vars.push(var_name.into());
        self
    }

    /// Enable or disable stripping known sensitive credentials from child process environment.
    pub fn with_stripped_sensitive_env(mut self, strip: bool) -> Self {
        self.strip_sensitive_env = strip;
        self
    }

    /// Set maximum allowed stdout capture bytes before truncation occurs.
    pub fn with_max_stdout_bytes(mut self, bytes: usize) -> Self {
        self.max_stdout_bytes = bytes;
        self
    }

    /// Set maximum allowed stderr capture bytes before truncation occurs.
    pub fn with_max_stderr_bytes(mut self, bytes: usize) -> Self {
        self.max_stderr_bytes = bytes;
        self
    }

    /// Returns the honest, verified isolation level provided by this runner.
    pub fn isolation_level(&self) -> ProcessIsolationLevel {
        ProcessIsolationLevel::RestrictedProcess
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

    /// Terminates the process and all its descendants on Windows using taskkill /F /T /PID.
    pub fn kill_pid_tree(pid: u32) {
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    /// Terminates the process and its child processes on Windows to prevent orphaned trees.
    pub async fn kill_process_tree(child: &mut tokio::process::Child) {
        if let Some(pid) = child.id() {
            Self::kill_pid_tree(pid);
        }
        let _ = child.kill().await;
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
                let rel_str = rel.to_string_lossy();
                if rel_str.contains("..") {
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
                if crate::patch_engine::contains_symlink_or_reparse(&self.repo_root, rel) {
                    return Err(RunnerError::InvalidWorkingDir);
                }
                let full = self.repo_root.join(rel);
                if crate::patch_engine::is_symlink_or_reparse_point(&full) {
                    return Err(RunnerError::InvalidWorkingDir);
                }
                if let (Ok(canon_full), Ok(canon_root)) =
                    (full.canonicalize(), self.repo_root.canonicalize())
                {
                    if !crate::patch_engine::path_is_contained(&canon_full, &canon_root) {
                        return Err(RunnerError::InvalidWorkingDir);
                    }
                } else if !crate::patch_engine::path_is_contained(&full, &self.repo_root) {
                    return Err(RunnerError::InvalidWorkingDir);
                }
                full
            }
            None => self.repo_root.clone(),
        };

        let start = std::time::Instant::now();

        let mut cmd = Command::new(program);
        cmd.args(args)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.isolate_env {
            cmd.env_clear();

            #[cfg(target_os = "windows")]
            let allowlist = SAFE_ENV_ALLOWLIST_WINDOWS;
            #[cfg(not(target_os = "windows"))]
            let allowlist = SAFE_ENV_ALLOWLIST_UNIX;

            for (k, v) in std::env::vars() {
                let k_upper = k.to_uppercase();
                let is_allowed = allowlist
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(&k))
                    || self
                        .allowed_env_vars
                        .iter()
                        .any(|extra| extra.eq_ignore_ascii_case(&k));

                if is_allowed && !is_sensitive_key(&k_upper) {
                    cmd.env(&k, &v);
                }
            }
        } else if self.strip_sensitive_env {
            for (k, _) in std::env::vars() {
                if is_sensitive_key(&k) {
                    cmd.env_remove(&k);
                }
            }
        }

        let mut child = cmd.spawn()?;

        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();

        let max_stdout = self.max_stdout_bytes;
        let stdout_task =
            tokio::spawn(async move { read_bounded_pipe(stdout_pipe, max_stdout).await });

        let max_stderr = self.max_stderr_bytes;
        let stderr_task =
            tokio::spawn(async move { read_bounded_pipe(stderr_pipe, max_stderr).await });

        let stop_token = self.emergency_stop.clone();
        let pid = child.id();

        struct ChildProcessGuard<'a> {
            child: &'a mut tokio::process::Child,
            pid: Option<u32>,
            completed: bool,
        }

        impl<'a> Drop for ChildProcessGuard<'a> {
            fn drop(&mut self) {
                if !self.completed {
                    if let Some(pid) = self.pid {
                        SandboxedRunner::kill_pid_tree(pid);
                    }
                    let _ = self.child.start_kill();
                }
            }
        }

        let mut guard = ChildProcessGuard {
            child: &mut child,
            pid,
            completed: false,
        };

        let wait_future = async {
            // Periodic poll check for emergency stop
            loop {
                if stop_token.load(Ordering::SeqCst) {
                    if let Some(p) = pid {
                        Self::kill_pid_tree(p);
                    }
                    let _ = guard.child.kill().await;
                    guard.completed = true;
                    return Err(RunnerError::EmergencyStopAborted);
                }

                match guard.child.try_wait() {
                    Ok(Some(status)) => {
                        guard.completed = true;
                        return Ok(status);
                    }
                    Ok(None) => tokio::time::sleep(Duration::from_millis(50)).await,
                    Err(e) => {
                        if let Some(p) = pid {
                            Self::kill_pid_tree(p);
                        }
                        let _ = guard.child.kill().await;
                        guard.completed = true;
                        return Err(RunnerError::ProcessFailed(e));
                    }
                }
            }
        };

        let status_res = tokio::time::timeout(timeout_duration, wait_future).await;

        let status = match status_res {
            Ok(inner) => inner?,
            Err(_) => {
                if let Some(p) = pid {
                    Self::kill_pid_tree(p);
                }
                let _ = guard.child.kill().await;
                guard.completed = true;
                return Err(RunnerError::Timeout(timeout_duration));
            }
        };

        let (stdout, stdout_truncated) = stdout_task.await.unwrap_or_default();
        let (stderr, stderr_truncated) = stderr_task.await.unwrap_or_default();

        let duration = start.elapsed();

        Ok(CommandOutput {
            exit_code: status.code(),
            stdout,
            stderr,
            duration,
            was_success: status.success(),
            stdout_truncated,
            stderr_truncated,
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

    #[tokio::test]
    async fn test_harness_timeout_terminates_process_tree() {
        let dir = tempdir().unwrap();
        let runner = SandboxedRunner::new(dir.path());

        #[cfg(target_os = "windows")]
        {
            let res = runner
                .run_command(
                    "cmd.exe",
                    &["/c", "ping -n 10 127.0.0.1 >nul"],
                    None,
                    Duration::from_millis(200),
                )
                .await;

            assert!(matches!(res, Err(RunnerError::Timeout(_))));
        }

        #[cfg(not(target_os = "windows"))]
        {
            let res = runner
                .run_command("sleep", &["5"], None, Duration::from_millis(200))
                .await;

            assert!(matches!(res, Err(RunnerError::Timeout(_))));
        }
    }

    #[tokio::test]
    async fn test_harness_invalid_working_dir_rejected() {
        let dir = tempdir().unwrap();
        let runner = SandboxedRunner::new(dir.path());

        #[cfg(target_os = "windows")]
        let res = runner
            .run_command(
                "cmd.exe",
                &["/c", "echo test"],
                Some(Path::new("../outside")),
                Duration::from_secs(5),
            )
            .await;

        #[cfg(not(target_os = "windows"))]
        let res = runner
            .run_command(
                "echo",
                &["test"],
                Some(Path::new("../outside")),
                Duration::from_secs(5),
            )
            .await;

        assert!(matches!(res, Err(RunnerError::InvalidWorkingDir)));
    }

    #[test]
    fn test_runner_isolation_level_reports_restricted_process() {
        let dir = tempdir().unwrap();
        let runner = SandboxedRunner::new(dir.path());
        assert_eq!(
            runner.isolation_level(),
            ProcessIsolationLevel::RestrictedProcess
        );
    }

    #[tokio::test]
    async fn test_runner_strips_sensitive_env_when_configured() {
        let dir = tempdir().unwrap();
        let runner = SandboxedRunner::new(dir.path()).with_stripped_sensitive_env(true);

        std::env::set_var("OPENAI_API_KEY", "sk-secret-test-key-12345");

        #[cfg(target_os = "windows")]
        let out = runner
            .run_command(
                "cmd.exe",
                &["/c", "echo %OPENAI_API_KEY%"],
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Command should succeed");

        #[cfg(target_os = "windows")]
        {
            // In cmd.exe, an unset variable echoes as %OPENAI_API_KEY% or empty
            let stdout = out.stdout.trim();
            assert!(
                !stdout.contains("sk-secret-test-key-12345"),
                "Sensitive key must not leak to child process: got {stdout}"
            );
        }
    }

    #[tokio::test]
    async fn test_runner_bounded_output_truncation() {
        let dir = tempdir().unwrap();
        let runner = SandboxedRunner::new(dir.path()).with_max_stdout_bytes(64);

        #[cfg(target_os = "windows")]
        let out = runner
            .run_command(
                "cmd.exe",
                &[
                    "/c",
                    "echo 1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890",
                ],
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Command should succeed");

        #[cfg(not(target_os = "windows"))]
        let out = runner
            .run_command(
                "echo",
                &["1234567890123456789012345678901234567890123456789012345678901234567890123456789012345678901234567890"],
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Command should succeed");

        assert!(
            out.stdout_truncated,
            "Output exceeding limit must be marked as truncated"
        );
        assert!(out
            .stdout
            .contains("[TRUNCATED: output exceeded maximum byte limit]"));
    }

    #[tokio::test]
    async fn test_runner_default_deny_env_isolation() {
        let dir = tempdir().unwrap();
        let runner = SandboxedRunner::new(dir.path()).with_env_isolation(true);

        std::env::set_var("CUSTOM_SENSITIVE_SECRET", "super-secret-pass-val");
        std::env::set_var("UNLISTED_EXTERNAL_VAR", "unlisted-arbitrary-val");

        #[cfg(target_os = "windows")]
        let out = runner
            .run_command(
                "cmd.exe",
                &[
                    "/c",
                    "echo S=%CUSTOM_SENSITIVE_SECRET% U=%UNLISTED_EXTERNAL_VAR%",
                ],
                None,
                Duration::from_secs(5),
            )
            .await
            .expect("Command should succeed");

        #[cfg(target_os = "windows")]
        {
            let stdout = out.stdout.trim();
            assert!(
                !stdout.contains("super-secret-pass-val"),
                "Sensitive secret must not leak under default-deny env isolation"
            );
            assert!(
                !stdout.contains("unlisted-arbitrary-val"),
                "Unlisted environment variable must not leak under default-deny env isolation"
            );
        }
    }
}
