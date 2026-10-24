//! OSMOO Internal Tools for Skill Discovery, Evaluation, Instruction Retrieval, and Verification.

use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use voxy_tool_calling::{
    RiskTier, Tool, ToolCategory, ToolContext, ToolMetadata, ToolResult,
};

use crate::activation::{
    ActivationEngine, CheckStatus, SkillExecutionRecord, TaskContext,
};

/// Tool 1: Discover available SOTA, Addy Osmani, and Senior Open Source skills.
pub struct OsmooSkillsDiscoverTool {
    metadata: ToolMetadata,
    engine: Arc<ActivationEngine>,
}

impl OsmooSkillsDiscoverTool {
    pub fn new(engine: Arc<ActivationEngine>) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_skills_discover",
            "Internal skill discovery catalog. Lists available skills filtered by category, language, role, or keyword query.",
            ToolCategory::Coding,
            RiskTier::Read,
        )
        .with_schema(json!({
            "type": "object",
            "properties": {
                "category": { "type": "string", "description": "Optional category filter" },
                "language": { "type": "string", "description": "Optional programming language filter (e.g. rust, python, typescript)" },
                "role": { "type": "string", "description": "Optional role filter (e.g. architect, implementer, qa, security, reviewer)" },
                "query": { "type": "string", "description": "Search keyword or trigger phrase" }
            }
        }));

        Self { metadata, engine }
    }
}

#[async_trait]
impl Tool for OsmooSkillsDiscoverTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> voxy_tool_calling::Result<ToolResult> {
        let category_filter = params.get("category").and_then(|v| v.as_str());
        let language_filter = params.get("language").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
        let role_filter = params.get("role").and_then(|v| v.as_str()).map(|s| s.to_lowercase());
        let query_filter = params.get("query").and_then(|v| v.as_str()).map(|s| s.to_lowercase());

        let all = self.engine.list_available();
        let filtered: Vec<_> = all
            .iter()
            .filter(|c| {
                if let Some(cat) = category_filter {
                    let cat_str = format!("{:?}", c.category).to_lowercase();
                    if !cat_str.contains(&cat.to_lowercase()) {
                        return false;
                    }
                }
                if let Some(ref lang) = language_filter {
                    if !c.languages.is_empty() && !c.languages.contains(lang) {
                        return false;
                    }
                }
                if let Some(ref r) = role_filter {
                    if !c.target_roles.contains(r) {
                        return false;
                    }
                }
                if let Some(ref q) = query_filter {
                    let name_match = c.name.to_lowercase().contains(q);
                    let desc_match = c.description.to_lowercase().contains(q);
                    let tag_match = c.trigger_tags.iter().any(|t| t.to_lowercase().contains(q));
                    if !name_match && !desc_match && !tag_match {
                        return false;
                    }
                }
                true
            })
            .map(|c| {
                json!({
                    "id": c.id,
                    "name": c.name,
                    "category": c.category,
                    "source_repo": c.source_repo,
                    "version_pinned": c.version_pinned,
                    "author": c.author,
                    "license": c.license,
                    "description": c.description,
                    "languages": c.languages,
                    "target_roles": c.target_roles,
                    "required_checks": c.required_checks,
                    "conflict_priority": c.conflict_priority,
                })
            })
            .collect();

        let total_count = filtered.len();
        Ok(ToolResult::success(json!({
            "total_matched": total_count,
            "skills": filtered
        })).with_observation(format!("Found {} matching skills in catalog", total_count)))
    }
}

/// Tool 2: Autonomous evaluation selecting the smallest effective set of skills.
pub struct OsmooSkillsEvaluateTool {
    metadata: ToolMetadata,
    engine: Arc<ActivationEngine>,
    records_store: Arc<Mutex<Vec<SkillExecutionRecord>>>,
}

