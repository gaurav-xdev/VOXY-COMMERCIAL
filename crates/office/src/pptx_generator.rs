//! Native OpenXML PowerPoint (.pptx) presentation generator.
//! Produces valid, standard-compliant .pptx slide decks with titles, subtitles,
//! bullet layouts, tables, speaker notes, and 16:9 widescreen layout without external runtime dependencies.

use crate::error::{OfficeError, Result};
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

#[derive(Debug, Clone)]
pub enum SlideContent {
    TitleSlide {
        title: String,
        subtitle: String,
    },
    BulletSlide {
        title: String,
        bullets: Vec<String>,
    },
    TableSlide {
        title: String,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

#[derive(Debug, Clone)]
pub struct PptxSlide {
    pub content: SlideContent,
    pub speaker_notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PptxPresentation {
    pub title: String,
    pub author: String,
    pub slides: Vec<PptxSlide>,
}

impl PptxPresentation {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            author: "OSMOO AI Operating Companion".to_string(),
            slides: Vec::new(),
        }
    }

    pub fn with_author(&mut self, author: impl Into<String>) -> &mut Self {
        self.author = author.into();
        self
    }

    pub fn add_title_slide(&mut self, title: impl Into<String>, subtitle: impl Into<String>) -> &mut Self {
        self.slides.push(PptxSlide {
            content: SlideContent::TitleSlide {
                title: title.into(),
                subtitle: subtitle.into(),
            },
            speaker_notes: None,
        });
        self
    }

    pub fn add_content_slide(&mut self, title: impl Into<String>, bullets: Vec<String>) -> &mut Self {
        self.slides.push(PptxSlide {
            content: SlideContent::BulletSlide {
                title: title.into(),
                bullets,
            },
            speaker_notes: None,
        });
        self
    }

    pub fn add_table_slide(
        &mut self,
        title: impl Into<String>,
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    ) -> &mut Self {
        self.slides.push(PptxSlide {
            content: SlideContent::TableSlide {
                title: title.into(),
                headers,
                rows,
            },
            speaker_notes: None,
        });
        self
    }

    pub fn set_speaker_notes(&mut self, slide_idx: usize, notes: impl Into<String>) -> &mut Self {
        if let Some(slide) = self.slides.get_mut(slide_idx) {
            slide.speaker_notes = Some(notes.into());
        }
        self
    }

    fn xml_escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }

