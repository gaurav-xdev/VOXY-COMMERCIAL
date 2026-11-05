# OSMOO Remaining Work & Future Roadmap

**Date:** October 28, 2026  
**Phase:** Post-Implementation Hardening & Long-Term Roadmap  

---

## 1. Short-Term Enhancements (Next Sprint)

1. **Embedded Chart Generation in OpenXML Excel:**
   - Add OpenXML DrawingML charts (`chart1.xml`) to `XlsxWorkbook` to render bar, column, and pie charts directly alongside tabular financial data.
2. **Interactive OAuth Web View Flow:**
   - Embed a lightweight local loopback HTTP server (port 8989) into the Tauri frontend to enable a seamless 1-click Google/Microsoft OAuth login prompt with PKCE code exchange.
3. **Office Document Previewer in Tauri UI:**
   - Integrate an in-app viewer to preview generated `.docx`, `.xlsx`, and `.pptx` documents directly inside the OSMOO companion sidecar without needing Microsoft Office installed locally.

---

## 2. Medium-Term Enhancements (Quarterly Horizon)

1. **Local Neural Speech Model Optimization:**
   - Optimize local ONNX-based TTS engines (e.g., Piper, Kokoro) with INT8 quantization for sub-100ms first-audio token-to-speech latency on standard laptop CPUs.
2. **Multi-Calendar Sync Aggregation:**
   - Support simultaneous bidirectional synchronization across Google Calendar, Apple iCloud Calendar (CalDAV), and Microsoft 365 Exchange.
3. **Automated Expense Report Reconciliation:**
   - Implement an autonomous workflow combining Gmail receipt extraction, OCR extraction from PDF attachments, and automatic compilation into an audited `.xlsx` expense spreadsheet with calculated totals.

---

## 3. Long-Term Architectural Evolution

1. **Multi-Agent Collaborative Task Execution:**
   - Enable distributed subagents to research, draft documents, inspect code, and cross-review outputs in parallel with peer review and consensus checks before submitting deliverables to the user.
2. **Zero-Knowledge Encrypted Memory Sync:**
   - Provide optional encrypted cloud backup for OSMOO's SQLite databases and conversation memory using Argon2id-derived keys owned exclusively by the user.
