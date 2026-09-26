use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::mpsc;
use crate::frames::{Frame, FrameDirection};

#[async_trait]
pub trait Processor: Send + Sync {
    fn name(&self) -> &str;
    
    async fn setup(&self) -> Result<(), String> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), String> {
        Ok(())
    }

    async fn process_frame(
        &self,
        frame: Frame,
        direction: FrameDirection,
        out_tx: &mpsc::Sender<Frame>,
    ) -> Result<(), String>;
}

/// Pipeline is a linear chain of processors through which frames flow.
pub struct Pipeline {
    processors: Vec<Arc<dyn Processor>>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
        }
    }

    pub fn add(&mut self, processor: Arc<dyn Processor>) {
        self.processors.push(processor);
    }

    pub fn processors(&self) -> &[Arc<dyn Processor>] {
        &self.processors
    }

    pub async fn setup(&self) -> Result<(), String> {
        for p in &self.processors {
            p.setup().await?;
        }
        Ok(())
    }

    pub async fn cleanup(&self) {
        for p in self.processors.iter().rev() {
            let _ = p.cleanup().await;
        }
    }
}
