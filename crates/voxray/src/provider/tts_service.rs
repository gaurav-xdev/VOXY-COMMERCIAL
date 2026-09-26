//! Bridge connecting TTSRouter to VoxrayTtsService.

use async_trait::async_trait;
use std::sync::Arc;

use crate::provider::router::TTSRouter;
use crate::tts::VoxrayTtsService;

pub struct CloudTTSService {
    router: Arc<TTSRouter>,
}

impl CloudTTSService {
    pub fn new(router: Arc<TTSRouter>) -> Self {
        Self { router }
    }
}

#[async_trait]
impl VoxrayTtsService for CloudTTSService {
    async fn synthesize(&self, text: &str, _sample_rate: u32) -> Result<Vec<u8>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let audio = self
            .router
            .synthesize(text, None)
            .await
            .map_err(|e| e.to_string())?;

        Ok(audio.pcm_bytes)
    }
}
