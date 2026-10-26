# Security Invariants & Crash Recovery Audit

## 1. Security Architecture & Boundary Definition

The security model of the OSMOO coding harness operates on a multi-layer defense-in-depth model, with explicit distinctions between operating system primitives and application-level controls.

### 1.1 Windows Job Objects vs. Sandboxing Reality
- **Job Object Boundary**: Job Objects are utilized via `WindowsJobObject` with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
  - **Guaranteed**: Process tree containment, CPU time limits, memory quota enforcement, and elimination of orphan child processes upon parent crash or timeout.
  - **Limitation (Crucial Realism)**: Windows Job Objects do NOT independently restrict filesystem reads/writes or outbound network sockets.
- **Application-Layer Isolation**:
  Filesystem isolation is enforced strictly at the application layer via `path_guard::validate_path` and `Patcher::canonicalize_workspace_rel`. Network operations are gated via proxy configuration and API key brokers.

---

## 2. Path Traversal & Filesystem Guards
- **Implementation**: [`Patcher::canonicalize_workspace_rel`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/patcher.rs#L85)
- **Path Confinement**: Resolves canonical path components, rejects relative path escape sequences (`..`), and checks that the target path begins with the workspace root:
  ```rust
  let full_path = self.workspace_root.join(rel_path);
  let canonical_root = self.workspace_root.canonicalize()?;
  let canonical_target = full_path.canonicalize()?;
  if !canonical_target.starts_with(&canonical_root) {
      return Err(HarnessError::PathTraversalViolation(rel_path.to_string_lossy().into()));
  }
  ```
- **Symlink Protection**: Reparse points and symlinks pointing outside the workspace boundary are explicitly rejected.

---

## 3. Secret Detection & Static Review Guardrails
- **Implementation**: Rule `SEC-SECRET-001` in [`crates/skills/src/code_review.rs`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/skills/src/code_review.rs#L160) and [`crates/coding_harness/src/reviewer.rs`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/reviewer.rs#L40).
- **Detection Patterns**: Scans diffs and patches for patterns resembling high-entropy credentials, including:
  - OpenAI / Anthropic keys (`sk-[a-zA-Z0-9_-]{20,}`)
  - GitHub tokens (`ghp_[a-zA-Z0-9]{36}`)
  - AWS access keys (`AKIA[0-9A-Z]{16}`)
  - Generic private key headers (`-----BEGIN PRIVATE KEY-----`)
- **Review Gate**: Any patch containing a hardcoded secret fails the code review gate with `FindingSeverity::Critical` and blocks task completion.

---

## 4. Transactional Atomic Rollback & Crash Recovery

### 4.1 In-Flight Transaction Rollback
- Before modifying any file on disk, `Patcher` records a snapshot of the original file content in memory and writes an `ActionType::FileEditPending` record to `TransactionJournal`.
- If an edit fails, or if compiler verification fails after editing, `Patcher::rollback_transaction` iterates through all modified files in reverse order, restoring their exact original contents.

### 4.2 Durable Crash Recovery
- **Journal Storage**: Located in `<workspace>/.osmoo/journal.jsonl`.
- **Write-Ahead Protocol**: Every state mutation is flushed to durable storage before disk modification.
- **Recovery Protocol**: On startup, `TransactionJournal::recover_pending` reads all pending uncommitted transactions. Any unfinished transaction is flagged and can be reverted automatically.
- **Verified In**: `crates/coding_harness/src/tests.rs::test_crash_recovery_journal_replay` and `test_transaction_rollback`.
