//! Computer-Use & Agentic Operation Loop
//!
//! Provides the production agentic loop: Observe → Plan → Act → Verify → Replan
//! with full human-in-the-loop governance (ApprovalBroker), safety boundaries,
//! emergency stop kill-switch, pause/resume, and user takeover.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{info, warn};
use uuid::Uuid;

use crate::backends::verification::VerificationEngine;
use crate::cursor::{RiskLevel, VoxyCursorController};
use crate::error::{action_err, timeout_err, Result};
use voxy_orchestrator::automation::{
    AutomationBackend, ElementSelector, MouseButton, StateVerification, WindowTarget,
};
use voxy_security::{ApprovalBroker, ApprovalRiskLevel, ApprovalStatus};

/// State machine for an agentic operation loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopState {
    Idle,
    Observing,
    Planning,
    AwaitingApproval,
    Acting,
    Verifying,
    Replanning,
    Paused,
    Completed,
    Failed,
    Cancelled,
    EmergencyStopped,
}

impl LoopState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Cancelled | Self::EmergencyStopped
        )
    }
}

/// Action to be performed in the "Act" stage of the loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action_type", content = "params")]
pub enum AgentAction {
    Click {
        x: i32,
        y: i32,
        button: MouseButton,
    },
    DoubleClick {
        x: i32,
        y: i32,
    },
    MoveMouse {
        x: i32,
        y: i32,
    },
    Drag {
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
    },
    Scroll {
        x: i32,
        y: i32,
        delta_x: i32,
        delta_y: i32,
    },
    TypeText {
        text: String,
        interval_ms: u64,
    },
    KeyPress {
        key: String,
    },
    KeyCombination {
        keys: Vec<String>,
    },
    FocusWindow {
        window_id: String,
    },
    CustomTool {
        name: String,
        parameters: serde_json::Value,
    },
}

/// Specification for action verification.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActionVerification {
    pub expected_window_title: Option<String>,
    pub expected_element: Option<ElementSelector>,
    pub expected_text_visible: Option<String>,
    pub timeout_ms: u64,
}

/// A planned single step in the agentic operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedStep {
    pub step_number: usize,
    pub description: String,
    pub action: AgentAction,
    pub risk_level: RiskLevel,
    pub verification: Option<ActionVerification>,
    pub max_retries: usize,
}

/// Observation result from the current desktop environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopObservation {
    pub timestamp: DateTime<Utc>,
    pub active_window: Option<WindowTarget>,
    pub screen_width: u32,
    pub screen_height: u32,
    pub visible_windows: Vec<WindowTarget>,
    pub context_summary: String,
}

/// Progress event emitted over broadcast channels during loop execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationProgress {
    pub operation_id: Uuid,
    pub state: LoopState,
    pub current_step: usize,
    pub total_steps: usize,
    pub step_description: String,
    pub elapsed_ms: u64,
    pub message: String,
}

/// Step execution record stored in history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRecord {
    pub step_number: usize,
    pub description: String,
    pub state: LoopState,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub retries_taken: usize,
}

/// Configuration settings for the Agentic Operation Loop.
#[derive(Debug, Clone)]
pub struct OperationLoopConfig {
    pub max_steps: usize,
    pub per_step_timeout: Duration,
    pub global_timeout: Duration,
    pub default_retries: usize,
    pub retry_backoff: Duration,
    pub auto_verify: bool,
}

impl Default for OperationLoopConfig {
    fn default() -> Self {
        Self {
            max_steps: 30,
            per_step_timeout: Duration::from_secs(30),
            global_timeout: Duration::from_secs(600),
            default_retries: 2,
            retry_backoff: Duration::from_millis(500),
            auto_verify: true,
        }
    }
}

