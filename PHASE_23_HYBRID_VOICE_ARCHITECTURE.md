# PHASE 23 — HYBRID VOICE ARCHITECTURE

How the reference-informed improvements compose with VOXY's existing Rust/Windows voice
pipeline. The mission rule was: **do not rebuild working components**. This document
records exactly what was kept, what was adopted (with source justification), and what was
rejected (with evidence) after comparing VOXY against LiveKit Agents, Pipecat,
isair/jarvis, and OpenVoiceOS. Full per-component comparison: see
`PHASE_23_VOICE_REFERENCE_SCORECARD.md`.

---

## 1. Architecture principle

VOXY's voice stack was built and validated over prior phases. The reference survey
confirmed that VOXY's own components are equal-or-better on most axes (Windows-native
WASAPI, partial-streaming STT, terminal-punctuation endpointing, sentence streaming TTS,
barge-in with rewind, failure recovery). Only **three** mechanisms from the references
were demonstrably worth integrating, all small, local, tested:

1. Env-tunable whisper compute-thread cap (Jarvis `cpu_threads=cpu_count()` pattern).
2. Word-threshold sentence emission during streaming (LiveKit min-length chunking).
3. Whisper warm-up inference at startup (Jarvis noise-warm pattern).

Everything else — the voice_stream components (SentenceChunker, TranscriptStabilizer,
TieredEndpointing, PreRollBuffer, SpeculativePrefill, BargeInCoordinator, rewind_target),
WASAPI capture/output, DSP normalization, Piper TTS, Groq streaming — is preserved
unchanged.

---

## 2. Pipeline composition (post-Phase 23)

```
mic ──▶ WASAPI capture (48k 2ch, fallback 16k 1ch, hot-swap/backoff)
        │ normalize_to_engine (48k stereo → 16k mono)
        ▼
   TieredEndpointing (settle 120ms · trailing_off 700ms · max_silence 2s · filler drop)
        │ pre-roll buffer · terminal-commit
        ▼
   whisper-rs STT  ◀─ PARTIALS every 500ms (decode_partial, 8 threads)
        │  · env thread cap  VOXY_WHISPER_THREADS (default 8, ≤ cores, ≤ 16)
        │  · warm-up inference at startup (removes cold-decoder first-utterance cost)
        │  · session carry (last_text), hallucination filter
        ▼
   TranscriptStabilizer → SpeculativePrefill (Groq speculative on partials)
        ▼
   Groq streaming LLM (TTFT 0.18–0.37s) ──▶ SentenceChunker(max_words=20)
        │   · punctuation split (`.!?\n`, closing quotes, decimal-avoidance)
        │   · NEW: word-threshold flush during feed() → TTS starts earlier on
        │     long answers even without terminal punctuation
        ▼
   Piper TTS (onnxruntime, eager resident, length_scale = 1.0/speed)
        ▼
   WASAPI output (packet queue, fade-in, playback_started gating)
        ▲  BargeInCoordinator (VAD during TTS → abort LLM+TTS, rewind to sentence)
```

Timing markers T0–T9 remain the instrumentation contract for honest end-to-end
measurement.

---

## 3. Adopted mechanisms — implementation detail

### 3.1 Whisper compute-thread cap (Jarvis `cpu_threads=cpu_count()` pattern)

- **Before:** `n_threads = available_parallelism().min(4)` — fixed 4 threads.
- **After:** shared helper `whisper_threads()` in `crates/whisper/src/lib.rs` used by all
  three inference paths (sync `transcribe_audio`, async `transcribe_audio_async`,
  `decode_partial`):

  ```rust
  requested = env VOXY_WHISPER_THREADS  (default 8)
  clamp: [1, min(available_parallelism, 16)]
  ```

