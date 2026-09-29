pub mod error;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use voxy_provider_core::{
    CapabilityDiscovery, DefaultCapabilityDiscovery, EmbeddingProvider, LlmProvider,
    ProviderCapability, ProviderInfo, ProviderRegistry, SttProvider, TtsProvider,
};

pub use error::{Result, RouterError};

#[derive(Debug, Clone, Copy, PartialEq)]
enum BreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
struct BreakerEntry {
    state: BreakerState,
    failures: u32,
    last_failure: Instant,
    opened_at: Instant,
}

impl BreakerEntry {
    fn new() -> Self {
        Self {
            state: BreakerState::Closed,
            failures: 0,
            last_failure: Instant::now(),
            opened_at: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum RoutingMode {
    #[default]
    Auto,
    LocalOnly,
    CloudOnly,
    ForceModel(String),
    ForceProvider(String),
}

#[derive(Debug, Clone)]
pub struct RouterConfig {
    pub mode: RoutingMode,
    pub local_first: bool,
    pub priority_order: Vec<String>,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_reset_secs: u64,
    pub provider_timeout_secs: u64,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            mode: RoutingMode::Auto,
            local_first: true,
            priority_order: Vec::new(),
            max_retries: 3,
            retry_delay_ms: 200,
            circuit_breaker_threshold: 5,
            circuit_breaker_reset_secs: 30,
            provider_timeout_secs: 15,
        }
    }
}

pub struct ModelRouter {
    registry: Arc<dyn ProviderRegistry>,
    config: parking_lot::RwLock<RouterConfig>,
    breakers: parking_lot::RwLock<HashMap<String, BreakerEntry>>,
    last_health_check: AtomicU64,
    llm_providers: parking_lot::RwLock<HashMap<String, Arc<dyn LlmProvider>>>,
    stt_providers: parking_lot::RwLock<HashMap<String, Arc<dyn SttProvider>>>,
    tts_providers: parking_lot::RwLock<HashMap<String, Arc<dyn TtsProvider>>>,
}

impl ModelRouter {
    pub fn new(registry: Arc<dyn ProviderRegistry>) -> Self {
        Self {
            registry,
            config: parking_lot::RwLock::new(RouterConfig::default()),
            breakers: parking_lot::RwLock::new(HashMap::new()),
            last_health_check: AtomicU64::new(0),
            llm_providers: parking_lot::RwLock::new(HashMap::new()),
            stt_providers: parking_lot::RwLock::new(HashMap::new()),
            tts_providers: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    pub fn with_config(registry: Arc<dyn ProviderRegistry>, config: RouterConfig) -> Self {
        Self {
            registry,
            config: parking_lot::RwLock::new(config),
            breakers: parking_lot::RwLock::new(HashMap::new()),
            last_health_check: AtomicU64::new(0),
            llm_providers: parking_lot::RwLock::new(HashMap::new()),
            stt_providers: parking_lot::RwLock::new(HashMap::new()),
            tts_providers: parking_lot::RwLock::new(HashMap::new()),
        }
    }

    pub fn config(&self) -> RouterConfig {
        self.config.read().clone()
    }

    pub fn set_config(&self, config: RouterConfig) {
        *self.config.write() = config;
    }

    pub fn set_mode(&self, mode: RoutingMode) {
        self.config.write().mode = mode;
    }

    fn is_breaker_open(&self, provider_id: &str) -> bool {
        let reset_secs = {
            let config = self.config.read();
            config.circuit_breaker_reset_secs
        };
        let reset = Duration::from_secs(reset_secs);
        let breakers = self.breakers.read();
        if let Some(entry) = breakers.get(provider_id) {
            match entry.state {
                BreakerState::Open => {
                    if entry.opened_at.elapsed() > reset {
                        drop(breakers);
                        let mut breakers = self.breakers.write();
                        if let Some(e) = breakers.get_mut(provider_id) {
                            if e.state == BreakerState::Open {
                                e.state = BreakerState::HalfOpen;
                            }
                        }
                        return false;
                    }
                    true
                }
                BreakerState::HalfOpen => false,
                BreakerState::Closed => false,
            }
        } else {
            false
        }
    }

    fn record_failure(&self, provider_id: &str) {
        let mut breakers = self.breakers.write();
        let config = self.config.read();
        let entry = breakers
            .entry(provider_id.to_string())
            .or_insert_with(BreakerEntry::new);
        entry.failures += 1;
        entry.last_failure = Instant::now();
        if entry.failures >= config.circuit_breaker_threshold {
            entry.state = BreakerState::Open;
            entry.opened_at = Instant::now();
        }
    }

    fn record_success(&self, provider_id: &str) {
        let mut breakers = self.breakers.write();
        if let Some(entry) = breakers.get_mut(provider_id) {
            entry.state = BreakerState::Closed;
            entry.failures = 0;
        }
    }

    async fn refresh_health(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last = self.last_health_check.load(Ordering::Relaxed);
        if now - last < 60 {
            return;
        }
        self.last_health_check.store(now, Ordering::Relaxed);
        if let Ok(providers) = self.registry.list_all().await {
            for p in &providers {
                if p.health.is_healthy {
                    self.record_success(&p.id);
                }
            }
        }
    }

    fn filter_healthy(&self, providers: Vec<ProviderInfo>) -> Vec<ProviderInfo> {
        providers
            .into_iter()
            .filter(|p| {
                let cb_open = self.is_breaker_open(&p.id);
                let unhealthy = !p.health.is_healthy
                    || !matches!(p.status, voxy_provider_core::ProviderStatus::Available);
                !cb_open && !unhealthy
            })
            .collect()
    }

    async fn select_fallback(&self, providers: Vec<ProviderInfo>) -> Result<ProviderInfo> {
        for p in &providers {
            if !self.is_breaker_open(&p.id) {
                return Ok(p.clone());
            }
        }
        Err(RouterError::AllProvidersExhausted(
            "all providers are circuit-broken or unhealthy".into(),
        ))
    }

    pub async fn select_provider(&self, capability: &ProviderCapability) -> Result<ProviderInfo> {
        self.refresh_health().await;
        let config_mode = { self.config.read().mode.clone() };
        let providers = self
            .registry
            .find_by_capability(capability.clone())
            .await
            .map_err(|e| RouterError::RoutingFailed(e.to_string()))?;

        if providers.is_empty() {
            return Err(RouterError::NoProviderAvailable(format!(
                "No provider found for {:?}",
                capability
            )));
        }

        let best = match &config_mode {
            RoutingMode::ForceModel(model_id) => {
                let mut best = None;
                for p in &providers {
                    if p.models.iter().any(|m| m.id == *model_id) {
                        best = Some(p.clone());
                        break;
                    }
                }
                best.ok_or_else(|| {
                    RouterError::NoProviderAvailable(format!(
                        "Model '{}' not found in any provider",
                        model_id
                    ))
                })
            }
            RoutingMode::ForceProvider(provider_id) => providers
                .into_iter()
                .find(|p| p.id == *provider_id)
                .ok_or_else(|| {
                    RouterError::NoProviderAvailable(format!(
                        "Provider '{}' not found",
                        provider_id
                    ))
                }),
            RoutingMode::LocalOnly => {
                let local: Vec<ProviderInfo> = providers
                    .into_iter()
                    .filter(|p| matches!(p.kind, voxy_provider_core::ProviderKind::Local))
                    .collect();
                if local.is_empty() {
                    Err(RouterError::NoProviderAvailable(
                        "No local provider available".into(),
                    ))
                } else {
                    Ok(local.into_iter().next().unwrap())
                }
            }
            RoutingMode::CloudOnly => {
                let cloud: Vec<ProviderInfo> = providers
                    .into_iter()
                    .filter(|p| matches!(p.kind, voxy_provider_core::ProviderKind::Cloud))
                    .collect();
                if cloud.is_empty() {
                    Err(RouterError::NoProviderAvailable(
                        "No cloud provider available".into(),
                    ))
                } else {
                    Ok(cloud.into_iter().next().unwrap())
                }
            }
            RoutingMode::Auto => {
                let discovery = DefaultCapabilityDiscovery::new(providers);
                if let Some(best_match) = discovery.find_best_match(capability) {
                    let id = best_match.provider_id.clone();
                    self.registry
                        .get(&id)
                        .await
                        .map_err(|e| RouterError::RoutingFailed(e.to_string()))
                } else {
                    Err(RouterError::NoProviderAvailable(format!(
                        "No suitable provider for {:?}",
                        capability
                    )))
                }
            }
        };

        let mut provider = match best {
            Ok(p) => p,
            Err(e) => return Err(e),
        };

        let all_providers = self
            .registry
            .find_by_capability(capability.clone())
            .await
            .unwrap_or_default();
        let all_healthy = self.filter_healthy(all_providers);

        let cfg = self.config.read().clone();
        let max_retries = cfg.max_retries;
        let retry_delay_ms = cfg.retry_delay_ms;
        drop(cfg);

        for attempt in 0..max_retries.saturating_add(1) {
            if self.is_breaker_open(&provider.id) {
                provider = match self.select_fallback(all_healthy.clone()).await {
                    Ok(p) => p,
                    Err(e) => {
                        if attempt < max_retries {
                            tokio::time::sleep(Duration::from_millis(
                                retry_delay_ms * (1u64 << attempt),
                            ))
                            .await;
                            continue;
                        }
                        return Err(e);
                    }
                };
            }

            let result = self
                .registry
                .get(&provider.id)
                .await
                .map_err(|e| RouterError::RoutingFailed(e.to_string()));

            match result {
                Ok(p) => {
                    self.record_success(&p.id);
                    return Ok(p);
                }
                Err(e) => {
                    self.record_failure(&provider.id);
                    if attempt < max_retries {
                        tokio::time::sleep(Duration::from_millis(
                            retry_delay_ms * (1u64 << attempt),
                        ))
                        .await;
                        provider = match self.select_fallback(all_healthy.clone()).await {
                            Ok(p) => p,
                            Err(fallback_err) => {
                                if attempt >= max_retries.saturating_sub(1) {
                                    return Err(fallback_err);
                                }
                                continue;
                            }
                        };
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(RouterError::AllProvidersExhausted(
            "retries exhausted".into(),
        ))
    }

    pub async fn complete(&self, provider: &dyn LlmProvider, prompt: &str) -> Result<String> {
        let timeout = {
            let config = self.config.read();
            Duration::from_secs(config.provider_timeout_secs)
        };
        let result = tokio::time::timeout(timeout, provider.complete(prompt)).await;
        match result {
            Ok(Ok(text)) => Ok(text),
            Ok(Err(e)) => Err(RouterError::ProviderError(format!(
                "LLM completion failed: {}",
                e
            ))),
            Err(_) => Err(RouterError::ProviderError(
                "LLM completion timed out".into(),
            )),
        }
    }

    pub async fn embed(&self, provider: &dyn EmbeddingProvider, text: &str) -> Result<Vec<f32>> {
        let timeout = {
            let config = self.config.read();
            Duration::from_secs(config.provider_timeout_secs)
        };
        let result = tokio::time::timeout(timeout, provider.embed(text)).await;
        match result {
            Ok(Ok(vec)) => Ok(vec),
            Ok(Err(e)) => Err(RouterError::ProviderError(format!(
                "Embedding failed: {}",
                e
            ))),
            Err(_) => Err(RouterError::ProviderError("Embedding timed out".into())),
        }
    }

    pub fn registry(&self) -> &Arc<dyn ProviderRegistry> {
        &self.registry
    }

    pub fn register_llm(&self, id: &str, provider: Arc<dyn LlmProvider>) {
        self.llm_providers.write().insert(id.to_string(), provider);
    }

    pub fn register_stt(&self, id: &str, provider: Arc<dyn SttProvider>) {
        self.stt_providers.write().insert(id.to_string(), provider);
    }

    pub fn register_tts(&self, id: &str, provider: Arc<dyn TtsProvider>) {
        self.tts_providers.write().insert(id.to_string(), provider);
    }

    pub async fn route_complete(&self, prompt: &str) -> Result<String> {
        let provider_info = self.select_provider(&ProviderCapability::Llm).await?;
        let maybe_inst = {
            self.llm_providers.read().get(&provider_info.id).cloned()
        };
        if let Some(inst) = maybe_inst {
            match self.complete(inst.as_ref(), prompt).await {
                Ok(res) => {
                    self.record_success(&provider_info.id);
                    Ok(res)
                }
                Err(e) => {
                    self.record_failure(&provider_info.id);
                    // Automatic fallback: try another healthy provider
                    let all = self.registry.find_by_capability(ProviderCapability::Llm).await.unwrap_or_default();
                    let healthy = self.filter_healthy(all);
                    for fallback_info in healthy {
                        if fallback_info.id != provider_info.id {
                            if let Some(fb_inst) = self.llm_providers.read().get(&fallback_info.id).cloned() {
                                if let Ok(res) = self.complete(fb_inst.as_ref(), prompt).await {
                                    self.record_success(&fallback_info.id);
                                    return Ok(res);
                                }
                            }
                        }
                    }
                    Err(e)
                }
            }
        } else {
            Err(RouterError::ProviderError(format!("Provider '{}' registered in registry but no implementation loaded in router", provider_info.id)))
        }
    }

    pub async fn route_transcribe(&self, audio: &[u8]) -> Result<String> {
        let provider_info = self.select_provider(&ProviderCapability::Stt).await?;
        let timeout = {
            let config = self.config.read();
            Duration::from_secs(config.provider_timeout_secs)
        };
        let maybe_inst = {
            self.stt_providers.read().get(&provider_info.id).cloned()
        };
        if let Some(inst) = maybe_inst {
            let res = tokio::time::timeout(timeout, inst.transcribe(audio)).await;
            match res {
                Ok(Ok(text)) => {
                    self.record_success(&provider_info.id);
                    Ok(text)
                }
                Ok(Err(e)) => {
                    self.record_failure(&provider_info.id);
                    // Failover to local/healthy STT
                    let all = self.registry.find_by_capability(ProviderCapability::Stt).await.unwrap_or_default();
                    let healthy = self.filter_healthy(all);
                    for fallback in healthy {
                        if fallback.id != provider_info.id {
                            if let Some(fb_inst) = self.stt_providers.read().get(&fallback.id).cloned() {
                                if let Ok(Ok(text)) = tokio::time::timeout(timeout, fb_inst.transcribe(audio)).await {
                                    self.record_success(&fallback.id);
                                    return Ok(text);
                                }
                            }
                        }
                    }
                    Err(RouterError::ProviderError(format!("STT failed: {e}")))
                }
                Err(_) => {
                    self.record_failure(&provider_info.id);
                    Err(RouterError::ProviderError("STT timed out".into()))
                }
            }
        } else {
            Err(RouterError::ProviderError(format!("STT provider '{}' not loaded", provider_info.id)))
        }
    }

    pub async fn route_synthesize(&self, text: &str) -> Result<Vec<u8>> {
        let provider_info = self.select_provider(&ProviderCapability::Tts).await?;
        let timeout = {
            let config = self.config.read();
            Duration::from_secs(config.provider_timeout_secs)
        };
        let maybe_inst = {
            self.tts_providers.read().get(&provider_info.id).cloned()
        };
        if let Some(inst) = maybe_inst {
            let res = tokio::time::timeout(timeout, inst.synthesize(text)).await;
            match res {
                Ok(Ok(bytes)) => {
                    self.record_success(&provider_info.id);
                    Ok(bytes)
                }
                Ok(Err(e)) => {
                    self.record_failure(&provider_info.id);
                    // Failover to local/healthy TTS
                    let all = self.registry.find_by_capability(ProviderCapability::Tts).await.unwrap_or_default();
                    let healthy = self.filter_healthy(all);
                    for fallback in healthy {
                        if fallback.id != provider_info.id {
                            if let Some(fb_inst) = self.tts_providers.read().get(&fallback.id).cloned() {
                                if let Ok(Ok(bytes)) = tokio::time::timeout(timeout, fb_inst.synthesize(text)).await {
                                    self.record_success(&fallback.id);
                                    return Ok(bytes);
                                }
                            }
                        }
                    }
                    Err(RouterError::ProviderError(format!("TTS failed: {e}")))
                }
                Err(_) => {
                    self.record_failure(&provider_info.id);
                    Err(RouterError::ProviderError("TTS timed out".into()))
                }
            }
        } else {
            Err(RouterError::ProviderError(format!("TTS provider '{}' not loaded", provider_info.id)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxy_provider_core::{
        DefaultProviderRegistry, ModelInfo, ProviderHealth, ProviderKind, ProviderStatus,
    };

    fn make_provider(
        id: &str,
        kind: ProviderKind,
        cap: ProviderCapability,
        status: ProviderStatus,
        priority: u32,
    ) -> ProviderInfo {
        ProviderInfo {
            id: id.to_string(),
            name: id.to_string(),
            kind,
            capability: cap,
            status,
            models: vec![ModelInfo::new(id, id)],
            health: ProviderHealth::new_healthy(Some(10.0)),
            base_url: None,
            priority,
        }
    }

    #[tokio::test]
    async fn test_router_selects_local_first() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        registry
            .register(make_provider(
                "cloud-llm",
                ProviderKind::Cloud,
                ProviderCapability::Llm,
                ProviderStatus::Available,
                0,
            ))
            .await
            .unwrap();
        registry
            .register(make_provider(
                "local-llm",
                ProviderKind::Local,
                ProviderCapability::Llm,
                ProviderStatus::Available,
                10,
            ))
            .await
            .unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::Auto);
        let selected = router
            .select_provider(&ProviderCapability::Llm)
            .await
            .unwrap();
        assert_eq!(selected.id, "local-llm");
    }

    #[tokio::test]
    async fn test_router_force_model() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        registry
            .register(make_provider(
                "ollama",
                ProviderKind::Local,
                ProviderCapability::Llm,
                ProviderStatus::Available,
                0,
            ))
            .await
            .unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::ForceModel("ollama".into()));
        let selected = router.select_provider(&ProviderCapability::Llm).await;
        assert!(selected.is_ok());
    }

    #[tokio::test]
    async fn test_router_force_model_not_found() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        registry
            .register(make_provider(
                "ollama",
                ProviderKind::Local,
                ProviderCapability::Llm,
                ProviderStatus::Available,
                0,
            ))
            .await
            .unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::ForceModel("nonexistent".into()));
        let result = router.select_provider(&ProviderCapability::Llm).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_router_local_only() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        registry
            .register(make_provider(
                "local-llm",
                ProviderKind::Local,
                ProviderCapability::Llm,
                ProviderStatus::Available,
                0,
            ))
            .await
            .unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::LocalOnly);
        let selected = router
            .select_provider(&ProviderCapability::Llm)
            .await
            .unwrap();
        assert_eq!(selected.id, "local-llm");
    }

    #[tokio::test]
    async fn test_router_local_only_no_local() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        registry
            .register(make_provider(
                "cloud-llm",
                ProviderKind::Cloud,
                ProviderCapability::Llm,
                ProviderStatus::Available,
                0,
            ))
            .await
            .unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::LocalOnly);
        let result = router.select_provider(&ProviderCapability::Llm).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_router_cloud_only() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        registry
            .register(make_provider(
                "cloud-llm",
                ProviderKind::Cloud,
                ProviderCapability::Llm,
                ProviderStatus::Available,
                0,
            ))
            .await
            .unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::CloudOnly);
        let selected = router
            .select_provider(&ProviderCapability::Llm)
            .await
            .unwrap();
        assert_eq!(selected.id, "cloud-llm");
    }

