# Dispatch for Worker M4 (Security Hardening Across Tools & Boundaries)

Target Working Directory: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m4_1\
Reference Specification: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\ORIGINAL_REQUEST.md
Project Blueprint: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\orchestrator_1\PROJECT.md

Mandatory Integrity Warning:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Scope & Tasks:
Milestone M4: R4 & A4.
Files Owned: `crates/security/`
1. SSRF & DNS Rebinding Defense:
   - Implement / verify URL and IP validation at socket connection points rejecting private subnets (RFC 1918: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), loopback (`127.0.0.1`, `[::1]`), link-local (`169.254.0.0/16`), and cloud metadata destinations (`169.254.169.254`).
   - Validate both IPv4 and IPv6 addresses.
2. Archive Extraction Safety:
   - In `crates/security/src/archive.rs`: validate zip and tar extraction call sites.
   - Strictly reject directory traversal (`../`), Windows drive absolute paths, NTFS alternate data streams (`:`).
   - Reject symlinks and hardlinks escaping the extraction root.
   - Enforce compression ratio limits (zip bombs) and maximum total uncompressed size limits.
   - Implement clean rollback of partially extracted files if an error or limit violation occurs during extraction.
3. Verification: Run `cargo test -p voxy-security` and ensure all security tests pass cleanly.

Write your completion report in:
`c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m4_1\handoff.md`


## 2026-10-14T03:04:16Z
[Message] timestamp=2026-10-14T03:04:16Z sender=0ad9b732-5c55-4e82-980c-1607f28d0b35 priority=MESSAGE_PRIORITY_HIGH content=You are Worker M4 for the OSMOO production mission.
Your working directory is:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m4_1\

You MUST read the original request before doing anything:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\ORIGINAL_REQUEST.md

Also read the project architecture blueprint:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\orchestrator_1\PROJECT.md

Also read your dispatch task:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m4_1\DISPATCH.md

DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Exclusive Write Ownership:
- `crates/security/`

Mission (R4 & A4):
1. SSRF & Rebinding Defense: Enforce URL/IP validation rejecting private (RFC1918), loopback (`127.0.0.1`, `[::1]`), link-local (`169.254.0.0/16`), and metadata destinations (`169.254.169.254`) at socket connection points.
2. Archive Extraction Safety: In `crates/security/src/archive.rs`, validate archive extraction against `../`, absolute paths, symlink escapes, and zip bombs (uncompressed ratio/size limits), and verify clean rollback on error.
3. Verify changes by executing `cargo test -p voxy-security` and document the output.

Write your completion report in:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m4_1\handoff.md
Maintain your liveness in:
c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.agents\teamwork\teamwork_preview_worker_m4_1\progress.md

When done, send a message to your caller notifying them that handoff.md is ready.
