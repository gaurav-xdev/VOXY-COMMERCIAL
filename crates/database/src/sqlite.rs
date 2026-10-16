use async_trait::async_trait;
use rusqlite::{params, Connection};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::DatabaseConfig;
use crate::error::{DatabaseError, Result};
use crate::storage::{
    AppliedMigration, HealthStatus, Migration, SearchResult, StorageProvider, Value,
};

#[derive(Debug)]
pub struct SqliteDatabase {
    writer: Mutex<Option<Connection>>,
    readers: Mutex<Vec<Connection>>,
    reader_semaphore: parking_lot::RwLock<Arc<tokio::sync::Semaphore>>,
    db_path: parking_lot::RwLock<Option<String>>,
    connected: AtomicBool,
    is_memory: AtomicBool,
    pool_size: AtomicUsize,
}

impl SqliteDatabase {
    pub fn new() -> Self {
        Self {
            writer: Mutex::new(None),
            readers: Mutex::new(Vec::new()),
            reader_semaphore: parking_lot::RwLock::new(Arc::new(tokio::sync::Semaphore::new(0))),
            db_path: parking_lot::RwLock::new(None),
            connected: AtomicBool::new(false),
            is_memory: AtomicBool::new(false),
            pool_size: AtomicUsize::new(4),
        }
    }

    pub fn pool_size(&self) -> usize {
        self.pool_size.load(Ordering::SeqCst)
    }

    pub async fn is_wal_mode(&self) -> Result<bool> {
        self.with_reader(|conn| {
            let mode: String = conn.query_row("PRAGMA journal_mode;", [], |r| r.get(0))?;
            Ok(mode.eq_ignore_ascii_case("wal"))
        })
        .await
    }

    async fn with_reader<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&Connection) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        if !self.is_connected() {
            return Err(DatabaseError::ConnectionFailed("Not connected".into()));
        }

        if self.is_memory.load(Ordering::SeqCst) {
            let guard = self.writer.lock().await;
            let conn = guard
                .as_ref()
                .ok_or_else(|| DatabaseError::ConnectionFailed("Not connected".into()))?;
            return f(conn);
        }

        let sem = self.reader_semaphore.read().clone();
        let permit = sem
            .acquire_owned()
            .await
            .map_err(|e| DatabaseError::ConnectionFailed(format!("Pool semaphore error: {e}")))?;

        let conn = {
            let mut pool = self.readers.lock().await;
            pool.pop()
        };

        let conn = match conn {
            Some(c) => c,
            None => {
                let path_opt = self.db_path.read().clone();
                let path = path_opt.ok_or_else(|| {
                    DatabaseError::ConnectionFailed("Database path is missing".into())
                })?;
                let c = Connection::open(&path)?;
                c.busy_timeout(std::time::Duration::from_millis(5000))?;
                c.execute_batch(
                    "PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA query_only=ON;",
                )?;
                c
            }
        };

        let result = f(&conn);

        {
            let mut pool = self.readers.lock().await;
            pool.push(conn);
        }
        drop(permit);

        result
    }

    async fn with_writer<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Connection) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        if !self.is_connected() {
            return Err(DatabaseError::ConnectionFailed("Not connected".into()));
        }
        let mut guard = self.writer.lock().await;
        let conn = guard
            .as_mut()
            .ok_or_else(|| DatabaseError::ConnectionFailed("Not connected".into()))?;
        f(conn)
    }

    pub async fn transaction<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        self.with_writer(|conn| {
            let tx = conn.transaction()?;
            let res = f(&tx)?;
            tx.commit()?;
            Ok(res)
        })
        .await
    }
}

impl Default for SqliteDatabase {
    fn default() -> Self {
        Self::new()
    }
}

fn to_rusqlite_values(params: &[Value]) -> Vec<rusqlite::types::Value> {
    params
        .iter()
        .map(|v| match v {
            Value::Null => rusqlite::types::Value::Null,
            Value::Bool(b) => rusqlite::types::Value::Integer(if *b { 1 } else { 0 }),
            Value::I64(n) => rusqlite::types::Value::Integer(*n),
            Value::F64(n) => rusqlite::types::Value::Real(*n),
            Value::String(s) => rusqlite::types::Value::Text(s.clone()),
            Value::Bytes(b) => rusqlite::types::Value::Blob(b.clone()),
        })
        .collect()
}

fn rusqlite_value_to_json(val: rusqlite::types::Value) -> serde_json::Value {
    match val {
        rusqlite::types::Value::Null => serde_json::Value::Null,
        rusqlite::types::Value::Integer(i) => serde_json::Value::Number(i.into()),
        rusqlite::types::Value::Real(f) => serde_json::Number::from_f64(f)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        rusqlite::types::Value::Text(s) => serde_json::Value::String(s),
        rusqlite::types::Value::Blob(b) => serde_json::Value::Array(
            b.into_iter()
                .map(|byte| serde_json::Value::Number(byte.into()))
                .collect(),
        ),
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum();
    let norm_b: f32 = b.iter().map(|x| x * x).sum();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot as f64) / ((norm_a as f64).sqrt() * (norm_b as f64).sqrt())
}

#[async_trait]
impl StorageProvider for SqliteDatabase {
    async fn connect(&self, config: &DatabaseConfig) -> Result<()> {
        let path = match config.path.as_deref() {
            Some(p) if !p.trim().is_empty() => p.trim(),
            _ => {
                return Err(DatabaseError::ConnectionFailed(
                    "Database path is required; silent fallback to volatile in-memory storage is prohibited".into(),
                ))
            }
        };

        let is_mem = path == ":memory:";
        self.is_memory.store(is_mem, Ordering::SeqCst);

        let writer_conn = if !is_mem {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let conn = Connection::open(path)?;
            conn.busy_timeout(std::time::Duration::from_millis(5000))?;
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
            )?;
            let jm: String = conn.query_row("PRAGMA journal_mode;", [], |r| r.get(0))?;
            if !jm.eq_ignore_ascii_case("wal") {
                return Err(DatabaseError::ConnectionFailed(format!(
                    "Failed to set WAL journal mode, obtained mode: {jm}"
                )));
            }
            conn
        } else {
            let conn = Connection::open(path)?;
            conn.busy_timeout(std::time::Duration::from_millis(5000))?;
            conn.execute_batch("PRAGMA foreign_keys=ON;")?;
            conn
        };

