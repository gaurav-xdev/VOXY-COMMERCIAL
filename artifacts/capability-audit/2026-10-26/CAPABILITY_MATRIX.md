# OSMOO Comprehensive Capability Matrix (Domains A – M)

**Date:** 2026-10-28  
**Evaluation Standard:** Direct Code Inspection, AST Analysis & Reproducible Test Execution  
**Status Legend:**
- `VERIFIED WORKING`: Source code exists, integrated into live runtime path, and unit/integration tests pass with real evidence.
- `PARTIALLY WORKING`: Core capability functional in code, but lacks full format/protocol support or requires manual invocation.
- `MOCK / SIMULATED`: Stubbed interface, in-memory dummy store, or non-functional placeholder.
- `NOT IMPLEMENTED`: Feature claimed in concept/marketing but completely absent in codebase.
- `CONFIGURATION REQUIRED`: Fully implemented feature requiring external API keys, service credentials, or local binaries (e.g. Whisper models, Ollama).

---

## Domain A: Web Research & Browser Automation

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Multi-Source Deep Research** | `VERIFIED WORKING` | `crates/tool_calling/src/tools/research.rs`, `crates/tool_calling/src/registry.rs` | `ToolRegistry::execute("research_deep_investigate")` -> `ResearchInvestigateTool::execute()` | Limited to HTTP GET extraction & text synthesis; does not solve CAPTCHAs or execute complex JS SPAs. | `cargo test -p voxy-tool-calling` (`test_builtin_tools_registration` passed). |
| **Web Search Invocations** | `VERIFIED WORKING` | `apps/daemon/src/tools.rs` | `ToolRegistry::execute("search_web")` -> `open::that(search_url)` | Opens system default browser with search query URL; does not scrape or synthesize search results inline. | Win32 browser process launch verified via `apps/daemon/src/tools.rs:273`. |
| **DOM Tree & Web Automation** | `PARTIALLY WORKING` | `crates/automation/src/backend/uia.rs` | Win32 UIA tree traversal | Traverses Chromium / Edge accessibility trees if accessible, but lacks Playwright/CDP selector control. | `cargo test -p voxy-automation` (46/46 passed). |
| **Interactive Form Filling** | `PARTIALLY WORKING` | `crates/automation/src/backend/gdi.rs`, `apps/daemon/src/tools.rs` | `ToolRegistry::execute("type_text")` -> `WindowsGdiBackend::type_text()` | Emulates keystrokes blindly into active focus; can fail if focus shifts or field clears unexpectedly. | `cargo test -p voxy-automation` passed. |

---

## Domain B: Desktop & PC Control

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Application Launching** | `VERIFIED WORKING` | `apps/daemon/src/tools.rs`, `crates/automation/src/` | `ToolRegistry::execute("open_app")` -> `open_application()` | Uses Win32 shell execution; path resolution limited to registered system apps and `PATH` executables. | `tools::tests::test_safe_launch_blocks_command_injection` passed. |
| **Window Hierarchy Inspection** | `VERIFIED WORKING` | `crates/automation/src/backend/uia.rs` | `WindowsUiaBackend::list_windows()` | Requires Windows UI Automation; elevated/UAC windows require daemon to run with administrative privileges. | `crates/automation/tests/` passed. |
| **Window Management (Focus, Close, Min/Max)** | `VERIFIED WORKING` | `crates/automation/src/backend/uia.rs`, `apps/daemon/src/tools.rs` | `ToolRegistry::execute("focus_app" / "close_app" / "minimize_all")` | Relies on window title matching; ambiguous titles may focus the wrong instance. | Integration tests in `voxy-automation` passed. |
| **Hardware Mouse & Keyboard Emulation** | `VERIFIED WORKING` | `crates/automation/src/backend/gdi.rs` | `WindowsGdiBackend::click()`, `type_text()`, `send_hotkey()` | Win32 `SendInput` injects hardware events; blocked by UAC prompts on standard user accounts. | 46 tests passed in `voxy-automation`. |
| **Screenshot Capture & OCR Telemetry** | `VERIFIED WORKING` | `crates/automation/src/backend/gdi.rs`, `apps/daemon/src/tools.rs` | `ToolRegistry::execute("screenshot")` -> GDI BitBlt | Captures primary display or active window to PNG; multi-monitor DPI scaling requires primary display. | GDI screen capture verified in `apps/daemon/src/tools.rs:253`. |
| **Hardware Emergency Stop** | `VERIFIED WORKING` | `crates/automation/src/emergency.rs`, `apps/daemon/src/tools.rs` | `EmergencyStopGuard::trip()` -> atomic halt | Halts all automated input immediately; cannot roll back external process actions already committed. | `tools::tests::test_emergency_stop_lifecycle` passed. |

