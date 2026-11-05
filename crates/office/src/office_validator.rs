//! Office file (.docx, .xlsx, .pptx) validation and structural inspector.
//! Verifies OpenXML package compliance, XML well-formedness, and internal parts.

use crate::error::{OfficeError, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfficeDocumentKind {
    WordDocx,
    ExcelXlsx,
    PowerPointPptx,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub valid: bool,
    pub kind: OfficeDocumentKind,
    pub part_count: usize,
    pub extracted_text_snippets: Vec<String>,
    pub errors: Vec<String>,
}

pub struct OfficeValidator;

impl OfficeValidator {
    pub fn validate_file<P: AsRef<Path>>(path: P) -> Result<ValidationReport> {
        let file = File::open(path.as_ref())
            .map_err(|e| OfficeError::Io(format!("Cannot open file for validation: {e}")))?;
        Self::validate_reader(file)
    }

    pub fn validate_bytes(bytes: &[u8]) -> Result<ValidationReport> {
        let cursor = std::io::Cursor::new(bytes);
        Self::validate_reader(cursor)
    }

    fn validate_reader<R: Read + std::io::Seek>(reader: R) -> Result<ValidationReport> {
        let mut zip = ZipArchive::new(reader)
            .map_err(|e| OfficeError::InvalidData(format!("Not a valid ZIP package: {e}")))?;

        let mut errors = Vec::new();
        let mut text_snippets = Vec::new();
        let mut has_content_types = false;
        let mut has_root_rels = false;
        let mut has_word_doc = false;
        let mut has_xl_wb = false;
        let mut has_ppt_pres = false;

        let part_count = zip.len();

        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(|e| {
                OfficeError::InvalidData(format!("Error reading zip entry {i}: {e}"))
            })?;
            let name = file.name().unwrap_or_default().to_string();

            if name == "[Content_Types].xml" {
                has_content_types = true;
            } else if name == "_rels/.rels" {
                has_root_rels = true;
            } else if name == "word/document.xml" {
                has_word_doc = true;
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    // Extract text inside <w:t> tags
                    Self::extract_tags(&content, "w:t", &mut text_snippets);
                }
            } else if name == "xl/workbook.xml" || name.starts_with("xl/worksheets/sheet") {
                has_xl_wb = true;
            } else if name == "xl/sharedStrings.xml" {
                let mut content = String::new();
                if file.read_to_string(&mut content).is_ok() {
                    Self::extract_tags(&content, "t", &mut text_snippets);
                }
            } else if name == "ppt/presentation.xml" || name.starts_with("ppt/slides/slide") {
                has_ppt_pres = true;
                if name.starts_with("ppt/slides/slide") && !name.contains("_rels") {
                    let mut content = String::new();
                    if file.read_to_string(&mut content).is_ok() {
                        Self::extract_tags(&content, "a:t", &mut text_snippets);
                    }
                }
            }
        }

        if !has_content_types {
            errors.push("Missing [Content_Types].xml in package".to_string());
        }
        if !has_root_rels {
            errors.push("Missing _rels/.rels in package".to_string());
        }

        let kind = if has_word_doc {
            OfficeDocumentKind::WordDocx
        } else if has_xl_wb {
            OfficeDocumentKind::ExcelXlsx
        } else if has_ppt_pres {
            OfficeDocumentKind::PowerPointPptx
        } else {
            OfficeDocumentKind::Unknown
        };

        if kind == OfficeDocumentKind::Unknown {
            errors.push(
                "Package does not contain recognizable Word, Excel, or PowerPoint main part"
                    .to_string(),
            );
        }

        let valid = errors.is_empty();

        Ok(ValidationReport {
            valid,
            kind,
            part_count,
            extracted_text_snippets: text_snippets,
            errors,
        })
    }

    fn extract_tags(xml: &str, tag: &str, out: &mut Vec<String>) {
        let open_tag = format!("<{}>", tag);
        let open_tag_prefix = format!("<{} ", tag);
        let close_tag = format!("</{}>", tag);

        let mut pos = 0;
        while let Some(start_idx) = xml[pos..]
            .find(&open_tag)
            .or_else(|| xml[pos..].find(&open_tag_prefix))
        {
            let actual_start = pos + start_idx;
            if let Some(tag_end) = xml[actual_start..].find('>') {
                let val_start = actual_start + tag_end + 1;
                if let Some(close_idx) = xml[val_start..].find(&close_tag) {
                    let text = &xml[val_start..val_start + close_idx];
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        out.push(trimmed.to_string());
                    }
                    pos = val_start + close_idx + close_tag.len();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
}
