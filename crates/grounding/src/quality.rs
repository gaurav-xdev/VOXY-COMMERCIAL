//! Source credibility scoring and quality evaluation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceTier {
    PrimaryAuthority, // Official documentation, RFCs, scientific standards, .gov/.edu
    ReputableIndustry, // Established tech blogs, major engineering teams, Wikipedia
    SecondaryGeneral, // General blogs, forums, aggregator sites
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceQualityScore {
    pub tier: SourceTier,
    pub score: f32, // 0.0 to 1.0
    pub rationale: String,
    pub domain: String,
}

pub struct SourceQualityEvaluator;

impl SourceQualityEvaluator {
    /// Evaluates domain quality and evidence depth.
    pub fn evaluate(domain: &str, content_length: usize, has_code_or_data: bool) -> SourceQualityScore {
        let dom_lower = domain.to_lowercase();

        let (tier, base_score, rationale) = if dom_lower.ends_with(".gov")
            || dom_lower.ends_with(".edu")
            || dom_lower.contains("rfc-editor.org")
            || dom_lower.contains("w3.org")
            || dom_lower.contains("rust-lang.org")
            || dom_lower.contains("kernel.org")
            || dom_lower.contains("mozilla.org")
            || dom_lower.contains("microsoft.com")
            || dom_lower.contains("google.com")
            || dom_lower.contains("apple.com")
        {
            (
                SourceTier::PrimaryAuthority,
                0.90,
                "Official standard, government, educational or primary vendor documentation".to_string(),
            )
        } else if dom_lower.contains("github.com")
            || dom_lower.contains("stackoverflow.com")
            || dom_lower.contains("wikipedia.org")
            || dom_lower.contains("arxiv.org")
            || dom_lower.contains("nature.com")
            || dom_lower.contains("acm.org")
        {
            (
                SourceTier::ReputableIndustry,
                0.75,
                "Reputable industry repository, open standard, or peer-reviewed portal".to_string(),
            )
        } else {
            (
                SourceTier::SecondaryGeneral,
                0.50,
                "General web source; secondary commentary".to_string(),
            )
        };

        // Length and depth modifiers
        let mut final_score: f32 = base_score;
        if content_length > 1000 {
            final_score += 0.05;
        }
        if has_code_or_data {
            final_score += 0.05;
        }

        SourceQualityScore {
            tier,
            score: final_score.min(1.0f32),
            rationale,
            domain: domain.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluator_scores_authoritative_sources_highest() {
        let rust_doc = SourceQualityEvaluator::evaluate("doc.rust-lang.org", 2500, true);
        assert_eq!(rust_doc.tier, SourceTier::PrimaryAuthority);
        assert!(rust_doc.score >= 0.95);

        let general = SourceQualityEvaluator::evaluate("randomblog.xyz", 300, false);
        assert_eq!(general.tier, SourceTier::SecondaryGeneral);
        assert!(general.score <= 0.55);
    }
}
