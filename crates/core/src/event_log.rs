//! decode : EventLogLine → KnownEvent ∪ UnknownEvent; append : AppendInput → AppendIntent.
//! This module is pure domain logic and performs no I/O.

use std::fmt;
use std::time::SystemTime;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use thiserror::Error;
use tracing::instrument;

/// Exact unparsed JSON submitted as the payload for one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnparsedPayload(String);

impl UnparsedPayload {
    /// Wrap owned payload text before it crosses into event-log domain logic.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// One supplied physical event-log tail line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLogTailLine(String);

impl EventLogTailLine {
    /// Wrap an owned tail line for complete validation by the append path.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

/// The supplied state at the end of an event log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventLogTail {
    /// The log contains no records.
    Empty,
    /// The log ends with the supplied physical record.
    Present(EventLogTailLine),
}

/// Validated bytes for exactly one compact, newline-terminated event record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendIntent(Vec<u8>);

impl AppendIntent {
    /// Borrow the complete bytes to pass to an append capability.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// The first-based position of an event in the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct Sequence(u64);

impl Sequence {
    /// Parse a first-based event sequence.
    ///
    /// # Errors
    ///
    /// Returns [`EventLogError::ZeroSequence`] when `value` is zero.
    #[instrument]
    pub fn parse(value: u64) -> Result<Self, EventLogError> {
        if value == 0 {
            return Err(EventLogError::ZeroSequence { value });
        }
        Ok(Self(value))
    }

    /// Return the first legal sequence.
    pub const fn first() -> Self {
        Self(1)
    }

    /// Return the numeric sequence.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for Sequence {
    type Error = EventLogError;

    #[instrument]
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<Sequence> for u64 {
    fn from(value: Sequence) -> Self {
        value.get()
    }
}

/// A UTC event timestamp with canonical millisecond JSON serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventTimestamp(DateTime<Utc>);

impl EventTimestamp {
    /// Construct a timestamp from an already-UTC value.
    pub const fn new(value: DateTime<Utc>) -> Self {
        Self(value)
    }

    /// Parse RFC 3339 input and convert it to UTC.
    ///
    /// # Errors
    ///
    /// Returns [`EventLogError::MalformedTimestamp`] when `raw` is not RFC 3339.
    #[instrument]
    pub fn parse(raw: &str) -> Result<Self, EventLogError> {
        DateTime::parse_from_rfc3339(raw)
            .map(|value| Self(value.with_timezone(&Utc)))
            .map_err(|_| EventLogError::MalformedTimestamp {
                input: raw.to_owned(),
            })
    }