    #[tokio::test]
    async fn test_router_no_providers() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        let router = ModelRouter::new(registry);
        let result = router.select_provider(&ProviderCapability::Llm).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_routing_mode_default() {
        assert!(matches!(RoutingMode::default(), RoutingMode::Auto));
    }

    #[test]
    fn test_router_config_default() {
        let config = RouterConfig::default();
        assert!(config.local_first);
        assert!(matches!(config.mode, RoutingMode::Auto));
    }

    struct MockLlm {
        name: String,
        should_fail: std::sync::atomic::AtomicBool,
    }
    #[async_trait::async_trait]
    impl LlmProvider for MockLlm {
        async fn complete(&self, prompt: &str) -> voxy_provider_core::Result<String> {
            if self.should_fail.load(std::sync::atomic::Ordering::Relaxed) {
                Err(voxy_provider_core::ProviderError::RequestFailed("Simulated API failure".into()))
            } else {
                Ok(format!("Mock response from {} to: {}", self.name, prompt))
            }
        }
        fn available_models(&self) -> Vec<String> {
            vec!["mock".into()]
        }
        fn name(&self) -> &str {
            &self.name
        }
    }

    struct MockStt {
        name: String,
        should_fail: std::sync::atomic::AtomicBool,
    }
    #[async_trait::async_trait]
    impl SttProvider for MockStt {
        async fn transcribe(&self, _audio: &[u8]) -> voxy_provider_core::Result<String> {
            if self.should_fail.load(std::sync::atomic::Ordering::Relaxed) {
                Err(voxy_provider_core::ProviderError::RequestFailed("Simulated STT failure".into()))
            } else {
                Ok(format!("Transcribed by {}", self.name))
            }
        }
        fn supported_languages(&self) -> Vec<String> {
            vec!["en".into(), "hi".into()]
        }
        fn name(&self) -> &str {
            &self.name
        }
    }

