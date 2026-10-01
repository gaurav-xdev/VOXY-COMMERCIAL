//! Stress tests — simulated extreme load conditions.

#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use voxy_event_bus::EventBus;
#[cfg(test)]
use voxy_integration::event_bridge::{EventBridge, Topics};
#[cfg(test)]
use voxy_memory::ltm_v2::ImportanceFactors;
#[cfg(test)]
use voxy_memory::ltm_v2::MemoryId as LtmMemoryId;

#[tokio::test]
async fn stress_event_bus_10k_publishes() {
    let bus = Arc::new(EventBus::new(256));
    let bridge = EventBridge::new(bus.clone());

    let mut rx = bridge.subscribe(Topics::SYSTEM_HEALTH).await.unwrap();

    let counter = Arc::new(AtomicU64::new(0));
    let counter_clone = counter.clone();

    let consumer = tokio::spawn(async move {
        while let Ok(_event) = rx.recv().await {
            counter_clone.fetch_add(1, Ordering::Relaxed);
        }
    });

    let start = std::time::Instant::now();

    for i in 0..10_000u64 {
        bridge
            .publish(
                Topics::SYSTEM_HEALTH,
                "stress_test",
                &format!("{{\"seq\":{}}}", i),
            )
            .await
            .unwrap();
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let elapsed = start.elapsed();
    let received = counter.load(Ordering::Relaxed);

    drop(bridge);
    drop(bus);
    let _ = consumer.await;

    let throughput = received as f64 / elapsed.as_secs_f64();

    assert!(
        throughput > 5000.0,
        "Event bus throughput too low: {:.0} events/sec",
        throughput
    );
    assert!(received >= 9000, "Too many events lost: {}/10000", received);
}

#[tokio::test]
async fn stress_event_bus_concurrent_publishers() {
    let bus = Arc::new(EventBus::new(256));
    let bridge = Arc::new(EventBridge::new(bus));

    let counter = Arc::new(AtomicU64::new(0));
    let mut handles = Vec::new();

    for publisher_id in 0..10u64 {
        let bridge = bridge.clone();
        let counter = counter.clone();
        handles.push(tokio::spawn(async move {
            for i in 0..1000u64 {
                let topic = match i % 4 {
                    0 => Topics::VOICE_WAKE,
                    1 => Topics::STT_FINAL,
                    2 => Topics::LLM_RESPONSE,
                    _ => Topics::TASK_COMPLETED,
                };
                let _ = bridge
                    .publish(
                        topic,
                        &format!("pub_{}", publisher_id),
                        &format!("{{\"pub\":{},\"seq\":{}}}", publisher_id, i),
                    )
                    .await;
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let total = counter.load(Ordering::Relaxed);
    assert_eq!(total, 10_000);
}

#[tokio::test]
async fn stress_telemetry_1k_subsystems() {
    let telemetry = voxy_integration::telemetry::CentralTelemetry::new();

    let start = std::time::Instant::now();

    for i in 0..1000 {
        telemetry.report(voxy_integration::telemetry::SubsystemMetrics {
            name: format!("subsystem_{}", i),
            latency_ms: (i as f64) * 0.1,
            error_count: i % 100,
            warning_count: i % 50,
            cpu_percent: (i as f32) % 100.0,
            memory_mb: i * 10,
            queue_size: (i % 200) as u32,
            events_per_sec: i as f64 * 0.5,
            uptime_seconds: i,
            last_error: None,
            timestamp: chrono::Utc::now(),
        });
    }

    let report_time = start.elapsed();
    let snapshot = telemetry.snapshot();

    assert_eq!(snapshot.subsystems.len(), 1000);
    assert!(
        report_time.as_millis() < 500,
        "Reporting 1000 subsystems took {:?}",
        report_time
    );
}

#[tokio::test]
async fn stress_memory_1k_operations() {
    let mut mem = voxy_memory::LongTermMemoryV2::default_memory();

    let start = std::time::Instant::now();

    for i in 0..1000 {
        let item = voxy_memory::MemoryItemV2 {
            id: LtmMemoryId::new(),
            category: match i % 6 {
                0 => voxy_memory::MemoryCategory::Project,
                1 => voxy_memory::MemoryCategory::UserPreference,
                2 => voxy_memory::MemoryCategory::Relationship,
                3 => voxy_memory::MemoryCategory::Episodic,
                4 => voxy_memory::MemoryCategory::Semantic,
                _ => voxy_memory::MemoryCategory::Procedural,
            },
            content: format!("Memory item {}", i),
            summary: None,
            tags: vec![format!("tag_{}", i % 10)],
            importance: ImportanceFactors {
                recency: 0.5,
                frequency: 0.3,
                relevance: 0.7,
                ..Default::default()
            },
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
            version: 1,
            compressed: false,
            archived: false,
            project_id: Some("stress_test".to_string()),
            related_memory_ids: Vec::new(),
            metadata: std::collections::HashMap::new(),
        };
        mem.store(item);
    }

    let elapsed = start.elapsed();
    assert_eq!(mem.count(), 1000);

    let ops_per_sec = 1000.0 / elapsed.as_secs_f64();
    assert!(
        ops_per_sec > 100.0,
        "Memory ops too slow: {:.0}/sec",
        ops_per_sec
    );
}

#[cfg(not(debug_assertions))]
#[tokio::test]
async fn stress_memory_query_performance() {
    let mut mem = voxy_memory::LongTermMemoryV2::new(10000, 5000, 500, 0.1);

    for i in 0..5000 {
        let item = voxy_memory::MemoryItemV2 {
            id: LtmMemoryId::new(),
            category: voxy_memory::MemoryCategory::Semantic,
            content: format!("Item {} with keyword rust performance", i),
            summary: None,
            tags: vec!["performance".to_string()],
            importance: ImportanceFactors {
                recency: (i as f64) / 5000.0,
                relevance: 0.8,
                ..Default::default()
            },
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: i as u64,
            version: 1,
            compressed: false,
            archived: false,
            project_id: None,
            related_memory_ids: Vec::new(),
            metadata: std::collections::HashMap::new(),
        };
        mem.store(item);
    }

    let start = std::time::Instant::now();

    for _ in 0..100 {
        let query = voxy_memory::MemoryQueryV2 {
            text: Some("rust".to_string()),
            min_importance: Some(0.3),
            max_results: 10,
            ..Default::default()
        };
        let _result = mem.query(&query);
    }

    let elapsed = start.elapsed();
    let queries_per_sec = 100.0 / elapsed.as_secs_f64();
    assert!(
        queries_per_sec > 50.0,
        "Memory queries too slow: {:.0}/sec",
        queries_per_sec
    );
}

#[tokio::test]
async fn stress_goal_engine_500_goals() {
    let mut engine = voxy_cognitive_orchestrator::goal_engine_v2::GoalEngineV2::new(1000, 100);

    let start = std::time::Instant::now();

    let mut goal_ids = Vec::new();
    for i in 0..500 {
        let id = engine
            .create_goal(
                format!("Goal {}", i),
                format!("Description for goal {}", i),
                voxy_cognitive_orchestrator::goal_engine_v2::GoalPriority::Medium,
                None,
            )
            .unwrap();
        goal_ids.push(id);
    }

    for id in &goal_ids[..250] {
        engine.update_progress(id, 1.0).unwrap();
    }

    let elapsed = start.elapsed();
    assert_eq!(engine.all().len(), 500);

    let ops_per_sec = 500.0 / elapsed.as_secs_f64();
    assert!(
        ops_per_sec > 100.0,
        "Goal engine too slow: {:.0}/sec",
        ops_per_sec
    );
}

#[tokio::test]
async fn stress_task_graph_200_tasks() {
    use voxy_planner::task_graph::*;

    let start = std::time::Instant::now();

    let mut builder = TaskGraphBuilder::new("stress_graph", "stress test graph");
    builder = builder.task("task_0", "desc", TaskType::Code);
    for i in 1..200 {
        builder = builder.then(&format!("task_{}", i), "desc", TaskType::Code);
    }

    let graph = builder.build().unwrap();
    let build_time = start.elapsed();

    let layers = graph.topological_layers().unwrap();
    let _sort_time = start.elapsed() - build_time;

    let critical_path = graph.critical_path().unwrap();

    assert_eq!(graph.node_count(), 200);
    assert!(!layers.is_empty());
    assert!(!critical_path.is_empty());
}
