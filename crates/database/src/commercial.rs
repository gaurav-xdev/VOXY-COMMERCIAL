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
        Migration {
            version: 103,
            name: "create_tasks_and_artifacts_history",
            sql: "
                CREATE TABLE IF NOT EXISTS tasks (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    workspace_id TEXT,
                    user_goal TEXT NOT NULL,
                    task_type TEXT NOT NULL,
                    status TEXT NOT NULL,
                    phase TEXT NOT NULL,
                    progress REAL NOT NULL DEFAULT 0.0,
                    parent_task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
                    error_message TEXT,
                    result_summary TEXT,
                    created_at TEXT NOT NULL,
                    started_at TEXT,
                    ended_at TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_tasks_session ON tasks(session_id);
                CREATE INDEX IF NOT EXISTS idx_tasks_workspace ON tasks(workspace_id);
                CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
                CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);

                CREATE TABLE IF NOT EXISTS task_artifacts (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                    workspace_id TEXT,
                    artifact_type TEXT NOT NULL,
                    name TEXT NOT NULL,
                    relative_path TEXT NOT NULL,
                    storage_uri TEXT NOT NULL,
                    content_hash TEXT NOT NULL,
                    size_bytes INTEGER NOT NULL,
                    metadata TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_task_artifacts_task ON task_artifacts(task_id);
                CREATE INDEX IF NOT EXISTS idx_task_artifacts_workspace ON task_artifacts(workspace_id);
                CREATE INDEX IF NOT EXISTS idx_task_artifacts_hash ON task_artifacts(content_hash);

                CREATE TABLE IF NOT EXISTS task_tool_invocations (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                    tool_name TEXT NOT NULL,
                    risk_tier TEXT NOT NULL,
                    approval_id TEXT,
                    duration_ms INTEGER NOT NULL,
                    was_success INTEGER NOT NULL,
                    error_message TEXT,
                    invoked_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_task_tool_invocations_task ON task_tool_invocations(task_id);
            ",
        },
        Migration {
            version: 104,
            name: "create_scoped_memories_and_provenance",
            sql: "
                CREATE TABLE IF NOT EXISTS scoped_memories (
                    id TEXT PRIMARY KEY,
                    account_id TEXT NOT NULL,
                    workspace_id TEXT,
                    memory_type TEXT NOT NULL,
                    content TEXT NOT NULL,
                    source TEXT NOT NULL,
                    source_reference TEXT,
                    confidence REAL NOT NULL DEFAULT 1.0,
                    importance REAL NOT NULL DEFAULT 0.5,
                    provenance TEXT NOT NULL DEFAULT '{}',
                    sensitivity TEXT NOT NULL DEFAULT 'standard',
                    status TEXT NOT NULL DEFAULT 'active',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    last_accessed_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_scoped_memories_account ON scoped_memories(account_id);
                CREATE INDEX IF NOT EXISTS idx_scoped_memories_workspace ON scoped_memories(workspace_id);
                CREATE INDEX IF NOT EXISTS idx_scoped_memories_type ON scoped_memories(memory_type);
                CREATE INDEX IF NOT EXISTS idx_scoped_memories_status ON scoped_memories(status);
                CREATE INDEX IF NOT EXISTS idx_scoped_memories_created ON scoped_memories(created_at);
            ",
        },
        Migration {
            version: 105,
            name: "create_skills_and_workflows_ecosystem",
            sql: "
                CREATE TABLE IF NOT EXISTS skills (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    description TEXT NOT NULL,
                    version TEXT NOT NULL,
                    publisher_id TEXT NOT NULL,
                    trust_level TEXT NOT NULL DEFAULT 'untrusted',
                    manifest_json TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'active',
                    checksum TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    UNIQUE(name, version)
                );
                CREATE INDEX IF NOT EXISTS idx_skills_name ON skills(name);
                CREATE INDEX IF NOT EXISTS idx_skills_trust ON skills(trust_level);

                CREATE TABLE IF NOT EXISTS workflows (
                    id TEXT PRIMARY KEY,
                    account_id TEXT NOT NULL,
                    workspace_id TEXT,
                    name TEXT NOT NULL,
                    description TEXT NOT NULL,
                    version TEXT NOT NULL,
                    workflow_definition TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'active',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_workflows_account ON workflows(account_id);
                CREATE INDEX IF NOT EXISTS idx_workflows_workspace ON workflows(workspace_id);

                CREATE TABLE IF NOT EXISTS workflow_executions (
                    id TEXT PRIMARY KEY,
                    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
                    account_id TEXT NOT NULL,
                    workspace_id TEXT,
                    status TEXT NOT NULL,
                    current_step INTEGER NOT NULL DEFAULT 0,
                    total_steps INTEGER NOT NULL DEFAULT 0,
                    input_data TEXT NOT NULL DEFAULT '{}',
                    output_data TEXT,
                    error_message TEXT,
                    started_at TEXT NOT NULL,
                    ended_at TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_workflow_executions_wf ON workflow_executions(workflow_id);
                CREATE INDEX IF NOT EXISTS idx_workflow_executions_account ON workflow_executions(account_id);
                CREATE INDEX IF NOT EXISTS idx_workflow_executions_status ON workflow_executions(status);
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Queued,
    Running,
    WaitingApproval,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Expired,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "QUEUED",
            Self::Running => "RUNNING",
            Self::WaitingApproval => "WAITING_APPROVAL",
            Self::Paused => "PAUSED",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Expired => "EXPIRED",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "QUEUED" => Self::Queued,
            "RUNNING" => Self::Running,
            "WAITING_APPROVAL" => Self::WaitingApproval,
            "PAUSED" => Self::Paused,
            "COMPLETED" => Self::Completed,
            "FAILED" => Self::Failed,
            "CANCELLED" => Self::Cancelled,
            "EXPIRED" => Self::Expired,
            _ => Self::Failed,
        }
    }

    /// Validates if transition from `self` to `target` is legally permitted.
    pub fn can_transition_to(&self, target: &TaskStatus) -> bool {
        match (self, target) {
            (Self::Queued, Self::Running) | (Self::Queued, Self::Cancelled) => true,
            (Self::Running, Self::WaitingApproval)
            | (Self::Running, Self::Paused)
            | (Self::Running, Self::Completed)
            | (Self::Running, Self::Failed)
            | (Self::Running, Self::Cancelled) => true,
            (Self::WaitingApproval, Self::Running)
            | (Self::WaitingApproval, Self::Cancelled)
            | (Self::WaitingApproval, Self::Expired) => true,
            (Self::Paused, Self::Running) | (Self::Paused, Self::Cancelled) => true,
            // Terminal states cannot transition
            (Self::Completed, _)
            | (Self::Failed, _)
            | (Self::Cancelled, _)
            | (Self::Expired, _) => false,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub session_id: String,
    pub workspace_id: Option<String>,
    pub user_goal: String,
    pub task_type: String,
    pub status: TaskStatus,
    pub phase: String,
    pub progress: f64,
    pub parent_task_id: Option<String>,
    pub error_message: Option<String>,
    pub result_summary: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskArtifactRecord {
    pub id: String,
    pub task_id: String,
    pub workspace_id: Option<String>,
    pub artifact_type: String,
    pub name: String,
    pub relative_path: String,
    pub storage_uri: String,
    pub content_hash: String,
    pub size_bytes: i64,
    pub metadata: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskToolInvocationRecord {
    pub id: String,
    pub task_id: String,
    pub tool_name: String,
    pub risk_tier: String,
    pub approval_id: Option<String>,
    pub duration_ms: i64,
    pub was_success: bool,
    pub error_message: Option<String>,
    pub invoked_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopedMemoryRecord {
    pub id: String,
    pub account_id: String,
    pub workspace_id: Option<String>,
    pub memory_type: String,
    pub content: String,
    pub source: String,
    pub source_reference: Option<String>,
    pub confidence: f64,
    pub importance: f64,
    pub provenance: String,
    pub sensitivity: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRecord {
    pub id: String,
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub version: String,
    pub publisher_id: String,
    pub trust_level: String,
    pub manifest_json: String,
    pub status: String,
    pub checksum: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowRecord {
    pub id: String,
    pub account_id: String,
    pub workspace_id: Option<String>,
    pub name: String,
    pub description: String,
    pub version: String,
    pub workflow_definition: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowExecutionRecord {
    pub id: String,
    pub workflow_id: String,
    pub account_id: String,
    pub workspace_id: Option<String>,
    pub status: String,
    pub current_step: i64,
    pub total_steps: i64,
    pub input_data: String,
    pub output_data: Option<String>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
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

    // ── Task & Artifact History Operations ──

    pub async fn create_task(
        &self,
        session_id: &str,
        workspace_id: Option<&str>,
        user_goal: &str,
        task_type: &str,
        parent_task_id: Option<&str>,
    ) -> Result<TaskRecord> {
        let id = format!("task_{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        let status = TaskStatus::Queued;
        let phase = "QUEUED";

        let sql = "
            INSERT INTO tasks (
                id, session_id, workspace_id, user_goal, task_type, status,
                phase, progress, parent_task_id, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 0.0, ?, ?)
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(session_id.to_string()),
                    workspace_id
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::String(user_goal.to_string()),
                    Value::String(task_type.to_string()),
                    Value::String(status.as_str().to_string()),
                    Value::String(phase.to_string()),
                    parent_task_id
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(TaskRecord {
            id,
            session_id: session_id.to_string(),
            workspace_id: workspace_id.map(|s| s.to_string()),
            user_goal: user_goal.to_string(),
            task_type: task_type.to_string(),
            status,
            phase: phase.to_string(),
            progress: 0.0,
            parent_task_id: parent_task_id.map(|s| s.to_string()),
            error_message: None,
            result_summary: None,
            created_at: now,
            started_at: None,
            ended_at: None,
        })
    }

    pub async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>> {
        let sql = "
            SELECT id, session_id, workspace_id, user_goal, task_type, status,
                   phase, progress, parent_task_id, error_message, result_summary,
                   created_at, started_at, ended_at
            FROM tasks WHERE id = ?
        ";

        let rows = self
            .db
            .query(sql, &[Value::String(task_id.to_string())])
            .await?;

        if let Some(row) = rows.into_iter().next() {
            let status_str = row
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("FAILED");
            let progress = row
                .get("progress")
                .and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|i| i as f64)))
                .unwrap_or(0.0);

            Ok(Some(TaskRecord {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                session_id: row
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                workspace_id: row
                    .get("workspace_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                user_goal: row
                    .get("user_goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                task_type: row
                    .get("task_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                status: TaskStatus::from_str(status_str),
                phase: row
                    .get("phase")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                progress,
                parent_task_id: row
                    .get("parent_task_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                error_message: row
                    .get("error_message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                result_summary: row
                    .get("result_summary")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                created_at: row
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                started_at: row
                    .get("started_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                ended_at: row
                    .get("ended_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn update_task_status(
        &self,
        task_id: &str,
        new_status: TaskStatus,
        phase: Option<&str>,
        progress: Option<f64>,
        error_message: Option<&str>,
        result_summary: Option<&str>,
    ) -> Result<bool> {
        let current = match self.get_task(task_id).await? {
            Some(t) => t,
            None => return Ok(false),
        };

        if !current.status.can_transition_to(&new_status) {
            return Err(crate::error::DatabaseError::QueryFailed(format!(
                "Illegal task status transition from {:?} to {:?}",
                current.status, new_status
            )));
        }

        let now = Utc::now().to_rfc3339();
        let phase_str = phase.unwrap_or(&current.phase);
        let progress_val = progress.unwrap_or(current.progress);

        let started_at_update =
            if current.status == TaskStatus::Queued && new_status == TaskStatus::Running {
                Some(now.clone())
            } else {
                current.started_at.clone()
            };

        let ended_at_update = match new_status {
            TaskStatus::Completed
            | TaskStatus::Failed
            | TaskStatus::Cancelled
            | TaskStatus::Expired => Some(now.clone()),
            _ => None,
        };

        let sql = "
            UPDATE tasks SET status = ?, phase = ?, progress = ?,
                             error_message = ?, result_summary = ?,
                             started_at = COALESCE(started_at, ?),
                             ended_at = ?
            WHERE id = ?
        ";

        let affected = self
            .db
            .execute(
                sql,
                &[
                    Value::String(new_status.as_str().to_string()),
                    Value::String(phase_str.to_string()),
                    Value::F64(progress_val),
                    error_message
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    result_summary
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    started_at_update
                        .map(|s| Value::String(s))
                        .unwrap_or(Value::Null),
                    ended_at_update
                        .map(|s| Value::String(s))
                        .unwrap_or(Value::Null),
                    Value::String(task_id.to_string()),
                ],
            )
            .await?;

        Ok(affected > 0)
    }

    /// Recovers from abnormal process shutdown by marking any uncompleted Running tasks as Failed.
    pub async fn recover_interrupted_tasks(&self) -> Result<u64> {
        let now = Utc::now().to_rfc3339();
        let sql = "
            UPDATE tasks SET status = 'FAILED',
                             phase = 'INTERRUPTED',
                             error_message = 'Task interrupted by system restart or unhandled termination',
                             ended_at = ?
            WHERE status = 'RUNNING' OR status = 'QUEUED'
        ";

        let affected = self.db.execute(sql, &[Value::String(now)]).await?;

        Ok(affected)
    }

    pub async fn create_artifact(
        &self,
        task_id: &str,
        workspace_id: Option<&str>,
        artifact_type: &str,
        name: &str,
        relative_path: &str,
        storage_uri: &str,
        content_hash: &str,
        size_bytes: i64,
        metadata: &str,
    ) -> Result<TaskArtifactRecord> {
        let id = format!("art_{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();

        let sql = "
            INSERT INTO task_artifacts (
                id, task_id, workspace_id, artifact_type, name,
                relative_path, storage_uri, content_hash, size_bytes,
                metadata, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(task_id.to_string()),
                    workspace_id
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::String(artifact_type.to_string()),
                    Value::String(name.to_string()),
                    Value::String(relative_path.to_string()),
                    Value::String(storage_uri.to_string()),
                    Value::String(content_hash.to_string()),
                    Value::I64(size_bytes),
                    Value::String(metadata.to_string()),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(TaskArtifactRecord {
            id,
            task_id: task_id.to_string(),
            workspace_id: workspace_id.map(|s| s.to_string()),
            artifact_type: artifact_type.to_string(),
            name: name.to_string(),
            relative_path: relative_path.to_string(),
            storage_uri: storage_uri.to_string(),
            content_hash: content_hash.to_string(),
            size_bytes,
            metadata: metadata.to_string(),
            created_at: now,
        })
    }

    pub async fn get_artifacts_for_task(&self, task_id: &str) -> Result<Vec<TaskArtifactRecord>> {
        let sql = "
            SELECT id, task_id, workspace_id, artifact_type, name,
                   relative_path, storage_uri, content_hash, size_bytes,
                   metadata, created_at
            FROM task_artifacts WHERE task_id = ? ORDER BY created_at ASC
        ";

        let rows = self
            .db
            .query(sql, &[Value::String(task_id.to_string())])
            .await?;

        let mut list = Vec::new();
        for row in rows {
            let size = row.get("size_bytes").and_then(|v| v.as_i64()).unwrap_or(0);

            list.push(TaskArtifactRecord {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                task_id: row
                    .get("task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                workspace_id: row
                    .get("workspace_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                artifact_type: row
                    .get("artifact_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: row
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                relative_path: row
                    .get("relative_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                storage_uri: row
                    .get("storage_uri")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                content_hash: row
                    .get("content_hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                size_bytes: size,
                metadata: row
                    .get("metadata")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: row
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }
        Ok(list)
    }

    pub async fn record_tool_invocation(
        &self,
        task_id: &str,
        tool_name: &str,
        risk_tier: &str,
        approval_id: Option<&str>,
        duration_ms: i64,
        was_success: bool,
        error_message: Option<&str>,
    ) -> Result<TaskToolInvocationRecord> {
        let id = format!("call_{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();

        let sql = "
            INSERT INTO task_tool_invocations (
                id, task_id, tool_name, risk_tier, approval_id,
                duration_ms, was_success, error_message, invoked_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(task_id.to_string()),
                    Value::String(tool_name.to_string()),
                    Value::String(risk_tier.to_string()),
                    approval_id
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::I64(duration_ms),
                    Value::I64(if was_success { 1 } else { 0 }),
                    error_message
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(TaskToolInvocationRecord {
            id,
            task_id: task_id.to_string(),
            tool_name: tool_name.to_string(),
            risk_tier: risk_tier.to_string(),
            approval_id: approval_id.map(|s| s.to_string()),
            duration_ms,
            was_success,
            error_message: error_message.map(|s| s.to_string()),
            invoked_at: now,
        })
    }

    // ── Scoped Long-Term Memory Operations ──

    pub async fn store_scoped_memory(
        &self,
        account_id: &str,
        workspace_id: Option<&str>,
        memory_type: &str,
        content: &str,
        source: &str,
        source_reference: Option<&str>,
        confidence: f64,
        importance: f64,
        provenance: &str,
        sensitivity: &str,
    ) -> Result<ScopedMemoryRecord> {
        let id = format!("mem_{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();

        let sql = "
            INSERT INTO scoped_memories (
                id, account_id, workspace_id, memory_type, content,
                source, source_reference, confidence, importance,
                provenance, sensitivity, status, created_at, updated_at, last_accessed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?)
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(account_id.to_string()),
                    workspace_id
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::String(memory_type.to_string()),
                    Value::String(content.to_string()),
                    Value::String(source.to_string()),
                    source_reference
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::F64(confidence),
                    Value::F64(importance),
                    Value::String(provenance.to_string()),
                    Value::String(sensitivity.to_string()),
                    Value::String(now.clone()),
                    Value::String(now.clone()),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(ScopedMemoryRecord {
            id,
            account_id: account_id.to_string(),
            workspace_id: workspace_id.map(|s| s.to_string()),
            memory_type: memory_type.to_string(),
            content: content.to_string(),
            source: source.to_string(),
            source_reference: source_reference.map(|s| s.to_string()),
            confidence,
            importance,
            provenance: provenance.to_string(),
            sensitivity: sensitivity.to_string(),
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            last_accessed_at: now,
        })
    }

    pub async fn get_scoped_memory(&self, id: &str) -> Result<Option<ScopedMemoryRecord>> {
        let sql = "SELECT * FROM scoped_memories WHERE id = ?";
        let rows = self.db.query(sql, &[Value::String(id.to_string())]).await?;
        if let Some(row) = rows.into_iter().next() {
            let conf = row
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let imp = row
                .get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);

            Ok(Some(ScopedMemoryRecord {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                account_id: row
                    .get("account_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                workspace_id: row
                    .get("workspace_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                memory_type: row
                    .get("memory_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                content: row
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                source: row
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                source_reference: row
                    .get("source_reference")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                confidence: conf,
                importance: imp,
                provenance: row
                    .get("provenance")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                sensitivity: row
                    .get("sensitivity")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                status: row
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: row
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                updated_at: row
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                last_accessed_at: row
                    .get("last_accessed_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn query_scoped_memories(
        &self,
        account_id: &str,
        workspace_id: Option<&str>,
        memory_type: Option<&str>,
        status_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ScopedMemoryRecord>> {
        let mut sql = "SELECT * FROM scoped_memories WHERE account_id = ?".to_string();
        let mut params = vec![Value::String(account_id.to_string())];

        if let Some(ws) = workspace_id {
            sql.push_str(" AND (workspace_id = ? OR workspace_id IS NULL)");
            params.push(Value::String(ws.to_string()));
        }

        if let Some(m_type) = memory_type {
            sql.push_str(" AND memory_type = ?");
            params.push(Value::String(m_type.to_string()));
        }

        if let Some(status) = status_filter {
            sql.push_str(" AND status = ?");
            params.push(Value::String(status.to_string()));
        } else {
            sql.push_str(" AND status = 'active'");
        }

        sql.push_str(" ORDER BY importance DESC, created_at DESC LIMIT ?");
        params.push(Value::I64(limit as i64));

        let rows = self.db.query(&sql, &params).await?;
        let mut list = Vec::new();

        for row in rows {
            let conf = row
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);
            let imp = row
                .get("importance")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.5);

            list.push(ScopedMemoryRecord {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                account_id: row
                    .get("account_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                workspace_id: row
                    .get("workspace_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                memory_type: row
                    .get("memory_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                content: row
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                source: row
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                source_reference: row
                    .get("source_reference")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                confidence: conf,
                importance: imp,
                provenance: row
                    .get("provenance")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                sensitivity: row
                    .get("sensitivity")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                status: row
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: row
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                updated_at: row
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                last_accessed_at: row
                    .get("last_accessed_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            });
        }

        Ok(list)
    }

    pub async fn update_memory_status(&self, id: &str, status: &str) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let sql = "UPDATE scoped_memories SET status = ?, updated_at = ? WHERE id = ?";
        let affected = self
            .db
            .execute(
                sql,
                &[
                    Value::String(status.to_string()),
                    Value::String(now),
                    Value::String(id.to_string()),
                ],
            )
            .await?;
        Ok(affected > 0)
    }

    pub async fn touch_scoped_memory(&self, id: &str) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let sql = "UPDATE scoped_memories SET last_accessed_at = ? WHERE id = ?";
        let affected = self
            .db
            .execute(sql, &[Value::String(now), Value::String(id.to_string())])
            .await?;
        Ok(affected > 0)
    }

    pub async fn delete_scoped_memory(&self, account_id: &str, id: &str) -> Result<bool> {
        let sql = "DELETE FROM scoped_memories WHERE account_id = ? AND id = ?";
        let affected = self
            .db
            .execute(
                sql,
                &[
                    Value::String(account_id.to_string()),
                    Value::String(id.to_string()),
                ],
            )
            .await?;
        Ok(affected > 0)
    }

    pub async fn delete_workspace_memories(
        &self,
        account_id: &str,
        workspace_id: &str,
    ) -> Result<u64> {
        let sql = "DELETE FROM scoped_memories WHERE account_id = ? AND workspace_id = ?";
        self.db
            .execute(
                sql,
                &[
                    Value::String(account_id.to_string()),
                    Value::String(workspace_id.to_string()),
                ],
            )
            .await
    }

    // ── Skills & Workflows Operations ──

    pub async fn register_skill(
        &self,
        name: &str,
        display_name: &str,
        description: &str,
        version: &str,
        publisher_id: &str,
        trust_level: &str,
        manifest_json: &str,
        checksum: &str,
    ) -> Result<SkillRecord> {
        let id = format!("skill_{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();

        let sql = "
            INSERT INTO skills (
                id, name, display_name, description, version,
                publisher_id, trust_level, manifest_json, status,
                checksum, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, ?)
            ON CONFLICT(name, version) DO UPDATE SET
                display_name = excluded.display_name,
                description = excluded.description,
                manifest_json = excluded.manifest_json,
                checksum = excluded.checksum,
                updated_at = excluded.updated_at
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(name.to_string()),
                    Value::String(display_name.to_string()),
                    Value::String(description.to_string()),
                    Value::String(version.to_string()),
                    Value::String(publisher_id.to_string()),
                    Value::String(trust_level.to_string()),
                    Value::String(manifest_json.to_string()),
                    Value::String(checksum.to_string()),
                    Value::String(now.clone()),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(SkillRecord {
            id,
            name: name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            version: version.to_string(),
            publisher_id: publisher_id.to_string(),
            trust_level: trust_level.to_string(),
            manifest_json: manifest_json.to_string(),
            status: "active".to_string(),
            checksum: checksum.to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn get_skill(&self, name: &str, version: &str) -> Result<Option<SkillRecord>> {
        let sql = "SELECT * FROM skills WHERE name = ? AND version = ?";
        let rows = self
            .db
            .query(
                sql,
                &[
                    Value::String(name.to_string()),
                    Value::String(version.to_string()),
                ],
            )
            .await?;

        if let Some(row) = rows.into_iter().next() {
            Ok(Some(SkillRecord {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                name: row
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                display_name: row
                    .get("display_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                description: row
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                version: row
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                publisher_id: row
                    .get("publisher_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                trust_level: row
                    .get("trust_level")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                manifest_json: row
                    .get("manifest_json")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                status: row
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                checksum: row
                    .get("checksum")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                created_at: row
                    .get("created_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                updated_at: row
                    .get("updated_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn create_workflow(
        &self,
        account_id: &str,
        workspace_id: Option<&str>,
        name: &str,
        description: &str,
        version: &str,
        workflow_def: &str,
    ) -> Result<WorkflowRecord> {
        let id = format!("wf_{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();

        let sql = "
            INSERT INTO workflows (
                id, account_id, workspace_id, name, description,
                version, workflow_definition, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 'active', ?, ?)
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(account_id.to_string()),
                    workspace_id
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::String(name.to_string()),
                    Value::String(description.to_string()),
                    Value::String(version.to_string()),
                    Value::String(workflow_def.to_string()),
                    Value::String(now.clone()),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(WorkflowRecord {
            id,
            account_id: account_id.to_string(),
            workspace_id: workspace_id.map(|s| s.to_string()),
            name: name.to_string(),
            description: description.to_string(),
            version: version.to_string(),
            workflow_definition: workflow_def.to_string(),
            status: "active".to_string(),
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub async fn create_workflow_execution(
        &self,
        workflow_id: &str,
        account_id: &str,
        workspace_id: Option<&str>,
        total_steps: i64,
        input_data: &str,
    ) -> Result<WorkflowExecutionRecord> {
        let id = format!("run_{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();

        let sql = "
            INSERT INTO workflow_executions (
                id, workflow_id, account_id, workspace_id, status,
                current_step, total_steps, input_data, started_at
            ) VALUES (?, ?, ?, ?, 'created', 0, ?, ?, ?)
        ";

        self.db
            .execute(
                sql,
                &[
                    Value::String(id.clone()),
                    Value::String(workflow_id.to_string()),
                    Value::String(account_id.to_string()),
                    workspace_id
                        .map(|s| Value::String(s.to_string()))
                        .unwrap_or(Value::Null),
                    Value::I64(total_steps),
                    Value::String(input_data.to_string()),
                    Value::String(now.clone()),
                ],
            )
            .await?;

        Ok(WorkflowExecutionRecord {
            id,
            workflow_id: workflow_id.to_string(),
            account_id: account_id.to_string(),
            workspace_id: workspace_id.map(|s| s.to_string()),
            status: "created".to_string(),
            current_step: 0,
            total_steps,
            input_data: input_data.to_string(),
            output_data: None,
            error_message: None,
            started_at: now,
            ended_at: None,
        })
    }

    pub async fn update_workflow_execution(
        &self,
        execution_id: &str,
        status: &str,
        current_step: i64,
        output_data: Option<&str>,
        error_message: Option<&str>,
        ended: bool,
    ) -> Result<bool> {
        let now = Utc::now().to_rfc3339();
        let mut sql = "UPDATE workflow_executions SET status = ?, current_step = ?".to_string();
        let mut params = vec![Value::String(status.to_string()), Value::I64(current_step)];

        if let Some(out) = output_data {
            sql.push_str(", output_data = ?");
            params.push(Value::String(out.to_string()));
        }

        if let Some(err) = error_message {
            sql.push_str(", error_message = ?");
            params.push(Value::String(err.to_string()));
        }

        if ended {
            sql.push_str(", ended_at = ?");
            params.push(Value::String(now));
        }

        sql.push_str(" WHERE id = ?");
        params.push(Value::String(execution_id.to_string()));

        let affected = self.db.execute(&sql, &params).await?;
        Ok(affected > 0)
    }

    pub async fn get_workflow_execution(
        &self,
        execution_id: &str,
    ) -> Result<Option<WorkflowExecutionRecord>> {
        let sql = "SELECT * FROM workflow_executions WHERE id = ?";
        let rows = self
            .db
            .query(sql, &[Value::String(execution_id.to_string())])
            .await?;

        if let Some(row) = rows.into_iter().next() {
            let cur = row
                .get("current_step")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let tot = row.get("total_steps").and_then(|v| v.as_i64()).unwrap_or(0);

            Ok(Some(WorkflowExecutionRecord {
                id: row
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                workflow_id: row
                    .get("workflow_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                account_id: row
                    .get("account_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                workspace_id: row
                    .get("workspace_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                status: row
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                current_step: cur,
                total_steps: tot,
                input_data: row
                    .get("input_data")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                output_data: row
                    .get("output_data")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                error_message: row
                    .get("error_message")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                started_at: row
                    .get("started_at")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                ended_at: row
                    .get("ended_at")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            }))
        } else {
            Ok(None)
        }
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

        // 7. Task History & Artifacts
        let task = store
            .create_task(
                "session_test_123",
                Some("workspace_voxy"),
                "Refactor auth system",
                "Coding",
                None,
            )
            .await
            .unwrap();
        assert_eq!(task.status, TaskStatus::Queued);

        // Transition: Queued -> Running
        let started = store
            .update_task_status(
                &task.id,
                TaskStatus::Running,
                Some("EXECUTING"),
                Some(0.2),
                None,
                None,
            )
            .await
            .unwrap();
        assert!(started);

        let current = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(current.status, TaskStatus::Running);
        assert!(current.started_at.is_some());

        // Create Artifact
        let art = store
            .create_artifact(
                &task.id,
                Some("workspace_voxy"),
                "patch",
                "auth_patch.diff",
                "crates/auth/src/lib.rs",
                "voxy://artifacts/auth_patch.diff",
                "sha256:abc123mockhash",
                1024,
                "{}",
            )
            .await
            .unwrap();
        assert_eq!(art.name, "auth_patch.diff");

        let artifacts = store.get_artifacts_for_task(&task.id).await.unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].content_hash, "sha256:abc123mockhash");

        // Record tool call
        let tool_inv = store
            .record_tool_invocation(
                &task.id,
                "harness_apply_patch",
                "Modify",
                None,
                150,
                true,
                None,
            )
            .await
            .unwrap();
        assert_eq!(tool_inv.tool_name, "harness_apply_patch");

        // Transition: Running -> Completed
        let finished = store
            .update_task_status(
                &task.id,
                TaskStatus::Completed,
                Some("VERIFIED"),
                Some(1.0),
                None,
                Some("Auth system patched cleanly"),
            )
            .await
            .unwrap();
        assert!(finished);

        let final_task = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(final_task.status, TaskStatus::Completed);
        assert_eq!(final_task.progress, 1.0);
        assert!(final_task.ended_at.is_some());

        // Test illegal transition (Completed -> Running must be rejected)
        let illegal = store
            .update_task_status(&task.id, TaskStatus::Running, None, None, None, None)
            .await;
        assert!(illegal.is_err());

        // Test crash recovery for abandoned running tasks
        let abandoned = store
            .create_task(
                "session_crash",
                None,
                "Abandoned operation",
                "Research",
                None,
            )
            .await
            .unwrap();
        store
            .update_task_status(&abandoned.id, TaskStatus::Running, None, None, None, None)
            .await
            .unwrap();

        let recovered = store.recover_interrupted_tasks().await.unwrap();
        assert!(recovered >= 1);

        let recovered_task = store.get_task(&abandoned.id).await.unwrap().unwrap();
        assert_eq!(recovered_task.status, TaskStatus::Failed);
        assert_eq!(recovered_task.phase, "INTERRUPTED");

        // 9. Scoped Memories Persistence & Boundary Isolation
        let mem1 = store
            .store_scoped_memory(
                "acc_primary",
                Some("ws_osmoo"),
                "Preference",
                "User prefers Rust 2021 edition and strict clippy checks",
                "user_explicit",
                None,
                1.0,
                0.9,
                "{\"source\":\"chat\"}",
                "standard",
            )
            .await
            .unwrap();
        assert_eq!(mem1.account_id, "acc_primary");
        assert_eq!(mem1.workspace_id.as_deref(), Some("ws_osmoo"));

        let mem2 = store
            .store_scoped_memory(
                "acc_secondary",
                Some("ws_other"),
                "Fact",
                "Internal endpoint is 10.0.0.1",
                "task_observation",
                Some("task_123"),
                0.95,
                0.6,
                "{\"source\":\"task_observation\"}",
                "restricted",
            )
            .await
            .unwrap();

        // Verify account isolation: acc_primary cannot see acc_secondary memories
        let acc1_memories = store
            .query_scoped_memories("acc_primary", Some("ws_osmoo"), None, None, 10)
            .await
            .unwrap();
        assert_eq!(acc1_memories.len(), 1);
        assert_eq!(acc1_memories[0].id, mem1.id);

        let acc2_memories = store
            .query_scoped_memories("acc_secondary", Some("ws_other"), None, None, 10)
            .await
            .unwrap();
        assert_eq!(acc2_memories.len(), 1);
        assert_eq!(acc2_memories[0].id, mem2.id);

        // Update status and touch
        assert!(store
            .update_memory_status(&mem1.id, "superseded")
            .await
            .unwrap());
        let re_queried = store
            .query_scoped_memories(
                "acc_primary",
                Some("ws_osmoo"),
                None,
                Some("superseded"),
                10,
            )
            .await
            .unwrap();
        assert_eq!(re_queried.len(), 1);

        // Deletion
        assert!(store
            .delete_scoped_memory("acc_primary", &mem1.id)
            .await
            .unwrap());
        let deleted = store.get_scoped_memory(&mem1.id).await.unwrap();
        assert!(deleted.is_none());

        // 10. Skills & Workflows Ecosystem Persistence
        let skill = store
            .register_skill(
                "repo_doctor",
                "Repository Diagnostics & Doctor",
                "Automated repository check and repair",
                "1.0.0",
                "publisher_osmoo",
                "verified",
                "{\"permissions\":[\"ReadFiles\",\"WriteFiles\"]}",
                "sha256:fakechecksum123",
            )
            .await
            .unwrap();
        assert_eq!(skill.name, "repo_doctor");
        assert_eq!(skill.trust_level, "verified");

        let fetched_skill = store
            .get_skill("repo_doctor", "1.0.0")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched_skill.id, skill.id);

        let workflow = store
            .create_workflow(
                "acc_team1",
                Some("ws_osmoo"),
                "auto_repair_flow",
                "End-to-end diagnostic and patch workflow",
                "1.0.0",
                "{\"steps\":[\"inspect\",\"diagnose\",\"patch\",\"verify\"]}",
            )
            .await
            .unwrap();
        assert_eq!(workflow.account_id, "acc_team1");
        assert_eq!(workflow.name, "auto_repair_flow");

        let run = store
            .create_workflow_execution(
                &workflow.id,
                "acc_team1",
                Some("ws_osmoo"),
                4,
                "{\"target\":\"src/main.rs\"}",
            )
            .await
            .unwrap();
        assert_eq!(run.status, "created");
        assert_eq!(run.total_steps, 4);

        assert!(store
            .update_workflow_execution(&run.id, "running", 2, None, None, false)
            .await
            .unwrap());

        assert!(store
            .update_workflow_execution(
                &run.id,
                "completed",
                4,
                Some("{\"repaired\":true}"),
                None,
                true
            )
            .await
            .unwrap());

        let final_run = store
            .get_workflow_execution(&run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(final_run.status, "completed");
        assert_eq!(final_run.current_step, 4);
        assert!(final_run.ended_at.is_some());
    }
}
