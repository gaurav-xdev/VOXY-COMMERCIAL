//! Native OpenXML Word (.docx) document generator.
//! Produces valid, standard-compliant .docx packages without external runtime dependencies.

use crate::error::{OfficeError, Result};
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Debug, Clone)]
pub enum DocxElement {
    Heading {
        text: String,
        level: u8,
    },
    Paragraph {
        text: String,
        bold: bool,
        italic: bool,
    },
    BulletItem {
        text: String,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    PageBreak,
}

#[derive(Debug, Clone)]
pub struct DocxDocument {
    pub title: String,
    pub author: String,
    pub elements: Vec<DocxElement>,
}

impl DocxDocument {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            author: "OSMOO AI Operating Companion".to_string(),
            elements: Vec::new(),
        }
    }

    pub fn with_author(&mut self, author: impl Into<String>) -> &mut Self {
        self.author = author.into();
        self
    }

    pub fn add_heading(&mut self, text: impl Into<String>, level: u8) -> &mut Self {
        self.elements.push(DocxElement::Heading {
            text: text.into(),
            level: level.clamp(1, 3),
        });
        self
    }

    pub fn add_paragraph(&mut self, text: impl Into<String>) -> &mut Self {
        self.elements.push(DocxElement::Paragraph {
            text: text.into(),
            bold: false,
            italic: false,
        });
        self
    }

    pub fn add_styled_paragraph(
        &mut self,
        text: impl Into<String>,
        bold: bool,
        italic: bool,
    ) -> &mut Self {
        self.elements.push(DocxElement::Paragraph {
            text: text.into(),
            bold,
            italic,
        });
        self
    }

    pub fn add_bullet(&mut self, text: impl Into<String>) -> &mut Self {
        self.elements
            .push(DocxElement::BulletItem { text: text.into() });
        self
    }

    pub fn add_table(&mut self, headers: Vec<String>, rows: Vec<Vec<String>>) -> &mut Self {
        self.elements.push(DocxElement::Table { headers, rows });
        self
    }

    pub fn add_page_break(&mut self) -> &mut Self {
        self.elements.push(DocxElement::PageBreak);
        self
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    /// Build the main word/document.xml payload
    fn build_document_xml(&self) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
"#,
        );

        // Document Title
        if !self.title.is_empty() {
            xml.push_str(&format!(
                r#"    <w:p>
      <w:pPr>
        <w:pStyle w:val="Title"/>
        <w:jc w:val="center"/>
      </w:pPr>
      <w:r>
        <w:rPr>
          <w:b/>
          <w:sz w:val="48"/>
          <w:color w:val="1F497D"/>
        </w:rPr>
        <w:t>{}</w:t>
      </w:r>
    </w:p>
"#,
                Self::xml_escape(&self.title)
            ));
        }

        // Process elements
        for el in &self.elements {
            match el {
                DocxElement::Heading { text, level } => {
                    let sz = match level {
                        1 => "36",
                        2 => "28",
                        _ => "24",
                    };
                    let color = match level {
                        1 => "1F497D",
                        2 => "595959",
                        _ => "000000",
                    };
                    xml.push_str(&format!(
                        r#"    <w:p>
      <w:pPr>
        <w:pStyle w:val="Heading{}"/>
        <w:spacing w:before="240" w:after="120"/>
      </w:pPr>
      <w:r>
        <w:rPr>
          <w:b/>
          <w:sz w:val="{}"/>
          <w:color w:val="{}"/>
        </w:rPr>
        <w:t>{}</w:t>
      </w:r>
    </w:p>
"#,
                        level,
                        sz,
                        color,
                        Self::xml_escape(text)
                    ));
                }
                DocxElement::Paragraph { text, bold, italic } => {
                    let mut rpr = String::new();
                    if *bold {
                        rpr.push_str("<w:b/>");
                    }
                    if *italic {
                        rpr.push_str("<w:i/>");
                    }
                    xml.push_str(&format!(
                        r#"    <w:p>
      <w:pPr>
        <w:spacing w:after="160" w:line="276" w:lineRule="auto"/>
      </w:pPr>
      <w:r>
        <w:rPr>{}</w:rPr>
        <w:t xml:space="preserve">{}</w:t>
      </w:r>
    </w:p>
"#,
                        rpr,
                        Self::xml_escape(text)
                    ));
                }
                DocxElement::BulletItem { text } => {
                    xml.push_str(&format!(
                        r#"    <w:p>
      <w:pPr>
        <w:pStyle w:val="ListParagraph"/>
        <w:numPr>
          <w:ilvl w:val="0"/>
          <w:numId w:val="1"/>
        </w:numPr>
        <w:spacing w:after="80"/>
      </w:pPr>
      <w:r>
        <w:t xml:space="preserve">&#x2022; {}</w:t>
      </w:r>
    </w:p>
"#,
                        Self::xml_escape(text)
                    ));
                }
                DocxElement::Table { headers, rows } => {
                    let col_count = if !headers.is_empty() {
                        headers.len()
                    } else {
                        rows.first().map(|r| r.len()).unwrap_or(1)
                    };
                    let col_width = (9000 / col_count.max(1)).to_string();

                    xml.push_str(
                        r#"    <w:tbl>
      <w:tblPr>
        <w:tblStyle w:val="TableGrid"/>
        <w:tblW w:w="5000" w:type="pct"/>
        <w:tblBorders>
          <w:top w:val="single" w:sz="4" w:space="0" w:color="D3D3D3"/>
          <w:left w:val="single" w:sz="4" w:space="0" w:color="D3D3D3"/>
          <w:bottom w:val="single" w:sz="8" w:space="0" w:color="1F497D"/>
          <w:right w:val="single" w:sz="4" w:space="0" w:color="D3D3D3"/>
          <w:insideH w:val="single" w:sz="4" w:space="0" w:color="E0E0E0"/>
          <w:insideV w:val="single" w:sz="4" w:space="0" w:color="E0E0E0"/>
        </w:tblBorders>
      </w:tblPr>
      <w:tblGrid>
"#,
                    );

                    for _ in 0..col_count {
                        xml.push_str(&format!(r#"        <w:gridCol w:w="{}"/>"#, col_width));
                        xml.push('\n');
                    }
                    xml.push_str("      </w:tblGrid>\n");

                    // Header row
                    if !headers.is_empty() {
                        xml.push_str(
                            r#"      <w:tr>
        <w:trPr>
          <w:tblHeader/>
        </w:trPr>
"#,
                        );
                        for h in headers {
                            xml.push_str(&format!(
                                r#"        <w:tc>
          <w:tcPr>
            <w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/>
          </w:tcPr>
          <w:p>
            <w:r>
              <w:rPr>
                <w:b/>
                <w:color w:val="1F497D"/>
              </w:rPr>
              <w:t>{}</w:t>
            </w:r>
          </w:p>
        </w:tc>
"#,
                                Self::xml_escape(h)
                            ));
                        }
                        xml.push_str("      </w:tr>\n");
                    }

                    // Body rows
                    for r in rows {
                        xml.push_str("      <w:tr>\n");
                        for c in r {
                            xml.push_str(&format!(
                                r#"        <w:tc>
          <w:p>
            <w:r>
              <w:t>{}</w:t>
            </w:r>
          </w:p>
        </w:tc>
"#,
                                Self::xml_escape(c)
                            ));
                        }
                        xml.push_str("      </w:tr>\n");
                    }

                    xml.push_str("    </w:tbl>\n");
                }
                DocxElement::PageBreak => {
                    xml.push_str("    <w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>\n");
                }
            }
        }

        // Section properties
        xml.push_str(
            r#"    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>
"#,
        );
        xml
    }

    /// Generate raw binary bytes of the .docx package
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        {
            let mut zip = ZipWriter::new(&mut buffer);

            // 1. [Content_Types].xml
            zip.start_file("[Content_Types].xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // 2. _rels/.rels
            zip.start_file("_rels/.rels", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // 3. word/document.xml
            zip.start_file("word/document.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let doc_xml = self.build_document_xml();
            zip.write_all(doc_xml.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // word/_rels/document.xml.rels
            zip.start_file("word/_rels/document.xml.rels", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // word/styles.xml
            zip.start_file("word/styles.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/>
        <w:sz w:val="22"/>
      </w:rPr>
    </w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:rPr><w:b/><w:sz w:val="32"/><w:color w:val="1F497D"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:rPr><w:b/><w:sz w:val="26"/><w:color w:val="595959"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Title">
    <w:name w:val="Title"/>
    <w:rPr><w:b/><w:sz w:val="48"/><w:color w:val="1F497D"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="ListParagraph">
    <w:name w:val="List Paragraph"/>
  </w:style>
  <w:style w:type="table" w:default="1" w:styleId="TableGrid">
    <w:name w:val="Table Grid"/>
    <w:tblPr>
      <w:tblBorders>
        <w:top w:val="single" w:sz="4" w:space="0" w:color="auto"/>
        <w:left w:val="single" w:sz="4" w:space="0" w:color="auto"/>
        <w:bottom w:val="single" w:sz="4" w:space="0" w:color="auto"/>
        <w:right w:val="single" w:sz="4" w:space="0" w:color="auto"/>
        <w:insideH w:val="single" w:sz="4" w:space="0" w:color="auto"/>
        <w:insideV w:val="single" w:sz="4" w:space="0" w:color="auto"/>
      </w:tblBorders>
    </w:tblPr>
  </w:style>
</w:styles>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // 4. docProps/core.xml (Metadata)
            zip.start_file("docProps/core.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let now = chrono::Utc::now().to_rfc3339();
            let core_xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/"
                   xmlns:dcterms="http://purl.org/dc/terms/"
                   xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>{}</dc:title>
  <dc:creator>{}</dc:creator>
  <cp:lastModifiedBy>OSMOO Engine</cp:lastModifiedBy>
  <dcterms:created xsi:type="dcterms:W3CDTF">{}</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">{}</dcterms:modified>
</cp:coreProperties>"#,
                Self::xml_escape(&self.title),
                Self::xml_escape(&self.author),
                now,
                now
            );
            zip.write_all(core_xml.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // 5. docProps/app.xml
            zip.start_file("docProps/app.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            zip.write_all(
                br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>OSMOO AI Operating Companion</Application>
  <DocSecurity>0</DocSecurity>
  <Lines>10</Lines>
  <Paragraphs>5</Paragraphs>
  <Company>VOXY Commercial</Company>
</Properties>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            zip.finish()
                .map_err(|e| OfficeError::InvalidData(format!("Zip finalize error: {e}")))?;
        }

        Ok(buffer.into_inner())
    }

    /// Save the document directly to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let bytes = self.to_bytes()?;
        let mut file = File::create(path.as_ref())
            .map_err(|e| OfficeError::Io(format!("Failed to create file: {e}")))?;
        file.write_all(&bytes)
            .map_err(|e| OfficeError::Io(format!("Failed to write file: {e}")))?;
        Ok(())
    }
}
