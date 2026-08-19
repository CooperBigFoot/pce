//! driver_state : WorkPackageGraph × DriverEvent* → DriverSnapshot
//!
//! The driver journal is the sole authority for post-worker judgement. The fold is pure so a
//! restarted process derives the same scheduling decision without retaining process memory.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DependencyKind, PackageGateChallenge, RecoveryBudget, RecoveryCriterionEvidence,
    RecoveryLimits, RecoveryRung, WorkPackageGraph, recovery_budget,
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

/// Counterfactual evidence for an amendment executed against a composed tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AmendmentProof {
    /// Reverting every repair diff succeeded and the criterion was executed in that state.
    Reverted { execution: CriterionExecution },
    /// At least one repair diff could not be reverted from the composed tree.
    Unconstructable {
        repository: String,
        repair_ref: String,
        detail: String,
    },
}
impl AmendmentProof {
    /// Whether the counterfactual state was constructed and made the amendment fail.
    pub fn proves_guard(&self) -> bool {
        matches!(self, Self::Reverted { execution } if !execution.exit_status().is_success())
    }
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
    /// Repository scope, resolution, reachability, or ancestry made the finding unusable.
    StructurallyMalformed,
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

/// The non-judgement result of preparing one repository environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnvironmentPreparationOutcome {
    Succeeded,
    Failed,
}

/// One package commit supplied as an input to repository composition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionInput {
    pub package: String,
    pub oid: String,
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

/// The best-effort result of closing one run-owned dispatch pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaneCleanupOutcome {
    /// Herdr confirmed that the pane was closed.
    Closed,
    /// The pane could not be closed; package judgement remains unchanged.
    Failed,
}

/// The process found in the worker pane immediately after Herdr accepted the dispatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DispatchWorkerProcessObservation {
    /// One foreground process supplied a stable identity for later comparison.
    Observed {
        process_id: u32,
        name: String,
        argv: Vec<String>,
    },
    /// Herdr accepted the agent but did not supply a usable foreground process observation.
    Inconclusive { detail: String },
}

/// The driver's conclusion from one attempted read of its dispatch environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchEnvironmentObservation {
    /// The recorded worker was absent and the pane had returned to its shell.
    Dead,
    /// The available Herdr evidence could not identify a live or dead worker.
    Inconclusive,
}

/// Why a remembered repair can no longer harden a rebuilt package lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StaleRepairCreditReason {
    /// The old repair commit is not available in its repository anymore.
    RepairUnavailable,
    /// The repair and current package lineage are both available but divergent.
    DivergentLineage,
    /// The repair hunk cannot be reverted from the composed tree although its criterion passes.
    CounterfactualUnconstructable,
}

/// The terminal product certified by one driver-created Git ref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DriverRefProduct {
    /// A package attempt retained as an input to the final assembly.
    PackageAttempt { package: String, issuance: u64 },
    /// The final composed assembly for one repository.
    Assembly,
    /// A conflict-resolution commit adopted by assembly composition.
    AssemblyResolution,
}

/// One append-only fact in the driver journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DriverEvent {
    /// The driver returned an error after recording the diagnostic for from-disk explanation.
    DriverAborted { reason: String },
    /// A prior abort was acknowledged by a new driver process before it resumed the fold.
    DriverResumed,
    /// A remembered repair does not belong to this package attempt's rebuilt lineage.
    RepairCreditStale {
        package: String,
        issuance: u64,
        repository: String,
        gate: String,
        finding: u64,
        repair_ref: String,
        lineage_oid: String,
        reason: StaleRepairCreditReason,
    },
    /// A newer immutable plan became active and names the prior completions it carries forward.
    PlanVersionAdvanced {
        from_plan_version: u64,
        to_plan_version: u64,
        carried_completions: Vec<String>,
        carried_amendments: Vec<(String, EffectiveCriterion)>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criterion_revisions_ratified_by: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        criterion_revisions: Vec<crate::CriterionRevision>,
    },
    /// A human ruled that one disputed package definition stands for one fresh attempt.
    PackageParkOverruled {
        package: String,
        plan_version: u64,
        rationale: String,
    },
    /// The initial names-only environment contract selected for worker workspaces in this run.
    WorkerEnvironmentDeclared { names: Vec<String> },
    /// Names added to the worker environment contract at one plan-version boundary.
    WorkerEnvironmentExtended {
        plan_version: u64,
        added_names: Vec<String>,
    },
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
    /// A gate attempt failed to deliver a complete judgment without charging package work.
    GateFailed {
        package: String,
        issuance: u64,
        gate: String,
        reason: String,
        detail: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        challenges: Vec<PackageGateChallenge>,
    },
    /// One distinct gate attempt was issued while the package remained in judgment.
    GateDispatched {
        package: String,
        issuance: u64,
        attempt: u32,
        gate: String,
    },
    /// Repetition established that this package's gate is persistently unavailable.
    PackageGateBlocked {
        package: String,
        issuance: u64,
        gate: String,
        reason: String,
        detail: String,
        identical_failures: u32,
    },
    /// Repetition established that this package's worker environment is persistently unavailable.
    PackageEnvironmentBlocked {
        package: String,
        issuance: u64,
        reason: String,
        identical_failures: u32,
    },
    /// Exhaustion parks only this package and retains the complete three-rung account.
    RecoveryParked {
        package: String,
        reason: String,
        attempts: Vec<RecoveryAttemptRecord>,
    },
    /// The spawning code observed that no child was produced.
    WorkerSpawnFailed {
        package: String,
        issuance: u64,
        reason: String,
    },
    /// One repository base containing all composing dependencies was produced for a package.
    PackageBaseComposed {
        package: String,
        repository: String,
        base_oid: String,
        dependencies: Vec<CompositionInput>,
    },
    /// A textual merge conflict became the dependent worker's starting condition.
    PackageJoinConflicted {
        package: String,
        repository: String,
        base_oid: String,
        dependencies: Vec<CompositionInput>,
        conflicting_input: CompositionInput,
        remaining_inputs: Vec<CompositionInput>,
        conflicted_paths: Vec<String>,
        reason: String,
    },
    /// An infrastructure fault prevented dependency composition from producing a worker base.
    PackageCompositionFailed {
        package: String,
        repository: String,
        dependencies: Vec<CompositionInput>,
        reason: String,
    },
    /// A worker was issued. Absence of a later outcome means it is still running.
    WorkerDispatched { package: String, issuance: u64 },
    /// Herdr's worktree-create response named the exact root pane and its workspace.
    DispatchPaneOpened {
        package: String,
        issuance: u64,
        pane_id: String,
        workspace_id: String,
    },
    /// Herdr identified the pane occupied by the worker and the process observed there at spawn.
    DispatchWorkerIdentified {
        package: String,
        issuance: u64,
        dispatch_sequence: u64,
        agent_name: String,
        pane_id: String,
        workspace_id: String,
        process: DispatchWorkerProcessObservation,
    },
    /// The driver recorded the evidence used to close one environment-lost dispatch.
    DispatchEnvironmentObserved {
        package: String,
        issuance: u64,
        dispatch_sequence: u64,
        observation: DispatchEnvironmentObservation,
        detail: String,
    },
    /// Best-effort cleanup was attempted only after this package attempt completed.
    DispatchPaneCleanup {
        package: String,
        issuance: u64,
        pane_id: String,
        workspace_id: String,
        outcome: PaneCleanupOutcome,
        detail: String,
    },
    /// Pane ownership could not be established; no pane was selected for later cleanup.
    DispatchPaneOwnershipUnresolved {
        package: String,
        issuance: u64,
        detail: String,
    },
    /// A caller-supplied observation bound elapsed after the worker was verified alive.
    DriverStoppedWaiting {
        package: String,
        issuance: u64,
        waited_ms: u64,
    },
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
    /// One repository environment was prepared inside a driver-owned materialization.
    EnvironmentPreparationExecuted {
        package: String,
        materialization: String,
        repository: String,
        command: String,
        outcome: EnvironmentPreparationOutcome,
        execution: CriterionExecution,
    },
    /// One parent criterion was re-executed after the dependent worker resolved a join.
    JoinCriterionExecuted {
        package: String,
        parent: String,
        name: String,
        origin: CriterionOrigin,
        execution: CriterionExecution,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amendment_proof: Option<AmendmentProof>,
    },
    /// One authored or amended criterion was executed.
    CriterionExecuted {
        package: String,
        name: String,
        origin: CriterionOrigin,
        execution: CriterionExecution,
    },
    /// One structurally unusable finding was rejected before executing its proposed command.
    FindingRejected {
        package: String,
        gate: String,
        finding: u64,
        command: String,
        reason: FindingRejectionReason,
        detail: String,
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
    /// A credited repair was fast-forwarded into its package's durable lineage.
    PackageRepairMerged {
        package: String,
        repository: String,
        gate: String,
        finding: u64,
        repair_ref: String,
        previous_oid: String,
        hardened_oid: String,
    },
    /// One authored or amended criterion was re-executed against gate-hardened lineage.
    GateReproofExecuted {
        package: String,
        gate: String,
        name: String,
        origin: CriterionOrigin,
        execution: CriterionExecution,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amendment_proof: Option<AmendmentProof>,
    },
    /// An environment preparation failed before gate re-proof and consumed no failure budget.
    GateReproofEnvironmentFailed {
        package: String,
        issuance: u64,
        gate: String,
        repository: String,
        command: String,
        execution: CriterionExecution,
    },
    /// A repair rejected by gate re-proof was removed from the package lineage.
    PackageRepairRolledBack {
        package: String,
        repository: String,
        gate: String,
        finding: u64,
        repair_ref: String,
        hardened_oid: String,
        restored_oid: String,
    },
    /// Assembly could not reconstruct a credited finding after this package rewrote its source.
    PackageHardeningInvalidated {
        package: String,
        hardened_package: String,
        repository: String,
        gate: String,
        finding: u64,
        repair_ref: String,
        detail: String,
    },
    /// All effective criteria and the gate accepted this package.
    PackageCompleted { package: String },
    /// Driver judgement failed; dependents remain blocked.
    PackageFailed { package: String, reason: String },
    /// One repository containing every completed package was produced for assembly gating.
    AssemblyRepositoryComposed {
        repository: String,
        base_oid: String,
        packages: Vec<CompositionInput>,
    },
    /// A textual assembly merge conflict was assigned to a resolution worker.
    AssemblyJoinConflicted {
        repository: String,
        packages: Vec<CompositionInput>,
        base_oid: String,
        conflicting_input: CompositionInput,
        remaining_inputs: Vec<CompositionInput>,
        conflicted_paths: Vec<String>,
        reason: String,
    },
    /// The assembly conflict worker was started without synthesizing a graph package.
    AssemblyResolutionDispatched { repository: String },
    /// The assembly conflict worker resolved and committed the join.
    AssemblyResolutionDone {
        repository: String,
        base_oid: String,
    },
    /// Completed package commits could not be composed because of infrastructure failure.
    AssemblyCompositionFailed {
        repository: String,
        packages: Vec<CompositionInput>,
        reason: String,
    },
    /// One effective package criterion was re-executed against the composed assembly.
    AssemblyCriterionExecuted {
        package: String,
        name: String,
        origin: CriterionOrigin,
        execution: CriterionExecution,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        amendment_proof: Option<AmendmentProof>,
    },
    /// The driver created a non-moving Git ref for a journal-proven terminal product.
    DriverRefMaterialized {
        repository: String,
        reference: String,
        oid: String,
        product: DriverRefProduct,
    },
    /// Every effective criterion passed against the composed assembly.
    AssemblyCompleted,
    /// Assembly gating reached a terminal failure distinct from package judgement.
    AssemblyFailed { reason: String },
}

