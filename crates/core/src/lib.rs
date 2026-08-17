//! Core domain logic for PCE workflows.

pub mod acceptance_criteria;
pub mod artifact_validation;
pub mod completion_gate;
pub mod contract_measurement;
pub mod criterion_change;
pub mod dispatch;
pub mod dispatch_check_in;
pub mod dispatch_ledger;
pub mod dispatch_meter;
pub mod dispatch_process_identity;
pub mod event_log;
pub mod gate_execution;
pub mod gate_replay;
pub mod herdr_dispatch;
pub mod landing_readiness;
pub mod package_completion;
pub mod package_driver;
pub mod package_gate;
pub mod package_recovery;
pub mod package_worker;
pub mod paired_execution_proof;
pub mod run_render;
pub mod run_state;
pub mod tracked_contract;
pub mod vision;
pub mod work_package_graph;
pub mod workflow_coverage;

pub use acceptance_criteria::{
    AcceptanceCriteria, AcceptanceCriteriaError, AcceptanceCriterion, CriterionField,
    CriterionInput, CriterionName, CriterionObservation, parse_acceptance_criteria,
};
pub use artifact_validation::{
    ArtifactValidationError, FileObservation, StructuredArtifactObservation, validate_artifact,
};
pub use completion_gate::{
    CompletionCriterionReport, CompletionCriterionStatus, CompletionDecision, CompletionGateResult,
    evaluate_completion,
};
pub use contract_measurement::{
    ContractMeasurementError, GateMeasurement, GateMeasurements, MeasuredContractSnapshot,
    ObservedExitStatus, measure_contract_snapshot,
};
pub use criterion_change::{
    CriterionChangeDecision, CriterionChangeVerification, verify_criterion_change,
};
pub use package_completion::{
    AbsolutePackageResultPath, PackageCompletionError, PackageWorkerResult, PackageWorkerStoppedAt,
    compose_package_worker_argv, derive_package_result_path, parse_package_worker_result,
    serialize_package_worker_result,
};
pub use package_driver::{
    AmendmentRepositoryRefs, CommandExitStatus, CompositionInput, CriterionExecution,
    CriterionOrigin, DriverAssemblyState, DriverEvent, DriverLoopOutcome, DriverPackageState,
    DriverSnapshot, EffectiveCriterion, EnvironmentPreparationOutcome, FindingRejectionReason,
    FindingReplayDecision, PackageDriverError, PaneCleanupOutcome, PendingPaneCleanup,
    RecoveryAttemptRecord, charged_failure_count, derive_driver_snapshot, effective_criteria,
    judge_finding_replay, latest_criterion_failure_evidence, pending_completed_pane_cleanups,
    recovery_attempt_records, recovery_base_brief, worker_environment_outcome,
};
pub use package_gate::{
    BuiltArtifactRef, PackageGateError, PackageGateFinding, PackageGateRepositoryRefs,
    ParsedPackageGateOutcome, compose_package_gate_brief, parse_package_gate_outcome,
    validate_package_gate_finding_repositories, validate_package_gate_repositories,
};
pub use package_recovery::{
    EnvironmentFailureLimit, LocalPatchLimit, RecoveryBudget, RecoveryCriterionEvidence,
    RecoveryLimits, RecoveryRung, RetryLimit, compose_local_patch_brief, recovery_budget,
};
pub use package_worker::{
    MisSpecificationFault, PackageOutcome, PackageWorkerError, RepositoryWorktree, VisionGoal,
    compose_package_worker_brief, parse_package_outcome,
};
pub use paired_execution_proof::{
    CampaignSide, FalsificationVerdictToken, PairedBlockingIssue, PairedCampaign,
    PairedExecutionProofError, PairedExecutionProofResult, PairedFalsificationVerdict,
    PairedProofDecision, PairedRefusalReason, PairedReplayClassification, PairedStimulusIdentity,
    ReferenceValidation, ReplayClassifications, fold_paired_execution_proof,
    paired_stimulus_identity, parse_paired_falsification_verdict,
};
pub use run_render::{RunRenderError, render_package_run};

