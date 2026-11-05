use crate::types::{
    CommandExecutionInput, CommandExecutionResult, HarnessError, ProcessExecutionInput, Result,
};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

pub struct ProcessRunner;

const SAFE_ENV_VARS: &[&str] = &[
    "PATH",
    "SYSTEMROOT",
    "SYSTEMDRIVE",
    "WINDIR",
    "COMSPEC",
    "PATHEXT",
    "TEMP",
    "TMP",
    "USERNAME",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "PSMODULEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "PROGRAMFILES",
    "PROGRAMFILES(X86)",
    "PROGRAMDATA",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "JAVA_HOME",
    "GOPATH",
    "GOROOT",
    "NODE_PATH",
    "PYTHONHOME",
    "PYTHONPATH",
];

impl ProcessRunner {
    /// Execute a shell command string using powershell on Windows or sh on Unix.
    /// Strictly enforces process tree containment via Windows Job Objects.
    pub async fn run_command(
        input: &CommandExecutionInput,
        workspace_root: &Path,
    ) -> Result<CommandExecutionResult> {
        let cwd = input.cwd.as_deref().unwrap_or(workspace_root);
        let timeout_duration =
            Duration::from_millis(input.timeout_ms.unwrap_or(120_000).min(600_000));

        let start = Instant::now();

        let mut cmd = if cfg!(windows) {
            let mut c = tokio::process::Command::new("powershell.exe");
            c.args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-InputFormat",
                "None",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &input.command,
            ]);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", &input.command]);
            c
        };

        cmd.current_dir(cwd);

        // Filter environment to allowlisted variables
        let safe_set: HashSet<String> = SAFE_ENV_VARS.iter().map(|s| s.to_uppercase()).collect();
        cmd.env_clear();
        for (k, v) in std::env::vars() {
            if safe_set.contains(&k.to_uppercase()) {
                cmd.env(k, v);
            }
        }

        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        let job = WindowsJobObject::new().map_err(|e| {
            HarnessError::CommandError(format!("Windows Job Object creation failed: {}", e))
        })?;

        let mut child = cmd
            .spawn()
            .map_err(|e| HarnessError::CommandError(format!("Failed to spawn process: {}", e)))?;

        #[cfg(windows)]
        if let Some(handle) = child.raw_handle() {
            if let Err(e) = job.assign_process(windows::Win32::Foundation::HANDLE(handle)) {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(HarnessError::CommandError(format!(
                    "Failed to assign process to Windows Job Object: {}",
                    e
                )));
            }
        }

        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();

        let wait_output = async {
            let mut out_buf = Vec::new();
            let mut err_buf = Vec::new();

            let out_task = async {
                if let Some(mut p) = stdout_pipe {
                    let _ = p.read_to_end(&mut out_buf).await;
                }
                out_buf
            };
            let err_task = async {
                if let Some(mut p) = stderr_pipe {
                    let _ = p.read_to_end(&mut err_buf).await;
                }
                err_buf
            };

            let (status, out, err) = tokio::join!(child.wait(), out_task, err_task);
            (status, out, err)
        };

        match timeout(timeout_duration, wait_output).await {
            Ok((Ok(status), out, err)) => {
                let stdout = String::from_utf8_lossy(&out).to_string();
                let stderr = String::from_utf8_lossy(&err).to_string();
                let exit_code = status.code().unwrap_or(-1);

                Ok(CommandExecutionResult {
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms: start.elapsed().as_millis() as u64,
                    timed_out: false,
                })
            }
            Ok((Err(e), _, _)) => Err(HarnessError::CommandError(format!("Process error: {}", e))),
            Err(_) => {
                // Timeout occurred: kill the direct child and let Job Object cleanup process tree
                let _ = child.start_kill();
                let _ = child.wait().await;
                Ok(CommandExecutionResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!(
                        "Process timed out after {} ms",
                        timeout_duration.as_millis()
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    timed_out: true,
                })
            }
        }
    }

    /// Execute a process directly with structured arguments without shell interpretation.
    /// Eliminates shell metacharacter injection and handles spaces, quotes, newlines, and Unicode safely.
    pub async fn run_process(
        input: &ProcessExecutionInput,
        workspace_root: &Path,
    ) -> Result<CommandExecutionResult> {
        let cwd = input.cwd.as_deref().unwrap_or(workspace_root);
        let timeout_duration =
            Duration::from_millis(input.timeout_ms.unwrap_or(120_000).min(600_000));

        let start = Instant::now();

        let mut cmd = tokio::process::Command::new(&input.program);
        for arg in &input.args {
            cmd.arg(arg);
        }

        cmd.current_dir(cwd);

        // Filter environment to allowlist
        let safe_set: HashSet<String> = SAFE_ENV_VARS.iter().map(|s| s.to_uppercase()).collect();
        cmd.env_clear();
        for (k, v) in std::env::vars() {
            if safe_set.contains(&k.to_uppercase()) {
                cmd.env(k, v);
            }
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        if input.stdin.is_some() {
            cmd.stdin(std::process::Stdio::piped());
        }

        #[cfg(windows)]
        let job = if input.require_job_object {
            Some(WindowsJobObject::new().map_err(|e| {
                HarnessError::CommandError(format!("Windows Job Object creation failed: {}", e))
            })?)
        } else {
            None
        };

        let mut child = cmd.spawn().map_err(|e| {
            HarnessError::CommandError(format!(
                "Failed to spawn process '{}': {}",
                input.program.display(),
                e
            ))
        })?;

        #[cfg(windows)]
        if let Some(ref j) = job {
            if let Some(handle) = child.raw_handle() {
                if let Err(e) = j.assign_process(windows::Win32::Foundation::HANDLE(handle)) {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return Err(HarnessError::CommandError(format!(
                        "Failed to assign process to Windows Job Object: {}",
                        e
                    )));
                }
            } else if input.require_job_object {
                let _ = child.start_kill();
                let _ = child.wait().await;
                return Err(HarnessError::CommandError(
                    "Could not retrieve raw process handle for Job Object containment".into(),
                ));
            }
        }

        if let Some(ref stdin_content) = input.stdin {
            if let Some(mut stdin_pipe) = child.stdin.take() {
                let bytes = stdin_content.as_bytes().to_vec();
                tokio::spawn(async move {
                    let _ = stdin_pipe.write_all(&bytes).await;
                    let _ = stdin_pipe.flush().await;
                });
            }
        }

        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();

        let wait_output = async {
            let mut out_buf = Vec::new();
            let mut err_buf = Vec::new();

            let out_task = async {
                if let Some(mut p) = stdout_pipe {
                    let _ = p.read_to_end(&mut out_buf).await;
                }
                out_buf
            };
            let err_task = async {
                if let Some(mut p) = stderr_pipe {
                    let _ = p.read_to_end(&mut err_buf).await;
                }
                err_buf
            };

            let (status, out, err) = tokio::join!(child.wait(), out_task, err_task);
            (status, out, err)
        };

        match timeout(timeout_duration, wait_output).await {
            Ok((Ok(status), out, err)) => {
                let stdout = String::from_utf8_lossy(&out).to_string();
                let stderr = String::from_utf8_lossy(&err).to_string();
                let exit_code = status.code().unwrap_or(-1);

                Ok(CommandExecutionResult {
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms: start.elapsed().as_millis() as u64,
                    timed_out: false,
                })
            }
            Ok((Err(e), _, _)) => Err(HarnessError::CommandError(format!(
                "Process execution error: {}",
                e
            ))),
            Err(_) => {
                // Timeout: forcibly kill direct child process and allow Job Object to clean up process tree
                let _ = child.start_kill();
                let _ = child.wait().await;
                Ok(CommandExecutionResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!(
                        "Process timed out after {} ms",
                        timeout_duration.as_millis()
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    timed_out: true,
                })
            }
        }
    }
}

#[cfg(windows)]
pub struct WindowsJobObject {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl WindowsJobObject {
    pub fn new() -> Result<Self> {
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let handle = CreateJobObjectW(None, None).map_err(|e| {
                HarnessError::CommandError(format!("CreateJobObject failed: {}", e))
            })?;

            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let res = SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if let Err(e) = res {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                return Err(HarnessError::CommandError(format!(
                    "SetInformationJobObject failed: {}",
                    e
                )));
            }

            Ok(Self { handle })
        }
    }

    pub fn assign_process(&self, process_handle: windows::Win32::Foundation::HANDLE) -> Result<()> {
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;
        unsafe {
            AssignProcessToJobObject(self.handle, process_handle).map_err(|e| {
                HarnessError::CommandError(format!("AssignProcessToJobObject failed: {}", e))
            })?;
        }
        Ok(())
    }

    pub fn is_valid(&self) -> bool {
        !self.handle.is_invalid()
    }
}

#[cfg(windows)]
impl Drop for WindowsJobObject {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.handle);
        }
    }
}
