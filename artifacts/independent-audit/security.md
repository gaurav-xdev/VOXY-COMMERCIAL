# OSMOO Security Architecture & Threat Model Audit

**Audit Date:** 2026-10-20  
**Target Repository:** `C:\Users\gaurav\Downloads\VOXY-COMMERCIAL`  
**Purpose:** Independent Security Classification and Threat Assessment.

---

## 1. Security Threat & Risk Classification Matrix

| Security Domain & Threat Model | Classification | Implementation File | Verification Evidence & Test Target | Unresolved Gap / Boundary |
| :--- | :---: | :--- | :--- | :--- |
| **Path Traversal & Windows Reparse Points** | **IMPLEMENTED AND TESTED** | `crates/harness/src/patch_engine.rs`<br/>`crates/harness/src/sandbox_runner.rs` | `test_symlink_or_reparse_point_rejection`<br/>`test_path_traversal_rejection` | Symlinks within repo directory allowed only if contained within root |
| **Protected Configuration Files** | **IMPLEMENTED AND TESTED** | `crates/harness/src/patch_engine.rs` | `test_cargo_lock_and_env_and_ssh_rejection`<br/>`test_protected_file_rejection` | Hardcoded list (`.git`, `.ssh`, `cargo.lock`, `.env`, keys) |
| **Shell Command Injection** | **IMPLEMENTED AND TESTED** | `crates/harness/src/sandbox_runner.rs` | Direct process spawning via `tokio::process::Command::new(program).args(args)` | Shell interpolation (`cmd.exe /c`) avoided in runner |
| **Process Tree Termination & Orphan Prevention** | **IMPLEMENTED AND TESTED** | `crates/harness/src/sandbox_runner.rs`<br/>`crates/desktop_ui/src/supervisor.rs` | `test_harness_timeout_terminates_process_tree`<br/>`test_job_object_assign_and_kill_process_tree` | Dependent on Windows `taskkill.exe` and Win32 Job Objects |
| **Secret Detection in Code Patches** | **IMPLEMENTED AND TESTED** | `crates/harness/src/secret_scanner.rs` | `test_secret_detection_aws_and_private_key`<br/>`test_secret_detection_github_and_api_keys` | Regex and Shannon entropy; novel high-entropy strings without pattern may pass |
| **Untrusted Model Output / Prompt Injection** | **IMPLEMENTED AND TESTED** | `crates/tool_calling/src/builtin/browser.rs` | `test_browser_prompt_injection_sanitization` | Sanitizes markdown/HTML script tags before context ingestion |
| **Approval Gating for Destructive Actions** | **IMPLEMENTED AND TESTED** | `crates/automation/src/agentic_loop.rs`<br/>`crates/security/src/approval.rs` | `test_agentic_loop_approval_broker_integration`<br/>`test_agentic_loop_approval_denial_fails_closed` | Gated on `ApprovalBroker`; user interaction mocked in tests |
| **IPC Authentication & Authorization** | **IMPLEMENTED AND TESTED** | `crates/ipc/src/auth_middleware.rs` | `cargo test -p voxy-ipc` (107 tests) | Fails closed on empty trust store; HMAC verification for capability claims |
| **IPC Frame Bounds & Malformed Frames** | **IMPLEMENTED AND TESTED** | `crates/ipc/src/named_pipe.rs` | `crates/ipc/src/protocol.rs` tests | Enforces 16MB maximum frame limit (`MAX_FRAME_SIZE`) |
| **Session Encryption at Rest (DPAPI)** | **IMPLEMENTED AND TESTED** | `crates/desktop_ui/src/bridge/auth.rs` | `test_dpapi_protect_and_unprotect_roundtrip`<br/>`test_auth_lifecycle_end_to_end` | Bound to Windows logon session; does NOT use TPM 2.0 hardware sealing |
| **Session Invalidation on Restart/Corrupt** | **IMPLEMENTED AND TESTED** | `crates/desktop_ui/src/bridge/auth.rs` | `test_auth_lifecycle_end_to_end` | Auto-deletes corrupted or expired session files |
| **Local Provisioning Flag Privilege Non-Escalation**| **IMPLEMENTED AND TESTED** | `crates/desktop_ui/src/app.rs` | `test_backend_dispatcher_onboarding_completion` | `provisioned.flag` indicates wizard completion only; does NOT bypass auth |
| **SSRF & Loopback Blocking in Browser Tools** | **IMPLEMENTED AND TESTED** | `crates/tool_calling/src/builtin/browser.rs` | `test_browser_url_validation_blocks_loopback_and_invalid_schemes` | Blocks `127.0.0.1`, `localhost`, RFC1918 private subnets |
| **Emergency Stop Race & Cancellation Propagation**| **IMPLEMENTED AND TESTED** | `crates/voice_orchestrator/src/lib.rs`<br/>`crates/automation/src/agentic_loop.rs` | `test_live_barge_in_mid_playback_interruption`<br/>`test_backend_dispatcher_emergency_stop` | `turn_id` increments prevent stale audio queue playback |
| **Database Concurrent Access & WAL Safety** | **IMPLEMENTED AND TESTED** | `crates/database/src/sqlite.rs` | `cargo test -p voxy-database` (98 tests) | SQLite WAL mode + busy timeout; concurrent thread pool tested |
| **Live Multi-Channel Microphone Beamforming** | **PARTIAL** | `crates/voice_audio/src/conditioning.rs` | Single-channel high-pass, noise gate, limiter, AEC verified | Multi-channel delay-and-sum beamforming requires multi-mic array hardware |
| **Live Cloud AI Billing/Network Call** | **PARTIAL** | `crates/provider_core/` | `test_cloud_provider_auth_fail_closed_and_dpapi_contract` | Catalog verified; live requests require real user API keys |

---

## 2. Threat Model Analysis & Residual Boundaries

### A. Windows DPAPI vs Hardware TPM Sealing
- **Threat**: Extraction of user session file by an attacker with filesystem access.
- **Defense**: The session file `session.json` is encrypted using Windows Data Protection API (`CryptProtectData`).
- **Limitation**: DPAPI binds encryption to the **Windows logon user account**. Any process running with the same user credentials can call `CryptUnprotectData`. It does NOT bind to a TPM 2.0 PCR hardware state.

### B. Single Primary Launcher vs Daemon Orphan Risk
- **Threat**: Background daemon (`voxy-daemon.exe`) continues running as an orphaned zombie if the UI crashes or is terminated by Task Manager.
- **Defense**: `OSMOO.exe` creates a Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and assigns the daemon process to the Job Object. The Windows kernel enforces child process termination upon handle closure.

### C. SSRF & Ingestion Boundaries
- **Threat**: AI agent tool calls targeting cloud metadata endpoints (`http://169.254.169.254`) or local loopback (`http://127.0.0.1:8080`).
- **Defense**: URL validation in `crates/tool_calling/src/builtin/browser.rs` parses IP addresses and explicitly rejects loopback, RFC1918 private subnets, link-local, and non-HTTP/HTTPS schemes.