/// Exact diagnostic emitted when a nested Seatbelt capability probe is denied.
pub const NESTED_SEATBELT_SKIP_MARKER: &str =
    "PCE_TEST_SKIP: nested Seatbelt unavailable; permissive capability probe was denied";
pub use dispatch::{
    AbsoluteOutputPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory, ActReversibility,
    ArgumentVector, ChildEnvironment, ClaudeResultEnvelope, ClaudeResultUsage,
    CodexTerminalObservation, CodexTerminalUsage, Deferred, DispatchEnvelope, DispatchError,
    DispatchInvocation, DispatchInvocationStdin, DispatchLogging, DispatchProjectionError,
    DispatchProjectionInput, DispatchTarget, Executable, PlanningFrameError, Sandbox,
    SeatbeltCapability, StdinBinding, classify_claude_result, classify_codex_terminal_usage,
    classify_seatbelt_capability, compose_gate_arguments, compose_planning_role_frame,
    dispatch_completion_payload, dispatch_invocation, dispatch_payload, parse_claude_result,
    render_dispatch_projection, seatbelt_capability_probe, validated_dispatch_completion_payload,
};
pub use dispatch_check_in::{
    DispatchCheckInEntry, DispatchCheckInError, DispatchCheckInReport,
    DispatchCompletionAccounting, DispatchIdentityObservation, DispatchLiveness,
    ProcessIdentityObservation, classify_dispatch_check_in, serialize_dispatch_check_in,
};
pub use dispatch_ledger::{
    DispatchAccounting, DispatchLedger, DispatchLedgerCompletion, DispatchLedgerEntry,
    DispatchLedgerError, DispatchLedgerIssuance, UnaccountedDispatchLedger, fold_dispatch_ledger,
};
pub use dispatch_meter::{
    DispatchMeterCompletion, DispatchMeterError, DispatchMeterIssuance, DispatchMeterRecord,
    ObservedDispatchMeterCompletion, ReconciledDeadDispatchMeterCompletion, meter_dispatches,
};
pub use dispatch_process_identity::{
    AbsoluteRequiredArtifactPath, DispatchProcessIdentity, DispatchProcessIdentityError,
    DispatchProcessIdentityExpectation, ProcessNumber, ProcessStartIdentity,
    RecordedProcessIdentity, parse_dispatch_process_identity,
    require_dispatch_process_identity_match, serialize_dispatch_process_identity,
};
pub use event_log::{
    AppendError, AppendIntent, AppendableRepositoryContract, ArtifactOutcome, ArtifactPath,
    ArtifactProduction, CacheCreationInputTokens, CacheReadInputTokens, CachedInputTokens,
    ChangeOfCourse, CriterionAddedPayload, CriterionExecutionOutcome, CriterionExecutionPayload,
    DeltaPayload, DispatchCompletionOutcomeRef, DispatchCompletionPayload, DispatchDuration,
    DispatchExitStatus, DispatchPayload, DispatchRef, DispatchRole, DispatchRootCause,
    DispatchTokenUsage, EscalationClosePayload, EscalationKey, EscalationOpenPayload, EventBodyRef,
    EventKindName, EventLogError, EventLogTail, EventLogTailError, EventLogTailLine, EventRecord,
    EventRecordFilter, EventTimestamp, Evidence, EvidencePolicy, EvidencePresence,
    ExceptionalMergeChainDeclaredPayload, ExitCode, FinishedResult, GateObservations, InputTokens,
    KeyFindingPayload, KnownPayload, LegacyRepositoryContractPayload, NodeId,
    NonProductionHoldClosePayload, NonProductionHoldOpenPayload, NonProductionHoldResolution,
    NonProductionKey, ObservedCriterionResult, ObservedDispatchCompletionPayload,
    ObservedDispatchCompletionWithArtifactPresencePayload, OutputTokens,
    PlanningArtifactApprovedPayload, ReadKind, ReadPayload, ReasoningOutputTokens,
    ReconciledDeadDispatchCompletionPayload, ReconciledDispatchOutcome, RepositoryContractPayload,
    RepositoryName, RepositoryRoot, RequiredArtifactPresence, Sequence, Sha256Digest, SignalNumber,
    SpawnDispatchOutcome, SpawnFailedDispatchCompletionPayload, StatedRepositoryContract,
    UnpaidCriterionReason, UnparsedPayload, UsageAbsenceReason, WorkflowMap, WriteKind,
    append_event, event_record_matches, parse_event_line, serialize_event_line, successor_sequence,
    validate_evidence_policy,
};
pub use gate_execution::{
    AbsoluteGateExecClientPath, AbsoluteGateExecutionEvidencePath, AbsoluteGateExecutionSocketPath,
    GateExecutionError, GateExecutionEvidence, GateExecutionRecord, GateExecutionRecorderConfig,
    GateExecutionRef, GateExecutionRejection, GateExecutionResponse, GateObservedResult,
    GateProcessObservation, GateProcessStimulus, GateStimulus, GateTerminalStatus,
    parse_gate_execution_evidence, parse_gate_stimulus, validate_verdict_references,
};
pub use gate_replay::{
    ArtifactConformance, CheckoutFailure, CheckoutStage, ExpectedMatch, ExpectedVerdictOutcome,
    GateReplayError, NamedReplayRef, OracleFailure, OracleStage, RepairSensitivity,
    ReplayArtifactObservation, ReplayObservation, ReplayRefOutcome, ReplayRefResult,
    RepositoryRelativePath, classify_replay_pair, fold_replay_runs, normalize_replay_observation,
    parse_replay_output_path, parse_replay_schema_path, rebase_gate_stimulus,
};
pub use herdr_dispatch::{
    AbsoluteDispatchTemporaryDirectory, AbsoluteWorktreeRoot, DispatchAttempt,
    DispatchVisionSource, HerdrAgentLocation, HerdrAgentName, HerdrDispatchPlanError,
    HerdrInvocation, HerdrPaneId, HerdrTabId, HerdrWorkPackageDispatchPlan, HerdrWorkspaceId,
    HerdrWorktreeSpec, RepositoryDispatchInput, WorkerArgumentVector, WorkerEnvironment,
    compose_herdr_work_package_dispatch,
};
pub use landing_readiness::{
    CompletionCriterionIndex, LandingCriterionEvidence, LandingReadinessDecision,
    LandingReadinessProblem, LandingReadinessResult, evaluate_landing_readiness,
};
pub use run_state::{
    ArtifactProvenance, ArtifactProvenanceCondition, AuthorityFailure, BlockingCriterion,
    BlockingCriterionOrigin, BranchSnapshot, BranchState, CanonicalNode, ConsecutiveNonProduction,
    CriterionExecutionObservation, CurrentArtifactObservation, CurrentArtifactState, CyclePosition,
    CyclePositionSnapshot, DeclaredIntegrationBranch, DefectRoundCount, DefectRoundSeries,
    DefectRoundSeriesSnapshot, DerivedRunState, DispatchAdmission, DispatchCandidate,
    DispatchLifecycleObservation, DispatchObservation, DispatchOutcomeState,
    DispatchRequiredArtifactObservation, DispatchRoleClass, DispatchSnapshot,
    DispatchabilityResult, ExactMatchCardinality, ExactPullRequestIdentity, ExactPullRequestState,
    ExceptionalMergeChain, ExceptionalMergeChainObservation, GitAuthorityObservation,
    GitHubAuthorityObservation, GitHubObservationSnapshot, GitHubPullRequestObservation,
    GitMergeObservation, GitObservationSnapshot, GitReachableState, HeadBranch, HoldObservation,
    HoldSnapshot, HoldStatus, HoldStatusSnapshot, IntegrationBranch, IssuanceOrdinal,
    IssuanceOrdinalSeries, IssuanceOrdinalSeriesSnapshot, MergeStatus, MergeSubject,
    MergeSubjectSnapshot, MilestoneMergeSubject, MilestoneNode, MilestoneNumber,
    NonProductionHoldObservation, NonProductionHoldSnapshot, NonProductionHoldStatus,
    NonProductionHoldStatusSnapshot, NonProductionSeries, NonProductionSeriesSnapshot,
    ObservedDispatchLifecycleObservation, OrderingEdge, ProvenanceConditionSnapshot,
    ProvenanceSnapshot, PullRequestAuthorityObservation, PullRequestNumber, PullRequestSelector,
    PullRequestSnapshot, PullRequestStateSnapshot, ReconciledDeadDispatchLifecycleObservation,
    RecoveryCategory, RecoveryDeltaEntry, RecoveryDigest, RecoveryElision, RecoveryFactEntry,
    RecoveryLogPath, RecoveryOpenHoldEntry, RecoveryRoundEntry, RepositoryBranchName,
    RepositoryFetchObservation, RepositoryFetchSnapshot, RepositoryObservation,
    RepositoryObservationFailure, RepositoryObservationRef, RepositorySnapshot, ResumeObservation,
    ResumeSnapshot, RoundClassification, RunSnapshot, RunStateError, SelectorSnapshot,
    SquashCommitOid, StepAuthorityObservation, StepMergeResult, StepMergeRoute, StepNode,
    StepNumber, StepSnapshot, TagName, TagSnapshot, TagState, TagTarget, ValidatedProductionCount,
    ValidatedProductionResumeState, ValidatedProductionSeries,
    ValidatedProductionSpendingSeriesSnapshot, ValidatedProductionSpendingSnapshot,
    ValidatedProductionSpendingState, VersionPolicy, VisionSlug, WorkPackageMergeSubject,
    WorktreeIdentity, WorktreeSnapshot, WorktreeState, classify_dispatch_admission,
    compute_dispatchability, derive_dispatch_outcome_state, derive_merge_status,
    derive_milestone_merge_status, derive_run_state, derive_run_state_with_dispatch_artifacts,
    derive_run_state_with_exceptional_merge_chains, derive_work_package_merge_status,
    render_human_snapshot,
};
pub use tracked_contract::{
    AppendableCategory, AppendableContract, AppendableFinding, BranchConvention, DefaultBranchName,
    EnvironmentHazard, FindingAdmission, GateCommand, GateCommands, GateKind, GateOrdering,
    LocalWorkflowStandIn, LockfileRule, MilestoneBranchPattern, MilestonePullRequestBase,
    PullRequestConvention, PullRequestMergeMethod, StatedContract, StepBranchPattern,
    StepPullRequestBase, TrackedContractError, TrackedRepositoryContract, WorkflowMapping,
    WorkflowMappings, WorkflowName, admit_recurrent_finding, parse_tracked_repository_contract,
    serialize_tracked_repository_contract,
};
pub use vision::{
    CreationDate, NewVision, Slug, VisionDir, VisionDirOutcome, VisionError, VisionName,
    create_vision, render_vision_stub,
};
pub use workflow_coverage::{
    ObservedWorkflowName, ObservedWorkflowNameError, WorkflowCoverageError,
    validate_workflow_coverage,
};

pub use work_package_graph::{
    ClassifiedWorkPackage, DependencyKind, ReadyWorkPackages, RiskOrdering, WorkPackage,
    WorkPackageClassification, WorkPackageCriterion, WorkPackageDependency, WorkPackageGraph,
    WorkPackageGraphError, WorkPackageId, WorkPackageMergeObservation, parse_work_package_graph,
    ready_work_packages, unchanged_package_ids,
};
