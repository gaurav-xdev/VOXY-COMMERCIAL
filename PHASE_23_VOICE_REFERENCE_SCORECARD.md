# PHASE 23 — VOICE REFERENCE SCORECARD

Deep architectural comparison of VOXY's existing voice pipeline against four mature
open-source voice systems: **LiveKit Agents**, **Pipecat**, **isair/jarvis**, and
**OpenVoiceOS**. Each reference was shallow-cloned and analyzed at source level
(`%TEMP%\opencode\ref\{livekit-agents,pipecat,jarvis,ovos}`). VOXY behavior was traced
from the actual Rust code (`crates/voice`, `crates/voice_stream`, `crates/whisper`,
`crates/kokoro`, `crates/audio`, `apps/daemon`).

Selection criteria (equal weight): latency, correctness, reliability, Windows
compatibility, streaming capability, resource usage, maintainability, failure
recovery, integration complexity, maturity.

Winner is chosen only where one implementation is **materially and demonstrably**
better for VOXY's Rust/Windows target. Where VOXY is equal-or-better, VOXY wins and is
kept (no code change). No popularity-based choices.

---

## 1. Audio capture (WASAPI)

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Capture backend | cpal/WASAPI, 48k 2ch, fallback 16k 1ch negotiation, hot-swap recovery with backoff | rtc (SFU tracks), browser/native | asyncio sounddevice/pyaudio, sample-rate conversion | sounddevice | pyaudio + `mycroft.conf` mic plugins | **VOXY** | Native WASAPI is the only first-class Windows path here. VOXY has device hot-swap, format negotiation (48k→16k fallback), and recovery. References are cross-platform generic backends (pyaudio/sounddevice) that VOXY already tested and rejected for Windows reliability. |

## 2. VAD + endpointing

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| VAD | energy threshold (0.05) + TieredEndpointing (settle 120ms, trailing_off 700ms, max_silence 2s, filler detection) | silero ONNX, hysteresis (act 0.5 / deact 0.35), ExpFilter(0.35), min_speech 50ms, min_silence 550ms, prefix 500ms | 4-state QUIET→STARTING→SPEAKING→STOPPING, confidence(0.7)+volume(0.6) gate, start/stop 200ms | WebRTC VAD aggr=2, pre_roll 240ms, endpoint_silence 800ms, max_utterance 12s | dinkum-listener FSM, VAD before_command 0.3s, silence 0.7s | **VOXY** (endpointing) + **Pipecat** (VAD hysteresis pattern) | VOXY's TieredEndpointing (terminal-commit, trailing-off, filler-drop, max-silence) is the most complete endpointing logic in the set. Pipecat's dual confidence+volume gate with STARTING/STOPPING hysteresis is the strongest anti-false-trigger VAD pattern. VOXY uses plain energy VAD; adopting Pipecat's hysteresis would harden it, but current VOXY VAD is not proven to misfire on the target hardware — marked PARTIAL (see §15). |

## 3. Wake word

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Wake word | energy-threshold only, disabled by default (`wake_word_enabled: false`) | not core (bring-your-own) | not core | post-ASR text match + fuzzy (0.78) + LLM intent judge confirmation | plugin hotword engines (precise/porcupine), multi-hotword, state-gated deactivation, hotword verifier | **OpenVoiceOS** (architecture) / **Jarvis** (real-word handling) | VOXY's energy-only detector is explicitly a limitation. OVOS has the most mature wake-word plugin discipline (multi-hotword, listen/wakeup/stopword classification, deactivation after activation). Jarvis demonstrates real-word + fuzzy-ASR matching inside natural speech with LLM judge anti-false-positive. **Decision: NOT integrated** — a neural wake-word framework is a large, risky addition the mission forbids ("do not introduce a giant new framework unless necessary"); VOXY keeps its energy detector and documents the limitation. |

