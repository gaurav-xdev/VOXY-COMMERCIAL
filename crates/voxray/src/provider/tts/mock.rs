//! Mock TTS provider for tests, fallback simulation, and quota benchmarking.

use async_trait::async_trait;
use futures::stream;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::provider::traits::{
    AudioData, EstimatedCost, ProviderError, TTSCapabilities, TTSProvider, TTSStream, VoiceLanguage,
};

#[derive(Clone)]
pub struct MockTTSProvider {
    id: &'static str,
    name: &'static str,
    capabilities: TTSCapabilities,
    simulated_latency: Duration,
    fail_first_n: Arc<AtomicU32>,
    return_rate_limited: bool,
    call_count: Arc<AtomicU32>,
}

impl MockTTSProvider {
    pub fn new(id: &'static str, name: &'static str) -> Self {
        Self {
            id,
            name,
            capabilities: TTSCapabilities {
                streaming: true,
                supported_languages: vec![
                    VoiceLanguage::English,
                    VoiceLanguage::Hindi,
                    VoiceLanguage::Hinglish,
                ],
                emotional_styles: true,
                speed_control: true,
                pitch_control: true,
                typical_first_byte_ms: 50,
                cost_per_char_usd: 0.00001,
            },
            simulated_latency: Duration::from_millis(5),
            fail_first_n: Arc::new(AtomicU32::new(0)),
            return_rate_limited: false,
            call_count: Arc::new(AtomicU32::new(0)),
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

    pub fn call_count(&self) -> u32 {
        self.call_count.load(Ordering::SeqCst)
    }

    fn generate_dummy_audio() -> AudioData {
        let sample_count = 16000 / 2; // 0.5s
        let pcm = vec![0u8; sample_count * 2];
        AudioData::new(pcm, 16000, 1)
    }
}

#[async_trait]
impl TTSProvider for MockTTSProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn capabilities(&self) -> &TTSCapabilities {
        &self.capabilities
    }

    async fn synthesize(
        &self,
        _text: &str,
        _language: Option<VoiceLanguage>,
    ) -> Result<AudioData, ProviderError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        if self.simulated_latency > Duration::ZERO {
            tokio::time::sleep(self.simulated_latency).await;
        }

        let fails_left = self.fail_first_n.fetch_sub(1, Ordering::SeqCst);
        if fails_left > 0 {
            return Err(ProviderError::NetworkError(format!(
                "Mock TTS failure on {}",
                self.id
            )));
        }
        if fails_left == 0 {
            self.fail_first_n.store(0, Ordering::SeqCst);
        }

        if self.return_rate_limited {
            return Err(ProviderError::RateLimited {
                retry_after: Some(Duration::from_secs(2)),
                reason: format!("Mock TTS rate limited on {}", self.id),
            });
        }

        Ok(Self::generate_dummy_audio())
    }

    async fn synthesize_stream(
        &self,
        text: &str,
        language: Option<VoiceLanguage>,
    ) -> Result<TTSStream, ProviderError> {
        let audio = self.synthesize(text, language).await?;
        let stream = stream::iter(vec![Ok(audio)]);
        Ok(Box::pin(stream))
    }

    fn estimate_cost(&self, character_count: usize) -> EstimatedCost {
        let chars = character_count as f64;
        let cost = chars * self.capabilities.cost_per_char_usd;
        EstimatedCost {
            amount_usd: cost,
            currency: "USD".into(),
            estimated_cost_usd: cost,
            billable_units: chars,
            unit_type: "characters".into(),
        }
    }
}
