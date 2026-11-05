# OSMOO Known Limitations & Operational Boundaries

**Date:** October 28, 2026  
**Status:** Documented for Production Clarity  

---

## 1. External Cloud Services & Credentials

1. **Google & Microsoft OAuth Cloud Sync:**
   - The native code provides full OAuth sync logic, data models, token persistence structures, and conflict resolution in `CalendarConnector::sync_remote_provider` and `MailConnector::with_oauth`.
   - However, live bidirectional continuous synchronization with Google Calendar or Microsoft Exchange in a user's deployment requires registering an application in Google Cloud Console / Microsoft Entra ID and obtaining end-user consent tokens.
   - When no live tokens are present, OSMOO operates in offline/local-first mode using its SQLite persistence engine.

2. **Live Merchant Scraping & Anti-Bot Mitigations:**
   - E-commerce and travel search tools (`travel_search_flights`, `shopping_discover_products`) provide deterministic product discovery and verified handoff links to official engines (Google Flights, Skyscanner, Amazon, Best Buy).
   - Some airline websites employ aggressive Cloudflare/Akamai bot detection that prevents automated headless checkouts. By design, OSMOO provides direct verified booking URLs for the user to complete checkout securely, eliminating the risk of bot-blocking or credential harvesting.

---

## 2. Document Format Features

1. **Microsoft Word (`.docx`):**
   - Implements paragraphs, headings (levels 1–3), bullet lists, formatted tables with headers and alternating borders, and page breaks.
   - Advanced features such as embedded vector diagrams, macro-enabled scripts (`.docm`), and complex dynamic table-of-contents fields are intentionally omitted in this version to preserve file safety and avoid macro-based malware risks.

2. **Microsoft Excel (`.xlsx`):**
   - Supports numeric cells, string indexing (shared strings table), cell formulas (`SUM`, `AVERAGE`), frozen header panes, and CSV-to-XLSX conversion.
   - Does not yet generate embedded interactive charts (e.g. pie/bar charts directly inside the Excel worksheet); users can view structured tables and formulas.

3. **Microsoft PowerPoint (`.pptx`):**
   - Generates standard 16:9 widescreen presentations with title slides, bullet body slides, table slides, and presenter speaker notes.
   - Video slide embedding and 3D shape animations are not supported in this release.

---

## 3. Desktop Automation Performance

1. **Windows UI Automation (UIA) Latency:**
   - Deep inspection of complex application trees (e.g. Electron apps with thousands of nested DOM nodes) can take 200–500ms depending on UI depth.
   - The hybrid backend mitigates this by combining UIA with GDI keystroke/mouse input and fallback coordinate targeting.
