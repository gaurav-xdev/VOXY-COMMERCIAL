use std::path::PathBuf;
use voxy_office::{
    DocxDocument, OfficeDocumentKind, OfficeValidator, PptxPresentation, XlsxWorkbook,
};

#[test]
fn test_generate_real_office_deliverables_on_disk() {
    let out_dir = PathBuf::from("test_output");
    std::fs::create_dir_all(&out_dir).expect("create test_output dir");

    // 1. Generate DOCX
    let docx_path = out_dir.join("OSMOO_Sample_Document.docx");
    let mut doc = DocxDocument::new("OSMOO Autonomous Operating Companion Report");
    doc.with_author("OSMOO Principal Engineering Team");
    doc.add_heading("OSMOO Architecture & Production Verification", 1);
    doc.add_heading("1. Executive Summary", 1);
    doc.add_paragraph("This document was generated directly by OSMOO's native OpenXML compiler engine without external desktop runtime dependencies.");
    doc.add_heading("2. Core Capabilities Verified", 2);
    doc.add_bullet("Native OpenXML binary document packaging (.docx, .xlsx, .pptx)");
    doc.add_bullet("Persistent SQLite scheduling surviving system restarts");
    doc.add_bullet("Multi-provider email management with mandatory approval gate");
    doc.add_bullet("Travel and shopping research with verified provider handoffs");
    doc.add_bullet("Strict payment sandbox tokenization (raw credit cards rejected)");
    doc.add_heading("3. Performance Benchmarks", 2);
    doc.add_table(
        vec!["Metric".to_string(), "Target".to_string(), "Actual Result".to_string(), "Compliance".to_string()],
        vec![
            vec!["Docx Generation Time".to_string(), "< 100ms".to_string(), "12ms".to_string(), "EXCEEDED".to_string()],
            vec!["Xlsx Generation Time".to_string(), "< 100ms".to_string(), "9ms".to_string(), "EXCEEDED".to_string()],
            vec!["Pptx Generation Time".to_string(), "< 150ms".to_string(), "18ms".to_string(), "EXCEEDED".to_string()],
            vec!["Zero Fallback Route Leak".to_string(), "0%".to_string(), "0%".to_string(), "100% STRICT".to_string()],
        ],
    );
    doc.add_page_break();
    doc.add_heading("4. Security Safeguards & Audit Ledger", 1);
    doc.add_paragraph("All I/O actions are validated against role-based scope sets and audited in append-only ledgers.");
    doc.save(&docx_path).expect("save docx");
    assert!(docx_path.exists());

    // Validate with OfficeValidator
    let docx_report = OfficeValidator::validate_file(&docx_path).expect("validate docx");
    assert!(docx_report.valid);
    assert_eq!(docx_report.kind, OfficeDocumentKind::WordDocx);
    assert!(docx_report.extracted_text_snippets.iter().any(|s| s.contains("Executive Summary")));

    // 2. Generate XLSX
    let xlsx_path = out_dir.join("OSMOO_Sample_Spreadsheet.xlsx");
    let mut wb = XlsxWorkbook::new();
    let sheet = wb.add_sheet("Financial_Projections");
    sheet.set_string(1, 1, "Category");
    sheet.set_string(1, 2, "Q1_USD");
    sheet.set_string(1, 3, "Q2_USD");
    sheet.set_string(1, 4, "Q3_USD");
    sheet.set_string(1, 5, "Q4_USD");
    sheet.set_string(1, 6, "Total_USD");

    let categories = [
        ("Engineering", 120000.0, 135000.0, 142000.0, 150000.0),
        ("Research_AI", 85000.0, 92000.0, 110000.0, 125000.0),
        ("Infrastructure", 45000.0, 48000.0, 53000.0, 58000.0),
        ("Security_Audits", 25000.0, 15000.0, 30000.0, 20000.0),
    ];

    for (idx, (cat, q1, q2, q3, q4)) in categories.iter().enumerate() {
        let r = (idx + 2) as u32;
        sheet.set_string(r, 1, *cat);
        sheet.set_number(r, 2, *q1);
        sheet.set_number(r, 3, *q2);
        sheet.set_number(r, 4, *q3);
        sheet.set_number(r, 5, *q4);
        sheet.set_formula(
            r,
            6,
            format!("SUM(B{}:E{})", r, r),
            Some(q1 + q2 + q3 + q4),
        );
    }
    sheet.freeze_panes(1, 1);

    // Also import a second sheet from CSV
    let csv_metrics = "Node_ID,Uptime_Pct,Latency_ms\nnode-useast-01,99.99,4.2\nnode-uswest-02,99.95,6.8\nnode-euwest-01,99.98,5.1";
    wb.import_csv("System_Metrics", csv_metrics).expect("import csv");

    wb.save(&xlsx_path).expect("save xlsx");
    assert!(xlsx_path.exists());

    // Validate with OfficeValidator
    let xlsx_report = OfficeValidator::validate_file(&xlsx_path).expect("validate xlsx");
    assert!(xlsx_report.valid);
    assert_eq!(xlsx_report.kind, OfficeDocumentKind::ExcelXlsx);
    assert!(xlsx_report.extracted_text_snippets.iter().any(|s| s.contains("Financial_Projections") || s.contains("Engineering")));

    // 3. Generate PPTX
    let pptx_path = out_dir.join("OSMOO_Sample_Presentation.pptx");
    let mut pres = PptxPresentation::new("OSMOO Companion Vision");
    pres.with_author("OSMOO Core Team");
    pres.add_title_slide(
        "OSMOO Companion 2027",
        "Autonomous AI Desktop Companion with Native OpenXML Office Capabilities",
    );
    pres.add_content_slide(
        "Native Office Architecture",
        vec![
            "Zero external runtime dependencies (no headless Office or Python required)".to_string(),
            "Standard ECMA-376 OpenXML standard compliance".to_string(),
            "Full support for Word styles, Excel formulas, and PowerPoint widescreen layouts".to_string(),
            "Automated internal structural validation via OfficeValidator".to_string(),
        ],
    );
    pres.add_table_slide(
        "Enterprise Readiness Assessment",
        vec!["Dimension".to_string(), "Standard Requirement".to_string(), "OSMOO Implementation".to_string()],
        vec![
            vec!["File Integrity".to_string(), "100% valid XML package".to_string(), "Verified by ZIP/XML schemas".to_string()],
            vec!["Security".to_string(), "No raw cards / approval gates".to_string(), "Zero card storage / Gated email send".to_string()],
            vec!["Scheduling".to_string(), "Survive restarts".to_string(), "Persistent SQLite database".to_string()],
        ],
    );
    pres.set_speaker_notes(0, "Presenter notes for Title Slide: Welcome all attendees.");
    pres.set_speaker_notes(1, "Presenter notes for Slide 2: Emphasize zero runtime dependencies.");
    pres.save(&pptx_path).expect("save pptx");
    assert!(pptx_path.exists());

    // Validate with OfficeValidator
    let pptx_report = OfficeValidator::validate_file(&pptx_path).expect("validate pptx");
    assert!(pptx_report.valid);
    assert_eq!(pptx_report.kind, OfficeDocumentKind::PowerPointPptx);
    assert!(pptx_report.extracted_text_snippets.iter().any(|s| s.contains("OSMOO Companion 2027")));
}
