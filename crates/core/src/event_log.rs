//! decode : EventLogLine → KnownEvent ∪ UnknownEvent; append : AppendInput → AppendIntent; criterion_execution : AcceptanceCriterion × FinishedResult × Outcome × Evidence → CriterionExecutionPayload; select : EventRecord × EventRecordFilter → Bool; persist_contract : RepositoryName × RepositoryRoot × TrackedRepositoryContract × GateMeasurements × Evidence → RepositoryContractPayload.
//! This module is pure domain logic and performs no I/O.

use std::collections::BTreeMap;
use std::fmt;
use std::time::SystemTime;

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use thiserror::Error;
use tracing::instrument;

use crate::AcceptanceCriterion;
use crate::contract_measurement::GateMeasurements;
pub use crate::contract_measurement::ObservedExitStatus;
use crate::run_state::VersionPolicy;
use crate::tracked_contract::{
    LocalWorkflowStandIn, MilestonePullRequestBase, PullRequestMergeMethod, StepPullRequestBase,
    TrackedRepositoryContract,
};

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

/// Errors produced while deriving the successor of an immutable event-log tail.
#[derive(Debug, Error)]
pub enum EventLogTailError {
    /// A present physical tail line is not a valid event envelope.
    #[error("supplied event-log tail is invalid: {source}")]
    InvalidTail {
        /// The complete-envelope parsing or validation failure.
        source: EventLogError,
    },
    /// A valid tail already has sequence `u64::MAX`.
    #[error("supplied event-log tail sequence {tail_sequence} has no valid successor")]
    SequenceOverflow {
        /// The maximum sequence that cannot be incremented.
        tail_sequence: u64,
    },
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

macro_rules! non_blank_event_string {
    ($name:ident, $error:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name(String);

        impl $name {
            /// Parse and trim a required domain string.
            ///
            /// # Errors
            ///
            #[doc = concat!("Returns [`EventLogError::", stringify!($error), "`] when `value` is blank.")]
            pub fn parse(value: &str) -> Result<Self, EventLogError> {
                let value = value.trim();
                if value.is_empty() {
                    return Err(EventLogError::$error);
                }
                Ok(Self(value.to_owned()))
            }

            /// Return the trimmed value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

non_blank_event_string!(
    FinishedResult,
    BlankFinishedResult,
    "The exact supplied finished result exercised by a criterion."
);
non_blank_event_string!(
    ObservedCriterionResult,
    BlankObservedCriterionResult,
    "The exact result observed while executing a criterion."
);
non_blank_event_string!(
    UnpaidCriterionReason,
    BlankUnpaidCriterionReason,
    "The measured reason a criterion cannot execute inside the run."
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
    /// The measured outcome of an earlier dispatch.
    DispatchCompletion,
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
    /// One criterion's observed outcome against a supplied finished result.
    CriterionExecution,
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
            "dispatch-completion" => Ok(Self::DispatchCompletion),
            "delta" => Ok(Self::Delta),
            "escalation-open" => Ok(Self::EscalationOpen),
            "escalation-close" => Ok(Self::EscalationClose),
            "key-finding" => Ok(Self::KeyFinding),
            "repository-contract" => Ok(Self::RepositoryContract),
            "planning-artifact-approved" => Ok(Self::PlanningArtifactApproved),
            "criterion-execution" => Ok(Self::CriterionExecution),
            _ => Err(EventLogError::UnknownWriteKind {
                kind: raw.to_owned(),
            }),
        }
    }

    /// Return the exact registered on-disk spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dispatch => "dispatch",
            Self::DispatchCompletion => "dispatch-completion",
            Self::Delta => "delta",
            Self::EscalationOpen => "escalation-open",
            Self::EscalationClose => "escalation-close",
            Self::KeyFinding => "key-finding",
            Self::RepositoryContract => "repository-contract",
            Self::PlanningArtifactApproved => "planning-artifact-approved",
            Self::CriterionExecution => "criterion-execution",
        }
    }

    /// Return this kind's fixed evidence policy.
    pub const fn evidence_policy(self) -> EvidencePolicy {
        match self {
            Self::Dispatch
            | Self::KeyFinding
            | Self::RepositoryContract
            | Self::PlanningArtifactApproved
            | Self::CriterionExecution => EvidencePolicy::Required,
            Self::DispatchCompletion
            | Self::Delta
            | Self::EscalationOpen
            | Self::EscalationClose => EvidencePolicy::Absent,
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

macro_rules! transparent_u64 {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            /// Construct the typed total.
            pub const fn new(value: u64) -> Self {
                Self(value)
            }
            /// Return the numeric total.
            pub const fn get(self) -> u64 {
                self.0
            }
        }
    };
}

transparent_u64!(
    DispatchDuration,
    "A checked elapsed wall-clock duration in milliseconds."
);
transparent_u64!(InputTokens, "A dispatch-route input-token total.");
transparent_u64!(CachedInputTokens, "A Codex cached-input-token total.");
transparent_u64!(OutputTokens, "A dispatch-route output-token total.");
transparent_u64!(
    CacheCreationInputTokens,
    "A Claude cache-creation input-token total."
);
transparent_u64!(
    CacheReadInputTokens,
    "A Claude cache-read input-token total."
);
transparent_u64!(
    ReasoningOutputTokens,
    "A Codex reasoning-output-token total."
);

/// Why terminal observation could not provide measured usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UsageAbsenceReason {
    /// Codex emitted `turn.failed`.
    TurnFailed,
    /// The process failed without a terminal turn.
    NoTerminalTurn,
    /// A JSONL line or terminal payload was malformed.
    MalformedTerminalData,
    /// A terminal event type occurred more than once.
    DuplicateTerminalData,
    /// Terminal facts disagreed with each other or the process exit.
    ContradictoryTerminalData,
    /// The Claude result bytes do not form a valid result envelope.
    ClaudeMalformedResult,
    /// A successful Claude result lacks one or more required usage counters.
    ClaudeMissingUsage,
    /// A Claude error envelope agrees with a failing process exit.
    ClaudeErrorEnvelope,
    /// A Claude result envelope's success meaning contradicts the process exit.
    ClaudeExitEnvelopeContradiction,
}

