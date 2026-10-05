//! Deep research tools: investigation, multi-source extraction, and evidence synthesis.

use async_trait::async_trait;
use serde_json::json;
use std::time::Instant;

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;

use voxy_grounding::{ResearchConfig, ResearchOrchestrator};

/// Deep investigation tool executing multi-source research with prompt-injection defense.
pub struct ResearchInvestigateTool {
    metadata: ToolMetadata,
}

impl ResearchInvestigateTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "research_deep_investigate",
                "Execute an evidence-driven research investigation with query decomposition and cross-source synthesis",
                ToolCategory::System,
                RiskTier::Read,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["objective"],
                "properties": {
                    "objective": { "type": "string", "description": "High-level research topic or question" },
                    "sources": {
                        "type": "array",
                        "description": "Optional list of initial sources (tuples of url and content)",
                        "items": {
                            "type": "object",
                            "required": ["url", "content"],
                            "properties": {
                                "url": { "type": "string" },
                                "content": { "type": "string" }
                            }
                        }
                    }
                }
            })),
        }
    }
}

impl Default for ResearchInvestigateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ResearchInvestigateTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        let start = Instant::now();
        let objective = params
            .get("objective")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'objective'".into()))?;

        let mut raw_sources = Vec::new();
        if let Some(sources_arr) = params.get("sources").and_then(|v| v.as_array()) {
            for item in sources_arr {
                if let (Some(url), Some(content)) = (
                    item.get("url").and_then(|v| v.as_str()),
                    item.get("content").and_then(|v| v.as_str()),
                ) {
                    raw_sources.push((url, content));
                }
            }
        }

        let mut orchestrator = ResearchOrchestrator::new(ResearchConfig::default());
        let report = orchestrator
            .execute_research(objective, raw_sources)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Research failed: {}", e)))?;

        Ok(ToolResult::success(json!({
            "objective": report.objective,
            "executive_summary": report.executive_summary,
            "has_contradictions": report.has_contradictions,
            "total_sources_analyzed": report.total_sources_analyzed,
            "claims": report.claims,
            "citations": report.citations,
        }))
        .with_observation(format!(
            "Research completed on '{}': {} sources analyzed, contradictions: {}",
            report.objective, report.total_sources_analyzed, report.has_contradictions
        ))
        .with_duration_ms(start.elapsed().as_millis() as u64))
    }
}
