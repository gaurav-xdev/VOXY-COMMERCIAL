# OSMOO Engineering Log

## Session 1 — 2026-10-17
Scope: initial 10-minute reconnaissance (no code changes).

### Commands run
- `git rev-parse`/`status`/`log` → branch `master`, HEAD `22155e3`, clean tree (+untracked `graphify/`, `.freebuff/`).
- Graphify: `uvx --from graphifyy graphify extract . --code-only --no-viz --exclude "graphify/**" --exclude ".freebuff/**"` → `graphify-out/graph.json`: 14,457 nodes, 29,865 edges, 545 communities; graph.html/report skipped.
- Graphify queries: voice pipeline; harness patch/rollback/approval; IPC transport/auth; whisper placeholder.
- Targeted source reads: voice_stt lib.rs:255-299, voice_tts lib.rs:216-255, voice_orchestrator lib.rs:1-120, daemon main.rs:1-120, plus greps (wake_word, named_pipe auth, pipecat port, desktop_ui runtimes).

### Graphify notes
- Graphify CLI not installed globally; ran via `uvx --from graphifyy graphify …` (package name `graphifyy`, double-y). Usage rule for future sessions: query graphify-out/ before grepping; run `graphify update .` (cost-free AST rebuild) after code changes.

### Outcomes
- Created `docs/engineering/OSMOO_CODEBASE_MAP.md`, `OSMOO_FEATURE_MATRIX.md`, `OSMOO_KNOWN_ISSUES.md` (this file).
- 4 issues CONFIRMED at source level (KI-1..KI-4 in known-issues register), 15 migrated as RE-VERIFY.

### Next steps (dependency order)
1. Read IPC auth gate (`crates/ipc/src/auth.rs` + named_pipe auth path) → settle KI-4 (security).
2. Read `crates/harness/src/patch_engine.rs`, `sandbox_runner.rs` → settle KI-12 (integrity).
3. Fix KI-1 with a real local STT binding or honest `Unavailable` errors + tests.
4. Trace `WakeWordDetector` callers; either wire it into a real acoustic model path or mark feature unavailable.
5. Voice-speed/barge-in/runtime lifecycle verification batch (KI-5..KI-11) in one pass through voice_orchestrator + daemon.

### Limitations
- No runtime verification performed this session; no build/test run (Windows toolchain path out of reach for recon budget).
- Graph was built code-only: docs/PDFs/images not semantically indexed (deliberate, local-first).
