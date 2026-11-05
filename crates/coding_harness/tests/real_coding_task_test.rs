use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use voxy_coding_harness::{
    CodingEngine, CommandExecutionInput, ContextExplorer, FileEditInput, TaskPlanner, TaskStatus,
    TaskStep, ToolchainDetector, ToolchainStatus,
};
use voxy_model_router::{ModelRouter, RouterConfig, RoutingMode};
use voxy_provider_core::{
    DefaultProviderRegistry, LlmProvider, ModelInfo, ProviderCapability, ProviderHealth,
    ProviderInfo, ProviderKind, ProviderRegistry, ProviderStatus,
};
use voxy_security::approval::ApprovalBroker;

/// Deterministic Harness Verification:
/// Verifies the lower-level building blocks: project layout, toolchain detection,
/// workspace search, defect detection via command runner, diagnostics, and atomic transaction commit.
#[tokio::test]
async fn test_deterministic_harness_patch_and_verify_cycle() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    // 1. Setup a real multi-file Rust project in the isolated workspace
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    let cargo_toml = r#"
[package]
name = "math_engine"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    fs::write(ws.join("Cargo.toml"), cargo_toml.trim()).unwrap();

    let lib_rs = r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    // Intentional defect for OSMOO to diagnose and repair:
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(3, 4), 12);
    }
}
"#;
    fs::write(src.join("lib.rs"), lib_rs.trim()).unwrap();

    // 2. Initialize OSMOO Coding Harness
    let broker = Arc::new(ApprovalBroker::new());
    let engine = CodingEngine::new(ws).with_approval_broker(broker);
    let recovered = engine.initialize().unwrap();
    assert_eq!(
        recovered.len(),
        0,
        "No pending crash recovery on clean start"
    );

    // 3. Detect toolchain
    let rust_toolchain = ToolchainDetector::detect_rust(ws);
    assert!(
        matches!(rust_toolchain.status, ToolchainStatus::Available { .. }),
        "Cargo toolchain must be available"
    );

    // 4. Explore codebase using ContextExplorer
    let globbed = ContextExplorer::glob_files(ws, "*.rs", None).unwrap();
    assert_eq!(globbed, vec!["src/lib.rs"]);

    let grep_matches = ContextExplorer::grep_pattern(ws, "fn multiply", None, None).unwrap();
    assert_eq!(grep_matches.len(), 1);
    assert_eq!(grep_matches[0].line_number, 5);

    // 5. Run initial test to detect the failure
    let initial_test_cmd = CommandExecutionInput {
        command: "cargo test --lib".into(),
        cwd: None,
        timeout_ms: Some(60_000),
        description: Some("Run initial unit tests to surface defect".into()),
        run_in_background: false,
    };
    let initial_res = engine
        .execute_command(initial_test_cmd, "task-verify-1")
        .await
        .unwrap();
    assert_ne!(
        initial_res.exit_code, 0,
        "Initial test must fail due to intentional defect"
    );
    assert!(
        initial_res
            .stdout
            .contains("test tests::test_multiply ... FAILED")
            || initial_res
                .stderr
                .contains("test tests::test_multiply ... FAILED")
    );

    // 6. Diagnose root cause via Debugger
    let diagnosis = engine.diagnose_failure(&initial_res.stdout);
    assert!(!diagnosis.primary_cause.is_empty());

    // 7. Create TaskPlan with dependency DAG
    let plan_step = TaskStep {
        id: "repair-multiply".into(),
        title: "Fix multiplication logic in lib.rs".into(),
        description: "Replace addition operator with multiplication operator in multiply function"
            .into(),
        status: TaskStatus::Pending,
        active_form: Some("Patching multiplication function".into()),
        dependencies: vec![],
        affected_files: vec!["src/lib.rs".into()],
        error: None,
    };
    let mut plan =
        TaskPlanner::create_plan("Math Repair", "Repair intentional defect", vec![plan_step]);

    let runnable = TaskPlanner::get_next_runnable_steps(&plan);
    assert_eq!(runnable.len(), 1);

    // 8. Begin durable transaction and apply patch
    let mut tx = engine.begin_transaction().unwrap();

    let patch_input = FileEditInput {
        file_path: "src/lib.rs".into(),
        old_string: "pub fn multiply(a: i32, b: i32) -> i32 {\n    // Intentional defect for OSMOO to diagnose and repair:\n    a + b\n}".into(),
        new_string: "pub fn multiply(a: i32, b: i32) -> i32 {\n    a * b\n}".into(),
        replace_all: false,
    };

    let patch_res = engine.apply_file_edit(&mut tx, &patch_input).unwrap();
    assert!(patch_res.applied);
    assert!(patch_res.diff.contains("+    a * b"));

    // 9. Re-run tests to verify the fix with empirical evidence
    let verify_cmd = CommandExecutionInput {
        command: "cargo test --lib".into(),
        cwd: None,
        timeout_ms: Some(60_000),
        description: Some("Verify repaired unit tests".into()),
        run_in_background: false,
    };
    let verify_res = engine
        .execute_command(verify_cmd, "task-verify-1")
        .await
        .unwrap();
    assert_eq!(
        verify_res.exit_code, 0,
        "Repaired project must pass all unit tests"
    );
    assert!(
        verify_res
            .stdout
            .contains("test result: ok. 2 passed; 0 failed")
            || verify_res.stdout.contains("2 passed; 0 failed")
    );

    // 10. Commit durable transaction
    tx.commit().unwrap();
    TaskPlanner::update_step_status(&mut plan, "repair-multiply", TaskStatus::Completed, None)
        .unwrap();
    assert!(TaskPlanner::is_complete(&plan));
}