impl OsmooSkillsEvaluateTool {
    pub fn new(
        engine: Arc<ActivationEngine>,
        records_store: Arc<Mutex<Vec<SkillExecutionRecord>>>,
    ) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_skills_evaluate",
            "Evaluates task prompt and requirements to select the smallest effective set of skills with required checks.",
            ToolCategory::Coding,
            RiskTier::Read,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["task_id", "prompt"],
            "properties": {
                "task_id": { "type": "string", "description": "Unique task identifier" },
                "prompt": { "type": "string", "description": "User intent or engineering task prompt" },
                "tags": { "type": "array", "items": { "type": "string" }, "description": "Optional contextual tags" },
                "language": { "type": "string", "description": "Detected or target programming language" },
                "role": { "type": "string", "description": "Target agent role (architect, implementer, qa, security, reviewer)" },
                "max_skills": { "type": "integer", "description": "Maximum number of active skills (smallest effective set limit)", "default": 4 }
            }
        }));

        Self {
            metadata,
            engine,
            records_store,
        }
    }
}

#[async_trait]
impl Tool for OsmooSkillsEvaluateTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> voxy_tool_calling::Result<ToolResult> {
        let task_id = params["task_id"].as_str().unwrap_or("task-default");
        let prompt = params["prompt"].as_str().unwrap_or("");
        let language = params.get("language").and_then(|v| v.as_str());
        let role = params.get("role").and_then(|v| v.as_str());
        let max_skills = params.get("max_skills").and_then(|v| v.as_u64()).unwrap_or(4) as usize;

        let mut task = TaskContext::new(task_id, prompt, std::env::current_dir().unwrap_or_default())
            .with_max_skills(max_skills);

        if let Some(l) = language {
            task = task.with_language(l);
        }
        if let Some(r) = role {
            task = task.with_role(r);
        }
        if let Some(tags_arr) = params.get("tags").and_then(|v| v.as_array()) {
            for t in tags_arr {
                if let Some(s) = t.as_str() {
                    task = task.with_tag(s);
                }
            }
        }

        let active_ctx = self.engine.evaluate_task(&task);
        let exec_record = self.engine.create_execution_record(&task, &active_ctx);

        // Store execution record
        {
            let mut store = self.records_store.lock().await;
            store.retain(|r| r.task_id != task_id);
            store.push(exec_record.clone());
        }

        let active_summary: Vec<_> = active_ctx
            .active_capabilities
            .iter()
            .map(|c| {
                json!({
                    "id": c.id,
                    "name": c.name,
                    "category": c.category,
                    "priority": c.conflict_priority,
                    "required_checks": c.required_checks,
                })
            })
            .collect();

        Ok(ToolResult::success(json!({
            "task_id": task_id,
            "task_type": task.task_type,
            "active_skills_count": active_ctx.active_capabilities.len(),
            "active_skills": active_summary,
            "conflict_resolutions": active_ctx.conflict_resolutions,
            "required_checks": active_ctx.required_checks,
            "rejected_count": active_ctx.rejected_capabilities.len(),
        })).with_observation(format!(
            "Activated {} focused skills for task (type: {:?})",
            active_ctx.active_capabilities.len(),
            task.task_type
        )))
    }
}

/// Tool 3: Retrieve sanitized instructions and rules for active skills.
pub struct OsmooSkillsLoadInstructionsTool {
    metadata: ToolMetadata,
    engine: Arc<ActivationEngine>,
}

impl OsmooSkillsLoadInstructionsTool {
    pub fn new(engine: Arc<ActivationEngine>) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_skills_load_instructions",
            "Loads sanitized engineering instructions, rules, and checklists for specified skill IDs.",
            ToolCategory::Coding,
            RiskTier::Read,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["skill_ids"],
            "properties": {
                "skill_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of active skill IDs to retrieve instructions for"
                }
            }
        }));

        Self { metadata, engine }
    }
}

#[async_trait]
impl Tool for OsmooSkillsLoadInstructionsTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> voxy_tool_calling::Result<ToolResult> {
        let ids = params.get("skill_ids").and_then(|v| v.as_array());
        if ids.is_none() {
            return Ok(ToolResult::failure("Missing 'skill_ids' array parameter"));
        }

