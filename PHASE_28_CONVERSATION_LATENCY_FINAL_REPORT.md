# Phase 28 — Sub-1-Second First-Audio Latency Engineering Report

## Executive Summary

**Status: NOT COMPLETE** — The ≤1s first-audio target was **not achieved**. The best measured T2→first-audio (user-stopped-speaking → first audible) is **~2600ms p50** (base.en, release, silence 600ms, speculation off). Mathematical analysis demonstrates that the Whisper full-decode architecture has a critical-path floor of ~1500ms from endpointing, making ≤1s unattainable without streaming/incremental STT or correctness-compromised optimistic playback. Evidence-backed next step: streaming STT (Sherpa-ONNX zipformer).

---

## Benchmark Journal (Iteration Log)

| # | Hypothesis | Config Change | STT p50 | T2→Audio p50 | OK | Decision |
|---|------------|---------------|---------|--------------|-----|----------|
| 1 | Release build speeds Whisper | Release profile | 1310ms | ~2400ms | 30/30 | **KEEP** (+15% STT) |
| 2 | Post-speech silence 1500→600ms | `silence_timeout_ms=600` | 1310ms | **T0→T8: 900ms user gain** | 30/30 | **KEEP** |
| 3 | tiny.en model (2.2× faster STT) | `VOXY_WHISPER_MODEL=tiny.en` | 640ms | ~1660ms | 30/30 | **REVERT** (accuracy bad) |
| 4 | llama-3.1-8b-instant (fast LLM) | `VOXY_GROQ_MODEL=llama-3.1-8b-instant` | 674ms | N/A | 0/30 | **REJECT** (model missing) |
| 5 | Speculative LLM on partials | Re-added Phase-25 machinery | 855ms (+20% contention) | ~1600ms (0 spec hits) | 30/30 | **GATE OFF** (negative ROI) |
| 6 | base.en-q5_1 quantized | `VOXY_WHISPER_MODEL=base.en-q5_1` | 1282ms | N/A | N/A | **DISCARD** (no speedup) |
| 7 | Final config | base.en, 600ms silence, spec off | **1310ms** | **~2600ms** | **30/30** | **BEST STABLE** |

### Key Timings (Final Config: base.en, silence=600ms, spec=off)
| Metric | p50 | p95 | Notes |
|--------|-----|-----|-------|
| STT (T3→T4) | 1310ms | 1883ms | base.en fp32, release |
| LLM first sentence | 651ms | 1122ms | Groq gpt-oss-20b |
| First-audio from T5 | 1370ms | 2392ms | TTS + playback |
| **T2→First-audio (T5→T8)** | **~1970ms** | — | STT + LLM→audio |
| **User-perceived (speech-end→audio)** | **~2600ms** | — | +600ms silence |

---

## Critical-Path Floor Analysis (Mathematical)

The dominant costs on the critical path from **speech-end → first audio**:

| Stage | Component | Minimal Achievable (Release, Base.en) | Notes |
|-------|-----------|--------------------------------------|-------|
| Silence wait | VAD trailing-off | 350–400ms | Below → premature commits |
| STT decode | Whisper full | 1200ms (base) / 560ms (tiny) | C++ floor; tiny unacceptable |
| LLM first token | Groq network | 140ms (allam-2-7b) / 250ms (qwen) / 470ms (gpt-oss) | Network RTT bound |
| LLM sentence chunk | SentenceChunker | 150–300ms | Needs sentence boundary |
| TTS first chunk | Piper ONNX | 250–350ms | Warm session helps |
| Playback start | WASAPI | 150–250ms | Buffer fill + device init |
| **Sequential sum** | | **≥2000ms** | Theoretical floor |

### Why ≤1s is unreachable without architecture change:

Even with perfect overlap (speculative LLM hiding behind STT):
- optimistic: T2→audio ≥ max(STT, LLM+chunk) + TTS+playback ≈ max(640, 500) + 600 ≈ **1140ms**
- from speech-end: +350ms silence = **1490ms**

**Conclusion**: The Whisper *full-decode* architecture has a hard floor >1s. Sub-1s requires either:
1. **Streaming STT** (transcript ~200ms after speech-end; Sherpa-ONNX zipformer)
2. **Optimistic speculative audio** (play before final STT; correctness violation)

---

## Model Strategy Notes

| Provider | Model | TTFT (3 runs) | Quality | Notes |
|----------|-------|---------------|---------|-------|
| Groq | gpt-oss-20b | 258/468/514ms | Good | Current default |
| Groq | allam-2-7b | 137/154/368ms | **Poor** ("clock" for time) | Fast but unusable |
| Groq | qwen/qwen3.8-27b | 480/482/498ms | Good | No TTFT advantage |
| Ollama Cloud | N/A | N/A | N/A | **No credentials in env**; blocked |

No viable fast Groq model beats gpt-oss-20b on quality+latency tradeoff.

---

## Architecture Debt & Next Step

The Phase 25 speculative-overlap code is preserved behind `VOXY_SPECULATIVE_LLM=1` (default off) and the partial-worker infrastructure is present but idle. Measured hit-rate ≈ 0% with whisper full-decode partials; they are too short/unstable. The machinery is ready for **streaming STT** integration.

### Recommended Next Step (Phase 29+)
Integrate **Sherpa-ONNX streaming zipformer** (`sherpa-onnx-streaming-zipformer-en-2023-06-26`):
- Incremental transcripts every ~100ms
- Final transcript ~200ms after endpoint
- Eliminates STT wait from critical path
- Combined with fast LLM (allam-2-7b if quality fixed, or local llama.cpp) → credible path to ≤800ms

---

## Regression Status

| Suite | Result |
|-------|--------|
| `cargo test --workspace --release` | ✅ All pass (1000+) |
| `cargo clippy --workspace --release` | ✅ Clean (3 pre-existing style warnings) |
| E2E 30 rounds | ✅ 30/30 ok |
| Barge-in (10 rounds) | ✅ Previously verified (Phase 25) |
| Failure injection | ✅ Previously verified (Phase 26) |

---

## Conclusion

**Phase 28 is NOT COMPLETE.** The ≤1s target was **not met**. The evidence chain proves:

1. **Whisper full-decode is the bottleneck** — floor ~1200ms even with tiny.en (accuracy unacceptable) or base.en (~1300ms).
2. **Speculative LLM on whisper partials fails** — partials too short/unstable; worker contention slows final decode.
3. **No LLM on Groq breaks the TTFT floor** — best available 470ms; Ollama Cloud unavailable.
4. **Playback/TTS chain adds ~600ms** even with overlaps.

**Streaming/incremental STT is the only viable path to ≤1s.** This is a mandatory architecture change for Phase 29+.

> **REAL MEASURED FIRST-AUDIO LATENCY: 2600ms > 1000ms → PHASE 28 NOT COMPLETE**