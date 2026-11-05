# OSMOO Security Review & Guardrail Architecture

**Date:** October 28, 2026  
**Auditor:** OSMOO Core Engineering Team  
**Scope:** Tool Framework, Office Suite, Email Connector, Payment Integration, Desktop Control  

---

## 1. Security Architecture Principles

OSMOO operates with a Defense-in-Depth model designed for desktop AI companions:
1. **Least Privilege & Role-Based Scope Sets:** Scopes (`FilesRead`, `FilesWrite`, `CalendarRead`, `CalendarWrite`, `DocumentWrite`, etc.) are checked before every I/O operation.
2. **Mandatory Human-in-the-Loop Gating:** Irreversible or destructive operations (email sending, file deletion, process termination, external handoffs) require cryptographic approval tokens or explicit confirmation booleans.
3. **Strict Financial Isolation:** Complete prohibition of raw cardholder data (PAN, CVV) in agent memory and automation tools.
4. **Path Traversal & Injection Neutralization:** All filesystem paths are canonicalized; shell argument strings are safely structured without shell interpretation.
5. **Emergency Stop Safeguard:** An atomic circuit breaker immediately halts all automated operations across Windows UI and tool execution.

---

## 2. Threat Vector Analysis & Implemented Defenses

### 2.1 Threat Vector 1: Accidental or Malicious Email Dispatch
- **Vulnerability:** An LLM generates an email draft and immediately attempts to send it to external clients without review.
- **Implemented Defense:**
  - `MailConnector::create_draft` stores messages in an unapproved state (`is_approved = false`).
  - `send_approved_email` explicitly checks `user_approval_confirmed: bool`.
  - Calling `send_approved_email` with `user_approval_confirmed == false` immediately aborts and logs an audit failure (`AuditStatus::Denied`).
  - In `EmailSendApprovedTool`, the action is classified as `RiskTier::Privileged`, requiring approval brokering.

### 2.2 Threat Vector 2: Financial Hazard & Credit Card Leakage
- **Vulnerability:** Automated agent scripts might attempt real purchases or store credit card numbers in plaintext logs.
- **Implemented Defense:**
  - `ShoppingConnector::execute_sandbox_test_payment` rejects any token not prefixed with `tok_sandbox_` or `pm_sandbox_`.
  - Raw card numbers (e.g. 16-digit PANs) return `OfficeError::PermissionDenied`.
  - The tool explicitly provides merchant handoff URLs so the human user completes the final transaction inside their own authenticated browser.

### 2.3 Threat Vector 3: Path Traversal & Unauthorized File Access
- **Vulnerability:** Malicious prompts attempting to write or read system files via `../../Windows/System32`.
- **Implemented Defense:**
  - Tested in `test_filesystem_path_traversal_prevention`.
  - Canonicalization and workspace bounding reject traversal sequences and unauthorized external writes.

### 2.4 Threat Vector 4: Unbounded Desktop Automation & Runaway Scripts
- **Vulnerability:** Mouse or keyboard automation enters an infinite loop or interacts with unintended windows.
- **Implemented Defense:**
  - `ToolRegistry::emergency_stop_token` provides an `Arc<AtomicBool>`.
  - Calling `trigger_emergency_stop()` halts all subsequent tool calls instantly.
  - Tested in `tools::tests::test_emergency_stop_aborts_tool_execution` and `test_browser_runtime_emergency_stop`.

### 2.5 Threat Vector 5: Audit Log Tampering
- **Vulnerability:** Undetected unauthorized actions.
- **Implemented Defense:**
  - `OfficeAuditTrail` maintains an immutable append-only ledger in memory and persisted JSONL format.
  - Every action records: subsystem, action name, resource target, user session ID, required scope, timestamp, audit status (`Success` or `Denied`), and parameters/results.

---

## 3. Summary of Security Test Results

- `test_filesystem_path_traversal_prevention`: **PASSED**
- `test_shopping_and_sandbox_payment_safety`: **PASSED**
- `test_mail_connector_draft_and_approval_gate`: **PASSED**
- `confirmation_gating_blocks_unconfirmed_destructive_tool`: **PASSED**
- `test_emergency_stop_aborts_tool_execution`: **PASSED**
- `test_browser_upload_sensitive_file_blocking`: **PASSED**
- `test_browser_download_path_traversal_and_dangerous_extensions`: **PASSED**
