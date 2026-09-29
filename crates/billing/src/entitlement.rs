//! Server-Side Entitlement Resolution & Verification Engine.
//!
//! Evaluates user entitlements strictly from verified database state.
//! Never trusts client-side localStorage, parameters, or headers.

use std::collections::HashSet;
use voxy_database::CommercialStore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureFlag {
    CodingHarness,
    CloudVoice,
    UnlimitedModels,
    ComputerControl,
    OfficeAutomation,
    MultiAgentTeams,
    PriorityCloudRouting,
}

impl FeatureFlag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CodingHarness => "coding_harness",
            Self::CloudVoice => "cloud_voice",
            Self::UnlimitedModels => "unlimited_models",
            Self::ComputerControl => "computer_control",
            Self::OfficeAutomation => "office_automation",
            Self::MultiAgentTeams => "multi_agent_teams",
            Self::PriorityCloudRouting => "priority_cloud_routing",
        }
    }
}

pub struct EntitlementEngine;

impl EntitlementEngine {
    /// Returns the complete set of features granted to a specific plan tier.
    pub fn features_for_tier(tier: &str) -> HashSet<&'static str> {
        let mut set = HashSet::new();

        match tier.to_lowercase().as_str() {
            "enterprise" => {
                set.insert(FeatureFlag::CodingHarness.as_str());
                set.insert(FeatureFlag::CloudVoice.as_str());
                set.insert(FeatureFlag::UnlimitedModels.as_str());
                set.insert(FeatureFlag::ComputerControl.as_str());
                set.insert(FeatureFlag::OfficeAutomation.as_str());
                set.insert(FeatureFlag::MultiAgentTeams.as_str());
                set.insert(FeatureFlag::PriorityCloudRouting.as_str());
            }
            "pro" => {
                set.insert(FeatureFlag::CodingHarness.as_str());
                set.insert(FeatureFlag::CloudVoice.as_str());
                set.insert(FeatureFlag::UnlimitedModels.as_str());
                set.insert(FeatureFlag::ComputerControl.as_str());
                set.insert(FeatureFlag::OfficeAutomation.as_str());
            }
            _ => {
                // Free tier defaults (local models, basic control)
            }
        }

        set
    }

    /// Evaluates whether a user is authorized for a specific feature.
    /// Strictly verifies the authoritative server-side database.
    pub async fn is_authorized(
        store: &CommercialStore,
        user_id: &str,
        feature: FeatureFlag,
    ) -> Result<bool, String> {
        // 1. Check direct entitlement override in database
        let has_entitlement = store
            .check_entitlement(user_id, feature.as_str())
            .await
            .map_err(|e| e.to_string())?;

        if has_entitlement {
            return Ok(true);
        }

        // 2. Check active subscription plan tier
        let active_sub = store
            .get_active_subscription_by_user_id(user_id)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(sub) = active_sub {
            if sub.status == "active" {
                let tier = if sub.plan_id.contains("enterprise") || sub.plan_id.contains("team") {
                    "enterprise"
                } else if sub.plan_id.contains("pro") {
                    "pro"
                } else {
                    "free"
                };

                let tier_features = Self::features_for_tier(tier);
                return Ok(tier_features.contains(feature.as_str()));
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use voxy_database::{DatabaseConfig, SqliteDatabase, StorageProvider};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_server_side_entitlement_resolution() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("entitlement_test.db");
        let config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };

        let db = Arc::new(SqliteDatabase::new());
        db.connect(&config).await.unwrap();

        let store = CommercialStore::new(db.clone());
        store.initialize_schema().await.unwrap();
        store.seed_default_plans().await.unwrap();

        let user = store.create_user("pro_user@voxy.ai", "hash").await.unwrap();

        // Initially no premium access
        assert!(!EntitlementEngine::is_authorized(&store, &user.id, FeatureFlag::CodingHarness).await.unwrap());

        // Grant active Pro subscription
        let sub = voxy_database::SubscriptionRecord {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user.id.clone(),
            plan_id: "plan_pro_monthly".into(),
            dodo_customer_id: "cus_1".into(),
            dodo_subscription_id: "sub_1".into(),
            status: "active".into(),
            current_period_start: chrono::Utc::now().to_rfc3339(),
            current_period_end: (chrono::Utc::now() + chrono::Duration::days(30)).to_rfc3339(),
            cancel_at_period_end: false,
            cancelled_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        store.upsert_subscription(&sub).await.unwrap();

        // Now authorized for Pro features
        assert!(EntitlementEngine::is_authorized(&store, &user.id, FeatureFlag::CodingHarness).await.unwrap());
        assert!(EntitlementEngine::is_authorized(&store, &user.id, FeatureFlag::OfficeAutomation).await.unwrap());

        // Enterprise feature should still be false
        assert!(!EntitlementEngine::is_authorized(&store, &user.id, FeatureFlag::MultiAgentTeams).await.unwrap());
    }

    #[tokio::test]
    async fn test_forged_client_entitlement_rejection() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("forged_entitlement_test.db");
        let config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };

        let db = Arc::new(SqliteDatabase::new());
        db.connect(&config).await.unwrap();

        let store = CommercialStore::new(db.clone());
        store.initialize_schema().await.unwrap();

        let free_user = store.create_user("free_user@voxy.ai", "hash").await.unwrap();

        // Client claiming they have Pro cannot access CodingHarness without DB record
        let is_granted = EntitlementEngine::is_authorized(&store, &free_user.id, FeatureFlag::CodingHarness).await.unwrap();
        assert!(!is_granted, "Client claims must be rejected without server database backing");
    }
}
