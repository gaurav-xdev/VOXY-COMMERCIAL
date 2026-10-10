# Master Product Completion & Single-EXE Release Plan: OSMOO

> **Objective:** Deliver the final, production-hardened OSMOO Windows AI Operating Companion by Osmiora with real authentication (DB + Google OAuth + Email OTP), single primary executable (`OSMOO.exe`), animated desktop overlay, real autostart, internal coding harness with deep tools and IDE opening, persistent memory, extensible skill engine with skill creator, video-use adapter, n8n/browser research, live voice integration, polished UI with upgrade card, and verified NSIS installer.

---

## Phase 1: Authentication, Database, Google Sign-In, OTP
- [ ] **1.1 Database Schema & Store Expansion:**
  - Add tables / operations for:
    - `auth_identities`: (user_id, provider, provider_user_id, email, access_token, refresh_token, created_at, updated_at).
    - `otp_codes`: (id, email, code_hash, purpose, expires_at, attempts_count, max_attempts, used_at, created_at).
  - Add `CommercialStore` methods for Google identity linking, OTP creation, OTP verification, password reset, and session management.
- [ ] **1.2 Real Password Reset & OTP Flow:**
  - Secure 6-digit cryptographically generated OTP via `rand::thread_rng`.
  - Rate limiting (cooldown window, max 5 attempts, expiry within 10 minutes).
  - Resend integration in `api_server::email` for delivering OTP codes.
- [ ] **1.3 Google Sign-In Flow:**
  - Standard OAuth 2.0 PKCE / Authorization Code loop for Windows Desktop applications (loopback HTTP listener on `127.0.0.1:<random_port>/callback`).
  - Google Token exchange and OpenID Connect ID token claim verification (`sub`, `email`, `email_verified`).
  - Link or auto-create local DB account with verified email.
  - Reject unverified emails or invalid state/nonce tokens.
- [ ] **1.4 Security & Admin Audit:**
  - Verify zero hardcoded logins, zero demo bypasses, zero admin secret shortcuts.
  - Require explicit RBAC checks in `AuthMiddleware`.
- [ ] **1.5 Phase 1 Testing:**
  - Unit and integration tests for Register, Login, Duplicate prevention, OTP generation/validation/expiry/attempt limits, Google token verification, Session revocation, and cross-user isolation.

---

## Phase 2: Single Primary Executable Architecture (`OSMOO.exe`)
- [ ] **2.1 Integrate Core Daemon & Overlay into Single Binary:**
  - Inspect `apps/daemon` and `apps/overlay` functionality.
  - Convert `apps/daemon` subsystem routines into library modules (`voxy_desktop_runtime` / `voxy_voice_orchestrator`).
  - Make `OSMOO.exe` the single entry point that manages the background voice engine, the Named Pipe IPC server, the main companion UI window, and the companion overlay window seamlessly in-process or via internal supervisor.
- [ ] **2.2 Remove User-Facing Multi-Binary Dependencies:**
  - Ensure users never need to launch `voxy-daemon.exe` or `voxy-overlay.exe` manually.
  - Update packaging / installer scripts so `OSMOO.exe` is the sole canonical application.

---

## Phase 3: Smart Moving Companion Overlay
- [ ] **3.1 Character Model & Expressions:**
  - Animated companion character: Circular head/face, expressive eyes, oval lower body, circular hands.
  - State-driven animation for Idle, Listening, Processing, Speaking, Happy, Serious/neutral, Sad, Surprised, Interrupted, Error.
- [ ] **3.2 Desktop Movement & Window Physics:**
  - Multi-monitor awareness, DPI scaling, screen boundaries, non-distracting positioning, click-through mode toggling.
- [ ] **3.3 Lifecycle Integration:**
  - Controlled cleanly by `OSMOO.exe` with tray visibility toggling and graceful shutdown.

---

## Phase 4: Real Windows Startup Integration
- [ ] **4.1 Registry / Startup Integration:**
  - Implement real Windows registry startup registration under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
  - Support `--autostart` / `--background` argument flag to start minimized to tray.
  - Provide live Settings UI toggle that checks and sets registry entry directly.
- [ ] **4.2 Testing:**
  - Unit tests for registry path generation and toggle logic.

---

## Phase 5: Internal Coding Harness & Tools
- [ ] **5.1 Internal Routing & Execution:**
  - Wire coding task intent detection directly to `voxy_harness`.
- [ ] **5.2 Tool Registry Expansion:**
  - File reader, Symbol search, Patch apply, Command runner with Guardian policy, Git diff inspector, and Compiler diagnostic parser.
- [ ] **5.3 Open in IDE Tool:**
  - Detect installed editors (VS Code `code`, Cursor, Windsurf, Visual Studio `devenv`).
  - Open workspace on user command and report success/failure honestly.

---

## Phase 6: Permanent Scoped Memory
- [ ] **6.1 Multi-Tier Persistent Memory:**
  - Wire `crates/memory` and `scoped_memories` table in SQLite for cross-restart retention of user preferences, architecture, task states, and verified facts.
- [ ] **6.2 Per-User Isolation & Privacy:**
  - Isolate memory queries by `user_id`. Fail-closed on secrets/tokens.

---

## Phase 7: Extensible Skill System & Skill Creator
- [ ] **7.1 Dynamic Skill Activation:**
  - Verify `crates/skills` manifest parsing, task classification, and scoped activation.
- [ ] **7.2 Interactive Skill Creator:**
  - Tool to author, validate, and persist new skills with safety boundaries.

---

## Phase 8: Video Editing Integration (`video-use`)
- [ ] **8.1 Adapter & Capability Scaffolding:**
  - Add skill/tool adapter for `video-use` workflows with ffmpeg/browser-use safety checks.

---

## Phase 9: Browser, Research, n8n, & Market Intelligence
- [ ] **9.1 Web Research & Scrape Engine:**
  - Deep web research engine with source grounding and URL citations.
- [ ] **9.2 n8n Workflow Automation Tool:**
  - Generate, validate, and inspect n8n workflow JSON contracts.

---

## Phase 10: Real Voice & Acoustic Integration
- [ ] **10.1 Real WASAPI Pipeline Check:**
  - Verify Low-latency WASAPI capture, VAD, Wake word, STT, LLM, TTS, Audio playback.
  - Fail-closed without mock fallbacks in release profile.

---

## Phase 11: Premium UI Polish & Real Upgrade Card
- [ ] **11.1 Visual Elements & Upgrade Card:**
  - Premium dark glassmorphism, 3D WebGL core reactivity, configurable `OSMOO_UPGRADE_URL`.

---

## Phase 12 & 13: Full Build, Tests & Verified Release Artifacts
- [ ] **12.1 Comprehensive Verification:**
  - Run full workspace tests, build `target/release/OSMOO.exe`, compile NSIS installer `OSMOO-Setup-x64.exe`, generate SHA-256 hashes, and output final evidence report.
