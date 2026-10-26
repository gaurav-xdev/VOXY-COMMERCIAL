# REA Capability Inventory

## 1. Executive Summary
REA (`rea-agents@6.8.0`) provides 175 CLI commands and 229 MCP tools covering static binary inspection, JavaScript/Web bundle deconstruction, dynamic Android/ADB manipulation, process scenario captures, and cross-artifact comparison.

## 2. Command Categories

### 2.1 Static JavaScript and Web Analysis
- `analyze-javascript-application`: Statically reconstructs a local JavaScript/Electron application into an AST graph.
- `analyze-web-bundle`: Captures and statically analyzes web bundle modules and export shapes.
- `compare-javascript-export-shapes`: Compares static JS export return signatures without execution.
- `recover-javascript-sources`: Recovers readable JavaScript modules with provenance.
- `trace-javascript-semantics`: Traces static relations through authenticated application evidence.
- `trace-web-module-imports`: Traces bundle imports to source URLs and candidates.

### 2.2 PE/ELF Binary and Address Analysis
- `address-to-file-offset`: Resolves runtime analysis addresses to original PE/ELF file byte offsets.
- `compare-managed-members`: Compares .NET PE/CLI member inventories by declared type and signature.
- `verify-managed-native-boundaries`: Verifies P/Invoke native boundary calls against static evidence.
- `read-bytes` / `search-bytes`: Inspects memory or file byte sequences.
- `decompile` / `decompile-all`: Pseudocode extraction (requires configured engine like Ghidra/Rizin/Hopper).

### 2.3 Scenario and Runtime Capture
- `capture-process`: Captures caller-selected process scenarios and thread memory snapshots.
- `capture-browser-scenario` / `capture-electron-scenario`: Runs bounded Playwright scenarios and captures DOM/network interactions.
- `capture-native-ui-scenario`: Executes declarative accessibility (AX) actions in native application windows.

### 2.4 Android & Device Diagnostics
- Complete suite of ADB file, activity, package, and logcat extraction commands (`pull-adb-package`, `read-adb-logcat`, `decode-android-resources`).

### 2.5 Agent Environment Synthesis
- `environment-build`, `environment-compare`, `environment-export`, `environment-serve`: Generates reproducible mock sandboxes from captured HAR and scenario evidence for autonomous agent testing.

## 3. Applicable Tooling for Claude Code v2.1.215
Given Claude Code v2.1.215 is delivered as a 256MB packaged Node binary (`bin/claude.exe`) with companion TypeScript definitions (`sdk-tools.d.ts`) and installation scripts (`install.cjs`, `cli-wrapper.cjs`), REA's static JavaScript deconstruction and direct file offset inspection capabilities provide authoritative evidence on internal tool schemas, routing contracts, and execution models.