    struct MockTts {
        name: String,
        should_fail: std::sync::atomic::AtomicBool,
    }
    #[async_trait::async_trait]
    impl TtsProvider for MockTts {
        async fn synthesize(&self, text: &str) -> voxy_provider_core::Result<Vec<u8>> {
            if self.should_fail.load(std::sync::atomic::Ordering::Relaxed) {
                Err(voxy_provider_core::ProviderError::RequestFailed("Simulated TTS failure".into()))
            } else {
                Ok(format!("Audio from {}: {}", self.name, text).into_bytes())
            }
        }
        fn list_voices(&self) -> Vec<String> {
            vec!["default".into()]
        }
        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_route_complete_with_fallback() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        let primary_info = make_provider(
            "cloud-llm",
            ProviderKind::Cloud,
            ProviderCapability::Llm,
            ProviderStatus::Available,
            1,
        );
        let fallback_info = make_provider(
            "local-llm",
            ProviderKind::Local,
            ProviderCapability::Llm,
            ProviderStatus::Available,
            10,
        );
        registry.register(primary_info).await.unwrap();
        registry.register(fallback_info).await.unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::CloudOnly);

        let primary_inst = Arc::new(MockLlm {
            name: "Cloud LLM".into(),
            should_fail: std::sync::atomic::AtomicBool::new(true), // primary fails!
        });
        let fallback_inst = Arc::new(MockLlm {
            name: "Local LLM".into(),
            should_fail: std::sync::atomic::AtomicBool::new(false),
        });

