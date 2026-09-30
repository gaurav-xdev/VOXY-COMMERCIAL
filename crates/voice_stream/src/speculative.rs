use std::time::{Duration, Instant};

use crate::transcript::longest_common_prefix;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefillDecision {
    Run,
    Skip,
}

pub struct SpeculativePrefill {
    in_flight: bool,
    last_run_len: usize,
    last_run_at: Option<Instant>,
    min_growth: usize,
    min_interval: Duration,
}

impl SpeculativePrefill {
    pub fn new(min_growth: usize, min_interval: Duration) -> Self {
        Self {
            in_flight: false,
            last_run_len: 0,
            last_run_at: None,
            min_growth,
            min_interval,
        }
    }

    pub fn should_run(&mut self, transcript_len: usize, now: Instant) -> PrefillDecision {
        if self.in_flight {
            return PrefillDecision::Skip;
        }
        if transcript_len.saturating_sub(self.last_run_len) < self.min_growth {
            return PrefillDecision::Skip;
        }
        if let Some(last) = self.last_run_at {
            if now.duration_since(last) < self.min_interval {
                return PrefillDecision::Skip;
            }
        }
        PrefillDecision::Run
    }

    pub fn mark_started(&mut self, transcript_len: usize, now: Instant) {
        self.in_flight = true;
        self.last_run_len = transcript_len;
        self.last_run_at = Some(now);
    }

    pub fn mark_finished(&mut self) {
        self.in_flight = false;
    }

    pub fn is_in_flight(&self) -> bool {
        self.in_flight
    }
}

impl Default for SpeculativePrefill {
    fn default() -> Self {
        Self::new(20, Duration::from_millis(1500))
    }
}

pub fn speculation_reuse_chars(
    spec_transcript: &str,
    committed_transcript: &str,
    max_extra: usize,
    max_spec_extra: usize,
) -> Option<usize> {
    let common = longest_common_prefix(spec_transcript, committed_transcript);
    let extra = committed_transcript.chars().count() - common;
    let spec_extra = spec_transcript.chars().count() - common;
    if extra <= max_extra && spec_extra <= max_spec_extra {
        Some(extra)
    } else {
        None
    }
}

/// Conservative check that a speculative (partial) transcript is a safe basis
/// for an early LLM reply: every non-trivial word of the partial must appear,
/// in order, inside the committed (final) transcript. This is stricter than
/// [`speculation_reuse_chars`]: it requires the partial to be an early
/// truncation of the SAME utterance, not merely a close variant. A partial
/// that diverges (e.g. `"open my eyes"` vs committed `"open my browser"`) is
/// rejected so a wrong speculative reply is never played. Punctuation and case
/// are ignored.
pub fn speculative_prefix_matches(spec_transcript: &str, committed_transcript: &str) -> bool {
    let spec_words: Vec<&str> = spec_transcript.split_whitespace().collect();
    if spec_words.is_empty() {
        return false;
    }
    let mut matched = 0usize;
    for word in committed_transcript.split_whitespace() {
        if matched < spec_words.len() {
            let a = spec_words[matched].trim_matches(|c: char| !c.is_alphanumeric());
            let b = word.trim_matches(|c: char| !c.is_alphanumeric());
            if !a.is_empty() && a.eq_ignore_ascii_case(b) {
                matched += 1;
            }
        }
    }
    matched == spec_words.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn skips_when_not_enough_growth() {
        let mut spec = SpeculativePrefill::new(20, Duration::from_millis(1500));
        let t0 = now();
        assert_eq!(spec.should_run(50, t0), PrefillDecision::Run);
        spec.mark_started(50, t0);
        spec.mark_finished();
        assert_eq!(spec.should_run(55, t0), PrefillDecision::Skip);
    }

    #[test]
    fn skips_when_in_flight() {
        let mut spec = SpeculativePrefill::new(20, Duration::from_millis(1500));
        let t0 = now();
        spec.mark_started(10, t0);
        assert_eq!(
            spec.should_run(100, t0 + Duration::from_secs(10)),
            PrefillDecision::Skip
        );
    }

    #[test]
    fn skips_within_min_interval() {
        let mut spec = SpeculativePrefill::new(20, Duration::from_millis(1500));
        let t0 = now();
        spec.mark_started(10, t0);
        spec.mark_finished();
        assert_eq!(
            spec.should_run(100, t0 + Duration::from_millis(100)),
            PrefillDecision::Skip
        );
    }

    #[test]
    fn runs_after_growth_and_interval() {
        let mut spec = SpeculativePrefill::new(20, Duration::from_millis(1500));
        let t0 = now();
        spec.mark_started(10, t0);
        spec.mark_finished();
        assert_eq!(
            spec.should_run(100, t0 + Duration::from_secs(2)),
            PrefillDecision::Run
        );
    }

    #[test]
    fn reuse_for_minor_refinement() {
        let spec = "what is the capital of france";
        let committed = "what is the capital of france?";
        assert_eq!(speculation_reuse_chars(spec, committed, 30, 8), Some(1));
    }

    #[test]
    fn reuse_rejected_when_question_diverges() {
        let spec = "what is the capital of france";
        let committed = "what is the population of india";
        assert_eq!(speculation_reuse_chars(spec, committed, 30, 8), None);
    }

    #[test]
    fn reuse_rejected_when_committed_diverges_too_far() {
        let spec = "hello there how are you";
        let committed = "hello there how are you doing this fine morning my friend";
        assert_eq!(speculation_reuse_chars(spec, committed, 5, 8), None);
    }

    #[test]
    fn prefix_matches_early_truncation() {
        assert!(speculative_prefix_matches("open my", "open my browser"));
        assert!(speculative_prefix_matches(
            "what is the",
            "what is the weather"
        ));
    }

    #[test]
    fn prefix_matches_ignores_case_and_punctuation() {
        assert!(speculative_prefix_matches(
            "Open my browser.",
            "open my browser"
        ));
        assert!(speculative_prefix_matches(
            "hello, voxy",
            "Hello Voxy, how can I help?"
        ));
    }

    #[test]
    fn prefix_rejects_divergence() {
        assert!(!speculative_prefix_matches(
            "open my eyes",
            "open my browser"
        ));
        assert!(!speculative_prefix_matches(
            "what time",
            "what is the weather"
        ));
    }

    #[test]
    fn prefix_rejects_when_spec_has_extra_words() {
        assert!(!speculative_prefix_matches(
            "open my browser now",
            "open my browser"
        ));
        assert!(!speculative_prefix_matches("", "anything"));
    }

    #[test]
    fn prefix_allows_interleaved_extra_final_words() {
        assert!(speculative_prefix_matches(
            "play some",
            "please play some relaxing music"
        ));
    }
}
