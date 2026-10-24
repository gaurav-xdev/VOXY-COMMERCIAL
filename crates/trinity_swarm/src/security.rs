use crate::error::{Result, SwarmError};
use parking_lot::RwLock;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

pub const OSMOO_OPERATOR_IDENTITY: &str = "OSMOO_AUTONOMOUS_OPERATOR";

/// Security boundary enforcing that Trinity/SwarmClaw orchestration
/// is an internal capability reserved exclusively for OSMOO's autonomous agent.
#[derive(Debug, Clone)]
pub struct SwarmSecurityBoundary {
    session_token: Arc<RwLock<String>>,
    redaction_regex: Regex,
}

impl SwarmSecurityBoundary {
    pub fn new() -> Self {
        let token = format!("osmoo-swarm-{}", Uuid::new_v4());
        let redaction_regex = Regex::new(
            r"(?i)(sk-[a-zA-Z0-9_\-]{20,}|sk-ant-[a-zA-Z0-9_\-]{20,}|AIzaSy[a-zA-Z0-9_\-]{30,}|ghp_[a-zA-Z0-9]{30,}|github_pat_[a-zA-Z0-9_]{30,}|AKIA[0-9A-Z]{16}|bearer\s+[a-zA-Z0-9_\-\.]{20,})"
        ).expect("Valid redaction regex");

        Self {
            session_token: Arc::new(RwLock::new(token)),
            redaction_regex,
        }
    }

    /// Retrieve active internal session token (held by OSMOO operator).
    pub fn session_token(&self) -> String {
        self.session_token.read().clone()
    }

    /// Rotate session token.
    pub fn rotate_token(&self) -> String {
        let mut token = self.session_token.write();
        *token = format!("osmoo-swarm-{}", Uuid::new_v4());
        token.clone()
    }

    /// Verify that caller is strictly OSMOO's autonomous agent engine.
    ///
    /// # Security Contract
    /// Direct user invocations, UI events, and external IPC requests are unconditionally blocked.
    pub fn verify_caller(&self, caller_identity: &str, provided_token: &str) -> Result<()> {
        if caller_identity != OSMOO_OPERATOR_IDENTITY {
            tracing::warn!(
                caller = %caller_identity,
                "Unauthorized attempt to invoke internal swarm orchestrator blocked"
            );
            return Err(SwarmError::AccessDenied(format!(
                "Direct user or external access forbidden. Trinity/Swarm capabilities are internal to OSMOO ('{caller_identity}' rejected)."
            )));
        }

        let active_token = self.session_token.read();
        if !constant_time_eq(provided_token.as_bytes(), active_token.as_bytes()) {
            tracing::warn!("Swarm orchestrator internal token mismatch");
            return Err(SwarmError::InvalidToken);
        }

        Ok(())
    }

    /// Redact any secrets or API tokens from agent output before logging or persistence.
    pub fn redact_secrets(&self, text: &str) -> String {
        self.redaction_regex
            .replace_all(text, "[REDACTED_CREDENTIAL]")
            .to_string()
    }

    /// Validate that a requested file path stays strictly within the workspace directory.
    pub fn validate_workspace_path(&self, workspace_root: &Path, target: &Path) -> Result<PathBuf> {
        let canonical_root = workspace_root.canonicalize().unwrap_or_else(|_| workspace_root.to_path_buf());
        let full_path = if target.is_absolute() {
            target.to_path_buf()
        } else {
            canonical_root.join(target)
        };

        let canonical_target = full_path.canonicalize().unwrap_or(full_path.clone());
        if !canonical_target.starts_with(&canonical_root) {
            return Err(SwarmError::SecurityViolation(format!(
                "Path traversal attempt blocked: path '{:?}' escapes workspace '{:?}'",
                target, canonical_root
            )));
        }

        Ok(canonical_target)
    }
}

impl Default for SwarmSecurityBoundary {
    fn default() -> Self {
        Self::new()
    }
}

/// Constant-time byte slice comparison to prevent timing side channels.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut res = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        res |= x ^ y;
    }
    res == 0
}
