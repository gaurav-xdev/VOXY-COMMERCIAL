use std::sync::Arc;
use voxy_model_router::{ModelRouter, RoutingMode};
use voxy_provider_core::{
    DefaultProviderRegistry, LlmProvider, ModelCapabilities, ModelInfo, ProviderCapability,
    ProviderHealth, ProviderInfo, ProviderKind, ProviderRegistry, ProviderStatus,
};

struct FailingCloudLlm;

#[async_trait::async_trait]
impl LlmProvider for FailingCloudLlm {
    async fn complete(&self, _prompt: &str) -> voxy_provider_core::Result<String> {
        Err(voxy_provider_core::ProviderError::RequestFailed(
            "Simulated Cloud Failure".into(),
        ))
    }
    fn available_models(&self) -> Vec<String> {
        vec!["claude-3-7-sonnet".into()]
    }
    fn name(&self) -> &str {
        "FailingCloudLlm"
    }
}

struct WorkingLocalLlm;

#[async_trait::async_trait]
impl LlmProvider for WorkingLocalLlm {
    async fn complete(&self, _prompt: &str) -> voxy_provider_core::Result<String> {
        Ok("Local LLM Response: 42".into())
    }
    fn available_models(&self) -> Vec<String> {
        vec!["qwen-coder-32b".into()]
    }
    fn name(&self) -> &str {
        "WorkingLocalLlm"
    }
}

#[tokio::test]
async fn test_failure_injection_proves_zero_fallback() {
    let cloud_info = ProviderInfo {
        id: "forced-cloud-provider".into(),
        name: "Forced Cloud".into(),
        kind: ProviderKind::Cloud,
        capability: ProviderCapability::Llm,
        status: ProviderStatus::Available,
        models: vec![ModelInfo {
            id: "claude-3-7-sonnet".into(),
            name: "Claude 3.7 Sonnet".into(),
            capabilities: vec![],
            model_capabilities: ModelCapabilities::default(),
            version: "1.0".into(),
            context_size: Some(200000),
            supports_vision: true,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_embeddings: false,
            metadata: std::collections::HashMap::new(),
        }],
        health: ProviderHealth {
            is_healthy: true,
            status: voxy_provider_core::HealthStatus::Healthy,
            last_check: chrono::Utc::now(),
            latency_ms: Some(15.0),
            details: None,
            consecutive_failures: 0,
            circuit_broken: false,
        },
        base_url: None,
        priority: 1,
    };

    let local_info = ProviderInfo {
        id: "local-fallback-provider".into(),
        name: "Local Qwen".into(),
        kind: ProviderKind::Local,
        capability: ProviderCapability::Llm,
        status: ProviderStatus::Available,
        models: vec![ModelInfo {
            id: "qwen-coder-32b".into(),
            name: "Qwen 2.5 Coder 32B".into(),
            capabilities: vec![],
            model_capabilities: ModelCapabilities::default(),
            version: "1.0".into(),
            context_size: Some(32768),
            supports_vision: false,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_embeddings: false,
            metadata: std::collections::HashMap::new(),
        }],
        health: ProviderHealth {
            is_healthy: true,
            status: voxy_provider_core::HealthStatus::Healthy,
            last_check: chrono::Utc::now(),
            latency_ms: Some(5.0),
            details: None,
            consecutive_failures: 0,
            circuit_broken: false,
        },
        base_url: None,
        priority: 10,
    };

    let registry = Arc::new(DefaultProviderRegistry::new());
    registry.register(cloud_info).await.unwrap();
    registry.register(local_info).await.unwrap();

    let router = ModelRouter::new(registry);
    router.register_llm("forced-cloud-provider", Arc::new(FailingCloudLlm));
    router.register_llm("local-fallback-provider", Arc::new(WorkingLocalLlm));

    // Configure STRICT ForceProvider mode
    router.set_mode(RoutingMode::ForceProvider("forced-cloud-provider".into()));

    // Trip circuit breaker on forced provider (5 failures)
    for _ in 0..5 {
        router.record_failure("forced-cloud-provider");
    }

    // Attempting completion MUST NOT secretly fall back to WorkingLocalLlm
    let select_res = router.select_provider(&ProviderCapability::Llm).await;
    assert!(
        select_res.is_err(),
        "Expected select_provider to fail when forced provider is broken, but got {:?}",
        select_res
    );

    let err_str = select_res.unwrap_err().to_string();
    assert!(
        err_str.contains("disallowed in ForceProvider mode"),
        "Expected error confirming zero fallback enforcement, got: {}",
        err_str
    );
}
