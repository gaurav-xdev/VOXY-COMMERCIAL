# Environment & Test Runner Context

## 1. Host and Workspace
- **Operating System:** Microsoft Windows 11 Enterprise (x86_64)
- **Repository Root:** `c:\Users\gaurav\Downloads\VOXY-COMMERCIAL`
- **Git HEAD Revision:** `794704ec3943020380e990c90d928c625b6a982a`
- **Compiler / Toolchain:** Rust `1.76+` (MSVC x86_64), Cargo, NSIS 3.10
- **Test Execution Mode:** Text-only session (Zero live TTS / STT audio API requests invoked per session instructions)

## 2. Isolation Architecture
- **Disposable Testing Fixture:** Dynamic ephemeral directory created via `tempfile::tempdir()`
- **Safety Policy:**
  - Zero modifications to host system registries or user startup keys.
  - Zero destructive modifications to host desktop or user repositories.
  - Sandboxed execution within isolated workspace directories.
  - Path boundary violation checks enforced (`../../` traversal rejected).
  - Protected file protection enforced (`.git/config` write rejected).
