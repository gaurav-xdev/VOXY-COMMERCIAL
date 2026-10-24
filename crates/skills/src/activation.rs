//! Task-based activation engine, external capability registry, and boundary enforcement.
//!
//! Principle: AVAILABLE ≠ ACTIVE
//! Capabilities are indexed and available offline, but activated strictly when
//! a task demands them and the execution context satisfies the capability's
//! required permissions and network constraints.
//!
//! Upgrades:
//! - Autonomous multi-dimensional skill evaluation (task type, language, roles, risk).
//! - Smallest effective set selection to prevent token bloat and context degradation.
//! - Deterministic conflict resolution across priority tiers.
//! - Required engineering checks extraction and execution evidence verification.
//! - Sanitized instruction loading with attribution and fallback support.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use tracing::{debug, info};

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
    LanguageEngineering,
    SecurityHardening,
    ArchitectureDesign,
    QualityTesting,
    OpenSourcePractices,
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

/// Task classification types for targeted skill selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    General,
    Feature,
    BugFix,
    Refactoring,
    SecurityAudit,
    ArchitectureDesign,
    Documentation,
    PerformanceOptimization,
    FrontendUi,
}

fn default_conflict_priority() -> u32 {
    70
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
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub target_roles: Vec<String>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub required_checks: Vec<String>,
    #[serde(default = "default_conflict_priority")]
    pub conflict_priority: u32,
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

/// Execution check verification status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pending,
    Passed,
    Failed,
    Skipped,
}

/// Concrete engineering check associated with an activated capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineeringCheckItem {
    pub skill_id: String,
    pub check_name: String,
    pub status: CheckStatus,
    pub evidence: Option<String>,
}

/// Result of evaluating an activation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivationDecision {
    pub capability_id: String,
    pub activated: bool,
    pub reason: String,
}

/// Active execution context populated with only the activated capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCapabilityContext {
    pub active_capabilities: Vec<ExternalCapability>,
    pub rejected_capabilities: Vec<ActivationDecision>,
    pub conflict_resolutions: Vec<String>,
    pub required_checks: Vec<EngineeringCheckItem>,
}

/// Record of verified skill execution for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecutionRecord {
    pub task_id: String,
    pub prompt: String,
    pub task_type: TaskType,
    pub active_skills: Vec<String>,
    pub required_checks: Vec<EngineeringCheckItem>,
    pub conflict_resolutions: Vec<String>,
    pub all_passed: bool,
    pub completed_at: Option<String>,
}

/// Intent or task submitted to the agent for execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub task_id: String,
    pub prompt: String,
    pub tags: Vec<String>,
    pub target_category: Option<ExternalCapabilityCategory>,
    pub target_role: Option<String>,
    pub detected_languages: Vec<String>,
    pub task_type: TaskType,
    pub max_active_skills: usize,
    pub workspace_root: PathBuf,
    pub network_enabled: bool,
    pub granted_permissions: HashSet<SkillPermission>,
    pub max_risk_level: CapabilityRiskLevel,
}

