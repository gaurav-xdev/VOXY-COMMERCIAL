use crate::enclave::VideoStudioEnclave;
use crate::pipeline::{StudioPipelineEngine, VideoProductionRequest};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use voxy_tool_calling::{
    RiskTier, Tool, ToolCategory, ToolContext, ToolError, ToolMetadata, ToolResult,
};

/// Tool for orchestrating OpenMontage production pipelines inside the OSMOO secure enclave.
pub struct VideoStudioPipelineTool {
    metadata: ToolMetadata,
    engine: Arc<StudioPipelineEngine>,
    enclave: VideoStudioEnclave,
}

impl VideoStudioPipelineTool {
    pub fn new(enclave: VideoStudioEnclave) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_video_pipeline",
            "Orchestrate an end-to-end video production pipeline (screen-demo, explainer, clip-factory, talking-head) inside the isolated OSMOO video studio enclave.",
            ToolCategory::Custom,
            RiskTier::Privileged,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["task_id", "pipeline_name", "topic", "enclave_auth_token", "caller_identity"],
            "properties": {
                "task_id": { "type": "string" },
                "pipeline_name": { "type": "string", "description": "Name of pipeline (e.g. screen-demo, animated-explainer, documentary-montage, clip-factory)" },
                "topic": { "type": "string", "description": "Subject or title of video" },
                "target_duration_secs": { "type": "integer", "default": 60 },
                "aspect_ratio": { "type": "string", "default": "16:9" },
                "style": { "type": "string", "default": "modern-minimal" },
                "custom_instructions": { "type": "string" },
                "enclave_auth_token": { "type": "string", "description": "Cryptographic session bearer token held by OSMOO agent" },
                "caller_identity": { "type": "string", "description": "Must be OSMOO_AUTONOMOUS_OPERATOR" }
            }
        }));

        let engine = Arc::new(StudioPipelineEngine::new(enclave.clone()));

        Self {
            metadata,
            engine,
            enclave,
        }
    }
}

#[async_trait]
impl Tool for VideoStudioPipelineTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> voxy_tool_calling::Result<ToolResult> {
        let caller = params["caller_identity"]
            .as_str()
            .unwrap_or("untrusted_user");
        let token = params["enclave_auth_token"]
            .as_str()
            .unwrap_or("");

        // STRICT ACCESS ENFORCEMENT: Only OSMOO can execute this tool
        if let Err(e) = self.enclave.access_controller().verify_access(caller, token) {
            return Ok(ToolResult::failure(format!("ACCESS DENIED: {e}"))
                .with_observation("Unauthorized attempt to invoke OSMOO Video Studio"));
        }

        let task_id = params["task_id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParams("Missing 'task_id'".into()))?;
        let pipeline_name = params["pipeline_name"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParams("Missing 'pipeline_name'".into()))?;
        let topic = params["topic"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidParams("Missing 'topic'".into()))?;
        let duration = params["target_duration_secs"]
            .as_u64()
            .unwrap_or(60) as u32;
        let aspect_ratio = params["aspect_ratio"]
            .as_str()
            .unwrap_or("16:9")
            .to_string();
        let style = params["style"]
            .as_str()
            .unwrap_or("modern-minimal")
            .to_string();
        let custom_instructions = params["custom_instructions"]
            .as_str()
            .map(|s| s.to_string());

        let req = VideoProductionRequest {
            task_id: task_id.to_string(),
            pipeline_name: pipeline_name.to_string(),
            topic: topic.to_string(),
            target_duration_secs: duration,
            aspect_ratio,
            style,
            custom_instructions,
        };

        match self.engine.plan_production(&req, token) {
            Ok(plan) => {
                let stages_count = plan.stages.len();
                Ok(ToolResult::success(json!({
                    "task_id": plan.task_id,
                    "pipeline": plan.pipeline_name,
                    "stages_count": stages_count,
                    "stages": plan.stages,
                    "workspace": plan.workspace_dir,
                }))
                .with_observation(format!(
                    "Initialized video production pipeline '{}' with {} autonomous stages in secure enclave.",
                    pipeline_name, stages_count
                ))
                .with_verification("Production plan validated against OpenMontage pipeline schema"))
            }
            Err(e) => Ok(ToolResult::failure(format!("Pipeline error: {e}"))),
        }
    }
}

/// Tool for querying installed OpenMontage skills and pipeline definitions within the enclave.
pub struct VideoStudioInspectTool {
    metadata: ToolMetadata,
    enclave: VideoStudioEnclave,
}

impl VideoStudioInspectTool {
    pub fn new(enclave: VideoStudioEnclave) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_video_inspect",
            "Inspect installed OpenMontage pipelines, skills, and tools inside the OSMOO video studio enclave.",
            ToolCategory::Custom,
            RiskTier::Read,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["enclave_auth_token", "caller_identity"],
            "properties": {
                "enclave_auth_token": { "type": "string" },
                "caller_identity": { "type": "string" }
            }
        }));

        Self { metadata, enclave }
    }
}

#[async_trait]
impl Tool for VideoStudioInspectTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> voxy_tool_calling::Result<ToolResult> {
        let caller = params["caller_identity"].as_str().unwrap_or("untrusted_user");
        let token = params["enclave_auth_token"].as_str().unwrap_or("");

        if let Err(e) = self.enclave.access_controller().verify_access(caller, token) {
            return Ok(ToolResult::failure(format!("ACCESS DENIED: {e}")));
        }

        let pipelines = self.enclave.list_pipelines(token).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let skills = self.enclave.list_skills(token).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        let tools = self.enclave.list_tools(token).map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolResult::success(json!({
            "enclave_status": "active_and_isolated",
            "pipelines_count": pipelines.len(),
            "pipelines": pipelines,
            "skills_count": skills.len(),
            "skills": skills,
            "tools_count": tools.len(),
            "tools": tools,
        }))
        .with_observation("Inspected video studio enclave successfully."))
    }
}