/// Dispatch token usage measured by a route, or absent from its terminal result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DispatchTokenUsage {
    /// All four terminal counters were observed.
    Measured {
        input_tokens: InputTokens,
        cached_input_tokens: CachedInputTokens,
        output_tokens: OutputTokens,
        reasoning_output_tokens: ReasoningOutputTokens,
    },
    /// All four Claude result-envelope counters were observed.
    ClaudeMeasured {
        input_tokens: InputTokens,
        output_tokens: OutputTokens,
        cache_creation_input_tokens: CacheCreationInputTokens,
        cache_read_input_tokens: CacheReadInputTokens,
    },
    /// No measured counters can be asserted.
    Absent { reason: UsageAbsenceReason },
}

transparent_u64!(ExitCode, "A child process exit code.");
transparent_u64!(SignalNumber, "A Unix child termination signal number.");

/// A child process exit preserving normal exit and Unix signal termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DispatchExitStatus {
    /// The child exited normally.
    Exited { code: ExitCode },
    /// The child was terminated by a Unix signal.
    Signaled { signal: SignalNumber },
}

/// The artifact-validation boundary reached by a dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactOutcome {
    /// No artifact validation occurred.
    NotValidated,
    /// Readable artifact JSON conforms to a valid schema.
    Validated,
    /// The artifact is absent or unreadable.
    Missing,
    /// The readable artifact bytes are not complete valid JSON.
    Truncated,
    /// The schema is absent, unreadable, unparsable, or cannot be compiled.
    SchemaInvalid,
    /// Parsed artifact JSON is rejected by a valid compiled schema.
    SchemaViolating,
}

/// The complete payload for `dispatch-completion`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchCompletionPayload {
    pub issuance_sequence: Sequence,
    pub duration_ms: DispatchDuration,
    pub usage: DispatchTokenUsage,
    pub exit_status: DispatchExitStatus,
    pub artifact_outcome: ArtifactOutcome,
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

/// The complete stated-authority half of a current repository contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatedRepositoryContract {
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
    /// The version policy governing dispatch admission.
    pub version_policy: VersionPolicy,
    /// The branch convention governing step work.
    pub branch_convention: String,
    /// The pull-request convention governing integration.
    pub pull_request_convention: String,
}

/// The measured-authority gate observations for a current repository contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateObservations {
    /// The format gate exit status.
    pub format: ObservedExitStatus,
    /// The lint gate exit status.
    pub lint: ObservedExitStatus,
    /// The typecheck gate exit status.
    pub typecheck: ObservedExitStatus,
    /// The test gate exit status.
    pub test: ObservedExitStatus,
    /// The build gate exit status.
    pub build: ObservedExitStatus,
}

/// Exact local stand-ins keyed by repository workflow path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowMap(BTreeMap<String, Option<String>>);

impl WorkflowMap {
    /// Construct an exact workflow-path mapping.
    pub const fn new(value: BTreeMap<String, Option<String>>) -> Self {
        Self(value)
    }

    /// Borrow the exact workflow-path mapping.
    pub const fn as_map(&self) -> &BTreeMap<String, Option<String>> {
        &self.0
    }
}

/// The appendable-authority half of a current repository contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppendableRepositoryContract {
    /// Environment hazards discovered while measuring the repository.
    pub environment_hazards: Vec<String>,
    /// Required orderings among repository gates.
    pub gate_orderings: Vec<String>,
    /// Rules governing repository lockfiles.
    pub lockfile_rules: Vec<String>,
}

/// The complete current payload for `repository-contract`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryContractPayload {
    /// The repository name.
    pub repository: RepositoryName,
    /// The measured repository root.
    pub repo_root: RepositoryRoot,
    /// The stated repository contract.
    pub stated: StatedRepositoryContract,
    /// The measured gate observations.
    pub observations: GateObservations,
    /// The exact workflow-path mapping.
    pub workflow_map: WorkflowMap,
    /// The appendable repository-contract authority.
    pub appendable: AppendableRepositoryContract,
    /// The multi-line invocation that measured the contract.
    pub evidence: Evidence,
}

impl RepositoryContractPayload {
    /// Convert typed stated, appendable, and measured authorities into the current payload.
    pub fn from_tracked_measurement(
        repository: RepositoryName,
        repo_root: RepositoryRoot,
        tracked: &TrackedRepositoryContract,
        gates: &GateMeasurements,
        evidence: Evidence,
    ) -> Self {
        let stated = tracked.stated();
        let commands = stated.gates();
        let observations = GateObservations {
            format: gates
                .get(crate::tracked_contract::GateKind::Format)
                .status(),
            lint: gates.get(crate::tracked_contract::GateKind::Lint).status(),
            typecheck: gates
                .get(crate::tracked_contract::GateKind::Typecheck)
                .status(),
            test: gates.get(crate::tracked_contract::GateKind::Test).status(),
            build: gates.get(crate::tracked_contract::GateKind::Build).status(),
        };
        let workflow_map = stated
            .workflows()
            .as_slice()
            .iter()
            .map(|mapping| {
                let stand_in = match mapping.stand_in() {
                    LocalWorkflowStandIn::Command(command) => Some(command.as_str().to_owned()),
                    LocalWorkflowStandIn::None => None,
                };
                (mapping.workflow().as_str().to_owned(), stand_in)
            })
            .collect::<BTreeMap<_, _>>();
        let appendable = tracked.appendable();

        Self {
            repository,
            repo_root,
            stated: StatedRepositoryContract {
                format: commands.format().as_str().to_owned(),
                lint: commands.lint().as_str().to_owned(),
                typecheck: commands.typecheck().as_str().to_owned(),
                test: commands.test().as_str().to_owned(),
                build: commands.build().as_str().to_owned(),
                version_policy: stated.version_policy().clone(),
                branch_convention: render_branch_convention(stated.branches()),
                pull_request_convention: render_pull_request_convention(stated.pull_requests()),
            },
            observations,
            workflow_map: WorkflowMap::new(workflow_map),
            appendable: AppendableRepositoryContract {
                environment_hazards: appendable
                    .environment_hazards()
                    .iter()
                    .map(|fact| fact.as_str().to_owned())
                    .collect(),
                gate_orderings: appendable
                    .gate_orderings()
                    .iter()
                    .map(|fact| fact.as_str().to_owned())
                    .collect(),
                lockfile_rules: appendable
                    .lockfile_rules()
                    .iter()
                    .map(|fact| fact.as_str().to_owned())
                    .collect(),
            },
            evidence,
        }
    }
}

fn render_branch_convention(branches: &crate::tracked_contract::BranchConvention) -> String {
    format!(
        "default={}; milestone={}; step={}",
        branches.default().as_str(),
        branches.milestone().as_str(),
        branches.step().as_str()
    )
}

