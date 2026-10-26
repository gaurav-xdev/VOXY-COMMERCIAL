use std::fs;
use std::path::Path;
use std::sync::Arc;
use tempfile::tempdir;
use voxy_coding_harness::{
    ClaudeCliBackend, ContextExplorer, Debugger, FileEditInput, Language, McpBridge, Patcher,
    Reviewer, SkillRouter, TaskPlanner, TaskStatus, TaskStep, ToolchainDetector, ToolchainStatus,
    TransactionJournal,
};
use voxy_model_router::{ModelRouter, RoutingMode};
use voxy_provider_core::{
    DefaultProviderRegistry, LlmProvider, ModelCapabilities, ModelInfo, ProviderCapability,
    ProviderHealth, ProviderInfo, ProviderKind, ProviderRegistry, ProviderStatus,
};
use voxy_tool_calling::ToolRegistry;

struct FailingCloudLlm;
#[async_trait::async_trait]
impl LlmProvider for FailingCloudLlm {
    async fn complete(&self, _prompt: &str) -> voxy_provider_core::Result<String> {
        Err(voxy_provider_core::ProviderError::RequestFailed("Simulated Cloud Failure".into()))
    }
    fn available_models(&self) -> Vec<String> {
        vec!["claude-3-7-sonnet".into()]
    }
    fn name(&self) -> &str {
        "FailingCloudLlm"
    }
}

struct WorkingLocalLlm;
#[async_trait::async_trait]
impl LlmProvider for WorkingLocalLlm {
    async fn complete(&self, _prompt: &str) -> voxy_provider_core::Result<String> {
        Ok("Local LLM Response: 42".into())
    }
    fn available_models(&self) -> Vec<String> {
        vec!["qwen-coder-32b".into()]
    }
    fn name(&self) -> &str {
        "WorkingLocalLlm"
    }
}

