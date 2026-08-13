//! driver_state : WorkPackageGraph × DriverEvent* → DriverSnapshot
//!
//! The driver journal is the sole authority for post-worker judgement. The fold is pure so a
//! restarted process derives the same scheduling decision without retaining process memory.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DependencyKind, RecoveryBudget, RecoveryCriterionEvidence, RecoveryLimits, RecoveryRung,
    WorkPackageGraph, recovery_budget,
};

/// One shell termination observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CommandExitStatus {
    /// The shell returned an exit code.
    Exited { code: i32 },
    /// The shell was terminated by a signal.
    Signaled { signal: i32 },
}
impl CommandExitStatus {
    /// Whether the shell returned exactly zero.
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Exited { code: 0 })
    }
}

/// Captured evidence from one criterion shell invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionExecution {
    command: String,
    working_directory: String,
    exit_status: CommandExitStatus,
    stdout: String,
    stderr: String,
}
impl CriterionExecution {
    /// Construct an execution record from composition-root observations.
    pub fn new(
        command: String,
        working_directory: String,
        exit_status: CommandExitStatus,
        stdout: String,
        stderr: String,
    ) -> Self {
        Self {
            command,
            working_directory,
            exit_status,
            stdout,
            stderr,
        }
    }
    /// Return the exact human-authored shell string.
    pub fn command(&self) -> &str {
        &self.command
    }
    /// Return the package-level working directory convention selected by the driver.
    pub fn working_directory(&self) -> &str {
        &self.working_directory
    }
    /// Return the observed shell status.
    pub const fn exit_status(&self) -> &CommandExitStatus {
        &self.exit_status
    }
    /// Return captured standard output.
    pub fn stdout(&self) -> &str {
        &self.stdout
    }
    /// Return captured standard error.
    pub fn stderr(&self) -> &str {
        &self.stderr
    }
}

/// Where one effective criterion came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CriterionOrigin {
    /// A criterion in the immutable graph.
    Authored,
    /// A falsifier credited to one gate finding.
    Amendment { gate: String, finding: u64 },
}

/// A repository-qualified witness/repair proof retained by an amendment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AmendmentRepositoryRefs {
    pub repository: String,
    pub witness_ref: String,
    pub repair_ref: String,
}

/// One criterion visible to a re-run, either authored or durably amended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveCriterion {
    pub name: String,
    pub command: String,
    pub origin: CriterionOrigin,
    pub repository_refs: Vec<AmendmentRepositoryRefs>,
}

/// Why a proposed gate finding was not credited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingRejectionReason {
    /// The proposed falsifier did not fail in the coordinated witness state.
    WitnessPassed,
    /// The proposed falsifier did not pass in the coordinated repaired state.
    RepairFailed,
}

/// The credit decision for a coordinated replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "kebab-case", deny_unknown_fields)]
pub enum FindingReplayDecision {
    /// Witness failed and repair passed, so the command becomes an amendment.
    Accepted,
    /// The finding did not demonstrate the required transition.
    Rejected { reason: FindingRejectionReason },
}

/// Judge a finding from its two coordinated package-scoped executions.
pub fn judge_finding_replay(
    witness: &CriterionExecution,
    repair: &CriterionExecution,
) -> FindingReplayDecision {
    if witness.exit_status.is_success() {
        FindingReplayDecision::Rejected {
            reason: FindingRejectionReason::WitnessPassed,
        }
    } else if !repair.exit_status.is_success() {
        FindingReplayDecision::Rejected {
            reason: FindingRejectionReason::RepairFailed,
        }
    } else {
        FindingReplayDecision::Accepted
    }
}

/// One rung retained in the final cold-readable parking record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAttemptRecord {
    pub rung: RecoveryRung,
    pub issuance: Option<u64>,
    pub what: String,
    pub evidence: Vec<RecoveryCriterionEvidence>,
}

