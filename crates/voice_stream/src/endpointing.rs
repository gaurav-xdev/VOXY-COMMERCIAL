use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointDecision {
    Commit,
    Continue,
    Drop,
}

const FILLERS: &[&str] = &[
    "uh", "um", "hmm", "mm", "mhm", "ah", "er", "like", "so", "yeah", "well",
];

pub struct TieredEndpointing {
    pub settle: Duration,
    pub trailing_off: Duration,
    pub max_silence: Duration,
    pub max_filler_chars: usize,
    pub max_filler_words: usize,
}

impl TieredEndpointing {
    pub fn new(settle: Duration, trailing_off: Duration, max_silence: Duration) -> Self {
        Self {
            settle,
            trailing_off,
            max_silence,
            max_filler_chars: 32,
            max_filler_words: 3,
        }
    }

    pub fn decision(
        &self,
        text: &str,
        silence_since_vad_stop: Duration,
    ) -> EndpointDecision {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return EndpointDecision::Continue;
        }
        if ends_with_terminal(trimmed) {
            if silence_since_vad_stop >= self.settle
                && is_filler(trimmed, self.max_filler_chars, self.max_filler_words)
            {
                return EndpointDecision::Drop;
            }
            return EndpointDecision::Commit;
        }
        if silence_since_vad_stop >= self.max_silence {
            return EndpointDecision::Commit;
        }
        if is_filler(trimmed, self.max_filler_chars, self.max_filler_words) {
            if silence_since_vad_stop >= self.settle {
                return EndpointDecision::Drop;
            }
            return EndpointDecision::Continue;
        }
        if silence_since_vad_stop >= self.trailing_off {
            return EndpointDecision::Commit;
        }
        EndpointDecision::Continue
    }
}

impl Default for TieredEndpointing {
    fn default() -> Self {
        Self::new(
            Duration::from_millis(120),
            Duration::from_millis(700),
            Duration::from_millis(2000),
        )
    }
}

pub fn ends_with_terminal(text: &str) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|c| matches!(c, '.' | '!' | '?'))
}

pub fn is_filler(text: &str, max_chars: usize, max_words: usize) -> bool {
    let trimmed = text.trim().to_lowercase();
    if trimmed.is_empty() || trimmed.chars().count() > max_chars {
        return false;
    }
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    if words.is_empty() || words.len() > max_words {
        return false;
    }
    words.iter().all(|w| {
        let word = w.trim_matches(|c: char| !c.is_alphanumeric());
        FILLERS.contains(&word)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decide(text: &str, silence_ms: u64) -> EndpointDecision {
        TieredEndpointing::default().decision(text, Duration::from_millis(silence_ms))
    }

    #[test]
    fn terminal_punctuation_commits_immediately() {
        assert_eq!(decide("hello.", 0), EndpointDecision::Commit);
        assert_eq!(decide("wait!", 0), EndpointDecision::Commit);
        assert_eq!(decide("really?", 0), EndpointDecision::Commit);
    }

    #[test]
    fn no_punctuation_waits_for_trailing_off() {
        assert_eq!(decide("hello", 100), EndpointDecision::Continue);
        assert_eq!(decide("hello", 699), EndpointDecision::Continue);
        assert_eq!(decide("hello", 700), EndpointDecision::Commit);
    }

    #[test]
    fn max_silence_forces_commit() {
        assert_eq!(decide("hello", 3000), EndpointDecision::Commit);
    }

    #[test]
    fn empty_text_never_commits() {
        assert_eq!(decide("", 5000), EndpointDecision::Continue);
        assert_eq!(decide("   ", 5000), EndpointDecision::Continue);
    }

    #[test]
    fn pure_filler_is_dropped() {
        assert_eq!(decide("um", 800), EndpointDecision::Drop);
        assert_eq!(decide("uh yeah", 800), EndpointDecision::Drop);
        assert_eq!(decide("hmm.", 800), EndpointDecision::Drop);
    }

    #[test]
    fn real_sentence_ending_in_filler_word_is_not_dropped() {
        assert_eq!(decide("what is the capital like", 800), EndpointDecision::Commit);
    }

    #[test]
    fn non_filler_commits_via_trailing_off() {
        assert_eq!(decide("hello there", 800), EndpointDecision::Commit);
    }

    #[test]
    fn ends_with_terminal_helper() {
        assert!(ends_with_terminal("done."));
        assert!(ends_with_terminal("ok? "));
        assert!(!ends_with_terminal("not yet"));
        assert!(!ends_with_terminal(""));
    }

    #[test]
    fn filler_detection_boundaries() {
        assert!(is_filler("um", 32, 3));
        assert!(is_filler("Uh, yeah", 32, 3));
        assert!(!is_filler("what time is it", 32, 3));
        assert!(!is_filler("", 32, 3));
    }
}