/// Planner trait to allow pluggable decision-making (heuristic, rule-based, or LLM-driven).
#[async_trait]
pub trait AgentPlanner: Send + Sync {
    /// Initial plan given task goal and observation.
    async fn plan(
        &self,
        goal: &str,
        observation: &DesktopObservation,
    ) -> Result<Vec<PlannedStep>>;

    /// Replan given task goal, observation, failed step, and error reason.
    async fn replan(
        &self,
        goal: &str,
        observation: &DesktopObservation,
        failed_step: &PlannedStep,
        error_reason: &str,
    ) -> Result<Vec<PlannedStep>>;
}

/// Computer-Use & Agentic Operation Loop Engine.
pub struct AgenticOperationLoop {
    id: Uuid,
    goal: String,
    config: OperationLoopConfig,
    state: Arc<RwLock<LoopState>>,
    backend: Arc<dyn AutomationBackend>,
    planner: Arc<dyn AgentPlanner>,
    approval_broker: Option<Arc<ApprovalBroker>>,
    cursor_controller: Option<Arc<VoxyCursorController>>,
    progress_tx: broadcast::Sender<OperationProgress>,
    history: Arc<RwLock<Vec<StepRecord>>>,
    paused_flag: Arc<AtomicBool>,
    cancelled_flag: Arc<AtomicBool>,
    emergency_stop_flag: Arc<AtomicBool>,
}

