//! Core domain logic for PCE workflows.

pub mod event_log;
pub mod run_state;
pub mod vision;

pub use event_log::{
    AppendError, AppendIntent, ArtifactPath, DeltaPayload, DispatchPayload, DispatchRef,
    DispatchRole, EscalationClosePayload, EscalationKey, EscalationOpenPayload, EventBodyRef,
    EventLogError, EventLogTail, EventLogTailLine, EventRecord, EventTimestamp, Evidence,
    EvidencePolicy, EvidencePresence, KeyFindingPayload, KnownPayload, NodeId,
    PlanningArtifactApprovedPayload, ReadKind, ReadPayload, RepositoryContractPayload,
    RepositoryName, RepositoryRoot, Sequence, Sha256Digest, UnparsedPayload, WriteKind,
    append_event, parse_event_line, serialize_event_line, validate_evidence_policy,
};
pub use run_state::{
    ArtifactProvenance, ArtifactProvenanceCondition, AuthorityFailure, BranchSnapshot, BranchState,
    CurrentArtifactObservation, CurrentArtifactState, CyclePosition, CyclePositionSnapshot,
    DerivedRunState, DispatchObservation, DispatchRoleClass, DispatchSnapshot,
    ExactMatchCardinality, ExactPullRequestIdentity, ExactPullRequestState,
    GitAuthorityObservation, GitHubAuthorityObservation, GitHubObservationSnapshot,
    GitHubPullRequestObservation, GitMergeObservation, GitObservationSnapshot, GitReachableState,
    HeadBranch, HoldObservation, HoldSnapshot, HoldStatus, HoldStatusSnapshot, IntegrationBranch,
    MergeStatus, MergeSubject, MergeSubjectSnapshot, MilestoneNumber, ProvenanceConditionSnapshot,
    ProvenanceSnapshot, PullRequestNumber, PullRequestSelector, PullRequestSnapshot,
    PullRequestStateSnapshot, RecoveryCategory, RecoveryDeltaEntry, RecoveryDigest,
    RecoveryElision, RecoveryFactEntry, RecoveryOpenHoldEntry, RecoveryRoundEntry,
    RepositoryBranchName, RepositoryFetchObservation, RepositoryFetchSnapshot,
    RepositoryObservation, RepositoryObservationFailure, RepositoryObservationRef,
    RepositorySnapshot, ResumeObservation, ResumeSnapshot, RoundClassification, RoundCount,
    RoundSeries, RoundSeriesSnapshot, RunSnapshot, RunStateError, SelectorSnapshot,
    SquashCommitOid, StepAuthorityObservation, StepMergeResult, StepNode, StepNumber, StepSnapshot,
    TagName, TagSnapshot, TagState, TagTarget, VisionSlug, WorktreeIdentity, WorktreeSnapshot,
    WorktreeState, derive_merge_status, derive_run_state,
};
pub use vision::{
    CreationDate, NewVision, Slug, VisionDir, VisionDirOutcome, VisionError, VisionName,
    create_vision, render_vision_stub,
};
