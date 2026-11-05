//! Native Office document creation and validation tools (.docx, .xlsx, .pptx).

use crate::error::{Result, ToolError};
use crate::metadata::{RiskTier, ToolCategory, ToolContext, ToolMetadata, ToolResult};
use crate::traits::Tool;
use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use voxy_office::{
    DocxDocument, OfficeDocumentKind, OfficeValidator, PptxPresentation, XlsxWorkbook,
};

/// Tool to create native Microsoft Word (.docx) documents
pub struct OfficeDocxCreateTool {
    metadata: ToolMetadata,
}

impl OfficeDocxCreateTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "office_docx_create",
                "Create a native Microsoft Word (.docx) document with headings, paragraphs, and tables",
                ToolCategory::System,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path", "title"],
                "properties": {
                    "path": { "type": "string", "description": "Target destination path for .docx file" },
                    "title": { "type": "string", "description": "Document title" },
                    "headings": { "type": "array", "items": { "type": "string" }, "description": "Section headings" },
                    "paragraphs": { "type": "array", "items": { "type": "string" }, "description": "Paragraph texts" },
                    "bullets": { "type": "array", "items": { "type": "string" }, "description": "Key bullet points" },
                    "table_headers": { "type": "array", "items": { "type": "string" }, "description": "Optional table headers" },
                    "table_rows": { "type": "array", "items": { "type": "array", "items": { "type": "string" } }, "description": "Optional table rows" }
                }
            })),
        }
    }
}

impl Default for OfficeDocxCreateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OfficeDocxCreateTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        // timer
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Document");

        let mut doc = DocxDocument::new(title);

        if let Some(headings) = params.get("headings").and_then(|v| v.as_array()) {
            for h in headings {
                if let Some(text) = h.as_str() {
                    doc.add_heading(text, 1);
                }
            }
        }

        if let Some(paragraphs) = params.get("paragraphs").and_then(|v| v.as_array()) {
            for p in paragraphs {
                if let Some(text) = p.as_str() {
                    doc.add_paragraph(text);
                }
            }
        }

        if let Some(bullets) = params.get("bullets").and_then(|v| v.as_array()) {
            for b in bullets {
                if let Some(text) = b.as_str() {
                    doc.add_bullet(text);
                }
            }
        }

        if let (Some(headers), Some(rows)) = (
            params.get("table_headers").and_then(|v| v.as_array()),
            params.get("table_rows").and_then(|v| v.as_array()),
        ) {
            let h_vec: Vec<String> = headers
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            let mut r_vec: Vec<Vec<String>> = Vec::new();
            for r in rows {
                if let Some(row_arr) = r.as_array() {
                    let r_row: Vec<String> = row_arr
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                    r_vec.push(r_row);
                }
            }
            if !h_vec.is_empty() {
                doc.add_table(h_vec, r_vec);
            }
        }

        let p = PathBuf::from(path_str);
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if !ext.eq_ignore_ascii_case("docx") {
                return Err(ToolError::InvalidParams(format!("Target path must have .docx extension, found .{ext}")));
            }
        } else {
            return Err(ToolError::InvalidParams("Target path must have .docx extension".into()));
        }
        if p.exists() && !ctx.user_confirmed {
            return Err(ToolError::ExecutionFailed(format!(
                "File '{}' already exists. User confirmation is required to overwrite existing files.",
                p.display()
            )));
        }
        doc.save(&p)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to save .docx: {e}")))?;

        Ok(ToolResult::success(
            json!({ "path": path_str, "format": "docx", "title": title })
        )
        .with_observation(format!("Successfully generated Microsoft Word document at '{}'", path_str)))
    }
}

/// Tool to create native Microsoft Excel (.xlsx) workbooks
pub struct OfficeXlsxCreateTool {
    metadata: ToolMetadata,
}

