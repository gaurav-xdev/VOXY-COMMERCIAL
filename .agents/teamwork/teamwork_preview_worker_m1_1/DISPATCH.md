# Dispatch for Worker M1 (Windows Application Lifecycle, Daemon Supervision & IPC)

Target Working Directory: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m1_1\
Reference Specification: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\ORIGINAL_REQUEST.md
Project Blueprint: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\orchestrator_1\PROJECT.md

Mandatory Integrity Warning:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Scope & Tasks:
Milestone M1: R1 & A1.
Files Owned: `crates/desktop_ui/`, `apps/daemon/`, `apps/overlay/`, `crates/ipc/`
1. Primary Executable & Supervisor:
   - Ensure `crates/desktop_ui` (`OSMOO.exe`) serves as the single unified entry point.
   - Implement reliable supervisor lifecycle managing the background daemon (`apps/daemon`) and overlay (`apps/overlay`).
   - Guarantee that terminating `OSMOO.exe` cleanly shuts down all child processes (daemon and overlay) with zero orphaned processes or zombie handles.
2. Authenticated Named Pipe IPC:
   - Enforce IPC authentication via local Windows Named Pipe (`\\.\pipe\voxy-com-ipc`).
   - Validate messages, enforce timeouts, handle graceful reconnection on disconnects, and ensure clean pipe closure.
3. Verification:
   - Run tests for `crates/ipc` (`cargo test -p voxy-ipc`) and `crates/desktop_ui`.
   - Verify that primary executable starts, connects to pipe, and handles clean shutdown.

Write your completion report in:
`c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m1_1\handoff.md`


## 2026-10-14T03:20:09Z
From: 0ad9b732-5c55-4e82-980c-1607f28d0b35
Content:
You are Worker M1 for the OSMOO production mission.
Your working directory is:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m1_1\

You MUST read the original request before doing anything:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\ORIGINAL_REQUEST.md

Also read the project architecture blueprint:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\orchestrator_1\PROJECT.md

Also read your dispatch task:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m1_1\DISPATCH.md

DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Exclusive Write Ownership:
- `crates/desktop_ui/`
- `apps/daemon/`
- `apps/overlay/`
- `crates/ipc/`

Mission (R1 & A1):
1. Primary Executable & Supervisor:
   - Ensure `crates/desktop_ui` (`OSMOO.exe`) is the unified Windows entry point.
   - Implement/verify supervisor lifecycle managing background daemon (`apps/daemon`) and overlay (`apps/overlay`).
   - Guarantee that closing or terminating `OSMOO.exe` shuts down daemon and overlay child processes with zero orphaned processes or zombie handles.
2. Authenticated Named Pipe IPC:
   - Enforce IPC authentication via local Windows Named Pipe (`\\.\pipe\voxy-com-ipc`).
   - Validate messages, timeouts, graceful reconnection on disconnects, and clean pipe closure.
3. Verification:
   - Run tests: `cargo test -p voxy-ipc` and tests for `desktop_ui`.
   - Verify that process lifecycle and IPC contracts are met.

Write your completion report in:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m1_1\handoff.md
Maintain your liveness in:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m1_1\progress.md

When done, send a message to your caller notifying them that handoff.md is ready.
