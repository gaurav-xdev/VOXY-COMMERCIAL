# Claude CLI Backend (`ClaudeCliBackend`) Audit Report

## 1. Overview & Verification

The Claude CLI Backend is implemented in [`crates/coding_harness/src/claude_cli_backend.rs`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/claude_cli_backend.rs). It serves as an optional delegated execution provider for subagent workflows when Claude Code CLI is present on the host system.

## 2. Environment Verification
- **Host Binary**: Verified present at `C:\Users\gaurav\AppData\Roaming\npm\claude.cmd` / `claude.ps1`.
- **Version Reported**: `2.1.215 (Claude Code)`
- **Discovery Mechanism**:
  `ClaudeCliBackend::detect_cli()` inspects:
  1. `CLAUDE_CLI_PATH` environment override.
  2. Standard NPM global roaming directory on Windows (`%APPDATA%\npm\claude.cmd`).
  3. `where.exe claude` or system `PATH`.

## 3. Windows Execution Syntax & Fix
- **Issue Discovered**: In Windows PowerShell environments, executing a quoted path containing spaces or extension (e.g., `"C:\...\claude.cmd" --version`) results in a PowerShell syntax error (`Unexpected token 'version' in expression or statement`).
- **Correction Applied**: On Windows, PowerShell commands spawned through `ProcessRunner` now prepend the call operator `& `:
  ```rust
  let invocation = if cfg!(windows) {
      format!("& \"{}\" {}", bin.display(), args)
  } else {
      format!("\"{}\" {}", bin.display(), args)
  };
  ```
- **Verification**: Verified using `claude_cli_backend::detect_cli()` which successfully executes `claude --version` and parses the semver string `2.1.215`.

## 4. Parameter Safety & Isolation
1. **Argument Sanitization**:
   User instructions are passed safely via dedicated `--prompt` flags or piped via stdin when interacting with Claude CLI, preventing command injection into the host shell.
2. **Timeout Enforcement**:
   All CLI executions are executed under `ProcessRunner::run_bounded` wrapped in a Windows Job Object. If Claude Code hangs or stalls beyond the configured timeout (default: 300,000ms), the Job Object terminates the entire process tree.
3. **Architectural Separation**:
   `ClaudeCliBackend` is completely isolated from the native Rust harness (`CodingHarnessEngine`). If Claude Code is uninstalled, corrupted, or unavailable, OSMOO's native DAG planner, patcher, debugger, and code reviewer continue operating with 100% functionality.
