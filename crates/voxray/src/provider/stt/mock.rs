//! Mock STT provider for tests, fallback simulation, and quota benchmarking.

use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, STTCapabilities, STTProvider, Transcript,
    VoiceLanguage,
};

#[derive(Clone)]
pub struct MockSTTProvider {
    id: &'static str,
    name: &'static str,
    capabilities: STTCapabilities,
    simulated_latency: Duration,
    fail_first_n: Arc<AtomicU32>,
    return_rate_limited: bool,
    custom_transcript: String,
}

impl MockSTTProvider {
    pub fn new(id: &'static str, name: &'static str) -> Self {
        Self {
            id,
            name,
            capabilities: STTCapabilities {
                streaming: true,
                word_timestamps: true,
                supported_languages: vec![
                    VoiceLanguage::English,
                    VoiceLanguage::Hindi,
                    VoiceLanguage::Hinglish,
                ],
                multilingual: true,
                custom_vocab: true,
                max_audio_duration_secs: 600,
                typical_latency_ms: 120,
                cost_per_second_usd: 0.0001,
            },
            simulated_latency: Duration::from_millis(10),
            fail_first_n: Arc::new(AtomicU32::new(0)),
            return_rate_limited: false,
            custom_transcript: "Hello, this is a mock transcription.".to_string(),
        }
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.simulated_latency = latency;
        self
    }

    pub fn with_failures(self, count: u32) -> Self {
        self.fail_first_n.store(count, Ordering::SeqCst);
        self
    }

    pub fn with_rate_limit(mut self, is_rate_limited: bool) -> Self {
        self.return_rate_limited = is_rate_limited;
        self
    }

    pub fn with_transcript(mut self, text: impl Into<String>) -> Self {
        self.custom_transcript = text.into();
        self
    }
}

#[async_trait]
impl STTProvider for MockSTTProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn capabilities(&self) -> &STTCapabilities {
        &self.capabilities
    }

    async fn transcribe(
        &self,
        audio: AudioData,
        language_hint: Option<VoiceLanguage>,
    ) -> Result<Transcript, ProviderError> {
        if self.simulated_latency > Duration::ZERO {
            tokio::time::sleep(self.simulated_latency).await;
        }

        let fails_left = self.fail_first_n.fetch_sub(1, Ordering::SeqCst);
        if fails_left > 0 {
            return Err(ProviderError::NetworkError(format!(
                "Simulated failure on mock provider {}",
                self.id
            )));
        }
        if fails_left == 0 {
            self.fail_first_n.store(0, Ordering::SeqCst);
        }

        if self.return_rate_limited {
            return Err(ProviderError::RateLimited {
                retry_after: Some(Duration::from_secs(2)),
                reason: format!("Mock rate limit hit on {}", self.id),
            });
        }

        Ok(Transcript {
            text: self.custom_transcript.clone(),
            is_final: true,
            confidence: Some(0.99),
            language: language_hint.or(Some(VoiceLanguage::English)),
            latency_ms: self.simulated_latency.as_millis() as u64,
            provider: self.id.to_string(),
            duration_secs: audio.duration_secs(),
            words: None,
        })
    }

    fn estimate_cost(&self, audio_duration_secs: f64) -> EstimatedCost {
        let cost = audio_duration_secs * self.capabilities.cost_per_second_usd;
        EstimatedCost {
            amount_usd: cost,
            currency: "USD".into(),
            estimated_cost_usd: cost,
            billable_units: audio_duration_secs,
            unit_type: "seconds".into(),
        }
    }
}