        let mut instructions_map = serde_json::Map::new();
        let mut loaded_count = 0;

        for id_val in ids.unwrap() {
            if let Some(id_str) = id_val.as_str() {
                if let Some(cap) = self.engine.find_capability(id_str) {
                    match self.engine.read_sanitized_instructions(cap) {
                        Ok(instructions) => {
                            instructions_map.insert(id_str.to_string(), json!(instructions));
                            loaded_count += 1;
                        }
                        Err(e) => {
                            instructions_map.insert(
                                id_str.to_string(),
                                json!(format!("Error reading instructions: {}", e)),
                            );
                        }
                    }
                } else {
                    instructions_map.insert(
                        id_str.to_string(),
                        json!("Skill not found in catalog"),
                    );
                }
            }
        }

        Ok(ToolResult::success(json!({
            "loaded_count": loaded_count,
            "instructions": instructions_map
        })).with_observation(format!("Loaded instructions for {} skills", loaded_count)))
    }
}

/// Tool 4: Record verification evidence for engineering checks before completion.
pub struct OsmooSkillsVerifyExecutionTool {
    metadata: ToolMetadata,
    engine: Arc<ActivationEngine>,
    records_store: Arc<Mutex<Vec<SkillExecutionRecord>>>,
}

impl OsmooSkillsVerifyExecutionTool {
    pub fn new(
        engine: Arc<ActivationEngine>,
        records_store: Arc<Mutex<Vec<SkillExecutionRecord>>>,
    ) -> Self {
        let metadata = ToolMetadata::new(
            "osmoo_skills_verify_execution",
            "Records verification evidence for required checks and verifies task completion readiness.",
            ToolCategory::Coding,
            RiskTier::Read,
        )
        .with_schema(json!({
            "type": "object",
            "required": ["task_id", "skill_id", "check_name", "status"],
            "properties": {
                "task_id": { "type": "string", "description": "Target task identifier" },
                "skill_id": { "type": "string", "description": "Skill owning the check" },
                "check_name": { "type": "string", "description": "Name of the check being verified" },
                "status": {
                    "type": "string",
                    "enum": ["passed", "failed", "skipped"],
                    "description": "Verification outcome"
                },
                "evidence": { "type": "string", "description": "Verification proof (command output, test result, diff summary)" }
            }
        }));

        Self {
            metadata,
            engine,
            records_store,
        }
    }
}

#[async_trait]
impl Tool for OsmooSkillsVerifyExecutionTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _ctx: &ToolContext,
    ) -> voxy_tool_calling::Result<ToolResult> {
        let task_id = params["task_id"].as_str().unwrap_or("");
        let skill_id = params["skill_id"].as_str().unwrap_or("");
        let check_name = params["check_name"].as_str().unwrap_or("");
        let status_str = params["status"].as_str().unwrap_or("failed");
        let evidence = params.get("evidence").and_then(|v| v.as_str()).map(|s| s.to_string());

        let check_status = match status_str {
            "passed" => CheckStatus::Passed,
            "skipped" => CheckStatus::Skipped,
            _ => CheckStatus::Failed,
        };

        let mut store = self.records_store.lock().await;
        let record = store.iter_mut().find(|r| r.task_id == task_id);

        if let Some(r) = record {
            if let Err(e) = self.engine.record_check_verification(
                r,
                skill_id,
                check_name,
                check_status,
                evidence,
            ) {
                return Ok(ToolResult::failure(format!("Failed to record check: {}", e)));
            }

            Ok(ToolResult::success(json!({
                "task_id": task_id,
                "all_passed": r.all_passed,
                "completed_at": r.completed_at,
                "remaining_pending": r.required_checks.iter().filter(|c| c.status == CheckStatus::Pending).count(),
                "checks": r.required_checks
            })).with_observation(format!(
                "Check '{}' for '{}' marked as {:?}. Task all_passed={}",
                check_name, skill_id, check_status, r.all_passed
            )))
        } else {
            Ok(ToolResult::failure(format!("No execution record found for task_id '{}'", task_id)))
        }
    }
}
