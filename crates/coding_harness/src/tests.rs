use super::*;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;
use voxy_security::approval::ApprovalBroker;

#[tokio::test]
async fn test_atomic_file_edit_success_and_conflict() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let file = ws.join("src").join("main.rs");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "fn main() {\n    println!(\"Hello\");\n}\n").unwrap();

    let edit = FileEditInput {
        file_path: "src/main.rs".into(),
        old_string: "println!(\"Hello\");".into(),
        new_string: "println!(\"Hello World\");".into(),
        replace_all: false,
    };

    let res = Patcher::apply_edit(ws, &edit).unwrap();
    assert!(res.applied);
    assert_eq!(res.occurrences, 1);
    assert!(res.diff.contains("-    println!(\"Hello\");"));
    assert!(res.diff.contains("+    println!(\"Hello World\");"));

    let updated = fs::read_to_string(&file).unwrap();
    assert!(updated.contains("Hello World"));

    // Conflict: old_string not found
    let err_edit = FileEditInput {
        file_path: "src/main.rs".into(),
        old_string: "non_existent()".into(),
        new_string: "replacement()".into(),
        replace_all: false,
    };
    assert!(Patcher::apply_edit(ws, &err_edit).is_err());
}

#[tokio::test]
async fn test_transaction_journal_commit_and_rollback() {
    let dir = tempdir().unwrap();
    let ws = dir.path();
    let engine = CodingEngine::new(ws);

    let file_a = ws.join("a.txt");
    let file_b = ws.join("b.txt");
    fs::write(&file_a, "Original A").unwrap();
    fs::write(&file_b, "Original B").unwrap();

    let mut tx = engine.begin_transaction().unwrap();

    let edit_a = FileEditInput {
        file_path: "a.txt".into(),
        old_string: "Original A".into(),
        new_string: "Modified A".into(),
        replace_all: false,
    };
    engine.apply_file_edit(&mut tx, &edit_a).unwrap();

    let edit_b = FileEditInput {
        file_path: "b.txt".into(),
        old_string: "Original B".into(),
        new_string: "Modified B".into(),
        replace_all: false,
    };
    engine.apply_file_edit(&mut tx, &edit_b).unwrap();

    assert_eq!(fs::read_to_string(&file_a).unwrap(), "Modified A");
    assert_eq!(fs::read_to_string(&file_b).unwrap(), "Modified B");

    // Rollback transaction
    tx.rollback().unwrap();

    assert_eq!(fs::read_to_string(&file_a).unwrap(), "Original A");
    assert_eq!(fs::read_to_string(&file_b).unwrap(), "Original B");
}

#[tokio::test]
async fn test_startup_crash_recovery() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let file = ws.join("important.rs");
    fs::write(&file, "pub fn safe() -> bool { true }").unwrap();

    // Start a transaction and modify file, but "crash" without committing or rolling back
    {
        let engine = CodingEngine::new(ws);
        let mut tx = engine.begin_transaction().unwrap();
        let edit = FileEditInput {
            file_path: "important.rs".into(),
            old_string: "true".into(),
            new_string: "false".into(),
            replace_all: false,
        };
        engine.apply_file_edit(&mut tx, &edit).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "pub fn safe() -> bool { false }"
        );
        // tx is dropped here without commit -> simulating crash while Active
    }

    // Now start a fresh engine on the same workspace and run initialize()
    let recovered_engine = CodingEngine::new(ws);
    let recovered_txs = recovered_engine.initialize().unwrap();

    assert_eq!(recovered_txs.len(), 1);
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "pub fn safe() -> bool { true }",
        "File should be cleanly restored to its pre-crash state"
    );
}

