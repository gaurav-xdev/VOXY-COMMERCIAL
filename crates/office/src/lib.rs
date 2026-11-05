//! Enterprise Office Automation Connectors for VOXY COM (REQ-OFFICE-01).
//!
//! Provides granular permission-scoped connectors for:
//! - Sandboxed Filesystem operations with path traversal protection
//! - Tabular / Spreadsheet data parsing, querying, aggregation, and export
//! - Real OpenXML Office generation (.docx, .xlsx, .pptx) with structural validation
//! - Persistent SQLite Calendar with recurrence, conflict detection, and OAuth sync
//! - Professional Email management (Gmail/Outlook) with mandatory human approval
//! - Travel research, price comparison, itinerary generation, and booking handoffs
//! - Shopping discovery, cart preparation, and payment sandbox safety controls
//! - Immutable structured audit logging of all connector actions

pub mod audit;
pub mod calendar_connector;
pub mod document_connector;
pub mod docx_generator;
pub mod error;
pub mod fs_connector;
pub mod mail_connector;
pub mod office_validator;
pub mod pptx_generator;
pub mod scope;
pub mod shopping_connector;
pub mod spreadsheet_connector;
pub mod travel_connector;
pub mod xlsx_generator;

pub use audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
pub use calendar_connector::{CalendarConnector, CalendarEvent, RecurrenceRule, SyncResult};
pub use document_connector::DocumentConnector;
pub use docx_generator::{DocxDocument, DocxElement};
pub use error::{OfficeError, Result};
pub use fs_connector::FilesystemConnector;
pub use mail_connector::{EmailDraft, EmailMessage, EmailProvider, MailConnector, ThreadSummary};
pub use office_validator::{OfficeDocumentKind, OfficeValidator, ValidationReport};
pub use pptx_generator::{PptxPresentation, PptxSlide, SlideContent};
pub use scope::{OfficeScope, ScopeSet};
pub use shopping_connector::{CartItem, ProductItem, SandboxPaymentReceipt, ShoppingCart, ShoppingConnector};
pub use spreadsheet_connector::{SpreadsheetConnector, TabularSheet};
pub use travel_connector::{FlightOption, HotelOption, TravelConnector, TravelItinerary};
pub use xlsx_generator::{CellValue, XlsxSheet, XlsxWorkbook};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, NaiveDate, Utc};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[test]
    fn test_office_connector_permissions() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let temp_dir = tempdir().unwrap();
        let fs_conn = FilesystemConnector::new(temp_dir.path(), audit.clone()).unwrap();

        let mut read_only_scopes = ScopeSet::read_only();

        // 1. Write file with read-only scopes -> must be denied
        let write_err = fs_conn
            .write_file("test.txt", "content", &read_only_scopes, Some("user_123"))
            .unwrap_err();
        assert!(matches!(write_err, OfficeError::PermissionDenied { .. }));

        // 2. Grant write scope -> must succeed
        read_only_scopes.grant(OfficeScope::FilesWrite);
        let write_ok =
            fs_conn.write_file("test.txt", "content", &read_only_scopes, Some("user_123"));
        assert!(write_ok.is_ok());

        // 3. Read back file -> must succeed
        let content = fs_conn
            .read_file("test.txt", &read_only_scopes, Some("user_123"))
            .unwrap();
        assert_eq!(content, "content");

        // 4. Revoke read scope -> read must now be denied
        read_only_scopes.revoke(&OfficeScope::FilesRead);
        let read_err = fs_conn
            .read_file("test.txt", &read_only_scopes, Some("user_123"))
            .unwrap_err();
        assert!(matches!(read_err, OfficeError::PermissionDenied { .. }));
    }

    #[test]
    fn test_office_action_audit_trail() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let temp_dir = tempdir().unwrap();
        let fs_conn = FilesystemConnector::new(temp_dir.path(), audit.clone()).unwrap();

        let mut scopes = ScopeSet::new();

        // Failed action due to missing scope
        let _ = fs_conn.write_file("report.csv", "a,b,c", &scopes, Some("auditor_01"));

        // Granted action
        scopes.grant(OfficeScope::FilesWrite);
        let _ = fs_conn.write_file("report.csv", "a,b,c", &scopes, Some("auditor_01"));

        let events = audit.get_events();
        assert_eq!(events.len(), 2);

        // First event was denied
        assert_eq!(events[0].status, AuditStatus::Denied);
        assert_eq!(events[0].connector, "Filesystem");
        assert_eq!(events[0].operation, "write_file");
        assert_eq!(events[0].user_id.as_deref(), Some("auditor_01"));

        // Second event was success
        assert_eq!(events[1].status, AuditStatus::Success);
        assert_eq!(events[1].connector, "Filesystem");
        assert_eq!(events[1].operation, "write_file");

        let denied_events = audit.filter_by_status(AuditStatus::Denied);
        assert_eq!(denied_events.len(), 1);

        let success_events = audit.filter_by_status(AuditStatus::Success);
        assert_eq!(success_events.len(), 1);
    }

    #[test]
    fn test_filesystem_path_traversal_prevention() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let temp_dir = tempdir().unwrap();
        let fs_conn = FilesystemConnector::new(temp_dir.path(), audit).unwrap();
        let scopes = ScopeSet::full_access();

        // Attempt path traversal with ".."
        let err = fs_conn
            .read_file("../../windows/system32/cmd.exe", &scopes, None)
            .unwrap_err();
        assert!(matches!(err, OfficeError::PathTraversalAttempt { .. }));
    }

    #[test]
    fn test_spreadsheet_connector_parsing_and_aggregation() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let ss_conn = SpreadsheetConnector::new(audit);
        let scopes = ScopeSet::full_access();

        let csv_data = "Employee,Department,Salary,Status\n\
                        Alice,Engineering,120000,Active\n\
                        Bob,Sales,90000,Active\n\
                        Charlie,Engineering,130000,Inactive\n\
                        Diana,Sales,95000,Active";

        let sheet = ss_conn
            .parse_csv("Employees", csv_data, &scopes, Some("hr_user"))
            .unwrap();
        assert_eq!(sheet.headers.len(), 4);
        assert_eq!(sheet.rows.len(), 4);

        // Query active engineering employees
        let query_res = ss_conn
            .query_rows(
                &sheet,
                "Department",
                "Engineering",
                &scopes,
                Some("hr_user"),
            )
            .unwrap();
        assert_eq!(query_res.len(), 2);

        // Aggregate salary
        let total_salary = ss_conn.aggregate_sum(&sheet, "Salary", &scopes).unwrap();
        assert_eq!(total_salary, 435000.0);

        // Export
        let exported = ss_conn
            .export_csv(&sheet, &scopes, Some("hr_user"))
            .unwrap();
        assert!(exported.contains("Alice,Engineering,120000,Active"));
    }

    #[test]
    fn test_calendar_connector_conflict_detection() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let cal = CalendarConnector::new(audit);
        let scopes = ScopeSet::full_access();

        let now = Utc::now();
        let start1 = now + Duration::hours(1);
        let end1 = now + Duration::hours(2);

        // Event 1 added successfully
        let ev1 = cal.add_event(
            "Quarterly Review",
            start1,
            end1,
            Some("Boardroom A"),
            Some("Review Q3 metrics"),
            vec!["alice@example.com".to_string()],
            &scopes,
            Some("user_1"),
        );
        assert!(ev1.is_ok());

        // Event 2 overlapping with Event 1 (1:30 to 2:30) -> must detect conflict
        let start2 = now + Duration::minutes(90);
        let end2 = now + Duration::minutes(150);

        let ev2 = cal.add_event(
            "Product Sync",
            start2,
            end2,
            Some("Boardroom A"),
            None,
            vec!["bob@example.com".to_string()],
            &scopes,
            Some("user_2"),
        );
        assert!(matches!(
            ev2.unwrap_err(),
            OfficeError::ConflictDetected { .. }
        ));
    }

    #[test]
    fn test_document_template_and_executive_report() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let doc_conn = DocumentConnector::new(audit);
        let scopes = ScopeSet::full_access();

        let mut vars = HashMap::new();
        vars.insert("project".to_string(), "VOXY COM".to_string());
        vars.insert("quarter".to_string(), "Q3".to_string());
        vars.insert("status".to_string(), "On Track".to_string());

        let template = "Project {{project}} status for {{quarter}} is currently {{status}}.";
        let rendered = doc_conn
            .render_template(template, &vars, &scopes, Some("manager"))
            .unwrap();
        assert_eq!(rendered, "Project VOXY COM status for Q3 is currently On Track.");

        let highlights = vec![
            "Completed Office connector security audit".to_string(),
            "Enforced granular permission scopes".to_string(),
        ];
        let report = doc_conn
            .generate_executive_report(
                "Monthly Operational Review",
                "Senior Architect",
                &highlights,
                "All systems verified with green test results.",
                &scopes,
                Some("manager"),
            )
            .unwrap();

        assert!(report.contains("# Monthly Operational Review"));
        assert!(report.contains("**Author:** Senior Architect"));
        assert!(report.contains("* Completed Office connector security audit"));
    }

    #[test]
    fn test_docx_generation_and_validation() {
        let temp_dir = tempdir().unwrap();
        let docx_path = temp_dir.path().join("Executive_Report.docx");

        let mut doc = DocxDocument::new("Q3 Engineering & Architecture Review");
        doc.with_author("OSMOO Autonomous Engineer")
            .add_heading("1. Executive Summary", 1)
            .add_paragraph("This document was generated natively by the OSMOO operating companion.")
            .add_heading("2. Key Milestones", 2)
            .add_bullet("Zero-fallback model routing implemented")
            .add_bullet("Sub-120ms token-to-speech streaming verified")
            .add_bullet("Native OpenXML document compilers deployed")
            .add_heading("3. Performance Matrix", 2)
            .add_table(
                vec!["Metric".to_string(), "Target".to_string(), "Actual".to_string()],
                vec![
                    vec!["First Word TTFT".to_string(), "< 150ms".to_string(), "108ms".to_string()],
                    vec!["Barge-In Cancel".to_string(), "< 50ms".to_string(), "32ms".to_string()],
                    vec!["Test Coverage".to_string(), "> 95%".to_string(), "100%".to_string()],
                ],
            );

        doc.save(&docx_path).expect("Failed to save .docx");
        assert!(docx_path.exists());

        // Validate generated DOCX package using OfficeValidator
        let report = OfficeValidator::validate_file(&docx_path).expect("Failed to validate docx");
        assert!(report.valid);
        assert_eq!(report.kind, OfficeDocumentKind::WordDocx);
        assert!(report.extracted_text_snippets.iter().any(|s| s.contains("Q3 Engineering")));
        assert!(report.extracted_text_snippets.iter().any(|s| s.contains("First Word TTFT")));
    }

    #[test]
    fn test_xlsx_generation_and_validation() {
        let temp_dir = tempdir().unwrap();
        let xlsx_path = temp_dir.path().join("Financial_Plan.xlsx");

        let mut wb = XlsxWorkbook::new();
        let sheet = wb.add_sheet("Q3 Budget");
        sheet.set_string(1, 1, "Category");
        sheet.set_string(1, 2, "Allocated (USD)");
        sheet.set_string(1, 3, "Spent (USD)");
        sheet.set_string(1, 4, "Remaining (USD)");

        sheet.set_string(2, 1, "Cloud Compute");
        sheet.set_number(2, 2, 50000.0);
        sheet.set_number(2, 3, 34200.0);
        sheet.set_formula(2, 4, "B2-C2", Some(15800.0));

        sheet.set_string(3, 1, "Audio Hardware & Labs");
        sheet.set_number(3, 2, 20000.0);
        sheet.set_number(3, 3, 11450.0);
        sheet.set_formula(3, 4, "B3-C3", Some(8550.0));

        sheet.freeze_panes(1, 1);

        // Also test CSV import onto a second sheet
        let csv_data = "Item,Count,Cost\nServers,4,12000\nMicrophones,8,3200\nMonitors,12,4800";
        wb.import_csv("Inventory", csv_data).expect("CSV import failed");

        wb.save(&xlsx_path).expect("Failed to save .xlsx");
        assert!(xlsx_path.exists());

        // Validate generated XLSX package
        let report = OfficeValidator::validate_file(&xlsx_path).expect("Failed to validate xlsx");
        assert!(report.valid);
        assert_eq!(report.kind, OfficeDocumentKind::ExcelXlsx);
        assert!(report.extracted_text_snippets.iter().any(|s| s.contains("Cloud Compute")));
        assert!(report.extracted_text_snippets.iter().any(|s| s.contains("Inventory") || s.contains("Servers")));
    }

    #[test]
    fn test_pptx_generation_and_validation() {
        let temp_dir = tempdir().unwrap();
        let pptx_path = temp_dir.path().join("Executive_Deck.pptx");

        let mut pres = PptxPresentation::new("OSMOO Production Architecture");
        pres.add_title_slide(
            "OSMOO: Autonomous Operating Companion",
            "Production Architecture & Capabilities Review",
        );
        pres.add_content_slide(
            "Key Architectural Pillars",
            vec![
                "Native Rust Coding Harness with Rollback Journaling".to_string(),
                "Sub-120ms Token-to-Speech Streaming Audio Engine".to_string(),
                "Deterministic Win32 GDI & UIA Desktop Automation".to_string(),
                "Persistent SQLite Memory and Calendar System".to_string(),
            ],
        );
        pres.add_table_slide(
            "Verification Summary",
            vec!["Subsystem".to_string(), "Status".to_string(), "Tests".to_string()],
            vec![
                vec!["Voice Audio".to_string(), "Verified".to_string(), "15 passed".to_string()],
                vec!["Coding Harness".to_string(), "Verified".to_string(), "12 passed".to_string()],
                vec!["Office Compilers".to_string(), "Verified".to_string(), "Full pass".to_string()],
            ],
        );
        pres.set_speaker_notes(0, "Welcome stakeholders. Today we demonstrate production readiness.");

        pres.save(&pptx_path).expect("Failed to save .pptx");
        assert!(pptx_path.exists());

        // Validate generated PPTX package
        let report = OfficeValidator::validate_file(&pptx_path).expect("Failed to validate pptx");
        assert!(report.valid);
        assert_eq!(report.kind, OfficeDocumentKind::PowerPointPptx);
        assert!(report.extracted_text_snippets.iter().any(|s| s.contains("Autonomous Operating Companion")));
        assert!(report.extracted_text_snippets.iter().any(|s| s.contains("Key Architectural Pillars")));
    }

    #[test]
    fn test_persistent_sqlite_calendar_survives_reopen() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("calendar_test.db");
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let scopes = ScopeSet::full_access();

        let start = Utc::now() + Duration::days(1);
        let end = start + Duration::hours(1);

        // 1. Create calendar connector with persistent SQLite file
        {
            let cal = CalendarConnector::with_sqlite(&db_path, audit.clone()).unwrap();
            let ev = cal
                .add_event_with_recurrence(
                    "Architecture Board Meeting",
                    start,
                    end,
                    Some("Virtual Room 4"),
                    Some("Discuss OpenXML integration"),
                    vec!["lead@voxy.ai".to_string()],
                    RecurrenceRule::Weekly,
                    &scopes,
                    Some("user_alice"),
                )
                .expect("Failed to add event");
            assert_eq!(ev.title, "Architecture Board Meeting");
            assert_eq!(ev.recurrence, RecurrenceRule::Weekly);
        } // Drop connector, closing connection

        // 2. Re-open connection from the EXACT same SQLite database file on disk!
        {
            let cal2 = CalendarConnector::with_sqlite(&db_path, audit).unwrap();
            let events = cal2
                .list_events(start - Duration::hours(1), end + Duration::hours(1), &scopes, Some("user_alice"))
                .expect("Failed to list events");
            assert_eq!(events.len(), 1, "Calendar events must survive connector restarts!");
            assert_eq!(events[0].title, "Architecture Board Meeting");
            assert_eq!(events[0].recurrence, RecurrenceRule::Weekly);
            assert_eq!(events[0].location.as_deref(), Some("Virtual Room 4"));
        }
    }

    #[test]
    fn test_mail_connector_draft_and_approval_gate() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let mail = MailConnector::new("osmoo@voxy.ai", audit);
        let scopes = ScopeSet::full_access();

        // Ingest an incoming message
        mail.ingest_message(EmailMessage {
            id: "msg_101".to_string(),
            thread_id: "thread_505".to_string(),
            from: "client@enterprise.com".to_string(),
            to: vec!["osmoo@voxy.ai".to_string()],
            cc: vec![],
            subject: "Urgent: Q3 Report Delivery Deadline".to_string(),
            body: "Hi team, please send the revised executive presentation by tomorrow EOD. We need to finalize the board deck.".to_string(),
            date: Utc::now(),
            has_attachments: false,
            labels: vec!["INBOX".to_string()],
        });

        // 1. Search inbox
        let search_results = mail.search_inbox("Report Delivery", 10, &scopes, Some("user_alice")).unwrap();
        assert_eq!(search_results.len(), 1);

        // 2. Read thread and extract action items
        let summary = mail.summarize_thread("thread_505", &scopes).unwrap();
        assert_eq!(summary.subject, "Urgent: Q3 Report Delivery Deadline");
        assert!(!summary.action_items.is_empty(), "Should extract action items with deadline keywords");

        // 3. Create draft
        let draft = mail
            .create_draft(
                vec!["client@enterprise.com".to_string()],
                vec![],
                "Re: Urgent: Q3 Report Delivery Deadline",
                "Hello, the revised presentation is attached and verified. Best regards.",
                Some("thread_505".to_string()),
                &scopes,
                Some("user_alice"),
            )
            .unwrap();
        assert_eq!(draft.status, "draft");
        assert!(!draft.is_approved);

        // 4. Try sending WITHOUT user approval confirmation -> MUST FAIL!
        let unauthorized_send = mail.send_approved_email(&draft.id, false, &scopes, Some("user_alice"));
        assert!(matches!(unauthorized_send.unwrap_err(), OfficeError::PermissionDenied { .. }));

        // 5. Approve draft and send with explicit confirmation -> MUST SUCCEED!
        mail.approve_draft(&draft.id, Some("user_alice")).unwrap();
        let sent = mail.send_approved_email(&draft.id, true, &scopes, Some("user_alice")).unwrap();
        assert_eq!(sent.to, vec!["client@enterprise.com".to_string()]);
        assert_eq!(sent.labels, vec!["SENT".to_string()]);
    }

    #[test]
    fn test_travel_connector_and_itinerary() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let travel = TravelConnector::new(audit);
        let scopes = ScopeSet::full_access();

        let dep_date = NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();
        let flights = travel.search_flights("SFO", "JFK", dep_date, 1, &scopes, Some("traveler")).unwrap();
        assert!(!flights.is_empty());
        assert!(flights[0].verified_booking_url.contains("google.com/travel") || flights[0].verified_booking_url.contains("skyscanner.com"));

        let check_out = NaiveDate::from_ymd_opt(2026, 11, 19).unwrap();
        let hotels = travel.search_hotels("New York", dep_date, check_out, &scopes, Some("traveler")).unwrap();
        assert!(!hotels.is_empty());
        assert!(hotels[0].verified_booking_url.contains("booking.com"));

        let itinerary = travel.build_itinerary("New York City", flights, hotels).unwrap();
        assert!(itinerary.total_estimated_cost_usd > 0.0);
        assert_eq!(itinerary.destination, "New York City");
    }

    #[test]
    fn test_shopping_and_sandbox_payment_safety() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let shopping = ShoppingConnector::new(audit);
        let scopes = ScopeSet::full_access();

        let products = shopping.discover_products("Mechanical Keyboard", &scopes, Some("shopper")).unwrap();
        assert_eq!(products.len(), 2);
        assert!(products[0].direct_product_url.contains("amazon.com") || products[0].direct_product_url.contains("bestbuy.com"));

        let cart = shopping
            .prepare_cart(vec![(products[0].clone(), 2), (products[1].clone(), 1)])
            .unwrap();
        assert!(cart.total_usd > cart.subtotal_usd, "Total must include estimated taxes");

        // 1. Attempt payment with forbidden raw card string -> MUST BE REJECTED
        let bad_payment = shopping.execute_sandbox_test_payment(&cart, "4111222233334444", Some("shopper"));
        assert!(matches!(bad_payment.unwrap_err(), OfficeError::PermissionDenied { .. }));

        // 2. Execute test payment with authorized sandbox token -> MUST SUCCEED
        let receipt = shopping
            .execute_sandbox_test_payment(&cart, "tok_sandbox_visa_4242", Some("shopper"))
            .unwrap();
        assert_eq!(receipt.status, "sandbox_settled");
        assert_eq!(receipt.environment, "sandbox_test_only");
        assert!(receipt.amount_usd > 0.0);
    }
}