    /// Return the underlying UTC timestamp.
    pub const fn as_datetime(&self) -> &DateTime<Utc> {
        &self.0
    }
}

impl Serialize for EventTimestamp {
    #[instrument(skip(serializer))]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_rfc3339_opts(SecondsFormat::Millis, true))
    }
}

impl<'de> Deserialize<'de> for EventTimestamp {
    #[instrument(skip(deserializer))]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// A non-empty node identifier, preserved byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NodeId(String);

impl NodeId {
    /// Parse an unnormalized node identifier.
    ///
    /// # Errors
    ///
    /// Returns [`EventLogError::EmptyNode`] only when `raw` has zero bytes.
    #[instrument]
    pub fn parse(raw: &str) -> Result<Self, EventLogError> {
        if raw.is_empty() {
            return Err(EventLogError::EmptyNode {
                value: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the node identifier unchanged.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NodeId {
    type Error = EventLogError;

    #[instrument]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<NodeId> for String {
    fn from(value: NodeId) -> Self {
        value.0
    }
}

/// A non-empty invocation string supporting arbitrary content and newlines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Evidence(String);

impl Evidence {
    /// Parse an invocation without trimming or syntax restrictions.
    ///
    /// # Errors
    ///
    /// Returns [`EventLogError::EmptyEvidence`] only when `raw` has zero bytes.
    #[instrument(skip(raw))]
    pub fn parse(raw: &str) -> Result<Self, EventLogError> {
        if raw.is_empty() {
            return Err(EventLogError::EmptyEvidence {
                value: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the invocation unchanged.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Evidence {
    type Error = EventLogError;

    #[instrument(skip(value))]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Evidence> for String {
    fn from(value: Evidence) -> Self {
        value.0
    }
}

macro_rules! string_domain_type {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Construct the domain value without normalization.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Return the stored value unchanged.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

string_domain_type!(DispatchRole, "The role receiving a dispatch.");
string_domain_type!(DispatchRef, "The exact repository ref for a dispatch.");
string_domain_type!(EscalationKey, "The stable key identifying an escalation.");
string_domain_type!(RepositoryName, "The repository's contract name.");
string_domain_type!(RepositoryRoot, "The measured repository root.");
string_domain_type!(
    ArtifactPath,
    "The approved planning artifact's identity path."
);

/// A lowercase hexadecimal SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Parse exactly 64 lowercase hexadecimal characters.
    ///
    /// # Errors
    ///
    /// Returns [`EventLogError::InvalidSha256Digest`] when the spelling is not canonical.
    #[instrument]
    pub fn parse(raw: &str) -> Result<Self, EventLogError> {
        let valid = raw.len() == 64
            && raw
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !valid {
            return Err(EventLogError::InvalidSha256Digest {
                digest: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the canonical digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Sha256Digest {
    type Error = EventLogError;

    #[instrument]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<Sha256Digest> for String {
    fn from(value: Sha256Digest) -> Self {
        value.0
    }
}

/// The exact closed registry of kinds accepted for writing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WriteKind {
    /// A role dispatch at an exact repository ref.
    Dispatch,
    /// A change in run understanding.
    Delta,
    /// A newly opened keyed escalation.
    EscalationOpen,
    /// The resolution of a keyed escalation.
    EscalationClose,
    /// A repository-backed finding.
    KeyFinding,
    /// A measured repository contract.
    RepositoryContract,
    /// The identity of an approved planning artifact.
    PlanningArtifactApproved,
}

impl WriteKind {
    /// Parse an exact registered on-disk kind for writing.
    ///
    /// # Errors
    ///
    /// Returns [`EventLogError::UnknownWriteKind`] for every unregistered spelling.
    #[instrument]
    pub fn parse(raw: &str) -> Result<Self, EventLogError> {
        match raw {
            "dispatch" => Ok(Self::Dispatch),
            "delta" => Ok(Self::Delta),
            "escalation-open" => Ok(Self::EscalationOpen),
            "escalation-close" => Ok(Self::EscalationClose),
            "key-finding" => Ok(Self::KeyFinding),
            "repository-contract" => Ok(Self::RepositoryContract),
            "planning-artifact-approved" => Ok(Self::PlanningArtifactApproved),
            _ => Err(EventLogError::UnknownWriteKind {
                kind: raw.to_owned(),
            }),
        }
    }

    /// Return the exact registered on-disk spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::Delta => "delta",
            Self::EscalationOpen => "escalation-open",
            Self::EscalationClose => "escalation-close",
            Self::KeyFinding => "key-finding",
            Self::RepositoryContract => "repository-contract",
            Self::PlanningArtifactApproved => "planning-artifact-approved",
        }
    }

    /// Return this kind's fixed evidence policy.
    pub const fn evidence_policy(self) -> EvidencePolicy {
        match self {
            Self::Dispatch
            | Self::KeyFinding
            | Self::RepositoryContract
            | Self::PlanningArtifactApproved => EvidencePolicy::Required,
            Self::Delta | Self::EscalationOpen | Self::EscalationClose => EvidencePolicy::Absent,
        }
    }
}

impl fmt::Display for WriteKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Whether a known kind requires or forbids an evidence field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidencePolicy {
    /// The payload must contain an `evidence` key.
    Required,
    /// The payload must not contain an `evidence` key.
    Absent,
}

/// The pre-decoding presence of an `evidence` object key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidencePresence {
    /// The object contains an `evidence` key, regardless of its value.
    Present,
    /// The object does not contain an `evidence` key.
    Absent,
}

/// Enforce the exhaustive evidence partition for a registered kind.
///
/// # Errors
///
/// Returns [`EventLogError::MissingRequiredEvidence`] or
/// [`EventLogError::ForbiddenEvidence`] when presence violates the kind's policy.
#[instrument]
pub fn validate_evidence_policy(
    kind: WriteKind,
    presence: EvidencePresence,
) -> Result<(), EventLogError> {
    match (kind.evidence_policy(), presence) {
        (EvidencePolicy::Required, EvidencePresence::Absent) => {
            Err(EventLogError::MissingRequiredEvidence { kind })
        }
        (EvidencePolicy::Absent, EvidencePresence::Present) => {
            Err(EventLogError::ForbiddenEvidence { kind })
        }
        (EvidencePolicy::Required, EvidencePresence::Present)
        | (EvidencePolicy::Absent, EvidencePresence::Absent) => Ok(()),
    }
}

/// The complete payload for `dispatch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchPayload {
    /// The receiving role.
    pub role: DispatchRole,
    /// The exact repository state asserted by the dispatch.
    pub r#ref: DispatchRef,
    /// The invocation that establishes the asserted state.
    pub evidence: Evidence,
}

/// The complete payload for `delta`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeltaPayload {
    /// The changed understanding.
    pub message: String,
}

/// The complete payload for `escalation-open`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationOpenPayload {
    /// The stable escalation identity.
    pub key: EscalationKey,
    /// The question requiring resolution.
    pub question: String,
}

/// The complete payload for `escalation-close`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationClosePayload {
    /// The stable escalation identity.
    pub key: EscalationKey,
    /// The recorded resolution.
    pub resolution: String,
}

/// The complete payload for `key-finding`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyFindingPayload {
    /// The repository-backed finding.
    pub finding: String,
    /// The invocation that produced the finding.
    pub evidence: Evidence,
}

/// The complete payload for `repository-contract`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryContractPayload {
    /// The repository name.
    pub repository: RepositoryName,
    /// The measured repository root.
    pub repo_root: RepositoryRoot,
    /// The measured implementation stack.
    pub stack: String,
    /// The format gate command.
    pub format: String,
    /// The lint gate command.
    pub lint: String,
    /// The typecheck gate command.
    pub typecheck: String,
    /// The test gate command.
    pub test: String,
    /// The build gate command.
    pub build: String,
    /// The preflight command.
    pub preflight: String,
    /// The rule governing acceptance gates.
    pub gates_rule: String,
    /// Installation required before gates can run.
    pub install: String,
    /// The multi-line invocation that measured the contract.
    pub evidence: Evidence,
}

/// The complete payload for `planning-artifact-approved`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningArtifactApprovedPayload {
    /// The planning artifact's identity path.
    pub path: ArtifactPath,
    /// The artifact's SHA-256 identity.
    pub sha256: Sha256Digest,
    /// The invocation that produced the digest.
    pub evidence: Evidence,
}

/// A typed known payload whose variant determines its write kind.
// Keeping the schema variants direct preserves the settled public construction API.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownPayload {
    /// A `dispatch` payload.
    Dispatch(DispatchPayload),
    /// A `delta` payload.
    Delta(DeltaPayload),
    /// An `escalation-open` payload.
    EscalationOpen(EscalationOpenPayload),
    /// An `escalation-close` payload.
    EscalationClose(EscalationClosePayload),
    /// A `key-finding` payload.
    KeyFinding(KeyFindingPayload),
    /// A `repository-contract` payload.
    RepositoryContract(RepositoryContractPayload),
    /// A `planning-artifact-approved` payload.
    PlanningArtifactApproved(PlanningArtifactApprovedPayload),
}

impl KnownPayload {
    /// Return the only write kind compatible with this payload.
    pub const fn kind(&self) -> WriteKind {
        match self {
            Self::Dispatch(_) => WriteKind::Dispatch,
            Self::Delta(_) => WriteKind::Delta,
            Self::EscalationOpen(_) => WriteKind::EscalationOpen,
            Self::EscalationClose(_) => WriteKind::EscalationClose,
            Self::KeyFinding(_) => WriteKind::KeyFinding,
            Self::RepositoryContract(_) => WriteKind::RepositoryContract,
            Self::PlanningArtifactApproved(_) => WriteKind::PlanningArtifactApproved,
        }
    }
}

impl Serialize for KnownPayload {
    #[instrument(skip(self, serializer))]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Dispatch(payload) => payload.serialize(serializer),
            Self::Delta(payload) => payload.serialize(serializer),
            Self::EscalationOpen(payload) => payload.serialize(serializer),
            Self::EscalationClose(payload) => payload.serialize(serializer),
            Self::KeyFinding(payload) => payload.serialize(serializer),
            Self::RepositoryContract(payload) => payload.serialize(serializer),
            Self::PlanningArtifactApproved(payload) => payload.serialize(serializer),
        }
    }
}

/// The open kind returned by the read API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadKind {
    /// A kind in the closed writable registry.
    Known(WriteKind),
    /// An unregistered kind preserved exactly.
    Unknown(String),
}

/// The open payload returned by the read API.
// This mirrors `KnownPayload` directly so callers do not need schema-specific boxing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadPayload {
    /// A payload decoded according to its registered kind.
    Known(KnownPayload),
    /// Arbitrary JSON paired with an unregistered kind.
    Unknown(Value),
}

// This internal correlation carrier retains the same direct typed payload representation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum EventBody {
    Known(KnownPayload),
    Unknown { kind: String, payload: Value },
}

/// A five-field event envelope with a correlated known or unknown body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRecord {
    sequence: Sequence,
    timestamp: EventTimestamp,
    node: NodeId,
    body: EventBody,
}

