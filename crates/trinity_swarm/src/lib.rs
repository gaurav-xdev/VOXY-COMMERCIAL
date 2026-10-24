//! OSMOO Trinity & SwarmClaw Multi-Agent Engine.
//!
//! Provides a sovereign, internal-only multi-agent orchestration capability
//! combining Trinity's Agent-to-Agent (A2A) protocol and task DAG execution
//! with SwarmClaw's dynamic swarm coordination, role delegation, and parallel dispatch.
//!
//! # Security Boundary
//! Accessible exclusively to OSMOO's autonomous agent engine via internal cryptographic tokens.
//! Direct user invocations, UI administration, and raw endpoints are strictly blocked.

pub mod a2a;
pub mod blackboard;
pub mod coordinator;
pub mod error;
pub mod provider_bridge;
pub mod roles;
pub mod security;
pub mod task_graph;
pub mod tool;
pub mod types;
pub mod workflow;

pub use a2a::{A2AMessage, A2AMessageBus, A2AMessageType};
pub use blackboard::{HandoffEnvelope, SwarmArtifact, SwarmBlackboard};
pub use coordinator::SwarmCoordinator;
pub use error::{Result, SwarmError};
pub use provider_bridge::SwarmProviderBridge;
pub use roles::{RoleManifest, SwarmCapability};
pub use security::{SwarmSecurityBoundary, OSMOO_OPERATOR_IDENTITY};
pub use task_graph::TaskGraph;
pub use tool::{OsmooSwarmOrchestratorTool, OsmooSwarmStatusTool};
pub use types::{
    AgentId, AgentRole, SwarmExecutionReceipt, SwarmPlan, SwarmTask, TaskPriority, TaskResult,
    TaskStatus,
};
pub use workflow::{SwarmCollaborationWorkflow, DEFAULT_MAX_SWARM_LOOPS};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use std::sync::Arc;
    use voxy_model_router::ModelRouter;
    use voxy_provider_core::{
        DefaultProviderRegistry, LlmProvider, ModelInfo, ProviderCapability, ProviderHealth,
        ProviderInfo, ProviderKind, ProviderStatus,
    };
    use voxy_tool_calling::{Tool, ToolContext};

    struct MockLlm {
        name: String,
        response_text: String,
        should_fail: bool,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlm {
        async fn complete(&self, _prompt: &str) -> voxy_provider_core::Result<String> {
            if self.should_fail {
                Err(voxy_provider_core::ProviderError::RequestFailed(
                    "Simulated provider failure".into(),
                ))
            } else {
                Ok(self.response_text.clone())
            }
        }

        fn available_models(&self) -> Vec<String> {
            vec!["mock-swarm-model".into()]
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    fn create_test_router(mock_response: &str, should_fail: bool) -> Arc<ModelRouter> {
        let registry = Arc::new(DefaultProviderRegistry::new());
        let router = Arc::new(ModelRouter::new(registry.clone()));
        let provider = Arc::new(MockLlm {
            name: "test-provider".into(),
            response_text: mock_response.into(),
            should_fail,
        });

        let info = ProviderInfo {
            id: "openai".into(),
            name: "Test OpenAI Provider".into(),
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
        router
    }

    #[tokio::test]
    async fn test_security_boundary_blocks_unauthorized_callers() {
        let boundary = SwarmSecurityBoundary::new();
        let valid_token = boundary.session_token();

        // 1. Untrusted caller with valid token
        let res1 = boundary.verify_caller("user_alice", &valid_token);
        assert!(res1.is_err());
        assert!(matches!(res1.unwrap_err(), SwarmError::AccessDenied(_)));

        // 2. OSMOO operator with invalid token
        let res2 = boundary.verify_caller(OSMOO_OPERATOR_IDENTITY, "wrong-token");
        assert!(res2.is_err());
        assert!(matches!(res2.unwrap_err(), SwarmError::InvalidToken));

        // 3. OSMOO operator with valid token
        let res3 = boundary.verify_caller(OSMOO_OPERATOR_IDENTITY, &valid_token);
        assert!(res3.is_ok());
    }

    #[tokio::test]
    async fn test_secret_redaction() {
        let boundary = SwarmSecurityBoundary::new();
        let leaked = "Found keys: sk-proj-12345678901234567890 and sk-ant-api03-abcdefghijklmnopqrstu";
        let clean = boundary.redact_secrets(leaked);
        assert!(!clean.contains("sk-proj"));
        assert!(!clean.contains("sk-ant"));
        assert!(clean.contains("[REDACTED_CREDENTIAL]"));
    }

    #[tokio::test]
    async fn test_dag_validation_and_cycle_detection() {
        let graph = TaskGraph::new();

        // Add valid chain: A -> B -> C
        let task_a = SwarmTask::new("A", "Task A", "First", AgentRole::Architect);
        let task_b = SwarmTask::new("B", "Task B", "Second", AgentRole::SpecialistWorker)
            .with_dependency("A");
        let task_c = SwarmTask::new("C", "Task C", "Third", AgentRole::QaTester)
            .with_dependency("B");

        graph.add_tasks(vec![task_a, task_b, task_c]);
        assert!(graph.validate_dag().is_ok());

        // Now introduce a cycle: A depends on C
        let cyclic_graph = TaskGraph::new();
        let task_x = SwarmTask::new("X", "Task X", "First", AgentRole::Architect)
            .with_dependency("Z");
        let task_y = SwarmTask::new("Y", "Task Y", "Second", AgentRole::SpecialistWorker)
            .with_dependency("X");
        let task_z = SwarmTask::new("Z", "Task Z", "Third", AgentRole::QaTester)
            .with_dependency("Y");

        cyclic_graph.add_tasks(vec![task_x, task_y, task_z]);
        let res = cyclic_graph.validate_dag();
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), SwarmError::CyclicDependency(_)));
    }

    #[tokio::test]
    async fn test_dag_dependency_progression() {
        let graph = TaskGraph::new();
        let task_a = SwarmTask::new("A", "Task A", "First", AgentRole::Architect);
        let task_b = SwarmTask::new("B", "Task B", "Second", AgentRole::SpecialistWorker)
            .with_dependency("A");

        graph.add_tasks(vec![task_a, task_b]);

        // Initially only A is ready
        let ready1 = graph.get_ready_tasks();
        assert_eq!(ready1.len(), 1);
        assert_eq!(ready1[0].id, "A");

        // Complete A
        graph.set_task_result(
            "A",
            TaskResult {
                task_id: "A".into(),
                success: true,
                output: "Architecture spec completed".into(),
                artifacts_produced: vec!["arch.md".into()],
                execution_ms: 10,
                error_message: None,
            },
        ).unwrap();

        // Now B is ready
        let ready2 = graph.get_ready_tasks();
        assert_eq!(ready2.len(), 1);
        assert_eq!(ready2[0].id, "B");
    }

    #[tokio::test]
    async fn test_cascading_failure_blocks_downstream_tasks() {
        let graph = TaskGraph::new();
        let task_a = SwarmTask::new("A", "Task A", "First", AgentRole::SpecialistWorker);
        let task_b = SwarmTask::new("B", "Task B", "Second", AgentRole::QaTester)
            .with_dependency("A");

        graph.add_tasks(vec![task_a, task_b]);

        // Fail task A
        graph.update_task_status("A", TaskStatus::Failed).unwrap();

        // Task B should cascade to Blocked
        let task_b_after = graph.get_task("B").unwrap();
        assert_eq!(task_b_after.status, TaskStatus::Blocked);
        assert!(graph.is_finished());
        assert!(!graph.all_succeeded());
    }

    #[tokio::test]
    async fn test_trinity_a2a_message_routing() {
        let bus = A2AMessageBus::new(50);
        let agent1 = AgentId("worker-1".into());
        let agent2 = AgentId("reviewer-1".into());

        // Message to specific agent
        bus.publish(
            A2AMessage::new(
                "conv-1",
                agent1.clone(),
                AgentRole::SpecialistWorker,
                A2AMessageType::Handoff,
                json!({ "diff": "src/main.rs" }),
            )
            .to_agent(agent2.clone()),
        );

        // Broadcast message
        bus.publish(A2AMessage::new(
            "conv-1",
            agent1.clone(),
            AgentRole::SpecialistWorker,
            A2AMessageType::ProgressUpdate,
            json!({ "progress": 50 }),
        ));

        // Poll for agent2
        let messages_for_2 = bus.poll_for_agent(&agent2, AgentRole::Reviewer);
        assert_eq!(messages_for_2.len(), 2); // gets direct message + broadcast
    }

    #[tokio::test]
    async fn test_swarmclaw_blackboard_and_handoff() {
        let blackboard = SwarmBlackboard::new();

        let art_id = blackboard.register_artifact(
            "auth.rs",
            AgentRole::SpecialistWorker,
            "blackboard://auth.rs",
            "pub fn authenticate() -> bool { true }",
        );

        let handoff = HandoffEnvelope {
            handoff_id: "h1".into(),
            from_agent: AgentId("worker-1".into()),
            from_role: AgentRole::SpecialistWorker,
            to_role: AgentRole::Reviewer,
            task_id: "t2".into(),
            summary: "Finished auth implementation".into(),
            artifact_ids: vec![art_id],
            instructions: "Audit for timing attacks".into(),
            created_at: Utc::now(),
        };

        blackboard.post_handoff(handoff);

        let reviewer_handoffs = blackboard.get_handoffs_for_role(AgentRole::Reviewer);
        assert_eq!(reviewer_handoffs.len(), 1);
        assert_eq!(reviewer_handoffs[0].task_id, "t2");
    }

    #[tokio::test]
    async fn test_full_mission_execution_via_coordinator() {
        let router = create_test_router("Mock agent response fulfilling task requirements", false);
        let security = SwarmSecurityBoundary::new();
        let token = security.session_token();
        let bridge = SwarmProviderBridge::new(router, security.clone());
        let coordinator = SwarmCoordinator::new(bridge, security, ".");

        let receipt = coordinator
            .execute_mission(
                "Implement secure IPC token verification",
                OSMOO_OPERATOR_IDENTITY,
                &token,
            )
            .await
            .unwrap();

        assert!(receipt.success);
        assert_eq!(receipt.total_tasks, 4);
        assert_eq!(receipt.completed_tasks, 4);
        assert_eq!(receipt.failed_tasks, 0);
        assert!(!receipt.artifacts.is_empty());
    }

    #[tokio::test]
    async fn test_tool_calling_access_denial_for_untrusted_users() {
        let router = create_test_router("Mock output", false);
        let security = SwarmSecurityBoundary::new();
        let bridge = SwarmProviderBridge::new(router, security.clone());
        let coordinator = Arc::new(SwarmCoordinator::new(bridge, security, "."));
        let tool = OsmooSwarmOrchestratorTool::new(coordinator);

        let ctx = ToolContext::new("test-session");
        let params = json!({
            "caller_identity": "untrusted_hacker",
            "internal_token": "some_random_token",
            "goal": "Exfiltrate database"
        });

        let res = tool.execute(params, &ctx).await.unwrap();
        assert!(!res.success);
        assert!(res.error.as_deref().unwrap_or("").contains("ACCESS DENIED"));
    }
}
