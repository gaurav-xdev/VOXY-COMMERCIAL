use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use voxy_coding_harness::{McpBridge, McpStdioClient, SkillRouter};
use voxy_tool_calling::ToolRegistry;

#[tokio::test]
async fn test_mcp_bridge_and_skill_routing() {
    // 1. Verify dynamic skill suggestions based on file extensions
    let rs_file = Path::new("src/main.rs");
    let py_file = Path::new("app/server.py");
    let ts_file = Path::new("frontend/index.ts");
    let go_file = Path::new("pkg/service.go");

    let rs_skills = SkillRouter::suggest_skills_for_file(rs_file);
    assert!(rs_skills.iter().any(|s| s.name == "clean-code"));
    assert!(rs_skills.iter().any(|s| s.name == "tdd-workflow"));

    let py_skills = SkillRouter::suggest_skills_for_file(py_file);
    assert!(py_skills.iter().any(|s| s.name == "python-pro"));

    let ts_skills = SkillRouter::suggest_skills_for_file(ts_file);
    assert!(ts_skills.iter().any(|s| s.name == "typescript-expert"));

    let go_skills = SkillRouter::suggest_skills_for_file(go_file);
    assert!(go_skills.iter().any(|s| s.name == "golang-pro"));

    // 2. Verify native tool adapter discovery
    let registry = Arc::new(ToolRegistry::with_builtins());
    let bridge = McpBridge::new(registry);

    let tools = bridge.list_available_tools().await;
    assert!(
        !tools.is_empty(),
        "Builtin tool registry should expose tools"
    );
    assert!(tools
        .iter()
        .any(|t| t.name == "file_list" || t.name == "window_list"));
}

/// P1 Test: Verify skill discovery across nested plugin layouts, e.g.:
/// `.gemini/config/plugins/<plugin_name>/skills/<skill_name>/SKILL.md`
/// and verify that instructions are loaded and injected into model prompts.
#[tokio::test]
async fn test_nested_plugin_skill_discovery_and_instruction_injection() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Setup nested plugin directory: root/plugins/agentic-tools/skills/gravitas-auditor/SKILL.md
    let nested_skill_dir = root
        .join("plugins")
        .join("agentic-tools")
        .join("skills")
        .join("gravitas-auditor");
    fs::create_dir_all(&nested_skill_dir).unwrap();

    let gravitas_skill_md = r#"---
name: gravitas-auditor
description: High-rigor engineering auditor enforcing evidence-backed facts and zero unverified claims
tags: [audit, verification, gravitas]
---
# Gravitas Auditor Guidelines
- Require empirical test logs.
- Never claim production readiness without verified test outputs.
- Test edge cases and bounded execution.
"#;
    fs::write(nested_skill_dir.join("SKILL.md"), gravitas_skill_md.trim()).unwrap();

    // Setup second skill in .agents/skills/tdd-workflow/SKILL.md
    let agents_skill_dir = root.join(".agents").join("skills").join("tdd-workflow");
    fs::create_dir_all(&agents_skill_dir).unwrap();
    let tdd_skill_md = r#"---
name: tdd-workflow
description: Test-Driven Development enforcing the RED-GREEN-REFACTOR cycle
tags: [testing, tdd]
---
# TDD Instructions
Write failing test first, make it pass with minimal code, then refactor.
"#;
    fs::write(agents_skill_dir.join("SKILL.md"), tdd_skill_md.trim()).unwrap();

    let search_roots = [root];

    // 1. Discover all installed skills across nested structures
    let discovered = SkillRouter::discover_installed_skills(&search_roots);
    assert_eq!(discovered.len(), 2, "Both nested skills must be discovered");
    assert!(discovered.iter().any(|s| s.name == "gravitas-auditor"));
    assert!(discovered.iter().any(|s| s.name == "tdd-workflow"));

    // 2. Select skills based on task prompt
    let audit_prompt =
        "Perform a high-rigor audit of repository security and produce an evidence-backed report";
    let selected = SkillRouter::select_skills_for_prompt(audit_prompt, &search_roots);
    assert!(!selected.is_empty(), "Prompt should match gravitas-auditor");
    assert_eq!(selected[0].name, "gravitas-auditor");

    // 3. Load skill instructions
    let instructions = SkillRouter::load_skill_instructions("gravitas-auditor", &search_roots);
    assert!(
        instructions.is_some(),
        "Skill instructions must be loaded from nested layout"
    );
    assert!(instructions
        .as_ref()
        .unwrap()
        .contains("Gravitas Auditor Guidelines"));

    // 4. Compose active skill system prompt injection
    let system_prompt = SkillRouter::compose_skill_system_prompt(&selected, &search_roots);
    assert!(system_prompt.contains("<active_engineering_skills>"));
    assert!(system_prompt.contains("## Skill: gravitas-auditor"));
    assert!(system_prompt.contains("Gravitas Auditor Guidelines"));
    assert!(
        system_prompt.contains("Never claim production readiness without verified test outputs")
    );
}

