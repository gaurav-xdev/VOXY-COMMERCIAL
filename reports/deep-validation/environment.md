# Environment & Test Runner Context

## 1. Host and Workspace
- **Operating System:** Microsoft Windows 11 Enterprise (x86_64)
- **Repository Root:** `c:\Users\gaurav\Downloads\VOXY-COMMERCIAL`
- **Git HEAD Revision:** `794704ec3943020380e990c90d928c625b6a982a`
- **Compiler / Toolchain:** Rust `1.76+` (MSVC x86_64), Cargo, NSIS 3.10
- **Test Execution Mode:** Text-only session (Zero live TTS / STT audio API requests invoked per session instructions)

## 2. Isolation Architecture & Boundaries
- **Ephemeral Filesystem Workspace:** Created via `tempfile::tempdir()`.
  > [!IMPORTANT]
  > `tempfile::tempdir()` provides filesystem namespace scoping for test artifacts and directory traversal checks. It does **not** provide an operating-system level security sandbox (processes, Windows registry, named pipes, and system calls remain shared with the host).
- **Invasive Action Policy:**
  - Potentially invasive operations (host registry modification, system-level process killing, network interface tampering, driver inspection) are **BLOCKED** from executing on the bare host without a dedicated virtual machine or Windows Sandbox instance.
  - Harness execution is restricted strictly within the canonicalized ephemeral fixture directory.
  - Path canonicalization with verbatim prefix stripping blocks symlink/reparse-point directory traversal escapes.
  - Atomic temporary writes (`tmp_{uuid}` + `fs::rename`) prevent partial write corruption.
  - Protected configuration paths (`.git/config`, `Cargo.lock`, etc.) and credential leaks (`AKIA...`, `ghp_...`) are blocked before disk write.
  - Process tree termination (`taskkill /F /T /PID` on Windows) is enforced to prevent orphaned child processes upon timeout or cancellation.