impl AgenticOperationLoop {
    pub fn new(
        goal: impl Into<String>,
        backend: Arc<dyn AutomationBackend>,
        planner: Arc<dyn AgentPlanner>,
        config: OperationLoopConfig,
    ) -> Self {
        let (progress_tx, _) = broadcast::channel(128);
        Self {
            id: Uuid::new_v4(),
            goal: goal.into(),
            config,
            state: Arc::new(RwLock::new(LoopState::Idle)),
            backend,
            planner,
            approval_broker: None,
            cursor_controller: None,
            progress_tx,
            history: Arc::new(RwLock::new(Vec::new())),
            paused_flag: Arc::new(AtomicBool::new(false)),
            cancelled_flag: Arc::new(AtomicBool::new(false)),
            emergency_stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_approval_broker(mut self, broker: Arc<ApprovalBroker>) -> Self {
        self.approval_broker = Some(broker);
        self
    }

    pub fn with_cursor_controller(mut self, cursor: Arc<VoxyCursorController>) -> Self {
        self.cursor_controller = Some(cursor);
        self
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn state(&self) -> LoopState {
        *self.state.read()
    }

    pub fn subscribe_progress(&self) -> broadcast::Receiver<OperationProgress> {
        self.progress_tx.subscribe()
    }

    pub fn get_history(&self) -> Vec<StepRecord> {
        self.history.read().clone()
    }

    pub fn pause(&self) {
        self.paused_flag.store(true, Ordering::SeqCst);
        let should_emit = {
            let mut st = self.state.write();
            if !st.is_terminal() {
                *st = LoopState::Paused;
                true
            } else {
                false
            }
        };
        if should_emit {
            self.emit_progress(0, 0, "Operation paused by user", "Paused");
        }
    }

    pub fn resume(&self) {
        self.paused_flag.store(false, Ordering::SeqCst);
        let should_emit = {
            let mut st = self.state.write();
            if *st == LoopState::Paused {
                *st = LoopState::Observing;
                true
            } else {
                false
            }
        };
        if should_emit {
            self.emit_progress(0, 0, "Operation resumed", "Resuming");
        }
    }

    pub fn cancel(&self) {
        self.cancelled_flag.store(true, Ordering::SeqCst);
        {
            let mut st = self.state.write();
            *st = LoopState::Cancelled;
        }
        self.emit_progress(0, 0, "Operation cancelled by user", "Cancelled");
    }

    pub fn emergency_stop(&self) {
        self.emergency_stop_flag.store(true, Ordering::SeqCst);
        if let Some(ref cc) = self.cursor_controller {
            cc.trigger_emergency_stop();
        }
        if let Some(ref ab) = self.approval_broker {
            ab.emergency_stop_all();
        }
        {
            let mut st = self.state.write();
            *st = LoopState::EmergencyStopped;
        }
        self.emit_progress(0, 0, "Global Emergency Stop triggered", "Emergency Stop");
    }

    fn emit_progress(&self, current: usize, total: usize, desc: &str, msg: &str) {
        let current_state = *self.state.read();
        let p = OperationProgress {
            operation_id: self.id,
            state: current_state,
            current_step: current,
            total_steps: total,
            step_description: desc.to_string(),
            elapsed_ms: 0,
            message: msg.to_string(),
        };
        let _ = self.progress_tx.send(p);
    }

    /// Primary execution loop: Observe → Plan → Act → Verify → Replan.
    pub async fn run(&self) -> Result<()> {
        let global_start = Instant::now();

        if self.emergency_stop_flag.load(Ordering::SeqCst) {
            self.set_state(LoopState::EmergencyStopped);
            return Err(action_err("Emergency Stop triggered"));
        }

        if self.cancelled_flag.load(Ordering::SeqCst) {
            self.set_state(LoopState::Cancelled);
            return Err(action_err("Operation cancelled"));
        }

        self.set_state(LoopState::Observing);
        self.emit_progress(0, 0, "Observing initial desktop state", "Starting task");

        // Step 1: Initial Observation
        let mut observation = self.observe().await?;

        // Step 2: Initial Plan
        self.set_state(LoopState::Planning);
        let mut steps = self.planner.plan(&self.goal, &observation).await?;
        if steps.is_empty() {
            self.set_state(LoopState::Completed);
            self.emit_progress(0, 0, "No actions required", "Completed");
            return Ok(());
        }

        let mut step_index = 0;
        let mut total_steps_executed = 0;

        while step_index < steps.len() {
            // Check global safety boundaries
            if self.emergency_stop_flag.load(Ordering::SeqCst) {
                self.set_state(LoopState::EmergencyStopped);
                return Err(action_err("Emergency Stop triggered"));
            }

            if self.cancelled_flag.load(Ordering::SeqCst) {
                self.set_state(LoopState::Cancelled);
                return Err(action_err("Operation cancelled"));
            }

            if global_start.elapsed() > self.config.global_timeout {
                self.set_state(LoopState::Failed);
                return Err(timeout_err(format!(
                    "Global operation timeout exceeded ({}s)",
                    self.config.global_timeout.as_secs()
                )));
            }

            if total_steps_executed >= self.config.max_steps {
                self.set_state(LoopState::Failed);
                return Err(action_err(format!(
                    "Maximum step count ({}) exceeded without completion",
                    self.config.max_steps
                )));
            }

            // Check pause
            while self.paused_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(200)).await;
                if self.cancelled_flag.load(Ordering::SeqCst)
                    || self.emergency_stop_flag.load(Ordering::SeqCst)
                {
                    break;
                }
            }

            let current_step = steps[step_index].clone();
            total_steps_executed += 1;

            let mut record = StepRecord {
                step_number: current_step.step_number,
                description: current_step.description.clone(),
                state: LoopState::Acting,
                started_at: Utc::now(),
                completed_at: None,
                error: None,
                retries_taken: 0,
            };

            self.emit_progress(
                step_index + 1,
                steps.len(),
                &current_step.description,
                "Executing step",
            );

            // 1. Approval Gate
            if self.requires_approval(&current_step) {
                self.set_state(LoopState::AwaitingApproval);
                self.emit_progress(
                    step_index + 1,
                    steps.len(),
                    &current_step.description,
                    "Awaiting human authorization",
                );

                let approved = self.request_approval(&current_step).await?;
                if !approved {
                    self.set_state(LoopState::Cancelled);
                    record.error = Some("Denied by user".to_string());
                    record.completed_at = Some(Utc::now());
                    self.history.write().push(record);
                    return Err(action_err("Action denied by human authority"));
                }
            }

            // 2. Act
            self.set_state(LoopState::Acting);
            let mut retries = 0;
            let mut act_success = false;

            while retries <= current_step.max_retries {
                let step_res = tokio::time::timeout(
                    self.config.per_step_timeout,
                    self.execute_action(&current_step.action),
                )
                .await;

                match step_res {
                    Ok(Ok(())) => {
                        // 3. Verify
                        if self.config.auto_verify && current_step.verification.is_some() {
                            self.set_state(LoopState::Verifying);
                            let verified = self
                                .verify_step(current_step.verification.as_ref().unwrap())
                                .await?;
                            if verified {
                                act_success = true;
                                break;
                            } else {
                                warn!(
                                    "Verification failed for step {} (attempt {}/{})",
                                    current_step.step_number,
                                    retries + 1,
                                    current_step.max_retries
                                );
                            }
                        } else {
                            act_success = true;
                            break;
                        }
                    }
                    Ok(Err(e)) => {
                        warn!(
                            "Action failed for step {} (attempt {}/{}): {}",
                            current_step.step_number,
                            retries + 1,
                            current_step.max_retries,
                            e
                        );
                    }
                    Err(_) => {
                        warn!(
                            "Action timed out for step {} (attempt {}/{})",
                            current_step.step_number,
                            retries + 1,
                            current_step.max_retries
                        );
                    }
                }

                retries += 1;
                record.retries_taken = retries;
                if retries <= current_step.max_retries {
                    tokio::time::sleep(self.config.retry_backoff).await;
                }
            }

            if act_success {
                record.state = LoopState::Completed;
                record.completed_at = Some(Utc::now());
                self.history.write().push(record);
                step_index += 1;
            } else {
                // 4. Replan
                self.set_state(LoopState::Replanning);
                self.emit_progress(
                    step_index + 1,
                    steps.len(),
                    &current_step.description,
                    "Step failed; replanning remaining operations",
                );

                observation = self.observe().await?;
                let replanned = self
                    .planner
                    .replan(
                        &self.goal,
                        &observation,
                        &current_step,
                        "Verification or action execution failed",
                    )
                    .await?;

                if replanned.is_empty() {
                    record.state = LoopState::Failed;
                    record.error = Some("Failed and replanner produced no alternatives".into());
                    record.completed_at = Some(Utc::now());
                    self.history.write().push(record);

                    self.set_state(LoopState::Failed);
                    return Err(action_err(format!(
                        "Step {} failed and could not be replanned",
                        current_step.step_number
                    )));
                }

                // Replace remaining steps with replanned actions
                steps = replanned;
                step_index = 0;
            }
        }

        self.set_state(LoopState::Completed);
        self.emit_progress(steps.len(), steps.len(), "All steps completed successfully", "Completed");
        Ok(())
    }

    /// Observe current active desktop state and windows.
    pub async fn observe(&self) -> Result<DesktopObservation> {
        let (width, height) = self.backend.screen_size().await.unwrap_or((1920, 1080));
        let active = self.backend.get_active_window().await.ok();
        let visible = self.backend.find_window("", None).await.unwrap_or_default();

        let context_summary = format!(
            "Resolution: {}x{}, Active: {}, Visible Windows: {}",
            width,
            height,
            active
                .as_ref()
                .map(|w| w.title.as_str())
                .unwrap_or("None"),
            visible.len()
        );

        Ok(DesktopObservation {
            timestamp: Utc::now(),
            active_window: active,
            screen_width: width,
            screen_height: height,
            visible_windows: visible,
            context_summary,
        })
    }

    fn requires_approval(&self, step: &PlannedStep) -> bool {
        match step.risk_level {
            RiskLevel::HighRisk | RiskLevel::CriticalDestructive => true,
            _ => false,
        }
    }

    async fn request_approval(&self, step: &PlannedStep) -> Result<bool> {
        if let Some(ref broker) = self.approval_broker {
            let risk_level = match step.risk_level {
                RiskLevel::CriticalDestructive => ApprovalRiskLevel::Destructive,
                RiskLevel::HighRisk => ApprovalRiskLevel::Privileged,
                RiskLevel::LowRisk => ApprovalRiskLevel::Modify,
                RiskLevel::Safe => ApprovalRiskLevel::Safe,
            };

            let params = serde_json::to_value(&step.action)
                .unwrap_or_else(|_| serde_json::json!({ "description": step.description }));

            let (_req, rx) = broker.submit_request(
                self.id.to_string(),
                format!("agent_step_{}", step.step_number),
                &params,
                risk_level,
                format!("Computer-Use step requires authorization: {}", step.description),
                Duration::from_secs(45),
            );

            match rx.await {
                Ok(decision) => Ok(decision.status == ApprovalStatus::Approved),
                Err(_) => Ok(false),
            }
        } else {
            // If no broker attached, fail closed on high-risk operations
            Ok(false)
        }
    }

    async fn execute_action(&self, action: &AgentAction) -> Result<()> {
        match action {
            AgentAction::Click { x, y, button } => {
                if let Some(ref cc) = self.cursor_controller {
                    let _ = cc.update_cursor(*x, *y, voxy_ipc::VoxyCursorState::Clicking, "Target", "Clicking", 1.0);
                }
                self.backend.click(*x, *y, *button).await
            }
            AgentAction::DoubleClick { x, y } => {
                if let Some(ref cc) = self.cursor_controller {
                    let _ = cc.update_cursor(*x, *y, voxy_ipc::VoxyCursorState::Clicking, "Target", "Double-Clicking", 1.0);
                }
                self.backend.double_click(*x, *y).await
            }
            AgentAction::MoveMouse { x, y } => {
                if let Some(ref cc) = self.cursor_controller {
                    let _ = cc.update_cursor(*x, *y, voxy_ipc::VoxyCursorState::Moving, "Target", "Moving Mouse", 1.0);
                }
                self.backend.move_mouse(*x, *y).await
            }
            AgentAction::Drag {
                from_x,
                from_y,
                to_x,
                to_y,
            } => {
                self.backend.drag(*from_x, *from_y, *to_x, *to_y).await
            }
            AgentAction::Scroll {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                self.backend.scroll(*x, *y, *delta_x, *delta_y).await
            }
            AgentAction::TypeText { text, interval_ms } => {
                self.backend.type_text(text, *interval_ms).await
            }
            AgentAction::KeyPress { key } => {
                self.backend.key_press(key).await
            }
            AgentAction::KeyCombination { keys } => {
                let key_refs: Vec<&str> = keys.iter().map(|s| s.as_str()).collect();
                self.backend.key_combination(&key_refs).await
            }
            AgentAction::FocusWindow { window_id } => {
                self.backend.focus_window(window_id).await
            }
            AgentAction::CustomTool { name, parameters } => {
                info!("CustomTool action invoked: {} with {:?}", name, parameters);
                Ok(())
            }
        }
    }

    async fn verify_step(&self, verification: &ActionVerification) -> Result<bool> {
        let verification_engine = VerificationEngine::new(self.backend.clone());
        let expected = StateVerification {
            window_title: verification.expected_window_title.clone(),
            element_present: verification.expected_element.clone(),
            text_visible: verification.expected_text_visible.clone(),
            timeout_ms: verification.timeout_ms.max(500),
        };

        verification_engine.verify_window_state(&expected).await
    }

    fn set_state(&self, new_state: LoopState) {
        *self.state.write() = new_state;
    }
}

/// Simple heuristic planner for standard desktop operations and tests.
pub struct HeuristicAgentPlanner {
    mock_steps: Vec<PlannedStep>,
}

impl HeuristicAgentPlanner {
    pub fn new(mock_steps: Vec<PlannedStep>) -> Self {
        Self { mock_steps }
    }
}

#[async_trait]
impl AgentPlanner for HeuristicAgentPlanner {
    async fn plan(
        &self,
        _goal: &str,
        _observation: &DesktopObservation,
    ) -> Result<Vec<PlannedStep>> {
        Ok(self.mock_steps.clone())
    }

