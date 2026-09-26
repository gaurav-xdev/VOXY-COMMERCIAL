# STT Contention Map — Phase 26

## Complete Runtime Path

```
MIC (WASAPI)
  │
  ▼
AudioInputStream::read()  ──► 48kHz stereo frames (frame_size=960)
  │
  ▼
normalize_to_engine()  ──► 16kHz mono f32 buffer (linear_resample + downmix)
  │
  ▼
VAD (TieredEndpointing)  ──► endpoint decision (Continue/Commit/Drop)
  │
  ├─► Partial STT Worker (spawned tokio task, 16-ch mpsc channel)
  │     │
  │     ▼
  │  utt_rx.recv()  ──► AudioChunk (is_final=false)
  │     │
  │     ▼
  │  partial_paused.load(Relaxed) ──► if true: continue (skip)
  │     │
  │     ▼
  │  partial_stt_engine.read().await  ──► Arc<WhisperSttEngine> (SHARED)
  │     │
  │     ▼
  │  stt.transcribe_partial(&chunk).await
  │     │
  │     ▼
  │  decode_partial(normalized_audio)
  │     │
  │     ▼
  │  tokio::task::spawn_blocking {
  │       let guard = context.lock();          ──► parking_lot::Mutex<Option<WhisperContext>>
  │       let ctx = guard.as_ref()?;
  │       let mut state = ctx.create_state();  ──► NEW WhisperState per call
  │       state.full(params, audio);           ──► whisper.cpp full decode
  │       // n_threads = whisper_threads() (default 8)
  │       // params: greedy, no_timestamps=true, suppress_blank/nst
  │  }
  │     │
  │     ▼
  │  TranscriptStabilizer.ingest()  ──► stable partial text
  │     │
  │     ▼
  │  latest_partial.lock().unwrap() = stable  ──► Arc<Mutex<String>>
  │     │
  │     ▼
  │  partial_tx.send(PartialTranscript)  ──► to UI/handler
  │
  └─► Commit (EndpointDecision::Commit)
        │
        ▼
    partial_paused.store(true, Relaxed)  ──► pauses partial worker
        │
        ▼
    Speculative LLM overlap (if partial ≥10 chars & ≥2 words)
        │
        ▼
    Final STT:
        │
        ▼
    stt_engine.read().await  ──► Arc<WhisperSttEngine> (SAME engine!)
        │
        ▼
    stt.transcribe(&final_chunk).await
        │
        ▼
    transcribe_audio_async(accumulated_audio)
        │
        ▼
    tokio::task::spawn_blocking {
          let mut guard = cached_state.lock();   ──► parking_lot::Mutex<Option<WhisperState>>
          let state = guard.as_mut()?;           ──► PERSISTED WhisperState (session carry)
          state.full(params, audio);             ──► whisper.cpp full decode
          // n_threads = whisper_threads() (default 8)
          // SAME params as partial
    }
        │
        ▼
    apply_session_carry(last_text, new_text)
        │
        ▼
    Transcript reconciliation (speculative_prefix_matches)
        │
        ▼
    LLM → SentenceChunker → TTS
```

---

## Shared Resources & Concurrency Points

| Resource | Type | Used By | Lock | Notes |
|----------|------|---------|------|-------|
| `WhisperSttEngine` | `Arc<RwLock<Option<...>>>` | Partial worker + Final STT | tokio `RwLock` (read) | Single instance cloned via `Arc` |
| `context` | `Arc<Mutex<Option<WhisperContext>>>` | Partial decoder (`decode_partial`) | `parking_lot::Mutex` | Locked for `ctx.create_state()` |
| `cached_state` | `Arc<Mutex<Option<WhisperState>>>` | Final decoder (`transcribe_audio_async`) | `parking_lot::Mutex` | Locked for entire `state.full()` |
| `buffer` | `Arc<Mutex<Vec<f32>>>` | Audio accumulation | `parking_lot::Mutex` | Final STT drains it (`take()`) |
| `last_text` | `Arc<Mutex<String>>` | Session carry | `parking_lot::Mutex` | Read/write during final STT |
| `latest_partial` | `Arc<Mutex<String>>` | Partial worker + Commit snapshot | `std::sync::Mutex` | Cleared on `is_final` chunk |
| `partial_paused` | `Arc<AtomicBool>` | Commit → Partial worker | Atomic (Relaxed) | Set true during final STT |
| `utt_tx/utt_rx` | `mpsc::channel(16)` | Capture loop → Partial worker | Lock-free | 16-chunk buffer |
| `whisper_threads()` | `i32` (env) | Both decoders | N/A | Controls whisper.cpp internal thread pool |

