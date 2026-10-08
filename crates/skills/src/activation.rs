//! Task-based activation engine, external capability registry, and boundary enforcement.
//!
//! Principle: AVAILABLE ≠ ACTIVE
//! Capabilities are indexed and available offline, but activated strictly when
//! a task demands them and the execution context satisfies the capability's
//! required permissions and network constraints.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::debug;

use crate::error::{Result, SkillsError};
use crate::types::SkillPermission;

/// Categories of indexed external capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalCapabilityCategory {
    GeneralEngineering,
    FrontendUi,
    WorkflowOrchestration,
    CodingTools,
    WebResearch,
}

/// Risk level required for executing a capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRiskLevel {
    Read,
    Modify,
    Privileged,
    Destructive,
}

/// Metadata description of an external capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCapability {
    pub id: String,
    pub name: String,
    pub category: ExternalCapabilityCategory,
    pub source_repo: String,
    pub version_pinned: String,
    pub description: String,
    pub trigger_tags: Vec<String>,
    pub required_permissions: Vec<SkillPermission>,
    pub network_required: bool,
    pub risk_level: CapabilityRiskLevel,
    pub doc_path: String,
    pub enabled: bool,
}

/// Catalog file structure for offline-available capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalCapabilityCatalog {
    pub version: String,
    pub last_updated: String,
    pub total_capabilities: usize,
    pub capabilities: Vec<ExternalCapability>,
}

impl ExternalCapabilityCatalog {
    /// Load catalog from JSON string.
    pub fn from_json_str(json: &str) -> Result<Self> {
        serde_json::from_str(json)
            .map_err(|e| SkillsError::InvalidConfig(format!("Failed to parse catalog: {}", e)))
    }

    /// Load the embedded built-in catalog.
    pub fn load_embedded() -> Self {
        const EMBEDDED_CATALOG_JSON: &str = include_str!("../catalog/skills_manifest.json");
        Self::from_json_str(EMBEDDED_CATALOG_JSON)
            .expect("Embedded skills manifest must always be valid JSON")
    }
}

/// Intent or task submitted to the agent for execution.
#[derive(Debug, Clone)]
pub struct TaskContext {
    pub task_id: String,
    pub prompt: String,
    pub tags: Vec<String>,
    pub target_category: Option<ExternalCapabilityCategory>,
    pub workspace_root: PathBuf,
    pub network_enabled: bool,
    pub granted_permissions: HashSet<SkillPermission>,
    pub max_risk_level: CapabilityRiskLevel,
}

impl TaskContext {
    pub fn new(task_id: impl Into<String>, prompt: impl Into<String>, workspace: impl Into<PathBuf>) -> Self {
        let mut permissions = HashSet::new();
        // Base read permission is standard for tasks
        permissions.insert(SkillPermission::ReadFiles);

        Self {
            task_id: task_id.into(),
            prompt: prompt.into(),
            tags: Vec::new(),
            target_category: None,
            workspace_root: workspace.into(),
            network_enabled: false,
            granted_permissions: permissions,
            max_risk_level: CapabilityRiskLevel::Read,
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for t in tags {
            self.tags.push(t.into());
        }
        self
    }

    pub fn with_permission(mut self, perm: SkillPermission) -> Self {
        self.granted_permissions.insert(perm);
        self
    }

    pub fn with_network(mut self, network: bool) -> Self {
        self.network_enabled = network;
        self
    }

    pub fn with_max_risk(mut self, risk: CapabilityRiskLevel) -> Self {
        self.max_risk_level = risk;
        self
    }

    pub fn with_category(mut self, category: ExternalCapabilityCategory) -> Self {
        self.target_category = Some(category);
        self
    }
}

/// Result of evaluating an activation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationDecision {
    pub capability_id: String,
    pub activated: bool,
    pub reason: String,
}

/// Active execution context populated with only the activated capabilities.
#[derive(Debug, Clone)]
pub struct ActiveCapabilityContext {
    pub active_capabilities: Vec<ExternalCapability>,
    pub rejected_capabilities: Vec<ActivationDecision>,
}

/// Central Activation Engine evaluating tasks against available external capabilities.
pub struct ActivationEngine {
    catalog: ExternalCapabilityCatalog,
    catalog_root: PathBuf,
}

impl ActivationEngine {
    /// Create engine with embedded catalog.
    pub fn new_embedded() -> Self {
        Self {
            catalog: ExternalCapabilityCatalog::load_embedded(),
            catalog_root: PathBuf::from("crates/skills/catalog"),
        }
    }

    /// Create engine with custom catalog and root path.
    pub fn new(catalog: ExternalCapabilityCatalog, catalog_root: PathBuf) -> Self {
        Self {
            catalog,
            catalog_root,
        }
    }