        writer_conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS embeddings (
                id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                metadata TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            );",
        )?;

        let pool_size = if is_mem {
            1
        } else {
            config.pool_size.max(1) as usize
        };
        let mut readers_vec = Vec::with_capacity(pool_size);
        if !is_mem {
            for _ in 0..pool_size {
                let rconn = Connection::open(path)?;
                rconn.busy_timeout(std::time::Duration::from_millis(5000))?;
                rconn.execute_batch(
                    "PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA query_only=ON;",
                )?;
                readers_vec.push(rconn);
            }
        }

        {
            let mut wguard = self.writer.lock().await;
            *wguard = Some(writer_conn);
        }
        {
            let mut rguard = self.readers.lock().await;
            *rguard = readers_vec;
        }

        *self.reader_semaphore.write() = Arc::new(tokio::sync::Semaphore::new(pool_size));
        self.pool_size.store(pool_size, Ordering::SeqCst);
        *self.db_path.write() = Some(path.to_string());
        self.connected.store(true, Ordering::SeqCst);

        tracing::info!(path = %path, pool_size = %pool_size, is_wal = %(!is_mem), "SQLite database connected");

        if config.auto_migrate {
            let all_migs = crate::migration::get_all_migrations();
            self.run_migrations(&all_migs).await?;
        }

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        {
            let mut wguard = self.writer.lock().await;
            *wguard = None;
        }
        {
            let mut rguard = self.readers.lock().await;
            rguard.clear();
        }
        *self.reader_semaphore.write() = Arc::new(tokio::sync::Semaphore::new(0));
        *self.db_path.write() = None;
        self.connected.store(false, Ordering::SeqCst);
        self.is_memory.store(false, Ordering::SeqCst);
        tracing::info!("SQLite database disconnected");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn execute(&self, query: &str, params: &[Value]) -> Result<u64> {
        let values = to_rusqlite_values(params);
        let q = query.to_string();
        self.with_writer(move |conn| {
            let refs: Vec<&dyn rusqlite::types::ToSql> = values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            let count = conn.execute(&q, refs.as_slice())?;
            Ok(count as u64)
        })
        .await
    }

    async fn query(&self, query: &str, params: &[Value]) -> Result<Vec<serde_json::Value>> {
        let values = to_rusqlite_values(params);
        let q = query.to_string();
        self.with_reader(move |conn| {
            let refs: Vec<&dyn rusqlite::types::ToSql> = values
                .iter()
                .map(|v| v as &dyn rusqlite::types::ToSql)
                .collect();
            let mut stmt = conn.prepare(&q)?;
            let column_count = stmt.column_count();
            let column_names: Vec<String> = (0..column_count)
                .map(|i| stmt.column_name(i).unwrap_or("unknown").to_string())
                .collect();
            let rows = stmt.query_map(refs.as_slice(), |row| {
                let mut map = serde_json::Map::new();
                for (i, name) in column_names.iter().enumerate() {
                    let value: rusqlite::types::Value = row.get(i)?;
                    map.insert(name.clone(), rusqlite_value_to_json(value));
                }
                Ok(serde_json::Value::Object(map))
            })?;
            let mut result = Vec::new();
            for row in rows {
                result.push(row?);
            }
            Ok(result)
        })
        .await
    }

    async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        let k = key.to_string();
        let val = value.to_vec();
        self.with_writer(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?1, ?2)",
                params![k, val],
            )?;
            Ok(())
        })
        .await
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let k = key.to_string();
        self.with_reader(move |conn| {
            let result = conn.query_row(
                "SELECT value FROM kv_store WHERE key = ?1",
                params![k],
                |row| row.get::<_, Vec<u8>>(0),
            );
            match result {
                Ok(value) => Ok(Some(value)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
        .await
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let k = key.to_string();
        self.with_writer(move |conn| {
            conn.execute("DELETE FROM kv_store WHERE key = ?1", params![k])?;
            Ok(())
        })
        .await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let k = key.to_string();
        self.with_reader(move |conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kv_store WHERE key = ?1",
                params![k],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })
        .await
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>> {
        let escaped_prefix = prefix
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("{}%", escaped_prefix);
        self.with_reader(move |conn| {
            let mut stmt =
                conn.prepare("SELECT key FROM kv_store WHERE key LIKE ?1 ESCAPE '\\'")?;
            let keys = stmt
                .query_map(params![pattern], |row| row.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(keys)
        })
        .await
    }

    async fn store_embedding(&self, id: &str, embedding: &[f32], metadata: &str) -> Result<()> {
        let id_s = id.to_string();
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
        let meta = metadata.to_string();
        self.with_writer(move |conn| {
            conn.execute(
                "INSERT OR REPLACE INTO embeddings (id, embedding, metadata) VALUES (?1, ?2, ?3)",
                params![id_s, embedding_bytes, meta],
            )?;
            Ok(())
        })
        .await
    }

    async fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let q_emb = query_embedding.to_vec();
        self.with_reader(move |conn| {
            let mut stmt = conn.prepare("SELECT id, embedding, metadata FROM embeddings")?;
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let blob: Vec<u8> = row.get(1)?;
                let metadata: String = row.get(2)?;
                Ok((id, blob, metadata))
            })?;

            let mut results: Vec<SearchResult> = rows
                .filter_map(|r| r.ok())
                .filter_map(|(id, blob, metadata)| {
                    let embedding: Vec<f32> = blob
                        .chunks_exact(4)
                        .filter_map(|chunk| {
                            let arr: [u8; 4] = chunk.try_into().ok()?;
                            Some(f32::from_le_bytes(arr))
                        })
                        .collect();
                    if embedding.is_empty() {
                        return None;
                    }
                    let score = cosine_similarity(&q_emb, &embedding);
                    Some(SearchResult {
                        id,
                        score,
                        metadata,
                    })
                })
                .collect();

            results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(limit);
            Ok(results)
        })
        .await
    }

    async fn delete_embedding(&self, id: &str) -> Result<()> {
        let id_s = id.to_string();
        self.with_writer(move |conn| {
            conn.execute("DELETE FROM embeddings WHERE id = ?1", params![id_s])?;
            Ok(())
        })
        .await
    }

    async fn run_migrations(&self, migrations: &[Migration]) -> Result<()> {
        let applied = self.applied_migrations().await?;
        let applied_versions: std::collections::HashSet<i64> =
            applied.iter().map(|m| m.version).collect();

        let migs = migrations.to_vec();
        self.with_writer(move |conn| {
            for migration in migs {
                if !applied_versions.contains(&migration.version) {
                    let now = chrono::Utc::now().to_rfc3339();
                    conn.execute("BEGIN", [])
                        .map_err(|e| DatabaseError::MigrationFailed(e.to_string()))?;
                    match conn.execute_batch(migration.sql) {
                        Ok(_) => {
                            conn.execute(
                                "INSERT INTO _migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                                params![migration.version, migration.name, now],
                            )
                            .map_err(|e| DatabaseError::MigrationFailed(e.to_string()))?;
                            conn.execute("COMMIT", [])
                                .map_err(|e| DatabaseError::MigrationFailed(e.to_string()))?;
                        }
                        Err(e) => {
                            conn.execute("ROLLBACK", []).ok();
                            return Err(DatabaseError::MigrationFailed(format!(
                                "Migration {} ('{}') failed: {e}",
                                migration.version, migration.name
                            )));
                        }
                    }
                }
            }
            Ok(())
        })
        .await
    }

    async fn applied_migrations(&self) -> Result<Vec<AppliedMigration>> {
        self.with_reader(|conn| {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS _migrations (
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                )",
                [],
            )?;

            let mut stmt =
                conn.prepare("SELECT version, name, applied_at FROM _migrations ORDER BY version")?;
            let rows = stmt.query_map([], |row| {
                let version: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                let applied_at_str: String = row.get(2)?;
                let applied_at = applied_at_str
                    .parse::<chrono::DateTime<chrono::Utc>>()
                    .unwrap_or(chrono::DateTime::UNIX_EPOCH);
                Ok(AppliedMigration {
                    version,
                    name,
                    applied_at,
                })
            })?;
            let mut result = Vec::new();
            for row in rows {
                result.push(row?);
            }
            Ok(result)
        })
        .await
    }

    async fn execute_batch_transaction(&self, queries: &[(&str, Vec<Value>)]) -> Result<Vec<u64>> {
        let items: Vec<(String, Vec<Value>)> = queries
            .iter()
            .map(|(q, params)| (q.to_string(), params.clone()))
            .collect();

        self.with_writer(move |conn| {
            let tx = conn.transaction()?;
            let mut counts = Vec::with_capacity(items.len());
            for (sql, params) in items {
                let values = to_rusqlite_values(&params);
                let refs: Vec<&dyn rusqlite::types::ToSql> = values
                    .iter()
                    .map(|v| v as &dyn rusqlite::types::ToSql)
                    .collect();
                let count = tx.execute(&sql, refs.as_slice())?;
                counts.push(count as u64);
            }
            tx.commit()?;
            Ok(counts)
        })
        .await
    }

    async fn backup(&self, path: &str) -> Result<()> {
        let p = path.to_string();
        self.with_writer(move |conn| {
            let mut dst = Connection::open(&p)?;
            let backup = rusqlite::backup::Backup::new(conn, &mut dst)?;
            backup.run_to_completion(100, std::time::Duration::from_millis(250), None)?;
            Ok(())
        })
        .await
    }

    async fn restore(&self, path: &str) -> Result<()> {
        let p = path.to_string();
        self.with_writer(move |conn| {
            let src = Connection::open(&p)?;
            let backup = rusqlite::backup::Backup::new(&src, conn)?;
            backup.run_to_completion(100, std::time::Duration::from_millis(250), None)?;
            Ok(())
        })
        .await
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        let start = std::time::Instant::now();
        if !self.is_connected() {
            return Ok(HealthStatus {
                connected: false,
                latency_ms: 0,
                version: None,
                storage_usage_bytes: None,
            });
        }
        self.with_reader(move |conn| {
            let version: String =
                conn.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
            let latency = start.elapsed().as_millis() as u64;
            let size: Option<u64> = conn
                .query_row(
                    "SELECT page_count * page_size FROM pragma_page_count, pragma_page_size",
                    [],
                    |row| row.get(0),
                )
                .ok();
            Ok(HealthStatus {
                connected: true,
                latency_ms: latency,
                version: Some(version),
                storage_usage_bytes: size,
            })
        })
        .await
    }
}

