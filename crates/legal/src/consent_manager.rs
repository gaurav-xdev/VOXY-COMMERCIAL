use crate::documents::{
    CURRENT_AI_DISCLOSURE_VERSION, CURRENT_PRIVACY_POLICY_VERSION, CURRENT_TERMS_OF_SERVICE_VERSION,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use voxy_database::CommercialStore;

/// Type of legal consent being tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicyType {
    PrivacyPolicy,
    TermsOfService,
    AiDataDisclosure,
    TelemetryOptIn,
}

impl PolicyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PrivacyPolicy => "privacy_policy",
            Self::TermsOfService => "terms_of_service",
            Self::AiDataDisclosure => "ai_data_disclosure",
            Self::TelemetryOptIn => "telemetry_opt_in",
        }
    }

    pub fn current_version(&self) -> &'static str {
        match self {
            Self::PrivacyPolicy => CURRENT_PRIVACY_POLICY_VERSION,
            Self::TermsOfService => CURRENT_TERMS_OF_SERVICE_VERSION,
            Self::AiDataDisclosure => CURRENT_AI_DISCLOSURE_VERSION,
            Self::TelemetryOptIn => CURRENT_PRIVACY_POLICY_VERSION,
        }
    }
}

/// Comprehensive Consent, GDPR Data Portability & Right to Erasure Engine.
pub struct ConsentManager {
    store: Arc<CommercialStore>,
}

impl ConsentManager {
    pub fn new(store: Arc<CommercialStore>) -> Self {
        Self { store }
    }

    /// Record a clickwrap consent event in the authoritative database.
    pub async fn record_consent(
        &self,
        user_id: &str,
        policy: PolicyType,
        version: &str,
        agreed: bool,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<(), String> {
        self.store
            .record_consent(
                user_id,
                policy.as_str(),
                version,
                agreed,
                ip_address,
                user_agent,
            )
            .await
            .map_err(|e| format!("Failed to record consent: {e}"))
    }

    /// Verify whether a user has agreed to the required version of a policy.
    pub async fn has_valid_consent(
        &self,
        user_id: &str,
        policy: PolicyType,
    ) -> Result<bool, String> {
        // Query database consents table for matching record
        let consents = self
            .store
            .get_consents_by_user_id(user_id)
            .await
            .map_err(|e| format!("Failed to fetch consents: {e}"))?;

        let req_ver = policy.current_version();
        let p_str = policy.as_str();

        for c in consents {
            if c.consent_type == p_str && c.version == req_ver && c.is_accepted {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Checks which required policies require updated user consent.
    pub async fn check_required_consents(&self, user_id: &str) -> Result<Vec<PolicyType>, String> {
        let mut needed = Vec::new();

        let mandatory = [
            PolicyType::PrivacyPolicy,
            PolicyType::TermsOfService,
            PolicyType::AiDataDisclosure,
        ];

        for policy in mandatory {
            let valid = self.has_valid_consent(user_id, policy).await?;
            if !valid {
                needed.push(policy);
            }
        }

        Ok(needed)
    }

    /// GDPR / CCPA Right to Portability: Export all user data as structured JSON.
    pub async fn export_user_data(&self, user_id: &str) -> Result<serde_json::Value, String> {
        let user = self
            .store
            .get_user_by_id(user_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "User not found".to_string())?;

        let consents = self
            .store
            .get_consents_by_user_id(user_id)
            .await
            .map_err(|e| e.to_string())?;

        let subscription = self
            .store
            .get_active_subscription_by_user_id(user_id)
            .await
            .map_err(|e| e.to_string())?;

        let export = serde_json::json!({
            "export_metadata": {
                "system": "VOXY COM",
                "exported_at": chrono::Utc::now().to_rfc3339(),
                "user_id": user_id,
            },
            "profile": {
                "id": user.id,
                "email": user.email,
                "status": user.status,
                "created_at": user.created_at,
            },
            "subscription": subscription,
            "consents": consents,
        });

        Ok(export)
    }

    /// GDPR / CCPA Right to Erasure ("Right to be Forgotten"): permanently deletes user account.
    pub async fn erase_user_data(&self, user_id: &str) -> Result<(), String> {
        // Revoke all sessions first
        let _ = self.store.revoke_all_user_sessions(user_id).await;

        // Anonymize user record
        self.store
            .delete_user(user_id)
            .await
            .map_err(|e| format!("Failed to erase user: {e}"))?;

        Ok(())
    }
}
