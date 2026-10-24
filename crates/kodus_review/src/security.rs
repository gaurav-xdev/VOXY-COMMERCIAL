use parking_lot::RwLock;
use regex::Regex;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub const OSMOO_OPERATOR_IDENTITY: &str = "OSMOO_AUTONOMOUS_OPERATOR";

#[derive(Debug, Error)]
pub enum KodusSecurityError {
    #[error("Unauthorized Kodus invocation: {0}")]
    AccessDenied(String),

    #[error("Internal token mismatch")]
    InvalidToken,

    #[error("Path traversal or boundary violation: {0}")]
    PathViolation(String),
}

/// Security boundary enforcing that Kodus is an internal-only autonomous capability.
#[derive(Debug, Clone)]
pub struct KodusSecurityBoundary {
    session_token: Arc<RwLock<String>>,
    redaction_regex: Regex,
}

impl KodusSecurityBoundary {
    pub fn new() -> Self {
        let token = format!("osmoo-kodus-{}", Uuid::new_v4());
        // Compile high-confidence patterns for API keys and tokens
        let redaction_regex = Regex::new(
            r"(?i)(sk-[a-zA-Z0-9_\-]{20,}|sk-ant-[a-zA-Z0-9_\-]{20,}|AIzaSy[a-zA-Z0-9_\-]{30,}|ghp_[a-zA-Z0-9]{30,}|github_pat_[a-zA-Z0-9_]{30,}|AKIA[0-9A-Z]{16}|kodus_[a-zA-Z0-9_\-]{20,}|bearer\s+[a-zA-Z0-9_\-\.]{20,})"
        ).expect("Valid redaction regex");

        Self {
            session_token: Arc::new(RwLock::new(token)),
            redaction_regex,
        }
    }

    /// Retrieve active internal session token (known only to OSMOO runtime).
    pub fn session_token(&self) -> String {
        self.session_token.read().clone()
    }

    /// Rotate the session token.
    pub fn rotate_token(&self) -> String {
        let mut token = self.session_token.write();
        *token = format!("osmoo-kodus-{}", Uuid::new_v4());
        token.clone()
    }

    /// Verify that the caller is strictly OSMOO's autonomous agent engine.
    ///
    /// # Security Contract
    /// - Ordinary human users, UI actions, or external IPC callers are unconditionally rejected.
    /// - Token comparison is performed in constant time.
    pub fn verify_caller(&self, caller_identity: &str, provided_token: &str) -> Result<(), KodusSecurityError> {
        if caller_identity != OSMOO_OPERATOR_IDENTITY {
            tracing::warn!(
                caller = %caller_identity,
                "Unauthorized attempt to invoke internal Kodus reviewer blocked"
            );
            return Err(KodusSecurityError::AccessDenied(format!(
                "Direct user or external access forbidden. Kodus is an internal capability exclusively available to OSMOO ('{caller_identity}' rejected)."
            )));
        }

        let active_token = self.session_token.read();
        if !constant_time_eq(provided_token.as_bytes(), active_token.as_bytes()) {
            return Err(KodusSecurityError::InvalidToken);
        }

        Ok(())
    }

    /// Scrub all known secret formats (OpenAI, Anthropic, Gemini, Kodus, GitHub, AWS, Bearer) from text.
    pub fn redact_secrets(&self, text: &str) -> String {
        self.redaction_regex
            .replace_all(text, "[REDACTED_API_KEY]")
            .to_string()
    }
}

impl Default for KodusSecurityBoundary {
    fn default() -> Self {
        Self::new()
    }
}

/// Constant-time byte slice comparison to mitigate timing side-channel attacks on tokens.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut res = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        res |= x ^ y;
    }
    res == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kodus_access_strictly_osmoo_only() {
        let boundary = KodusSecurityBoundary::new();
        let token = boundary.session_token();

        // 1. Authorized OSMOO caller succeeds
        assert!(boundary.verify_caller(OSMOO_OPERATOR_IDENTITY, &token).is_ok());

        // 2. Direct user caller is rejected
        let user_err = boundary.verify_caller("user", &token).unwrap_err();
        assert!(matches!(user_err, KodusSecurityError::AccessDenied(_)));

        // 3. UI client caller is rejected
        let ui_err = boundary.verify_caller("desktop_ui", &token).unwrap_err();
        assert!(matches!(ui_err, KodusSecurityError::AccessDenied(_)));

        // 4. Invalid token for OSMOO is rejected
        let bad_token_err = boundary
            .verify_caller(OSMOO_OPERATOR_IDENTITY, "fake-token-xyz")
            .unwrap_err();
        assert!(matches!(bad_token_err, KodusSecurityError::InvalidToken));
    }

    #[test]
    fn test_secret_redaction() {
        let boundary = KodusSecurityBoundary::new();
        let input = "Calling OpenAI with sk-1234567890abcdef1234567890 and Anthropic sk-ant-api03-abcdef12345678901234567890";
        let scrubbed = boundary.redact_secrets(input);
        assert!(!scrubbed.contains("sk-1234567890abcdef1234567890"));
        assert!(!scrubbed.contains("sk-ant-api03-abcdef12345678901234567890"));
        assert!(scrubbed.contains("[REDACTED_API_KEY]"));
    }
}
