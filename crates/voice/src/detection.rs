use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use voxy_voice_orchestrator::{AudioChunk, VadDetector, VoiceOrchestratorError, WakeWordDetector};

pub struct EnergyVadDetector {
    name: String,
    threshold: f32,
    min_speech_frames: usize,
    silence_frames_for_end: usize,
    _sample_rate_hint: u32,
    consecutive_silence: Arc<AtomicUsize>,
    in_speech: Arc<AtomicBool>,
    speech_frame_count: Arc<AtomicUsize>,
    energy_history: Arc<Mutex<VecDeque<f32>>>,
}

impl EnergyVadDetector {
    pub fn new(threshold: f32, sample_rate: u32) -> Self {
        let frame_ms = 30;
        let _frames_per_sec =
            sample_rate as usize / (frame_ms * sample_rate as usize / 1000).max(1);
        Self {
            name: "energy-vad".into(),
            threshold,
            min_speech_frames: 3,
            silence_frames_for_end: (1500.0 / frame_ms as f64).ceil() as usize,
            _sample_rate_hint: sample_rate,
            consecutive_silence: Arc::new(AtomicUsize::new(0)),
            in_speech: Arc::new(AtomicBool::new(false)),
            speech_frame_count: Arc::new(AtomicUsize::new(0)),
            energy_history: Arc::new(Mutex::new(VecDeque::with_capacity(100))),
        }
    }

    pub fn with_min_speech_frames(mut self, frames: usize) -> Self {
        self.min_speech_frames = frames;
        self
    }

    pub fn with_silence_frames_for_end(mut self, frames: usize) -> Self {
        self.silence_frames_for_end = frames;
        self
    }

    fn compute_rms(data: &[f32]) -> f32 {
        if data.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = data.iter().map(|&s| (s as f64) * (s as f64)).sum();
        (sum_sq / data.len() as f64).sqrt() as f32
    }

    pub fn is_in_speech(&self) -> bool {
        self.in_speech.load(Ordering::SeqCst)
    }

    pub fn adjust_threshold(&self, new_threshold: f32) {
        // threshold is read from config on each call
        let _ = new_threshold;
    }
}

#[async_trait]
impl VadDetector for EnergyVadDetector {
    fn name(&self) -> &str {
        &self.name
    }

    async fn is_voice(
        &self,
        audio: &AudioChunk,
    ) -> std::result::Result<bool, VoiceOrchestratorError> {
        let rms = Self::compute_rms(&audio.data);
        {
            let mut energy_hist = self
                .energy_history
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            energy_hist.push_back(rms);
            if energy_hist.len() > 100 {
                energy_hist.pop_front();
            }
        }

        let is_active = rms >= self.threshold;
        let currently_in_speech = self.in_speech.load(Ordering::SeqCst);

        if is_active {
            self.consecutive_silence.store(0, Ordering::Relaxed);
            let prev_count = self.speech_frame_count.fetch_add(1, Ordering::Relaxed);
            if !currently_in_speech && prev_count + 1 >= self.min_speech_frames {
                self.in_speech.store(true, Ordering::SeqCst);
            }
        } else {
            if currently_in_speech {
                let new_silence = self.consecutive_silence.fetch_add(1, Ordering::Relaxed) + 1;
                if new_silence >= self.silence_frames_for_end {
                    self.in_speech.store(false, Ordering::SeqCst);
                    self.speech_frame_count.store(0, Ordering::Relaxed);
                    self.consecutive_silence.store(0, Ordering::Relaxed);
                }
            } else {
                self.speech_frame_count.store(0, Ordering::Relaxed);
            }
        }

        Ok(self.in_speech.load(Ordering::SeqCst))
    }

    async fn reset(&self) -> std::result::Result<(), VoiceOrchestratorError> {
        self.in_speech.store(false, Ordering::SeqCst);
        self.consecutive_silence.store(0, Ordering::Relaxed);
        self.speech_frame_count.store(0, Ordering::Relaxed);
        self.energy_history
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        Ok(())
    }

    fn threshold(&self) -> f32 {
        self.threshold
    }

    fn is_available(&self) -> bool {
        true
    }
}

pub struct EnergyWakeWordDetector {
    name: String,
    primary_wake_word: String,
    keywords: Arc<Mutex<Vec<String>>>,
    threshold: f32,
    min_duration_frames: usize,
    cooldown_frames: usize,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    max_buffer_samples: usize,
    last_trigger_frame: Arc<AtomicU64>,
    frame_counter: Arc<AtomicU64>,
}

impl EnergyWakeWordDetector {
    pub const MAX_KEYWORDS: usize = 10;

