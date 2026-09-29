use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProviderCapability {
    Llm,
    Stt,
    Tts,
    Embedding,
    Vision,
    VoiceActivity,
    WakeWord,
    Hardware,
}

impl ProviderCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Llm => "llm",
            Self::Stt => "stt",
            Self::Tts => "tts",
            Self::Embedding => "embedding",
            Self::Vision => "vision",
            Self::VoiceActivity => "voice_activity",
            Self::WakeWord => "wake_word",
            Self::Hardware => "hardware",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProviderStatus {
    Available,
    Busy,
    Degraded(String),
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HealthStatus {
    Unknown,
    Checking,
    Healthy,
    Degraded(String),
    Unavailable(String),
    RateLimited { retry_after_secs: Option<u64> },
    AuthenticationFailed(String),
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ModelCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub vision: bool,
    pub multilingual: Vec<String>,
    pub local: bool,
    pub context_size: Option<u32>,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            streaming: true,
            tool_calling: false,
            vision: false,
            multilingual: vec!["en".to_string(), "hi".to_string(), "hinglish".to_string()],
            local: false,
            context_size: Some(4096),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub capabilities: Vec<ProviderCapability>,
    pub model_capabilities: ModelCapabilities,
    pub version: String,
    pub context_size: Option<u32>,
    pub supports_vision: bool,
    pub supports_tool_calling: bool,
    pub supports_streaming: bool,
    pub supports_embeddings: bool,
    pub metadata: HashMap<String, String>,
}

impl ModelInfo {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            capabilities: Vec::new(),
            model_capabilities: ModelCapabilities::default(),
            version: String::new(),
            context_size: None,
            supports_vision: false,
            supports_tool_calling: false,
            supports_streaming: true,
            supports_embeddings: false,
            metadata: HashMap::new(),
        }
    }

    pub fn with_capabilities(mut self, caps: ModelCapabilities) -> Self {
        self.supports_streaming = caps.streaming;
        self.supports_tool_calling = caps.tool_calling;
        self.supports_vision = caps.vision;
        self.context_size = caps.context_size;
        self.model_capabilities = caps;
        self
    }

    pub fn with_multilingual(mut self, languages: &[&str]) -> Self {
        self.model_capabilities.multilingual = languages.iter().map(|s| s.to_string()).collect();
        self
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderHealth {
    pub is_healthy: bool,
    pub status: HealthStatus,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub latency_ms: Option<f64>,
    pub details: Option<String>,
    pub consecutive_failures: u32,
    pub circuit_broken: bool,
}

impl Default for ProviderHealth {
    fn default() -> Self {
        Self {
            is_healthy: true,
            status: HealthStatus::Healthy,
            last_check: chrono::Utc::now(),
            latency_ms: None,
            details: None,
            consecutive_failures: 0,
            circuit_broken: false,
        }
    }
}

impl ProviderHealth {
    pub fn new_healthy(latency_ms: Option<f64>) -> Self {
        Self {
            is_healthy: true,
            status: HealthStatus::Healthy,
            last_check: chrono::Utc::now(),
            latency_ms,
            details: None,
            consecutive_failures: 0,
            circuit_broken: false,
        }
    }

    pub fn new_degraded(reason: impl Into<String>, latency_ms: Option<f64>) -> Self {
        let r = reason.into();
        Self {
            is_healthy: true,
            status: HealthStatus::Degraded(r.clone()),
            last_check: chrono::Utc::now(),
            latency_ms,
            details: Some(r),
            consecutive_failures: 0,
            circuit_broken: false,
        }
    }

    pub fn new_unavailable(reason: impl Into<String>) -> Self {
        let r = reason.into();
        Self {
            is_healthy: false,
            status: HealthStatus::Unavailable(r.clone()),
            last_check: chrono::Utc::now(),
            latency_ms: None,
            details: Some(r),
            consecutive_failures: 1,
            circuit_broken: false,
        }
    }

    pub fn new_rate_limited(retry_after_secs: Option<u64>) -> Self {
        Self {
            is_healthy: false,
            status: HealthStatus::RateLimited { retry_after_secs },
            last_check: chrono::Utc::now(),
            latency_ms: None,
            details: Some(format!("Rate limited. Retry after: {:?}", retry_after_secs)),
            consecutive_failures: 1,
            circuit_broken: false,
        }
    }

    pub fn new_auth_failed(reason: impl Into<String>) -> Self {
        let r = reason.into();
        Self {
            is_healthy: false,
            status: HealthStatus::AuthenticationFailed(r.clone()),
            last_check: chrono::Utc::now(),
            latency_ms: None,
            details: Some(r),
            consecutive_failures: 1,
            circuit_broken: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProviderKind {
    Local,
    Cloud,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub capability: ProviderCapability,
    pub status: ProviderStatus,
    pub models: Vec<ModelInfo>,
    pub health: ProviderHealth,
    pub base_url: Option<String>,
    pub priority: u32,
}

#[async_trait]
pub trait ProviderRegistry: Send + Sync {
    async fn register(&self, provider: ProviderInfo) -> Result<()>;
    async fn unregister(&self, id: &str) -> Result<()>;
    async fn get(&self, id: &str) -> Result<ProviderInfo>;
    async fn find_by_capability(&self, capability: ProviderCapability)
        -> Result<Vec<ProviderInfo>>;
    async fn list_all(&self) -> Result<Vec<ProviderInfo>>;
    async fn update_status(&self, id: &str, status: ProviderStatus) -> Result<()>;
    async fn is_registered(&self, id: &str) -> Result<bool>;
    async fn select_with_failover(&self, capability: ProviderCapability) -> Result<ProviderInfo>;
}

pub struct DefaultProviderRegistry {
    providers: RwLock<HashMap<String, ProviderInfo>>,
}

impl DefaultProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderRegistry for DefaultProviderRegistry {
    async fn register(&self, provider: ProviderInfo) -> Result<()> {
        self.providers.write().insert(provider.id.clone(), provider);
        Ok(())
    }

    async fn unregister(&self, id: &str) -> Result<()> {
        self.providers.write().remove(id);
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<ProviderInfo> {
        self.providers
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| crate::ProviderError::ModelNotFound(id.to_string()))
    }

    async fn find_by_capability(
        &self,
        capability: ProviderCapability,
    ) -> Result<Vec<ProviderInfo>> {
        let providers = self.providers.read();
        Ok(providers
            .values()
            .filter(|p| {
                p.capability == capability
                    || p.models
                        .iter()
                        .any(|m| m.capabilities.contains(&capability))
            })
            .cloned()
            .collect())
    }

    async fn list_all(&self) -> Result<Vec<ProviderInfo>> {
        Ok(self.providers.read().values().cloned().collect())
    }

    async fn update_status(&self, id: &str, status: ProviderStatus) -> Result<()> {
        let mut providers = self.providers.write();
        if let Some(provider) = providers.get_mut(id) {
            provider.status = status;
            Ok(())
        } else {
            Err(crate::ProviderError::ModelNotFound(id.to_string()))
        }
    }

    async fn is_registered(&self, id: &str) -> Result<bool> {
        Ok(self.providers.read().contains_key(id))
    }

    async fn select_with_failover(&self, capability: ProviderCapability) -> Result<ProviderInfo> {
        let providers = self.providers.read();
        let mut candidates: Vec<ProviderInfo> = providers
            .values()
            .filter(|p| {
                (p.capability == capability
                    || p.models
                        .iter()
                        .any(|m| m.capabilities.contains(&capability)))
                    && matches!(p.status, ProviderStatus::Available)
                    && p.health.is_healthy
            })
            .cloned()
            .collect();
        if candidates.is_empty() {
            return Err(crate::ProviderError::ModelNotFound(format!(
                "no available provider for {:?}",
                capability
            )));
        }
        candidates.sort_by_key(|p| p.priority);
        Ok(candidates.into_iter().next().unwrap())
    }
}

pub type SharedProviderRegistry = Arc<dyn ProviderRegistry>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_capability_debug_and_clone() {
        let cap = ProviderCapability::Llm;
        assert_eq!(format!("{cap:?}"), "Llm");
    }

    #[test]
    fn test_provider_status_variants() {
        let available = ProviderStatus::Available;
        let busy = ProviderStatus::Busy;
        let degraded = ProviderStatus::Degraded("slow".into());
        let unavailable = ProviderStatus::Unavailable;
        assert!(matches!(available, ProviderStatus::Available));
        assert!(matches!(busy, ProviderStatus::Busy));
        assert!(matches!(degraded, ProviderStatus::Degraded(_)));
        assert!(matches!(unavailable, ProviderStatus::Unavailable));
    }

    #[test]
    fn test_model_info_fields() {
        let model = ModelInfo {
            id: "gpt-4".into(),
            name: "GPT-4".into(),
            capabilities: vec![ProviderCapability::Llm],
            model_capabilities: ModelCapabilities::default(),
            version: "1.0".into(),
            context_size: Some(8192),
            supports_vision: true,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_embeddings: false,
            metadata: HashMap::new(),
        };
        assert_eq!(model.id, "gpt-4");
        assert_eq!(model.version, "1.0");
        assert!(model.supports_vision);
    }

    #[test]
    fn test_new_model_info() {
        let m = ModelInfo::new("model-1", "Model One");
        assert_eq!(m.id, "model-1");
        assert!(m.supports_streaming);
    }

    #[tokio::test]
    async fn test_default_registry_register_and_get() {
        let registry = DefaultProviderRegistry::new();
        let info = ProviderInfo {
            id: "test".into(),
            name: "Test".into(),
            kind: ProviderKind::Local,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![],
            health: ProviderHealth::new_healthy(None),
            base_url: None,
            priority: 0,
        };
        registry.register(info.clone()).await.unwrap();
        let retrieved = registry.get("test").await.unwrap();
        assert_eq!(retrieved.id, "test");
    }

    #[tokio::test]
    async fn test_registry_unregister() {
        let registry = DefaultProviderRegistry::new();
        let info = ProviderInfo {
            id: "del".into(),
            name: "Delete".into(),
            kind: ProviderKind::Cloud,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![],
            health: ProviderHealth::new_healthy(None),
            base_url: None,
            priority: 0,
        };
        registry.register(info).await.unwrap();
        assert!(registry.is_registered("del").await.unwrap());
        registry.unregister("del").await.unwrap();
        assert!(!registry.is_registered("del").await.unwrap());
    }

    #[tokio::test]
    async fn test_registry_find_by_capability() {
        let registry = DefaultProviderRegistry::new();
        let llm = ProviderInfo {
            id: "llm-1".into(),
            name: "LLM One".into(),
            kind: ProviderKind::Cloud,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![],
            health: ProviderHealth::new_healthy(None),
            base_url: None,
            priority: 0,
        };
        registry.register(llm).await.unwrap();
        let results = registry
            .find_by_capability(ProviderCapability::Llm)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        let tts_results = registry
            .find_by_capability(ProviderCapability::Tts)
            .await
            .unwrap();
        assert!(tts_results.is_empty());
    }

    #[test]
    fn test_provider_health_default() {
        let health = ProviderHealth::new_healthy(Some(42.0));
        assert!(health.is_healthy);
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.latency_ms.is_some());
    }

    #[test]
    fn test_capability_as_str() {
        assert_eq!(ProviderCapability::Llm.as_str(), "llm");
        assert_eq!(ProviderCapability::Stt.as_str(), "stt");
        assert_eq!(ProviderCapability::Tts.as_str(), "tts");
        assert_eq!(ProviderCapability::Embedding.as_str(), "embedding");
        assert_eq!(ProviderCapability::Vision.as_str(), "vision");
    }

    #[tokio::test]
    async fn test_select_with_failover_returns_healthy_available() {
        let registry = DefaultProviderRegistry::new();
        let healthy = ProviderInfo {
            id: "healthy-llm".into(),
            name: "Healthy LLM".into(),
            kind: ProviderKind::Local,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![],
            health: ProviderHealth::new_healthy(Some(50.0)),
            base_url: None,
            priority: 0,
        };
        registry.register(healthy).await.unwrap();
        let selected = registry
            .select_with_failover(ProviderCapability::Llm)
            .await
            .unwrap();
        assert_eq!(selected.id, "healthy-llm");
    }

    #[tokio::test]
    async fn test_select_with_failover_skips_unhealthy() {
        let registry = DefaultProviderRegistry::new();
        let unhealthy = ProviderInfo {
            id: "sick-llm".into(),
            name: "Sick LLM".into(),
            kind: ProviderKind::Local,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Degraded("down".into()),
            models: vec![],
            health: ProviderHealth::new_unavailable("unhealthy"),
            base_url: None,
            priority: 0,
        };
        registry.register(unhealthy).await.unwrap();
        let result = registry.select_with_failover(ProviderCapability::Llm).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_select_with_failover_picks_highest_priority() {
        let registry = DefaultProviderRegistry::new();
        let low = ProviderInfo {
            id: "low-pri".into(),
            name: "Low Priority".into(),
            kind: ProviderKind::Local,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![],
            health: ProviderHealth::new_healthy(None),
            base_url: None,
            priority: 10,
        };
        let high = ProviderInfo {
            id: "high-pri".into(),
            name: "High Priority".into(),
            kind: ProviderKind::Local,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![],
            health: ProviderHealth::new_healthy(None),
            base_url: None,
            priority: 1,
        };
        registry.register(low).await.unwrap();
        registry.register(high).await.unwrap();
        let selected = registry
            .select_with_failover(ProviderCapability::Llm)
            .await
            .unwrap();
        assert_eq!(selected.id, "high-pri");
    }
}