- **Why 8 as default:** measured on the validation hardware (12 threads, i5-13420H):
  STT at 4 threads ≈ 1.95–2.6 s; at 8 threads ≈ 1.26–1.9 s (fastest single-inference
  result, and the only config that consistently produced exact-punctuation MATCH in the
  e2e probe). 8 stays ≤ 12 cores so the async runtime is not oversubscribed during a
  single inference; the residual partial+final overlap window is transient and was not
  observed to reintroduce the prior multi-second TTS-task starvation (see
  verification report §4).
- **Tunability:** `VOXY_WHISPER_THREADS` lets slower/low-power machines drop back to 4
  without a rebuild.
- **Tests:** `whisper_threads_respects_env_override_and_upper_cap`,
  `whisper_threads_clamps_to_one_minimum`.

### 3.2 Sentence-chunker word-threshold emission (LiveKit `BufferedTokenStream` min-length)

- **Before:** `SentenceChunker::feed()` emitted only on sentence boundaries (`.!?\n`);
  the `max_words` fallback ran only on `finish()`. A long answer without punctuation was
  spoken only after the LLM stream ended.
- **After:** `feed()` now also calls `flush_word_overflow()` — once `pending` reaches
  `max_words`, full word-groups are emitted immediately (no terminal punctuation added,
  matching the existing long-sentence grouping), and the remainder (< `max_words`) stays
  pending so the next sentence boundary is still detected correctly. `finish()` is
  unchanged (guarantees terminal punctuation).
- **Net effect:** for answers longer than `max_words` words, TTS synthesis starts while
  the LLM is still streaming — first-audio latency for long replies drops from
  "wait for LLM end" to "wait for first `max_words` words".
- **Call sites:** unchanged (`SentenceChunker::new(20)` at main.rs spec-reuse and
  streaming paths; the streaming handler feeds per-LLM-delta at main.rs:1290, so flush
  is genuinely incremental).
- **Tests:** added `long_runs_flush_during_feed_without_terminal`,
  `word_flush_keeps_remainder_then_sentence_end`, `word_flush_never_fires_below_threshold`,
  `exact_threshold_flushes_entire_pending`; updated
  `word_count_fallback_splits_long_sentences` for the streaming behavior.

### 3.3 Whisper warm-up inference (Jarvis noise-warm pattern)

- **Before:** the first real utterance paid the whisper cold-decoder cost (allocations,
  thread-pool bring-up) on top of STT itself.
- **After:** `WhisperSttEngine::warmup()` runs one inference over 0.5 s of silence
  immediately after `load_model()` in the daemon startup path (main.rs). Measured
  ~1.49 s once at startup; discarded output (silence hallucination is expected and
  logged at debug level). Best-effort: a warm-up failure logs and continues.
- **Net effect:** the first utterance's STT no longer includes model bring-up; the cost
  is moved to background startup time.

---

## 4. Rejected mechanisms (recorded for honesty)

| Mechanism | Source | Why rejected |
|---|---|---|
| VAD hysteresis (confidence+volume, STARTING/STOPPING) | Pipecat | Energy VAD not proven to misfire on target hardware; change risks endpointing regressions with no measured benefit. |
| Neural wake word | OVOS/Jarvis | Large new framework; mission forbids unless necessary. Energy detector kept; limitation documented. |
| Multilingual whisper model | Jarvis/OVOS | Bigger model worsens already-dominant STT latency. Primary multilingual limitation; documented. |
| SentenceStreamPacer / TTS cache | LiveKit/OVOS | Measured TTS first-audio 330–613 ms is already good; no evidence of benefit. |
| Formal turn FSM | OVOS | Large refactor; behavior equivalent in the active path. |
| False-interruption resume / echo-salvage | LiveKit/Jarvis | VOXY barge-in verified working; refinements noted as future work. |

---

## 5. Configuration surface (new)

| Setting | Default | Meaning |
|---|---|---|
| `VOXY_WHISPER_THREADS` | 8 | whisper compute threads per inference; clamp `[1, min(cores,16)]` |

No other config, API, or CLI surface changed.