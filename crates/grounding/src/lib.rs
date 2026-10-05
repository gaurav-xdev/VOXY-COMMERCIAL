//! Abstract intention to concrete UI target translation.
//!
//! Grounding resolves high-level user intents (e.g. "open the browser")
//! into concrete UI targets (window handles, element selectors, coordinates).

pub mod defense;
pub mod error;
pub mod extractor;
pub mod orchestrator;
pub mod quality;
pub mod synthesis;

pub use defense::{PromptInjectionDefense, SanitizedContent};
pub use error::{GroundingError, Result};
pub use extractor::{ContentExtractor, ExtractedDocument};
pub use orchestrator::{ResearchConfig, ResearchOrchestrator, ResearchState};
pub use quality::{SourceQualityEvaluator, SourceQualityScore, SourceTier};
pub use synthesis::{Citation, ClaimConfidence, CrossSourceSynthesizer, ResearchClaim, SynthesisReport};

use async_trait::async_trait;
use voxy_world_model::context::WorldContext;

/// Pluggable automation verification backend.
pub trait TargetVerifier: Send + Sync {}

/// A resolved UI target that can be passed to an automation backend.
#[derive(Debug, Clone)]
pub enum ResolvedTarget {
    /// A window identified by title, class, or handle.
    Window {
        title: Option<String>,
        class: Option<String>,
        handle: Option<String>,
    },
    /// A screen coordinate region.
    Region {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    },
    /// An abstract action descriptor for the automation system.
    Action(String),
}

/// A grounding request created from a user intent.
#[derive(Debug, Clone)]
pub struct GroundingRequest {
    /// The textual description of what the user wants to do.
    pub description: String,
    /// Optional qualifiers (e.g. "in Chrome", "on the left side").
    pub qualifiers: Vec<String>,
    /// The current world context.
    pub context: Option<WorldContext>,
}

/// The grounding engine translates high-level requests into concrete targets.
#[async_trait]
pub trait GroundingEngine: Send + Sync {
    /// Resolve a request into one or more possible UI targets.
    async fn resolve(&self, request: &GroundingRequest) -> Result<Vec<ResolvedTarget>>;

    /// Rank potential targets by their likelihood of matching the intent.
    async fn rank_targets(
        &self,
        targets: Vec<ResolvedTarget>,
    ) -> Result<Vec<(ResolvedTarget, f64)>>;

    /// Verify that a resolved target is still valid (e.g. window still exists).
    async fn verify_target(
        &self,
        target: &ResolvedTarget,
        backend: &dyn TargetVerifier,
    ) -> Result<bool>;
}

/// In-memory implementation of the grounding engine using keyword matching.
pub struct InMemoryGroundingEngine;

#[async_trait]
impl GroundingEngine for InMemoryGroundingEngine {
    async fn resolve(&self, request: &GroundingRequest) -> Result<Vec<ResolvedTarget>> {
        let desc = request.description.to_lowercase();
        let mut targets = Vec::new();

        // Window-related intents
        if desc.contains("open") || desc.contains("launch") || desc.contains("start") {
            let app = request.qualifiers.first().cloned().unwrap_or_default();
            targets.push(ResolvedTarget::Action(format!("launch:{}", app)));
        }

        if desc.contains("close") || desc.contains("exit") || desc.contains("quit") {
            targets.push(ResolvedTarget::Action("close:active".to_string()));
        }

        if desc.contains("minimize") {
            targets.push(ResolvedTarget::Action("minimize:active".to_string()));
        }

        if desc.contains("maximize") || desc.contains("fullscreen") {
            targets.push(ResolvedTarget::Action("maximize:active".to_string()));
        }

        // Fallback: treat the entire description as a search term
        if targets.is_empty() {
            targets.push(ResolvedTarget::Window {
                title: Some(request.description.clone()),
                class: None,
                handle: None,
            });
        }

        Ok(targets)
    }

    async fn rank_targets(
        &self,
        targets: Vec<ResolvedTarget>,
    ) -> Result<Vec<(ResolvedTarget, f64)>> {
        let mut ranked: Vec<_> = targets
            .into_iter()
            .enumerate()
            .map(|(i, t)| (t, 1.0 - (i as f64 * 0.1)))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(ranked)
    }

    async fn verify_target(
        &self,
        target: &ResolvedTarget,
        _backend: &dyn TargetVerifier,
    ) -> Result<bool> {
        match target {
            ResolvedTarget::Window {
                handle: Some(_), ..
            } => Ok(true),
            ResolvedTarget::Region { .. } => Ok(true),
            ResolvedTarget::Action(_) => Ok(true),
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounding_engine_creates() {
        let _g = InMemoryGroundingEngine;
    }

    #[tokio::test]
    async fn test_resolve_open() {
        let engine = InMemoryGroundingEngine;
        let request = GroundingRequest {
            description: "Open the browser".to_string(),
            qualifiers: vec!["chrome".to_string()],
            context: None,
        };
        let targets = engine.resolve(&request).await.unwrap();
        assert!(!targets.is_empty());
        assert!(matches!(targets[0], ResolvedTarget::Action(ref a) if a.contains("launch")));
    }

    #[tokio::test]
    async fn test_resolve_close() {
        let engine = InMemoryGroundingEngine;
        let request = GroundingRequest {
            description: "Close this window".to_string(),
            qualifiers: vec![],
            context: None,
        };
        let targets = engine.resolve(&request).await.unwrap();
        assert!(!targets.is_empty());
    }

    #[tokio::test]
    async fn test_rank_targets() {
        let engine = InMemoryGroundingEngine;
        let targets = vec![
            ResolvedTarget::Action("launch:chrome".to_string()),
            ResolvedTarget::Window {
                title: Some("Chrome".to_string()),
                class: None,
                handle: None,
            },
        ];
        let ranked = engine.rank_targets(targets).await.unwrap();
        assert_eq!(ranked.len(), 2);
        // First target should have higher score
        assert!(ranked[0].1 >= ranked[1].1);
    }
}