/// P1 Test: Controlled MCP Protocol Server Test.
/// Spawns a real Python process speaking Model Context Protocol JSON-RPC 2.0 over stdio.
/// Exercises initialization, tools/list, tools/call, protocol errors, and timeouts.
#[tokio::test]
async fn test_mcp_stdio_client_protocol_handshake_and_execution() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let mcp_server_script = ws.join("mock_mcp_server.py");
    fs::write(
        &mcp_server_script,
        r#"
import sys
import json

def send_response(resp):
    sys.stdout.write(json.dumps(resp) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except Exception as e:
        continue

    method = req.get("method")
    req_id = req.get("id")

    if method == "initialize":
        send_response({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "controlled-mock-mcp-server",
                    "version": "1.2.3"
                }
            }
        })
    elif method == "notifications/initialized":
        # Notification, no response needed
        pass
    elif method == "tools/list":
        send_response({
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "tools": [
                    {
                        "name": "query_database_schema",
                        "description": "Inspect SQLite database tables",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "table_name": {"type": "string"}
                            },
                            "required": ["table_name"]
                        }
                    }
                ]
            }
        })
    elif method == "tools/call":
        params = req.get("params", {})
        tool_name = params.get("name")
        args = params.get("arguments", {})

        if tool_name == "query_database_schema":
            table = args.get("table_name", "")
            if table == "users":
                send_response({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [
                            {"type": "text", "text": "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT);"}
                        ],
                        "isError": False
                    }
                })
            else:
                send_response({
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [
                            {"type": "text", "text": f"Table '{table}' not found"}
                        ],
                        "isError": True
                    }
                })
        elif tool_name == "slow_tool":
            import time
            time.sleep(10)
        else:
            send_response({
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {
                    "code": -32601,
                    "message": f"Method or tool '{tool_name}' not found"
                }
            })
"#,
    )
    .unwrap();

    let python_bin = std::env::var("PYTHON").unwrap_or_else(|_| {
        if std::process::Command::new("python")
            .arg("--version")
            .output()
            .is_ok()
        {
            "python".to_string()
        } else {
            "python3".to_string()
        }
    });
    let mut client = McpStdioClient::new(
        PathBuf::from(python_bin),
        vec![mcp_server_script.to_string_lossy().to_string()],
        Some(ws),
        Duration::from_secs(5),
    );

    // 1. Initialize
    let server_info = client.initialize().await.unwrap();
    assert_eq!(server_info.name, "controlled-mock-mcp-server");
    assert_eq!(server_info.version.as_deref(), Some("1.2.3"));
    assert_eq!(server_info.protocol_version.as_deref(), Some("2024-11-05"));

    // 2. Discover tools
    let tools = client.list_tools().await.unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "query_database_schema");
    assert_eq!(
        tools[0].description.as_deref(),
        Some("Inspect SQLite database tables")
    );

    // 3. Call tool with arguments
    let call_res = client
        .call_tool(
            "query_database_schema",
            serde_json::json!({ "table_name": "users" }),
        )
        .await
        .unwrap();

    assert!(!call_res.is_error);
    assert_eq!(call_res.content.len(), 1);
    assert!(call_res.content[0]
        .text
        .as_ref()
        .unwrap()
        .contains("CREATE TABLE users"));

    // 4. Protocol error for non-existent tool
    let err_res = client
        .call_tool("nonexistent_tool", serde_json::json!({}))
        .await;
    assert!(err_res.is_err(), "Must return error for non-existent tool");
    assert!(err_res.unwrap_err().to_string().contains("not found"));
}

/// P1 Test: Verify that McpStdioClient enforces bounded timeouts and kills process on hang.
#[tokio::test]
async fn test_mcp_stdio_client_timeout_handling() {
    let dir = tempdir().unwrap();
    let ws = dir.path();

    let hang_script = ws.join("hang_server.py");
    fs::write(
        &hang_script,
        r#"
import time
time.sleep(30)
"#,
    )
    .unwrap();

    let mut client = McpStdioClient::new(
        PathBuf::from("python"),
        vec![hang_script.to_string_lossy().to_string()],
        Some(ws),
        Duration::from_millis(400), // 400ms timeout
    );

    let res = client.initialize().await;
    assert!(res.is_err(), "Must time out when server does not respond");
    assert!(res.unwrap_err().to_string().contains("timed out"));
}