        router.register_llm("cloud-llm", primary_inst);
        router.register_llm("local-llm", fallback_inst);

        let response = router.route_complete("hello").await.unwrap();
        assert!(response.contains("Local LLM"), "Expected fallback response but got: {response}");
    }

    #[tokio::test]
    async fn test_route_transcribe_with_fallback() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        let primary_info = make_provider(
            "cloud-stt",
            ProviderKind::Cloud,
            ProviderCapability::Stt,
            ProviderStatus::Available,
            1,
        );
        let fallback_info = make_provider(
            "local-stt",
            ProviderKind::Local,
            ProviderCapability::Stt,
            ProviderStatus::Available,
            10,
        );
        registry.register(primary_info).await.unwrap();
        registry.register(fallback_info).await.unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::CloudOnly);

        let primary_inst = Arc::new(MockStt {
            name: "Cloud STT".into(),
            should_fail: std::sync::atomic::AtomicBool::new(true), // cloud fails
        });
        let fallback_inst = Arc::new(MockStt {
            name: "Local STT".into(),
            should_fail: std::sync::atomic::AtomicBool::new(false),
        });

        router.register_stt("cloud-stt", primary_inst);
        router.register_stt("local-stt", fallback_inst);

        let transcript = router.route_transcribe(b"test audio").await.unwrap();
        assert!(transcript.contains("Local STT"), "Expected fallback STT but got: {transcript}");
    }

    #[tokio::test]
    async fn test_route_synthesize_with_fallback() {
        let registry = Arc::new(DefaultProviderRegistry::new());
        let primary_info = make_provider(
            "cloud-tts",
            ProviderKind::Cloud,
            ProviderCapability::Tts,
            ProviderStatus::Available,
            1,
        );
        let fallback_info = make_provider(
            "local-tts",
            ProviderKind::Local,
            ProviderCapability::Tts,
            ProviderStatus::Available,
            10,
        );
        registry.register(primary_info).await.unwrap();
        registry.register(fallback_info).await.unwrap();

        let router = ModelRouter::new(registry);
        router.set_mode(RoutingMode::CloudOnly);

        let primary_inst = Arc::new(MockTts {
            name: "Cloud TTS".into(),
            should_fail: std::sync::atomic::AtomicBool::new(true), // cloud fails
        });
        let fallback_inst = Arc::new(MockTts {
            name: "Local TTS".into(),
            should_fail: std::sync::atomic::AtomicBool::new(false),
        });

        router.register_tts("cloud-tts", primary_inst);
        router.register_tts("local-tts", fallback_inst);

        let audio = router.route_synthesize("Hello world").await.unwrap();
        let audio_str = String::from_utf8_lossy(&audio);
        assert!(audio_str.contains("Local TTS"), "Expected fallback TTS but got: {audio_str}");
    }
}
