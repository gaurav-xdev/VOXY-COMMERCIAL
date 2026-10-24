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
                let code = trimmed.find('[').and_then(|start| {
                    trimmed
                        .find(']')
                        .map(|end| trimmed[start + 1..end].to_string())
                });

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
                let code = trimmed.find('[').and_then(|start| {
                    trimmed
                        .find(']')
                        .map(|end| trimmed[start + 1..end].to_string())
                });

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
            } else if let Some(loc_str) = trimmed.strip_prefix("--> ") {
                // Associates file location with the most recent item
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
                        let line = loc_parts
                            .first()
                            .and_then(|s| s.trim().parse::<usize>().ok());
                        let col = loc_parts
                            .get(1)
                            .and_then(|s| s.trim().parse::<usize>().ok());

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
            } else if trimmed.ends_with("Error:")
                || (trimmed.contains("Error: ") && !trimmed.starts_with("-->"))
            {
                // Python exception line e.g. "ValueError: invalid literal"
                if let Some(last) = report.items.last_mut() {
                    if last.code.as_deref() == Some("PythonTraceback")
                        && last.message == "Exception traceback frame"
                    {
                        last.message = trimmed.to_string();
                    }
                }
            }
            // 4. C & C++ (GCC / Clang):
            // e.g.: src/engine.cpp:25:10: error: 'vector' was not declared in this scope
            //       src/util.c:42:5: warning: implicit declaration [-Wimplicit-function-declaration]
            else if (trimmed.contains(": error: ") || trimmed.contains(": warning: "))
                && !trimmed.starts_with("-->")
            {
                let is_error = trimmed.contains(": error: ");
                let marker = if is_error { ": error: " } else { ": warning: " };
                if let Some(pos) = trimmed.find(marker) {
                    let loc_str = &trimmed[..pos];
                    let msg = trimmed[pos + marker.len()..].trim();
                    let parts: Vec<&str> = loc_str.split(':').collect();
                    if parts.len() >= 3 {
                        let file = parts[0].trim();
                        let line = parts[1].trim().parse::<usize>().ok();
                        let col = parts[2].trim().parse::<usize>().ok();

                        report.add_item(DiagnosticItem {
                            severity: if is_error {
                                DiagnosticSeverity::Error
                            } else {
                                DiagnosticSeverity::Warning
                            },
                            code: None,
                            message: msg.to_string(),
                            file_path: Some(PathBuf::from(file)),
                            line,
                            column: col,
                            context_snippet: None,
                        });
                    }
                }
            }
            // 5. C & C++ (MSVC) and C# (.NET):
            // e.g.: Program.cs(12,18): error CS0103: The name 'myVar' does not exist in the current context
            //       engine.cpp(25,10): error C2065: 'vector': undeclared identifier
            else if (trimmed.contains("): error C") || trimmed.contains("): warning C")
                || trimmed.contains("): error CS") || trimmed.contains("): warning CS"))
                && trimmed.contains('(')
            {
                let is_error = trimmed.contains("): error ");
                if let Some(open_p) = trimmed.find('(') {
                    if let Some(close_p) = trimmed.find(')') {
                        let file = trimmed[..open_p].trim();
                        let loc = &trimmed[open_p + 1..close_p];
                        let parts: Vec<&str> = loc.split(',').collect();
                        let line = parts.first().and_then(|s| s.trim().parse::<usize>().ok());
                        let col = parts.get(1).and_then(|s| s.trim().parse::<usize>().ok());

                        let remainder = trimmed[close_p + 1..].trim_start_matches(':').trim();
                        let (code, msg) = if let Some(colon) = remainder.find(':') {
                            let cd = remainder[..colon].trim().split_whitespace().last().unwrap_or("").to_string();
                            let m = remainder[colon + 1..].trim().to_string();
                            (if cd.is_empty() { None } else { Some(cd) }, m)
                        } else {
                            (None, remainder.to_string())
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
            // 6. Java (Maven / Gradle / Javac):
            // e.g.: [ERROR] /src/main/java/App.java:[42,15] cannot find symbol
            //       App.java:42: error: cannot find symbol
            else if trimmed.starts_with("[ERROR] ") && trimmed.contains(":[") && trimmed.contains(']') {
                let rest = &trimmed[8..];
                if let Some(bracket_open) = rest.find(":[") {
                    if let Some(bracket_close) = rest.find(']') {
                        let file = rest[..bracket_open].trim();
                        let loc = &rest[bracket_open + 2..bracket_close];
                        let parts: Vec<&str> = loc.split(',').collect();
                        let line = parts.first().and_then(|s| s.trim().parse::<usize>().ok());
                        let col = parts.get(1).and_then(|s| s.trim().parse::<usize>().ok());
                        let msg = rest[bracket_close + 1..].trim().to_string();

                        report.add_item(DiagnosticItem {
                            severity: DiagnosticSeverity::Error,
                            code: Some("JavaError".to_string()),
                            message: msg,
                            file_path: Some(PathBuf::from(file)),
                            line,
                            column: col,
                            context_snippet: None,
                        });
                    }
                }
            }
            // 7. Go (go build / go vet / go test):
            // e.g.: main.go:15:2: undefined: fmt.Printlnn
            //       --- FAIL: TestProcessData (0.02s)
            else if trimmed.ends_with(".go:") || (trimmed.contains(".go:") && trimmed.matches(':').count() >= 3) {
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 4 {
                    let file = parts[0].trim();
                    let line = parts[1].trim().parse::<usize>().ok();
                    let col = parts[2].trim().parse::<usize>().ok();
                    let msg = parts[3..].join(":").trim().to_string();

                    report.add_item(DiagnosticItem {
                        severity: DiagnosticSeverity::Error,
                        code: Some("GoCompileError".to_string()),
                        message: msg,
                        file_path: Some(PathBuf::from(file)),
                        line,
                        column: col,
                        context_snippet: None,
                    });
                }
            } else if trimmed.starts_with("--- FAIL: ") {
                let test_name = trimmed.strip_prefix("--- FAIL: ").unwrap_or("").split_whitespace().next().unwrap_or("");
                report.add_item(DiagnosticItem {
                    severity: DiagnosticSeverity::Error,
                    code: Some("GoTestFail".to_string()),
                    message: format!("Go test failed: {}", test_name),
                    file_path: None,
                    line: None,
                    column: None,
                    context_snippet: Some(trimmed.to_string()),
                });
            }
            // 8. SQL (psql / SQLite / sqlfluff):
            // e.g.: psql:schema.sql:24: ERROR: syntax error at or near "SELCT"
            //       Error: near line 15: table users already exists
            //       L:  12 | P:   5 | L010 | Inconsistent capitalisation of keywords.
            else if trimmed.starts_with("psql:") && trimmed.contains(": ERROR: ") {
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() >= 4 {
                    let file = parts[1].trim();
                    let line = parts[2].trim().parse::<usize>().ok();
                    let msg = parts[3..].join(":").trim().to_string();

                    report.add_item(DiagnosticItem {
                        severity: DiagnosticSeverity::Error,
                        code: Some("PostgreSQLError".to_string()),
                        message: msg,
                        file_path: Some(PathBuf::from(file)),
                        line,
                        column: None,
                        context_snippet: None,
                    });
                }
            } else if trimmed.starts_with("Error: near line ") {
                let after = trimmed.strip_prefix("Error: near line ").unwrap_or("");
                let line = after.split(':').next().and_then(|s| s.trim().parse::<usize>().ok());
                let msg = after.split(':').nth(1).unwrap_or("").trim().to_string();

                report.add_item(DiagnosticItem {
                    severity: DiagnosticSeverity::Error,
                    code: Some("SQLiteError".to_string()),
                    message: msg,
                    file_path: None,
                    line,
                    column: None,
                    context_snippet: None,
                });
            } else if trimmed.starts_with("L:") && trimmed.contains(" | P:") && trimmed.contains(" | L") {
                // sqlfluff linter output format: L:  12 | P:   5 | L010 | Inconsistent capitalisation
                let parts: Vec<&str> = trimmed.split('|').collect();
                if parts.len() >= 4 {
                    let line = parts[0].replace("L:", "").trim().parse::<usize>().ok();
                    let col = parts[1].replace("P:", "").trim().parse::<usize>().ok();
                    let code = parts[2].trim().to_string();
                    let msg = parts[3].trim().to_string();

                    report.add_item(DiagnosticItem {
                        severity: DiagnosticSeverity::Warning,
                        code: Some(code),
                        message: msg,
                        file_path: None,
                        line,
                        column: col,
                        context_snippet: None,
                    });
                }
            }
            // 9. Rust test panics:
            // e.g.: thread 'tests::test_add' panicked at src/lib.rs:11:9:
            // assertion `left == right` failed
            else if trimmed.contains("panicked at ") {
                let mut file_path = None;
                let mut line = None;
                let mut col = None;
                if let Some(idx) = trimmed.find("panicked at ") {
                    let loc_str = trimmed[idx + 12..].trim_end_matches(':');
                    let parts: Vec<&str> = loc_str.split(':').collect();
                    if parts.len() >= 2 {
                        file_path = Some(PathBuf::from(parts[0]));
                        line = parts[1].parse::<usize>().ok();
                        col = parts.get(2).and_then(|s| s.parse::<usize>().ok());
                    }
                }

                report.add_item(DiagnosticItem {
                    severity: DiagnosticSeverity::Error,
                    code: Some("Panic".to_string()),
                    message: trimmed.to_string(),
                    file_path,
                    line,
                    column: col,
                    context_snippet: None,
                });
            } else if trimmed.starts_with("assertion `") || trimmed.starts_with("assertion failed:")
            {
                if let Some(last) = report.items.last_mut() {
                    if last.code.as_deref() == Some("Panic") {
                        last.context_snippet = Some(trimmed.to_string());
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
        assert_eq!(
            report.items[0].file_path,
            Some(PathBuf::from("src/components/Header.tsx"))
        );
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
        assert_eq!(
            report.items[0].file_path,
            Some(PathBuf::from("test_runner.py"))
        );
        assert_eq!(report.items[0].line, Some(88));
        assert!(report.items[0].message.contains("AssertionError"));
    }

    #[test]
    fn test_c_cpp_gcc_clang_parsing() {
        let output = r#"src/engine.cpp:25:10: error: 'vector' was not declared in this scope"#;
        let report = DiagnosticParser::parse(output);
        assert_eq!(report.total_errors, 1);
        assert_eq!(report.items[0].file_path, Some(PathBuf::from("src/engine.cpp")));
        assert_eq!(report.items[0].line, Some(25));
        assert_eq!(report.items[0].column, Some(10));
        assert!(report.items[0].message.contains("not declared"));
    }

    #[test]
    fn test_csharp_msvc_parsing() {
        let output = r#"Program.cs(12,18): error CS0103: The name 'myVar' does not exist in the current context"#;
        let report = DiagnosticParser::parse(output);
        assert_eq!(report.total_errors, 1);
        assert_eq!(report.items[0].code.as_deref(), Some("CS0103"));
        assert_eq!(report.items[0].file_path, Some(PathBuf::from("Program.cs")));
        assert_eq!(report.items[0].line, Some(12));
        assert_eq!(report.items[0].column, Some(18));
    }

    #[test]
    fn test_java_maven_parsing() {
        let output = r#"[ERROR] /src/main/java/App.java:[42,15] cannot find symbol"#;
        let report = DiagnosticParser::parse(output);
        assert_eq!(report.total_errors, 1);
        assert_eq!(report.items[0].file_path, Some(PathBuf::from("/src/main/java/App.java")));
        assert_eq!(report.items[0].line, Some(42));
        assert_eq!(report.items[0].column, Some(15));
        assert_eq!(report.items[0].message, "cannot find symbol");
    }

    #[test]
    fn test_go_compile_and_test_parsing() {
        let output = "main.go:15:2: undefined: fmt.Printlnn\n--- FAIL: TestProcessData (0.02s)";
        let report = DiagnosticParser::parse(output);
        assert_eq!(report.total_errors, 2);
        assert_eq!(report.items[0].file_path, Some(PathBuf::from("main.go")));
        assert_eq!(report.items[0].line, Some(15));
        assert_eq!(report.items[0].column, Some(2));
        assert_eq!(report.items[1].code.as_deref(), Some("GoTestFail"));
    }

    #[test]
    fn test_sql_diagnostic_parsing() {
        let psql_out = r#"psql:schema.sql:24: ERROR: syntax error at or near "SELCT""#;
        let psql_report = DiagnosticParser::parse(psql_out);
        assert_eq!(psql_report.total_errors, 1);
        assert_eq!(psql_report.items[0].file_path, Some(PathBuf::from("schema.sql")));
        assert_eq!(psql_report.items[0].line, Some(24));

        let sqlfluff_out = "L:  12 | P:   5 | L010 | Inconsistent capitalisation of keywords.";
        let fluff_report = DiagnosticParser::parse(sqlfluff_out);
        assert_eq!(fluff_report.total_warnings, 1);
        assert_eq!(fluff_report.items[0].line, Some(12));
        assert_eq!(fluff_report.items[0].column, Some(5));
        assert_eq!(fluff_report.items[0].code.as_deref(), Some("L010"));
    }
}
