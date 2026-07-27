//! Core domain logic for PCE workflows.

pub mod event_log;
pub mod vision;

pub use event_log::{
    AppendError, AppendIntent, ArtifactPath, DeltaPayload, DispatchPayload, DispatchRef,
    DispatchRole, EscalationClosePayload, EscalationKey, EscalationOpenPayload, EventLogError,
    EventLogTail, EventLogTailLine, EventRecord, EventTimestamp, Evidence, EvidencePolicy,
    EvidencePresence, KeyFindingPayload, KnownPayload, NodeId, PlanningArtifactApprovedPayload,
    ReadKind, ReadPayload, RepositoryContractPayload, RepositoryName, RepositoryRoot, Sequence,
    Sha256Digest, UnparsedPayload, WriteKind, append_event, parse_event_line, serialize_event_line,
    validate_evidence_policy,
};
pub use vision::{
    CreationDate, NewVision, Slug, VisionDir, VisionDirOutcome, VisionError, VisionName,
    create_vision, render_vision_stub,
};
