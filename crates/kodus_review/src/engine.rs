use crate::error::{KodusReviewError, Result};
use crate::prompts::{build_kodus_system_prompt, build_kodus_user_prompt};
use crate::provider_bridge::KodusProviderBridge;
use crate::security::{KodusSecurityBoundary, OSMOO_OPERATOR_IDENTITY};
use crate::types::{
    KodusCategory, KodusFinding, KodusReviewReport, KodusSeverity, ReviewScope, ReviewTrigger,
};
use chrono::Utc;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use uuid::Uuid;

/// Core Kodus Code Review Engine integrated with OSMOO's provider bridge and workspace tools.
pub struct KodusReviewEngine {
    repo_root: PathBuf,
    provider_bridge: KodusProviderBridge,
    security: KodusSecurityBoundary,
}

impl KodusReviewEngine {
    pub fn new(
        repo_root: impl Into<PathBuf>,
        provider_bridge: KodusProviderBridge,
        security: KodusSecurityBoundary,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            provider_bridge,
            security,
        }
    }

    pub fn security(&self) -> &KodusSecurityBoundary {
        &self.security
    }

    /// Perform an internal code review for OSMOO's autonomous coding workflow.
    pub async fn review(
        &self,
        scope: ReviewScope,
        trigger: ReviewTrigger,
        internal_token: &str,
    ) -> Result<KodusReviewReport> {
        let start = Instant::now();

        // 1. Strict security boundary check: OSMOO only
        self.security
            .verify_caller(OSMOO_OPERATOR_IDENTITY, internal_token)?;

        // 2. Change-aware diff resolution
        let diff = self.resolve_diff(&scope)?;
        if diff.trim().is_empty() {
            return Ok(KodusReviewReport::empty(
                trigger,
                "none".into(),
                "no-changes".into(),
            ));
        }

        // 3. Extract relevant affected files
        let files = extract_changed_files(&diff);

        // 4. Gather optional surrounding context
        let context = self.gather_surrounding_context(&files);

        // 5. Build prompts following official Kodus contracts
        let system_prompt = build_kodus_system_prompt();
        let user_prompt = build_kodus_user_prompt(&diff, &trigger, context.as_deref());

        // 6. Complete review prompt via OSMOO's user-configured model provider
        let (raw_response, provider_name) = self
            .provider_bridge
            .complete_review_prompt(&system_prompt, &user_prompt)
            .await?;

        // 7. Parse, deduplicate, and sort findings
        let mut findings = parse_kodus_findings(&raw_response);
        deduplicate_findings(&mut findings);
        findings.sort_by(|a, b| b.severity.cmp(&a.severity));

        let has_blocking = findings.iter().any(|f| f.severity.is_blocking());
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(KodusReviewReport {
            report_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            trigger,
            provider: provider_name,
            model: "osmoo-active-model".to_string(),
            files_reviewed: files,
            findings,
            duration_ms,
            has_blocking_defects: has_blocking,
        })
    }

    /// Resolve git diff based on review scope.
    fn resolve_diff(&self, scope: &ReviewScope) -> Result<String> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.repo_root);

        match scope {
            ReviewScope::WorkingTree => {
                cmd.args(["diff", "HEAD"]);
            }
            ReviewScope::StagedOnly => {
                cmd.args(["diff", "--staged"]);
            }
            ReviewScope::Branch(branch) => {
                cmd.args(["diff", branch]);
            }
            ReviewScope::Commit(commit) => {
                cmd.args(["diff", &format!("{commit}~1"), commit]);
            }
            ReviewScope::Files(paths) => {
                cmd.args(["diff", "HEAD", "--"]);
                for p in paths {
                    cmd.arg(p.to_string_lossy().as_ref());
                }
            }
        }

        let output = cmd
            .output()
            .map_err(|e| KodusReviewError::DiffError(format!("Failed to execute git diff: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(KodusReviewError::DiffError(format!("git diff failed: {stderr}")));
        }

        let raw = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(filter_diff_noise(&raw))
    }

    /// Read small snippets of surrounding context for touched files.
    fn gather_surrounding_context(&self, files: &[String]) -> Option<String> {
        if files.is_empty() {
            return None;
        }

        let mut out = String::new();
        for file in files.iter().take(3) {
            let path = self.repo_root.join(file);
            if path.exists() && path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let lines: Vec<&str> = content.lines().take(60).collect();
                    out.push_str(&format!("--- File: {} (first 60 lines) ---\n", file));
                    out.push_str(&lines.join("\n"));
                    out.push_str("\n\n");
                }
            }
        }

        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }
}