/// One append-only fact in the driver journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DriverEvent {
    /// The named spending limits selected once for this graph run.
    RecoveryConfigured { limits: RecoveryLimits },
    /// A recovery dispatch and the exact augmented brief supplied to it.
    RecoveryRungAttempted {
        package: String,
        issuance: u64,
        rung: RecoveryRung,
        evidence: Vec<RecoveryCriterionEvidence>,
        brief: String,
    },
    /// A process/toolchain failure returned the package to readiness without charging its budget.
    WorkerEnvironmentFailed {
        package: String,
        issuance: u64,
        reason: String,
    },
    /// Exhaustion parks only this package and retains the complete three-rung account.
    RecoveryParked {
        package: String,
        reason: String,
        attempts: Vec<RecoveryAttemptRecord>,
    },
    /// A worker was issued. Absence of a later outcome means it is still running.
    WorkerDispatched { package: String, issuance: u64 },
    /// A worker claimed implementation completion and may now be judged.
    WorkerDone { package: String, issuance: u64 },
    /// A retryable worker failure stopped this attempt.
    WorkerFailed {
        package: String,
        issuance: u64,
        reason: String,
    },
    /// A graph fault parked this package permanently for this plan version.
    PackageParked {
        package: String,
        issuance: u64,
        reason: String,
    },
    /// One authored or amended criterion was executed.
    CriterionExecuted {
        package: String,
        name: String,
        origin: CriterionOrigin,
        execution: CriterionExecution,
    },
    /// One finding was replayed against coordinated repository states.
    FindingReplayed {
        package: String,
        gate: String,
        finding: u64,
        command: String,
        repository_refs: Vec<AmendmentRepositoryRefs>,
        witness: CriterionExecution,
        repair: CriterionExecution,
        decision: FindingReplayDecision,
    },
    /// The gate finished, including the zero-findings case.
    GateFinished { package: String, gate: String },
    /// All effective criteria and the gate accepted this package.
    PackageCompleted { package: String },
    /// Driver judgement failed; dependents remain blocked.
    PackageFailed { package: String, reason: String },
}

/// Restart-derived lifecycle state of one graph package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum DriverPackageState {
    Pending,
    Running { issuance: u64 },
    Judging { issuance: u64 },
    Complete,
    Failed { reason: String },
    Parked { reason: String },
}

/// Terminal state of the graph driver loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriverLoopOutcome {
    Running,
    Finished,
    Blocked,
}

/// Complete view reconstructed from graph and journal only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriverSnapshot {
    packages: Vec<(String, DriverPackageState)>,
    ready: Vec<String>,
    amendments: Vec<(String, EffectiveCriterion)>,
    recovery: Vec<(String, RecoveryBudget)>,
    outcome: DriverLoopOutcome,
}
impl DriverSnapshot {
    pub fn packages(&self) -> &[(String, DriverPackageState)] {
        &self.packages
    }
    pub fn ready(&self) -> &[String] {
        &self.ready
    }
    pub fn amendments(&self) -> &[(String, EffectiveCriterion)] {
        &self.amendments
    }
    pub fn recovery(&self) -> &[(String, RecoveryBudget)] {
        &self.recovery
    }
    pub const fn outcome(&self) -> DriverLoopOutcome {
        self.outcome
    }
}

/// A malformed or contradictory durable driver history.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PackageDriverError {
    /// An event names no package in the frozen graph.
    #[error("driver event names unknown package `{package}`")]
    UnknownPackage { package: String },
    /// One issuance is zero, which cannot identify an attempt.
    #[error("driver issuance for package `{package}` must be positive")]
    ZeroIssuance { package: String },
    /// A completion event follows no matching running attempt.
    #[error("driver outcome for package `{package}` issuance {issuance} has no matching dispatch")]
    UnmatchedOutcome { package: String, issuance: u64 },
    /// A terminal package was subsequently mutated by an impossible lifecycle event.
    #[error("driver event occurs after package `{package}` reached a terminal state")]
    EventAfterTerminal { package: String },
    /// A finding record's stored decision disagrees with its executions.
    #[error(
        "finding replay decision for package `{package}` gate `{gate}` finding {finding} disagrees with evidence"
    )]
    InconsistentFindingDecision {
        package: String,
        gate: String,
        finding: u64,
    },
    /// More than one distinct limit configuration appears in one graph-run journal.
    #[error("driver journal contains conflicting recovery limit configurations")]
    ConflictingRecoveryLimits,
}