impl OfficeXlsxCreateTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "office_xlsx_create",
                "Create a native Microsoft Excel (.xlsx) workbook with sheets, headers, formulas, and CSV data",
                ToolCategory::System,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path", "sheet_name"],
                "properties": {
                    "path": { "type": "string", "description": "Target destination path for .xlsx file" },
                    "sheet_name": { "type": "string", "description": "Primary sheet name" },
                    "headers": { "type": "array", "items": { "type": "string" }, "description": "Column header names" },
                    "rows": { "type": "array", "items": { "type": "array" }, "description": "Row values" },
                    "csv_data": { "type": "string", "description": "Optional raw CSV string to import directly" }
                }
            })),
        }
    }
}

impl Default for OfficeXlsxCreateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OfficeXlsxCreateTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        // timer
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;
        let sheet_name = params
            .get("sheet_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Sheet1");

        let mut wb = XlsxWorkbook::new();

        if let Some(csv_data) = params.get("csv_data").and_then(|v| v.as_str()) {
            wb.import_csv(sheet_name, csv_data)
                .map_err(|e| ToolError::ExecutionFailed(format!("CSV import failed: {e}")))?;
        } else {
            let sheet = wb.add_sheet(sheet_name);
            sheet.freeze_panes(1, 1);

            if let Some(headers) = params.get("headers").and_then(|v| v.as_array()) {
                // col_count
                for (c_idx, h) in headers.iter().enumerate() {
                    let col = (c_idx + 1) as u32;
                    sheet.set_string(1, col, h.as_str().unwrap_or_default());
                }
            }

            if let Some(rows) = params.get("rows").and_then(|v| v.as_array()) {
                for (r_idx, row_val) in rows.iter().enumerate() {
                    let row_num = (r_idx + 2) as u32;
                    if let Some(cols) = row_val.as_array() {
                        for (c_idx, val) in cols.iter().enumerate() {
                            let col_num = (c_idx + 1) as u32;
                            if let Some(num) = val.as_f64() {
                                sheet.set_number(row_num, col_num, num);
                            } else if let Some(s) = val.as_str() {
                                if let Some(formula) = s.strip_prefix('=') {
                                    sheet.set_formula(row_num, col_num, formula, None);
                                } else {
                                    sheet.set_string(row_num, col_num, s);
                                }
                            }
                        }
                    }
                }
            }
        }

        let p = PathBuf::from(path_str);
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if !ext.eq_ignore_ascii_case("xlsx") {
                return Err(ToolError::InvalidParams(format!("Target path must have .xlsx extension, found .{ext}")));
            }
        } else {
            return Err(ToolError::InvalidParams("Target path must have .xlsx extension".into()));
        }
        if p.exists() && !ctx.user_confirmed {
            return Err(ToolError::ExecutionFailed(format!(
                "File '{}' already exists. User confirmation is required to overwrite existing files.",
                p.display()
            )));
        }
        wb.save(&p)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to save .xlsx: {e}")))?;

        Ok(ToolResult::success(
            json!({ "path": path_str, "format": "xlsx", "sheet": sheet_name })
        )
        .with_observation(format!("Successfully generated Microsoft Excel workbook at '{}'", path_str)))
    }
}

/// Tool to create native Microsoft PowerPoint (.pptx) presentations
pub struct OfficePptxCreateTool {
    metadata: ToolMetadata,
}

impl OfficePptxCreateTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "office_pptx_create",
                "Create a native Microsoft PowerPoint (.pptx) presentation with titles, bullets, and tables",
                ToolCategory::System,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path", "title"],
                "properties": {
                    "path": { "type": "string", "description": "Target destination path for .pptx file" },
                    "title": { "type": "string", "description": "Presentation main title" },
                    "subtitle": { "type": "string", "description": "Presentation subtitle" },
                    "slides": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "required": ["title"],
                            "properties": {
                                "title": { "type": "string" },
                                "bullets": { "type": "array", "items": { "type": "string" } }
                            }
                        },
                        "description": "Content slides with bullet points"
                    }
                }
            })),
        }
    }
}

