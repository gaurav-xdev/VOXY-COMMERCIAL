# OSMOO Desktop Backend Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Connect the OSMOO Dioxus desktop frontend (`crates/desktop_ui`) to real backend services (cognition, SQLite conversation persistence, atomic settings management, DPAPI authentication, and Windows Job Object IPC supervision) without mock responses or fake controls.

**Architecture:** Create a typed backend command layer (`crates/desktop_ui/src/backend/`) that interfaces between Dioxus UI components and `AppBridge`. Wire UI events (prompt submission, settings changes, onboarding completion, emergency halt, and authentication) to real domain services, SQLite persistence, and IPC commands with full error handling and state feedback.

**Tech Stack:** Rust, Dioxus 0.6, Tokio, SQLite (`voxy-database`), `voxy-cognition`, `voxy-desktop-runtime`, `voxy-ipc`, `voxy-security`, Windows DPAPI, Windows Job Objects.

---

### Task 1: Create Typed Backend Command & Service Dispatch Module

**Files:**
- Create: `crates/desktop_ui/src/backend/mod.rs`
- Create: `crates/desktop_ui/src/backend/commands.rs`
- Modify: `crates/desktop_ui/src/lib.rs`
- Modify: `crates/desktop_ui/src/main.rs`
- Test: `crates/desktop_ui/tests/backend_command_test.rs`

**Step 1: Write failing integration test for backend commands**
Create `crates/desktop_ui/tests/backend_command_test.rs` testing `BackendDispatcher` execution of `SaveSettings`, `SendPrompt`, `EmergencyStop`, and `GetChatHistory`.

**Step 2: Run test to verify it fails**
Run: `cargo test -p voxy-desktop-ui --test backend_command_test`
Expected: FAIL (module not defined)

**Step 3: Implement `BackendDispatcher`**
Implement typed `BackendCommand` enum, `BackendResponse` enum, and `BackendDispatcher` executing on `AppBridge`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p voxy-desktop-ui --test backend_command_test`
Expected: PASS

---

### Task 2: Wire Chat Dialogue & Conversation History to SQLite Persistence

**Files:**
- Modify: `crates/desktop_ui/src/app.rs`
- Modify: `crates/desktop_ui/src/components/assistant_hud.rs`
- Test: `crates/desktop_ui/tests/backend_command_test.rs`

**Step 1: Write failing test for conversation persistence**
Add test in `backend_command_test.rs` verifying that sending a prompt creates messages in `SqliteConversationStore` and retrieves them via `GetChatHistory`.

**Step 2: Run test to verify failure**
Run: `cargo test -p voxy-desktop-ui --test backend_command_test`

**Step 3: Connect `app.rs` chat prompt handler to `BackendDispatcher`**
- On initial load: query `SqliteConversationStore` for existing messages.
- On `on_send_prompt`: store user message in DB, set state to `Thinking`, dispatch to `cognition` and/or IPC daemon, store response in DB, update chat history signal, return to `Idle`.

**Step 4: Run test to verify pass**
Run: `cargo test -p voxy-desktop-ui --test backend_command_test`
Expected: PASS

---

### Task 3: Wire Settings Modal to Atomic `SettingsManager` Persistence

**Files:**
- Modify: `crates/desktop_ui/src/views/settings.rs`
- Modify: `crates/desktop_ui/src/app.rs`
- Test: `crates/desktop_ui/tests/settings_persistence_test.rs`

**Step 1: Write failing test for settings load, update, and disk verification**
Test `SettingsManager` update from `SettingsFormState` and verify that disk JSON contains updated values.

**Step 2: Run test to verify failure**
Run: `cargo test -p voxy-desktop-ui --test settings_persistence_test`

**Step 3: Connect `SettingsView` in `app.rs`**
- Populate `initial_settings` from `bridge.settings.get_snapshot()`.
- On `on_save`: call `BackendDispatcher::dispatch(BackendCommand::SaveSettings(updated))`.
- Persist atomically and return feedback.

**Step 4: Run test to verify pass**
Run: `cargo test -p voxy-desktop-ui --test settings_persistence_test`
Expected: PASS

---

### Task 4: Connect Onboarding Completion to Real Configuration & Provisioning

**Files:**
- Modify: `crates/desktop_ui/src/components/onboarding_wizard.rs`
- Modify: `crates/desktop_ui/src/app.rs`
- Test: `crates/desktop_ui/tests/ui_state_test.rs`

**Step 1: Write test for onboarding completion**
Verify that onboarding completion updates `SettingsSnapshot.onboarded = true` and writes provisioned flag.

**Step 2: Connect `on_finish` in `app.rs`**
Update `SettingsManager` and write provisioning flag.

**Step 3: Run tests to verify pass**
Run: `cargo test -p voxy-desktop-ui`
Expected: PASS

---

### Task 5: Wire Authentication View to Real `AuthBridge` & DPAPI

**Files:**
- Modify: `crates/desktop_ui/src/views/auth.rs`
- Modify: `crates/desktop_ui/src/app.rs`
- Test: `crates/desktop_ui/tests/ui_state_test.rs`

**Step 1: Write test for `AuthBridge` dispatch**
Verify that login with invalid credentials returns honest error, and valid login stores DPAPI session.

**Step 2: Connect `AuthView` submit handler**
Call `bridge.auth.login()` and `bridge.auth.register()` via async task in Dioxus. Display real error messages or trigger `on_success`.

**Step 3: Run tests to verify pass**
Run: `cargo test -p voxy-desktop-ui`
Expected: PASS

---

### Task 6: Real System Telemetry & Health Stream

**Files:**
- Modify: `crates/desktop_ui/src/app.rs`
- Modify: `crates/desktop_ui/src/components/assistant_hud.rs`

**Step 1: Connect telemetry periodic sampler**
Sample CPU and memory usage from `bridge.health` and IPC daemon connection status from `bridge.ipc.watch_connected()`.

**Step 2: Run cargo test & cargo check**
Run: `cargo check -p voxy-desktop-ui`
Expected: PASS

---

### Task 7: Full Workspace Verification & Report Generation

**Files:**
- All modified files
- Verification Report

**Step 1: Run all desktop UI tests**
Run: `cargo test -p voxy-desktop-ui`

**Step 2: Run daemon tests**
Run: `cargo test -p voxy-daemon`

**Step 3: Build release binary**
Run: `cargo build -p voxy-desktop-ui --bin OSMOO`

**Step 4: Create final verification report**