/// Mock LLM provider for autonomous repair test.
/// Dynamically examines the model prompt (containing diagnosis and source code)
/// and emits a structured JSON patch proposal.
struct AutonomousRepairMockLlm;

#[async_trait::async_trait]
impl LlmProvider for AutonomousRepairMockLlm {
    async fn complete(&self, prompt: &str) -> voxy_provider_core::Result<String> {
        // Assert that the prompt contains required context components
        assert!(prompt.contains("Task:"));
        assert!(prompt.contains("Failure Diagnosis:"));
        assert!(prompt.contains("File 'src/lib.rs':"));

        // Emit model-generated JSON patch
        let patch_json = r#"{
            "file_path": "src/lib.rs",
            "old_string": "    // Intentional defect for OSMOO to diagnose and repair:\n    a + b",
            "new_string": "    a * b",
            "replace_all": false
        }"#;

        Ok(patch_json.to_string())
    }

    fn available_models(&self) -> Vec<String> {
        vec!["mock-repair-model".into()]
    }

    fn name(&self) -> &str {
        "AutonomousRepairMockLlm"
    }
}

/// Model-Driven Autonomous Repair Test:
/// Tests the end-to-end model-driven autonomous loop:
/// 1. Harness detects compiler test failure.
/// 2. Harness reads source code and diagnoses failure root cause.
/// 3. Harness queries the ModelRouter.
/// 4. Model returns a structured JSON patch proposal.
/// 5. Harness dynamically parses the patch (NOT hardcoded in test), applies it,
///    verifies with cargo test, runs independent code review, and commits.
#[tokio::test]
async fn test_model_driven_autonomous_repair_workflow() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    let cargo_toml = r#"
[package]
name = "math_auto"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;
    fs::write(ws.join("Cargo.toml"), cargo_toml.trim()).unwrap();

    let lib_rs = r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    // Intentional defect for OSMOO to diagnose and repair:
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(3, 4), 12);
    }
}
"#;
    fs::write(src.join("lib.rs"), lib_rs.trim()).unwrap();

    // Configure ModelRouter with AutonomousRepairMockLlm
    let registry = Arc::new(DefaultProviderRegistry::new());
    let provider_info = ProviderInfo {
        id: "autonomous-mock-llm".to_string(),
        name: "Autonomous Mock LLM".to_string(),
        kind: ProviderKind::Local,
        capability: ProviderCapability::Llm,
        status: ProviderStatus::Available,
        models: vec![ModelInfo::new("mock-repair-model", "mock-repair-model")],
        health: ProviderHealth::new_healthy(Some(5.0)),
        base_url: None,
        priority: 10,
    };
    registry.register(provider_info).await.unwrap();

    let router = Arc::new(ModelRouter::with_config(
        registry,
        RouterConfig {
            mode: RoutingMode::ForceProvider("autonomous-mock-llm".into()),
            ..Default::default()
        },
    ));
    router.register_llm("autonomous-mock-llm", Arc::new(AutonomousRepairMockLlm));

    let engine = CodingEngine::new(ws).with_model_router(router);
    engine.initialize().unwrap();

    // Execute the autonomous repair task
    let outcome = engine
        .execute_autonomous_repair_task(
            "Diagnose and fix the failing multiplication test in src/lib.rs",
            "session-auto-1",
        )
        .await
        .unwrap();

    assert!(outcome.success, "Autonomous repair workflow must succeed");
    assert!(
        outcome.proposed_patch.is_some(),
        "Model must propose a patch"
    );
    assert!(
        outcome.diff.is_some(),
        "Harness must produce a verified diff"
    );
    assert!(!outcome.review_blocking, "Review gate must pass cleanly");
    assert!(
        outcome
            .verification_stdout
            .contains("test result: ok. 2 passed; 0 failed")
            || outcome.verification_stdout.contains("2 passed; 0 failed"),
        "Verification output must show 2 passed tests"
    );

    // Verify file content on disk was actually updated
    let repaired_content = fs::read_to_string(src.join("lib.rs")).unwrap();
    assert!(repaired_content.contains("a * b"));
}
