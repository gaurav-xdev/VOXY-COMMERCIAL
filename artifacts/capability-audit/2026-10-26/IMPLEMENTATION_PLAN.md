# OSMOO Engineering Roadmap: Closing Capability Gaps

**Date:** 2026-10-28  
**Objective:** High-Priority Engineering Plan to Close Discrepancies Between Advertised & Implemented Features

---

## Priority 1: Native Office Document Compilation (DOCX, XLSX, PPTX)

### Objective
Enable OSMOO to compile native binary Microsoft Office files directly from LLM structured output without requiring installed desktop applications.

### Tasks
1. **Word Document Compiler (`crates/office/src/docx.rs`):**
   - Integrate `docx-rs` crate.
   - Implement structured Markdown-to-DOCX AST converter (headings, bullet points, code blocks, tables).
   - Expose tool: `office_docx_create(path, title, sections)`.
2. **Excel Spreadsheet Compiler (`crates/office/src/xlsx.rs`):**
   - Integrate `umya-spreadsheet` or `rust_xlsxwriter`.
   - Implement table/data-frame serializer with auto-column width, header formatting, and basic sum formulas.
   - Expose tool: `office_xlsx_create(path, sheet_name, headers, rows)`.
3. **PowerPoint Presentation Compiler (`crates/office/src/pptx.rs`):**
   - Implement structured slide deck generator using OpenXML slide templates.
   - Expose tool: `office_pptx_create(path, title, slides)`.

---

## Priority 2: Persistent Calendar Engine & Cloud Sync

### Objective
Upgrade OSMOO's calendar from an ephemeral in-memory vector to a persistent, synchronized schedule manager.

### Tasks
1. **SQLite Storage Backend (`crates/tool_calling/src/tools/calendar.rs`):**
   - Replace `Arc<RwLock<Vec<CalendarEvent>>>` with SQLite table schema (`events`: `id`, `title`, `start_time`, `end_time`, `recurrence`, `status`).
2. **Google Calendar & Outlook CalDAV Sync:**
   - Implement OAuth 2.0 PKCE flow for Google Calendar API.
   - Add background sync loop checking for meeting updates every 15 minutes.
   - Add natural language conflict resolution ("You have a conflict at 2 PM with Team Sync").

---

## Priority 3: User Email Client with Draft-and-Confirm Security

### Objective
Allow OSMOO to assist users with their personal inboxes while enforcing strict approval controls to prevent unauthorized email dispatch.

### Tasks
1. **IMAP Thread Ingestion Engine (`crates/mail_client/`):**
   - Implement async IMAP connection pool using `async-imap` + `rustls`.
   - Parse MIME headers, plain text, and attachment metadata.
2. **SMTP Dispatch with Mandatory Approval:**
   - Integrate `lettre` for secure SMTP submission.
   - Wire all outbound emails into `ApprovalBroker`. Outbound emails are held in `draft` state until the user reviews recipient, subject, and body in the UI and clicks "Approve".

---

## Priority 4: Headless Browser Operator (Playwright / CDP)

### Objective
Upgrade web research from basic HTTP GET text extraction to full dynamic web page interaction.

### Tasks
1. **Chrome DevTools Protocol (CDP) Bridge:**
   - Integrate headless Chromium control via CDP WebSockets.
2. **Interactive DOM Exploration:**
   - Capture element bounding boxes, click dynamic buttons, fill multi-step forms, and scroll infinite feeds.
   - Enforce `SSRFGuard` and domain allowlisting before navigation.