impl EventRecord {
    /// Construct a writable record from a typed known payload.
    pub fn known(
        sequence: Sequence,
        timestamp: EventTimestamp,
        node: NodeId,
        payload: KnownPayload,
    ) -> Self {
        Self {
            sequence,
            timestamp,
            node,
            body: EventBody::Known(payload),
        }
    }

    /// Return the event sequence.
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }

    /// Return the UTC timestamp.
    pub const fn timestamp(&self) -> &EventTimestamp {
        &self.timestamp
    }

    /// Return the node identifier.
    pub const fn node(&self) -> &NodeId {
        &self.node
    }

    /// Return the open read kind.
    pub fn kind(&self) -> ReadKind {
        match &self.body {
            EventBody::Known(payload) => ReadKind::Known(payload.kind()),
            EventBody::Unknown { kind, .. } => ReadKind::Unknown(kind.clone()),
        }
    }

    /// Return the open read payload.
    pub fn payload(&self) -> ReadPayload {
        match &self.body {
            EventBody::Known(payload) => ReadPayload::Known(payload.clone()),
            EventBody::Unknown { payload, .. } => ReadPayload::Unknown(payload.clone()),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEnvelope {
    sequence: Sequence,
    timestamp: EventTimestamp,
    kind: String,
    node: NodeId,
    payload: Value,
}

#[derive(Serialize)]
struct EnvelopeRef<'a, K: Serialize + ?Sized, P: Serialize + ?Sized> {
    sequence: Sequence,
    timestamp: EventTimestamp,
    kind: &'a K,
    node: &'a NodeId,
    payload: &'a P,
}

impl Serialize for EventRecord {
    #[instrument(skip(self, serializer))]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match &self.body {
            EventBody::Known(payload) => EnvelopeRef {
                sequence: self.sequence,
                timestamp: self.timestamp,
                kind: &payload.kind(),
                node: &self.node,
                payload,
            }
            .serialize(serializer),
            EventBody::Unknown { kind, payload } => EnvelopeRef {
                sequence: self.sequence,
                timestamp: self.timestamp,
                kind,
                node: &self.node,
                payload,
            }
            .serialize(serializer),
        }
    }
}

/// Decode one physical JSONL record without performing I/O.
///
/// # Errors
///
/// Returns a specific evidence-policy error when inspectable evidence presence is
/// invalid, [`EventLogError::InvalidKnownPayload`] when a registered payload is
/// malformed, or [`EventLogError::MalformedEnvelope`] when the envelope is invalid.
#[instrument(skip(line))]
pub fn parse_event_line(line: &str) -> Result<EventRecord, EventLogError> {
    let raw: RawEnvelope =
        serde_json::from_str(line).map_err(|source| EventLogError::MalformedEnvelope { source })?;

    let kind = match WriteKind::parse(&raw.kind) {
        Ok(kind) => kind,
        Err(EventLogError::UnknownWriteKind { .. }) => {
            return Ok(EventRecord {
                sequence: raw.sequence,
                timestamp: raw.timestamp,
                node: raw.node,
                body: EventBody::Unknown {
                    kind: raw.kind,
                    payload: raw.payload,
                },
            });
        }
        Err(error) => return Err(error),
    };

    let payload = validate_and_decode_known_payload(kind, raw.payload)?;

    Ok(EventRecord {
        sequence: raw.sequence,
        timestamp: raw.timestamp,
        node: raw.node,
        body: EventBody::Known(payload),
    })
}

