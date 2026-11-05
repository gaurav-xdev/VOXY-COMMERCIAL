use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub code: Option<String>,
    pub message: String,
    pub severity: DiagnosticSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugAnalysis {
    pub diagnostics: Vec<Diagnostic>,
    pub primary_cause: String,
    pub recommended_action: String,
}

pub struct Debugger;

impl Debugger {
    pub fn analyze_compiler_output(output: &str) -> DebugAnalysis {
        let mut diagnostics = Vec::new();

        // Regex for Rust errors: error[E0308]: mismatched types \n --> src/main.rs:10:5
        let rust_loc_re = Regex::new(r"-->\s+([^\s:]+):(\d+):(\d+)").unwrap();
        let rust_err_re = Regex::new(r"error(?:\[([A-Z0-9]+)\])?:\s+(.+)").unwrap();

        // Extract primary rustc location if present
        let mut rust_loc_file = None;
        let mut rust_loc_line = None;
        if let Some(cap) = rust_loc_re.captures(output) {
            rust_loc_file = cap.get(1).map(|m| m.as_str().to_string());
            rust_loc_line = cap.get(2).and_then(|m| m.as_str().parse::<usize>().ok());
        }

        for cap in rust_err_re.captures_iter(output) {
            let code = cap.get(1).map(|m| m.as_str().to_string());
            let msg = cap
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            diagnostics.push(Diagnostic {
                file_path: rust_loc_file.clone(),
                line_number: rust_loc_line,
                code,
                message: msg,
                severity: DiagnosticSeverity::Error,
            });
        }

        // Regex for standard file:line:col error patterns (TS, C, Clang, Python)
        let std_loc_re =
            Regex::new(r"(?:^|\n)([\w/\\.-]+):(\d+)(?::(\d+))?:\s*(?:error|warning)?:\s*(.+)")
                .unwrap();

        for cap in std_loc_re.captures_iter(output) {
            let file = cap.get(1).map(|m| m.as_str().to_string());
            let line = cap.get(2).and_then(|m| m.as_str().parse::<usize>().ok());
            let msg = cap
                .get(4)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            diagnostics.push(Diagnostic {
                file_path: file,
                line_number: line,
                code: None,
                message: msg,
                severity: DiagnosticSeverity::Error,
            });
        }

        let primary_cause = if let Some(first) = diagnostics.first() {
            if let Some(ref c) = first.code {
                format!("Failure [{}]: {}", c, first.message)
            } else {
                format!("Failure: {}", first.message)
            }
        } else if output.contains("panicked at") {
            "Runtime panic detected during execution".into()
        } else {
            "Process failed with non-zero exit code".into()
        };

        let recommended_action = if output.contains("cannot find") || output.contains("not found") {
            "Check imported modules, dependency declarations, and identifier spelling.".into()
        } else if output.contains("mismatched types") || output.contains("type mismatch") {
            "Adjust variable types or provide explicit type casting / conversions.".into()
        } else if output.contains("borrow of moved value") || output.contains("cannot borrow") {
            "Clone the value or restructure ownership / lifetimes.".into()
        } else {
            "Inspect the failure location and review test preconditions.".into()
        };

        DebugAnalysis {
            diagnostics,
            primary_cause,
            recommended_action,
        }
    }
}
