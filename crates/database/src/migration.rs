use crate::error::Result;
use crate::storage::{Migration, StorageProvider};

#[derive(Debug)]
pub struct MigrationRunner {
    migrations: Vec<Migration>,
}

impl MigrationRunner {
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    pub fn register(&mut self, migration: Migration) {
        self.migrations.push(migration);
    }

    pub fn register_all(&mut self, migrations: Vec<Migration>) {
        self.migrations.extend(migrations);
    }

    pub fn migrations(&self) -> &[Migration] {
        &self.migrations
    }

    pub async fn run(&self, provider: &dyn StorageProvider) -> Result<()> {
        provider.run_migrations(&self.migrations).await
    }

    pub async fn status(&self, provider: &dyn StorageProvider) -> Result<MigrationStatus> {
        let applied = provider.applied_migrations().await?;
        let applied_versions: std::collections::HashSet<i64> =
            applied.iter().map(|m| m.version).collect();

        let mut pending = Vec::new();
        let mut completed = Vec::new();

        for migration in &self.migrations {
            if applied_versions.contains(&migration.version) {
                completed.push(migration.version);
            } else {
                pending.push(migration.version);
            }
        }

        Ok(MigrationStatus {
            total: self.migrations.len(),
            applied: completed.len(),
            pending: pending.len(),
            pending_versions: pending,
            applied_versions: completed,
        })
    }
}

impl Default for MigrationRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MigrationStatus {
    pub total: usize,
    pub applied: usize,
    pub pending: usize,
    pub pending_versions: Vec<i64>,
    pub applied_versions: Vec<i64>,
}

/// Core schema migrations ensuring conversation indexing, full-text search (FTS5),
/// and persistent audit logging.
pub fn get_core_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 10,
            name: "core_conversations_and_messages",
            sql: "
                CREATE TABLE IF NOT EXISTS conversations (
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
            ",
        },
        Migration {
            version: 11,
            name: "core_messages_fts5_indexing",
            sql: "
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
                END;
            ",
        },
        Migration {
            version: 12,
            name: "core_audit_log",
            sql: "
                CREATE TABLE IF NOT EXISTS audit_log (
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
                CREATE INDEX IF NOT EXISTS idx_audit_ts ON audit_log(timestamp);
            ",
        },
    ]
}

/// Returns all default system migrations (core migrations + commercial migrations).
pub fn get_all_migrations() -> Vec<Migration> {
    let mut migrations = get_core_migrations();
    migrations.extend(crate::commercial::get_commercial_migrations());
    migrations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{DatabaseConfig, DatabaseKind};
    use crate::sqlite::SqliteDatabase;

    #[tokio::test]
    async fn test_register_migration() {
        let mut runner = MigrationRunner::new();
        assert_eq!(runner.migrations().len(), 0);

        runner.register(Migration {
            version: 1,
            name: "test",
            sql: "CREATE TABLE IF NOT EXISTS t (id INTEGER)",
        });
        assert_eq!(runner.migrations().len(), 1);
    }

    #[tokio::test]
    async fn test_register_all() {
        let mut runner = MigrationRunner::new();
        runner.register_all(vec![
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
        ]);
        assert_eq!(runner.migrations().len(), 2);
    }

    #[tokio::test]
    async fn test_migration_status() {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(":memory:".into()),
            auto_migrate: false,
            ..Default::default()
        };
        db.connect(&config).await.unwrap();

        let mut runner = MigrationRunner::new();
        runner.register(Migration {
            version: 1,
            name: "v1",
            sql: "CREATE TABLE IF NOT EXISTS t (id INTEGER)",
        });
        runner.register(Migration {
            version: 2,
            name: "v2",
            sql: "CREATE TABLE IF NOT EXISTS u (id INTEGER)",
        });

        let status = runner.status(&db).await.unwrap();
        assert_eq!(status.total, 2);
        assert_eq!(status.applied, 0);
        assert_eq!(status.pending, 2);

        runner.run(&db).await.unwrap();

        let status = runner.status(&db).await.unwrap();
        assert_eq!(status.total, 2);
        assert_eq!(status.applied, 2);
        assert_eq!(status.pending, 0);
    }
}
