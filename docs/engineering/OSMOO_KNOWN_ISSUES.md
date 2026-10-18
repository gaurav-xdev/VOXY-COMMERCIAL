# OSMOO Known Issues Register

Each entry: status (CONFIRMED / RE-VERIFY / OPEN) + evidence. Statuses refer to HEAD `22155e3`, 2026-10-17. Add remediation notes as fixes land.

## Confirmed this session

### KI-1 · Local Whisper STT is a fabricated placeholder — CONFIRMED, HIGH
`crates/voice_stt/src/lib.rs:273-292` (`LocalWhisperSttEngine::transcribe`) returns
`format!("[Local Whisper ({})] Audio transcribed", self.model_name)` with `confidence: 0.90, is_final: true` —
no recognition model is invoked. Daemon (`apps/daemon/src/main.rs:548`) wires this engine as the offline fallback
whenever Groq credentials are absent, so an offline user gets fabricated transcripts.
Fix direction: real whisper.cpp binding (e.g. `whisper-rs` / `whisper_cpp` crate) or explicitly error
`SttError::Unavailable` instead of fabricating text; never return invented transcription text.

### KI-2 · Wake-word detection is an energy-envelope heuristic, not a trained model — CONFIRMED, MED
`crates/voice_activation/src/wake_word.rs:88-130` uses rolling-buffer energy + `count_energy_peaks(1600)`
("two to three energetic envelope peaks characteristic of Hey-OS-MOO"). Doc comment mentions
openWakeWord-format embeddings but no model is loaded. Additionally the daemon has **no wiring** of
`WakeWordDetector` at all (grep of apps/daemon/src/main.rs for `wake` returned no wiring lines) —
word-boundary FST or onnx openWakeWord model needed, then integrate into production capture path.

### KI-3 · Provider fallback silently switches to Ollama — CONFIRMED, MED
`apps/daemon/src/main.rs` (all cloud provider branches): missing env key ⇒ `warn!` + silent Ollama fallback.
No user-visible signal, no manifest change; a user believing they run OpenAI actually runs local Ollama
(privacy-relevant). Fix: explicit `ProviderSelection` event surfaced via IPC/UI.

### KI-4 · IPC auth secret is optional `Option<String>` — RE-VERIFY, potentially HIGH
`crates/ipc/src/named_pipe.rs:143` reads `VOXY_IPC_AUTH_TOKEN` via `.ok()` into `auth_secret: Option<String>`
(line 127). Whether the server enforces fail-closed when unset is **not yet verified** (audit item 20) —
read the auth gate before any IPC work; do not assume.

## Re-verify next session (carried from prior audit, not yet re-checked)
- KI-5 Voice startup may report success on audio-init failure (item 3).
- KI-6 voice_speed/voice selection may not reach TTS backends (item 4) — check `SentenceChunker`/TTS impls.
- KI-7 Sleep-state query always false (item 5).
- KI-8 Per-frame task spawning in audio processing path (item 6).
- KI-9 Fixed 44-byte WAV header assumption / 16-bit PCM only (item 7) — `decode_wav_to_f32` in voice_orchestrator tests + voice_tts encode path.
- KI-10 Whole-response-then-TTS instead of token streaming (item 8) — inspect `dispatch_llm_and_tts` (daemon main.rs).
- KI-11 Pipecat TCP 18889 (pipecat.rs:158) vs named-pipe IPC transport compatibility (item 10); supervisor health wiring (item 11) — note `PipecatSupervisor::new` failure is "deferred" and pipeline starts anyway (voice_orchestrator lib.rs:96-104).
- KI-12 Harness rollback emptiness heuristic / snapshot ownership (items 12-13) — `crates/harness/src/patch_engine.rs`, `sandbox_runner.rs` (item 15), patch-verify failures (item 14).
- KI-13 desktop_ui multiple `Runtime::new()` + `block_on` sites (main.rs:136, 396, 429) — runtime drop while bridge tasks alive (item 16).
- KI-14 UI dual response paths / duplicate replies (item 17); onboarding/settings partial persistence (item 18).
- KI-15 Auth bridge local-only vs live entitlements (item 19); plaintext session storage (item 21).
- KI-16 Default model `gpt-oss:120b-cloud` behind "local/private Ollama" labeling (item 22).
- KI-17 UIA OCR/screen-text-search backend coverage (item 23).
- KI-18 E2E tier-4 "real world" test uses synthetic audio/mocks (item 9) — `crates/voice_orchestrator/tests/voice_e2e_real_world_test.rs`.
- KI-19 Release artifact `release/OSMOO-v1.0.0-Portable-x64.zip` provenance/build reproducibility unverified (item 10 rules).

## Verification status
MainWindow-level runtime checks this session: none (recon only). Everything marked RE-VERIFY requires source inspection first, runtime check second.
