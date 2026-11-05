//! OSMOO Sovereign Video Production Studio & Secure Agent Enclave.
//!
//! Provides an isolated, protected video editing studio integrating OpenMontage
//! pipelines, Remotion/FFmpeg rendering, scriptwriting, voiceover ducking, and subtitle synchronization.
//!
//! # Security Boundary
//! Accessible exclusively to OSMOO's autonomous cognitive brain via cryptographic bearer tokens.
//! Direct user invocations or untrusted external clients are strictly blocked at the access gate.

pub mod enclave;
pub mod error;
pub mod pipeline;
pub mod security;
pub mod skills_loader;
pub mod tools;

pub use enclave::VideoStudioEnclave;
pub use error::{Result, StudioError};
pub use pipeline::{StudioPipelineEngine, VideoProductionPlan, VideoProductionRequest};
pub use security::{EnclaveAccessController, OSMOO_OPERATOR_IDENTITY};
pub use skills_loader::OpenMontageSkillsLoader;
pub use tools::{VideoStudioInspectTool, VideoStudioPipelineTool};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use voxy_skills::{CapabilityRegistry, InMemoryCapabilityRegistry};
    use voxy_tool_calling::{Tool, ToolContext};

    fn get_test_enclave() -> VideoStudioEnclave {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let enclave_dir = manifest_dir.join("enclave");
        VideoStudioEnclave::new(enclave_dir)
    }

    #[test]
    fn test_access_enforcement_blocks_direct_user() {
        let enclave = get_test_enclave();
        let token = enclave.access_controller().session_token();

        // 1. Direct user is rejected
        let user_res = enclave.access_controller().verify_access("user", &token);
        assert!(user_res.is_err());

        // 2. OSMOO operator is accepted
        let osmoo_res = enclave
            .access_controller()
            .verify_access(OSMOO_OPERATOR_IDENTITY, &token);
        assert!(osmoo_res.is_ok());
    }

    #[test]
    fn test_enclave_enumerates_installed_pipelines_and_skills() {
        let enclave = get_test_enclave();
        let token = enclave.access_controller().session_token();

        let pipelines = enclave.list_pipelines(&token).expect("List pipelines");
        assert!(
            pipelines.contains(&"screen-demo".to_string()),
            "Should contain screen-demo"
        );
        assert!(
            pipelines.contains(&"animated-explainer".to_string()),
            "Should contain animated-explainer"
        );

        let skills = enclave.list_skills(&token).expect("List skills");
        assert!(!skills.is_empty(), "Should contain installed skills");
        assert!(
            skills.iter().any(|s| s.contains("ffmpeg"))
                || skills.iter().any(|s| s.contains("remotion")),
            "Should contain core editing skills"
        );
    }

    #[tokio::test]
    async fn test_pipeline_tool_blocks_unauthorized_user() {
        let enclave = get_test_enclave();
        let token = enclave.access_controller().session_token();
        let tool = VideoStudioPipelineTool::new(enclave);
        let ctx = ToolContext::new("test-session");

        // Attempt 1: User tries to call tool directly
        let user_params = json!({
            "task_id": "vid-101",
            "pipeline_name": "screen-demo",
            "topic": "Rust Concurrency",
            "caller_identity": "human_user",
            "enclave_auth_token": token
        });
        let result = tool.execute(user_params, &ctx).await.unwrap();
        assert!(!result.success, "Direct human invocation must fail");
        assert!(result.error.unwrap().contains("ACCESS DENIED"));

        // Attempt 2: OSMOO autonomous operator calls tool
        let osmoo_params = json!({
            "task_id": "vid-101",
            "pipeline_name": "screen-demo",
            "topic": "Rust Concurrency",
            "caller_identity": OSMOO_OPERATOR_IDENTITY,
            "enclave_auth_token": token
        });
        let result = tool.execute(osmoo_params, &ctx).await.unwrap();
        assert!(result.success, "OSMOO operator invocation must succeed");
        assert_eq!(result.data["stages_count"], 5);
    }

    #[tokio::test]
    async fn test_skills_loader_registers_capabilities() {
        let enclave = get_test_enclave();
        let token = enclave.access_controller().session_token();
        let registry = InMemoryCapabilityRegistry::new();

        let registered =
            OpenMontageSkillsLoader::register_enclave_capabilities(&enclave, &registry, &token)
                .await
                .expect("Register capabilities");

        assert!(registered > 0, "Must register OpenMontage skills");
        let all_caps = registry
            .list_capabilities()
            .await
            .expect("List capabilities");
        assert!(all_caps.len() >= registered);
    }
}
