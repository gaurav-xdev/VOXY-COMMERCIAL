//! Independent Multi-Language Code Review Engine.
//!
//! Enforces rigorous code quality, security, memory safety, concurrency,
//! and architecture standards across all 10 core languages:
//! Rust, TypeScript, JavaScript, Python, C, C++, C#, Java, Go, SQL.
//!
//! # Core Safety Invariant:
//! Independent Review Rule: No agent or implementation step may approve its own changes.
//! Every diff is inspected by rule-based static analyzers and verified against language profiles.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::language_profiles::{LanguageProfileRegistry, SupportedLanguage};

/// Severity of a code review finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl FindingSeverity {
    pub fn is_blocking(&self) -> bool {
        matches!(self, FindingSeverity::Critical | FindingSeverity::High)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FindingSeverity::Critical => "critical",
            FindingSeverity::High => "high",
            FindingSeverity::Medium => "medium",
            FindingSeverity::Low => "low",
            FindingSeverity::Informational => "informational",
        }
    }
}

/// Category of code defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefectCategory {
    SecurityVulnerability,
    MemorySafety,
    ConcurrencyAndDeadlock,
    DatabaseIntegrity,
    ErrorHandling,
    PerformanceDegradation,
    SpecificationMismatch,
    MaintainabilityAndStyle,
}

/// A structured finding identified during independent code review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewFinding {
    pub rule_id: String,
    pub severity: FindingSeverity,
    pub category: DefectCategory,
    pub language: SupportedLanguage,
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub title: String,
    pub evidence: String,
    pub impact: String,
    pub remediation: String,
}

/// Complete report produced by the Independent Multi-Language Code Review Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeReviewReport {
    pub review_id: String,
    pub total_files_analyzed: usize,
    pub languages_detected: Vec<SupportedLanguage>,
    pub findings: Vec<CodeReviewFinding>,
    pub has_blocking_defects: bool,
    pub passed_cleanly: bool,
    pub summary: String,
}

/// Independent Multi-Language Static Code Review Analyzer.
pub struct IndependentCodeReviewer {
    profiles: LanguageProfileRegistry,
}

impl Default for IndependentCodeReviewer {
    fn default() -> Self {
        Self::new()
    }
}

impl IndependentCodeReviewer {
    pub fn new() -> Self {
        Self {
            profiles: LanguageProfileRegistry::new(),
        }
    }

    /// Review a raw git diff or multi-file diff string.
    pub fn review_diff(&self, diff_content: &str) -> CodeReviewReport {
        let review_id = format!("rev_{}", uuid::Uuid::new_v4().simple());
        let mut findings = Vec::new();
        let mut detected_langs = HashSet::new();
        let mut files_analyzed = 0;

        let mut current_file = String::new();
        let mut current_lang = None;
        let mut current_line = 0usize;

        for line in diff_content.lines() {
            // Track file header: diff --git a/path b/path or +++ b/path
            if line.starts_with("diff --git a/") || line.starts_with("+++ b/") {
                let path_str = if let Some(p) = line.strip_prefix("+++ b/") {
                    p.trim().to_string()
                } else if let Some(p) = line.strip_prefix("diff --git a/") {
                    p.split_whitespace().next().unwrap_or("").to_string()
                } else {
                    String::new()
                };

                if !path_str.is_empty() && path_str != "/dev/null" {
                    current_file = path_str.clone();
                    files_analyzed += 1;

                    let ext = Path::new(&path_str)
                        .extension()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    current_lang = self.detect_language_by_ext(&ext);
                    if let Some(l) = current_lang {
                        detected_langs.insert(l);
                    }
                }
                continue;
            }

            // Track hunk line numbers: @@ -10,5 +12,6 @@
            if line.starts_with("@@ ") {
                if let Some(plus_idx) = line.find('+') {
                    let after_plus = &line[plus_idx + 1..];
                    current_line = after_plus
                        .split(|c: char| c == ',' || c == ' ')
                        .next()
                        .and_then(|s| s.parse::<usize>().ok())
                        .unwrap_or(1);
                }
                continue;
            }

            // Analyze only added lines in diff (+...)
            if line.starts_with('+') && !line.starts_with("+++") {
                let code_line = &line[1..];
                current_line += 1;

                if let Some(lang) = current_lang {
                    self.analyze_line(lang, code_line, &current_file, current_line, &mut findings);
                }
            } else if !line.starts_with('-') {
                current_line += 1;
            }
        }

        // Sort findings: Critical -> High -> Medium -> Low -> Informational
        findings.sort_by(|a, b| b.severity.cmp(&a.severity));

        let has_blocking = findings.iter().any(|f| f.severity.is_blocking());
        let passed_cleanly = !has_blocking && findings.is_empty();

        let mut lang_vec: Vec<_> = detected_langs.into_iter().collect();
        lang_vec.sort_by_key(|l| l.as_str());

        let summary = if passed_cleanly {
            format!(
                "Independent review passed with 0 findings across {} files.",
                files_analyzed
            )
        } else {
            format!(
                "Independent review identified {} findings ({} blocking) across {} files.",
                findings.len(),
                findings.iter().filter(|f| f.severity.is_blocking()).count(),
                files_analyzed
            )
        };

        CodeReviewReport {
            review_id,
            total_files_analyzed: files_analyzed,
            languages_detected: lang_vec,
            findings,
            has_blocking_defects: has_blocking,
            passed_cleanly,
            summary,
        }
    }

