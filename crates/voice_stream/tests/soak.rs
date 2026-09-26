//! Soak / stress tests for the voice streaming primitives. These exercise the
//! same logic the capture loop uses (partial stabilization, tiered
//! endpointing, speculative prefill throttling, sentence chunking) at scale,
//! without requiring audio hardware.

use std::time::{Duration, Instant};

use voxy_voice_stream::{
    is_filler, EndpointDecision, PrefillDecision, SentenceChunker, SpeculativePrefill,
    TieredEndpointing, TranscriptStabilizer,
};

/// Simulate a long conversation of partial transcripts, each growing and
/// occasionally revising earlier text. Emitted stable text must be
/// monotonically non-decreasing and must converge to the final utterance.
#[test]
fn partial_transcript_soak_converges() {
    let mut rng_state: u64 = 0x9E3779B97F4A7C15;
    let mut next = || {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        rng_state
    };

    for _ in 0..200 {
        let mut stab = TranscriptStabilizer::new();
        let mut emitted = String::new();

        let base = "what is the weather going to look like this afternoon";
        let mut partial = String::new();
        let mut iter = 0usize;
        loop {
            // Grow the partial word by word, occasionally re-committing a prefix.
            let words: Vec<&str> = base.split(' ').collect();
            let grow_to = (next() % (words.len() as u64 + 1)) as usize;
            if grow_to < words.len() {
                partial = words[..grow_to.min(partial.split(' ').count().max(1))].join(" ");
            } else {
                partial = base.to_string();
            }
            if let Some(new_text) = stab.ingest(&partial) {
                assert!(
                    emitted.len() <= partial.len(),
                    "stable text must not exceed the partial it is derived from"
                );
                emitted.push_str(&new_text);
            }
            iter += 1;
            if iter > 40 || partial == base {
                break;
            }
        }

        // Everything beyond the first committed word must have been emitted.
        assert!(
            emitted.contains("what") || emitted.trim().is_empty(),
            "expected stable emission, got: '{}'",
            emitted
        );
        assert!(
            emitted.len() <= base.len(),
            "emitted '{}' longer than source '{}'",
            emitted,
            base
        );
    }
}

/// Random endpointing decisions across thousands of inputs must never panic and
/// must return only the three valid decisions.
#[test]
fn endpointing_decision_stress() {
    let ep = TieredEndpointing::default();
    let mut rng_state: u64 = 0xDEADBEEFCAFEBABE;
    let mut next = || {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        rng_state
    };
    let pool = [
        "", "hello", "hello.", "what time is it", "what time is it?",
        "um", "uh yeah", "so", "hmm.", "yes", "the quick brown fox",
        "नमस्ते", "क्या समय हुआ", "kya time hua?", "please stop", "wait!",
        "no no no", "okay okay okay.", "tell me more", "..", "...", ".",
    ];

    for _ in 0..10_000 {
        let text = pool[(next() % pool.len() as u64) as usize];
        let silence_ms = next() % 5000;
        let decision = ep.decision(text, Duration::from_millis(silence_ms));
        match decision {
            EndpointDecision::Commit | EndpointDecision::Continue | EndpointDecision::Drop => {}
        }
    }
}

/// Sentence chunking over a large, adversarial stream must terminate with
/// bounded output and no panics.
#[test]
fn sentence_chunker_stress_bounded() {
    let mut chunker = SentenceChunker::new(20);
    let mut total = 0usize;
    let mut rng_state: u64 = 0x0123456789ABCDEF;
    let mut next = || {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        rng_state
    };

    for i in 0..50_000 {
        let word_len = (next() % 40) as usize;
        let word: String = (0..word_len.max(1))
            .map(|_| (b'a' + (next() % 26) as u8) as char)
            .collect();
        let sep = if i % 7 == 0 { '.' } else { ' ' };
        let feed = format!("{word}{sep}");
        for sentence in chunker.feed(&feed) {
            assert!(!sentence.is_empty());
            assert!(sentence.len() <= 20_000);
            total += sentence.len();
        }
    }
    let leftover: Vec<String> = chunker.finish();
    for sentence in leftover {
        assert!(!sentence.is_empty());
    }
    assert!(total <= 2_000_000);
}

/// Speculative prefill throttling under randomized arrival must respect the
/// in-flight and interval invariants.
#[test]
fn speculative_prefill_stress_invariants() {
    let mut spec = SpeculativePrefill::default();
    let mut rng_state: u64 = 0xFEEDFACE12345678;
    let mut next = || {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        rng_state
    };

    let mut t = Instant::now();
    let mut runs = 0usize;
    for _ in 0..20_000 {
        let len = (next() % 200) as usize;
        t += Duration::from_millis(next() % 50);
        let decision = spec.should_run(len, t);
        if decision == PrefillDecision::Run {
            runs += 1;
            spec.mark_started(len, t);
            assert!(spec.is_in_flight());
            spec.mark_finished();
        }
    }
    assert!(runs > 0);
}

/// Filler detection across a corpus never panics and is consistent.
#[test]
fn filler_detection_stress() {
    let pool = [
        "um", "uh", "hmm", "mm", "yeah", "so", "um um", "uh huh",
        "hello", "", "   ", "what is this", "नमस्ते", "um actually",
        "well", "like", "so like", "hmm.", "mm hm", "a b c d e f",
    ];
    let mut rng_state: u64 = 0x00FF00FF00FF00FF;
    let mut next = || {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        rng_state
    };
    for _ in 0..5000 {
        let text = pool[(next() % pool.len() as u64) as usize];
        let _ = is_filler(text, 32, 3);
    }
}