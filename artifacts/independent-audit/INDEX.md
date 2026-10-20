# OSMOO Evidence Directory: Index & Navigation

**Directory Location:** `C:\Users\gaurav\Downloads\VOXY-COMMERCIAL\artifacts\independent-audit\`  
**Target Repository:** `C:\Users\gaurav\Downloads\VOXY-COMMERCIAL`  
**Git Branch / Commit:** `master` / `ccb5737d9d9b65877f32b4a66aa40df44c1d228d`  
**Date:** 2026-10-20

---

## Evidence Inventory

| File | Description | Key Content |
| :--- | :--- | :--- |
| [`INDEX.md`](./INDEX.md) | Evidence inventory & navigation guide | Master index, file descriptions, and navigation links. |
| [`environment.txt`](./environment.txt) | Environment & compiler metadata | Rustc, Cargo, Python, Node, OS versions, git commit, binary SHA-256 hashes. |
| [`commands.log`](./commands.log) | Exact command execution audit log | Timestamps, exit codes, and output summaries for 8 critical commands. |
| [`tests.md`](./tests.md) | Consolidated test inventory & classification | Breakdown of 170+ test executions: real OS/Win32 vs. tempdir vs. mocks. |
| [`source-map.md`](./source-map.md) | Critical source files, line numbers & symbols | Exact line-numbered excerpts for harness, automation, voice, lifecycle, and auth. |
| [`security.md`](./security.md) | Security classification & threat model audit | Classification of 16 security controls (path traversal, SSRF, DPAPI, secrets). |
| [`runtime.md`](./runtime.md) | Real runtime verification evidence | Binary size (9.2MB), CLI `--help` and `--status` outputs, Windows Job Object status. |
| [`limitations.md`](./limitations.md) | Honest disclosure of boundaries & blockers | Explanations of what is verified vs. unverified (mic arrays, cloud billing, TPM). |
| [`fix-report.md`](./fix-report.md) | Critical defect remediation report | Detailed RCA, code changes, and test results for all 4 audit issues. |

---

## Raw Execution Logs Directory (`./logs/`)

The `./logs/` subdirectory contains unedited raw compiler and test execution outputs:
- [`logs/cargo_test_desktop_ui.log`](./logs/cargo_test_desktop_ui.log): 35 tests across unittests, `backend_command_test`, `supervisor_test`, `ui_state_test`.
- [`logs/cargo_build_release_osmoo.log`](./logs/cargo_build_release_osmoo.log): Full 39m 50s release compilation log of `target\release\OSMOO.exe`.
- [`logs/cargo_test_harness.log`](./logs/cargo_test_harness.log): 25 tests across patch engine, sandboxed runner, and diagnostic parser.
- [`logs/cargo_test_automation_tool_calling.log`](./logs/cargo_test_automation_tool_calling.log): 57 tests across UIA automation and tool registry.
- [`logs/cargo_test_voice_audio_orchestrator.log`](./logs/cargo_test_voice_audio_orchestrator.log): 21 tests across audio conditioning and live barge-in.
- [`logs/cargo_test_platform_windows.log`](./logs/cargo_test_platform_windows.log): 8 tests across native Windows DPI, display, and process APIs.
- [`logs/cargo_check_workspace_tests.log`](./logs/cargo_check_workspace_tests.log): Clean compilation of all 77 workspace packages for test targets.

---

## Instructions for Independent Evaluator (ChatGPT)

1. Cross-examine the SHA-256 hash in `environment.txt` against the binary generated at `target\release\OSMOO.exe`.
2. Inspect `source-map.md` to trace execution paths from UI interaction down to Win32 system APIs (`crypt32.dll`, `taskkill.exe`, `CreateJobObjectW`, `cpal` WASAPI).
3. Review `security.md` to evaluate the fail-closed threat boundaries (SSRF blocking, path traversal, reparse point rejection, DPAPI logon binding).
4. Review `limitations.md` for explicit disclosure of non-tested items (cloud provider billing, physical mic arrays).
