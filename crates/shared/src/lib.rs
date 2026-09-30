//! Common types, traits, error model, and events for VOXY.
//!
//! This crate provides the foundational types that all other VOXY crates depend on.
//! It should have zero dependencies on other VOXY crates.

pub mod error;
pub mod event;
pub mod production;
pub mod providers;
pub mod traits;
pub mod types;
pub mod version;

pub use error::{ErrorKind, Result, Severity, VoxyError};
pub use event::{Event, Priority, TypedEvent};
pub use production::{ErrorContext, RetryPolicy};
pub use providers::{
    AuthContext, AuthProvider, AuthResult, BillingProvider, Credentials, LocalAuthProvider,
    OfflineBillingProvider, OfflineSyncProvider, Subscription, SubscriptionStatus, SyncConflict,
    SyncProvider, TokenPair, TrustLevel, UsageStats,
};
pub use traits::{Configurable, HealthStatus, Lifecycle};
pub use types::Rect;
pub use version::{BuildInfo, VersionInfo};

/// Get the current VOXY version.
pub fn version() -> VersionInfo {
    version::version()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_correct() {
        let v = version();
        assert_eq!(v.major(), 0);
        assert_eq!(v.minor(), 1);
        assert_eq!(v.patch(), 0);
    }

    #[test]
    fn test_requirements_schema_and_integrity() {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
        let req_path = std::path::Path::new(&manifest_dir).join("../../REQUIREMENTS.json");
        if !req_path.exists() {
            // Check current directory fallback
            let local_path = std::path::Path::new("REQUIREMENTS.json");
            if !local_path.exists() {
                return; // skipped if path resolution differs in isolated test runners
            }
        }
        let content = std::fs::read_to_string(&req_path)
            .or_else(|_| std::fs::read_to_string("REQUIREMENTS.json"))
            .expect("REQUIREMENTS.json must be readable");

        let json: serde_json::Value =
            serde_json::from_str(&content).expect("REQUIREMENTS.json must be valid JSON");

        let reqs = json
            .get("requirements")
            .and_then(|r| r.as_array())
            .expect("requirements must be an array");
        assert!(!reqs.is_empty(), "requirements array must not be empty");

        let valid_stages = [
            "PROPOSED",
            "ANALYZED",
            "PLANNED",
            "IMPLEMENTING",
            "INTEGRATED",
            "TESTING",
            "SECURITY_REVIEW",
            "VERIFIED",
            "RELEASED",
        ];

        let mut ids = std::collections::HashSet::new();
        for req in reqs {
            let id = req["id"].as_str().expect("id must be string");
            assert!(!id.is_empty(), "id cannot be empty");
            assert!(
                ids.insert(id.to_string()),
                "duplicate requirement id: {}",
                id
            );

            let state = req["implementation_state"]
                .as_str()
                .expect("state must be string");
            assert!(valid_stages.contains(&state), "invalid state: {}", state);

            let priority = req["priority"].as_str().expect("priority must be string");
            assert!(
                matches!(priority, "P0" | "P1" | "P2"),
                "invalid priority: {}",
                priority
            );

            let tests = req["tests"].as_array().expect("tests must be array");
            assert!(!tests.is_empty(), "requirement {} must define tests", id);
        }
    }
}