/// Restart-derived lifecycle state of one graph package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum DriverPackageState {
    Pending,
    Running {
        issuance: u64,
    },
    Judging {
        issuance: u64,
    },
    Complete,
    Failed {
        reason: String,
    },
    CompositionFailed {
        repository: String,
        reason: String,
    },
    EnvironmentPreparationFailed {
        repository: String,
        command: String,
    },
    EnvironmentBlocked {
        reason: String,
        identical_failures: u32,
    },
    GateBlocked {
        gate: String,
        reason: String,
        identical_failures: u32,
    },
    Parked {
        reason: String,
    },
}

/// Restart-derived lifecycle state of the final graph assembly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum DriverAssemblyState {
    Pending,
    Gating,
    Complete,
    Failed { reason: String },
}

/// Terminal state of the graph driver loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriverLoopOutcome {
    Running,
    Finished,
    Blocked,
}

/// One human-ratified criterion revision reconstructed from a typed plan transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RatifiedCriterionRevision {
    from_plan_version: u64,
    to_plan_version: u64,
    ratified_by: String,
    revision: crate::CriterionRevision,
}

impl RatifiedCriterionRevision {
    /// Return the predecessor plan version.
    pub const fn from_plan_version(&self) -> u64 {
        self.from_plan_version
    }

    /// Return the successor plan version.
    pub const fn to_plan_version(&self) -> u64 {
        self.to_plan_version
    }

    /// Return the human identity attributed with the ruling.
    pub fn ratified_by(&self) -> &str {
        &self.ratified_by
    }

    /// Return the exact revision record.
    pub const fn revision(&self) -> &crate::CriterionRevision {
        &self.revision
    }
}

