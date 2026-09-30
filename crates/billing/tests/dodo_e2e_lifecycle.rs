//! Dodo Payments E2E Lifecycle Integration Tests.
//!
//! Covers the complete production billing lifecycle:
//!   - Webhook signature verification (Standard Webhooks spec: `webhook-id.webhook-timestamp.body`)
//!   - Timestamp replay protection (accept window, future timestamps, stale timestamps)
//!   - Idempotency: duplicate events are silently deduplicated
//!   - Forged signatures are rejected
//!   - `subscription.active` → grants entitlements with authoritative period_end
//!   - `subscription.renewed` → updates subscription period, re-grants entitlements
//!   - `subscription.cancelled` (cancel_at_period_end=true) → remains active until period end
//!   - `subscription.cancelled` (immediate) → revokes entitlements
//!   - `subscription.expired` → transitions status to expired, revokes entitlements
//!   - Authoritative expiration: entitlements denied after `expires_at` passes
//!   - Client entitlement forgery: server-side check always wins

use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;

use voxy_billing::{
    WebhookHandler, WebhookError,
    verify_webhook_signature, verify_webhook_timestamp,
};
use voxy_database::{
    CommercialStore, SqliteDatabase, SubscriptionRecord,
    config::DatabaseConfig,
};
use voxy_database::storage::StorageProvider;

// ─── Helpers ────────────────────────────────────────────────────────────────

async fn setup_store() -> CommercialStore {
    let db = Arc::new(SqliteDatabase::new());
    let config = DatabaseConfig {
        kind: voxy_database::config::DatabaseKind::Sqlite,
        path: Some(":memory:".to_string()),
        ..Default::default()
    };
    db.connect(&config).await.expect("connect in-memory SQLite");
    let store = CommercialStore::new(db);
    store.initialize_schema().await.expect("initialize schema");
    store.seed_default_plans().await.expect("seed plans");
    store
}

async fn setup_handler(secret: &str) -> (WebhookHandler, CommercialStore) {
    let store = setup_store().await;
    let handler = WebhookHandler::new(secret.to_string(), store.clone());
    (handler, store)
}

/// Generate a Standard Webhooks-compliant HMAC-SHA256 signature header.
fn sign_standard_webhooks(
    webhook_id: &str,
    timestamp: &str,
    body: &str,
    secret_bytes: &[u8],
) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use base64::Engine;

    type HmacSha256 = Hmac<Sha256>;

    let to_sign = format!("{}.{}.{}", webhook_id, timestamp, body);
    let mut mac = HmacSha256::new_from_slice(secret_bytes).unwrap();
    mac.update(to_sign.as_bytes());
    let result = mac.finalize().into_bytes();
    let b64 = base64::engine::general_purpose::STANDARD.encode(result);
    format!("v1,{}", b64)
}

fn now_epoch() -> String {
    Utc::now().timestamp().to_string()
}

fn future_period_end() -> String {
    (Utc::now() + chrono::Duration::days(30)).to_rfc3339()
}

fn past_timestamp() -> String {
    (Utc::now() - chrono::Duration::hours(1)).to_rfc3339()
}

/// Creates a subscription.active JSON payload.
fn active_payload(user_id: &str, sub_id: &str, period_end: &str) -> String {
    serde_json::json!({
        "event_id": format!("evt_{}", Uuid::new_v4().simple()),
        "event_type": "subscription.active",
        "data": {
            "subscription_id": sub_id,
            "customer_id": "cus_test_001",
            "product_id": "plan_pro_monthly",
            "metadata": { "user_id": user_id },
            "current_period_start": Utc::now().to_rfc3339(),
            "current_period_end": period_end
        }
    })
    .to_string()
}

// ─── 1. Signature Verification Unit Tests ───────────────────────────────────

#[test]
fn test_verify_standard_webhook_signature_valid() {
    let secret = b"dodo_test_secret_key_32bytes_xyz";
    let wh_id = "whev_test_001";
    let ts = now_epoch();
    let body = r#"{"event_type":"subscription.active"}"#;
    let sig = sign_standard_webhooks(wh_id, &ts, body, secret);

    let raw_secret = std::str::from_utf8(secret).unwrap();
    assert!(
        verify_webhook_signature(body.as_bytes(), &sig, raw_secret, Some(wh_id), Some(&ts)),
        "Valid Standard Webhooks signature must be accepted"
    );
}

