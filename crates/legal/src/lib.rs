//! Privacy Policy, Terms of Service, GDPR/CCPA Data Subject Rights & Clickwrap Consent (REQ-LEGAL-01).
//!
//! Provides canonical production legal texts and immutable clickwrap consent tracking:
//! - Discloses Local vs Cloud AI processing models and zero audio storage
//! - Discloses Dodo Payments checkout security with zero card retention
//! - Enforces versioned consent records with re-consent detection upon policy updates
//! - Provides GDPR Data Portability (JSON export) and Right to Erasure (account deletion)

pub mod consent_manager;
pub mod documents;

pub use consent_manager::{ConsentManager, PolicyType};
pub use documents::{
    verify_privacy_policy_completeness, CURRENT_AI_DISCLOSURE_VERSION,
    CURRENT_PRIVACY_POLICY_VERSION, CURRENT_TERMS_OF_SERVICE_VERSION, PRIVACY_POLICY_MARKDOWN,
    TERMS_OF_SERVICE_MARKDOWN,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;
    use voxy_database::{CommercialStore, DatabaseConfig, SqliteDatabase, StorageProvider};

    async fn setup_test_consent_manager() -> (Arc<CommercialStore>, ConsentManager, String) {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("legal_test.db");
        let config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };
        let db = Arc::new(SqliteDatabase::new());
        db.connect(&config).await.unwrap();

        let store = Arc::new(CommercialStore::new(db));
        store.initialize_schema().await.unwrap();

        // Create test user
        let user = store
            .create_user("legal_tester@voxy.ai", "argon2id_placeholder_hash")
            .await
            .unwrap();

        let manager = ConsentManager::new(store.clone());
        (store, manager, user.id)
    }

    #[test]
    fn test_privacy_statement_completeness() {
        // Verify canonical privacy policy includes all required commercial disclosures
        let missing = verify_privacy_policy_completeness(PRIVACY_POLICY_MARKDOWN);
        assert!(
            missing.is_empty(),
            "Privacy policy is missing required disclosures: {:?}",
            missing
        );

        // Verify terms of service mentions 100% user ownership and autonomous operations disclaimer
        assert!(TERMS_OF_SERVICE_MARKDOWN.contains("100% User Ownership"));
        assert!(TERMS_OF_SERVICE_MARKDOWN.contains("Human Oversight"));
        assert!(TERMS_OF_SERVICE_MARKDOWN.contains("Emergency Stop"));
    }

    #[tokio::test]
    async fn test_consent_record_and_version_audit() {
        let (_, manager, user_id) = setup_test_consent_manager().await;

        // 1. Initial state: user has NOT consented to Privacy Policy
        let has_consented_init = manager
            .has_valid_consent(&user_id, PolicyType::PrivacyPolicy)
            .await
            .unwrap();
        assert!(!has_consented_init);

        let needed_consents = manager.check_required_consents(&user_id).await.unwrap();
        assert_eq!(needed_consents.len(), 3);
        assert!(needed_consents.contains(&PolicyType::PrivacyPolicy));
        assert!(needed_consents.contains(&PolicyType::TermsOfService));
        assert!(needed_consents.contains(&PolicyType::AiDataDisclosure));

        // 2. Record clickwrap consent for Privacy Policy
        manager
            .record_consent(
                &user_id,
                PolicyType::PrivacyPolicy,
                CURRENT_PRIVACY_POLICY_VERSION,
                true,
                Some("192.168.1.50".to_string()),
                Some("VOXY-Windows-Desktop/1.0".to_string()),
            )
            .await
            .unwrap();

        let has_consented_after = manager
            .has_valid_consent(&user_id, PolicyType::PrivacyPolicy)
            .await
            .unwrap();
        assert!(has_consented_after);

        // 3. Outdated version test: if user consented to older version "2025-01-01", it should require re-consent
        manager
            .record_consent(
                &user_id,
                PolicyType::TermsOfService,
                "2025-01-01", // Old version
                true,
                None,
                None,
            )
            .await
            .unwrap();

        let terms_valid = manager
            .has_valid_consent(&user_id, PolicyType::TermsOfService)
            .await
            .unwrap();
        assert!(!terms_valid, "Old policy version must require updated consent");

        // 4. Update to current Terms version
        manager
            .record_consent(
                &user_id,
                PolicyType::TermsOfService,
                CURRENT_TERMS_OF_SERVICE_VERSION,
                true,
                None,
                None,
            )
            .await
            .unwrap();

        let terms_now_valid = manager
            .has_valid_consent(&user_id, PolicyType::TermsOfService)
            .await
            .unwrap();
        assert!(terms_now_valid);

        // 5. GDPR Right to Data Portability (Export)
        let exported = manager.export_user_data(&user_id).await.unwrap();
        assert_eq!(exported["profile"]["email"], "legal_tester@voxy.ai");
        assert!(exported["consents"].as_array().unwrap().len() >= 2);

        // 6. GDPR Right to Erasure ("Forget Me")
        manager.erase_user_data(&user_id).await.unwrap();
        let erased_export = manager.export_user_data(&user_id).await;
        assert!(erased_export.is_err(), "Erased user must not be retrievable");
    }
}
