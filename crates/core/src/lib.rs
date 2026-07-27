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
    ArtifactProvenance, ArtifactProvenanceCondition, AuthorityFailure, BranchState,
    CurrentArtifactObservation, CurrentArtifactState, CyclePosition, DerivedRunState,
    DispatchObservation, DispatchRoleClass, ExactPullRequestIdentity, ExactPullRequestState,
    GitAuthorityObservation, GitHubAuthorityObservation, GitHubPullRequestObservation,
    GitMergeObservation, HeadBranch, HoldObservation, HoldStatus, IntegrationBranch, MergeStatus,
    MergeSubject, MilestoneNumber, PullRequestNumber, PullRequestSelector, RepositoryBranchName,
    RepositoryFetchObservation, RepositoryObservation, RepositoryObservationFailure,
    RepositoryObservationRef, ResumeObservation, RoundCount, RoundSeries, RunStateError,
    SquashCommitOid, StepAuthorityObservation, StepMergeResult, StepNode, StepNumber, TagName,
    TagState, TagTarget, VisionSlug, WorktreeIdentity, WorktreeState, derive_merge_status,
    derive_run_state,
};
pub use vision::{
    CreationDate, NewVision, Slug, VisionDir, VisionDirOutcome, VisionError, VisionName,
    create_vision, render_vision_stub,
};
