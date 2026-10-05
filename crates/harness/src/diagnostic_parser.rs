//! Compiler & test diagnostic parser for automated build feedback.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
    Help,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticItem {
    pub severity: DiagnosticSeverity,
    pub code: Option<String>,
    pub message: String,
    pub file_path: Option<PathBuf>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub context_snippet: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub success: bool,
    pub total_errors: usize,
    pub total_warnings: usize,
    pub items: Vec<DiagnosticItem>,
}

impl DiagnosticReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_item(&mut self, item: DiagnosticItem) {
        match item.severity {
            DiagnosticSeverity::Error => self.total_errors += 1,
            DiagnosticSeverity::Warning => self.total_warnings += 1,
            _ => {}
        }
        self.items.push(item);
        self.success = self.total_errors == 0;
    }
}

pub struct DiagnosticParser;

impl DiagnosticParser {
    /// Automatically detects language/tool and parses stdout/stderr into structured diagnostics.
    pub fn parse(output: &str) -> DiagnosticReport {
        let mut report = DiagnosticReport::new();

        for line in output.lines() {
            let trimmed = line.trim();

            // 1. Rustc / Cargo errors:
            // e.g.: error[E0308]: mismatched types
            //  --> src/main.rs:12:5
            if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
                let code = if let Some(start) = trimmed.find('[') {
                    if let Some(end) = trimmed.find(']') {
                        Some(trimmed[start + 1..end].to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let msg = if let Some(colon_pos) = trimmed.find(':') {
                    trimmed[colon_pos + 1..].trim().to_string()
                } else {
                    trimmed.to_string()
                };

                report.add_item(DiagnosticItem {
                    severity: DiagnosticSeverity::Error,
                    code,
                    message: msg,
                    file_path: None,
                    line: None,
                    column: None,
                    context_snippet: None,
                });
            } else if trimmed.starts_with("warning[") || trimmed.starts_with("warning:") {
                let code = if let Some(start) = trimmed.find('[') {
                    if let Some(end) = trimmed.find(']') {
                        Some(trimmed[start + 1..end].to_string())
                    } else {
                        None
                    }
                } else {
                    None
                };

                let msg = if let Some(colon_pos) = trimmed.find(':') {
                    trimmed[colon_pos + 1..].trim().to_string()
                } else {
                    trimmed.to_string()
                };

                report.add_item(DiagnosticItem {
                    severity: DiagnosticSeverity::Warning,
                    code,
                    message: msg,
                    file_path: None,
                    line: None,
                    column: None,
                    context_snippet: None,
                });
            } else if trimmed.starts_with("--> ") {
                // Associates file location with the most recent item
                let loc_str = &trimmed[4..];
                let parts: Vec<&str> = loc_str.split(':').collect();
                if parts.len() >= 2 {
                    let file_path = PathBuf::from(parts[0]);
                    let line = parts[1].parse::<usize>().ok();
                    let col = parts.get(2).and_then(|s| s.parse::<usize>().ok());

                    if let Some(last) = report.items.last_mut() {
                        last.file_path = Some(file_path);
                        last.line = line;
                        last.column = col;
                    }
                }
            }
            // 2. TypeScript (tsc):
            // e.g.: src/index.ts(14,5): error TS2322: Type 'string' is not assignable to type 'number'.
            else if trimmed.contains("): error TS") || trimmed.contains("): warning TS") {
                let is_error = trimmed.contains("): error TS");
                if let Some(paren_open) = trimmed.find('(') {
                    if let Some(paren_close) = trimmed.find(')') {
                        let file = trimmed[..paren_open].trim();
                        let loc_str = &trimmed[paren_open + 1..paren_close];
                        let loc_parts: Vec<&str> = loc_str.split(',').collect();
                        let line = loc_parts.first().and_then(|s| s.trim().parse::<usize>().ok());
                        let col = loc_parts.get(1).and_then(|s| s.trim().parse::<usize>().ok());

                        let after_paren = &trimmed[paren_close + 1..];
                        let code = if let Some(ts_idx) = after_paren.find("TS") {
                            let code_str = &after_paren[ts_idx..];
                            code_str.split(':').next().map(|s| s.trim().to_string())
                        } else {
                            None
                        };

                        let msg = if let Some(colon) = after_paren.find(':') {
                            after_paren[colon + 1..].trim().to_string()
                        } else {
                            after_paren.trim().to_string()
                        };

                        report.add_item(DiagnosticItem {
                            severity: if is_error {
                                DiagnosticSeverity::Error
                            } else {
                                DiagnosticSeverity::Warning
                            },
                            code,
                            message: msg,
                            file_path: Some(PathBuf::from(file)),
                            line,
                            column: col,
                            context_snippet: None,
                        });
                    }
                }
            }
            // 3. Python / Pytest tracebacks:
            // e.g.: File "test_app.py", line 42, in test_something
            //       AssertionError: assert 1 == 2
            else if trimmed.starts_with("File \"") && trimmed.contains("\", line ") {
                let after_file = &trimmed[6..];
                if let Some(quote_idx) = after_file.find('\"') {
                    let file = &after_file[..quote_idx];
                    let rest = &after_file[quote_idx + 1..];
                    if let Some(line_idx) = rest.find("line ") {
                        let line_str = &rest[line_idx + 5..];
                        let line_num = line_str
                            .split(|c: char| !c.is_numeric())
                            .next()
                            .and_then(|s| s.parse::<usize>().ok());

                        report.add_item(DiagnosticItem {
                            severity: DiagnosticSeverity::Error,
                            code: Some("PythonTraceback".to_string()),
                            message: "Exception traceback frame".to_string(),
                            file_path: Some(PathBuf::from(file)),
                            line: line_num,
                            column: None,
                            context_snippet: None,
                        });
                    }
                }
            } else if trimmed.ends_with("Error:") || (trimmed.contains("Error: ") && !trimmed.starts_with("-->")) {
                // Python exception line e.g. "ValueError: invalid literal"
                if let Some(last) = report.items.last_mut() {
                    if last.code.as_deref() == Some("PythonTraceback") && last.message == "Exception traceback frame" {
                        last.message = trimmed.to_string();
                    }
                }
            }
        }

        report.success = report.total_errors == 0;
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rustc_error_parsing() {
        let output = r#"
error[E0308]: mismatched types
  --> src/main.rs:15:9
   |
15 |     let x: u32 = "hello";
   |                  ^^^^^^^ expected `u32`, found `&str`
warning: unused variable: `y`
  --> src/main.rs:20:9
"#;
        let report = DiagnosticParser::parse(output);
        assert_eq!(report.total_errors, 1);
        assert_eq!(report.total_warnings, 1);
        assert!(!report.success);

        let err = &report.items[0];
        assert_eq!(err.code.as_deref(), Some("E0308"));
        assert_eq!(err.file_path, Some(PathBuf::from("src/main.rs")));
        assert_eq!(err.line, Some(15));
        assert_eq!(err.column, Some(9));
    }

    #[test]
    fn test_tsc_error_parsing() {
        let output = r#"src/components/Header.tsx(45,12): error TS2322: Type 'number' is not assignable to type 'string'."#;
        let report = DiagnosticParser::parse(output);
        assert_eq!(report.total_errors, 1);
        assert_eq!(report.items[0].code.as_deref(), Some("TS2322"));
        assert_eq!(report.items[0].file_path, Some(PathBuf::from("src/components/Header.tsx")));
        assert_eq!(report.items[0].line, Some(45));
        assert_eq!(report.items[0].column, Some(12));
    }

    #[test]
    fn test_python_traceback_parsing() {
        let output = r#"
Traceback (most recent call last):
  File "test_runner.py", line 88, in run_test
    assert result == expected
AssertionError: assert False == True
"#;
        let report = DiagnosticParser::parse(output);
        assert_eq!(report.total_errors, 1);
        assert_eq!(report.items[0].file_path, Some(PathBuf::from("test_runner.py")));
        assert_eq!(report.items[0].line, Some(88));
        assert!(report.items[0].message.contains("AssertionError"));
    }
}
