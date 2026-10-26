use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use voxy_coding_harness::{
    CodingEngine, CommandExecutionInput, ContextExplorer, FileEditInput, TaskPlanner, TaskStatus,
    TaskStep, ToolchainDetector, ToolchainStatus,
};
use voxy_security::approval::ApprovalBroker;

#[tokio::test]
async fn test_real_autonomous_coding_task_end_to_end() {
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
    assert_eq!(recovered.len(), 0, "No pending crash recovery on clean start");

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
    let initial_res = engine.execute_command(initial_test_cmd, "task-verify-1").await.unwrap();
    assert_ne!(
        initial_res.exit_code, 0,
        "Initial test must fail due to intentional defect"
    );
    assert!(
        initial_res.stdout.contains("test tests::test_multiply ... FAILED")
            || initial_res.stderr.contains("test tests::test_multiply ... FAILED")
    );

    // 6. Diagnose root cause via Debugger
    let diagnosis = engine.diagnose_failure(&initial_res.stdout);
    assert!(!diagnosis.primary_cause.is_empty());

    // 7. Create TaskPlan with dependency DAG
    let plan_step = TaskStep {
        id: "repair-multiply".into(),
        title: "Fix multiplication logic in lib.rs".into(),
        description: "Replace addition operator with multiplication operator in multiply function".into(),
        status: TaskStatus::Pending,
        active_form: Some("Patching multiplication function".into()),
        dependencies: vec![],
        affected_files: vec!["src/lib.rs".into()],
        error: None,
    };
    let mut plan = TaskPlanner::create_plan("Math Repair", "Repair intentional defect", vec![plan_step]);

    let runnable = TaskPlanner::get_next_runnable_steps(&plan);
    assert_eq!(runnable.len(), 1);

    // 8. Begin durable transaction and apply atomic patch
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
    let verify_res = engine.execute_command(verify_cmd, "task-verify-1").await.unwrap();
    assert_eq!(
        verify_res.exit_code, 0,
        "Repaired project must pass all unit tests. Output:\nSTDOUT: {}\nSTDERR: {}",
        verify_res.stdout, verify_res.stderr
    );
    assert!(
        verify_res.stdout.contains("test result: ok. 2 passed; 0 failed")
            || verify_res.stdout.contains("2 passed; 0 failed")
    );

    // 10. Commit durable transaction
    tx.commit().unwrap();
    TaskPlanner::update_step_status(&mut plan, "repair-multiply", TaskStatus::Completed, None).unwrap();
    assert!(TaskPlanner::is_complete(&plan));
}
