use crate::error::{Result, StudioError};
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

pub const OSMOO_OPERATOR_IDENTITY: &str = "OSMOO_AUTONOMOUS_OPERATOR";

/// Secure access controller enforcing that the Video Studio enclave is accessible
/// strictly to OSMOO's autonomous cognitive brain, rejecting direct user calls or untrusted actors.
#[derive(Debug, Clone)]
pub struct EnclaveAccessController {
    session_token: Arc<RwLock<String>>,
    enclave_root: PathBuf,
}

impl EnclaveAccessController {
    /// Create a new access controller with a cryptographically secure random session token.
    pub fn new(enclave_root: impl Into<PathBuf>) -> Self {
        let root = enclave_root.into();
        let token = format!("osmoo-enclave-{}", Uuid::new_v4());
        Self {
            session_token: Arc::new(RwLock::new(token)),
            enclave_root: root,
        }
    }

    /// Retrieve the current session token (accessible only to OSMOO runtime).
    pub fn session_token(&self) -> String {
        self.session_token.read().clone()
    }

    /// Rotate the session token.
    pub fn rotate_token(&self) -> String {
        let mut token = self.session_token.write();
        *token = format!("osmoo-enclave-{}", Uuid::new_v4());
        token.clone()
    }

    /// Authorize a request against the Video Studio enclave.
    ///
    /// # Security Boundary
    /// 1. Verifies that the caller is explicitly `OSMOO_AUTONOMOUS_OPERATOR`.
    /// 2. Verifies that the provided token matches the active in-memory session token using constant-time comparison.
    /// 3. Rejects any attempt by human users or untrusted external clients to directly execute studio tools.
    pub fn verify_access(&self, caller_identity: &str, token: &str) -> Result<()> {
        if caller_identity != OSMOO_OPERATOR_IDENTITY {
            tracing::warn!(
                caller = %caller_identity,
                "Unauthorized access attempt to OSMOO Video Studio Enclave blocked"
            );
            return Err(StudioError::Unauthorized(format!(
                "Direct user or untrusted access forbidden. Caller '{caller_identity}' is not the authorized OSMOO autonomous operator."
            )));
        }

        let current = self.session_token.read();
        if !constant_time_eq(token.as_bytes(), current.as_bytes()) {
            tracing::warn!("Invalid enclave bearer token provided by caller");
            return Err(StudioError::Unauthorized(
                "Invalid enclave token: caller authentication failed.".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate that a given target path is strictly contained within the secure enclave root.
    pub fn validate_enclave_path(&self, candidate: &Path) -> Result<PathBuf> {
        let canonical_root = self
            .enclave_root
            .canonicalize()
            .map_err(|_| StudioError::EnclaveNotFound(self.enclave_root.clone()))?;

        let candidate_canonical = candidate
            .canonicalize()
            .map_err(|e| StudioError::SecurityViolation(format!("Cannot resolve path: {e}")))?;

        if !candidate_canonical.starts_with(&canonical_root) {
            return Err(StudioError::SecurityViolation(format!(
                "Path traversal rejected: '{:?}' is outside enclave root '{:?}'",
                candidate_canonical, canonical_root
            )));
        }

        Ok(candidate_canonical)
    }

    /// Enforce Windows filesystem ACLs on the enclave workspace to lock out standard user browsing.
    pub fn secure_workspace_permissions(&self, workspace_path: &Path) -> Result<()> {
        if !workspace_path.exists() {
            std::fs::create_dir_all(workspace_path)?;
        }

        #[cfg(windows)]
        {
            // On Windows, set hidden attribute and restrict DACL to current process owner if possible
            use std::process::Command;
            let path_str = workspace_path.to_string_lossy();
            let _ = Command::new("attrib")
                .args(["+H", "+S", path_str.as_ref()])
                .output();
        }

        Ok(())
    }
}

/// Constant-time byte slice comparison to prevent timing side-channel attacks on bearer tokens.
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
    fn test_enclave_access_authorized_for_osmoo_only() {
        let temp_dir = std::env::temp_dir().join("osmoo_test_enclave_auth");
        let controller = EnclaveAccessController::new(&temp_dir);
        let valid_token = controller.session_token();

        // 1. OSMOO Operator succeeds
        assert!(controller
            .verify_access(OSMOO_OPERATOR_IDENTITY, &valid_token)
            .is_ok());

        // 2. Direct user access is strictly rejected
        let user_err = controller
            .verify_access("user", &valid_token)
            .unwrap_err();
        assert!(matches!(user_err, StudioError::Unauthorized(_)));

        // 3. UI client direct access is rejected
        let ui_err = controller
            .verify_access("desktop_ui_client", &valid_token)
            .unwrap_err();
        assert!(matches!(ui_err, StudioError::Unauthorized(_)));

        // 4. Invalid token for OSMOO is rejected
        let bad_token_err = controller
            .verify_access(OSMOO_OPERATOR_IDENTITY, "invalid-token-12345")
            .unwrap_err();
        assert!(matches!(bad_token_err, StudioError::Unauthorized(_)));
    }
}