/// Encode one record as a compact physical JSONL object without a terminal newline.
///
/// # Errors
///
/// Returns [`EventLogError::MalformedEnvelope`] if JSON serialization fails.
#[instrument(skip(record))]
pub fn serialize_event_line(record: &EventRecord) -> Result<String, EventLogError> {
    serde_json::to_string(record).map_err(|source| EventLogError::MalformedEnvelope { source })
}

/// Validate one submitted payload, derive the next sequence, and invoke an append capability once.
///
/// The capability error type should represent the boundary operation directly. In the binary,
/// use `std::io::Error` rather than `anyhow::Error`, then add anyhow context outside core.
///
/// # Errors
///
/// Returns a specific [`AppendError`] variant when submitted JSON is malformed, payload or tail
/// validation fails, the supplied external tail has no successor, envelope serialization fails,
/// or the single capability invocation rejects the validated bytes.
#[instrument(skip(payload, tail, capability))]
pub fn append_event<E, F>(
    kind: WriteKind,
    payload: UnparsedPayload,
    tail: EventLogTail,
    node: NodeId,
    timestamp: SystemTime,
    capability: F,
) -> Result<AppendIntent, AppendError<E>>
where
    F: FnOnce(&[u8]) -> Result<(), E>,
{
    let payload_value = serde_json::from_str(&payload.0)
        .map_err(|source| AppendError::MalformedSubmittedPayload { source })?;
    let known_payload = validate_and_decode_known_payload(kind, payload_value)
        .map_err(|source| AppendError::InvalidSubmittedPayload { source })?;

    let sequence = match tail {
        EventLogTail::Empty => Sequence::first(),
        EventLogTail::Present(line) => {
            let tail_record =
                parse_event_line(&line.0).map_err(|source| AppendError::InvalidTail { source })?;
            let tail_sequence = tail_record.sequence().get();
            let successor = tail_sequence
                .checked_add(1)
                .ok_or(AppendError::SequenceOverflow { tail_sequence })?;
            Sequence::parse(successor).map_err(|source| AppendError::InvalidTail { source })?
        }
    };

    let timestamp = EventTimestamp::new(DateTime::<Utc>::from(timestamp));
    let record = EventRecord::known(sequence, timestamp, node, known_payload);
    let mut bytes = serialize_event_line(&record)
        .map_err(|source| AppendError::SerializationFailed { source })?
        .into_bytes();
    bytes.push(b'\n');
    let intent = AppendIntent(bytes);

    capability(intent.as_bytes()).map_err(|source| AppendError::CapabilityFailed { source })?;
    Ok(intent)
}

fn validate_and_decode_known_payload(
    kind: WriteKind,
    payload: Value,
) -> Result<KnownPayload, EventLogError> {
    let object = payload
        .as_object()
        .ok_or_else(|| EventLogError::InvalidKnownPayload {
            kind,
            detail: "payload must be a JSON object".to_owned(),
        })?;
    let presence = if object.contains_key("evidence") {
        EvidencePresence::Present
    } else {
        EvidencePresence::Absent
    };
    validate_evidence_policy(kind, presence)?;
    decode_known_payload(kind, payload)
}

fn decode_known_payload(kind: WriteKind, payload: Value) -> Result<KnownPayload, EventLogError> {
    let decoded = match kind {
        WriteKind::Dispatch => serde_json::from_value(payload).map(KnownPayload::Dispatch),
        WriteKind::Delta => serde_json::from_value(payload).map(KnownPayload::Delta),
        WriteKind::EscalationOpen => {
            serde_json::from_value(payload).map(KnownPayload::EscalationOpen)
        }
        WriteKind::EscalationClose => {
            serde_json::from_value(payload).map(KnownPayload::EscalationClose)
        }
        WriteKind::KeyFinding => serde_json::from_value(payload).map(KnownPayload::KeyFinding),
        WriteKind::RepositoryContract => {
            serde_json::from_value(payload).map(KnownPayload::RepositoryContract)
        }
        WriteKind::PlanningArtifactApproved => {
            serde_json::from_value(payload).map(KnownPayload::PlanningArtifactApproved)
        }
    };
    decoded.map_err(|source| EventLogError::InvalidKnownPayload {
        kind,
        detail: source.to_string(),
    })
}

/// Errors produced by event-log domain parsing and validation.
#[derive(Debug, Error)]
pub enum EventLogError {
    /// Returned when a write-kind spelling is not one of the seven registered values.
    #[error("unregistered event kind cannot be written: {kind:?}")]
    UnknownWriteKind {
        /// The rejected kind spelling.
        kind: String,
    },

    /// Returned when sequence zero is supplied to typed construction or deserialization.
    #[error("event sequence must start at 1, got {value}")]
    ZeroSequence {
        /// The rejected sequence value.
        value: u64,
    },

    /// Returned when a node identifier has zero bytes.
    #[error("event node identifier cannot be empty: {value:?}")]
    EmptyNode {
        /// The rejected zero-byte-length identifier.
        value: String,
    },

    /// Returned when an evidence invocation has zero bytes.
    #[error("event evidence cannot be empty: {value:?}")]
    EmptyEvidence {
        /// The rejected zero-byte-length invocation.
        value: String,
    },

    /// Returned when an event timestamp is not valid RFC 3339.
    #[error("invalid RFC 3339 event timestamp: {input:?}")]
    MalformedTimestamp {
        /// The rejected timestamp spelling.
        input: String,
    },

    /// Returned when the five-field JSON envelope cannot be decoded or encoded.
    #[error("invalid event envelope: {source}")]
    MalformedEnvelope {
        /// The JSON codec failure.
        source: serde_json::Error,
    },

    /// Returned when a registered kind's payload does not match its exact schema.
    #[error("invalid payload for known event kind {kind}: {detail}")]
    InvalidKnownPayload {
        /// The registered kind directing payload decoding.
        kind: WriteKind,
        /// The actionable serde or structural failure.
        detail: String,
    },

