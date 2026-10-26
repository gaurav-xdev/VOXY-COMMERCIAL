# OSMOO Bug and Defect Audit

## 1. Summary of Identified Defects

### 1.1 ModelRouter Silent Fallback Bug
- **Location**: `crates/model_router/src/lib.rs`
- **Issue**: In `select_provider()`, when a forced provider (via `RoutingMode::ForceProvider` or `ForceModel`) is unhealthy or has an open circuit breaker, the code falls back to `select_fallback(all_healthy)`.
- **Impact**: Silently routes requests to alternate cloud providers despite explicit configuration requiring a specific provider or local-only inference.
- **Fix**: Check `self.config.mode` inside `select_fallback` or `select_provider`. If mode is `ForceProvider` or `ForceModel`, abort immediately and return `RoutingError::ForcedProviderUnavailable` without checking fallback candidates.

### 1.2 In-Memory Only AiActionJournal
- **Location**: `crates/security/src/journal.rs`
- **Issue**: `AiActionJournal` stores audit entries in a standard in-memory vector `Vec<AiActionEntry>`.
- **Impact**: If the daemon crashes or the system reboots mid-operation, all transaction history is lost. No write-ahead logs exist on disk to revert incomplete multi-file edits.
- **Fix**: In `crates/coding_harness`, implement `TransactionJournal` writing to `.osmoo/journal/<tx_id>/` with pre-image file backups and an atomic completion sentinel. Perform automatic rollback scans during harness initialization.

### 1.3 Unbounded Windows Subprocess Tree Termination
- **Location**: Subprocess execution across tools
- **Issue**: Killing a root process handle on Windows (`process.kill()`) does not terminate child processes spawned by compilers, shells, or test harnesses (e.g., node child workers, rustc, build scripts).
- **Impact**: Accumulates orphan processes, consumes system RAM/CPU, and locks workspace files preventing subsequent edits.
- **Fix**: Create a Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` for every executed command tree. Assign the child process to the Job Object immediately upon creation.

### 1.4 Path Guard Absolute Path Handling
- **Location**: `crates/security/src/path_guard.rs`
- **Issue**: `validate_path` returns an error if passed an absolute path (`"Path must be relative to the workspace"`). However, tool schemas and agent prompts frequently provide canonical absolute file paths.
- **Impact**: Unhandled absolute paths cause immediate tool failure.
- **Fix**: In `voxy-coding-harness`, canonicalize workspace paths and convert absolute target paths to relative paths against the canonical workspace root before invoking `path_guard::validate_path`.