impl Default for OfficePptxCreateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OfficePptxCreateTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext) -> Result<ToolResult> {
        // timer
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Presentation");
        let subtitle = params
            .get("subtitle")
            .and_then(|v| v.as_str())
            .unwrap_or("Prepared by OSMOO Companion");

        let mut pres = PptxPresentation::new(title);
        pres.add_title_slide(title, subtitle);

        if let Some(slides) = params.get("slides").and_then(|v| v.as_array()) {
            for s in slides {
                let s_title = s.get("title").and_then(|v| v.as_str()).unwrap_or("Slide");
                let mut bullets_vec = Vec::new();
                if let Some(bullets) = s.get("bullets").and_then(|v| v.as_array()) {
                    for b in bullets {
                        if let Some(text) = b.as_str() {
                            bullets_vec.push(text.to_string());
                        }
                    }
                }
                pres.add_content_slide(s_title, bullets_vec);
            }
        }

        let p = PathBuf::from(path_str);
        if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if !ext.eq_ignore_ascii_case("pptx") {
                return Err(ToolError::InvalidParams(format!("Target path must have .pptx extension, found .{ext}")));
            }
        } else {
            return Err(ToolError::InvalidParams("Target path must have .pptx extension".into()));
        }
        if p.exists() && !ctx.user_confirmed {
            return Err(ToolError::ExecutionFailed(format!(
                "File '{}' already exists. User confirmation is required to overwrite existing files.",
                p.display()
            )));
        }
        pres.save(&p)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to save .pptx: {e}")))?;

        Ok(ToolResult::success(
            json!({ "path": path_str, "format": "pptx", "title": title })
        )
        .with_observation(format!("Successfully generated Microsoft PowerPoint presentation at '{}'", path_str)))
    }
}

/// Tool to inspect and validate Microsoft Office packages (.docx, .xlsx, .pptx)
pub struct OfficeFileValidateTool {
    metadata: ToolMetadata,
}

impl OfficeFileValidateTool {
    pub fn new() -> Self {
        Self {
            metadata: ToolMetadata::new(
                "office_file_validate",
                "Verify and inspect a Microsoft Office file (.docx, .xlsx, .pptx) for OpenXML structural compliance",
                ToolCategory::System,
                RiskTier::LowRisk,
            )
            .with_schema(json!({
                "type": "object",
                "required": ["path"],
                "properties": {
                    "path": { "type": "string", "description": "File path of .docx, .xlsx, or .pptx to validate" }
                }
            })),
        }
    }
}

impl Default for OfficeFileValidateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for OfficeFileValidateTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(&self, params: serde_json::Value, _ctx: &ToolContext) -> Result<ToolResult> {
        // timer
        let path_str = params
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("Missing 'path'".into()))?;

        let report = OfficeValidator::validate_file(path_str)
            .map_err(|e| ToolError::ExecutionFailed(format!("Validation failed: {e}")))?;

        let kind_str = match report.kind {
            OfficeDocumentKind::WordDocx => "Word (.docx)",
            OfficeDocumentKind::ExcelXlsx => "Excel (.xlsx)",
            OfficeDocumentKind::PowerPointPptx => "PowerPoint (.pptx)",
            OfficeDocumentKind::Unknown => "Unknown / Invalid",
        };

        Ok(ToolResult::success(
            json!({
                "valid": report.valid,
                "document_type": kind_str,
                "parts_count": report.part_count,
                "extracted_sample_count": report.extracted_text_snippets.len(),
                "sample_text": report.extracted_text_snippets.iter().take(5).cloned().collect::<Vec<_>>(),
                "errors": report.errors,
            })
        )
        .with_observation(format!(
            "Office package validation: valid={}, type='{}', parts={}, samples={}",
            report.valid, kind_str, report.part_count, report.extracted_text_snippets.len()
        )))
    }
}
