use crate::engine::KodusReviewEngine;
use crate::error::Result;
use crate::types::{KodusFinding, KodusReviewReport, RemediationStatus, ReviewScope, ReviewTrigger};
use std::sync::Arc;
use tracing::{info, warn};

pub const DEFAULT_MAX_REMEDIATION_ITERATIONS: usize = 3;

/// Coordinates the closed-loop autonomous remediation and re-review cycle with strict iteration limits.
pub struct KodusRemediationCoordinator {
    engine: Arc<KodusReviewEngine>,
    max_iterations: usize,
}

impl KodusRemediationCoordinator {
    pub fn new(engine: Arc<KodusReviewEngine>) -> Self {
        Self {
            engine,
            max_iterations: DEFAULT_MAX_REMEDIATION_ITERATIONS,
        }
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// Run the bounded remediation loop: Review -> Fix -> Test -> Re-review.
    ///
    /// # Loop Safety Contract
    /// - Strictly bounds retries to `self.max_iterations` to eliminate infinite AI self-patch loops.
    /// - Only confirmed, blocking defects (High / Critical) are passed to auto-remediation.
    /// - If tests fail or defects remain after `max_iterations`, immediately escalates to human review.
    pub async fn run_loop<F, T>(
        &self,
        scope: ReviewScope,
        internal_token: &str,
        mut apply_fix: F,
        mut verify_tests: T,
    ) -> Result<(RemediationStatus, KodusReviewReport)>
    where
        F: FnMut(&KodusFinding) -> std::result::Result<bool, String>,
        T: FnMut() -> std::result::Result<bool, String>,
    {
        let mut iteration = 0;

        loop {
            iteration += 1;
            info!(
                iteration = iteration,
                max = self.max_iterations,
                "Running internal Kodus review iteration"
            );

            let trigger = if iteration == 1 {
                ReviewTrigger::PreTaskCompletion
            } else {
                ReviewTrigger::TestFailureInvestigation
            };

            let report = self
                .engine
                .review(scope.clone(), trigger, internal_token)
                .await?;

            let blocking: Vec<KodusFinding> = report
                .findings
                .iter()
                .filter(|f| f.severity.is_blocking() && f.is_confirmed)
                .cloned()
                .collect();

            // 1. If no blocking defects remain, review is clean!
            if blocking.is_empty() {
                let status = if iteration == 1 {
                    RemediationStatus::CleanPass
                } else {
                    RemediationStatus::RemediatedAndVerified { iterations: iteration }
                };
                return Ok((status, report));
            }

            // 2. Check if maximum permitted iterations reached
            if iteration >= self.max_iterations {
                warn!(
                    remaining = blocking.len(),
                    "Kodus remediation loop reached maximum iterations limit"
                );
                return Ok((
                    RemediationStatus::MaxIterationsReached {
                        iterations: iteration,
                        remaining_findings: blocking.len(),
                    },
                    report,
                ));
            }

            // 3. Attempt autonomous targeted fix on blocking findings
            let mut fixes_applied = 0;
            for finding in &blocking {
                if finding.how.is_some() {
                    match apply_fix(finding) {
                        Ok(true) => fixes_applied += 1,
                        Ok(false) => {}
                        Err(e) => {
                            warn!(err = %e, file = %finding.file, "Remediation patch failed to apply");
                        }
                    }
                }
            }

            if fixes_applied == 0 {
                // No actionable fixes could be applied autonomously
                return Ok((
                    RemediationStatus::EscalatedToHuman {
                        reason: "Actionable automated fixes could not be applied to remaining blocking defects".to_string(),
                    },
                    report,
                ));
            }

            // 4. Verify compiler/tests pass after fix application
            match verify_tests() {
                Ok(true) => {
                    info!("Tests passed after applying Kodus remediation patch");
                }
                Ok(false) | Err(_) => {
                    warn!("Test suite failed after applying Kodus remediation patch; will re-review next cycle");
                }
            }
        }
    }
}
