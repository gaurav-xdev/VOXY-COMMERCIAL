use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkillId(pub String);

impl SkillId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InvocationId(pub String);

impl InvocationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InvocationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trust classification tier for skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillTrustLevel {
    System,
    Verified,
    Published,
    Untrusted,
    Blocked,
}

impl Default for SkillTrustLevel {
    fn default() -> Self {
        Self::Untrusted
    }
}

/// Explicit permission categories required by skills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillPermission {
    ReadFiles,
    WriteFiles,
    DeleteFiles,
    BrowserAccess,
    NetworkAccess,
    ProcessControl,
    ComputerControl,
    MemoryRead,
    MemoryWrite,
    ResearchAccess,
    CodeHarnessAccess,
    TaskHistoryAccess,
    NotifyUser,
}

/// Production Skill Manifest describing capabilities, permissions, and integrity.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillManifest {
    pub skill_id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub publisher_id: String,
    #[serde(default)]
    pub trust_level: SkillTrustLevel,
    #[serde(default)]
    pub permissions: Vec<SkillPermission>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub inputs_schema: serde_json::Value,
    #[serde(default)]
    pub outputs_schema: serde_json::Value,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_max_steps")]
    pub max_steps: usize,
    #[serde(default)]
    pub checksum: String,
}

fn default_timeout_seconds() -> u64 {
    60
}

fn default_max_steps() -> usize {
    10
}

impl SkillManifest {
    pub fn validate(&self) -> Result<(), String> {
        if self.skill_id.is_empty() {
            return Err("skill_id cannot be empty".into());
        }
        if self.name.is_empty() {
            return Err("name cannot be empty".into());
        }
        if self.version.is_empty() {
            return Err("version cannot be empty".into());
        }
        if self.trust_level == SkillTrustLevel::Blocked {
            return Err("skill is explicitly blocked".into());
        }
        Ok(())
    }

    pub fn has_permission(&self, permission: SkillPermission) -> bool {
        self.permissions.contains(&permission)
    }
}

/// Workflow Step Definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowStep {
    pub step_id: String,
    pub name: String,
    pub required_skill_or_tool: String,
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub requires_approval: bool,
}

/// Orchestrated Workflow Definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowDefinition {
    pub workflow_id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub steps: Vec<WorkflowStep>,
    #[serde(default = "default_timeout_seconds")]
    pub timeout_seconds: u64,
}

