use crate::audit::{AuditStatus, OfficeAuditEvent, OfficeAuditTrail};
use crate::error::{OfficeError, Result};
use crate::scope::{OfficeScope, ScopeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Sandboxed filesystem connector that guarantees all file operations
/// stay strictly bounded within the authorized root workspace.
#[derive(Debug, Clone)]
pub struct FilesystemConnector {
    allowed_root: PathBuf,
    audit_trail: Arc<OfficeAuditTrail>,
}

impl FilesystemConnector {
    pub fn new<P: AsRef<Path>>(
        allowed_root: P,
        audit_trail: Arc<OfficeAuditTrail>,
    ) -> Result<Self> {
        let root = allowed_root
            .as_ref()
            .canonicalize()
            .map_err(|e| OfficeError::Io(format!("Invalid allowed root: {e}")))?;

        Ok(Self {
            allowed_root: root,
            audit_trail,
        })
    }

    /// Resolve and validate that a requested path does NOT escape the allowed root.
    fn resolve_safe_path(&self, relative_path: &str) -> Result<PathBuf> {
        // Prevent obvious directory traversal patterns in raw string
        if relative_path.contains("..") {
            return Err(OfficeError::PathTraversalAttempt {
                path: relative_path.to_string(),
                root: self.allowed_root.display().to_string(),
            });
        }

        let full_path = self.allowed_root.join(relative_path);

        // If target already exists, verify its canonical path starts with allowed_root
        if full_path.exists() {
            let canonical = full_path
                .canonicalize()
                .map_err(|e| OfficeError::Io(e.to_string()))?;
            if !canonical.starts_with(&self.allowed_root) {
                return Err(OfficeError::PathTraversalAttempt {
                    path: relative_path.to_string(),
                    root: self.allowed_root.display().to_string(),
                });
            }
            Ok(canonical)
        } else {
            // For new files, verify parent path is within allowed_root
            if let Some(parent) = full_path.parent() {
                if parent.exists() {
                    let canon_parent = parent
                        .canonicalize()
                        .map_err(|e| OfficeError::Io(e.to_string()))?;
                    if !canon_parent.starts_with(&self.allowed_root) {
                        return Err(OfficeError::PathTraversalAttempt {
                            path: relative_path.to_string(),
                            root: self.allowed_root.display().to_string(),
                        });
                    }
                }
            }
            Ok(full_path)
        }
    }

    /// Read a file within the sandbox, enforcing `OfficeScope::FilesRead`.
    pub fn read_file(
        &self,
        relative_path: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<String> {
        if let Err(e) = scopes.check(OfficeScope::FilesRead) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Filesystem",
                "read_file",
                relative_path,
                user_id,
                OfficeScope::FilesRead.as_str(),
                AuditStatus::Denied,
                serde_json::json!({ "reason": e.to_string() }),
            ));
            return Err(e);
        }

        let safe_path = match self.resolve_safe_path(relative_path) {
            Ok(p) => p,
            Err(e) => {
                self.audit_trail.record(OfficeAuditEvent::new(
                    "Filesystem",
                    "read_file",
                    relative_path,
                    user_id,
                    OfficeScope::FilesRead.as_str(),
                    AuditStatus::Failed,
                    serde_json::json!({ "error": e.to_string() }),
                ));
                return Err(e);
            }
        };

        match fs::read_to_string(&safe_path) {
            Ok(content) => {
                self.audit_trail.record(OfficeAuditEvent::new(
                    "Filesystem",
                    "read_file",
                    relative_path,
                    user_id,
                    OfficeScope::FilesRead.as_str(),
                    AuditStatus::Success,
                    serde_json::json!({ "bytes_read": content.len() }),
                ));
                Ok(content)
            }
            Err(e) => {
                self.audit_trail.record(OfficeAuditEvent::new(
                    "Filesystem",
                    "read_file",
                    relative_path,
                    user_id,
                    OfficeScope::FilesRead.as_str(),
                    AuditStatus::Failed,
                    serde_json::json!({ "error": e.to_string() }),
                ));
                Err(OfficeError::NotFound(format!(
                    "Failed to read file '{}': {e}",
                    relative_path
                )))
            }
        }
    }

    /// Write content to a file within sandbox, enforcing `OfficeScope::FilesWrite`.
    pub fn write_file(
        &self,
        relative_path: &str,
        content: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<()> {
        if let Err(e) = scopes.check(OfficeScope::FilesWrite) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Filesystem",
                "write_file",
                relative_path,
                user_id,
                OfficeScope::FilesWrite.as_str(),
                AuditStatus::Denied,
                serde_json::json!({ "reason": e.to_string() }),
            ));
            return Err(e);
        }

        let safe_path = match self.resolve_safe_path(relative_path) {
            Ok(p) => p,
            Err(e) => {
                self.audit_trail.record(OfficeAuditEvent::new(
                    "Filesystem",
                    "write_file",
                    relative_path,
                    user_id,
                    OfficeScope::FilesWrite.as_str(),
                    AuditStatus::Failed,
                    serde_json::json!({ "error": e.to_string() }),
                ));
                return Err(e);
            }
        };

        if let Some(parent) = safe_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        match fs::write(&safe_path, content) {
            Ok(_) => {
                self.audit_trail.record(OfficeAuditEvent::new(
                    "Filesystem",
                    "write_file",
                    relative_path,
                    user_id,
                    OfficeScope::FilesWrite.as_str(),
                    AuditStatus::Success,
                    serde_json::json!({ "bytes_written": content.len() }),
                ));
                Ok(())
            }
            Err(e) => {
                self.audit_trail.record(OfficeAuditEvent::new(
                    "Filesystem",
                    "write_file",
                    relative_path,
                    user_id,
                    OfficeScope::FilesWrite.as_str(),
                    AuditStatus::Failed,
                    serde_json::json!({ "error": e.to_string() }),
                ));
                Err(OfficeError::Io(format!(
                    "Failed to write file '{}': {e}",
                    relative_path
                )))
            }
        }
    }

    /// List directory contents within sandbox.
    pub fn list_directory(
        &self,
        relative_path: &str,
        scopes: &ScopeSet,
        user_id: Option<&str>,
    ) -> Result<Vec<String>> {
        if let Err(e) = scopes.check(OfficeScope::FilesRead) {
            self.audit_trail.record(OfficeAuditEvent::new(
                "Filesystem",
                "list_directory",
                relative_path,
                user_id,
                OfficeScope::FilesRead.as_str(),
                AuditStatus::Denied,
                serde_json::json!({ "reason": e.to_string() }),
            ));
            return Err(e);
        }

        let safe_path = self.resolve_safe_path(relative_path)?;
        let read_dir = fs::read_dir(&safe_path)
            .map_err(|e| OfficeError::Io(format!("Failed to list directory: {e}")))?;

        let mut entries = Vec::new();
        for entry in read_dir.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                entries.push(name);
            }
        }
        entries.sort();

        self.audit_trail.record(OfficeAuditEvent::new(
            "Filesystem",
            "list_directory",
            relative_path,
            user_id,
            OfficeScope::FilesRead.as_str(),
            AuditStatus::Success,
            serde_json::json!({ "count": entries.len() }),
        ));

        Ok(entries)
    }
}
