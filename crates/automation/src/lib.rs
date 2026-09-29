#![allow(
    dead_code,
    non_snake_case,
    clippy::upper_case_acronyms,
    clippy::new_without_default,
    clippy::type_complexity,
    clippy::redundant_closure,
    clippy::question_mark,
    clippy::redundant_pattern_matching,
    clippy::clone_on_copy,
    clippy::manual_div_ceil
)]

pub mod backends;
pub mod config;
pub mod cursor;
pub mod error;

pub use backends::{
    hybrid::HybridBackend, hybrid::HybridBuilder, hybrid::HybridConfig, openclaw::OpenClawBackend,
    recovery::RecoveryEngine, verification::VerificationEngine, windows_uia::WindowsUiaBackend,
};
pub use config::AutomationConfig;
pub use cursor::{
    AutomationAction, ConfirmationStatus, DestructiveActionGuard, RiskEvaluation, RiskLevel,
    VoxyCursorController,
};
pub use error::{AutomationError, Result};

#[cfg(test)]
mod tests {
    use super::*;
    use voxy_orchestrator::automation::AutomationBackend;

    #[test]
    fn test_automation_error_action_failed() {
        let err = AutomationError::ActionFailed("test".into());
        assert_eq!(format!("{}", err), "Action failed: test");
    }

    #[test]
    fn test_automation_error_element_not_found() {
        let err = AutomationError::ElementNotFound("btn".into());
        assert_eq!(format!("{}", err), "Element not found: btn");
    }

    #[test]
    fn test_automation_error_backend_unavailable() {
        let err = AutomationError::BackendUnavailable("no backend".into());
        assert_eq!(format!("{}", err), "Backend unavailable: no backend");
    }

    #[test]
    fn test_automation_error_verification_failed() {
        let err = AutomationError::VerificationFailed("mismatch".into());
        assert_eq!(format!("{}", err), "Verification failed: mismatch");
    }

    #[test]
    fn test_automation_error_timeout() {
        let err = AutomationError::Timeout("5000ms".into());
        assert_eq!(format!("{}", err), "Timeout: 5000ms");
    }

    #[test]
    fn test_automation_error_ocr_failed() {
        let err = AutomationError::OcrFailed("no text".into());
        assert_eq!(format!("{}", err), "OCR failed: no text");
    }

    #[test]
    fn test_automation_error_unsupported() {
        let err = AutomationError::UnsupportedOperation("gpu".into());
        assert_eq!(format!("{}", err), "Unsupported operation: gpu");
    }

    #[test]
    fn test_automation_error_initialization_failed() {
        let err = AutomationError::InitializationFailed("config".into());
        assert!(format!("{}", err).contains("Initialization failed"));
    }

    #[test]
    fn test_automation_error_cancelled() {
        let err = AutomationError::Cancelled("user".into());
        assert_eq!(format!("{}", err), "Cancelled: user");
    }

    #[test]
    fn test_automation_error_to_orchestrator() {
        let auto = AutomationError::Timeout("test".into());
        let orch: voxy_orchestrator::OrchestratorError = auto.into();
        let s = format!("{}", orch);
        assert!(s.contains("Timeout"));
    }

    #[tokio::test]
    async fn test_windows_uia_backend_creates() {
        let backend = WindowsUiaBackend::new();
        assert_eq!(backend.name().await, "windows-uia");
    }

    #[tokio::test]
    async fn test_openclaw_backend_creates() {
        let backend = OpenClawBackend::default();
        assert_eq!(backend.name().await, "openclaw");
    }

    #[tokio::test]
    async fn test_hybrid_backend_creates() {
        let uia = std::sync::Arc::new(WindowsUiaBackend::new());
        let backend = HybridBackend::builder().with_primary(uia).build().unwrap();
        assert_eq!(backend.name().await, "hybrid");
    }

    #[test]
    fn test_automation_config_default() {
        let config = AutomationConfig::default();
        assert_eq!(config.default_backend, config::BackendType::Hybrid);
        assert_eq!(config.retry_policy.max_retries, 3);
        assert_eq!(config.retry_policy.base_delay_ms, 100);
        assert_eq!(config.timeouts.element_wait_ms, 5000);
        assert_eq!(config.timeouts.window_wait_ms, 10000);
        assert_eq!(config.timeouts.action_ms, 30000);
        assert!(config.dpi_aware);
        assert!(config.multi_monitor);
    }

