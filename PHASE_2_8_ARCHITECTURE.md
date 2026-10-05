# PHASE 2.8 ARCHITECTURE & SYSTEM DESIGN SPECIFICATION
## Subsystem: Skills + Workflows Ecosystem
**Target System:** OSMOO AI Operating Companion  
**Status:** IMPLEMENTATION PHASE  
**Author:** OSMOO Autonomous Engineering Lead  

---

## 1. Executive Summary & Mission
Phase 2.8 elevates OSMOO from a collection of discrete agents and tools into a composable, secure, versioned, sandboxed, and auditable **Skill + Workflow Ecosystem**.

A **Skill** in OSMOO is not a prompt template; it is a verified capability package with:
1. **Immutable Manifest & Versioning:** `skill_id`, `version`, `publisher_id`, `trust_level` (`System`, `Verified`, `Published`, `Untrusted`, `Blocked`), integrity checksum.
2. **Explicit Permissions & Least Privilege:** Mandatory declaration of required permissions (`ReadFiles`, `WriteFiles`, `DeleteFiles`, `BrowserAccess`, `NetworkAccess`, `ProcessControl`, `ComputerControl`, `MemoryRead`, `MemoryWrite`, `ResearchAccess`, `CodeHarnessAccess`, `TaskHistoryAccess`). Unmatched or undeclared requests fail closed.
3. **Sandboxed Execution Runtime:** Enforces timeouts, resource bounds, dependency resolution, and isolation.
4. **ApprovalBroker Integration:** Any high-risk or destructive tool invocations triggered by a skill are gated through the centralized approval broker with human confirmation.

A **Workflow** is an orchestration of skills and tools:
1. **Observable Lifecycle:** `Created`, `Queued`, `Running`, `WaitingApproval`, `Paused`, `Verifying`, `Completed`, `Failed`, `Cancelled`, `Expired`.
2. **Deterministic Step Transitions:** Bounded Observe → Plan → Act → Verify loop. No runaway autonomous loops.
3. **Tenant & Workspace Boundary Isolation:** Strict scoping ensuring skills and workflows cannot access cross-tenant data.
4. **Crash Recovery & Idempotency:** State and artifacts persisted to SQLite via `CommercialStore` (Migration 105: `skills`, `workflows`, `workflow_executions`).

---

## 2. Relational Schema: Migration 105 (`skills_and_workflows`)

Registered in `crates/database/src/commercial.rs`:

```sql
CREATE TABLE IF NOT EXISTS skills (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    display_name TEXT NOT NULL,
    description TEXT NOT NULL,
    version TEXT NOT NULL,
    publisher_id TEXT NOT NULL,
    trust_level TEXT NOT NULL DEFAULT 'untrusted', -- 'system', 'verified', 'published', 'untrusted', 'blocked'
    manifest_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',        -- 'active', 'disabled', 'revoked'
    checksum TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(name, version)
);
CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
CREATE INDEX IF NOT EXISTS idx_skills_trust ON skills(trust_level);

CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    workspace_id TEXT,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    version TEXT NOT NULL,
    workflow_definition TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_workflows_account ON workflows(account_id);
CREATE INDEX IF NOT EXISTS idx_workflows_workspace ON workflows(workspace_id);

CREATE TABLE IF NOT EXISTS workflow_executions (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL,
    workspace_id TEXT,
    status TEXT NOT NULL, -- 'created', 'queued', 'running', 'waiting_approval', 'paused', 'verifying', 'completed', 'failed', 'cancelled'
    current_step INTEGER NOT NULL DEFAULT 0,
    total_steps INTEGER NOT NULL DEFAULT 0,
    input_data TEXT NOT NULL DEFAULT '{}',
    output_data TEXT,
    error_message TEXT,
    started_at TEXT NOT NULL,
    ended_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_wf ON workflow_executions(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_account ON workflow_executions(account_id);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_status ON workflow_executions(status);
```

---

## 3. Skill Manifest Specification

```json
{
  "skill_id": "osmoo.coding.patch_repair",
  "name": "patch_repair",
  "display_name": "Autonomous Patch Repair",
  "version": "1.0.0",
  "publisher_id": "osmoo_core",
  "trust_level": "Verified",
  "permissions": [
    "ReadFiles",
    "WriteFiles",
    "CodeHarnessAccess"
  ],
  "required_tools": [
    "file_read",
    "harness_apply_patch",
    "harness_parse_diagnostics"
  ],
  "inputs_schema": { ... },
  "outputs_schema": { ... },
  "timeout_seconds": 60,
  "max_steps": 10
}
```

---

## 4. Runtime & Security Boundary Enforcement

1. **Permission Check Hook:**
   Before any tool in `ToolRegistry` is dispatched on behalf of a skill, the skill's declared permissions are cross-referenced. If undeclared, execution terminates immediately with `PermissionDenied`.
2. **Approval Broker Integration:**
   Destructive operations (`FileDeleteTool`, `ProcessKillTool`, etc.) strictly enforce human-in-the-loop approval.
3. **Prompt Injection Quarantine:**
   Any input coming from web sources or research remains tagged as `UNTRUSTED_DATA` and cannot override workflow step instructions.
4. **Cancellation & Watchdogs:**
   Atomic cancellation flags and execution timeouts prevent hangs or runaway tasks.
