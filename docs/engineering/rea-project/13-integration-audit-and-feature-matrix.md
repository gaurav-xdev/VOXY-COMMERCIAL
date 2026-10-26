# OSMOO: Comprehensive Integration Audit & Feature Matrix

**Document ID:** `OSMOO-REA-13`  
**Date:** 2026-10-25  
**Auditor:** OSMOO Autonomous Systems Engineering Group  
**Status:** Canonical Reference

---

## 1. Executive Summary

This document performs an exhaustive audit across the OSMOO repository to verify the operational state of the Claude Code-inspired Autonomous Senior Engineering Harness (`voxy-coding-harness`). Each capability is classified under one of four explicit statuses:
- **`IMPLEMENTED AND VERIFIED`**: Code is fully implemented, wired into runtime/daemon/IPC, and verified by passing unit/integration tests with real execution artifacts.
- **`PARTIALLY IMPLEMENTED`**: Implementation exists and functions in isolation, but requires deeper runtime wiring or prompt context injection.
- **`MISSING`**: Component or feature is required by specification but not yet authored.
- **`NOT APPLICABLE`**: Architectural pattern or mechanism not applicable to Windows or safe desktop agent operation (e.g., raw Linux cgroups, proprietary internal code cloning).

---

## 2. Comprehensive Feature Matrix

| Capability / Subsystem | Status | Implementation Source Path | Verification Test Evidence | Approvals & Security Limitations |
| :--- | :--- | :--- | :--- | :--- |
| **Atomic Multi-File Patching** | `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/patcher.rs` | `tests::test_atomic_file_edit_success_and_conflict` (PASS) | Enforces unified diff, old string matching, workspace confinement via `Patcher::canonicalize_workspace_rel` |
| **Write-Ahead Crash Recovery Journal** | `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/journal.rs` | `tests::test_startup_crash_recovery`, `tests::test_transaction_journal_commit_and_rollback` (PASS) | Rolling write-ahead pre-images in `.osmoo/transactions/`. Uncommitted transactions rolled back on restart. |
| **Process Tree Management (Job Objects)** | `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/process_runner.rs` | `tests::test_process_runner_bounded_execution` (PASS) | Uses Win32 `CreateJobObjectW` + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Process tree containment, not network/filesystem sandbox. |
| **Task Planner DAG with Checkpoints** | `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/planner.rs` | `tests::test_task_planner_dag` (PASS) | Topological sorting, dependency validation, checkpoint recovery states. |
| **Multi-Language Toolchain Detection** | `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/toolchain.rs` | `tests::test_toolchain_detection_matrix` (PASS) | Detects Rust (`cargo`), Node/TS (`node`/`npm`/`bun`), Python (`python`/`pytest`), Go (`go`), C/C++ (`clang`/`cmake`), Java (`javac`), C# (`dotnet`). |
| **Compiler Failure Diagnostics & Repair**| `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/debugger.rs` | `real_coding_task_test::test_real_autonomous_coding_task_end_to_end` (PASS) | Regex-based diagnostic parser for rustc (`E0308`, `E0425`, etc.), clippy, and compiler outputs. |
| **Independent Code Review Gate** | `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/reviewer.rs` | `crates/skills/src/code_review.rs`, `real_coding_task_test` (PASS) | Blocks on high-severity findings (hardcoded secrets, command injections, unsafe derefs). |
| **Zero-Fallback Provider Enforcement** | `IMPLEMENTED AND VERIFIED` | `crates/model_router/src/lib.rs` | `provider_enforcement_test::test_failure_injection_proves_zero_fallback` (PASS) | `ForceProvider` and `ForceModel` return `AllProvidersExhausted` immediately when primary fails. No silent downgrade. |
| **Action-Bound Human Approval Gate** | `IMPLEMENTED AND VERIFIED` | `voxy_security::approval::ApprovalBroker`, `crates/coding_harness/src/engine.rs` | `tests::test_human_approval_flow` (PASS) | Destructive shell commands (`rm`, `del`, `format`, `git push`) block until user decision is received. |
| **Daemon & IPC Protocol Integration** | `IMPLEMENTED AND VERIFIED` | `crates/ipc/src/voxy_protocol.rs`, `apps/daemon/src/main.rs`, `apps/daemon/src/tools.rs` | `tests/e2e` (PASS), `execute_coding_task` tool endpoint | Supports `ClientCommand::ExecuteCodingTask` and broadcasts `DaemonMessage::CodingHarnessEvent`. |
| **Desktop UI Progress & Diff Rendering**| `PARTIALLY IMPLEMENTED` | `crates/desktop_ui/src/overlay/mod.rs`, `crates/desktop_ui/src/app.rs` | `ToolStep` wired; needs explicit `CodingHarnessEvent` matching in main app and overlay | Displays step status (`Running`, `Completed`, `Failed`) in drawer; needs real-time event routing for harness diffs. |
| **Dynamic Skill Instruction Ingestion**| `PARTIALLY IMPLEMENTED` | `crates/coding_harness/src/skill_router.rs` | `tests/mcp_and_skills_test.rs` (PASS) | Suggests skills by file extension; needs direct `SKILL.md` body reading and prompt context injection. |
| **MCP Tool Bridge Execution** | `IMPLEMENTED AND VERIFIED` | `crates/coding_harness/src/mcp_bridge.rs` | `tests/mcp_and_skills_test.rs` (PASS) | Discovers tools, validates schemas, and executes via `ToolRegistry`. |
| **Claude Code CLI Optional Backend** | `PARTIALLY IMPLEMENTED` | `where.exe claude` verified v2.1.215 | Verified via `claude.cmd --version` (2.1.215) | Needs explicit adapter module `claude_cli_backend.rs` to allow optional CLI invocation. |
| **Linux Cgroups / Bubblewrap Sandbox** | `NOT APPLICABLE` | Windows OS Native Runtime | OS Architecture Requirement | Windows uses Job Objects for process lifecycles; filesystem isolation enforced via path guards. |

