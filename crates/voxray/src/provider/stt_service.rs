//! Bridge connecting STTRouter to VoxraySttService.

use async_trait::async_trait;
use std::sync::Arc;

use crate::provider::router::STTRouter;
use crate::provider::traits::AudioData;
use crate::stt::VoxraySttService;

pub struct CloudSTTService {
    router: Arc<STTRouter>,
}

impl CloudSTTService {
    pub fn new(router: Arc<STTRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl VoxraySttService for CloudSTTService {
    async fn transcribe(
        &self,
        audio_pcm: &[u8],
        sample_rate: u32,
        channels: u16,
    ) -> Result<String, String> {
        if audio_pcm.is_empty() {
            return Ok(String::new());
        }

        let audio = AudioData::new(audio_pcm.to_vec(), sample_rate, channels);
        self.router
            .transcribe(audio, None)
            .await
            .map(|t| t.text)
            .map_err(|e| e.to_string())
    }
}
