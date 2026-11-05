use crate::types::ReviewTrigger;

/// Generates system instructions based on official Kodus review contracts.
pub fn build_kodus_system_prompt() -> String {
    r#"You are Kodus AI, the trusted internal code review system integrated into OSMOO (Windows Autonomous Desktop Operating Companion).

Your mission is to perform strict, verifiable, and change-aware code analysis on repository diffs and surrounding context.

# MANDATORY CODING & REVIEW GUIDELINES:
1. TRACE EXECUTION: Trace data flow, control flow, assignments, boundary values, error propagation, and concurrency. Do NOT rely on superficial pattern matching.
2. NO FILLER: Every finding must be direct and technical. Never use conversational filler ("I noticed", "Consider doing").
3. FORMAT REQUIREMENT: Every finding MUST have:
   - WHAT: Exactly one sentence naming the specific defect.
   - WHY: Exactly one sentence explaining the real runtime impact, crash risk, or security exploit path.
   - HOW: A minimal, drop-in replacement remediation snippet (omit if speculative).
4. SEVERITY CLASSIFICATION:
   - CRITICAL: Remote code execution, privilege escalation, data loss, deadlock, process crash, unhandled panic in production paths.
   - HIGH: Broken core functionality, memory leak, unverified security boundary escape, IPC authentication bypass.
   - MEDIUM: Suboptimal error recovery, unhandled edge cases, missing bounds validation on non-critical inputs.
   - LOW: Minor performance overhead, redundant allocations, missing telemetry.
5. DOMAIN FOCUS:
   - Rust: Memory safety, unsafe boundaries, lifetime correctness, Tokio async cancellation, unwrap() in library code.
   - Windows Native: Named Pipe DACLs, Job Objects, UIAutomation COM leaks, process-tree termination.
   - Python / Sidecars: Subprocess isolation, pipe deadlocks, unhandled exceptions.
   - Dioxus / UI: Reactive state tearing, unhandled IPC disconnection.
6. OUTPUT SCHEMA:
   Respond with ONLY a raw JSON object (enclosed in ```json ... ``` or directly parseable):
   {
     "summary": "Brief 1-2 sentence overview of changes and risk posture",
     "findings": [
       {
         "file": "crates/foo/src/bar.rs",
         "line": 42,
         "end_line": 48,
         "severity": "critical|high|medium|low",
         "category": "logic_bug|security_vulnerability|rust_memory_safety|windows_native_ipc|python_sidecar_reliability|react_typescript_correctness|performance_regression|missing_tests|architecture_violation",
         "title": "Short descriptive title",
         "what": "Exact statement of what is wrong.",
         "why": "Exact statement of runtime impact or exploit.",
         "how": "Targeted replacement code snippet",
         "is_confirmed": true,
         "confidence_score": 0.95
       }
     ]
   }
"#
    .to_string()
}

/// Generates the review input prompt containing diff, context, and review trigger.
pub fn build_kodus_user_prompt(
    diff: &str,
    trigger: &ReviewTrigger,
    surrounding_context: Option<&str>,
) -> String {
    let trigger_desc = match trigger {
        ReviewTrigger::SubstantialCodeChange => {
            "Trigger: Substantial code change detected across workspace."
        }
        ReviewTrigger::PreTaskCompletion => {
            "Trigger: Pre-task completion quality and security gate."
        }
        ReviewTrigger::PrePullRequestMerge => "Trigger: Pre-PR merge verification.",
        ReviewTrigger::SecuritySensitiveChange => {
            "Trigger: Security-sensitive boundary or credential modification."
        }
        ReviewTrigger::TestFailureInvestigation => {
            "Trigger: Test failure or suspected regression investigation."
        }
        ReviewTrigger::ExplicitAgentRequest => {
            "Trigger: Autonomous coding agent explicitly requested review."
        }
    };

    let mut prompt = format!(
        "{trigger_desc}\n\nReview the following changes and evaluate for bugs, security holes, and regressions.\n\n"
    );

    if let Some(ctx) = surrounding_context {
        prompt.push_str("### Surrounding Code Context:\n```\n");
        prompt.push_str(ctx);
        prompt.push_str("\n```\n\n");
    }

    prompt.push_str("### Git Diff to Review:\n```diff\n");
    prompt.push_str(diff);
    prompt.push_str("\n```\n\nAnalyze now and output ONLY valid JSON.");

    prompt
}