---

## Thread Boundaries

```
Main tokio runtime (multi-threaded, default worker threads = cores)
│
├─ Capture loop (tokio task) ──► audio_input.read() → VAD → speech_buffer → utt_tx.send()
│
├─ Partial worker (tokio task) ──► utt_rx.recv() → spawn_blocking → Whisper decode
│
├─ Final STT (inline in capture loop) ──► spawn_blocking → Whisper decode
│
├─ Speculative LLM (tokio task, if triggered) ──► async LLM streaming
│
├─ TTS consumer (tokio task) ──► sentence_rx.recv() → TTS synthesize → audio_output.write()
│
└─ tokio blocking pool (spawn_blocking) ──► Whisper decode runs here
      │
      ├─ Partial decoder: 1 blocking task per ~500ms
      └─ Final decoder: 1 blocking task per utterance
```

---

## Whisper Context/State Ownership

```
WhisperSttEngine (single instance, Arc-cloned)
│
├─ context: Arc<Mutex<Option<WhisperContext>>>  ──► SHARED
│     │
│     └─ WhisperContext (whisper_rs) ──► loads model, holds mel buffers, vocab, etc.
│           │
│           └─ create_state() → NEW WhisperState per call (allocates KV caches, compute buffers)
│
├─ cached_state: Arc<Mutex<Option<WhisperState>>>  ──► SHARED
│     │
│     └─ WhisperState (persisted across utterances for session carry)
│           │
│           └─ full() mutates internal state (KV cache, decode position)
│
└─ buffer, last_text: other shared state
```

---

## Concurrency Scenarios

| Scenario | Partial Active? | Final Active? | Overlap? | Risk |
|----------|----------------|---------------|----------|------|
| Normal utterance | Yes (every 500ms) | No | No | Baseline |
| Commit starts | Yes (in-flight) | Starting | ~500ms | **partial_paused set AFTER in-flight decode starts** |
| Commit, paused | No (paused) | Yes | None | **Phase 25 fix** |
| Rapid utterances | Yes | Yes (sequential) | None | Queue in utt_rx |
| Barge-in during STT | Partial may be mid-decode | Final may be running | Possible | Abort + recovery |

---

## Potential Hang Sources (Hypotheses)

1. **whisper.cpp mel buffer contention**: `ctx.create_state()` allocates per-state buffers, but `whisper_pcm_to_mel()` uses `ctx->mel` (context-shared). Concurrent `full()` on different states sharing one `Context` → data race on mel buffer.

2. **whisper.cpp thread pool saturation**: `whisper_threads()=8` + tokio blocking pool + multiple concurrent `spawn_blocking` → thread starvation, priority inversion.

3. **parking_lot Mutex contention**: `context.lock()` (partial) vs `cached_state.lock()` (final) are DIFFERENT mutexes, but whisper.cpp internal locks may conflict.

4. **tokio blocking pool exhaustion**: Long-running Whisper decode tasks (2-3s each) occupy blocking threads. Default pool = 512. Unlikely but possible under load.

5. **WhisperState reuse corruption**: `cached_state` persists across utterances. If `state.full()` called while another `full()` on different state shares Context → undefined behavior.

6. **Memory pressure**: Repeated `create_state()` allocations (KV caches: ~130MB per state) + blocked final decode → OOM/thrashing.

7. **Audio buffer growth**: `speech_buffer` unbounded if endpointing fails → huge final decode.

8. **Cancellation leaving decoder invalid**: `task.abort()` during `spawn_blocking` → WhisperState left in inconsistent state.

---

## Verification Checklist (Step 1 Complete)

- [x] Every Whisper engine instance identified: **1 shared `WhisperSttEngine`**
- [x] Every Whisper context identified: **1 `context`, 1 `cached_state`**
- [x] Every decoder invocation identified: **`decode_partial` (new state), `transcribe_audio_async` (persisted state)**
- [x] Every shared buffer identified: **context mel, cached_state KV, audio buffer, latest_partial**
- [x] Every mutex identified: **4 parking_lot, 1 std::sync, 1 AtomicBool, 1 tokio RwLock**
- [x] Every async task identified: **capture, partial worker, speculative LLM, TTS consumer, blocking pool**
- [x] Every thread boundary identified: **tokio runtime → spawn_blocking → whisper.cpp threads**
- [x] Partial/final overlap points: **commit start (in-flight partial), pause flag race**
- [x] Context reuse concurrency: **context shared, states separate but mel buffer shared**

---

## Next: Step 2 — Reproduce the Hang

Build deterministic stress test with configurable `VOXY_WHISPER_THREADS`.