/// Fold the append-only journal into scheduling state.
///
/// # Errors
///
/// Returns [`PackageDriverError`] when an event names an unknown package or contradicts the
/// lifecycle/evidence that precedes it.
pub fn derive_driver_snapshot(
    graph: &WorkPackageGraph,
    events: &[DriverEvent],
    override_risk_ordering: bool,
) -> Result<DriverSnapshot, PackageDriverError> {
    let mut configured_limits = None;
    for event in events {
        if let DriverEvent::RecoveryConfigured { limits } = event {
            if configured_limits.is_some_and(|configured| configured != *limits) {
                return Err(PackageDriverError::ConflictingRecoveryLimits);
            }
            configured_limits = Some(*limits);
        }
    }
    let known = graph
        .packages()
        .iter()
        .map(|p| p.id().as_str())
        .collect::<HashSet<_>>();
    let mut states = graph
        .packages()
        .iter()
        .map(|p| (p.id().as_str().to_owned(), DriverPackageState::Pending))
        .collect::<HashMap<_, _>>();
    let mut amendments = Vec::new();
    let terminal = |state: &DriverPackageState| {
        matches!(
            state,
            DriverPackageState::Complete
                | DriverPackageState::Failed { .. }
                | DriverPackageState::Parked { .. }
        )
    };
    for (event_index, event) in events.iter().enumerate() {
        let package = match event {
            DriverEvent::RecoveryConfigured { .. } => continue,
            DriverEvent::RecoveryRungAttempted { package, .. }
            | DriverEvent::WorkerEnvironmentFailed { package, .. }
            | DriverEvent::RecoveryParked { package, .. }
            | DriverEvent::WorkerDispatched { package, .. }
            | DriverEvent::WorkerDone { package, .. }
            | DriverEvent::WorkerFailed { package, .. }
            | DriverEvent::PackageParked { package, .. }
            | DriverEvent::CriterionExecuted { package, .. }
            | DriverEvent::FindingReplayed { package, .. }
            | DriverEvent::GateFinished { package, .. }
            | DriverEvent::PackageCompleted { package }
            | DriverEvent::PackageFailed { package, .. } => package,
        };
        if !known.contains(package.as_str()) {
            return Err(PackageDriverError::UnknownPackage {
                package: package.clone(),
            });
        }
        let state = states
            .get_mut(package)
            .unwrap_or_else(|| unreachable!("known package initialized"));
        match event {
            DriverEvent::RecoveryConfigured { .. } => unreachable!("configuration handled above"),
            DriverEvent::RecoveryRungAttempted { .. } => {
                if !matches!(state, DriverPackageState::Pending) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
            }
            DriverEvent::WorkerEnvironmentFailed { issuance, .. } => match state {
                DriverPackageState::Running { issuance: running } if running == issuance => {
                    *state = DriverPackageState::Pending;
                }
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::RecoveryParked { reason, .. } => {
                if matches!(
                    state,
                    DriverPackageState::Complete | DriverPackageState::Parked { .. }
                ) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                *state = DriverPackageState::Parked {
                    reason: reason.clone(),
                };
            }
            DriverEvent::WorkerDispatched { issuance, .. } => {
                if *issuance == 0 {
                    return Err(PackageDriverError::ZeroIssuance {
                        package: package.clone(),
                    });
                }
                if terminal(state) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                *state = DriverPackageState::Running {
                    issuance: *issuance,
                };
            }
            DriverEvent::WorkerDone { issuance, .. } => match state {
                DriverPackageState::Running { issuance: running } if running == issuance => {
                    *state = DriverPackageState::Judging {
                        issuance: *issuance,
                    }
                }
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::WorkerFailed {
                issuance, reason, ..
            } => match state {
                DriverPackageState::Running { issuance: running } if running == issuance => {
                    let charged = charged_failure_count(&events[..=event_index], package);
                    if configured_limits.is_some_and(|limits| {
                        recovery_budget(limits, charged).next_rung != RecoveryRung::Replan
                    }) {
                        *state = DriverPackageState::Pending;
                    } else {
                        *state = DriverPackageState::Failed {
                            reason: reason.clone(),
                        };
                    }
                }
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::PackageParked {
                issuance, reason, ..
            } => match state {
                DriverPackageState::Running { issuance: running } if running == issuance => {
                    *state = DriverPackageState::Parked {
                        reason: reason.clone(),
                    }
                }
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::CriterionExecuted { .. } | DriverEvent::GateFinished { .. } => {
                if !matches!(state, DriverPackageState::Judging { .. }) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
            }
            DriverEvent::FindingReplayed {
                gate,
                finding,
                command,
                repository_refs,
                witness,
                repair,
                decision,
                ..
            } => {
                if !matches!(state, DriverPackageState::Judging { .. }) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                let judged = judge_finding_replay(witness, repair);
                if &judged != decision {
                    return Err(PackageDriverError::InconsistentFindingDecision {
                        package: package.clone(),
                        gate: gate.clone(),
                        finding: *finding,
                    });
                }
                if matches!(decision, FindingReplayDecision::Accepted) {
                    amendments.push((
                        package.clone(),
                        EffectiveCriterion {
                            name: format!("gate:{gate}:finding:{finding}"),
                            command: command.clone(),
                            origin: CriterionOrigin::Amendment {
                                gate: gate.clone(),
                                finding: *finding,
                            },
                            repository_refs: repository_refs.clone(),
                        },
                    ));
                }
            }
            DriverEvent::PackageCompleted { .. } => {
                if !matches!(state, DriverPackageState::Judging { .. }) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                *state = DriverPackageState::Complete;
            }
            DriverEvent::PackageFailed { reason, .. } => {
                if terminal(state) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                let charged = charged_failure_count(&events[..=event_index], package);
                if configured_limits.is_some_and(|limits| {
                    recovery_budget(limits, charged).next_rung != RecoveryRung::Replan
                }) {
                    *state = DriverPackageState::Pending;
                } else {
                    *state = DriverPackageState::Failed {
                        reason: reason.clone(),
                    };
                }
            }
        }
    }
    let mut ready = Vec::new();
    for package in graph.packages() {
        if !matches!(states[package.id().as_str()], DriverPackageState::Pending) {
            continue;
        }
        let dependencies_complete = package.depends_on().iter().all(|edge| {
            (override_risk_ordering && edge.kind() == DependencyKind::RiskOrdering)
                || matches!(states[edge.id().as_str()], DriverPackageState::Complete)
        });
        if dependencies_complete {
            ready.push(package.id().as_str().to_owned());
        }
    }
    let running = states.values().any(|state| {
        matches!(
            state,
            DriverPackageState::Running { .. } | DriverPackageState::Judging { .. }
        )
    });
    let all_complete = states
        .values()
        .all(|state| matches!(state, DriverPackageState::Complete));
    let outcome = if all_complete {
        DriverLoopOutcome::Finished
    } else if running || !ready.is_empty() {
        DriverLoopOutcome::Running
    } else {
        DriverLoopOutcome::Blocked
    };
    let recovery = graph
        .packages()
        .iter()
        .map(|package| {
            let charged = charged_failure_count(events, package.id().as_str());
            (
                package.id().as_str().to_owned(),
                recovery_budget(configured_limits.unwrap_or_default(), charged),
            )
        })
        .collect();
    let packages = graph
        .packages()
        .iter()
        .map(|p| {
            (
                p.id().as_str().to_owned(),
                states
                    .remove(p.id().as_str())
                    .unwrap_or_else(|| unreachable!("initialized graph package")),
            )
        })
        .collect();
    Ok(DriverSnapshot {
        packages,
        ready,
        amendments,
        recovery,
        outcome,
    })
}

/// Count only worker-reported or criterion-judgement failures attributed to package work.
pub fn charged_failure_count(events: &[DriverEvent], package_id: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            matches!(event,
        DriverEvent::WorkerFailed { package, .. } | DriverEvent::PackageFailed { package, .. }
        if package == package_id)
        })
        .count()
}

/// Return the most recent failed criterion executions for a package.
pub fn latest_criterion_failure_evidence(
    events: &[DriverEvent],
    package_id: &str,
) -> Vec<RecoveryCriterionEvidence> {
    let start = events
        .iter()
        .rposition(|event| {
            matches!(event,
        DriverEvent::WorkerDispatched { package, .. } if package == package_id)
        })
        .unwrap_or(0);
    events[start..]
        .iter()
        .filter_map(|event| match event {
            DriverEvent::CriterionExecuted {
                package,
                name,
                execution,
                ..
            } if package == package_id && !execution.exit_status().is_success() => {
                let exit_status = match execution.exit_status() {
                    CommandExitStatus::Exited { code } => format!("exited {code}"),
                    CommandExitStatus::Signaled { signal } => format!("signaled {signal}"),
                };
                Some(RecoveryCriterionEvidence {
                    criterion: name.clone(),
                    command: execution.command().to_owned(),
                    exit_status,
                    stdout: execution.stdout().to_owned(),
                    stderr: execution.stderr().to_owned(),
                })
            }
            _ => None,
        })
        .collect()
}

/// Reconstruct the cold-readable record for all attempted rungs, adding the final replan.
pub fn recovery_attempt_records(
    events: &[DriverEvent],
    package_id: &str,
    final_evidence: Vec<RecoveryCriterionEvidence>,
) -> Vec<RecoveryAttemptRecord> {
    let mut records = events.iter().filter_map(|event| match event {
        DriverEvent::RecoveryRungAttempted { package, issuance, rung, evidence, .. }
            if package == package_id => Some(RecoveryAttemptRecord {
                rung: *rung, issuance: Some(*issuance),
                what: match rung { RecoveryRung::Retry => "same package and same brief with a fresh worker".to_owned(), RecoveryRung::LocalPatch => "same package with recorded criterion failure evidence appended to its brief".to_owned(), RecoveryRung::Replan => "park for plan version n+1".to_owned() },
                evidence: evidence.clone(),
            }),
        _ => None,
    }).collect::<Vec<_>>();
    records.push(RecoveryAttemptRecord {
        rung: RecoveryRung::Replan,
        issuance: None,
        what: "park for human re-authoring as plan version n+1".to_owned(),
        evidence: final_evidence,
    });
    records
}

/// Compose the graph-authored portion passed unchanged to retry and extended for local patch.
pub fn recovery_base_brief(
    graph: &WorkPackageGraph,
    package_id: &str,
) -> Result<String, PackageDriverError> {
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .ok_or_else(|| PackageDriverError::UnknownPackage {
            package: package_id.to_owned(),
        })?;
    let mut brief = format!(
        "# Work package {}: {}\n\nCriteria:\n",
        package.id().as_str(),
        package.title()
    );
    for criterion in package.criteria() {
        brief.push_str(&format!(
            "- {}\n  Input: {}\n  Observation: {}\n  Command: {}\n",
            criterion.name(),
            criterion.input(),
            criterion.observation(),
            criterion.command()
        ));
    }
    Ok(brief)
}

/// Return graph criteria plus accepted amendments, preserving authored then journal order.
pub fn effective_criteria(
    graph: &WorkPackageGraph,
    package_id: &str,
    events: &[DriverEvent],
) -> Result<Vec<EffectiveCriterion>, PackageDriverError> {
    let Some(package) = graph
        .packages()
        .iter()
        .find(|p| p.id().as_str() == package_id)
    else {
        return Err(PackageDriverError::UnknownPackage {
            package: package_id.to_owned(),
        });
    };
    let mut criteria = package
        .criteria()
        .iter()
        .map(|criterion| EffectiveCriterion {
            name: criterion.name().to_owned(),
            command: criterion.command().to_owned(),
            origin: CriterionOrigin::Authored,
            repository_refs: Vec::new(),
        })
        .collect::<Vec<_>>();
    let snapshot = derive_driver_snapshot(graph, events, false)?;
    criteria.extend(
        snapshot
            .amendments
            .into_iter()
            .filter_map(|(package, criterion)| (package == package_id).then_some(criterion)),
    );
    Ok(criteria)
}

#[cfg(test)]
mod tests {
    use super::{DriverEvent, DriverLoopOutcome, DriverPackageState, derive_driver_snapshot};
    use crate::{LocalPatchLimit, RecoveryLimits, RetryLimit, parse_work_package_graph};

    fn graph() -> crate::WorkPackageGraph {
        parse_work_package_graph(br#"{"vision":"v","plan_version":1,"authored_at_ref":"HEAD","packages":[{"id":"A","title":"A","repositories":["r"],"criteria":[{"name":"a","input":"i","observation":"o","command":"true"}],"depends_on":[]},{"id":"B","title":"B","repositories":["r"],"criteria":[{"name":"b","input":"i","observation":"o","command":"true"}],"depends_on":[{"id":"A","kind":"buildability","reason":"A"}]}]}"#).expect("valid graph")
    }

    #[test]
    fn restart_rederives_identical_recovery_budget_and_ready_dispatch() {
        let graph = graph();
        let events = vec![
            DriverEvent::RecoveryConfigured {
                limits: RecoveryLimits::new(RetryLimit::new(1), LocalPatchLimit::new(1)),
            },
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            DriverEvent::WorkerFailed {
                package: "A".to_owned(),
                issuance: 1,
                reason: "work blocker".to_owned(),
            },
        ];
        let before = derive_driver_snapshot(&graph, &events, false).expect("before restart");
        let restarted = derive_driver_snapshot(&graph, &events, false).expect("after restart");
        assert_eq!(before, restarted);
        assert_eq!(restarted.ready(), &["A"]);
        assert_eq!(restarted.recovery()[0].1.dispatches_remaining, 2);
        assert_eq!(
            restarted.recovery()[0].1.next_rung,
            crate::RecoveryRung::Retry
        );
    }

    #[test]
    fn restart_rederives_identical_running_complete_failed_and_parked_views() {
        let graph = graph();
        let running = vec![DriverEvent::WorkerDispatched {
            package: "A".to_owned(),
            issuance: 1,
        }];
        let first = derive_driver_snapshot(&graph, &running, false).expect("first fold");
        let restarted = derive_driver_snapshot(&graph, &running, false).expect("restart fold");
        assert_eq!(first, restarted);
        assert!(matches!(
            first.packages()[0].1,
            DriverPackageState::Running { issuance: 1 }
        ));
        assert_eq!(first.outcome(), DriverLoopOutcome::Running);

        let parked = vec![
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            DriverEvent::PackageParked {
                package: "A".to_owned(),
                issuance: 1,
                reason: "criterion: wrong".to_owned(),
            },
        ];
        let snapshot = derive_driver_snapshot(&graph, &parked, false).expect("parked fold");
        assert!(matches!(
            snapshot.packages()[0].1,
            DriverPackageState::Parked { .. }
        ));
        assert!(matches!(
            snapshot.packages()[1].1,
            DriverPackageState::Pending
        ));
        assert_eq!(snapshot.outcome(), DriverLoopOutcome::Blocked);
    }
}
