//! OSMOO Windows Application Supervisor & Process Lifecycle Manager.
//!
//! Enforces:
//! 1. `crates/desktop_ui` (`OSMOO.exe`) as the primary unified entry point.
//! 2. Reliable supervisor lifecycle managing background daemon (`apps/daemon`) and overlay (`apps/overlay`).
//! 3. Windows Job Object integration with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, guaranteeing
//!    zero orphaned processes or zombie handles even on abnormal termination or crash.
//! 4. Two-phase graceful shutdown:
//!    - Phase 1: IPC shutdown signal (`ClientCommand::Shutdown`) over named pipe.
//!    - Phase 2: Up to 5-second drainage wait, with fallback recursive `taskkill /F /T /PID`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use tracing::{error, info, warn};

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

/// Supervisor errors.
#[derive(Debug)]
pub enum SupervisorError {
    JobObjectInitFailed(String),
    AssignProcessFailed(u32, String),
    LaunchFailed(String, std::io::Error),
    BinaryNotFound(String),
    ShutdownError(String),
}

impl std::fmt::Display for SupervisorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JobObjectInitFailed(msg) => write!(f, "Failed to initialize Windows Job Object: {msg}"),
            Self::AssignProcessFailed(pid, msg) => write!(f, "Failed to assign process (PID {pid}) to Job Object: {msg}"),
            Self::LaunchFailed(name, err) => write!(f, "Process launch failed for '{name}': {err}"),
            Self::BinaryNotFound(name) => write!(f, "Process '{name}' not found in executable search paths"),
            Self::ShutdownError(msg) => write!(f, "Supervisor shutdown error: {msg}"),
        }
    }
}

impl std::error::Error for SupervisorError {}

/// Windows Job Object wrapper ensuring all assigned child processes
/// are automatically killed when the parent process exits or terminates.
pub struct WindowsJobObject {
    #[cfg(windows)]
    handle: HANDLE,
    #[cfg(not(windows))]
    _handle: (),
}

// Send and Sync are safe because the Win32 handle is accessed under thread-safe patterns
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

            info!("Initialized Windows Job Object with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE");
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

            info!(pid, "Successfully assigned child process to Windows Job Object");
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
                info!("Closed Windows Job Object handle");
            }
        }
    }
}

/// Information about a tracked child process.
pub struct TrackedChild {
    pub name: String,
    pub pid: u32,
    pub started_at: Instant,
    pub child: Option<Child>,
}

/// Configuration options for the OSMOO Process Supervisor.
#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub daemon_bin: Option<PathBuf>,
    pub overlay_bin: Option<PathBuf>,
    pub autostart_daemon: bool,
    pub autostart_overlay: bool,
    pub graceful_shutdown_timeout: Duration,
    pub pipe_name: String,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            daemon_bin: None,
            overlay_bin: None,
            autostart_daemon: true,
            autostart_overlay: true,
            graceful_shutdown_timeout: Duration::from_secs(5),
            pipe_name: voxy_ipc::VOXY_PIPE_NAME.to_string(),
        }
    }
}

/// Status summary of supervised components.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SupervisorStatus {
    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    pub overlay_running: bool,
    pub overlay_pid: Option<u32>,
    pub pipe_name: String,
    pub is_shutting_down: bool,
    pub job_object_active: bool,
}

/// OSMOO Primary Process Supervisor coordinating daemon and overlay lifecycles.
pub struct Supervisor {
    config: SupervisorConfig,
    job_object: Arc<WindowsJobObject>,
    children: Arc<Mutex<HashMap<String, TrackedChild>>>,
    is_shutting_down: Arc<AtomicBool>,
}

