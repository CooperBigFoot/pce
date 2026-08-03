//! Core domain logic for PCE workflows.

pub mod acceptance_criteria;
pub mod artifact_validation;
pub mod contract_measurement;
pub mod dispatch;
pub mod dispatch_meter;
pub mod event_log;
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

/// Exact diagnostic emitted when a nested Seatbelt capability probe is denied.
pub const NESTED_SEATBELT_SKIP_MARKER: &str =
    "PCE_TEST_SKIP: nested Seatbelt unavailable; permissive capability probe was denied";
pub use dispatch::{
    AbsoluteOutputPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory, ArgumentVector,
    ChildEnvironment, ClaudeResultEnvelope, ClaudeResultUsage, CodexTerminalObservation,
    CodexTerminalUsage, Deferred, DispatchEnvelope, DispatchError, DispatchInvocation,
    DispatchInvocationStdin, DispatchLogging, DispatchProjectionError, DispatchProjectionInput,
    DispatchTarget, Executable, Sandbox, SeatbeltCapability, StdinBinding, classify_claude_result,
    classify_codex_terminal_usage, classify_seatbelt_capability, dispatch_completion_payload,
    dispatch_invocation, dispatch_payload, parse_claude_result, render_dispatch_projection,
    seatbelt_capability_probe,
};
pub use dispatch_meter::{
    DispatchMeterCompletion, DispatchMeterError, DispatchMeterIssuance, DispatchMeterRecord,
    meter_dispatches,
};
pub use event_log::{
    AppendError, AppendIntent, AppendableRepositoryContract, ArtifactOutcome, ArtifactPath,
    CacheCreationInputTokens, CacheReadInputTokens, CachedInputTokens, DeltaPayload,
    DispatchCompletionPayload, DispatchDuration, DispatchExitStatus, DispatchPayload, DispatchRef,
    DispatchRole, DispatchTokenUsage, EscalationClosePayload, EscalationKey, EscalationOpenPayload,
    EventBodyRef, EventKindName, EventLogError, EventLogTail, EventLogTailError, EventLogTailLine,
    EventRecord, EventRecordFilter, EventTimestamp, Evidence, EvidencePolicy, EvidencePresence,
    ExitCode, GateObservations, InputTokens, KeyFindingPayload, KnownPayload,
    LegacyRepositoryContractPayload, NodeId, OutputTokens, PlanningArtifactApprovedPayload,
    ReadKind, ReadPayload, ReasoningOutputTokens, RepositoryContractPayload, RepositoryName,
    RepositoryRoot, Sequence, Sha256Digest, SignalNumber, StatedRepositoryContract,
    UnparsedPayload, UsageAbsenceReason, WorkflowMap, WriteKind, append_event,
    event_record_matches, parse_event_line, serialize_event_line, successor_sequence,
    validate_evidence_policy,
};
pub use run_state::{
    ArtifactProvenance, ArtifactProvenanceCondition, AuthorityFailure, BranchSnapshot, BranchState,
    CanonicalNode, CurrentArtifactObservation, CurrentArtifactState, CyclePosition,
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
