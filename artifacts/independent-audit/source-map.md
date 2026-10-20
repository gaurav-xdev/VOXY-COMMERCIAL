# OSMOO Critical Subsystem Source Map & Traceability Guide

**Audit Date:** 2026-10-20  
**Repository:** `C:\Users\gaurav\Downloads\VOXY-COMMERCIAL`  
**Purpose:** End-to-end line-numbered source evidence for independent architectural evaluation.

---

## Subsystem A: Coding Harness & Agent Sandboxing

### 1. Process Sandboxing & Tree Termination (`crates/harness/src/sandbox_runner.rs`)
- **Process Tree Kill (`SandboxedRunner::kill_pid_tree`, lines 69–86)**:
  Uses Windows Win32 `taskkill.exe /F /T /PID` to recursively terminate child processes and grandchildren on timeout or cancellation:
  ```rust
  pub fn kill_pid_tree(pid: u32) {
      #[cfg(target_os = "windows")]
      {
          let _ = std::process::Command::new("taskkill")
              .args(["/F", "/T", "/PID", &pid.to_string()])
              .stdout(Stdio::null())
              .stderr(Stdio::null())
              .status();
      }
  }
  ```
- **RAII Process Guard (`ChildProcessGuard`, lines 179–195)**:
  Ensures that if the tokio task drops, panics, or cancels, child processes cannot become orphaned zombies:
  ```rust
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
  ```
- **Cancellation & Emergency Stop Polling (lines 203–229)**:
  Polls `emergency_stop: Arc<AtomicBool>` every 50ms during command execution. When signaled, triggers `kill_pid_tree(p)` and returns `RunnerError::EmergencyStopAborted`.

### 2. Path Containment & Protected File Enforcer (`crates/harness/src/patch_engine.rs`)
- **Protected File Filter (`is_protected_file`, lines 46–105)**:
  Rejects patches targeting `.git`, `.ssh`, `cargo.lock`, `.env`, or SSH private keys:
  ```rust
  pub fn is_protected_file(path: &Path) -> bool {
      // Rejects .git, .ssh, cargo.lock, .env, id_rsa, id_ed25519
  }
  ```
- **Reparse-Point & Symlink Containment (`contains_symlink_or_reparse`, lines 108–160)**:
  On Windows, checks `FILE_ATTRIBUTE_REPARSE_POINT` using `std::os::windows::fs::MetadataExt`. Rejects symlinks escaping the repository directory.
- **Atomic Patch Rollback (`apply_patches_with_rollback`, lines 235–310)**:
  Captures pre-patch snapshots. If any file conflict, boundary violation, or secret is detected, all modified files are restored to their original snapshot state.

### 3. Secret Scanner (`crates/harness/src/secret_scanner.rs`)
- **Entropy & Pattern Matching (`SecretScanner::scan_text`, lines 45–90)**:
  Scans proposed patches using regex and Shannon entropy algorithms for AWS access keys (`AKIA[0-9A-Z]{16}`), GitHub PATs (`ghp_...`), private key headers (`-----BEGIN RSA PRIVATE KEY-----`), and provider API keys (`sk-...`).

---

## Subsystem B: Desktop Control & Windows UI Automation

### 1. Agentic Loop State Machine (`crates/automation/src/agentic_loop.rs`)
- **State Machine (lines 30–52)**:
  `LoopState`: `Idle`, `Observing`, `Planning`, `AwaitingApproval`, `Acting`, `Verifying`, `Replanning`, `Paused`, `Completed`, `Failed`, `Cancelled`, `EmergencyStopped`.
  Terminal states: `Completed`, `Failed`, `Cancelled`, `EmergencyStopped`.
- **Human-in-the-Loop Governance (`ApprovalBroker`, lines 350–420)**:
  Consequential or destructive actions (closing windows, filesystem modifications) request authorization via `voxy_security::ApprovalBroker`. If rejected, state transitions to `LoopState::Failed` with `ApprovalDenied`.
- **Kill-Switch (`AgenticLoop::emergency_stop`, lines 460–485)**:
  Sets `AtomicBool` stop token, halts in-flight cursor moves or key presses, and halts UIA tree iteration.

### 2. Windows UI Automation Backend (`crates/automation/src/backends/windows_uia.rs`)
- **Element Tree Discovery (`observe_element_tree`, lines 85–180)**:
  Initializes Windows COM UIAutomation interface `IUIAutomation`, queries root element `GetRootElement`, and constructs hierarchical UI tree of controls with bounding rectangles, automation IDs, and class names.

