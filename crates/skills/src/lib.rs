pub mod activation;
pub mod capabilities;
pub mod config;
pub mod error;
pub mod event;
pub mod runtime;
#[cfg(test)]
mod tests;
pub mod traits;
pub mod types;
pub mod workflow;

pub use activation::{
    ActivationDecision, ActivationEngine, ActiveCapabilityContext, CapabilityRiskLevel,
    ExternalCapability, ExternalCapabilityCatalog, ExternalCapabilityCategory, TaskContext,
};
pub use capabilities::{
    CapabilityDescriptor, CapabilityId, CapabilityRegistry, InMemoryCapabilityRegistry,
};
pub use config::SkillsConfig;
pub use error::{Result, SkillsError};
pub use event::SkillsEvent;
pub use runtime::SkillRuntime;
pub use traits::{Skill, SkillContext, SkillInput, SkillOutput, SkillRegistry};
pub use types::{
    InvocationId, SkillId, SkillManifest, SkillPermission, SkillTrustLevel, WorkflowDefinition,
    WorkflowStep,
};
pub use workflow::{WorkflowEngine, WorkflowStatus};

pub mod prelude {
    pub use crate::capabilities::{
        CapabilityDescriptor, CapabilityId, CapabilityRegistry, InMemoryCapabilityRegistry,
    };
    pub use crate::config::SkillsConfig;
    pub use crate::error::{Result, SkillsError};
    pub use crate::event::SkillsEvent;
    pub use crate::traits::{Skill, SkillContext, SkillInput, SkillOutput, SkillRegistry};
    pub use crate::types::{InvocationId, SkillId};
}