    /// Returned when a required-evidence kind has no `evidence` object key.
    #[error("event kind {kind} requires an evidence field")]
    MissingRequiredEvidence {
        /// The kind whose required evidence key is absent.
        kind: WriteKind,
    },

    /// Returned when an evidence-absent kind has an `evidence` object key.
    #[error("event kind {kind} forbids an evidence field")]
    ForbiddenEvidence {
        /// The kind for which evidence must be absent.
        kind: WriteKind,
    },

    /// Returned when a digest is not exactly 64 lowercase hexadecimal characters.
    #[error("invalid lowercase SHA-256 digest: {digest:?}")]
    InvalidSha256Digest {
        /// The rejected digest spelling.
        digest: String,
    },
}

/// Errors produced while validating and delivering one append intent.
///
/// The generic `E` is the narrow capability's native error. Downstream composition should use
/// `AppendError<std::io::Error>` for file append operations; `anyhow::Error` does not satisfy the
/// source-error bounds inferred by `thiserror` for [`AppendError::CapabilityFailed`].
#[derive(Debug, Error)]
pub enum AppendError<E> {
    /// Returned when the submitted bare payload text is not valid JSON.
    #[error("submitted event payload is malformed JSON: {source}")]
    MalformedSubmittedPayload {
        /// The JSON parser failure for the submitted payload text.
        source: serde_json::Error,
    },

    /// Returned when submitted JSON violates the selected kind's schema or evidence policy.
    #[error("submitted event payload is invalid: {source}")]
    InvalidSubmittedPayload {
        /// The payload-domain validation failure.
        source: EventLogError,
    },

    /// Returned when a present tail is not one complete, structurally valid event envelope.
    #[error("supplied event-log tail is invalid: {source}")]
    InvalidTail {
        /// The complete-envelope parsing or validation failure.
        source: EventLogError,
    },

    /// Returned only when an otherwise-valid supplied external tail already has sequence `u64::MAX`.
    #[error("supplied event-log tail sequence {tail_sequence} has no valid successor")]
    SequenceOverflow {
        /// The maximum external tail sequence that cannot be incremented.
        tail_sequence: u64,
    },

    /// Returned when the validated five-field envelope cannot be serialized.
    #[error("validated event envelope could not be serialized: {source}")]
    SerializationFailed {
        /// The envelope serialization failure.
        source: EventLogError,
    },

    /// Returned when the sole append-capability invocation rejects the complete intent bytes.
    #[error("event append capability failed: {source}")]
    CapabilityFailed {
        /// The capability's original error, preserved without alteration.
        source: E,
    },
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::io;
    use std::time::{Duration, SystemTime};

    use serde_json::{Value, json};

    use crate::event_log::{
        AppendError, EventLogError, EventLogTail, EventLogTailLine, Evidence, EvidencePresence,
        KnownPayload, NodeId, ReadKind, ReadPayload, Sequence, Sha256Digest, UnparsedPayload,
        WriteKind, append_event, parse_event_line, serialize_event_line, validate_evidence_policy,
    };

    const APPEND_TIME_SECONDS: u64 = 1_785_155_696;
    const VALID_DELTA_PAYLOAD: &str = r#"{"message":"append one validated event"}"#;
    const FIRST_APPEND: &str = concat!(
        r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m1-s2","payload":{"message":"append one validated event"}}"#,
        "\n"
    );
    const UNKNOWN_TAIL: &str = r#"{"sequence":41,"timestamp":"2026-07-27T12:34:55.000Z","kind":"future-kind","node":"m1-s1","payload":{"nested":{"answer":42},"items":[true,null,"kept"]}}"#;
    const SECOND_APPEND: &str = concat!(
        r#"{"sequence":42,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m1-s2","payload":{"message":"append one validated event"}}"#,
        "\n"
    );

