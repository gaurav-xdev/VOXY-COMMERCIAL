# Original User Request

## Initial Request — 2026-10-13T16:18:19Z

OSMOO (Operating System Machine Optimization Operator) is a Windows-native AI operating companion developed by Osmiora Computational Systems. This mission takes the existing 50+ crate repository from its current state to a robust, fully integrated, and verified production build with complete Windows audio pipeline, daemon/UI supervision, secure coding harness, reliable persistence, and a verified installer.

Working directory: c:\Users\gaurav\Downloads\VOXY-COMMERCIAL
Integrity mode: development
Requested team: Full team (Build, Systems, Security, QA, Release)

## Requirements

### R1. Windows Application Lifecycle, Daemon Supervision & IPC
- Unify the application into one coherent Windows entry point (`apps/desktop_ui` / `installer/OSMOO/OSMOO.exe`).
- Provide reliable supervisor lifecycle for the background daemon, system tray, and overlay.
- Enforce IPC authentication via local Windows Named Pipe (`\\.\pipe\voxy-com-ipc`), message validation, timeouts, graceful reconnection, and clean shutdown without orphaned processes.
- Ensure DPI-aware rendering, multi-monitor support, and clean crash reporting.

### R2. Production-Grade Voice Pipeline (Local & Cloud)
- Connect the complete end-to-end voice loop: Microphone Capture (WASAPI) → Audio Processing/Noise Suppression → VAD/Wake-word → STT → LLM Streaming Context → TTS Chunking → Windows Audio Playback.
- Implement both Local Voice (configurable local STT/TTS models, VRAM/RAM aware resource management) and Cloud Voice (secure credential storage, streaming tokens/audio, exponential retries, cancellation/interruption on user speech).
- Implement hardware fault tolerance: handle device disconnection, microphone permission errors, audio-device changes, and buffer underruns gracefully.

### R3. Hardened Autonomous Coding Harness & Agent System
- Support repository indexing, AST symbol discovery, diff generation, atomic patch transactions, and safe rollback.
- Integrate runtime command execution with process-tree termination (`taskkill /F /T /PID` on Windows), timeouts, and emergency-stop tokens.
- Enforce strict security boundaries: prevent path traversal / symlink / reparse-point escapes, block protected configuration modification (`.git`, `Cargo.lock`), and detect credentials (AWS, GitHub, API keys) before write.

### R4. Security Hardening across Tools & Boundaries
- SSRF & Rebinding Defense: Enforce URL/IP validation rejecting private, loopback, link-local, and metadata destinations at actual socket connection points across browser/network tools.
- Archive Extraction Safety: Validate zip/tar extraction call sites against directory traversal, zip bombs (uncompressed ratio/size limits), symlink containment escapes, and clean rollback on corruption.
- Defense in Depth: Audit named pipes, shell execution, database file permissions, and credential masking.

### R5. Memory & Durable Persistence
- Ensure SQLite WAL mode persistence with schema migrations, conversation history indexing, durable long-term user memory, and full-text search.
- Guarantee transaction safety, concurrent connection pooling, and explicit error handling without silent fallback to volatile in-memory storage.

### R6. Canonical Windows Release Pipeline & Verified Packaging
- Align Cargo workspace release profiles, executable naming (`OSMOO.exe`), and NSIS installer packaging (`installer/OSMOO/OSMOO-Setup-x64.exe`).
- Verify installation, upgrade, shortcut creation, migration from legacy VOXY paths, clean uninstallation (preserving user databases), and hash verification (SHA-256).

---

## Acceptance Criteria

### A1. Process Lifecycle & IPC
- [ ] Primary executable starts, launches daemon/tray, establishes authenticated IPC, and handles graceful shutdown.
- [ ] No orphaned child processes or zombie pipes remain upon termination.

### A2. Voice Pipeline
- [ ] WASAPI audio loop captures and plays back audio buffers without deadlock.
- [ ] Interruption signals immediately halt in-flight TTS playback and streaming LLM tokens.
- [ ] Unavailable hardware or credentials produce explicit, structured error states rather than silent mock substitutions.

### A3. Coding Harness
- [ ] End-to-end automated harness test passes: accurately parses compiler/test failures, applies atomic patch diffs, verifies passing re-tests, and executes clean rollbacks.
- [ ] Symlink/reparse-point traversal attempts and credential injections are strictly rejected with typed errors.

### A4. Security Boundaries
- [ ] URL validator rejects SSRF attempts to `127.0.0.1`, `169.254.169.254`, `[::1]`, and private RFC1918 subnets.
- [ ] Archive extraction safely rejects paths containing `../`, symlinks escaping destination, and over-quota archives.

### A5. Persistence & Packaging
- [ ] SQLite memory initializes in WAL mode and survives application restarts.
- [ ] NSIS installer produces a valid `OSMOO-Setup-x64.exe` installer with matching SHA-256 and verified silent/interactive install flows.
