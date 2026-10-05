//! HTML and text extraction with normalization and boundary clamping.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractedDocument {
    pub title: String,
    pub domain: String,
    pub content_type: String,
    pub text_content: String,
    pub byte_size: usize,
    pub is_truncated: bool,
}

pub struct ContentExtractor;

impl ContentExtractor {
    const MAX_EXTRACTED_BYTES: usize = 128 * 1024; // 128 KB clamp

    /// Strips HTML tags, comments, script and style elements, producing clean readable text.
    pub fn extract_text(raw_html_or_text: &str, url: &str) -> ExtractedDocument {
        let domain = Self::extract_domain(url);
        let mut clean = String::with_capacity(raw_html_or_text.len().min(Self::MAX_EXTRACTED_BYTES));
        let mut in_tag = false;
        let mut in_script = false;
        let mut in_style = false;
        let mut tag_buffer = String::new();
        let mut title = String::new();
        let mut capturing_title = false;

        let lower = raw_html_or_text.to_lowercase();
        let chars: Vec<char> = raw_html_or_text.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            if c == '<' {
                in_tag = true;
                tag_buffer.clear();
                i += 1;
                continue;
            }

            if in_tag {
                if c == '>' {
                    in_tag = false;
                    let tag_lower = tag_buffer.trim().to_lowercase();

                    if tag_lower.starts_with("script") {
                        in_script = true;
                    } else if tag_lower.starts_with("/script") {
                        in_script = false;
                    } else if tag_lower.starts_with("style") {
                        in_style = true;
                    } else if tag_lower.starts_with("/style") {
                        in_style = false;
                    } else if tag_lower.starts_with("title") {
                        capturing_title = true;
                    } else if tag_lower.starts_with("/title") {
                        capturing_title = false;
                    } else if tag_lower == "p" || tag_lower == "br" || tag_lower == "div" || tag_lower == "li" {
                        clean.push('\n');
                    }
                } else {
                    tag_buffer.push(c);
                }
                i += 1;
                continue;
            }

            if !in_script && !in_style {
                if capturing_title {
                    title.push(c);
                } else {
                    clean.push(c);
                }
            }

            if clean.len() >= Self::MAX_EXTRACTED_BYTES {
                break;
            }

            i += 1;
        }

        let is_truncated = clean.len() >= Self::MAX_EXTRACTED_BYTES;

        // Normalize whitespace
        let mut normalized = String::new();
        let mut last_was_ws = false;
        for c in clean.chars() {
            if c.is_whitespace() {
                if !last_was_ws {
                    normalized.push(if c == '\n' { '\n' } else { ' ' });
                    last_was_ws = true;
                }
            } else {
                normalized.push(c);
                last_was_ws = false;
            }
        }

        ExtractedDocument {
            title: if title.trim().is_empty() { domain.clone() } else { title.trim().to_string() },
            domain,
            content_type: if lower.contains("<html") { "html".to_string() } else { "text/plain".to_string() },
            text_content: normalized.trim().to_string(),
            byte_size: raw_html_or_text.len(),
            is_truncated,
        }
    }

    fn extract_domain(url: &str) -> String {
        let trimmed = url.trim();
        if let Some(pos) = trimmed.find("://") {
            let after = &trimmed[pos + 3..];
            after.split('/').next().unwrap_or("unknown").to_string()
        } else {
            trimmed.split('/').next().unwrap_or("unknown").to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_tag_stripping_and_title_extraction() {
        let html = r#"
        <html>
            <head>
                <title>Rust 2026 Edition Guide</title>
                <style>body { background: #000; }</style>
            </head>
            <body>
                <script>console.log("analytics");</script>
                <h1>Welcome</h1>
                <p>Rust provides memory safety without a garbage collector.</p>
            </body>
        </html>
        "#;

        let doc = ContentExtractor::extract_text(html, "https://doc.rust-lang.org/edition-guide/");
        assert_eq!(doc.title, "Rust 2026 Edition Guide");
        assert_eq!(doc.domain, "doc.rust-lang.org");
        assert!(doc.text_content.contains("Rust provides memory safety"));
        assert!(!doc.text_content.contains("analytics"));
        assert!(!doc.text_content.contains("background"));
    }
}
