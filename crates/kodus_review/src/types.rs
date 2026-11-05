use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KodusSeverity {
    Informational = 0,
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl KodusSeverity {
    pub fn is_blocking(&self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Informational => "informational",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KodusCategory {
    LogicBug,
    SecurityVulnerability,
    RustMemorySafety,
    WindowsNativeIpc,
    PythonSidecarReliability,
    ReactTypeScriptCorrectness,
    PerformanceRegression,
    MissingTests,
    ArchitectureViolation,
}

impl KodusCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LogicBug => "logic_bug",
            Self::SecurityVulnerability => "security_vulnerability",
            Self::RustMemorySafety => "rust_memory_safety",
            Self::WindowsNativeIpc => "windows_native_ipc",
            Self::PythonSidecarReliability => "python_sidecar_reliability",
            Self::ReactTypeScriptCorrectness => "react_typescript_correctness",
            Self::PerformanceRegression => "performance_regression",
            Self::MissingTests => "missing_tests",
            Self::ArchitectureViolation => "architecture_violation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewTrigger {
    SubstantialCodeChange,
    PreTaskCompletion,
    PrePullRequestMerge,
    SecuritySensitiveChange,
    TestFailureInvestigation,
    ExplicitAgentRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewScope {
    WorkingTree,
    StagedOnly,
    Branch(String),
    Commit(String),
    Files(Vec<PathBuf>),
    ExplicitDiff(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KodusFinding {
    pub id: String,
    pub file: String,
    pub line: usize,
    pub end_line: usize,
    pub severity: KodusSeverity,
    pub category: KodusCategory,
    pub title: String,
    pub what: String,
    pub why: String,
    pub how: Option<String>,
    pub is_confirmed: bool,
    pub confidence_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KodusReviewReport {
    pub report_id: String,
    pub timestamp: DateTime<Utc>,
    pub trigger: ReviewTrigger,
    pub provider: String,
    pub model: String,
    pub files_reviewed: Vec<String>,
    pub findings: Vec<KodusFinding>,
    pub duration_ms: u64,
    pub has_blocking_defects: bool,
}

impl KodusReviewReport {
    pub fn empty(trigger: ReviewTrigger, provider: String, model: String) -> Self {
        Self {
            report_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            trigger,
            provider,
            model,
            files_reviewed: Vec::new(),
            findings: Vec::new(),
            duration_ms: 0,
            has_blocking_defects: false,
        }
    }

    pub fn blocking_findings(&self) -> Vec<&KodusFinding> {
        self.findings
            .iter()
            .filter(|f| f.severity.is_blocking())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationStatus {
    CleanPass,
    RemediatedAndVerified {
        iterations: usize,
    },
    MaxIterationsReached {
        iterations: usize,
        remaining_findings: usize,
    },
    EscalatedToHuman {
        reason: String,
    },
}
