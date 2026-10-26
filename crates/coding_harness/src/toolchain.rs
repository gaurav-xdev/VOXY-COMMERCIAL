use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Language {
    Rust,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Sql,
    Python,
    JavaScript,
    TypeScript,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ToolchainStatus {
    Available { binary: String, version: String },
    MissingToolchain { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageToolchain {
    pub language: Language,
    pub status: ToolchainStatus,
    pub build_command: Option<String>,
    pub test_command: Option<String>,
}

pub struct ToolchainDetector;

impl ToolchainDetector {
    pub fn detect_all(workspace_root: &Path) -> Vec<LanguageToolchain> {
        vec![
            Self::detect_rust(workspace_root),
            Self::detect_go(workspace_root),
            Self::detect_java(workspace_root),
            Self::detect_c(workspace_root),
            Self::detect_cpp(workspace_root),
            Self::detect_csharp(workspace_root),
            Self::detect_sql(workspace_root),
            Self::detect_python(workspace_root),
            Self::detect_javascript(workspace_root),
            Self::detect_typescript(workspace_root),
        ]
    }

    pub fn detect_rust(workspace_root: &Path) -> LanguageToolchain {
        let has_manifest = workspace_root.join("Cargo.toml").exists();
        let status = match std::process::Command::new("cargo").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolchainStatus::Available { binary: "cargo".into(), version: ver }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "cargo executable not found".into() },
        };

        LanguageToolchain {
            language: Language::Rust,
            status,
            build_command: if has_manifest { Some("cargo check".into()) } else { None },
            test_command: if has_manifest { Some("cargo test".into()) } else { None },
        }
    }

    pub fn detect_go(workspace_root: &Path) -> LanguageToolchain {
        let has_manifest = workspace_root.join("go.mod").exists();
        let status = match std::process::Command::new("go").arg("version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolchainStatus::Available { binary: "go".into(), version: ver }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "go executable not found".into() },
        };

        LanguageToolchain {
            language: Language::Go,
            status,
            build_command: if has_manifest { Some("go build ./...".into()) } else { None },
            test_command: if has_manifest { Some("go test ./...".into()) } else { None },
        }
    }

    pub fn detect_java(workspace_root: &Path) -> LanguageToolchain {
        let has_gradle = workspace_root.join("build.gradle").exists() || workspace_root.join("build.gradle.kts").exists();
        let has_maven = workspace_root.join("pom.xml").exists();
        let status = match std::process::Command::new("javac").arg("-version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let ver = if ver.is_empty() { String::from_utf8_lossy(&output.stderr).trim().to_string() } else { ver };
                ToolchainStatus::Available { binary: "javac".into(), version: ver }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "javac executable not found".into() },
        };

        let (build_cmd, test_cmd) = if has_gradle {
            (Some("gradle build".into()), Some("gradle test".into()))
        } else if has_maven {
            (Some("mvn compile".into()), Some("mvn test".into()))
        } else {
            (None, None)
        };

        LanguageToolchain {
            language: Language::Java,
            status,
            build_command: build_cmd,
            test_command: test_cmd,
        }
    }

    pub fn detect_c(workspace_root: &Path) -> LanguageToolchain {
        let has_cmake = workspace_root.join("CMakeLists.txt").exists();
        let status = match std::process::Command::new("clang").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let first_line = String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("clang").to_string();
                ToolchainStatus::Available { binary: "clang".into(), version: first_line }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "clang executable not found".into() },
        };

        LanguageToolchain {
            language: Language::C,
            status,
            build_command: if has_cmake { Some("cmake --build build".into()) } else { None },
            test_command: if has_cmake { Some("ctest --test-dir build".into()) } else { None },
        }
    }

    pub fn detect_cpp(workspace_root: &Path) -> LanguageToolchain {
        let has_cmake = workspace_root.join("CMakeLists.txt").exists();
        let status = match std::process::Command::new("clang++").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let first_line = String::from_utf8_lossy(&output.stdout).lines().next().unwrap_or("clang++").to_string();
                ToolchainStatus::Available { binary: "clang++".into(), version: first_line }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "clang++ executable not found".into() },
        };

        LanguageToolchain {
            language: Language::Cpp,
            status,
            build_command: if has_cmake { Some("cmake --build build".into()) } else { None },
            test_command: if has_cmake { Some("ctest --test-dir build".into()) } else { None },
        }
    }

    pub fn detect_csharp(_workspace_root: &Path) -> LanguageToolchain {
        let status = match std::process::Command::new("dotnet").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolchainStatus::Available { binary: "dotnet".into(), version: ver }
            }
            _ => ToolchainStatus::MissingToolchain {
                reason: "dotnet CLI not installed in PATH on this host".into(),
            },
        };

        LanguageToolchain {
            language: Language::CSharp,
            status,
            build_command: None,
            test_command: None,
        }
    }

    pub fn detect_sql(workspace_root: &Path) -> LanguageToolchain {
        let has_sql = workspace_root.join("schema.sql").exists() || workspace_root.join("migrations").exists();
        let status = match std::process::Command::new("sqlite3").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolchainStatus::Available { binary: "sqlite3".into(), version: ver }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "sqlite3 binary not found".into() },
        };

        LanguageToolchain {
            language: Language::Sql,
            status,
            build_command: None,
            test_command: if has_sql { Some("sqlite3 :memory: < schema.sql".into()) } else { None },
        }
    }

    pub fn detect_python(workspace_root: &Path) -> LanguageToolchain {
        let has_py = workspace_root.join("pyproject.toml").exists()
            || workspace_root.join("setup.py").exists()
            || workspace_root.join("requirements.txt").exists();

        let status = match std::process::Command::new("python").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let ver = if ver.is_empty() { String::from_utf8_lossy(&output.stderr).trim().to_string() } else { ver };
                ToolchainStatus::Available { binary: "python".into(), version: ver }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "python binary not found".into() },
        };

        LanguageToolchain {
            language: Language::Python,
            status,
            build_command: None,
            test_command: if has_py { Some("python -m pytest".into()) } else { None },
        }
    }

    pub fn detect_javascript(workspace_root: &Path) -> LanguageToolchain {
        let has_pkg = workspace_root.join("package.json").exists();
        let status = match std::process::Command::new("node").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                ToolchainStatus::Available { binary: "node".into(), version: ver }
            }
            _ => ToolchainStatus::MissingToolchain { reason: "node binary not found".into() },
        };

        LanguageToolchain {
            language: Language::JavaScript,
            status,
            build_command: if has_pkg { Some("npm run build".into()) } else { None },
            test_command: if has_pkg { Some("npm test".into()) } else { None },
        }
    }

    pub fn detect_typescript(workspace_root: &Path) -> LanguageToolchain {
        let has_tsconfig = workspace_root.join("tsconfig.json").exists();
        let local_tsc = workspace_root.join("node_modules").join(".bin").join("tsc.cmd");

        let status = if local_tsc.exists() {
            ToolchainStatus::Available {
                binary: local_tsc.to_string_lossy().to_string(),
                version: "local node_modules tsc".into(),
            }
        } else {
            match std::process::Command::new("tsc").arg("--version").output() {
                Ok(output) if output.status.success() => {
                    let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    ToolchainStatus::Available { binary: "tsc".into(), version: ver }
                }
                _ => ToolchainStatus::MissingToolchain {
                    reason: "tsc not installed globally or in local node_modules".into(),
                },
            }
        };

        LanguageToolchain {
            language: Language::TypeScript,
            status,
            build_command: if has_tsconfig { Some("npx tsc --noEmit".into()) } else { None },
            test_command: if has_tsconfig { Some("npm test".into()) } else { None },
        }
    }
}
