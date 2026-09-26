use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;
use crate::frames::{Frame, FrameDirection, LLMTextFrame};
use crate::processor::Processor;

pub type StreamingLlmFn = Arc<
    dyn Fn(String, mpsc::Sender<String>) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// LLMProcessor receives TranscriptionFrame from STT and streams LLM output tokens as LLMTextFrame.
pub struct LLMProcessor {
    streaming_fn: StreamingLlmFn,
}

impl LLMProcessor {
    pub fn new(streaming_fn: StreamingLlmFn) -> Self {
        Self { streaming_fn }
    }
}

#[async_trait]
impl Processor for LLMProcessor {
    fn name(&self) -> &str {
        "LLMProcessor"
    }

    async fn process_frame(
        &self,
        frame: Frame,
        direction: FrameDirection,
        out_tx: &mpsc::Sender<Frame>,
    ) -> Result<(), String> {
        if direction != FrameDirection::Downstream {
            let _ = out_tx.send(frame).await;
            return Ok(());
        }

        match frame {
            Frame::Transcription(tf) => {
                if tf.finalized && !tf.text.is_empty() {
                    tracing::info!("[VOXRAY:LLM] Processing transcription: '{}'", tf.text);
                    let (token_tx, mut token_rx) = mpsc::channel::<String>(32);
                    let prompt = tf.text.clone();
                    let stream_closure = self.streaming_fn.clone();

                    let forward_out_tx = out_tx.clone();
                    tokio::spawn(async move {
                        while let Some(chunk) = token_rx.recv().await {
                            let f = Frame::LLMText(LLMTextFrame::new(chunk));
                            if forward_out_tx.send(f).await.is_err() {
                                break;
                            }
                        }
                    });

                    (stream_closure)(prompt, token_tx).await;
                }
            }
            other => {
                let _ = out_tx.send(other).await;
            }
        }

        Ok(())
    }
}