impl TaskContext {
    pub fn new(
        task_id: impl Into<String>,
        prompt: impl Into<String>,
        workspace: impl Into<PathBuf>,
    ) -> Self {
        let mut permissions = HashSet::new();
        permissions.insert(SkillPermission::ReadFiles);

        let p: String = prompt.into();
        let inferred_type = Self::infer_task_type(&p);
        let inferred_langs = Self::infer_languages(&p);

        Self {
            task_id: task_id.into(),
            prompt: p,
            tags: Vec::new(),
            target_category: None,
            target_role: None,
            detected_languages: inferred_langs,
            task_type: inferred_type,
            max_active_skills: 4, // Default smallest effective set: 4 skills max
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

    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.detected_languages.push(lang.into().to_lowercase());
        self
    }

    pub fn with_languages<I, S>(mut self, langs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for l in langs {
            self.detected_languages.push(l.into().to_lowercase());
        }
        self
    }

    pub fn with_task_type(mut self, task_type: TaskType) -> Self {
        self.task_type = task_type;
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.target_role = Some(role.into().to_lowercase());
        self
    }

    pub fn with_max_skills(mut self, max: usize) -> Self {
        self.max_active_skills = max;
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

    fn infer_task_type(prompt: &str) -> TaskType {
        let p = prompt.to_lowercase();
        if p.contains("scrape") || p.contains("crawl") || p.contains("firecrawl") {
            TaskType::General
        } else if (p.contains("readme") || p.contains("documentation") || p.contains("docs") || p.contains("adr"))
            && !p.contains("fix") && !p.contains("bug") && !p.contains("concurrency") && !p.contains("refactor")
        {
            TaskType::Documentation
        } else if p.contains("security") || p.contains("audit") || p.contains("threat") || p.contains("vulnerability") || p.contains("secret") || p.contains("ssrf") {
            TaskType::SecurityAudit
        } else if p.contains("bug") || p.contains("crash") || p.contains("panic") || p.contains("fix") || p.contains("investigate") || p.contains("error") {
            TaskType::BugFix
        } else if p.contains("architecture") || p.contains("system design") || p.contains("boundary") || p.contains("microservice") {
            TaskType::ArchitectureDesign
        } else if p.contains("perf") || p.contains("optimize") || p.contains("bottleneck") || p.contains("allocation") {
            TaskType::PerformanceOptimization
        } else if p.contains("ui") || p.contains("react") || p.contains("modal") || p.contains("accessibility") || p.contains("frontend") {
            TaskType::FrontendUi
        } else if p.contains("refactor") || p.contains("simplify") || p.contains("clean up") || p.contains("dead code") {
            TaskType::Refactoring
        } else {
            TaskType::General
        }
    }

    fn infer_languages(prompt: &str) -> Vec<String> {
        let p = prompt.to_lowercase();
        let mut langs = Vec::new();
        if p.contains("rust") || p.contains("cargo") || p.contains("tokio") || p.contains("clippy") {
            langs.push("rust".into());
        }
        if p.contains("python") || p.contains("pytest") || p.contains("pip") {
            langs.push("python".into());
        }
        if p.contains("typescript") || p.contains("react") || p.contains("javascript") || p.contains("npm") {
            langs.push("typescript".into());
            langs.push("javascript".into());
        }
        if p.contains("c++") || p.contains("cpp") || p.contains("clang") {
            langs.push("cpp".into());
            langs.push("c".into());
        }
        if p.contains("golang") || p.contains("go ") || p.contains("goroutine") {
            langs.push("go".into());
        }
        langs
    }
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

    /// Find capability by ID.
    pub fn find_capability(&self, id: &str) -> Option<&ExternalCapability> {
        self.catalog.capabilities.iter().find(|c| c.id == id)
    }

    /// Read documentation/manifest content for an active capability.
    pub fn read_capability_doc(&self, capability: &ExternalCapability) -> Result<String> {
        let path = self.catalog_root.join(&capability.doc_path);
        if path.exists() {
            std::fs::read_to_string(&path).map_err(|e| {
                SkillsError::SkillExecutionFailed(format!(
                    "Failed to read capability doc {:?}: {}",
                    path, e
                ))
            })
        } else {
            // Provide clean structured fallback if file is not on disk in current directory
            Ok(self.generate_fallback_doc(capability))
        }
    }

    /// Read sanitized instructions suitable for injecting into an agent prompt.
    /// Filters out prompt injections and wraps with verified provenance header.
    pub fn read_sanitized_instructions(&self, capability: &ExternalCapability) -> Result<String> {
        let raw = self.read_capability_doc(capability)?;
        let sanitized = Self::sanitize_prompt_content(&raw);
        let header = format!(
            "<!-- OSMOO Engineering Capability: {} | Author: {} | Upstream: {}@{} | License: {} -->\n\n",
            capability.id,
            capability.author.as_deref().unwrap_or("Unknown Author"),
            capability.source_repo,
            capability.version_pinned,
            capability.license.as_deref().unwrap_or("Attribution Required")
        );
        Ok(format!("{}{}", header, sanitized))
    }

    fn generate_fallback_doc(&self, cap: &ExternalCapability) -> String {
        format!(
            "# {}\n\n**Category**: {:?}\n**Source**: {} ({})\n**License**: {}\n\n## Description\n{}\n\n## Required Verification Checks\n{}\n",
            cap.name,
            cap.category,
            cap.source_repo,
            cap.version_pinned,
            cap.license.as_deref().unwrap_or("Standard Open Source"),
            cap.description,
            cap.required_checks
                .iter()
                .map(|c| format!("- [ ] {}", c))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }

    /// Sanitize raw skill markdown against prompt injection attacks.
    fn sanitize_prompt_content(content: &str) -> String {
        let patterns = [
            "<system>", "</system>",
            "IGNORE ALL PREVIOUS INSTRUCTIONS",
            "SYSTEM INSTRUCTION:",
            "OVERRIDE_SECURITY_BOUNDARY",
        ];
        let mut out = content.to_string();
        for p in &patterns {
            out = out.replace(p, "[REDACTED_INJECTION_PATTERN]");
        }
        out
    }

    /// Classify a task prompt and tags to detect relevant trigger tags.
    pub fn extract_tags_from_prompt(&self, prompt: &str) -> HashSet<String> {
        let mut extracted = HashSet::new();
        let prompt_lower = prompt.to_lowercase();

        for cap in &self.catalog.capabilities {
            for tag in &cap.trigger_tags {
                if prompt_lower.contains(tag) {
                    extracted.insert(tag.clone());
                }
            }
        }

        extracted
    }

    /// Autonomous task evaluation selecting the smallest effective set of skills.
    ///
    /// Principle: Available ≠ Active.
    /// Evaluates:
    /// 1. Category, permission, risk tier, and network constraints.
    /// 2. Language affinity (strict filter to prevent loading Rust skills for Python or vice versa).
    /// 3. Task type affinity (strict filter for documentation tasks).
    /// 4. Tag and role relevance scoring.
    /// 5. Top-K truncation to max_active_skills (smallest effective set).
    /// 6. Deterministic conflict resolution.
    pub fn evaluate_task(&self, task: &TaskContext) -> ActiveCapabilityContext {
        let mut scored_candidates: Vec<(&ExternalCapability, i32)> = Vec::new();
        let mut rejected = Vec::new();

        let prompt_tags = self.extract_tags_from_prompt(&task.prompt);
        let mut combined_tags: HashSet<String> =
            task.tags.iter().map(|t| t.to_lowercase()).collect();
        combined_tags.extend(prompt_tags);

        let prompt_lower = task.prompt.to_lowercase();

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
                    reason: format!("Missing required permissions: {:?}", missing_permissions),
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

            // Documentation-only task guardrail:
            // Small docs changes must NOT load heavy engineering skills (concurrency, rust, k8s, etc.)
            if task.task_type == TaskType::Documentation {
                let is_doc_relevant = cap.category == ExternalCapabilityCategory::ArchitectureDesign
                    || cap.id.contains("docs")
                    || cap.id.contains("adrs")
                    || cap.id.contains("senior-open-source");

                if !is_doc_relevant {
                    rejected.push(ActivationDecision {
                        capability_id: cap.id.clone(),
                        activated: false,
                        reason: "Task is documentation-only: heavy engineering skill excluded".into(),
                    });
                    continue;
                }
            }

            // Language match & cross-language conflict exclusion
            let mut language_score = 0;
            if !cap.languages.is_empty() {
                if !task.detected_languages.is_empty() {
                    let has_matching_lang = cap
                        .languages
                        .iter()
                        .any(|l| task.detected_languages.contains(l));

                    if has_matching_lang {
                        language_score += 60;
                    } else {
                        // Capability requires a specific language, but task is for a different language
                        rejected.push(ActivationDecision {
                            capability_id: cap.id.clone(),
                            activated: false,
                            reason: format!(
                                "Language mismatch: capability requires {:?}, task uses {:?}",
                                cap.languages, task.detected_languages
                            ),
                        });
                        continue;
                    }
                }
            }

            // Calculate relevance score
            let mut score = language_score;

            // Trigger tag matches
            let matching_tag_count = cap
                .trigger_tags
                .iter()
                .filter(|t| combined_tags.contains(&t.to_lowercase()))
                .count();

            if matching_tag_count == 0 && language_score == 0 && !prompt_lower.contains(&cap.name.to_lowercase()) {
                rejected.push(ActivationDecision {
                    capability_id: cap.id.clone(),
                    activated: false,
                    reason: "No matching trigger tags, language affinity, or keywords found".into(),
                });
                continue;
            }

            score += (matching_tag_count as i32) * 12;

            // Task type affinity boost
            match task.task_type {
                TaskType::SecurityAudit if cap.category == ExternalCapabilityCategory::SecurityHardening => {
                    score += 45;
                }
                TaskType::BugFix
                    if cap.category == ExternalCapabilityCategory::QualityTesting
                        || cap.id.contains("debug")
                        || cap.id.contains("tdd") =>
                {
                    score += 40;
                }
                TaskType::ArchitectureDesign
                    if cap.category == ExternalCapabilityCategory::ArchitectureDesign =>
                {
                    score += 45;
                }
                TaskType::FrontendUi
                    if cap.category == ExternalCapabilityCategory::FrontendUi =>
                {
                    score += 45;
                }
                TaskType::Refactoring
                    if cap.id.contains("simplification") || cap.category == ExternalCapabilityCategory::QualityTesting =>
                {
                    score += 35;
                }
                _ => {}
            }

            // Senior open source engineer boost for engineering tasks
            if cap.id == "oss:senior-open-source-engineer" {
                score += 30;
            }

            // Role match boost
            if let Some(ref role) = task.target_role {
                if cap.target_roles.iter().any(|r| r == role) {
                    score += 25;
                }
            }

            // Keyword boost in title
            if prompt_lower.contains(&cap.name.to_lowercase()) {
                score += 50;
            }

            scored_candidates.push((cap, score));
        }

        // Sort deterministically:
        // 1. Relevance Score (descending)
        // 2. Conflict Priority (descending)
        // 3. Capability ID (ascending - deterministic tiebreaker)
        scored_candidates.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.0.conflict_priority.cmp(&a.0.conflict_priority))
                .then_with(|| a.0.id.cmp(&b.0.id))
        });

