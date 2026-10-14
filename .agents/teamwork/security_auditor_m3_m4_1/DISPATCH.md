# Dispatch for Forensic & Security Auditor (Milestones M3 & M4)

Target Working Directory: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\security_auditor_m3_m4_1\
Reference Specification: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\ORIGINAL_REQUEST.md
Project Blueprint: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\orchestrator_1\PROJECT.md

Worker Handoffs to Audit:
- c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m3_1\handoff.md
- c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m4_1\handoff.md

Mission:
Perform strict integrity and security verification. Audit for:
1. Zero Mocks & Genuine Implementation:
   - Verify NO mock substitutions, fake test returns, or hardcoded strings exist in `crates/harness` or `crates/security`.
   - Verify all security checks are real, robust, and execute genuinely.
2. SSRF & Rebinding Defenses:
   - Check if socket connection validation blocks all private/loopback/link-local/metadata IPs without bypasses.
   - Verify IPv4-mapped IPv6 and DNS rebinding protections.
3. Archive Extraction Safety:
   - Check if `safe_extract_zip` and `safe_extract_tar` enforce strict directory containment, prevent symlink escape, enforce decompression ratios, and rollback completely on error.
4. Process Lifecycle & Reparse Points:
   - Audit Windows `taskkill /F /T /PID` execution and `FILE_ATTRIBUTE_REPARSE_POINT` handling.

Write your verdict (CLEAN or INTEGRITY VIOLATION / REQUEST_CHANGES) with full evidence to:
`c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\security_auditor_m3_m4_1\handoff.md`
