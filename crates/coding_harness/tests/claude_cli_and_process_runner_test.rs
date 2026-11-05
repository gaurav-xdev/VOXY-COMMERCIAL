use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::tempdir;
use voxy_coding_harness::{ClaudeCliBackend, ProcessExecutionInput, ProcessRunner};

/// P0 Test: Verify that ProcessRunner preserves argument integrity without shell interpolation.
/// Untrusted prompts containing quotes, spaces, newlines, Unicode, and shell metacharacters
/// must reach the process intact without being evaluated by cmd or PowerShell.
#[tokio::test]
async fn test_process_runner_safe_argument_integrity() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let echo_script = ws.join("echo_args.py");
    fs::write(
        &echo_script,
        r#"
import sys
import json
print(json.dumps(sys.argv[1:]))
"#,
    )
    .unwrap();

    let tricky_args = vec![
        "simple_arg".to_string(),
        "argument with spaces and tabs\t".to_string(),
        "arg \"with double quotes\" and \\backslashes\\".to_string(),
        "🦀 Unicode: こんにちは & 世界 -- UTF-8 test".to_string(),
        "line1\nline2\r\nline3".to_string(),
        "$ENV:SECRET_TOKEN & calc.exe | dir ; rm -rf / > nul < test ^ %PATH%".to_string(),
    ];

    let mut full_args = vec![echo_script.to_string_lossy().to_string()];
    full_args.extend(tricky_args.clone());

    let input = ProcessExecutionInput {
        program: PathBuf::from("python"),
        args: full_args,
        cwd: Some(ws.to_path_buf()),
        timeout_ms: Some(15_000),
        description: Some("Echo args test".into()),
        stdin: None,
        require_job_object: true,
    };

    let result = ProcessRunner::run_process(&input, ws).await.unwrap();
    assert_eq!(result.exit_code, 0, "Python echo script must succeed");

    let received_args: Vec<String> = serde_json::from_str(result.stdout.trim()).unwrap();
    assert_eq!(
        received_args, tricky_args,
        "Arguments received by process must match tricky input exactly, with zero shell expansion"
    );
}

/// P0 Test: Verify bounded timeout execution and child termination.
#[tokio::test]
async fn test_process_runner_timeout_and_termination() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let sleep_script = ws.join("sleep_long.py");
    fs::write(
        &sleep_script,
        r#"
import time
time.sleep(10)
"#,
    )
    .unwrap();

    let input = ProcessExecutionInput {
        program: PathBuf::from("python"),
        args: vec![sleep_script.to_string_lossy().to_string()],
        cwd: Some(ws.to_path_buf()),
        timeout_ms: Some(500), // 500ms timeout
        description: Some("Timeout test".into()),
        stdin: None,
        require_job_object: true,
    };

    let result = ProcessRunner::run_process(&input, ws).await.unwrap();
    assert!(result.timed_out, "Execution must report timeout");
    assert_eq!(
        result.exit_code, -1,
        "Timed out process returns -1 exit code"
    );
    assert!(result.duration_ms < 3000, "Timeout must abort promptly");
}

/// P1 Test: Verify ClaudeCliBackend execution against a controlled mock CLI.
/// Validates argument passing, working directory, exit code handling, and timeout behavior.
#[tokio::test]
async fn test_claude_cli_backend_mock_execution() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let mock_cli = ws.join("mock_claude.py");
    fs::write(
        &mock_cli,
        r#"
import sys
import os

args = sys.argv[1:]
if "--version" in args:
    print("2.1.215 (Claude Code)")
    sys.exit(0)

if "-p" in args:
    idx = args.index("-p")
    prompt = args[idx + 1] if idx + 1 < len(args) else ""
    if prompt == "FAIL_NOW":
        sys.stderr.write("Simulated CLI fatal error\n")
        sys.exit(42)
    elif prompt == "SLOW_NOW":
        import time
        time.sleep(10)
        sys.exit(0)
    else:
        cwd = os.getcwd()
        print(f"[CLAUDE_TASK_SUCCESS] CWD={cwd} PROMPT={prompt}")
        sys.exit(0)

sys.stderr.write("Unknown arguments\n")
sys.exit(1)
"#,
    )
    .unwrap();

    let mock_cmd = if cfg!(windows) {
        let cmd_path = ws.join("mock_claude.cmd");
        fs::write(
            &cmd_path,
            format!("@echo off\r\npython \"{}\" %*\r\n", mock_cli.display()),
        )
        .unwrap();
        cmd_path
    } else {
        mock_cli.clone()
    };

    let _session_backend =
        ClaudeCliBackend::new(&mock_cmd, ws).with_session_id("test-session-42".into());

    // 1. Query version
    // Create dedicated version mock backend pointing to mock_cmd
    let version_backend = ClaudeCliBackend::new(&mock_cmd, ws);
    let version = version_backend.get_version().await.unwrap();
    assert_eq!(version, "2.1.215 (Claude Code)");

    // 2. Execute task with tricky shell metacharacters
    let untrusted_prompt = "Refactor auth: $ENV:TOKEN ; format C: & echo `whoami`";
    let task_result = version_backend
        .execute_task(untrusted_prompt)
        .await
        .unwrap();
    assert!(task_result.contains("[CLAUDE_TASK_SUCCESS]"));
    assert!(
        task_result.contains(untrusted_prompt),
        "Prompt must be preserved verbatim"
    );
    assert!(task_result.contains(&ws.to_string_lossy().to_string()));

    // 3. Error handling: non-zero exit code
    let fail_res = version_backend.execute_task("FAIL_NOW").await;
    assert!(fail_res.is_err(), "Must error on non-zero exit code");
    let err_msg = fail_res.unwrap_err().to_string();
    assert!(err_msg.contains("code 42"));
    assert!(err_msg.contains("Simulated CLI fatal error"));

    // 4. Timeout handling
    let timeout_backend = version_backend.with_timeout(Duration::from_millis(500));
    let timeout_res = timeout_backend.execute_task("SLOW_NOW").await;
    assert!(timeout_res.is_err(), "Must error on timeout");
    assert!(timeout_res.unwrap_err().to_string().contains("timed out"));
}

/// Verify live host Claude Code CLI detection if present on the host environment.
#[tokio::test]
async fn test_claude_cli_live_host_detection() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    if let Some(backend) = ClaudeCliBackend::detect(ws) {
        println!(
            "Live Claude Code CLI detected at: {}",
            backend.binary_path().display()
        );
        let version_res = backend.get_version().await;
        if let Ok(v) = version_res {
            assert!(v.contains("Claude Code"), "Live CLI version string: {}", v);
        }
    } else {
        println!("Live Claude Code CLI not found on host (mock tests verify backend correctness)");
    }
}
