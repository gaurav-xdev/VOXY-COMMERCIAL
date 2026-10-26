# OSMOO Coding Harness: Source Code Traceability Matrix

## 1. Traceability Overview

Every core autonomous engineering capability claimed by OSMOO is traced below to its concrete Rust implementation, module boundary, line numbers, and verification test.

| Capability | Implementing Module | Concrete Function / Method | Enforcing Tests |
| :--- | :--- | :--- | :--- |
| **DAG Planning & Decomposition** | `crates/coding_harness/src/planner.rs` | `DagPlanner::decompose_task`, `topological_sort` | `tests::test_dag_planner_generation_and_topological_sort` |
| **Context Exploration & Search** | `crates/coding_harness/src/context_explorer.rs` | `ContextExplorer::search_text`, `glob_files`, `find_symbol` | `tests::test_context_explorer_workspace_search` |
| **Atomic File Patching** | `crates/coding_harness/src/patcher.rs` | `Patcher::apply_edit_atomic`, `canonicalize_workspace_rel` | `tests::test_patcher_atomic_edit` |
| **Write-Ahead Crash Journal** | `crates/coding_harness/src/journal.rs` | `TransactionJournal::record_step`, `recover_pending` | `tests::test_crash_recovery_journal_replay` |
| **Atomic Multi-File Rollback** | `crates/coding_harness/src/patcher.rs` | `Patcher::rollback_transaction` | `tests::test_transaction_rollback` |
| **Bounded Job Object Execution** | `crates/coding_harness/src/process_runner.rs` | `ProcessRunner::run_bounded`, `WindowsJobObject` | `tests::test_bounded_process_runner` |
| **Autonomous Diagnostic Repair** | `crates/coding_harness/src/debugger.rs` | `DiagnosticDebugger::diagnose_failure`, `propose_patch` | `tests/real_coding_task_test.rs` |
| **Independent Multi-Axis Review** | `crates/coding_harness/src/reviewer.rs` | `IndependentCodeReviewer::review_diff`, `SEC-SECRET-001` | `tests/end_to_end_integration_test.rs` |
| **Strict Model Routing** | `crates/model_router/src/lib.rs` | `RoutingMode::ForceProvider`, `ForceModel` | `tests/provider_enforcement_test.rs` |
| **Dynamic Skill Discovery** | `crates/coding_harness/src/skill_router.rs` | `DynamicSkillRouter::discover_skills`, `inject_instructions` | `tests/mcp_and_skills_test.rs` |
| **MCP Tool Bridge & Gating** | `crates/coding_harness/src/mcp_bridge.rs` | `McpBridge::list_available_tools`, `call_tool_gated` | `tests/mcp_and_skills_test.rs` |
| **Daemon IPC Task Execution** | `apps/daemon/src/tools.rs` | `ToolHandler::execute_coding_task` | `voxy-daemon/tests/integration.rs` |
| **Desktop UI Telemetry** | `apps/overlay/src/state.rs` | `AppState::record_tool_step`, `ToolExecutionStep` | `voxy-overlay/src/types.rs:tests` |

---

## 2. Detailed Verification by Capability

### 2.1 Planning & DAG Task Decomposition
- **Implementation**: [`DagPlanner`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/planner.rs#L12) parses natural language coding requests into a directed acyclic graph (DAG) of discrete engineering steps (Inspect, Plan, Edit, Verify, Review).
- **Cycle Detection**: Lines 45–78 execute Kahn's algorithm for topological sorting. If circular dependencies are discovered, it returns `HarnessError::PlanDependencyCycle`.
- **Verified In**: `crates/coding_harness/src/tests.rs::test_dag_planner_generation_and_topological_sort`.

### 2.2 Context Exploration & Workspace Traversal
- **Implementation**: [`ContextExplorer`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/context_explorer.rs#L10) implements bounded directory traversals, regex-based content search (`search_text`), and pattern globbing (`glob_files`).
- **Path Confinement**: Traversal skips `.git`, `target`, and `node_modules` by default, protecting memory budget and execution speed.
- **Verified In**: `crates/coding_harness/src/tests.rs::test_context_explorer_workspace_search`.

### 2.3 Safe Multi-File Editing & Crash Journal
- **Implementation**: [`Patcher`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/patcher.rs#L15) implements write-ahead journaling and backup snapshots before modifying files on disk.
- **Path Guard**: `canonicalize_workspace_rel` validates that all file edits resolve strictly inside the target workspace root, preventing path traversal attacks (e.g. `../../Windows/System32`).
- **Crash Recovery**: [`TransactionJournal`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/journal.rs#L10) writes JSONL records with `flush()` to durable disk. Uncommitted operations are detected and can be rolled back or replayed after a process crash.
- **Verified In**: `crates/coding_harness/src/tests.rs::test_patcher_atomic_edit` and `test_crash_recovery_journal_replay`.

### 2.4 Bounded Execution & Win32 Job Objects
- **Implementation**: [`ProcessRunner`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/process_runner.rs#L18) creates Windows Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
- **Termination Guarantee**: Any spawned child processes and child process trees (e.g. cargo spawning rustc) are forcibly terminated if the timeout expires or the parent exits, preventing orphan zombie processes.
- **Security Reality**: The Job Object enforces resource limits and process group cleanup. It does NOT enforce sandbox isolation on filesystem or network access; filesystem isolation is enforced at the application layer via `path_guard`.
- **Verified In**: `crates/coding_harness/src/tests.rs::test_bounded_process_runner`.

### 2.5 Strict Provider & Model Routing (Zero Fallback)
- **Implementation**: `voxy-model-router` provides `RoutingMode::ForceProvider` and `RoutingMode::ForceModel`.
- **Invariant**: If the user or task mandates a specific provider (e.g. Ollama, Anthropic, or OpenAI), failure at that provider immediately returns a fatal error rather than silently routing prompts to an alternate or cloud provider.
- **Verified In**: `crates/coding_harness/tests/provider_enforcement_test.rs::test_failure_injection_proves_zero_fallback`.
