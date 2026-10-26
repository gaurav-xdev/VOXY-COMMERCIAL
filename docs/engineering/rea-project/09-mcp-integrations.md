# OSMOO Coding Harness MCP Integrations

## 1. Overview
The OSMOO autonomous senior engineering harness integrates Model Context Protocol (MCP) servers to extend capabilities beyond native file editing and shell execution. The bridge connects the harness with `voxy_tool_calling::McpManager`.

## 2. Integrated MCP Servers & Tool Suites

### 2.1 Code Search & AST Inspection
- **Ripgrep & Ast-Grep**: Fast codebase semantic patterns and structural AST queries.
- **Git MCP**: Git graph exploration, commit tree inspections, and branch manipulation without raw shell invocations.

### 2.2 Science & External Viewers
- **Biomolecular Structure Viewer MCP**: Visual analysis and validation for bioinformatics and structural biological computing workflows.
- Registered tools: `load_structure`, `highlight_region`, `set_representation`, `set_overlays`, `annotate_region`.

### 2.3 Memory & Knowledge Graphs
- **Graphify**: Codebase knowledge graph querying (`graphify query`, `graphify explain`, `graphify path`) for deep architectural navigation before modifying unfamiliar files.

## 3. Protocol Enforcement
- All MCP tool calls are dispatched via JSON-RPC stdio.
- Tool input schemas are validated against parameters prior to transmission.
- Tool outputs are bounded by token budgets to prevent context window explosion.