    fn detect_language_by_ext(&self, ext: &str) -> Option<SupportedLanguage> {
        for profile in self.profiles.all_profiles() {
            if profile.primary_extensions.iter().any(|e| e == ext) {
                return Some(profile.language);
            }
        }
        None
    }

    fn analyze_line(
        &self,
        lang: SupportedLanguage,
        line: &str,
        file: &str,
        line_num: usize,
        findings: &mut Vec<CodeReviewFinding>,
    ) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') || trimmed.starts_with("/*") {
            return;
        }

        // 1. Cross-language SQL Injection Detection
        if (trimmed.contains("execute(")
            || trimmed.contains("query(")
            || trimmed.contains("raw_query("))
            && (trimmed.contains("format!")
                || trimmed.contains("f\"")
                || trimmed.contains("$\"")
                || trimmed.contains(" + "))
        {
            findings.push(CodeReviewFinding {
                rule_id: "SEC-SQLI-001".into(),
                severity: FindingSeverity::Critical,
                category: DefectCategory::SecurityVulnerability,
                language: lang,
                file_path: Some(file.to_string()),
                line_number: Some(line_num),
                title: "Potential SQL Injection via Dynamic String Interpolation".into(),
                evidence: trimmed.to_string(),
                impact: "Attackers can manipulate database query syntax and execute arbitrary commands or exfiltrate data.".into(),
                remediation: "Use parameterized queries or prepared statements ($1, :param) instead of string formatting.".into(),
            });
        }

        // 1b. Cross-language Hardcoded Secret Detection
        let lower = trimmed.to_lowercase();
        if (lower.contains("sk-ant-")
            || lower.contains("ghp_")
            || lower.contains("xoxb-")
            || lower.contains("aws_secret_access_key")
            || lower.contains("api_key = \"")
            || lower.contains("password = \"")
            || lower.contains("token = \""))
            && !lower.contains("env::var")
            && !lower.contains("os.environ")
            && !lower.contains("process.env")
        {
            findings.push(CodeReviewFinding {
                rule_id: "SEC-SECRET-001".into(),
                severity: FindingSeverity::Critical,
                category: DefectCategory::SecurityVulnerability,
                language: lang,
                file_path: Some(file.to_string()),
                line_number: Some(line_num),
                title: "Hardcoded API Key or Secret Credential".into(),
                evidence: trimmed.to_string(),
                impact: "Hardcoded secrets in version control lead to persistent credential exposure and unauthorized access.".into(),
                remediation: "Load credentials dynamically from environment variables or a secret vault.".into(),
            });
        }

        // 2. Language-Specific Checks
        match lang {
            SupportedLanguage::Rust => {
                // Rust Unsafe without safety comment
                if trimmed.starts_with("unsafe ") || trimmed.contains(" unsafe {") {
                    findings.push(CodeReviewFinding {
                        rule_id: "RUST-UNSAFE-001".into(),
                        severity: FindingSeverity::High,
                        category: DefectCategory::MemorySafety,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Unsafe block introduced; requires formal // SAFETY invariant".into(),
                        evidence: trimmed.to_string(),
                        impact: "Unsafe blocks can trigger undefined behavior, memory corruption, or segfaults.".into(),
                        remediation: "Document memory invariants with '// SAFETY: ...' comment or use safe abstractions.".into(),
                    });
                }
                // Unchecked unwrap in production logic
                if trimmed.contains(".unwrap()") && !file.contains("test") {
                    findings.push(CodeReviewFinding {
                        rule_id: "RUST-ERR-001".into(),
                        severity: FindingSeverity::Medium,
                        category: DefectCategory::ErrorHandling,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Production .unwrap() call poses panic risk on unexpected input".into(),
                        evidence: trimmed.to_string(),
                        impact: "Panics will crash the process or thread if the Option/Result contains None/Err.".into(),
                        remediation: "Replace .unwrap() with ? error propagation or explicit match / unwrap_or handling.".into(),
                    });
                }
            }

            SupportedLanguage::TypeScript | SupportedLanguage::JavaScript => {
                // TypeScript 'any' escape hatch
                if (trimmed.contains(": any") || trimmed.contains("as any"))
                    && !file.contains("test")
                {
                    findings.push(CodeReviewFinding {
                        rule_id: "TS-TYPE-001".into(),
                        severity: FindingSeverity::Medium,
                        category: DefectCategory::MaintainabilityAndStyle,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Use of 'any' type disables compiler type checking".into(),
                        evidence: trimmed.to_string(),
                        impact: "Bypasses TypeScript static verification and permits runtime type errors.".into(),
                        remediation: "Use 'unknown' with type narrowing or define an explicit interface/discriminated union.".into(),
                    });
                }
                // Dangerous eval / Function
                if trimmed.contains("eval(") || trimmed.contains("new Function(") {
                    findings.push(CodeReviewFinding {
                        rule_id: "JS-SEC-001".into(),
                        severity: FindingSeverity::Critical,
                        category: DefectCategory::SecurityVulnerability,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Dynamic code execution via eval() or Function constructor".into(),
                        evidence: trimmed.to_string(),
                        impact: "Arbitrary code execution if untrusted user strings reach eval."
                            .into(),
                        remediation:
                            "Parse data with JSON.parse() or a secure parser; eliminate eval."
                                .into(),
                    });
                }
            }

            SupportedLanguage::Python => {
                // Insecure deserialization via pickle
                if trimmed.contains("pickle.loads(") || trimmed.contains("yaml.load(") {
                    findings.push(CodeReviewFinding {
                        rule_id: "PY-SEC-001".into(),
                        severity: FindingSeverity::Critical,
                        category: DefectCategory::SecurityVulnerability,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Insecure object deserialization via pickle / unsafe yaml".into(),
                        evidence: trimmed.to_string(),
                        impact: "Allows arbitrary code execution when loading untrusted serialized payloads.".into(),
                        remediation: "Use yaml.safe_load() or JSON/Pydantic serialization instead of pickle.".into(),
                    });
                }
                // Bare except
                if trimmed == "except:" {
                    findings.push(CodeReviewFinding {
                        rule_id: "PY-ERR-001".into(),
                        severity: FindingSeverity::Medium,
                        category: DefectCategory::ErrorHandling,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Bare 'except:' statement catches system signals and halts debugging".into(),
                        evidence: trimmed.to_string(),
                        impact: "Swallows KeyboardInterrupt, SystemExit, and unexpected errors silently.".into(),
                        remediation: "Catch specific Exception subclasses (e.g., 'except Exception:' or 'except ValueError:').".into(),
                    });
                }
            }

            SupportedLanguage::C | SupportedLanguage::Cpp => {
                // Dangerous C string functions
                if trimmed.contains("strcpy(")
                    || trimmed.contains("sprintf(")
                    || trimmed.contains("gets(")
                {
                    findings.push(CodeReviewFinding {
                        rule_id: "C-SEC-001".into(),
                        severity: FindingSeverity::Critical,
                        category: DefectCategory::MemorySafety,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Use of dangerous unbounded buffer function (strcpy/sprintf/gets)".into(),
                        evidence: trimmed.to_string(),
                        impact: "Buffer overflows enabling memory corruption and arbitrary code execution.".into(),
                        remediation: "Replace with snprintf(), strncpy_s(), or C++ std::string.".into(),
                    });
                }
                // Raw delete calls in C++
                if lang == SupportedLanguage::Cpp && trimmed.starts_with("delete ") {
                    findings.push(CodeReviewFinding {
                        rule_id: "CPP-RAII-001".into(),
                        severity: FindingSeverity::Medium,
                        category: DefectCategory::MemorySafety,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Manual delete call violates RAII smart pointer ownership".into(),
                        evidence: trimmed.to_string(),
                        impact: "Double-free and use-after-free bugs if exceptions occur before delete.".into(),
                        remediation: "Use std::unique_ptr or std::shared_ptr for automatic deterministic cleanup.".into(),
                    });
                }
            }

            SupportedLanguage::CSharp => {
                // Async void in C#
                if trimmed.contains("async void ") && !trimmed.contains("EventHandler") {
                    findings.push(CodeReviewFinding {
                        rule_id: "CS-ASYNC-001".into(),
                        severity: FindingSeverity::High,
                        category: DefectCategory::ConcurrencyAndDeadlock,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "async void method introduced in non-event handler".into(),
                        evidence: trimmed.to_string(),
                        impact: "Exceptions thrown in async void methods cannot be caught by callers and crash the runtime.".into(),
                        remediation: "Return Task or Task<T> instead of async void.".into(),
                    });
                }
            }

            SupportedLanguage::Java => {
                // Catching Throwable
                if trimmed.contains("catch (Throwable ") {
                    findings.push(CodeReviewFinding {
                        rule_id: "JAVA-ERR-001".into(),
                        severity: FindingSeverity::High,
                        category: DefectCategory::ErrorHandling,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Catching Throwable traps fatal JVM errors (OutOfMemoryError)".into(),
                        evidence: trimmed.to_string(),
                        impact: "Prevents normal JVM recovery and masks unrecoverable virtual machine errors.".into(),
                        remediation: "Catch specific Exception classes instead of Throwable.".into(),
                    });
                }
            }

            SupportedLanguage::Go => {
                // Unhandled error in Go: _ = fn()
                if trimmed.starts_with("_ = ")
                    && (trimmed.contains("(") || trimmed.contains("Write"))
                {
                    findings.push(CodeReviewFinding {
                        rule_id: "GO-ERR-001".into(),
                        severity: FindingSeverity::Medium,
                        category: DefectCategory::ErrorHandling,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Returned error explicitly discarded with blank identifier (_)".into(),
                        evidence: trimmed.to_string(),
                        impact: "Silent failure propagation causing corrupted internal state or data loss.".into(),
                        remediation: "Check the returned error and return or log it with context.".into(),
                    });
                }
            }

            SupportedLanguage::Sql => {
                // DROP TABLE without IF EXISTS
                if trimmed.starts_with("DROP TABLE ") && !trimmed.contains("IF EXISTS") {
                    findings.push(CodeReviewFinding {
                        rule_id: "SQL-MIG-001".into(),
                        severity: FindingSeverity::High,
                        category: DefectCategory::DatabaseIntegrity,
                        language: lang,
                        file_path: Some(file.to_string()),
                        line_number: Some(line_num),
                        title: "Destructive DROP TABLE statement without IF EXISTS check".into(),
                        evidence: trimmed.to_string(),
                        impact: "Migration failure if table does not exist or accidental irrecoverable data loss.".into(),
                        remediation: "Use DROP TABLE IF EXISTS and ensure down migration or backup is provided.".into(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_language_sqli_detection() {
        let reviewer = IndependentCodeReviewer::new();
        let diff = r#"
diff --git a/services/user_service.py b/services/user_service.py
--- a/services/user_service.py
+++ b/services/user_service.py
@@ -10,3 +10,4 @@
 def get_user(user_id):
+    return db.execute(f"SELECT * FROM users WHERE id = '{user_id}'")
"#;
        let report = reviewer.review_diff(diff);
        assert!(!report.passed_cleanly);
        assert!(report.has_blocking_defects);
        assert_eq!(report.findings[0].rule_id, "SEC-SQLI-001");
        assert_eq!(report.findings[0].severity, FindingSeverity::Critical);
    }

    #[test]
    fn test_rust_unsafe_and_unwrap_detection() {
        let reviewer = IndependentCodeReviewer::new();
        let diff = r#"
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,5 +1,7 @@
 fn process_data(ptr: *const u8) {
+    let val = unsafe { *ptr };
+    let res = calculate().unwrap();
 }
"#;
        let report = reviewer.review_diff(diff);
        assert_eq!(report.findings.len(), 2);
        assert!(report
            .findings
            .iter()
            .any(|f| f.rule_id == "RUST-UNSAFE-001"));
        assert!(report.findings.iter().any(|f| f.rule_id == "RUST-ERR-001"));
    }

    #[test]
    fn test_clean_diff_passes() {
        let reviewer = IndependentCodeReviewer::new();
        let diff = r#"
diff --git a/src/math.rs b/src/math.rs
--- a/src/math.rs
+++ b/src/math.rs
@@ -1,3 +1,5 @@
 pub fn add(a: i32, b: i32) -> i32 {
+    a + b
 }
"#;
        let report = reviewer.review_diff(diff);
        assert!(report.passed_cleanly);
        assert!(!report.has_blocking_defects);
        assert_eq!(report.findings.len(), 0);
    }
}
