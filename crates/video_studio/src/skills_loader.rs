use crate::enclave::VideoStudioEnclave;
use crate::error::Result;
use voxy_skills::{CapabilityDescriptor, CapabilityId, CapabilityRegistry};

/// Loads OpenMontage skills into OSMOO's internal capability registry.
pub struct OpenMontageSkillsLoader;

impl OpenMontageSkillsLoader {
    /// Register all core, creative, and pipeline skills from the video studio enclave.
    pub async fn register_enclave_capabilities(
        enclave: &VideoStudioEnclave,
        registry: &(dyn CapabilityRegistry + Send + Sync),
        token: &str,
    ) -> Result<usize> {
        let skills = enclave.list_skills(token)?;
        let mut count = 0;

        for skill_name in skills {
            let cap_id = CapabilityId(format!("video_studio.{}", skill_name));
            let descriptor = CapabilityDescriptor {
                id: cap_id,
                name: format!("Video Production Skill: {}", skill_name),
                description: format!(
                    "Internal OpenMontage skill for video creation: {}. Accessible exclusively to OSMOO.",
                    skill_name
                ),
                version: "1.0.0".to_string(),
                provider_hint: Some("OSMOO_VIDEO_STUDIO_ENCLAVE".to_string()),
            };

            if registry.register_capability(descriptor).await.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }
}