    async fn replan(
        &self,
        _goal: &str,
        _observation: &DesktopObservation,
        failed_step: &PlannedStep,
        _error_reason: &str,
    ) -> Result<Vec<PlannedStep>> {
        // Fallback: skip or adjust action
        let mut modified = failed_step.clone();
        modified.description = format!("Recovered: {}", failed_step.description);
        modified.verification = None; // Relax verification on retry
        Ok(vec![modified])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAutomationBackend;

    #[async_trait]
    impl AutomationBackend for MockAutomationBackend {
        async fn name(&self) -> &str {
            "mock-backend"
        }
        async fn initialize(&self) -> Result<()> {
            Ok(())
        }
        async fn shutdown(&self) -> Result<()> {
            Ok(())
        }
        async fn is_available(&self) -> bool {
            true
        }
        async fn click(&self, _x: i32, _y: i32, _button: MouseButton) -> Result<()> {
            Ok(())
        }
        async fn double_click(&self, _x: i32, _y: i32) -> Result<()> {
            Ok(())
        }
        async fn move_mouse(&self, _x: i32, _y: i32) -> Result<()> {
            Ok(())
        }
        async fn drag(&self, _from_x: i32, _from_y: i32, _to_x: i32, _to_y: i32) -> Result<()> {
            Ok(())
        }
        async fn scroll(&self, _x: i32, _y: i32, _delta_x: i32, _delta_y: i32) -> Result<()> {
            Ok(())
        }
        async fn type_text(&self, _text: &str, _interval_ms: u64) -> Result<()> {
            Ok(())
        }
        async fn key_press(&self, _key: &str) -> Result<()> {
            Ok(())
        }
        async fn key_combination(&self, _keys: &[&str]) -> Result<()> {
            Ok(())
        }
        async fn hold_key(&self, _key: &str, _duration_ms: u64) -> Result<()> {
            Ok(())
        }
        async fn screenshot(
            &self,
            _x: Option<i32>,
            _y: Option<i32>,
            _width: Option<u32>,
            _height: Option<u32>,
        ) -> Result<Vec<u8>> {
            Ok(vec![])
        }
        async fn screen_size(&self) -> Result<(u32, u32)> {
            Ok((1920, 1080))
        }
        async fn get_pixel_color(&self, _x: i32, _y: i32) -> Result<(u8, u8, u8)> {
            Ok((255, 255, 255))
        }
        async fn get_active_window(&self) -> Result<WindowTarget> {
            Ok(WindowTarget {
                id: "win-1".into(),
                title: "Calculator".into(),
                class_name: None,
                process_id: Some(100),
                bounds: voxy_shared::types::Rect::new(0, 0, 800, 600),
                is_visible: true,
                is_focused: true,
            })
        }
        async fn find_window(&self, _title: &str, _class: Option<&str>) -> Result<Vec<WindowTarget>> {
            Ok(vec![])
        }
        async fn focus_window(&self, _window_id: &str) -> Result<()> {
            Ok(())
        }
        async fn get_window_bounds(&self, _window_id: &str) -> Result<voxy_shared::types::Rect> {
            Ok(voxy_shared::types::Rect::new(0, 0, 800, 600))
        }
        async fn resize_window(&self, _window_id: &str, _width: u32, _height: u32) -> Result<()> {
            Ok(())
        }
        async fn move_window(&self, _window_id: &str, _x: i32, _y: i32) -> Result<()> {
            Ok(())
        }
        async fn close_window(&self, _window_id: &str) -> Result<()> {
            Ok(())
        }
        async fn minimize_window(&self, _window_id: &str) -> Result<()> {
            Ok(())
        }
        async fn maximize_window(&self, _window_id: &str) -> Result<()> {
            Ok(())
        }
        async fn restore_window(&self, _window_id: &str) -> Result<()> {
            Ok(())
        }
        async fn find_element(&self, _selector: &ElementSelector) -> Result<Vec<voxy_orchestrator::automation::ElementInfo>> {
            Ok(vec![])
        }
        async fn get_element_text(&self, _element_id: &str) -> Result<String> {
            Ok("OK".into())
        }
        async fn click_element(&self, _element_id: &str) -> Result<()> {
            Ok(())
        }
        async fn get_element_bounds(&self, _element_id: &str) -> Result<voxy_shared::types::Rect> {
            Ok(voxy_shared::types::Rect::new(0, 0, 100, 50))
        }
        async fn wait_for_element(&self, _selector: &ElementSelector, _timeout_ms: u64) -> Result<voxy_orchestrator::automation::ElementInfo> {
            Ok(voxy_orchestrator::automation::ElementInfo {
                id: "elem-1".into(),
                name: "Button".into(),
                control_type: "Button".into(),
                bounds: voxy_shared::types::Rect::new(0, 0, 100, 50),
                is_enabled: true,
                is_visible: true,
                text: Some("OK".into()),
                children: vec![],
            })
        }
        async fn ocr_region(&self, _image: &[u8], _language: Option<&str>) -> Result<String> {
            Ok("Text".into())
        }
        async fn find_text_on_screen(&self, _text: &str, _region: Option<voxy_shared::types::Rect>) -> Result<Vec<voxy_shared::types::Rect>> {
            Ok(vec![])
        }
        async fn verify_state(&self, _expected: &StateVerification) -> Result<bool> {
            Ok(true)
        }
        async fn recover(&self, _error: &str) -> Result<bool> {
            Ok(true)
        }
        async fn get_backend_capabilities(&self) -> Vec<voxy_orchestrator::automation::AutomationCapability> {
            vec![]
        }
    }

    #[tokio::test]
    async fn test_agentic_loop_successful_execution() {
        let backend = Arc::new(MockAutomationBackend);
        let steps = vec![
            PlannedStep {
                step_number: 1,
                description: "Move to start menu".to_string(),
                action: AgentAction::MoveMouse { x: 100, y: 100 },
                risk_level: RiskLevel::Safe,
                verification: None,
                max_retries: 1,
            },
            PlannedStep {
                step_number: 2,
                description: "Type search query".to_string(),
                action: AgentAction::TypeText {
                    text: "calculator".to_string(),
                    interval_ms: 10,
                },
                risk_level: RiskLevel::LowRisk,
                verification: None,
                max_retries: 1,
            },
        ];

        let planner = Arc::new(HeuristicAgentPlanner::new(steps));
        let loop_engine = AgenticOperationLoop::new(
            "Open calculator",
            backend,
            planner,
            OperationLoopConfig::default(),
        );

        let res = loop_engine.run().await;
        assert!(res.is_ok());
        assert_eq!(loop_engine.state(), LoopState::Completed);

        let history = loop_engine.get_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].state, LoopState::Completed);
        assert_eq!(history[1].state, LoopState::Completed);
    }

