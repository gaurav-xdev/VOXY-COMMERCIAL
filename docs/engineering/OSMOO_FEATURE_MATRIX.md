# OSMOO Feature Matrix

Legend: ✅ implemented+integrated · ⚠ partial/unverified · 🧪 mock/test-only · ✖ stub/placeholder · ❓ unverified. Each row carries evidence; "unverified" means no runtime demonstration found this session.

| Feature | Status | Evidence / source |
|---|---|---|
| DashScope (Ollama) LLM provider | ✅ wired | `apps/daemon/src/main.rs:31-40` default `VOXY_LLM_PROVIDER=ollama` |
| OpenAI / Anthropic / Gemini / Groq / OpenRouter / NVIDIA-NIM providers | ⚠ implemented, fail back to Ollama when key missing (silent provider switch) | main.rs:42-120 |
| Provider fallback semantics | ⚠ fallback = silent log + switch, no observable signal to user | main.rs:46, 66-73 |
| Tool calling | ⚠ `crates/tool_calling` exists; integration unverified in daemon path this session | graph: provider_core registry |
| Cloud STT (Groq whisper-large-v3-turbo) | ⚠ implemented, real HTTP client; needs live credential test | voice_stt lib.rs:97-152, daemon main.rs:538 |
| Local Whisper STT | ✖ **STUB — confirmed placeholder** | voice_stt lib.rs:273-292 returns `"[Local Whisper ({model})] Audio transcribed"`, confidence 0.90, no model call |
| Windows SAPI TTS (local) | ⚠ real COM invocation via PowerShell temp file, but file-based temp+PowerShell design is fragile & slow; no verification this session | voice_tts lib.rs:229-250 |
| Cloud TTS / Chatterbox TTS | ❓ code exists (voice_tts lib.rs:129, 282), unverified | — |
| Streaming TTS / sentence chunker | ⚠ `SentenceChunker` exists (voice_tts lib.rs:32); incremental LLM→chunk→TTS wiring unverified | voice_orchestrator lib.rs:27 |
| Barge-in / interruption | ⚠ `active_cancellation: CancellationToken` + `trigger_barge_in`; behavior at runtime untested here | voice_orchestrator lib.rs:79 |
| VAD | ✅ energy/RMS detector (not trained model) | voice_activation lib.rs |
| Wake word | ✖ **heuristic only — energy peaks (silly-burst counting), NOT a trained acoustic model** | wake_word.rs:88-130; daemon main.rs has no wake wiring (grep confirmed) |
| Mock STT/Mock TTS | 🧪 explicitly test-only | voice_stt lib.rs:298, voice_tts lib.rs:359 |
| Pipecat bridge | ⚠ Rust supervisor on TCP port 18889 (pipecat.rs:158); Python service exists (`services/voice_pipecat`); transport compat end-to-end unverified | both sides |
| Pipecat supervisor health/restart | ❓ `PipecatSupervisor::new` used with `init deferred` on failure (voice_orchestrator lib.rs:96-104) — startup proceeds without Pipecat; unverified |
| IPC transport | ✅ Windows named pipe server `VoxyIpcServer` (named_pipe.rs:125) + `Tcp` variant in transport.rs:9 + protocol `DaemonMessage` | named_pipe.rs, voxy_protocol.rs |
| IPC auth | ⚠ secret read from env `VOXY_IPC_AUTH_TOKEN` as `Option` (named_pipe.rs:143) — behavior when unset NOT verified; fail-closed not yet proven |
| IPC replay/cancellation | ❓ `ipc/src/replay.rs`, `cancellation.rs` exist, unverified | — |
| Patch engine (harness) | ⚠ `PatchSnapshot` (patch_engine.rs:31) + `secret_scanner`, `sandbox_runner` — rollback emptiness heuristic (audit item 12) NOT yet re-verified | — |
| Approval enforcement | ❓ `ApprovalBroker` (security/approval.rs:124) exists; coverage across tool entry points unverified | — |
| OS sandboxing | ❓ `sandbox_runner.rs` exists; whether it enforces real OS isolation (Job Object/AppContainer/WSB) unverified (audit item 15) | — |
| Desktop automation | ⚠ `WindowsUiaBackend` wired in daemon (main.rs:14); capability coverage (OCR/screen search) unverified (audit item 23) | — |
| Long-term memory | ⚠ SQLite engine (`memory/sqlite_engine.rs`) + encryption in `database/src/encryption.rs`; migrations/recovery unverified | — |
| Session persistence security | ❓ audit item 21 (plaintext JSON session) not yet re-verified | — |
| Auth bridge / Supabase entitlements | ❓ `desktop_ui/src/bridge/auth.rs` exists; audit item 19 (local-only auth) not re-verified | — |
| UI event flow / duplicate replies | ❓ audit item 17 not re-verified | desktop_ui bridge |
| E2E tests | ⚠ 4 tiers exist (`tests/e2e/tests/tier1..tier4` incl. `voice_e2e_real_world_test.rs`); audit item 9 (synthetic audio + mocks) not re-verified; tier4 name says "real world" but evidence pending | — |
| Stress/production hardening | ✅ suite exists | `crates/production_harden`, root stress scripts, STRESS-TEST-REPORT.md |
| Windows packaging | ⚠ `installer/`, `wix/`, `release/OSMOO-v1.0.0-Portable-x64.zip` exist on disk (graphify scan saw AppxManifest.xml) — artifact provenance **unverified** | graphify extract output |

## Voice speed/settings propagation
❓ `VoicePipelineConfig.voice_speed` exists (voice_orchestrator lib.rs:51); whether TTS engines honor it unverified (audit item 4).