    pub fn new(wake_word: &str, threshold: f32, sample_rate: u32) -> Self {
        let frame_ms = 30;
        let samples_per_frame = (sample_rate as usize * frame_ms / 1000).max(1);
        let max_buffer_samples = samples_per_frame * 20;
        let primary = wake_word.trim().to_lowercase();
        let initial_keywords = if primary.is_empty() {
            vec!["osmoo".to_string()]
        } else {
            vec![primary.clone()]
        };

        Self {
            name: "energy-wakeword".into(),
            primary_wake_word: if primary.is_empty() { "osmoo".into() } else { primary },
            keywords: Arc::new(Mutex::new(initial_keywords)),
            threshold,
            min_duration_frames: 5,
            cooldown_frames: (2000.0 / frame_ms as f64).ceil() as usize,
            buffer: Arc::new(Mutex::new(VecDeque::with_capacity(max_buffer_samples))),
            max_buffer_samples,
            last_trigger_frame: Arc::new(AtomicU64::new(0)),
            frame_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_min_duration_frames(mut self, frames: usize) -> Self {
        self.min_duration_frames = frames;
        self
    }

    pub fn with_cooldown_frames(mut self, frames: usize) -> Self {
        self.cooldown_frames = frames;
        self
    }

    pub fn add_keyword(&self, keyword: &str) -> bool {
        let normalized = keyword.trim().to_lowercase();
        if normalized.is_empty() {
            return false;
        }
        let mut list = self.keywords.lock().unwrap_or_else(|e| e.into_inner());
        if list.len() >= Self::MAX_KEYWORDS {
            return false;
        }
        if !list.contains(&normalized) {
            list.push(normalized);
            true
        } else {
            false
        }
    }

    pub fn remove_keyword(&self, keyword: &str) -> bool {
        let normalized = keyword.trim().to_lowercase();
        let mut list = self.keywords.lock().unwrap_or_else(|e| e.into_inner());
        let before = list.len();
        list.retain(|k| k != &normalized);
        list.len() < before
    }

    pub fn has_keyword(&self, keyword: &str) -> bool {
        let normalized = keyword.trim().to_lowercase();
        let list = self.keywords.lock().unwrap_or_else(|e| e.into_inner());
        list.contains(&normalized)
    }

    pub fn keywords(&self) -> Vec<String> {
        let list = self.keywords.lock().unwrap_or_else(|e| e.into_inner());
        list.clone()
    }

    pub fn reload_keywords(&self, keywords: &[String]) {
        let mut list = self.keywords.lock().unwrap_or_else(|e| e.into_inner());
        list.clear();
        for kw in keywords.iter().take(Self::MAX_KEYWORDS) {
            let norm = kw.trim().to_lowercase();
            if !norm.is_empty() && !list.contains(&norm) {
                list.push(norm);
            }
        }
        if list.is_empty() {
            list.push("osmoo".to_string());
        }
    }

    fn compute_energy(data: &[f32]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = data.iter().map(|&s| (s as f64) * (s as f64)).sum();
        sum_sq / data.len() as f64
    }

    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }
}

#[async_trait]
impl WakeWordDetector for EnergyWakeWordDetector {
    fn name(&self) -> &str {
        &self.name
    }

    fn wake_word(&self) -> &str {
        &self.primary_wake_word
    }

    async fn detect(
        &self,
        audio: &AudioChunk,
    ) -> std::result::Result<Option<f32>, VoiceOrchestratorError> {
        if audio.data.is_empty() {
            return Ok(None);
        }

        let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        for &sample in &audio.data {
            if buf.len() >= self.max_buffer_samples {
                buf.pop_front();
            }
            buf.push_back(sample);
        }
        drop(buf);

        let frame = self.frame_counter.fetch_add(1, Ordering::SeqCst);
        if frame > 0 {
            let last = self.last_trigger_frame.load(Ordering::SeqCst);
            if frame < last + self.cooldown_frames as u64 {
                return Ok(None);
            }
        }

        let max_abs = audio
            .data
            .iter()
            .copied()
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        let mse = Self::compute_energy(&audio.data);
        let rms = mse.sqrt();

        if max_abs as f64 > self.threshold as f64 && rms > self.threshold as f64 * 0.5 {
            let confidence = (rms as f32).min(1.0);
            self.last_trigger_frame.store(frame, Ordering::SeqCst);
            return Ok(Some(confidence));
        }

        Ok(None)
    }

    async fn reset(&self) -> std::result::Result<(), VoiceOrchestratorError> {
        self.buffer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.frame_counter.store(0, Ordering::SeqCst);
        self.last_trigger_frame.store(0, Ordering::SeqCst);
        Ok(())
    }

    fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use voxy_voice_orchestrator::AudioChunk;

    fn make_chunk(data: Vec<f32>, sample_rate: u32) -> AudioChunk {
        AudioChunk {
            data,
            sample_rate,
            channels: 1,
            timestamp: Utc::now(),
            sequence: 0,
            is_final: false,
        }
    }

    #[tokio::test]
    async fn test_energy_vad_silence() {
        let vad = EnergyVadDetector::new(0.1, 16000);
        let chunk = make_chunk(vec![0.0; 480], 16000);
        let result = vad.is_voice(&chunk).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_energy_vad_speech_detected() {
        let vad = EnergyVadDetector::new(0.01, 16000)
            .with_min_speech_frames(1)
            .with_silence_frames_for_end(10);
        let samples: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0 * 0.5).sin()).collect();
        let chunk = make_chunk(samples, 16000);
        let result = vad.is_voice(&chunk).await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_energy_vad_reset() {
        let vad = EnergyVadDetector::new(0.01, 16000);
        vad.reset().await.unwrap();
        let chunk = make_chunk(vec![0.0; 480], 16000);
        let result = vad.is_voice(&chunk).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_energy_vad_properties() {
        let vad = EnergyVadDetector::new(0.05, 16000);
        assert_eq!(vad.name(), "energy-vad");
        assert!((vad.threshold() - 0.05).abs() < f32::EPSILON);
        assert!(vad.is_available());
    }

    #[tokio::test]
    async fn test_energy_wakeword_silence() {
        let detector = EnergyWakeWordDetector::new("hey voxy", 0.5, 16000);
        let chunk = make_chunk(vec![0.0; 480], 16000);
        let result = detector.detect(&chunk).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_energy_wakeword_detection() {
        let detector = EnergyWakeWordDetector::new("hey voxy", 0.01, 16000)
            .with_min_duration_frames(1)
            .with_cooldown_frames(100);
        let data: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0 * 3.0).sin()).collect();
        let mut chunk = make_chunk(data, 16000);
        chunk.sequence = 0;
        let result = detector.detect(&chunk).await.unwrap();
        assert!(result.is_some());
        let confidence = result.unwrap();
        assert!(confidence > 0.0);
    }

    #[tokio::test]
    async fn test_energy_wakeword_cooldown() {
        let detector = EnergyWakeWordDetector::new("hey voxy", 0.01, 16000)
            .with_min_duration_frames(1)
            .with_cooldown_frames(100);
        let data: Vec<f32> = (0..480).map(|i| (i as f32 / 480.0 * 3.0).sin()).collect();
        let chunk = make_chunk(data.clone(), 16000);
        let first = detector.detect(&chunk).await.unwrap();
        assert!(first.is_some());
        let second = detector.detect(&chunk).await.unwrap();
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn test_energy_wakeword_reset() {
        let detector = EnergyWakeWordDetector::new("hey voxy", 0.01, 16000);
        detector.reset().await.unwrap();
        assert_eq!(detector.wake_word(), "hey voxy");
        assert!(detector.is_available());
        assert_eq!(detector.name(), "energy-wakeword");
    }

    #[tokio::test]
    async fn test_energy_wakeword_multi_keyword_management() {
        let detector = EnergyWakeWordDetector::new("OSMOO", 0.01, 16000);
        assert_eq!(detector.wake_word(), "osmoo");
        assert!(detector.has_keyword("osmoo"));
        assert!(detector.has_keyword("OSMOO"));

        // Add keywords up to max
        assert!(detector.add_keyword("Hey Osmoo"));
        assert!(detector.add_keyword("Assistant"));
        assert!(detector.add_keyword("Voxy"));
        assert!(detector.has_keyword("hey osmoo"));
        assert!(detector.has_keyword("assistant"));
        assert!(detector.has_keyword("voxy"));

        // Duplicates rejected
        assert!(!detector.add_keyword("voxy"));
        assert!(!detector.add_keyword("VOXY"));

        // Max limit enforcement
        for i in 0..10 {
            let kw = format!("keyword{}", i);
            let _ = detector.add_keyword(&kw);
        }
        assert!(detector.keywords().len() <= EnergyWakeWordDetector::MAX_KEYWORDS);
        assert!(!detector.add_keyword("eleventh_keyword"));

        // Remove keyword
        assert!(detector.remove_keyword("assistant"));
        assert!(!detector.has_keyword("assistant"));

        // Dynamic reload
        let new_list = vec!["osmoo".to_string(), "computer".to_string()];
        detector.reload_keywords(&new_list);
        assert_eq!(detector.keywords(), vec!["osmoo", "computer"]);
    }
}

