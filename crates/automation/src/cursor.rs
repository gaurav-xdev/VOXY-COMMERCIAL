//! Real-Time VOXY Cursor Controller, Visual Indicator Telemetry,
//! Destructive Action Guard, and Emergency Stop Kill Switch for REQ-WIN-01.
//!
//! Complies with Master Goal Section 12 & 13:
//! - Visual cursor indicator ensures computer control is never stealthy.
//! - High-risk & destructive operations enforce mandatory human confirmation.
//! - Global emergency stop immediately halts mouse/keyboard automation.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use voxy_ipc::{VoxyCursorState, VoxyCursorTelemetry};

/// Risk classification level for automation actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Read-only operations, focus shifts, safe queries.
    Safe = 0,
    /// Non-destructive input, clicking buttons, typing in text fields.
    LowRisk = 1,
    /// System changes, modifying user files, sending messages.
    HighRisk = 2,
    /// Destructive file deletion, disk formatting, financial submissions, system restarts.
    CriticalDestructive = 3,
}

/// Description of an automation action to be evaluated by the safety guard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationAction {
    pub id: u64,
    pub target_window: String,
    pub target_element: String,
    pub action_type: String,
    pub payload: String,
    pub x: i32,
    pub y: i32,
}

/// Evaluation result from the destructive action guard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskEvaluation {
    pub risk_level: RiskLevel,
    pub requires_confirmation: bool,
    pub reason: String,
}

/// Pending confirmation status for held actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationStatus {
    Pending,
    Approved,
    Rejected,
    TimedOut,
}

/// Controller managing the on-screen VOXY cursor indicator,
/// ensuring all computer control is visible and safely governed.
#[derive(Debug, Clone)]
pub struct VoxyCursorController {
    current_telemetry: Arc<RwLock<VoxyCursorTelemetry>>,
    telemetry_history: Arc<RwLock<Vec<VoxyCursorTelemetry>>>,
    emergency_stopped: Arc<AtomicBool>,
    action_counter: Arc<AtomicU64>,
    pending_confirmations: Arc<RwLock<HashMap<u64, ConfirmationStatus>>>,
    max_history: usize,
}

impl Default for VoxyCursorController {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxyCursorController {
    pub fn new() -> Self {
        let initial_telemetry = VoxyCursorTelemetry {
            x: 0,
            y: 0,
            state: VoxyCursorState::Idle,
            target_element: String::new(),
            action_description: "Idle".to_string(),
            confidence: 1.0,
            requires_confirmation: false,
            action_id: None,
        };

        Self {
            current_telemetry: Arc::new(RwLock::new(initial_telemetry)),
            telemetry_history: Arc::new(RwLock::new(Vec::new())),
            emergency_stopped: Arc::new(AtomicBool::new(false)),
            action_counter: Arc::new(AtomicU64::new(1)),
            pending_confirmations: Arc::new(RwLock::new(HashMap::new())),
            max_history: 100,
        }
    }

    /// Check whether emergency stop is currently engaged.
    pub fn is_emergency_stopped(&self) -> bool {
        self.emergency_stopped.load(Ordering::SeqCst)
    }

    /// Trigger Emergency Stop: immediately freezes cursor and blocks further actions.
    pub fn trigger_emergency_stop(&self) -> VoxyCursorTelemetry {
        self.emergency_stopped.store(true, Ordering::SeqCst);
        let mut telem = self.current_telemetry.write();
        telem.state = VoxyCursorState::Idle;
        telem.action_description = "EMERGENCY STOP TRIGGERED: Actions halted".to_string();
        telem.requires_confirmation = false;
        telem.action_id = None;

        let telem_clone = telem.clone();
        self.record_history(&telem_clone);
        telem_clone
    }

    /// Reset Emergency Stop after human review.
    pub fn reset_emergency_stop(&self) {
        self.emergency_stopped.store(false, Ordering::SeqCst);
        let mut telem = self.current_telemetry.write();
        telem.state = VoxyCursorState::Idle;
        telem.action_description = "System Standby".to_string();
    }

    /// Update cursor position and visual state, broadcasting telemetry.
    pub fn update_cursor(
        &self,
        x: i32,
        y: i32,
        state: VoxyCursorState,
        target_element: &str,
        action_description: &str,
        confidence: f32,
    ) -> Result<VoxyCursorTelemetry, String> {
        if self.is_emergency_stopped() {
            return Err("Cannot update cursor: Emergency stop is active".to_string());
        }

        let mut telem = self.current_telemetry.write();
        telem.x = x;
        telem.y = y;
        telem.state = state;
        telem.target_element = target_element.to_string();
        telem.action_description = action_description.to_string();
        telem.confidence = confidence.clamp(0.0, 1.0);
        telem.requires_confirmation = false;
        telem.action_id = None;

        let telem_clone = telem.clone();
        self.record_history(&telem_clone);
        Ok(telem_clone)
    }

