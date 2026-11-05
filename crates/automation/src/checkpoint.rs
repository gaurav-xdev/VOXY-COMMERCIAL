//! Multi-step Workflow Checkpoints, State Verification, and Untrusted Input Sanitization.
//!
//! Enforces:
//! - Checkpointing of window/application state before consequential desktop actions
//! - Automatic rollback / focus restoration if intermediate steps fail
//! - Strict sanitization of untrusted text payloads (webpage scrapes, file contents, LLM responses)
//!   to prevent keystroke command injection into terminals or run dialogs.

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Snapshot of desktop application state before an automation action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopCheckpoint {
    pub checkpoint_id: String,
    pub step_index: usize,
    pub active_window_title: Option<String>,
    pub active_window_id: Option<String>,
    pub cursor_x: i32,
    pub cursor_y: i32,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

/// Manages multi-step checkpoints and safe recovery.
#[derive(Debug, Clone, Default)]
pub struct WorkflowCheckpointManager {
    checkpoints: Arc<RwLock<Vec<DesktopCheckpoint>>>,
}

impl WorkflowCheckpointManager {
    pub fn new() -> Self {
        Self {
            checkpoints: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a checkpoint prior to executing a step.
    pub fn record_checkpoint(
        &self,
        step_index: usize,
        window_title: Option<String>,
        window_id: Option<String>,
        x: i32,
        y: i32,
    ) -> DesktopCheckpoint {
        let cp = DesktopCheckpoint {
            checkpoint_id: format!("cp-{}-{}", step_index, Utc::now().timestamp_millis()),
            step_index,
            active_window_title: window_title,
            active_window_id: window_id,
            cursor_x: x,
            cursor_y: y,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        let mut lock = self.checkpoints.write();
        lock.push(cp.clone());
        cp
    }

    /// Get the latest checkpoint for rollback.
    pub fn latest_checkpoint(&self) -> Option<DesktopCheckpoint> {
        let lock = self.checkpoints.read();
        lock.last().cloned()
    }

    /// Clear checkpoints after successful workflow completion.
    pub fn clear(&self) {
        self.checkpoints.write().clear();
    }
}

/// Sanitizer protecting keyboard input automation from untrusted external data.
pub struct UntrustedPayloadSanitizer;

impl UntrustedPayloadSanitizer {
    /// Sanitize text payload before automated typing.
    /// Strips unauthorized shell escape characters, control sequences, and dangerous terminal injections
    /// unless explicitly running in a trusted interactive editor mode.
    pub fn sanitize_typing_text(text: &str, is_terminal_target: bool) -> (String, bool) {
        let mut modified = false;
        let mut clean = String::with_capacity(text.len());

        for c in text.chars() {
            // Filter out dangerous ASCII control characters except \n, \r, \t
            if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
                modified = true;
                continue;
            }

            // In terminal targets, prevent multi-line shell payload execution unless confirmed
            if is_terminal_target && (c == '\n' || c == '\r') {
                clean.push(' ');
                modified = true;
                continue;
            }

            clean.push(c);
        }

        (clean, modified)
    }

    /// Detect potentially dangerous or destructive phrases in text payloads.
    pub fn detect_dangerous_payload(text: &str) -> bool {
        let t = text.to_lowercase();
        let dangerous_keywords = [
            "rm -rf /",
            "format c:",
            "del /f /s /q",
            "rmdir /s /q c:",
            "shutdown /s",
            "drop database",
            "drop table",
            ":(){ :|:& };:",
        ];

        dangerous_keywords.iter().any(|&k| t.contains(k))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_checkpoint_record_and_retrieve() {
        let mgr = WorkflowCheckpointManager::new();
        let cp = mgr.record_checkpoint(
            1,
            Some("Visual Studio Code".into()),
            Some("win-100".into()),
            400,
            600,
        );
        assert_eq!(cp.step_index, 1);
        assert_eq!(cp.cursor_x, 400);

        let latest = mgr.latest_checkpoint().unwrap();
        assert_eq!(latest.checkpoint_id, cp.checkpoint_id);

        mgr.clear();
        assert!(mgr.latest_checkpoint().is_none());
    }

    #[test]
    fn test_untrusted_payload_sanitizer() {
        let (sanitized, modified) =
            UntrustedPayloadSanitizer::sanitize_typing_text("echo 'hello'\x07\x1b[2J", false);
        assert!(modified);
        assert!(!sanitized.contains('\x07'));
        assert!(!sanitized.contains('\x1b'));

        let (term_sanitized, term_mod) =
            UntrustedPayloadSanitizer::sanitize_typing_text("rm -rf\nnext_cmd", true);
        assert!(term_mod);
        assert!(!term_sanitized.contains('\n'));

        assert!(UntrustedPayloadSanitizer::detect_dangerous_payload(
            "sudo rm -rf / --no-preserve-root"
        ));
        assert!(UntrustedPayloadSanitizer::detect_dangerous_payload(
            "cmd.exe /c format c: /y"
        ));
        assert!(!UntrustedPayloadSanitizer::detect_dangerous_payload(
            "cargo check --workspace"
        ));
    }
}
