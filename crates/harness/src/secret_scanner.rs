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
        if crate::patch_engine::is_protected_file(file_path) {
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
                    rule: "private_key",
                    description: "Private cryptographic key block detected",
                    line_number: line_num,
                });
            } else if line.contains("PuTTY-User-Key-File") {
                detected.push(DetectedSecret {
                    rule: "private_key",
                    description: "PuTTY private key detected",
                    line_number: line_num,
                });
            }

            // GitHub personal access tokens
            if line.contains("ghp_")
                || line.contains("gho_")
                || line.contains("ghu_")
                || line.contains("ghs_")
                || line.contains("ghr_")
                || line.contains("github_pat_")
            {
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

        let with_aws_asia = "let temp_key = \"ASIATEMP1234567890AB\";\n";
        let errs_asia = SecretScanner::scan_patch(Path::new("src/config.rs"), with_aws_asia).unwrap_err();
        assert_eq!(errs_asia[0].rule, "aws_access_key");

        let with_key = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA0\n";
        let errs2 = SecretScanner::scan_patch(Path::new("src/keys.rs"), with_key).unwrap_err();
        assert_eq!(errs2[0].rule, "private_key");

        let with_ec_key = "-----BEGIN EC PRIVATE KEY-----\nMHcCAQEEI...\n";
        let errs3 = SecretScanner::scan_patch(Path::new("src/keys.rs"), with_ec_key).unwrap_err();
        assert_eq!(errs3[0].rule, "private_key");
    }

    #[test]
    fn test_secret_detection_github_and_api_keys() {
        let with_gh = "const token = 'ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';\n";
        let errs = SecretScanner::scan_patch(Path::new("src/auth.ts"), with_gh).unwrap_err();
        assert_eq!(errs[0].rule, "github_token");

        let with_gh_pat = "const token = 'github_pat_11AAAAAA_BBBBBBBBBBBBBBBBBBBB';\n";
        let errs_pat = SecretScanner::scan_patch(Path::new("src/auth.ts"), with_gh_pat).unwrap_err();
        assert_eq!(errs_pat[0].rule, "github_token");

        let with_sk = "OPENAI_API_KEY = 'sk-proj-1234567890abcdefghijklmnopqrstuvwxyz'\n";
        let errs2 = SecretScanner::scan_patch(Path::new("config.py"), with_sk).unwrap_err();
        assert_eq!(errs2[0].rule, "generic_api_key");

        let with_anthropic = "ANTHROPIC_API_KEY = 'sk-ant-api03-abcdef1234567890uvwxyz'\n";
        let errs3 = SecretScanner::scan_patch(Path::new("config.py"), with_anthropic).unwrap_err();
        assert_eq!(errs3[0].rule, "generic_api_key");
    }

    #[test]
    fn test_secret_detection_protected_files() {
        let clean = "pub fn foo() {}\n";
        let err_cargo_lock = SecretScanner::scan_patch(Path::new("Cargo.lock"), clean).unwrap_err();
        assert_eq!(err_cargo_lock[0].rule, "protected_secret_file");

        let err_git = SecretScanner::scan_patch(Path::new(".git/config"), clean).unwrap_err();
        assert_eq!(err_git[0].rule, "protected_secret_file");

        let err_env = SecretScanner::scan_patch(Path::new(".env.local"), clean).unwrap_err();
        assert_eq!(err_env[0].rule, "protected_secret_file");

        let err_ssh = SecretScanner::scan_patch(Path::new(".ssh/id_rsa"), clean).unwrap_err();
        assert_eq!(err_ssh[0].rule, "protected_secret_file");
    }
}
