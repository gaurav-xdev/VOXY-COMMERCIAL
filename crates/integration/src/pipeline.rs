//! Unified Pipeline — execution flow from input to output.
//!
//! Connects: Voice/Text → Streaming STT → Conversation → Decision Engine →
//! Planner → Task Graph → Agent Orchestrator → Automation → Memory Update → TTS
//!
//! Every stage reports metrics to CentralTelemetry and events to EventBridge.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::event_bridge::EventBridge;
use crate::telemetry::{CentralTelemetry, SubsystemMetrics};

// ============================================================================
// Pipeline Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipelineStage {
    Input,
    StreamingStt,
    Conversation,
    DecisionEngine,
    Planner,
    TaskGraph,
    AgentOrchestrator,
    Automation,
    MemoryUpdate,
    StreamingTts,
    Output,
}

impl std::fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Input => write!(f, "Input"),
            Self::StreamingStt => write!(f, "StreamingSTT"),
            Self::Conversation => write!(f, "Conversation"),
            Self::DecisionEngine => write!(f, "DecisionEngine"),
            Self::Planner => write!(f, "Planner"),
            Self::TaskGraph => write!(f, "TaskGraph"),
            Self::AgentOrchestrator => write!(f, "AgentOrchestrator"),
            Self::Automation => write!(f, "Automation"),
            Self::MemoryUpdate => write!(f, "MemoryUpdate"),
            Self::StreamingTts => write!(f, "StreamingTTS"),
            Self::Output => write!(f, "Output"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageMetrics {
    pub stage: PipelineStage,
    pub latency_ms: f64,
    pub success: bool,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineRequest {
    pub id: String,
    pub input_text: Option<String>,
    pub input_audio: Option<Vec<f32>>,
    pub context: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

impl PipelineRequest {
    pub fn from_text(text: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            input_text: Some(text.to_string()),
            input_audio: None,
            context: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    pub fn from_audio(audio: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            input_text: None,
            input_audio: Some(audio),
            context: HashMap::new(),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResponse {
    pub request_id: String,
    pub output_text: Option<String>,
    pub output_audio: Option<Vec<f32>>,
    pub stages_completed: Vec<PipelineStage>,
    pub total_latency_ms: f64,
    pub success: bool,
    pub error: Option<String>,
}

// ============================================================================
// Stage Handler Trait
// ============================================================================

#[async_trait::async_trait]
pub trait StageHandler: Send + Sync {
    fn stage(&self) -> PipelineStage;
    async fn process(
        &self,
        request: &PipelineRequest,
        context: &mut PipelineContext,
    ) -> Result<(), String>;
}

#[derive(Debug, Default)]
pub struct PipelineContext {
    pub data: HashMap<String, String>,
    pub intermediate_results: Vec<String>,
}

// ============================================================================
// Unified Pipeline
// ============================================================================

/// Connects all processing stages into one continuous pipeline.
pub struct UnifiedPipeline {
    handlers: RwLock<Vec<Arc<dyn StageHandler>>>,
    stage_order: Vec<PipelineStage>,
    event_bridge: Arc<EventBridge>,
    telemetry: Arc<CentralTelemetry>,
}

impl UnifiedPipeline {
    pub fn new(event_bridge: Arc<EventBridge>, telemetry: Arc<CentralTelemetry>) -> Self {
        Self {
            handlers: RwLock::new(Vec::new()),
            stage_order: vec![
                PipelineStage::Input,
                PipelineStage::StreamingStt,
                PipelineStage::Conversation,
                PipelineStage::DecisionEngine,
                PipelineStage::Planner,
                PipelineStage::TaskGraph,
                PipelineStage::AgentOrchestrator,
                PipelineStage::Automation,
                PipelineStage::MemoryUpdate,
                PipelineStage::StreamingTts,
                PipelineStage::Output,
            ],
            event_bridge,
            telemetry,
        }
    }

    /// Register a stage handler.
    pub fn register_handler(&self, handler: Arc<dyn StageHandler>) {
        self.handlers.write().push(handler);
    }

    /// Execute the full pipeline for a request.
    pub async fn execute(&self, request: PipelineRequest) -> PipelineResponse {
        let start = std::time::Instant::now();
        let mut context = PipelineContext::default();
        let mut stages_completed = Vec::new();
        let mut last_error = None;

        for stage in &self.stage_order {
            let handler = {
                let handlers = self.handlers.read();
                handlers.iter().find(|h| h.stage() == *stage).cloned()
            };

            if let Some(handler) = handler {
                let stage_start = std::time::Instant::now();
                let result = handler.process(&request, &mut context).await;
                let stage_latency = stage_start.elapsed().as_secs_f64() * 1000.0;

                // Report metrics
                self.telemetry.report(SubsystemMetrics {
                    name: format!("pipeline_{}", stage),
                    latency_ms: stage_latency,
                    error_count: if result.is_err() { 1 } else { 0 },
                    ..SubsystemMetrics::new(format!("pipeline_{}", stage))
                });

                // Publish stage event
                let stage_event = StageMetrics {
                    stage: stage.clone(),
                    latency_ms: stage_latency,
                    success: result.is_ok(),
                    error: result.as_ref().err().cloned(),
                    timestamp: Utc::now(),
                };
                let _ = self
                    .event_bridge
                    .publish(
                        &format!(
                            "pipeline.{}.{}",
                            stage,
                            if result.is_ok() { "done" } else { "failed" }
                        ),
                        "pipeline",
                        &stage_event,
                    )
                    .await;

                match result {
                    Ok(()) => stages_completed.push(stage.clone()),
                    Err(e) => {
                        last_error = Some(format!("Stage {} failed: {}", stage, e));
                        break;
                    }
                }
            }
        }

        let total_latency = start.elapsed().as_secs_f64() * 1000.0;

        PipelineResponse {
            request_id: request.id,
            output_text: context.data.get("output_text").cloned(),
            output_audio: None,
            stages_completed,
            total_latency_ms: total_latency,
            success: last_error.is_none(),
            error: last_error,
        }
    }

    /// Get the stage execution order.
    pub fn stage_order(&self) -> &[PipelineStage] {
        &self.stage_order
    }

    /// Get registered handler count.
    pub fn handler_count(&self) -> usize {
        self.handlers.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockInputHandler;

    #[async_trait::async_trait]
    impl StageHandler for MockInputHandler {
        fn stage(&self) -> PipelineStage {
            PipelineStage::Input
        }
        async fn process(
            &self,
            request: &PipelineRequest,
            context: &mut PipelineContext,
        ) -> Result<(), String> {
            if let Some(text) = &request.input_text {
                context.data.insert("raw_input".to_string(), text.clone());
            }
            Ok(())
        }
    }

    struct MockSttHandler;

    #[async_trait::async_trait]
    impl StageHandler for MockSttHandler {
        fn stage(&self) -> PipelineStage {
            PipelineStage::StreamingStt
        }
        async fn process(
            &self,
            _request: &PipelineRequest,
            context: &mut PipelineContext,
        ) -> Result<(), String> {
            let raw = context.data.get("raw_input").cloned().unwrap_or_default();
            context.data.insert("stt_text".to_string(), raw);
            Ok(())
        }
    }

    struct MockFailingHandler;

    #[async_trait::async_trait]
    impl StageHandler for MockFailingHandler {
        fn stage(&self) -> PipelineStage {
            PipelineStage::DecisionEngine
        }
        async fn process(
            &self,
            _request: &PipelineRequest,
            _context: &mut PipelineContext,
        ) -> Result<(), String> {
            Err("decision timeout".to_string())
        }
    }

    fn test_pipeline() -> UnifiedPipeline {
        let bus = Arc::new(voxy_event_bus::EventBus::new(64));
        let bridge = Arc::new(EventBridge::new(bus));
        let telemetry = Arc::new(CentralTelemetry::new());
        let pipeline = UnifiedPipeline::new(bridge, telemetry);
        pipeline.register_handler(Arc::new(MockInputHandler));
        pipeline.register_handler(Arc::new(MockSttHandler));
        pipeline
    }

    #[tokio::test]
    async fn pipeline_creation() {
        let pipeline = test_pipeline();
        assert_eq!(pipeline.handler_count(), 2);
        assert_eq!(pipeline.stage_order().len(), 11);
    }

    #[tokio::test]
    async fn pipeline_execute_success() {
        let pipeline = test_pipeline();
        let request = PipelineRequest::from_text("hello world");
        let response = pipeline.execute(request).await;
        assert!(response.success);
        assert!(response.error.is_none());
        assert!(!response.stages_completed.is_empty());
    }

    #[tokio::test]
    async fn pipeline_execute_with_failure() {
        let bus = Arc::new(voxy_event_bus::EventBus::new(64));
        let bridge = Arc::new(EventBridge::new(bus));
        let telemetry = Arc::new(CentralTelemetry::new());
        let pipeline = UnifiedPipeline::new(bridge, telemetry);
        pipeline.register_handler(Arc::new(MockInputHandler));
        pipeline.register_handler(Arc::new(MockFailingHandler));

        let request = PipelineRequest::from_text("test");
        let response = pipeline.execute(request).await;
        assert!(!response.success);
        assert!(response.error.is_some());
    }

    #[test]
    fn pipeline_request_from_text() {
        let req = PipelineRequest::from_text("hi");
        assert_eq!(req.input_text.as_deref(), Some("hi"));
        assert!(req.input_audio.is_none());
    }

    #[test]
    fn pipeline_request_from_audio() {
        let req = PipelineRequest::from_audio(vec![0.1, 0.2, 0.3]);
        assert!(req.input_text.is_none());
        assert!(req.input_audio.is_some());
    }

    #[test]
    fn stage_display() {
        assert_eq!(PipelineStage::Input.to_string(), "Input");
        assert_eq!(PipelineStage::StreamingStt.to_string(), "StreamingSTT");
        assert_eq!(PipelineStage::DecisionEngine.to_string(), "DecisionEngine");
        assert_eq!(PipelineStage::Output.to_string(), "Output");
    }

    #[test]
    fn pipeline_stage_order_is_correct() {
        let pipeline = test_pipeline();
        let order = pipeline.stage_order();
        assert_eq!(order[0], PipelineStage::Input);
        assert_eq!(order[10], PipelineStage::Output);
    }
}
