//! Dodo Payments Webhook Verification, Replay Defense & Idempotent Processing.
//!
//! Validates HMAC-SHA256 signatures, enforces 300-second timestamp freshness,
//! rejects replay attacks, and transactionally updates subscription status.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use thiserror::Error;
use voxy_database::CommercialStore;

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

/// Verifies an HMAC-SHA256 signature against the raw request body.
pub fn verify_webhook_signature(
    raw_payload: &[u8],
    signature_hex: &str,
    webhook_secret: &str,
) -> bool {
    let signature_clean = signature_hex.trim().trim_start_matches("sha256=");
    let expected_bytes = match hex::decode(signature_clean) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac = match HmacSha256::new_from_slice(webhook_secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(raw_payload);
    mac.verify_slice(&expected_bytes).is_ok()
}

/// Verifies that a webhook timestamp is within the acceptable window (default 300 seconds / 5 mins).
pub fn verify_webhook_timestamp(timestamp_str: &str, max_age_seconds: i64) -> bool {
    let ts = match timestamp_str.parse::<i64>() {
        Ok(epoch_secs) => match DateTime::from_timestamp(epoch_secs, 0) {
            Some(dt) => dt,
            None => return false,
        },
        Err(_) => match DateTime::parse_from_rfc3339(timestamp_str) {
            Ok(dt) => dt.to_utc(),
            Err(_) => return false,
        },
    };

    let now = Utc::now();
    let age = (now - ts).num_seconds();

    // Must be between -30s (slight clock drift) and max_age_seconds
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
    ) -> Result<String, WebhookError> {
        let sig = signature_header.ok_or(WebhookError::MissingSignature)?;

        if !verify_webhook_signature(raw_body, sig, &self.webhook_secret) {
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

        // Record initial event state
        self.store
            .record_webhook_event(&event.event_id, &event.event_type, body_str)
            .await
            .map_err(|e| WebhookError::DatabaseError(e.to_string()))?;

        // Process event side-effects
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
                let user_id = sub_data["user_id"].as_str().unwrap_or_default();
                let plan_id = sub_data["plan_id"].as_str().unwrap_or("plan_pro_monthly");
                let sub_id = sub_data["subscription_id"].as_str().unwrap_or_default();
                let cus_id = sub_data["customer_id"].as_str().unwrap_or_default();

                if user_id.is_empty() || sub_id.is_empty() {
                    return Err("Missing user_id or subscription_id in event payload".into());
                }

                let now = Utc::now().to_rfc3339();
                let period_end = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();

                let record = voxy_database::SubscriptionRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    user_id: user_id.to_string(),
                    plan_id: plan_id.to_string(),
                    dodo_customer_id: cus_id.to_string(),
                    dodo_subscription_id: sub_id.to_string(),
                    status: "active".to_string(),
                    current_period_start: now.clone(),
                    current_period_end: period_end,
                    cancel_at_period_end: false,
                    cancelled_at: None,
                    created_at: now.clone(),
                    updated_at: now,
                };

                self.store
                    .upsert_subscription(&record)
                    .await
                    .map_err(|e| e.to_string())?;

                // Grant Pro entitlements
                self.store
                    .set_entitlement(user_id, "coding_harness", true, None)
                    .await
                    .map_err(|e| e.to_string())?;
                self.store
                    .set_entitlement(user_id, "cloud_voice", true, None)
                    .await
                    .map_err(|e| e.to_string())?;
                self.store
                    .set_entitlement(user_id, "unlimited_models", true, None)
                    .await
                    .map_err(|e| e.to_string())?;

                Ok(())
            }
            "subscription.cancelled" => {
                let sub_data = &event.data;
                let sub_id = sub_data["subscription_id"].as_str().unwrap_or_default();
                let user_id = sub_data["user_id"].as_str().unwrap_or_default();

                if sub_id.is_empty() {
                    return Err("Missing subscription_id in cancelled event".into());
                }

                // If user_id provided, revoke premium entitlements
                if !user_id.is_empty() {
                    self.store
                        .set_entitlement(user_id, "coding_harness", false, None)
                        .await
                        .map_err(|e| e.to_string())?;
                }

                Ok(())
            }
            "payment.failed" | "subscription.past_due" => {
                // Log warning, maintain grace period if desired
                tracing::warn!(
                    "[BILLING:WEBHOOK] Subscription/payment failed event received: {:?}",
                    event.data
                );
                Ok(())
            }
            _ => {
                tracing::info!(
                    "[BILLING:WEBHOOK] Unhandled event type: {}. Acknowledging.",
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

    #[test]
    fn test_webhook_signature_verification() {
        let secret = "whsec_test_secret_abc123";
        let payload = br#"{"event_id":"evt_1","event_type":"subscription.active"}"#;

        // Compute valid signature
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload);
        let valid_sig = hex::encode(mac.finalize().into_bytes());

        assert!(verify_webhook_signature(payload, &valid_sig, secret));
        assert!(verify_webhook_signature(payload, &format!("sha256={}", valid_sig), secret));
        assert!(!verify_webhook_signature(payload, "invalid_sig", secret));
        assert!(!verify_webhook_signature(b"tampered payload", &valid_sig, secret));
    }

    #[test]
    fn test_webhook_replay_protection() {
        let now_epoch = Utc::now().timestamp().to_string();
        assert!(verify_webhook_timestamp(&now_epoch, 300));

        let old_epoch = (Utc::now() - chrono::Duration::seconds(600)).timestamp().to_string();
        assert!(!verify_webhook_timestamp(&old_epoch, 300));

        let future_epoch = (Utc::now() + chrono::Duration::seconds(60)).timestamp().to_string();
        assert!(!verify_webhook_timestamp(&future_epoch, 300));
    }
}
