//! Capability registry, tokens, policy engine, trust, identity, consent,
//! secrets, audit, threat detection, guardian, journal, rollback,
//! system prompts, input sanitization, signed artifacts, monitoring, and auto-defense.

pub mod audit;
pub mod auth;
pub mod capability;
pub mod consent;
pub mod defense;
pub mod error;
pub mod guardian;
pub mod identity;
pub mod integrity;
pub mod journal;
pub mod monitoring;
pub mod policy;
pub mod prompt;
pub mod recovery;
pub mod rollback;
pub mod sanitizer;
pub mod secrets;
pub mod signed;
pub mod threat;
pub mod token;
pub mod trust;

pub use auth::{AuthPasswordHasher, AuthRateLimiter, SessionTokenManager};
pub use audit::{AuditEntry, AuditLog, AuditEventType};
pub use capability::{Capability, CapabilityCategory, CapabilityRegistry, RiskLevel};
pub use consent::{ConsentManager, ConsentRequest};
pub use defense::{AutoDefense, DefenseAction, RiskProfile};
pub use error::{Result, SecurityError};
pub use guardian::{GuardianConfig, GuardianDecision, GuardianEngine};
pub use identity::{EntityType, Identity, IdentityManager};
pub use integrity::IntegrityVerifier;
pub use journal::AiActionJournal;
pub use monitoring::{ForensicsEvent, SecurityEventSeverity, SecurityMonitor};
pub use policy::{PolicyEngine, PolicyInput, PolicyResult, PolicyRule};
pub use prompt::SystemPromptBuilder;
pub use recovery::{RecoveryMode, RecoveryReport, RecoveryState};
pub use rollback::{RollbackManager, RollbackResult, RollbackStrategy};
pub use sanitizer::{sanitize_context, sanitize_llm_output, sanitize_user_input, SanitizedInput};
pub use secrets::SecretVault;
pub use signed::{ArtifactType, SignatureVerdict, SignatureVerifier, SignedArtifact};
pub use threat::{ThreatDetector, ThreatSeverity};
pub use token::{CapabilityToken, PermissionGrant};
pub use trust::{TrustLevel, TrustManager};