fn render_pull_request_convention(
    pull_requests: &crate::tracked_contract::PullRequestConvention,
) -> String {
    let step_base = match pull_requests.step_base() {
        StepPullRequestBase::Milestone => "MILESTONE",
    };
    let milestone_base = match pull_requests.milestone_base() {
        MilestonePullRequestBase::Default => "DEFAULT",
    };
    let merge_method = match pull_requests.merge_method() {
        PullRequestMergeMethod::Squash => "SQUASH",
    };
    format!("step_base={step_base}; milestone_base={milestone_base}; merge_method={merge_method}")
}

/// A persisted legacy twelve-field repository contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyRepositoryContractPayload {
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

/// The closed outcome partition for one criterion execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CriterionExecutionOutcome {
    /// The observed result satisfied the expected observation.
    Passed {
        /// The exact result observed from the finished result.
        observed_result: ObservedCriterionResult,
    },
    /// The observed result did not satisfy the expected observation.
    Failed {
        /// The exact result observed from the finished result.
        observed_result: ObservedCriterionResult,
    },
    /// The criterion cannot execute within this run.
    Unpaid {
        /// The measured boundary preventing an in-run observation.
        reason: UnpaidCriterionReason,
    },
}

/// The complete payload for `criterion-execution`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionExecutionPayload {
    /// The exact criterion input and expected observation.
    pub criterion: AcceptanceCriterion,
    /// The exact supplied finished result.
    pub finished_result: FinishedResult,
    /// The distinct passed, failed, or unpaid outcome.
    pub outcome: CriterionExecutionOutcome,
    /// The invocation measuring the result or execution boundary.
    pub evidence: Evidence,
}

/// A typed known payload whose variant determines its write kind.
// Keeping the schema variants direct preserves the settled public construction API.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownPayload {
    /// A `dispatch` payload.
    Dispatch(DispatchPayload),
    /// A `dispatch-completion` payload.
    DispatchCompletion(DispatchCompletionPayload),
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
    /// A persisted legacy `repository-contract` payload.
    LegacyRepositoryContract(LegacyRepositoryContractPayload),
    /// A `planning-artifact-approved` payload.
    PlanningArtifactApproved(PlanningArtifactApprovedPayload),
    /// A `criterion-execution` payload.
    CriterionExecution(CriterionExecutionPayload),
}

