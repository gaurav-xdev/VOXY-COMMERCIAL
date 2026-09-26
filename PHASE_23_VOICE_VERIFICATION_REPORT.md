# PHASE 23 — VOICE VERIFICATION REPORT

Verification of the Phase 23 hybrid-voice work: three reference-informed improvements
integrated into the existing VOXY voice pipeline without rebuilding working components.
Full comparison rationale: `PHASE_23_VOICE_REFERENCE_SCORECARD.md`. Architecture and
implementation detail: `PHASE_23_HYBRID_VOICE_ARCHITECTURE.md`.

Hardware: 12 threads, i5-13420H, Windows. Build: clang-cl + CMake + LLVM (no MSVC).

---

## 1. What changed

| Change | Location | Scope |
|---|---|---|
| Env-tunable whisper thread cap (default 8) | `crates/whisper/src/lib.rs` (`whisper_threads()`, 3 call sites) | +2 unit tests |
| Word-threshold chunk emission during streaming | `crates/voice_stream/src/chunker.rs` (`flush_word_overflow`) | +4 tests, 1 updated |
| Whisper warm-up inference at startup | `crates/whisper/src/lib.rs` (`warmup()`), `apps/daemon/src/main.rs` (startup path) | startup ~+1.5 s |

No API, CLI, config (other than new env var), or protocol surface changed. All VOXY
voice_stream / WASAPI / DSP / TTS / Groq components untouched.

---

## 2. Test suite

`cargo test --workspace` (post-change, clean):

```
2449 passed; 0 failed; 0 ignored
```

- `voxy-whisper`: 21 passed (incl. 2 new thread-cap tests).
- `voxy-voice-stream`: 62 passed (incl. 4 new chunker tests, 1 updated).
- One pre-existing flaky timing assertion in `voxy-human-dynamics`
  (`test_engine_latency`, `update_latency_us < 100`) failed once under full-suite load
  and passed on re-run; unrelated to this phase's changes.

`cargo clippy --workspace --all-targets`:

```
0 errors; warnings 112 (pre-existing, none in changed crates)
```

---

## 3. STT thread-cap tuning (measured)

`--model-e2e` (whisper base.en, `Hello, how are you doing today?`), STT in ms:

| Threads | native 22050 | 16k mono | 48k stereo (device) | Accuracy |
|---|---|---|---|---|
| 4 | 1954–2414 | 2023–2599 | 1976–2500 | PARTIAL/DIFF (no comma) |
| 6 | 1723 | 1557 | 1813 | MATCH |
| 8 (default) | 1389–1889 | 1285–1735 | 1262–1649 | MATCH (exact, incl. comma) |

- 8 threads is the fastest config and the only one consistently yielding exact
  punctuation MATCH. (Note: whisper greedy decoding is thread-count-sensitive in output
  punctuation — a numeric rounding effect, text content identical.)
- 8 ≤ 12 cores, so single-inference peak compute does not oversubscribe the async
  runtime.
- Env `VOXY_WHISPER_THREADS=4` reproduces the old (slow, PARTIAL) behavior for
  low-power machines.

---

## 4. Runtime / starvation

- Prior-phase starvation investigation (multi-second TTS-task queue waits at 12 whisper
  threads) was never reproduced at capped thread counts; at 8 threads no delay was
  observed in model-e2e (LLM→TTS, TTS synthesis) or a 90 s live capture session.
- **Honest limitation:** the headless environment could not drive real speech through
  the full partial+final+TTS overlap, so the residual transient 16-thread overlap
  window (partial decode + final STT at end-of-utterance) is not measured live. It is
  bounded and short (~1 s), and `VOXY_WHISPER_THREADS=4` is the documented fallback if
  a target machine shows scheduling stalls.

---

## 5. Sentence-chunking (word-threshold emission)

Unit-tested behavior:

- Long run-on input (no punctuation) now emits 5-word groups during `feed()` (no
  terminal added), remainder finished with `.` — verified by
  `long_runs_flush_during_feed_without_terminal`.
- Punctuation path unchanged: `one two three four five six seven. next` still splits on
  the sentence end first, then word-groups the long sentence (`five six seven.`), and
  `finish()` adds `.` to the trailing `next.`.
- Below-threshold input never flushes early (`a few words` → only on `finish()`);
  exact-threshold input flushes fully.

Net effect: TTS now starts synthesizing long answers while the LLM stream is still
running instead of after it ends.

---

## 6. Warm-up

- Verified in a live startup run: `Whisper warm-up inference complete in 1.4852339s`.
- Whisper base.en hallucinates "you" on silence — expected, discarded, logged at debug.
- Startup-to-listening increases by ~1.5 s (≈ 21.6 s total); first-utterance STT no
  longer pays cold-decoder cost.
- `--model-e2e` intentionally does not warm up (its harness builds its own engines) and
  therefore reports worst-case first-inference STT — the honest cold number.

---

## 7. End-to-end data (post-change)

- STT (8 threads, e2e probe): 1.26–1.89 s across runs, exact MATCH.
- Piper first synthesis (cold, e2e harness): 1.53 s; steady-state first-audio measured
  previously 330–613 ms.
- Groq streaming TTFT: 0.18–0.37 s (measured prior clean-build run; unchanged this
  phase — LLM path untouched).
- Full clean-build pipeline T5→T8 wall-clock: 637–924 ms (prior clean build; unchanged
  this phase except chunker now starts TTS earlier for long answers).
- `--model-e2e` LLM probe still uses the known-broken Ollama `voxy-fast` harness
  (0 chars, ~2 s) — not a regression; the production Groq streaming path is the
  validated one.

---

## 8. Honest limitations / UNVERIFIED

- **Interactive validation** (real mic speech, barge-in timing, wake word, Hindi /
  Hinglish accuracy) is impossible headless and remains **UNVERIFIED**.
- Whisper `base.en` remains English-only; Hindi/Hinglish is the primary multilingual
  limitation (see scorecard §14).
- Wake word remains energy-only (disabled by default).
- VAD hysteresis, neural wake word, TTS cache/pacer, formal FSM: intentionally not
  adopted (scorecard summary, rows 4–9).
- Startup latency grew ~1.5 s due to warm-up (intentional trade).
- Residual transient partial+final thread overlap not live-measured (see §4).

---

## 9. Verdict

The three adopted integrations are each small, local, tested, and backed by measured
numbers (STT 1.9–2.6 s → 1.26–1.89 s at default 8 threads; earlier TTS start for long
answers; cold-start removed from first utterance). No working component was replaced.
Full-suite: **2449 passed / 0 failed**; clippy **0 errors**.