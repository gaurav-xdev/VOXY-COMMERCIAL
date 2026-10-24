//! End-to-end integration test demonstrating autonomous skill selection, instruction loading,
//! multi-agent role distribution, engineering check verification, and evidence-backed completion.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use voxy_skills::{
    OsmooSkillsDiscoverTool, OsmooSkillsLoadInstructionsTool, OsmooSkillsVerifyExecutionTool,
    SkillRuntime, TaskContext,
};
use voxy_tool_calling::{Tool, ToolContext, ToolRegistry};

#[tokio::test]
async fn test_autonomous_coding_task_with_skills_lifecycle() {
    let registry = Arc::new(ToolRegistry::with_builtins());
    let runtime = SkillRuntime::new(Arc::clone(&registry));
    let engine = runtime.activation_engine();
    let ctx = ToolContext::new("autonomous-agent-session");

    // =========================================================================
    // STEP 1: Discovery via Tool Interface
    // =========================================================================
    let discover_tool = OsmooSkillsDiscoverTool::new(Arc::clone(engine));
    let disc_res = discover_tool
        .execute(
            serde_json::json!({
                "language": "rust",
                "role": "implementer"
            }),
            &ctx,
        )
        .await
        .expect("Discovery should execute");
    assert!(disc_res.success);
    let matched = disc_res.data["total_matched"].as_u64().unwrap();
    assert!(matched >= 1, "Must find at least 1 Rust implementer skill");

    // =========================================================================
    // STEP 2: Autonomous Task Evaluation & Smallest Effective Set Selection
    // =========================================================================
    let task = TaskContext::new(
        "task-eng-upgrade-2026",
        "Investigate memory leak and race condition in async tokio channel worker, fix bug with TDD",
        PathBuf::from("."),
    );

    let active_ctx = engine.evaluate_task(&task);

    // Verify Smallest Effective Set (Ponytail & Super-Code rules: max 4 skills)
    assert!(active_ctx.active_capabilities.len() <= 4);
    assert!(!active_ctx.active_capabilities.is_empty());

    let active_ids: Vec<_> = active_ctx.active_capabilities.iter().map(|c| c.id.as_str()).collect();
    println!("Active skills selected: {:?}", active_ids);

    // Concurrency/Rust/Debugging skills selected
    assert!(
        active_ids.iter().any(|id| *id == "sota:sota-rust" || *id == "sota:sota-async-concurrency"),
        "Must select SOTA Rust or Async Concurrency"
    );
    assert!(
        active_ids.iter().any(|id| *id == "addy:debugging-and-error-recovery" || *id == "addy:test-driven-development"),
        "Must select Addy Debugging or TDD"
    );

    // Irrelevant languages must NOT be activated
    assert!(!active_ids.iter().any(|id| id.contains("python") || id.contains("php") || id.contains("ruby")));

    // =========================================================================
    // STEP 3: Multi-Agent Role Guidance Extraction
    // =========================================================================
    // Architect receives high-level design guidelines
    let architect_skills: Vec<_> = active_ctx
        .active_capabilities
        .iter()
        .filter(|c| c.target_roles.contains(&"architect".to_string()))
        .collect();
    // Implementer receives concrete implementation patterns
    let implementer_skills: Vec<_> = active_ctx
        .active_capabilities
        .iter()
        .filter(|c| c.target_roles.contains(&"implementer".to_string()))
        .collect();
    // QA receives testing protocols
    let qa_skills: Vec<_> = active_ctx
        .active_capabilities
        .iter()
        .filter(|c| c.target_roles.contains(&"qa".to_string()))
        .collect();

    assert!(!implementer_skills.is_empty(), "Implementer must receive role guidance");
    println!("Implementer guidance count: {}", implementer_skills.len());
    println!("QA guidance count: {}", qa_skills.len());
    println!("Architect guidance count: {}", architect_skills.len());

    // =========================================================================
    // STEP 4: Instruction Loading with Sanitization & Provenance
    // =========================================================================
    let load_tool = OsmooSkillsLoadInstructionsTool::new(Arc::clone(engine));
    let load_res = load_tool
        .execute(
            serde_json::json!({
                "skill_ids": active_ids
            }),
            &ctx,
        )
        .await
        .expect("Should load instructions");
    assert!(load_res.success);

    // =========================================================================
    // STEP 5: Execution Tracking & Engineering Checks Verification
    // =========================================================================
    let store = Arc::new(Mutex::new(Vec::new()));
    let verify_tool = OsmooSkillsVerifyExecutionTool::new(Arc::clone(engine), Arc::clone(&store));

    // Initialize execution record in store
    let record = engine.create_execution_record(&task, &active_ctx);
    assert!(!record.all_passed, "Checks must initially be unverified (pending)");
    assert!(!record.required_checks.is_empty(), "Active skills must have required checks");

    {
        let mut l = store.lock().await;
        l.push(record.clone());
    }

    // Agent executes checks and provides real verification evidence
    for check in &record.required_checks {
        let verify_res = verify_tool
            .execute(
                serde_json::json!({
                    "task_id": "task-eng-upgrade-2026",
                    "skill_id": check.skill_id,
                    "check_name": check.check_name,
                    "status": "passed",
                    "evidence": format!("Verified: {} satisfied by test suite", check.check_name)
                }),
                &ctx,
            )
            .await
            .expect("Verification recording must succeed");
        assert!(verify_res.success);
    }

    // Verify all requirements are satisfied
    let final_store = store.lock().await;
    let final_record = final_store.iter().find(|r| r.task_id == "task-eng-upgrade-2026").unwrap();
    assert!(final_record.all_passed, "All engineering checks must pass before completion");
    assert!(final_record.completed_at.is_some(), "Must have completed_at timestamp");
}
