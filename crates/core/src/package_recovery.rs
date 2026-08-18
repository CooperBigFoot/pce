//! recovery_decision : RecoveryLimits × ChargedFailure* → RecoveryRung
//!
//! The ladder is a per-package, per-graph-run spending policy. Only failures attributed to the
//! package work enter the sequence; environmental failures are observations with zero charge.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// Number of same-brief retry dispatches allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RetryLimit(u32);
impl RetryLimit {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Number of evidence-bearing local-patch dispatches allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalPatchLimit(u32);
impl LocalPatchLimit {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Number of identical worker-environment reports at which the environment is deemed persistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EnvironmentFailureLimit(u32);
impl EnvironmentFailureLimit {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl Default for EnvironmentFailureLimit {
    fn default() -> Self {
        Self::new(6)
    }
}

/// Number of identical gate-failure reports at which judgment is deemed persistently unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GateFailureLimit(u32);
impl GateFailureLimit {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u32 {
        self.0
    }
}
impl Default for GateFailureLimit {
    fn default() -> Self {
        Self::new(3)
    }
}

/// Adjustable driver limits, including the recovery ladder's unchanged spending allowances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryLimits {
    retry_attempts: RetryLimit,
    local_patch_attempts: LocalPatchLimit,
    #[serde(default)]
    environment_failures: EnvironmentFailureLimit,
    #[serde(default)]
    gate_failures: GateFailureLimit,
}
impl RecoveryLimits {
    /// Construct named limits for the two dispatching rungs and the default environment backstop.
    pub const fn new(retry_attempts: RetryLimit, local_patch_attempts: LocalPatchLimit) -> Self {
        Self {
            retry_attempts,
            local_patch_attempts,
            environment_failures: EnvironmentFailureLimit::new(6),
            gate_failures: GateFailureLimit::new(3),
        }
    }
    /// Select the identical-environment-failure threshold without changing recovery spending.
    pub const fn with_environment_failure_limit(mut self, limit: EnvironmentFailureLimit) -> Self {
        self.environment_failures = limit;
        self
    }
    /// Select the identical-gate-failure threshold without changing recovery spending.
    pub const fn with_gate_failure_limit(mut self, limit: GateFailureLimit) -> Self {
        self.gate_failures = limit;
        self
    }
    pub const fn retry_attempts(self) -> u32 {
        self.retry_attempts.get()
    }
    pub const fn local_patch_attempts(self) -> u32 {
        self.local_patch_attempts.get()
    }
    pub const fn environment_failures(self) -> u32 {
        self.environment_failures.get()
    }
    pub const fn gate_failures(self) -> u32 {
        self.gate_failures.get()
    }
    pub const fn dispatch_budget(self) -> u32 {
        self.retry_attempts()
            .saturating_add(self.local_patch_attempts())
    }
}
impl Default for RecoveryLimits {
    fn default() -> Self {
        Self::new(RetryLimit::new(1), LocalPatchLimit::new(1))
    }
}

/// The ordered action selected after an attributable failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryRung {
    Retry,
    LocalPatch,
    Replan,
}

/// Exact criterion evidence supplied to a local-patch worker and retained in the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryCriterionEvidence {
    pub criterion: String,
    pub command: String,
    pub exit_status: String,
    pub stdout: String,
    pub stderr: String,
}

/// Visible remaining dispatch allowance and next action for one package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecoveryBudget {
    pub retry_remaining: u32,
    pub local_patch_remaining: u32,
    pub dispatches_remaining: u32,
    pub next_rung: RecoveryRung,
}

/// Select the next rung from the number of attributable failures already observed.
pub fn recovery_budget(limits: RecoveryLimits, charged_failures: usize) -> RecoveryBudget {
    let charged = u32::try_from(charged_failures).unwrap_or(u32::MAX);
    let prior_recoveries = charged.saturating_sub(1);
    let retry_used = prior_recoveries.min(limits.retry_attempts());
    let after_retry = prior_recoveries.saturating_sub(limits.retry_attempts());
    let patch_used = after_retry.min(limits.local_patch_attempts());
    let retry_remaining = limits.retry_attempts().saturating_sub(retry_used);
    let local_patch_remaining = limits.local_patch_attempts().saturating_sub(patch_used);
    let dispatches_remaining = retry_remaining.saturating_add(local_patch_remaining);
    let next_rung = if charged <= limits.retry_attempts() {
        RecoveryRung::Retry
    } else if charged <= limits.dispatch_budget() {
        RecoveryRung::LocalPatch
    } else {
        RecoveryRung::Replan
    };
    RecoveryBudget {
        retry_remaining,
        local_patch_remaining,
        dispatches_remaining,
        next_rung,
    }
}

/// Add exact recorded failures to the same package brief for the local-patch rung.
pub fn compose_local_patch_brief(base: &str, evidence: &[RecoveryCriterionEvidence]) -> String {
    let mut output = base.to_owned();
    output.push_str("\n\n## Recovery rung: local patch\n\nThe prior attempt failed independent driver judgement. Repair only the recorded failures below.\n");
    for failure in evidence {
        let _ = writeln!(output, "\n### Criterion: {}", failure.criterion);
        let _ = writeln!(output, "Command: {}", failure.command);
        let _ = writeln!(output, "Exit status: {}", failure.exit_status);
        output.push_str("Stdout:\n```text\n");
        output.push_str(&failure.stdout);
        output.push_str("\n```\n");
        output.push_str("Stderr:\n```text\n");
        output.push_str(&failure.stderr);
        output.push_str("\n```\n");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        LocalPatchLimit, RecoveryCriterionEvidence, RecoveryLimits, RecoveryRung, RetryLimit,
        compose_local_patch_brief, recovery_budget,
    };

    #[test]
    fn legacy_limits_default_the_gate_failure_ceiling() {
        let limits: RecoveryLimits = serde_json::from_str(
            r#"{"retry_attempts":1,"local_patch_attempts":1,"environment_failures":6}"#,
        )
        .expect("legacy limits");
        assert_eq!(limits.gate_failures(), 3);
    }

    #[test]
    fn identical_failure_count_dispatches_under_budget_and_stops_without_it() {
        let enabled = recovery_budget(
            RecoveryLimits::new(RetryLimit::new(1), LocalPatchLimit::new(1)),
            2,
        );
        let removed = recovery_budget(
            RecoveryLimits::new(RetryLimit::new(0), LocalPatchLimit::new(0)),
            1,
        );
        assert_eq!(enabled.next_rung, RecoveryRung::LocalPatch);
        assert_eq!(enabled.dispatches_remaining, 1);
        assert_eq!(removed.next_rung, RecoveryRung::Replan);
        assert_eq!(removed.dispatches_remaining, 0);
    }

    #[test]
    fn local_patch_contains_exact_failure_evidence() {
        let brief = compose_local_patch_brief(
            "same base",
            &[RecoveryCriterionEvidence {
                criterion: "compile".into(),
                command: "cargo check".into(),
                exit_status: "exited 7".into(),
                stdout: "specific out".into(),
                stderr: "specific err".into(),
            }],
        );
        for expected in [
            "same base",
            "compile",
            "cargo check",
            "exited 7",
            "specific out",
            "specific err",
        ] {
            assert!(brief.contains(expected));
        }
    }
}