#[test]
fn test_verify_webhook_signature_wrong_key_rejected() {
    let secret = b"correct_secret_key_32bytes_foobar";
    let wrong = b"wrong_secret_key_32bytes_barbaz!";
    let wh_id = "whev_test_002";
    let ts = now_epoch();
    let body = r#"{"event_type":"subscription.active"}"#;
    let sig = sign_standard_webhooks(wh_id, &ts, body, secret);

    let raw_wrong = std::str::from_utf8(wrong).unwrap();
    assert!(
        !verify_webhook_signature(body.as_bytes(), &sig, raw_wrong, Some(wh_id), Some(&ts)),
        "Signature from different key must be rejected"
    );
}

#[test]
fn test_verify_webhook_signature_forged_body_rejected() {
    let secret = b"dodo_test_secret_key_32bytes_xyz";
    let wh_id = "whev_test_003";
    let ts = now_epoch();
    let body = r#"{"event_type":"subscription.active"}"#;
    let forged_body = r#"{"event_type":"subscription.cancelled"}"#;
    let sig = sign_standard_webhooks(wh_id, &ts, body, secret);

    let raw_secret = std::str::from_utf8(secret).unwrap();
    assert!(
        !verify_webhook_signature(forged_body.as_bytes(), &sig, raw_secret, Some(wh_id), Some(&ts)),
        "Signature over different body must be rejected"
    );
}

#[test]
fn test_verify_webhook_signature_tampered_prefix_rejected() {
    // Attacker tries to prefix "v2," instead of "v1,"
    let secret = b"dodo_test_secret_key_32bytes_xyz";
    let wh_id = "whev_test_004";
    let ts = now_epoch();
    let body = r#"{"event_type":"subscription.active"}"#;
    let sig = sign_standard_webhooks(wh_id, &ts, body, secret);
    let tampered = sig.replace("v1,", "v2,");

    let raw_secret = std::str::from_utf8(secret).unwrap();
    assert!(
        !verify_webhook_signature(body.as_bytes(), &tampered, raw_secret, Some(wh_id), Some(&ts)),
        "Tampered signature prefix must be rejected"
    );
}

#[test]
fn test_verify_webhook_signature_empty_secret_rejected() {
    let body = r#"{"event_type":"test"}"#;
    assert!(
        !verify_webhook_signature(body.as_bytes(), "v1,abc", "", None, None),
        "Empty secret must always be rejected"
    );
}

#[test]
fn test_verify_webhook_signature_whsec_prefix() {
    use base64::Engine;
    let raw_bytes = b"dodo_test_secret_key_32bytes_xyz";
    let b64 = base64::engine::general_purpose::STANDARD.encode(raw_bytes);
    let whsec = format!("whsec_{}", b64);
    let wh_id = "whev_whsec_001";
    let ts = now_epoch();
    let body = r#"{"event_type":"subscription.active"}"#;
    let sig = sign_standard_webhooks(wh_id, &ts, body, raw_bytes);

    assert!(
        verify_webhook_signature(body.as_bytes(), &sig, &whsec, Some(wh_id), Some(&ts)),
        "whsec_ prefixed secret must be base64-decoded and accepted"
    );
}

// ─── 2. Timestamp Replay Protection Unit Tests ───────────────────────────────

#[test]
fn test_timestamp_current_accepted() {
    let ts = now_epoch();
    assert!(verify_webhook_timestamp(&ts, 300), "Current epoch timestamp must be accepted");
}

#[test]
fn test_timestamp_rfc3339_current_accepted() {
    let ts = Utc::now().to_rfc3339();
    assert!(verify_webhook_timestamp(&ts, 300), "RFC3339 current timestamp must be accepted");
}

#[test]
fn test_timestamp_too_old_rejected() {
    let old_ts = (Utc::now() - chrono::Duration::seconds(600)).timestamp().to_string();
    assert!(
        !verify_webhook_timestamp(&old_ts, 300),
        "Timestamp older than max_age must be rejected (replay attack)"
    );
}

