use std::collections::HashMap;
use std::sync::Arc;
use crate::audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
use crate::error::{OfficeError, Result};
use crate::scope::{OfficeScope, ScopeSet};

/// In-memory tabular representation of spreadsheet data.
#[derive(Debug, Clone)]
pub struct TabularSheet {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Enterprise spreadsheet connector supporting CSV parsing, querying,
/// column aggregation, and export with permission enforcement and audit logging.
#[derive(Debug, Clone)]
pub struct SpreadsheetConnector {
    audit_trail: Arc<OfficeAuditTrail>,
}

impl SpreadsheetConnector {
    pub fn new(audit_trail: Arc<OfficeAuditTrail>) -> Self {
        Self { audit_trail }
    }

    /// Parse raw CSV string into a `TabularSheet`, enforcing `OfficeScope::SpreadsheetRead`.
    pub fn parse_csv(
        &self,
        name: &str,
        raw_csv: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<TabularSheet> {
        if let Err(e) = scopes.check(OfficeScope::SpreadsheetRead) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Spreadsheet",
                "parse_csv",
                name,
                user_id,
                OfficeScope::SpreadsheetRead.as_str(),
                AuditStatus::Denied,
                serde_json::json!({ "reason": e.to_string() }),
            ));
            return Err(e);
        }

        let mut lines = raw_csv.lines().filter(|l| !l.trim().is_empty());
        let header_line = match lines.next() {
            Some(h) => h,
            None => {
                return Err(OfficeError::InvalidData("CSV content is empty".to_string()));
            }
        };

        let headers: Vec<String> = header_line
            .split(',')
            .map(|col| col.trim().trim_matches('"').to_string())
            .collect();

        let mut rows = Vec::new();
        for line in lines {
            let row: Vec<String> = line
                .split(',')
                .map(|col| col.trim().trim_matches('"').to_string())
                .collect();
            if row.len() == headers.len() {
                rows.push(row);
            }
        }

        self.audit_trail.record(OfficeAuditEvent::new(
            "Spreadsheet",
            "parse_csv",
            name,
            user_id,
            OfficeScope::SpreadsheetRead.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "columns": headers.len(), "rows": rows.len() }),
        ));

        Ok(TabularSheet {
            name: name.to_string(),
            headers,
            rows,
        })
    }

    /// Query rows matching a filter condition, e.g. column `status == "Active"`.
    pub fn query_rows(
        &self,
        sheet: &TabularSheet,
        col_name: &str,
        match_value: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<Vec<HashMap<String, String>>> {
        scopes.check(OfficeScope::SpreadsheetRead)?;

        let col_idx = sheet
            .headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(col_name))
            .ok_or_else(|| {
                OfficeError::NotFound(format!("Column '{}' not found in spreadsheet", col_name))
            })?;

        let mut matches = Vec::new();
        for row in &sheet.rows {
            if let Some(val) = row.get(col_idx) {
                if val.eq_ignore_ascii_case(match_value) {
                    let mut map = HashMap::new();
                    for (i, h) in sheet.headers.iter().enumerate() {
                        if let Some(cell) = row.get(i) {
                            map.insert(h.clone(), cell.clone());
                        }
                    }
                    matches.push(map);
                }
            }
        }

        self.audit_trail.record(OfficeAuditEvent::new(
            "Spreadsheet",
            "query_rows",
            &sheet.name,
            user_id,
            OfficeScope::SpreadsheetRead.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "filter_col": col_name, "filter_val": match_value, "matched": matches.len() }),
        ));

        Ok(matches)
    }

    /// Calculate numeric sum of a column.
    pub fn aggregate_sum(
        &self,
        sheet: &TabularSheet,
        col_name: &str,
        scopes: &ScopeSet,
    ) -> Result<f64> {
        scopes.check(OfficeScope::SpreadsheetRead)?;

        let col_idx = sheet
            .headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(col_name))
            .ok_or_else(|| {
                OfficeError::NotFound(format!("Column '{}' not found in spreadsheet", col_name))
            })?;

        let mut sum = 0.0;
        for row in &sheet.rows {
            if let Some(val) = row.get(col_idx) {
                if let Ok(num) = val.parse::<f64>() {
                    sum += num;
                }
            }
        }

        Ok(sum)
    }

    /// Export `TabularSheet` to serialized CSV, enforcing `OfficeScope::SpreadsheetExport`.
    pub fn export_csv(
        &self,
        sheet: &TabularSheet,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<String> {
        if let Err(e) = scopes.check(OfficeScope::SpreadsheetExport) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Spreadsheet",
                "export_csv",
                &sheet.name,
                user_id,
                OfficeScope::SpreadsheetExport.as_str(),
                AuditStatus::Denied,
                serde_json::json!({ "reason": e.to_string() }),
            ));
            return Err(e);
        }

        let mut out = String::new();
        out.push_str(&sheet.headers.join(","));
        out.push('\n');

        for row in &sheet.rows {
            out.push_str(&row.join(","));
            out.push('\n');
        }

        self.audit_trail.record(OfficeAuditEvent::new(
            "Spreadsheet",
            "export_csv",
            &sheet.name,
            user_id,
            OfficeScope::SpreadsheetExport.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "exported_bytes": out.len() }),
        ));

        Ok(out)
    }
}
