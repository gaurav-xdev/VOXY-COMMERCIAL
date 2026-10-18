# OSMOO / VOXY: Production Readiness & Verification Audit Ledger

Last Updated: 2026-10-18 (Head: `7ddf904`)
Status: ACTIVE VERIFICATION LEDGER

## 1. Subsystem Production Readiness Matrix

| Subsystem | Owner Crate/Module | Core Capabilities | Readiness Status | Evidence / Verification |
|---|---|---|---|---|
| **Audio Capture & Playback** | `crates/voice_audio` | Low-latency WASAPI capture, audio conditioning (HPF, Noise Gate, AES -22dB, Soft Limiter), ring buffer | **VERIFIED** | 8 unit tests passed, live WASAPI hardware stream test passed. Physical stream fail-closed enforced. |
| **Voice Activation & VAD** | `crates/voice_activation` | RMS/ZCR VAD endpointing, Push-to-Talk, multi-peak energy wake word detector | **PARTIALLY VERIFIED** | 3 unit tests + noise/silence test passed. Wake-word model requires ONNX openWakeWord model for full production certification. |
| **Speech-to-Text (STT)** | `crates/voice_stt` | Cloud Groq Whisper turbo, Local Whisper CLI adapter, Hindi/Hinglish biasing | **VERIFIED** | 7 tests passed. Filesystem path check returns honest typed `LocalModelError` when binaries missing; zero mock fraud. |
| **Speech Synthesis (TTS)** | `crates/voice_tts` | SentenceChunker, Cloud TTS, Windows SAPI / Local TTS, Chatterbox | **VERIFIED** | 6 tests passed. Chunker hardened against splitting URLs, abbreviations, and ellipses. |
| **Voice Orchestrator** | `crates/voice_orchestrator` | Session state machine, Barge-in interruption, monotonic turn invalidation, Pipecat supervisor | **VERIFIED** | 10 unit + 2 e2e integration tests passed. Stale async turn invalidation via `turn_id` verified. |
| **Coding Harness** | `crates/harness` | Repository AST indexer, diagnostic parser, sandboxed runner, atomic patch transactions | **VERIFIED** | 20 unit + 1 isolated workflow test passed. Rollback preserves empty existing files; secrets scanner active. |
| **Security & Auth** | `crates/security`, `crates/ipc` | HMAC-SHA256 IPC token signing, constant-time verification, DPAPI session encryption | **VERIFIED** | Token forgery tests passed; DPAPI roundtrip encrypt/decrypt verified on Windows. |
| **Agent Runtime** | `crates/agent_runtime` | Multi-agent orchestrator, role assignments, message bus, task lifecycle | **VERIFIED** | 10 unit tests passed. Role matching and capacity bounds verified. |
| **Desktop UI & Overlay** | `crates/desktop_ui`, `apps/overlay` | Slint UI, system tray, overlay companion surface, DPAPI auth bridge | **PARTIALLY VERIFIED** | Auth bridge protected with DPAPI. Multiple Tokio runtime allocations in UI remain to be consolidated. |

## 2. Verified Resolutions & Proof Ledger

1. **Audio Capture Fail-Closed (Rule 3 Compliance)**
   - *Issue*: `crates/voice_audio/src/lib.rs` fell back to virtual handle with `is_active: true` when physical stream initialization failed.
   - *Fix*: Added `allow_virtual_fallback` flag to `AudioConfig` (default `false`). Now returns honest `Err(AudioError::InputStreamError)` unless virtual simulation is explicitly enabled. `is_active()` requires active physical stream handle.
   - *Proof*: `cargo test -p voxy-voice-audio` (8 tests passed, live WASAPI passed).

2. **Linguistic Sentence Boundary Protection in TTS**
   - *Issue*: `SentenceChunker::find_sentence_boundary` split on `.`, breaking URLs (`osmoo.ai`), files (`lib.rs`), and abbreviations (`Dr.`, `e.g.`, `vs.`).
   - *Fix*: Implemented delimiter lookahead, ellipsis guard (`...`), decimal guard, and abbreviation dictionary checks in `crates/voice_tts/src/lib.rs`.
   - *Proof*: `cargo test -p voxy-voice-tts` (`test_sentence_chunker_url_and_abbrev_preservation` passed).

3. **Monotonic Turn ID & Stale Turn Invalidation in Voice Orchestration**
   - *Issue*: In-flight LLM/TTS generation could speak stale text after barge-in or subsequent user utterance.
   - *Fix*: Added `turn_id: Arc<AtomicU64>` to `VoicePipeline`. Barge-in increments `turn_id`. `dispatch_llm_and_tts` records active `turn_id` and checks equality before triggering TTS stream.
   - *Proof*: `cargo test -p voxy-voice-orchestrator` (10 unit + 2 e2e tests passed).

4. **Harness Atomic Rollback Empty-File Preservation**
   - *Issue*: `PatchSnapshot` treated empty 0-byte existing files as non-existent and deleted them upon rollback.
   - *Fix*: Added `existed: bool` flag to `PatchSnapshot`. If `existed` is true, rollback truncates/preserves the file.
   - *Proof*: `cargo test -p voxy-harness` (20 unit tests + isolated workflow test passed).

5. **Desktop Session DPAPI Encryption at Rest**
   - *Issue*: Auth tokens stored in plaintext JSON on Windows filesystem.
   - *Fix*: Added Windows DPAPI `CryptProtectData` / `CryptUnprotectData` in `crates/desktop_ui/src/bridge/auth.rs`.
   - *Proof*: `test_dpapi_protect_and_unprotect_roundtrip` passed.

6. **HMAC-SHA256 Constant-Time IPC Token Authentication**
   - *Issue*: IPC authentication token lacked cryptographic signature verification.
   - *Fix*: Added HMAC-SHA256 signature verification in `crates/security/src/token.rs` and `crates/ipc/src/auth.rs`.
   - *Proof*: Unit tests for token expiration, tampered secret, and altered signature rejection passed.