impl KnownPayload {
    /// Return the only write kind compatible with this payload.
    pub const fn kind(&self) -> WriteKind {
        match self {
            Self::Dispatch(_) => WriteKind::Dispatch,
            Self::DispatchCompletion(_) => WriteKind::DispatchCompletion,
            Self::Delta(_) => WriteKind::Delta,
            Self::EscalationOpen(_) => WriteKind::EscalationOpen,
            Self::EscalationClose(_) => WriteKind::EscalationClose,
            Self::KeyFinding(_) => WriteKind::KeyFinding,
            Self::RepositoryContract(_) => WriteKind::RepositoryContract,
            Self::LegacyRepositoryContract(_) => WriteKind::RepositoryContract,
            Self::PlanningArtifactApproved(_) => WriteKind::PlanningArtifactApproved,
            Self::CriterionExecution(_) => WriteKind::CriterionExecution,
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
            Self::DispatchCompletion(payload) => payload.serialize(serializer),
            Self::Delta(payload) => payload.serialize(serializer),
            Self::EscalationOpen(payload) => payload.serialize(serializer),
            Self::EscalationClose(payload) => payload.serialize(serializer),
            Self::KeyFinding(payload) => payload.serialize(serializer),
            Self::RepositoryContract(payload) => payload.serialize(serializer),
            Self::LegacyRepositoryContract(payload) => payload.serialize(serializer),
            Self::PlanningArtifactApproved(payload) => payload.serialize(serializer),
            Self::CriterionExecution(payload) => payload.serialize(serializer),
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

/// An exact open-world event-kind spelling used by raw-record retrieval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventKindName(String);

impl EventKindName {
    /// Preserve an event-kind spelling without applying the writable registry.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the exact event-kind spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The four legal raw-record filter combinations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventRecordFilter {
    /// Select every record.
    All,
    /// Select records with one exact open-world kind spelling.
    Kind(EventKindName),
    /// Select records with one exact node.
    Node(NodeId),
    /// Select records matching both exact values.
    KindAndNode {
        /// The exact open-world kind spelling.
        kind: EventKindName,
        /// The exact node.
        node: NodeId,
    },
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

/// A correlated borrowed view of a known or unknown event body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventBodyRef<'a> {
    /// A payload decoded according to its registered kind.
    Known(&'a KnownPayload),
    /// An unregistered kind paired with its arbitrary JSON payload.
    Unknown {
        /// The exact unregistered kind spelling.
        kind: &'a str,
        /// The arbitrary JSON payload paired with the kind.
        payload: &'a Value,
    },
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

    /// Borrow the correlated known or unknown event body without cloning it.
    pub fn body_ref(&self) -> EventBodyRef<'_> {
        match &self.body {
            EventBody::Known(payload) => EventBodyRef::Known(payload),
            EventBody::Unknown { kind, payload } => EventBodyRef::Unknown { kind, payload },
        }
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

    fn kind_spelling(&self) -> &str {
        match &self.body {
            EventBody::Known(payload) => payload.kind().as_str(),
            EventBody::Unknown { kind, .. } => kind,
        }
    }
}

/// Select an event record using one of the four legal raw-read filters.
pub fn event_record_matches(record: &EventRecord, filter: &EventRecordFilter) -> bool {
    match filter {
        EventRecordFilter::All => true,
        EventRecordFilter::Kind(kind) => record.kind_spelling() == kind.as_str(),
        EventRecordFilter::Node(node) => record.node() == node,
        EventRecordFilter::KindAndNode { kind, node } => {
            record.kind_spelling() == kind.as_str() && record.node() == node
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

    let payload =
        validate_and_decode_known_payload(kind, raw.payload, PayloadDecodeContext::PersistedRead)?;

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

/// Derive the next legal sequence from an immutable physical event-log tail.
///
/// # Errors
///
/// Returns [`EventLogTailError::InvalidTail`] for a malformed present line and
/// [`EventLogTailError::SequenceOverflow`] when the tail sequence is `u64::MAX`.
pub fn successor_sequence(tail: &EventLogTail) -> Result<Sequence, EventLogTailError> {
    match tail {
        EventLogTail::Empty => Ok(Sequence::first()),
        EventLogTail::Present(line) => {
            let record = parse_event_line(&line.0)
                .map_err(|source| EventLogTailError::InvalidTail { source })?;
            let tail_sequence = record.sequence().get();
            let successor = tail_sequence
                .checked_add(1)
                .ok_or(EventLogTailError::SequenceOverflow { tail_sequence })?;
            Sequence::parse(successor).map_err(|source| EventLogTailError::InvalidTail { source })
        }
    }
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
    let known_payload = validate_and_decode_known_payload(
        kind,
        payload_value,
        PayloadDecodeContext::SubmittedAppend,
    )
    .map_err(|source| AppendError::InvalidSubmittedPayload { source })?;

    let sequence = successor_sequence(&tail).map_err(|source| match source {
        EventLogTailError::InvalidTail { source } => AppendError::InvalidTail { source },
        EventLogTailError::SequenceOverflow { tail_sequence } => {
            AppendError::SequenceOverflow { tail_sequence }
        }
    })?;

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
    context: PayloadDecodeContext,
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
    decode_known_payload(kind, payload, context)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadDecodeContext {
    PersistedRead,
    SubmittedAppend,
}

fn decode_known_payload(
    kind: WriteKind,
    payload: Value,
    context: PayloadDecodeContext,
) -> Result<KnownPayload, EventLogError> {
    if kind == WriteKind::RepositoryContract {
        return match context {
            PayloadDecodeContext::SubmittedAppend => serde_json::from_value(payload)
                .map(KnownPayload::RepositoryContract)
                .map_err(|source| EventLogError::InvalidKnownPayload {
                    kind,
                    detail: source.to_string(),
                }),
            PayloadDecodeContext::PersistedRead => {
                let current =
                    serde_json::from_value(payload.clone()).map(KnownPayload::RepositoryContract);
                match current {
                    Ok(payload) => Ok(payload),
                    Err(current_source) => serde_json::from_value(payload)
                        .map(KnownPayload::LegacyRepositoryContract)
                        .map_err(|legacy_source| EventLogError::InvalidKnownPayload {
                            kind,
                            detail: format!(
                                "current schema: {current_source}; legacy schema: {legacy_source}"
                            ),
                        }),
                }
            }
        };
    }

    let decoded = match kind {
        WriteKind::Dispatch => serde_json::from_value(payload).map(KnownPayload::Dispatch),
        WriteKind::DispatchCompletion => {
            serde_json::from_value(payload).map(KnownPayload::DispatchCompletion)
        }
        WriteKind::Delta => serde_json::from_value(payload).map(KnownPayload::Delta),
        WriteKind::EscalationOpen => {
            serde_json::from_value(payload).map(KnownPayload::EscalationOpen)
        }
        WriteKind::EscalationClose => {
            serde_json::from_value(payload).map(KnownPayload::EscalationClose)
        }
        WriteKind::KeyFinding => serde_json::from_value(payload).map(KnownPayload::KeyFinding),
        WriteKind::RepositoryContract => unreachable!("repository contract decoded above"),
        WriteKind::PlanningArtifactApproved => {
            serde_json::from_value(payload).map(KnownPayload::PlanningArtifactApproved)
        }
        WriteKind::CriterionExecution => {
            serde_json::from_value(payload).map(KnownPayload::CriterionExecution)
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
    /// Returned when a write-kind spelling is not one of the nine registered values.
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

    /// Returned when a finished result is empty after trimming.
    #[error("finished result cannot be blank")]
    BlankFinishedResult,

    /// Returned when an observed criterion result is empty after trimming.
    #[error("observed criterion result cannot be blank")]
    BlankObservedCriterionResult,

    /// Returned when an unpaid criterion reason is empty after trimming.
    #[error("unpaid criterion reason cannot be blank")]
    BlankUnpaidCriterionReason,

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
    use std::error::Error;
    use std::io;
    use std::time::{Duration, SystemTime};

    use serde_json::{Value, json};

    use crate::contract_measurement::{ObservedExitStatus, measure_contract_snapshot};
    use crate::event_log::{
        AppendError, ArtifactOutcome, CacheCreationInputTokens, CacheReadInputTokens,
        DispatchTokenUsage, EventBodyRef, EventKindName, EventLogError, EventLogTail,
        EventLogTailError, EventLogTailLine, EventRecord, EventRecordFilter, EventTimestamp,
        Evidence, EvidencePresence, FinishedResult, InputTokens, KnownPayload, NodeId,
        ObservedCriterionResult, OutputTokens, ReadKind, ReadPayload, RepositoryContractPayload,
        RepositoryName, RepositoryRoot, Sequence, Sha256Digest, UnpaidCriterionReason,
        UnparsedPayload, WriteKind, append_event, event_record_matches, parse_event_line,
        serialize_event_line, successor_sequence, validate_evidence_policy,
    };
    use crate::run_state::VersionPolicy;
    use crate::tracked_contract::{GateKind, parse_tracked_repository_contract};

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

    #[test]
    fn successor_sequence_covers_every_physical_tail_state() {
        assert_eq!(
            successor_sequence(&EventLogTail::Empty)
                .expect("empty tail successor")
                .get(),
            1
        );
        assert_eq!(
            successor_sequence(&EventLogTail::Present(EventLogTailLine::new(
                KNOWN_LINES[0]
            )))
            .expect("present tail successor")
            .get(),
            2
        );
        assert!(matches!(
            successor_sequence(&EventLogTail::Present(EventLogTailLine::new("not-json"))),
            Err(EventLogTailError::InvalidTail { .. })
        ));
        let maximum = r#"{"sequence":18446744073709551615,"timestamp":"2026-07-27T12:34:55.000Z","kind":"delta","node":"m1-s1","payload":{"message":"maximum"}}"#;
        assert!(matches!(
            successor_sequence(&EventLogTail::Present(EventLogTailLine::new(maximum))),
            Err(EventLogTailError::SequenceOverflow {
                tail_sequence: u64::MAX
            })
        ));
    }

    const KNOWN_LINES: [&str; 8] = [
        r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"dispatch","node":"m1-s1","payload":{"role":"step-executor","ref":"ca9788ded3daec9b9e9fd7679caa24e7c64a8193","evidence":"git rev-parse HEAD"}}"#,
        r#"{"sequence":2,"timestamp":"2026-07-27T12:34:57.000Z","kind":"delta","node":"m1-s1","payload":{"message":"Require exact UTC timestamp spelling in the event envelope."}}"#,
        r#"{"sequence":3,"timestamp":"2026-07-27T12:34:58.000Z","kind":"escalation-open","node":"m1-s1","payload":{"key":"timestamp-precision","question":"Which RFC 3339 sub-second precision is canonical?"}}"#,
        r#"{"sequence":4,"timestamp":"2026-07-27T12:34:59.000Z","kind":"escalation-close","node":"m1-s1","payload":{"key":"timestamp-precision","resolution":"Use milliseconds and a Z suffix."}}"#,
        r##"{"sequence":5,"timestamp":"2026-07-27T12:35:00.000Z","kind":"key-finding","node":"m1-s1","payload":{"finding":"The repository has exactly five tests at the ground-truth ref.","evidence":"git grep -n '#[test]' ca9788ded3daec9b9e9fd7679caa24e7c64a8193 -- crates/core/src/vision.rs"}}"##,
        r#"{"sequence":6,"timestamp":"2026-07-27T12:35:01.000Z","kind":"repository-contract","node":"m1-s1","payload":{"repository":"pce","repo_root":"/workspace/pce","stack":"Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)","format":"cargo fmt --all --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --workspace","preflight":"cargo check --workspace --all-targets","gates_rule":"From the repo root, all four gates must exit zero before committing.","install":"None required for gates.","evidence":"rustc --version\ncargo --version\ngit rev-parse --show-toplevel"}}"#,
        r#"{"sequence":7,"timestamp":"2026-07-27T12:35:02.000Z","kind":"planning-artifact-approved","node":"m1-s1","payload":{"path":"planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","evidence":"shasum -a 256 planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json"}}"#,
        r#"{"sequence":8,"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}}"#,
    ];
    const UNKNOWN_LINE: &str = r#"{"sequence":9,"timestamp":"2026-07-27T12:35:04.000Z","kind":"future-kind","node":"m1-s1","payload":{"nested":{"answer":42},"items":[true,null,"kept"]}}"#;
    const CURRENT_REPOSITORY_CONTRACT_LINE: &str = r#"{"sequence":10,"timestamp":"2026-07-27T12:35:05.000Z","kind":"repository-contract","node":"m2-s1","payload":{"repository":"pce","repo_root":"/workspace/pce","stated":{"format":"cargo fmt --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --release","version_policy":"NONE","branch_convention":"pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>","pull_request_convention":"step head targets the matching milestone integration branch"},"observations":{"format":0,"lint":0,"typecheck":0,"test":0,"build":0},"workflow_map":{"ci.yml":"cargo test --workspace","docs.yml":null},"appendable":{"environment_hazards":["stdin is reserved for event payload input"],"gate_orderings":["format before lint before typecheck before test before build"],"lockfile_rules":["Cargo.lock must remain synchronized with Cargo.toml"]},"evidence":"cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"}}"#;
    const TRACKED_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "cargo fmt --check",
      "lint": "cargo clippy --workspace --all-targets",
      "typecheck": "cargo check --workspace --all-targets",
      "test": "cargo test --workspace",
      "build": "cargo build --release"
    },
    "version_policy": "NONE",
    "branches": {
      "default": "main",
      "milestone": "pce/{vision}/milestone-{milestone}",
      "step": "pce/{vision}/m{milestone}-s{step}"
    },
    "pull_requests": {
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    },
    "workflows": [
      {
        "workflow": "ci.yml",
        "stand_in": {
          "kind": "COMMAND",
          "command": "cargo test --workspace"
        }
      },
      {
        "workflow": "release.yml",
        "stand_in": {
          "kind": "NONE"
        }
      }
    ]
  },
  "appendable": {
    "environment_hazards": ["pipe Codex stdin from /dev/null"],
    "gate_orderings": ["run cargo fmt --check before clippy"],
    "lockfile_rules": ["commit Cargo.lock when dependency resolution changes"]
  }
}"#;

    #[test]
    fn tracked_measurement_round_trips_every_current_payload_field() -> Result<(), Box<dyn Error>> {
        let tracked = parse_tracked_repository_contract(TRACKED_CONTRACT)?;
        let snapshot = measure_contract_snapshot(tracked.stated(), |_command| {
            Ok::<ObservedExitStatus, io::Error>(ObservedExitStatus::from_code(0))
        })?;
        let evidence = "cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release";
        let payload = RepositoryContractPayload::from_tracked_measurement(
            RepositoryName::new("pce"),
            RepositoryRoot::new("/workspace/pce"),
            &tracked,
            snapshot.gates(),
            Evidence::parse(evidence)?,
        );
        let record = EventRecord::known(
            Sequence::parse(9)?,
            EventTimestamp::parse("2026-07-27T12:35:04.000Z")?,
            NodeId::parse("m3-s1")?,
            KnownPayload::RepositoryContract(payload),
        );
        let line = serialize_event_line(&record)?;
        let reparsed = parse_event_line(&line)?;
        let EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) = reparsed.body_ref()
        else {
            panic!("current repository contract expected");
        };

        assert_eq!(
            serde_json::to_value(payload)?,
            json!({
                "repository": "pce",
                "repo_root": "/workspace/pce",
                "stated": {
                    "format": "cargo fmt --check",
                    "lint": "cargo clippy --workspace --all-targets",
                    "typecheck": "cargo check --workspace --all-targets",
                    "test": "cargo test --workspace",
                    "build": "cargo build --release",
                    "version_policy": "NONE",
                    "branch_convention": "default=main; milestone=pce/{vision}/milestone-{milestone}; step=pce/{vision}/m{milestone}-s{step}",
                    "pull_request_convention": "step_base=MILESTONE; milestone_base=DEFAULT; merge_method=SQUASH"
                },
                "observations": {
                    "format": 0,
                    "lint": 0,
                    "typecheck": 0,
                    "test": 0,
                    "build": 0
                },
                "workflow_map": {
                    "ci.yml": "cargo test --workspace",
                    "release.yml": null
                },
                "appendable": {
                    "environment_hazards": ["pipe Codex stdin from /dev/null"],
                    "gate_orderings": ["run cargo fmt --check before clippy"],
                    "lockfile_rules": ["commit Cargo.lock when dependency resolution changes"]
                },
                "evidence": evidence
            })
        );
        assert_eq!(payload.repository.as_str(), "pce");
        assert_eq!(payload.repo_root.as_str(), "/workspace/pce");
        assert_eq!(payload.stated.version_policy, VersionPolicy::None);
        for kind in [
            GateKind::Format,
            GateKind::Lint,
            GateKind::Typecheck,
            GateKind::Test,
            GateKind::Build,
        ] {
            let status = match kind {
                GateKind::Format => payload.observations.format,
                GateKind::Lint => payload.observations.lint,
                GateKind::Typecheck => payload.observations.typecheck,
                GateKind::Test => payload.observations.test,
                GateKind::Build => payload.observations.build,
            };
            assert_eq!(status.code(), 0);
        }
        assert_eq!(
            payload.workflow_map.as_map().get("ci.yml"),
            Some(&Some("cargo test --workspace".to_owned()))
        );
        assert_eq!(
            payload.workflow_map.as_map().get("release.yml"),
            Some(&None)
        );
        assert_eq!(payload.evidence.as_str(), evidence);
        Ok(())
    }

    #[test]
    fn raw_record_filters_match_known_and_unknown_kinds_exactly() -> Result<(), EventLogError> {
        let known = parse_event_line(KNOWN_LINES[1])?;
        let unknown = parse_event_line(UNKNOWN_LINE)?;
        let node = NodeId::parse("m1-s1")?;

        assert!(event_record_matches(&known, &EventRecordFilter::All));
        assert!(event_record_matches(
            &known,
            &EventRecordFilter::Kind(EventKindName::new("delta"))
        ));
        assert!(event_record_matches(
            &unknown,
            &EventRecordFilter::Kind(EventKindName::new("future-kind"))
        ));
        assert!(event_record_matches(
            &unknown,
            &EventRecordFilter::Node(node.clone())
        ));
        assert!(event_record_matches(
            &unknown,
            &EventRecordFilter::KindAndNode {
                kind: EventKindName::new("future-kind"),
                node,
            }
        ));
        assert!(!event_record_matches(
            &known,
            &EventRecordFilter::Kind(EventKindName::new("future-kind"))
        ));
        Ok(())
    }

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
    fn append_after_legacy_repository_contract_tail_uses_successor_sequence()
    -> Result<(), EventLogError> {
        let expected = concat!(
            r#"{"sequence":7,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m1-s2","payload":{"message":"append one validated event"}}"#,
            "\n"
        );
        let calls = Cell::new(0);
        let recorded = RefCell::new(Vec::new());

        let intent = append_event(
            WriteKind::Delta,
            UnparsedPayload::new(VALID_DELTA_PAYLOAD),
            EventLogTail::Present(EventLogTailLine::new(KNOWN_LINES[5])),
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
        assert_eq!(recorded.borrow().as_slice(), expected.as_bytes());
        assert_eq!(intent.as_bytes(), expected.as_bytes());
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
            WriteKind::CriterionExecution,
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
                    | (
                        5,
                        ReadPayload::Known(KnownPayload::LegacyRepositoryContract(_))
                    )
                    | (
                        6,
                        ReadPayload::Known(KnownPayload::PlanningArtifactApproved(_))
                    )
                    | (7, ReadPayload::Known(KnownPayload::CriterionExecution(_)))
            );
            assert!(expected_payload);
            assert_eq!(serialize_event_line(&record)?, line);
        }
        Ok(())
    }

    #[test]
    fn borrowed_body_view_preserves_all_known_and_unknown_correlations() -> Result<(), EventLogError>
    {
        for (index, line) in KNOWN_LINES.into_iter().enumerate() {
            let record = parse_event_line(line)?;
            let expected_payload = matches!(
                (index, record.body_ref()),
                (0, EventBodyRef::Known(KnownPayload::Dispatch(_)))
                    | (1, EventBodyRef::Known(KnownPayload::Delta(_)))
                    | (2, EventBodyRef::Known(KnownPayload::EscalationOpen(_)))
                    | (3, EventBodyRef::Known(KnownPayload::EscalationClose(_)))
                    | (4, EventBodyRef::Known(KnownPayload::KeyFinding(_)))
                    | (
                        5,
                        EventBodyRef::Known(KnownPayload::LegacyRepositoryContract(_))
                    )
                    | (
                        6,
                        EventBodyRef::Known(KnownPayload::PlanningArtifactApproved(_))
                    )
                    | (7, EventBodyRef::Known(KnownPayload::CriterionExecution(_)))
            );
            assert!(expected_payload);
        }

        let record = parse_event_line(UNKNOWN_LINE)?;
        let EventBodyRef::Unknown { kind, payload } = record.body_ref() else {
            panic!("unknown borrowed body expected");
        };
        assert_eq!(kind, "future-kind");
        assert_eq!(payload["nested"]["answer"], 42);
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
            WriteKind::CriterionExecution,
        ];
        let absent = [
            WriteKind::DispatchCompletion,
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
        let ReadPayload::Known(KnownPayload::LegacyRepositoryContract(payload)) = record.payload()
        else {
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
    fn legacy_repository_contract_line_remains_readable_and_exactly_serializable()
    -> Result<(), EventLogError> {
        let record = parse_event_line(KNOWN_LINES[5])?;
        assert_eq!(
            record.kind(),
            ReadKind::Known(WriteKind::RepositoryContract)
        );
        let ReadPayload::Known(KnownPayload::LegacyRepositoryContract(payload)) = record.payload()
        else {
            panic!("legacy repository contract expected");
        };
        assert_eq!(payload.repository.as_str(), "pce");
        assert_eq!(payload.repo_root.as_str(), "/workspace/pce");
        assert_eq!(
            payload.evidence.as_str(),
            "rustc --version\ncargo --version\ngit rev-parse --show-toplevel"
        );
        assert_eq!(serialize_event_line(&record)?, KNOWN_LINES[5]);
        Ok(())
    }

    #[test]
    fn current_repository_contract_round_trips_every_two_authority_field()
    -> Result<(), EventLogError> {
        let record = parse_event_line(CURRENT_REPOSITORY_CONTRACT_LINE)?;
        let ReadPayload::Known(KnownPayload::RepositoryContract(payload)) = record.payload() else {
            panic!("current repository contract expected");
        };
        assert_eq!(payload.repository.as_str(), "pce");
        assert_eq!(payload.repo_root.as_str(), "/workspace/pce");
        assert_eq!(payload.stated.format, "cargo fmt --check");
        assert_eq!(
            payload.stated.lint,
            "cargo clippy --workspace --all-targets"
        );
        assert_eq!(
            payload.stated.typecheck,
            "cargo check --workspace --all-targets"
        );
        assert_eq!(payload.stated.test, "cargo test --workspace");
        assert_eq!(payload.stated.build, "cargo build --release");
        assert_eq!(payload.stated.version_policy, VersionPolicy::None);
        assert_eq!(
            payload.stated.branch_convention,
            "pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>"
        );
        assert_eq!(
            payload.stated.pull_request_convention,
            "step head targets the matching milestone integration branch"
        );
        assert_eq!(payload.observations.format.get(), 0);
        assert_eq!(payload.observations.lint.get(), 0);
        assert_eq!(payload.observations.typecheck.get(), 0);
        assert_eq!(payload.observations.test.get(), 0);
        assert_eq!(payload.observations.build.get(), 0);
        assert_eq!(
            payload.workflow_map.as_map().get("ci.yml"),
            Some(&Some("cargo test --workspace".to_owned()))
        );
        assert_eq!(payload.workflow_map.as_map().get("docs.yml"), Some(&None));
        assert_eq!(
            payload.appendable.environment_hazards,
            ["stdin is reserved for event payload input"]
        );
        assert_eq!(
            payload.appendable.gate_orderings,
            ["format before lint before typecheck before test before build"]
        );
        assert_eq!(
            payload.appendable.lockfile_rules,
            ["Cargo.lock must remain synchronized with Cargo.toml"]
        );
        assert_eq!(
            payload.evidence.as_str(),
            "cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"
        );

        let serialized = serialize_event_line(&record)?;
        assert_eq!(serialized, CURRENT_REPOSITORY_CONTRACT_LINE);
        assert_eq!(parse_event_line(&serialized)?, record);
        Ok(())
    }

    #[test]
    fn current_repository_contract_rejects_unknown_fields() {
        let mut top_level: Value =
            serde_json::from_str(CURRENT_REPOSITORY_CONTRACT_LINE).expect("fixture must parse");
        top_level["payload"]["stack"] = json!("Rust");
        let mut nested = serde_json::from_str::<Value>(CURRENT_REPOSITORY_CONTRACT_LINE)
            .expect("fixture must parse");
        nested["payload"]["stated"]["preflight"] = json!("cargo check");

        for line in [top_level.to_string(), nested.to_string()] {
            assert!(matches!(
                parse_event_line(&line),
                Err(EventLogError::InvalidKnownPayload {
                    kind: WriteKind::RepositoryContract,
                    ..
                })
            ));
        }
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
        assert!(matches!(
            WriteKind::parse("dispatch-completion"),
            Ok(WriteKind::DispatchCompletion)
        ));
        assert_eq!(
            WriteKind::DispatchCompletion.as_str(),
            "dispatch-completion"
        );
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
        assert_eq!(serialized.join("\n").lines().count(), 8);
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

    #[test]
    fn dispatch_completion_literal_is_closed_and_byte_stable() {
        let literal = r#"{"sequence":8,"timestamp":"2026-07-27T12:34:56.000Z","kind":"dispatch-completion","node":"m3-s1","payload":{"issuance_sequence":7,"duration_ms":200,"usage":{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
        let record = parse_event_line(literal).expect("parse literal completion");
        assert_eq!(
            serialize_event_line(&record).expect("serialize completion"),
            literal
        );
        let EventBodyRef::Known(KnownPayload::DispatchCompletion(payload)) = record.body_ref()
        else {
            panic!("completion decoded as another kind");
        };
        assert_eq!(payload.issuance_sequence.get(), 7);
        assert_eq!(payload.duration_ms.get(), 200);
        assert_eq!(payload.artifact_outcome, ArtifactOutcome::NotValidated);

        for invalid in [
            literal.replace(
                "\"artifact_outcome\":\"not-validated\"",
                "\"artifact_outcome\":\"not-validated\",\"outer\":1",
            ),
            literal.replace("\"input_tokens\":101", "\"input_tokens\":101,\"nested\":1"),
            literal.replace("\"code\":0", "\"code\":0,\"nested\":1"),
        ] {
            assert!(matches!(
                parse_event_line(&invalid),
                Err(EventLogError::InvalidKnownPayload {
                    kind: WriteKind::DispatchCompletion,
                    ..
                })
            ));
        }
        let with_evidence = literal.replace(
            "\"issuance_sequence\":7",
            "\"evidence\":\"forbidden\",\"issuance_sequence\":7",
        );
        assert!(matches!(
            parse_event_line(&with_evidence),
            Err(EventLogError::ForbiddenEvidence {
                kind: WriteKind::DispatchCompletion
            })
        ));
    }

    #[test]
    fn dispatch_absence_literals_remain_byte_stable() {
        for reason in [
            "turn-failed",
            "no-terminal-turn",
            "malformed-terminal-data",
            "duplicate-terminal-data",
            "contradictory-terminal-data",
            "claude-malformed-result",
            "claude-missing-usage",
            "claude-error-envelope",
            "claude-exit-envelope-contradiction",
        ] {
            let literal = format!(
                r#"{{"sequence":8,"timestamp":"2026-07-27T12:34:56.000Z","kind":"dispatch-completion","node":"m3-s1","payload":{{"issuance_sequence":7,"duration_ms":200,"usage":{{"availability":"absent","reason":"{reason}"}},"exit_status":{{"kind":"exited","code":1}},"artifact_outcome":"not-validated"}}}}"#
            );
            let record = parse_event_line(&literal).expect("parse absent completion literal");
            assert_eq!(
                serialize_event_line(&record).expect("serialize absent completion"),
                literal
            );
        }
    }

    #[test]
    fn claude_measured_usage_has_exact_closed_serialization() {
        let usage = DispatchTokenUsage::ClaudeMeasured {
            input_tokens: InputTokens::new(2),
            output_tokens: OutputTokens::new(4),
            cache_creation_input_tokens: CacheCreationInputTokens::new(9572),
            cache_read_input_tokens: CacheReadInputTokens::new(15410),
        };
        let literal = r#"{"availability":"claude-measured","input_tokens":2,"output_tokens":4,"cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}"#;
        assert_eq!(
            serde_json::to_string(&usage).expect("serialize usage"),
            literal
        );
        assert_eq!(
            serde_json::from_str::<DispatchTokenUsage>(literal).expect("parse usage"),
            usage
        );
        let with_billing = literal.replace(
            "\"cache_read_input_tokens\":15410",
            "\"cache_read_input_tokens\":15410,\"total_cost_usd\":0.104116",
        );
        assert!(serde_json::from_str::<DispatchTokenUsage>(&with_billing).is_err());
    }

    #[test]
    fn artifact_outcome_has_exact_closed_serialization() {
        for outcome in [
            ArtifactOutcome::NotValidated,
            ArtifactOutcome::Validated,
            ArtifactOutcome::Missing,
            ArtifactOutcome::Truncated,
            ArtifactOutcome::SchemaInvalid,
            ArtifactOutcome::SchemaViolating,
        ] {
            let spelling = match outcome {
                ArtifactOutcome::NotValidated => "not-validated",
                ArtifactOutcome::Validated => "validated",
                ArtifactOutcome::Missing => "missing",
                ArtifactOutcome::Truncated => "truncated",
                ArtifactOutcome::SchemaInvalid => "schema-invalid",
                ArtifactOutcome::SchemaViolating => "schema-violating",
            };
            let encoded = serde_json::to_string(&outcome).expect("serialize artifact outcome");
            assert_eq!(encoded, format!("\"{spelling}\""));
            assert_eq!(
                serde_json::from_str::<ArtifactOutcome>(&encoded)
                    .expect("deserialize artifact outcome"),
                outcome
            );
        }
    }

    #[test]
    fn criterion_execution_wire_shapes_are_strict_and_exact() -> Result<(), EventLogError> {
        let lines = [
            KNOWN_LINES[7],
            r#"{"sequence":2,"timestamp":"2026-07-27T12:35:04.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed","observed_result":"The command exited 7."},"evidence":"git rev-parse HEAD\n./broken-command"}}"#,
            r#"{"sequence":3,"timestamp":"2026-07-27T12:35:05.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}}"#,
        ];
        for line in lines {
            let record = parse_event_line(line)?;
            assert!(matches!(
                record.payload(),
                ReadPayload::Known(KnownPayload::CriterionExecution(_))
            ));
            assert_eq!(serialize_event_line(&record)?, line);
        }
        assert!(matches!(
            WriteKind::parse("criterion-execution"),
            Ok(WriteKind::CriterionExecution)
        ));
        assert_eq!(
            WriteKind::CriterionExecution.as_str(),
            "criterion-execution"
        );
        Ok(())
    }

    #[test]
    fn criterion_execution_domain_strings_trim_and_reject_blanks() -> Result<(), EventLogError> {
        assert_eq!(
            FinishedResult::parse("  main@0123456789abcdef  ")?.as_str(),
            "main@0123456789abcdef"
        );
        assert_eq!(
            ObservedCriterionResult::parse("  The command exited 0.  ")?.as_str(),
            "The command exited 0."
        );
        assert_eq!(
            UnpaidCriterionReason::parse("  The run cannot activate the human-installed hook.  ")?
                .as_str(),
            "The run cannot activate the human-installed hook."
        );
        for value in ["", "   "] {
            assert!(matches!(
                FinishedResult::parse(value),
                Err(EventLogError::BlankFinishedResult)
            ));
            assert!(matches!(
                ObservedCriterionResult::parse(value),
                Err(EventLogError::BlankObservedCriterionResult)
            ));
            assert!(matches!(
                UnpaidCriterionReason::parse(value),
                Err(EventLogError::BlankUnpaidCriterionReason)
            ));
        }
        Ok(())
    }

    fn assert_invalid_criterion_execution_payload(payload: &str) -> Result<(), EventLogError> {
        let calls = Cell::new(0);
        let result = append_event(
            WriteKind::CriterionExecution,
            UnparsedPayload::new(payload),
            EventLogTail::Empty,
            NodeId::parse("m3-s2")?,
            append_time(),
            |_| {
                calls.set(calls.get() + 1);
                Ok::<(), io::Error>(())
            },
        );

        assert!(matches!(
            result,
            Err(AppendError::InvalidSubmittedPayload {
                source: EventLogError::InvalidKnownPayload {
                    kind: WriteKind::CriterionExecution,
                    ..
                }
            })
        ));
        assert_eq!(calls.get(), 0);
        Ok(())
    }

    #[test]
    fn criterion_execution_rejects_every_malformed_payload_before_append()
    -> Result<(), EventLogError> {
        assert_invalid_criterion_execution_payload(
            r#"{"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;

        let calls = Cell::new(0);
        let missing_evidence = append_event(
            WriteKind::CriterionExecution,
            UnparsedPayload::new(
                r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."}}"#,
            ),
            EventLogTail::Empty,
            NodeId::parse("m3-s2")?,
            append_time(),
            |_| {
                calls.set(calls.get() + 1);
                Ok::<(), io::Error>(())
            },
        );
        assert!(matches!(
            missing_evidence,
            Err(AppendError::InvalidSubmittedPayload {
                source: EventLogError::MissingRequiredEvidence {
                    kind: WriteKind::CriterionExecution
                }
            })
        ));
        assert_eq!(calls.get(), 0);

        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command","extra":true}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"green","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed"},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0.","reason":"not allowed"},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed"},"evidence":"git rev-parse HEAD\n./broken-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed","observed_result":"The command exited 7.","reason":"not allowed"},"evidence":"git rev-parse HEAD\n./broken-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid"},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook.","observed_result":"not allowed"},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0.","extra":true},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0.","extra":true},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;

        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"   ","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"   ","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":""},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"   "},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"   ","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":""},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"   "},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":""},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}"#,
        )?;
        assert_invalid_criterion_execution_payload(
            r#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"   "},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}"#,
        )?;
        Ok(())
    }

    #[test]
    fn appends_exact_criterion_execution_bytes_at_fixed_timestamp() -> Result<(), EventLogError> {
        let payload = r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#;
        let expected = concat!(
            r#"{"sequence":1,"timestamp":"2026-07-27T12:35:03.000Z","kind":"criterion-execution","node":"m3-s2","payload":{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}}"#,
            "\n"
        );
        let calls = Cell::new(0);
        let recorded = RefCell::new(Vec::new());
        let timestamp = SystemTime::UNIX_EPOCH + Duration::from_secs(APPEND_TIME_SECONDS + 7);

        let intent = append_event(
            WriteKind::CriterionExecution,
            UnparsedPayload::new(payload),
            EventLogTail::Empty,
            NodeId::parse("m3-s2")?,
            timestamp,
            |bytes| {
                calls.set(calls.get() + 1);
                recorded.borrow_mut().extend_from_slice(bytes);
                Ok::<(), io::Error>(())
            },
        )
        .map_err(append_test_error)?;

        assert_eq!(calls.get(), 1);
        assert_eq!(recorded.borrow().as_slice(), expected.as_bytes());
        assert_eq!(intent.as_bytes(), expected.as_bytes());
        assert_eq!(
            intent
                .as_bytes()
                .iter()
                .filter(|byte| **byte == b'\n')
                .count(),
            1
        );
        Ok(())
    }
}
