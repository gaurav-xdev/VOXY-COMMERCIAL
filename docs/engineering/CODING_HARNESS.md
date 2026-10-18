# OSMOO Autonomous Coding Harness & Agent Adapter Specification

Last Updated: 2026-10-18
Status: ACTIVE SPECIFICATION & ARCHITECTURE

## 1. Overview & Capabilities

The OSMOO Coding Harness (`crates/harness` & `crates/agent_runtime`) provides a secure, sandboxed execution environment for autonomous coding agents (Claude Code, OpenAI Codex CLI, Google Antigravity, and native OSMOO subagents).

### Core Components
1. **Repository Indexer (`repo_indexer.rs`)**: Multi-language symbol extraction (Rust, TypeScript, Python) and directory structure mapping.
2. **Diagnostic Parser (`diagnostic_parser.rs`)**: Structured error extraction for `rustc`, `tsc`, and Python tracebacks.
3. **Secret Scanner (`secret_scanner.rs`)**: Proactive blocking of AWS keys, GitHub tokens, private keys, and `.env`/`.ssh` files before write transactions.
4. **Patch Engine (`patch_engine.rs`)**: Atomic transactions with two-phase rollback, symlink/reparse-point escape detection, and path validation.
5. **Sandboxed Runner (`sandbox_runner.rs`)**: Windows process execution with job objects, timeouts, and emergency-stop cancellation tokens.
6. **Multi-Agent Orchestrator (`crates/agent_runtime/src/orchestrator.rs`)**: Role-based agent coordinator (Planner, Researcher, Coder, QA, Security, Reviewer).

---

## 2. Universal Agent Adapter Interface

To avoid tight coupling to any single vendor CLI or proprietary agent format, the coding harness provides a capability-negotiated agent adapter interface:

```rust
#[async_trait::async_trait]
pub trait CodingAgentAdapter: Send + Sync {
    /// Unique identifier for this adapter (e.g., "claude-code", "antigravity", "codex-cli").
    fn agent_id(&self) -> &str;

    /// Query adapter capabilities and execution mode.
    fn capabilities(&self) -> AgentCapabilities;

    /// Execute a bounded coding task within the specified workspace boundary.
    async fn execute_task(
        &self,
        task: CodingTask,
        harness: &HarnessContext,
    ) -> Result<TaskExecutionResult, HarnessError>;

    /// Issue an immediate cancellation request to the active agent process tree.
    async fn cancel(&self) -> Result<(), HarnessError>;
}
```

### Safety & Permission Gates
- **Least-Privilege Workspace Confinement**: All file modifications must resolve inside the project boundary. Any attempt to write outside results in `HarnessError::PathTraversalViolation`.
- **Protected File Invariant**: Modifications to `.git`, `.env`, `Cargo.lock`, or system binaries are rejected with `PatchError::ProtectedFile`.
- **Emergency Stop Tokens**: `CancellationToken` wired to `taskkill /F /T /PID` guarantees no zombie processes remain after interruption or timeout.
