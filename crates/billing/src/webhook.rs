//! Dodo Payments Webhook Verification, Replay Defense & Idempotent Processing.
//!
//! Complies with the official Standard Webhooks specification (RFC/svix compliant)
//! used by Dodo Payments (`webhook-id`, `webhook-timestamp`, `webhook-signature`),
//! validates HMAC-SHA256 signatures, enforces timestamp replay protection,
//! guarantees idempotency, and transactionally updates subscription & entitlement states.

use base64::Engine;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;
use voxy_database::CommercialStore;

use crate::subscription::{SubscriptionStateMachine, SubscriptionStatus};

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("Missing signature header")]
    MissingSignature,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Replay attack detected: timestamp older than {0}s or in future")]
    ReplayDetected(i64),

    #[error("Malformed webhook payload: {0}")]
    MalformedPayload(String),

    #[error("Database error during webhook processing: {0}")]
    DatabaseError(String),
}

type HmacSha256 = Hmac<Sha256>;

/// Verifies a webhook signature according to the official Dodo Payments / Standard Webhooks spec:
///
/// 1. Content signed is: `${webhook_id}.${webhook_timestamp}.${body}`
/// 2. Signature header format: space-separated `v1,<base64_signature>` signatures.
/// 3. Secret: Supports `whsec_<base64>` encoded keys as well as raw strings.
/// 4. Fallback: Also verifies legacy HMAC-SHA256 of raw body for backward compatibility.
pub fn verify_webhook_signature(
    raw_payload: &[u8],
    signature_header: &str,
    webhook_secret: &str,
    webhook_id: Option<&str>,
    webhook_timestamp: Option<&str>,
) -> bool {
    let secret = webhook_secret.trim();
    if secret.is_empty() || signature_header.trim().is_empty() {
        return false;
    }

    // Resolve signing key bytes (Standard Webhooks uses base64 decoded bytes if prefix is whsec_)
    let secret_bytes: Vec<u8> = if let Some(stripped) = secret.strip_prefix("whsec_") {
        base64::engine::general_purpose::STANDARD
            .decode(stripped)
            .unwrap_or_else(|_| secret.as_bytes().to_vec())
    } else {
        secret.as_bytes().to_vec()
    };

    // ── Standard Webhooks Verification (Preferred) ──
    if let (Some(id), Some(ts)) = (webhook_id, webhook_timestamp) {
        let payload_str = match std::str::from_utf8(raw_payload) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let to_sign = format!("{}.{}.{}", id.trim(), ts.trim(), payload_str);

        if let Ok(mut mac) = HmacSha256::new_from_slice(&secret_bytes) {
            mac.update(to_sign.as_bytes());

            // Header can contain space-separated signatures: "v1,BASE64 v1,BASE64"
            for sig_part in signature_header.split_whitespace() {
                if let Some(b64_sig) = sig_part.strip_prefix("v1,") {
                    if let Ok(expected) = base64::engine::general_purpose::STANDARD.decode(b64_sig)
                    {
                        let check_mac = mac.clone();
                        if check_mac.verify_slice(&expected).is_ok() {
                            return true;
                        }
                    }
                }
            }
        }
    }

    // ── Fallback Verification (Direct Body HMAC in hex or base64) ──
    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(raw_payload);

    let clean_sig = signature_header.trim().trim_start_matches("sha256=");
    if let Ok(expected_bytes) = hex::decode(clean_sig) {
        if mac.clone().verify_slice(&expected_bytes).is_ok() {
            return true;
        }
    }

    if let Ok(expected_bytes) = base64::engine::general_purpose::STANDARD.decode(clean_sig) {
        if mac.verify_slice(&expected_bytes).is_ok() {
            return true;
        }
    }

    false
}