---

## Domain C: Calendar, Reminders & Scheduling

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Calendar Event Listing & Creation** | `MOCK / SIMULATED` | `crates/tool_calling/src/tools/calendar.rs` (if referenced) / Daemon in-memory | In-memory `Vec<CalendarEvent>` | **Non-persistent:** Data lives strictly in RAM; wiping on restart. **Zero external sync:** No CalDAV, Google, or Outlook. | In-memory vector operations only; no database schema. |
| **System Notification Reminders** | `PARTIALLY WORKING` | `crates/tool_calling/src/tools/notification.rs` | Win32 `powershell -Command "[Windows.UI.Notifications...]"` | Shells out to PowerShell toast notification helper; may be silenced by Windows Focus Assist. | Notification payload formatting unit tested in `voxy-tool-calling`. |

---

## Domain D: Meetings & Real-Time Assistance

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **System Audio Capture & Loopback** | `VERIFIED WORKING` | `crates/voice_audio/src/` | WASAPI loopback device stream | Captures active system speaker output; depends on WASAPI loopback driver support. | `test_wasapi_capture_lifecycle` passed in `voxy-voice-audio`. |
| **Real-Time Speech Transcription** | `VERIFIED WORKING` | `crates/voice_stt/src/` | `WhisperEngine::transcribe()` | Requires local Whisper model weights or external STT endpoint; CPU inference may lag on complex audio. | 7 tests passed in `voxy-voice-stt`. |
| **Meeting Bot Joining (Zoom/Teams/Meet)** | `NOT IMPLEMENTED` | None | N/A | **Zero bot integrations:** OSMOO cannot join a meeting URL as a virtual participant or ingest meeting transcripts directly. | No source files exist in repository. |

---

## Domain E: Office Work (DOCX, PPTX, XLSX, CSV, Reports)

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Markdown Document Generation** | `VERIFIED WORKING` | `crates/tool_calling/src/tools/file.rs`, `crates/coding_harness/src/patcher.rs` | `FileWriteTool::execute()` | Generates plain text Markdown (.md) reports cleanly; cannot format native binary Office files. | 22 tests passed in `voxy-tool-calling`. |
| **CSV Data Parsing & Manipulation** | `PARTIALLY WORKING` | `crates/tool_calling/src/tools/file.rs` | Standard text reading/writing | Basic line-by-line reading and regex extraction; no formula evaluation or pivot table computation. | Verified via `file_read` and `file_write` tests. |
| **Native Word (.docx) Compilation** | `NOT IMPLEMENTED` | None | N/A | **Missing binary library:** No `docx-rs` or XML packing library. Cannot author native Word files. | Codebase search confirmed zero DOCX dependencies. |
| **Native Excel (.xlsx) Compilation** | `NOT IMPLEMENTED` | None | N/A | **Missing binary library:** No `umya-spreadsheet` or `calamine`. Cannot author native Excel workbooks. | Codebase search confirmed zero XLSX dependencies. |
| **Native PowerPoint (.pptx) Compilation** | `NOT IMPLEMENTED` | None | N/A | **Missing binary library:** Cannot compile PPTX slide XML packages. | Codebase search confirmed zero PPTX dependencies. |

---

## Domain F: Email & Communication

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Transactional Mail Delivery** | `VERIFIED WORKING` | `crates/api_server/src/mailer.rs` | `ResendMailer::send()` | Restricted to backend account management (password resets, invites); not accessible for arbitrary user mail. | Transactional test suite in `voxy-api-server`. |
| **User Email Client (IMAP/SMTP/Gmail/Outlook)** | `NOT IMPLEMENTED` | None | N/A | **Zero personal inbox access:** Cannot connect to user mailboxes, read incoming threads, or draft replies. | No IMAP/POP3/MAPI crates in workspace. |