## 4. STT / partial transcription

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| STT | whisper-rs ggml base.en, CPU 4 threads, session carry (last_text prompt), partial re-decode every 500ms, TranscriptStabilizer, confidence/no-speech filter | cloud STT (Deepgram etc.), streaming + interim results, keyterms/context injection, STT stream adapter | cloud STT (Deepgram/Gladia/etc.), interim frames, `request_finalize`/`confirm_finalize` | faster-whisper (CTranslate2) medium, int8, `cpu_threads=os.cpu_count()`, batch (no partials), VAD-filter off | streaming STT wrapper, batch engines via FakeStreamingSTT, no partials | **VOXY** (partial streaming) + **Jarvis** (thread/config) | Only VOXY (with its partial→stabilizer→spec architecture) and LiveKit/Pipecat do true partial streaming; VOXY is the only local-CPU implementation of it, with session carry and hallucination filtering. Jarvis's `cpu_threads=cpu_count()` with int8 shows VOXY's 4-thread cap is conservative — this is the **highest-value tuning lever** (measured STT 1.9–3.1s at 4 threads). Adopted: env-driven thread cap with tuned default (see implementation). |

## 5. Turn detection

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Turn commit | TieredEndpointing (terminal-commit, trailing-off 700ms, filler drop) + commit transcript | DynamicEndpointing (learned min_delay, max_delay 3s), EOT model, STT commit with 0.2s silence flush | SpeechTimeout (0.6s + STT ttfs safety), MinWords (1 word during bot speech) | 800ms endpoint silence | 0.7s silence in IN_COMMAND | **VOXY** | VOXY's terminal-punctuation-commit + trailing-off is faster than fixed-silence approaches for punctuated speech and has filler-drop to avoid committing "um/uh". LiveKit's learned dynamic endpointing is sophisticated but cloud-model dependent and over-engineered for VOXY's local scope. |

## 6. LLM streaming + preemptive generation

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| LLM | streaming (Groq), sentence-chunked TTS, SpeculativePrefill on partials, TTFT 0.18–0.37s (measured), session memory, cancellation via task abort + cooperative checks | streaming + preemptive generation on interim transcript (enabled, max_speech 10s, retries 3), ttft/tps metrics, flush sentinel | per-token LLMTextFrame, interruption cancels tools but not stream | non-streaming batch chat (no incremental TTS) | n/a (no in-pipeline LLM) | **LiveKit** (preemptive gen parity) / **VOXY** | VOXY already implements the proven preemptive pattern (SpeculativePrefill) — parity with LiveKit's preemptive generation. VOXY is strictly ahead of Jarvis (non-streaming) and Pipecat (no speculative). Keep VOXY; the spec-reuse gate measured working. |

## 7. Sentence chunking

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Chunker | punctuation (`.!?\n` + closing quotes + decimal-avoidance), max_words=20 fallback **only on finish()**, guarantee terminal punct | Blingfire tokenizer, min_sentence_len 20, stream_context_len 10, BufferedTokenStream with min/max caps | NLTK with lookahead, flush-on-LLM-end, force-promote pending sentence at turn end | whole-reply single synthesis | quebra_frases sentence tokenize | **LiveKit** (word-threshold emission) / **VOXY** (punct correctness) | VOXY's punctuation handling (decimal points, closing quotes, newlines, Devanagari-capable endings) is more careful than LiveKit's, but LiveKit emits once a minimum token length is reached **even without punctuation** — the exact "safe word threshold" the mission requires. VOXY only flushes word-count fallback on `finish()`. **Adopted: emit at word threshold during streaming** (small, tested change). Pipecat's "no timer, flush-on-end" is equivalent to VOXY's finish(); a wall-clock timer adds complexity without measured benefit. |

