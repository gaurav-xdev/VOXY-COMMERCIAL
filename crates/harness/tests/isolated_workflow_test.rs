use std::time::Duration;
use voxy_harness::{
    DiagnosticParser, PatchEngine, PatchTransaction, RepositoryIndexer, SandboxedRunner,
};

fn setup_isolated_fixture(base: &std::path::Path) {
    let src_dir = base.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();

    let cargo_toml = "[package]\nname = \"isolated-calc\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\n";
    std::fs::write(base.join("Cargo.toml"), cargo_toml).unwrap();

    let buggy_lib = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_add() {\n        assert_eq!(add(2, 3), 5);\n    }\n}\n";
    std::fs::write(src_dir.join("lib.rs"), buggy_lib).unwrap();
}

#[tokio::test]
async fn test_isolated_coding_harness_workflow() {
    let tmp = tempfile::tempdir().expect("Failed to create isolated temp dir");
    let repo_root = tmp.path().to_path_buf();
    setup_isolated_fixture(&repo_root);

    // Phase 1: Repository Discovery & Indexing
    let mut indexer = RepositoryIndexer::new(&repo_root);
    let count = indexer.index_repository().expect("Indexing should succeed");
    assert!(count >= 2, "Should index Cargo.toml and lib.rs");

    let symbols = indexer.find_symbols("add");
    assert!(!symbols.is_empty(), "Should find function 'add'");
    assert!(symbols.iter().any(|s| s.name == "add"), "Should find symbol 'add'");

    // Phase 2: Sandboxed Command Execution & Reproduction of Failure
    let runner = SandboxedRunner::new(&repo_root);
    let test_run_1 = runner
        .run_command("cargo", &["test"], None, Duration::from_secs(45))
        .await
        .expect("Sandboxed cargo test should execute");

    assert!(!test_run_1.was_success, "Test should initially fail due to intentional bug");
    assert!(
        test_run_1.stdout.contains("FAILED") || test_run_1.stderr.contains("FAILED"),
        "Failure output should report test failure"
    );

    // Phase 3: Diagnostic Parsing
    let diagnostics = DiagnosticParser::parse(&format!("{}\n{}", test_run_1.stdout, test_run_1.stderr));
    let _ = diagnostics;

    // Phase 4: Atomic Patch Application
    let fixed_content = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_add() {\n        assert_eq!(add(2, 3), 5);\n    }\n}\n";

    let mut patch_engine = PatchEngine::new(&repo_root);
    let diff = patch_engine
        .apply_modification("src/lib.rs", fixed_content)
        .expect("Patch application should succeed");

    assert!(diff.contains("+    a + b"), "Diff must reflect the fix");
    assert!(diff.contains("-    a - b"), "Diff must remove the bug");

    // Phase 5: Verification of Fix via Clean Re-test
    let test_run_2 = runner
        .run_command("cargo", &["test"], None, Duration::from_secs(45))
        .await
        .expect("Sandboxed cargo test should execute after fix");

    assert!(
        test_run_2.was_success,
        "Test must pass after patch! Stderr: {}, Stdout: {}",
        test_run_2.stderr,
        test_run_2.stdout
    );
    assert!(test_run_2.stdout.contains("test result: ok. 1 passed"));

    // Phase 6: Rollback Verification
    let mut tx = PatchTransaction::new(&repo_root);
    tx.add_change("src/lib.rs", "broken content");
    tx.apply().expect("Transaction apply should work");
    assert_eq!(
        std::fs::read_to_string(repo_root.join("src/lib.rs")).unwrap(),
        "broken content"
    );
    let rolled_back_count = tx.rollback();
    assert_eq!(rolled_back_count, 1, "Should roll back 1 file");
    assert_eq!(
        std::fs::read_to_string(repo_root.join("src/lib.rs")).unwrap(),
        fixed_content
    );

    // Phase 7: Boundary Violation Protection
    let boundary_err = patch_engine.apply_modification("../../escaped.rs", "malicious");
    assert!(boundary_err.is_err(), "Patch engine must reject escaping paths");

    // Phase 8: Protected File Modification Rejection
    let protected_err = patch_engine.apply_modification(".git/config", "malicious");
    assert!(protected_err.is_err(), "Patch engine must reject protected files");
}
