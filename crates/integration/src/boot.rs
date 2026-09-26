//! Boot Sequence — deterministic startup with status reporting.
//!
//! Each stage reports status. Boot failures trigger automatic recovery.
//! Order: Kernel → Config → Database → Security → Providers → Memory →
//! Planner → Agents → Automation → Voice → Dashboard → Ready

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use voxy_shared::HealthStatus;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BootPhase {
    Kernel,
    Config,
    Database,
    Security,
    Providers,
    Memory,
    Planner,
    Agents,
    Automation,
    Voice,
    Dashboard,
    Ready,
}

impl BootPhase {
    pub fn all() -> Vec<BootPhase> {
        vec![
            Self::Kernel,
            Self::Config,
            Self::Database,
            Self::Security,
            Self::Providers,
            Self::Memory,
            Self::Planner,
            Self::Agents,
            Self::Automation,
            Self::Voice,
            Self::Dashboard,
            Self::Ready,
        ]
    }

    pub fn index(&self) -> usize {
        match self {
            Self::Kernel => 0,
            Self::Config => 1,
            Self::Database => 2,
            Self::Security => 3,
            Self::Providers => 4,
            Self::Memory => 5,
            Self::Planner => 6,
            Self::Agents => 7,
            Self::Automation => 8,
            Self::Voice => 9,
            Self::Dashboard => 10,
            Self::Ready => 11,
        }
    }
}

