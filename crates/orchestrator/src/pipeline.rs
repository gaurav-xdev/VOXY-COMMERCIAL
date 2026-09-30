use async_trait::async_trait;
use tokio::time::{timeout, Duration};
use tracing::{error, warn};

use crate::execution::{
    CancellationFlag, ExecutionContext, PipelineInput, PipelineOutput, PipelineStage,
};
use crate::types::SystemComponent;

#[async_trait]
pub trait StageHandler: Send + Sync {
    fn name(&self) -> &str;
    fn component(&self) -> SystemComponent;
    async fn execute(
        &self,
        ctx: &ExecutionContext,
    ) -> std::result::Result<serde_json::Value, String>;
}

pub async fn execute_pipeline(
    input: PipelineInput,
    stages: &[Box<dyn StageHandler>],
    cancellation: &CancellationFlag,
    timeout_seconds: u64,
    max_retries: u32,
) -> PipelineOutput {
    let mut ctx = ExecutionContext::new()
        .with_session_id(input.session_id.unwrap_or_default())
        .with_user_input(input.text.unwrap_or_default());
    if !input.metadata.is_empty() {
        ctx.metadata = input.metadata;
    }

    let overall_timeout = Duration::from_secs(timeout_seconds);
    let result = timeout(
        overall_timeout,
        run_stages(&mut ctx, stages, cancellation, timeout_seconds, max_retries),
    )
    .await;

    let (success, response_text, error) = match result {
        Ok(Ok(resp)) => (true, Some(resp), None),
        Ok(Err(e)) => (false, None, Some(e)),
        Err(_elapsed) => (
            false,
            None,
            Some("Pipeline execution timed out".to_string()),
        ),
    };

    let total_ms = ctx.total_duration_ms();

    PipelineOutput {
        correlation_id: ctx.correlation_id,
        success,
        response_text,
        timeline: ctx.timeline,
        audit_events: ctx.audit_events,
        error,
        total_duration_ms: total_ms,
    }
}

async fn run_stages(
    ctx: &mut ExecutionContext,
    stages: &[Box<dyn StageHandler>],
    cancellation: &CancellationFlag,
    timeout_seconds: u64,
    max_retries: u32,
) -> std::result::Result<String, String> {
    for handler in stages {
        cancellation.check()?;

        let stage_enum = component_to_stage(handler.component());
        ctx.start_stage(stage_enum);

        let result = execute_with_retry(
            handler.as_ref(),
            ctx,
            cancellation,
            timeout_seconds,
            max_retries,
        )
        .await;

        match result {
            Ok(value) => {
                ctx.complete_stage(stage_enum, true, None);
                if let Some(s) = value.as_str() {
                    if !s.is_empty() {
                        ctx.metadata
                            .insert(format!("result_{}", handler.name()), s.to_string());
                    }
                }
            }
            Err(e) => {
                ctx.complete_stage(stage_enum, false, Some(e.clone()));
                error!("Stage {} failed: {}", handler.name(), e);
                return Err(format!("Stage {} failed: {}", handler.name(), e));
            }
        }
    }

    Ok(ctx
        .metadata
        .get("result_cognition")
        .cloned()
        .unwrap_or_default())
}

async fn execute_with_retry(
    handler: &dyn StageHandler,
    ctx: &ExecutionContext,
    cancellation: &CancellationFlag,
    timeout_seconds: u64,
    max_retries: u32,
) -> std::result::Result<serde_json::Value, String> {
    let mut last_error = String::new();
    let max_attempts = match handler.component() {
        SystemComponent::Automation | SystemComponent::Vision => max_retries.max(1),
        _ => 1,
    };

    for attempt in 0..max_attempts {
        cancellation.check()?;

        if attempt > 0 {
            let delay = Duration::from_millis(
                100u64.saturating_mul(2u64.checked_pow(attempt).unwrap_or(u64::MAX)),
            );
            tokio::time::sleep(delay).await;
        }

        let stage_timeout = Duration::from_secs(timeout_seconds);
        match timeout(stage_timeout, handler.execute(ctx)).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(e)) => {
                last_error = e.clone();
                warn!("{} attempt {} failed: {}", handler.name(), attempt + 1, e);
            }
            Err(_elapsed) => {
                last_error = format!("{} timed out after {}s", handler.name(), timeout_seconds);
                warn!("{}", last_error);
            }
        }
    }

    Err(last_error)
}

fn component_to_stage(component: SystemComponent) -> PipelineStage {
    match component {
        SystemComponent::Voice => PipelineStage::VoiceProcessing,
        SystemComponent::Conversation => PipelineStage::Conversation,
        SystemComponent::Cognition => PipelineStage::Cognition,
        SystemComponent::Planner => PipelineStage::Planning,
        SystemComponent::Providers => PipelineStage::ToolSelection,
        SystemComponent::Guardian => PipelineStage::GuardianCheck,
        SystemComponent::WorldModel => PipelineStage::WorldModelUpdate,
        SystemComponent::Automation | SystemComponent::Vision | SystemComponent::Home => {
            PipelineStage::Execution
        }
        SystemComponent::Reflection => PipelineStage::Reflection,
        SystemComponent::Memory => PipelineStage::MemoryStorage,
        SystemComponent::Learning => PipelineStage::Learning,
        _ => PipelineStage::Conversation,
    }
}

pub fn create_pipeline_stages(stages: Vec<Box<dyn StageHandler>>) -> Vec<Box<dyn StageHandler>> {
    stages
}