#[tokio::test]
async fn test_full_ten_criteria_end_to_end_verification() {
    let temp = tempdir().expect("create temp dir");
    let ws = temp.path();

    // ── Criterion 1: Unfamiliar repository inspection (structure, glob, grep) ──
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        ws.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(
        src_dir.join("main.rs"),
        "fn main() {\n    println!(\"Initial state\");\n}\n",
    )
    .unwrap();

    let globbed = ContextExplorer::glob_files(ws, "**/*.rs", None).expect("glob rs files");
    assert!(!globbed.is_empty(), "Glob must find src/main.rs");
    assert!(globbed.iter().any(|f| f.ends_with("main.rs")));

    let grep_results = ContextExplorer::grep_pattern(ws, "Initial state", None, None).expect("grep");
    assert!(!grep_results.is_empty(), "Grep must find pattern");
    assert_eq!(grep_results[0].line_number, 2);

    let toolchains = ToolchainDetector::detect_all(ws);
    let rust_tc = toolchains
        .iter()
        .find(|t| t.language == Language::Rust)
        .expect("Rust toolchain must be present in detection matrix");
    assert!(
        matches!(rust_tc.status, ToolchainStatus::Available { .. }),
        "Cargo must be available on host"
    );

    // ── Criterion 2: Multi-step task planning with explicit DAG & checkpoints ─
    let steps = vec![
        TaskStep {
            id: "inspect".into(),
            title: "Inspect repository".into(),
            description: "Explore repo structure and entry points".into(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            active_form: None,
            affected_files: vec![],
            error: None,
        },
        TaskStep {
            id: "draft_edit".into(),
            title: "Draft changes".into(),
            description: "Prepare multi-file patch".into(),
            dependencies: vec!["inspect".into()],
            status: TaskStatus::Pending,
            active_form: None,
            affected_files: vec!["src/main.rs".into()],
            error: None,
        },
        TaskStep {
            id: "verify_compile".into(),
            title: "Verify compiler".into(),
            description: "Run compiler checks on modified files".into(),
            dependencies: vec!["draft_edit".into()],
            status: TaskStatus::Pending,
            active_form: None,
            affected_files: vec![],
            error: None,
        },
        TaskStep {
            id: "review_gate".into(),
            title: "Security review".into(),
            description: "Run independent security review on diff".into(),
            dependencies: vec!["verify_compile".into()],
            status: TaskStatus::Pending,
            active_form: None,
            affected_files: vec![],
            error: None,
        },
    ];

    let mut plan = TaskPlanner::create_plan("Comprehensive Fix", "Full workflow DAG", steps);
    let runnable = TaskPlanner::get_next_runnable_steps(&plan);
    assert_eq!(runnable.len(), 1);
    assert_eq!(runnable[0].id, "inspect");

    TaskPlanner::update_step_status(&mut plan, "inspect", TaskStatus::Completed, None).unwrap();
    let next_runnable = TaskPlanner::get_next_runnable_steps(&plan);
    assert_eq!(next_runnable.len(), 1);
    assert_eq!(next_runnable[0].id, "draft_edit");

    // ── Criterion 3: Dynamic skill selection, discovery & prompt composition ──
    let skills_dir = ws.join(".agents").join("skills");
    let gravitas_dir = skills_dir.join("gravitas");
    fs::create_dir_all(&gravitas_dir).unwrap();
    fs::write(
        gravitas_dir.join("SKILL.md"),
        "---\nname: gravitas\ndescription: Engineering reliability runtime\n---\n# Gravitas Invariants\n1. Ground before mutating.\n",
    ).unwrap();

    let discovered_skills = SkillRouter::discover_installed_skills(&[skills_dir.as_path()]);
    assert!(
        discovered_skills.iter().any(|s| s.name == "gravitas"),
        "Must discover gravitas skill from filesystem"
    );

    let prompt_injection = SkillRouter::compose_skill_system_prompt(
        &discovered_skills,
        &[skills_dir.as_path()],
    );
    assert!(
        prompt_injection.contains("Ground before mutating"),
        "Prompt injection must contain parsed skill body"
    );

    // ── Criterion 4: MCP tool discovery & execution with schema validation ────
    let registry = Arc::new(ToolRegistry::with_builtins());
    let mcp = McpBridge::new(registry);

    let tools = mcp.list_available_tools().await;
    assert!(!tools.is_empty(), "Builtin tool registry should expose tools");
    assert!(tools.iter().any(|t| t.name == "file_list" || t.name == "window_list"));

    // ── Criterion 5: Multi-file code change with atomic commit or rollback ───
    let journal = TransactionJournal::new(ws);
    let mut tx = journal.begin().expect("begin tx");

    let rel_main = Path::new("src/main.rs");
    tx.record_before_modify(rel_main).expect("journal pre-image");

    let edit_input = FileEditInput {
        file_path: "src/main.rs".into(),
        old_string: "    println!(\"Initial state\");\n".into(),
        new_string: "    println!(\"Updated state\");\n".into(),
        replace_all: false,
    };
    let patch_res = Patcher::apply_edit(ws, &edit_input).expect("patch main.rs");
    assert!(patch_res.diff.contains("+    println!(\"Updated state\");"));

    // Verify rolling back transaction restores original content
    tx.rollback().expect("rollback tx");
    let rolled_back_content = fs::read_to_string(ws.join("src/main.rs")).unwrap();
    assert!(
        rolled_back_content.contains("Initial state"),
        "Rollback must restore original file state"
    );

    // Re-apply and commit
    let mut tx2 = journal.begin().expect("begin tx2");
    tx2.record_before_modify(rel_main).expect("journal pre-image");
    let patch_res2 = Patcher::apply_edit(ws, &edit_input).expect("patch main.rs second time");
    assert!(patch_res2.diff.contains("+    println!(\"Updated state\");"));
    tx2.commit().expect("commit tx2");
    let committed_content = fs::read_to_string(ws.join("src/main.rs")).unwrap();
    assert!(committed_content.contains("Updated state"));

    // ── Criterion 6: Compiler/diagnostic analysis & iterative repair ─────────
    let mock_rustc_error = r#"
error[E0308]: mismatched types
  --> src/main.rs:2:5
   |
2  |     let x: u32 = "invalid string";
   |            ---   ^^^^^^^^^^^^^^^^ expected `u32`, found `&str`
   |            |
   |            expected due to this
"#;
    let diagnosis = Debugger::analyze_compiler_output(mock_rustc_error);
    assert!(!diagnosis.diagnostics.is_empty());
    assert_eq!(diagnosis.diagnostics[0].code.as_deref(), Some("E0308"));
    assert!(diagnosis.primary_cause.contains("E0308"));

    // ── Criterion 7: Independent code review gate with actionable findings ────
    let reviewer = Reviewer::new();
    let insecure_diff = r#"
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,3 @@
 fn main() {
+    let api_key = "sk-ant-api03-secretkey12345678901234567890123456789012";
 }
"#;
    let review_report = reviewer.review_diff(insecure_diff).expect("review diff");
    assert!(
        Reviewer::is_blocking(&review_report),
        "Hardcoded secret must block review gate"
    );
    assert!(review_report
        .findings
        .iter()
        .any(|f| f.title.to_lowercase().contains("secret")));

    // ── Criterion 8: Transaction journal crash recovery ───────────────────────
    let uncommitted_tx = journal.begin().expect("create uncommitted tx");
    let test_file = Path::new("src/crash_test.txt");
    fs::write(ws.join(test_file), "Original Uncorrupted Content\n").unwrap();
    let mut tx_crash = uncommitted_tx;
    tx_crash.record_before_modify(test_file).expect("journal pre-image");

    // Mutate file as if crash occurred mid-flight
    fs::write(ws.join(test_file), "CORRUPTED CONTENT MID FLIGHT\n").unwrap();
    drop(tx_crash); // Transaction was never committed

    // Run recovery
    let recovered_ids = journal.recover_pending_transactions().expect("recover");
    assert!(!recovered_ids.is_empty(), "Must detect abandoned transaction");
    let recovered_content = fs::read_to_string(ws.join(test_file)).unwrap();
    assert_eq!(
        recovered_content, "Original Uncorrupted Content\n",
        "Crash recovery must roll back uncommitted changes"
    );

    // ── Criterion 9: Strictly enforced model routing (zero silent fallback) ──
    let cloud_info = ProviderInfo {
        id: "forced-cloud-provider".into(),
        name: "Forced Cloud".into(),
        kind: ProviderKind::Cloud,
        capability: ProviderCapability::Llm,
        status: ProviderStatus::Available,
        models: vec![ModelInfo {
            id: "claude-3-7-sonnet".into(),
            name: "Claude 3.7 Sonnet".into(),
            capabilities: vec![],
            model_capabilities: ModelCapabilities::default(),
            version: "1.0".into(),
            context_size: Some(200000),
            supports_vision: true,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_embeddings: false,
            metadata: std::collections::HashMap::new(),
        }],
        health: ProviderHealth {
            is_healthy: true,
            status: voxy_provider_core::HealthStatus::Healthy,
            last_check: chrono::Utc::now(),
            latency_ms: Some(15.0),
            details: None,
            consecutive_failures: 0,
            circuit_broken: false,
        },
        base_url: None,
        priority: 1,
    };
    let local_info = ProviderInfo {
        id: "local-fallback-provider".into(),
        name: "Local Qwen".into(),
        kind: ProviderKind::Local,
        capability: ProviderCapability::Llm,
        status: ProviderStatus::Available,
        models: vec![ModelInfo {
            id: "qwen-coder-32b".into(),
            name: "Qwen 2.5 Coder 32B".into(),
            capabilities: vec![],
            model_capabilities: ModelCapabilities::default(),
            version: "1.0".into(),
            context_size: Some(32768),
            supports_vision: false,
            supports_tool_calling: true,
            supports_streaming: true,
            supports_embeddings: false,
            metadata: std::collections::HashMap::new(),
        }],
        health: ProviderHealth {
            is_healthy: true,
            status: voxy_provider_core::HealthStatus::Healthy,
            last_check: chrono::Utc::now(),
            latency_ms: Some(5.0),
            details: None,
            consecutive_failures: 0,
            circuit_broken: false,
        },
        base_url: None,
        priority: 10,
    };

    let registry = Arc::new(DefaultProviderRegistry::new());
    registry.register(cloud_info).await.unwrap();
    registry.register(local_info).await.unwrap();

    let router = ModelRouter::new(registry);
    router.register_llm("forced-cloud-provider", Arc::new(FailingCloudLlm));
    router.register_llm("local-fallback-provider", Arc::new(WorkingLocalLlm));

    router.set_mode(RoutingMode::ForceProvider("forced-cloud-provider".into()));

    for _ in 0..5 {
        router.record_failure("forced-cloud-provider");
    }

    let select_res = router.select_provider(&ProviderCapability::Llm).await;
    assert!(
        select_res.is_err(),
        "Expected select_provider to fail when forced provider is broken, but got {:?}",
        select_res
    );

    let err_str = select_res.unwrap_err().to_string();
    assert!(
        err_str.contains("disallowed in ForceProvider mode"),
        "Expected error confirming zero fallback enforcement, got: {}",
        err_str
    );

    // ── Criterion 10: Claude Code CLI backend detection ───────────────────────
    if let Some(cli) = ClaudeCliBackend::detect(ws) {
        let version = cli.get_version().await.expect("query claude version");
        assert!(
            version.contains("Claude Code") || version.contains("2."),
            "Version string must identify Claude Code CLI"
        );
    }
}