---

## 3. Subsystem Breakdown and Audit Details

### 3.1 Traceability to REA Findings
- **Investigation Artifact:** `docs/engineering/rea-project/04-claude-code-investigation.md`
- **Finding 1 (Process Management):** Claude Code uses subshells with timeouts and orphan killing. Implemented in OSMOO via Win32 Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
- **Finding 2 (File Editing Safety):** Claude Code verifies file contents before replacement. Implemented in OSMOO via `Patcher::apply_edit` with string replacement, pre-image hashing, and write-ahead transaction journaling.
- **Finding 3 (Prompt Invariant Enforcement):** Claude Code uses system instructions for ground-before-mutating and review loops. Implemented via OSMOO's `Reviewer` and `SkillRouter`.

### 3.2 Preinstalled Skills Inventory
- **Local Repository Skills:** `.agents/skills/gravitas/SKILL.md` (6 invariants, verification contracts, failure recovery).
- **Global Gemini/Claude Skills:** Over 400 specialized skills indexed in user configuration (including `clean-code`, `tdd-workflow`, `typescript-expert`, `rust-pro`, `python-pro`).
- **Runtime Invocation Status:** To ensure active execution during tasks, the engine must load the actual `SKILL.md` instructions into the context when a skill is active.

### 3.3 Configured MCP Servers
- **Biomolecular Structure Viewer MCP:** Lazily loaded via MCP configuration.
- **Graphify Knowledge Graph:** Live in `graphify-out/` (29,678 nodes, 54,981 edges). Accessible via CLI and native queries.
- **Native Tool Registry:** Built-in tools for window management, process management, file inspection, and coding harness tasks.

### 3.4 Claude Code CLI Backend Strategy
- **Version Detected:** `2.1.215 (Claude Code)` at `C:\Users\gaurav\AppData\Roaming\npm\claude.cmd`.
- **Integration Policy:**
  1. Default to OSMOO's native Rust coding harness (`voxy-coding-harness`).
  2. Provide an optional, explicitly enabled `ClaudeCliBackend` adapter that wraps `claude.cmd -p "<prompt>"` for users wanting to delegate directly to the installed CLI.
  3. No copying of proprietary source files or private tokens.

---

## 4. Remediation Plan to Close All Gaps
1. **Enhance `crates/coding_harness/src/skill_router.rs`**: Add file-based `SKILL.md` discovery, frontmatter parsing, and prompt instruction loading.
2. **Implement `crates/coding_harness/src/claude_cli_backend.rs`**: Safe adapter for calling `claude.cmd` when configured, checking version, non-interactive execution, and error propagation.
3. **Enhance `crates/desktop_ui/src/app.rs` & `overlay/mod.rs`**: Explicitly handle `DaemonMessage::CodingHarnessEvent` to display real-time planning, diff review, and execution events in the desktop UI.
4. **Author End-to-End Test Suite**: Complete integration test covering all 10 criteria.
