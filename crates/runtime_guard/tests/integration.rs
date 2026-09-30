use voxy_runtime_guard::{GuardConfig, RuntimeGuard};

#[tokio::test]
async fn full_guard_lifecycle() {
    let guard = RuntimeGuard::new(GuardConfig::default());

    guard
        .register_subsystem("audio", || async {
            voxy_health::HealthReport::new("audio", voxy_shared::HealthStatus::Healthy)
        })
        .await;
    guard
        .register_subsystem("whisper", || async {
            voxy_health::HealthReport::new("whisper", voxy_shared::HealthStatus::Healthy)
        })
        .await;
    guard
        .register_subsystem("piper", || async {
            voxy_health::HealthReport::new("piper", voxy_shared::HealthStatus::Healthy)
        })
        .await;

    guard.heartbeat("audio");
    guard.heartbeat("whisper");
    guard.heartbeat("piper");

    assert!(guard.is_alive("audio"));
    assert!(guard.is_alive("whisper"));
    assert!(guard.is_alive("piper"));

    let snap = guard.snapshot().await;
    assert_eq!(snap.subsystems.len(), 3);
    assert!(snap.health_pct >= 0.0);
    assert!(snap.health_pct <= 100.0);
}

#[tokio::test]
async fn dashboard_after_registration() {
    let guard = RuntimeGuard::new(GuardConfig::default());
    guard
        .register_subsystem("svc1", || async {
            voxy_health::HealthReport::new("svc1", voxy_shared::HealthStatus::Healthy)
        })
        .await;
    guard.heartbeat("svc1");

    let dash = guard.dashboard().await;
    assert!(dash.html.contains("svc1"));
    assert!(dash.html.contains("Runtime Dashboard"));
}

#[tokio::test]
async fn heartbeat_liveness_check() {
    let guard = RuntimeGuard::new(GuardConfig::default());
    guard
        .register_subsystem("svc", || async {
            voxy_health::HealthReport::new("svc", voxy_shared::HealthStatus::Healthy)
        })
        .await;
    guard.heartbeat("svc");
    assert!(guard.is_alive("svc"));
}

#[tokio::test]
async fn multiple_snapshots_consistent() {
    let guard = RuntimeGuard::new(GuardConfig::default());
    guard
        .register_subsystem("a", || async {
            voxy_health::HealthReport::new("a", voxy_shared::HealthStatus::Healthy)
        })
        .await;
    guard.heartbeat("a");

    let snap1 = guard.snapshot().await;
    let snap2 = guard.snapshot().await;
    assert_eq!(snap1.subsystems.len(), snap2.subsystems.len());
}

#[tokio::test]
async fn snapshot_json_is_valid() {
    let guard = RuntimeGuard::new(GuardConfig::default());
    guard
        .register_subsystem("test", || async {
            voxy_health::HealthReport::new("test", voxy_shared::HealthStatus::Healthy)
        })
        .await;
    guard.heartbeat("test");

    let snap = guard.snapshot().await;
    let json = snap.to_json();
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("invalid JSON");
    assert_eq!(parsed["subsystems"].as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn self_healing_register_and_heal() {
    use voxy_runtime_guard::HealingConfig;

    let guard = RuntimeGuard::new(GuardConfig {
        healing: HealingConfig {
            base_backoff_ms: 0,
            max_restart_attempts: 3,
            ..Default::default()
        },
        ..Default::default()
    });

    let heal_called = std::sync::Arc::new(tokio::sync::Mutex::new(false));
    let flag = heal_called.clone();

    guard
        .register_healable(
            "failing_svc",
            || async {
                voxy_health::HealthReport::new(
                    "failing_svc",
                    voxy_shared::HealthStatus::Unhealthy("down".into()),
                )
            },
            move || {
                let flag = flag.clone();
                async move {
                    *flag.lock().await = true;
                    Ok(())
                }
            },
        )
        .await;

    guard.heartbeat("failing_svc");

    let result = guard.heal("failing_svc").await;
    assert!(result.is_ok());
    assert!(*heal_called.lock().await);
}

#[tokio::test]
async fn snapshot_system_metrics() {
    let guard = RuntimeGuard::new(GuardConfig::default());
    guard
        .register_subsystem("svc", || async {
            voxy_health::HealthReport::new("svc", voxy_shared::HealthStatus::Healthy)
        })
        .await;
    guard.heartbeat("svc");

    let snap = guard.snapshot().await;
    assert!(snap.ram_total_mb > 0);
    assert!(snap.thread_count > 0);
}
