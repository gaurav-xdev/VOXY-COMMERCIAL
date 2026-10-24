//! OSMOO Internal Kodus AI Code Review Engine.
//!
//! Provides a sovereign, internal-only automated code review system powered by Kodus AI
//! contracts, integrated directly into OSMOO's autonomous coding harness and model router.
//!
//! # Security Boundary
//! Accessible exclusively to OSMOO's autonomous agent engine via internal cryptographic tokens.
//! Direct user invocations, UI administration, and raw endpoints are strictly blocked.

pub mod engine;
pub mod error;
pub mod prompts;
pub mod provider_bridge;
pub mod remediation_loop;
pub mod security;
pub mod tool;
pub mod types;

pub use engine::{deduplicate_findings, extract_changed_files, parse_kodus_findings, KodusReviewEngine};
pub use error::{KodusReviewError, Result};
pub use provider_bridge::KodusProviderBridge;
pub use remediation_loop::{KodusRemediationCoordinator, DEFAULT_MAX_REMEDIATION_ITERATIONS};
pub use security::{KodusSecurityBoundary, OSMOO_OPERATOR_IDENTITY};
pub use tool::KodusInternalReviewTool;
pub use types::{
    KodusCategory, KodusFinding, KodusReviewReport, KodusSeverity, RemediationStatus, ReviewScope,
    ReviewTrigger,
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use voxy_model_router::ModelRouter;
    use voxy_provider_core::{
        DefaultProviderRegistry, LlmProvider, ModelInfo, ProviderCapability, ProviderHealth,
        ProviderInfo, ProviderKind, ProviderStatus,
    };
    use voxy_tool_calling::{Tool, ToolContext};

    struct MockLlm {
        name: String,
        response_text: String,
        should_fail: bool,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlm {
        async fn complete(&self, _prompt: &str) -> voxy_provider_core::Result<String> {
            if self.should_fail {
                Err(voxy_provider_core::ProviderError::RequestFailed(
                    "Simulated provider outage".into(),
                ))
            } else {
                Ok(self.response_text.clone())
            }
        }

        fn available_models(&self) -> Vec<String> {
            vec!["mock-model".into()]
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    fn create_test_router(mock_response: &str, should_fail: bool) -> Arc<ModelRouter> {
        let registry = Arc::new(DefaultProviderRegistry::new());
        let router = Arc::new(ModelRouter::new(registry.clone()));

        let mock_provider = Arc::new(MockLlm {
            name: "TestOpenAI".into(),
            response_text: mock_response.into(),
            should_fail,
        });

        let info = ProviderInfo {
            id: "openai".into(),
            name: "Test OpenAI Provider".into(),
            kind: ProviderKind::Cloud,
            capability: ProviderCapability::Llm,
            status: ProviderStatus::Available,
            models: vec![ModelInfo::new("gpt-4o", "GPT-4o")],
            health: ProviderHealth::new_healthy(None),
            base_url: None,
            priority: 1,
        };

        registry.register_sync(info);
        router.register_llm("openai", mock_provider);
        router
    }

    #[test]
    fn test_security_blocks_direct_user_access() {
        let boundary = KodusSecurityBoundary::new();
        let token = boundary.session_token();

        // 1. Direct user cannot access Kodus
        let user_err = boundary.verify_caller("user", &token).unwrap_err();
        assert!(format!("{user_err}").contains("Direct user or external access forbidden"));

        // 2. Desktop UI client cannot access Kodus directly
        let ui_err = boundary.verify_caller("desktop_ui_user", &token).unwrap_err();
        assert!(format!("{ui_err}").contains("Direct user or external access forbidden"));

        // 3. Fake token is rejected
        let token_err = boundary
            .verify_caller(OSMOO_OPERATOR_IDENTITY, "invalid-token")
            .unwrap_err();
        assert!(matches!(token_err, security::KodusSecurityError::InvalidToken));

        // 4. Autonomous OSMOO operator with valid token is authorized
        assert!(boundary.verify_caller(OSMOO_OPERATOR_IDENTITY, &token).is_ok());
    }

    #[test]
    fn test_secret_redaction_prevents_credential_leakage() {
        let boundary = KodusSecurityBoundary::new();
        let text_with_keys = "Connecting using sk-abcdef1234567890abcdef1234567890 and kodus_secret_token_12345678901234567890";
        let clean = boundary.redact_secrets(text_with_keys);

        assert!(!clean.contains("sk-abcdef1234567890abcdef1234567890"));
        assert!(!clean.contains("kodus_secret_token_12345678901234567890"));
        assert!(clean.contains("[REDACTED_API_KEY]"));
    }

    #[test]
    fn test_findings_parsing_and_deduplication() {
        let raw_json = r#"{
            "summary": "Detected critical logic error in auth module",
            "findings": [
                {
                    "file": "crates/security/src/auth.rs",
                    "line": 42,
                    "end_line": 45,
                    "severity": "critical",
                    "category": "security_vulnerability",
                    "title": "Unauthenticated pipe access",
                    "what": "Named pipe accepts connections without DACL",
                    "why": "Allows local privilege escalation",
                    "how": "Set security descriptor",
                    "is_confirmed": true,
                    "confidence_score": 0.98
                },
                {
                    "file": "crates/security/src/auth.rs",
                    "line": 42,
                    "end_line": 45,
                    "severity": "critical",
                    "category": "security_vulnerability",
                    "title": "Duplicate finding",
                    "what": "Named pipe accepts connections without DACL",
                    "why": "Allows local privilege escalation",
                    "how": "Set security descriptor",
                    "is_confirmed": true,
                    "confidence_score": 0.98
                }
            ]
        }"#;

        let mut findings = parse_kodus_findings(raw_json);
        assert_eq!(findings.len(), 2, "Initially parsed 2 findings");

        deduplicate_findings(&mut findings);
        assert_eq!(findings.len(), 1, "Deduplication reduced to 1 unique finding");
        assert_eq!(findings[0].severity, KodusSeverity::Critical);
        assert!(findings[0].severity.is_blocking());
    }

    #[tokio::test]
    async fn test_provider_bridge_reuses_osmoo_credentials_cleanly() {
        let mock_json = r#"{"summary": "Clean", "findings": []}"#;
        let router = create_test_router(mock_json, false);
        let security = KodusSecurityBoundary::new();
        let bridge = KodusProviderBridge::new(router, security);

        let (resp, provider) = bridge
            .complete_review_prompt("system prompt", "user diff")
            .await
            .expect("Completion succeeded");

        assert!(resp.contains("Clean"));
        assert_eq!(provider, "Test OpenAI Provider");
    }

    #[tokio::test]
    async fn test_kodus_tool_blocks_user_and_allows_osmoo() {
        let mock_json = r#"{"summary": "Clean code", "findings": []}"#;
        let router = create_test_router(mock_json, false);
        let security = KodusSecurityBoundary::new();
        let token = security.session_token();
        let bridge = KodusProviderBridge::new(router, security.clone());
        let engine = Arc::new(KodusReviewEngine::new(".", bridge, security));
        let tool = KodusInternalReviewTool::new(engine);
        let ctx = ToolContext::new("session-1");

        // 1. Direct user call fails closed
        let user_params = json!({
            "caller_identity": "regular_user",
            "internal_token": token
        });
        let res = tool.execute(user_params, &ctx).await.unwrap();
        assert!(!res.success);
        assert!(res.error.unwrap().contains("ACCESS DENIED"));

        // 2. OSMOO operator call succeeds
        let osmoo_params = json!({
            "caller_identity": OSMOO_OPERATOR_IDENTITY,
            "internal_token": token,
            "scope": "staged"
        });
        let res = tool.execute(osmoo_params, &ctx).await.unwrap();
        assert!(res.success);
    }

    #[tokio::test]
    async fn test_remediation_loop_bounds_retries() {
        let mock_json = r#"{
            "summary": "Persistent bug",
            "findings": [{
                "file": "crates/foo/src/lib.rs",
                "line": 10,
                "end_line": 12,
                "severity": "critical",
                "category": "logic_bug",
                "title": "Loop bug",
                "what": "Always fails",
                "why": "Never fixed",
                "how": "Fix",
                "is_confirmed": true,
                "confidence_score": 0.9
            }]
        }"#;

        let router = create_test_router(mock_json, false);
        let security = KodusSecurityBoundary::new();
        let token = security.session_token();
        let bridge = KodusProviderBridge::new(router, security.clone());
        let engine = Arc::new(KodusReviewEngine::new(".", bridge, security));
        let coordinator = KodusRemediationCoordinator::new(engine).with_max_iterations(2);

        let (status, _report) = coordinator
            .run_loop(
                ReviewScope::WorkingTree,
                &token,
                |_finding| Ok(true), // Simulates applying fix
                || Ok(false),        // Tests keep failing
            )
            .await
            .unwrap();

        match status {
            RemediationStatus::MaxIterationsReached { iterations, remaining_findings } => {
                assert_eq!(iterations, 2);
                assert_eq!(remaining_findings, 1);
            }
            _ => panic!("Expected MaxIterationsReached status, got {:?}", status),
        }
    }
}
