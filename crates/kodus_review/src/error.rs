use crate::provider_bridge::ProviderBridgeError;
use crate::security::KodusSecurityError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, KodusReviewError>;

#[derive(Debug, Error)]
pub enum KodusReviewError {
    #[error("Security violation: {0}")]
    Security(#[from] KodusSecurityError),

    #[error("Provider bridge error: {0}")]
    Provider(#[from] ProviderBridgeError),

    #[error("Diff resolution error: {0}")]
    DiffError(String),

    #[error("Remediation loop error: {0}")]
    RemediationError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
