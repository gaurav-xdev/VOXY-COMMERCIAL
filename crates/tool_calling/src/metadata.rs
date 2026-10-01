//! Structured tool metadata, risk tiers, and execution context.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Tool risk classification tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    /// Safe, read-only operation with zero side effects.
    Read,
    /// Low-risk state change (e.g. focusing window, minimizing).
    LowRisk,
    /// Moderate modification (e.g. creating/modifying file, resizing window).
    Modify,
    /// Elevated execution (e.g. executing sandboxed command, launching process).
    Privileged,
    /// Destructive operation requiring explicit confirmation (e.g. deleting files, killing processes).
    Destructive,
}

impl RiskTier {
    /// Returns whether this tier requires explicit human confirmation by default.
    pub fn requires_confirmation_by_default(&self) -> bool {
        matches!(self, Self::Privileged | Self::Destructive)
    }
}

/// Category of capability provided by the tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    System,
    Window,
    Filesystem,
    Process,
    Coding,
    Browser,
    Network,
    Custom,
}

/// Metadata descriptor for a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub risk_tier: RiskTier,
    pub parameters_schema: serde_json::Value,
    pub timeout_ms: u64,
    pub requires_confirmation: bool,
    pub supports_rollback: bool,
}

impl ToolMetadata {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        category: ToolCategory,
        risk_tier: RiskTier,
    ) -> Self {
        let requires_confirmation = risk_tier.requires_confirmation_by_default();
        Self {
            name: name.into(),
            description: description.into(),
            category,
            risk_tier,
            parameters_schema: serde_json::json!({ "type": "object" }),
            timeout_ms: 15_000,
            requires_confirmation,
            supports_rollback: false,
        }
    }

    pub fn with_schema(mut self, schema: serde_json::Value) -> Self {
        self.parameters_schema = schema;
        self
    }

    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn with_confirmation(mut self, required: bool) -> Self {
        self.requires_confirmation = required;
        self
    }

    pub fn with_rollback(mut self, supported: bool) -> Self {
        self.supports_rollback = supported;
        self
    }
}

/// Context passed to a tool during invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolContext {
    pub session_id: String,
    pub user_confirmed: bool,
    pub working_dir: Option<PathBuf>,
}

impl ToolContext {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            user_confirmed: false,
            working_dir: None,
        }
    }

    pub fn with_confirmation(mut self, confirmed: bool) -> Self {
        self.user_confirmed = confirmed;
        self
    }

    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }
}

/// Output of a tool execution with verification and observation telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
    pub observation: Option<String>,
    pub verification: Option<String>,
    pub duration_ms: u64,
    pub rollback_token: Option<String>,
}

impl ToolResult {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            success: true,
            data,
            error: None,
            observation: None,
            verification: None,
            duration_ms: 0,
            rollback_token: None,
        }
    }

    pub fn failure(err: impl Into<String>) -> Self {
        Self {
            success: false,
            data: serde_json::Value::Null,
            error: Some(err.into()),
            observation: None,
            verification: None,
            duration_ms: 0,
            rollback_token: None,
        }
    }

    pub fn with_observation(mut self, observation: impl Into<String>) -> Self {
        self.observation = Some(observation.into());
        self
    }

    pub fn with_verification(mut self, verification: impl Into<String>) -> Self {
        self.verification = Some(verification.into());
        self
    }

    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn with_rollback_token(mut self, token: impl Into<String>) -> Self {
        self.rollback_token = Some(token.into());
        self
    }
}
