#!/usr/bin/env python3
"""
Independent End-to-End Validation Script for OSMOO Generated Office Deliverables.
Validates .docx, .xlsx, and .pptx using python-docx, openpyxl, python-pptx,
zipfile, and xml.etree.ElementTree.
"""

import os
import sys
import zipfile
import xml.etree.ElementTree as ET
import hashlib

def hash_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()

def validate_zip_and_xmls(path):
    print(f"\n[1. ZIP & XML Parsing] Inspecting: {path}")
    assert os.path.exists(path), f"File not found: {path}"
    file_size = os.path.getsize(path)
    file_sha = hash_file(path)
    print(f"  Size: {file_size} bytes | SHA256: {file_sha[:16]}...")

    with zipfile.ZipFile(path, 'r') as z:
        namelist = z.namelist()
        print(f"  Archive contains {len(namelist)} items:")
        for name in namelist:
            print(f"    - {name}")

        # Check essential package parts
        assert "[Content_Types].xml" in namelist, "Missing [Content_Types].xml"
        assert "_rels/.rels" in namelist, "Missing _rels/.rels"

        # Validate that every single XML file parses without syntax error
        xml_count = 0
        for name in namelist:
            if name.endswith(".xml") or name.endswith(".rels"):
                xml_count += 1
                xml_data = z.read(name)
                try:
                    root = ET.fromstring(xml_data)
                    # Verify root is a valid XML element
                    assert root is not None
                except Exception as e:
                    raise AssertionError(f"XML parse error in {name}: {e}")
        print(f"  [OK] All {xml_count} XML and .rels parts parsed successfully with zero syntax errors!")

def test_docx(docx_path):
    print(f"\n[2. python-docx End-to-End Verification] Opening {docx_path}")
    import docx
    doc = docx.Document(docx_path)

    paragraphs = [p.text for p in doc.paragraphs if p.text.strip()]
    print(f"  Extracted {len(paragraphs)} non-empty paragraphs/headings:")
    for i, p in enumerate(paragraphs):
        print(f"    P{i+1}: {p[:75]}...")

    # Assertions
    assert any("OSMOO Architecture" in p for p in paragraphs), "Missing main title"
    assert any("1. Executive Summary" in p for p in paragraphs), "Missing Executive Summary"
    assert any("Native OpenXML binary document packaging" in p for p in paragraphs), "Missing bullet item"

    # Tables inspection
    print(f"  Extracted {len(doc.tables)} table(s):")
    for t_idx, table in enumerate(doc.tables):
        print(f"  Table {t_idx+1}: {len(table.rows)} rows x {len(table.columns)} columns")
        for r_idx, row in enumerate(table.rows):
            row_vals = [cell.text.strip() for cell in row.cells]
            print(f"    Row {r_idx+1}: {row_vals}")
        assert len(table.rows) >= 4, "Table should have at least 4 rows"
        header_vals = [cell.text.strip() for cell in table.rows[0].cells]
        assert "Metric" in header_vals and "Compliance" in header_vals, "Table headers mismatch"
        data_row1 = [cell.text.strip() for cell in table.rows[1].cells]
        assert "Docx Generation Time" in data_row1, "Data row 1 missing"

    print("  [OK] python-docx verification PASSED with 100% compliance!")

def test_xlsx(xlsx_path):
    print(f"\n[3. openpyxl End-to-End Verification] Opening {xlsx_path}")
    import openpyxl
    wb = openpyxl.load_workbook(xlsx_path, data_only=False)

    sheet_names = wb.sheetnames
    print(f"  Found {len(sheet_names)} sheets: {sheet_names}")
    assert "Financial_Projections" in sheet_names, "Missing Financial_Projections sheet"
    assert "System_Metrics" in sheet_names, "Missing System_Metrics sheet"

    # Inspect Financial_Projections
    ws1 = wb["Financial_Projections"]
    print(f"  Sheet 'Financial_Projections' dimensions: {ws1.dimensions}")
    headers = [ws1.cell(row=1, column=c).value for c in range(1, 7)]
    print(f"    Headers (Row 1): {headers}")
    assert headers == ["Category", "Q1_USD", "Q2_USD", "Q3_USD", "Q4_USD", "Total_USD"], "Headers mismatch"

    # Check engineering row
    cat = ws1.cell(row=2, column=1).value
    q1 = ws1.cell(row=2, column=2).value
    formula = ws1.cell(row=2, column=6).value
    print(f"    Row 2 (Engineering): Cat={cat}, Q1={q1}, Formula={formula}")
    assert cat == "Engineering"
    assert q1 == 120000.0 or q1 == 120000
    assert formula == "=SUM(B2:E2)" or formula == "SUM(B2:E2)" or "SUM(B2:E2)" in str(formula)

    # Inspect System_Metrics (from CSV import)
    ws2 = wb["System_Metrics"]
    print(f"  Sheet 'System_Metrics' dimensions: {ws2.dimensions}")
    h2 = [ws2.cell(row=1, column=c).value for c in range(1, 4)]
    print(f"    Headers (Row 1): {h2}")
    assert h2 == ["Node_ID", "Uptime_Pct", "Latency_ms"]
    node1 = ws2.cell(row=2, column=1).value
    uptime1 = ws2.cell(row=2, column=2).value
    print(f"    Row 2: Node={node1}, Uptime={uptime1}")
    assert node1 == "node-useast-01"

    print("  [OK] openpyxl verification PASSED with 100% compliance!")