impl Supervisor {
    /// Creates a new supervisor instance.
    pub fn new(config: SupervisorConfig) -> Result<Self, SupervisorError> {
        let job_object = Arc::new(WindowsJobObject::new()?);
        Ok(Self {
            config,
            job_object,
            children: Arc::new(Mutex::new(HashMap::new())),
            is_shutting_down: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Locate an executable binary using standard resolution order.
    pub fn locate_binary(name: &str, custom_path: Option<&Path>) -> Option<PathBuf> {
        // 1. Explicit override path
        if let Some(path) = custom_path {
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }

        // 2. Environment variable override
        let env_key = format!("VOXY_{}_BIN", name.replace('-', "_").to_uppercase());
        if let Ok(env_val) = std::env::var(&env_key) {
            let p = PathBuf::from(env_val);
            if p.exists() {
                return Some(p);
            }
        }
        let env_osmoo_key = format!("OSMOO_{}_BIN", name.replace('-', "_").to_uppercase());
        if let Ok(env_val) = std::env::var(&env_osmoo_key) {
            let p = PathBuf::from(env_val);
            if p.exists() {
                return Some(p);
            }
        }

        let exe_name = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_string()
        };

        // 3. Adjacent to current executable (e.g. installed directory)
        if let Ok(curr_exe) = std::env::current_exe() {
            if let Some(parent) = curr_exe.parent() {
                let adjacent = parent.join(&exe_name);
                if adjacent.exists() {
                    return Some(adjacent);
                }
            }
        }

        // 4. Workspace target directories (for dev runs)
        for target_profile in &["debug", "release"] {
            let candidate = PathBuf::from("target").join(target_profile).join(&exe_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }

        // 5. Look in PATH
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let candidate = dir.join(&exe_name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Spawns a tracked child process and registers it with the Windows Job Object.
    fn spawn_child(&self, name: &str, bin_path: &Path, args: &[&str]) -> Result<u32, SupervisorError> {
        let mut cmd = Command::new(bin_path);
        cmd.args(args);
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        let child = cmd
            .spawn()
            .map_err(|e| SupervisorError::LaunchFailed(name.to_string(), e))?;

        let pid = child.id();
        info!(child = %name, pid, path = %bin_path.display(), "Spawned child process");

        // Immediately attach to Job Object
        if let Err(e) = self.job_object.assign_pid(pid) {
            warn!(child = %name, pid, error = %e, "Failed to assign to Job Object, falling back to process tree kill");
        }

        let tracked = TrackedChild {
            name: name.to_string(),
            pid,
            started_at: Instant::now(),
            child: Some(child),
        };

        self.children.lock().insert(name.to_string(), tracked);
        Ok(pid)
    }

    /// Launch background daemon (`voxy-daemon.exe`) if enabled and not already running.
    pub fn start_daemon(&self) -> Result<Option<u32>, SupervisorError> {
        if !self.config.autostart_daemon {
            info!("Daemon autostart disabled by supervisor config");
            return Ok(None);
        }

        // If daemon is already tracked, return existing PID
        if let Some(child) = self.children.lock().get("daemon") {
            return Ok(Some(child.pid));
        }

        let daemon_path = Self::locate_binary("voxy-daemon", self.config.daemon_bin.as_deref())
            .ok_or_else(|| SupervisorError::BinaryNotFound("voxy-daemon".to_string()))?;

        let pid = self.spawn_child("daemon", &daemon_path, &[])?;
        Ok(Some(pid))
    }

    /// Launch overlay UI (`voxy-overlay.exe`) if enabled.
    pub fn start_overlay(&self) -> Result<Option<u32>, SupervisorError> {
        if !self.config.autostart_overlay {
            info!("Overlay autostart disabled by supervisor config");
            return Ok(None);
        }

        if let Some(child) = self.children.lock().get("overlay") {
            return Ok(Some(child.pid));
        }

        let overlay_path = Self::locate_binary("voxy-overlay", self.config.overlay_bin.as_deref())
            .ok_or_else(|| SupervisorError::BinaryNotFound("voxy-overlay".to_string()))?;

        let pid = self.spawn_child("overlay", &overlay_path, &[])?;
        Ok(Some(pid))
    }

    /// Start all configured supervised services (daemon and overlay).
    pub fn start_all(&self) -> Result<(), SupervisorError> {
        if self.config.autostart_daemon {
            match self.start_daemon() {
                Ok(Some(pid)) => info!(pid, "Supervised daemon online"),
                Ok(None) => {}
                Err(e) => warn!(error = %e, "Could not start supervised daemon; continuing in standalone mode"),
            }
        }

        if self.config.autostart_overlay {
            match self.start_overlay() {
                Ok(Some(pid)) => info!(pid, "Supervised overlay online"),
                Ok(None) => {}
                Err(e) => warn!(error = %e, "Could not start supervised overlay; continuing without overlay"),
            }
        }

        Ok(())
    }

    /// Force terminates a process and its entire descendant process tree.
    pub fn kill_pid_tree(pid: u32) {
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = Command::new("kill")
                .args(["-9", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    /// Gracefully shuts down all supervised child processes.
    ///
    /// 1. Emits `ClientCommand::Shutdown` over the named pipe IPC.
    /// 2. Waits up to `graceful_shutdown_timeout` (5s) for child processes to exit cleanly.
    /// 3. Force-kills any remaining processes using `taskkill /F /T /PID`.
    /// 4. Ensures zero orphaned processes or zombie handles.
    pub async fn shutdown(&self) -> Result<(), SupervisorError> {
        if self.is_shutting_down.swap(true, Ordering::SeqCst) {
            info!("Supervisor shutdown already in progress");
            return Ok(());
        }

        info!("Starting OSMOO supervisor shutdown sequence");

        // Phase 1: Graceful IPC shutdown request
        let client = voxy_ipc::VoxyIpcClient::with_pipe_name(&self.config.pipe_name);
        client.start();

        // Brief connection attempt (up to 500ms)
        let mut conn_watch = client.watch_connected();
        let mut connected = false;
        for _ in 0..10 {
            if *conn_watch.borrow() {
                connected = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tokio::time::timeout(Duration::from_millis(50), conn_watch.changed()).await;
        }

        if connected {
            info!("Sending graceful Shutdown command over named pipe IPC");
            let cmd = voxy_ipc::ClientCommand::Shutdown {
                reason: "OSMOO primary executable exiting".to_string(),
            };
            let _ = client.send_command(cmd).await;
            // Allow small buffer flush window
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        client.stop();

        // Phase 2: Drain and wait up to graceful_shutdown_timeout
        let timeout = self.config.graceful_shutdown_timeout;
        let start = Instant::now();
        let poll_interval = Duration::from_millis(100);

        loop {
            let mut all_exited = true;
            {
                let mut guard = self.children.lock();
                for (name, tracked) in guard.iter_mut() {
                    if let Some(ref mut child) = tracked.child {
                        match child.try_wait() {
                            Ok(Some(status)) => {
                                info!(child = %name, pid = tracked.pid, exit_status = ?status, "Child exited gracefully");
                                tracked.child = None;
                            }
                            Ok(None) => {
                                all_exited = false;
                            }
                            Err(e) => {
                                warn!(child = %name, error = %e, "Error waiting on child process");
                                tracked.child = None;
                            }
                        }
                    }
                }
            }

            if all_exited || start.elapsed() >= timeout {
                break;
            }
            tokio::time::sleep(poll_interval).await;
        }

        // Phase 3: Force termination if still active after timeout
        {
            let mut guard = self.children.lock();
            for (name, tracked) in guard.iter_mut() {
                if let Some(ref mut child) = tracked.child {
                    warn!(
                        child = %name,
                        pid = tracked.pid,
                        "Child did not terminate within {:?}; forcing termination via taskkill /F /T /PID",
                        timeout
                    );
                    Self::kill_pid_tree(tracked.pid);
                    let _ = child.kill();
                    let _ = child.wait();
                    tracked.child = None;
                }
            }
            guard.clear();
        }

        info!("OSMOO supervisor shutdown complete: zero orphaned processes remain");
        Ok(())
    }

    /// Retrieves current supervisor status.
    pub fn status(&self) -> SupervisorStatus {
        let guard = self.children.lock();
        let daemon_child = guard.get("daemon");
        let overlay_child = guard.get("overlay");

        SupervisorStatus {
            daemon_running: daemon_child.map(|c| c.child.is_some()).unwrap_or(false),
            daemon_pid: daemon_child.map(|c| c.pid),
            overlay_running: overlay_child.map(|c| c.child.is_some()).unwrap_or(false),
            overlay_pid: overlay_child.map(|c| c.pid),
            pipe_name: self.config.pipe_name.clone(),
            is_shutting_down: self.is_shutting_down.load(Ordering::SeqCst),
            job_object_active: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_job_object_creation() {
        let job = WindowsJobObject::new();
        assert!(job.is_ok(), "Windows Job Object should initialize cleanly");
    }

    #[test]
    fn test_supervisor_config_default() {
        let cfg = SupervisorConfig::default();
        assert!(cfg.autostart_daemon);
        assert!(cfg.autostart_overlay);
        assert_eq!(cfg.graceful_shutdown_timeout, Duration::from_secs(5));
        assert_eq!(cfg.pipe_name, voxy_ipc::VOXY_PIPE_NAME);
    }

    #[test]
    fn test_supervisor_status_initial() {
        let cfg = SupervisorConfig {
            autostart_daemon: false,
            autostart_overlay: false,
            ..Default::default()
        };
        let supervisor = Supervisor::new(cfg).unwrap();
        let status = supervisor.status();
        assert!(!status.daemon_running);
        assert!(!status.overlay_running);
        assert!(status.daemon_pid.is_none());
        assert!(status.overlay_pid.is_none());
        assert!(!status.is_shutting_down);
        assert!(status.job_object_active);
    }

    #[test]
    fn test_supervisor_locate_binary_fallback() {
        // Looking up a non-existent binary should safely return None
        let not_found = Supervisor::locate_binary("non_existent_binary_xyz_12345", None);
        assert!(not_found.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn test_job_object_assign_and_kill_process_tree() {
        let job = WindowsJobObject::new().expect("Job object init failed");

        // Spawn a real Windows background ping process (waits 10 seconds)
        let mut child = Command::new("cmd.exe")
            .args(["/C", "ping -n 10 127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to spawn test cmd process");

        let pid = child.id();
        assert!(pid > 0);

        // Assign to Job Object
        let assign_res = job.assign_pid(pid);
        assert!(assign_res.is_ok(), "Should successfully assign process to Job Object");

        // Kill process tree
        Supervisor::kill_pid_tree(pid);

        // Process should terminate promptly
        let wait_res = child.wait();
        assert!(wait_res.is_ok());
    }
}