    #[tokio::test]
    async fn test_agentic_loop_cancellation() {
        let backend = Arc::new(MockAutomationBackend);
        let steps = vec![PlannedStep {
            step_number: 1,
            description: "Move mouse".to_string(),
            action: AgentAction::MoveMouse { x: 50, y: 50 },
            risk_level: RiskLevel::Safe,
            verification: None,
            max_retries: 0,
        }];

        let planner = Arc::new(HeuristicAgentPlanner::new(steps));
        let loop_engine = AgenticOperationLoop::new(
            "Test cancellation",
            backend,
            planner,
            OperationLoopConfig::default(),
        );

        loop_engine.cancel();
        assert_eq!(loop_engine.state(), LoopState::Cancelled);

        let res = loop_engine.run().await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_agentic_loop_emergency_stop() {
        let backend = Arc::new(MockAutomationBackend);
        let steps = vec![PlannedStep {
            step_number: 1,
            description: "Click".to_string(),
            action: AgentAction::Click {
                x: 10,
                y: 10,
                button: MouseButton::Left,
            },
            risk_level: RiskLevel::Safe,
            verification: None,
            max_retries: 0,
        }];

        let planner = Arc::new(HeuristicAgentPlanner::new(steps));
        let cursor_controller = Arc::new(VoxyCursorController::new());
        let loop_engine = AgenticOperationLoop::new(
            "Test emergency stop",
            backend,
            planner,
            OperationLoopConfig::default(),
        )
        .with_cursor_controller(cursor_controller.clone());

        loop_engine.emergency_stop();
        assert_eq!(loop_engine.state(), LoopState::EmergencyStopped);
        assert!(cursor_controller.is_emergency_stopped());

        let res = loop_engine.run().await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn test_agentic_loop_approval_broker_integration() {
        let backend = Arc::new(MockAutomationBackend);
        let broker = Arc::new(ApprovalBroker::new());

        let steps = vec![PlannedStep {
            step_number: 1,
            description: "High-risk system modification".to_string(),
            action: AgentAction::KeyPress {
                key: "enter".to_string(),
            },
            risk_level: RiskLevel::HighRisk,
            verification: None,
            max_retries: 0,
        }];

        let planner = Arc::new(HeuristicAgentPlanner::new(steps));
        let loop_engine = AgenticOperationLoop::new(
            "Run high-risk task",
            backend,
            planner,
            OperationLoopConfig::default(),
        )
        .with_approval_broker(broker.clone());

        // Spawn loop run in background
        let loop_arc = Arc::new(loop_engine);
        let loop_clone = loop_arc.clone();
        let handle = tokio::spawn(async move { loop_clone.run().await });

        // Wait for loop to submit approval request
        tokio::time::sleep(Duration::from_millis(50)).await;
        let pending = broker.get_pending();
        assert_eq!(pending.len(), 1);

        // Authorize action
        let res = broker.resolve(pending[0].id, true, Some("Authorized by QA".into()));
        assert!(res.is_ok());

        let run_res = handle.await.unwrap();
        assert!(run_res.is_ok());
        assert_eq!(loop_arc.state(), LoopState::Completed);
    }

    #[tokio::test]
    async fn test_agentic_loop_approval_denial_fails_closed() {
        let backend = Arc::new(MockAutomationBackend);
        let broker = Arc::new(ApprovalBroker::new());

        let steps = vec![PlannedStep {
            step_number: 1,
            description: "Critical destructive action".to_string(),
            action: AgentAction::KeyPress {
                key: "delete".to_string(),
            },
            risk_level: RiskLevel::CriticalDestructive,
            verification: None,
            max_retries: 0,
        }];

        let planner = Arc::new(HeuristicAgentPlanner::new(steps));
        let loop_engine = AgenticOperationLoop::new(
            "Run destructive action",
            backend,
            planner,
            OperationLoopConfig::default(),
        )
        .with_approval_broker(broker.clone());

        let loop_arc = Arc::new(loop_engine);
        let loop_clone = loop_arc.clone();
        let handle = tokio::spawn(async move { loop_clone.run().await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        let pending = broker.get_pending();
        assert_eq!(pending.len(), 1);

        // Deny action
        let res = broker.resolve(pending[0].id, false, Some("Denied by user".into()));
        assert!(res.is_ok());

        let run_res = handle.await.unwrap();
        assert!(run_res.is_err());
        assert_eq!(loop_arc.state(), LoopState::Cancelled);
    }
}
