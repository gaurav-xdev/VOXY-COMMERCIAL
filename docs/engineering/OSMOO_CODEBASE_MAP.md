# OSMOO / VOXY Codebase Map

Last verified: 2026-10-17 at HEAD `22155e3` (branch `master`, clean tree except untracked `graphify/` + `.freebuff/`).
Evidence tool: Graphify graph (`graphify-out/graph.json`, 14,457 nodes / 29,865 edges, code-only AST extraction) + direct source reads.

## Identity
- Repo `gaurav-xdev/VOXY-COMMERCIAL`; product name in code is **VOXY/OSMOO** interchangeably.
- Rust workspace: `apps/*` + `crates/*` (~76 crates), `resolver = 2`, `rust-version 1.76`.
- Binaries: `apps/daemon` (headless companion service, ~2,100-line main.rs) and `apps/overlay` (overlay UI). A *desktop companion UI* lives in `crates/desktop_ui` (lib, used by overlay), NOT `apps/`.
- Python Pipecat microservice: `services/voice_pipecat/pipecat_pipeline.py`.

## Major subsystems (Graphify communities, source-verified)
| Subsystem | Path | Key symbols |
|---|---|---|
| Daemon app | `apps/daemon/src/{main,background,shutdown,tools}.rs` | `create_llm_provider()` (env-driven: ollama/openai/anthropic/gemini/groq/openrouter/nvidia, falls back to Ollama when key missing) |
| Voice pipeline | `crates/voice_orchestrator/src/lib.rs` | `VoicePipeline`, `VoicePipelineConfig`, `PipelineError`, `ResponseHandlerFn` |
| Pipecat bridge | `crates/voice_orchestrator/src/pipecat.rs` | `PipecatSupervisor`, `PipecatConfig` (TCP `port 18889`, confirmed `pipecat.rs:158`) |
| Voice audio | `crates/voice_audio` | `AudioRuntime`, `AudioCaptureHandle`, `AudioPlaybackHandle` |
| Voice activation | `crates/voice_activation/{lib,wake_word}.rs` | `VadDetector`, `VadEvent`, `WakeWordDetector` (energy-envelope heuristic, `count_energy_peaks` at wake_word.rs:130) |
| STT | `crates/voice_stt/src/lib.rs` | trait `SttEngine`; impls `GroqSttEngine` (cloud whisper-large-v3-turbo), `LocalWhisperSttEngine` (**placeholder**), `MockSttEngine` |
| TTS | `crates/voice_tts/src/lib.rs` | trait `TtsEngine`; impls `CloudTtsEngine`, `LocalTtsEngine` (Windows SAPI via PowerShell COM), `ChatterboxTtsEngine`, `MockTtsEngine`, `SentenceChunker` |
| LLM providers | `crates/{provider_core,openai,anthropic,gemini,ollama,model_router}` | `LlmProvider`, `select_provider()` (model_router/lib.rs:219) |
| Harness | `crates/harness/src/{lib,patch_engine,sandbox_runner,repo_indexer,diagnostic_parser,secret_scanner,ide_launcher}.rs` | `PatchSnapshot` (patch_engine.rs:31) |
| Security | `crates/security/src/{approval,rollback,secrets}.rs` + guardian | `ApprovalBroker` (approval.rs:124), rollback.rs |
| IPC | `crates/ipc/src/{transport,transport.rs(named_pipe serves in transport.rs),named_pipe,server,protocol,voxy_protocol,auth,auth_middleware,replay,cancellation}.rs` | `VoxyIpcServer` (named_pipe.rs:125), `DaemonMessage` (voxy_protocol.rs:91), transport.rs:9 has `Tcp` variant |
| Automation | `crates/automation` (incl. `backends/hybrid.rs`) | `WindowsUiaBackend` (used in daemon main.rs) |
| Cognition/brain | `crates/{brain,cognition,companion_intelligence,cognitive_orchestrator,conversation}` | `CognitiveEngine`, `CognitiveBridge`, `MomentEngine`, `ProactiveEngine` |
| Memory | `crates/memory/sqlite_engine.rs` (SQLite LTM V2), `crates/database` | sqlite_engine.rs |
| Desktop UI | `crates/desktop_ui/src/main.rs` + `bridge/{auth,ipc}.rs` | multiple `Runtime::new()` + `block_on` sites (main.rs:136,396,429) |
| Pipecat service | `services/voice_pipecat/pipecat_pipeline.py` | `Frame` (L65), `TranscriptionFrame` (L80) |
| Desktop runtime | `crates/desktop_runtime` + `crates/{platform_windows,platform_core,platform_linux,platform_macos}` | `DesktopRuntime` |
| Vision | `crates/vision` | `CaptureEngine`, `PixelFormat` |
| E2E tests | `tests/e2e/tests/{tier1..tier4}*.rs` | tier4 = "real world" |

## Entry points & process boundaries
1. `apps/daemon` — spawns voice pipeline, cognitive engine, automation backend (`WindowsUiaBackend`), runtime guard, world model bridge, tools (`src/tools.rs`), background services.
2. `crates/desktop_ui` binaries — user-facing UI; auto-spawns daemon + overlay (see help text at main.rs:350).
3. `apps/overlay` — always-on-top companion surface.
4. Python Pipecat process managed out-of-band by `PipecatSupervisor` over TCP 18889.

## Text pipeline of daemon chat (main.rs)
`create_llm_provider()` (env `VOXY_LLM_PROVIDER`) → sanitize_input (voxy_security) → provider → tools/automation → `sanitize_llm_output` → audit log (`AuditLog`, `AuditEventType`) → runtime_guard.

## Known structural smells confirmed this session
- Daemon `main.rs` = 2,101 lines: provider factory, voice bootstrap, chat loop, acoustic loopback test all inline (legacy patch-target of record — see OSMOO_KNOWN_ISSUES).
- `crates/desktop_ui/src/main.rs` creates standalone Tokio runtimes inside `block_on` blocks at ≥3 sites — runtime lifetime for spawned bridge tasks is an open re-verification item.
- `LocalWhisperSttEngine` returns fabricated text with confidence 0.90 (voice_stt lib.rs:273-292) — see issues register.
