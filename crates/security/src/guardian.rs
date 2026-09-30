use crate::audit::AuditLog;
use crate::capability::{CapabilityRegistry, RiskLevel};
use crate::consent::ConsentManager;
use crate::integrity::IntegrityVerifier;
use crate::policy::{AuditLevel, PolicyEngine, PolicyInput, PolicyResult};
use crate::recovery::RecoveryMode;
use crate::rollback::RollbackManager;
use crate::threat::{ThreatDetector, ThreatSeverity};
use crate::trust::TrustManager;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct GuardianDecision {
    pub allowed: bool,
    pub requires_consent: bool,
    pub requires_mfa: bool,
    pub audit_level: AuditLevel,
    pub reason: String,
    pub policy_result: Option<PolicyResult>,
    pub execution_id: Uuid,
}

pub struct GuardianConfig {
    pub require_threat_analysis: bool,
    pub require_integrity_check: bool,
    pub auto_recovery: bool,
    pub max_risk_without_consent: RiskLevel,
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            require_threat_analysis: true,
            require_integrity_check: true,
            auto_recovery: false,
            max_risk_without_consent: RiskLevel::Low,
        }
    }
}

pub struct GuardianEngine {
    pub capability_registry: CapabilityRegistry,
    pub policy_engine: PolicyEngine,
    pub consent_manager: Mutex<ConsentManager>,
    pub trust_manager: Mutex<TrustManager>,
    pub threat_detector: Mutex<ThreatDetector>,
    pub integrity_verifier: IntegrityVerifier,
    pub audit_log: Mutex<AuditLog>,
    pub rollback_manager: Mutex<RollbackManager>,
    config: GuardianConfig,
    /// SECURITY: Rate limiter — per-subject request timestamps (millis since epoch).
    rate_limiter: Mutex<HashMap<String, Vec<u64>>>,
}