/// Filter noise, lockfiles, and binaries from diff.
fn filter_diff_noise(raw: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;

    for line in raw.lines() {
        if line.starts_with("diff --git") {
            let lower = line.to_lowercase();
            skipping = lower.contains("cargo.lock")
                || lower.contains("package-lock.json")
                || lower.contains(".pnp")
                || lower.contains("transcript")
                || lower.contains(".ico")
                || lower.contains(".png")
                || lower.contains(".rgba");
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Extract list of modified file paths from a unified diff.
pub fn extract_changed_files(diff: &str) -> Vec<String> {
    let mut files = HashSet::new();
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            let path = rest.trim();
            if path != "/dev/null" && !path.is_empty() {
                files.insert(path.to_string());
            }
        }
    }
    let mut res: Vec<String> = files.into_iter().collect();
    res.sort();
    res
}

/// Parse raw model response into structured `KodusFinding` objects.
pub fn parse_kodus_findings(text: &str) -> Vec<KodusFinding> {
    let mut findings = Vec::new();

    // Look for JSON block or raw JSON
    let json_text = if let Some(start) = text.find("```json") {
        let rest = &text[start + 7..];
        if let Some(end) = rest.find("```") {
            &rest[..end]
        } else {
            rest
        }
    } else if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        return findings;
    };

    let parsed: Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(_) => return findings,
    };

    if let Some(arr) = parsed.get("findings").and_then(|f| f.as_array()) {
        for item in arr {
            let file = item["file"].as_str().unwrap_or("unknown").to_string();
            let line = item["line"].as_u64().unwrap_or(1) as usize;
            let end_line = item["end_line"].as_u64().unwrap_or(line as u64) as usize;
            let title = item["title"].as_str().unwrap_or("Code issue").to_string();
            let what = item["what"].as_str().unwrap_or("").to_string();
            let why = item["why"].as_str().unwrap_or("").to_string();
            let how = item["how"].as_str().map(|s| s.to_string());
            let is_confirmed = item["is_confirmed"].as_bool().unwrap_or(true);
            let confidence = item["confidence_score"].as_f64().unwrap_or(0.85) as f32;

            let severity = match item["severity"].as_str().unwrap_or("medium").to_lowercase().as_str() {
                "critical" => KodusSeverity::Critical,
                "high" => KodusSeverity::High,
                "low" => KodusSeverity::Low,
                "info" | "informational" => KodusSeverity::Informational,
                _ => KodusSeverity::Medium,
            };

            let category = match item["category"].as_str().unwrap_or("logic_bug").to_lowercase().as_str() {
                "security_vulnerability" | "security" => KodusCategory::SecurityVulnerability,
                "rust_memory_safety" | "rust" => KodusCategory::RustMemorySafety,
                "windows_native_ipc" | "windows" | "ipc" => KodusCategory::WindowsNativeIpc,
                "python_sidecar_reliability" | "python" => KodusCategory::PythonSidecarReliability,
                "react_typescript_correctness" | "react" | "typescript" => KodusCategory::ReactTypeScriptCorrectness,
                "performance_regression" | "performance" => KodusCategory::PerformanceRegression,
                "missing_tests" | "tests" => KodusCategory::MissingTests,
                "architecture_violation" | "architecture" => KodusCategory::ArchitectureViolation,
                _ => KodusCategory::LogicBug,
            };

            let sig = format!("{}:{}:{}", file, line, category.as_str());
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            sig.hash(&mut hasher);
            let id = format!("{:016x}", hasher.finish());

            findings.push(KodusFinding {
                id,
                file,
                line,
                end_line,
                severity,
                category,
                title,
                what,
                why,
                how,
                is_confirmed,
                confidence_score: confidence,
            });
        }
    }

    findings
}

/// Deduplicate findings sharing the exact same file, line, and category.
pub fn deduplicate_findings(findings: &mut Vec<KodusFinding>) {
    let mut seen = HashSet::new();
    findings.retain(|f| {
        let key = format!("{}:{}:{}", f.file, f.line, f.category.as_str());
        seen.insert(key)
    });
}