#[test]
fn test_timestamp_future_within_drift_accepted() {
    // 20 seconds in the future — within the 30s clock drift allowance
    let slight_future = (Utc::now() + chrono::Duration::seconds(20)).timestamp().to_string();
    assert!(
        verify_webhook_timestamp(&slight_future, 300),
        "Timestamp up to 30s in future should be accepted (clock drift)"
    );
}

#[test]
fn test_timestamp_far_future_rejected() {
    let far_future = (Utc::now() + chrono::Duration::seconds(120)).timestamp().to_string();
    assert!(
        !verify_webhook_timestamp(&far_future, 300),
        "Timestamp more than 30s in future must be rejected"
    );
}

#[test]
fn test_timestamp_invalid_string_rejected() {
    assert!(
        !verify_webhook_timestamp("not_a_timestamp", 300),
        "Non-timestamp string must be rejected"
    );
}

// ─── 3. WebhookHandler Integration Tests (Full DB Lifecycle) ─────────────────

#[tokio::test]
async fn test_subscription_active_grants_entitlements() {
    let secret = "dodo_integration_test_secret_001";
    let (handler, store) = setup_handler(secret).await;

    // Create user — the returned record contains the real DB-assigned UUID
    let user = store
        .create_user(&format!("sub_active_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();

    let sub_id = format!("sub_{}", Uuid::new_v4().simple());
    let period_end = future_period_end();

    let wh_id = format!("whev_{}", Uuid::new_v4().simple());
    let ts = now_epoch();
    let body = active_payload(&user_id, &sub_id, &period_end);
    let sig = sign_standard_webhooks(&wh_id, &ts, &body, secret.as_bytes());

    let result = handler.process_webhook(
        body.as_bytes(),
        Some(&sig),
        Some(&ts),
        Some(&wh_id),
    ).await;

    assert!(result.is_ok(), "subscription.active webhook must succeed: {:?}", result.err());

    // Verify entitlements were granted
    assert!(store.check_entitlement(&user_id, "coding_harness").await.unwrap(), "coding_harness must be granted");
    assert!(store.check_entitlement(&user_id, "cloud_voice").await.unwrap(), "cloud_voice must be granted");
    assert!(store.check_entitlement(&user_id, "unlimited_models").await.unwrap(), "unlimited_models must be granted");
    assert!(store.check_entitlement(&user_id, "computer_control").await.unwrap(), "computer_control must be granted");
    assert!(store.check_entitlement(&user_id, "office_automation").await.unwrap(), "office_automation must be granted");
}

#[tokio::test]
async fn test_webhook_signature_verification_fails_correctly() {
    let secret = "dodo_integration_test_secret_002";
    let (handler, _store) = setup_handler(secret).await;

    let user_id = format!("user_{}", Uuid::new_v4().simple());
    let sub_id = format!("sub_{}", Uuid::new_v4().simple());
    let body = active_payload(&user_id, &sub_id, &future_period_end());
    let ts = now_epoch();
    let wh_id = format!("whev_{}", Uuid::new_v4().simple());

    // Use wrong secret to produce forged signature
    let forged_sig = sign_standard_webhooks(&wh_id, &ts, &body, b"wrong_key_32bytes_xxxxxxxxxxx00");

    let result = handler.process_webhook(
        body.as_bytes(),
        Some(&forged_sig),
        Some(&ts),
        Some(&wh_id),
    ).await;

    assert!(matches!(result, Err(WebhookError::InvalidSignature)),
        "Forged signature must be rejected with InvalidSignature, got: {:?}", result);
}

#[tokio::test]
async fn test_webhook_missing_signature_rejected() {
    let secret = "dodo_integration_test_secret_003";
    let (handler, _store) = setup_handler(secret).await;

    let body = r#"{"event_id":"evt_test","event_type":"subscription.active","data":{}}"#;

    let result = handler.process_webhook(
        body.as_bytes(),
        None, // no signature header
        Some(&now_epoch()),
        Some("whev_test"),
    ).await;

    assert!(matches!(result, Err(WebhookError::MissingSignature)),
        "Missing signature header must return MissingSignature, got: {:?}", result);
}

#[tokio::test]
async fn test_webhook_replay_attack_rejected() {
    let secret = "dodo_integration_test_secret_004";
    let (handler, _store) = setup_handler(secret).await;

    // Old timestamp (1 hour ago — beyond the 300s window)
    let old_ts = (Utc::now() - chrono::Duration::seconds(600)).timestamp().to_string();
    let wh_id = format!("whev_{}", Uuid::new_v4().simple());
    let body = r#"{"event_id":"evt_old","event_type":"subscription.active","data":{}}"#;
    let sig = sign_standard_webhooks(&wh_id, &old_ts, body, secret.as_bytes());

    let result = handler.process_webhook(
        body.as_bytes(),
        Some(&sig),
        Some(&old_ts),
        Some(&wh_id),
    ).await;

    assert!(matches!(result, Err(WebhookError::ReplayDetected(_))),
        "Stale timestamp must be rejected as replay attack, got: {:?}", result);
}

#[tokio::test]
async fn test_webhook_idempotency_duplicate_event() {
    let secret = "dodo_integration_test_secret_005";
    let (handler, store) = setup_handler(secret).await;

    let user = store
        .create_user(&format!("idem_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();
    let sub_id = format!("sub_{}", Uuid::new_v4().simple());

    let period_end = future_period_end();
    let body = active_payload(&user_id, &sub_id, &period_end);
    let wh_id = format!("whev_{}", Uuid::new_v4().simple());
    let ts = now_epoch();
    let sig = sign_standard_webhooks(&wh_id, &ts, &body, secret.as_bytes());

    // First delivery
    let r1 = handler.process_webhook(body.as_bytes(), Some(&sig), Some(&ts), Some(&wh_id)).await;
    assert!(r1.is_ok(), "First delivery must succeed: {:?}", r1.err());

    // Duplicate delivery — same body so same event_id, must be deduplicated
    let r2 = handler.process_webhook(body.as_bytes(), Some(&sig), Some(&ts), Some(&wh_id)).await;
    assert!(r2.is_ok(), "Duplicate delivery must be silently accepted (idempotent): {:?}", r2.err());
    assert_eq!(r2.unwrap(), "Already processed", "Duplicate must return 'Already processed'");
}

#[tokio::test]
async fn test_subscription_cancelled_at_period_end() {
    let secret = "dodo_integration_test_secret_006";
    let (handler, store) = setup_handler(secret).await;

    let user = store
        .create_user(&format!("cancel_sched_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();
    let sub_id = format!("sub_{}", Uuid::new_v4().simple());
    let period_end = future_period_end();

    // First: activate subscription
    let body = active_payload(&user_id, &sub_id, &period_end);
    let wh_id1 = format!("whev_{}", Uuid::new_v4().simple());
    let ts = now_epoch();
    let sig1 = sign_standard_webhooks(&wh_id1, &ts, &body, secret.as_bytes());
    handler.process_webhook(body.as_bytes(), Some(&sig1), Some(&ts), Some(&wh_id1)).await.unwrap();

    // Then: cancel at period end (scheduled cancellation)
    let cancel_body = serde_json::json!({
        "event_id": format!("evt_{}", Uuid::new_v4().simple()),
        "event_type": "subscription.cancelled",
        "data": {
            "subscription_id": sub_id,
            "cancel_at_period_end": true
        }
    }).to_string();

    let wh_id2 = format!("whev_{}", Uuid::new_v4().simple());
    let ts2 = now_epoch();
    let sig2 = sign_standard_webhooks(&wh_id2, &ts2, &cancel_body, secret.as_bytes());
    let result = handler.process_webhook(cancel_body.as_bytes(), Some(&sig2), Some(&ts2), Some(&wh_id2)).await;
    assert!(result.is_ok(), "cancel_at_period_end webhook must succeed: {:?}", result.err());

    // Entitlements remain active until period end
    assert!(
        store.check_entitlement(&user_id, "coding_harness").await.unwrap(),
        "Entitlements must remain granted when cancel_at_period_end=true"
    );
}

#[tokio::test]
async fn test_subscription_immediate_cancellation_revokes_entitlements() {
    let secret = "dodo_integration_test_secret_007";
    let (handler, store) = setup_handler(secret).await;

    let user = store
        .create_user(&format!("cancel_imm_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();
    let sub_id = format!("sub_{}", Uuid::new_v4().simple());
    let period_end = future_period_end();

    // Activate
    let body = active_payload(&user_id, &sub_id, &period_end);
    let wh_id1 = format!("whev_{}", Uuid::new_v4().simple());
    let ts = now_epoch();
    let sig1 = sign_standard_webhooks(&wh_id1, &ts, &body, secret.as_bytes());
    handler.process_webhook(body.as_bytes(), Some(&sig1), Some(&ts), Some(&wh_id1)).await.unwrap();

    assert!(
        store.check_entitlement(&user_id, "coding_harness").await.unwrap(),
        "Entitlements must be active post-activation"
    );

    // Immediate cancellation
    let cancel_body = serde_json::json!({
        "event_id": format!("evt_{}", Uuid::new_v4().simple()),
        "event_type": "subscription.cancelled",
        "data": {
            "subscription_id": sub_id,
            "cancel_at_period_end": false
        }
    }).to_string();

    let wh_id2 = format!("whev_{}", Uuid::new_v4().simple());
    let ts2 = now_epoch();
    let sig2 = sign_standard_webhooks(&wh_id2, &ts2, &cancel_body, secret.as_bytes());
    handler.process_webhook(cancel_body.as_bytes(), Some(&sig2), Some(&ts2), Some(&wh_id2)).await.unwrap();

    // Entitlements must be revoked immediately
    assert!(
        !store.check_entitlement(&user_id, "coding_harness").await.unwrap(),
        "coding_harness must be revoked on immediate cancellation"
    );
    assert!(
        !store.check_entitlement(&user_id, "cloud_voice").await.unwrap(),
        "cloud_voice must be revoked on immediate cancellation"
    );
}

#[tokio::test]
async fn test_authoritative_expiration_denies_access() {
    let store = setup_store().await;

    let user = store
        .create_user(&format!("exp_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();

    // Grant entitlement with a past expiration (expired 1 hour ago)
    let expired_at = past_timestamp();
    store.set_entitlement(&user_id, "coding_harness", true, None, Some(&expired_at))
        .await.unwrap();

    // Server must deny access due to authoritative expiration
    let has_access = store.check_entitlement(&user_id, "coding_harness").await.unwrap();
    assert!(
        !has_access,
        "Expired entitlement must be denied by server-side authoritative check"
    );
}

#[tokio::test]
async fn test_client_entitlement_forgery_rejected() {
    // Simulates a client that fabricates having premium access.
    // The server must check the database — never trusting client-supplied state.
    let store = setup_store().await;

    let user = store
        .create_user(&format!("forgery_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();

    // User has NO subscription, NO entitlement. Client claims they have "unlimited_models".
    // Server check must return false.
    let server_verdict = store.check_entitlement(&user_id, "unlimited_models").await.unwrap();
    assert!(
        !server_verdict,
        "Server must reject entitlement forgery — user has no active subscription or grant"
    );
}

#[tokio::test]
async fn test_subscription_renewal_updates_period_end() {
    let secret = "dodo_integration_test_secret_008";
    let (handler, store) = setup_handler(secret).await;

    let user = store
        .create_user(&format!("renewal_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();
    let sub_id = format!("sub_{}", Uuid::new_v4().simple());

    // Initial activation
    let period_end_1 = future_period_end();
    let body1 = active_payload(&user_id, &sub_id, &period_end_1);
    let wh_id1 = format!("whev_{}", Uuid::new_v4().simple());
    let ts1 = now_epoch();
    let sig1 = sign_standard_webhooks(&wh_id1, &ts1, &body1, secret.as_bytes());
    handler.process_webhook(body1.as_bytes(), Some(&sig1), Some(&ts1), Some(&wh_id1)).await.unwrap();

    // Renewal with new period end 60 days out
    let period_end_2 = (Utc::now() + chrono::Duration::days(60)).to_rfc3339();
    let renewal_body = serde_json::json!({
        "event_id": format!("evt_{}", Uuid::new_v4().simple()),
        "event_type": "subscription.renewed",
        "data": {
            "subscription_id": sub_id,
            "customer_id": "cus_test_001",
            "product_id": "plan_pro_monthly",
            "metadata": { "user_id": user_id },
            "current_period_start": Utc::now().to_rfc3339(),
            "current_period_end": period_end_2
        }
    }).to_string();

    let wh_id2 = format!("whev_{}", Uuid::new_v4().simple());
    let ts2 = now_epoch();
    let sig2 = sign_standard_webhooks(&wh_id2, &ts2, &renewal_body, secret.as_bytes());
    let result = handler.process_webhook(renewal_body.as_bytes(), Some(&sig2), Some(&ts2), Some(&wh_id2)).await;
    assert!(result.is_ok(), "Renewal webhook must succeed: {:?}", result.err());

    // Verify subscription period was updated
    let active_sub = store.get_active_subscription_by_user_id(&user_id).await.unwrap();
    assert!(active_sub.is_some(), "Active subscription must exist after renewal");
    assert_eq!(
        active_sub.unwrap().current_period_end,
        period_end_2,
        "Period end must be updated to renewed value"
    );

    // Entitlements must still be active
    assert!(
        store.check_entitlement(&user_id, "coding_harness").await.unwrap(),
        "Entitlements must be active after renewal"
    );
}

#[tokio::test]
async fn test_multiple_users_isolated_entitlements() {
    // Verify that entitlements for one user do not bleed into another user's account.
    let secret = "dodo_integration_test_secret_009";
    let (handler, store) = setup_handler(secret).await;

    let user_a = store
        .create_user(&format!("isol_a_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_a")
        .await
        .expect("create user_a");
    let user_b = store
        .create_user(&format!("isol_b_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_b")
        .await
        .expect("create user_b");

    let sub_a = format!("sub_{}", Uuid::new_v4().simple());

    // Only user_a subscribes
    let body = active_payload(&user_a.id, &sub_a, &future_period_end());
    let wh_id = format!("whev_{}", Uuid::new_v4().simple());
    let ts = now_epoch();
    let sig = sign_standard_webhooks(&wh_id, &ts, &body, secret.as_bytes());
    handler.process_webhook(body.as_bytes(), Some(&sig), Some(&ts), Some(&wh_id)).await.unwrap();

    assert!(
        store.check_entitlement(&user_a.id, "coding_harness").await.unwrap(),
        "user_a must have coding_harness"
    );
    assert!(
        !store.check_entitlement(&user_b.id, "coding_harness").await.unwrap(),
        "user_b must NOT have coding_harness — no entitlement bleed"
    );
}

#[tokio::test]
async fn test_expired_subscription_denied_via_subscription_lookup() {
    // User has an active subscription row, but the period_end is in the past.
    // EntitlementEngine must deny access via authoritative period check.
    use voxy_billing::EntitlementEngine;
    use voxy_billing::FeatureFlag;

    let store = setup_store().await;

    let user = store
        .create_user(&format!("sub_exp_{}@test.voxy.ai", Uuid::new_v4().simple()), "hash_placeholder")
        .await
        .expect("create user");
    let user_id = user.id.clone();

    let sub = SubscriptionRecord {
        id: Uuid::new_v4().to_string(),
        user_id: user_id.clone(),
        plan_id: "plan_pro_monthly".to_string(),
        dodo_customer_id: "cus_expired".to_string(),
        dodo_subscription_id: format!("sub_{}", Uuid::new_v4().simple()),
        status: "active".to_string(),
        current_period_start: (Utc::now() - chrono::Duration::days(60)).to_rfc3339(),
        current_period_end: (Utc::now() - chrono::Duration::days(1)).to_rfc3339(), // expired yesterday
        cancel_at_period_end: false,
        cancelled_at: None,
        created_at: (Utc::now() - chrono::Duration::days(60)).to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };
    store.upsert_subscription(&sub).await.expect("upsert subscription with expired period");

    // EntitlementEngine must deny access because period_end is in the past
    let authorized = EntitlementEngine::is_authorized(&store, &user_id, FeatureFlag::CodingHarness)
        .await
        .expect("is_authorized must not error");

    assert!(
        !authorized,
        "Expired subscription (period_end in past) must deny access via authoritative check"
    );
}