/// Complete view reconstructed from graph and journal only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriverSnapshot {
    packages: Vec<(String, DriverPackageState)>,
    ready: Vec<String>,
    amendments: Vec<(String, EffectiveCriterion)>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    criterion_revisions: Vec<RatifiedCriterionRevision>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    worker_environment: Vec<String>,
    recovery: Vec<(String, RecoveryBudget)>,
    assembly: DriverAssemblyState,
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
    pub fn criterion_revisions(&self) -> &[RatifiedCriterionRevision] {
        &self.criterion_revisions
    }
    /// Return the names-only worker environment contract for this run.
    pub fn worker_environment(&self) -> &[String] {
        &self.worker_environment
    }
    pub fn recovery(&self) -> &[(String, RecoveryBudget)] {
        &self.recovery
    }
    pub const fn assembly(&self) -> &DriverAssemblyState {
        &self.assembly
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
    /// The active journal plan does not match the graph supplied for replay.
    #[error(
        "driver journal plan version {journal_version} does not match graph plan version {graph_version}"
    )]
    PlanVersionMismatch {
        journal_version: u64,
        graph_version: u64,
    },
    /// A plan transition's criterion revisions lack exact human attribution.
    #[error("driver plan transition criterion revisions have invalid human attribution")]
    InvalidCriterionRevisionAttribution,
    /// A plan transition did not advance by exactly one immutable version.
    #[error(
        "driver plan transition must advance from version {from_plan_version} to {to_plan_version}"
    )]
    InvalidPlanTransition {
        from_plan_version: u64,
        to_plan_version: u64,
    },
    /// An overrule did not name the active plan version or a non-empty rationale.
    #[error("park overrule for package `{package}` is invalid for plan version {plan_version}")]
    InvalidParkOverrule { package: String, plan_version: u64 },
    /// The package was not parked by a worker's specification dispute.
    #[error("package `{package}` has no specification-dispute park to overrule")]
    NoDisputedPark { package: String },
    /// One human overrule has already been consumed for this package definition.
    #[error("package `{package}` was already overruled; graph revision is the only exit")]
    RepeatedParkOverrule { package: String },
    /// A plan transition claimed completion without prior journal proof.
    #[error("plan transition carries package `{package}` without prior completion proof")]
    UnprovenCarriedCompletion { package: String },
    /// One issuance is zero, which cannot identify an attempt.
    #[error("driver issuance for package `{package}` must be positive")]
    ZeroIssuance { package: String },
    /// A dispatch issuance did not increase strictly across the append-only journal.
    #[error("worker dispatch issuance {issuance} is not greater than prior issuance {prior}")]
    NonMonotonicIssuance { issuance: u64, prior: u64 },
    /// A completion event follows no matching running attempt.
    #[error("driver outcome for package `{package}` issuance {issuance} has no matching dispatch")]
    UnmatchedOutcome { package: String, issuance: u64 },
    /// A terminal package was subsequently mutated by an impossible lifecycle event.
    #[error("driver event occurs after package `{package}` reached a terminal state")]
    EventAfterTerminal { package: String },
    /// A preparation record's outcome disagrees with its shell status.
    #[error(
        "environment preparation outcome for package `{package}` repository `{repository}` disagrees with evidence"
    )]
    InconsistentEnvironmentPreparationOutcome { package: String, repository: String },
    /// A finding record's stored decision disagrees with its executions.
    #[error(
        "finding replay decision for package `{package}` gate `{gate}` finding {finding} disagrees with evidence"
    )]
    InconsistentFindingDecision {
        package: String,
        gate: String,
        finding: u64,
    },
    /// A pre-replay rejection used a reason reserved for execution evidence.
    #[error("structural finding rejection for package `{package}` has a non-structural reason")]
    InvalidStructuralFindingReason { package: String },
    /// More than one distinct names-only worker environment contract appears in one run journal.
    #[error("driver journal contains conflicting worker environment declarations")]
    ConflictingWorkerEnvironment,
    /// A worker environment extension is not attached to the named open plan-version boundary.
    #[error(
        "worker environment extension for plan version {plan_version} does not occur at an open plan-version boundary"
    )]
    WorkerEnvironmentExtensionOutsidePlanBoundary { plan_version: u64 },
    /// More than one distinct limit configuration appears in one graph-run journal.
    #[error("driver journal contains conflicting recovery limit configurations")]
    ConflictingRecoveryLimits,
    /// Assembly work began before every package had completed successfully.
    #[error("assembly event occurs before every graph package is complete")]
    AssemblyBeforePackagesComplete,
    /// An assembly event contradicts an already-terminal assembly outcome.
    #[error("assembly event occurs after assembly reached a terminal state")]
    AssemblyEventAfterTerminal,
    /// A criterion was recorded before an assembly repository entered gating.
    #[error("assembly criterion for package `{package}` occurs before assembly gating")]
    AssemblyCriterionBeforeGating { package: String },
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
    let mut configured_worker_environment = None;
    let mut worker_environment_extension_boundary = None;
    let mut prior_issuance = 0_u64;
    for event in events {
        match event {
            DriverEvent::PlanVersionAdvanced {
                to_plan_version, ..
            } => worker_environment_extension_boundary = Some(*to_plan_version),
            DriverEvent::DriverAborted { .. }
            | DriverEvent::DriverResumed
            | DriverEvent::WorkerEnvironmentDeclared { .. }
            | DriverEvent::WorkerEnvironmentExtended { .. }
            | DriverEvent::RecoveryConfigured { .. } => {}
            _ => worker_environment_extension_boundary = None,
        }
        if let DriverEvent::WorkerDispatched { package, issuance } = event {
            if *issuance == 0 {
                return Err(PackageDriverError::ZeroIssuance {
                    package: package.clone(),
                });
            }
            if *issuance <= prior_issuance {
                return Err(PackageDriverError::NonMonotonicIssuance {
                    issuance: *issuance,
                    prior: prior_issuance,
                });
            }
            prior_issuance = *issuance;
        }

        match event {
            DriverEvent::WorkerEnvironmentDeclared { names } => {
                if configured_worker_environment
                    .as_ref()
                    .is_some_and(|configured| configured != names)
                {
                    return Err(PackageDriverError::ConflictingWorkerEnvironment);
                }
                configured_worker_environment = Some(names.clone());
            }
            DriverEvent::WorkerEnvironmentExtended {
                plan_version,
                added_names,
            } => {
                if worker_environment_extension_boundary != Some(*plan_version) {
                    return Err(
                        PackageDriverError::WorkerEnvironmentExtensionOutsidePlanBoundary {
                            plan_version: *plan_version,
                        },
                    );
                }
                let configured = configured_worker_environment
                    .as_mut()
                    .ok_or(PackageDriverError::ConflictingWorkerEnvironment)?;
                if added_names.is_empty()
                    || added_names.iter().any(|name| configured.contains(name))
                    || added_names.iter().collect::<HashSet<_>>().len() != added_names.len()
                {
                    return Err(PackageDriverError::ConflictingWorkerEnvironment);
                }
                configured.extend(added_names.iter().cloned());
                configured.sort();
            }
            _ => {}
        }
        if let DriverEvent::RecoveryConfigured { limits } = event {
            if configured_limits.is_some_and(|configured| configured != *limits) {
                return Err(PackageDriverError::ConflictingRecoveryLimits);
            }
            configured_limits = Some(*limits);
        }
    }
    let mut proven_completions = HashSet::<String>::new();
    for event in events {
        match event {
            DriverEvent::PlanVersionAdvanced {
                carried_completions,
                ..
            } => {
                for package in carried_completions {
                    if !proven_completions.contains(package) {
                        return Err(PackageDriverError::UnprovenCarriedCompletion {
                            package: package.clone(),
                        });
                    }
                }
                proven_completions = carried_completions.iter().cloned().collect();
            }
            DriverEvent::WorkerDispatched { package, .. } => {
                proven_completions.remove(package);
            }
            DriverEvent::PackageCompleted { package } => {
                proven_completions.insert(package.clone());
            }
            _ => {}
        }
    }
    let mut criterion_revisions = Vec::new();
    for event in events {
        if let DriverEvent::PlanVersionAdvanced {
            from_plan_version,
            to_plan_version,
            criterion_revisions_ratified_by,
            criterion_revisions: revisions,
            ..
        } = event
        {
            match (criterion_revisions_ratified_by, revisions.is_empty()) {
                (None, true) => {}
                (Some(ratified_by), false) if !ratified_by.trim().is_empty() => {
                    criterion_revisions.extend(revisions.iter().cloned().map(|revision| {
                        RatifiedCriterionRevision {
                            from_plan_version: *from_plan_version,
                            to_plan_version: *to_plan_version,
                            ratified_by: ratified_by.clone(),
                            revision,
                        }
                    }));
                }
                _ => return Err(PackageDriverError::InvalidCriterionRevisionAttribution),
            }
        }
    }
    let known = graph
        .packages()
        .iter()
        .map(|p| p.id().as_str())
        .collect::<HashSet<_>>();
    let active_events = if let Some(index) = events
        .iter()
        .rposition(|event| matches!(event, DriverEvent::PlanVersionAdvanced { .. }))
    {
        let DriverEvent::PlanVersionAdvanced {
            to_plan_version, ..
        } = &events[index]
        else {
            unreachable!("transition index names a transition");
        };
        if *to_plan_version != graph.plan_version() {
            return Err(PackageDriverError::PlanVersionMismatch {
                journal_version: *to_plan_version,
                graph_version: graph.plan_version(),
            });
        }
        &events[index..]
    } else {
        events
    };
    let mut states = graph
        .packages()
        .iter()
        .map(|p| (p.id().as_str().to_owned(), DriverPackageState::Pending))
        .collect::<HashMap<_, _>>();
    let mut amendments = Vec::new();
    let mut assembly = DriverAssemblyState::Pending;
    let mut disputed_parks = HashSet::new();
    let mut consumed_overrules = HashSet::new();
    let mut driver_aborted = false;
    let terminal = |state: &DriverPackageState| {
        matches!(
            state,
            DriverPackageState::Complete
                | DriverPackageState::Failed { .. }
                | DriverPackageState::EnvironmentPreparationFailed { .. }
                | DriverPackageState::EnvironmentBlocked { .. }
                | DriverPackageState::GateBlocked { .. }
                | DriverPackageState::Parked { .. }
        )
    };
    for (event_index, event) in active_events.iter().enumerate() {
        let all_packages_complete = || {
            states
                .values()
                .all(|state| matches!(state, DriverPackageState::Complete))
        };
        match event {
            DriverEvent::DriverAborted { .. } => {
                driver_aborted = true;
                continue;
            }
            DriverEvent::DriverResumed => {
                driver_aborted = false;
                continue;
            }
            DriverEvent::DriverRefMaterialized { .. } => continue,
            DriverEvent::PlanVersionAdvanced {
                from_plan_version,
                to_plan_version,
                carried_completions,
                carried_amendments,
                ..
            } => {
                if from_plan_version.checked_add(1) != Some(*to_plan_version)
                    || *to_plan_version != graph.plan_version()
                {
                    return Err(PackageDriverError::InvalidPlanTransition {
                        from_plan_version: *from_plan_version,
                        to_plan_version: *to_plan_version,
                    });
                }
                let mut carried = HashSet::new();
                for package in carried_completions {
                    if !known.contains(package.as_str()) {
                        return Err(PackageDriverError::UnknownPackage {
                            package: package.clone(),
                        });
                    }
                    if !carried.insert(package.as_str()) {
                        return Err(PackageDriverError::EventAfterTerminal {
                            package: package.clone(),
                        });
                    }
                    states.insert(package.clone(), DriverPackageState::Complete);
                }
                for (package, criterion) in carried_amendments {
                    if !known.contains(package.as_str()) {
                        return Err(PackageDriverError::UnknownPackage {
                            package: package.clone(),
                        });
                    }
                    amendments.push((package.clone(), criterion.clone()));
                }
                assembly = DriverAssemblyState::Pending;
                disputed_parks.clear();
                consumed_overrules.clear();
                continue;
            }
            DriverEvent::PackageHardeningInvalidated { .. } => {
                assembly = DriverAssemblyState::Pending;
            }
            DriverEvent::AssemblyRepositoryComposed { .. } => {
                if !all_packages_complete() {
                    return Err(PackageDriverError::AssemblyBeforePackagesComplete);
                }
                if matches!(
                    assembly,
                    DriverAssemblyState::Complete | DriverAssemblyState::Failed { .. }
                ) {
                    return Err(PackageDriverError::AssemblyEventAfterTerminal);
                }
                assembly = DriverAssemblyState::Gating;
                continue;
            }
            DriverEvent::AssemblyJoinConflicted { .. }
            | DriverEvent::AssemblyResolutionDispatched { .. }
            | DriverEvent::AssemblyResolutionDone { .. } => {
                if !all_packages_complete() {
                    return Err(PackageDriverError::AssemblyBeforePackagesComplete);
                }
                if matches!(
                    assembly,
                    DriverAssemblyState::Complete | DriverAssemblyState::Failed { .. }
                ) {
                    return Err(PackageDriverError::AssemblyEventAfterTerminal);
                }
                continue;
            }
            DriverEvent::AssemblyCompositionFailed { reason, .. } => {
                if !all_packages_complete() {
                    return Err(PackageDriverError::AssemblyBeforePackagesComplete);
                }
                if matches!(
                    assembly,
                    DriverAssemblyState::Complete | DriverAssemblyState::Failed { .. }
                ) {
                    return Err(PackageDriverError::AssemblyEventAfterTerminal);
                }
                assembly = DriverAssemblyState::Failed {
                    reason: reason.clone(),
                };
                continue;
            }
            DriverEvent::AssemblyCriterionExecuted { package, .. } => {
                if !known.contains(package.as_str()) {
                    return Err(PackageDriverError::UnknownPackage {
                        package: package.clone(),
                    });
                }
                if !all_packages_complete() {
                    return Err(PackageDriverError::AssemblyBeforePackagesComplete);
                }
                match assembly {
                    DriverAssemblyState::Gating => {}
                    DriverAssemblyState::Complete | DriverAssemblyState::Failed { .. } => {
                        return Err(PackageDriverError::AssemblyEventAfterTerminal);
                    }
                    DriverAssemblyState::Pending => {
                        return Err(PackageDriverError::AssemblyCriterionBeforeGating {
                            package: package.clone(),
                        });
                    }
                }
                continue;
            }
            DriverEvent::AssemblyCompleted => {
                if !all_packages_complete() {
                    return Err(PackageDriverError::AssemblyBeforePackagesComplete);
                }
                match assembly {
                    DriverAssemblyState::Gating => {
                        assembly = DriverAssemblyState::Complete;
                    }
                    DriverAssemblyState::Complete | DriverAssemblyState::Failed { .. } => {
                        return Err(PackageDriverError::AssemblyEventAfterTerminal);
                    }
                    DriverAssemblyState::Pending => {
                        return Err(PackageDriverError::AssemblyCriterionBeforeGating {
                            package: "<completion>".to_owned(),
                        });
                    }
                }
                continue;
            }
            DriverEvent::AssemblyFailed { reason } => {
                if !all_packages_complete() {
                    return Err(PackageDriverError::AssemblyBeforePackagesComplete);
                }
                if matches!(
                    assembly,
                    DriverAssemblyState::Complete | DriverAssemblyState::Failed { .. }
                ) {
                    return Err(PackageDriverError::AssemblyEventAfterTerminal);
                }
                assembly = DriverAssemblyState::Failed {
                    reason: reason.clone(),
                };
                continue;
            }
            _ => {}
        }
        let package = match event {
            DriverEvent::DriverAborted { .. }
            | DriverEvent::DriverResumed
            | DriverEvent::PlanVersionAdvanced { .. }
            | DriverEvent::WorkerEnvironmentDeclared { .. }
            | DriverEvent::WorkerEnvironmentExtended { .. }
            | DriverEvent::RecoveryConfigured { .. }
            | DriverEvent::DriverRefMaterialized { .. } => {
                continue;
            }
            DriverEvent::PackageParkOverruled { package, .. }
            | DriverEvent::RecoveryRungAttempted { package, .. }
            | DriverEvent::PackageBaseComposed { package, .. }
            | DriverEvent::PackageJoinConflicted { package, .. }
            | DriverEvent::PackageCompositionFailed { package, .. }
            | DriverEvent::GateFailed { package, .. }
            | DriverEvent::GateDispatched { package, .. }
            | DriverEvent::PackageGateBlocked { package, .. }
            | DriverEvent::WorkerEnvironmentFailed { package, .. }
            | DriverEvent::PackageEnvironmentBlocked { package, .. }
            | DriverEvent::RecoveryParked { package, .. }
            | DriverEvent::WorkerSpawnFailed { package, .. }
            | DriverEvent::WorkerDispatched { package, .. }
            | DriverEvent::DispatchPaneOpened { package, .. }
            | DriverEvent::DispatchWorkerIdentified { package, .. }
            | DriverEvent::DispatchEnvironmentObserved { package, .. }
            | DriverEvent::DispatchPaneCleanup { package, .. }
            | DriverEvent::DispatchPaneOwnershipUnresolved { package, .. }
            | DriverEvent::DriverStoppedWaiting { package, .. }
            | DriverEvent::WorkerDone { package, .. }
            | DriverEvent::WorkerFailed { package, .. }
            | DriverEvent::PackageParked { package, .. }
            | DriverEvent::EnvironmentPreparationExecuted { package, .. }
            | DriverEvent::JoinCriterionExecuted { package, .. }
            | DriverEvent::CriterionExecuted { package, .. }
            | DriverEvent::FindingRejected { package, .. }
            | DriverEvent::FindingReplayed { package, .. }
            | DriverEvent::GateFinished { package, .. }
            | DriverEvent::PackageRepairMerged { package, .. }
            | DriverEvent::GateReproofExecuted { package, .. }
            | DriverEvent::GateReproofEnvironmentFailed { package, .. }
            | DriverEvent::PackageRepairRolledBack { package, .. }
            | DriverEvent::RepairCreditStale { package, .. }
            | DriverEvent::PackageHardeningInvalidated { package, .. }
            | DriverEvent::PackageCompleted { package }
            | DriverEvent::PackageFailed { package, .. } => package,
            DriverEvent::AssemblyRepositoryComposed { .. }
            | DriverEvent::AssemblyJoinConflicted { .. }
            | DriverEvent::AssemblyResolutionDispatched { .. }
            | DriverEvent::AssemblyResolutionDone { .. }
            | DriverEvent::AssemblyCompositionFailed { .. }
            | DriverEvent::AssemblyCriterionExecuted { .. }
            | DriverEvent::AssemblyCompleted
            | DriverEvent::AssemblyFailed { .. } => unreachable!("assembly handled above"),
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
            DriverEvent::DriverAborted { .. } | DriverEvent::DriverResumed => {
                unreachable!("driver lifecycle handled above")
            }
            DriverEvent::PlanVersionAdvanced { .. }
            | DriverEvent::WorkerEnvironmentDeclared { .. }
            | DriverEvent::WorkerEnvironmentExtended { .. }
            | DriverEvent::RecoveryConfigured { .. }
            | DriverEvent::DriverRefMaterialized { .. } => {
                unreachable!("configuration or ref evidence handled above")
            }
            DriverEvent::PackageParkOverruled {
                plan_version,
                rationale,
                ..
            } => {
                if *plan_version != graph.plan_version() || rationale.trim().is_empty() {
                    return Err(PackageDriverError::InvalidParkOverrule {
                        package: package.clone(),
                        plan_version: *plan_version,
                    });
                }
                if consumed_overrules.contains(package) {
                    return Err(PackageDriverError::RepeatedParkOverrule {
                        package: package.clone(),
                    });
                }
                if !matches!(state, DriverPackageState::Parked { .. })
                    || !disputed_parks.contains(package)
                {
                    return Err(PackageDriverError::NoDisputedPark {
                        package: package.clone(),
                    });
                }
                consumed_overrules.insert(package.clone());
                *state = DriverPackageState::Pending;
            }
            DriverEvent::PackageBaseComposed { .. } | DriverEvent::PackageJoinConflicted { .. } => {
                if !matches!(state, DriverPackageState::Pending) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
            }
            DriverEvent::PackageCompositionFailed { .. } => {
                if !matches!(state, DriverPackageState::Pending) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
            }
            DriverEvent::RecoveryRungAttempted { .. } => {
                if !matches!(state, DriverPackageState::Pending) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
            }
            DriverEvent::GateDispatched { issuance, .. }
            | DriverEvent::GateFailed { issuance, .. } => match state {
                DriverPackageState::Judging { issuance: judging } if judging == issuance => {}
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::PackageGateBlocked {
                issuance,
                gate,
                reason,
                identical_failures,
                ..
            } => match state {
                DriverPackageState::Judging { issuance: judging } if judging == issuance => {
                    *state = DriverPackageState::GateBlocked {
                        gate: gate.clone(),
                        reason: reason.clone(),
                        identical_failures: *identical_failures,
                    };
                }
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
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
            DriverEvent::PackageEnvironmentBlocked {
                issuance,
                reason,
                identical_failures,
                ..
            } => match state {
                DriverPackageState::Running { issuance: running } if running == issuance => {
                    *state = DriverPackageState::EnvironmentBlocked {
                        reason: reason.clone(),
                        identical_failures: *identical_failures,
                    };
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
            DriverEvent::WorkerSpawnFailed { issuance, .. } => match state {
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
            DriverEvent::DispatchPaneOpened { issuance, .. }
            | DriverEvent::DispatchWorkerIdentified { issuance, .. }
            | DriverEvent::DispatchEnvironmentObserved { issuance, .. }
            | DriverEvent::DispatchPaneOwnershipUnresolved { issuance, .. } => match state {
                DriverPackageState::Running { issuance: active }
                | DriverPackageState::Judging { issuance: active }
                    if active == issuance => {}
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::DispatchPaneCleanup { issuance, .. } => {
                if *issuance == 0 || !matches!(state, DriverPackageState::Complete) {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            }
            DriverEvent::DriverStoppedWaiting { issuance, .. } => match state {
                DriverPackageState::Running { issuance: running } if running == issuance => {}
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
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
                    let charged = charged_failure_count(&active_events[..=event_index], package);
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
                    };
                    disputed_parks.insert(package.clone());
                }
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::EnvironmentPreparationExecuted {
                repository,
                command,
                outcome,
                execution,
                ..
            } => {
                if !matches!(state, DriverPackageState::Judging { .. }) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                let succeeded = execution.exit_status().is_success();
                if succeeded != matches!(outcome, EnvironmentPreparationOutcome::Succeeded) {
                    return Err(
                        PackageDriverError::InconsistentEnvironmentPreparationOutcome {
                            package: package.clone(),
                            repository: repository.clone(),
                        },
                    );
                }
                if !succeeded {
                    *state = DriverPackageState::EnvironmentPreparationFailed {
                        repository: repository.clone(),
                        command: command.clone(),
                    };
                }
            }
            DriverEvent::JoinCriterionExecuted { .. }
            | DriverEvent::CriterionExecuted { .. }
            | DriverEvent::GateFinished { .. }
            | DriverEvent::GateReproofExecuted { .. } => {
                if !matches!(state, DriverPackageState::Judging { .. }) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
            }
            DriverEvent::GateReproofEnvironmentFailed { issuance, .. } => match state {
                DriverPackageState::Judging { issuance: judging } if judging == issuance => {}
                _ => {
                    return Err(PackageDriverError::UnmatchedOutcome {
                        package: package.clone(),
                        issuance: *issuance,
                    });
                }
            },
            DriverEvent::FindingRejected { reason, .. } => {
                if !matches!(state, DriverPackageState::Judging { .. }) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                if !matches!(reason, FindingRejectionReason::StructurallyMalformed) {
                    return Err(PackageDriverError::InvalidStructuralFindingReason {
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
            DriverEvent::PackageRepairMerged { .. } => {
                if !matches!(
                    state,
                    DriverPackageState::Complete | DriverPackageState::Judging { .. }
                ) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
            }
            DriverEvent::RepairCreditStale { gate, finding, .. } => {
                if !matches!(
                    state,
                    DriverPackageState::Complete | DriverPackageState::Judging { .. }
                ) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                for (amended_package, criterion) in &mut amendments {
                    if amended_package == package
                        && matches!(
                            &criterion.origin,
                            CriterionOrigin::Amendment {
                                gate: amended_gate,
                                finding: amended_finding,
                            } if amended_gate == gate && amended_finding == finding
                        )
                    {
                        criterion.repository_refs.clear();
                    }
                }
            }
            DriverEvent::PackageRepairRolledBack { gate, finding, .. } => {
                if !matches!(state, DriverPackageState::Judging { .. }) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                amendments.retain(|(amended_package, criterion)| {
                    amended_package != package
                        || !matches!(
                            &criterion.origin,
                            CriterionOrigin::Amendment {
                                gate: amended_gate,
                                finding: amended_finding,
                            } if amended_gate == gate && amended_finding == finding
                        )
                });
            }
            DriverEvent::PackageHardeningInvalidated { .. } => {
                if !matches!(state, DriverPackageState::Complete) {
                    return Err(PackageDriverError::EventAfterTerminal {
                        package: package.clone(),
                    });
                }
                *state = DriverPackageState::Pending;
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
                let charged = charged_failure_count(&active_events[..=event_index], package);
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
            DriverEvent::AssemblyRepositoryComposed { .. }
            | DriverEvent::AssemblyJoinConflicted { .. }
            | DriverEvent::AssemblyResolutionDispatched { .. }
            | DriverEvent::AssemblyResolutionDone { .. }
            | DriverEvent::AssemblyCompositionFailed { .. }
            | DriverEvent::AssemblyCriterionExecuted { .. }
            | DriverEvent::AssemblyCompleted
            | DriverEvent::AssemblyFailed { .. } => unreachable!("assembly handled above"),
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
    let outcome = if driver_aborted {
        DriverLoopOutcome::Blocked
    } else if all_complete {
        match assembly {
            DriverAssemblyState::Complete => DriverLoopOutcome::Finished,
            DriverAssemblyState::Failed { .. } => DriverLoopOutcome::Blocked,
            DriverAssemblyState::Pending | DriverAssemblyState::Gating => {
                DriverLoopOutcome::Running
            }
        }
    } else if running || !ready.is_empty() {
        DriverLoopOutcome::Running
    } else {
        DriverLoopOutcome::Blocked
    };
    let recovery = graph
        .packages()
        .iter()
        .map(|package| {
            let charged = charged_failure_count(active_events, package.id().as_str());
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
        criterion_revisions,
        worker_environment: configured_worker_environment.unwrap_or_default(),
        recovery,
        assembly,
        outcome,
    })
}

/// One exact pane capability belonging to a package attempt that completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPaneCleanup {
    package: String,
    issuance: u64,
    pane_id: String,
    workspace_id: String,
}

impl PendingPaneCleanup {
    /// Return the completed package identity.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Return the attempt whose worker and gate panes may be removed.
    pub const fn issuance(&self) -> u64 {
        self.issuance
    }

    /// Return the exact pane capability supplied by Herdr at creation.
    pub fn pane_id(&self) -> &str {
        &self.pane_id
    }

    /// Return the exact worktree workspace containing the run-created root pane.
    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }
}

/// Derive exact, not-yet-attempted cleanup targets only for completed package attempts.
pub fn pending_completed_pane_cleanups(events: &[DriverEvent]) -> Vec<PendingPaneCleanup> {
    let mut last_done = HashMap::<&str, u64>::new();
    let mut completed = HashSet::<(&str, u64)>::new();
    let mut accounted = HashSet::<(&str, u64, &str)>::new();
    for event in events {
        match event {
            DriverEvent::WorkerDone { package, issuance } => {
                last_done.insert(package, *issuance);
            }
            DriverEvent::PackageCompleted { package } => {
                if let Some(issuance) = last_done.get(package.as_str()) {
                    completed.insert((package, *issuance));
                }
            }
            DriverEvent::DispatchPaneCleanup {
                package,
                issuance,
                pane_id,
                ..
            } => {
                accounted.insert((package, *issuance, pane_id));
            }
            _ => {}
        }
    }
    events
        .iter()
        .filter_map(|event| match event {
            DriverEvent::DispatchPaneOpened {
                package,
                issuance,
                pane_id,
                workspace_id,
            } if completed.contains(&(package.as_str(), *issuance))
                && !accounted.contains(&(package.as_str(), *issuance, pane_id.as_str())) =>
            {
                Some(PendingPaneCleanup {
                    package: package.clone(),
                    issuance: *issuance,
                    pane_id: pane_id.clone(),
                    workspace_id: workspace_id.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

/// Construct the zero-charge outcome for one incomplete gate judgment from durable history.
pub fn gate_failure_outcome(
    events: &[DriverEvent],
    limits: RecoveryLimits,
    package: String,
    issuance: u64,
    gate: String,
    reason: String,
    detail: String,
    challenges: Vec<PackageGateChallenge>,
) -> DriverEvent {
    let prior_identical = events_for_active_plan(events)
        .iter()
        .filter(|event| matches!(
            event,
            DriverEvent::GateFailed { package: observed_package, issuance: observed_issuance, reason: observed_reason, .. }
                if observed_package == &package && observed_issuance == &issuance && observed_reason == &reason
        ))
        .count();
    let identical_failures = u32::try_from(prior_identical)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    if identical_failures >= limits.gate_failures() {
        DriverEvent::PackageGateBlocked {
            package,
            issuance,
            gate,
            reason,
            detail,
            identical_failures,
        }
    } else {
        DriverEvent::GateFailed {
            package,
            issuance,
            gate,
            reason,
            detail,
            challenges,
        }
    }
}

/// Return the next durable gate-attempt identity for one judging issuance.
pub fn next_gate_attempt(events: &[DriverEvent], package: &str, issuance: u64) -> u32 {
    events_for_active_plan(events)
        .iter()
        .filter(|event| matches!(event, DriverEvent::GateDispatched { package: observed, issuance: observed_issuance, .. } if observed == package && observed_issuance == &issuance))
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

/// Return challenges retained by the latest failed gate attempt in this judging issuance.
pub fn pending_gate_challenges(
    events: &[DriverEvent],
    package: &str,
    issuance: u64,
) -> Vec<PackageGateChallenge> {
    events_for_active_plan(events)
        .iter()
        .rev()
        .find_map(|event| match event {
            DriverEvent::GateFailed {
                package: observed,
                issuance: observed_issuance,
                challenges,
                ..
            } if observed == package && observed_issuance == &issuance => Some(challenges.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Construct the zero-charge outcome for one worker-environment report from durable history.
///
/// Exact equality of the package and reason identifies recurrence. The report that reaches the
/// configured threshold becomes the distinct terminal event, so every observed cause remains in
/// the journal without a second, non-atomic state transition.
pub fn worker_environment_outcome(
    events: &[DriverEvent],
    limits: RecoveryLimits,
    package: String,
    issuance: u64,
    reason: String,
) -> DriverEvent {
    let mut prior_identical = 0_usize;
    for event in events_for_active_plan(events).iter().rev() {
        match event {
            DriverEvent::WorkerEnvironmentFailed {
                package: observed_package,
                reason: observed_reason,
                ..
            } if observed_package == &package && observed_reason == &reason => {
                prior_identical = prior_identical.saturating_add(1);
            }
            DriverEvent::WorkerEnvironmentFailed {
                package: observed_package,
                ..
            }
            | DriverEvent::WorkerSpawnFailed {
                package: observed_package,
                ..
            }
            | DriverEvent::WorkerDone {
                package: observed_package,
                ..
            }
            | DriverEvent::WorkerFailed {
                package: observed_package,
                ..
            } if observed_package == &package => break,
            _ => {}
        }
    }
    let identical_failures = u32::try_from(prior_identical)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    if identical_failures >= limits.environment_failures() {
        DriverEvent::PackageEnvironmentBlocked {
            package,
            issuance,
            reason,
            identical_failures,
        }
    } else {
        DriverEvent::WorkerEnvironmentFailed {
            package,
            issuance,
            reason,
        }
    }
}

fn events_for_active_plan(events: &[DriverEvent]) -> &[DriverEvent] {
    events
        .iter()
        .rposition(|event| matches!(event, DriverEvent::PlanVersionAdvanced { .. }))
        .map_or(events, |index| &events[index..])
}

/// Count only worker-reported or criterion-judgement failures attributed to package work.
pub fn charged_failure_count(events: &[DriverEvent], package_id: &str) -> usize {
    events_for_active_plan(events)
        .iter()
        .filter(|event| {
            matches!(event,
        DriverEvent::WorkerFailed { package, .. }
            | DriverEvent::PackageFailed { package, .. }
            | DriverEvent::PackageHardeningInvalidated { package, .. }
        if package == package_id)
        })
        .count()
}

fn failed_criterion_evidence(
    criterion: String,
    execution: &CriterionExecution,
) -> Option<RecoveryCriterionEvidence> {
    let exit_status = match execution.exit_status() {
        CommandExitStatus::Exited { code } => format!("exited {code}"),
        CommandExitStatus::Signaled { signal } => format!("signaled {signal}"),
    };
    Some(RecoveryCriterionEvidence {
        criterion,
        command: execution.command().to_owned(),
        exit_status,
        stdout: execution.stdout().to_owned(),
        stderr: execution.stderr().to_owned(),
    })
}

/// Return the most recent failed criterion executions for a package.
pub fn latest_criterion_failure_evidence(
    events: &[DriverEvent],
    package_id: &str,
) -> Vec<RecoveryCriterionEvidence> {
    let events = events_for_active_plan(events);
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
                failed_criterion_evidence(name.clone(), execution)
            }
            DriverEvent::JoinCriterionExecuted {
                package,
                parent,
                name,
                execution,
                amendment_proof,
                ..
            } if package == package_id
                && (!execution.exit_status().is_success()
                    || amendment_proof
                        .as_ref()
                        .is_some_and(|proof| !proof.proves_guard())) =>
            {
                failed_criterion_evidence(format!("parent:{parent}:{name}"), execution)
            }
            DriverEvent::PackageHardeningInvalidated {
                package,
                hardened_package,
                gate,
                finding,
                repair_ref,
                detail,
                ..
            } if package == package_id => Some(RecoveryCriterionEvidence {
                criterion: format!("gate:{gate}:finding:{finding}:restore-provability"),
                command: format!("restore repair {repair_ref} credited to {hardened_package}"),
                exit_status: "revert unconstructable".to_owned(),
                stdout: String::new(),
                stderr: detail.clone(),
            }),
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
    let mut records = events_for_active_plan(events).iter().filter_map(|event| match event {
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
    use super::{
        CommandExitStatus, CompositionInput, CriterionExecution, CriterionOrigin,
        DriverAssemblyState, DriverEvent, DriverLoopOutcome, DriverPackageState,
        PackageDriverError, charged_failure_count, derive_driver_snapshot,
        worker_environment_outcome,
    };
    use crate::{
        EnvironmentFailureLimit, LocalPatchLimit, RecoveryLimits, RetryLimit,
        parse_work_package_graph,
    };

    fn graph_at_plan_version(plan_version: u64) -> crate::WorkPackageGraph {
        let graph = format!(
            r#"{{"vision":"v","plan_version":{plan_version},"authored_at_ref":"HEAD","packages":[{{"id":"A","title":"A","repositories":["r"],"criteria":[{{"name":"a","input":"i","observation":"o","command":"true"}}],"depends_on":[]}},{{"id":"B","title":"B","repositories":["r"],"criteria":[{{"name":"b","input":"i","observation":"o","command":"true"}}],"depends_on":[{{"id":"A","kind":"buildability","reason":"A"}}]}}]}}"#
        );
        parse_work_package_graph(graph.as_bytes()).expect("valid graph")
    }

    fn graph() -> crate::WorkPackageGraph {
        graph_at_plan_version(1)
    }

    fn completed_packages() -> Vec<DriverEvent> {
        [("A", 1), ("B", 2)]
            .into_iter()
            .flat_map(|(package, issuance)| {
                [
                    DriverEvent::WorkerDispatched {
                        package: package.to_owned(),
                        issuance,
                    },
                    DriverEvent::WorkerDone {
                        package: package.to_owned(),
                        issuance,
                    },
                    DriverEvent::PackageCompleted {
                        package: package.to_owned(),
                    },
                ]
            })
            .collect()
    }

    #[test]
    fn journal_ending_in_driver_abort_is_blocked_until_a_resume_event() {
        let graph = graph();
        let mut events = vec![DriverEvent::WorkerDispatched {
            package: "A".to_owned(),
            issuance: 1,
        }];
        events.push(DriverEvent::DriverAborted {
            reason: "credited repair is stale".to_owned(),
        });

        let aborted = derive_driver_snapshot(&graph, &events, false).expect("aborted snapshot");
        assert_eq!(aborted.outcome(), DriverLoopOutcome::Blocked);

        events.push(DriverEvent::DriverResumed);
        let resumed = derive_driver_snapshot(&graph, &events, false).expect("resumed snapshot");
        assert_eq!(resumed.outcome(), DriverLoopOutcome::Running);
    }

    #[test]
    fn gate_failures_preserve_judging_without_spending_package_recovery() {
        let graph = graph();
        let limits = RecoveryLimits::new(RetryLimit::new(1), LocalPatchLimit::new(1))
            .with_gate_failure_limit(crate::GateFailureLimit::new(2));
        let mut events = vec![
            DriverEvent::RecoveryConfigured { limits },
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            DriverEvent::WorkerDone {
                package: "A".to_owned(),
                issuance: 1,
            },
        ];
        events.push(super::gate_failure_outcome(
            &events,
            limits,
            "A".to_owned(),
            1,
            "package-gate-1-1".to_owned(),
            "gate dispatch stopped without an outcome".to_owned(),
            "exited 1".to_owned(),
            Vec::new(),
        ));
        let snapshot = derive_driver_snapshot(&graph, &events, false).expect("snapshot");
        assert!(matches!(
            snapshot.packages()[0].1,
            DriverPackageState::Judging { issuance: 1 }
        ));
        assert_eq!(charged_failure_count(&events, "A"), 0);

        events.push(super::gate_failure_outcome(
            &events,
            limits,
            "A".to_owned(),
            1,
            "package-gate-1-2".to_owned(),
            "gate dispatch stopped without an outcome".to_owned(),
            "exited 1 again".to_owned(),
            Vec::new(),
        ));
        let snapshot = derive_driver_snapshot(&graph, &events, false).expect("blocked snapshot");
        assert!(matches!(
            &snapshot.packages()[0].1,
            DriverPackageState::GateBlocked { gate, reason, identical_failures: 2 }
                if gate == "package-gate-1-2" && reason == "gate dispatch stopped without an outcome"
        ));
        assert_eq!(charged_failure_count(&events, "A"), 0);
        assert_eq!(snapshot.outcome(), DriverLoopOutcome::Blocked);
        assert!(matches!(
            snapshot.packages()[1].1,
            DriverPackageState::Pending
        ));
    }

    #[test]
    fn worker_environment_extensions_fold_as_exact_additions() {
        let graph = graph_at_plan_version(2);
        let events = vec![
            DriverEvent::WorkerEnvironmentDeclared { names: Vec::new() },
            DriverEvent::PlanVersionAdvanced {
                from_plan_version: 1,
                to_plan_version: 2,
                carried_completions: Vec::new(),
                carried_amendments: Vec::new(),
                criterion_revisions_ratified_by: None,
                criterion_revisions: Vec::new(),
            },
            DriverEvent::WorkerEnvironmentExtended {
                plan_version: 2,
                added_names: vec!["AUTHORIZATION".to_owned(), "WHEEL".to_owned()],
            },
        ];

        let snapshot = derive_driver_snapshot(&graph, &events, false)
            .expect("additions-only environment must derive");
        assert_eq!(
            snapshot.worker_environment(),
            &["AUTHORIZATION".to_owned(), "WHEEL".to_owned()]
        );
        let encoded = serde_json::to_value(&events[2]).expect("extension event JSON");
        assert_eq!(encoded["event"], "worker-environment-extended");
        assert_eq!(
            encoded["added_names"],
            serde_json::json!(["AUTHORIZATION", "WHEEL"])
        );
    }

    #[test]
    fn worker_environment_extension_rejects_non_exact_additions() {
        let graph = graph();
        for added_names in [
            Vec::new(),
            vec!["EXISTING".to_owned()],
            vec!["NEW".to_owned(), "NEW".to_owned()],
        ] {
            let events = vec![
                DriverEvent::WorkerEnvironmentDeclared {
                    names: vec!["EXISTING".to_owned()],
                },
                DriverEvent::PlanVersionAdvanced {
                    from_plan_version: 1,
                    to_plan_version: 2,
                    carried_completions: Vec::new(),
                    carried_amendments: Vec::new(),
                    criterion_revisions_ratified_by: None,
                    criterion_revisions: Vec::new(),
                },
                DriverEvent::WorkerEnvironmentExtended {
                    plan_version: 2,
                    added_names,
                },
            ];
            assert!(matches!(
                derive_driver_snapshot(&graph, &events, false),
                Err(PackageDriverError::ConflictingWorkerEnvironment)
            ));
        }
    }

    #[test]
    fn worker_environment_extension_requires_the_named_open_boundary() {
        let graph = graph();
        for events in [
            vec![
                DriverEvent::WorkerEnvironmentDeclared { names: Vec::new() },
                DriverEvent::PlanVersionAdvanced {
                    from_plan_version: 1,
                    to_plan_version: 2,
                    carried_completions: Vec::new(),
                    carried_amendments: Vec::new(),
                    criterion_revisions_ratified_by: None,
                    criterion_revisions: Vec::new(),
                },
                DriverEvent::WorkerEnvironmentExtended {
                    plan_version: 3,
                    added_names: vec!["NEW".to_owned()],
                },
            ],
            vec![
                DriverEvent::WorkerEnvironmentDeclared { names: Vec::new() },
                DriverEvent::PlanVersionAdvanced {
                    from_plan_version: 1,
                    to_plan_version: 2,
                    carried_completions: Vec::new(),
                    carried_amendments: Vec::new(),
                    criterion_revisions_ratified_by: None,
                    criterion_revisions: Vec::new(),
                },
                DriverEvent::WorkerDispatched {
                    package: "A".to_owned(),
                    issuance: 1,
                },
                DriverEvent::WorkerEnvironmentExtended {
                    plan_version: 2,
                    added_names: vec!["NEW".to_owned()],
                },
            ],
        ] {
            assert!(matches!(
                derive_driver_snapshot(&graph, &events, false),
                Err(PackageDriverError::WorkerEnvironmentExtensionOutsidePlanBoundary { .. })
            ));
        }
    }

    #[test]
    fn restart_rederives_identical_environment_failures_and_blocks_at_configured_limit() {
        let graph = graph();
        let limits = RecoveryLimits::new(RetryLimit::new(1), LocalPatchLimit::new(1))
            .with_environment_failure_limit(EnvironmentFailureLimit::new(2));
        let mut events = vec![
            DriverEvent::RecoveryConfigured { limits },
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
        ];
        events.push(worker_environment_outcome(
            &events,
            limits,
            "A".to_owned(),
            1,
            "same environment".to_owned(),
        ));
        let journal = serde_json::to_vec(&events).expect("serialize journal");
        let mut restarted: Vec<DriverEvent> =
            serde_json::from_slice(&journal).expect("reload journal");
        restarted.push(DriverEvent::WorkerDispatched {
            package: "A".to_owned(),
            issuance: 2,
        });
        restarted.push(worker_environment_outcome(
            &restarted,
            limits,
            "A".to_owned(),
            2,
            "same environment".to_owned(),
        ));
        let snapshot = derive_driver_snapshot(&graph, &restarted, false).expect("snapshot");
        assert!(matches!(
            &snapshot.packages()[0].1,
            DriverPackageState::EnvironmentBlocked {
                reason,
                identical_failures: 2
            } if reason == "same environment"
        ));
        assert!(matches!(
            &snapshot.packages()[1].1,
            DriverPackageState::Pending
        ));
        assert_eq!(snapshot.outcome(), DriverLoopOutcome::Blocked);
        assert_eq!(charged_failure_count(&restarted, "A"), 0);
    }

    #[test]
    fn a_different_environment_reason_resets_the_identical_streak() {
        let limits = RecoveryLimits::new(RetryLimit::new(1), LocalPatchLimit::new(1))
            .with_environment_failure_limit(EnvironmentFailureLimit::new(2));
        let events = vec![DriverEvent::WorkerEnvironmentFailed {
            package: "A".to_owned(),
            issuance: 1,
            reason: "first cause".to_owned(),
        }];
        assert!(matches!(
            worker_environment_outcome(
                &events,
                limits,
                "A".to_owned(),
                2,
                "second cause".to_owned(),
            ),
            DriverEvent::WorkerEnvironmentFailed { .. }
        ));
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

        let preparation_failed = vec![
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            DriverEvent::WorkerDone {
                package: "A".to_owned(),
                issuance: 1,
            },
            DriverEvent::EnvironmentPreparationExecuted {
                package: "A".to_owned(),
                materialization: "criteria".to_owned(),
                repository: "r".to_owned(),
                command: "exit 9".to_owned(),
                outcome: super::EnvironmentPreparationOutcome::Failed,
                execution: super::CriterionExecution::new(
                    "exit 9".to_owned(),
                    "/clone".to_owned(),
                    super::CommandExitStatus::Exited { code: 9 },
                    String::new(),
                    String::new(),
                ),
            },
        ];
        let snapshot = derive_driver_snapshot(&graph, &preparation_failed, false)
            .expect("preparation failure fold");
        assert!(matches!(
            snapshot.packages()[0].1,
            DriverPackageState::EnvironmentPreparationFailed { .. }
        ));
        assert!(matches!(
            snapshot.packages()[1].1,
            DriverPackageState::Pending
        ));
        assert_eq!(snapshot.outcome(), DriverLoopOutcome::Blocked);
    }
    #[test]
    fn observed_spawn_failure_returns_package_to_restart_ready_state() {
        let graph = graph();
        let events = vec![
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            DriverEvent::WorkerSpawnFailed {
                package: "A".to_owned(),
                issuance: 1,
                reason: "Herdr refused before creating a child".to_owned(),
            },
        ];
        let snapshot = derive_driver_snapshot(&graph, &events, false).expect("spawn failure fold");
        let restarted = derive_driver_snapshot(&graph, &events, false).expect("restart fold");
        assert_eq!(snapshot, restarted);
        assert_eq!(restarted.ready(), &["A"]);
        assert!(matches!(
            restarted.packages()[0].1,
            DriverPackageState::Pending
        ));

        let unmatched = derive_driver_snapshot(&graph, &events[1..], false)
            .expect_err("spawn failure requires a matching request");
        assert!(matches!(
            unmatched,
            super::PackageDriverError::UnmatchedOutcome { issuance: 1, .. }
        ));
    }

    #[test]
    fn pane_cleanup_targets_only_the_completed_attempt_exact_ids() {
        let events = vec![
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            DriverEvent::DispatchPaneOpened {
                package: "A".to_owned(),
                issuance: 1,
                pane_id: "failed-attempt".to_owned(),
                workspace_id: "owned-workspace".to_owned(),
            },
            DriverEvent::WorkerFailed {
                package: "A".to_owned(),
                issuance: 1,
                reason: "retry".to_owned(),
            },
            DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 2,
            },
            DriverEvent::DispatchPaneOpened {
                package: "A".to_owned(),
                issuance: 2,
                pane_id: "completed-worker".to_owned(),
                workspace_id: "owned-workspace".to_owned(),
            },
            DriverEvent::WorkerDone {
                package: "A".to_owned(),
                issuance: 2,
            },
            DriverEvent::DispatchPaneOpened {
                package: "A".to_owned(),
                issuance: 2,
                pane_id: "completed-gate".to_owned(),
                workspace_id: "owned-workspace".to_owned(),
            },
            DriverEvent::PackageCompleted {
                package: "A".to_owned(),
            },
            DriverEvent::WorkerDispatched {
                package: "B".to_owned(),
                issuance: 3,
            },
            DriverEvent::DispatchPaneOpened {
                package: "B".to_owned(),
                issuance: 3,
                pane_id: "unrelated-running".to_owned(),
                workspace_id: "owned-workspace".to_owned(),
            },
        ];
        let pending = super::pending_completed_pane_cleanups(&events);
        assert_eq!(
            pending
                .iter()
                .map(super::PendingPaneCleanup::pane_id)
                .collect::<Vec<_>>(),
            ["completed-worker", "completed-gate"]
        );
        let mut accounted = events;
        accounted.push(DriverEvent::DispatchPaneCleanup {
            package: "A".to_owned(),
            issuance: 2,
            pane_id: "completed-worker".to_owned(),
            workspace_id: "owned-workspace".to_owned(),
            outcome: super::PaneCleanupOutcome::Failed,
            detail: "herdr unavailable".to_owned(),
        });
        assert_eq!(
            super::pending_completed_pane_cleanups(&accounted)
                .iter()
                .map(super::PendingPaneCleanup::pane_id)
                .collect::<Vec<_>>(),
            ["completed-gate"]
        );
    }

    #[test]
    fn composition_observations_remain_restart_dispatchable_and_uncharged() {
        let graph = graph();
        let base = vec![DriverEvent::PackageBaseComposed {
            package: "A".to_owned(),
            repository: "r".to_owned(),
            base_oid: "base-a".to_owned(),
            dependencies: Vec::new(),
        }];
        let snapshot = derive_driver_snapshot(&graph, &base, false).expect("composed base fold");
        assert!(matches!(
            snapshot.packages()[0].1,
            DriverPackageState::Pending
        ));
        assert_eq!(snapshot.ready(), &["A"]);

        let failed = vec![DriverEvent::PackageCompositionFailed {
            package: "A".to_owned(),
            repository: "r".to_owned(),
            dependencies: vec![CompositionInput {
                package: "dependency".to_owned(),
                oid: "deadbeef".to_owned(),
            }],
            reason: "merge conflict".to_owned(),
        }];
        let snapshot = derive_driver_snapshot(&graph, &failed, false).expect("failed composition");
        assert!(matches!(
            &snapshot.packages()[0].1,
            DriverPackageState::Pending
        ));
        assert_eq!(snapshot.ready(), &["A"]);
        assert_eq!(snapshot.outcome(), DriverLoopOutcome::Running);
        assert_eq!(charged_failure_count(&failed, "A"), 0);

        let conflicted = vec![DriverEvent::PackageJoinConflicted {
            package: "A".to_owned(),
            repository: "r".to_owned(),
            base_oid: "base".to_owned(),
            dependencies: vec![CompositionInput {
                package: "dependency".to_owned(),
                oid: "deadbeef".to_owned(),
            }],
            conflicting_input: CompositionInput {
                package: "dependency".to_owned(),
                oid: "deadbeef".to_owned(),
            },
            remaining_inputs: Vec::new(),
            conflicted_paths: vec!["src/lib.rs".to_owned()],
            reason: "CONFLICT (content): Merge conflict in src/lib.rs".to_owned(),
        }];
        let snapshot = derive_driver_snapshot(&graph, &conflicted, false).expect("conflict fold");
        assert!(matches!(
            snapshot.packages()[0].1,
            DriverPackageState::Pending
        ));
        assert_eq!(snapshot.ready(), &["A"]);
        assert_eq!(charged_failure_count(&conflicted, "A"), 0);
    }

    #[test]
    fn assembly_lifecycle_is_restart_derived_and_controls_graph_outcome() {
        let graph = graph();
        let mut events = completed_packages();
        let pending = derive_driver_snapshot(&graph, &events, false).expect("pending assembly");
        assert_eq!(pending.assembly(), &DriverAssemblyState::Pending);
        assert_eq!(pending.outcome(), DriverLoopOutcome::Running);

        events.push(DriverEvent::AssemblyRepositoryComposed {
            repository: "r".to_owned(),
            base_oid: "assembly".to_owned(),
            packages: vec![
                CompositionInput {
                    package: "A".to_owned(),
                    oid: "a".to_owned(),
                },
                CompositionInput {
                    package: "B".to_owned(),
                    oid: "b".to_owned(),
                },
            ],
        });
        events.push(DriverEvent::AssemblyCriterionExecuted {
            package: "A".to_owned(),
            name: "a".to_owned(),
            origin: CriterionOrigin::Authored,
            execution: CriterionExecution::new(
                "true".to_owned(),
                "/assembly/r".to_owned(),
                CommandExitStatus::Exited { code: 0 },
                String::new(),
                String::new(),
            ),
            amendment_proof: None,
        });
        let gating = derive_driver_snapshot(&graph, &events, false).expect("assembly gating");
        assert_eq!(gating.assembly(), &DriverAssemblyState::Gating);
        assert_eq!(gating.outcome(), DriverLoopOutcome::Running);

        events.push(DriverEvent::AssemblyCompleted);
        let completed = derive_driver_snapshot(&graph, &events, false).expect("assembly complete");
        let restarted = derive_driver_snapshot(&graph, &events, false).expect("restart fold");
        assert_eq!(completed, restarted);
        assert_eq!(completed.assembly(), &DriverAssemblyState::Complete);
        assert_eq!(completed.outcome(), DriverLoopOutcome::Finished);
    }

    #[test]
    fn assembly_composition_and_gating_failures_are_distinct_blocked_states() {
        let graph = graph();
        let mut composition_events = completed_packages();
        composition_events.push(DriverEvent::AssemblyCompositionFailed {
            repository: "r".to_owned(),
            packages: Vec::new(),
            reason: "packages conflict".to_owned(),
        });
        let composition = derive_driver_snapshot(&graph, &composition_events, false)
            .expect("assembly composition failure");
        assert_eq!(
            composition.assembly(),
            &DriverAssemblyState::Failed {
                reason: "packages conflict".to_owned(),
            }
        );
        assert_eq!(composition.outcome(), DriverLoopOutcome::Blocked);

        let mut gating_events = completed_packages();
        gating_events.push(DriverEvent::AssemblyRepositoryComposed {
            repository: "r".to_owned(),
            base_oid: "assembly".to_owned(),
            packages: Vec::new(),
        });
        gating_events.push(DriverEvent::AssemblyFailed {
            reason: "criterion a failed".to_owned(),
        });
        let gating = derive_driver_snapshot(&graph, &gating_events, false)
            .expect("assembly criterion failure");
        assert_eq!(
            gating.assembly(),
            &DriverAssemblyState::Failed {
                reason: "criterion a failed".to_owned(),
            }
        );
        assert_eq!(gating.outcome(), DriverLoopOutcome::Blocked);
    }
}