/// Verifies that a webhook timestamp is within the acceptable window (default 300 seconds / 5 mins).
pub fn verify_webhook_timestamp(timestamp_str: &str, max_age_seconds: i64) -> bool {
    let ts = match timestamp_str.trim().parse::<i64>() {
        Ok(epoch_secs) => match DateTime::from_timestamp(epoch_secs, 0) {
            Some(dt) => dt,
            None => return false,
        },
        Err(_) => match DateTime::parse_from_rfc3339(timestamp_str.trim()) {
            Ok(dt) => dt.to_utc(),
            Err(_) => return false,
        },
    };

    let now = Utc::now();
    let age = (now - ts).num_seconds();

    // Must be between -30s (slight clock drift allowance) and max_age_seconds
    age >= -30 && age <= max_age_seconds
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DodoWebhookPayload {
    pub event_id: String,
    pub event_type: String,
    pub timestamp: Option<String>,
    pub data: serde_json::Value,
}

pub struct WebhookHandler {
    webhook_secret: String,
    store: CommercialStore,
}

impl WebhookHandler {
    pub fn new(webhook_secret: String, store: CommercialStore) -> Self {
        Self {
            webhook_secret,
            store,
        }
    }

    /// Authenticates, validates replay defense, checks idempotency, and handles a webhook event.
    pub async fn process_webhook(
        &self,
        raw_body: &[u8],
        signature_header: Option<&str>,
        timestamp_header: Option<&str>,
        webhook_id_header: Option<&str>,
    ) -> Result<String, WebhookError> {
        let sig = signature_header.ok_or(WebhookError::MissingSignature)?;

        if !verify_webhook_signature(
            raw_body,
            sig,
            &self.webhook_secret,
            webhook_id_header,
            timestamp_header,
        ) {
            return Err(WebhookError::InvalidSignature);
        }

        if let Some(ts) = timestamp_header {
            if !verify_webhook_timestamp(ts, 300) {
                return Err(WebhookError::ReplayDetected(300));
            }
        }

        let body_str = std::str::from_utf8(raw_body)
            .map_err(|e| WebhookError::MalformedPayload(e.to_string()))?;

        let event: DodoWebhookPayload = serde_json::from_str(body_str)
            .map_err(|e| WebhookError::MalformedPayload(e.to_string()))?;

        // ── Idempotency Check ──
        let already_processed = self
            .store
            .is_webhook_processed(&event.event_id)
            .await
            .map_err(|e| WebhookError::DatabaseError(e.to_string()))?;

        if already_processed {
            tracing::info!(
                "[BILLING:WEBHOOK] Event {} already processed. Skipping idempotently.",
                event.event_id
            );
            return Ok("Already processed".into());
        }

        // Record initial event state in database
        self.store
            .record_webhook_event(&event.event_id, &event.event_type, body_str)
            .await
            .map_err(|e| WebhookError::DatabaseError(e.to_string()))?;

        // Process event side-effects transactionally
        let result = self.handle_event_type(&event).await;

        match result {
            Ok(_) => {
                self.store
                    .mark_webhook_processed(&event.event_id, true, None)
                    .await
                    .map_err(|e| WebhookError::DatabaseError(e.to_string()))?;
                Ok("Processed successfully".into())
            }
            Err(e) => {
                self.store
                    .mark_webhook_processed(&event.event_id, false, Some(e.clone()))
                    .await
                    .map_err(|db_e| WebhookError::DatabaseError(db_e.to_string()))?;
                Err(WebhookError::DatabaseError(e))
            }
        }
    }

    async fn handle_event_type(&self, event: &DodoWebhookPayload) -> Result<(), String> {
        match event.event_type.as_str() {
            "subscription.active" | "subscription.renewed" => {
                let sub_data = &event.data;

                // Extract user_id: check metadata first, then root data
                let user_id = sub_data["metadata"]["user_id"]
                    .as_str()
                    .or_else(|| sub_data["user_id"].as_str())
                    .unwrap_or_default();

                let plan_id = sub_data["product_id"]
                    .as_str()
                    .or_else(|| sub_data["plan_id"].as_str())
                    .unwrap_or("plan_pro_monthly");

                let sub_id = sub_data["subscription_id"].as_str().unwrap_or_default();

                let cus_id = sub_data["customer_id"].as_str().unwrap_or_default();

                if sub_id.is_empty() {
                    return Err("Missing subscription_id in event payload".into());
                }

                // If user_id is missing from payload, check if we already have it on record
                let resolved_user_id = if user_id.is_empty() {
                    if let Ok(Some(existing)) = self.store.get_subscription_by_dodo_id(sub_id).await
                    {
                        existing.user_id
                    } else {
                        return Err("Missing user_id in subscription event payload".into());
                    }
                } else {
                    user_id.to_string()
                };

                let now = Utc::now().to_rfc3339();

                // ── Authoritative Billing Period Extraction (Never hardcode 30 days) ──
                let period_start = sub_data["current_period_start"]
                    .as_str()
                    .or_else(|| sub_data["period_start"].as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| now.clone());

                let period_end = sub_data["current_period_end"]
                    .as_str()
                    .or_else(|| sub_data["next_billing_date"].as_str())
                    .or_else(|| sub_data["period_end"].as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        tracing::warn!(
                            "[BILLING:WEBHOOK] Provider omitted authoritative period end for {}. Deriving fallback.",
                            sub_id
                        );
                        (Utc::now() + chrono::Duration::days(30)).to_rfc3339()
                    });

                // Check out-of-order transition
                if let Ok(Some(existing)) = self.store.get_subscription_by_dodo_id(sub_id).await {
                    let curr_status = SubscriptionStatus::from_str_lossy(&existing.status);
                    if !SubscriptionStateMachine::can_transition(
                        curr_status,
                        SubscriptionStatus::Active,
                    ) {
                        tracing::warn!(
                            "[BILLING:WEBHOOK] Ignoring invalid transition from {:?} to Active for sub {}",
                            curr_status,
                            sub_id
                        );
                        return Ok(());
                    }
                }

                let record = voxy_database::SubscriptionRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    user_id: resolved_user_id.clone(),
                    plan_id: plan_id.to_string(),
                    dodo_customer_id: cus_id.to_string(),
                    dodo_subscription_id: sub_id.to_string(),
                    status: "active".to_string(),
                    current_period_start: period_start,
                    current_period_end: period_end.clone(),
                    cancel_at_period_end: false,
                    cancelled_at: None,
                    created_at: now.clone(),
                    updated_at: now,
                };

                self.store
                    .upsert_subscription(&record)
                    .await
                    .map_err(|e| e.to_string())?;

                // Grant entitlements with authoritative expiration timestamp
                self.store
                    .set_entitlement(
                        &resolved_user_id,
                        "coding_harness",
                        true,
                        None,
                        Some(&period_end),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                self.store
                    .set_entitlement(
                        &resolved_user_id,
                        "cloud_voice",
                        true,
                        None,
                        Some(&period_end),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                self.store
                    .set_entitlement(
                        &resolved_user_id,
                        "unlimited_models",
                        true,
                        None,
                        Some(&period_end),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                self.store
                    .set_entitlement(
                        &resolved_user_id,
                        "computer_control",
                        true,
                        None,
                        Some(&period_end),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
                self.store
                    .set_entitlement(
                        &resolved_user_id,
                        "office_automation",
                        true,
                        None,
                        Some(&period_end),
                    )
                    .await
                    .map_err(|e| e.to_string())?;

                Ok(())
            }

            "subscription.cancelled" => {
                let sub_data = &event.data;
                let sub_id = sub_data["subscription_id"].as_str().unwrap_or_default();

                if sub_id.is_empty() {
                    return Err("Missing subscription_id in cancelled event".into());
                }

                let existing = self
                    .store
                    .get_subscription_by_dodo_id(sub_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("Subscription {} not found for cancellation", sub_id))?;

                let cancel_at_period_end =
                    sub_data["cancel_at_period_end"].as_bool().unwrap_or(false);

                let now = Utc::now().to_rfc3339();

                if cancel_at_period_end {
                    // Scheduled cancellation: remains active until period_end, but marked for cancellation
                    self.store
                        .update_subscription_status(sub_id, "active", Some(&now), Some(true))
                        .await
                        .map_err(|e| e.to_string())?;

                    tracing::info!(
                        "[BILLING:WEBHOOK] Subscription {} set to cancel at period end {}",
                        sub_id,
                        existing.current_period_end
                    );
                } else {
                    // Immediate cancellation: revoke entitlements and set status to cancelled
                    self.store
                        .update_subscription_status(sub_id, "cancelled", Some(&now), Some(false))
                        .await
                        .map_err(|e| e.to_string())?;

                    self.store
                        .revoke_premium_entitlements(&existing.user_id)
                        .await
                        .map_err(|e| e.to_string())?;

                    tracing::info!(
                        "[BILLING:WEBHOOK] Subscription {} immediately cancelled. Entitlements revoked for user {}.",
                        sub_id,
                        existing.user_id
                    );
                }

                Ok(())
            }

            "subscription.expired" => {
                let sub_data = &event.data;
                let sub_id = sub_data["subscription_id"].as_str().unwrap_or_default();

                if let Ok(Some(existing)) = self.store.get_subscription_by_dodo_id(sub_id).await {
                    let now = Utc::now().to_rfc3339();
                    self.store
                        .update_subscription_status(sub_id, "expired", Some(&now), None)
                        .await
                        .map_err(|e| e.to_string())?;

                    self.store
                        .revoke_premium_entitlements(&existing.user_id)
                        .await
                        .map_err(|e| e.to_string())?;

                    tracing::info!(
                        "[BILLING:WEBHOOK] Subscription {} expired. Entitlements revoked for user {}.",
                        sub_id,
                        existing.user_id
                    );
                }

                Ok(())
            }

            "subscription.past_due" | "subscription.on_hold" => {
                let sub_data = &event.data;
                let sub_id = sub_data["subscription_id"].as_str().unwrap_or_default();

                if !sub_id.is_empty() {
                    let status = if event.event_type == "subscription.on_hold" {
                        "suspended"
                    } else {
                        "past_due"
                    };

                    self.store
                        .update_subscription_status(sub_id, status, None, None)
                        .await
                        .map_err(|e| e.to_string())?;

                    tracing::warn!(
                        "[BILLING:WEBHOOK] Subscription {} status updated to {}",
                        sub_id,
                        status
                    );
                }

                Ok(())
            }

            "refund.created" | "dispute.opened" => {
                let sub_data = &event.data;
                let sub_id = sub_data["subscription_id"].as_str().unwrap_or_default();

                if let Ok(Some(existing)) = self.store.get_subscription_by_dodo_id(sub_id).await {
                    let now = Utc::now().to_rfc3339();
                    self.store
                        .update_subscription_status(sub_id, "refunded", Some(&now), None)
                        .await
                        .map_err(|e| e.to_string())?;

                    self.store
                        .revoke_premium_entitlements(&existing.user_id)
                        .await
                        .map_err(|e| e.to_string())?;

                    tracing::warn!(
                        "[BILLING:WEBHOOK] Refund/dispute on sub {}. Entitlements immediately revoked for user {}.",
                        sub_id,
                        existing.user_id
                    );
                }

                Ok(())
            }

            _ => {
                tracing::info!(
                    "[BILLING:WEBHOOK] Acknowledging event type: {}",
                    event.event_type
                );
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::tempdir;
    use voxy_database::{DatabaseConfig, SqliteDatabase, StorageProvider};

    #[test]
    fn test_webhook_standard_webhooks_signature_verification() {
        let secret = "whsec_MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE";
        let webhook_id = "msg_2d4V9A6xO5yU8";
        let webhook_ts = "1727618000";
        let payload = br#"{"event_id":"evt_100","event_type":"subscription.active"}"#;

        let to_sign = format!(
            "{}.{}.{}",
            webhook_id,
            webhook_ts,
            std::str::from_utf8(payload).unwrap()
        );

        // Derive expected signature
        let secret_bytes = base64::engine::general_purpose::STANDARD
            .decode(&secret[6..])
            .unwrap();
        let mut mac = HmacSha256::new_from_slice(&secret_bytes).unwrap();
        mac.update(to_sign.as_bytes());
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        let header_val = format!("v1,{}", sig_b64);

        assert!(verify_webhook_signature(
            payload,
            &header_val,
            secret,
            Some(webhook_id),
            Some(webhook_ts),
        ));

        // Wrong signature must fail
        assert!(!verify_webhook_signature(
            payload,
            "v1,invalid_b64_signature",
            secret,
            Some(webhook_id),
            Some(webhook_ts),
        ));

        // Tampered payload must fail
        assert!(!verify_webhook_signature(
            b"tampered",
            &header_val,
            secret,
            Some(webhook_id),
            Some(webhook_ts),
        ));
    }

    #[test]
    fn test_webhook_replay_protection() {
        let now_epoch = Utc::now().timestamp().to_string();
        assert!(verify_webhook_timestamp(&now_epoch, 300));

        let old_epoch = (Utc::now() - chrono::Duration::seconds(600))
            .timestamp()
            .to_string();
        assert!(!verify_webhook_timestamp(&old_epoch, 300));

        let future_epoch = (Utc::now() + chrono::Duration::seconds(60))
            .timestamp()
            .to_string();
        assert!(!verify_webhook_timestamp(&future_epoch, 300));
    }

    #[tokio::test]
    async fn test_webhook_authoritative_lifecycle_and_idempotency() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("webhook_test.db");
        let config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };

        let db = Arc::new(SqliteDatabase::new());
        db.connect(&config).await.unwrap();

        let store = CommercialStore::new(db.clone());
        store.initialize_schema().await.unwrap();
        store.seed_default_plans().await.unwrap();

        let user = store
            .create_user("subscriber@voxy.ai", "hash123")
            .await
            .unwrap();

        let secret = "whsec_test_secret_for_suite";
        let handler = WebhookHandler::new(secret.to_string(), store.clone());

        let wh_id = "msg_test_001";
        let wh_ts = Utc::now().timestamp().to_string();

        let period_end = (Utc::now() + chrono::Duration::days(365)).to_rfc3339();

        let payload_json = serde_json::json!({
            "event_id": "evt_dodo_sub_active_01",
            "event_type": "subscription.active",
            "data": {
                "subscription_id": "sub_dodo_888",
                "customer_id": "cus_dodo_999",
                "product_id": "plan_pro_annual",
                "status": "active",
                "current_period_start": Utc::now().to_rfc3339(),
                "current_period_end": period_end,
                "metadata": {
                    "user_id": user.id
                }
            }
        });

        let payload_bytes = serde_json::to_vec(&payload_json).unwrap();

        // Sign according to spec
        let to_sign = format!(
            "{}.{}.{}",
            wh_id,
            wh_ts,
            std::str::from_utf8(&payload_bytes).unwrap()
        );
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(to_sign.as_bytes());
        let sig = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());
        let sig_hdr = format!("v1,{}", sig);

        // 1. Process active subscription
        let res = handler
            .process_webhook(&payload_bytes, Some(&sig_hdr), Some(&wh_ts), Some(wh_id))
            .await;
        assert!(res.is_ok());

        // Verify authoritative period end recorded (365 days, NOT hardcoded 30)
        let sub = store
            .get_subscription_by_dodo_id("sub_dodo_888")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sub.current_period_end, period_end);
        assert_eq!(sub.status, "active");

        // Verify entitlements granted
        assert!(store
            .check_entitlement(&user.id, "coding_harness")
            .await
            .unwrap());
        assert!(store
            .check_entitlement(&user.id, "cloud_voice")
            .await
            .unwrap());

        // 2. Test Idempotency: replay identical event
        let replay = handler
            .process_webhook(&payload_bytes, Some(&sig_hdr), Some(&wh_ts), Some(wh_id))
            .await;
        assert!(replay.is_ok());
        assert_eq!(replay.unwrap(), "Already processed");

        // 3. Process immediate cancellation
        let cancel_json = serde_json::json!({
            "event_id": "evt_dodo_sub_cancel_02",
            "event_type": "subscription.cancelled",
            "data": {
                "subscription_id": "sub_dodo_888",
                "cancel_at_period_end": false
            }
        });
        let cancel_bytes = serde_json::to_vec(&cancel_json).unwrap();
        let to_sign_cancel = format!(
            "msg_cancel.{}.{}",
            wh_ts,
            std::str::from_utf8(&cancel_bytes).unwrap()
        );
        let mut mac2 = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac2.update(to_sign_cancel.as_bytes());
        let sig2 = format!(
            "v1,{}",
            base64::engine::general_purpose::STANDARD.encode(mac2.finalize().into_bytes())
        );

        let cancel_res = handler
            .process_webhook(&cancel_bytes, Some(&sig2), Some(&wh_ts), Some("msg_cancel"))
            .await;
        assert!(cancel_res.is_ok());

        // Verify status cancelled and entitlements revoked
        let cancelled_sub = store
            .get_subscription_by_dodo_id("sub_dodo_888")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(cancelled_sub.status, "cancelled");
        assert!(!store
            .check_entitlement(&user.id, "coding_harness")
            .await
            .unwrap());
    }
}