#[tokio::test]
async fn test_context_explorer_glob_and_grep() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("lib.rs"),
        "pub fn test_alpha() {}\npub fn test_beta() {}\n",
    )
    .unwrap();
    fs::write(src.join("app.rs"), "pub fn run() {}\n").unwrap();

    let matched = ContextExplorer::glob_files(ws, "*.rs", None).unwrap();
    assert_eq!(matched.len(), 2);
    assert!(matched.contains(&"src/lib.rs".to_string()));
    assert!(matched.contains(&"src/app.rs".to_string()));

    let greps = ContextExplorer::grep_pattern(ws, "test_\\w+", None, None).unwrap();
    assert_eq!(greps.len(), 2);
    assert_eq!(greps[0].line_number, 1);
    assert_eq!(greps[1].line_number, 2);
}

#[tokio::test]
async fn test_task_planner_dag() {
    let step1 = TaskStep {
        id: "step-1".into(),
        title: "Setup schema".into(),
        description: "Create tables".into(),
        status: TaskStatus::Pending,
        active_form: Some("Creating tables".into()),
        dependencies: vec![],
        affected_files: vec!["schema.sql".into()],
        error: None,
    };
    let step2 = TaskStep {
        id: "step-2".into(),
        title: "Add queries".into(),
        description: "Write queries".into(),
        status: TaskStatus::Pending,
        active_form: Some("Writing queries".into()),
        dependencies: vec!["step-1".into()],
        affected_files: vec!["queries.sql".into()],
        error: None,
    };

    let mut plan = TaskPlanner::create_plan("SQL Task", "Plan description", vec![step1, step2]);

    let runnable = TaskPlanner::get_next_runnable_steps(&plan);
    assert_eq!(runnable.len(), 1);
    assert_eq!(runnable[0].id, "step-1");

    TaskPlanner::update_step_status(&mut plan, "step-1", TaskStatus::Completed, None).unwrap();

    let runnable2 = TaskPlanner::get_next_runnable_steps(&plan);
    assert_eq!(runnable2.len(), 1);
    assert_eq!(runnable2[0].id, "step-2");
}

#[tokio::test]
async fn test_toolchain_detection_matrix() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let rust = ToolchainDetector::detect_rust(ws);
    assert!(matches!(rust.status, ToolchainStatus::Available { .. }));

    let csharp = ToolchainDetector::detect_csharp(ws);
    assert!(matches!(
        csharp.status,
        ToolchainStatus::MissingToolchain { .. }
    ));
}

#[tokio::test]
async fn test_process_runner_bounded_execution() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let input = CommandExecutionInput {
        command: "echo 'hello windows job object'".into(),
        cwd: None,
        timeout_ms: Some(10_000),
        description: Some("Test command".into()),
        run_in_background: false,
    };

    let res = ProcessRunner::run_command(&input, ws).await.unwrap();
    assert_eq!(res.exit_code, 0);
    assert!(res.stdout.contains("hello windows job object"));
    assert!(!res.timed_out);
}

#[tokio::test]
async fn test_human_approval_flow() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let broker = Arc::new(ApprovalBroker::new());
    let mut rx = broker.subscribe_requests();
    let engine = CodingEngine::new(ws).with_approval_broker(broker.clone());

    // Spawn handler that resolves the request when received
    let broker_clone = broker.clone();
    tokio::spawn(async move {
        if let Ok(req) = rx.recv().await {
            let _ = broker_clone.resolve(
                req.id,
                false,
                Some("Deletion of critical files rejected by security officer".into()),
            );
        }
    });

    let input = CommandExecutionInput {
        command: "rm -rf critical_data/".into(),
        cwd: None,
        timeout_ms: Some(5000),
        description: Some("Destructive delete".into()),
        run_in_background: false,
    };

    let res = engine.execute_command(input, "session-123").await;
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("rejected"),
        "Expected rejection error, got: {}",
        err_str
    );
}

