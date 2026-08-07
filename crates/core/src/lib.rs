//! Core domain logic for PCE workflows.

pub mod acceptance_criteria;
pub mod artifact_validation;
pub mod contract_measurement;
pub mod dispatch;
pub mod dispatch_meter;
pub mod event_log;
pub mod gate_execution;
pub mod gate_replay;
pub mod paired_execution_proof;
pub mod run_state;
pub mod tracked_contract;
pub mod vision;
pub mod workflow_coverage;

pub use acceptance_criteria::{
    AcceptanceCriteria, AcceptanceCriteriaError, AcceptanceCriterion, CriterionField,
    CriterionInput, CriterionName, CriterionObservation, parse_acceptance_criteria,
};
pub use artifact_validation::{
    ArtifactValidationError, FileObservation, StructuredArtifactObservation, validate_artifact,
};
pub use contract_measurement::{
    ContractMeasurementError, GateMeasurement, GateMeasurements, MeasuredContractSnapshot,
    ObservedExitStatus, measure_contract_snapshot,
};
pub use paired_execution_proof::{
    CampaignSide, FalsificationVerdictToken, PairedBlockingIssue, PairedCampaign,
    PairedExecutionProofError, PairedExecutionProofResult, PairedFalsificationVerdict,
    PairedProofDecision, PairedRefusalReason, PairedReplayClassification, PairedStimulusIdentity,
    ReferenceValidation, ReplayClassifications, fold_paired_execution_proof,
    paired_stimulus_identity, parse_paired_falsification_verdict,
};

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
    render_dispatch_projection, seatbelt_capability_probe,
};
pub use dispatch_meter::{
    DispatchMeterCompletion, DispatchMeterError, DispatchMeterIssuance, DispatchMeterRecord,
    meter_dispatches,
};
pub use event_log::{
    AppendError, AppendIntent, AppendableRepositoryContract, ArtifactOutcome, ArtifactPath,
    CacheCreationInputTokens, CacheReadInputTokens, CachedInputTokens, ChangeOfCourse,
    CriterionAddedPayload, CriterionExecutionOutcome, CriterionExecutionPayload, DeltaPayload,
    DispatchCompletionPayload, DispatchDuration, DispatchExitStatus, DispatchPayload, DispatchRef,
    DispatchRole, DispatchTokenUsage, EscalationClosePayload, EscalationKey, EscalationOpenPayload,
    EventBodyRef, EventKindName, EventLogError, EventLogTail, EventLogTailError, EventLogTailLine,
    EventRecord, EventRecordFilter, EventTimestamp, Evidence, EvidencePolicy, EvidencePresence,
    ExitCode, FinishedResult, GateObservations, InputTokens, KeyFindingPayload, KnownPayload,
    LegacyRepositoryContractPayload, NodeId, ObservedCriterionResult, OutputTokens,
    PlanningArtifactApprovedPayload, ReadKind, ReadPayload, ReasoningOutputTokens,
    RepositoryContractPayload, RepositoryName, RepositoryRoot, Sequence, Sha256Digest,
    SignalNumber, StatedRepositoryContract, UnpaidCriterionReason, UnparsedPayload,
    UsageAbsenceReason, WorkflowMap, WriteKind, append_event, event_record_matches,
    parse_event_line, serialize_event_line, successor_sequence, validate_evidence_policy,
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
pub use run_state::{
    ArtifactProvenance, ArtifactProvenanceCondition, AuthorityFailure, BlockingCriterion,
    BlockingCriterionOrigin, BranchSnapshot, BranchState, CanonicalNode,
    CriterionExecutionObservation, CurrentArtifactObservation, CurrentArtifactState, CyclePosition,
    CyclePositionSnapshot, DerivedRunState, DispatchCandidate, DispatchLifecycleObservation,
    DispatchObservation, DispatchRoleClass, DispatchSnapshot, DispatchabilityResult,
    ExactMatchCardinality, ExactPullRequestIdentity, ExactPullRequestState,
    GitAuthorityObservation, GitHubAuthorityObservation, GitHubObservationSnapshot,
    GitHubPullRequestObservation, GitMergeObservation, GitObservationSnapshot, GitReachableState,
    HeadBranch, HoldObservation, HoldSnapshot, HoldStatus, HoldStatusSnapshot, IntegrationBranch,
    MergeStatus, MergeSubject, MergeSubjectSnapshot, MilestoneMergeSubject, MilestoneNode,
    MilestoneNumber, OrderingEdge, ProvenanceConditionSnapshot, ProvenanceSnapshot,
    PullRequestNumber, PullRequestSelector, PullRequestSnapshot, PullRequestStateSnapshot,
    RecoveryCategory, RecoveryDeltaEntry, RecoveryDigest, RecoveryElision, RecoveryFactEntry,
    RecoveryLogPath, RecoveryOpenHoldEntry, RecoveryRoundEntry, RepositoryBranchName,
    RepositoryFetchObservation, RepositoryFetchSnapshot, RepositoryObservation,
    RepositoryObservationFailure, RepositoryObservationRef, RepositorySnapshot, ResumeObservation,
    ResumeSnapshot, RoundClassification, RoundCount, RoundSeries, RoundSeriesSnapshot, RunSnapshot,
    RunStateError, SelectorSnapshot, SquashCommitOid, StepAuthorityObservation, StepMergeResult,
    StepNode, StepNumber, StepSnapshot, TagName, TagSnapshot, TagState, TagTarget, VersionPolicy,
    VisionSlug, WorktreeIdentity, WorktreeSnapshot, WorktreeState, compute_dispatchability,
    derive_merge_status, derive_milestone_merge_status, derive_run_state, render_human_snapshot,
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