// ============================================================================
// SQLite-backed ConversationStore + AuditLogStore
// ============================================================================

use crate::conversation::{
    Conversation, ConversationStats, ConversationStore, Message, MessageRole,
};
use crate::persistent_audit::{AuditLogEntry, AuditLogStore};

type StdResult<T, E> = std::result::Result<T, E>;

fn parse_dt(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn role_to_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::System => "system",
    }
}

const CONV_COLUMNS: &str = "id, user_id, title, created_at, updated_at, is_active, metadata";
const MSG_COLUMNS: &str = "id, conversation_id, role, content, timestamp, token_count, metadata";
const AUDIT_COLUMNS: &str = "id, timestamp, subject, action, resource, result, reason, risk_level, trust_level, previous_hash, hash, audit_level, metadata";

fn row_to_conversation(row: &rusqlite::Row) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        user_id: row.get(1)?,
        title: row.get(2)?,
        created_at: parse_dt(&row.get::<_, String>(3)?),
        updated_at: parse_dt(&row.get::<_, String>(4)?),
        is_active: row.get::<_, i64>(5)? != 0,
        metadata: row.get(6)?,
    })
}

fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<Message> {
    let role_str: String = row.get(2)?;
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        role: role_str.parse().unwrap_or(MessageRole::User),
        content: row.get(3)?,
        timestamp: parse_dt(&row.get::<_, String>(4)?),
        token_count: row.get(5)?,
        metadata: row.get(6)?,
    })
}

fn row_to_audit_entry(row: &rusqlite::Row) -> rusqlite::Result<AuditLogEntry> {
    Ok(AuditLogEntry {
        id: row.get(0)?,
        timestamp: parse_dt(&row.get::<_, String>(1)?),
        subject: row.get(2)?,
        action: row.get(3)?,
        resource: row.get(4)?,
        result: row.get(5)?,
        reason: row.get(6)?,
        risk_level: row.get(7)?,
        trust_level: row.get(8)?,
        previous_hash: row.get(9)?,
        hash: row.get(10)?,
        audit_level: row.get(11)?,
        metadata: row.get(12)?,
    })
}

// ============================================================================
// SqliteConversationStore
// ============================================================================

pub struct SqliteConversationStore {
    writer: Arc<Mutex<rusqlite::Connection>>,
    readers: Arc<Mutex<Vec<rusqlite::Connection>>>,
    reader_semaphore: Arc<tokio::sync::Semaphore>,
    path: String,
    is_memory: bool,
}

