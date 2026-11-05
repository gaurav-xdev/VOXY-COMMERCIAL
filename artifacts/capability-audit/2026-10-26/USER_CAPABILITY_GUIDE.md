# OSMOO User Capability Guide: Practical Everyday Usage

**Date:** 2026-10-28  
**Audience:** End-Users, Developers, System Administrators  
**Applicable Build:** `VOXY-COMMERCIAL` Desktop & Daemon Runtime

---

## 1. What You Can Say to OSMOO Right Now (Verified Working)

OSMOO processes natural spoken requests and text commands through its integrated voice pipeline and LLM orchestrator. Below are real-world commands that trigger verified production workflows:

### A. Desktop Automation & System Control

| What to Say / Type | Under-the-Hood Action | Expected Result |
| :--- | :--- | :--- |
| *"Open Notepad"* or *"Launch Chrome"* | Tool: `open_app(name="notepad")` | Opens the application via Windows shell execution. |
| *"Focus Visual Studio Code"* | Tool: `focus_app(name="Visual Studio Code")` | Brings the VS Code window to the foreground. |
| *"Close Calculator"* | Tool: `close_app(name="Calculator")` | Sends Win32 close message to Calculator window. |
| *"Minimize all windows"* or *"Show desktop"* | Tool: `minimize_all()` | Emulates Win+D to minimize all active windows. |
| *"Take a screenshot"* | Tool: `screenshot()` | Captures primary display to PNG in the working directory. |
| *"Mute volume"* or *"Turn volume up"* | Tool: `volume(action="mute" / "up")` | Adjusts Windows master volume via endpoint audio API. |
| *"Lock my screen"* | Tool: `lock_screen()` | Calls Windows `LockWorkStation` API to secure desktop. |
| *"STOP ALL ACTIONS"* | Emergency Stop Trigger | Instantly trips `EmergencyStopGuard`, halting all automation. |

### B. Software Engineering & Coding

| What to Say / Type | Under-the-Hood Action | Expected Result |
| :--- | :--- | :--- |
| *"Inspect this repository and fix all compiler warnings in crates/audio"* | Tool: `execute_coding_task(prompt=..., workspace_root=".")` | Initializes coding harness, plans task, runs `cargo check`, applies patches, validates fix. |
| *"Review my latest changes for security vulnerabilities"* | Tool: `execute_coding_task(prompt="run security review")` | Runs `voxy-coding-harness` reviewer, checking for path leaks, unsafe blocks, and SSRF flaws. |
| *"Index symbols in this project"* | Coding Harness `SymbolIndex` | Builds an AST symbol table of all structs, functions, and traits. |

### C. File & Process Diagnostics

| What to Say / Type | Under-the-Hood Action | Expected Result |
| :--- | :--- | :--- |
| *"List files in src"* | Tool: `file_list(path="src")` | Returns file names, sizes, and timestamps within workspace. |
| *"Read the contents of config.toml"* | Tool: `file_read(path="config.toml")` | Safely reads text file subject to `PathGuard` sandboxing. |
| *"Show me what processes are using high memory"* | Tool: `process_list(filter_name=...)` | Returns top processes sorted by PID and RAM consumption. |

### D. Memory & Persistent Context

| What to Say / Type | Under-the-Hood Action | Expected Result |
| :--- | :--- | :--- |
| *"Remember that my preferred compiler flag is --release"* | Tool: `memory_store(key="pref:flag", value="--release")` | Writes key-value pair to persistent SQLite memory. |
| *"What compiler flags do I prefer?"* | Tool: `memory_recall(query="compiler flags")` | Queries SQLite memory via semantic/vector ranking. |

---

## 2. Prerequisites & Setup Instructions

To unlock full functionality, configure the following settings:

### Environment Variables
Configure your model preferences in `.env` or system environment:

```powershell
# Select LLM Provider: "ollama", "openai", "anthropic", "gemini", "groq", "openrouter", "nvidia"
$env:VOXY_LLM_PROVIDER = "anthropic"

# Set Corresponding API Key:
$env:VOXY_API_KEYS_ANTHROPIC = "sk-ant-api03-..."
$env:VOXY_ANTHROPIC_MODEL = "claude-sonnet-4-20250514"

# Optional: Strict Zero-Fallback Enforcement
$env:VOXY_ZERO_FALLBACK = "1"
$env:VOXY_STRICT_MODEL_ROUTING = "1"

# Voice Settings
$env:VOXY_VOICE_CHUNK_MS = "40"
$env:VOXY_VOICE_SAMPLE_RATE = "16000"
```

### Local Offline Mode (Zero Cloud)
If running completely offline:
1. Ensure Ollama is installed and running: `ollama run gpt-oss:120b-cloud` (or `llama3.2`).
2. Set `$env:VOXY_LLM_PROVIDER = "ollama"`.
3. Set `$env:VOXY_OLLAMA_URL = "http://127.0.0.1:11434"`.

---

## 3. What Will Fail Explicitly (Unsupported Operations)

If you attempt the following requests, OSMOO will inform you of its limitations:

1. **"Book a flight to San Francisco tomorrow"**
   - *Result:* OSMOO will report that travel booking and flight reservation APIs are not connected.
2. **"Create a PowerPoint presentation about Q3 sales"**
   - *Result:* OSMOO can author a Markdown outline or generate slides as text, but cannot compile a `.pptx` binary file.
3. **"Check my Google Calendar for upcoming meetings"**
   - *Result:* OSMOO only maintains an in-memory calendar session. It cannot synchronize with Google Calendar or Outlook without external CalDAV/OAuth integration.
4. **"Check my inbox for emails from Alice"**
   - *Result:* OSMOO does not have an IMAP/SMTP personal email client configured.