## 8. TTS

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| TTS | Piper (onnxruntime), sentence-level streaming synthesis, speed→length_scale mapping, loaded eagerly at startup | streaming websocket TTS, 50ms audio chunks, bounded source queue (200ms), prewarm(), SentenceStreamPacer | streaming services, AggregatedFrameSequencer (SENTENCE/TOKEN), serialization queue, pause watchdog | Piper with `length_scale=0.65` (≈30% faster), eager prewarm | ovos-audio PlaybackThread, TTS cache by sentence hash, serialize on playback_lock | **VOXY** (sentence streaming) + **LiveKit** (pacer) | VOXY does real sentence-level streaming TTS with backpressure and eager model residency — ahead of Jarvis (whole-reply) and OVOS (batch files). LiveKit's SentenceStreamPacer (first sentence immediate, batch when audio stalls) and OVOS's TTS-cache are the notable extras. TTS first-audio measured 330–613ms (good); no change needed. Piper `length_scale` tuning left at 1.0 to avoid altering voice personality (mission rule). |

## 9. Playback

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Playback | WASAPI output 48k, audio packet queue, fade-in, reference ring buffer for rewind, playback_started tracking | participant audio output, clear_buffer, pause/resume, resampler | 40ms out-chunks, end-silence 2s, clock queue, interruption-preserving FrameQueue reset | sounddevice blocksize 1024, interrupt aborts stream | play_audio subprocess, queue drain | **VOXY** | VOXY's ring-buffer + rewind_target enables the verified barge-in rewind-to-sentence-boundary; none of the references offer rewind. Playback device handling is WASAPI-native. |

## 10. Barge-in

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Barge-in | BargeInCoordinator, VAD-frame detection during TTS (gated on `tts_playback_started`), abort LLM+TTS tasks, rewind to sentence boundary, verified in real runs | VAD speech≥0.5s → pause/hard interrupt, false-interruption timeout 2.0s + resume, clear_buffer | InterruptionFrame broadcast (bypass queue), reset aggregator + audio context, keep UninterruptibleFrame | stop-words + echo detection/salvage (distinguish self-audio), tts.interrupt() | `mycroft.stop` → clear player (no acoustic barge-in) | **VOXY** | VOXY is the only one with measured, working acoustic barge-in that rewinds to a sentence boundary without corrupting playback. LiveKit's false-interruption timeout/resume and Jarvis's echo detection are the strongest complementary ideas; both are refinements VOXY could add later, not blockers. No change required. |

## 11. Conversation state machine

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| State | listening loop, is_awake flag, wake-word gate (disabled), utterance lifecycle, session carry | idle→thinking→speaking→listening, SpeechHandle queue, authorization gate, follow-up via tool ctx.update | turn-start/stop strategies | wake→hot window (3s, no wake word needed)→follow-up via 120s transcript buffer | dinkum-listener FSM (DETECT_WAKEWORD→CONFIRMATION→BEFORE_COMMAND→IN_COMMAND→AFTER_COMMAND), ListeningMode, session-based converse | **OpenVoiceOS** (FSM clarity) / **VOXY** | VOXY's listening loop is functional and instrumented, but OVOS's explicit FSM + ListeningMode is the clearest, most battle-tested lifecycle. Converting VOXY to a formal FSM is a large refactor with no measured latency benefit — NOT adopted; VOXY's behavior is equivalent in the active path. Jarvis's hot-window follow-up is a good pattern VOXY partially has (spec reuse + session). |

## 12. Performance / prewarming / instrumentation

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Instrumentation | T0–T9 timing logs, watchdog, metrics collector, streaming metrics (tts_first_chunk, llm_first_token) | ttft/tps/ttfs/ttfb/e2e latency trace attrs, early metrics | startup-timing observer, user-bot-latency observer | n/a | n/a | **LiveKit** (richest) / **VOXY** (adequate) | VOXY's T0–T9 instrumentation is comprehensive enough for the validation report. LiveKit adds per-node ttfs/ttfb/e2e which VOXY approximates. Keep VOXY's; no change. |
| Prewarming | whisper + piper eager load at startup (~20s) | TTS.prewarm, forkserver native preload | warm_deferred_imports (NLTK in thread) | parallel LLM warmup + noise-warm whisper + eager piper | eager hotwords/TTS, lazy fallback | **VOXY** (parity) | VOXY already does eager model residency. Jarvis's whisper noise-warm (run decoder once on low-amplitude noise) is a real refinement — VOXY's first real STT pays cold-decoder cost. **Adopted: whisper warm-up inference at startup** (cheap, removes first-utterance STT cold start). |

