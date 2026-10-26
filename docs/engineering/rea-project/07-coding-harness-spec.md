# OSMOO Autonomous Senior Engineering Harness Specification

## 1. System Architecture
The new crate `crates/coding_harness` (`voxy-coding-harness`) serves as OSMOO's autonomous senior engineering runtime. It provides:
1. **Task Planning & DAG Management** (`planner.rs`): Decomposes natural language requests into structured subtasks with dependencies and status tracking (`pending`, `in_progress`, `completed`, `failed`).
2. **Context Exploration** (`context_explorer.rs`): Explores repositories using gitignore-aware directory walking (`ignore` crate) and pattern matching (`Grep`/`Glob`) within token budgets.
3. **Atomic File Patching** (`patcher.rs`): Applies unambiguous text replacements (`old_string` -> `new_string`) with unified diff calculation and conflict rejection.
4. **Durable Transaction Journal** (`journal.rs`): Disk-backed write-ahead log (`.osmoo/journal/<tx_id>/`) supporting multi-file atomic commit and rollback on failure or startup crash recovery.
5. **Sandboxed Process Runner** (`process_runner.rs`): Executes shell commands bounded by Win32 Job Objects (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), environment allowlists, and execution timeouts.
6. **Code Review & Automated Repair Loop** (`reviewer.rs`, `debugger.rs`): Inspects compiler/test errors, analyzes root causes, and guides iterative repairs.
7. **Skill & MCP Bridging** (`skill_router.rs`, `mcp_bridge.rs`): Dynamically resolves relevant skills and MCP tools based on active file extensions and task domain.
8. **Coding Engine Core** (`engine.rs`): Coordinates the plan-execute-verify-repair loop with real-time event streaming and `voxy_security::ApprovalBroker` integration.

---

## 2. Module Specifications

### 2.1 `Patcher`
- `apply_edit(workspace_root: &Path, edit: FileEditInput) -> Result<FileEditResult, PatcherError>`
- Ensures `edit.file_path` resolves inside workspace via `path_guard::validate_path`.
- Reads existing file content. Verifies that `old_string` exists. If `replace_all == false`, verifies that `old_string` appears exactly once.
- Calculates unified diff using `diff` crate.
- Backs up pre-image before applying changes.

### 2.2 `TransactionJournal`
- Root: `<workspace>/.osmoo/journal/<tx_id>/`
- Files:
  - `manifest.json`: List of affected files, transaction state (`Prepared`, `Committed`, `Aborted`).
  - `pre_images/<rel_path>`: Snapshot of each file prior to modification.
- API:
  - `begin(tx_id: Uuid, workspace: &Path) -> Result<Transaction, JournalError>`
  - `record_file(tx: &mut Transaction, rel_path: &Path, pre_image: &[u8]) -> Result<(), JournalError>`
  - `commit(tx: Transaction) -> Result<(), JournalError>`
  - `rollback(tx: Transaction) -> Result<(), JournalError>`
  - `recover_pending_transactions(workspace: &Path) -> Result<Vec<RecoveryReport>, JournalError>`

### 2.3 `ProcessRunner`
- Windows execution:
  - Creates an anonymous Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
  - Spawns process suspended or assigns process handle immediately to Job Object.
  - Limits environment variables to safe allowlist (`PATH`, `SYSTEMROOT`, `COMSPEC`, `TEMP`, `TMP`, `CARGO_HOME`, `RUSTUP_HOME`, `JAVA_HOME`, `GOPATH`).
  - Enforces execution timeout; closes Job Object on completion or timeout to guarantee termination of the entire process tree.

### 2.4 `ModelRouter` Integration
- Enforces zero fallback if `ForceProvider` or `ForceModel` is active.
- Emits explicit routing failure if forced provider is unhealthy.

### 2.5 `ApprovalBroker` Integration
- Before executing commands flagged with risk level `High` or modifying files outside current task scope, submits an `ApprovalRequest` to `ApprovalBroker`.
- Awaits human decision via oneshot channel before proceeding.
