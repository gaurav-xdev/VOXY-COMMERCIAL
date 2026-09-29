pub mod backup;
pub mod commercial;
pub mod config;
pub mod conversation;
pub mod error;
pub mod migration;
pub mod persistent_audit;
pub mod query;
pub mod storage;

#[cfg(feature = "encryption")]
pub mod encryption;
#[cfg(feature = "remote")]
pub mod remote;
#[cfg(feature = "sqlite")]
pub mod sqlite;

pub use commercial::{
    get_commercial_migrations, CommercialStore, ConsentRecord, EntitlementRecord, PlanRecord,
    SessionRecord, SubscriptionRecord, UserRecord, WebhookEventRecord,
};

pub use backup::BackupManager;
pub use config::{DatabaseConfig, DatabaseKind};
pub use conversation::{
    Conversation, ConversationStats, ConversationStore, InMemoryConversationStore, Message,
    MessageRole,
};
pub use error::{DatabaseError, Result};
pub use migration::MigrationRunner;
pub use persistent_audit::{AuditLogEntry, AuditLogStore, InMemoryAuditLogStore};
pub use query::QueryBuilder;
pub use storage::{
    create_storage_provider, AppliedMigration, HealthStatus, Migration, SearchResult,
    StorageProvider, Value,
};

#[cfg(feature = "sqlite")]
pub use sqlite::{SqliteAuditLogStore, SqliteConversationStore, SqliteDatabase};