impl SqliteConversationStore {
    pub fn new(path: &str) -> StdResult<Self, String> {
        if path.trim().is_empty() {
            return Err(
                "Database path is required; silent fallback to in-memory storage is prohibited"
                    .to_string(),
            );
        }

        let is_mem = path == ":memory:";
        if !is_mem {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
        }

        let conn =
            rusqlite::Connection::open(path).map_err(|e| format!("Failed to open SQLite: {e}"))?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))
            .map_err(|e| format!("Failed to set busy timeout: {e}"))?;

        if !is_mem {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
            )
            .map_err(|e| format!("Failed to set WAL mode: {e}"))?;
            let jm: String = conn
                .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
                .map_err(|e| format!("Failed to read journal mode: {e}"))?;
            if !jm.eq_ignore_ascii_case("wal") {
                return Err(format!("Expected WAL journal mode, obtained {jm}"));
            }
        } else {
            conn.execute_batch("PRAGMA foreign_keys=ON;")
                .map_err(|e| format!("Failed to set foreign keys: {e}"))?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                title TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                metadata TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_conv_user ON conversations(user_id);
            CREATE INDEX IF NOT EXISTS idx_conv_updated ON conversations(updated_at);

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                token_count INTEGER,
                metadata TEXT,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_msg_conv ON messages(conversation_id);
            CREATE INDEX IF NOT EXISTS idx_msg_ts ON messages(timestamp);

            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                content,
                conversation_id UNINDEXED,
                message_id UNINDEXED
            );

            CREATE TRIGGER IF NOT EXISTS trg_messages_fts_insert AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(content, conversation_id, message_id)
                VALUES (new.content, new.conversation_id, new.id);
            END;

            CREATE TRIGGER IF NOT EXISTS trg_messages_fts_delete AFTER DELETE ON messages BEGIN
                DELETE FROM messages_fts WHERE message_id = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS trg_messages_fts_update AFTER UPDATE ON messages BEGIN
                DELETE FROM messages_fts WHERE message_id = old.id;
                INSERT INTO messages_fts(content, conversation_id, message_id)
                VALUES(new.content, new.conversation_id, new.id);
            END;",
        )
        .map_err(|e| format!("Failed to create tables: {e}"))?;

        let pool_size = if is_mem { 1 } else { 4 };
        let mut readers_vec = Vec::with_capacity(pool_size);
        if !is_mem {
            for _ in 0..pool_size {
                let rconn =
                    rusqlite::Connection::open(path).map_err(|e| format!("Reader open: {e}"))?;
                rconn
                    .busy_timeout(std::time::Duration::from_millis(5000))
                    .map_err(|e| format!("Reader timeout: {e}"))?;
                rconn
                    .execute_batch(
                        "PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA query_only=ON;",
                    )
                    .map_err(|e| format!("Reader pragmas: {e}"))?;
                readers_vec.push(rconn);
            }
        }

        Ok(Self {
            writer: Arc::new(Mutex::new(conn)),
            readers: Arc::new(Mutex::new(readers_vec)),
            reader_semaphore: Arc::new(tokio::sync::Semaphore::new(pool_size)),
            path: path.to_string(),
            is_memory: is_mem,
        })
    }

    pub fn new_in_memory() -> StdResult<Self, String> {
        Self::new(":memory:")
    }

    async fn with_reader<F, R>(&self, f: F) -> StdResult<R, String>
    where
        F: FnOnce(&rusqlite::Connection) -> StdResult<R, String> + Send + 'static,
        R: Send + 'static,
    {
        if self.is_memory {
            let guard = self.writer.lock().await;
            return f(&guard);
        }

        let permit = self
            .reader_semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| format!("Reader pool acquire failed: {e}"))?;

        let conn = {
            let mut pool = self.readers.lock().await;
            pool.pop()
        };

        let conn = match conn {
            Some(c) => c,
            None => {
                let c = rusqlite::Connection::open(&self.path)
                    .map_err(|e| format!("Open reader: {e}"))?;
                c.busy_timeout(std::time::Duration::from_millis(5000))
                    .map_err(|e| format!("Timeout: {e}"))?;
                c.execute_batch(
                    "PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON; PRAGMA query_only=ON;",
                )
                .map_err(|e| format!("Pragmas: {e}"))?;
                c
            }
        };

        let result = f(&conn);

        {
            let mut pool = self.readers.lock().await;
            pool.push(conn);
        }
        drop(permit);

        result
    }

    pub async fn is_wal_mode(&self) -> StdResult<bool, String> {
        self.with_reader(|conn| {
            let mode: String = conn
                .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
                .map_err(|e| format!("Query mode: {e}"))?;
            Ok(mode.eq_ignore_ascii_case("wal"))
        })
        .await
    }
}

#[async_trait::async_trait]
impl ConversationStore for SqliteConversationStore {
    async fn create_conversation(
        &self,
        user_id: &str,
        title: Option<&str>,
    ) -> StdResult<Conversation, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.writer.lock().await;
        conn.execute(
            "INSERT INTO conversations (id, user_id, title, created_at, updated_at, is_active) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            rusqlite::params![id, user_id, title, now, now],
        ).map_err(|e| format!("Insert conversation: {e}"))?;
        Ok(Conversation {
            id,
            user_id: user_id.to_string(),
            title: title.map(|t| t.to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            is_active: true,
            metadata: None,
        })
    }

    async fn get_conversation(
        &self,
        conversation_id: &str,
    ) -> StdResult<Option<Conversation>, String> {
        let cid = conversation_id.to_string();
        self.with_reader(move |conn| {
            let sql = format!("SELECT {CONV_COLUMNS} FROM conversations WHERE id = ?1");
            let mut stmt = conn.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
            let mut rows = stmt
                .query_map(rusqlite::params![cid], row_to_conversation)
                .map_err(|e| format!("Query: {e}"))?;
            match rows.next() {
                Some(Ok(c)) => Ok(Some(c)),
                Some(Err(e)) => Err(format!("Row: {e}")),
                None => Ok(None),
            }
        })
        .await
    }

    async fn list_conversations(
        &self,
        user_id: &str,
        limit: usize,
        offset: usize,
    ) -> StdResult<Vec<Conversation>, String> {
        let uid = user_id.to_string();
        self.with_reader(move |conn| {
            let sql = format!("SELECT {CONV_COLUMNS} FROM conversations WHERE user_id = ?1 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3");
            let mut stmt = conn.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
            let rows = stmt
                .query_map(
                    rusqlite::params![uid, limit as i64, offset as i64],
                    row_to_conversation,
                )
                .map_err(|e| format!("Query: {e}"))?;
            rows.collect::<StdResult<Vec<_>, _>>()
                .map_err(|e| format!("Row: {e}"))
        })
        .await
    }

