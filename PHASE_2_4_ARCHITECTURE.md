# PHASE 2.4 — ARCHITECTURE & DESIGN SPECIFICATION
## Autonomous Software Engineering / Code Harness Enhancement for OSMOO

### 1. Objectives & Executive Summary
In this milestone, OSMOO's software engineering subsystem (`voxy-harness` and `voxy-tool-calling`) is transformed from basic file manipulation into a production-grade Autonomous Developer Agent.
The system provides:
1. **Multi-Language AST & Symbol Extraction**: Full syntax-aware symbol recognition for Rust, TypeScript/JavaScript, Python, Go, and PowerShell.
2. **Compiler Diagnostic & Error Parsing**: Real-time parsing of structured error diagnostics from `cargo check`, `tsc`, `pytest`/`python`, and `go test`, producing actionable compiler feedback tokens.
3. **Multi-File Atomic Patch Transactions**: Transactional multi-file patch management with pre-flight conflict detection, atomic commits, and automatic rollback on verification or build failures.
4. **Secret Scanning in Generated Patches**: Pre-commit scanning for accidental secret leakage (AWS keys, tokens, private keys, high-entropy secrets) to prevent accidental persistence or exposure.
5. **Tool & Agent Runtime Integration**: Direct integration with the `ApprovalBroker` for privileged operations and binding to `AgentRole::Coder` in `voxy-agent-runtime`.

---

### 2. Architecture & Component Diagram

```
                 +-----------------------------------------+
                 |          AgentRole::Coder (Orchestrator)|
                 +--------------------+--------------------+
                                      |
                         +------------v------------+
                         | ToolRegistry (voxy-tool)|
                         +------------+------------+
                                      |
         +----------------------------+----------------------------+
         |                            |                            |
+--------v---------+        +---------v--------+         +---------v--------+
| HarnessIndexTool |        | HarnessPatchTool |         | HarnessDiagTool  |
| (Multi-language) |        | (Atomic Tx &     |         | (Cargo/TSC/Pytest|
|                  |        |  Secret Scanner) |         |  Diagnostics)    |
+--------+---------+        +---------+--------+         +---------+--------+
         |                            |                            |
+--------v---------+        +---------v--------+         +---------v--------+
| RepositoryIndexer|        | PatchTransaction |         | DiagnosticParser |
| AST/Symbol Engine|        | Rollback Engine  |         | Cargo/TSC/Python |
+------------------+        +------------------+         +------------------+
```

---

### 3. Detailed Component Design

#### 3.1 Multi-Language Symbol Extraction (`repo_indexer.rs`)
- Expand regex/heuristic lexer to extract functions, classes, interfaces, traits, structs, enums, type aliases, and constants across:
  - **Rust**: `fn`, `struct`, `enum`, `trait`, `type`, `impl`, `const`
  - **TypeScript/JavaScript**: `function`, `class`, `interface`, `type`, `const`, `let`, `export default`
  - **Python**: `def`, `class`, `async def`
  - **Go**: `func`, `type ... struct`, `type ... interface`
  - **PowerShell**: `function`, `filter`, `class`

#### 3.2 Compiler & Test Diagnostic Parser (`diagnostic_parser.rs`)
- Parses structured compiler error output:
  - **Cargo / Rustc**: Extracts error codes (e.g. `E0308`), primary file path, line/column numbers, and primary error message.
  - **TypeScript (`tsc`)**: Matches `filepath(line,col): error TSxxxx: message`.
  - **Python / Pytest**: Matches `Traceback (most recent call last)`, file/line references, and `ExceptionType: message`.
- Converts stdout/stderr into structured `DiagnosticReport` with `DiagnosticItem` (severity: Error, Warning, Note).

#### 3.3 Transactional Multi-File Patch Engine (`patch_engine.rs`)
- `PatchTransaction`:
  - Collects changes across multiple files.
  - Runs pre-flight checks:
    - Target files within repo boundary.
    - Protected files check (`.git`, `.env`, private keys).
    - Secret scanner check: reject patches introducing raw API keys (`AKIA...`, `sk-...`, `ghp_...`, RSA/EC private keys).
  - Backs up original file contents before applying.
  - Applies all file modifications in sequence.
  - Supports `commit()` (releasing backup) or `rollback()` (restoring all files on compilation/test failure).

#### 3.4 Integration in `voxy-tool-calling`
- New tools:
  - `harness_parse_diagnostics`: Inspects command output for compiler errors.
  - `harness_apply_patch_transaction`: Atomically applies multi-file patches with automatic secret scanning and rollback token tracking.
- Existing tools updated to support the enhanced harness engines.

---

### 4. Security & Safety Boundaries
1. **Repository Boundary Enforcement**: Zero path-traversal allowed. Canonical paths must reside within the designated workspace root.
2. **Secret Leakage Prevention**: Patches containing API keys or private keys are rejected prior to write.
3. **Protected Paths**: `.env`, `.git/`, `.ssh/`, credential stores are read-only / immutable to the patch engine.
4. **Emergency Stop Token**: All command runs poll the global `AtomicBool` token and kill child processes immediately upon trigger.
5. **Human Authorization**: Any privileged execution (`run_command`, writing to code files) passes through the `ApprovalBroker`.
