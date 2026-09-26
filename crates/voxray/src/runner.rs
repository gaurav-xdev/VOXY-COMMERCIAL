use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::frames::{Frame, FrameDirection, StartFrame};
use crate::processor::Pipeline;

pub trait Transport: Send + Sync {
    fn input(&self) -> mpsc::Receiver<Frame>;
    fn output(&self) -> mpsc::Sender<Frame>;
}

/// PipelineRunner coordinates execution across processors in the pipeline.
pub struct PipelineRunner {
    pipeline: Arc<Pipeline>,
    in_rx: mpsc::Receiver<Frame>,
    out_tx: mpsc::Sender<Frame>,
    running: Arc<AtomicBool>,
}

impl PipelineRunner {
    pub fn new(
        pipeline: Arc<Pipeline>,
        in_rx: mpsc::Receiver<Frame>,
        out_tx: mpsc::Sender<Frame>,
    ) -> Self {
        Self {
            pipeline,
            in_rx,
            out_tx,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn running_flag(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    pub async fn run(mut self, start_frame: Option<StartFrame>) -> Result<(), String> {
        self.pipeline.setup().await?;
        self.running.store(true, Ordering::SeqCst);

        // Feed start frame if provided
        let start = start_frame.unwrap_or_default();
        let _ = self.push_downstream(Frame::Start(start)).await;

        while self.running.load(Ordering::Relaxed) {
            match self.in_rx.recv().await {
                Some(frame) => {
                    if let Frame::Cancel(_) | Frame::Stop(_) = frame {
                        let _ = self.push_downstream(frame).await;
                        break;
                    }
                    if let Err(e) = self.push_downstream(frame).await {
                        tracing::warn!("[VOXRAY:RUNNER] Frame processing error: {}", e);
                    }
                }
                None => break,
            }
        }

        self.running.store(false, Ordering::SeqCst);
        self.pipeline.cleanup().await;
        Ok(())
    }

    async fn push_downstream(&self, initial_frame: Frame) -> Result<(), String> {
        let procs = self.pipeline.processors();
        if procs.is_empty() {
            let _ = self.out_tx.send(initial_frame).await;
            return Ok(());
        }

        // Forward through processor chain
        let mut current_frames = vec![initial_frame];
        for proc in procs {
            let mut next_frames = Vec::new();
            for f in current_frames {
                let (tx, mut rx) = mpsc::channel(32);
                proc.process_frame(f, FrameDirection::Downstream, &tx).await?;
                drop(tx);
                while let Some(out_f) = rx.recv().await {
                    next_frames.push(out_f);
                }
            }
            current_frames = next_frames;
            if current_frames.is_empty() {
                break;
            }
        }

        for f in current_frames {
            let _ = self.out_tx.send(f).await;
        }

        Ok(())
    }
}
