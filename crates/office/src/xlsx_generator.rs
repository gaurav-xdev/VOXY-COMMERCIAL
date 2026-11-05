//! Native OpenXML Excel (.xlsx) workbook generator.
//! Produces valid, standard-compliant .xlsx workbooks with numbers, formulas,
//! string tables, formatting, frozen panes, and CSV import without external runtime dependencies.

use crate::error::{OfficeError, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Debug, Clone)]
pub enum CellValue {
    String(String),
    Number(f64),
    Formula {
        expr: String,
        cached_val: Option<f64>,
    },
}

#[derive(Debug, Clone)]
pub struct XlsxSheet {
    pub name: String,
    pub cells: HashMap<(u32, u32), CellValue>, // (row, col) 1-indexed
    pub freeze_row: Option<u32>,
    pub freeze_col: Option<u32>,
}

impl XlsxSheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: HashMap::new(),
            freeze_row: None,
            freeze_col: None,
        }
    }

    pub fn set_string(&mut self, row: u32, col: u32, val: impl Into<String>) {
        self.cells.insert((row, col), CellValue::String(val.into()));
    }

    pub fn set_number(&mut self, row: u32, col: u32, val: f64) {
        self.cells.insert((row, col), CellValue::Number(val));
    }

    pub fn set_formula(
        &mut self,
        row: u32,
        col: u32,
        expr: impl Into<String>,
        cached: Option<f64>,
    ) {
        self.cells.insert(
            (row, col),
            CellValue::Formula {
                expr: expr.into(),
                cached_val: cached,
            },
        );
    }

    pub fn freeze_panes(&mut self, row: u32, col: u32) {
        self.freeze_row = Some(row);
        self.freeze_col = Some(col);
    }
}

#[derive(Debug, Clone)]
pub struct XlsxWorkbook {
    pub sheets: Vec<XlsxSheet>,
}

impl Default for XlsxWorkbook {
    fn default() -> Self {
        Self::new()
    }
}

impl XlsxWorkbook {
    pub fn new() -> Self {
        Self { sheets: Vec::new() }
    }

    pub fn add_sheet(&mut self, name: impl Into<String>) -> &mut XlsxSheet {
        self.sheets.push(XlsxSheet::new(name));
        self.sheets.last_mut().unwrap()
    }

    pub fn get_or_create_sheet(&mut self, name: &str) -> &mut XlsxSheet {
        if let Some(pos) = self.sheets.iter().position(|s| s.name == name) {
            &mut self.sheets[pos]
        } else {
            self.add_sheet(name)
        }
    }

    pub fn import_csv(&mut self, sheet_name: &str, csv_text: &str) -> Result<()> {
        let sheet = self.get_or_create_sheet(sheet_name);
        let mut row_idx = 1;

        for line in csv_text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let cols: Vec<&str> = line.split(',').collect();
            for (col_idx, val) in cols.into_iter().enumerate() {
                let val = val.trim().trim_matches('"');
                let c_idx = (col_idx + 1) as u32;

                if let Ok(num) = val.parse::<f64>() {
                    sheet.set_number(row_idx, c_idx, num);
                } else {
                    sheet.set_string(row_idx, c_idx, val);
                }
            }
            row_idx += 1;
        }

