# OSMOO v0.1.0 Release Notes

**Package:** `OSMOO-Setup.exe`  
**Platform:** Windows 10 / 11 (64-bit x86_64)  
**Release Date:** November 1, 2026  
**Publisher:** OSMOO Technologies  
**SHA-256 Checksum:** `c0bbc16b97c8a6280375fff5a3fde889c617b4dd89bdea640480dba00b4e7ab9`

---

## Overview

OSMOO is a native Windows AI Operating Companion designed to assist with desktop productivity, research, multi-modal reasoning, software engineering, and document authoring.

This release features a completely rebuilt first-run experience with mandatory provider configuration, fail-closed desktop overlay gating, and an animated, layered 2D cat companion rigged directly from official pixel artwork.

---

## What's New in This Release

### 1. Mandatory 5-Step First-Run Onboarding Wizard
- **Gated Launch:** On first run (absence of `provisioned.flag`), OSMOO strictly gates the desktop overlay and companion interactions until configuration is completed.
- **Step 1: Welcome & Identity:** Official branding, custom companion naming (default: OSMOO).
- **Step 2: AI Provider & Model Configuration:**
  - Support for local Ollama (`llama3.2`, `llama3.1`, `qwen2.5`, `mistral`) and cloud endpoints (OpenAI, Anthropic, Gemini, Groq).
  - Real-time connection testing (`TestProviderConnection`) that pings provider endpoints, validates credentials, and reports latency before continuing.
  - Strict zero-fallback routing guarantees.
- **Step 3: Voice Engine & Permissions:**
  - Audio input & output device selection with instant speaker playback test.
  - Push-to-talk (`Ctrl+Space`) and Wake-word mode configuration.
- **Step 4: Companion Preferences:**
  - Interactive live preview of the rigged 2D companion with real-time responsive scaling (75%–135%).
  - Toggles for Autonomous Desktop Walking, Proactive Spoken Suggestions, and Windows Startup.
- **Step 5: Subsystem Verification & Finish:**
  - 4-point verification checklist covering local database initialization, DPAPI credential encryption, IPC socket binding, and model router readiness.
  - Atomic persistence: Writes `provisioned.flag` and encrypted configuration to DPAPI before launching the desktop overlay.

### 2. Layered Rigged 2D Cat Companion Overlay
- **Authentic Pixel-Art Character:** Rigged directly from official artwork (`media_1793272405006_66f9b5ef.png`) featuring the cat in dark sunglasses and black "osmoo" t-shirt.
- **Segmented 2D Animation Hierarchy:**
  - Tail: Anchored to torso base with continuous organic sine-wave swish keyframes.
  - Torso: Subtle breathing animation with soft contact drop shadow.
  - Head: Fluid idle tilt and attentive postures.
  - Ears: Asynchronous organic twitches (left: 4.8s cycle, right: 6.2s cycle).
  - Sunglasses & Eyes: Specular glint animations and organic eyelid blinking (3.2–5.6s intervals).
  - Dynamic Mouth Visemes: 6 SVG viseme shapes (`closed`, `slight`, `medium`, `wide`, `smile`, `pout`) driven by smoothed real-time audio RMS energy during TTS playback.
- **Autonomous Desktop Movement:**
  - Smooth walking state machine along the bottom of the active display with walking bob and tail sway.
  - Dynamic direction flipping (faces direction of movement).
  - Screen boundary clamping and idle pause cycles.
  - Toggleable at any time via the companion right-click context menu.
- **Interactive Drag & Drop:**
  - Reposition anywhere on screen with physics settle hop upon release.
  - Scale adjustments (75%, 100%, 135%) and persistent coordinates.
- **Desktop Context Menu:**
  - Right-click anywhere on the companion overlay to access quick settings: Open Dashboard, Toggle Mic, Toggle Autonomous Walking, Resize, Always on Top, Emergency Halt, and Exit.

### 3. Core Engine & Production Hardening
- **Process Supervisor:** Windows Job Objects with `KILL_ON_JOB_CLOSE` ensure zero zombie processes across daemon, overlay, and main UI.
- **SSRF & Security Isolation:** Strict DNS rebinding protection with socket-bound DNS resolution (`SsrfSafeDnsResolver`).
- **OpenXML Document Generation:** Native creation and validation of `.docx`, `.xlsx`, and `.pptx` documents without external Office installations.

---

## Distributable Package Contents

`OSMOO Installer/` contains:
- `OSMOO-Setup.exe`: Standalone unified Windows installer (embeds `payload.zip` containing all release binaries, rigged 2D cat assets, official icons, and default configuration).
- `SHA256SUMS.txt`: Cryptographic SHA-256 hash for package verification.
- `RELEASE_NOTES.md`: This document.

---

## Installation & CLI Flags

Run `OSMOO-Setup.exe` directly or via command line:

```powershell
# Interactive GUI installation:
.\OSMOO-Setup.exe

# Unattended silent installation:
.\OSMOO-Setup.exe --silent

# Custom installation directory:
.\OSMOO-Setup.exe --dir "C:\Tools\OSMOO" --silent

# Unattended uninstallation:
.\Uninstall.exe --uninstall --silent
```

### Uninstallation
To uninstall, navigate to Windows **Settings > Apps > Installed apps**, find **OSMOO AI Operating Companion**, and click **Uninstall**, or execute `Uninstall.exe` in the application directory.
Shortcuts, registry keys, and application binaries are cleanly uninstalled; user data and database files in `%APPDATA%\osmoo` are safely preserved.
