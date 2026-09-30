//! Enterprise Office Automation Connectors for VOXY COM (REQ-OFFICE-01).
//!
//! Provides granular permission-scoped connectors for:
//! - Sandboxed Filesystem operations with path traversal protection
//! - Tabular / Spreadsheet data parsing, querying, aggregation, and export
//! - Calendar event scheduling with automated conflict detection
//! - Document / Executive report template rendering
//! - Immutable structured audit logging of all connector actions

pub mod audit;
pub mod calendar_connector;
pub mod document_connector;
pub mod error;
pub mod fs_connector;
pub mod scope;
pub mod spreadsheet_connector;

pub use audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
pub use calendar_connector::{CalendarConnector, CalendarEvent};
pub use document_connector::DocumentConnector;
pub use error::{OfficeError, Result};
pub use fs_connector::FilesystemConnector;
pub use scope::{OfficeScope, ScopeSet};
pub use spreadsheet_connector::{SpreadsheetConnector, TabularSheet};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
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

        // Event 3 after Event 1 (2:00 to 3:00) -> should succeed
        let start3 = end1;
        let end3 = start3 + Duration::hours(1);
        let ev3 = cal.add_event(
            "Engineering Demo",
            start3,
            end3,
            None,
            None,
            vec![],
            &scopes,
            Some("user_1"),
        );
        assert!(ev3.is_ok());

        // Query events
        let found = cal
            .get_events_between(
                start1 - Duration::minutes(10),
                end3 + Duration::minutes(10),
                &scopes,
                None,
            )
            .unwrap();
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn test_document_template_and_executive_report() {
        let audit = Arc::new(OfficeAuditTrail::new(100));
        let doc_conn = DocumentConnector::new(audit);
        let scopes = ScopeSet::full_access();

        let template = "Hello {{name}}, your subscription to {{plan}} is {{status}}.";
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "Jane Doe".to_string());
        vars.insert("plan".to_string(), "VOXY Enterprise".to_string());
        vars.insert("status".to_string(), "Active".to_string());

        let rendered = doc_conn
            .render_template(template, &vars, &scopes, Some("template_bot"))
            .unwrap();
        assert_eq!(
            rendered,
            "Hello Jane Doe, your subscription to VOXY Enterprise is Active."
        );

        let highlights = vec![
            "Completed migration to Dodo Payments API".to_string(),
            "Enforced zero personal credentials in repository".to_string(),
        ];
        let report = doc_conn
            .generate_executive_report(
                "VOXY COM Production Milestone",
                "VOXY Lead Architect",
                &highlights,
                "All systems operational with 100% test pass.",
                &scopes,
                Some("lead_architect"),
            )
            .unwrap();

        assert!(report.contains("# VOXY COM Production Milestone"));
        assert!(report.contains("* Completed migration to Dodo Payments API"));
    }
}
