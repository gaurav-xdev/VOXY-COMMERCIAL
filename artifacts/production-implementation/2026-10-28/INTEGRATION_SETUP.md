# OSMOO Production Integration & Setup Guide

**Date:** October 28, 2026  
**Target Environments:** Windows 10/11 x64, macOS, Linux  

---

## 1. Storage & Database Layout

OSMOO uses local-first persistent storage. All office tools, calendar stores, and audit logs default to the platform data directory:

### Windows Paths:
- **Calendar SQLite Database:** `%LOCALAPPDATA%\voxy\calendar\calendar.db`
- **Audit Trail Log:** `%LOCALAPPDATA%\voxy\audit\office_audit.jsonl`
- **Document Output Directory:** User specified (e.g. `C:\Users\<user>\Documents\` or workspace-relative `./output/`)

### Linux/macOS Paths:
- **Calendar SQLite Database:** `~/.local/share/voxy/calendar/calendar.db`
- **Audit Trail Log:** `~/.local/share/voxy/audit/office_audit.jsonl`

---

## 2. Environment Variables & OAuth Configuration

To enable cloud synchronization for Google Calendar and Microsoft Outlook 365, provide the following environment variables (or define them in `.env`):

```bash
# ==========================================
# Google Calendar & Gmail Integration
# ==========================================
OSMOO_GOOGLE_CLIENT_ID="<your-google-client-id>.apps.googleusercontent.com"
OSMOO_GOOGLE_CLIENT_SECRET="<your-google-client-secret>"
OSMOO_GOOGLE_REDIRECT_URI="http://localhost:8989/oauth/callback/google"
OSMOO_GOOGLE_SCOPES="https://www.googleapis.com/auth/calendar.events https://www.googleapis.com/auth/gmail.modify"

# ==========================================
# Microsoft Outlook & Office 365 Integration
# ==========================================
OSMOO_MS_TENANT_ID="common"
OSMOO_MS_CLIENT_ID="<your-azure-app-client-id>"
OSMOO_MS_CLIENT_SECRET="<your-azure-app-client-secret>"
OSMOO_MS_REDIRECT_URI="http://localhost:8989/oauth/callback/microsoft"
OSMOO_MS_SCOPES="Calendars.ReadWrite Mail.ReadWrite Mail.Send"

# ==========================================
# Payment Sandbox Credentials (Optional)
# ==========================================
OSMOO_STRIPE_SANDBOX_KEY="sk_test_..."
OSMOO_DODO_SANDBOX_KEY="dodo_test_..."
```

*(Note: In the absence of live OAuth tokens, OSMOO operates in local-first mock/offline mode with full SQLite persistence and full OpenXML document generation.)*

---

## 3. Starting the Daemon with Builtin Tools

To run the daemon with all native office, calendar, email, and desktop control capabilities active:

```powershell
# From the repository root
cargo run -p voxy-daemon
```

### Checking Daemon Health & Tool Availability:
When the daemon starts up, it registers all tools into the central `ToolRegistry`. The LLM receives the complete tool definitions automatically in its system prompt.

---

## 4. Triggering Tools from the Coding Agent or LLM

The LLM can invoke any of the native tools using structured JSON formatting:

### 4.1 Create a Word Document:
```json
TOOL: {"tool": "office_docx_create", "params": {
  "path": "C:/Reports/Q3_Summary.docx",
  "title": "Quarterly Performance",
  "headings": ["Executive Summary", "Key Highlights"],
  "paragraphs": ["Revenue exceeded expectations by 18%."],
  "bullets": ["Growth: 24%", "Retention: 98%"],
  "table_headers": ["Quarter", "Revenue ($M)", "Growth"],
  "table_rows": [["Q1", "4.2", "+12%"], ["Q2", "5.1", "+21%"]]
}}
```

### 4.2 Schedule a Calendar Event with Recurrence:
```json
TOOL: {"tool": "calendar_create_event", "params": {
  "title": "Weekly Sprint Planning",
  "start_time": "2026-11-02T10:00:00Z",
  "duration_minutes": 60,
  "recurrence": "weekly",
  "location": "Virtual Room Alpha"
}}
```

### 4.3 Search Email and Draft a Response:
```json
TOOL: {"tool": "email_search_inbox", "params": {"query": "contract renewal", "max_results": 5}}
```
```json
TOOL: {"tool": "email_create_draft", "params": {
  "to": ["client@example.com"],
  "subject": "Re: Contract Renewal 2027",
  "body": "Hi Jane,\n\nWe would love to extend for another year at the agreed terms."
}}
```
*(Sending will pause until explicit human approval is confirmed in the UI or via security token.)*

### 4.4 Search Flights & Hotels:
```json
TOOL: {"tool": "travel_search_flights", "params": {
  "origin": "SFO",
  "destination": "JFK",
  "date": "2026-11-20",
  "passengers": 1
}}
```
