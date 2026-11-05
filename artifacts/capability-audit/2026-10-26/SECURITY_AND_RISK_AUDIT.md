# OSMOO Security, Privacy & Risk Audit

**Date:** 2026-10-28  
**Audit Scope:** `crates/security`, `crates/coding_harness`, `crates/automation`, `apps/daemon`  
**Security Posture:** Enterprise-Grade Defensive Architecture

---

## 1. Threat Model & Security Posture Overview

Because OSMOO possesses active operating system privileges (desktop control, keystroke emulation, process management, file I/O, and shell execution), the system operates under an **assumed-hostile execution model**:
1. All user voice inputs and web-scraped contents are treated as untrusted text capable of prompt injection or indirect command hijacking.
2. All file paths requested by tools or LLMs are treated as potentially malicious path traversal attempts (`../../Windows/System32`).
3. All network requests are checked to prevent Server-Side Request Forgery (SSRF) against internal cluster services or cloud instance metadata.
4. All toolchain and compiler invocations are run under strict Windows Job Object sandboxes to prevent fork bombs, runaway loops, or excessive memory consumption.

---

## 2. Core Security Controls & Verification

### A. Human-in-the-Loop Approval Broker (`voxy-security::approval`)
- **Mechanism:** For high-impact, destructive, or irreversible actions (e.g., `file_delete`, `process_kill`, executing unknown binaries), the engine invokes `ApprovalBroker::request_approval()`.
- **Enforcement:** Actions requiring confirmation are blocked until explicitly approved via IPC from the user desktop UI or CLI prompt.
- **Verification:** Unit test `tools::tests::test_emergency_stop_aborts_tool_execution` verifies that unauthorized or stopped actions are rejected immediately with descriptive telemetry.

### B. Path Traversal & Boundary Guard (`voxy-security::path_guard`)
- **Mechanism:** `PathGuard::validate_path()` resolves canonical paths using Win32 API `GetFinalPathNameByHandle` to neutralize directory traversal (`..`), alternate data streams (`::$DATA`), and symlink junction points pointing outside the project root.
- **Verification:** Rejects absolute references outside the designated workspace root with a `PathTraversalError`.

### C. Server-Side Request Forgery (SSRF) Guard (`voxy-security::ssrf_guard`)
- **Mechanism:** Outbound HTTP clients (used in `research_deep_investigate` and model provider adapters) validate destination IP addresses before connection.
- **Blocked Ranges:**
  - `127.0.0.0/8` (Loopback)
  - `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16` (RFC 1918 Private)
  - `169.254.169.254` (Cloud Instance Metadata Service - AWS, GCP, Azure)
  - `::1` (IPv6 loopback)

### D. Process Isolation & Resource Limits (`voxy-coding-harness::process_runner`)
- **Mechanism:** Compilers, tests, and CLI backends run inside dedicated Windows Job Objects.
- **Resource Constraints:**
  - Memory limit enforced via `JOB_OBJECT_LIMIT_PROCESS_MEMORY`.
  - Process tree kill guarantees: When a test times out, the Job Object terminates all child processes, preventing orphan zombie background workers.
  - Hard timeouts (default 30 seconds for standard commands, configurable per task).

### E. Prompt Injection & System Prompt Leak Defense
- **User Prompt Sanitization:** `voxy_security::sanitize_user_input` escapes known control delimiters.
- **Output Sanitization:** `sanitize_llm_output()` inspects generated tokens for system prompt leakage, API keys, or embedded secrets, suppressing them before delivery to the user or TTS engine.
- **Command Injection Prevention:** Spawning applications uses structured argument vectors (`Command::new(binary).args(&[...])`) rather than concatenating untrusted text into `cmd.exe /c` or PowerShell strings. Verified in `tools::tests::test_safe_launch_blocks_command_injection`.

---

## 3. Residual Risks & Recommended Hardening

1. **GDI Keystroke Injection:** Win32 `SendInput` operates at the desktop session level. If the focused window shifts during typing, keystrokes will land in the newly focused window.
   - *Recommendation:* Require explicit window handle locking before dispatching keystroke streams.
2. **Local Audio Privacy:** While audio processing is local, model routing to external providers (`openai`, `anthropic`) transmits transcribed text to the cloud.
   - *Recommendation:* Provide a prominent UI badge distinguishing "Local Sovereign Mode" from "Cloud Connected Mode".
