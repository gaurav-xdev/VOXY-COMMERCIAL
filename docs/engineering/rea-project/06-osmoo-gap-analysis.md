# OSMOO Architectural Gap Analysis

## 1. Context
Following the complete removal of the legacy coding harness from OSMOO, the remaining codebase consists of:
- `apps/daemon`: IPC pipe server, voice pipeline, Kokoro TTS, STT, tray, background audio.
- `crates/security`: `ApprovalBroker`, `path_guard` (`validate_path`, `is_protected_file`, `contains_symlink_or_reparse`), `SecretScanner`, in-memory `AiActionJournal`.
- `crates/model_router`: Provider health checking, circuit breaking, fallbacks.
- `crates/tool_calling`: MCP client, ToolRegistry, function calling schemas.
- `crates/skills`: `SKILL.md` parser and validator.
- `crates/desktop_ui`: Slint-based GUI window and voice visualizer overlay.

## 2. Identified Gaps

### 2.1 Missing Autonomous Coding Engine
- **Current State**: OSMOO has no coding loop, planning DAG, or automated patch applicator.
- **Required State**: `crates/coding_harness` (`voxy-coding-harness`) providing full task decomposition, context indexing, atomic patch application, build/test execution, review, and verified repair loop.

### 2.2 ModelRouter Fallback Leak
- **Current State**: In `crates/model_router/src/lib.rs`, when a provider fails, `select_fallback` iterates over `all_healthy` and returns another provider regardless of `config.mode`.
- **Required State**: Strict zero-fallback when `ForceProvider` or `ForceModel` is specified. If the forced provider is unavailable, immediately return an error (`RoutingError::ForcedProviderUnavailable`).

### 2.3 Durable Multi-File Transaction Journal
- **Current State**: `crates/security/src/journal.rs` provides an in-memory `Vec<AiActionEntry>`. It is destroyed on process crash or restart.
- **Required State**: Disk-backed write-ahead transaction directory (`.osmoo/journal/<tx_id>/`) with `tx.json`, pre-image file backups, atomic commit markers, and startup crash recovery.

### 2.4 Windows Subprocess Leaks
- **Current State**: Commands executed via standard `tokio::process::Command` leave child processes (compiler daemons, build scripts, test runners) dangling if killed or timed out.
- **Required State**: Win32 Job Object abstraction with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` that encapsulates the process and all child trees.

### 2.5 Security Path Canonicalization
- **Current State**: `voxy_security::path_guard::validate_path(root, rel)` strictly errors if `rel` is absolute (`"Path must be relative to the workspace"`).
- **Required State**: The coding harness must canonicalize all file paths relative to the canonical workspace root before invoking `validate_path`.

### 2.6 Multi-Language Toolchain Detection
- **Current State**: Hardcoded commands in tests without dynamic toolchain detection.
- **Required State**: Toolchain detection engine capable of identifying available compilers (Rust `cargo`, Go `go`, Java `javac`, Clang `clang`, SQLite `sqlite3`) and skipping unavailable ones (e.g. C# `dotnet`) with explicit skipped status.