    fn append_time() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(APPEND_TIME_SECONDS)
    }

    const KNOWN_LINES: [&str; 7] = [
        r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"dispatch","node":"m1-s1","payload":{"role":"step-executor","ref":"ca9788ded3daec9b9e9fd7679caa24e7c64a8193","evidence":"git rev-parse HEAD"}}"#,
        r#"{"sequence":2,"timestamp":"2026-07-27T12:34:57.000Z","kind":"delta","node":"m1-s1","payload":{"message":"Require exact UTC timestamp spelling in the event envelope."}}"#,
        r#"{"sequence":3,"timestamp":"2026-07-27T12:34:58.000Z","kind":"escalation-open","node":"m1-s1","payload":{"key":"timestamp-precision","question":"Which RFC 3339 sub-second precision is canonical?"}}"#,
        r#"{"sequence":4,"timestamp":"2026-07-27T12:34:59.000Z","kind":"escalation-close","node":"m1-s1","payload":{"key":"timestamp-precision","resolution":"Use milliseconds and a Z suffix."}}"#,
        r##"{"sequence":5,"timestamp":"2026-07-27T12:35:00.000Z","kind":"key-finding","node":"m1-s1","payload":{"finding":"The repository has exactly five tests at the ground-truth ref.","evidence":"git grep -n '#[test]' ca9788ded3daec9b9e9fd7679caa24e7c64a8193 -- crates/core/src/vision.rs"}}"##,
        r#"{"sequence":6,"timestamp":"2026-07-27T12:35:01.000Z","kind":"repository-contract","node":"m1-s1","payload":{"repository":"pce","repo_root":"/workspace/pce","stack":"Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)","format":"cargo fmt --all --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --workspace","preflight":"cargo check --workspace --all-targets","gates_rule":"From the repo root, all four gates must exit zero before committing.","install":"None required for gates.","evidence":"rustc --version\ncargo --version\ngit rev-parse --show-toplevel"}}"#,
        r#"{"sequence":7,"timestamp":"2026-07-27T12:35:02.000Z","kind":"planning-artifact-approved","node":"m1-s1","payload":{"path":"planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","evidence":"shasum -a 256 planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json"}}"#,
    ];
    const UNKNOWN_LINE: &str = r#"{"sequence":8,"timestamp":"2026-07-27T12:35:03.000Z","kind":"future-kind","node":"m1-s1","payload":{"nested":{"answer":42},"items":[true,null,"kept"]}}"#;

    #[test]
    fn empty_tail_builds_exact_first_intent_and_invokes_once() -> Result<(), EventLogError> {
        let calls = Cell::new(0);
        let recorded = RefCell::new(Vec::new());

        let intent = append_event(
            WriteKind::Delta,
            UnparsedPayload::new(VALID_DELTA_PAYLOAD),
            EventLogTail::Empty,
            NodeId::parse("m1-s2")?,
            append_time(),
            |bytes| {
                calls.set(calls.get() + 1);
                recorded.borrow_mut().extend_from_slice(bytes);
                Ok::<(), io::Error>(())
            },
        )
        .map_err(append_test_error)?;

        assert_eq!(calls.get(), 1);
        assert_eq!(recorded.borrow().as_slice(), FIRST_APPEND.as_bytes());
        assert_eq!(intent.as_bytes(), FIRST_APPEND.as_bytes());
        Ok(())
    }

    #[test]
    fn unknown_kind_tail_builds_exact_successor_intent() -> Result<(), EventLogError> {
        let calls = Cell::new(0);
        let recorded = RefCell::new(Vec::new());

        let intent = append_event(
            WriteKind::Delta,
            UnparsedPayload::new(VALID_DELTA_PAYLOAD),
            EventLogTail::Present(EventLogTailLine::new(UNKNOWN_TAIL)),
            NodeId::parse("m1-s2")?,
            append_time(),
            |bytes| {
                calls.set(calls.get() + 1);
                recorded.borrow_mut().extend_from_slice(bytes);
                Ok::<(), io::Error>(())
            },
        )
        .map_err(append_test_error)?;

        assert_eq!(calls.get(), 1);
        assert_eq!(recorded.borrow().as_slice(), SECOND_APPEND.as_bytes());
        assert_eq!(intent.as_bytes(), SECOND_APPEND.as_bytes());
        Ok(())
    }

    #[test]
    fn required_evidence_newline_stays_inside_one_physical_record() -> Result<(), EventLogError> {
        let payload = r#"{"finding":"the measured fact","evidence":"git rev-parse HEAD\ncargo test --workspace"}"#;
        let expected = concat!(
            r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"key-finding","node":"m1-s2","payload":{"finding":"the measured fact","evidence":"git rev-parse HEAD\ncargo test --workspace"}}"#,
            "\n"
        );
        let calls = Cell::new(0);

        let intent = append_event(
            WriteKind::KeyFinding,
            UnparsedPayload::new(payload),
            EventLogTail::Empty,
            NodeId::parse("m1-s2")?,
            append_time(),
            |_| {
                calls.set(calls.get() + 1);
                Ok::<(), io::Error>(())
            },
        )
        .map_err(append_test_error)?;

        assert_eq!(calls.get(), 1);
        assert_eq!(intent.as_bytes(), expected.as_bytes());
        assert_eq!(
            intent
                .as_bytes()
                .iter()
                .filter(|byte| **byte == b'\n')
                .count(),
            1
        );
        assert!(intent.as_bytes().ends_with(b"\n"));
        Ok(())
    }

    #[test]
    fn invalid_submitted_payloads_never_invoke_capability() -> Result<(), EventLogError> {
        let cases = [
            (WriteKind::Delta, "{"),
            (WriteKind::Delta, "{}"),
            (
                WriteKind::KeyFinding,
                r#"{"finding":"no evidence supplied"}"#,
            ),
            (
                WriteKind::Delta,
                r#"{"message":"wrong evidence","evidence":"must be absent"}"#,
            ),
        ];

        for (index, (kind, payload)) in cases.into_iter().enumerate() {
            let calls = Cell::new(0);
            let result = append_event(
                kind,
                UnparsedPayload::new(payload),
                EventLogTail::Empty,
                NodeId::parse("m1-s2")?,
                append_time(),
                |_| {
                    calls.set(calls.get() + 1);
                    Ok::<(), io::Error>(())
                },
            );

            match index {
                0 => assert!(matches!(
                    result,
                    Err(AppendError::MalformedSubmittedPayload { .. })
                )),
                1 => assert!(matches!(
                    result,
                    Err(AppendError::InvalidSubmittedPayload {
                        source: EventLogError::InvalidKnownPayload {
                            kind: WriteKind::Delta,
                            ..
                        }
                    })
                )),
                2 => assert!(matches!(
                    result,
                    Err(AppendError::InvalidSubmittedPayload {
                        source: EventLogError::MissingRequiredEvidence {
                            kind: WriteKind::KeyFinding
                        }
                    })
                )),
                3 => assert!(matches!(
                    result,
                    Err(AppendError::InvalidSubmittedPayload {
                        source: EventLogError::ForbiddenEvidence {
                            kind: WriteKind::Delta
                        }
                    })
                )),
                _ => unreachable!("every fixture index is classified"),
            }
            assert_eq!(calls.get(), 0);
        }
        Ok(())
    }

    #[test]
    fn unknown_write_kind_is_rejected_before_append_is_reachable() {
        let calls = Cell::new(0);
        let kind = WriteKind::parse("future-kind");

        assert!(matches!(
            kind,
            Err(EventLogError::UnknownWriteKind { kind }) if kind == "future-kind"
        ));
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn invalid_tails_never_guess_or_invoke_capability() -> Result<(), EventLogError> {
        let tails = [
            "{",
            r#"{"sequence":41,"timestamp":"2026-07-27T12:34:55.000Z","kind":"delta","node":"m1-s1"}"#,
        ];

        for tail in tails {
            let calls = Cell::new(0);
            let result = append_event(
                WriteKind::Delta,
                UnparsedPayload::new(VALID_DELTA_PAYLOAD),
                EventLogTail::Present(EventLogTailLine::new(tail)),
                NodeId::parse("m1-s2")?,
                append_time(),
                |_| {
                    calls.set(calls.get() + 1);
                    Ok::<(), io::Error>(())
                },
            );

            assert!(matches!(result, Err(AppendError::InvalidTail { .. })));
            assert_eq!(calls.get(), 0);
        }
        Ok(())
    }

    #[test]
    fn maximum_external_tail_overflows_before_capability() -> Result<(), EventLogError> {
        let tail = r#"{"sequence":18446744073709551615,"timestamp":"2026-07-27T12:34:55.000Z","kind":"delta","node":"m1-s1","payload":{"message":"external maximum tail"}}"#;
        let calls = Cell::new(0);
        let result = append_event(
            WriteKind::Delta,
            UnparsedPayload::new(VALID_DELTA_PAYLOAD),
            EventLogTail::Present(EventLogTailLine::new(tail)),
            NodeId::parse("m1-s2")?,
            append_time(),
            |_| {
                calls.set(calls.get() + 1);
                Ok::<(), io::Error>(())
            },
        );

        assert!(matches!(
            result,
            Err(AppendError::SequenceOverflow {
                tail_sequence: u64::MAX
            })
        ));
        assert_eq!(calls.get(), 0);
        Ok(())
    }

    #[test]
    fn capability_failure_is_preserved_without_retry() -> Result<(), EventLogError> {
        let calls = Cell::new(0);
        let result = append_event(
            WriteKind::Delta,
            UnparsedPayload::new(VALID_DELTA_PAYLOAD),
            EventLogTail::Empty,
            NodeId::parse("m1-s2")?,
            append_time(),
            |_| {
                calls.set(calls.get() + 1);
                Err(io::Error::other("append refused"))
            },
        );

        let Err(AppendError::CapabilityFailed { source }) = result else {
            panic!("capability failure expected");
        };
        assert_eq!(source.to_string(), "append refused");
        assert_eq!(source.kind(), io::ErrorKind::Other);
        assert_eq!(calls.get(), 1);
        Ok(())
    }

    fn append_test_error(error: AppendError<io::Error>) -> EventLogError {
        match error {
            AppendError::InvalidSubmittedPayload { source }
            | AppendError::InvalidTail { source }
            | AppendError::SerializationFailed { source } => source,
            other => panic!("unexpected append failure: {other}"),
        }
    }

    #[test]
    fn parses_and_exactly_serializes_all_known_examples() -> Result<(), EventLogError> {
        let expected_kinds = [
            WriteKind::Dispatch,
            WriteKind::Delta,
            WriteKind::EscalationOpen,
            WriteKind::EscalationClose,
            WriteKind::KeyFinding,
            WriteKind::RepositoryContract,
            WriteKind::PlanningArtifactApproved,
        ];

        for (index, (line, expected_kind)) in
            KNOWN_LINES.into_iter().zip(expected_kinds).enumerate()
        {
            let record = parse_event_line(line)?;
            assert_eq!(record.kind(), ReadKind::Known(expected_kind));
            let expected_payload = matches!(
                (index, record.payload()),
                (0, ReadPayload::Known(KnownPayload::Dispatch(_)))
                    | (1, ReadPayload::Known(KnownPayload::Delta(_)))
                    | (2, ReadPayload::Known(KnownPayload::EscalationOpen(_)))
                    | (3, ReadPayload::Known(KnownPayload::EscalationClose(_)))
                    | (4, ReadPayload::Known(KnownPayload::KeyFinding(_)))
                    | (5, ReadPayload::Known(KnownPayload::RepositoryContract(_)))
                    | (
                        6,
                        ReadPayload::Known(KnownPayload::PlanningArtifactApproved(_))
                    )
            );
            assert!(expected_payload);
            assert_eq!(serialize_event_line(&record)?, line);
        }
        Ok(())
    }

    #[test]
    fn timestamp_serialization_is_utc_with_exact_milliseconds() -> Result<(), EventLogError> {
        let fractional = parse_event_line(
            r#"{"sequence":1,"timestamp":"2026-07-27T14:34:56.987654+02:00","kind":"delta","node":"n","payload":{"message":"m"}}"#,
        )?;
        let whole = parse_event_line(
            r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56Z","kind":"delta","node":"n","payload":{"message":"m"}}"#,
        )?;

        let fractional_json = serialize_event_line(&fractional)?;
        let whole_json = serialize_event_line(&whole)?;
        assert!(fractional_json.contains(r#""timestamp":"2026-07-27T12:34:56.987Z""#));
        assert!(whole_json.contains(r#""timestamp":"2026-07-27T12:34:56.000Z""#));
        assert!(!fractional_json.contains("+00:00"));
        assert!(!whole_json.contains("+00:00"));
        Ok(())
    }

    #[test]
    fn sequence_and_node_scalar_boundaries_are_enforced() -> Result<(), EventLogError> {
        assert_eq!(Sequence::parse(1)?, Sequence::first());
        assert!(matches!(
            Sequence::parse(0),
            Err(EventLogError::ZeroSequence { value: 0 })
        ));
        assert!(matches!(
            parse_event_line(
                r#"{"sequence":0,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"n","payload":{"message":"m"}}"#
            ),
            Err(EventLogError::MalformedEnvelope { .. })
        ));
        assert!(matches!(
            NodeId::parse(""),
            Err(EventLogError::EmptyNode { .. })
        ));
        assert_eq!(NodeId::parse("   ")?.as_str(), "   ");
        assert!(matches!(
            Evidence::parse(""),
            Err(EventLogError::EmptyEvidence { .. })
        ));
        assert!(matches!(
            Sha256Digest::parse("ABC"),
            Err(EventLogError::InvalidSha256Digest { .. })
        ));
        Ok(())
    }

    #[test]
    fn validates_every_member_of_the_evidence_partition() -> Result<(), EventLogError> {
        let required = [
            WriteKind::Dispatch,
            WriteKind::KeyFinding,
            WriteKind::RepositoryContract,
            WriteKind::PlanningArtifactApproved,
        ];
        let absent = [
            WriteKind::Delta,
            WriteKind::EscalationOpen,
            WriteKind::EscalationClose,
        ];

        for kind in required {
            validate_evidence_policy(kind, EvidencePresence::Present)?;
            assert!(matches!(
                validate_evidence_policy(kind, EvidencePresence::Absent),
                Err(EventLogError::MissingRequiredEvidence { kind: failed }) if failed == kind
            ));
        }
        for kind in absent {
            validate_evidence_policy(kind, EvidencePresence::Absent)?;
            assert!(matches!(
                validate_evidence_policy(kind, EvidencePresence::Present),
                Err(EventLogError::ForbiddenEvidence { kind: failed }) if failed == kind
            ));
        }
        Ok(())
    }

    #[test]
    fn preserves_multiline_live_invocation_evidence() -> Result<(), EventLogError> {
        let record = parse_event_line(KNOWN_LINES[5])?;
        let ReadPayload::Known(KnownPayload::RepositoryContract(payload)) = record.payload() else {
            panic!("repository contract payload expected");
        };
        assert_eq!(
            payload.evidence.as_str(),
            "rustc --version\ncargo --version\ngit rev-parse --show-toplevel"
        );

        let live_invocation =
            "Invoke the repository analyst with:\n  ref: ca9788d\n  task: inspect contract";
        let live_line = json!({
            "sequence": 9,
            "timestamp": "2026-07-27T12:35:04.000Z",
            "kind": "dispatch",
            "node": "m1-s1",
            "payload": {
                "role": "repository-analyst",
                "ref": "ca9788d",
                "evidence": live_invocation
            }
        })
        .to_string();
        let live_record = parse_event_line(&live_line)?;
        let ReadPayload::Known(KnownPayload::Dispatch(live_payload)) = live_record.payload() else {
            panic!("dispatch payload expected");
        };
        assert_eq!(live_payload.evidence.as_str(), live_invocation);
        assert!(!serialize_event_line(&live_record)?.contains('\n'));

        let line = serialize_event_line(&record)?;
        assert!(!line.contains('\n'));
        let reparsed = parse_event_line(&line)?;
        assert_eq!(reparsed, record);
        Ok(())
    }

    #[test]
    fn rejects_malformed_known_payload_schemas() {
        let malformed = [
            r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"n","payload":{}}"#,
            r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"n","payload":{"message":42}}"#,
            r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"n","payload":{"message":"m","extra":true}}"#,
        ];
        for line in malformed {
            assert!(matches!(
                parse_event_line(line),
                Err(EventLogError::InvalidKnownPayload {
                    kind: WriteKind::Delta,
                    ..
                })
            ));
        }
    }

    #[test]
    fn preserves_unknown_kind_and_arbitrary_payload_round_trip() -> Result<(), EventLogError> {
        let record = parse_event_line(UNKNOWN_LINE)?;
        let expected_payload = json!({
            "nested": {"answer": 42},
            "items": [true, null, "kept"]
        });
        assert_eq!(record.kind(), ReadKind::Unknown("future-kind".to_owned()));
        assert_eq!(record.payload(), ReadPayload::Unknown(expected_payload));

        let serialized = serialize_event_line(&record)?;
        let reparsed = parse_event_line(&serialized)?;
        assert_eq!(reparsed, record);
        Ok(())
    }

    #[test]
    fn write_kind_parser_is_exact_and_closed() {
        for raw in ["future-kind", "dispatch-cost", "Dispatch", "DISPATCH", ""] {
            assert!(matches!(
                WriteKind::parse(raw),
                Err(EventLogError::UnknownWriteKind { .. })
            ));
        }
    }

    #[test]
    fn complete_fixture_remains_physically_newline_delimited() -> Result<(), EventLogError> {
        let mut serialized = Vec::new();
        for line in KNOWN_LINES {
            let encoded = serialize_event_line(&parse_event_line(line)?)?;
            assert!(!encoded.contains('\n'));
            serialized.push(encoded);
        }
        assert_eq!(serialized.join("\n").lines().count(), 7);
        Ok(())
    }

    #[test]
    fn evidence_preinspection_pins_null_and_non_object_behavior() {
        let absent_null = r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"n","payload":{"message":"m","evidence":null}}"#;
        let required_null = r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"dispatch","node":"n","payload":{"role":"r","ref":"x","evidence":null}}"#;
        assert!(matches!(
            parse_event_line(absent_null),
            Err(EventLogError::ForbiddenEvidence {
                kind: WriteKind::Delta
            })
        ));
        assert!(matches!(
            parse_event_line(required_null),
            Err(EventLogError::InvalidKnownPayload {
                kind: WriteKind::Dispatch,
                ..
            })
        ));

        let non_objects: [Value; 5] = [
            json!("text"),
            json!([]),
            json!(42),
            json!(true),
            Value::Null,
        ];
        for payload in non_objects {
            let line = json!({
                "sequence": 1,
                "timestamp": "2026-07-27T12:34:56.000Z",
                "kind": "dispatch",
                "node": "n",
                "payload": payload
            })
            .to_string();
            assert!(matches!(
                parse_event_line(&line),
                Err(EventLogError::InvalidKnownPayload {
                    kind: WriteKind::Dispatch,
                    ..
                })
            ));
        }
    }
}