        // Smallest effective set selection: take up to max_active_skills
        let mut active = Vec::new();
        let mut required_checks = Vec::new();
        let mut conflict_resolutions = Vec::new();

        for (idx, (cap, score)) in scored_candidates.into_iter().enumerate() {
            if idx < task.max_active_skills {
                debug!(capability = %cap.id, score = score, "Activating capability for task");
                active.push((*cap).clone());

                for chk in &cap.required_checks {
                    if !required_checks.iter().any(|c: &EngineeringCheckItem| c.skill_id == cap.id && c.check_name == *chk) {
                        required_checks.push(EngineeringCheckItem {
                            skill_id: cap.id.clone(),
                            check_name: chk.clone(),
                            status: CheckStatus::Pending,
                            evidence: None,
                        });
                    }
                }
            } else {
                rejected.push(ActivationDecision {
                    capability_id: cap.id.clone(),
                    activated: false,
                    reason: format!(
                        "Superseded: score {} below cutoff for smallest effective set (max {})",
                        score, task.max_active_skills
                    ),
                });
            }
        }

        // Deterministic Conflict Resolution check
        // Check for minimal-diff (priority 90) vs any expansive instructions
        let has_minimal_diff_rule = active.iter().any(|c| c.conflict_priority >= 90);
        if has_minimal_diff_rule {
            conflict_resolutions.push(
                "Conflict Resolution: Senior Open Source Engineer minimal-diff & repository-adaptation rules (priority 90) supersede speculative rewrites or expansive additions."
                    .to_string(),
            );
        }

