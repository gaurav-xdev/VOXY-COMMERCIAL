//! In-memory LRU/TTL duplicate request prevention and audio cache.
//!
//! Avoids generating duplicate TTS audio for repeated phrases or common prompts,
//! preserving cloud quota and delivering sub-millisecond responses without
//! interfering with live streaming or barge-in interruption.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::provider::traits::{AudioData, VoiceLanguage};

const DEFAULT_TTL: Duration = Duration::from_secs(15 * 60); // 15 minutes
const MAX_CACHE_ENTRIES: usize = 512;

struct CacheEntry {
    audio: AudioData,
    created_at: Instant,
    last_accessed: Instant,
}

pub struct TTSCache {
    entries: RwLock<HashMap<u64, CacheEntry>>,
    ttl: Duration,
    max_entries: usize,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Default for TTSCache {
    fn default() -> Self {
        Self::new(DEFAULT_TTL, MAX_CACHE_ENTRIES)
    }
}

impl TTSCache {
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
            max_entries,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    fn compute_key(text: &str, language: Option<&VoiceLanguage>) -> u64 {
        let mut hasher = DefaultHasher::new();
        // Normalize whitespace and casing for caching
        text.trim().to_lowercase().hash(&mut hasher);
        if let Some(lang) = language {
            match lang {
                VoiceLanguage::English => "en".hash(&mut hasher),
                VoiceLanguage::Hindi => "hi".hash(&mut hasher),
                VoiceLanguage::Hinglish => "hinglish".hash(&mut hasher),
                VoiceLanguage::Auto | VoiceLanguage::AutoDetect => "auto".hash(&mut hasher),
                VoiceLanguage::Other(s) | VoiceLanguage::Custom(s) => s.hash(&mut hasher),
            }
        }
        hasher.finish()
    }

    /// Retrieve cached audio if present and not expired.
    pub fn get(&self, text: &str, language: Option<&VoiceLanguage>) -> Option<AudioData> {
        let key = Self::compute_key(text, language);
        let mut lock = self.entries.write();

        if let Some(entry) = lock.get_mut(&key) {
            if entry.created_at.elapsed() < self.ttl {
                entry.last_accessed = Instant::now();
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(entry.audio.clone());
            } else {
                // Expired
                lock.remove(&key);
            }
        }

        self.misses.fetch_add(1, Ordering::Relaxed);
        None
    }

    /// Store synthesized audio in the cache.
    pub fn insert(&self, text: &str, language: Option<&VoiceLanguage>, audio: AudioData) {
        // Do not cache very long paragraphs (larger than 400 chars) to preserve memory
        if text.chars().count() > 400 {
            return;
        }

        let key = Self::compute_key(text, language);
        let mut lock = self.entries.write();

        // Evict expired entries or oldest if capacity reached
        if lock.len() >= self.max_entries {
            let ttl = self.ttl;
            lock.retain(|_, v| v.created_at.elapsed() < ttl);

            if lock.len() >= self.max_entries {
                // Evict oldest by last_accessed
                if let Some((&oldest_key, _)) = lock.iter().min_by_key(|(_, v)| v.last_accessed) {
                    lock.remove(&oldest_key);
                }
            }
        }

        let now = Instant::now();
        lock.insert(
            key,
            CacheEntry {
                audio,
                created_at: now,
                last_accessed: now,
            },
        );
    }

    pub fn clear(&self) {
        self.entries.write().clear();
    }

    pub fn stats(&self) -> (u64, u64, usize) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
            self.entries.read().len(),
        )
    }
}