    async fn delete_conversation(&self, conversation_id: &str) -> StdResult<(), String> {
        let conn = self.writer.lock().await;
        conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            rusqlite::params![conversation_id],
        )
        .map_err(|e| format!("Delete messages: {e}"))?;
        conn.execute(
            "DELETE FROM conversations WHERE id = ?1",
            rusqlite::params![conversation_id],
        )
        .map_err(|e| format!("Delete conversation: {e}"))?;
        Ok(())
    }

    async fn update_conversation_title(
        &self,
        conversation_id: &str,
        title: &str,
    ) -> StdResult<(), String> {
        let conn = self.writer.lock().await;
        let now = chrono::Utc::now().to_rfc3339();
        let affected = conn
            .execute(
                "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![title, now, conversation_id],
            )
            .map_err(|e| format!("Update: {e}"))?;
        if affected == 0 {
            Err("Conversation not found".to_string())
        } else {
            Ok(())
        }
    }

    async fn add_message(
        &self,
        conversation_id: &str,
        role: MessageRole,
        content: &str,
        token_count: Option<i64>,
        metadata: Option<&str>,
    ) -> StdResult<Message, String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.writer.lock().await;
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, token_count, metadata) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, conversation_id, role_to_str(&role), content, now, token_count, metadata],
        ).map_err(|e| format!("Insert message: {e}"))?;
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, conversation_id],
        )
        .map_err(|e| format!("Update conv timestamp: {e}"))?;
        Ok(Message {
            id,
            conversation_id: conversation_id.to_string(),
            role,
            content: content.to_string(),
            timestamp: chrono::Utc::now(),
            token_count,
            metadata: metadata.map(|m| m.to_string()),
        })
    }

    async fn get_messages(
        &self,
        conversation_id: &str,
        limit: usize,
        offset: usize,
    ) -> StdResult<Vec<Message>, String> {
        let cid = conversation_id.to_string();
        self.with_reader(move |conn| {
            let sql = format!("SELECT {MSG_COLUMNS} FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC LIMIT ?2 OFFSET ?3");
            let mut stmt = conn.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
            let rows = stmt
                .query_map(
                    rusqlite::params![cid, limit as i64, offset as i64],
                    row_to_message,
                )
                .map_err(|e| format!("Query: {e}"))?;
            rows.collect::<StdResult<Vec<_>, _>>()
                .map_err(|e| format!("Row: {e}"))
        })
        .await
    }

    async fn get_message_count(&self, conversation_id: &str) -> StdResult<usize, String> {
        let cid = conversation_id.to_string();
        self.with_reader(move |conn| {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                    rusqlite::params![cid],
                    |row| row.get(0),
                )
                .map_err(|e| format!("Count: {e}"))?;
            Ok(count as usize)
        })
        .await
    }

    async fn delete_message(&self, message_id: &str) -> StdResult<(), String> {
        let conn = self.writer.lock().await;
        conn.execute(
            "DELETE FROM messages WHERE id = ?1",
            rusqlite::params![message_id],
        )
        .map_err(|e| format!("Delete: {e}"))?;
        Ok(())
    }

    async fn stats(&self, user_id: &str) -> StdResult<ConversationStats, String> {
        let uid = user_id.to_string();
        self.with_reader(move |conn| {
            let total_conversations: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM conversations WHERE user_id = ?1",
                    rusqlite::params![uid],
                    |r| r.get(0),
                )
                .map_err(|e| format!("Stats conv: {e}"))?;
            let active_conversations: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM conversations WHERE user_id = ?1 AND is_active = 1",
                    rusqlite::params![uid],
                    |r| r.get(0),
                )
                .map_err(|e| format!("Stats active: {e}"))?;
            let total_messages: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages m JOIN conversations c ON m.conversation_id = c.id WHERE c.user_id = ?1",
                rusqlite::params![uid], |r| r.get(0),
            ).map_err(|e| format!("Stats msg: {e}"))?;
            Ok(ConversationStats {
                total_conversations: total_conversations as usize,
                total_messages: total_messages as usize,
                active_conversations: active_conversations as usize,
            })
        })
        .await
    }

    async fn search_messages(&self, query: &str, limit: usize) -> StdResult<Vec<Message>, String> {
        let q = query.to_string();
        self.with_reader(move |conn| {
            let clean_q = q.replace('"', "\"\"");
            let match_pattern = format!("\"{}\"", clean_q);
            let sql = "SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, m.token_count, m.metadata
                 FROM messages m
                 JOIN messages_fts fts ON m.rowid = fts.rowid
                 WHERE messages_fts MATCH ?1
                 ORDER BY fts.rank
                 LIMIT ?2";
            let mut stmt = conn.prepare(sql).map_err(|e| format!("Prepare FTS: {e}"))?;
            let rows = stmt
                .query_map(
                    rusqlite::params![match_pattern, limit as i64],
                    row_to_message,
                )
                .map_err(|e| format!("Query FTS: {e}"))?;
            rows.collect::<StdResult<Vec<_>, _>>()
                .map_err(|e| format!("Row FTS: {e}"))
        })
        .await
    }

    async fn search_conversation_messages(
        &self,
        conversation_id: &str,
        query: &str,
        limit: usize,
    ) -> StdResult<Vec<Message>, String> {
        let q = query.to_string();
        let cid = conversation_id.to_string();
        self.with_reader(move |conn| {
            let clean_q = q.replace('"', "\"\"");
            let match_pattern = format!("\"{}\"", clean_q);
            let sql = "SELECT m.id, m.conversation_id, m.role, m.content, m.timestamp, m.token_count, m.metadata
                 FROM messages m
                 JOIN messages_fts fts ON m.rowid = fts.rowid
                 WHERE m.conversation_id = ?1 AND messages_fts MATCH ?2
                 ORDER BY fts.rank
                 LIMIT ?3";
            let mut stmt = conn.prepare(sql).map_err(|e| format!("Prepare FTS: {e}"))?;
            let rows = stmt
                .query_map(
                    rusqlite::params![cid, match_pattern, limit as i64],
                    row_to_message,
                )
                .map_err(|e| format!("Query FTS: {e}"))?;
            rows.collect::<StdResult<Vec<_>, _>>()
                .map_err(|e| format!("Row FTS: {e}"))
        })
        .await
    }
}

// ============================================================================
// SqliteAuditLogStore
// ============================================================================

pub struct SqliteAuditLogStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl SqliteAuditLogStore {
    pub fn new(path: &str) -> StdResult<Self, String> {
        if path.trim().is_empty() {
            return Err(
                "Database path is required; silent fallback to in-memory storage is prohibited"
                    .to_string(),
            );
        }

        let is_mem = path == ":memory:";
        if !is_mem {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
        }

        let conn =
            rusqlite::Connection::open(path).map_err(|e| format!("Failed to open SQLite: {e}"))?;
        conn.busy_timeout(std::time::Duration::from_millis(5000))
            .map_err(|e| format!("Failed to set busy timeout: {e}"))?;

        if !is_mem {
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
            )
            .map_err(|e| format!("Failed to set WAL mode: {e}"))?;
            let jm: String = conn
                .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
                .map_err(|e| format!("Failed to check journal mode: {e}"))?;
            if !jm.eq_ignore_ascii_case("wal") {
                return Err(format!("Expected WAL journal mode, obtained {jm}"));
            }
        } else {
            conn.execute_batch("PRAGMA foreign_keys=ON;")
                .map_err(|e| format!("Failed to set foreign keys: {e}"))?;
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                subject TEXT NOT NULL,
                action TEXT NOT NULL,
                resource TEXT,
                result TEXT NOT NULL,
                reason TEXT,
                risk_level TEXT NOT NULL,
                trust_level TEXT NOT NULL,
                previous_hash TEXT NOT NULL,
                hash TEXT NOT NULL,
                audit_level TEXT NOT NULL,
                metadata TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_audit_subject ON audit_log(subject);
            CREATE INDEX IF NOT EXISTS idx_audit_action ON audit_log(action);
            CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit_log(timestamp);",
        )
        .map_err(|e| format!("Failed to create audit_log table: {e}"))?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn new_in_memory() -> StdResult<Self, String> {
        Self::new(":memory:")
    }

