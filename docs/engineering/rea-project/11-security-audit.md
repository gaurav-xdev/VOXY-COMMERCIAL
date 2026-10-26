# OSMOO Engineering Harness Security Architecture & Windows Security Model

## 1. Security Principles
1. **Zero Silent Fallback**: Hard boundaries preventing involuntary data transfer across providers.
2. **Defensive Path Validation**: Strict containment inside workspace directory tree; symlink and junction dereference defense.
3. **Action-Bound Approvals**: Elevation gates for destructive commands and out-of-workspace writes.
4. **Child Process Tree Containment**: Deterministic lifetime bounds on child processes.

---

## 2. Windows Security Model & Sandboxing Boundaries

### 2.1 Win32 Job Objects
- **Mechanism**: Windows Job Objects (`CreateJobObjectW`, `AssignProcessToJobObject`, `SetInformationJobObject`).
- **Enforceable Guarantees**:
  - `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: Guarantees that when the job object handle closes (via completion, timeout, crash, or panic), the Windows kernel forcibly terminates all processes in the job tree.
  - Active process limits: Bounds total simultaneous child processes to prevent fork bombs.
- **Explicit Limitations**:
  - Job Objects manage lifetimes, memory quotas, and processor affinity.
  - Job Objects do **NOT** restrict network access, file system reading, or file system writing. True OS-level filesystem isolation on Windows requires Windows AppContainers, Hyper-V containers, or separate user accounts. Therefore, filesystem containment is strictly enforced at the application layer via `path_guard::validate_path`.

### 2.2 Filesystem Security & Jailbreak Defense
- **Path Guard Verification**:
  - Validates that every target path is within the canonical workspace directory.
  - Rejects traversal sequences (`..`).
  - Blocks access to protected configuration and system files (`is_protected_file`).
  - Detects and rejects directory reparse points and symlinks (`contains_symlink_or_reparse`) that escape the workspace.

### 2.3 Environment Variable Filtering
- Process environments are stripped of sensitive tokens (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `AWS_SECRET_ACCESS_KEY`, etc.).
- Only safe system variables (`PATH`, `SYSTEMROOT`, `COMSPEC`, `TEMP`, `TMP`, toolchain homes) are forwarded to subprocesses.

### 2.4 Human Approval Broker Integration
- High-risk operations (e.g. system commands, git push, deletes) submit an explicit `ApprovalRequest` to `voxy_security::ApprovalBroker`.
- Operations block on an async receiver until the user approves or rejects via desktop UI or CLI.
