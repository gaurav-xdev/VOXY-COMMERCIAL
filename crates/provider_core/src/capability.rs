use crate::registry::{ProviderCapability, ProviderInfo, ProviderStatus};

#[derive(Debug, Clone)]
pub struct CapabilityScore {
    pub capability: ProviderCapability,
    pub score: f64,
    pub provider_id: String,
}

#[derive(Debug, Clone)]
pub struct CapabilityMatch {
    pub capability: ProviderCapability,
    pub provider_id: String,
    pub score: f64,
    pub is_available: bool,
    pub details: Option<String>,
}

pub trait CapabilityDiscovery: Send + Sync {
    fn find_best_match(&self, capability: &ProviderCapability) -> Option<CapabilityMatch>;
    fn score_for(&self, capability: &ProviderCapability) -> Option<CapabilityScore>;
    fn list_capabilities(&self) -> Vec<CapabilityScore>;
}

pub struct DefaultCapabilityDiscovery {
    providers: Vec<ProviderInfo>,
}

impl DefaultCapabilityDiscovery {
    pub fn new(providers: Vec<ProviderInfo>) -> Self {
        Self { providers }
    }

    fn score_provider(
        &self,
        provider: &ProviderInfo,
        capability: &ProviderCapability,
    ) -> Option<f64> {
        let has_capability = provider.capability == *capability
            || provider
                .models
                .iter()
                .any(|m| m.capabilities.contains(capability));

        if !has_capability {
            return None;
        }

        let mut score = 0.5;

        if matches!(provider.status, ProviderStatus::Available) {
            score += 0.3;
        }

        if provider.health.is_healthy {
            score += 0.1;
        }

        if let Some(latency) = provider.health.latency_ms {
            if latency < 100.0 {
                score += 0.1;
            } else if latency > 1000.0 {
                score -= 0.2;
            }
        }

        score += provider.priority as f64 * 0.1;

        if matches!(provider.kind, crate::registry::ProviderKind::Local) {
            score += 0.5;
        }

        Some(score.max(0.0))
    }
}

impl CapabilityDiscovery for DefaultCapabilityDiscovery {
    fn find_best_match(&self, capability: &ProviderCapability) -> Option<CapabilityMatch> {
        self.providers
            .iter()
            .filter_map(|p| {
                self.score_provider(p, capability)
                    .map(|score| CapabilityMatch {
                        capability: capability.clone(),
                        provider_id: p.id.clone(),
                        score,
                        is_available: matches!(p.status, ProviderStatus::Available),
                        details: Some(format!("Provider: {}, score: {:.2}", p.name, score)),
                    })
            })
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    fn score_for(&self, capability: &ProviderCapability) -> Option<CapabilityScore> {
        self.providers
            .iter()
            .filter_map(|p| {
                self.score_provider(p, capability)
                    .map(|score| CapabilityScore {
                        capability: capability.clone(),
                        score,
                        provider_id: p.id.clone(),
                    })
            })
            .max_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    fn list_capabilities(&self) -> Vec<CapabilityScore> {
        let mut scores = Vec::new();
        for provider in &self.providers {
            let cap = &provider.capability;
            if let Some(score) = self.score_provider(provider, cap) {
                scores.push(CapabilityScore {
                    capability: cap.clone(),
                    score,
                    provider_id: provider.id.clone(),
                });
            }
            for model in &provider.models {
                for cap in &model.capabilities {
                    scores.push(CapabilityScore {
                        capability: cap.clone(),
                        score: 0.8,
                        provider_id: format!("{}/{}", provider.id, model.id),
                    });
                }
            }
        }
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ProviderHealth, ProviderKind, ProviderStatus};

    fn make_provider(
        id: &str,
        cap: ProviderCapability,
        healthy: bool,
        priority: u32,
    ) -> ProviderInfo {
        ProviderInfo {
            id: id.to_string(),
            name: id.to_string(),
            kind: ProviderKind::Cloud,
            capability: cap,
            status: if healthy {
                ProviderStatus::Available
            } else {
                ProviderStatus::Unavailable
            },
            models: vec![],
            health: if healthy {
                ProviderHealth::new_healthy(Some(50.0))
            } else {
                ProviderHealth::new_unavailable("unhealthy")
            },
            base_url: None,
            priority,
        }
    }

    #[test]
    fn test_capability_score_creation() {
        let score = CapabilityScore {
            capability: ProviderCapability::Llm,
            score: 0.95,
            provider_id: "openai".into(),
        };
        assert_eq!(score.score, 0.95);
        assert_eq!(score.provider_id, "openai");
    }

    #[test]
    fn test_capability_match_creation() {
        let m = CapabilityMatch {
            capability: ProviderCapability::Stt,
            provider_id: "whisper".into(),
            score: 0.98,
            is_available: true,
            details: Some("high accuracy".into()),
        };
        assert!(m.is_available);
        assert!(m.details.is_some());
    }

    #[test]
    fn test_default_discovery_finds_best() {
        let providers = vec![
            make_provider("slow-llm", ProviderCapability::Llm, true, 0),
            make_provider("fast-llm", ProviderCapability::Llm, true, 1),
        ];
        let discovery = DefaultCapabilityDiscovery::new(providers);
        let best = discovery.find_best_match(&ProviderCapability::Llm);
        assert!(best.is_some());
        assert_eq!(best.unwrap().provider_id, "fast-llm");
    }

    #[test]
    fn test_default_discovery_no_match() {
        let providers = vec![make_provider("tts-1", ProviderCapability::Tts, true, 0)];
        let discovery = DefaultCapabilityDiscovery::new(providers);
        let best = discovery.find_best_match(&ProviderCapability::Stt);
        assert!(best.is_none());
    }

    #[test]
    fn test_default_discovery_unavailable_lower_score() {
        let providers = vec![
            make_provider("available", ProviderCapability::Llm, true, 0),
            make_provider("unavailable", ProviderCapability::Llm, false, 0),
        ];
        let discovery = DefaultCapabilityDiscovery::new(providers);
        let best = discovery.find_best_match(&ProviderCapability::Llm).unwrap();
        assert_eq!(best.provider_id, "available");
    }
}
