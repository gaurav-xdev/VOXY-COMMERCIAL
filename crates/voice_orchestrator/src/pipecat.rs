//! Supervised Pipecat Streaming Voice Pipeline Sidecar Controller.
//!
//! Manages the lifecycle of the Python Pipecat sidecar process:
//! - Spawns `python services/voice_pipecat/pipecat_pipeline.py`
//! - Binds the child process to a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
//!   so that if OSMOO or voxy-daemon exits or crashes, the sidecar is automatically and
//!   guaranteed to be terminated by the Windows kernel without leaving orphan processes.
//! - **Explicit readiness handshake**: the sidecar must emit `{"status":"ready"}` on
//!   stdout within `startup_timeout_secs`, otherwise the child is killed and launch fails.
//! - Drains stdout/stderr continuously so the child can never block on full pipes.
//! - Monitors child exit status, and performs bounded crash restarts with backoff
//!   (`auto_restart` / `max_restart_retries` / `restart_backoff_ms`).
//! - Active health check: supervisor Ping → sidecar Pong over stdin/stdout.

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
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

/// Grace period a sidecar has to exit after EOF on stdin before it is killed.
const DEFAULT_SIDECAR_SHUTDOWN_GRACE_MS: u64 = 1500;

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
            let handle = CreateJobObjectW(None, None).map_err(|e| {
                SupervisorError::JobObjectInitFailed(format!("CreateJobObjectW failed: {e}"))
            })?;

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

            tracing::info!(
                "[PIPECAT-SUPERVISOR] Windows Job Object created with KILL_ON_JOB_CLOSE"
            );
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
                .map_err(|e| {
                    SupervisorError::AssignProcessFailed(pid, format!("OpenProcess failed: {e}"))
                })?;

            let assign_res = AssignProcessToJobObject(self.handle, proc_handle);
            let _ = CloseHandle(proc_handle);

            assign_res.map_err(|e| {
                SupervisorError::AssignProcessFailed(
                    pid,
                    format!("AssignProcessToJobObject failed: {e}"),
                )
            })?;

            tracing::info!(
                pid,
                "[PIPECAT-SUPERVISOR] Assigned child process to Windows Job Object"
            );
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
    /// Maximum time between process spawn and the sidecar's explicit
    /// `{"status":"ready"}` stdout handshake before the child is killed.
    #[serde(default = "default_startup_timeout_secs")]
    pub startup_timeout_secs: u64,
    /// Backoff applied between crash-restart attempts (doubles up to a cap).
    #[serde(default = "default_restart_backoff_ms")]
    pub restart_backoff_ms: u64,
    /// Shared secret the sidecar must present to the daemon IPC endpoint.
    #[serde(default)]
    pub auth_token: Option<String>,
}

fn default_startup_timeout_secs() -> u64 {
    20
}

