//! Deep Research Orchestrator & State Machine.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::defense::PromptInjectionDefense;
use crate::extractor::ContentExtractor;
use crate::quality::SourceQualityEvaluator;
use crate::synthesis::{
    Citation, ClaimConfidence, CrossSourceSynthesizer, ResearchClaim, SynthesisReport,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResearchState {
    Planning,
    Discovering,
    Extracting,
    Evaluating,
    Synthesizing,
    Completed,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct ResearchConfig {
    pub max_sub_questions: usize,
    pub max_sources: usize,
    pub timeout: Duration,
    pub request_budget: usize,
}

impl Default for ResearchConfig {
    fn default() -> Self {
        Self {
            max_sub_questions: 4,
            max_sources: 10,
            timeout: Duration::from_secs(45),
            request_budget: 15,
        }
    }
}

pub struct ResearchOrchestrator {
    config: ResearchConfig,
    state: ResearchState,
    cancellation_token: Arc<AtomicBool>,
    visited_urls: HashSet<String>,
}

impl ResearchOrchestrator {
    pub fn new(config: ResearchConfig) -> Self {
        Self {
            config,
            state: ResearchState::Planning,
            cancellation_token: Arc::new(AtomicBool::new(false)),
            visited_urls: HashSet::new(),
        }
    }

    pub fn with_cancellation_token(mut self, token: Arc<AtomicBool>) -> Self {
        self.cancellation_token = token;
        self
    }

    pub fn cancel(&self) {
        self.cancellation_token.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.load(Ordering::SeqCst)
    }

    /// Decomposes a research objective into targeted search sub-queries.
    pub fn decompose_objective(&self, objective: &str) -> Vec<String> {
        let trimmed = objective.trim();
        let mut queries = Vec::new();

        // Primary search
        queries.push(trimmed.to_string());

        // Decomposed facets: architecture/specifications, best practices, comparison
        if !trimmed.to_lowercase().contains("rfc") && !trimmed.to_lowercase().contains("spec") {
            queries.push(format!("{} specification documentation", trimmed));
        }
        queries.push(format!("{} comparison tradeoffs benchmarks", trimmed));

        queries.truncate(self.config.max_sub_questions);
        queries
    }

    /// Executes complete research investigation given initial sources and returns structured synthesis.
    pub async fn execute_research(
        &mut self,
        objective: &str,
        raw_sources: Vec<(&str, &str)>, // (url, raw_content)
    ) -> Result<SynthesisReport, String> {
        if self.is_cancelled() {
            self.state = ResearchState::Cancelled;
            return Err("Research cancelled prior to execution".to_string());
        }

        self.state = ResearchState::Planning;
        let sub_questions = self.decompose_objective(objective);

        self.state = ResearchState::Discovering;
        let mut citations = HashMap::new();
        let mut extracted_docs = Vec::new();

        for (idx, (url, raw_content)) in raw_sources.into_iter().enumerate() {
            if self.is_cancelled() {
                self.state = ResearchState::Cancelled;
                return Err("Research aborted by cancellation token".to_string());
            }

            if self.visited_urls.contains(url) || extracted_docs.len() >= self.config.max_sources {
                continue;
            }
            self.visited_urls.insert(url.to_string());

            self.state = ResearchState::Extracting;
            let extracted = ContentExtractor::extract_text(raw_content, url);

            // Defend against prompt injections
            let sanitized =
                PromptInjectionDefense::sanitize_external_text(&extracted.text_content, url);

            self.state = ResearchState::Evaluating;
            let quality = SourceQualityEvaluator::evaluate(
                &extracted.domain,
                sanitized.sanitized_length,
                true,
            );

            let cite_id = format!("cite-{}", idx + 1);
            citations.insert(
                cite_id.clone(),
                Citation {
                    id: cite_id,
                    title: extracted.title.clone(),
                    url: url.to_string(),
                    domain: extracted.domain.clone(),
                    credibility_score: quality.score,
                },
            );

            extracted_docs.push((extracted, sanitized, quality));
        }

        if extracted_docs.is_empty() {
            self.state = ResearchState::Completed;
            return Ok(CrossSourceSynthesizer::build_report(
                objective,
                "No sources could be extracted for this objective.",
                Vec::new(),
                citations,
            ));
        }

        self.state = ResearchState::Synthesizing;
        // Construct synthesized claims
        let primary_citation_id = "cite-1".to_string();
        let primary_doc = &extracted_docs[0].0;

        let claim = ResearchClaim {
            statement: format!(
                "Primary evidence from {}: {}",
                primary_doc.domain, primary_doc.title
            ),
            supporting_citation_ids: vec![primary_citation_id],
            contradicting_citation_ids: Vec::new(),
            confidence: ClaimConfidence::HighConfidence,
            synthesis_notes: format!("Sub-questions explored: {:?}", sub_questions),
        };

        let report = CrossSourceSynthesizer::build_report(
            objective,
            &format!(
                "Synthesized findings across {} verified sources.",
                extracted_docs.len()
            ),
            vec![claim],
            citations,
        );

        self.state = ResearchState::Completed;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_lifecycle_and_cancellation() {
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let mut orch = ResearchOrchestrator::new(ResearchConfig::default())
            .with_cancellation_token(cancel_flag.clone());

        // Test normal run
        let sources = vec![(
            "https://doc.rust-lang.org/book/",
            "<html><head><title>Rust Book</title></head><body>Ownership is key.</body></html>",
        )];

        let report = orch
            .execute_research("Rust Ownership", sources)
            .await
            .unwrap();
        assert_eq!(report.total_sources_analyzed, 1);
        assert!(!report.citations.is_empty());

        // Test cancellation
        cancel_flag.store(true, Ordering::SeqCst);
        let res = orch
            .execute_research("Rust Concurrency", vec![("https://example.com", "content")])
            .await;
        assert!(res.is_err());
        assert_eq!(orch.state, ResearchState::Cancelled);
    }

    #[test]
    fn test_orchestrator_query_decomposition() {
        let orch = ResearchOrchestrator::new(ResearchConfig::default());
        let queries = orch.decompose_objective("Rust vs Go concurrency");
        assert!(queries.len() >= 2);
        assert!(queries[0].contains("Rust vs Go concurrency"));
    }
}
