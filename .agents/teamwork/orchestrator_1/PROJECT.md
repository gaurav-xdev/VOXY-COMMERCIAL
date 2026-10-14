# Project: OSMOO (Operating System Machine Optimization Operator)

## Architecture
OSMOO is a Windows-native AI operating companion structured as a modular Cargo workspace containing applications (`apps/daemon`, `apps/overlay`, `crates/desktop_ui`) and 79 backend library crates (`crates/*`).
- **Core Entry Point**: `crates/desktop_ui` (`OSMOO.exe`), coordinating user interface, system tray, and background daemon supervision.
- **IPC Bus**: Windows Named Pipe (`\\.\pipe\voxy-com-ipc`) providing authenticated local messaging between desktop UI, overlay, and background daemon.
- **Voice Engine**: Full duplex WASAPI audio loop (`crates/audio`, `crates/voice_orchestrator`) connecting microphone capture, noise suppression/VAD, STT router (local Whisper + Cloud STT), LLM streaming, TTS chunking (Kokoro / Cloud), and playback with real-time user speech interruption.
- **Autonomous Coding Harness**: Controlled execution and code modification system (`crates/harness`) featuring AST indexing, atomic multi-file patch transactions, automatic rollback, process-tree kill (`taskkill /F /T /PID`), emergency stop tokens, and secret detection.
- **Security Perimeter**: Hardened defense system (`crates/security`) with SSRF/DNS rebinding network validator, safe archive extraction engine (`safe_extract_zip`), and access control policies.
- **Durable Persistence**: SQLite database (`crates/database`, `crates/memory`) operating in WAL mode with connection pooling, schema migrations, conversation history indexing, and full-text search.
- **Release & Installer**: NSIS installer packaging (`installer/nsis/osmoo.nsi`) generating `OSMOO-Setup-x64.exe` with SHA-256 verification and clean uninstaller preserving user data.

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Unified Windows Entry Point | Single executable (`OSMOO.exe`) launching desktop UI, system tray, and supervisor | M1 | R1, A1 |
| 2 | Background Daemon Supervision | Reliable supervisor lifecycle for background service and overlay, no orphan processes | M1 | R1, A1 |
| 3 | Authenticated Named Pipe IPC | Local pipe `\\.\pipe\voxy-com-ipc` with message validation, timeouts, reconnect, clean shutdown | M1 | R1, A1 |
| 4 | DPI & Multi-Monitor Support | Multi-monitor coordinates, DPI awareness, and crash telemetry | M1 | R1 |
| 5 | WASAPI Audio Loop | Low-latency audio capture and playback via Windows WASAPI without deadlock | M2 | R2, A2 |
| 6 | Noise Suppression & VAD | Audio preprocessing, voice activity detection, and wake-word activation | M2 | R2 |
| 7 | Hybrid STT & TTS Pipeline | Local Whisper / Kokoro and Cloud voice streaming with token/audio chunking | M2 | R2 |
| 8 | Interruption Handling | Immediate halt of TTS playback and streaming LLM tokens on user speech | M2 | R2, A2 |
| 9 | Hardware Fault Tolerance | Graceful handling of device disconnection, permission errors, and buffer underruns | M2 | R2, A2 |
| 10 | Repository Indexing & AST Symbols | Workspace file discovery and semantic symbol extraction across languages | M3 | R3, A3 |
| 11 | Atomic Patching & Rollback | Transactional multi-file patch application, diff computation, and atomic rollback | M3 | R3, A3 |
| 12 | Sandboxed Process Tree Kill | Subprocess execution with Windows `taskkill /F /T /PID`, timeouts, and emergency stop | M3 | R3, A3 |
| 13 | Path Traversal & Protected Files | Strict rejection of `../`, symlinks/reparse points, and protected files (`.git`, `Cargo.lock`) | M3 | R3, A3 |
| 14 | Secret Detection | Pre-write scanner blocking AWS keys, SSH keys, GitHub tokens, and API credentials | M3 | R3, A3 |
| 15 | SSRF & DNS Rebinding Defense | Socket-level validation rejecting loopback, link-local (169.254.169.254), private subnets, metadata | M4 | R4, A4 |
| 16 | Archive Extraction Safety | Safe zip/tar extraction blocking zip-slips (`../`), symlink escapes, ratio/size bombs, rollback | M4 | R4, A4 |
| 17 | Defense in Depth & Permissions | Named pipe ACL auditing, file permissions, and credential masking | M4 | R4 |
| 18 | SQLite WAL Persistence | SQLite initialized with PRAGMA journal_mode=WAL, survive restarts | M5 | R5, A5 |
| 19 | Schema Migrations & Indexing | Automatic database migrations, conversation indexing, and full-text search (FTS) | M5 | R5 |
| 20 | Connection Pooling & Safety | Concurrent connection pool, explicit error handling, zero silent volatile fallbacks | M5 | R5 |
| 21 | Workspace Release Alignment | Cargo release profile (`opt-level=3`, `lto=true`, `OSMOO.exe` binary name) | M6 | R6, A5 |
| 22 | NSIS Installer Packaging | NSIS script `osmoo.nsi` generating `OSMOO-Setup-x64.exe` with SHA-256 hash | M6 | R6, A5 |
| 23 | Upgrade & Legacy Migration | In-place upgrade, desktop shortcuts, migration from legacy VOXY paths, uninstaller | M6 | R6 |
| 24 | E2E Acceptance Verification | Comprehensive end-to-end test verification passing 100% of Acceptance Criteria A1-A5 | M7 | A1-A5 |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Windows App Lifecycle & IPC | Unified `OSMOO.exe`, supervisor for daemon/overlay, authenticated `\\.\pipe\voxy-com-ipc` | none | IN_PROGRESS |
| M2 | Production Voice Pipeline | WASAPI loop, noise/VAD, STT/TTS streaming, interruption handling, hardware recovery | none | IN_PROGRESS |
| M3 | Hardened Coding Harness | Repo indexing, AST symbols, atomic patch engine, process tree kill, secret scanner | none | DONE |
| M4 | Security Hardening Across Tools | SSRF & DNS rebinding socket validation, safe archive extraction & zip bomb limits | none | DONE |
| M5 | Memory & Durable Persistence | SQLite WAL mode, schema migrations, conversation indexing, connection pooling | none | IN_PROGRESS |
| M6 | Release Pipeline & Installer | Workspace release profile, `OSMOO-Setup-x64.exe` packaging, SHA-256, upgrade/uninstall | M1, M5 | PLANNED |
| M7 | E2E Acceptance Verification | Pass 100% of E2E verification test suite across A1, A2, A3, A4, A5 | M1, M2, M3, M4, M5, M6 | PLANNED |