fn default_restart_backoff_ms() -> u64 {
    2_000
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
            startup_timeout_secs: default_startup_timeout_secs(),
            restart_backoff_ms: default_restart_backoff_ms(),
            auth_token: None,
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

// ==============================================================================
// Free spawn helpers (usable from the supervision thread without shared &self)
// ==============================================================================

fn resolve_python_path_for(config: &PipecatConfig) -> Result<PathBuf, SupervisorError> {
    if let Some(ref path) = config.python_path {
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

fn resolve_script_path_for(config: &PipecatConfig) -> Result<PathBuf, SupervisorError> {
    if config.script_path.exists() {
        return Ok(config.script_path.clone());
    }

    // Check relative to current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(&config.script_path);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    // Check workspace root if running from a crate dir
    let root_candidate = Path::new("../../").join(&config.script_path);
    if root_candidate.exists() {
        return Ok(root_candidate);
    }

    Err(SupervisorError::ScriptNotFound(
        config.script_path.display().to_string(),
    ))
}

/// Spawn command builder: always pipes stdin/stdout/stderr so the supervisor
/// fully drains child output (no pipe-buffer deadlocks) and can send commands.
fn build_spawn_command(config: &PipecatConfig) -> Result<std::process::Command, SupervisorError> {
    let python = resolve_python_path_for(config)?;
    let script = resolve_script_path_for(config)?;

    tracing::info!(
        python = %python.display(),
        script = %script.display(),
        host = %config.host,
        port = config.port,
        "[PIPECAT-SUPERVISOR] Preparing Pipecat streaming voice sidecar spawn"
    );

    let mut command = std::process::Command::new(python);
    command
        .arg(script)
        .arg("--host")
        .arg(&config.host)
        .arg("--port")
        .arg(config.port.to_string());

    // Explicit auth handshake secret forwarded to the sidecar environment.
    if let Some(ref token) = config.auth_token {
        command.env("VOXY_IPC_AUTH_TOKEN", token);
    }

    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    Ok(command)
}

fn assign_sidecar_to_job_object(job: &WindowsJobObject, pid: u32) -> Result<(), SupervisorError> {
    if let Err(e) = job.assign_pid(pid) {
        tracing::warn!(
            pid,
            error = %e,
            "[PIPECAT-SUPERVISOR] Failed to assign sidecar to Job Object"
        );
        return Err(e);
    }
    Ok(())
}

/// Shared supervisor internals so the crash-recovery thread can operate on
/// plain `Arc` handles without retaining a `&PipecatSupervisor`.
struct SupervisorInternals {
    job_object: WindowsJobObject,
    config: PipecatConfig,
    child: Mutex<Option<std::process::Child>>,
    current_pid: AtomicU32,
    state: Mutex<PipecatState>,
    is_ready: AtomicBool,
    stopping: AtomicBool,
    /// Bounded recent stdout lines emitted by the sidecar (readiness + Pong consumers).
    stdout_lines: Arc<Mutex<VecDeque<String>>>,
    /// Set to true once the sidecar emits its ready handshake on stdout.
    ready_signal: Arc<AtomicBool>,
}

impl SupervisorInternals {
    /// Clear per-child state (ready flag + buffered lines) before a fresh spawn.
    fn reset_child_state(&self) {
        self.ready_signal.store(false, Ordering::SeqCst);
        self.stdout_lines.lock().clear();
    }

    /// Non-destructive exit probe of the current child, if any.
    fn child_exited(&self) -> bool {
        let mut guard = self.child.lock();
        if let Some(child) = guard.as_mut() {
            matches!(child.try_wait(), Ok(Some(_)))
        } else {
            false
        }
    }

    /// Spawn the sidecar child and bind its streams to drain threads.
    fn spawn_and_bind(&self) -> Result<(), SupervisorError> {
        let mut cmd = build_spawn_command(&self.config)?;
        let mut child = cmd
            .spawn()
            .map_err(|e| SupervisorError::SpawnFailed(e.to_string()))?;
        let pid = child.id();
        self.current_pid.store(pid, Ordering::SeqCst);
        assign_sidecar_to_job_object(&self.job_object, pid)?;

        if let Some(stdout) = child.stdout.take() {
            spawn_stdout_drain_thread(
                stdout,
                Arc::new(StdoutState {
                    lines: Arc::clone(&self.stdout_lines),
                    ready_signal: Arc::clone(&self.ready_signal),
                }),
            );
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_stderr_drain_thread(stderr);
        }
        *self.child.lock() = Some(child);
        Ok(())
    }

    /// Drain stale stdout lines and return them.
    /// Drain stale stdout lines and return them.
    fn drain_stdout_lines(&self) -> Vec<String> {
        let mut buf = self.stdout_lines.lock();
        std::mem::take(&mut *buf).into_iter().collect()
    }

    /// Snapshot queued stdout lines without consuming them.
    fn peek_stdout_lines(&self) -> Vec<String> {
        self.stdout_lines.lock().iter().cloned().collect()
    }

    /// Wait (bounded) for the sidecar's readiness handshake on stdout.
    fn await_readiness(&self) -> Result<(), SupervisorError> {
        let deadline = Duration::from_secs(self.config.startup_timeout_secs);
        let poll = Duration::from_millis(50);
        let mut waited = Duration::ZERO;
        loop {
            if self.ready_signal.load(Ordering::SeqCst) {
                return Ok(());
            }
            if self.child_exited() {
                self.cleanup_dead_child();
                *self.state.lock() = PipecatState::Terminated;
                return Err(SupervisorError::ReadinessTimeout(
                    "sidecar exited before sending readiness handshake".to_string(),
                ));
            }
            if waited >= deadline {
                tracing::error!(
                    "[PIPECAT-SUPERVISOR] Readiness deadline exceeded after {deadline:?}; killing sidecar"
                );
                self.interrupt_child();
                return Err(SupervisorError::ReadinessTimeout(format!(
                    "no readiness handshake within {:?}",
                    deadline
                )));
            }
            std::thread::sleep(poll);
            waited += poll;
        }
    }

    /// Close the child's stdin (EOF => graceful exit signal to the sidecar).
    fn signal_shutdown_via_stdin(&self) {
        let mut guard = self.child.lock();
        if let Some(child) = guard.as_mut() {
            drop(child.stdin.take());
        }
    }

    /// Write a supervisor command JSON line to the sidecar's stdin.
    fn write_stdin_line(&self, line: &str) {
        let mut guard = self.child.lock();
        if let Some(child) = guard.as_mut() {
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                if let Err(e) = writeln!(stdin, "{{\"type\":\"{}\"}}", line) {
                    tracing::debug!(error = %e, "[PIPECAT-SUPERVISOR] Failed to write stdin command");
                }
                let _ = stdin.flush();
                return;
            }
        }
        tracing::debug!("[PIPECAT-SUPERVISOR] stdin unavailable (no running child)");
    }

    /// Ask the sidecar to exit; wait with grace, then kill.
    fn interrupt_child(&self) {
        self.signal_shutdown_via_stdin();
        self.await_child_exit_or_kill(Duration::from_millis(DEFAULT_SIDECAR_SHUTDOWN_GRACE_MS));
    }

    fn await_child_exit_or_kill(&self, grace: Duration) {
        let start = std::time::Instant::now();
        loop {
            let mut guard = self.child.lock();
            match guard.as_mut().map(|c| c.try_wait()) {
                Some(Ok(Some(_status))) => {
                    guard.take();
                    self.current_pid.store(0, Ordering::SeqCst);
                    return;
                }
                Some(Ok(None)) => {
                    if start.elapsed() > grace {
                        tracing::warn!(
                            "[PIPECAT-SUPERVISOR] Sidecar did not exit in time; killing process"
                        );
                        if let Some(mut child) = guard.take() {
                            let pid = child.id();
                            let _ = child.kill();
                            let _ = child.wait();
                            drop(child);
                            let _ = pid;
                        }
                        self.current_pid.store(0, Ordering::SeqCst);
                        return;
                    }
                }
                Some(Err(e)) => {
                    tracing::error!(error = %e, "[PIPECAT-SUPERVISOR] Error waiting for sidecar");
                    if let Some(mut child) = guard.take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    self.current_pid.store(0, Ordering::SeqCst);
                    return;
                }
                None => return, // No child
            }
            drop(guard);
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn cleanup_dead_child(&self) {
        if let Some(mut child) = self.child.lock().take() {
            if let Ok(status) = child.wait() {
                tracing::info!(exit = ?status, "[PIPECAT-SUPERVISOR] Reaped exited sidecar process");
            }
        }
        self.current_pid.store(0, Ordering::SeqCst);
    }
}

/// Per-child stdout bookkeeping (bounded buffer + readiness flag).
struct StdoutState {
    lines: Arc<Mutex<VecDeque<String>>>,
    ready_signal: Arc<AtomicBool>,
}

/// Drain loop for the sidecar's stdout: bounded buffer + readiness handshake.
fn spawn_stdout_drain_thread(reader: std::process::ChildStdout, state: Arc<StdoutState>) {
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    tracing::debug!("[PIPECAT-SUPERVISOR] Sidecar stdout stream ended");
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim_end().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    // Explicit readiness handshake from the sidecar.
                    if trimmed.starts_with('{') && trimmed.contains("ready") {
                        state.ready_signal.store(true, Ordering::SeqCst);
                    }
                    {
                        let mut buf = state.lines.lock();
                        if buf.len() >= 64 {
                            buf.pop_front();
                        }
                        buf.push_back(trimmed.clone());
                    }
                    tracing::debug!("[PIPECAT-SUPERVISOR] stdout: {trimmed}");
                }
                Err(e) => {
                    tracing::debug!(error = %e, "[PIPECAT-SUPERVISOR] stdout read error");
                    break;
                }
            }
        }
    });
}

