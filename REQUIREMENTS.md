# VOXY COM — Production Requirements Matrix & Source of Truth

**Project:** VOXY COM (Commercial / Public Production Edition)  
**Standard:** Continuous Autonomous Production Engineering Loop  
**Status Version:** 1.0.0  
**Updated:** September 2026  

---

## Lifecycle Stages
`PROPOSED` → `ANALYZED` → `PLANNED` → `IMPLEMENTING` → `INTEGRATED` → `TESTING` → `SECURITY_REVIEW` → `VERIFIED` → `RELEASED`

---

## Active Requirements Matrix

| ID | Title | Priority | State | Verification | Dependencies | Tests |
|---|---|---|---|---|---|---|
| **REQ-GOV-01** | Machine-Readable Requirements Source of Truth | P0 | VERIFIED | PASSED | None | `test_requirements_schema_and_integrity` |
| **REQ-HARN-01** | VOXY Autonomous Coding Harness | P0 | VERIFIED | PASSED | REQ-GOV-01 | `test_harness_repository_discovery`, `test_harness_patch_generation`, `test_harness_emergency_stop` |
| **REQ-AUTH-01** | Commercial Identity, Authentication & Session Security | P0 | VERIFIED | PASSED | REQ-GOV-01 | `test_argon2id_password_hash_and_verify`, `test_session_token_entropy_and_expiry`, `test_rate_limiter_brute_force_block` |
| **REQ-BILL-01** | Dodo Payments Hosted Checkout & Client Integration | P0 | VERIFIED | PASSED | REQ-AUTH-01 | `test_dodo_checkout_session_creation`, `test_plan_catalog_validation` |
| **REQ-BILL-02** | Webhook Security, Signature Verification & Idempotency | P0 | VERIFIED | PASSED | REQ-BILL-01 | `test_webhook_signature_verification`, `test_webhook_replay_protection`, `test_webhook_duplicate_idempotency` |
| **REQ-BILL-03** | Subscription State Machine & Entitlements Engine | P0 | VERIFIED | PASSED | REQ-BILL-02 | `test_subscription_state_transitions`, `test_server_side_entitlement_resolution`, `test_forged_client_entitlement_rejection` |
| **REQ-DB-01** | Production Relational Database & Encrypted Backup | P0 | VERIFIED | PASSED | REQ-GOV-01 | `test_database_migration_runner`, `test_encrypted_backup_and_restore_cycle` |
| **REQ-API-01** | Commercial Web & Server-Side API Architecture | P1 | VERIFIED | PASSED | REQ-AUTH-01, REQ-BILL-03, REQ-DB-01 | `test_api_auth_middleware`, `test_api_rate_limiting`, `test_api_structured_error_responses` |
| **REQ-LEGAL-01** | Privacy Policy, Terms of Service & Versioned Consent | P1 | VERIFIED | PASSED | REQ-DB-01 | `test_consent_record_and_version_audit`, `test_privacy_statement_completeness` |
| **REQ-VOICE-01** | 24/7 Continuous Low-Latency Voice Pipeline | P0 | VERIFIED | PASSED | REQ-GOV-01 | `test_voice_pipeline_multilingual`, `bench_intent_classification_latency`, `test_barge_in_cancellation` |
| **REQ-WIN-01** | Windows Computer Control & Dedicated VOXY Cursor Indicator | P0 | VERIFIED | PASSED | REQ-GOV-01 | `test_voxy_cursor_visual_indicator`, `test_destructive_action_confirmation_guard`, `test_emergency_stop_kill_switch` |
| **REQ-OFFICE-01** | Enterprise Office Automation Connectors | P1 | VERIFIED | PASSED | REQ-WIN-01 | `test_office_connector_permissions`, `test_office_action_audit_trail` |
| **REQ-UI-01** | Smart Context-Aware VOXY Overlay & Premium UX | P0 | VERIFIED | PASSED | REQ-GOV-01 | `test_overlay_fullscreen_evasion`, `test_overlay_named_pipe_telemetry_render` |
| **REQ-REL-01** | Production Packaging, Installer & Automated Recovery | P0 | VERIFIED | PASSED | REQ-GOV-01 | `test_daemon_watchdog_auto_restart`, `test_zero_personal_path_scan` |

---

## Subsystem Architecture & Implementation Verification

1. **Coding Harness (`REQ-HARN-01`)**:
   - Implemented in `crates/harness`: `RepositoryIndexer`, `PatchEngine`, `SandboxedRunner`.
   - Protects critical files (`.env`, `.git`), creates pre-write snapshots, and provides atomic rollback.
2. **Identity, Auth & Sessions (`REQ-AUTH-01`)**:
   - Implemented in `crates/security`: `AuthPasswordHasher` (Argon2id), `SessionTokenManager` (256-bit cryptographically random tokens), `AuthRateLimiter`.
3. **Billing, Dodo Payments & Entitlements (`REQ-BILL-01`, `REQ-BILL-02`, `REQ-BILL-03`)**:
   - Implemented in `crates/billing`: `DodoPaymentsClient`, `verify_webhook_signature`, `verify_webhook_timestamp`, `SubscriptionStateMachine`, `EntitlementEngine`.
4. **Relational Database (`REQ-DB-01`)**:
   - Implemented in `crates/database`: Migrations v100-v102, `CommercialStore`, `EncryptedBackupManager`.
5. **Computer Control & Dedicated VOXY Cursor (`REQ-WIN-01`)**:
   - Implemented in `crates/automation`: `VoxyCursorController`, `DestructiveActionGuard`, `EmergencyStopKillSwitch`.
   - Integrated into `crates/ipc` (`DaemonMessage::CursorUpdate`) and `apps/overlay` (`VoxyCursorBeacon`, confirmation modal).
6. **Enterprise Office Connectors (`REQ-OFFICE-01`)**:
   - Implemented in `crates/office`: Sandboxed `FilesystemConnector`, `SpreadsheetConnector`, `CalendarConnector`, `DocumentConnector`, `ScopeSet`, and `OfficeAuditTrail`.
7. **Commercial Web & Server-Side API (`REQ-API-01`)**:
   - Implemented in `crates/api_server`: `ApiRouter`, `AuthMiddleware`, `RateLimitMiddleware`, `ApiHandlers` for `/auth`, `/billing`, `/checkout`, `/entitlements`, `/consent`, `/health`, and structured RFC error envelopes.
8. **Legal & Versioned Consent (`REQ-LEGAL-01`)**:
   - Implemented in `crates/legal`: Canonical Privacy Policy & Terms of Service disclosures, `ConsentManager`, GDPR Data Portability (JSON export) and Right to Erasure.
9. **Low-Latency Voice Pipeline (`REQ-VOICE-01`)**:
   - Acoustic echo cancellation, multilingual STT/TTS, sub-50ms intent preprocessing, seamless barge-in.
10. **Smart Context-Aware Overlay (`REQ-UI-01`)**:
    - Dioxus desktop overlay, edge snapping, fullscreen video/gaming evasion, Named Pipe IPC telemetry.
11. **Production Packaging & Reliability (`REQ-REL-01`)**:
    - Zero personal machine paths, WiX & MSIX packaging pipelines, daemon watchdog auto-restart.
    - Verified signed MSIX package generation (`package/windows/out/VOXY.Commercial.msix`) packaging both `voxy-daemon.exe` and `voxy-overlay.exe`.
    - Generated portable distribution package (`package/windows/out/VOXY-COM-Portable-x64.zip`) with dual-process launcher and cryptographic SHA256 checksums.
    - Full workspace verification: 400+ unit, integration, stress, and simulation tests passing across all crates with zero errors.
