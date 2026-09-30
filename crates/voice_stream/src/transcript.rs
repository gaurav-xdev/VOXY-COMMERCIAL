pub fn longest_common_prefix(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

pub fn join_carry(old: &str, new: &str) -> (String, String) {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let mut best = 0;
    let max = old_chars.len().min(new_chars.len());
    for i in 0..=max {
        if old_chars[old_chars.len() - i..] == new_chars[..i] {
            best = i;
        }
    }
    let carry: String = new_chars[..best].iter().collect();
    let rest: String = new_chars[best..].iter().collect();
    (carry, rest)
}

#[derive(Debug, Clone)]
pub struct TranscriptStabilizer {
    committed: String,
}

impl TranscriptStabilizer {
    pub fn new() -> Self {
        Self {
            committed: String::new(),
        }
    }

    pub fn committed(&self) -> &str {
        &self.committed
    }

    pub fn committed_len(&self, transcript: &str) -> usize {
        longest_common_prefix(&self.committed, transcript)
    }

    pub fn new_speech<'a>(&self, transcript: &'a str) -> &'a str {
        let committed_chars = self.committed_len(transcript);
        let byte = char_byte_offset(transcript, committed_chars);
        &transcript[byte..]
    }

    pub fn commit(&mut self, transcript: &str) {
        self.committed = transcript.to_string();
    }

    /// Ingest a partial transcript and return the stable text that has not yet
    /// been emitted. Returns `None` when the partial adds nothing new or is
    /// empty. The committed marker advances to the partial so that ASR
    /// revisions of already-committed text are not re-emitted.
    pub fn ingest(&mut self, partial: &str) -> Option<String> {
        if partial.trim().is_empty() {
            return None;
        }
        let stable = self.new_speech(partial);
        self.commit(partial);
        if stable.trim().is_empty() {
            return None;
        }
        Some(stable.to_string())
    }

    pub fn rotate(&mut self, first_partial: &str) -> (String, String) {
        let (carry, rest) = join_carry(&self.committed, first_partial);
        self.committed = carry.clone();
        (carry, rest)
    }
}

impl Default for TranscriptStabilizer {
    fn default() -> Self {
        Self::new()
    }
}

fn char_byte_offset(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longest_common_prefix_counts_matching_chars() {
        assert_eq!(longest_common_prefix("hello world", "hello brave"), 6);
        assert_eq!(longest_common_prefix("abc", "abd"), 2);
        assert_eq!(longest_common_prefix("abc", "xyz"), 0);
        assert_eq!(longest_common_prefix("", "abc"), 0);
    }

    #[test]
    fn new_speech_is_text_beyond_committed_marker() {
        let mut stab = TranscriptStabilizer::new();
        stab.commit("hello world");
        assert_eq!(stab.new_speech("hello world foo"), " foo");
        assert_eq!(stab.new_speech("hello world"), "");
    }

    #[test]
    fn asr_revision_does_not_regress_committed_text() {
        let mut stab = TranscriptStabilizer::new();
        stab.commit("hello world");
        assert_eq!(stab.new_speech("hello worl"), "");
    }

    #[test]
    fn commit_updates_marker() {
        let mut stab = TranscriptStabilizer::new();
        stab.commit("hello world");
        stab.commit("hello world foo bar");
        assert_eq!(stab.committed(), "hello world foo bar");
        assert_eq!(stab.new_speech("hello world foo bar baz"), " baz");
    }

    #[test]
    fn join_carry_finds_longest_overlap_between_sessions() {
        let (carry, rest) = join_carry("hello world foo", "world foo bar");
        assert_eq!(carry, "world foo");
        assert_eq!(rest, " bar");
    }

    #[test]
    fn join_carry_with_no_overlap_returns_empty_carry() {
        let (carry, rest) = join_carry("hello world", "totally different");
        assert!(carry.is_empty());
        assert_eq!(rest, "totally different");
    }

    #[test]
    fn rotate_sets_committed_to_carry() {
        let mut stab = TranscriptStabilizer::new();
        stab.commit("hello world foo");
        let (carry, rest) = stab.rotate("world foo bar");
        assert_eq!(carry, "world foo");
        assert_eq!(rest, " bar");
        assert_eq!(stab.committed(), "world foo");
    }

    #[test]
    fn unicode_chars_count_individually() {
        assert_eq!(longest_common_prefix("नमस्ते दुनिया", "नमस्ते भारत"), 7);
    }

    #[test]
    fn ingest_emits_new_text_only_once() {
        let mut stab = TranscriptStabilizer::new();
        assert_eq!(stab.ingest("hello"), Some("hello".to_string()));
        assert_eq!(stab.ingest("hello world"), Some(" world".to_string()));
        assert_eq!(stab.ingest("hello world"), None);
        assert_eq!(stab.ingest("hello worl"), None);
    }

    #[test]
    fn ingest_rejects_empty_and_whitespace() {
        let mut stab = TranscriptStabilizer::new();
        assert_eq!(stab.ingest(""), None);
        assert_eq!(stab.ingest("   "), None);
    }

    #[test]
    fn ingest_revision_beyond_committed_emits_replacement() {
        let mut stab = TranscriptStabilizer::new();
        assert_eq!(
            stab.ingest("hello world foo"),
            Some("hello world foo".to_string())
        );
        assert_eq!(stab.ingest("hello world bar"), Some("bar".to_string()));
    }
}
