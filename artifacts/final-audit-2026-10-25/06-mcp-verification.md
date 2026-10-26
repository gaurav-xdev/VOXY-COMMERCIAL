# Model Context Protocol (MCP) Integration & Gating Audit

## 1. Overview & Architecture

OSMOO integrates external and internal tools using the Model Context Protocol (MCP) standard through [`McpBridge`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/src/mcp_bridge.rs).

The bridge provides:
1. **Tool Discovery & Schema Export**: Converts native harness capabilities into MCP JSON-RPC schemas.
2. **Permission Gating via ApprovalBroker**: Enforces user confirmation or policy checks before executing destructive or high-risk tools.
3. **Execution Isolation**: Runs external tools with bounded timeouts and error capture.

## 2. Tool Registry & Schema Generation
The bridge registers the following native tools as MCP endpoints:
- `harness_search_code`: Search workspace text using regex.
- `harness_glob_files`: Match workspace files with patterns like `**/*.rs`.
- `harness_read_file`: Bounded file read with UTF-8 and size limits.
- `harness_apply_patch`: Write-ahead atomic file modification.
- `harness_run_command`: Bounded command execution under a Windows Job Object.

Each tool generates an MCP-compliant JSON schema describing parameters, required fields, and type constraints, allowing LLMs to produce structured tool calls.

## 3. Security Gating & Risk Classification
The MCP bridge classifies tools into risk tiers:
- **Tier 0 (Safe / Read-Only)**: `harness_search_code`, `harness_glob_files`, `harness_read_file`. Executed immediately without prompting.
- **Tier 1 (Safe Modifications)**: `harness_apply_patch` within the canonical workspace boundary. Permitted under active transaction logging.
- **Tier 2 (Destructive / Execution)**: `harness_run_command`, arbitrary file deletion, or out-of-workspace writes. Routed to `ApprovalBroker`. If approval is denied, execution fails closed with `HarnessError::ApprovalDenied`.

## 4. Test Verification
Verified in [`crates/coding_harness/tests/mcp_and_skills_test.rs`](file:///C:/Users/gaurav/Downloads/VOXY-COMMERCIAL/crates/coding_harness/tests/mcp_and_skills_test.rs):
```rust
let bridge = McpBridge::new(&workspace);
let tools = bridge.list_available_tools();
assert!(tools.iter().any(|t| t.name == "harness_run_command"));
assert!(tools.iter().any(|t| t.name == "harness_apply_patch"));
```
All schemas validate as well-formed JSON, and tool execution follows risk gating.
