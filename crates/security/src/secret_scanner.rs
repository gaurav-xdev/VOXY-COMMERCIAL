//! Security scanner for detecting accidental secret or credential insertion.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedSecret {
    pub rule: &'static str,
    pub description: &'static str,
    pub line_number: usize,
}

pub struct SecretScanner;

impl SecretScanner {
    /// Inspects content and returns any matched secrets or private key material.
    pub fn scan_patch(file_path: &Path, content: &str) -> Result<(), Vec<DetectedSecret>> {
        Self::scan(file_path, content)
    }

    /// Inspects content and returns any matched secrets or private key material.
    pub fn scan(file_path: &Path, content: &str) -> Result<(), Vec<DetectedSecret>> {
        let mut detected = Vec::new();

        // 1. Strict path exclusion: files that should never be written
        if crate::path_guard::is_protected_file(file_path) {
            detected.push(DetectedSecret {
                rule: "protected_secret_file",
                description: "Modification of environment secret files, Cargo.lock, .git, or SSH keys is blocked",
                line_number: 1,
            });
            return Err(detected);
        }

        // 2. Content pattern scanning
        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;

            // AWS Access Key ID (standard AKIA or temporary ASIA)
            for prefix in &["AKIA", "ASIA"] {
                let mut start = 0;
                while let Some(pos) = line[start..].find(prefix) {
                    let abs_pos = start + pos;
                    let candidate = &line[abs_pos..];
                    let key_part: String = candidate.chars().take(20).collect();
                    if key_part.len() == 20 && key_part.chars().all(|c| c.is_ascii_alphanumeric()) {
                        detected.push(DetectedSecret {
                            rule: "aws_access_key",
                            description: "AWS Access Key pattern detected",
                            line_number: line_num,
                        });
                        break;
                    }
                    start = abs_pos + prefix.len();
                }
            }

            // Private Keys (RSA, EC, OpenSSH, DSA, PGP)
            if line.contains("-----BEGIN")
                && (line.contains("PRIVATE KEY")
                    || line.contains("RSA PRIVATE KEY")
                    || line.contains("EC PRIVATE KEY")
                    || line.contains("OPENSSH PRIVATE KEY")
                    || line.contains("DSA PRIVATE KEY"))
            {
                detected.push(DetectedSecret {
                    rule: "private_key_material",
                    description: "Cryptographic private key header detected",
                    line_number: line_num,
                });
            }

            // GitHub personal access tokens (classic ghp_ or modern github_pat_)
            if line.contains("ghp_") || line.contains("github_pat_") {
                detected.push(DetectedSecret {
                    rule: "github_token",
                    description: "GitHub personal access token detected",
                    line_number: line_num,
                });
            }

            // OpenAI / Anthropic / Generic API keys (sk-...)
            if let Some(pos) = line.find("sk-") {
                let candidate = &line[pos..];
                let prefix_chars: String = candidate
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                if prefix_chars.len() >= 20 {
                    detected.push(DetectedSecret {
                        rule: "generic_api_key",
                        description: "API secret token (sk-*) detected",
                        line_number: line_num,
                    });
                }
            }

            // Generic API key assignments
            let trimmed = line.trim();
            if (trimmed.starts_with("api_key")
                || trimmed.starts_with("apiKey")
                || trimmed.starts_with("API_KEY")
                || trimmed.starts_with("secret_key")
                || trimmed.starts_with("SECRET_KEY"))
                && (trimmed.contains('=') || trimmed.contains(':'))
            {
                let val_part = if let Some(pos) = trimmed.find('=') {
                    trimmed[pos + 1..].trim()
                } else if let Some(pos) = trimmed.find(':') {
                    trimmed[pos + 1..].trim()
                } else {
                    ""
                };

                let clean_val = val_part.trim_matches(|c| c == '"' || c == '\'' || c == ',' || c == ';');
                if clean_val.len() >= 20
                    && !clean_val.contains("placeholder")
                    && !clean_val.contains("example")
                    && !clean_val.contains("test")
                    && !clean_val.contains("your_")
                {
                    detected.push(DetectedSecret {
                        rule: "generic_api_key",
                        description: "High-entropy API key or secret token detected",
                        line_number: line_num,
                    });
                }
            }
        }

        if detected.is_empty() {
            Ok(())
        } else {
            Err(detected)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_detection() {
        let clean = "fn main() { println!(\"Hello World\"); }";
        assert!(SecretScanner::scan(Path::new("src/main.rs"), clean).is_ok());

        let with_key = "let key = \"AKIAIOSFODNN7EXAMPLE1\";";
        assert!(SecretScanner::scan(Path::new("src/main.rs"), with_key).is_err());
    }
}
