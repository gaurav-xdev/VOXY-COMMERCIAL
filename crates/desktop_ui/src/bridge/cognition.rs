use std::sync::Arc;

use voxy_cognition::{CognitiveEngine, CognitiveResult, CognitiveState, IntentInput};

#[derive(Clone)]
pub struct CognitionBridge {
    engine: Arc<dyn CognitiveEngine>,
}

impl CognitionBridge {
    pub fn new(engine: Arc<dyn CognitiveEngine>) -> Self {
        Self { engine }
    }

    pub async fn process(&self, input: &IntentInput) -> Result<CognitiveResult, String> {
        self.engine
            .process(input)
            .await
            .map_err(|e| format!("Process failed: {}", e))
    }

    pub async fn state(&self) -> CognitiveState {
        self.engine.state().await
    }

    pub async fn cancel_current(&self) -> Result<(), String> {
        if let Some(intent_id) = self.engine.current_intent().await {
            self.engine
                .cancel(&intent_id)
                .await
                .map_err(|e| format!("Cancel failed: {}", e))?;
        }
        Ok(())
    }
}
