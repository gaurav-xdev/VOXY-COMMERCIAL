use std::sync::Arc;

use voxy_voice::VoicePipeline;

#[derive(Clone)]
pub struct VoiceBridge {
    pipeline: Arc<VoicePipeline>,
}

impl VoiceBridge {
    pub fn new(pipeline: Arc<VoicePipeline>) -> Self {
        Self { pipeline }
    }

    pub async fn start_listening(&self) -> Result<(), String> {
        self.pipeline
            .start_listening()
            .await
            .map_err(|e| format!("Start listening failed: {}", e))
    }

    pub async fn stop_listening(&self) {
        self.pipeline.stop_listening().await;
    }

    pub async fn speak(&self, text: &str) -> Result<(), String> {
        self.pipeline
            .speak(text)
            .await
            .map_err(|e| format!("Speak failed: {}", e))
    }

    pub async fn interrupt(&self) {
        self.pipeline.interrupt_tts().await;
    }

    pub async fn barge_in(&self) {
        self.pipeline.barge_in().await;
    }

    pub fn is_running(&self) -> bool {
        self.pipeline.is_running()
    }

    pub fn is_speaking(&self) -> bool {
        self.pipeline.is_speaking()
    }

    pub async fn streaming_metrics(&self) -> voxy_voice::pipeline::StreamingMetrics {
        self.pipeline.streaming_metrics().await
    }
}
