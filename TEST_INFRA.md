# OSMOO E2E Test Infrastructure Specification

## 1. Overview & Architectural Philosophy

The OSMOO End-to-End (E2E) Test Suite provides a rigorous, 4-tier opaque-box verification framework for the commercial OSMOO desktop voice and autonomous coding agent. Designed specifically for Windows x64 environments, it tests complete operational paths, security boundary defenses, cross-module concurrency, and deployment artifacts.

### Guiding Principles:
1. **Opaque-Box Verification**: The test suite interacts with public module APIs, Windows Named Pipe transports (`\\.\pipe\...`), filesystem sandboxes, audio buffers, and SQLite persistence layers without mocking core business logic or bypassing security filters.
2. **Deterministic Independence**: Every test generates unique, collision-free resource identifiers (UUID-based named pipes, isolated temporary workspace directories, distinct SQLite databases) and performs automated cleanup upon termination.
3. **Fail-Closed Security Verification**: Boundary tests actively mount adversarial attacks—including Zip Slip directory traversal, NTFS Alternate Data Streams (`:stream`), path traversal (`../`), AWS/OpenAI credential leakage, and SSRF private IP access—to ensure defenses fail closed with structured errors.

---

## 2. Directory Layout & Test Suite Structure

The test suite is located in `tests/e2e/` configured as an independent test crate with a standalone workspace manifest (`[workspace]`):

```
tests/e2e/
├── Cargo.toml                          # Test suite dependencies & test target definitions
├── src/
│   └── lib.rs                          # Shared fixtures, factories, and utility helpers
└── tests/
    ├── e2e_acceptance_all.rs           # Unified acceptance runner validating A1 through A5
    ├── tier1_feature_coverage.rs       # Tier 1: Core operational path coverage (A1-A5)
    ├── tier2_boundary_corner.rs        # Tier 2: Adversarial & boundary corner cases (A1-A5)
    ├── tier3_cross_feature.rs          # Tier 3: Compound multi-system interactions (A1-A5)
    └── tier4_real_world.rs             # Tier 4: Real-world operational scenarios (A1-A5)
```

---

## 3. Shared Fixtures & Testing Utilities (`src/lib.rs`)

`osmoo-e2e-tests` provides centralized, production-grade test utilities:

| Helper Function | Purpose |
|-----------------|---------|
| `workspace_root() -> PathBuf` | Dynamically resolves the root repository directory across execution contexts. |
| `generate_test_pipe_name() -> String` | Creates collision-free named pipe paths (e.g. `\\.\pipe\osmoo-test-<uuid>`). |
| `generate_test_auth_token() -> String` | Produces cryptographically unique shared secrets for IPC handshakes. |
| `compute_client_auth_token(...) -> String` | Generates HMAC / SHA-256 tokens matching the server's timestamped authentication protocol. |
| `create_mock_workspace() -> TempDir` | Constructs a temporary workspace complete with `.git/config`, `src/main.rs`, `Cargo.toml`, `Cargo.lock`, and `.env`. |
| `create_synthetic_audio_chunk(...) -> Vec<f32>` | Synthesizes pure sine-wave PCM audio buffers for VAD and voice pipeline verification. |
| `create_test_zip_archive(...) -> Vec<u8>` | Compiles in-memory zip archives containing specified paths and binary payloads. |
| `create_traversal_zip_archive(...) -> Vec<u8>` | Generates Zip Slip adversarial archives targeting directory traversal (`../../`). |
| `compute_sha256(...) -> Result<String>` | Computes SHA-256 hashes of build artifacts and installer executables. |

---

## 4. 4-Tier Test Matrix Mapping

### Tier 1: Feature Coverage (Standard Paths)
- **A1**: Daemon Named Pipe server startup, client authentication handshake, bidirectional state streaming, snapshot delivery, and graceful shutdown.
- **A2**: Voice pipeline lifecycle, initialization idempotency, hardware fault signaling (`VoiceError::CaptureError`), and sentence chunking.
- **A3**: Autonomous coding repository indexing, symbol discovery, multi-file atomic patch application, and rollback.
- **A4**: SSRF IP blacklisting (`127.0.0.1`, `169.254.169.254`, `::1`) and safe zip archive extraction.
- **A5**: SQLite WAL mode persistence verification (`PRAGMA journal_mode = WAL`), conversation history CRUD, and installer NSIS definitions.

### Tier 2: Boundary & Corner Cases (Adversarial Defenses)
- **A1**: IPC unauthorized client rejection, privileged command gating (`EmergencyStop`), and replay attack rejection (> 300s timestamp drift).
- **A2**: Structured hardware error reporting for missing WASAPI audio devices, unsupported sample rates, and buffer underflows.
- **A3**: Rejecting path traversal in patches (`../`), blocking writes to protected metadata (`.git/config`, `Cargo.lock`, `.env`), and preventing secret key injection (AWS `AKIA...`).
- **A4**: SSRF filtering for IPv4-mapped IPv6, CGNAT (`100.64.0.0/10`), broadcast, and non-HTTP URL schemes (`file://`, `ftp://`). Archive rejection for Zip Slip, NTFS streams (`filename:stream`), and uncompressed entry size limits.
- **A5**: SQLite high-concurrency access across multiple threads, and strict rejection of empty database paths.

### Tier 3: Cross-Feature Interactions
- **A1 + A4**: Cryptographic authentication handshakes gating privileged client commands.
- **A2**: Barge-in interruption during active speech synthesis immediately aborting LLM token streams and audio playback with cooldown enforcement.
- **A3 + A4**: Multi-file atomic patch transactions rolling back all changes if any security constraint or secret rule is violated.
- **A5**: SQLite WAL database consistency and recovery across simulated process disconnects.

### Tier 4: Real-World Scenarios
- **Scenario 1 (Autonomous Coding Loop)**: Index repository -> apply multi-file refactoring -> parse compiler warning/error diagnostics (`rustc`) -> execute atomic rollback.
- **Scenario 2 (Voice Interaction & Barge-In)**: Audio ingestion -> Energy VAD detection -> STT transcript event -> LLM answer chunking -> TTS playback -> User interruption (Barge-in) -> immediate transition back to Listening.
- **Scenario 3 (Release Deployment & Packaging)**: NSIS script validation (`installer/nsis/osmoo.nsi`) ensuring uninstallation data preservation, product naming, and SHA-256 verification of packaged release binaries.

---

## 5. Execution Instructions

Run all E2E test suites with cargo:
```powershell
cargo test --manifest-path tests/e2e/Cargo.toml
```

Run specific test tiers:
```powershell
# Unified Acceptance Runner (A1 - A5)
cargo test --manifest-path tests/e2e/Cargo.toml --test e2e_acceptance_all

# Tier 1: Feature Coverage
cargo test --manifest-path tests/e2e/Cargo.toml --test tier1_feature_coverage

# Tier 2: Boundary & Corner Cases
cargo test --manifest-path tests/e2e/Cargo.toml --test tier2_boundary_corner

# Tier 3: Cross-Feature Interactions
cargo test --manifest-path tests/e2e/Cargo.toml --test tier3_cross_feature

# Tier 4: Real-World Scenarios
cargo test --manifest-path tests/e2e/Cargo.toml --test tier4_real_world
```