---

## Subsystem C: Voice Pipeline & Audio DSP

### 1. Audio Conditioning & DSP (`crates/voice_audio/src/conditioning.rs`)
- **80Hz High-Pass Filter (lines 73–84 & 130–145)**:
  Eliminates DC offset and rumble using WebRTC APM standard pole:
  ```rust
  // rc = 1.0 / (2 * pi * 80) = ~0.001989, dt = 1.0 / 16000
  // alpha = rc / (rc + dt) = 0.9695f32
  let hp_y = self.hp_alpha * (self.hp_prev_y + sample - self.hp_prev_x);
  ```
- **Adaptive Noise Gate with Hysteresis (lines 146–185)**:
  Tracks smoothed energy envelope. When energy falls below `noise_gate_threshold` (default 0.008 RMS), applies soft attenuation (`noise_attenuation` = 0.15 / -14dB) rather than hard silence, preventing unnatural syllable chopping.
- **Acoustic Echo Suppression (lines 88–95 & 195–215)**:
  When `is_speaker_active` is true (assistant audio playing), capture input is attenuated by `echo_attenuation` (0.08 / -26dB) to prevent feedback loops into STT.

### 2. Voice Session & Barge-In Controller (`crates/voice_orchestrator/src/lib.rs`)
- **Barge-In Cancellation (`VoicePipeline::handle_vad_speech`, lines 480–550)**:
  ```rust
  if let Some(cancel) = self.active_cancellation.lock().take() {
      cancel.cancel(); // Halts in-flight LLM streaming task & TTS playback
  }
  self.turn_id.fetch_add(1, Ordering::SeqCst); // Stale audio chunks discarded
  ```
- **Sentence-Aware Streaming TTS (`SentenceChunker`, lines 600–650)**:
  Splits LLM token stream at sentence delimiters (`.`, `!`, `?`, `\n`) so TTS synthesis begins on chunk 1 before chunk 2 finishes generation.

---

## Subsystem D: Daemon, IPC & Process Lifecycle

### 1. Windows Job Object Supervision (`crates/desktop_ui/src/supervisor.rs`)
- **Job Object Setup (`WindowsJobObject::new`, lines 45–80)**:
  ```rust
  let job = CreateJobObjectW(None, None)?;
  let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
  info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
  SetInformationJobObject(job, JobObjectExtendedLimitInformation, ...);
  ```
- **Child Process Ownership (`assign_process`, lines 85–100)**:
  Spawns `voxy-daemon.exe` and assigns its handle via `AssignProcessToJobObject`.
  Guarantees that if `OSMOO.exe` is killed or crashes, the Windows kernel terminates `voxy-daemon.exe`.

### 2. Named-Pipe IPC Security (`crates/ipc/src/auth_middleware.rs` & `named_pipe.rs`)
- **Pipe Address**: `\\.\pipe\voxy-com-ipc`.
- **HMAC Capability Token Verification**: Validates issued-at, expiry, token ID, and required permissions (`capability`, `resource`, `constraints`).
- **Fail-Closed Default**: If trust store is empty, all authentication attempts are rejected.

---

## Subsystem E: UI, Authentication, Storage & Settings

### 1. Authentication Bridge & Windows DPAPI (`crates/desktop_ui/src/bridge/auth.rs`)
- **DPAPI Protection (`dpapi_protect`, lines 240–270)**:
  Calls `crypt32.dll::CryptProtectData` to encrypt `session.json` using the active Windows logon session key:
  ```rust
  let mut in_blob = DATA_BLOB { cbData: plaintext.len() as u32, pbData: plaintext.as_mut_ptr() };
  let mut out_blob = DATA_BLOB::default();
  CryptProtectData(&mut in_blob, ...);
  ```
- **Session Recovery & Invalidation (`AuthBridge::load_session`, lines 180–230)**:
  Decrypts via `CryptUnprotectData`. If corrupted or expired, deletes file and returns unauthenticated state.

### 2. SQLite WAL Persistence (`crates/desktop_ui/src/backend/dispatcher.rs`)
- **Prompt & History Hydration (`BackendDispatcher::dispatch`, lines 80–180)**:
  - `BackendCommand::SendPrompt`: commits user message to `SqliteConversationStore` (`conversations.db`), invokes cognitive reasoning, commits assistant response turn.
  - `BackendCommand::GetChatHistory`: queries stored turns for startup hydration.
  - `BackendCommand::SaveSettings`: persists mutated configuration atomically to `settings.toml`.
