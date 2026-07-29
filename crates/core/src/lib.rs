//! Core domain logic for PCE workflows.

pub mod contract_measurement;
pub mod event_log;
pub mod run_state;
pub mod tracked_contract;
pub mod vision;
pub mod workflow_coverage;

pub use contract_measurement::{
    ContractMeasurementError, GateMeasurement, GateMeasurements, MeasuredContractSnapshot,
    ObservedExitStatus, measure_contract_snapshot,
};
pub use event_log::{
    AppendError, AppendIntent, ArtifactPath, DeltaPayload, DispatchPayload, DispatchRef,
    DispatchRole, EscalationClosePayload, EscalationKey, EscalationOpenPayload, EventBodyRef,
    EventKindName, EventLogError, EventLogTail, EventLogTailLine, EventRecord, EventRecordFilter,
    EventTimestamp, Evidence, EvidencePolicy, EvidencePresence, KeyFindingPayload, KnownPayload,
    NodeId, PlanningArtifactApprovedPayload, ReadKind, ReadPayload, RepositoryContractPayload,
    RepositoryName, RepositoryRoot, Sequence, Sha256Digest, UnparsedPayload, WriteKind,
    append_event, event_record_matches, parse_event_line, serialize_event_line,
    validate_evidence_policy,
};
pub use run_state::{
    ArtifactProvenance, ArtifactProvenanceCondition, AuthorityFailure, BranchSnapshot, BranchState,
    CanonicalNode, CurrentArtifactObservation, CurrentArtifactState, CyclePosition,
    CyclePositionSnapshot, DerivedRunState, DispatchCandidate, DispatchObservation,
    DispatchRoleClass, DispatchSnapshot, DispatchabilityResult, ExactMatchCardinality,
    ExactPullRequestIdentity, ExactPullRequestState, GitAuthorityObservation,
    GitHubAuthorityObservation, GitHubObservationSnapshot, GitHubPullRequestObservation,
    GitMergeObservation, GitObservationSnapshot, GitReachableState, HeadBranch, HoldObservation,
    HoldSnapshot, HoldStatus, HoldStatusSnapshot, IntegrationBranch, MergeStatus, MergeSubject,
    MergeSubjectSnapshot, MilestoneMergeSubject, MilestoneNode, MilestoneNumber, OrderingEdge,
    ProvenanceConditionSnapshot, ProvenanceSnapshot, PullRequestNumber, PullRequestSelector,
    PullRequestSnapshot, PullRequestStateSnapshot, RecoveryCategory, RecoveryDeltaEntry,
    RecoveryDigest, RecoveryElision, RecoveryFactEntry, RecoveryLogPath, RecoveryOpenHoldEntry,
    RecoveryRoundEntry, RepositoryBranchName, RepositoryFetchObservation, RepositoryFetchSnapshot,
    RepositoryObservation, RepositoryObservationFailure, RepositoryObservationRef,
    RepositorySnapshot, ResumeObservation, ResumeSnapshot, RoundClassification, RoundCount,
    RoundSeries, RoundSeriesSnapshot, RunSnapshot, RunStateError, SelectorSnapshot,
    SquashCommitOid, StepAuthorityObservation, StepMergeResult, StepNode, StepNumber, StepSnapshot,
    TagName, TagSnapshot, TagState, TagTarget, VersionPolicy, VisionSlug, WorktreeIdentity,
    WorktreeSnapshot, WorktreeState, compute_dispatchability, derive_merge_status,
    derive_milestone_merge_status, derive_run_state, render_human_snapshot,
};
pub use tracked_contract::{
    AppendableContract, BranchConvention, DefaultBranchName, EnvironmentHazard, GateCommand,
    GateCommands, GateKind, GateOrdering, LocalWorkflowStandIn, LockfileRule,
    MilestoneBranchPattern, MilestonePullRequestBase, PullRequestConvention,
    PullRequestMergeMethod, StatedContract, StepBranchPattern, StepPullRequestBase,
    TrackedContractError, TrackedRepositoryContract, WorkflowMapping, WorkflowMappings,
    WorkflowName, parse_tracked_repository_contract,
};
pub use vision::{
    CreationDate, NewVision, Slug, VisionDir, VisionDirOutcome, VisionError, VisionName,
    create_vision, render_vision_stub,
};
pub use workflow_coverage::{
    ObservedWorkflowName, ObservedWorkflowNameError, WorkflowCoverageError,
    validate_workflow_coverage,
};
