//! Language Toolchain and Verification Sandbox Registry.
//!
//! Provides verifiable environment detection, tool availability checks,
//! and bounded command execution via `SandboxedRunner` and `DiagnosticParser`.
//!
//! Enforces the Non-Negotiable Verification Rule:
//! A verification check can NEVER be reported as `Passed` if the underlying process
//! failed or exited with a non-zero exit code.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::language_profiles::SupportedLanguage;

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("Execution timed out after {0:?}")]
    Timeout(Duration),
    #[error("Command execution failed: {0}")]
    ProcessFailed(#[from] std::io::Error),
    #[error("Security error: working directory escapes repository boundary")]
    InvalidWorkingDir,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub was_success: bool,
}

pub struct SandboxedRunner {
    repo_root: PathBuf,
}

impl SandboxedRunner {
    pub fn new(repo_root: impl AsRef<Path>) -> Self {
        Self {
            repo_root: repo_root.as_ref().to_path_buf(),
        }
    }

    pub async fn run_command(
        &self,
        program: &str,
        args: &[&str],
        relative_cwd: Option<&Path>,
        timeout_duration: Duration,
    ) -> std::result::Result<CommandOutput, RunnerError> {
        let work_dir = match relative_cwd {
            Some(rel) => {
                let joined = self.repo_root.join(rel);
                if !joined.starts_with(&self.repo_root) {
                    return Err(RunnerError::InvalidWorkingDir);
                }
                joined
            }
            None => self.repo_root.clone(),
        };

        let start = Instant::now();
        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args);
        cmd.current_dir(&work_dir);

        if program == "cargo" {
            let isolated_target = std::env::temp_dir().join("voxy_isolated_target");
            cmd.env("CARGO_TARGET_DIR", isolated_target);
        }

        let output_future = cmd.output();
        let output = match tokio::time::timeout(timeout_duration, output_future).await {
            Ok(res) => res?,
            Err(_) => return Err(RunnerError::Timeout(timeout_duration)),
        };

        let duration = start.elapsed();
        let exit_code = output.status.code();
        let was_success = output.status.success();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        Ok(CommandOutput {
            exit_code,
            stdout,
            stderr,
            duration,
            was_success,
        })
    }
}

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
    pub fn parse(output: &str) -> DiagnosticReport {
        let mut report = DiagnosticReport::new();
        for line in output.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("error[") || trimmed.starts_with("error:") {
                let code = trimmed.find('[').and_then(|start| {
                    trimmed
                        .find(']')
                        .map(|end| trimmed[start + 1..end].to_string())
                });
                let msg = if let Some(colon) = trimmed.find(':') {
                    trimmed[colon + 1..].trim().to_string()
                } else {
                    trimmed.to_string()
                };
                report.add_item(DiagnosticItem {
                    severity: DiagnosticSeverity::Error,
                    code,
                    message: msg,
                });
            } else if trimmed.starts_with("warning[") || trimmed.starts_with("warning:") {
                let code = trimmed.find('[').and_then(|start| {
                    trimmed
                        .find(']')
                        .map(|end| trimmed[start + 1..end].to_string())
                });
                let msg = if let Some(colon) = trimmed.find(':') {
                    trimmed[colon + 1..].trim().to_string()
                } else {
                    trimmed.to_string()
                };
                report.add_item(DiagnosticItem {
                    severity: DiagnosticSeverity::Warning,
                    code,
                    message: msg,
                });
            }
        }
        report
    }
}

/// Verification operation categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOp {
    Check,
    Test,
    Lint,
    Format,
    Analyze,
}

impl VerificationOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            VerificationOp::Check => "check",
            VerificationOp::Test => "test",
            VerificationOp::Lint => "lint",
            VerificationOp::Format => "format",
            VerificationOp::Analyze => "analyze",
        }
    }
}

/// Status of an executed verification check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStatus {
    Passed,
    Failed,
    Skipped,
    Unavailable,
    TimedOut,
}

/// Tool availability descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAvailabilityInfo {
    pub tool_name: String,
    pub is_available: bool,
    pub detected_version: Option<String>,
    pub binary_path: Option<String>,
    pub supported_languages: Vec<SupportedLanguage>,
    pub supported_ops: Vec<VerificationOp>,
}