    #[test]
    fn test_retry_policy_backoff() {
        let policy = config::RetryPolicy {
            max_retries: 5,
            base_delay_ms: 100,
            max_delay_ms: 10000,
            backoff_factor: 2.0,
        };
        assert_eq!(policy.max_retries, 5);
        assert_eq!(policy.backoff_factor, 2.0);
    }

    #[test]
    fn test_hybrid_config_default() {
        let config = HybridConfig::default();
        assert_eq!(config.primary_backend_name, "windows-uia");
        assert!(config.fallback_on_failure);
        assert_eq!(config.latency_threshold_ms, 100);
    }

    #[tokio::test]
    async fn test_windows_uia_backend_is_not_available_on_non_windows() {
        let backend = WindowsUiaBackend::new();
        let available = backend.is_available().await;
        assert!(available == cfg!(windows));
    }

    #[tokio::test]
    async fn test_openclaw_backend_is_not_available_by_default() {
        let backend = OpenClawBackend::default();
        let available = backend.is_available().await;
        assert!(!available);
    }

    #[test]
    fn test_hybrid_backend_initialization_fails_without_backends() {
        let result = HybridBackend::builder().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_hybrid_builder_requires_backends() {
        let result = HybridBackend::builder().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_hybrid_builder_with_primary() {
        let uia = WindowsUiaBackend::new();
        let result = HybridBackend::builder()
            .with_primary(std::sync::Arc::new(uia))
            .build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_verification_engine_creates() {
        let uia = WindowsUiaBackend::new();
        let engine = VerificationEngine::new(std::sync::Arc::new(uia));
        assert_eq!(engine.name().await, "verification-engine");
    }

    #[tokio::test]
    async fn test_recovery_engine_creates() {
        let uia = WindowsUiaBackend::new();
        let engine = RecoveryEngine::new(std::sync::Arc::new(uia));
        assert_eq!(engine.name().await, "recovery-engine");
    }

    #[tokio::test]
    async fn test_windows_uia_backend_capabilities() {
        let backend = WindowsUiaBackend::new();
        let caps = backend.get_backend_capabilities().await;
        assert!(caps.contains(&voxy_orchestrator::automation::AutomationCapability::Mouse));
        assert!(caps.contains(&voxy_orchestrator::automation::AutomationCapability::Keyboard));
        assert!(caps.contains(&voxy_orchestrator::automation::AutomationCapability::ScreenCapture));
        assert!(
            caps.contains(&voxy_orchestrator::automation::AutomationCapability::WindowManagement)
        );
        assert!(
            caps.contains(&voxy_orchestrator::automation::AutomationCapability::ElementDetection)
        );
    }

    #[tokio::test]
    async fn test_openclaw_backend_capabilities() {
        let backend = OpenClawBackend::default();
        let caps = backend.get_backend_capabilities().await;
        assert!(caps.contains(&voxy_orchestrator::automation::AutomationCapability::Ocr));
    }

    #[test]
    fn test_error_helpers_return_orchestrator_error() {
        let e1 = crate::error::action_err("msg");
        assert!(format!("{}", e1).contains("Action failed"));

        let e2 = crate::error::timeout_err("msg");
        assert!(format!("{}", e2).contains("Timeout"));

        let e3 = crate::error::unsupported_err("msg");
        assert!(format!("{}", e3).contains("Unsupported"));

        let e4 = crate::error::unavail_err("msg");
        assert!(format!("{}", e4).contains("Unavailable"));

        let e5 = crate::error::not_found_err("msg");
        assert!(format!("{}", e5).contains("Not found"));
    }

    #[tokio::test]
    async fn test_verification_with_screenshots() {
        let uia = WindowsUiaBackend::new();
        let engine =
            VerificationEngine::new(std::sync::Arc::new(uia)).with_screenshots(true, false);
        let caps = engine.get_backend_capabilities().await;
        assert!(
            caps.contains(&voxy_orchestrator::automation::AutomationCapability::StateVerification)
        );
    }

    #[tokio::test]
    async fn test_hybrid_built_backend() {
        let uia = std::sync::Arc::new(WindowsUiaBackend::new());
        let hybrid = HybridBackend::builder().with_primary(uia).build().unwrap();
        assert_eq!(hybrid.name().await, "hybrid");
    }

    #[tokio::test]
    async fn test_recovery_engine_handles_error() {
        let uia = WindowsUiaBackend::new();
        let engine = RecoveryEngine::new(std::sync::Arc::new(uia));
        let caps = engine.get_backend_capabilities().await;
        assert!(caps.contains(&voxy_orchestrator::automation::AutomationCapability::Recovery));
    }
}
