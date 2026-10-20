# OSMOO Real Runtime Verification & Environmental Evidence

**Audit Date:** 2026-10-20  
**Target Binary:** `target\release\OSMOO.exe`  
**Host Machine:** Windows 11 Pro 64-bit (`Microsoft Windows NT 10.0.26200.0`)

---

## 1. Release Binary Metadata & Verification

- **File Path**: `c:\Users\gaurav\Downloads\VOXY-COMMERCIAL\target\release\OSMOO.exe`
- **File Size**: `9,211,904 bytes` (8.78 MB)
- **Compilation Timestamp**: `2026-10-20 12:11:06`
- **Cargo Profile**: `release` (LTO enabled, opt-level 3, codegen-units 1)
- **SHA-256 Hash**:
  `EDBF0418970B52D0FC1F7A4BFC250DE30607EAB1E150C3F754A787608335AB20`

---

## 2. CLI Execution Observations

### Invocation A: Command-Line Help
```powershell
PS C:\Users\gaurav\Downloads\VOXY-COMMERCIAL> .\target\release\OSMOO.exe --help
Loaded .env from C:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.env
OSMOO — Windows AI Operating Companion v0.1.0
Usage: OSMOO.exe [OPTIONS]

Options:
  -h, --help           Show this help message
  -v, --version        Show application version
  --status             Display supervisor and component status
  --daemon-only        Run background daemon under supervision without GUI
  --no-daemon          Launch UI without auto-spawning background daemon
  --no-overlay         Launch without auto-spawning overlay
  -a, --autostart      Start minimized to system tray
```
- **Exit Code**: `0`
- **Result**: Help parsed, flags displayed accurately, no panics.

### Invocation B: Real-Time Supervisor & Job Object Status
```powershell
PS C:\Users\gaurav\Downloads\VOXY-COMMERCIAL> .\target\release\OSMOO.exe --status
Loaded .env from C:\Users\gaurav\Downloads\VOXY-COMMERCIAL\.env
2026-10-20T06:42:11.739939Z  INFO OSMOO: Starting OSMOO Desktop UI v0.1.0
2026-10-20T06:42:11.740065Z  INFO OSMOO::supervisor: Initialized Windows Job Object with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
{
  "daemon_running": false,
  "daemon_pid": null,
  "overlay_running": false,
  "overlay_pid": null,
  "pipe_name": "\\\\.\\pipe\\voxy-com-ipc",
  "is_shutting_down": false,
  "job_object_active": true
}
```
- **Exit Code**: `0`
- **Result**: Confirms Windows Win32 Job Object successfully initialized with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`.

---

## 3. Runtime Lifecycle & Supervision State

- **Supervisor State Interpretation**:
  - `job_object_active: true`: The process successfully called `CreateJobObjectW` and set extended limit information.
  - `daemon_running: false`: In `--status` mode, the supervisor checks status without launching the background daemon unless requested.
  - `pipe_name: "\\\\.\\pipe\\voxy-com-ipc"`: The canonical local Windows named pipe endpoint.

---

## 4. Hardware & Interactive Windows Desktop Dependencies

1. **Interactive Desktop Window Station**:
   - `OSMOO.exe` uses Dioxus with `wry` and `webview2-com` (Edge Chromium engine).
   - In non-interactive headless agent sessions or CI containers lacking an interactive Windows desktop window station (`WinSta0\Default`), spawning the GUI window will block or error if no desktop shell is attached.
   - For non-interactive automation, `OSMOO.exe --status` and `OSMOO.exe --daemon-only` function without a display.
2. **Audio Capture Hardware**:
   - WASAPI capture and playback loops were verified against the OS audio subsystem. In environments with no physical microphone connected, the WASAPI stream reports device unavailable, and the voice pipeline operates in degraded mode (logging a structured `degraded_reason`).
