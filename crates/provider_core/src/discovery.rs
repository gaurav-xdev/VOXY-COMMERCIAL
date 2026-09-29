use crate::error::Result;
use crate::registry::{
    ModelInfo, ProviderCapability, ProviderHealth, ProviderInfo, ProviderKind, ProviderStatus,
};

pub struct DiscoveredLocalProvider {
    pub name: String,
    pub base_url: String,
    pub provider_type: LocalProviderType,
    pub models: Vec<DiscoveredModel>,
}

pub enum LocalProviderType {
    Ollama,
    LmStudio,
    LlamaCpp,
    Vllm,
    LocalAi,
    KoboldCpp,
    TextGenerationWebui,
    JanAi,
    OpenaiCompatible,
}

impl LocalProviderType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LmStudio => "lm-studio",
            Self::LlamaCpp => "llama.cpp",
            Self::Vllm => "vllm",
            Self::LocalAi => "localai",
            Self::KoboldCpp => "koboldcpp",
            Self::TextGenerationWebui => "text-generation-webui",
            Self::JanAi => "jan-ai",
            Self::OpenaiCompatible => "openai-compatible",
        }
    }
}

pub struct DiscoveredModel {
    pub id: String,
    pub name: String,
    pub context_size: Option<u32>,
}

pub struct LocalProviderDetector;

impl Default for LocalProviderDetector {
    fn default() -> Self {
        Self
    }
}

impl LocalProviderDetector {
    pub fn new() -> Self {
        Self
    }

    pub async fn detect_all(&self) -> Vec<DiscoveredLocalProvider> {
        let mut providers = Vec::new();

        let checks: Vec<(LocalProviderType, &str, &str)> = vec![
            (
                LocalProviderType::Ollama,
                "http://127.0.0.1:11434",
                "/api/tags",
            ),
            (
                LocalProviderType::LmStudio,
                "http://127.0.0.1:1234",
                "/v1/models",
            ),
            (
                LocalProviderType::LlamaCpp,
                "http://127.0.0.1:8080",
                "/v1/models",
            ),
            (
                LocalProviderType::Vllm,
                "http://127.0.0.1:8000",
                "/v1/models",
            ),
            (
                LocalProviderType::LocalAi,
                "http://127.0.0.1:8080",
                "/v1/models",
            ),
            (
                LocalProviderType::KoboldCpp,
                "http://127.0.0.1:5001",
                "/v1/models",
            ),
            (
                LocalProviderType::TextGenerationWebui,
                "http://127.0.0.1:5000",
                "/v1/models",
            ),
            (
                LocalProviderType::JanAi,
                "http://127.0.0.1:1337",
                "/v1/models",
            ),
        ];

        for (provider_type, base_url, _health_path) in checks {
            match self.check_endpoint(base_url).await {
                Ok(true) => {
                    let models = self.fetch_models(base_url, &provider_type).await;
                    providers.push(DiscoveredLocalProvider {
                        name: provider_type.as_str().to_string(),
                        base_url: base_url.to_string(),
                        provider_type,
                        models,
                    });
                }
                _ => continue,
            }
        }

        providers
    }