def test_pptx(pptx_path):
    print(f"\n[4. python-pptx End-to-End Verification] Opening {pptx_path}")
    import pptx
    pres = pptx.Presentation(pptx_path)

    slides = pres.slides
    print(f"  Found {len(slides)} slides in presentation:")
    assert len(slides) == 3, f"Expected 3 slides, found {len(slides)}"

    # Slide 1: Title Slide
    s1 = slides[0]
    s1_texts = []
    for shape in s1.shapes:
        if shape.has_text_frame:
            for p in shape.text_frame.paragraphs:
                if p.text.strip():
                    s1_texts.append(p.text.strip())
    print(f"  Slide 1 texts: {s1_texts}")
    assert any("OSMOO Companion 2027" in t for t in s1_texts), "Slide 1 title missing"
    assert any("Autonomous AI Desktop Companion" in t for t in s1_texts), "Slide 1 subtitle missing"
    # Check notes
    if s1.has_notes_slide:
        notes_text = s1.notes_slide.notes_text_frame.text
        print(f"    Slide 1 Presenter Notes: {notes_text}")
        assert "Welcome all attendees" in notes_text, "Slide 1 notes missing"

    # Slide 2: Bullet Body Slide
    s2 = slides[1]
    s2_texts = []
    for shape in s2.shapes:
        if shape.has_text_frame:
            for p in shape.text_frame.paragraphs:
                if p.text.strip():
                    s2_texts.append(p.text.strip())
    print(f"  Slide 2 texts: {s2_texts}")
    assert any("Native Office Architecture" in t for t in s2_texts)
    assert any("Zero external runtime dependencies" in t for t in s2_texts)

    # Slide 3: Table Slide
    s3 = slides[2]
    table_found = False
    for shape in s3.shapes:
        if shape.has_table:
            table_found = True
            tbl = shape.table
            print(f"  Slide 3 Table: {len(tbl.rows)} rows x {len(tbl.columns)} columns")
            for r_idx, row in enumerate(tbl.rows):
                r_cells = [cell.text.strip() for cell in row.cells]
                print(f"    Row {r_idx+1}: {r_cells}")
            assert len(tbl.rows) == 4, "Slide 3 table rows mismatch"
            assert "Dimension" in [c.text.strip() for c in tbl.rows[0].cells]
    assert table_found, "Slide 3 table shape not found"

    print("  [OK] python-pptx verification PASSED with 100% compliance!")

if __name__ == "__main__":
    docx_file = os.path.join("test_output", "OSMOO_Sample_Document.docx")
    xlsx_file = os.path.join("test_output", "OSMOO_Sample_Spreadsheet.xlsx")
    pptx_file = os.path.join("test_output", "OSMOO_Sample_Presentation.pptx")

    print("=====================================================================")
    print("      OSMOO INDEPENDENT OFFICE OPENXML VERIFICATION HARNESS")
    print("=====================================================================")

    # Step 1: Physical archive integrity & XML well-formedness
    validate_zip_and_xmls(docx_file)
    validate_zip_and_xmls(xlsx_file)
    validate_zip_and_xmls(pptx_file)

    # Step 2: Semantic object model parsing with industry-standard libraries
    test_docx(docx_file)
    test_xlsx(xlsx_file)
    test_pptx(pptx_file)

    print("\n=====================================================================")
    print("  >>> ALL 3 DELIVERABLES FULLY VALIDATED BY INDEPENDENT PARSERS! <<<")
    print("=====================================================================")