        Ok(())
    }

    fn col_name(mut col: u32) -> String {
        let mut name = String::new();
        while col > 0 {
            col -= 1;
            let rem = (col % 26) as u8;
            name.insert(0, (b'A' + rem) as char);
            col /= 26;
        }
        name
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        if self.sheets.is_empty() {
            return Err(OfficeError::InvalidData(
                "Workbook must contain at least one sheet".into(),
            ));
        }

        // 1. Gather all shared strings
        let mut shared_strings: Vec<String> = Vec::new();
        let mut string_indices: HashMap<String, usize> = HashMap::new();

        for sheet in &self.sheets {
            for val in sheet.cells.values() {
                if let CellValue::String(s) = val {
                    if !string_indices.contains_key(s) {
                        let idx = shared_strings.len();
                        shared_strings.push(s.clone());
                        string_indices.insert(s.clone(), idx);
                    }
                }
            }
        }

        let mut buffer = Cursor::new(Vec::new());
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        {
            let mut zip = ZipWriter::new(&mut buffer);

            // [Content_Types].xml
            zip.start_file("[Content_Types].xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let mut ct = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
  <Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>
"#,
            );
            for i in 1..=self.sheets.len() {
                ct.push_str(&format!(
                    r#"  <Override PartName="/xl/worksheets/sheet{}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
"#,
                    i
                ));
            }
            ct.push_str("</Types>");
            zip.write_all(ct.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // _rels/.rels
            zip.start_file("_rels/.rels", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // xl/_rels/workbook.xml.rels
            zip.start_file("xl/_rels/workbook.xml.rels", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let mut wb_rels = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rIdSharedStrings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>
"#,
            );
            for (idx, _sheet) in self.sheets.iter().enumerate() {
                let id = idx + 1;
                wb_rels.push_str(&format!(
                    r#"  <Relationship Id="rIdSheet{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{}.xml"/>
"#,
                    id, id
                ));
            }
            wb_rels.push_str("</Relationships>");
            zip.write_all(wb_rels.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // xl/workbook.xml
            zip.start_file("xl/workbook.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let mut wb_xml = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
"#,
            );
            for (idx, sheet) in self.sheets.iter().enumerate() {
                let id = idx + 1;
                wb_xml.push_str(&format!(
                    r#"    <sheet name="{}" sheetId="{}" r:id="rIdSheet{}"/>
"#,
                    Self::xml_escape(&sheet.name),
                    id,
                    id
                ));
            }
            wb_xml.push_str(
                r#"  </sheets>
</workbook>"#,
            );
            zip.write_all(wb_xml.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // xl/styles.xml
            zip.start_file("xl/styles.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="2">
    <font><name val="Calibri"/><sz val="11"/></font>
    <font><name val="Calibri"/><sz val="11"/><b/><color rgb="FF1F497D"/></font>
  </fonts>
  <fills count="2">
    <fill><patternFill patternType="none"/></fill>
    <fill><patternFill patternType="gray125"/></fill>
  </fills>
  <borders count="1">
    <border><left/><right/><top/><bottom/><diagonal/></border>
  </borders>
  <cellStyleXfs count="1">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0"/>
  </cellStyleXfs>
  <cellXfs count="2">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
    <xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>
  </cellXfs>
</styleSheet>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // xl/sharedStrings.xml
            zip.start_file("xl/sharedStrings.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let mut ss_xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{}" uniqueCount="{}">
"#,
                shared_strings.len(),
                shared_strings.len()
            );
            for s in &shared_strings {
                ss_xml.push_str(&format!(
                    "  <si><t xml:space=\"preserve\">{}</t></si>\n",
                    Self::xml_escape(s)
                ));
            }
            ss_xml.push_str("</sst>");
            zip.write_all(ss_xml.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // xl/worksheets/sheetN.xml
            for (idx, sheet) in self.sheets.iter().enumerate() {
                let id = idx + 1;
                zip.start_file(format!("xl/worksheets/sheet{}.xml", id), options)
                    .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;

                let mut ws_xml = String::from(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
"#,
                );

                if let (Some(r), Some(c)) = (sheet.freeze_row, sheet.freeze_col) {
                    ws_xml.push_str(&format!(
                        r#"  <sheetViews>
    <sheetView tabSelected="1" workbookViewId="0">
      <pane ySplit="{}" xSplit="{}" topLeftCell="{}{}" activePane="bottomRight" state="frozen"/>
    </sheetView>
  </sheetViews>
"#,
                        r,
                        c,
                        Self::col_name(c + 1),
                        r + 1
                    ));
                }

                ws_xml.push_str("  <sheetData>\n");

                // Group cells by row
                let mut max_row = 0;
                for &(r, _) in sheet.cells.keys() {
                    if r > max_row {
                        max_row = r;
                    }
                }

                for r in 1..=max_row {
                    let mut row_cells: Vec<(u32, &CellValue)> = sheet
                        .cells
                        .iter()
                        .filter(|&(&(row, _), _)| row == r)
                        .map(|(&(_, col), val)| (col, val))
                        .collect();
                    row_cells.sort_by_key(|&(c, _)| c);

                    if row_cells.is_empty() {
                        continue;
                    }

                    ws_xml.push_str(&format!("    <row r=\"{}\">\n", r));
                    for (c, val) in row_cells {
                        let ref_str = format!("{}{}", Self::col_name(c), r);
                        let style_attr = if r == 1 { " s=\"1\"" } else { "" };

                        match val {
                            CellValue::String(s) => {
                                let s_idx = string_indices[s];
                                ws_xml.push_str(&format!(
                                    "      <c r=\"{}\" t=\"s\"{}>\n        <v>{}</v>\n      </c>\n",
                                    ref_str, style_attr, s_idx
                                ));
                            }
                            CellValue::Number(n) => {
                                ws_xml.push_str(&format!(
                                    "      <c r=\"{}\"{}>\n        <v>{}</v>\n      </c>\n",
                                    ref_str, style_attr, n
                                ));
                            }
                            CellValue::Formula { expr, cached_val } => {
                                let cached = cached_val
                                    .map(|v| format!("\n        <v>{}</v>", v))
                                    .unwrap_or_default();
                                ws_xml.push_str(&format!(
                                    "      <c r=\"{}\"{}>\n        <f>{}</f>{} \n      </c>\n",
                                    ref_str,
                                    style_attr,
                                    Self::xml_escape(expr),
                                    cached
                                ));
                            }
                        }
                    }
                    ws_xml.push_str("    </row>\n");
                }

                ws_xml.push_str(
                    r#"  </sheetData>
</worksheet>"#,
                );
                zip.write_all(ws_xml.as_bytes())
                    .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;
            }

            zip.finish()
                .map_err(|e| OfficeError::InvalidData(format!("Zip finalize error: {e}")))?;
        }

        Ok(buffer.into_inner())
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let bytes = self.to_bytes()?;
        let mut file = File::create(path.as_ref())
            .map_err(|e| OfficeError::Io(format!("Failed to create file: {e}")))?;
        file.write_all(&bytes)
            .map_err(|e| OfficeError::Io(format!("Failed to write file: {e}")))?;
        Ok(())
    }
}
