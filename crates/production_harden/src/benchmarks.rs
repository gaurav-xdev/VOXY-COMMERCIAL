//! Benchmarks — measurable performance baselines.

#[cfg(test)]
use voxy_memory::ltm_v2::ImportanceFactors;
#[cfg(test)]
use voxy_memory::ltm_v2::MemoryId as LtmMemoryId;

#[tokio::test]
async fn bench_event_bus_latency() {
    use std::sync::Arc;
    use voxy_event_bus::EventBus;
    use voxy_integration::event_bridge::{EventBridge, Topics};

    let bus = Arc::new(EventBus::new(256));
    let bridge = EventBridge::new(bus);

    let mut rx = bridge.subscribe(Topics::SYSTEM_HEALTH).await.unwrap();

    let mut latencies = Vec::with_capacity(1000);

    for i in 0..1000 {
        let start = std::time::Instant::now();
        bridge
            .publish(
                Topics::SYSTEM_HEALTH,
                "bench",
                &format!("{{\"seq\":{}}}", i),
            )
            .await
            .unwrap();
        let _event = rx.recv().await.unwrap();
        latencies.push(start.elapsed().as_nanos() as f64);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = latencies[latencies.len() * 99 / 100];

    assert!(p99 < 500_000.0, "Event bus P99 too high: {:.0}ns", p99);
}

#[tokio::test]
async fn bench_telemetry_report_latency() {
    use voxy_integration::telemetry::{CentralTelemetry, SubsystemMetrics};

    let telemetry = CentralTelemetry::new();

    for i in 0..100 {
        telemetry.report(SubsystemMetrics::new(format!("svc_{}", i)));
    }

    let mut latencies = Vec::with_capacity(10000);

    for i in 0..10_000 {
        let start = std::time::Instant::now();
        telemetry.report(SubsystemMetrics {
            name: format!("svc_{}", i % 100),
            latency_ms: i as f64 * 0.01,
            ..SubsystemMetrics::new(format!("svc_{}", i % 100))
        });
        latencies.push(start.elapsed().as_nanos() as f64);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = latencies[latencies.len() * 99 / 100];

    assert!(p99 < 50_000.0, "Telemetry P99 too high: {:.0}ns", p99);
}

#[tokio::test]
async fn bench_memory_store_latency() {
    let mut mem = voxy_memory::LongTermMemoryV2::new(10000, 5000, 500, 0.1);

    let mut latencies = Vec::with_capacity(1000);

    for i in 0..1000 {
        let item = voxy_memory::MemoryItemV2 {
            id: LtmMemoryId::new(),
            category: voxy_memory::MemoryCategory::Semantic,
            content: format!("Benchmark item {}", i),
            summary: None,
            tags: vec!["bench".to_string()],
            importance: ImportanceFactors {
                recency: 0.5,
                relevance: 0.8,
                ..Default::default()
            },
            created_at: chrono::Utc::now(),
            last_accessed: chrono::Utc::now(),
            access_count: 0,
            version: 1,
            compressed: false,
            archived: false,
            project_id: None,
            related_memory_ids: Vec::new(),
            metadata: std::collections::HashMap::new(),
        };

        let start = std::time::Instant::now();
        mem.store(item);
        latencies.push(start.elapsed().as_nanos() as f64);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = latencies[latencies.len() * 99 / 100];
    let threshold = if cfg!(debug_assertions) {
        500_000.0
    } else {
        100_000.0
    };
    assert!(p99 < threshold, "Memory store P99 too high: {:.0}ns", p99);
}

#[cfg(not(debug_assertions))]
#[tokio::test]
async fn bench_goal_engine_latency() {
    use voxy_cognitive_orchestrator::goal_engine_v2::{GoalEngineV2, GoalPriority};

    let mut engine = GoalEngineV2::new(1000, 100);

    let mut create_latencies = Vec::with_capacity(500);
    let mut update_latencies = Vec::with_capacity(500);
    let mut ids = Vec::new();

    for i in 0..500 {
        let start = std::time::Instant::now();
        let id = engine
            .create_goal(
                format!("Bench Goal {}", i),
                "desc".to_string(),
                GoalPriority::Medium,
                None,
            )
            .unwrap();
        create_latencies.push(start.elapsed().as_nanos() as f64);
        ids.push(id);
    }

    for id in &ids {
        let start = std::time::Instant::now();
        engine.update_progress(id, 0.5).unwrap();
        update_latencies.push(start.elapsed().as_nanos() as f64);
    }

    create_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    update_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let create_p99 = create_latencies[create_latencies.len() * 99 / 100];
    let update_p99 = update_latencies[update_latencies.len() * 99 / 100];

    assert!(
        create_p99 < 200_000.0,
        "Goal create P99 too high: {:.0}ns",
        create_p99
    );
    assert!(
        update_p99 < 200_000.0,
        "Goal update P99 too high: {:.0}ns",
        update_p99
    );
}

#[cfg(not(debug_assertions))]
#[tokio::test]
async fn bench_task_graph_latency() {
    use voxy_planner::task_graph::*;

    let mut build_latencies = Vec::with_capacity(100);
    let mut sort_latencies = Vec::with_capacity(100);

    for trial in 0..100 {
        let start = std::time::Instant::now();
        let mut builder = TaskGraphBuilder::new(&format!("bench_{}", trial), "benchmark graph");
        builder = builder.task("task_0", "desc", TaskType::Code);
        for i in 1..50 {
            builder = builder.then(&format!("task_{}", i), "desc", TaskType::Code);
        }
        let graph = builder.build().unwrap();
        build_latencies.push(start.elapsed().as_nanos() as f64);

        let sort_start = std::time::Instant::now();
        let _layers = graph.topological_layers().unwrap();
        sort_latencies.push(sort_start.elapsed().as_nanos() as f64);
    }

    build_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sort_latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let build_p99 = build_latencies[build_latencies.len() * 99 / 100];
    let sort_p99 = sort_latencies[sort_latencies.len() * 99 / 100];

    assert!(
        build_p99 < 5_000_000.0,
        "Task graph build P99 too high: {:.0}ns",
        build_p99
    );
    assert!(
        sort_p99 < 5_000_000.0,
        "Task graph sort P99 too high: {:.0}ns",
        sort_p99
    );
}

#[tokio::test]
async fn bench_recovery_latency() {
    use voxy_integration::recovery::{RecoveryConfig, SubsystemRecovery};

    let recovery = SubsystemRecovery::new(RecoveryConfig {
        max_restarts: 100,
        restart_delay_ms: 10,
        ..Default::default()
    });

    for i in 0..100 {
        recovery.register(format!("svc_{}", i));
    }

    let mut latencies = Vec::with_capacity(1000);

    for i in 0..1000 {
        let svc = format!("svc_{}", i % 100);
        let start = std::time::Instant::now();
        recovery.mark_running(&svc);
        recovery.report_failure(&svc, "test");
        latencies.push(start.elapsed().as_nanos() as f64);
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p99 = latencies[latencies.len() * 99 / 100];

    assert!(p99 < 100_000.0, "Recovery P99 too high: {:.0}ns", p99);
}

/// Detailed profiling benchmark — prints actual latency numbers.
#[cfg(not(debug_assertions))]
#[tokio::test]
async fn benchdetailed_latency_profile() {
    fn percentiles(data: &mut [f64]) -> (f64, f64, f64, f64) {
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let len = data.len();
        let p50 = data[len * 50 / 100];
        let p90 = data[len * 90 / 100];
        let p99 = data[len * 99 / 100];
        let max = data[len - 1];
        (p50, p90, p99, max)
    }

    // ── Event Bus ──
    {
        use std::sync::Arc;
        use voxy_event_bus::EventBus;
        use voxy_integration::event_bridge::{EventBridge, Topics};
        let bus = Arc::new(EventBus::new(256));
        let bridge = EventBridge::new(bus);
        let mut rx = bridge.subscribe(Topics::SYSTEM_HEALTH).await.unwrap();
        let mut latencies = Vec::with_capacity(5000);
        for i in 0..5000 {
            let start = std::time::Instant::now();
            bridge
                .publish(
                    Topics::SYSTEM_HEALTH,
                    "bench",
                    &format!("{{\"seq\":{}}}", i),
                )
                .await
                .unwrap();
            let _ = rx.recv().await.unwrap();
            latencies.push(start.elapsed().as_nanos() as f64);
        }
        let (p50, p90, p99, max) = percentiles(&mut latencies);
        eprintln!(
            "[EVENT BUS]      p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
    }

    // ── Telemetry ──
    {
        use voxy_integration::telemetry::{CentralTelemetry, SubsystemMetrics};
        let telemetry = CentralTelemetry::new();
        for i in 0..100 {
            telemetry.report(SubsystemMetrics::new(format!("svc_{}", i)));
        }
        let mut latencies = Vec::with_capacity(10000);
        for i in 0..10_000 {
            let start = std::time::Instant::now();
            telemetry.report(SubsystemMetrics {
                name: format!("svc_{}", i % 100),
                latency_ms: i as f64 * 0.01,
                ..SubsystemMetrics::new(format!("svc_{}", i % 100))
            });
            latencies.push(start.elapsed().as_nanos() as f64);
        }
        let (p50, p90, p99, max) = percentiles(&mut latencies);
        eprintln!(
            "[TELEMETRY]      p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
    }

    // ── Memory Store ──
    {
        let mut mem = voxy_memory::LongTermMemoryV2::new(10000, 5000, 500, 0.1);
        let mut latencies = Vec::with_capacity(1000);
        for i in 0..1000 {
            let item = voxy_memory::MemoryItemV2 {
                id: LtmMemoryId::new(),
                category: voxy_memory::MemoryCategory::Semantic,
                content: format!("Benchmark item {}", i),
                summary: None,
                tags: vec!["bench".to_string()],
                importance: ImportanceFactors {
                    recency: 0.5,
                    relevance: 0.8,
                    ..Default::default()
                },
                created_at: chrono::Utc::now(),
                last_accessed: chrono::Utc::now(),
                access_count: 0,
                version: 1,
                compressed: false,
                archived: false,
                project_id: None,
                related_memory_ids: Vec::new(),
                metadata: std::collections::HashMap::new(),
            };
            let start = std::time::Instant::now();
            mem.store(item);
            latencies.push(start.elapsed().as_nanos() as f64);
        }
        let (p50, p90, p99, max) = percentiles(&mut latencies);
        eprintln!(
            "[MEMORY STORE]   p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
    }

    // ── Goal Engine ──
    {
        use voxy_cognitive_orchestrator::goal_engine_v2::{GoalEngineV2, GoalPriority};
        let mut engine = GoalEngineV2::new(1000, 100);
        let mut create_latencies = Vec::with_capacity(500);
        let mut update_latencies = Vec::with_capacity(500);
        let mut ids = Vec::new();
        for i in 0..500 {
            let start = std::time::Instant::now();
            let id = engine
                .create_goal(
                    format!("Bench Goal {}", i),
                    "desc".into(),
                    GoalPriority::Medium,
                    None,
                )
                .unwrap();
            create_latencies.push(start.elapsed().as_nanos() as f64);
            ids.push(id);
        }
        for id in &ids {
            let start = std::time::Instant::now();
            engine.update_progress(id, 0.5).unwrap();
            update_latencies.push(start.elapsed().as_nanos() as f64);
        }
        let (p50, p90, p99, max) = percentiles(&mut create_latencies);
        eprintln!(
            "[GOAL CREATE]    p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
        let (p50, p90, p99, max) = percentiles(&mut update_latencies);
        eprintln!(
            "[GOAL UPDATE]    p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
    }

    // ── Task Graph ──
    {
        use voxy_planner::task_graph::*;
        let mut build_latencies = Vec::with_capacity(100);
        let mut sort_latencies = Vec::with_capacity(100);
        for trial in 0..100 {
            let start = std::time::Instant::now();
            let mut builder = TaskGraphBuilder::new(&format!("bench_{}", trial), "benchmark graph");
            builder = builder.task("task_0", "desc", TaskType::Code);
            for i in 1..50 {
                builder = builder.then(&format!("task_{}", i), "desc", TaskType::Code);
            }
            let graph = builder.build().unwrap();
            build_latencies.push(start.elapsed().as_nanos() as f64);
            let sort_start = std::time::Instant::now();
            let _layers = graph.topological_layers().unwrap();
            sort_latencies.push(sort_start.elapsed().as_nanos() as f64);
        }
        let (p50, p90, p99, max) = percentiles(&mut build_latencies);
        eprintln!(
            "[GRAPH BUILD]    p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
        let (p50, p90, p99, max) = percentiles(&mut sort_latencies);
        eprintln!(
            "[GRAPH SORT]     p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
    }

    // ── Recovery ──
    {
        use voxy_integration::recovery::{RecoveryConfig, SubsystemRecovery};
        let recovery = SubsystemRecovery::new(RecoveryConfig {
            max_restarts: 100,
            restart_delay_ms: 10,
            ..Default::default()
        });
        for i in 0..100 {
            recovery.register(format!("svc_{}", i));
        }
        let mut latencies = Vec::with_capacity(1000);
        for i in 0..1000 {
            let svc = format!("svc_{}", i % 100);
            let start = std::time::Instant::now();
            recovery.mark_running(&svc);
            recovery.report_failure(&svc, "test");
            latencies.push(start.elapsed().as_nanos() as f64);
        }
        let (p50, p90, p99, max) = percentiles(&mut latencies);
        eprintln!(
            "[RECOVERY]       p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
    }

    // ── Decision Engine ──
    {
        use voxy_cognitive_orchestrator::decision_engine::{
            ActionOption, ActionType, DecisionContext, DecisionEngine, DecisionId, ResourceCost,
            SecurityLevel, Urgency,
        };
        let mut engine = DecisionEngine::new(0.7, 0.5, SecurityLevel::High);
        let mut latencies = Vec::with_capacity(500);
        let options = vec![ActionOption {
            id: DecisionId::new(),
            name: "test_action".into(),
            description: "bench".into(),
            action_type: ActionType::Execute,
            estimated_time_ms: 100,
            resource_cost: ResourceCost::default(),
            security_level: SecurityLevel::Low,
            reversible: true,
            prerequisites: vec![],
        }];
        for i in 0..500 {
            let ctx = DecisionContext {
                id: DecisionId::new(),
                description: format!("bench decision {}", i),
                goals: vec!["test".into()],
                constraints: vec![],
                current_state: std::collections::HashMap::new(),
                urgency: Urgency::Medium,
                max_time_ms: Some(1000),
                preferred_action_type: None,
            };
            let start = std::time::Instant::now();
            let _ = engine.decide(&ctx, &options);
            latencies.push(start.elapsed().as_nanos() as f64);
        }
        let (p50, p90, p99, max) = percentiles(&mut latencies);
        eprintln!(
            "[DECISION]       p50={:.0}ns p90={:.0}ns p99={:.0}ns max={:.0}ns",
            p50, p90, p99, max
        );
    }

    eprintln!("\n[PROFILE COMPLETE]");
}
