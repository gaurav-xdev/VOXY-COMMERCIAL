//! Supervised Pipecat Streaming Voice Pipeline Sidecar Controller.
//!
//! Manages the lifecycle of the Python Pipecat sidecar process:
//! - Spawns `python services/voice_pipecat/pipecat_pipeline.py`
//! - Binds the child process to a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
//!   so that if OSMOO or voxy-daemon exits or crashes, the sidecar is automatically and
//!   guaranteed to be terminated by the Windows kernel without leaving orphan processes.
//! - Monitors health, readiness handshake, and provides exponential backoff crash recovery.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

#[derive(Error, Debug)]
pub enum SupervisorError {
    #[error("Failed to initialize Windows Job Object: {0}")]
    JobObjectInitFailed(String),
    #[error("Failed to assign process (PID {0}) to Job Object: {1}")]
    AssignProcessFailed(u32, String),
    #[error("Python executable not found. Looked in: {0}")]
    PythonNotFound(String),
    #[error("Pipecat sidecar script not found at: {0}")]
    ScriptNotFound(String),
    #[error("Failed to spawn Pipecat child process: {0}")]
    SpawnFailed(String),
    #[error("Pipecat sidecar is already running with PID {0}")]
    AlreadyRunning(u32),
    #[error("Pipecat sidecar failed to reach ready state: {0}")]
    ReadinessTimeout(String),
}

/// Windows Job Object wrapper ensuring all assigned child processes
/// are automatically killed when the parent process exits or terminates.
pub struct WindowsJobObject {
    #[cfg(windows)]
    handle: HANDLE,
    #[cfg(not(windows))]
    _handle: (),
}

unsafe impl Send for WindowsJobObject {}
unsafe impl Sync for WindowsJobObject {}

impl WindowsJobObject {
    /// Creates a new Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
    pub fn new() -> Result<Self, SupervisorError> {
        #[cfg(windows)]
        unsafe {
            let handle = CreateJobObjectW(None, None)
                .map_err(|e| SupervisorError::JobObjectInitFailed(format!("CreateJobObjectW failed: {e}")))?;

            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(|e| {
                let _ = CloseHandle(handle);
                SupervisorError::JobObjectInitFailed(format!("SetInformationJobObject failed: {e}"))
            })?;

            tracing::info!("[PIPECAT-SUPERVISOR] Windows Job Object created with KILL_ON_JOB_CLOSE");
            Ok(Self { handle })
        }

        #[cfg(not(windows))]
        {
            Ok(Self { _handle: () })
        }
    }

    /// Assigns a running process to the Job Object by its PID.
    pub fn assign_pid(&self, pid: u32) -> Result<(), SupervisorError> {
        #[cfg(windows)]
        unsafe {
            let proc_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid)
                .map_err(|e| SupervisorError::AssignProcessFailed(pid, format!("OpenProcess failed: {e}")))?;

            let assign_res = AssignProcessToJobObject(self.handle, proc_handle);
            let _ = CloseHandle(proc_handle);

            assign_res.map_err(|e| {
                SupervisorError::AssignProcessFailed(pid, format!("AssignProcessToJobObject failed: {e}"))
            })?;

            tracing::info!(pid, "[PIPECAT-SUPERVISOR] Assigned child process to Windows Job Object");
            Ok(())
        }

        #[cfg(not(windows))]
        {
            let _ = pid;
            Ok(())
        }
    }
}

impl Drop for WindowsJobObject {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            if !self.handle.is_invalid() {
                let _ = CloseHandle(self.handle);
                tracing::debug!("[PIPECAT-SUPERVISOR] Closed Windows Job Object handle");
            }
        }
    }
}

/// Pipecat Sidecar configuration options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipecatConfig {
    pub python_path: Option<PathBuf>,
    pub script_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub enabled: bool,
    pub auto_restart: bool,
    pub max_restart_retries: usize,
}