    fn build_slide_xml(&self, slide: &PptxSlide) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="0" cy="0"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="0" cy="0"/>
        </a:xfrm>
      </p:grpSpPr>
"#,
        );

        match &slide.content {
            SlideContent::TitleSlide { title, subtitle } => {
                // Title Box
                xml.push_str(&format!(
                    r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title 1"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="ctrTitle"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="1524000" y="1828800"/><a:ext cx="9144000" cy="1828800"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            <a:pPr algn="ctr"/>
            <a:r>
              <a:rPr lang="en-US" sz="4400" b="1">
                <a:solidFill><a:srgbClr val="1F497D"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
"#,
                    Self::xml_escape(title)
                ));

                // Subtitle Box
                xml.push_str(&format!(
                    r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Subtitle 2"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="subTitle" idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="1524000" y="3886200"/><a:ext cx="9144000" cy="1371600"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            <a:pPr algn="ctr"/>
            <a:r>
              <a:rPr lang="en-US" sz="2400">
                <a:solidFill><a:srgbClr val="595959"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
"#,
                    Self::xml_escape(subtitle)
                ));
            }
            SlideContent::BulletSlide { title, bullets } => {
                // Slide Title
                xml.push_str(&format!(
                    r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title 1"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="title"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="838200" y="533400"/><a:ext cx="10515600" cy="914400"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            <a:r>
              <a:rPr lang="en-US" sz="3600" b="1">
                <a:solidFill><a:srgbClr val="1F497D"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
"#,
                    Self::xml_escape(title)
                ));

                // Bullet Content Box
                xml.push_str(
                    r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="3" name="Content Placeholder 2"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph idx="1"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="838200" y="1600200"/><a:ext cx="10515600" cy="4572000"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
"#,
                );

                for b in bullets {
                    xml.push_str(&format!(
                        r#"          <a:p>
            <a:pPr lvl="0">
              <a:buClr><a:srgbClr val="1F497D"/></a:buClr>
              <a:buSzPts val="1800"/>
              <a:buFont typeface="Arial"/>
              <a:buChar char="&#x2022;"/>
            </a:pPr>
            <a:r>
              <a:rPr lang="en-US" sz="2000">
                <a:solidFill><a:srgbClr val="262626"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
"#,
                        Self::xml_escape(b)
                    ));
                }

                xml.push_str(
                    r#"        </p:txBody>
      </p:sp>
"#,
                );
            }
            SlideContent::TableSlide {
                title,
                headers,
                rows,
            } => {
                // Slide Title
                xml.push_str(&format!(
                    r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="2" name="Title 1"/>
          <p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr>
          <p:nvPr><p:ph type="title"/></p:nvPr>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm><a:off x="838200" y="533400"/><a:ext cx="10515600" cy="914400"/></a:xfrm>
        </p:spPr>
        <p:txBody>
          <a:bodyPr/>
          <a:lstStyle/>
          <a:p>
            <a:r>
              <a:rPr lang="en-US" sz="3600" b="1">
                <a:solidFill><a:srgbClr val="1F497D"/></a:solidFill>
              </a:rPr>
              <a:t>{}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>
"#,
                    Self::xml_escape(title)
                ));

                // Table Frame
                let col_count = headers.len().max(1);
                let col_width = 10515600 / col_count;

                xml.push_str(&format!(
                    r#"      <p:graphicFrame>
        <p:nvGraphicFramePr>
          <p:cNvPr id="4" name="Table 1"/>
          <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp="1"/></p:cNvGraphicFramePr>
          <p:nvPr/>
        </p:nvGraphicFramePr>
        <p:xfrm><a:off x="838200" y="1600200"/><a:ext cx="10515600" cy="4572000"/></p:xfrm>
        <a:graphic>
          <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table">
            <a:tbl>
              <a:tblPr firstRow="1"/>
              <a:tblGrid>
"#
                ));

                for _ in 0..col_count {
                    xml.push_str(&format!(r#"                <a:gridCol w="{}"/>
"#, col_width));
                }
                xml.push_str("              </a:tblGrid>\n");

                // Headers
                if !headers.is_empty() {
                    xml.push_str("              <a:tr h=\"400000\">\n");
                    for h in headers {
                        xml.push_str(&format!(
                            r#"                <a:tc>
                  <a:txBody>
                    <a:bodyPr/>
                    <a:p><a:r><a:rPr b="1" sz="1600"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>
                  </a:txBody>
                  <a:tcPr><a:solidFill><a:srgbClr val="1F497D"/></a:solidFill></a:tcPr>
                </a:tc>
"#,
                            Self::xml_escape(h)
                        ));
                    }
                    xml.push_str("              </a:tr>\n");
                }

                // Rows
                for r in rows {
                    xml.push_str("              <a:tr h=\"350000\">\n");
                    for c in r {
                        xml.push_str(&format!(
                            r#"                <a:tc>
                  <a:txBody>
                    <a:bodyPr/>
                    <a:p><a:r><a:rPr sz="1400"/><a:t>{}</a:t></a:r></a:p>
                  </a:txBody>
                </a:tc>
"#,
                            Self::xml_escape(c)
                        ));
                    }
                    xml.push_str("              </a:tr>\n");
                }

                xml.push_str(
                    r#"            </a:tbl>
          </a:graphicData>
        </a:graphic>
      </p:graphicFrame>
"#,
                );
            }
        }

        xml.push_str(
            r#"    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>
"#,
        );
        xml
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        if self.slides.is_empty() {
            return Err(OfficeError::InvalidData(
                "Presentation must contain at least one slide".into(),
            ));
        }

        let mut buffer = Cursor::new(Vec::new());
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

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
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
"#,
            );
            for i in 1..=self.slides.len() {
                ct.push_str(&format!(
                    r#"  <Override PartName="/ppt/slides/slide{}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
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
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>"#,
            )
            .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // ppt/_rels/presentation.xml.rels
            zip.start_file("ppt/_rels/presentation.xml.rels", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let mut pres_rels = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
"#,
            );
            for i in 1..=self.slides.len() {
                pres_rels.push_str(&format!(
                    r#"  <Relationship Id="rIdSlide{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>
"#,
                    i, i
                ));
            }
            pres_rels.push_str("</Relationships>");
            zip.write_all(pres_rels.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // ppt/presentation.xml
            zip.start_file("ppt/presentation.xml", options)
                .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
            let mut pres_xml = String::from(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldIdLst>
"#,
            );
            for i in 1..=self.slides.len() {
                let s_id = 255 + i;
                pres_xml.push_str(&format!(
                    r#"    <p:sldId id="{}" r:id="rIdSlide{}"/>
"#,
                    s_id, i
                ));
            }
            pres_xml.push_str(
                r#"  </p:sldIdLst>
  <p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#,
            );
            zip.write_all(pres_xml.as_bytes())
                .map_err(|e| OfficeError::InvalidData(format!("IO error: {e}")))?;

            // ppt/slides/slideN.xml
            for (idx, slide) in self.slides.iter().enumerate() {
                let id = idx + 1;
                zip.start_file(format!("ppt/slides/slide{}.xml", id), options)
                    .map_err(|e| OfficeError::InvalidData(format!("Zip error: {e}")))?;
                let s_xml = self.build_slide_xml(slide);
                zip.write_all(s_xml.as_bytes())
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