impl std::fmt::Display for BootPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kernel => write!(f, "Kernel"),
            Self::Config => write!(f, "Configuration"),
            Self::Database => write!(f, "Database"),
            Self::Security => write!(f, "Security"),
            Self::Providers => write!(f, "Providers"),
            Self::Memory => write!(f, "Memory"),
            Self::Planner => write!(f, "Planner"),
            Self::Agents => write!(f, "Agents"),
            Self::Automation => write!(f, "Automation"),
            Self::Voice => write!(f, "Voice"),
            Self::Dashboard => write!(f, "Dashboard"),
            Self::Ready => write!(f, "Ready"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootStatus {
    Pending,
    InProgress,
    Completed,
    Failed { reason: String },
    Recovering { attempt: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseReport {
    pub phase: BootPhase,
    pub status: BootStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<f64>,
    pub health: HealthStatus,
    pub details: Option<String>,
}

// ============================================================================
// Boot Sequence
// ============================================================================

/// Manages deterministic startup of all subsystems.
pub struct BootSequence {
    phases: RwLock<HashMap<BootPhase, PhaseReport>>,
    max_recovery_attempts: u32,
    recovery_attempts: RwLock<HashMap<BootPhase, u32>>,
    boot_started_at: RwLock<Option<DateTime<Utc>>>,
}

impl BootSequence {
    pub fn new(max_recovery_attempts: u32) -> Self {
        let mut phases = HashMap::new();
        for phase in BootPhase::all() {
            phases.insert(
                phase.clone(),
                PhaseReport {
                    phase: phase.clone(),
                    status: BootStatus::Pending,
                    started_at: None,
                    completed_at: None,
                    duration_ms: None,
                    health: HealthStatus::Healthy,
                    details: None,
                },
            );
        }

        Self {
            phases: RwLock::new(phases),
            max_recovery_attempts,
            recovery_attempts: RwLock::new(HashMap::new()),
            boot_started_at: RwLock::new(None),
        }
    }

    pub fn default_config() -> Self {
        Self::new(3)
    }

    /// Start the boot sequence.
    pub fn begin(&self) {
        *self.boot_started_at.write() = Some(Utc::now());
    }

    /// Mark a phase as in-progress.
    pub fn start_phase(&self, phase: &BootPhase) {
        let mut phases = self.phases.write();
        if let Some(report) = phases.get_mut(phase) {
            report.status = BootStatus::InProgress;
            report.started_at = Some(Utc::now());
        }
    }

    /// Mark a phase as completed.
    pub fn complete_phase(&self, phase: &BootPhase, details: Option<String>) {
        let mut phases = self.phases.write();
        if let Some(report) = phases.get_mut(phase) {
            let now = Utc::now();
            report.status = BootStatus::Completed;
            report.completed_at = Some(now);
            report.details = details;
            if let Some(started) = report.started_at {
                report.duration_ms = Some((now - started).num_milliseconds() as f64);
            }
        }
        // A successful completion resets the recovery budget for this phase.
        self.recovery_attempts.write().remove(phase);
    }

    /// Mark a phase as failed and attempt recovery.
    ///
    /// Returns `RecoveryDecision::Retry` until the phase has been retried
    /// `max_recovery_attempts` times, after which it returns
    /// `RecoveryDecision::Abort`.
    pub fn fail_phase(&self, phase: &BootPhase, reason: &str) -> RecoveryDecision {
        let mut phases = self.phases.write();
        let report = match phases.get_mut(phase) {
            Some(r) => r,
            None => return RecoveryDecision::Abort,
        };

        report.status = BootStatus::Failed {
            reason: reason.to_string(),
        };
        report.health = HealthStatus::Unhealthy(reason.to_string());

        let attempt = {
            let mut attempts = self.recovery_attempts.write();
            let count = attempts.entry(phase.clone()).or_insert(0);
            *count += 1;
            *count
        };

        if attempt > self.max_recovery_attempts {
            return RecoveryDecision::Abort;
        }

        RecoveryDecision::Retry {
            phase: phase.clone(),
            reason: reason.to_string(),
        }
    }

    /// Get the report for a specific phase.
    pub fn phase_report(&self, phase: &BootPhase) -> Option<PhaseReport> {
        self.phases.read().get(phase).cloned()
    }

    /// Get all phase reports.
    pub fn all_reports(&self) -> Vec<PhaseReport> {
        self.phases.read().values().cloned().collect()
    }

    /// Get the current boot phase (last completed or in-progress).
    pub fn current_phase(&self) -> Option<BootPhase> {
        let phases = self.phases.read();
        let mut current = None;
        for phase in BootPhase::all() {
            if let Some(report) = phases.get(&phase) {
                match &report.status {
                    BootStatus::InProgress | BootStatus::Completed => {
                        current = Some(phase);
                    }
                    BootStatus::Failed { .. } => return Some(phase),
                    _ => {}
                }
            }
        }
        current
    }

    /// Check if boot is complete.
    pub fn is_ready(&self) -> bool {
        let phases = self.phases.read();
        BootPhase::all().iter().all(|phase| {
            phases
                .get(phase)
                .map(|r| matches!(r.status, BootStatus::Completed))
                .unwrap_or(false)
        })
    }

    /// Get total boot duration.
    pub fn total_duration_ms(&self) -> Option<f64> {
        let started = *self.boot_started_at.read();
        let started = started?;
        let phases = self.phases.read();
        let last_completed = phases.values().filter_map(|r| r.completed_at).max()?;
        Some((last_completed - started).num_milliseconds() as f64)
    }

    /// Get phase count.
    pub fn phase_count(&self) -> usize {
        self.phases.read().len()
    }

    /// Get completed phase count.
    pub fn completed_count(&self) -> usize {
        self.phases
            .read()
            .values()
            .filter(|r| matches!(r.status, BootStatus::Completed))
            .count()
    }

    /// Get failed phases.
    pub fn failed_phases(&self) -> Vec<PhaseReport> {
        self.phases
            .read()
            .values()
            .filter(|r| matches!(r.status, BootStatus::Failed { .. }))
            .cloned()
            .collect()
    }

    /// Reset the boot sequence.
    pub fn reset(&self) {
        *self.boot_started_at.write() = None;
        self.recovery_attempts.write().clear();
        let mut phases = self.phases.write();
        for report in phases.values_mut() {
            report.status = BootStatus::Pending;
            report.started_at = None;
            report.completed_at = None;
            report.duration_ms = None;
            report.health = HealthStatus::Healthy;
            report.details = None;
        }
    }
}

#[derive(Debug, Clone)]
pub enum RecoveryDecision {
    Retry { phase: BootPhase, reason: String },
    Abort,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boot_sequence_creation() {
        let boot = BootSequence::default_config();
        assert_eq!(boot.phase_count(), 12);
        assert!(!boot.is_ready());
    }

    #[test]
    fn phase_order() {
        let phases = BootPhase::all();
        assert_eq!(phases[0], BootPhase::Kernel);
        assert_eq!(phases[11], BootPhase::Ready);
    }

    #[test]
    fn complete_all_phases() {
        let boot = BootSequence::default_config();
        boot.begin();
        for phase in BootPhase::all() {
            boot.start_phase(&phase);
            boot.complete_phase(&phase, None);
        }
        assert!(boot.is_ready());
        assert_eq!(boot.completed_count(), 12);
    }

    #[test]
    fn fail_and_detect() {
        let boot = BootSequence::default_config();
        boot.begin();
        boot.start_phase(&BootPhase::Kernel);
        let decision = boot.fail_phase(&BootPhase::Kernel, "init failed");
        assert!(matches!(decision, RecoveryDecision::Retry { .. }));
        assert!(!boot.is_ready());
        assert_eq!(boot.failed_phases().len(), 1);
    }

    #[test]
    fn current_phase_tracking() {
        let boot = BootSequence::default_config();
        boot.begin();
        boot.start_phase(&BootPhase::Kernel);
        boot.complete_phase(&BootPhase::Kernel, None);
        boot.start_phase(&BootPhase::Config);

        let current = boot.current_phase().unwrap();
        assert_eq!(current, BootPhase::Config);
    }

    #[test]
    fn phase_duration() {
        let boot = BootSequence::default_config();
        boot.begin();
        boot.start_phase(&BootPhase::Kernel);
        // Simulate some work
        std::thread::sleep(std::time::Duration::from_millis(10));
        boot.complete_phase(&BootPhase::Kernel, None);

        let report = boot.phase_report(&BootPhase::Kernel).unwrap();
        assert!(report.duration_ms.unwrap() >= 10.0);
    }

    #[test]
    fn reset_boot() {
        let boot = BootSequence::default_config();
        boot.begin();
        boot.start_phase(&BootPhase::Kernel);
        boot.complete_phase(&BootPhase::Kernel, None);
        boot.reset();
        assert!(!boot.is_ready());
        assert_eq!(boot.completed_count(), 0);
    }

    #[test]
    fn phase_display() {
        assert_eq!(BootPhase::Kernel.to_string(), "Kernel");
        assert_eq!(BootPhase::Config.to_string(), "Configuration");
        assert_eq!(BootPhase::Ready.to_string(), "Ready");
    }

    #[test]
    fn boot_duration() {
        let boot = BootSequence::default_config();
        // No start, so duration should be None
        assert!(boot.total_duration_ms().is_none());
    }

    #[test]
    fn failed_phase_report() {
        let boot = BootSequence::default_config();
        boot.start_phase(&BootPhase::Memory);
        boot.fail_phase(&BootPhase::Memory, "OOM");

        let report = boot.phase_report(&BootPhase::Memory).unwrap();
        assert!(report.health.is_unhealthy());
        match report.status {
            BootStatus::Failed { reason } => assert_eq!(reason, "OOM"),
            _ => panic!("Expected Failed status"),
        }
    }

    #[test]
    fn recovery_budget_aborts_after_max_attempts() {
        let boot = BootSequence::new(2);
        boot.start_phase(&BootPhase::Kernel);

        assert!(matches!(
            boot.fail_phase(&BootPhase::Kernel, "transient"),
            RecoveryDecision::Retry { .. }
        ));
        assert!(matches!(
            boot.fail_phase(&BootPhase::Kernel, "transient"),
            RecoveryDecision::Retry { .. }
        ));
        assert!(matches!(
            boot.fail_phase(&BootPhase::Kernel, "transient"),
            RecoveryDecision::Abort
        ));
    }

    #[test]
    fn successful_retry_resets_recovery_budget() {
        let boot = BootSequence::new(2);
        boot.start_phase(&BootPhase::Config);
        assert!(matches!(
            boot.fail_phase(&BootPhase::Config, "transient"),
            RecoveryDecision::Retry { .. }
        ));

        // Retry succeeds, budget is reset.
        boot.complete_phase(&BootPhase::Config, None);
        boot.start_phase(&BootPhase::Config);
        assert!(matches!(
            boot.fail_phase(&BootPhase::Config, "transient"),
            RecoveryDecision::Retry { .. }
        ));
    }

    #[test]
    fn abort_does_not_change_phase_status() {
        let boot = BootSequence::new(1);
        boot.start_phase(&BootPhase::Database);
        boot.fail_phase(&BootPhase::Database, "err1");
        let decision = boot.fail_phase(&BootPhase::Database, "err2");
        assert!(matches!(decision, RecoveryDecision::Abort));

        let report = boot.phase_report(&BootPhase::Database).unwrap();
        assert!(report.health.is_unhealthy());
    }
}
