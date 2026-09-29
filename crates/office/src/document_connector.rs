use std::collections::HashMap;
use std::sync::Arc;
use crate::audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
use crate::error::Result;
use crate::scope::{OfficeScope, ScopeSet};

/// Document and reporting connector for templated summaries and executive reports.
#[derive(Debug, Clone)]
pub struct DocumentConnector {
    audit_trail: Arc<OfficeAuditTrail>,
}

impl DocumentConnector {
    pub fn new(audit_trail: Arc<OfficeAuditTrail>) -> Self {
        Self { audit_trail }
    }

    /// Render a template string by replacing `{{key}}` with corresponding map values.
    pub fn render_template(
        &self,
        template: &str,
        vars: &HashMap<String, String>,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<String> {
        scopes.check(OfficeScope::DocumentWrite)?;

        let mut output = template.to_string();
        for (k, v) in vars {
            let placeholder = format!("{{{{{}}}}}", k);
            output = output.replace(&placeholder, v);
        }

        self.audit_trail.record(OfficeAuditEvent::new(
            "Document",
            "render_template",
            "memory_template",
            user_id,
            OfficeScope::DocumentWrite.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "vars_count": vars.len(), "rendered_bytes": output.len() }),
        ));

        Ok(output)
    }

    /// Generate an executive markdown report with metadata header and bulleted highlights.
    pub fn generate_executive_report(
        &self,
        title: &str,
        author: &str,
        highlights: &[String],
        body: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<String> {
        scopes.check(OfficeScope::DocumentWrite)?;

        let mut report = String::new();
        report.push_str(&format!("# {}\n\n", title));
        report.push_str(&format!("**Author:** {}  \n", author));
        report.push_str(&format!("**Date:** {}  \n\n", chrono::Utc::now().to_rfc3339()));
        report.push_str("## Executive Summary\n\n");

        for h in highlights {
            report.push_str(&format!("* {}\n", h));
        }
        report.push('\n');

        report.push_str("## Detailed Findings\n\n");
        report.push_str(body);
        report.push('\n');

        self.audit_trail.record(OfficeAuditEvent::new(
            "Document",
            "generate_executive_report",
            title,
            user_id,
            OfficeScope::DocumentWrite.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "highlights_count": highlights.len(), "report_len": report.len() }),
        ));

        Ok(report)
    }
}
