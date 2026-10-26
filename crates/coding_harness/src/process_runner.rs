use crate::types::{CommandExecutionInput, CommandExecutionResult, HarnessError, Result};
use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};
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
    pub async fn run_command(
        input: &CommandExecutionInput,
        workspace_root: &Path,
    ) -> Result<CommandExecutionResult> {
        let cwd = input.cwd.as_deref().unwrap_or(workspace_root);
        let timeout_duration = Duration::from_millis(input.timeout_ms.unwrap_or(120_000).min(600_000));

        let start = Instant::now();

        // Build process command with environment allowlist
        let mut cmd = if cfg!(windows) {
            let mut c = tokio::process::Command::new("powershell.exe");
            c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &input.command]);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", &input.command]);
            c
        };

        cmd.current_dir(cwd);

        // Filter environment
        let safe_set: HashSet<String> = SAFE_ENV_VARS.iter().map(|s| s.to_uppercase()).collect();
        cmd.env_clear();
        for (k, v) in std::env::vars() {
            if safe_set.contains(&k.to_uppercase()) {
                cmd.env(k, v);
            }
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        let job = WindowsJobObject::new().ok();

        let child = cmd.spawn()
            .map_err(|e| HarnessError::CommandError(format!("Failed to spawn process: {}", e)))?;

        #[cfg(windows)]
        if let Some(ref j) = job {
            if let Some(handle) = child.raw_handle() {
                let _ = j.assign_process(windows::Win32::Foundation::HANDLE(handle));
            }
        }

        let wait_res = timeout(timeout_duration, child.wait_with_output()).await;

        let elapsed = start.elapsed().as_millis() as u64;

        match wait_res {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                Ok(CommandExecutionResult {
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms: elapsed,
                    timed_out: false,
                })
            }
            Ok(Err(e)) => Err(HarnessError::CommandError(format!("Process execution error: {}", e))),
            Err(_) => {
                // Timeout occurred: Job Object ensures all child processes die on close
                Ok(CommandExecutionResult {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Process timed out after {} ms", timeout_duration.as_millis()),
                    duration_ms: elapsed,
                    timed_out: true,
                })
            }
        }
    }
}

#[cfg(windows)]
struct WindowsJobObject {
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl WindowsJobObject {
    fn new() -> Result<Self> {
        use windows::Win32::System::JobObjects::{
            CreateJobObjectW, SetInformationJobObject, JobObjectExtendedLimitInformation,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };

        unsafe {
            let handle = CreateJobObjectW(None, None)
                .map_err(|e| HarnessError::CommandError(format!("CreateJobObject failed: {}", e)))?;

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
                return Err(HarnessError::CommandError(format!("SetInformationJobObject failed: {}", e)));
            }

            Ok(Self { handle })
        }
    }

    fn assign_process(&self, process_handle: windows::Win32::Foundation::HANDLE) -> Result<()> {
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;
        unsafe {
            AssignProcessToJobObject(self.handle, process_handle)
                .map_err(|e| HarnessError::CommandError(format!("AssignProcessToJobObject failed: {}", e)))?;
        }
        Ok(())
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