## 13. Failure recovery / resilience

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Recovery | device hot-swap with backoff, watchdog + self-healing, graceful model-load failure (verified: whisper/piper/LLM), queue bounds, runtime guard | retry/fallback via inference workers | transport restart, interruption-safe queue reset | n/a | plugin fallback (STT/TTS/wakeword fallback_*), config hot-reload | **VOXY** | VOXY has the most comprehensive local failure-path handling of the set (verified by fault injection in prior phases). OVOS's `fallback_*` plugin chaining is the only comparable pattern. No change. |

## 14. Multilingual (English / Hindi / Hinglish)

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| Lang support | whisper base.en (English-only) | cloud multi-lang | cloud multi-lang | whisper medium auto language | plugin STT multi-lang | **OpenVoiceOS / Jarvis** (multi-lang STT) | VOXY's whisper **base.en is English-only** — Hindi/Hinglish is not supported by the loaded model. Jarvis (whisper medium, `language=None` auto-detect) and OVOS (pluggable multilingual STT) demonstrate the path: a multilingual whisper model. **Decision: NOT swapped in this pass** — model change is a large runtime decision (bigger model = higher STT latency, already the biggest cost); documented as the primary multilingual limitation. |

## 15. Windows compatibility

| Component | VOXY | LiveKit | Pipecat | Jarvis | OpenVoiceOS | Winner | Reason |
|---|---|---|---|---|---|---|---|
| OS focus | Windows 10/11 native (WASAPI, clang-cl builds, LLVM toolchain) | cross-platform (SFU) | cross-platform (asyncio) | macOS/Linux focus | Linux-first (KDE/skills) | **VOXY** | VOXY is the only Windows-native implementation in the set; all references assume POSIX audio stacks. This is VOXY's structural advantage, not something to copy from the references. |

---

## Summary of selection decisions

| # | Decision | Source mechanism | Status |
|---|---|---|---|
| 1 | **STT thread tuning**: replace fixed `.min(4)` whisper thread cap with env-tunable cap; pick best default on this hardware to cut measured STT 1.9–3.1s | Jarvis `cpu_threads=cpu_count()` (faster-whisper int8) | **IMPLEMENT** |
| 2 | **Sentence chunking word-threshold emission**: flush a chunk when `max_words` accumulated during streaming, even without punctuation | LiveKit `BufferedTokenStream` min-length emission | **IMPLEMENT** |
| 3 | **Whisper warm-up inference** at startup to remove first-utterance cold-decoder latency | Jarvis noise-warm whisper | **IMPLEMENT** |
| 4 | VAD hysteresis (confidence+volume, STARTING/STOPPING) | Pipecat VADAnalyzer | **NOT adopted** — energy VAD not proven to misfire; change risks endpointing regressions without measured benefit. Documented as PARTIAL |
| 5 | Neural wake word | OVOS/Jarvis | **NOT adopted** — mission forbids large new framework; energy detector kept, limitation documented |
| 6 | Multilingual whisper model | Jarvis/OVOS | **NOT adopted** — bigger model worsens already-dominant STT latency; documented as primary limitation |
| 7 | LiveKit SentenceStreamPacer, OVOS TTS cache | LiveKit/OVOS | **NOT adopted** — measured TTS first-audio (330–613ms) already good; no evidence of benefit |
| 8 | OVOS formal FSM, plugin factories | OVOS | **NOT adopted** — large refactor, no measured latency benefit |
| 9 | LiveKit false-interruption resume / Jarvis echo-salvage | LiveKit/Jarvis | **NOT adopted** — VOXY barge-in already verified working; refinements noted as future work |