    pub async fn is_wal_mode(&self) -> StdResult<bool, String> {
        let conn = self.conn.lock().await;
        let mode: String = conn
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .map_err(|e| format!("Query mode: {e}"))?;
        Ok(mode.eq_ignore_ascii_case("wal"))
    }
}

#[async_trait::async_trait]
impl AuditLogStore for SqliteAuditLogStore {
    async fn record_entry(&self, entry: &AuditLogEntry) -> StdResult<(), String> {
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO audit_log (id, timestamp, subject, action, resource, result, reason, risk_level, trust_level, previous_hash, hash, audit_level, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                entry.id, entry.timestamp.to_rfc3339(), entry.subject, entry.action,
                entry.resource, entry.result, entry.reason, entry.risk_level,
                entry.trust_level, entry.previous_hash, entry.hash, entry.audit_level,
                entry.metadata,
            ],
        ).map_err(|e| format!("Insert audit entry: {e}"))?;
        Ok(())
    }

    async fn get_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> StdResult<Vec<AuditLogEntry>, String> {
        let conn = self.conn.lock().await;
        let sql = format!(
            "SELECT {AUDIT_COLUMNS} FROM audit_log ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
        let rows = stmt
            .query_map(
                rusqlite::params![limit as i64, offset as i64],
                row_to_audit_entry,
            )
            .map_err(|e| format!("Query: {e}"))?;
        rows.collect::<StdResult<Vec<_>, _>>()
            .map_err(|e| format!("Row: {e}"))
    }

    async fn get_entries_by_subject(
        &self,
        subject: &str,
        limit: usize,
    ) -> StdResult<Vec<AuditLogEntry>, String> {
        let conn = self.conn.lock().await;
        let sql = format!("SELECT {AUDIT_COLUMNS} FROM audit_log WHERE subject = ?1 ORDER BY timestamp DESC LIMIT ?2");
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![subject, limit as i64], row_to_audit_entry)
            .map_err(|e| format!("Query: {e}"))?;
        rows.collect::<StdResult<Vec<_>, _>>()
            .map_err(|e| format!("Row: {e}"))
    }

    async fn get_entries_by_action(
        &self,
        action: &str,
        limit: usize,
    ) -> StdResult<Vec<AuditLogEntry>, String> {
        let conn = self.conn.lock().await;
        let sql = format!("SELECT {AUDIT_COLUMNS} FROM audit_log WHERE action = ?1 ORDER BY timestamp DESC LIMIT ?2");
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
        let rows = stmt
            .query_map(rusqlite::params![action, limit as i64], row_to_audit_entry)
            .map_err(|e| format!("Query: {e}"))?;
        rows.collect::<StdResult<Vec<_>, _>>()
            .map_err(|e| format!("Row: {e}"))
    }

    async fn get_entries_in_range(
        &self,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> StdResult<Vec<AuditLogEntry>, String> {
        let conn = self.conn.lock().await;
        let sql = format!("SELECT {AUDIT_COLUMNS} FROM audit_log WHERE timestamp >= ?1 AND timestamp <= ?2 ORDER BY timestamp DESC");
        let mut stmt = conn.prepare(&sql).map_err(|e| format!("Prepare: {e}"))?;
        let rows = stmt
            .query_map(
                rusqlite::params![from.to_rfc3339(), to.to_rfc3339()],
                row_to_audit_entry,
            )
            .map_err(|e| format!("Query: {e}"))?;
        rows.collect::<StdResult<Vec<_>, _>>()
            .map_err(|e| format!("Row: {e}"))
    }

    async fn verify_chain(&self) -> StdResult<bool, String> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT previous_hash, hash FROM audit_log ORDER BY timestamp ASC")
            .map_err(|e| format!("Prepare: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| format!("Query: {e}"))?;
        let mut prev_hash = String::new();
        for row in rows {
            let (entry_prev_hash, entry_hash) = row.map_err(|e| format!("Row: {e}"))?;
            if entry_prev_hash != prev_hash {
                return Ok(false);
            }
            prev_hash = entry_hash;
        }
        Ok(true)
    }

    async fn count(&self) -> StdResult<usize, String> {
        let conn = self.conn.lock().await;
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_log", [], |r| r.get(0))
            .map_err(|e| format!("Count: {e}"))?;
        Ok(count as usize)
    }

    async fn clear(&self) -> StdResult<(), String> {
        let conn = self.conn.lock().await;
        conn.execute("DELETE FROM audit_log", [])
            .map_err(|e| format!("Clear: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DatabaseConfig, DatabaseKind};
    use crate::conversation::{ConversationStore, MessageRole};
    use crate::storage::Value;

    #[tokio::test]
    async fn test_connect_and_disconnect() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        assert!(!db.is_connected());
        db.connect(&config).await.unwrap();
        assert!(db.is_connected());
        db.disconnect().await.unwrap();
        assert!(!db.is_connected());
    }

    #[tokio::test]
    async fn test_kv_set_get() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.set("name", b"Alice").await.unwrap();
        let val = db.get("name").await.unwrap();
        assert_eq!(val, Some(b"Alice".to_vec()));
    }

    #[tokio::test]
    async fn test_kv_get_not_found() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let val = db.get("nonexistent").await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn test_kv_delete() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.set("temp", b"value").await.unwrap();
        assert!(db.exists("temp").await.unwrap());
        db.delete("temp").await.unwrap();
        assert!(!db.exists("temp").await.unwrap());
    }

    #[tokio::test]
    async fn test_kv_exists() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        assert!(!db.exists("mykey").await.unwrap());
        db.set("mykey", b"val").await.unwrap();
        assert!(db.exists("mykey").await.unwrap());
    }

    #[tokio::test]
    async fn test_kv_overwrite() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.set("key", b"old").await.unwrap();
        db.set("key", b"new").await.unwrap();
        let val = db.get("key").await.unwrap();
        assert_eq!(val, Some(b"new".to_vec()));
    }

    #[tokio::test]
    async fn test_kv_list_keys() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.set("a:one", b"1").await.unwrap();
        db.set("a:two", b"2").await.unwrap();
        db.set("b:one", b"3").await.unwrap();

        let all = db.list_keys("").await.unwrap();
        assert_eq!(all.len(), 3);

        let a_keys = db.list_keys("a:").await.unwrap();
        assert_eq!(a_keys.len(), 2);
        assert!(a_keys.contains(&"a:one".to_string()));
        assert!(a_keys.contains(&"a:two".to_string()));

        let b_keys = db.list_keys("b:").await.unwrap();
        assert_eq!(b_keys.len(), 1);
    }

    #[tokio::test]
    async fn test_kv_list_keys_prefix_no_match() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();
        db.set("x", b"1").await.unwrap();
        let keys = db.list_keys("z:").await.unwrap();
        assert!(keys.is_empty());
    }

    #[tokio::test]
    async fn test_execute_insert_select() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.execute("CREATE TABLE test (id INTEGER, val TEXT)", &[])
            .await
            .unwrap();
        let affected = db
            .execute(
                "INSERT INTO test (id, val) VALUES (?1, ?2)",
                &[Value::I64(1), Value::String("hello".into())],
            )
            .await
            .unwrap();
        assert_eq!(affected, 1);

        let rows = db.query("SELECT * FROM test", &[]).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], serde_json::json!(1));
        assert_eq!(rows[0]["val"], serde_json::json!("hello"));
    }

    #[tokio::test]
    async fn test_execute_affected_rows() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.execute("CREATE TABLE t (id INTEGER)", &[])
            .await
            .unwrap();
        db.execute("INSERT INTO t VALUES (1)", &[]).await.unwrap();
        db.execute("INSERT INTO t VALUES (2)", &[]).await.unwrap();
        let affected = db
            .execute("UPDATE t SET id = 3 WHERE id = 1", &[])
            .await
            .unwrap();
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_query_multiple_rows() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();
        db.execute("CREATE TABLE t (x INTEGER)", &[]).await.unwrap();
        db.execute("INSERT INTO t VALUES (10)", &[]).await.unwrap();
        db.execute("INSERT INTO t VALUES (20)", &[]).await.unwrap();
        db.execute("INSERT INTO t VALUES (30)", &[]).await.unwrap();

        let rows = db.query("SELECT * FROM t ORDER BY x", &[]).await.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["x"], serde_json::json!(10));
        assert_eq!(rows[2]["x"], serde_json::json!(30));
    }

    #[tokio::test]
    async fn test_query_with_params() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();
        db.execute("CREATE TABLE t (id INTEGER, name TEXT)", &[])
            .await
            .unwrap();
        db.execute("INSERT INTO t VALUES (1, 'Alice')", &[])
            .await
            .unwrap();
        db.execute("INSERT INTO t VALUES (2, 'Bob')", &[])
            .await
            .unwrap();

        let rows = db
            .query("SELECT * FROM t WHERE id = ?1", &[Value::I64(2)])
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], serde_json::json!("Bob"));
    }

    #[tokio::test]
    async fn test_embedding_store_search() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let emb1: Vec<f32> = vec![1.0, 0.0, 0.0];
        let emb2: Vec<f32> = vec![0.0, 1.0, 0.0];
        let emb3: Vec<f32> = vec![0.9, 0.1, 0.0];

        db.store_embedding("a", &emb1, r#"{"label":"x-axis"}"#)
            .await
            .unwrap();
        db.store_embedding("b", &emb2, r#"{"label":"y-axis"}"#)
            .await
            .unwrap();
        db.store_embedding("c", &emb3, r#"{"label":"near-x"}"#)
            .await
            .unwrap();

        let query: Vec<f32> = vec![1.0, 0.0, 0.0];
        let results = db.search_similar(&query, 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "a");
        assert!((results[0].score - 1.0).abs() < 0.001);
        assert_eq!(results[1].id, "c");
    }

    #[tokio::test]
    async fn test_embedding_search_empty() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let query: Vec<f32> = vec![1.0, 0.0, 0.0];
        let results = db.search_similar(&query, 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_embedding_search_limit() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        for i in 0..10 {
            let emb: Vec<f32> = vec![i as f32, 0.0, 0.0];
            db.store_embedding(&format!("k{}", i), &emb, "")
                .await
                .unwrap();
        }

        let query: Vec<f32> = vec![1.0, 0.0, 0.0];
        let results = db.search_similar(&query, 3).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_embedding_delete() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.store_embedding("del_me", &[1.0, 0.0], "{}")
            .await
            .unwrap();
        let results = db.search_similar(&[1.0, 0.0], 10).await.unwrap();
        assert_eq!(results.len(), 1);

        db.delete_embedding("del_me").await.unwrap();
        let results = db.search_similar(&[1.0, 0.0], 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_migration_run() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            auto_migrate: false,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let migrations = vec![Migration {
            version: 1,
            name: "create_users",
            sql: "CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY, name TEXT)",
        }];

        db.run_migrations(&migrations).await.unwrap();
        let applied = db.applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].version, 1);
        assert_eq!(applied[0].name, "create_users");
    }

    #[tokio::test]
    async fn test_migration_applied() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            auto_migrate: false,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let applied_before = db.applied_migrations().await.unwrap();
        assert!(applied_before.is_empty());
    }

    #[tokio::test]
    async fn test_migration_idempotent() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            auto_migrate: false,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let migrations = vec![Migration {
            version: 1,
            name: "test_mig",
            sql: "CREATE TABLE IF NOT EXISTS test (id INTEGER)",
        }];

        db.run_migrations(&migrations).await.unwrap();
        db.run_migrations(&migrations).await.unwrap(); // second run

        let applied = db.applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 1);
    }

    #[tokio::test]
    async fn test_migration_multiple() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            auto_migrate: false,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let migrations = vec![
            Migration {
                version: 1,
                name: "v1",
                sql: "CREATE TABLE IF NOT EXISTS t1 (id INTEGER)",
            },
            Migration {
                version: 2,
                name: "v2",
                sql: "CREATE TABLE IF NOT EXISTS t2 (id INTEGER)",
            },
        ];

        db.run_migrations(&migrations).await.unwrap();
        let applied = db.applied_migrations().await.unwrap();
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[0].version, 1);
        assert_eq!(applied[1].version, 2);
    }

    #[tokio::test]
    async fn test_backup_restore() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.set("bk_key", b"bk_value").await.unwrap();

        let tmp_dir = tempfile::tempdir().unwrap();
        let backup_path = tmp_dir.path().join("backup.db");

        db.backup(backup_path.to_str().unwrap()).await.unwrap();
        assert!(backup_path.exists());

        db.delete("bk_key").await.unwrap();
        assert!(!db.exists("bk_key").await.unwrap());

        db.restore(backup_path.to_str().unwrap()).await.unwrap();
        let val = db.get("bk_key").await.unwrap();
        assert_eq!(val, Some(b"bk_value".to_vec()));
    }

    #[tokio::test]
    async fn test_backup_file_created() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        db.set("x", b"y").await.unwrap();
        let tmp_dir = tempfile::tempdir().unwrap();
        let path = tmp_dir.path().join("test.db");

        db.backup(path.to_str().unwrap()).await.unwrap();
        assert!(path.exists());
        assert!(path.metadata().unwrap().len() > 0);
    }

    #[tokio::test]
    async fn test_health_check() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let health = db.health_check().await.unwrap();
        assert!(health.connected);
        assert!(health.version.is_some());
        assert!(health.version.unwrap().contains("3"));
    }

    #[tokio::test]
    async fn test_health_check_disconnected() {
        let db = SqliteDatabase::new();
        let health = db.health_check().await.unwrap();
        assert!(!health.connected);
        assert_eq!(health.latency_ms, 0);
    }

    #[tokio::test]
    async fn test_disconnect_operations_fail() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();
        db.disconnect().await.unwrap();

        let result = db.get("any").await;
        assert!(result.is_err());
        match result {
            Err(DatabaseError::ConnectionFailed(_)) => {}
            _ => panic!("Expected ConnectionFailed error"),
        }
    }

    #[tokio::test]
    async fn test_large_values() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let large_val: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        db.set("large", &large_val).await.unwrap();
        let retrieved = db.get("large").await.unwrap().unwrap();
        assert_eq!(retrieved.len(), 10_000);
        assert_eq!(retrieved, large_val);
    }

    #[test]
    fn test_fts5_support() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE test_fts USING fts5(content, title);
             INSERT INTO test_fts (content, title) VALUES ('hello world', 'greeting');",
        )
        .unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT count(*) FROM test_fts WHERE test_fts MATCH 'hello'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_auto_migrate_applies_all_migrations() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            auto_migrate: true,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let applied = db.applied_migrations().await.unwrap();
        assert!(applied.len() >= 10);
        assert!(applied
            .iter()
            .any(|m| m.version == 10 && m.name == "core_conversations_and_messages"));
        assert!(applied
            .iter()
            .any(|m| m.version == 11 && m.name == "core_messages_fts5_indexing"));
        assert!(applied
            .iter()
            .any(|m| m.version == 12 && m.name == "core_audit_log"));
    }

    #[tokio::test]
    async fn test_empty_or_whitespace_path_rejected() {
        let db = SqliteDatabase::new();
        let config_none = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: None,
            ..Default::default()
        };
        assert!(db.connect(&config_none).await.is_err());

        let config_empty = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some("   ".to_string()),
            ..Default::default()
        };
        assert!(db.connect(&config_empty).await.is_err());

        assert!(SqliteConversationStore::new("").is_err());
        assert!(SqliteConversationStore::new("   ").is_err());

        assert!(SqliteAuditLogStore::new("").is_err());
        assert!(SqliteAuditLogStore::new("   ").is_err());
    }

    #[tokio::test]
    async fn test_wal_mode_on_disk() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let db_path = tmp_dir.path().join("wal_test.db");
        let path_str = db_path.to_str().unwrap().to_string();

        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(path_str.clone()),
            pool_size: 4,
            auto_migrate: false,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        assert!(db.is_wal_mode().await.unwrap());

        let conv_store = SqliteConversationStore::new(&path_str).unwrap();
        assert!(conv_store.is_wal_mode().await.unwrap());

        let audit_store = SqliteAuditLogStore::new(&path_str).unwrap();
        assert!(audit_store.is_wal_mode().await.unwrap());
    }

    #[tokio::test]
    async fn test_execute_batch_transaction_atomic_rollback() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            auto_migrate: false,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        // 1. Successful batch transaction
        let ops = vec![
            (
                "INSERT INTO kv_store (key, value) VALUES (?1, ?2)",
                vec![Value::String("k1".into()), Value::Bytes(b"v1".to_vec())],
            ),
            (
                "INSERT INTO kv_store (key, value) VALUES (?1, ?2)",
                vec![Value::String("k2".into()), Value::Bytes(b"v2".to_vec())],
            ),
        ];
        let counts = db.execute_batch_transaction(&ops).await.unwrap();
        assert_eq!(counts, vec![1, 1]);
        assert_eq!(db.get("k1").await.unwrap(), Some(b"v1".to_vec()));
        assert_eq!(db.get("k2").await.unwrap(), Some(b"v2".to_vec()));

        // 2. Failing batch transaction should roll back completely
        let failing_ops = vec![
            (
                "INSERT INTO kv_store (key, value) VALUES (?1, ?2)",
                vec![Value::String("k3".into()), Value::Bytes(b"v3".to_vec())],
            ),
            (
                "INSERT INTO non_existent_table (x) VALUES (?1)",
                vec![Value::String("fail".into())],
            ),
        ];
        let err = db.execute_batch_transaction(&failing_ops).await;
        assert!(err.is_err());
        assert_eq!(db.get("k3").await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_conversation_store_fts5_search_and_triggers() {
        let store = SqliteConversationStore::new(":memory:").unwrap();

        let conv = store
            .create_conversation("user-1", Some("Rust SQLite Discussion"))
            .await
            .unwrap();

        let msg1 = store
            .add_message(
                &conv.id,
                MessageRole::User,
                "How does SQLite WAL mode improve concurrency?",
                None,
                None,
            )
            .await
            .unwrap();
        let msg2 = store
            .add_message(
                &conv.id,
                MessageRole::Assistant,
                "WAL mode separates readers and writers using a write-ahead log.",
                None,
                None,
            )
            .await
            .unwrap();

        // Search across all conversations
        let results = store.search_messages("concurrency", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, msg1.id);

        let results_wal = store.search_messages("write-ahead", 10).await.unwrap();
        assert_eq!(results_wal.len(), 1);
        assert_eq!(results_wal[0].id, msg2.id);

        // Search scoped to conversation
        let conv_results = store
            .search_conversation_messages(&conv.id, "readers", 10)
            .await
            .unwrap();
        assert_eq!(conv_results.len(), 1);
        assert_eq!(conv_results[0].id, msg2.id);

        // Verify delete removes message and triggers delete from FTS
        store.delete_message(&msg2.id).await.unwrap();
        let results_after_del = store.search_messages("write-ahead", 10).await.unwrap();
        assert_eq!(results_after_del.len(), 0);
    }
}