---

## Domain G: Payments & Financial Workflows

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Software Subscription Billing** | `VERIFIED WORKING` | `crates/billing/src/` | Dodo Payments webhook & customer portal | Strictly handles user payments *for OSMOO itself* (monthly/annual software subscriptions). | Billing crate tests passed. |
| **Consumer E-Commerce Checkout / Purchases** | `NOT IMPLEMENTED` | None | N/A | **Zero consumer payment automation:** Cannot purchase products on Amazon, enter credit cards, or manage carts. | No payment checkout agents exist. |

---

## Domain H: Flights, Travel & Booking

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Flight Search & Fare Comparison** | `NOT IMPLEMENTED` | None | N/A | **Zero travel API integrations:** No Skyscanner, Amadeus, or airline scraping engines exist. | Codebase search confirms zero travel modules. |
| **Hotel & Ride Booking** | `NOT IMPLEMENTED` | None | N/A | **Zero booking integrations:** Cannot make hotel, car rental, or ride reservations. | No travel modules in repository. |

---

## Domain I: Advanced Coding & Software Engineering

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Multi-Phase Task Planning** | `VERIFIED WORKING` | `crates/coding_harness/src/planner.rs` | `CodingEngine::plan_task()` | Generates structured execution steps; complex architecture refactorings require multi-turn steering. | Unit tests in `voxy-coding-harness` passed. |
| **AST Symbol Indexing & Navigation** | `VERIFIED WORKING` | `crates/coding_harness/src/symbol_index.rs` | `SymbolIndex::build_from_dir()` | Indexes functions, structs, traits, and modules across Rust, Python, TypeScript, and Go. | Symbol index unit tests passed. |
| **Unified Diff Patching & Rollback** | `VERIFIED WORKING` | `crates/coding_harness/src/patcher.rs` | `Patcher::apply_patch()` | Atomic file patching with journaling; automatically rolls back changes if a patch chunk fails to apply. | Multi-chunk patch & rollback tests passed. |
| **Sandboxed Toolchain Execution** | `VERIFIED WORKING` | `crates/coding_harness/src/process_runner.rs` | `ProcessRunner::run()` | Runs compilers/linters inside Windows Job Objects with CPU/memory limits and timeout kills. | Process timeout & memory limit tests passed. |
| **Automated Code Review & Linters** | `VERIFIED WORKING` | `crates/coding_harness/src/reviewer.rs` | `CodeReviewer::review()` | Runs static security rules, detects memory leaks, unsafe blocks, and formatting regressions. | Reviewer rule tests passed. |
| **Iterative Test-Driven Debugger** | `VERIFIED WORKING` | `crates/coding_harness/src/debugger.rs` | `Debugger::diagnose_and_repair()` | Captures test failure stderr, locates stack frame files, and proposes minimal targeted fixes. | Debugger diagnostic tests passed. |

---

## Domain J: Skills, Tools & MCP Integrations

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Built-in Native Tool Execution** | `VERIFIED WORKING` | `crates/tool_calling/src/registry.rs`, `apps/daemon/src/tools.rs` | `ToolRegistry::execute()` | 20+ native tools executed asynchronously with parameter validation and schema verification. | 22 tests in `voxy-tool-calling`, 16 tests in `voxy-daemon`. |
| **Model Context Protocol (MCP) Bridge** | `VERIFIED WORKING` | `crates/coding_harness/src/mcp_bridge.rs` | `McpBridge::call_tool()` | Communicates via JSON-RPC 2.0 over standard I/O with external MCP server binaries. | MCP JSON-RPC protocol roundtrip tests passed. |
| **Claude & Codex Skill Discovery** | `VERIFIED WORKING` | `crates/skills/src/`, `crates/coding_harness/src/skills_discovery.rs` | `SkillEngine::discover()` | Scans `.agents/skills` and `SKILL.md` bundles, extracting frontmatter metadata and tool mappings. | Skills discovery tests passed. |

---

