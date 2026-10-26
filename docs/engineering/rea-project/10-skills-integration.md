# OSMOO Dynamic Skills Integration

## 1. Skill Resolution Architecture
OSMOO leverages the `voxy-skills` crate to parse and execute Markdown-based skills (`SKILL.md`). The coding harness dynamically activates relevant skills based on:
1. **Target File Extensions**:
   - `.rs`, `Cargo.toml` -> Rust idioms, borrow checker strategies, cargo clippy/test patterns.
   - `.ts`, `.tsx`, `.js`, `package.json` -> TypeScript compiler diagnostics, React/Node testing.
   - `.py`, `pyproject.toml` -> Python typing, pytest, virtualenv patterns.
   - `.go`, `go.mod` -> Go concurrency, go test, error wrapping patterns.
   - `.java`, `pom.xml`, `build.gradle` -> Java JDK 21 modern records, JUnit patterns.
   - `.c`, `.cpp`, `CMakeLists.txt` -> Clang, memory management, ASAN patterns.
   - `.sql` -> SQL schema migrations, transaction integrity, indexing.
2. **Task Intent & Workflow Triggers**:
   - `test-driven-development` -> RED-GREEN-REFACTOR cycle enforcement.
   - `clean-code` -> Readability, minimal surface area, descriptive identifiers.
   - `systematic-debugging` -> Backward-then-forward root cause tracing before writing fixes.
   - `verification-before-completion` -> Refuses to mark tasks complete without passing executable evidence.

## 2. Skill Loading & Execution Flow
```
User Prompt / Codebase
       │
       ▼
 [Context Explorer] ──> Analyzes modified files & language stack
       │
       ▼
  [Skill Router]    ──> Matches available SKILL.md documents
       │
       ▼
 [Planner & Engine] ──> Injects skill constraints into prompt context & review checks
```
