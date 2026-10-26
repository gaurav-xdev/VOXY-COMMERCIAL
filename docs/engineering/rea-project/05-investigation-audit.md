# Reverse Engineering Investigation Audit

## 1. Methodology
This audit evaluates the evidence obtained during the reverse engineering of Claude Code v2.1.215 on Windows 11 x64 using REA (`rea-agents@6.8.0`), inspection of package artifacts (`sdk-tools.d.ts`, `cli-wrapper.cjs`, `install.cjs`, `package.json`), and executable CLI checks (`claude --help`).

## 2. Classification of Findings: Verified Facts vs Assumptions

### 2.1 Verified Facts
1. **Tool Schema Contract**: Every tool in Claude Code implements a typed interface defined in `sdk-tools.d.ts`. Specifically:
   - `FileEditInput` (`file_path`, `old_string`, `new_string`, `replace_all`) enforces exact text replacement rather than diff chunk fuzziness.
   - `FileReadInput` requires absolute paths and accepts optional `offset` and `limit`.
   - `BashInput` takes `command`, `timeout`, `description`, `run_in_background`, and `dangerouslyDisableSandbox`.
   - `TodoWriteInput` & `TaskCreateInput` maintain structured DAG and checklist task states.
2. **Platform Binary Wrapping**: `package.json` specifies `@anthropic-ai/claude-code-win32-x64` as an optional dependency copied to `bin/claude.exe` (256MB) during `postinstall`.
3. **Session & CLI Control**: CLI exposes `--session-id <uuid>`, `-r, --resume`, `-c, --continue`, `--allowed-tools`, `--disallowed-tools`, `--safe-mode`, and `--worktree`.

### 2.2 Inferred / Assumed Capabilities
1. **Subprocess Sandboxing on Windows**: Claude Code offers `dangerouslyDisableSandbox`, but on Windows native execution, OS sandboxing is limited without AppContainers or virtualized environments. Process management in production requires Win32 Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.
2. **Model Fallback**: Claude Code CLI accepts `--fallback-model`. In contrast, OSMOO requires explicit, strict zero-fallback when `ForceProvider` or `ForceModel` is requested, to prevent unintended billing or data transmission.

## 3. Implications for OSMOO Coding Harness
- OSMOO will adopt Claude Code's atomic `FileEdit` interface semantics for high predictability.
- OSMOO will implement persistent write-ahead transaction journaling (`.osmoo/journal/<tx_id>/`) to surpass Claude Code's basic error recovery with deterministic rollback.
- OSMOO will wrap all spawned processes in Windows Job Objects to ensure 100% clean process tree termination on timeout or cancellation.
