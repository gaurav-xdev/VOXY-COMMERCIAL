use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;
use crate::frames::{Frame, FrameDirection, InterruptionFrame};
use crate::processor::Processor;

/// InterruptionController observes bot speech state and user speaking events,
/// emitting InterruptionFrame to cancel in-flight TTS playback and LLM streaming (barge-in).
pub struct InterruptionController {
    bot_speaking: Arc<AtomicBool>,
    allow_interruptions: Arc<AtomicBool>,
}

impl InterruptionController {
    pub fn new(allow_interruptions: bool) -> Self {
        Self {
            bot_speaking: Arc::new(AtomicBool::new(false)),
            allow_interruptions: Arc::new(AtomicBool::new(allow_interruptions)),
        }
    }

    pub fn set_allow_interruptions(&self, allow: bool) {
        self.allow_interruptions.store(allow, Ordering::SeqCst);
    }
}

#[async_trait]
impl Processor for InterruptionController {
    fn name(&self) -> &str {
        "InterruptionController"
    }

    async fn process_frame(
        &self,
        frame: Frame,
        _direction: FrameDirection,
        out_tx: &mpsc::Sender<Frame>,
    ) -> Result<(), String> {
        match &frame {
            Frame::BotStartedSpeaking(_) => {
                self.bot_speaking.store(true, Ordering::SeqCst);
            }
            Frame::BotStoppedSpeaking(_) => {
                self.bot_speaking.store(false, Ordering::SeqCst);
            }
            Frame::UserStartedSpeaking(_) => {
                if self.allow_interruptions.load(Ordering::Relaxed)
                    && self.bot_speaking.load(Ordering::SeqCst)
                {
                    tracing::info!("[VOXRAY:BARGE-IN] Interruption triggered by user speech!");
                    self.bot_speaking.store(false, Ordering::SeqCst);

                    // Send interruption frame downstream to clear playback buffer
                    let _ = out_tx.send(Frame::Interruption(InterruptionFrame::default())).await;
                }
            }
            _ => {}
        }

        let _ = out_tx.send(frame).await;
        Ok(())
    }
}
