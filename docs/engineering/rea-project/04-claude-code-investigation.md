# Claude Code v2.1.215 Reverse Engineering Investigation

## 1. System Overview
- **Package**: `@anthropic-ai/claude-code@2.1.215`
- **Location**: `C:\Users\gaurav\AppData\Roaming\npm\node_modules\@anthropic-ai\claude-code`
- **Architecture**: Packaged native binary (`bin/claude.exe`, 256MB) wrapped by Node launcher (`cli-wrapper.cjs`, `install.cjs`) with complete TypeScript tool contract (`sdk-tools.d.ts`, 148,929 bytes, 3,806 lines).

---

## 2. Deep Dive Across the 13 Architectural Categories

### 2.1 CLI Entry Points, Arguments, Commands & Flags
*Evidence*: `claude --help` & `package.json`
- **Binary Wrapper**: `cli-wrapper.cjs` detects host platform (`win32-x64`) and executes `bin/claude.exe`.
- **Top-Level Commands**:
  - `agents`: Manage background agents.
  - `auth`: Manage credentials and tokens.
  - `auto-mode`: Auto-mode classifier configuration.
  - `doctor`: Health check and diagnostic verification.
  - `mcp`: Add, remove, list MCP servers.
  - `plugin` / `plugins`: Manage plugins.
  - `project`: Manage project state.
- **Key Flags**:
  - `--session-id <uuid>`: Deterministic session UUID.
  - `-r, --resume [value]`: Resume conversation by session ID.
  - `-c, --continue`: Continue most recent conversation in cwd.
  - `--tools <tools...>` / `--allowed-tools` / `--disallowed-tools`: Granular tool access control.
  - `--worktree [name]`: Isolated git worktree per session.
  - `--safe-mode`: Clean run disabling custom hooks/plugins.
  - `--fallback-model <model>`: Model fallback configuration.

### 2.2 Tool Suite & Schema Definitions
*Evidence*: `sdk-tools.d.ts` lines 11–55 (`ToolInputSchemas`), 583–625 (`FileEditInput`, `FileReadInput`, `FileWriteInput`)
- **Core Engineering Tools**:
  - `FileRead`: Absolute `file_path`, optional `offset`, `limit`, and PDF `pages` (lines 601–618).
  - `FileEdit`: Strict atomic replacement (`file_path`, `old_string`, `new_string`, `replace_all`) (lines 583–600).
  - `FileWrite`: Overwrite/create with absolute `file_path` and `content` (lines 619–628).
  - `Glob`: Path matching pattern with directory root (lines 629–638).
  - `Grep`: Ripgrep-backed regex search (`pattern`, `path`, `glob`, `output_mode`: `content` | `files_with_matches` | `count`, `-B`, `-A`, `-C`, `-n`, `-i`, `-o`, `type`, `head_limit`, `offset`, `multiline`) (lines 639–700).

### 2.3 Planning & Task Decomposition
*Evidence*: `sdk-tools.d.ts` lines 813–822 (`TodoWriteInput`), lines 2481–2548 (`EnterPlanModeInput`, `TaskCreateInput`, `TaskUpdateInput`, `TaskListInput`, `TaskGetInput`)
- **Plan Modes**:
  - `EnterPlanMode` / `ExitPlanMode`: Guarded state where edits are locked until planning is approved.
  - `TodoWrite`: Checklist tracking (`content`, `status`: `"pending"` | `"in_progress"` | `"completed"`, `activeForm`).
  - `TaskCreate` / `TaskUpdate`: DAG task tracking with dependency links (`addBlocks`, `addBlockedBy`, `activeForm`, `metadata`).

### 2.4 File Editing & Verification
*Evidence*: `sdk-tools.d.ts` lines 583–600
- **Diff / Patch Mechanism**: Substring exact replacement (`old_string` -> `new_string`). Must fail if `old_string` is not unique unless `replace_all: true`. Requires strict pre-read before edit.

### 2.5 Shell & Command Execution
*Evidence*: `sdk-tools.d.ts` lines 521–552 (`BashInput`)
- Parameters: `command`, `timeout` (max 600,000 ms), `description` (human-readable action summary), `run_in_background` (boolean).
- Security boundary: `dangerouslyDisableSandbox` (boolean flag to bypass container/sandbox).

### 2.6 Subagents & Delegation
*Evidence*: `sdk-tools.d.ts` lines 483–520 (`AgentInput`)
- Spawns specialized agents with `description`, `prompt`, `subagent_type`, `model` (`"sonnet" | "opus" | "haiku" | "fable"`), `run_in_background`, `name`, `isolation`: `"worktree" | "remote"`.
- Communicates via `TaskOutputInput` (`task_id`, `block`, `timeout`) and `TaskStopInput`.

### 2.7 MCP Client & Server
*Evidence*: `sdk-tools.d.ts` lines 711–765 (`ListMcpResourcesInput`, `RefreshMcpToolsInput`, `McpInput`, `ReadMcpResourceInput`, `ReadMcpResourceDirInput`)
- Standard JSON-RPC stdio protocol integration with tools, resources, and dynamic refresh.

### 2.8 Skills System
*Evidence*: `sdk-tools.d.ts` lines 2677–2715 (`ProposeSkillsInput`)
- Markdown-based skills (`SKILL.md`) structured with frontmatter + `Trigger` / `Steps` / `Verification` sections.
- Supports proposing new skills and improvements from observed user workflows.

### 2.9 Model Routing & Fallback
*Evidence*: CLI flag `--fallback-model <model>` and CLI help output
- Supports primary model selection with explicit fallback models when overloaded.
- In enterprise/audited configurations, implicit fallback leads to cost/provider drift; OSMOO requires strict, configurable zero-fallback on `ForceProvider` and `ForceModel`.

### 2.10 Session State & Context Compaction
*Evidence*: `--session-id <uuid>`, `-r, --resume`, `--continue`, `exclude-dynamic-system-prompt-sections`
- Persistent session directories storing conversation history, task DAGs, and tool outcomes. Dynamic prompt sections separated for cache optimization.

### 2.11 Permission & Approval Framework
*Evidence*: CLI options `--allowed-tools`, `--disallowed-tools`, `AgentInput.mode` (`"acceptEdits" | "auto" | "bypassPermissions" | "default" | "dontAsk" | "plan"`).
- Action-bound confirmation prompts for destructive tools (shell commands, file modifications outside workspace).

### 2.12 Crash Recovery & Persistence
*Evidence*: File write/edit contracts and session restoration flags.
- Real-time disk journaling required to ensure that interrupted multi-file edits can be rolled back to their pre-transaction clean state.

### 2.13 Windows Platform Specifics
*Evidence*: `install.cjs`, `cli-wrapper.cjs`, Windows process limits.
- On Windows, process lifetime and tree termination require Win32 Job Objects (`CreateJobObjectW`, `AssignProcessToJobObject`, `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). Simple PID termination leaks child processes.
- Path normalization must account for Windows verbatim paths (`\\?\`), case insensitivity, and reparse points/junctions.

---

## 3. Findings Summary for OSMOO Engineering Harness
The Claude Code investigation establishes the concrete architecture for `voxy-coding-harness`:
1. Use exact `old_string`/`new_string` atomic patch semantics.
2. Implement disk-backed write-ahead transaction journaling (`.osmoo/journal/<tx_id>/`) with rollbacks.
3. Manage all spawned commands via Windows Job Objects with automatic child cleanup.
4. Integrate with `voxy_security::ApprovalBroker` for permission elevation.
5. Provide strict zero-fallback model routing in `voxy-model-router`.
