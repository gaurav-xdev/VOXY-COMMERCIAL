use crate::enclave::VideoStudioEnclave;
use crate::error::Result;
use crate::security::OSMOO_OPERATOR_IDENTITY;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProductionRequest {
    pub task_id: String,
    pub pipeline_name: String,
    pub topic: String,
    pub target_duration_secs: u32,
    pub aspect_ratio: String,
    pub style: String,
    pub custom_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionStage {
    pub name: String,
    pub skill_ref: String,
    pub tool_ref: String,
    pub description: String,
    pub required_artifacts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProductionPlan {
    pub task_id: String,
    pub pipeline_name: String,
    pub stages: Vec<ProductionStage>,
    pub workspace_dir: String,
}

/// Orchestrates multi-stage video generation within the secure enclave.
pub struct StudioPipelineEngine {
    enclave: VideoStudioEnclave,
}

impl StudioPipelineEngine {
    pub fn new(enclave: VideoStudioEnclave) -> Self {
        Self { enclave }
    }

    /// Generate an actionable multi-stage production plan mapped to OpenMontage skills and tools.
    pub fn plan_production(
        &self,
        req: &VideoProductionRequest,
        token: &str,
    ) -> Result<VideoProductionPlan> {
        self.enclave
            .access_controller()
            .verify_access(OSMOO_OPERATOR_IDENTITY, token)?;

        let stages = match req.pipeline_name.as_str() {
            "screen-demo" => vec![
                ProductionStage {
                    name: "Script & Scenario Planning".to_string(),
                    skill_ref: "pipelines/screen-demo/script-director".to_string(),
                    tool_ref: "tools/analysis/scene_detector.py".to_string(),
                    description: "Generate structured storyboard, UI focus bounding boxes, and narrative voiceover.".to_string(),
                    required_artifacts: vec!["script.json".to_string(), "storyboard.json".to_string()],
                },
                ProductionStage {
                    name: "Screen Capture & Asset Slicing".to_string(),
                    skill_ref: "pipelines/screen-demo/asset-director".to_string(),
                    tool_ref: "tools/capture/screen_capture.py".to_string(),
                    description: "Record application interactions and extract high-DPI keyframes.".to_string(),
                    required_artifacts: vec!["raw_capture.mp4".to_string(), "key_crops/".to_string()],
                },
                ProductionStage {
                    name: "Audio Synthesis & Loudness Normalization".to_string(),
                    skill_ref: "pipelines/screen-demo/compose-director".to_string(),
                    tool_ref: "tools/audio/voiceover.py".to_string(),
                    description: "Synthesize voiceover, align speech timestamps, and generate ducked background audio.".to_string(),
                    required_artifacts: vec!["voiceover.wav".to_string(), "timed_transcription.json".to_string()],
                },
                ProductionStage {
                    name: "Subtitle & Motion Graphic Composition".to_string(),
                    skill_ref: "pipelines/screen-demo/edit-director".to_string(),
                    tool_ref: "remotion-composer".to_string(),
                    description: "Assemble Remotion compositions with animated cursor paths, zoom-ins, and kinetic typography.".to_string(),
                    required_artifacts: vec!["composition.tsx".to_string()],
                },
                ProductionStage {
                    name: "Final Master Render & Quality Gate".to_string(),
                    skill_ref: "pipelines/screen-demo/publish-director".to_string(),
                    tool_ref: "tools/video/ffmpeg_render.py".to_string(),
                    description: "Render master MP4 via NVENC/CPU FFmpeg and perform automated black-frame/freeze checks.".to_string(),
                    required_artifacts: vec!["output_master.mp4".to_string()],
                },
            ],
            _ => vec![
                ProductionStage {
                    name: "Concept & Narrative Scripting".to_string(),
                    skill_ref: "core/script-director".to_string(),
                    tool_ref: "tools/analysis/narrative.py".to_string(),
                    description: format!("Generate 3-act narrative script for topic: {}", req.topic),
                    required_artifacts: vec!["script.json".to_string()],
                },
                ProductionStage {
                    name: "Visual Asset Synthesis & B-Roll".to_string(),
                    skill_ref: "creative/broll-planning".to_string(),
                    tool_ref: "tools/graphics/asset_gen.py".to_string(),
                    description: "Plan and synthesize thematic visual assets according to requested style.".to_string(),
                    required_artifacts: vec!["assets/".to_string()],
                },
                ProductionStage {
                    name: "Audio & Subtitle Alignment".to_string(),
                    skill_ref: "core/subtitle-sync".to_string(),
                    tool_ref: "tools/subtitle/align.py".to_string(),
                    description: "Generate speech tokens and calibrate subtitle pacing.".to_string(),
                    required_artifacts: vec!["subtitles.srt".to_string(), "audio_mix.wav".to_string()],
                },
                ProductionStage {
                    name: "Timeline Assembly & Render".to_string(),
                    skill_ref: "core/ffmpeg".to_string(),
                    tool_ref: "tools/video/renderer.py".to_string(),
                    description: "Render composite video stream with Remotion/FFmpeg.".to_string(),
                    required_artifacts: vec!["final_video.mp4".to_string()],
                },
            ],
        };

        let task_workspace = self
            .enclave
            .workspace_path()
            .join(&req.task_id)
            .to_string_lossy()
            .to_string();

        Ok(VideoProductionPlan {
            task_id: req.task_id.clone(),
            pipeline_name: req.pipeline_name.clone(),
            stages,
            workspace_dir: task_workspace,
        })
    }
}