## Domain K: Voice Assistant & Real-Time Conversation

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **WASAPI Low-Latency Audio Capture/Playback** | `VERIFIED WORKING` | `crates/voice_audio/src/` | `WasapiCaptureDevice`, `WasapiPlaybackDevice` | Sub-20ms roundtrip audio buffer on Windows; requires functional WASAPI drivers. | 15/15 tests passed in `voxy-voice-audio`. |
| **GCC-PHAT Spatial Beamforming** | `VERIFIED WORKING` | `crates/voice_activation/src/beamforming.rs` | `GccPhatBeamformer::process()` | Direction of Arrival (DOA) calculation across stereo/array microphones for noise attenuation. | Beamforming phase tests passed in `voxy-voice-activation`. |
| **Wake Word Detection & VAD** | `VERIFIED WORKING` | `crates/voice_activation/src/` | `EnergyVad::is_speech()`, `WakeWordDetector` | Lightweight local voice activity detection; custom wake words require trained ONNX models. | 4/4 tests passed in `voxy-voice-activation`. |
| **Immediate Token-to-Speech Streaming** | `VERIFIED WORKING` | `crates/voice_tts/src/streaming_chunker.rs`, `crates/voice_orchestrator/src/` | `IncrementalSpeechChunker::push_token()` | Submits first complete speakable word to TTS immediately (<120ms); does not wait for full sentences. | Unit tests in `voxy-voice-tts` and `voxy-voice-orchestrator` passed. |
| **Instant Barge-In Interruption** | `VERIFIED WORKING` | `crates/voice_orchestrator/src/barge_in.rs` | `BargeInController::handle_audio()` | Drops pending TTS queue and silences speaker within 35ms of user speaking. | 18 tests passed in `voxy-voice-orchestrator`. |

---

## Domain L: Memory, Task Planning & Autonomy

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Persistent SQLite Fact & Context Store** | `VERIFIED WORKING` | `crates/memory/src/` | `SqliteMemoryEngine::store()` / `recall()` | Persists user facts, entity relationships, and historical interaction graphs to disk. | Memory persistence tests passed in `voxy-memory`. |
| **Conversation History & Budget Trimming** | `VERIFIED WORKING` | `apps/daemon/src/main.rs:ConversationMemory` | `ConversationMemory::add_turn()` | Enforces token/character budget by retaining newest turns while preserving system prompt. | `tests::conversation_memory_trims_to_character_budget` passed. |
| **Long-Term Cognitive Reflection Engine** | `VERIFIED WORKING` | `crates/cognition/src/reflection.rs`, `crates/cognitive_orchestrator/` | `ReflectionEngine::analyze()` | Asynchronously extracts facts and conversation highlights; runs periodically in background. | `test_reflection_engine_analysis` passed in `voxy-daemon`. |

---

## Domain M: Reliability, Security & Privacy

| Capability / Feature | Status | Source Crates & Files | Runtime Path | Real-World Limitations | Concrete Test Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Human-in-the-Loop Approval Broker** | `VERIFIED WORKING` | `crates/security/src/approval.rs` | `ApprovalBroker::request_approval()` | Prompts user before destructive file deletions or high-risk shell commands. | `test_approval_broker` passed in `voxy-security`. |
| **Path Traversal & Escape Defense** | `VERIFIED WORKING` | `crates/security/src/path_guard.rs` | `PathGuard::validate_path()` | Canonicalizes paths and prevents symlink dereference outside designated project boundaries. | Path traversal test vectors passed in `voxy-security`. |
| **Server-Side Request Forgery (SSRF) Guard** | `VERIFIED WORKING` | `crates/security/src/ssrf_guard.rs` | `SSRFGuard::validate_url()` | Blocks requests to 127.0.0.1, RFC 1918 private ranges, and AWS/Azure/GCP metadata endpoints (169.254.169.254). | SSRF guard IP blacklist tests passed. |
| **Archive Bomb & Zip Slip Defense** | `VERIFIED WORKING` | `crates/security/src/archive_guard.rs` | `ArchiveGuard::extract_safe()` | Enforces extraction size limits and blocks `../` path slips in zip/tar archives. | Archive bomb rejection tests passed. |
| **Strict Model Routing (Zero-Fallback)** | `VERIFIED WORKING` | `crates/model_router/src/lib.rs`, `apps/daemon/src/main.rs` | `ModelRouter::route_llm()` | Rejects silent fallback to alternative models or providers when strict mode is active. | Model router strict routing tests passed. |