#[tokio::test]
async fn test_symbol_indexer_and_repository_map() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    // Create a Rust file
    let rs_file = ws.join("src").join("router.rs");
    fs::create_dir_all(rs_file.parent().unwrap()).unwrap();
    fs::write(
        &rs_file,
        r#"
/// Core router configuration.
pub struct RouterConfig {
    pub mode: String,
}

pub enum RoutingMode {
    Auto,
    Manual,
}

/// Dispatches an inference request.
pub async fn dispatch_request(req: &str) -> bool {
    true
}
"#,
    )
    .unwrap();

    // Create a TypeScript file
    let ts_file = ws.join("src").join("client.ts");
    fs::write(
        &ts_file,
        r#"
export interface ClientOptions {
    timeoutMs: number;
}

export class ApiClient {
    constructor(private options: ClientOptions) {}
}

export async function fetchHealth(): Promise<boolean> {
    return true;
}
"#,
    )
    .unwrap();

    let map = SymbolIndexer::index_workspace(ws, None).unwrap();
    assert!(map.total_symbols >= 5);

    let search_res = SymbolIndexer::search_symbols(&map, "dispatch_request");
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].name, "dispatch_request");
    assert_eq!(search_res[0].kind, SymbolKind::Function);
    assert!(search_res[0]
        .doc_comment
        .as_ref()
        .unwrap()
        .contains("Dispatches an inference request"));

    let compact = map.render_compact();
    assert!(compact.contains("router.rs"));
    assert!(compact.contains("struct RouterConfig"));
    assert!(compact.contains("fn pub async fn dispatch_request"));
    assert!(compact.contains("client.ts"));
    assert!(compact.contains("interface ClientOptions"));
}

#[tokio::test]
async fn test_token_budget_and_targeted_context_selector() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let big_file = ws.join("src").join("large_module.rs");
    fs::create_dir_all(big_file.parent().unwrap()).unwrap();

    let mut body = String::new();
    body.push_str("pub struct LargeModule {\n");
    for i in 0..100 {
        body.push_str(&format!("    pub field_{}: u64,\n", i));
    }
    body.push_str("}\n\n");
    body.push_str("/// Core execution engine entrypoint.\n");
    body.push_str("pub fn execute_engine_workflow() -> bool {\n");
    for i in 0..50 {
        body.push_str(&format!("    println!(\"step {}\");\n", i));
    }
    body.push_str("    true\n}\n");

    fs::write(&big_file, &body).unwrap();

    let repo_map = SymbolIndexer::index_workspace(ws, None).unwrap();

    let budget = BudgetConfig {
        max_prompt_tokens: 1000,
        max_repo_map_tokens: 200,
        max_file_context_tokens: 400,
        reserve_completion_tokens: 400,
    };

    let (context_str, metrics) = TargetedContextSelector::select_context(
        ws,
        &repo_map,
        "How does execute_engine_workflow work in large_module?",
        &budget,
    )
    .unwrap();

    assert!(context_str.contains("<relevant_repository_symbols>"));
    assert!(context_str.contains("execute_engine_workflow"));
    assert!(context_str.contains("<relevant_file_excerpts>"));
    assert!(metrics.prompt_tokens_estimated <= budget.max_prompt_tokens);
    assert!(metrics.tokens_saved_by_compression > 0);

    // Verify chunk deduplication
    let chunk1 = SelectedContextChunk {
        file_path: "src/a.rs".into(),
        start_line: 1,
        end_line: 20,
        content: "chunk1".into(),
        token_estimate: 10,
        relevance_score: 5,
    };
    let chunk2 = SelectedContextChunk {
        file_path: "src/a.rs".into(),
        start_line: 15,
        end_line: 35,
        content: "chunk2".into(),
        token_estimate: 10,
        relevance_score: 5,
    };
    let deduplicated = ContextCompressor::deduplicate_chunks(vec![chunk1, chunk2]);
    assert_eq!(deduplicated.len(), 1, "Overlapping chunks must be merged");
    assert_eq!(deduplicated[0].start_line, 1);
    assert_eq!(deduplicated[0].end_line, 35);
}
