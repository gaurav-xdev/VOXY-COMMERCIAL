use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::config::DatabaseKind;
use crate::error::{DatabaseError, Result};

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::I64(n)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::F64(n)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Value::Bytes(bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub score: f64,
    pub metadata: String,
}

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedMigration {
    pub version: i64,
    pub name: String,
    pub applied_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub connected: bool,
    pub latency_ms: u64,
    pub version: Option<String>,
    pub storage_usage_bytes: Option<u64>,
}

#[async_trait]
pub trait StorageProvider: Send + Sync + Debug {
    async fn connect(&self, config: &crate::config::DatabaseConfig) -> Result<()>;
    async fn disconnect(&self) -> Result<()>;
    fn is_connected(&self) -> bool;

    async fn execute(&self, query: &str, params: &[Value]) -> Result<u64>;
    async fn query(&self, query: &str, params: &[Value]) -> Result<Vec<serde_json::Value>>;

    async fn set(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>>;

    async fn store_embedding(&self, id: &str, embedding: &[f32], metadata: &str) -> Result<()>;
    async fn search_similar(&self, embedding: &[f32], limit: usize) -> Result<Vec<SearchResult>>;
    async fn delete_embedding(&self, id: &str) -> Result<()>;

    async fn run_migrations(&self, migrations: &[Migration]) -> Result<()>;
    async fn applied_migrations(&self) -> Result<Vec<AppliedMigration>>;

    async fn backup(&self, path: &str) -> Result<()>;
    async fn restore(&self, path: &str) -> Result<()>;

    async fn health_check(&self) -> Result<HealthStatus>;

    async fn execute_batch_transaction(&self, _queries: &[(&str, Vec<Value>)]) -> Result<Vec<u64>> {
        Err(DatabaseError::UnsupportedOperation(
            "Batch transactions not implemented for this provider".into(),
        ))
    }
}

pub async fn create_storage_provider(kind: DatabaseKind) -> Result<Box<dyn StorageProvider>> {
    match kind {
        DatabaseKind::Sqlite => create_sqlite_provider(),
        DatabaseKind::Postgres => create_postgres_provider(),
        DatabaseKind::DuckDb => create_duckdb_provider(),
        DatabaseKind::Remote => create_remote_provider(),
    }
}

#[cfg(feature = "sqlite")]
fn create_sqlite_provider() -> Result<Box<dyn StorageProvider>> {
    Ok(Box::new(crate::sqlite::SqliteDatabase::new()))
}
#[cfg(not(feature = "sqlite"))]
fn create_sqlite_provider() -> Result<Box<dyn StorageProvider>> {
    Err(DatabaseError::UnsupportedOperation(
        "SQLite support not enabled".into(),
    ))
}

#[cfg(feature = "remote")]
fn create_remote_provider() -> Result<Box<dyn StorageProvider>> {
    Ok(Box::new(crate::remote::RemoteDatabase::default()))
}
#[cfg(not(feature = "remote"))]
fn create_remote_provider() -> Result<Box<dyn StorageProvider>> {
    Err(DatabaseError::UnsupportedOperation(
        "Remote database support not enabled".into(),
    ))
}

fn create_postgres_provider() -> Result<Box<dyn StorageProvider>> {
    Err(DatabaseError::UnsupportedOperation(
        "Postgres support not yet implemented".into(),
    ))
}

fn create_duckdb_provider() -> Result<Box<dyn StorageProvider>> {
    Err(DatabaseError::UnsupportedOperation(
        "DuckDB support not yet implemented".into(),
    ))
}
