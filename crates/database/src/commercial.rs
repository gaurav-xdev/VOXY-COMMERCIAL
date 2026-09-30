//! Commercial Relational Schema, Models & Data Access Layer.
//!
//! Provides relational persistence for Users, Auth Identities, Sessions,
//! Plans, Subscriptions, Payment Customers, Entitlements, Webhooks, Consents,
//! and Audit trails with strict foreign keys, unique constraints, and migrations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::error::Result;
use crate::sqlite::SqliteDatabase;
use crate::storage::{Migration, StorageProvider, Value};

/// Returns the standard commercial migrations to be registered with the `MigrationRunner`.
pub fn get_commercial_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 100,
            name: "create_commercial_users_and_auth",
            sql: "
                CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY,
                    email TEXT UNIQUE NOT NULL,
                    password_hash TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'active',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS auth_identities (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    provider TEXT NOT NULL,
                    provider_user_id TEXT NOT NULL,
                    email TEXT,
                    created_at TEXT NOT NULL,
                    UNIQUE(provider, provider_user_id)
                );

                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    session_token_hash TEXT UNIQUE NOT NULL,
                    expires_at TEXT NOT NULL,
                    revoked_at TEXT,
                    created_at TEXT NOT NULL,
                    ip_address TEXT,
                    user_agent TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
                CREATE INDEX IF NOT EXISTS idx_sessions_token_hash ON sessions(session_token_hash);
            ",
        },
        Migration {
            version: 101,
            name: "create_commercial_plans_and_billing",
            sql: "
                CREATE TABLE IF NOT EXISTS plans (
                    id TEXT PRIMARY KEY,
                    product_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    tier TEXT NOT NULL,
                    billing_interval TEXT NOT NULL,
                    price_cents INTEGER NOT NULL,
                    currency TEXT NOT NULL DEFAULT 'USD',
                    is_active INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS payment_customers (
                    id TEXT PRIMARY KEY,
                    user_id TEXT UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    dodo_customer_id TEXT UNIQUE NOT NULL,
                    email TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS subscriptions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    plan_id TEXT NOT NULL REFERENCES plans(id),
                    dodo_customer_id TEXT NOT NULL,
                    dodo_subscription_id TEXT UNIQUE NOT NULL,
                    status TEXT NOT NULL,
                    current_period_start TEXT NOT NULL,
                    current_period_end TEXT NOT NULL,
                    cancel_at_period_end INTEGER NOT NULL DEFAULT 0,
                    cancelled_at TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_subscriptions_user_id ON subscriptions(user_id);
                CREATE INDEX IF NOT EXISTS idx_subscriptions_dodo_id ON subscriptions(dodo_subscription_id);

                CREATE TABLE IF NOT EXISTS entitlements (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    feature_key TEXT NOT NULL,
                    is_granted INTEGER NOT NULL DEFAULT 1,
                    max_usage INTEGER,
                    current_usage INTEGER NOT NULL DEFAULT 0,
                    expires_at TEXT,
                    updated_at TEXT NOT NULL,
                    UNIQUE(user_id, feature_key)
                );
                CREATE INDEX IF NOT EXISTS idx_entitlements_user_feature ON entitlements(user_id, feature_key);
            ",
        },
        Migration {
            version: 102,
            name: "create_commercial_webhooks_and_compliance",
            sql: "
                CREATE TABLE IF NOT EXISTS webhook_events (
                    id TEXT PRIMARY KEY,
                    event_id TEXT UNIQUE NOT NULL,
                    event_type TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    status TEXT NOT NULL,
                    error_message TEXT,
                    received_at TEXT NOT NULL,
                    processed_at TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_webhook_events_event_id ON webhook_events(event_id);

                CREATE TABLE IF NOT EXISTS commercial_audit_events (
                    id TEXT PRIMARY KEY,
                    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
                    action TEXT NOT NULL,
                    resource TEXT NOT NULL,
                    details TEXT NOT NULL DEFAULT '{}',
                    ip_address TEXT,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_commercial_audit_user ON commercial_audit_events(user_id);

                CREATE TABLE IF NOT EXISTS consents (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                    consent_type TEXT NOT NULL,
                    version TEXT NOT NULL,
                    is_accepted INTEGER NOT NULL DEFAULT 1,
                    ip_address TEXT,
                    user_agent TEXT,
                    timestamp TEXT NOT NULL,
                    UNIQUE(user_id, consent_type, version)
                );
                CREATE INDEX IF NOT EXISTS idx_consents_user ON consents(user_id);
            ",
        },
    ]
}

// ── Models ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub email: String,
    pub password_hash: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub user_id: String,
    pub session_token_hash: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub created_at: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanRecord {
    pub id: String,
    pub product_id: String,
    pub name: String,
    pub tier: String,
    pub billing_interval: String,
    pub price_cents: i64,
    pub currency: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionRecord {
    pub id: String,
    pub user_id: String,
    pub plan_id: String,
    pub dodo_customer_id: String,
    pub dodo_subscription_id: String,
    pub status: String,
    pub current_period_start: String,
    pub current_period_end: String,
    pub cancel_at_period_end: bool,
    pub cancelled_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitlementRecord {
    pub id: String,
    pub user_id: String,
    pub feature_key: String,
    pub is_granted: bool,
    pub max_usage: Option<i64>,
    pub current_usage: i64,
    pub expires_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookEventRecord {
    pub id: String,
    pub event_id: String,
    pub event_type: String,
    pub payload: String,
    pub status: String,
    pub error_message: Option<String>,
    pub received_at: String,
    pub processed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentRecord {
    pub id: String,
    pub user_id: String,
    pub consent_type: String,
    pub version: String,
    pub is_accepted: bool,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub timestamp: String,
}

// ── Commercial Data Store ──────────────────────────────────────────────

#[derive(Clone)]
pub struct CommercialStore {
    db: Arc<SqliteDatabase>,
}

impl CommercialStore {
    pub fn new(db: Arc<SqliteDatabase>) -> Self {
        Self { db }
    }

    /// Initializes and applies all commercial migrations to the database.
    pub async fn initialize_schema(&self) -> Result<()> {
        let migrations = get_commercial_migrations();
        self.db.run_migrations(&migrations).await
    }

    // ── User Operations ──

    pub async fn create_user(&self, email: &str, password_hash: &str) -> Result<UserRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let sql = "INSERT INTO users (id, email, password_hash, status, created_at, updated_at) VALUES (?, ?, ?, 'active', ?, ?)";
        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(email.to_string()),
                    Value::String(password_hash.to_string()),
                    Value::String(now.clone()),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(UserRecord {
            id,
            email: email.to_string(),
            password_hash: password_hash.to_string(),
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn get_user_by_email(&self, email: &str) -> Result<Option<UserRecord>> {
        let sql = "SELECT id, email, password_hash, status, created_at, updated_at FROM users WHERE email = ? LIMIT 1";
        let rows = self
            .db
            .query(sql, &[Value::String(email.to_string())])
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let r = &rows[0];
        Ok(Some(UserRecord {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            email: r["email"].as_str().unwrap_or_default().to_string(),
            password_hash: r["password_hash"].as_str().unwrap_or_default().to_string(),
            status: r["status"].as_str().unwrap_or_default().to_string(),
            created_at: r["created_at"].as_str().unwrap_or_default().to_string(),
            updated_at: r["updated_at"].as_str().unwrap_or_default().to_string(),
        }))
    }

    pub async fn get_user_by_id(&self, user_id: &str) -> Result<Option<UserRecord>> {
        let sql = "SELECT id, email, password_hash, status, created_at, updated_at FROM users WHERE id = ? LIMIT 1";
        let rows = self
            .db
            .query(sql, &[Value::String(user_id.to_string())])
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let r = &rows[0];
        Ok(Some(UserRecord {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            email: r["email"].as_str().unwrap_or_default().to_string(),
            password_hash: r["password_hash"].as_str().unwrap_or_default().to_string(),
            status: r["status"].as_str().unwrap_or_default().to_string(),
            created_at: r["created_at"].as_str().unwrap_or_default().to_string(),
            updated_at: r["updated_at"].as_str().unwrap_or_default().to_string(),
        }))
    }

    // ── Session Operations ──

    pub async fn create_session(
        &self,
        user_id: &str,
        session_token_hash: &str,
        expires_at: DateTime<Utc>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<SessionRecord> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let exp = expires_at.to_rfc3339();

        let sql = "INSERT INTO sessions (id, user_id, session_token_hash, expires_at, created_at, ip_address, user_agent) VALUES (?, ?, ?, ?, ?, ?, ?)";
        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(user_id.to_string()),
                    Value::String(session_token_hash.to_string()),
                    Value::String(exp.clone()),
                    Value::String(now.clone()),
                    ip_address
                        .as_ref()
                        .map(|s| Value::String(s.clone()))
                        .unwrap_or(Value::Null),
                    user_agent
                        .as_ref()
                        .map(|s| Value::String(s.clone()))
                        .unwrap_or(Value::Null),
                ],
            )
            .await?;

        Ok(SessionRecord {
            id,
            user_id: user_id.to_string(),
            session_token_hash: session_token_hash.to_string(),
            expires_at: exp,
            revoked_at: None,
            created_at: now,
            ip_address,
            user_agent,
        })
    }

    pub async fn get_session_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<SessionRecord>> {
        let sql = "SELECT id, user_id, session_token_hash, expires_at, revoked_at, created_at, ip_address, user_agent FROM sessions WHERE session_token_hash = ? AND revoked_at IS NULL LIMIT 1";
        let rows = self
            .db
            .query(sql, &[Value::String(token_hash.to_string())])
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let r = &rows[0];
        let exp_str = r["expires_at"].as_str().unwrap_or_default();
        if let Ok(exp) = DateTime::parse_from_rfc3339(exp_str) {
            if exp.to_utc() < Utc::now() {
                return Ok(None); // Expired session
            }
        }

        Ok(Some(SessionRecord {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            user_id: r["user_id"].as_str().unwrap_or_default().to_string(),
            session_token_hash: r["session_token_hash"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            expires_at: exp_str.to_string(),
            revoked_at: r["revoked_at"].as_str().map(|s| s.to_string()),
            created_at: r["created_at"].as_str().unwrap_or_default().to_string(),
            ip_address: r["ip_address"].as_str().map(|s| s.to_string()),
            user_agent: r["user_agent"].as_str().map(|s| s.to_string()),
        }))
    }

    pub async fn revoke_session(&self, token_hash: &str) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let sql = "UPDATE sessions SET revoked_at = ? WHERE session_token_hash = ? AND revoked_at IS NULL";
        let rows_affected = self
            .db
            .execute(
                sql,
                &[Value::String(now), Value::String(token_hash.to_string())],
            )
            .await?;
        Ok(rows_affected > 0)
    }

    pub async fn revoke_all_user_sessions(&self, user_id: &str) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let sql = "UPDATE sessions SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL";
        self.db
            .execute(
                sql,
                &[Value::String(now), Value::String(user_id.to_string())],
            )
            .await
    }

    // ── Plans & Subscriptions ──

    pub async fn seed_default_plans(&self) -> Result<()> {
        let plans = [
            (
                "plan_free",
                "prod_free",
                "VOXY Free",
                "free",
                "month",
                0,
                "USD",
            ),
            (
                "plan_pro_monthly",
                "prod_pro_m",
                "VOXY Pro Monthly",
                "pro",
                "month",
                2900,
                "USD",
            ),
            (
                "plan_pro_annual",
                "prod_pro_y",
                "VOXY Pro Annual",
                "pro",
                "year",
                29000,
                "USD",
            ),
            (
                "plan_team",
                "prod_team",
                "VOXY Enterprise",
                "enterprise",
                "month",
                9900,
                "USD",
            ),
        ];

        let now = Utc::now().to_rfc3339();
        for (id, prod, name, tier, interval, price, cur) in plans {
            let sql = "INSERT OR IGNORE INTO plans (id, product_id, name, tier, billing_interval, price_cents, currency, is_active, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?)";
            self.db
                .execute(
                    sql,
                    &[
                        Value::String(id.into()),
                        Value::String(prod.into()),
                        Value::String(name.into()),
                        Value::String(tier.into()),
                        Value::String(interval.into()),
                        Value::I64(price),
                        Value::String(cur.into()),
                        Value::String(now.clone()),
                    ],
                )
                .await?;
        }
        Ok(())
    }

    pub async fn upsert_subscription(&self, sub: &SubscriptionRecord) -> Result<()> {
        let sql = "
            INSERT INTO subscriptions (
                id, user_id, plan_id, dodo_customer_id, dodo_subscription_id,
                status, current_period_start, current_period_end, cancel_at_period_end,
                cancelled_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(dodo_subscription_id) DO UPDATE SET
                status = excluded.status,
                plan_id = excluded.plan_id,
                current_period_start = excluded.current_period_start,
                current_period_end = excluded.current_period_end,
                cancel_at_period_end = excluded.cancel_at_period_end,
                cancelled_at = excluded.cancelled_at,
                updated_at = excluded.updated_at
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(sub.id.clone()),
                    Value::String(sub.user_id.clone()),
                    Value::String(sub.plan_id.clone()),
                    Value::String(sub.dodo_customer_id.clone()),
                    Value::String(sub.dodo_subscription_id.clone()),
                    Value::String(sub.status.clone()),
                    Value::String(sub.current_period_start.clone()),
                    Value::String(sub.current_period_end.clone()),
                    Value::Bool(sub.cancel_at_period_end),
                    sub.cancelled_at
                        .as_ref()
                        .map(|s| Value::String(s.clone()))
                        .unwrap_or(Value::Null),
                    Value::String(sub.created_at.clone()),
                    Value::String(sub.updated_at.clone()),
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn get_active_subscription_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Option<SubscriptionRecord>> {
        let sql = "SELECT id, user_id, plan_id, dodo_customer_id, dodo_subscription_id, status, current_period_start, current_period_end, cancel_at_period_end, cancelled_at, created_at, updated_at FROM subscriptions WHERE user_id = ? AND status = 'active' ORDER BY created_at DESC LIMIT 1";
        let rows = self
            .db
            .query(sql, &[Value::String(user_id.to_string())])
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let r = &rows[0];
        Ok(Some(SubscriptionRecord {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            user_id: r["user_id"].as_str().unwrap_or_default().to_string(),
            plan_id: r["plan_id"].as_str().unwrap_or_default().to_string(),
            dodo_customer_id: r["dodo_customer_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            dodo_subscription_id: r["dodo_subscription_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            status: r["status"].as_str().unwrap_or_default().to_string(),
            current_period_start: r["current_period_start"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            current_period_end: r["current_period_end"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            cancel_at_period_end: r["cancel_at_period_end"].as_i64().unwrap_or(0) == 1,
            cancelled_at: r["cancelled_at"].as_str().map(|s| s.to_string()),
            created_at: r["created_at"].as_str().unwrap_or_default().to_string(),
            updated_at: r["updated_at"].as_str().unwrap_or_default().to_string(),
        }))
    }

    pub async fn get_subscription_by_dodo_id(
        &self,
        dodo_subscription_id: &str,
    ) -> Result<Option<SubscriptionRecord>> {
        let sql = "SELECT id, user_id, plan_id, dodo_customer_id, dodo_subscription_id, status, current_period_start, current_period_end, cancel_at_period_end, cancelled_at, created_at, updated_at FROM subscriptions WHERE dodo_subscription_id = ? LIMIT 1";
        let rows = self
            .db
            .query(sql, &[Value::String(dodo_subscription_id.to_string())])
            .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let r = &rows[0];
        Ok(Some(SubscriptionRecord {
            id: r["id"].as_str().unwrap_or_default().to_string(),
            user_id: r["user_id"].as_str().unwrap_or_default().to_string(),
            plan_id: r["plan_id"].as_str().unwrap_or_default().to_string(),
            dodo_customer_id: r["dodo_customer_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            dodo_subscription_id: r["dodo_subscription_id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            status: r["status"].as_str().unwrap_or_default().to_string(),
            current_period_start: r["current_period_start"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            current_period_end: r["current_period_end"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            cancel_at_period_end: r["cancel_at_period_end"].as_i64().unwrap_or(0) == 1,
            cancelled_at: r["cancelled_at"].as_str().map(|s| s.to_string()),
            created_at: r["created_at"].as_str().unwrap_or_default().to_string(),
            updated_at: r["updated_at"].as_str().unwrap_or_default().to_string(),
        }))
    }

    pub async fn update_subscription_status(
        &self,
        dodo_subscription_id: &str,
        status: &str,
        cancelled_at: Option<&str>,
        cancel_at_period_end: Option<bool>,
    ) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let sql = "UPDATE subscriptions SET status = ?, cancelled_at = COALESCE(?, cancelled_at), cancel_at_period_end = COALESCE(?, cancel_at_period_end), updated_at = ? WHERE dodo_subscription_id = ?";
        let rows = self
            .db
            .execute(
                sql,
                &[
                    Value::String(status.to_string()),
                    cancelled_at
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    cancel_at_period_end.map(Value::Bool).unwrap_or(Value::Null),
                    Value::String(now),
                    Value::String(dodo_subscription_id.to_string()),
                ],
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn revoke_premium_entitlements(&self, user_id: &str) -> Result<()> {
        let premium_features = [
            "coding_harness",
            "cloud_voice",
            "unlimited_models",
            "computer_control",
            "office_automation",
            "multi_agent_teams",
            "priority_cloud_routing",
        ];
        for feat in premium_features {
            self.set_entitlement(user_id, feat, false, None, None)
                .await?;
        }
        Ok(())
    }

    // ── Entitlements ──

    pub async fn set_entitlement(
        &self,
        user_id: &str,
        feature_key: &str,
        is_granted: bool,
        max_usage: Option<i64>,
        expires_at: Option<&str>,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let sql = "
            INSERT INTO entitlements (id, user_id, feature_key, is_granted, max_usage, current_usage, expires_at, updated_at)
            VALUES (?, ?, ?, ?, ?, 0, ?, ?)
            ON CONFLICT(user_id, feature_key) DO UPDATE SET
                is_granted = excluded.is_granted,
                max_usage = excluded.max_usage,
                expires_at = excluded.expires_at,
                updated_at = excluded.updated_at
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id),
                    Value::String(user_id.to_string()),
                    Value::String(feature_key.to_string()),
                    Value::Bool(is_granted),
                    max_usage.map(Value::I64).unwrap_or(Value::Null),
                    expires_at
                        .map(|e| Value::String(e.to_string()))
                        .unwrap_or(Value::Null),
                    Value::String(now),
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn check_entitlement(&self, user_id: &str, feature_key: &str) -> Result<bool> {
        let sql = "SELECT is_granted, max_usage, current_usage, expires_at FROM entitlements WHERE user_id = ? AND feature_key = ? LIMIT 1";
        let rows = self
            .db
            .query(
                sql,
                &[
                    Value::String(user_id.to_string()),
                    Value::String(feature_key.to_string()),
                ],
            )
            .await?;

        if rows.is_empty() {
            return Ok(false);
        }

        let is_granted = rows[0]["is_granted"].as_i64().unwrap_or(0) == 1;
        if !is_granted {
            return Ok(false);
        }

        // Authoritative expiration verification
        if let Some(exp_str) = rows[0]["expires_at"].as_str() {
            if let Ok(exp_dt) = DateTime::parse_from_rfc3339(exp_str) {
                if exp_dt.to_utc() < Utc::now() {
                    return Ok(false); // Entitlement expired
                }
            }
        }

        if let Some(max_usage) = rows[0]["max_usage"].as_i64() {
            let current_usage = rows[0]["current_usage"].as_i64().unwrap_or(0);
            return Ok(current_usage < max_usage);
        }

        Ok(true)
    }

    // ── Webhook Idempotency ──

    pub async fn is_webhook_processed(&self, event_id: &str) -> Result<bool> {
        let sql = "SELECT status FROM webhook_events WHERE event_id = ? LIMIT 1";
        let rows = self
            .db
            .query(sql, &[Value::String(event_id.to_string())])
            .await?;
        if rows.is_empty() {
            return Ok(false);
        }
        let status = rows[0]["status"].as_str().unwrap_or_default();
        Ok(status == "processed")
    }

    pub async fn record_webhook_event(
        &self,
        event_id: &str,
        event_type: &str,
        payload: &str,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let sql = "
            INSERT INTO webhook_events (id, event_id, event_type, payload, status, received_at)
            VALUES (?, ?, ?, ?, 'received', ?)
            ON CONFLICT(event_id) DO NOTHING
        ";
        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(event_id.to_string()),
                    Value::String(event_type.to_string()),
                    Value::String(payload.to_string()),
                    Value::String(now),
                ],
            )
            .await?;

        Ok(id)
    }

    pub async fn mark_webhook_processed(
        &self,
        event_id: &str,
        success: bool,
        error: Option<String>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let status = if success { "processed" } else { "failed" };
        let sql = "UPDATE webhook_events SET status = ?, error_message = ?, processed_at = ? WHERE event_id = ?";
        self.db
            .execute(
                sql,
                &[
                    Value::String(status.into()),
                    error.map(Value::String).unwrap_or(Value::Null),
                    Value::String(now),
                    Value::String(event_id.to_string()),
                ],
            )
            .await?;
        Ok(())
    }

    // ── Consents ──

    pub async fn record_consent(
        &self,
        user_id: &str,
        consent_type: &str,
        version: &str,
        is_accepted: bool,
        ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let sql = "
            INSERT INTO consents (id, user_id, consent_type, version, is_accepted, ip_address, user_agent, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, consent_type, version) DO UPDATE SET
                is_accepted = excluded.is_accepted,
                timestamp = excluded.timestamp
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id),
                    Value::String(user_id.to_string()),
                    Value::String(consent_type.to_string()),
                    Value::String(version.to_string()),
                    Value::Bool(is_accepted),
                    ip.map(Value::String).unwrap_or(Value::Null),
                    user_agent.map(Value::String).unwrap_or(Value::Null),
                    Value::String(now),
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn get_consents_by_user_id(&self, user_id: &str) -> Result<Vec<ConsentRecord>> {
        let sql = "SELECT id, user_id, consent_type, version, is_accepted, ip_address, user_agent, timestamp FROM consents WHERE user_id = ?";
        let rows = self
            .db
            .query(sql, &[Value::String(user_id.to_string())])
            .await?;
        let mut list = Vec::new();
        for r in rows {
            list.push(ConsentRecord {
                id: r["id"].as_str().unwrap_or_default().to_string(),
                user_id: r["user_id"].as_str().unwrap_or_default().to_string(),
                consent_type: r["consent_type"].as_str().unwrap_or_default().to_string(),
                version: r["version"].as_str().unwrap_or_default().to_string(),
                is_accepted: r["is_accepted"].as_i64().unwrap_or(0) == 1
                    || r["is_accepted"].as_bool().unwrap_or(false),
                ip_address: r["ip_address"].as_str().map(|s| s.to_string()),
                user_agent: r["user_agent"].as_str().map(|s| s.to_string()),
                timestamp: r["timestamp"].as_str().unwrap_or_default().to_string(),
            });
        }
        Ok(list)
    }

    pub async fn delete_user(&self, user_id: &str) -> Result<bool> {
        let sql = "DELETE FROM users WHERE id = ?";
        let affected = self
            .db
            .execute(sql, &[Value::String(user_id.to_string())])
            .await?;
        Ok(affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DatabaseConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_database_migration_runner() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("commercial_test.db");
        let config = DatabaseConfig {
            path: Some(db_path.to_string_lossy().to_string()),
            ..Default::default()
        };

        let db = Arc::new(SqliteDatabase::new());
        db.connect(&config).await.unwrap();

        let store = CommercialStore::new(db.clone());
        store.initialize_schema().await.unwrap();

        // 1. User creation and retrieval
        let user = store
            .create_user("founder@voxy.ai", "argon2id_hash_placeholder")
            .await
            .unwrap();
        assert_eq!(user.email, "founder@voxy.ai");

        let fetched = store
            .get_user_by_email("founder@voxy.ai")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.id, user.id);

        // 2. Session creation and token lookup
        let token_hash = "f3b2597b864a781079d2a6a8b7a4c7e6";
        let session = store
            .create_session(
                &user.id,
                token_hash,
                Utc::now() + chrono::Duration::hours(24),
                Some("127.0.0.1".into()),
                Some("VOXY-Client/1.0".into()),
            )
            .await
            .unwrap();
        assert_eq!(session.user_id, user.id);

        let active_sess = store.get_session_by_token_hash(token_hash).await.unwrap();
        assert!(active_sess.is_some());

        // Revoke session
        assert!(store.revoke_session(token_hash).await.unwrap());
        let revoked_sess = store.get_session_by_token_hash(token_hash).await.unwrap();
        assert!(revoked_sess.is_none());

        // 3. Plans seeding
        store.seed_default_plans().await.unwrap();

        // 4. Subscriptions & Entitlements
        let sub = SubscriptionRecord {
            id: Uuid::new_v4().to_string(),
            user_id: user.id.clone(),
            plan_id: "plan_pro_monthly".into(),
            dodo_customer_id: "cus_12345".into(),
            dodo_subscription_id: "sub_12345".into(),
            status: "active".into(),
            current_period_start: Utc::now().to_rfc3339(),
            current_period_end: (Utc::now() + chrono::Duration::days(30)).to_rfc3339(),
            cancel_at_period_end: false,
            cancelled_at: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        store.upsert_subscription(&sub).await.unwrap();

        let active_sub = store
            .get_active_subscription_by_user_id(&user.id)
            .await
            .unwrap();
        assert!(active_sub.is_some());
        assert_eq!(active_sub.unwrap().plan_id, "plan_pro_monthly");

        // Grant entitlement
        store
            .set_entitlement(&user.id, "coding_harness", true, None, None)
            .await
            .unwrap();
        assert!(store
            .check_entitlement(&user.id, "coding_harness")
            .await
            .unwrap());
        assert!(!store
            .check_entitlement(&user.id, "unauthorized_feature")
            .await
            .unwrap());

        // Test expired entitlement
        let past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        store
            .set_entitlement(&user.id, "temp_feature", true, None, Some(&past))
            .await
            .unwrap();
        assert!(!store
            .check_entitlement(&user.id, "temp_feature")
            .await
            .unwrap());

        // 5. Webhook idempotency
        let event_id = "evt_dodo_9999";
        assert!(!store.is_webhook_processed(event_id).await.unwrap());

        store
            .record_webhook_event(event_id, "subscription.active", "{}")
            .await
            .unwrap();
        store
            .mark_webhook_processed(event_id, true, None)
            .await
            .unwrap();

        assert!(store.is_webhook_processed(event_id).await.unwrap());

        // 6. Consents
        store
            .record_consent(
                &user.id,
                "terms_of_service",
                "1.0",
                true,
                Some("127.0.0.1".into()),
                None,
            )
            .await
            .unwrap();
    }
}
