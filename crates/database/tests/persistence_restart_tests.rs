//! Integration Tests for Milestone M5: Durable SQLite WAL Persistence,
//! Process Restart Recovery, Connection Pooling, and Atomic Transactions.

use std::sync::Arc;
use tempfile::TempDir;

use voxy_database::{
    config::{DatabaseConfig, DatabaseKind},
    conversation::{ConversationStore, MessageRole},
    persistent_audit::{AuditLogEntry, AuditLogStore},
    storage::{StorageProvider, Value},
    SqliteAuditLogStore, SqliteConversationStore, SqliteDatabase,
};

#[tokio::test]
async fn test_sqlite_wal_mode_enforced_and_verified() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("wal_verify.db");
    let path_str = db_path.to_str().unwrap().to_string();

    let db = SqliteDatabase::new();
    let config = DatabaseConfig {
        kind: DatabaseKind::Sqlite,
        path: Some(path_str.clone()),
        pool_size: 4,
        auto_migrate: false,
        ..Default::default()
    };
    db.connect(&config).await.expect("Database connect failed");

    // Must strictly be in WAL journal mode
    assert!(
        db.is_wal_mode().await.expect("query wal mode"),
        "Database must initialize strictly in WAL journal mode"
    );

    // Also verify stores initialize in WAL mode
    let conv_store = SqliteConversationStore::new(&path_str).expect("SqliteConversationStore new");
    assert!(conv_store.is_wal_mode().await.expect("conv store wal mode"));

    let audit_store = SqliteAuditLogStore::new(&path_str).expect("SqliteAuditLogStore new");
    assert!(audit_store
        .is_wal_mode()
        .await
        .expect("audit store wal mode"));
}

#[tokio::test]
async fn test_data_persists_across_simulated_process_restart() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("restart_persistence.db");
    let path_str = db_path.to_str().unwrap().to_string();

    // ── Phase 1: Process 1 initializes schema and populates data ──────────────
    {
        let db = SqliteDatabase::new();
        let config = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(path_str.clone()),
            pool_size: 4,
            auto_migrate: true,
            ..Default::default()
        };
        db.connect(&config).await.expect("Phase 1 connect");
        assert!(db.is_wal_mode().await.expect("Phase 1 wal check"));

        // Store KV pairs
        db.set("session:token:123", b"user_auth_state_payload")
            .await
            .expect("set kv");
        db.set("config:system_mode", b"production")
            .await
            .expect("set config");

        // Populate conversations and messages with FTS5 indexing
        let conv_store = SqliteConversationStore::new(&path_str).expect("Phase 1 conv_store new");
        let conv = conv_store
            .create_conversation("user-persist-42", Some("Persistent Session"))
            .await
            .expect("create conversation");

        conv_store
            .add_message(
                &conv.id,
                MessageRole::User,
                "Persistent query about distributed consensus and raft log replication.",
                Some(12),
                None,
            )
            .await
            .expect("add user message");

        conv_store
            .add_message(
                &conv.id,
                MessageRole::Assistant,
                "Raft ensures durability by writing entries to WAL before acknowledging commit.",
                Some(16),
                None,
            )
            .await
            .expect("add assistant message");

        // Record persistent audit logs
        let audit_store = SqliteAuditLogStore::new(&path_str).expect("Phase 1 audit_store new");
        let audit_entry = AuditLogEntry {
            id: "audit-evt-001".to_string(),
            timestamp: chrono::Utc::now(),
            subject: "user-persist-42".to_string(),
            action: "system.authenticate".to_string(),
            resource: Some("/api/v1/auth".to_string()),
            result: "success".to_string(),
            reason: None,
            risk_level: "low".to_string(),
            trust_level: "verified".to_string(),
            previous_hash: String::new(),
            hash: "genesis_hash_001".to_string(),
            audit_level: "standard".to_string(),
            metadata: None,
        };
        audit_store
            .record_entry(&audit_entry)
            .await
            .expect("record audit entry");

        // Explicitly disconnect and drop references (simulate process exit)
        db.disconnect().await.expect("Phase 1 disconnect");
    }

    // ── Phase 2: Process 2 restarts and re-opens the exact same file ──────────
    {
        let db2 = SqliteDatabase::new();
        let config2 = DatabaseConfig {
            kind: DatabaseKind::Sqlite,
            path: Some(path_str.clone()),
            pool_size: 4,
            auto_migrate: false, // Schema was already migrated by Phase 1
            ..Default::default()
        };
        db2.connect(&config2).await.expect("Phase 2 connect");
        assert!(db2.is_wal_mode().await.expect("Phase 2 wal check"));

        // Verify KV persistence
        let val1 = db2
            .get("session:token:123")
            .await
            .expect("get val1")
            .expect("val1 exists");
        assert_eq!(val1, b"user_auth_state_payload");

        let val2 = db2
            .get("config:system_mode")
            .await
            .expect("get val2")
            .expect("val2 exists");
        assert_eq!(val2, b"production");

        // Verify conversations and messages survived restart
        let conv_store2 = SqliteConversationStore::new(&path_str).expect("Phase 2 conv_store new");
        let convs = conv_store2
            .list_conversations("user-persist-42", 10, 0)
            .await
            .expect("list conversations");
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0].title.as_deref(), Some("Persistent Session"));

        let messages = conv_store2
            .get_messages(&convs[0].id, 10, 0)
            .await
            .expect("get messages");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[1].role, MessageRole::Assistant);

        // Verify FTS5 full-text search works across process restart
        let search_results = conv_store2
            .search_messages("consensus", 10)
            .await
            .expect("search consensus");
        assert_eq!(search_results.len(), 1);
        assert_eq!(search_results[0].id, messages[0].id);

        let raft_results = conv_store2
            .search_conversation_messages(&convs[0].id, "durability", 10)
            .await
            .expect("search durability");
        assert_eq!(raft_results.len(), 1);
        assert_eq!(raft_results[0].id, messages[1].id);

        // Verify audit log survived restart
        let audit_store2 = SqliteAuditLogStore::new(&path_str).expect("Phase 2 audit_store new");
        let entries = audit_store2
            .get_entries(10, 0)
            .await
            .expect("get audit entries");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "audit-evt-001");
        assert_eq!(entries[0].action, "system.authenticate");
        assert_eq!(entries[0].hash, "genesis_hash_001");

        db2.disconnect().await.expect("Phase 2 disconnect");
    }
}