    async fn check_endpoint(&self, base_url: &str) -> Result<bool> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|e| crate::ProviderError::ConnectionFailed(e.to_string()))?;

        let resp = client.get(base_url).send().await;

        match resp {
            Ok(r) => Ok(r.status().is_success() || r.status().is_informational()),
            Err(_) => Ok(false),
        }
    }

    async fn fetch_models(
        &self,
        base_url: &str,
        provider_type: &LocalProviderType,
    ) -> Vec<DiscoveredModel> {
        match provider_type {
            LocalProviderType::Ollama => self.fetch_ollama_models(base_url).await,
            _ => self.fetch_openai_models(base_url).await,
        }
    }

    async fn fetch_ollama_models(&self, base_url: &str) -> Vec<DiscoveredModel> {
        let url = format!("{}/api/tags", base_url);
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(body) = r.json::<serde_json::Value>().await {
                    if let Some(models) = body["models"].as_array() {
                        return models
                            .iter()
                            .filter_map(|m| {
                                let name = m["name"].as_str()?.to_string();
                                Some(DiscoveredModel {
                                    id: name.clone(),
                                    name,
                                    context_size: m
                                        .get("details")
                                        .and_then(|d| d.get("context_length"))
                                        .and_then(|v| v.as_u64())
                                        .map(|v| v as u32),
                                })
                            })
                            .collect();
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    async fn fetch_openai_models(&self, base_url: &str) -> Vec<DiscoveredModel> {
        let url = format!("{}/v1/models", base_url);
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .timeout(std::time::Duration::from_secs(3))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(body) = r.json::<serde_json::Value>().await {
                    if let Some(data) = body["data"].as_array() {
                        return data
                            .iter()
                            .filter_map(|m| {
                                let id = m["id"].as_str()?.to_string();
                                Some(DiscoveredModel {
                                    id: id.clone(),
                                    name: id,
                                    context_size: None,
                                })
                            })
                            .collect();
                    }
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    pub fn to_provider_info(&self, discovered: &DiscoveredLocalProvider) -> ProviderInfo {
        let models: Vec<ModelInfo> = discovered
            .models
            .iter()
            .map(|m| {
                let mut mi = ModelInfo::new(&m.id, &m.name);
                mi.context_size = m.context_size;
                mi.supports_embeddings =
                    matches!(discovered.provider_type, LocalProviderType::Ollama);
                mi.capabilities = vec![
                    ProviderCapability::Llm,
                    if mi.supports_embeddings {
                        ProviderCapability::Embedding
                    } else {
                        ProviderCapability::Llm
                    },
                ];
                mi
            })
            .collect();

        ProviderInfo {
            id: discovered.name.clone(),
            name: discovered.name.clone(),
            kind: ProviderKind::Local,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models,
            health: ProviderHealth::new_healthy(Some(5.0)),
            base_url: Some(discovered.base_url.clone()),
            priority: 10,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_as_str() {
        assert_eq!(LocalProviderType::Ollama.as_str(), "ollama");
        assert_eq!(LocalProviderType::LmStudio.as_str(), "lm-studio");
        assert_eq!(
            LocalProviderType::OpenaiCompatible.as_str(),
            "openai-compatible"
        );
    }

    #[test]
    fn test_detector_creates() {
        let _detector = LocalProviderDetector::new();
    }

    #[tokio::test]
    async fn test_detect_all_no_local_servers() {
        let detector = LocalProviderDetector::new();
        let providers = detector.detect_all().await;
        // In CI there are no local LLM servers running
        assert!(providers.is_empty());
    }

    #[test]
    fn test_to_provider_info_ollama() {
        let detector = LocalProviderDetector::new();
        let discovered = DiscoveredLocalProvider {
            name: "ollama".into(),
            base_url: "http://127.0.0.1:11434".into(),
            provider_type: LocalProviderType::Ollama,
            models: vec![DiscoveredModel {
                id: "llama3.2:3b".into(),
                name: "llama3.2:3b".into(),
                context_size: Some(8192),
            }],
        };
        let info = detector.to_provider_info(&discovered);
        assert_eq!(info.id, "ollama");
        assert_eq!(info.models.len(), 1);
        assert_eq!(info.models[0].id, "llama3.2:3b");
        assert!(info.models[0].supports_embeddings);
        assert!(matches!(info.kind, ProviderKind::Local));
    }

    #[test]
    fn test_to_provider_info_openai_compatible() {
        let detector = LocalProviderDetector::new();
        let discovered = DiscoveredLocalProvider {
            name: "lm-studio".into(),
            base_url: "http://127.0.0.1:1234".into(),
            provider_type: LocalProviderType::LmStudio,
            models: vec![DiscoveredModel {
                id: "local-model".into(),
                name: "Local Model".into(),
                context_size: None,
            }],
        };
        let info = detector.to_provider_info(&discovered);
        assert_eq!(info.id, "lm-studio");
        assert_eq!(info.models[0].id, "local-model");
    }
}
