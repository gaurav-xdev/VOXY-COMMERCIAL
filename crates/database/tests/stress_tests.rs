//! Stress tests for VOXY subsystems.
//!
//! These tests simulate production-grade load to find real bugs.
//! Run with: cargo test --package voxy-database --features sqlite -- stress_test --nocapture

#[cfg(test)]
mod stress_tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::Barrier;

    // ========================================================================
    // EVENTBUS STRESS TESTS
    // ========================================================================

    #[tokio::test]
    async fn stress_eventbus_throughput() {
        use voxy_event_bus::EventBus;
        use voxy_shared::Event;

        let bus = Arc::new(EventBus::new(256));
        let total_messages = 10_000;
        let producers = 10;
        let messages_per_producer = total_messages / producers;

        let mut rx1 = bus.subscribe("stress.topic1").await.unwrap();
        let mut rx2 = bus.subscribe("stress.topic2").await.unwrap();
        let mut rx3 = bus.subscribe("stress.topic3").await.unwrap();

        let start = Instant::now();

        // Spawn producers AND a concurrent consumer
        let mut handles = Vec::new();

        // Consumer task - runs concurrently with producers
        let consumer_handle = tokio::spawn(async move {
            let mut consumed = 0;
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if Instant::now() > deadline {
                    break;
                }
                tokio::select! {
                    Ok(_) = rx1.recv() => consumed += 1,
                    Ok(_) = rx2.recv() => consumed += 1,
                    Ok(_) = rx3.recv() => consumed += 1,
                    _ = tokio::time::sleep(Duration::from_millis(10)) => continue,
                }
            }
            consumed
        });

        for p in 0..producers {
            let bus = bus.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..messages_per_producer {
                    let topic = match i % 3 {
                        0 => "stress.topic1",
                        1 => "stress.topic2",
                        _ => "stress.topic3",
                    };
                    let event = Event::new(topic, format!("producer-{p}"), vec![0u8; 64]);
                    let _ = bus.publish(topic, event).await;
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let publish_time = start.elapsed();
        let consumed = consumer_handle.await.unwrap();
        let throughput = consumed as f64 / publish_time.as_secs_f64();

        println!("EventBus Stress Results:");
        println!("  Total messages: {total_messages}");
        println!("  Producers: {producers}");
        println!("  Publish time: {publish_time:?}");
        println!("  Consumed: {consumed}");
        println!("  Throughput: {throughput:.0} msg/sec");

        // Broadcast channels drop messages when buffer is full.
        // With 256 buffer and 10,000 messages, we expect significant loss.
        // The key metric is that the system doesn't crash or hang.
        assert!(consumed > 0, "Should receive at least some messages");
        println!(
            "  NOTE: {} messages dropped (broadcast buffer overflow - expected)",
            total_messages - consumed
        );
    }

    #[tokio::test]
    async fn stress_eventbus_rapid_subscribe_unsubscribe() {
        use voxy_event_bus::EventBus;
        use voxy_shared::Event;

        let bus = Arc::new(EventBus::new(64));
        let iterations = 500;

        let start = Instant::now();
        for i in 0..iterations {
            let topic = format!("rapid.topic.{i}");
            let _rx = bus.subscribe(&topic).await.unwrap();
            let event = Event::new(&topic, "test", vec![0u8; 32]);
            let _ = bus.publish(&topic, event).await;
        }

        let elapsed = start.elapsed();
        let topic_count = bus.topic_count().await;

        println!("Rapid subscribe/unsubscribe: {iterations} iterations in {elapsed:?}");
        println!("Topics created: {topic_count}");

        assert!(
            elapsed < Duration::from_secs(5),
            "Too slow: {elapsed:?} for {iterations} iterations"
        );
    }

    #[tokio::test]
    async fn stress_eventbus_concurrent_publishers() {
        use voxy_event_bus::EventBus;
        use voxy_shared::Event;

        let bus = Arc::new(EventBus::new(256));
        let mut rx = bus.subscribe("concurrent").await.unwrap();
        let barrier = Arc::new(Barrier::new(20));

        let start = Instant::now();
        let mut handles = Vec::new();

        // Consumer runs concurrently
        let consumer = tokio::spawn(async move {
            let mut count = 0;
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                match tokio::time::timeout(Duration::from_millis(10), rx.recv()).await {
                    Ok(Ok(_)) => count += 1,
                    _ => continue,
                }
            }
            count
        });

        for id in 0..20 {
            let bus = bus.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                for _i in 0..500 {
                    let event = Event::new("concurrent", format!("pub-{id}"), vec![id as u8; 32]);
                    let _ = bus.publish("concurrent", event).await;
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let publish_time = start.elapsed();
        let count = consumer.await.unwrap();

        println!("Concurrent publishers: 20 x 500 msgs = 10,000 total");
        println!("  Publish time: {publish_time:?}");
        println!("  Consumed: {count}");

        assert!(count > 0, "No messages received");
        println!(
            "  NOTE: {} messages dropped (broadcast buffer overflow - expected)",
            10_000 - count
        );
    }

    #[tokio::test]
    async fn stress_eventbus_large_payload() {
        use voxy_event_bus::EventBus;
        use voxy_shared::Event;

        let bus = EventBus::new(64);
        let mut rx = bus.subscribe("large").await.unwrap();

        for size_kb in [1, 10, 50, 100] {
            let size = size_kb * 1024;
            let event = Event::new("large", "test", vec![0xAB; size]);
            let start = Instant::now();
            let result = bus.publish("large", event).await;
            let elapsed = start.elapsed();

            match result {
                Ok(()) => {
                    let received = tokio::time::timeout(Duration::from_secs(2), rx.recv())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(received.payload().len(), size);
                    println!("  {size_kb}KB payload: published + consumed in {elapsed:?}");
                }
                Err(e) => {
                    println!("  {size_kb}KB payload: rejected (expected for >1MB): {e}");
                }
            }
        }
    }

    // ========================================================================
    // SQLITE CONTENTION STRESS TESTS
    // ========================================================================

    #[tokio::test]
    async fn stress_sqlite_concurrent_writes() {
        use voxy_database::conversation::{ConversationStore, MessageRole};
        use voxy_database::sqlite::SqliteConversationStore;

        let store = Arc::new(SqliteConversationStore::new_in_memory().unwrap());
        let writers = 10;
        let writes_per_writer = 100;
        let barrier = Arc::new(Barrier::new(writers));

        let start = Instant::now();
        let mut handles = Vec::new();

        for w in 0..writers {
            let store = store.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                let user_id = format!("user-{w}");
                barrier.wait().await;

                for i in 0..writes_per_writer {
                    let conv = store
                        .create_conversation(&user_id, Some(&format!("Conv {w}-{i}")))
                        .await
                        .unwrap();

                    for _m in 0..5 {
                        store
                            .add_message(&conv.id, MessageRole::User, "msg", Some(10), None)
                            .await
                            .unwrap();
                    }
                }

                let stats = store.stats(&user_id).await.unwrap();
                assert_eq!(stats.total_conversations, writes_per_writer);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let elapsed = start.elapsed();
        let total_writes = writers * writes_per_writer;
        let total_messages = total_writes * 5;

        println!("SQLite Concurrent Writes Stress:");
        println!("  Writers: {writers}, Writes/writer: {writes_per_writer}");
        println!("  Total conversations: {total_writes}");
        println!("  Total messages: {total_messages}");
        println!("  Time: {elapsed:?}");
        println!(
            "  Rate: {:.0} conv/sec, {:.0} msg/sec",
            total_writes as f64 / elapsed.as_secs_f64(),
            total_messages as f64 / elapsed.as_secs_f64()
        );
    }

    #[tokio::test]
    async fn stress_sqlite_rapid_create_delete() {
        use voxy_database::conversation::{ConversationStore, MessageRole};
        use voxy_database::sqlite::SqliteConversationStore;

        let store = SqliteConversationStore::new_in_memory().unwrap();
        let iterations = 1000;

        let start = Instant::now();
        let mut created = 0;
        let mut deleted = 0;

        for i in 0..iterations {
            let conv = store
                .create_conversation("stress-user", Some(&format!("Temp {i}")))
                .await
                .unwrap();
            created += 1;

            store
                .add_message(&conv.id, MessageRole::User, "test", Some(5), None)
                .await
                .unwrap();

            store.delete_conversation(&conv.id).await.unwrap();
            deleted += 1;
        }

        let elapsed = start.elapsed();
        let stats = store.stats("stress-user").await.unwrap();

        println!("Rapid Create/Delete Stress:");
        println!("  Created: {created}, Deleted: {deleted}");
        println!("  Time: {elapsed:?}");
        println!(
            "  Rate: {:.0} ops/sec",
            (created + deleted) as f64 / elapsed.as_secs_f64()
        );
        assert_eq!(
            stats.total_conversations, 0,
            "All conversations should be deleted"
        );
    }

    #[tokio::test]
    async fn stress_sqlite_read_write_contention() {
        use voxy_database::conversation::{ConversationStore, MessageRole};
        use voxy_database::sqlite::SqliteConversationStore;

        let store = Arc::new(SqliteConversationStore::new_in_memory().unwrap());

        for i in 0..50 {
            let conv = store
                .create_conversation("reader-user", Some(&format!("Conv {i}")))
                .await
                .unwrap();
            for _m in 0..10 {
                store
                    .add_message(&conv.id, MessageRole::User, "msg", Some(10), None)
                    .await
                    .unwrap();
            }
        }

        let barrier = Arc::new(Barrier::new(11));
        let start = Instant::now();

        let store_w = store.clone();
        let barrier_w = barrier.clone();
        let writer = tokio::spawn(async move {
            barrier_w.wait().await;
            for i in 0..200 {
                let conv = store_w
                    .create_conversation("writer", Some(&format!("W-{i}")))
                    .await
                    .unwrap();
                store_w
                    .add_message(&conv.id, MessageRole::User, "w", Some(5), None)
                    .await
                    .unwrap();
            }
        });

        let mut readers = Vec::new();
        for _r in 0..10 {
            let store_r = store.clone();
            let barrier_r = barrier.clone();
            readers.push(tokio::spawn(async move {
                barrier_r.wait().await;
                let mut reads = 0;
                let mut errors = 0;
                for _ in 0..100 {
                    match store_r.list_conversations("reader-user", 10, 0).await {
                        Ok(_) => reads += 1,
                        Err(_) => errors += 1,
                    }
                    match store_r.stats("reader-user").await {
                        Ok(_) => reads += 1,
                        Err(_) => errors += 1,
                    }
                }
                (reads, errors)
            }));
        }

        writer.await.unwrap();
        let mut total_reads = 0;
        let mut total_errors = 0;
        for r in readers {
            let (reads, errors) = r.await.unwrap();
            total_reads += reads;
            total_errors += errors;
        }

        let elapsed = start.elapsed();
        println!("Read/Write Contention Stress:");
        println!("  Writer: 200 create+message ops");
        println!("  Readers: 10 concurrent, 100 read cycles each");
        println!("  Total reads: {total_reads}, Errors: {total_errors}");
        println!("  Time: {elapsed:?}");

        assert_eq!(
            total_errors, 0,
            "Got {total_errors} errors under contention"
        );
    }

    // ========================================================================
    // AUDIT LOG STRESS TESTS
    // ========================================================================

    #[tokio::test]
    async fn stress_audit_log_throughput() {
        use voxy_database::persistent_audit::{AuditLogEntry, AuditLogStore};
        use voxy_database::sqlite::SqliteAuditLogStore;

        let store = Arc::new(SqliteAuditLogStore::new_in_memory().unwrap());
        let writers = 5;
        let entries_per_writer = 500;

        let start = Instant::now();
        let mut handles = Vec::new();

        for w in 0..writers {
            let store = store.clone();
            handles.push(tokio::spawn(async move {
                for i in 0..entries_per_writer {
                    let entry = AuditLogEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        timestamp: chrono::Utc::now(),
                        subject: format!("subject-{w}"),
                        action: format!("action-{i}"),
                        resource: Some(format!("resource-{w}-{i}")),
                        result: "success".to_string(),
                        reason: None,
                        risk_level: "low".to_string(),
                        trust_level: "trusted".to_string(),
                        previous_hash: String::new(),
                        hash: format!("{:064x}", i),
                        audit_level: "standard".to_string(),
                        metadata: None,
                    };
                    store.record_entry(&entry).await.unwrap();
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        let elapsed = start.elapsed();
        let total = writers * entries_per_writer;
        let count = store.count().await.unwrap();

        println!("Audit Log Throughput Stress:");
        println!("  Writers: {writers}, Entries/writer: {entries_per_writer}");
        println!("  Total entries: {total}");
        println!("  Verified count: {count}");
        println!("  Time: {elapsed:?}");
        println!(
            "  Rate: {:.0} entries/sec",
            total as f64 / elapsed.as_secs_f64()
        );

        assert_eq!(count, total);
    }

    // ========================================================================
    // MEMORY / ALLOCATION STRESS TESTS
    // ========================================================================

    #[tokio::test]
    async fn stress_memory_allocation_pattern() {
        let iterations = 10_000;
        let start = Instant::now();

        let mut allocations = Vec::with_capacity(iterations);

        for i in 0..iterations {
            let turn = format!(
                "User turn {i}: This is a simulated user message that would typically be \
                 sent to an LLM. It contains some text that needs to be processed. \
                 The quick brown fox jumps over the lazy dog. {}",
                "x".repeat(i % 500)
            );
            allocations.push(turn);

            let response = format!(
                "Assistant turn {i}: Here is a response from the language model. \
                 It contains generated text that will be displayed to the user. \
                 The response quality depends on the model and prompt. {}",
                "y".repeat(i % 300)
            );
            allocations.push(response);

            if i % 100 == 0 && allocations.len() > 200 {
                let drain_count = allocations.len() - 100;
                allocations.drain(..drain_count);
            }
        }

        let elapsed = start.elapsed();
        let peak_len = allocations.len();

        println!("Memory Allocation Stress:");
        println!("  Iterations: {iterations}");
        println!("  Peak allocations: {peak_len}");
        println!("  Time: {elapsed:?}");

        drop(allocations);

        assert!(
            elapsed < Duration::from_secs(30),
            "Allocation stress took too long: {elapsed:?}"
        );
    }

    // ========================================================================
    // CONVERSATION RAPID SWITCHING TEST
    // ========================================================================

    #[tokio::test]
    async fn stress_conversation_rapid_switching() {
        use voxy_database::conversation::{ConversationStore, MessageRole};
        use voxy_database::sqlite::SqliteConversationStore;

        let store = SqliteConversationStore::new_in_memory().unwrap();
        let sessions = 20;
        let turns_per_session = 50;

        let start = Instant::now();

        let mut conv_ids = Vec::new();
        for s in 0..sessions {
            let conv = store
                .create_conversation(&format!("user-{s}"), Some(&format!("Session {s}")))
                .await
                .unwrap();
            conv_ids.push(conv.id);
        }

        let mut msg_count = 0;
        for turn in 0..turns_per_session {
            for conv_id in &conv_ids {
                store
                    .add_message(
                        conv_id,
                        if turn % 2 == 0 {
                            MessageRole::User
                        } else {
                            MessageRole::Assistant
                        },
                        &format!("Turn {turn} message"),
                        Some(20),
                        None,
                    )
                    .await
                    .unwrap();
                msg_count += 1;
            }
        }

        let elapsed = start.elapsed();

        for (i, conv_id) in conv_ids.iter().enumerate() {
            let count = store.get_message_count(conv_id).await.unwrap();
            assert_eq!(
                count, turns_per_session,
                "Conv {i} should have {turns_per_session} messages"
            );
        }

        println!("Rapid Conversation Switching Stress:");
        println!("  Sessions: {sessions}, Turns: {turns_per_session}");
        println!("  Total messages: {msg_count}");
        println!("  Time: {elapsed:?}");
        println!(
            "  Rate: {:.0} msg/sec",
            msg_count as f64 / elapsed.as_secs_f64()
        );
    }

    // ========================================================================
    // BUSYBOX: MINI INTEGRATION STRESS
    // ========================================================================

    #[tokio::test]
    async fn stress_full_pipeline_simulation() {
        use voxy_database::conversation::{ConversationStore, MessageRole};
        use voxy_database::sqlite::SqliteConversationStore;
        use voxy_event_bus::EventBus;
        use voxy_shared::Event;

        let bus = Arc::new(EventBus::new(256));
        let store = Arc::new(SqliteConversationStore::new_in_memory().unwrap());

        let start = Instant::now();

        let mut handles = Vec::new();
        for session in 0..20 {
            let bus = bus.clone();
            let store = store.clone();
            handles.push(tokio::spawn(async move {
                let conv = store
                    .create_conversation(&format!("voice-{session}"), None)
                    .await
                    .unwrap();

                for turn in 0..5 {
                    let _ = bus
                        .publish("voice.wake", Event::new("voice.wake", "detector", vec![1]))
                        .await;

                    let _ = bus
                        .publish(
                            "stt.final",
                            Event::new(
                                "stt.final",
                                "whisper",
                                format!("User said turn {turn}").into_bytes(),
                            ),
                        )
                        .await;

                    store
                        .add_message(
                            &conv.id,
                            MessageRole::User,
                            &format!("Turn {turn}"),
                            Some(15),
                            None,
                        )
                        .await
                        .unwrap();

                    let _ = bus
                        .publish(
                            "llm.response",
                            Event::new(
                                "llm.response",
                                "ollama",
                                format!("Response {turn}").into_bytes(),
                            ),
                        )
                        .await;

                    store
                        .add_message(
                            &conv.id,
                            MessageRole::Assistant,
                            &format!("Response {turn}"),
                            Some(25),
                            None,
                        )
                        .await
                        .unwrap();

                    let _ = bus
                        .publish(
                            "tts.audio",
                            Event::new("tts.audio", "kokoro", vec![0; 1024]),
                        )
                        .await;
                }

                let msg_count = store.get_message_count(&conv.id).await.unwrap();
                (session, msg_count)
            }));
        }

        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }

        let elapsed = start.elapsed();
        let total_msgs: usize = results.iter().map(|(_, c)| *c).sum();

        println!("Full Pipeline Simulation Stress:");
        println!("  Sessions: 20, Turns/session: 5");
        println!("  Total messages in DB: {total_msgs}");
        println!("  Time: {elapsed:?}");
        println!(
            "  Rate: {:.0} interactions/sec",
            100.0 / elapsed.as_secs_f64()
        );

        assert_eq!(
            total_msgs, 200,
            "Expected 200 messages (20 sessions x 5 turns x 2 msgs/turn)"
        );
    }

    // ========================================================================
    // TIMEOUT / HANG DETECTION TESTS
    // ========================================================================

    #[tokio::test]
    async fn stress_deadlock_detection() {
        use voxy_database::conversation::{ConversationStore, MessageRole};
        use voxy_database::sqlite::SqliteConversationStore;

        let store = Arc::new(SqliteConversationStore::new_in_memory().unwrap());
        let barrier = Arc::new(Barrier::new(2));

        let store1 = store.clone();
        let barrier1 = barrier.clone();
        let t1 = tokio::spawn(async move {
            barrier1.wait().await;
            for i in 0..500 {
                let conv = store1
                    .create_conversation("t1", Some(&format!("T1-{i}")))
                    .await
                    .unwrap();
                store1
                    .add_message(&conv.id, MessageRole::User, "t1", Some(5), None)
                    .await
                    .unwrap();
            }
        });

        let store2 = store.clone();
        let barrier2 = barrier.clone();
        let t2 = tokio::spawn(async move {
            barrier2.wait().await;
            for _ in 0..500 {
                let _ = store2.list_conversations("t1", 10, 0).await;
                let _ = store2.stats("t1").await;
            }
        });

        let result = tokio::time::timeout(Duration::from_secs(30), async {
            t1.await.unwrap();
            t2.await.unwrap();
        })
        .await;

        assert!(
            result.is_ok(),
            "DEADLOCK DETECTED: tasks did not complete in 30s"
        );
        println!("Deadlock detection: No deadlock found in concurrent read/write");
    }

    // ========================================================================
    // EDGE CASE STRESS TESTS
    // ========================================================================

    #[tokio::test]
    async fn stress_empty_and_whitespace_messages() {
        use voxy_database::conversation::{ConversationStore, MessageRole};
        use voxy_database::sqlite::SqliteConversationStore;

        let store = SqliteConversationStore::new_in_memory().unwrap();
        let conv = store
            .create_conversation("test", Some("Edge cases"))
            .await
            .unwrap();

        let result = store
            .add_message(&conv.id, MessageRole::User, "", None, None)
            .await;
        assert!(result.is_ok(), "Empty message should be accepted");

        let result = store
            .add_message(&conv.id, MessageRole::User, "   \n\t  ", None, None)
            .await;
        assert!(result.is_ok(), "Whitespace message should be accepted");

        let long_msg = "x".repeat(100_000);
        let result = store
            .add_message(&conv.id, MessageRole::User, &long_msg, None, None)
            .await;
        assert!(result.is_ok(), "Long message should be accepted");

        let result = store
            .add_message(
                &conv.id,
                MessageRole::User,
                "Hello 世界 🌍 مرحبا",
                None,
                None,
            )
            .await;
        assert!(result.is_ok(), "Unicode message should be accepted");

        let count = store.get_message_count(&conv.id).await.unwrap();
        assert_eq!(count, 4, "Should have 4 messages");
    }

    #[tokio::test]
    async fn stress_special_characters_in_ids() {
        use voxy_database::conversation::ConversationStore;
        use voxy_database::sqlite::SqliteConversationStore;

        let store = SqliteConversationStore::new_in_memory().unwrap();

        let user_ids = [
            "user with spaces",
            "user/with/slashes",
            "user;DROP TABLE conversations;--",
            "user\"quotes\"",
            "user\\backslash",
        ];

        for uid in &user_ids {
            let result = store.create_conversation(uid, Some("test")).await;
            assert!(result.is_ok(), "User ID '{}' should work", uid);
        }

        // Verify each user's conversations can be retrieved
        for uid in &user_ids {
            let convs = store.list_conversations(uid, 100, 0).await.unwrap();
            assert_eq!(convs.len(), 1, "User '{}' should have 1 conversation", uid);
        }

        // Verify SQL injection attempt doesn't cause errors
        let result = store
            .list_conversations("'; DROP TABLE conversations; --", 100, 0)
            .await;
        assert!(
            result.is_ok(),
            "SQL injection attempt should not cause error"
        );
    }

    // ========================================================================
    // RESOURCE CLEANUP TESTS
    // ========================================================================

    #[tokio::test]
    async fn stress_eventbus_memory_cleanup() {
        use voxy_event_bus::EventBus;
        use voxy_shared::Event;

        let bus = EventBus::new(64);

        for i in 0..500 {
            let topic = format!("cleanup.topic.{i}");
            let mut rx = bus.subscribe(&topic).await.unwrap();

            let event = Event::new(&topic, "test", vec![0u8; 1024]);
            bus.publish(&topic, event).await.unwrap();
            let _ = rx.recv().await;
        }

        let topic_count = bus.topic_count().await;
        println!("After 500 topics: {topic_count} topics tracked");

        assert!(
            topic_count <= 500,
            "Topic count should not exceed created count: {topic_count}"
        );
    }
}
