# PHASE 2.7 ARCHITECTURE & SYSTEM DESIGN SPECIFICATION
## Subsystem: Memory + RAG + Long-Term Context Intelligence Engine
**Target System:** OSMOO AI Operating Companion  
**Status:** IMPLEMENTATION PHASE  
**Author:** OSMOO Autonomous Engineering Lead  

---

## 1. Executive Summary & Core Mission
Phase 2.7 transforms OSMOO's memory capabilities into an enterprise-grade, privacy-governed, provenance-tracked, context-budgeted retrieval and long-term intelligence engine.

Memory in an AI operating companion is distinct from a generic vector database:
1. **Provenance & Attribution:** Every remembered item must record its exact origin (user statement, research artifact, task observation, tool execution).
2. **Account & Workspace Isolation:** Stored knowledge must never leak across account or workspace boundaries (fail-closed tenant isolation).
3. **Anti-Poisoning Quarantine:** Retrieved memories are untrusted persistent data. When injected into prompt context, they must be strictly encapsulated in quarantine envelopes with adversarial patterns neutralized.
4. **Secret Scanning & Privacy Exclusion:** OSMOO must never persistently store API keys, tokens, session hashes, private keys, or credentials.
5. **Durable Lifecycle & Consolidation:** Support for memory states (`Active`, `Superseded`, `Contradicted`, `Stale`, `Revoked`), recency decay, and user-initiated deletion ("forget this workspace context").
6. **Token-Budgeted Hybrid Retrieval:** Combines exact/lexical matching and semantic similarity with a strict token budget limiter to prevent context window explosion.

---

## 2. Layered Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                        User / Agent Execution                          │
│          (Voice Command / Chat Prompt / Autonomous Subagent)           │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
               ┌────────────────────┴────────────────────┐
               │                                         │
               ▼                                         ▼
   ┌───────────────────────┐                 ┌───────────────────────┐
   │     Memory Store      │                 │     Memory Recall     │
   │  (Extraction Filter)  │                 │    (RAG Retrieval)    │
   └───────────┬───────────┘                 └───────────┬───────────┘
               │                                         │
               ▼                                         ▼
   ┌───────────────────────┐                 ┌───────────────────────┐
   │ Pre-Persist Security  │                 │ Multi-Score Ranking   │
   │ - SecretScanner check │                 │ - Lexical (BM25/match)│
   │ - Sensitivity tiering │                 │ - Semantic Cosine Sim │
   │ - Deduplication check │                 │ - Recency Decay       │
   └───────────┬───────────┘                 └───────────┬───────────┘
               │                                         │
               ▼                                         ▼
   ┌───────────────────────┐                 ┌───────────────────────┐
   │ Database Migration    │                 │ Context Budget &      │
   │  Version 104          │                 │ Poisoning Defense     │
   │ `scoped_memories` tbl │                 │ - Quarantine container│
   │ (SQLite WAL mode)     │                 │ - Token budget cap    │
   └───────────────────────┘                 └───────────────────────┘
```

---

## 3. Relational Schema: Migration 104 (`scoped_memories`)

Registered in `crates/database/src/commercial.rs`:

```sql
CREATE TABLE IF NOT EXISTS scoped_memories (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    workspace_id TEXT,
    memory_type TEXT NOT NULL,         -- 'Fact', 'Preference', 'Decision', 'TaskContext', 'ProjectContext', 'WorkspaceKnowledge'
    content TEXT NOT NULL,
    source TEXT NOT NULL,              -- 'user_explicit', 'task_observation', 'research', 'harness', 'system'
    source_reference TEXT,             -- Task ID, URL, or File path
    confidence REAL NOT NULL DEFAULT 1.0,
    importance REAL NOT NULL DEFAULT 0.5,
    provenance TEXT NOT NULL DEFAULT '{}',
    sensitivity TEXT NOT NULL DEFAULT 'standard', -- 'standard', 'sensitive', 'restricted'
    status TEXT NOT NULL DEFAULT 'active',        -- 'active', 'superseded', 'contradicted', 'stale', 'revoked'
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scoped_memories_account ON scoped_memories(account_id);
CREATE INDEX IF NOT EXISTS idx_scoped_memories_workspace ON scoped_memories(workspace_id);
CREATE INDEX IF NOT EXISTS idx_scoped_memories_type ON scoped_memories(memory_type);
CREATE INDEX IF NOT EXISTS idx_scoped_memories_status ON scoped_memories(status);
CREATE INDEX IF NOT EXISTS idx_scoped_memories_created ON scoped_memories(created_at);
```

---

## 4. Security & Privacy Model

1. **Secret Scanning Pre-Filter:**
   - Any content submitted to `memory_store` is evaluated by `voxy_harness::secret_scanner::SecretScanner`.
   - If secrets (API keys, JWTs, private keys, passwords) are detected, storage is rejected with `ValidationError` to prevent secret persistence.
2. **Fail-Closed Boundary Separation:**
   - Queries must supply `account_id` and optional `workspace_id`. Cross-account retrieval is strictly forbidden at SQL parameter binding level.
3. **Memory Poisoning Defense:**
   - Retrieved memories injected into planner/agent context are sanitized using `voxy_grounding::defense::PromptInjectionDefense` and enclosed in:
     `<<<UNTRUSTED_MEMORY_DATA_START [id: ... provenance: ...]>>> ... <<<UNTRUSTED_MEMORY_DATA_END>>>`
4. **User Control & Right to Erasure:**
   - `memory_forget` provides explicit revoking of memories by ID, by workspace, or by pattern.

---

## 5. Tool Calling Integration

Exposed through `crates/tool_calling`:
1. `MemoryStoreTool` (`memory_store`): Stores knowledge with sensitivity checks, deduplication, and provenance.
2. `MemoryRecallTool` (`memory_recall`): Hybrid search with context budget and quarantine wrapping.
3. `MemoryForgetTool` (`memory_forget`): Revokes or deletes records safely upon user command.
