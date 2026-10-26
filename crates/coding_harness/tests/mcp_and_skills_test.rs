use std::path::Path;
use std::sync::Arc;
use voxy_coding_harness::{McpBridge, SkillRouter};
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

    // 2. Verify MCP bridge tool discovery
    let registry = Arc::new(ToolRegistry::with_builtins());
    let bridge = McpBridge::new(registry);

    let tools = bridge.list_available_tools().await;
    assert!(!tools.is_empty(), "Builtin tool registry should expose tools");
    assert!(tools.iter().any(|t| t.name == "file_list" || t.name == "window_list"));
}