#[tokio::test]
async fn test_concurrent_reader_pool_under_load() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("concurrent_pool.db");
    let path_str = db_path.to_str().unwrap().to_string();

    let db = Arc::new(SqliteDatabase::new());
    let config = DatabaseConfig {
        kind: DatabaseKind::Sqlite,
        path: Some(path_str),
        pool_size: 8,
        auto_migrate: false,
        ..Default::default()
    };
    db.connect(&config).await.expect("connect");

    // Populate baseline keys
    for i in 0..20 {
        let key = format!("concurrent_key_{i}");
        let val = format!("val_{i}").into_bytes();
        db.set(&key, &val).await.expect("populate key");
    }

    // Launch 16 concurrent reader tasks across the 8 reader pool connections
    let mut tasks = Vec::new();
    for reader_id in 0..16 {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            for round in 0..15 {
                let key_idx = (reader_id + round) % 20;
                let key = format!("concurrent_key_{key_idx}");
                let expected = format!("val_{key_idx}").into_bytes();
                let retrieved = db_clone
                    .get(&key)
                    .await
                    .expect("read get")
                    .expect("key exists");
                assert_eq!(retrieved, expected);
            }
        });
        tasks.push(handle);
    }

    for task in tasks {
        task.await.expect("Reader task join");
    }

    db.disconnect().await.expect("disconnect");
}

#[tokio::test]
async fn test_transaction_atomic_rollback_on_failure() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("atomic_tx.db");
    let path_str = db_path.to_str().unwrap().to_string();

    let db = SqliteDatabase::new();
    let config = DatabaseConfig {
        kind: DatabaseKind::Sqlite,
        path: Some(path_str),
        pool_size: 4,
        auto_migrate: false,
        ..Default::default()
    };
    db.connect(&config).await.expect("connect");

    // Successful batch transaction
    let good_batch = vec![
        (
            "INSERT INTO kv_store (key, value) VALUES (?1, ?2)",
            vec![
                Value::String("tx_key_1".into()),
                Value::Bytes(b"val1".to_vec()),
            ],
        ),
        (
            "INSERT INTO kv_store (key, value) VALUES (?1, ?2)",
            vec![
                Value::String("tx_key_2".into()),
                Value::Bytes(b"val2".to_vec()),
            ],
        ),
    ];
    let counts = db
        .execute_batch_transaction(&good_batch)
        .await
        .expect("good batch");
    assert_eq!(counts, vec![1, 1]);
    assert_eq!(db.get("tx_key_1").await.unwrap(), Some(b"val1".to_vec()));
    assert_eq!(db.get("tx_key_2").await.unwrap(), Some(b"val2".to_vec()));

    // Failing batch transaction: contains an invalid query that will trigger failure
    let failing_batch = vec![
        (
            "INSERT INTO kv_store (key, value) VALUES (?1, ?2)",
            vec![
                Value::String("tx_key_3".into()),
                Value::Bytes(b"val3".to_vec()),
            ],
        ),
        (
            "INSERT INTO non_existent_table_xyz (col) VALUES (?1)",
            vec![Value::String("error_val".into())],
        ),
    ];
    let res = db.execute_batch_transaction(&failing_batch).await;
    assert!(res.is_err(), "Transaction with error must fail");

    // tx_key_3 must NOT be present due to atomic rollback
    assert_eq!(
        db.get("tx_key_3").await.unwrap(),
        None,
        "Statement in failed batch must be rolled back atomically"
    );

    db.disconnect().await.expect("disconnect");
}
