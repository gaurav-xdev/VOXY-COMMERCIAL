const SENTENCE_END: [char; 4] = ['.', '!', '?', '\n'];
const TERMINAL_PUNCT: [char; 3] = ['.', '!', '?'];
const CLOSING: [char; 6] = ['"', '\'', ')', ']', '\u{201d}', '\u{2019}'];

pub struct SentenceChunker {
    max_words: usize,
    pending: String,
}

impl SentenceChunker {
    pub fn new(max_words: usize) -> Self {
        Self {
            max_words: max_words.max(1),
            pending: String::new(),
        }
    }

    pub fn feed(&mut self, text: &str) -> Vec<String> {
        self.pending.push_str(text);
        let mut out = Vec::new();
        while let Some(end) = find_sentence_end(&self.pending) {
            let sentence = self.pending[..end].to_string();
            self.pending = self.pending[end..].to_string();
            self.emit(&sentence, &mut out);
        }
        self.flush_word_overflow(&mut out);
        out
    }

    fn flush_word_overflow(&mut self, out: &mut Vec<String>) {
        let words: Vec<&str> = self.pending.split_whitespace().collect();
        if words.len() < self.max_words {
            return;
        }
        let n_groups = words.len() / self.max_words;
        for g in 0..n_groups {
            let group: Vec<&str> = words[g * self.max_words..(g + 1) * self.max_words].to_vec();
            out.push(group.join(" "));
        }
        let tail_start = n_groups * self.max_words;
        self.pending = if tail_start < words.len() {
            words[tail_start..].join(" ")
        } else {
            String::new()
        };
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        let rest = self.pending.trim().to_string();
        self.pending.clear();
        if rest.is_empty() {
            return out;
        }
        if !ends_with_terminal(&rest) {
            let mut completed = rest;
            completed.push('.');
            self.emit(&completed, &mut out);
        } else {
            self.emit(&rest, &mut out);
        }
        out
    }

    fn emit(&self, sentence: &str, out: &mut Vec<String>) {
        let trimmed = sentence.trim();
        if trimmed.is_empty() {
            return;
        }
        if count_words(trimmed) <= self.max_words {
            out.push(trimmed.to_string());
            return;
        }
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let mut group: Vec<&str> = Vec::with_capacity(self.max_words);
        for word in words {
            group.push(word);
            if group.len() == self.max_words {
                out.push(group.join(" "));
                group.clear();
            }
        }
        if !group.is_empty() {
            out.push(group.join(" "));
        }
    }
}

pub fn ends_with_terminal(text: &str) -> bool {
    text.trim_end()
        .chars()
        .last()
        .is_some_and(|c| TERMINAL_PUNCT.contains(&c))
}

fn find_sentence_end(text: &str) -> Option<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (_, c) = chars[i];
        if c == '\n' {
            return Some(chars[i].0 + c.len_utf8());
        }
        if !SENTENCE_END.contains(&c) {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < chars.len() && SENTENCE_END.contains(&chars[j].1) {
            j += 1;
        }
        let mut k = j;
        while k < chars.len() && CLOSING.contains(&chars[k].1) {
            k += 1;
        }
        if k >= chars.len() {
            return Some(text.len());
        }
        let next = chars[k].1;
        if next.is_whitespace() || next == '[' {
            return Some(chars[k].0);
        }
        i = j;
    }
    None
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

