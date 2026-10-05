use std::fmt;

#[derive(Debug)]
pub enum SecurityError {
    // Existing
    PermissionDenied(String),
    TokenExpired,
    InvalidToken(String),
    CapabilityNotFound(String),
    CapabilityAlreadyRegistered(String),
    MaximumGrantsExceeded,
    SerializationError(String),
    // Identity
    IdentityNotFound(String),
    IdentityAlreadyRegistered(String),
    // Consent
    ConsentRequestNotFound(String),
    // Secrets
    SecretNotFound(String),
    NoMasterKey,
    // Rollback
    RollbackFailed(String),
    RollbackExpired(String),
    // Guardian
    GuardianDenied(String),
    ThreatDetected(String),
    IntegrityViolation(String),
    // Policy
    PolicyError(String),
    // Approval
    ApprovalError(String),
    Internal(String),
}

impl fmt::Display for SecurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            Self::TokenExpired => write!(f, "Token expired"),
            Self::InvalidToken(msg) => write!(f, "Invalid token: {msg}"),
            Self::CapabilityNotFound(msg) => write!(f, "Capability not found: {msg}"),
            Self::CapabilityAlreadyRegistered(msg) => {
                write!(f, "Capability already registered: {msg}")
            }
            Self::MaximumGrantsExceeded => write!(f, "Maximum grants exceeded"),
            Self::SerializationError(msg) => write!(f, "Serialization error: {msg}"),
            Self::IdentityNotFound(msg) => write!(f, "Identity not found: {msg}"),
            Self::IdentityAlreadyRegistered(msg) => write!(f, "Identity already registered: {msg}"),
            Self::ConsentRequestNotFound(msg) => write!(f, "Consent request not found: {msg}"),
            Self::SecretNotFound(msg) => write!(f, "Secret not found: {msg}"),
            Self::NoMasterKey => write!(f, "No master key configured"),
            Self::RollbackFailed(msg) => write!(f, "Rollback failed: {msg}"),
            Self::RollbackExpired(msg) => write!(f, "Rollback expired: {msg}"),
            Self::GuardianDenied(msg) => write!(f, "Guardian denied: {msg}"),
            Self::ThreatDetected(msg) => write!(f, "Threat detected: {msg}"),
            Self::IntegrityViolation(msg) => write!(f, "Integrity violation: {msg}"),
            Self::PolicyError(msg) => write!(f, "Policy error: {msg}"),
            Self::ApprovalError(msg) => write!(f, "Approval error: {msg}"),
            Self::Internal(msg) => write!(f, "Internal security error: {msg}"),
        }
    }
}

impl std::error::Error for SecurityError {}

pub type Result<T> = std::result::Result<T, SecurityError>;