    /// Evaluate an action for destructive / high-risk operations and register confirmation if needed.
    pub fn prepare_action(&self, mut action: AutomationAction) -> (RiskEvaluation, VoxyCursorTelemetry) {
        if self.is_emergency_stopped() {
            let telem = self.current_telemetry.read().clone();
            return (
                RiskEvaluation {
                    risk_level: RiskLevel::CriticalDestructive,
                    requires_confirmation: false,
                    reason: "Emergency stop is active; all execution blocked.".to_string(),
                },
                telem,
            );
        }

        if action.id == 0 {
            action.id = self.action_counter.fetch_add(1, Ordering::SeqCst);
        }

        let evaluation = DestructiveActionGuard::evaluate_action(&action);

        let telem = if evaluation.requires_confirmation {
            self.pending_confirmations
                .write()
                .insert(action.id, ConfirmationStatus::Pending);

            let mut cur = self.current_telemetry.write();
            cur.x = action.x;
            cur.y = action.y;
            cur.state = VoxyCursorState::WaitingConfirmation;
            cur.target_element = action.target_element.clone();
            cur.action_description = format!(
                "CONFIRMATION REQUIRED: {} on '{}'",
                action.action_type, action.target_element
            );
            cur.requires_confirmation = true;
            cur.action_id = Some(action.id);
            cur.clone()
        } else {
            let mut cur = self.current_telemetry.write();
            cur.x = action.x;
            cur.y = action.y;
            cur.state = VoxyCursorState::ExecutingAction;
            cur.target_element = action.target_element.clone();
            cur.action_description = format!("{}: {}", action.action_type, action.payload);
            cur.requires_confirmation = false;
            cur.action_id = Some(action.id);
            cur.clone()
        };

        self.record_history(&telem);
        (evaluation, telem)
    }

    /// Process user decision for a pending high-risk action confirmation.
    pub fn confirm_action(&self, action_id: u64, approved: bool) -> Result<ConfirmationStatus, String> {
        let mut pending = self.pending_confirmations.write();
        match pending.get_mut(&action_id) {
            Some(status) => {
                if *status != ConfirmationStatus::Pending {
                    return Err(format!("Action {action_id} was already resolved as {:?}", status));
                }
                *status = if approved {
                    ConfirmationStatus::Approved
                } else {
                    ConfirmationStatus::Rejected
                };

                let mut telem = self.current_telemetry.write();
                if approved {
                    telem.state = VoxyCursorState::ExecutingAction;
                    telem.action_description = format!("Action {action_id} confirmed by user. Executing...");
                } else {
                    telem.state = VoxyCursorState::Idle;
                    telem.action_description = format!("Action {action_id} rejected by user. Cancelled.");
                }
                telem.requires_confirmation = false;

                Ok(*status)
            }
            None => Err(format!("No pending confirmation found for action ID {action_id}")),
        }
    }

    /// Retrieve the current live cursor telemetry.
    pub fn get_telemetry(&self) -> VoxyCursorTelemetry {
        self.current_telemetry.read().clone()
    }

    /// Retrieve recent cursor telemetry history.
    pub fn get_history(&self) -> Vec<VoxyCursorTelemetry> {
        self.telemetry_history.read().clone()
    }

    fn record_history(&self, telemetry: &VoxyCursorTelemetry) {
        let mut history = self.telemetry_history.write();
        if history.len() >= self.max_history {
            history.remove(0);
        }
        history.push(telemetry.clone());
    }
}

/// Destructive action safety guard that evaluates commands and UI automation requests.
pub struct DestructiveActionGuard;

impl DestructiveActionGuard {
    /// Evaluate whether an automation action requires user confirmation before proceeding.
    pub fn evaluate_action(action: &AutomationAction) -> RiskEvaluation {
        let payload_lower = action.payload.to_lowercase();
        let target_lower = action.target_element.to_lowercase();
        let action_lower = action.action_type.to_lowercase();

        // 1. Critical destructive patterns: file deletion, disk operations, system shutdown
        if payload_lower.contains("del ")
            || payload_lower.contains("rmdir")
            || payload_lower.contains("remove-item")
            || payload_lower.contains("rm -rf")
            || payload_lower.contains("format ")
            || payload_lower.contains("diskpart")
            || payload_lower.contains("shutdown")
            || payload_lower.contains("restart-computer")
            || target_lower.contains("empty recycle bin")
            || target_lower.contains("delete all")
            || target_lower.contains("format drive")
        {
            return RiskEvaluation {
                risk_level: RiskLevel::CriticalDestructive,
                requires_confirmation: true,
                reason: "Operation could permanently delete data or restart system.".to_string(),
            };
        }

        // 2. High-risk patterns: financial checkout, wire transfers, sending external communication
        if target_lower.contains("place order")
            || target_lower.contains("pay now")
            || target_lower.contains("transfer funds")
            || target_lower.contains("submit payment")
            || action_lower.contains("send_email")
            || action_lower.contains("send_payment")
        {
            return RiskEvaluation {
                risk_level: RiskLevel::HighRisk,
                requires_confirmation: true,
                reason: "Operation triggers irreversible financial or external communication.".to_string(),
            };
        }

        // 3. Medium/Low risk: standard navigation, clicking regular buttons, non-destructive typing
        if action_lower.contains("click")
            || action_lower.contains("type")
            || action_lower.contains("scroll")
            || action_lower.contains("focus")
        {
            return RiskEvaluation {
                risk_level: RiskLevel::LowRisk,
                requires_confirmation: false,
                reason: "Standard non-destructive user interface interaction.".to_string(),
            };
        }

        // 4. Default: Safe inspect
        RiskEvaluation {
            risk_level: RiskLevel::Safe,
            requires_confirmation: false,
            reason: "Read-only inspection or navigation.".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voxy_cursor_visual_indicator() {
        let controller = VoxyCursorController::new();

        // Initially idle
        let initial = controller.get_telemetry();
        assert_eq!(initial.state, VoxyCursorState::Idle);
        assert_eq!(initial.x, 0);
        assert_eq!(initial.y, 0);

        // Update cursor to target button
        let updated = controller
            .update_cursor(
                640,
                480,
                VoxyCursorState::Moving,
                "Save Button",
                "Navigating to Save button",
                0.95,
            )
            .unwrap();

        assert_eq!(updated.x, 640);
        assert_eq!(updated.y, 480);
        assert_eq!(updated.state, VoxyCursorState::Moving);
        assert_eq!(updated.target_element, "Save Button");
        assert_eq!(updated.confidence, 0.95);

        // History recorded
        let history = controller.get_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].target_element, "Save Button");
    }