/// Drain loop for the sidecar's stderr: every line is surfaced through tracing so
/// the child can never block on a full stderr pipe.
fn spawn_stderr_drain_thread(reader: std::process::ChildStderr) {
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut reader = std::io::BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end();
                    if !trimmed.is_empty() {
                        tracing::info!("[PIPECAT-SIDECAR] {trimmed}");
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Pipecat Supervisor overseeing the Python sidecar process.
pub struct PipecatSupervisor {
    internals: Arc<SupervisorInternals>,
}

impl PipecatSupervisor {
    pub fn new(config: PipecatConfig) -> Result<Self, SupervisorError> {
        let job_object = WindowsJobObject::new()?;
        let backoff_cap = 30_000u64;
        tracing::info!(
            "[PIPECAT-SUPERVISOR] Supervisor initialized (startup_timeout={}s, restart_backoff={}ms, max_restarts={})",
            config.startup_timeout_secs,
            config.restart_backoff_ms,
            config.max_restart_retries
        );
        let _ = backoff_cap;
        Ok(Self {
            internals: Arc::new(SupervisorInternals {
                job_object,
                config,
                current_pid: AtomicU32::new(0),
                child: Mutex::new(None),
                state: Mutex::new(PipecatState::Stopped),
                is_ready: AtomicBool::new(false),
                stopping: AtomicBool::new(false),
                stdout_lines: Arc::new(Mutex::new(VecDeque::new())),
                ready_signal: Arc::new(AtomicBool::new(false)),
            }),
        })
    }

    pub fn config(&self) -> &PipecatConfig {
        &self.internals.config
    }

    pub fn state(&self) -> PipecatState {
        *self.internals.state.lock()
    }

    pub fn pid(&self) -> Option<u32> {
        let pid = self.internals.current_pid.load(Ordering::SeqCst);
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }

    pub fn is_ready(&self) -> bool {
        self.internals.is_ready.load(Ordering::SeqCst)
    }

    /// Resolve the path to the Python runtime executable.
    pub fn resolve_python_path(&self) -> Result<PathBuf, SupervisorError> {
        resolve_python_path_for(&self.internals.config)
    }

    /// Resolve the location of the Pipecat sidecar script.
    pub fn resolve_script_path(&self) -> Result<PathBuf, SupervisorError> {
        resolve_script_path_for(&self.internals.config)
    }

    /// Launch and supervise the Pipecat sidecar process.
    ///
    /// Readiness is explicit: the sidecar must emit its `{"status":"ready"}` JSON
    /// handshake on stdout within `startup_timeout_secs`, otherwise the child is
    /// killed and the launch fails. A health-monitor thread watches the child and
    /// performs crash restarts with configured backoff while the supervisor lives.
    pub fn start(&self) -> Result<(), SupervisorError> {
        let internals = &self.internals;
        if !internals.config.enabled {
            tracing::info!("[PIPECAT-SUPERVISOR] Pipecat sidecar is disabled in configuration");
            return Ok(());
        }

        {
            let child_guard = internals.child.lock();
            if child_guard.is_some() {
                let pid = internals.current_pid.load(Ordering::SeqCst);
                return Err(SupervisorError::AlreadyRunning(pid));
            }
        }

        internals.stopping.store(false, Ordering::SeqCst);
        internals.reset_child_state();
        *internals.state.lock() = PipecatState::Starting;
        internals.is_ready.store(false, Ordering::SeqCst);

        internals.spawn_and_bind()?;
        internals.await_readiness()?;

        *internals.state.lock() = PipecatState::Ready;
        internals.is_ready.store(true, Ordering::SeqCst);

        // ── Health monitor + bounded-restart crash recovery ──
        spawn_supervision_loop(Arc::clone(&self.internals));

        tracing::info!(
            pid = internals.current_pid.load(Ordering::SeqCst),
            "[PIPECAT-SUPERVISOR] Pipecat sidecar readiness handshake confirmed; supervision active"
        );
        Ok(())
    }

    /// Active health check: sends a supervisor Ping and waits for the sidecar's
    /// Pong on stdout within the timeout.
    pub fn health_check(&self, timeout: Duration) -> Result<Vec<String>, SupervisorError> {
        let internals = &self.internals;
        if internals.child.lock().is_none() {
            return Err(SupervisorError::ReadinessTimeout(
                "no running sidecar process".to_string(),
            ));
        }

        internals.drain_stdout_lines();
        internals.write_stdin_line("Ping");

        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            for line in internals.peek_stdout_lines() {
                if line.contains("Pong") {
                    return Ok(internals.drain_stdout_lines());
                }
            }
            if internals.child_exited() {
                return Err(SupervisorError::ReadinessTimeout(
                    "sidecar exited during health check".to_string(),
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Err(SupervisorError::ReadinessTimeout(format!(
            "no Pong within {:?}",
            timeout
        )))
    }

    /// Stop the Pipecat sidecar gracefully.
    pub fn stop(&self) -> Result<(), SupervisorError> {
        let internals = &self.internals;
        internals.stopping.store(true, Ordering::SeqCst);
        if internals.child.lock().is_some() {
            let pid = internals.current_pid.load(Ordering::SeqCst);
            internals.is_ready.store(false, Ordering::SeqCst);
            *internals.state.lock() = PipecatState::Terminated;

            tracing::info!(pid, "[PIPECAT-SUPERVISOR] Stopping Pipecat sidecar");
            internals.interrupt_child();
        }
        Ok(())
    }
}

impl Drop for PipecatSupervisor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Health monitor thread: watches the child process and recovers crashes with
/// bounded restart attempts and configured backoff. One active production
/// sidecar is enforced at all times. Exits when `stopping` is set.
fn spawn_supervision_loop(internals: Arc<SupervisorInternals>) {
    std::thread::spawn(move || {
        let config = internals.config.clone();
        let mut restarts = 0usize;
        let mut backoff = config.restart_backoff_ms.max(100);
        let backoff_cap = 30_000u64;

        while !internals.stopping.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(500));
            if internals.stopping.load(Ordering::SeqCst) {
                break;
            }

            let (exited, exit_status) = {
                let mut guard = internals.child.lock();
                match guard.as_mut() {
                    Some(c) => match c.try_wait() {
                        Ok(Some(status)) => (true, Some(status)),
                        Ok(None) => (false, None),
                        Err(_) => (false, None),
                    },
                    None => (false, None),
                }
            };

            if !exited {
                continue;
            }

            tracing::warn!(
                exit = ?exit_status,
                "[PIPECAT-SUPERVISOR] Sidecar exited unexpectedly"
            );
            internals.is_ready.store(false, Ordering::SeqCst);

            if !config.auto_restart || restarts >= config.max_restart_retries {
                *internals.state.lock() = PipecatState::Degraded;
                tracing::error!(
                    restarts,
                    max = config.max_restart_retries,
                    "[PIPECAT-SUPERVISOR] Sidecar unrecoverable; marked Degraded (no further restarts)"
                );
                break;
            }

            restarts += 1;
            *internals.state.lock() = PipecatState::Starting;
            tracing::info!(
                restarts,
                backoff_ms = backoff,
                "[PIPECAT-SUPERVISOR] Restarting sidecar after backoff"
            );
            std::thread::sleep(Duration::from_millis(backoff));
            if internals.stopping.load(Ordering::SeqCst) {
                break;
            }

            internals.reset_child_state();
            let spawn_result = internals.spawn_and_bind();

            match spawn_result {
                Ok(()) => match internals.await_readiness() {
                    Ok(()) => {
                        *internals.state.lock() = PipecatState::Ready;
                        internals.is_ready.store(true, Ordering::SeqCst);
                        restarts = 0;
                        backoff = config.restart_backoff_ms.max(100);
                        tracing::info!(
                            "[PIPECAT-SUPERVISOR] Restarted sidecar reached readiness handshake"
                        );
                    }
                    Err(e) => {
                        tracing::error!(restarts, error = %e, "[PIPECAT-SUPERVISOR] Restart readiness failed");
                        backoff = (backoff * 2).min(backoff_cap);
                    }
                },
                Err(e) => {
                    tracing::error!(restarts, error = %e, "[PIPECAT-SUPERVISOR] Restart spawn failed");
                    backoff = (backoff * 2).min(backoff_cap);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a temporary sidecar script and produce a config pointing at it.
    fn test_config(py_body: &str) -> (PipecatConfig, PathBuf) {
        let script_path = std::env::temp_dir().join(format!(
            "osmoo_pipecat_supervisor_test_{}.py",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&script_path, py_body).expect("write test sidecar script");
        let config = PipecatConfig {
            script_path: script_path.clone(),
            host: "127.0.0.1".to_string(),
            port: 0, // sidecar test mode: no real TCP dial
            ..Default::default()
        };
        (config, script_path)
    }

    #[test]
    fn test_pipecat_config_defaults() {
        let config = PipecatConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 18889);
        assert!(config.enabled);
        assert!(config.auto_restart);
        assert_eq!(config.max_restart_retries, 3);
        assert_eq!(config.startup_timeout_secs, 20);
        assert_eq!(config.restart_backoff_ms, 2_000);
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

    /// Readiness handshake: sidecar prints ready line -> supervisor reports Ready.
    #[test]
    fn test_start_reaches_ready_only_after_handshake() {
        let body: &str = r#"import json, time, sys
import time as _t
_t.sleep(0.2)
print(json.dumps({'status': 'ready'}), flush=True)
while True:
    line = sys.stdin.readline()
    if not line:
        break
"#;
        let (config, script) = test_config(body);
        let supervisor = PipecatSupervisor::new(config).unwrap();
        let result = supervisor.start();
        assert!(
            result.is_ok(),
            "start should succeed after handshake: {result:?}"
        );
        assert!(supervisor.is_ready());
        assert_eq!(supervisor.state(), PipecatState::Ready);
        assert!(supervisor.pid().is_some());
        supervisor.stop().unwrap();
        assert_eq!(supervisor.state(), PipecatState::Terminated);
        std::fs::remove_file(script).ok();
    }

    /// A sidecar that never emits the handshake must time out and be killed,
    /// and must report a real readiness error rather than a fake ready state.
    #[test]
    fn test_start_fails_when_handshake_missing() {
        let (mut config, script) = test_config(
            "import time\n\
             time.sleep(5)\n",
        );
        config.startup_timeout_secs = 1;
        let supervisor = PipecatSupervisor::new(config).unwrap();
        let result = supervisor.start();
        assert!(matches!(result, Err(SupervisorError::ReadinessTimeout(_))));
        assert!(!supervisor.is_ready());
        supervisor.stop().ok();
        std::fs::remove_file(script).ok();
    }

    /// A sidecar that crashes before reaching ready must fail with real exit evidence.
    #[test]
    fn test_start_fails_when_sidecar_exits_early() {
        let (config, script) = test_config("import sys\nsys.exit(3)\n");
        let supervisor = PipecatSupervisor::new(config).unwrap();
        let result = supervisor.start();
        assert!(
            matches!(result, Err(SupervisorError::ReadinessTimeout(_))),
            "got {result:?}"
        );
        std::fs::remove_file(script).ok();
    }

    /// Health check: supervisor Ping must produce a sidecar Pong.
    #[test]
    fn test_health_check_ping_pong() {
        let body: &str = "import json, sys\nprint(json.dumps({'status': 'ready'}), flush=True)\nwhile True:\n    line = sys.stdin.readline()\n    if not line:\n        break\n    try:\n        cmd = json.loads(line)\n    except Exception:\n        continue\n    if cmd.get('type') == 'Ping':\n        print(json.dumps({'type': 'Pong', 'status': 'ok'}), flush=True)\n";
        let (config, script) = test_config(body);
        let supervisor = PipecatSupervisor::new(config).unwrap();
        supervisor.start().expect("sidecar should become ready");
        let pong = supervisor.health_check(Duration::from_secs(5));
        assert!(pong.is_ok(), "expected Pong, got {pong:?}");
        supervisor.stop().unwrap();
        std::fs::remove_file(script).ok();
    }
}