        ActiveCapabilityContext {
            active_capabilities: active,
            rejected_capabilities: rejected,
            conflict_resolutions,
            required_checks,
        }
    }

    /// Initialize a new execution record to track verified skill application.
    pub fn create_execution_record(
        &self,
        task: &TaskContext,
        ctx: &ActiveCapabilityContext,
    ) -> SkillExecutionRecord {
        SkillExecutionRecord {
            task_id: task.task_id.clone(),
            prompt: task.prompt.clone(),
            task_type: task.task_type,
            active_skills: ctx.active_capabilities.iter().map(|c| c.id.clone()).collect(),
            required_checks: ctx.required_checks.clone(),
            conflict_resolutions: ctx.conflict_resolutions.clone(),
            all_passed: false,
            completed_at: None,
        }
    }

    /// Record verification evidence for a check and determine if all requirements are satisfied.
    pub fn record_check_verification(
        &self,
        record: &mut SkillExecutionRecord,
        skill_id: &str,
        check_name: &str,
        status: CheckStatus,
        evidence: Option<String>,
    ) -> Result<()> {
        let mut found = false;
        for item in &mut record.required_checks {
            if item.skill_id == skill_id && item.check_name == check_name {
                item.status = status;
                item.evidence = evidence.clone();
                found = true;
                break;
            }
        }

        if !found {
            return Err(SkillsError::InvalidInput(format!(
                "Check '{}' for skill '{}' not found in execution record",
                check_name, skill_id
            )));
        }

        // Verify if all required checks are passed or skipped
        record.all_passed = record.required_checks.iter().all(|c| {
            c.status == CheckStatus::Passed || c.status == CheckStatus::Skipped
        });

        if record.all_passed {
            record.completed_at = Some(chrono::Utc::now().to_rfc3339());
            info!(task_id = %record.task_id, "All engineering checks verified for active skills");
        }

        Ok(())
    }

    /// Registers the 4 OSMOO internal skill tools into any ToolRegistry.
    pub fn register_tools(self: &std::sync::Arc<Self>, registry: &voxy_tool_calling::ToolRegistry) {
        use crate::tools::{
            OsmooSkillsDiscoverTool, OsmooSkillsEvaluateTool, OsmooSkillsLoadInstructionsTool,
            OsmooSkillsVerifyExecutionTool,
        };
        let store = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
        registry.register_sync(std::sync::Arc::new(OsmooSkillsDiscoverTool::new(std::sync::Arc::clone(self))));
        registry.register_sync(std::sync::Arc::new(OsmooSkillsEvaluateTool::new(std::sync::Arc::clone(self), std::sync::Arc::clone(&store))));
        registry.register_sync(std::sync::Arc::new(OsmooSkillsLoadInstructionsTool::new(std::sync::Arc::clone(self))));
        registry.register_sync(std::sync::Arc::new(OsmooSkillsVerifyExecutionTool::new(std::sync::Arc::clone(self), store)));
    }
}
