# PHASE 2.6 — ARCHITECTURE & DESIGN SPECIFICATION
## Task History, Artifacts & Workspace Intelligence for OSMOO

### 1. Objectives & Executive Summary
Phase 2.6 establishes OSMOO's unified execution memory and workspace intelligence layer.
Instead of treating user queries as ephemeral chat logs, OSMOO maintains a durable, auditable record of what it planned, approved, executed, verified, and produced across projects and sessions.

Key Subsystems:
1. **Relational Task Execution Schema (Database Migration 103)**:
   - `tasks`: Full execution lifecycle tracking (`id`, `session_id`, `user_goal`, `task_type`, `status`, `phase`, `progress`, `parent_task_id`, `created_at`, `started_at`, `ended_at`, `error_message`, `result_summary`).
   - `task_artifacts`: Persistent artifact provenance (`id`, `task_id`, `workspace_id`, `artifact_type`, `name`, `relative_path`, `storage_uri`, `content_hash`, `size_bytes`, `metadata`, `created_at`).
   - `task_tool_invocations`: Exact tool call audit logs with execution durations, risk tiers, and approval IDs.
2. **Task Lifecycle State Machine (`TaskStore`)**:
   - Validated state transitions:
     `QUEUED` -> `RUNNING` -> `WAITING_APPROVAL` -> `RUNNING` -> `COMPLETED` / `FAILED` / `CANCELLED` / `EXPIRED`.
   - Rejection of invalid transitions (e.g. `COMPLETED` cannot move back to `RUNNING`).
   - Interrupted task detection upon system boot: marks stale `RUNNING` tasks as `FAILED("Interrupted by daemon restart")`.
3. **Artifact Abstraction & Security**:
   - Supports: `GeneratedCode`, `Patch`, `ResearchReport`, `DiagnosticLog`, `TerminalOutput`, `Screenshot`.
   - Content hashing (SHA-256) for tamper detection.
   - Pre-commit secret scanning on textual artifacts.
   - Path-traversal guards ensuring artifacts remain strictly within project/workspace storage roots.
4. **Workspace Intelligence Engine**:
   - Cross-references tasks and artifacts with `voxy-harness` repository indexer.
   - Queries recent tasks, failed tasks, workspace-specific tasks, and artifact associations.

---

### 2. Architecture & Data Flow

```
                      +-----------------------------+
                      |    OSMOO Agent / UI Loop    |
                      +--------------+--------------+
                                     |
                                     v
                      +-----------------------------+
                      |          TaskStore          |
                      |   - State Machine Validation|
                      |   - Crash / Restart Recovery|
                      +--------------+--------------+
                                     |
              +----------------------+----------------------+
              |                                             |
              v                                             v
  +-----------------------+                     +-----------------------+
  |    Database Layer     |                     |    ArtifactManager    |
  | (Migration 103: tasks,|                     | - File & Report Store |
  |  artifacts, tool_logs)|                     | - Path Traversal Guard|
  +-----------------------+                     | - SHA-256 & Secrets   |
                                                +-----------------------+
```

---

### 3. Security, Privacy & Integrity Gates
1. **Secret Redaction**: No API keys, credentials, or session tokens in `result_summary` or `metadata`.
2. **Path Sanitization**: Relative paths canonicalized against designated project roots.
3. **Crash Resilience**: Interrupted tasks fail-closed; never marked falsely complete.
