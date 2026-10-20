# OSMOO Consolidated Test Inventory & Execution Classifications

**Audit Date:** 2026-10-20  
**Scope:** Automated Test Suites across `voxy-desktop-ui`, `voxy-harness`, `voxy-automation`, `voxy-tool-calling`, `voxy-voice-audio`, `voxy-voice-orchestrator`, `voxy-platform-windows`, `voxy-ipc`, `voxy-daemon`, `voxy-database`.

---

## 1. Test Execution Summary

| Crate / Target | Test Command | Passed | Failed | Ignored | Environment / Harness Type | Raw Log |
| :--- | :--- | :---: | :---: | :---: | :--- | :--- |
| `voxy-desktop-ui (lib)` | `cargo test -p voxy-desktop-ui --lib` | 10 | 0 | 0 | Real Win32 DPAPI + Real Win32 Job Object | `cargo_test_desktop_ui.log` |
| `voxy-desktop-ui (bin)` | `cargo test -p voxy-desktop-ui --bin OSMOO` | 10 | 0 | 0 | Identical to lib tests (shared module unittests) | `cargo_test_desktop_ui.log` |
| `backend_command_test` | `cargo test -p voxy-desktop-ui --test backend_command_test` | 5 | 0 | 0 | Real SQLite WAL + TempDir + Dispatcher | `cargo_test_desktop_ui.log` |
| `supervisor_test` | `cargo test -p voxy-desktop-ui --test supervisor_test` | 2 | 0 | 0 | Real Win32 Job Object Limits & Process Exit | `cargo_test_desktop_ui.log` |
| `ui_state_test` | `cargo test -p voxy-desktop-ui --test ui_state_test` | 8 | 0 | 0 | Deterministic Math & Config Validation | `cargo_test_desktop_ui.log` |
| `voxy-harness (lib)` | `cargo test -p voxy-harness --lib` | 24 | 0 | 0 | TempDir + Process Tree (`taskkill /F /T`) | `cargo_test_harness.log` |
| `isolated_workflow_test` | `cargo test -p voxy-harness --test isolated_workflow_test` | 1 | 0 | 0 | Real Git/Patch Engine + TempDir | `cargo_test_harness.log` |
| `voxy-automation` | `cargo test -p voxy-automation` | 37 | 0 | 0 | Real Win32 UIA Backend + ApprovalBroker | `cargo_test_automation_tool_calling.log` |
| `voxy-tool-calling` | `cargo test -p voxy-tool-calling` | 20 | 0 | 0 | Real Filesystem TempDir + Mock Browser Network | `cargo_test_automation_tool_calling.log` |
| `voxy-voice-audio` | `cargo test -p voxy-voice-audio` | 9 | 0 | 0 | Real WASAPI Stream + Synthetic DSP Audio Buffers | `cargo_test_voice_audio_orchestrator.log` |
| `voxy-voice-orchestrator` | `cargo test -p voxy-voice-orchestrator` | 12 | 0 | 0 | Live In-Memory Pipeline + Simulated Barge-In | `cargo_test_voice_audio_orchestrator.log` |
| `voxy-platform-windows` | `cargo test -p voxy-platform-windows` | 8 | 0 | 0 | Real Win32 Desktop API (DPI, Displays, Windows) | `cargo_test_platform_windows.log` |
| `voxy-ipc` | `cargo test -p voxy-ipc` | 107 | 0 | 0 | Real Named Pipe (`\\.\pipe\voxy-com-ipc`) + HMAC | Prior session verified |
| `voxy-daemon` | `cargo test -p voxy-daemon` | 23 | 0 | 0 | Daemon Watchdog & Supervision Integration | Prior session verified |
| `voxy-database` | `cargo test -p voxy-database` | 98 | 0 | 0 | Real SQLite WAL + Concurrent Threads | Prior session verified |

---

## 2. Test Classification: Real OS/Hardware vs. Mocks/Fixtures