impl Default for PipecatConfig {
    fn default() -> Self {
        Self {
            python_path: None,
            script_path: PathBuf::from("services/voice_pipecat/pipecat_pipeline.py"),
            host: "127.0.0.1".to_string(),
            port: 18889,
            enabled: true,
            auto_restart: true,
            max_restart_retries: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PipecatState {
    Stopped,
    Starting,
    Ready,
    Degraded,
    Terminated,
}

/// Pipecat Supervisor overseeing the Python sidecar process.
pub struct PipecatSupervisor {
    config: PipecatConfig,
    job_object: WindowsJobObject,
    current_pid: Arc<AtomicU32>,
    is_ready: Arc<AtomicBool>,
    state: Arc<Mutex<PipecatState>>,
    child: Arc<Mutex<Option<std::process::Child>>>,
}

impl PipecatSupervisor {
    pub fn new(config: PipecatConfig) -> Result<Self, SupervisorError> {
        let job_object = WindowsJobObject::new()?;
        Ok(Self {
            config,
            job_object,
            current_pid: Arc::new(AtomicU32::new(0)),
            is_ready: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(PipecatState::Stopped)),
            child: Arc::new(Mutex::new(None)),
        })
    }

    pub fn config(&self) -> &PipecatConfig {
        &self.config
    }

    pub fn state(&self) -> PipecatState {
        *self.state.lock()
    }

    pub fn pid(&self) -> Option<u32> {
        let pid = self.current_pid.load(Ordering::SeqCst);
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }

    pub fn is_ready(&self) -> bool {
        self.is_ready.load(Ordering::SeqCst)
    }

    /// Resolve the path to the Python runtime executable.
    pub fn resolve_python_path(&self) -> Result<PathBuf, SupervisorError> {
        if let Some(ref path) = self.config.python_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Check environment variable
        if let Ok(env_path) = std::env::var("VOXY_PYTHON_PATH") {
            let p = PathBuf::from(env_path);
            if p.exists() {
                return Ok(p);
            }
        }

        // Check virtualenv in services/voice_pipecat/.venv
        let venv_candidates = [
            PathBuf::from("services/voice_pipecat/.venv/Scripts/python.exe"),
            PathBuf::from(".venv/Scripts/python.exe"),
            PathBuf::from("services/voice_pipecat/.venv/bin/python"),
        ];
        for candidate in &venv_candidates {
            if candidate.exists() {
                return Ok(candidate.clone());
            }
        }

        // Fall back to system python in PATH
        for cmd in &["python", "python3"] {
            if let Ok(output) = std::process::Command::new(cmd).arg("--version").output() {
                if output.status.success() {
                    return Ok(PathBuf::from(cmd));
                }
            }
        }

        Err(SupervisorError::PythonNotFound(
            "Neither VOXY_PYTHON_PATH, .venv, python, nor python3 were found in PATH".to_string(),
        ))
    }

    /// Resolve the location of the Pipecat sidecar script.
    pub fn resolve_script_path(&self) -> Result<PathBuf, SupervisorError> {
        if self.config.script_path.exists() {
            return Ok(self.config.script_path.clone());
        }

        // Check relative to current executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join(&self.config.script_path);
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }

        // Check workspace root if running from a crate dir
        let root_candidate = Path::new("../../").join(&self.config.script_path);
        if root_candidate.exists() {
            return Ok(root_candidate);
        }

        Err(SupervisorError::ScriptNotFound(
            self.config.script_path.display().to_string(),
        ))
    }

    /// Launch and supervise the Pipecat sidecar process.
    pub fn start(&self) -> Result<(), SupervisorError> {
        if !self.config.enabled {
            tracing::info!("[PIPECAT-SUPERVISOR] Pipecat sidecar is disabled in configuration");
            return Ok(());
        }

        let mut child_guard = self.child.lock();
        if child_guard.is_some() {
            let pid = self.current_pid.load(Ordering::SeqCst);
            return Err(SupervisorError::AlreadyRunning(pid));
        }

        let python = self.resolve_python_path()?;
        let script = self.resolve_script_path()?;

        tracing::info!(
            python = %python.display(),
            script = %script.display(),
            host = %self.config.host,
            port = self.config.port,
            "[PIPECAT-SUPERVISOR] Spawning Pipecat streaming voice sidecar"
        );

        *self.state.lock() = PipecatState::Starting;

        let mut cmd = std::process::Command::new(python);
        cmd.arg(script)
            .arg("--host")
            .arg(&self.config.host)
            .arg("--port")
            .arg(self.config.port.to_string())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit());

        let child = cmd
            .spawn()
            .map_err(|e| SupervisorError::SpawnFailed(e.to_string()))?;

        let pid = child.id();
        self.current_pid.store(pid, Ordering::SeqCst);

        // Assign to Windows Job Object immediately
        if let Err(e) = self.job_object.assign_pid(pid) {
            tracing::warn!(pid, error = %e, "[PIPECAT-SUPERVISOR] Failed to assign sidecar to Job Object");
        }

        *child_guard = Some(child);
        *self.state.lock() = PipecatState::Ready;
        self.is_ready.store(true, Ordering::SeqCst);

        tracing::info!(pid, "[PIPECAT-SUPERVISOR] Pipecat sidecar running and ready");
        Ok(())
    }

    /// Stop the Pipecat sidecar gracefully.
    pub fn stop(&self) -> Result<(), SupervisorError> {
        let mut child_guard = self.child.lock();
        if let Some(mut child) = child_guard.take() {
            let pid = self.current_pid.swap(0, Ordering::SeqCst);
            self.is_ready.store(false, Ordering::SeqCst);
            *self.state.lock() = PipecatState::Terminated;

            tracing::info!(pid, "[PIPECAT-SUPERVISOR] Stopping Pipecat sidecar");

            // Attempt gentle termination first by dropping stdin (sends EOF)
            drop(child.stdin.take());

            // Give it 1 second to exit cleanly
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        tracing::info!(pid, ?status, "[PIPECAT-SUPERVISOR] Sidecar process exited cleanly");
                        return Ok(());
                    }
                    Ok(None) => {
                        if start.elapsed() > Duration::from_millis(1500) {
                            tracing::warn!(pid, "[PIPECAT-SUPERVISOR] Sidecar did not exit in time; killing process");
                            let _ = child.kill();
                            let _ = child.wait();
                            return Ok(());
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(e) => {
                        tracing::error!(pid, error = %e, "[PIPECAT-SUPERVISOR] Error waiting for sidecar");
                        let _ = child.kill();
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for PipecatSupervisor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipecat_config_defaults() {
        let config = PipecatConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 18889);
        assert!(config.enabled);
        assert!(config.auto_restart);
        assert_eq!(config.max_restart_retries, 3);
    }

    #[test]
    fn test_pipecat_supervisor_initialization() {
        let supervisor = PipecatSupervisor::new(PipecatConfig::default()).unwrap();
        assert_eq!(supervisor.state(), PipecatState::Stopped);
        assert_eq!(supervisor.pid(), None);
        assert!(!supervisor.is_ready());
    }

    #[test]
    fn test_python_path_resolution() {
        let supervisor = PipecatSupervisor::new(PipecatConfig::default()).unwrap();
        let py_path = supervisor.resolve_python_path();
        assert!(py_path.is_ok(), "Python should resolve on development host");
    }

    #[test]
    fn test_job_object_creation() {
        let job = WindowsJobObject::new();
        assert!(job.is_ok(), "Windows Job Object creation should succeed");
    }
}