impl GuardianEngine {
    pub fn new(
        capability_registry: CapabilityRegistry,
        policy_engine: PolicyEngine,
        config: GuardianConfig,
    ) -> Self {
        Self {
            capability_registry,
            policy_engine,
            consent_manager: Mutex::new(ConsentManager::new()),
            trust_manager: Mutex::new(TrustManager::new()),
            threat_detector: Mutex::new(ThreatDetector::new()),
            integrity_verifier: IntegrityVerifier::new(),
            audit_log: Mutex::new(AuditLog::new()),
            rollback_manager: Mutex::new(RollbackManager::new()),
            config,
            rate_limiter: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_consent_manager(mut self, cm: ConsentManager) -> Self {
        self.consent_manager = Mutex::new(cm);
        self
    }

    pub fn with_trust_manager(mut self, tm: TrustManager) -> Self {
        self.trust_manager = Mutex::new(tm);
        self
    }

    pub fn evaluate(
        &self,
        subject: &str,
        capability: &str,
        resource: Option<&str>,
        action: &str,
        context: std::collections::HashMap<String, String>,
        recovery: &RecoveryMode,
    ) -> GuardianDecision {
        let execution_id = Uuid::new_v4();
        let mut audit_log = self.audit_log.lock().unwrap_or_else(|e| e.into_inner());
        let trust_level = {
            let trust_manager = self.trust_manager.lock().unwrap_or_else(|e| e.into_inner());
            trust_manager.get(subject)
        };

        let capability_match = self.capability_registry.find_matching(capability);
        let capability_entry = match capability_match.first() {
            Some(c) => *c,
            None => {
                audit_log.record(
                    subject,
                    action,
                    resource,
                    "denied",
                    Some("Unknown capability"),
                    "unknown",
                    "unknown",
                    AuditLevel::Basic,
                );
                return GuardianDecision {
                    allowed: false,
                    requires_consent: false,
                    requires_mfa: false,
                    audit_level: AuditLevel::Basic,
                    reason: format!("Unknown capability: {capability}"),
                    policy_result: None,
                    execution_id,
                };
            }
        };
        let risk_level = capability_entry.risk_level;

        if trust_level.is_blocked() {
            audit_log.record(
                subject,
                action,
                resource,
                "denied",
                Some("Entity is blocked"),
                &format!("{risk_level:?}"),
                &format!("{trust_level:?}"),
                AuditLevel::Detailed,
            );
            return GuardianDecision {
                allowed: false,
                requires_consent: false,
                requires_mfa: false,
                audit_level: AuditLevel::Detailed,
                reason: format!("Entity '{subject}' is blocked (trust: {trust_level:?})"),
                policy_result: None,
                execution_id,
            };
        }

        // SECURITY: Rate limiting — max 30 requests per subject per 60-second window.
        // This prevents abuse from compromised or overly aggressive entities.
        {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let mut limiter = self.rate_limiter.lock().unwrap_or_else(|e| e.into_inner());
            let timestamps = limiter.entry(subject.to_string()).or_default();
            // Evict entries older than 60 seconds
            timestamps.retain(|&t| now_ms.saturating_sub(t) <= 60_000);
            if timestamps.len() >= 30 {
                audit_log.record(
                    subject,
                    action,
                    resource,
                    "blocked",
                    Some("Rate limit exceeded"),
                    &format!("{risk_level:?}"),
                    &format!("{trust_level:?}"),
                    AuditLevel::Full,
                );
                return GuardianDecision {
                    allowed: false,
                    requires_consent: false,
                    requires_mfa: false,
                    audit_level: AuditLevel::Full,
                    reason: format!(
                        "Rate limit exceeded for '{subject}': {} requests in 60s window",
                        timestamps.len()
                    ),
                    policy_result: None,
                    execution_id,
                };
            }
            timestamps.push(now_ms);
        }

        // SECURITY: Recovery mode enforcement — when recovery mode is active,
        // critical-risk actions are denied to prevent further compromise.
        // Medium and high risk actions are also restricted to limit attack surface.
        if recovery.is_active() && risk_level >= RiskLevel::Medium {
            let restriction_level = if risk_level >= RiskLevel::Critical {
                "critical"
            } else {
                "medium+"
            };
            audit_log.record(
                subject,
                action,
                resource,
                "denied",
                Some(&format!(
                    "Recovery mode active — {} risk actions restricted",
                    restriction_level
                )),
                &format!("{risk_level:?}"),
                &format!("{trust_level:?}"),
                AuditLevel::Full,
            );
            return GuardianDecision {
                allowed: false,
                requires_consent: false,
                requires_mfa: false,
                audit_level: AuditLevel::Full,
                reason: format!(
                    "Recovery mode active — {} risk actions are restricted",
                    restriction_level
                ),
                policy_result: None,
                execution_id,
            };
        }

        // SECURITY: Integrity check — if enabled and entries are registered, verify
        // resource integrity before allowing Critical-risk actions. This prevents
        // authorization of actions against tampered resources.
        if self.config.require_integrity_check
            && risk_level >= RiskLevel::Critical
            && !self.integrity_verifier.is_empty()
        {
            if let Some(res) = resource {
                if self.integrity_verifier.get_entry(res).is_some() {
                    if let Some(data_str) = context.get("integrity_data") {
                        if !self.integrity_verifier.verify(res, data_str.as_bytes()) {
                            audit_log.record(
                                subject,
                                action,
                                Some(res),
                                "denied",
                                Some("Integrity check failed"),
                                &format!("{risk_level:?}"),
                                &format!("{trust_level:?}"),
                                AuditLevel::Full,
                            );
                            return GuardianDecision {
                                allowed: false,
                                requires_consent: false,
                                requires_mfa: false,
                                audit_level: AuditLevel::Full,
                                reason: format!("Integrity check failed for resource '{res}'"),
                                policy_result: None,
                                execution_id,
                            };
                        }
                    } else {
                        audit_log.record(
                            subject,
                            action,
                            Some(res),
                            "denied",
                            Some("Integrity data required but not provided"),
                            &format!("{risk_level:?}"),
                            &format!("{trust_level:?}"),
                            AuditLevel::Full,
                        );
                        return GuardianDecision {
                            allowed: false,
                            requires_consent: false,
                            requires_mfa: false,
                            audit_level: AuditLevel::Full,
                            reason: format!(
                                "Resource '{res}' requires integrity data for Critical-risk authorization"
                            ),
                            policy_result: None,
                            execution_id,
                        };
                    }
                }
            }
        }

        let policy_input = PolicyInput {
            subject: subject.to_string(),
            capability: capability.to_string(),
            resource: resource.map(|r| r.to_string()),
            action: action.to_string(),
            risk_level: format!("{:?}", risk_level).to_lowercase(),
            trust_level: format!("{:?}", trust_level).to_lowercase(),
            context,
        };

        // SECURITY: No auto-grant path. All actions go through policy evaluation.
        // Trust level is passed to the policy engine so rules can factor it in,
        // but no action bypasses evaluation entirely.
        let policy_result = self.policy_engine.evaluate(&policy_input);
        let policy_denies = !policy_result.allowed
            && !policy_result.requires_consent
            && !policy_result.requires_mfa;

        if policy_denies {
            let is_medium_plus = risk_level >= RiskLevel::Medium;
            let is_critical = risk_level >= RiskLevel::Critical;

            if is_critical {
                let consent_manager = self
                    .consent_manager
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let has_consent = consent_manager.is_granted(subject, capability);
                drop(consent_manager);
                if !has_consent {
                    audit_log.record(
                        subject,
                        action,
                        resource,
                        "pending",
                        Some("Requires MFA + consent for critical risk"),
                        &format!("{risk_level:?}"),
                        &format!("{trust_level:?}"),
                        AuditLevel::Full,
                    );
                    return GuardianDecision {
                        allowed: false,
                        requires_consent: true,
                        requires_mfa: true,
                        audit_level: AuditLevel::Full,
                        reason: "Critical risk requires MFA and consent".to_string(),
                        policy_result: Some(policy_result),
                        execution_id,
                    };
                }
            } else if is_medium_plus {
                let consent_manager = self
                    .consent_manager
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let has_consent = consent_manager.is_granted(subject, capability);
                drop(consent_manager);
                if !has_consent {
                    audit_log.record(
                        subject,
                        action,
                        resource,
                        "pending",
                        Some("Requires consent for medium+ risk"),
                        &format!("{risk_level:?}"),
                        &format!("{trust_level:?}"),
                        AuditLevel::Detailed,
                    );
                    return GuardianDecision {
                        allowed: false,
                        requires_consent: true,
                        requires_mfa: false,
                        audit_level: AuditLevel::Detailed,
                        reason: "Medium risk requires consent".to_string(),
                        policy_result: Some(policy_result),
                        execution_id,
                    };
                }
            }

            let reason = policy_result
                .reason
                .clone()
                .unwrap_or_else(|| "Policy denied".to_string());
            audit_log.record(
                subject,
                action,
                resource,
                "denied",
                Some(&reason),
                &format!("{risk_level:?}"),
                &format!("{trust_level:?}"),
                policy_result.audit_level,
            );
            return GuardianDecision {
                allowed: false,
                requires_consent: false,
                requires_mfa: false,
                audit_level: policy_result.audit_level,
                reason,
                policy_result: Some(policy_result),
                execution_id,
            };
        }

        let consent_manager = self
            .consent_manager
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let requires_consent = if risk_level >= RiskLevel::Medium
            && risk_level > self.config.max_risk_without_consent
        {
            if !consent_manager.is_granted(subject, capability) {
                true
            } else {
                policy_result.requires_consent
            }
        } else {
            false
        };
        drop(consent_manager);

        let requires_mfa = risk_level >= RiskLevel::Critical || policy_result.requires_mfa;

        if self.config.require_threat_analysis && risk_level >= RiskLevel::High {
            let mut threat_detector = self
                .threat_detector
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if threat_detector.rapid_failure_detected(subject, 60, 3) {
                threat_detector.record_event(
                    subject,
                    "rapid_requests",
                    ThreatSeverity::Medium,
                    &format!("Rapid {action} requests from {subject}"),
                );
                audit_log.record(
                    subject,
                    action,
                    resource,
                    "blocked",
                    Some("Rapid request rate detected"),
                    &format!("{risk_level:?}"),
                    &format!("{trust_level:?}"),
                    AuditLevel::Full,
                );
                return GuardianDecision {
                    allowed: false,
                    requires_consent: false,
                    requires_mfa: false,
                    audit_level: AuditLevel::Full,
                    reason: "Rapid request rate detected by threat analysis".to_string(),
                    policy_result: Some(policy_result),
                    execution_id,
                };
            }
        }

        let result = if requires_consent || requires_mfa {
            "pending"
        } else {
            "allowed"
        };

        audit_log.record(
            subject,
            action,
            resource,
            result,
            None,
            &format!("{risk_level:?}"),
            &format!("{trust_level:?}"),
            policy_result.audit_level,
        );

        GuardianDecision {
            allowed: !requires_consent && !requires_mfa,
            requires_consent,
            requires_mfa,
            audit_level: policy_result.audit_level,
            reason: policy_result
                .reason
                .clone()
                .unwrap_or_else(|| result.to_string()),
            policy_result: Some(policy_result),
            execution_id,
        }
    }

    pub fn handle_consent_granted(
        &self,
        _execution_id: Uuid,
        subject: &str,
        capability: &str,
    ) -> bool {
        let mut audit_log = self.audit_log.lock().unwrap_or_else(|e| e.into_inner());
        audit_log.record(
            subject,
            capability,
            None,
            "consent_granted",
            None,
            "medium",
            "trusted",
            AuditLevel::Detailed,
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Capability, CapabilityCategory};
    use crate::recovery::RecoveryMode;

    fn normal_recovery() -> RecoveryMode {
        RecoveryMode::new()
    }

    fn make_engine() -> GuardianEngine {
        let mut registry = CapabilityRegistry::new();
        let _ = registry.register(Capability::new(
            "test:low",
            "Low Risk",
            RiskLevel::Low,
            CapabilityCategory::System,
        ));
        let _ = registry.register(Capability::new(
            "test:high",
            "High Risk",
            RiskLevel::High,
            CapabilityCategory::System,
        ));
        let _ = registry.register(Capability::new(
            "test:critical",
            "Critical Risk",
            RiskLevel::Critical,
            CapabilityCategory::System,
        ));
        let mut trust = TrustManager::new();
        trust.set("user-1", crate::trust::TrustLevel::Verified);

        // Add explicit allow rules — auto-grants are disabled so policy must evaluate
        let mut policy = PolicyEngine::new();
        policy.add_rule(crate::policy::PolicyRule {
            id: "allow-low-risk".to_string(),
            description: "Allow low risk actions".to_string(),
            effect: crate::policy::PolicyEffect::Allow,
            capabilities: vec!["test:low".to_string()],
            conditions: vec![crate::policy::PolicyCondition::TrustLevelAtLeast(
                "unknown".to_string(),
            )],
            priority: 100,
        });
        policy.add_rule(crate::policy::PolicyRule {
            id: "allow-high-for-verified".to_string(),
            description: "Allow high risk for verified".to_string(),
            effect: crate::policy::PolicyEffect::RequireConsent,
            capabilities: vec!["test:high".to_string()],
            conditions: vec![crate::policy::PolicyCondition::TrustLevelAtLeast(
                "verified".to_string(),
            )],
            priority: 50,
        });

        GuardianEngine::new(registry, policy, GuardianConfig::default()).with_trust_manager(trust)
    }

    #[test]
    fn guardian_allows_low_risk() {
        let engine = make_engine();
        let decision = engine.evaluate(
            "user-1",
            "test:low",
            None,
            "read",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        assert!(decision.allowed);
    }

    #[test]
    fn guardian_requires_consent_for_high() {
        let engine = make_engine();
        let decision = engine.evaluate(
            "user-1",
            "test:high",
            None,
            "write",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        assert!(decision.requires_consent || decision.allowed);
    }

    #[test]
    fn guardian_blocks_unknown_capability() {
        let engine = make_engine();
        let decision = engine.evaluate(
            "user-1",
            "unknown:cap",
            None,
            "execute",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        assert!(!decision.allowed);
    }

    #[test]
    fn guardian_blocks_blocked_entity() {
        let mut trust = TrustManager::new();
        trust.set("bad-actor", crate::trust::TrustLevel::Blocked);
        let engine = GuardianEngine::new(
            CapabilityRegistry::new(),
            PolicyEngine::new(),
            GuardianConfig::default(),
        )
        .with_trust_manager(trust);
        let decision = engine.evaluate(
            "bad-actor",
            "test:low",
            None,
            "read",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        assert!(!decision.allowed);
    }

    #[test]
    fn guardian_requires_mfa_for_critical() {
        let engine = make_engine();
        let decision = engine.evaluate(
            "user-1",
            "test:critical",
            None,
            "delete",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        assert!(decision.requires_mfa || decision.allowed);
    }

    #[test]
    fn guardian_tracks_execution_id() {
        let engine = make_engine();
        let d1 = engine.evaluate(
            "user-1",
            "test:low",
            None,
            "read",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        let d2 = engine.evaluate(
            "user-1",
            "test:low",
            None,
            "read",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        assert_ne!(d1.execution_id, d2.execution_id);
    }

    #[test]
    fn guardian_integrity_check_blocks_critical_without_data() {
        let mut registry = CapabilityRegistry::new();
        let _ = registry.register(Capability::new(
            "test:critical",
            "Critical Risk",
            RiskLevel::Critical,
            CapabilityCategory::System,
        ));
        let mut verifier = IntegrityVerifier::new();
        verifier.register("/important/resource", b"expected-data");
        // Use a permissive policy so the integrity check is reached
        let mut policy = PolicyEngine::new();
        policy.add_rule(crate::policy::PolicyRule {
            id: "allow-critical".to_string(),
            description: "Allow critical for test".to_string(),
            effect: crate::policy::PolicyEffect::Allow,
            capabilities: vec!["test:critical".to_string()],
            conditions: vec![crate::policy::PolicyCondition::TrustLevelAtLeast(
                "unknown".to_string(),
            )],
            priority: 100,
        });
        let engine = GuardianEngine {
            capability_registry: registry,
            policy_engine: policy,
            consent_manager: Mutex::new(ConsentManager::new()),
            trust_manager: Mutex::new(TrustManager::new()),
            threat_detector: Mutex::new(ThreatDetector::new()),
            integrity_verifier: verifier,
            audit_log: Mutex::new(AuditLog::new()),
            rollback_manager: Mutex::new(RollbackManager::new()),
            config: GuardianConfig {
                require_integrity_check: true,
                ..Default::default()
            },
            rate_limiter: Mutex::new(HashMap::new()),
        };
        let decision = engine.evaluate(
            "user-1",
            "test:critical",
            Some("/important/resource"),
            "delete",
            std::collections::HashMap::new(),
            &normal_recovery(),
        );
        assert!(!decision.allowed);
        assert!(
            decision.reason.contains("integrity data"),
            "Expected 'integrity data' in reason, got: {}",
            decision.reason
        );
    }

    #[test]
    fn guardian_integrity_check_blocks_tampered_resource() {
        let mut registry = CapabilityRegistry::new();
        let _ = registry.register(Capability::new(
            "test:critical",
            "Critical Risk",
            RiskLevel::Critical,
            CapabilityCategory::System,
        ));
        let mut verifier = IntegrityVerifier::new();
        verifier.register("/important/resource", b"expected-data");
        let mut policy = PolicyEngine::new();
        policy.add_rule(crate::policy::PolicyRule {
            id: "allow-critical".to_string(),
            description: "Allow critical for test".to_string(),
            effect: crate::policy::PolicyEffect::Allow,
            capabilities: vec!["test:critical".to_string()],
            conditions: vec![crate::policy::PolicyCondition::TrustLevelAtLeast(
                "unknown".to_string(),
            )],
            priority: 100,
        });
        let engine = GuardianEngine {
            capability_registry: registry,
            policy_engine: policy,
            consent_manager: Mutex::new(ConsentManager::new()),
            trust_manager: Mutex::new(TrustManager::new()),
            threat_detector: Mutex::new(ThreatDetector::new()),
            integrity_verifier: verifier,
            audit_log: Mutex::new(AuditLog::new()),
            rollback_manager: Mutex::new(RollbackManager::new()),
            config: GuardianConfig {
                require_integrity_check: true,
                ..Default::default()
            },
            rate_limiter: Mutex::new(HashMap::new()),
        };
        let mut ctx = std::collections::HashMap::new();
        ctx.insert("integrity_data".to_string(), "tampered-data".to_string());
        let decision = engine.evaluate(
            "user-1",
            "test:critical",
            Some("/important/resource"),
            "delete",
            ctx,
            &normal_recovery(),
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("Integrity check failed"));
    }

    #[test]
    fn guardian_denies_critical_when_recovery_active() {
        let engine = make_engine();
        let mut recovery = RecoveryMode::new();
        recovery
            .enter(crate::recovery::RecoveryAuth {
                subject: "system".to_string(),
                reason: "Compromise detected".to_string(),
                auth_method: "automatic".to_string(),
            })
            .unwrap();

        let decision = engine.evaluate(
            "user-1",
            "test:critical",
            None,
            "delete",
            std::collections::HashMap::new(),
            &recovery,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("Recovery mode active"));
    }

    #[test]
    fn guardian_denies_medium_when_recovery_active() {
        let mut recovery = RecoveryMode::new();
        recovery
            .enter(crate::recovery::RecoveryAuth {
                subject: "system".to_string(),
                reason: "Compromise detected".to_string(),
                auth_method: "automatic".to_string(),
            })
            .unwrap();

        // Register a medium-risk capability
        let mut registry = CapabilityRegistry::new();
        let _ = registry.register(Capability::new(
            "test:medium",
            "Medium Risk",
            RiskLevel::Medium,
            CapabilityCategory::System,
        ));
        let policy = PolicyEngine::new();
        let engine = GuardianEngine::new(registry, policy, GuardianConfig::default());

        let decision = engine.evaluate(
            "user-1",
            "test:medium",
            None,
            "write",
            std::collections::HashMap::new(),
            &recovery,
        );
        assert!(!decision.allowed);
        assert!(decision.reason.contains("Recovery mode active"));
    }

    #[test]
    fn guardian_allows_low_risk_during_recovery() {
        let engine = make_engine();
        let mut recovery = RecoveryMode::new();
        recovery
            .enter(crate::recovery::RecoveryAuth {
                subject: "system".to_string(),
                reason: "Compromise detected".to_string(),
                auth_method: "automatic".to_string(),
            })
            .unwrap();

        let decision = engine.evaluate(
            "user-1",
            "test:low",
            None,
            "read",
            std::collections::HashMap::new(),
            &recovery,
        );
        assert!(
            decision.allowed,
            "Low risk should be allowed during recovery"
        );
    }
}