### A. Real OS & Win32 Hardware Calls (Exercised against OS kernel)
1. **Windows DPAPI** (`bridge::auth::tests::test_dpapi_protect_and_unprotect_roundtrip` & `test_auth_lifecycle_end_to_end`):
   Calls `crypt32.dll` (`CryptProtectData` and `CryptUnprotectData`). Verified that encrypted bytes can only be decrypted within the active Windows logon session.
2. **Windows Job Objects** (`supervisor::tests::test_windows_job_object_creation`, `test_job_object_assign_and_kill_process_tree`, `test_integration_windows_job_object_limits`):
   Calls `CreateJobObjectW`, `SetInformationJobObject` (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), and `AssignProcessToJobObject`. Confirms child processes terminate upon Job close.
3. **Process Tree Termination** (`sandbox_runner::tests::test_harness_timeout_terminates_process_tree`):
   Spawns child and grandchild processes (`cmd.exe /c ping -n 30 127.0.0.1`) and terminates them using `taskkill /F /T /PID`.
4. **WASAPI Audio Loopback & Physical Stream** (`voxy_voice_audio::tests::test_audio_capture_handle_physical_stream_semantics` and `live_wasapi_test`):
   Initializes COM audio client via `cpal` with WASAPI backend.
5. **Windows UI Automation (UIA)** (`voxy_automation::tests::test_windows_uia_backend_capabilities`):
   Queries Windows UIAutomation COM interface to discover desktop element trees and window handles.
6. **Windows Desktop & Display Metrics** (`voxy_platform_windows::platform::tests::*`):
   Calls Win32 APIs: `SetProcessDpiAwarenessContext`, `GetDpiForMonitor`, `EnumDisplayMonitors`, `GetForegroundWindow`.

### B. Isolated Filesystem & In-Memory Engines (No mock shims, real code)
1. **SQLite WAL Persistence** (`backend_command_test::test_backend_dispatcher_prompt_and_conversation_persistence`):
   Uses real `SqliteConversationStore` writing to a temporary `conversations.db` on disk. Tests transactions, schema migrations, and message roundtrips.
2. **Patch & Diagnostic Engine** (`patch_engine::tests::*`, `diagnostic_parser::tests::*`):
   Uses real `similar::TextDiff` to compute hunk offsets and roll back files atomically. Diagnostic parser parses real compiler output from `rustc` and `tsc`.
3. **Secret Scanner** (`secret_scanner::tests::*`):
   Evaluates regex patterns and Shannon entropy on real sample keys (AWS, GitHub tokens, RSA private keys).
4. **Audio DSP Filters** (`conditioning::tests::*`):
   Feeds synthetic float PCM buffers through the 80Hz high-pass filter, noise gate, and peak limiter to verify mathematical DC offset elimination and dB attenuation.

### C. Mocks and Network Boundaries
1. **Cloud AI Providers** (OpenAI, Anthropic, Cartesia):
   Tests verify catalog contract (`requires_api_key = true` fails closed when unconfigured). No external live billing requests are made during automated tests to avoid network flakiness and credential exposure.
2. **Local Model Discovery** (`DiscoveryBridge`):
   Unit tests provide empty or malformed JSON payloads to verify graceful fallback when Ollama is offline.
3. **Simulated Barge-In Audio** (`test_live_barge_in_mid_playback_interruption`):
   Generates a synthetic speech burst into the VAD queue while playback is simulated to verify that `CancellationToken` cancels the in-flight playback future.

---

## 3. Duplicate Test Targets

- **`voxy-desktop-ui` Lib vs Bin**:
  - `src/lib.rs` and `src/main.rs` both compile and run the 10 unit tests in `bridge::auth`, `bridge::discovery`, and `supervisor`.
  - Both targets execute identically because `src/main.rs` includes modules from the crate root.
  - This is expected behavior in Cargo packages that expose both a library and an executable target.
