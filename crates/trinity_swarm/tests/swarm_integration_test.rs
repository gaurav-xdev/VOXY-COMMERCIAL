use std::sync::Arc;
use voxy_model_router::ModelRouter;
use voxy_provider_core::{
    DefaultProviderRegistry, LlmProvider, ModelInfo, ProviderCapability, ProviderHealth,
    ProviderInfo, ProviderKind, ProviderStatus,
};
use voxy_tool_calling::{Tool, ToolContext};
use voxy_trinity_swarm::{
    AgentRole, OsmooSwarmOrchestratorTool, OsmooSwarmStatusTool,
    SwarmCollaborationWorkflow, SwarmCoordinator, SwarmProviderBridge,
    SwarmSecurityBoundary, OSMOO_OPERATOR_IDENTITY,
};

struct DynamicMockLlm;

#[async_trait::async_trait]
impl LlmProvider for DynamicMockLlm {
    async fn complete(&self, prompt: &str) -> voxy_provider_core::Result<String> {
        if prompt.contains("Supervisor") {
            Ok("Supervisor: Created 4-phase DAG mission plan.".into())
        } else if prompt.contains("Architect") {
            Ok("Architect: Defined schema for SecureChannel and TokenVerifier.".into())
        } else if prompt.contains("Specialist Worker") {
            Ok("Specialist Worker: Implemented HMAC-SHA256 constant-time comparison in Rust.".into())
        } else if prompt.contains("Reviewer") {
            Ok("Reviewer: Kodus AI inspection passed with 0 blocking defects. Invariant held.".into())
        } else if prompt.contains("QA and Canary Tester") {
            Ok("QA: All unit and canary assertions verified successfully.".into())
        } else {
            Ok("Mock LLM generic completion".into())
        }
    }

    fn available_models(&self) -> Vec<String> {
        vec!["gpt-4o".into()]
    }

    fn name(&self) -> &str {
        "dynamic-mock-llm"
    }
}

fn setup_test_coordinator() -> (Arc<SwarmCoordinator>, String) {
    let registry = Arc::new(DefaultProviderRegistry::new());
    let router = Arc::new(ModelRouter::new(registry.clone()));
    let provider = Arc::new(DynamicMockLlm);

    let info = ProviderInfo {
        id: "openai".into(),
        name: "Mock Provider".into(),
        kind: ProviderKind::Cloud,
        capability: ProviderCapability::Llm,
        status: ProviderStatus::Available,
        models: vec![ModelInfo::new("gpt-4o", "GPT-4o")],
        health: ProviderHealth::new_healthy(None),
        base_url: None,
        priority: 1,
    };

    registry.register_sync(info);
    router.register_llm("openai", provider);

    let security = SwarmSecurityBoundary::new();
    let token = security.session_token();
    let bridge = SwarmProviderBridge::new(router, security.clone());
    let coordinator = Arc::new(SwarmCoordinator::new(bridge, security, "."));

    (coordinator, token)
}

#[tokio::test]
async fn test_end_to_end_trinity_swarm_mission_execution() {
    let (coordinator, token) = setup_test_coordinator();

    let receipt = coordinator
        .execute_mission(
            "Architect and implement zero-trust memory channel",
            OSMOO_OPERATOR_IDENTITY,
            &token,
        )
        .await
        .expect("Mission execution should succeed");

    assert!(receipt.success);
    assert_eq!(receipt.total_tasks, 4);
    assert_eq!(receipt.completed_tasks, 4);
    assert_eq!(receipt.failed_tasks, 0);

    // Verify artifacts were registered in blackboard
    assert_eq!(receipt.artifacts.len(), 4);
    for art_id in &receipt.artifacts {
        let art = coordinator.blackboard().get_artifact(art_id);
        assert!(art.is_some());
    }

    // Verify Trinity A2A message bus tracked each agent turn
    let total_messages = coordinator.message_bus().count();
    // 1 supervisor proposal + (1 accepted + 1 completed) * 4 tasks = 9 messages
    assert!(total_messages >= 9);

    // Verify SwarmClaw handoff envelopes were generated
    let worker_handoffs = coordinator.blackboard().get_handoffs_for_role(AgentRole::SpecialistWorker);
    assert_eq!(worker_handoffs.len(), 1);
    assert!(worker_handoffs[0].summary.contains("Architectural & Contract"));

    let reviewer_handoffs = coordinator.blackboard().get_handoffs_for_role(AgentRole::Reviewer);
    assert_eq!(reviewer_handoffs.len(), 1);
    assert!(reviewer_handoffs[0].summary.contains("Specialist Code Implementation"));

    let qa_handoffs = coordinator.blackboard().get_handoffs_for_role(AgentRole::QaTester);
    assert_eq!(qa_handoffs.len(), 1);
    assert!(qa_handoffs[0].summary.contains("Kodus & Security Review"));
}

#[tokio::test]
async fn test_collaboration_workflow_bounded_loop() {
    let (coordinator, token) = setup_test_coordinator();
    let workflow = SwarmCollaborationWorkflow::new(coordinator.clone()).with_max_loops(2);

    let receipt = workflow
        .run("Build hardened sandbox boundary", OSMOO_OPERATOR_IDENTITY, &token)
        .await
        .expect("Workflow run should succeed");

    assert!(receipt.success);
    assert_eq!(receipt.completed_tasks, 4);
}

#[tokio::test]
async fn test_orchestrator_tool_and_status_tool_security_enforcement() {
    let (coordinator, valid_token) = setup_test_coordinator();
    let orchestrator_tool = OsmooSwarmOrchestratorTool::new(coordinator.clone());
    let status_tool = OsmooSwarmStatusTool::new(coordinator.clone());
    let ctx = ToolContext::new("osmoo-operator-session");

    // 1. Untrusted caller rejected on orchestrator tool
    let bad_orch = orchestrator_tool
        .execute(
            serde_json::json!({
                "caller_identity": "external_script",
                "internal_token": valid_token,
                "goal": "Malicious payload"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!bad_orch.success);
    assert!(bad_orch.error.unwrap().contains("ACCESS DENIED"));

    // 2. Untrusted caller rejected on status tool
    let bad_status = status_tool
        .execute(
            serde_json::json!({
                "caller_identity": "external_script",
                "internal_token": valid_token
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!bad_status.success);
    assert!(bad_status.error.unwrap().contains("ACCESS DENIED"));

    // 3. Trusted OSMOO operator succeeds on orchestrator tool
    let good_orch = orchestrator_tool
        .execute(
            serde_json::json!({
                "caller_identity": OSMOO_OPERATOR_IDENTITY,
                "internal_token": valid_token,
                "goal": "Verify IPC descriptor safety"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(good_orch.success);

    // 4. Trusted OSMOO operator succeeds on status tool
    let good_status = status_tool
        .execute(
            serde_json::json!({
                "caller_identity": OSMOO_OPERATOR_IDENTITY,
                "internal_token": valid_token
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(good_status.success);
    assert!(good_status.observation.unwrap().contains("Swarm Status:"));
}
