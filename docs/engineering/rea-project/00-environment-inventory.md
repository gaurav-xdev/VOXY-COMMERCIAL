# Environment and Discovery Inventory

## 1. System Environment
- **Operating System**: Microsoft Windows 11 Home Single Language (Version 10.0.26200, 64-bit)
- **CPU**: 13th Gen Intel(R) Core(TM) i5-13420H (8 Cores, 12 Logical Processors)
- **RAM**: 16 GB Total (~5.2 GB Free Physical Memory)
- **Disk Space**: C: Drive has ~51.1 GB free space
- **Node.js**: v24.14.0
- **npm**: 11.9.0
- **Python**: 3.11.9
- **Rustc / Cargo**: 1.99.1 (MSVC x86_64)
- **Git**: 2.54.0.windows.1

## 2. OSMOO Project State
- **Repository Location**: `c:\Users\gaurav\Downloads\VOXY-COMMERCIAL`
- **Active Branch**: `master` (ahead of `origin/master` by 7 commits)
- **Current Workspace Architecture**:
  - `apps/daemon`: Core background service daemon with IPC, tools, state machine, and provider routing.
  - `apps/overlay`: Native overlay UI / HUD.
  - `crates/desktop_ui`: Unified main app UI and overlay in single binary (`OSMOO.exe`).
  - `crates/security`: Centralized security module containing `path_guard` (directory containment, Windows reparse/symlink escape defenses) and `secret_scanner` (AWS, GitHub PAT, API key pattern redaction).
  - `crates/tool_calling`: Registry and dispatch for assistant tools (`filesystem`, `memory`, `web`, `process`).
  - `crates/skills`: Dynamic skills engine with autonomous activation, 10-language toolchain compiler sandboxing, and engineering workflow.
  - `crates/model_router`: Multi-provider routing (Anthropic, OpenAI, Gemini, Ollama) with fallback policy and strict provider selection.
  - `crates/voice_*`: Full voice pipeline (activation, STT, TTS, streaming, orchestrator).
  - `crates/kodus_review`, `crates/trinity_swarm`, `crates/video_studio`: Isolated capability sub-systems.
  - `tests/e2e`: Comprehensive multi-tier acceptance tests (all 21 passing).

## 3. Claude Code Target Inventory
- **Version**: 2.1.215 (Claude Code)
- **CLI Executables**:
  - `C:\Users\gaurav\AppData\Roaming\npm\claude`
  - `C:\Users\gaurav\AppData\Roaming\npm\claude.cmd`
- **Package Location**: `C:\Users\gaurav\AppData\Roaming\npm\node_modules\@anthropic-ai\claude-code`
- **Package Layout**:
  - `bin/claude.exe`: Native single-executable binary (~256 MB)
  - `sdk-tools.d.ts`: 3,806 lines of complete TypeScript definitions and JSON Schema definitions for Claude CLI tool inputs
  - `cli-wrapper.cjs`: Platform detection and binary launcher fallback
  - `install.cjs`: Platform package resolution and postinstall installer
  - `package.json`: Manifest declaring supported platforms, engines (`>=22.0.0`), and metadata

## 4. Reverse Engineer Anything (REA) Status
- **Official Repository**: `https://github.com/morluto/rea`
- **npm Package**: `rea-agents@6.8.0` (Latest release, published ~25 mins ago)
- **Binary Targets**: `rea`, `rea-agents`
- **Current Installation State**: Not yet installed globally. Documented setup command: `npx rea-agents setup` or `npm install -g rea-agents`.

## 5. Prerequisites, Boundaries and Risks
- **Provider Enforcement**: Must respect configured user API provider and model; no silent fallbacks.
- **Preservation of Non-Coding Systems**: Voice pipeline, desktop overlay, IPC, and security broker must remain untouched and fully passing tests.
- **Windows File Locks**: Rust build locks must be avoided by using isolated `CARGO_TARGET_DIR` for child compiler operations.
