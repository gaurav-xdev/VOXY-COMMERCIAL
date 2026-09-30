pub mod frames;
pub mod interruptions;
pub mod llm;
pub mod processor;
pub mod provider;
pub mod runner;
pub mod stt;
pub mod tts;
pub mod turn;

pub use frames::*;
pub use interruptions::InterruptionController;
pub use llm::{LLMProcessor, StreamingLlmFn};
pub use processor::{Pipeline, Processor};
pub use provider::{
    AudioData, CloudSTTService, CloudTTSService, EstimatedCost, HealthStatus, ProviderError,
    ProviderQuotaManager, STTProvider, STTRouter, TTSProvider, TTSRouter, VoiceLanguage,
    VoiceSystem,
};
pub use runner::PipelineRunner;
pub use stt::{CloudOpenAiSttService, EchoSttService, STTProcessor, VoxraySttService};
pub use tts::{CloudOpenAiTtsService, TTSProcessor, ToneTtsService, VoxrayTtsService};
pub use turn::{TurnProcessor, TurnProcessorConfig};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_voxray_pipeline_basic_flow() {
        let mut pipeline = Pipeline::new();
        let stt_service = Arc::new(EchoSttService);
        let tts_service = Arc::new(ToneTtsService);

        pipeline.add(Arc::new(TurnProcessor::new(TurnProcessorConfig::default())));
        pipeline.add(Arc::new(STTProcessor::new(stt_service)));
        pipeline.add(Arc::new(TTSProcessor::new(tts_service, 16000)));

        let (in_tx, in_rx) = mpsc::channel(32);
        let (out_tx, mut out_rx) = mpsc::channel(32);
        let runner = PipelineRunner::new(Arc::new(pipeline), in_rx, out_tx);

        let runner_handle = tokio::spawn(async move {
            let _ = runner.run(None).await;
        });

        // Push mock audio chunk (1600 bytes)
        let audio_data = vec![0x20u8; 1600];
        let audio_frame = Frame::AudioRaw(AudioRawFrame::new(audio_data, 16000, 1));
        let _ = in_tx.send(audio_frame).await;

        // Close stream
        drop(in_tx);
        let _ = runner_handle.await;

        let mut received_start = false;
        while let Some(f) = out_rx.recv().await {
            if let Frame::Start(_) = f {
                received_start = true;
            }
        }
        assert!(received_start);
    }

    #[tokio::test]
    async fn test_voxray_llm_streaming_and_tts() {
        let mut pipeline = Pipeline::new();

        let streaming_llm: StreamingLlmFn = Arc::new(|prompt, sender| {
            Box::pin(async move {
                assert_eq!(prompt, "hello voxy");
                let _ = sender.send("Hello ".to_string()).await;
                let _ = sender.send("there. How can I help?\n".to_string()).await;
            })
        });

        pipeline.add(Arc::new(LLMProcessor::new(streaming_llm)));
        pipeline.add(Arc::new(TTSProcessor::new(Arc::new(ToneTtsService), 16000)));

        let (in_tx, in_rx) = mpsc::channel(32);
        let (out_tx, mut out_rx) = mpsc::channel(32);
        let runner = PipelineRunner::new(Arc::new(pipeline), in_rx, out_tx);

        let runner_handle = tokio::spawn(async move {
            let _ = runner.run(None).await;
        });

        // Send a transcription frame
        let tf = TranscriptionFrame::new("hello voxy", true);
        let _ = in_tx.send(Frame::Transcription(tf)).await;

        drop(in_tx);
        let _ = runner_handle.await;

        let mut received_tts_audio = false;
        while let Some(f) = out_rx.recv().await {
            if let Frame::TTSAudioRaw(raw) = f {
                if !raw.audio.is_empty() {
                    received_tts_audio = true;
                }
            }
        }
        assert!(received_tts_audio);
    }

    #[tokio::test]
    async fn test_voxray_barge_in_interruption() {
        let mut pipeline = Pipeline::new();
        let interrupter = Arc::new(InterruptionController::new(true));
        pipeline.add(interrupter);

        let (in_tx, in_rx) = mpsc::channel(32);
        let (out_tx, mut out_rx) = mpsc::channel(32);
        let runner = PipelineRunner::new(Arc::new(pipeline), in_rx, out_tx);

        let runner_handle = tokio::spawn(async move {
            let _ = runner.run(None).await;
        });

        // Bot begins speaking
        let _ = in_tx
            .send(Frame::BotStartedSpeaking(BotStartedSpeakingFrame::default()))
            .await;
        // User speaks while bot is speaking -> should trigger barge-in InterruptionFrame
        let _ = in_tx
            .send(Frame::UserStartedSpeaking(
                UserStartedSpeakingFrame::default(),
            ))
            .await;

        drop(in_tx);
        let _ = runner_handle.await;

        let mut saw_interruption = false;
        while let Some(f) = out_rx.recv().await {
            if let Frame::Interruption(_) = f {
                saw_interruption = true;
            }
        }
        assert!(saw_interruption, "Expected InterruptionFrame on barge-in");
    }
}
