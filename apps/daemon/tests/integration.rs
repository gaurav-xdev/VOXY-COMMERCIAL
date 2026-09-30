use std::sync::Arc;

#[tokio::test]
async fn test_bounded_channel_backpressure() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(3);

    assert!(tx.try_send("a").is_ok());
    assert!(tx.try_send("b").is_ok());
    assert!(tx.try_send("c").is_ok());
    assert!(tx.try_send("d").is_err());

    assert_eq!(rx.try_recv().unwrap(), "a");
    assert!(tx.try_send("d").is_ok());
}

#[tokio::test]
async fn test_knowledge_validation() {
    use voxy_cognitive_orchestrator::config::{KnowledgeValidationConfig, RiskLevel};
    use voxy_cognitive_orchestrator::knowledge_validation::{
        KnowledgeItem, KnowledgeValidator, SourceType, ValidationStatus,
    };

    let config = KnowledgeValidationConfig::default();
    let mut validator = KnowledgeValidator::new(config);

    let item = KnowledgeItem {
        id: uuid::Uuid::new_v4(),
        content: "Rust is a systems programming language".to_string(),
        source: "test".to_string(),
        source_type: SourceType::Verified,
        trust_score: 0.9,
        risk_level: RiskLevel::Low,
        cross_references: vec!["ref1".to_string(), "ref2".to_string()],
        validation_status: ValidationStatus::Pending,
        timestamp: chrono::Utc::now(),
    };

    let result = validator.validate(item).unwrap();
    assert_eq!(result.status, ValidationStatus::Validated);
    assert!(result.trust_score > 0.5);
}

#[tokio::test]
async fn test_reflection_engine_analysis() {
    use voxy_cognitive_orchestrator::config::OrchestratorConfig;
    use voxy_cognitive_orchestrator::reflection::{ConversationRecord, ReflectionEngine};

    let config = OrchestratorConfig::default();
    let mut engine = ReflectionEngine::new(config.reflection);

    let record = ConversationRecord {
        id: uuid::Uuid::new_v4(),
        messages: vec![
            ("user".to_string(), "What is Rust?".to_string()),
            ("assistant".to_string(), "Rust is a systems programming language focused on safety, speed, and concurrency.".to_string()),
            ("user".to_string(), "How do I learn it?".to_string()),
            ("assistant".to_string(), "Start with the official Rust Book at doc.rust-lang.org/book/. It's free and comprehensive.".to_string()),
            ("user".to_string(), "Thanks!".to_string()),
            ("assistant".to_string(), "You're welcome! Feel free to ask more questions.".to_string()),
        ],
        context: "test".to_string(),
        timestamp: chrono::Utc::now(),
    };

    let result = engine.analyze_conversation(record).unwrap();
    assert!(result.quality_score > 0.0);
    assert!(result.correctness_score > 0.0);
}

#[tokio::test]
async fn test_cognitive_bridge_creation() {
    use voxy_cognitive_orchestrator::bridge::{CognitiveBridge, CognitiveEvent};
    use voxy_cognitive_orchestrator::config::OrchestratorConfig;

    let config = OrchestratorConfig::default();
    let bridge = CognitiveBridge::new(config);

    assert_eq!(bridge.reflection.get_lessons().len(), 0);
    assert_eq!(bridge.knowledge_validator.get_validated().len(), 0);

    let mut rx = bridge.subscribe();
    bridge.emit(CognitiveEvent::DecisionMade("test".to_string()));
    assert!(rx.try_recv().is_ok());
}

#[tokio::test]
async fn test_runtime_guard_heartbeat() {
    use voxy_runtime_guard::{GuardConfig, RuntimeGuard};

    let guard = RuntimeGuard::new(GuardConfig::default());
    guard
        .register_subsystem("test_svc", || async {
            voxy_health::HealthReport::new("test_svc", voxy_shared::HealthStatus::Healthy)
        })
        .await;

    guard.heartbeat("test_svc");
    assert!(guard.is_alive("test_svc"));

    let snap = guard.snapshot().await;
    assert!(snap.subsystems.contains_key("test_svc"));
}

#[tokio::test]
async fn test_broadcast_channel_shutdown() {
    let (tx, _) = tokio::sync::broadcast::channel::<()>(16);
    let mut rx1 = tx.subscribe();
    let mut rx2 = tx.subscribe();

    // Send shutdown signal
    let _ = tx.send(());

    // Both receivers should get the signal
    assert!(rx1.try_recv().is_ok());
    assert!(rx2.try_recv().is_ok());
}

#[tokio::test]
async fn test_concurrent_channel_senders() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let tx = Arc::new(tx);

    let mut handles = Vec::new();
    for i in 0..10 {
        let tx = tx.clone();
        handles.push(tokio::spawn(async move {
            tx.send(format!("msg_{i}")).await.unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    assert_eq!(count, 10);
}
