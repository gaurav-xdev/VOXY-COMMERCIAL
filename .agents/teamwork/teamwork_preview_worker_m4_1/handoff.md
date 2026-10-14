# Handoff Report — Worker M4: Security Hardening Across Tools & Boundaries

## 1. Observation
- **Initial Baseline State**:
  - `crates/security/src/lib.rs` and `crates/security/src/archive.rs` provided an initial `safe_extract_zip` function with basic path validation, but lacked rollback capability when extraction failed midway through multi-file archives.
  - No tar extraction safety engine was implemented in `crates/security`.
  - No centralized SSRF / DNS rebinding socket validation engine existed in `crates/security` to fulfill contract `validate_destination_url(url: &Url) -> Result<SocketAddr, SecurityError>` specified in `orchestrator_1/PROJECT.md` line 71.
  - Baseline test run `cargo test -p voxy-security`:
    ```
    test result: ok. 165 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.24s
    ```
- **Modifications Made**:
  1. `crates/security/Cargo.toml`: Added dependencies `url = "2"` and `tar = "0.4"`.
  2. `crates/security/src/error.rs`: Added `SsrfBlocked(String)` and `InvalidDestination(String)` error variants with standard `Display` formatting.
  3. `crates/security/src/ssrf.rs`: Implemented full SSRF & DNS rebinding defense engine providing:
     - `is_ip_restricted(ip: IpAddr) -> bool`
     - `is_ipv4_restricted(v4: &Ipv4Addr) -> bool`
     - `is_ipv6_restricted(v6: &Ipv6Addr) -> bool`
     - `is_host_restricted(host: &str) -> bool`
     - `validate_destination_url(url: &Url) -> Result<SocketAddr, SecurityError>`
     - `validate_destination_url_str(url_str: &str) -> Result<SocketAddr, SecurityError>`
     - `safe_resolve_destination(host: &str, port: u16) -> Result<Vec<SocketAddr>, SecurityError>`
     - `validate_socket_addr(addr: &SocketAddr) -> Result<(), SecurityError>`
     - `safe_connect_tcp(url: &Url, timeout_duration: Duration) -> Result<(tokio::net::TcpStream, SocketAddr), SecurityError>`
  4. `crates/security/src/archive.rs`:
     - Added `ArchiveRollbackTracker` with RAII `Drop` implementation to guarantee that if any error or quota violation occurs prior to explicit commit, all created files and newly created ancestor directories are deleted in reverse order.
     - Hardened `validate_archive_path` against null bytes (`\0`), Windows UNC network paths (`\\server\share`), NTFS alternate data streams (`:`), Windows drive prefixes, and relative traversal (`../`, `..\`).
     - Added `safe_extract_tar` for safe tar archive extraction with path traversal validation, entry size checks, total size checks, symlink and special device node blocking, and clean rollback.
     - Updated `safe_extract_zip` to utilize `ArchiveRollbackTracker`, live decompression ratio tracking, and seekable archive size verification.
  5. `crates/security/src/lib.rs`: Re-exported SSRF validation functions, `safe_extract_tar`, and rollback tracker types.
  6. `crates/security/tests/security_integration_tests.rs`: Added 12 end-to-end integration tests validating SSRF rejection of loopback, RFC1918, link-local, cloud metadata, and verifying clean rollback of partially extracted zip and tar archives upon traversal or quota violation.
- **Verification Run**:
  - `cargo test -p voxy-security`:
    ```
    test result: ok. 179 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.33s
    Running tests\security_integration_tests.rs
    test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
    Total: 191 passed, 0 failed.
    ```
  - Direct dependent regression test `cargo test -p voxy-tool-calling`:
    ```
    test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.84s
    ```

## 2. Logic Chain
1. *Requirement R4 & A4 (SSRF)*: Network-facing tools must fail closed against internal targets. By inspecting both IP literals and performing single-step DNS resolution in `validate_destination_url`, we return a pinned `SocketAddr`. This eliminates Time-of-Check to Time-of-Use (TOCTOU) DNS rebinding attacks because callers connect to the validated `SocketAddr` directly rather than allowing the operating system to re-resolve the domain.
2. *IP Coverage*:
   - IPv4: Loopback (`127.0.0.0/8`), Link-Local (`169.254.0.0/16`), Cloud IMDS (`169.254.169.254`, `169.254.170.2`, `100.100.100.200`), RFC 1918 (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), CGNAT (`100.64.0.0/10`), Broadcast (`255.255.255.255`), Unspecified (`0.0.0.0/8`), Multicast (`224.0.0.0/4`), Reserved (`240.0.0.0/4`), Documentation (`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`).
   - IPv6: Loopback (`::1`), Unspecified (`::`), Link-Local (`fe80::/10`), ULA / Cloud IMDS (`fc00::/7` including `fd00:ec2::254`), Documentation (`2001:db8::/32`), Multicast (`ff00::/8`), and IPv4-mapped IPv6 (`::ffff:x.x.x.x`) which extracts the IPv4 octets and evaluates them against all IPv4 rules.
3. *Requirement R4 & A4 (Archive Extraction & Rollback)*:
   - Malicious archives often interleave innocent files before a directory traversal or zip bomb entry. Without rollback, innocent files remain extracted on disk.
   - `ArchiveRollbackTracker` records every created file and newly created directory during the extraction transaction.
   - If `tracker.commit()` is not called (e.g. error returned via `?` or panic), `Drop` invokes `rollback()`: all created files are removed, and newly created directories are removed in reverse topological order (deepest first).
   - Validated against both zip and tar formats with multi-file archives in unit and integration tests.

## 3. Caveats
- Hostname resolution in `safe_resolve_destination` depends on the host's DNS resolver via `ToSocketAddrs`. If an environment has no network connectivity or DNS server, hostname resolution will return an `InvalidDestination` error (failing safe). IP literals bypass DNS resolution and are evaluated directly.
- The archive rollback tracker only tracks files and directories created during the current extraction session; pre-existing files in the destination directory are left untouched.

## 4. Conclusion
Milestone M4 requirements (R4 & A4) are fully implemented and verified:
- SSRF and DNS rebinding defense strictly rejects private subnets, loopbacks, link-local, cloud metadata, and dangerous schemes, pinning the socket address for safe connection.
- Archive extraction safety engine validates path traversal, NTFS alternate data streams, drive letters, symlink escapes, entry size quotas, total size quotas, and compression ratios for both zip and tar formats, executing clean atomic rollback on any failure.
- All 191 security unit and integration tests pass cleanly with zero failures and zero regressions across dependent crates.

## 5. Verification Method
1. Run crate unit and integration tests:
   ```powershell
   cargo test -p voxy-security
   ```
   Expected: 179 unit tests and 12 integration tests pass (191 total, 0 failed).
2. Run dependent tool-calling tests:
   ```powershell
   cargo test -p voxy-tool-calling
   ```
   Expected: 20 tests pass, 0 failed.
3. Inspect implementation files:
   - `crates/security/src/ssrf.rs`
   - `crates/security/src/archive.rs`
   - `crates/security/tests/security_integration_tests.rs`
