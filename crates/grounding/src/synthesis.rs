//! Cross-source synthesis, contradiction detection, and citation mapping.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimConfidence {
    HighConfidence, // Corroborated by multiple high-tier sources without contradiction
    Moderate,       // Single source or mixed tiers
    Contested,      // Direct contradictory claims discovered across sources
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub id: String,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub credibility_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchClaim {
    pub statement: String,
    pub supporting_citation_ids: Vec<String>,
    pub contradicting_citation_ids: Vec<String>,
    pub confidence: ClaimConfidence,
    pub synthesis_notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SynthesisReport {
    pub objective: String,
    pub executive_summary: String,
    pub claims: Vec<ResearchClaim>,
    pub citations: HashMap<String, Citation>,
    pub has_contradictions: bool,
    pub total_sources_analyzed: usize,
}

pub struct CrossSourceSynthesizer;

impl CrossSourceSynthesizer {
    /// Builds a structured synthesis report from claims and verified citations.
    pub fn build_report(
        objective: &str,
        summary: &str,
        claims: Vec<ResearchClaim>,
        citations: HashMap<String, Citation>,
    ) -> SynthesisReport {
        let has_contradictions = claims.iter().any(|c| {
            !c.contradicting_citation_ids.is_empty() || c.confidence == ClaimConfidence::Contested
        });
        let total = citations.len();

        SynthesisReport {
            objective: objective.to_string(),
            executive_summary: summary.to_string(),
            claims,
            citations,
            has_contradictions,
            total_sources_analyzed: total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesis_report_tracks_contradictions_and_citations() {
        let mut citations = HashMap::new();
        citations.insert(
            "src-1".to_string(),
            Citation {
                id: "src-1".to_string(),
                title: "Rust Official Docs".to_string(),
                url: "https://doc.rust-lang.org".to_string(),
                domain: "doc.rust-lang.org".to_string(),
                credibility_score: 0.95,
            },
        );
        citations.insert(
            "src-2".to_string(),
            Citation {
                id: "src-2".to_string(),
                title: "Outdated Blog 2018".to_string(),
                url: "https://oldblog.com".to_string(),
                domain: "oldblog.com".to_string(),
                credibility_score: 0.50,
            },
        );

        let claim = ResearchClaim {
            statement: "Async fn in traits is supported natively in Rust 2024".to_string(),
            supporting_citation_ids: vec!["src-1".to_string()],
            contradicting_citation_ids: vec!["src-2".to_string()],
            confidence: ClaimConfidence::Contested,
            synthesis_notes: "Source 2 claims async trait requires async-trait macro, but is outdated as of Rust 1.75+".to_string(),
        };

        let report = CrossSourceSynthesizer::build_report(
            "Verify Async Trait Status",
            "Async fn in traits is supported without third-party crates.",
            vec![claim],
            citations,
        );

        assert!(report.has_contradictions);
        assert_eq!(report.total_sources_analyzed, 2);
        assert_eq!(report.claims[0].supporting_citation_ids.len(), 1);
        assert_eq!(report.claims[0].contradicting_citation_ids.len(), 1);
    }
}