impl Default for SentenceChunker {
    fn default() -> Self {
        Self::new(20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_terminal_punctuation() {
        let mut chunker = SentenceChunker::new(20);
        let chunks = chunker.feed("Hello world. This is a test!");
        assert_eq!(chunks, vec!["Hello world.", "This is a test!"]);
    }

    #[test]
    fn does_not_split_on_decimal_point() {
        let mut chunker = SentenceChunker::new(20);
        let chunks = chunker.feed("Pi is 3.14. Ok?");
        assert_eq!(chunks, vec!["Pi is 3.14.", "Ok?"]);
    }

    #[test]
    fn splits_after_closing_quote() {
        let mut chunker = SentenceChunker::new(20);
        let chunks = chunker.feed("He said \"Hi.\" Next.");
        assert_eq!(chunks, vec!["He said \"Hi.\"", "Next."]);
    }

    #[test]
    fn splits_on_newline() {
        let mut chunker = SentenceChunker::new(20);
        let chunks = chunker.feed("Line one\nLine two");
        assert_eq!(chunks, vec!["Line one"]);
        assert_eq!(chunker.finish(), vec!["Line two."]);
    }

    #[test]
    fn emits_at_end_of_buffer_on_punctuation() {
        let mut chunker = SentenceChunker::new(20);
        assert!(chunker.feed("Hello").is_empty());
        assert_eq!(chunker.feed(" world."), vec!["Hello world."]);
    }

    #[test]
    fn finish_guarantees_terminal_punctuation() {
        let mut chunker = SentenceChunker::new(20);
        chunker.feed("hello there");
        assert_eq!(chunker.finish(), vec!["hello there."]);
    }

    #[test]
    fn complete_sentence_emitted_during_feed() {
        let mut chunker = SentenceChunker::new(20);
        assert_eq!(chunker.feed("already done!"), vec!["already done!"]);
        assert!(chunker.finish().is_empty());
    }

    #[test]
    fn finish_guarantees_terminal_punctuation_on_unfinished_text() {
        let mut chunker = SentenceChunker::new(20);
        chunker.feed("almost done");
        assert_eq!(chunker.finish(), vec!["almost done."]);
    }

    #[test]
    fn finish_returns_nothing_for_empty_buffer() {
        let mut chunker = SentenceChunker::new(20);
        assert!(chunker.finish().is_empty());
    }

    #[test]
    fn word_count_fallback_splits_long_sentences() {
        let mut chunker = SentenceChunker::new(3);
        let fed = chunker.feed("this is a very long run-on sentence without punctuation at all");
        let mut chunks = fed;
        chunks.extend(chunker.finish());
        assert_eq!(chunks.len(), 4);
        assert!(chunks.iter().all(|c| count_words(c) <= 4));
        assert!(chunks.last().unwrap().ends_with('.'));
    }

    #[test]
    fn long_runs_flush_during_feed_without_terminal() {
        let mut chunker = SentenceChunker::new(5);
        let chunks = chunker.feed("the quick brown fox jumps over the lazy dog");
        assert_eq!(chunks, vec!["the quick brown fox jumps"]);
        assert!(
            !chunks[0].ends_with('.'),
            "partial chunk must not get terminal"
        );
        assert_eq!(chunker.finish(), vec!["over the lazy dog."]);
    }

    #[test]
    fn word_flush_keeps_remainder_then_sentence_end() {
        let mut chunker = SentenceChunker::new(4);
        let chunks = chunker.feed("one two three four five six seven. next");
        assert_eq!(chunks, vec!["one two three four", "five six seven."]);
        assert_eq!(chunker.finish(), vec!["next."]);
    }

    #[test]
    fn word_flush_never_fires_below_threshold() {
        let mut chunker = SentenceChunker::new(20);
        let chunks = chunker.feed("a few words");
        assert!(chunks.is_empty(), "below threshold must not flush");
        assert_eq!(chunker.finish(), vec!["a few words."]);
    }

    #[test]
    fn exact_threshold_flushes_entire_pending() {
        let mut chunker = SentenceChunker::new(3);
        let chunks = chunker.feed("alpha beta gamma");
        assert_eq!(chunks, vec!["alpha beta gamma"]);
        assert!(chunker.finish().is_empty());
    }

    #[test]
    fn trailing_whitespace_is_stripped_from_chunks() {
        let mut chunker = SentenceChunker::new(20);
        let chunks = chunker.feed("   padded.   \nnext");
        assert_eq!(chunks, vec!["padded."]);
    }

    #[test]
    fn empty_text_ignored() {
        let mut chunker = SentenceChunker::new(20);
        assert!(chunker.feed("").is_empty());
        assert!(chunker.feed("   ").is_empty());
    }
}
