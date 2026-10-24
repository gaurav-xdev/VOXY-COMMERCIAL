pub mod activation;
pub mod benchmarks;
pub mod capabilities;
pub mod code_review;
pub mod config;
pub mod engineering_workflow;
pub mod error;
pub mod event;
pub mod language_profiles;
pub mod runtime;
#[cfg(test)]
mod tests;
pub mod toolchains;
pub mod tools;
pub mod traits;
pub mod types;
pub mod workflow;

pub use activation::{
    ActivationDecision, ActivationEngine, ActiveCapabilityContext, CapabilityRiskLevel,
    CheckStatus, EngineeringCheckItem, ExternalCapability, ExternalCapabilityCatalog,
    ExternalCapabilityCategory, SkillExecutionRecord, SkillLifecycleEvent, TaskContext, TaskType,
};
pub use benchmarks::{
    BenchmarkCaseResult, BenchmarkScorecard, BenchmarkTaskCategory, BenchmarkTestCase,
    MultiLanguageBenchmarkHarness,
};
pub use capabilities::{
    CapabilityDescriptor, CapabilityId, CapabilityRegistry, InMemoryCapabilityRegistry,
};
pub use code_review::{
    CodeReviewFinding, CodeReviewReport, DefectCategory, FindingSeverity, IndependentCodeReviewer,
};
pub use config::SkillsConfig;
pub use engineering_workflow::{
    EngineeringTaskSpec, ProductionEngineeringWorkflow, StageRecord, WorkflowExecutionSummary,
    WorkflowStage,
};
pub use error::{Result, SkillsError};
pub use event::SkillsEvent;
pub use language_profiles::{
    LanguageEvidence, LanguageProfile, LanguageProfileRegistry, MultiLanguageDetectionResult,
    RepositoryLanguageDetector, SupportedLanguage,
};
pub use runtime::SkillRuntime;
pub use toolchains::{
    ToolAvailabilityInfo, ToolchainCommandSpec, ToolchainRegistry, VerificationExecutionResult,
    VerificationOp, VerificationStatus,
};
pub use tools::{
    OsmooBenchmarkRunTool, OsmooCodeReviewDiffTool, OsmooEngineeringWorkflowRunTool,
    OsmooLanguageProfileGetTool, OsmooProjectDetectLanguagesTool, OsmooSkillsDiscoverTool,
    OsmooSkillsEvaluateTool, OsmooSkillsLoadInstructionsTool, OsmooSkillsVerifyExecutionTool,
    OsmooToolchainsInspectTool,
};
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
    pub use crate::language_profiles::{LanguageProfileRegistry, SupportedLanguage};
    pub use crate::tools::{
        OsmooBenchmarkRunTool, OsmooCodeReviewDiffTool, OsmooEngineeringWorkflowRunTool,
        OsmooLanguageProfileGetTool, OsmooProjectDetectLanguagesTool, OsmooSkillsDiscoverTool,
        OsmooSkillsEvaluateTool, OsmooSkillsLoadInstructionsTool, OsmooSkillsVerifyExecutionTool,
        OsmooToolchainsInspectTool,
    };
    pub use crate::traits::{Skill, SkillContext, SkillInput, SkillOutput, SkillRegistry};
    pub use crate::types::{InvocationId, SkillId};
}