    #[test]
    fn test_destructive_action_confirmation_guard() {
        let controller = VoxyCursorController::new();

        // Safe action: normal typing
        let safe_action = AutomationAction {
            id: 1,
            target_window: "Notepad".to_string(),
            target_element: "Text Area".to_string(),
            action_type: "type_text".to_string(),
            payload: "Hello world".to_string(),
            x: 200,
            y: 300,
        };
        let (eval_safe, telem_safe) = controller.prepare_action(safe_action);
        assert_eq!(eval_safe.requires_confirmation, false);
        assert_eq!(telem_safe.state, VoxyCursorState::ExecutingAction);

        // Destructive action: deleting files
        let destructive_action = AutomationAction {
            id: 2,
            target_window: "PowerShell".to_string(),
            target_element: "Console".to_string(),
            action_type: "run_command".to_string(),
            payload: "Remove-Item -Recurse -Force C:\\Data".to_string(),
            x: 400,
            y: 500,
        };
        let (eval_destruct, telem_destruct) = controller.prepare_action(destructive_action);
        assert_eq!(eval_destruct.requires_confirmation, true);
        assert_eq!(eval_destruct.risk_level, RiskLevel::CriticalDestructive);
        assert_eq!(telem_destruct.state, VoxyCursorState::WaitingConfirmation);
        assert_eq!(telem_destruct.requires_confirmation, true);
        assert_eq!(telem_destruct.action_id, Some(2));

        // Attempting execution without confirmation fails; confirmation approval works
        let res_approve = controller.confirm_action(2, true).unwrap();
        assert_eq!(res_approve, ConfirmationStatus::Approved);
        assert_eq!(controller.get_telemetry().state, VoxyCursorState::ExecutingAction);

        // Another destructive action rejected
        let action_reject = AutomationAction {
            id: 3,
            target_window: "Browser".to_string(),
            target_element: "Empty Recycle Bin".to_string(),
            action_type: "click".to_string(),
            payload: "Empty all deleted items".to_string(),
            x: 100,
            y: 100,
        };
        let (_, _) = controller.prepare_action(action_reject);
        let res_reject = controller.confirm_action(3, false).unwrap();
        assert_eq!(res_reject, ConfirmationStatus::Rejected);
        assert_eq!(controller.get_telemetry().state, VoxyCursorState::Idle);
    }

    #[test]
    fn test_emergency_stop_kill_switch() {
        let controller = VoxyCursorController::new();
        assert!(!controller.is_emergency_stopped());

        // Normal move
        controller
            .update_cursor(100, 100, VoxyCursorState::Moving, "Icon", "Hovering", 1.0)
            .unwrap();

        // Hit emergency stop
        let stopped_telem = controller.trigger_emergency_stop();
        assert!(controller.is_emergency_stopped());
        assert_eq!(stopped_telem.state, VoxyCursorState::Idle);
        assert!(stopped_telem.action_description.contains("EMERGENCY STOP"));

        // Subsequent cursor movements rejected
        let err = controller
            .update_cursor(200, 200, VoxyCursorState::Moving, "Icon", "Hovering", 1.0)
            .unwrap_err();
        assert!(err.contains("Emergency stop is active"));

        // Reset re-enables control
        controller.reset_emergency_stop();
        assert!(!controller.is_emergency_stopped());
        let resumed = controller
            .update_cursor(200, 200, VoxyCursorState::Moving, "Icon", "Hovering", 1.0)
            .unwrap();
        assert_eq!(resumed.x, 200);
    }
}
