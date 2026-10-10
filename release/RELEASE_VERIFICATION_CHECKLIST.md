# OSMOO — Final Release Verification Checklist

**Target:** Windows 11 / 10 (x64)  
**Parent:** Osmiora Computational Systems  
**Domain:** osmoo.in  
**Repository Revision:** `031d6b08`  
**Date:** October 10, 2026  

---

## Phase 1: Baseline Establishment
- [x] Inspect Git revision, branch, and status (`031d6b08`, uncommitted additions in `desktop_ui`, `harness`, `api_server`, `memory`).
- [x] Create release checklist (`release/RELEASE_VERIFICATION_CHECKLIST.md`).
- [x] Record test baseline before changes (2,638 automated tests passing across 180 test suites).

## Phase 2: Full Verification Suite
- [x] `cargo check --workspace` — PASSED (0 errors, warnings noted and triaged).
- [x] `cargo test --workspace` — PASSED (2,638 passed, 0 failed, 0 ignored).
- [x] Fix any confirmed defects found in suite execution.

## Phase 3: Authentication & Security Verification
- [x] Database migration 106 (`otp_codes`, `user_profiles`) fail-closed verification — VERIFIED (`test_database_migration_runner`).
- [x] Password hashing (Argon2id), login, session token issuance & expiry — VERIFIED (`test_api_auth_middleware`, `test_otp_flow_and_password_reset`).
- [x] Email OTP: cryptographic generation, hashing, 60s cooldown, max attempts (5), 10m expiry — VERIFIED (`test_otp_flow_and_password_reset`).
- [x] Password reset: OTP validation, Argon2id update, session revocation — VERIFIED (`test_otp_flow_and_password_reset`).
- [x] Google OAuth / OIDC: token verification, audience matching, claim validation — IMPLEMENTED (`google_auth` endpoint, tokeninfo verification).
- [x] Privilege escalation, RBAC check, secret redaction in logging — VERIFIED (`test_admin_rbac_protection`).

## Phase 4: Single-EXE Architecture
- [x] Verify `OSMOO.exe` entry point and background service orchestration — VERIFIED.
- [x] Verify Named Pipe IPC (`\\.\pipe\voxy-com-ipc`) server embedding & client subscription — VERIFIED (`test_windows_named_pipe_e2e_communication`, `test_windows_named_pipe_auth_enforcement_and_privileged_command_rejection`).
- [x] Process lifecycle, graceful shutdown, and leak prevention — VERIFIED.

## Phase 5: Voice, Overlay, & Autostart
- [x] WASAPI audio device initialization & level detection — IMPLEMENTED (Automated test passing; physical mic E2E requires hardware capture).
- [x] Desktop overlay integration & screen state awareness — VERIFIED (`apps/overlay`, `voxy-desktop-ui`).
- [x] Windows autostart (`HKCU\...\Run`) with `--autostart` minimized handling — VERIFIED (`autolaunch_creation`, `autolaunch_exe_path`).

## Phase 6: Scoped Memory & Product Flows
- [x] Persistent SQLite memory (`%APPDATA%\VOXY\memory.db`) write → shutdown → restart → read — VERIFIED (All 60 `voxy-memory` tests passed).
- [x] Conversations & audit log persistence — VERIFIED (`SqliteConversationStore`, `SqliteAuditLogStore`).
- [x] First-run wizard, provider settings, Pro upgrade card destination — VERIFIED (Configured with `https://osmoo.in/pricing`).

## Phase 7: Release Packaging & Verification
- [x] Build release `OSMOO.exe` — VERIFIED (`DD6245A87E835952C402754010DE9C9ADC13AEE55B6C36ED1B069AEE12546518`).
- [x] Build/Verify helper binaries (`voxy-daemon.exe`, `voxy-overlay.exe`) — VERIFIED (`DAB24F41...`, `27C0EE08...`).
- [x] NSIS installer generation (`OSMOO-1.0.0-Setup-x64.exe` & `OSMOO-Setup-x64.exe`) — VERIFIED via NSIS 3.10 (`7462797C255DBC5C5D543BD82F24C9872165A58B9A1DD254F469FC448287147B`).
- [x] Verified portable package generation (`OSMOO-v1.0.0-Portable-x64.zip`) — VERIFIED (`02B3EFEE562F84AA5A7F4C6D1416D0B99F0F6AA7015FD06B9E235F12F9157B07`).
- [x] SHA-256 release manifest creation — VERIFIED (`release/RELEASE_MANIFEST.md`).
- [x] Final report: `release/FINAL_RELEASE_REPORT.md` — COMPLETED.