## Interface Contracts
### Desktop UI (`crates/desktop_ui`) ↔ Daemon (`apps/daemon`)
- Pipe Name: `\\.\pipe\voxy-com-ipc`
- Protocol: JSON-RPC / typed IPC messages defined in `crates/ipc`
- Handshake: Mutual authentication token validation on connection open
- Heartbeat: Ping/Pong every 5 seconds, 15 second timeout triggers graceful reconnection
- Termination: Shutdown signal initiates graceful worker drainage; supervisor forces `taskkill` if unresponsive after 5 seconds

### Voice Orchestrator (`crates/voice_orchestrator`) ↔ Audio (`crates/audio`)
- Capture Stream: WASAPI 16kHz 16-bit mono PCM buffer queue
- Output Stream: WASAPI 24kHz / 48kHz stereo PCM buffer queue
- Interruption: `InterruptionSignal` immediately drains output buffer and aborts in-flight token reader

### Coding Harness (`crates/harness`) ↔ File System & Process
- `PatchTransaction`: `apply() -> Result<Vec<Diff>, PatchError>`, `rollback() -> usize`
- `SandboxedRunner`: `run_command(...) -> Result<CommandOutput, RunnerError>` with recursive process tree termination on timeout or emergency abort

### Security (`crates/security`) ↔ Tools & Archive
- `safe_extract_zip(reader, dest, limits) -> Result<usize, ArchiveSecurityError>` with rollback on error
- `validate_destination_url(url: &Url) -> Result<SocketAddr, SecurityError>` blocking RFC1918, loopback, link-local, cloud metadata

## Code Layout
- `apps/daemon/`: Background service and agent daemon (`voxy-daemon.exe`)
- `apps/overlay/`: Lightweight desktop overlay window (`voxy-overlay.exe`)
- `crates/desktop_ui/`: Main user interface and supervisor entry point (`OSMOO.exe`)
- `crates/ipc/`: Windows Named Pipe communication protocol and client/server implementation
- `crates/audio/`: WASAPI low-latency audio capture and playback implementation
- `crates/voice_orchestrator/`: Full voice lifecycle coordination, VAD, and interruption
- `crates/harness/`: Autonomous coding harness, patch engine, and process sandbox
- `crates/security/`: Security guards, SSRF protection, archive extraction safety
- `crates/database/`: SQLite persistence, migrations, and conversation indexing
- `installer/nsis/`: NSIS scripts (`osmoo.nsi`, `voxy.nsi`)
- `installer/OSMOO/`: Release packaging directory for `OSMOO-Setup-x64.exe`
- `tests/`: Integration and acceptance verification test suites