    /// Get total number of capabilities currently available in the index.
    pub fn total_available(&self) -> usize {
        self.catalog.capabilities.len()
    }

    /// List all indexed capabilities.
    pub fn list_available(&self) -> &[ExternalCapability] {
        &self.catalog.capabilities
    }

    /// Read documentation/manifest content for an active capability.
    pub fn read_capability_doc(&self, capability: &ExternalCapability) -> Result<String> {
        let path = self.catalog_root.join(&capability.doc_path);
        if path.exists() {
            std::fs::read_to_string(&path)
                .map_err(|e| SkillsError::SkillExecutionFailed(format!("Failed to read capability doc {:?}: {}", path, e)))
        } else {
            Err(SkillsError::SkillNotFound(format!("Capability doc not found at {:?}", path)))
        }
    }

    /// Classify a task prompt and tags to detect relevant trigger tags.
    pub fn extract_tags_from_prompt(&self, prompt: &str) -> HashSet<String> {
        let mut extracted = HashSet::new();
        let prompt_lower = prompt.to_lowercase();

        for cap in &self.catalog.capabilities {
            for tag in &cap.trigger_tags {
                // Word boundary check or substring check for known trigger tags
                if prompt_lower.contains(tag) {
                    extracted.insert(tag.clone());
                }
            }
        }

        extracted
    }

    /// Match and activate capabilities strictly demanded by the task context.
    ///
    /// Evaluates:
    /// 1. Is the capability enabled?
    /// 2. If target category is specified, does it match?
    /// 3. Does the prompt or tags contain any of the capability's trigger tags?
    /// 4. Does the context have network enabled if the capability requires network?
    /// 5. Does the context possess all required permissions?
    /// 6. Does the context permit the capability's risk level?
    pub fn evaluate_task(&self, task: &TaskContext) -> ActiveCapabilityContext {
        let mut active = Vec::new();
        let mut rejected = Vec::new();

        let prompt_tags = self.extract_tags_from_prompt(&task.prompt);
        let mut combined_tags: HashSet<String> = task.tags.iter().map(|t| t.to_lowercase()).collect();
        combined_tags.extend(prompt_tags);

        for cap in &self.catalog.capabilities {
            if !cap.enabled {
                rejected.push(ActivationDecision {
                    capability_id: cap.id.clone(),
                    activated: false,
                    reason: "Capability is disabled in catalog".into(),
                });
                continue;
            }

            // Category filter if caller explicitly requested one
            if let Some(target_cat) = task.target_category {
                if cap.category != target_cat {
                    rejected.push(ActivationDecision {
                        capability_id: cap.id.clone(),
                        activated: false,
                        reason: format!(
                            "Category mismatch: requested {:?}, capability is {:?}",
                            target_cat, cap.category
                        ),
                    });
                    continue;
                }
            }

            // Tag/Keyword trigger check
            let matches_trigger = cap
                .trigger_tags
                .iter()
                .any(|t| combined_tags.contains(&t.to_lowercase()));

            if !matches_trigger {
                rejected.push(ActivationDecision {
                    capability_id: cap.id.clone(),
                    activated: false,
                    reason: "No matching trigger tags or keywords found in task".into(),
                });
                continue;
            }

            // Network requirement check
            if cap.network_required && !task.network_enabled {
                rejected.push(ActivationDecision {
                    capability_id: cap.id.clone(),
                    activated: false,
                    reason: "Capability requires network access but task context is offline".into(),
                });
                continue;
            }

            // Permissions check
            let missing_permissions: Vec<_> = cap
                .required_permissions
                .iter()
                .filter(|p| !task.granted_permissions.contains(p))
                .collect();

            if !missing_permissions.is_empty() {
                rejected.push(ActivationDecision {
                    capability_id: cap.id.clone(),
                    activated: false,
                    reason: format!(
                        "Missing required permissions: {:?}",
                        missing_permissions
                    ),
                });
                continue;
            }

            // Risk level check
            if cap.risk_level > task.max_risk_level {
                rejected.push(ActivationDecision {
                    capability_id: cap.id.clone(),
                    activated: false,
                    reason: format!(
                        "Capability risk level {:?} exceeds task allowed risk level {:?}",
                        cap.risk_level, task.max_risk_level
                    ),
                });
                continue;
            }

            // All checks passed -> capability is ACTIVE for this task
            debug!(capability = %cap.id, "Activating external capability for task");
            active.push(cap.clone());
        }

        ActiveCapabilityContext {
            active_capabilities: active,
            rejected_capabilities: rejected,
        }
    }
}