/// Result of executing a toolchain verification command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationExecutionResult {
    pub check_id: String,
    pub language: SupportedLanguage,
    pub operation: VerificationOp,
    pub tool_name: String,
    pub command: String,
    pub status: VerificationStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub parsed_diagnostics: DiagnosticReport,
    pub truth_verified: bool,
    pub observation: String,
}

/// Language toolchain command specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolchainCommandSpec {
    pub language: SupportedLanguage,
    pub operation: VerificationOp,
    pub program: String,
    pub default_args: Vec<String>,
    pub timeout_seconds: u64,
}

/// Enterprise Toolchain Registry managing availability and execution.
pub struct ToolchainRegistry {
    workspace_root: PathBuf,
    command_specs: Vec<ToolchainCommandSpec>,
}

impl ToolchainRegistry {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        let root = workspace_root.into();
        let mut specs = Vec::new();

        // 1. Rust commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Rust,
            operation: VerificationOp::Check,
            program: "cargo".into(),
            default_args: vec!["check".into(), "--message-format=short".into()],
            timeout_seconds: 90,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Rust,
            operation: VerificationOp::Test,
            program: "cargo".into(),
            default_args: vec!["test".into(), "--no-fail-fast".into()],
            timeout_seconds: 120,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Rust,
            operation: VerificationOp::Lint,
            program: "cargo".into(),
            default_args: vec!["clippy".into(), "--no-deps".into()],
            timeout_seconds: 90,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Rust,
            operation: VerificationOp::Format,
            program: "cargo".into(),
            default_args: vec!["fmt".into(), "--check".into()],
            timeout_seconds: 30,
        });

        // 2. TypeScript commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::TypeScript,
            operation: VerificationOp::Check,
            program: "npx".into(),
            default_args: vec!["tsc".into(), "--noEmit".into()],
            timeout_seconds: 60,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::TypeScript,
            operation: VerificationOp::Test,
            program: "npm".into(),
            default_args: vec!["test".into()],
            timeout_seconds: 90,
        });

        // 3. JavaScript commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::JavaScript,
            operation: VerificationOp::Lint,
            program: "npx".into(),
            default_args: vec!["eslint".into(), ".".into()],
            timeout_seconds: 60,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::JavaScript,
            operation: VerificationOp::Test,
            program: "npm".into(),
            default_args: vec!["test".into()],
            timeout_seconds: 60,
        });

        // 4. Python commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Python,
            operation: VerificationOp::Test,
            program: "pytest".into(),
            default_args: vec!["-v".into()],
            timeout_seconds: 90,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Python,
            operation: VerificationOp::Check,
            program: "mypy".into(),
            default_args: vec![".".into()],
            timeout_seconds: 60,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Python,
            operation: VerificationOp::Lint,
            program: "ruff".into(),
            default_args: vec!["check".into(), ".".into()],
            timeout_seconds: 30,
        });

        // 5. C commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::C,
            operation: VerificationOp::Check,
            program: "gcc".into(),
            default_args: vec!["-Wall".into(), "-Wextra".into(), "-Werror".into(), "-fsyntax-only".into()],
            timeout_seconds: 60,
        });

        // 6. C++ commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Cpp,
            operation: VerificationOp::Check,
            program: "cmake".into(),
            default_args: vec!["--build".into(), ".".into()],
            timeout_seconds: 120,
        });

        // 7. C# (.NET) commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::CSharp,
            operation: VerificationOp::Check,
            program: "dotnet".into(),
            default_args: vec!["build".into(), "/warnaserror".into()],
            timeout_seconds: 90,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::CSharp,
            operation: VerificationOp::Test,
            program: "dotnet".into(),
            default_args: vec!["test".into()],
            timeout_seconds: 120,
        });

        // 8. Java commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Java,
            operation: VerificationOp::Check,
            program: "mvn".into(),
            default_args: vec!["compile".into()],
            timeout_seconds: 120,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Java,
            operation: VerificationOp::Test,
            program: "mvn".into(),
            default_args: vec!["test".into()],
            timeout_seconds: 180,
        });

        // 9. Go commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Go,
            operation: VerificationOp::Check,
            program: "go".into(),
            default_args: vec!["vet".into(), "./...".into()],
            timeout_seconds: 60,
        });
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Go,
            operation: VerificationOp::Test,
            program: "go".into(),
            default_args: vec!["test".into(), "-race".into(), "./...".into()],
            timeout_seconds: 90,
        });

        // 10. SQL commands
        specs.push(ToolchainCommandSpec {
            language: SupportedLanguage::Sql,
            operation: VerificationOp::Lint,
            program: "sqlfluff".into(),
            default_args: vec!["lint".into(), ".".into()],
            timeout_seconds: 45,
        });

        Self {
            workspace_root: root,
            command_specs: specs,
        }
    }

    /// Check if a binary exists on the system PATH.
    pub async fn check_binary_available(&self, binary: &str) -> (bool, Option<String>) {
        let runner = SandboxedRunner::new(&self.workspace_root);

        // Try executing `<binary> --version` or `-version` or `version`
        let probe_args = match binary {
            "go" => vec!["version"],
            "dotnet" => vec!["--version"],
            "cargo" | "rustc" | "python" | "node" | "npm" | "npx" | "pytest" | "ruff" | "mypy"
            | "cmake" | "sqlfluff" => vec!["--version"],
            "mvn" => vec!["-version"],
            "gcc" | "clang" | "clang++" | "g++" => vec!["--version"],
            _ => vec!["--version"],
        };

        match runner
            .run_command(binary, &probe_args, None, Duration::from_secs(5))
            .await
        {
            Ok(output) if output.was_success || !output.stdout.is_empty() => {
                let first_line = output
                    .stdout
                    .lines()
                    .next()
                    .or_else(|| output.stderr.lines().next())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                (true, Some(first_line))
            }
            _ => (false, None),
        }
    }

    /// Inspect availability of toolchains for the target language.
    pub async fn inspect_toolchain(&self, language: SupportedLanguage) -> Vec<ToolAvailabilityInfo> {
        let specs_for_lang: Vec<_> = self
            .command_specs
            .iter()
            .filter(|s| s.language == language)
            .collect();

        let mut seen_programs = HashMap::new();

        for spec in specs_for_lang {
            let entry = seen_programs
                .entry(spec.program.clone())
                .or_insert_with(Vec::new);
            entry.push(spec.operation);
        }

        let mut results = Vec::new();

        for (program, ops) in seen_programs {
            let (avail, version) = self.check_binary_available(&program).await;
            results.push(ToolAvailabilityInfo {
                tool_name: program,
                is_available: avail,
                detected_version: version,
                binary_path: None,
                supported_languages: vec![language],
                supported_ops: ops,
            });
        }

        results
    }

    /// Execute a verified toolchain check within the sandbox.
    ///
    /// GUARANTEE: Returns `VerificationStatus::Passed` ONLY IF:
    /// 1. The underlying tool was available.
    /// 2. The process exited with code 0.
    /// 3. Zero fatal compiler/test diagnostics were parsed.
    pub async fn execute_verification(
        &self,
        language: SupportedLanguage,
        operation: VerificationOp,
        custom_args: Option<Vec<String>>,
        relative_cwd: Option<&Path>,
    ) -> VerificationExecutionResult {
        let start = Instant::now();
        let check_id = format!("{}-{}-{}", language.as_str(), operation.as_str(), uuid::Uuid::new_v4().simple());

        let spec = self
            .command_specs
            .iter()
            .find(|s| s.language == language && s.operation == operation);

        let spec = match spec {
            Some(s) => s,
            None => {
                return VerificationExecutionResult {
                    check_id,
                    language,
                    operation,
                    tool_name: "none".into(),
                    command: "none".into(),
                    status: VerificationStatus::Unavailable,
                    exit_code: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    stdout: String::new(),
                    stderr: format!("No command specification configured for {} {:?}", language.as_str(), operation),
                    parsed_diagnostics: DiagnosticReport::new(),
                    truth_verified: false,
                    observation: format!("Verification operation {:?} not configured for {}", operation, language.as_str()),
                };
            }
        };

        // Check availability
        let (is_avail, _) = self.check_binary_available(&spec.program).await;
        if !is_avail {
            return VerificationExecutionResult {
                check_id,
                language,
                operation,
                tool_name: spec.program.clone(),
                command: spec.program.clone(),
                status: VerificationStatus::Unavailable,
                exit_code: None,
                duration_ms: start.elapsed().as_millis() as u64,
                stdout: String::new(),
                stderr: format!("Tool '{}' is not installed or available on system PATH", spec.program),
                parsed_diagnostics: DiagnosticReport::new(),
                truth_verified: false,
                observation: format!("Skipped: tool '{}' unavailable", spec.program),
            };
        }

        let args = custom_args.unwrap_or_else(|| spec.default_args.clone());
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let command_str = format!("{} {}", spec.program, args.join(" "));

        let runner = SandboxedRunner::new(&self.workspace_root);
        let run_result = runner
            .run_command(
                &spec.program,
                &args_refs,
                relative_cwd,
                Duration::from_secs(spec.timeout_seconds),
            )
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match run_result {
            Ok(output) => {
                let merged_output = format!("{}\n{}", output.stdout, output.stderr);
                let parsed = DiagnosticParser::parse(&merged_output);

                let is_clean_exit = output.was_success && output.exit_code == Some(0);
                let zero_errors = parsed.total_errors == 0;

                let (status, truth_verified) = if is_clean_exit && zero_errors {
                    (VerificationStatus::Passed, true)
                } else {
                    (VerificationStatus::Failed, false)
                };

                let obs = if truth_verified {
                    format!("Passed: '{}' completed in {}ms with 0 errors", command_str, duration_ms)
                } else {
                    format!(
                        "Failed: '{}' exited with code {:?} ({} errors parsed)",
                        command_str, output.exit_code, parsed.total_errors
                    )
                };

                VerificationExecutionResult {
                    check_id,
                    language,
                    operation,
                    tool_name: spec.program.clone(),
                    command: command_str,
                    status,
                    exit_code: output.exit_code,
                    duration_ms,
                    stdout: output.stdout,
                    stderr: output.stderr,
                    parsed_diagnostics: parsed,
                    truth_verified,
                    observation: obs,
                }
            }
            Err(RunnerError::Timeout(d)) => VerificationExecutionResult {
                check_id,
                language,
                operation,
                tool_name: spec.program.clone(),
                command: command_str,
                status: VerificationStatus::TimedOut,
                exit_code: None,
                duration_ms,
                stdout: String::new(),
                stderr: format!("Command timed out after {:?}", d),
                parsed_diagnostics: DiagnosticReport::new(),
                truth_verified: false,
                observation: format!("Timed out after {:?}", d),
            },
            Err(e) => VerificationExecutionResult {
                check_id,
                language,
                operation,
                tool_name: spec.program.clone(),
                command: command_str,
                status: VerificationStatus::Failed,
                exit_code: None,
                duration_ms,
                stdout: String::new(),
                stderr: format!("Process execution error: {}", e),
                parsed_diagnostics: DiagnosticReport::new(),
                truth_verified: false,
                observation: format!("Process execution failed: {}", e),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_toolchain_registry_cargo_check() {
        let registry = ToolchainRegistry::new(".");
        let info = registry.inspect_toolchain(SupportedLanguage::Rust).await;
        assert!(!info.is_empty());
        let cargo = info.iter().find(|t| t.tool_name == "cargo").unwrap();
        assert!(cargo.is_available, "cargo must be available in dev environment");
        assert!(cargo.detected_version.is_some());
    }

    #[tokio::test]
    async fn test_toolchain_uninstalled_tool_honest_reporting() {
        let registry = ToolchainRegistry::new(".");
        let res = registry
            .execute_verification(SupportedLanguage::Sql, VerificationOp::Lint, None, None)
            .await;

        // If sqlfluff is not installed, it must report Unavailable honestly, NEVER Passed
        if res.status == VerificationStatus::Unavailable {
            assert!(!res.truth_verified);
            assert!(res.stderr.contains("not installed"));
        } else {
            // If installed, it must have verified exit code
            assert_eq!(res.status == VerificationStatus::Passed, res.truth_verified);
        }
    }
}
