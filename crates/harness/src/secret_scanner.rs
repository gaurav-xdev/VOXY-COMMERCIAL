//! Security scanner for detecting accidental secret insertion in generated patches.

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
        let mut detected = Vec::new();

        // 1. Strict path exclusion: files that should never be written via patch engine
        let path_str = file_path.to_string_lossy();
        if path_str.ends_with(".env") || path_str.contains(".ssh") || path_str.ends_with("id_rsa") {
            detected.push(DetectedSecret {
                rule: "protected_secret_file",
                description: "Modification of environment secret files or SSH keys is blocked",
                line_number: 1,
            });
            return Err(detected);
        }

        // 2. Content pattern scanning
        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;

            // AWS Access Key ID
            if line.contains("AKIA") && line.len() >= 20 {
                if let Some(pos) = line.find("AKIA") {
                    let candidate = &line[pos..];
                    let key_part: String = candidate.chars().take(20).collect();
                    if key_part.len() == 20 && key_part.chars().all(|c| c.is_ascii_alphanumeric()) {
                        detected.push(DetectedSecret {
                            rule: "aws_access_key",
                            description: "AWS Access Key pattern detected",
                            line_number: line_num,
                        });
                    }
                }
            }

            // Private Keys (RSA, EC, OpenSSH)
            if line.contains("-----BEGIN") && (line.contains("PRIVATE KEY") || line.contains("RSA PRIVATE KEY")) {
                detected.push(DetectedSecret {
                    rule: "private_key",
                    description: "Private cryptographic key block detected",
                    line_number: line_num,
                });
            }

            // GitHub personal access tokens
            if line.contains("ghp_") || line.contains("gho_") || line.contains("github_pat_") {
                detected.push(DetectedSecret {
                    rule: "github_token",
                    description: "GitHub personal access token detected",
                    line_number: line_num,
                });
            }

            // OpenAI / Generic API keys (sk-...)
            if line.contains("sk-") && line.len() > 25 {
                if let Some(pos) = line.find("sk-") {
                    let candidate = &line[pos..];
                    let prefix_chars: String = candidate.chars().take(20).collect();
                    if prefix_chars.len() >= 20 && prefix_chars.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                        detected.push(DetectedSecret {
                            rule: "generic_api_key",
                            description: "API secret token (sk-*) detected",
                            line_number: line_num,
                        });
                    }
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
    fn test_secret_detection_aws_and_private_key() {
        let clean = "pub fn add(a: i32, b: i32) -> i32 { a + b }\n";
        assert!(SecretScanner::scan_patch(Path::new("src/math.rs"), clean).is_ok());

        let with_aws = "let key = \"AKIAIOSFODNN7EXAMPLE\";\n";
        let errs = SecretScanner::scan_patch(Path::new("src/config.rs"), with_aws).unwrap_err();
        assert_eq!(errs[0].rule, "aws_access_key");

        let with_key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0\n";
        let errs2 = SecretScanner::scan_patch(Path::new("src/keys.rs"), with_key).unwrap_err();
        assert_eq!(errs2[0].rule, "private_key");
    }

    #[test]
    fn test_secret_detection_github_and_api_keys() {
        let with_gh = "const token = 'ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';\n";
        let errs = SecretScanner::scan_patch(Path::new("src/auth.ts"), with_gh).unwrap_err();
        assert_eq!(errs[0].rule, "github_token");

        let with_sk = "OPENAI_API_KEY = 'sk-proj-1234567890abcdefghijklmnopqrstuvwxyz'\n";
        let errs2 = SecretScanner::scan_patch(Path::new("config.py"), with_sk).unwrap_err();
        assert_eq!(errs2[0].rule, "generic_api_key");
    }
}
