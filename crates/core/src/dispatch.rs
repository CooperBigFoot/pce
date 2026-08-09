//! dispatch_invocation : DispatchTarget × DispatchEnvelope → Executable × Argv; compose_gate_arguments : Option<DispatchRole> × AbsoluteOutputPath × Option<AbsoluteGateExecClientPath> × ArgumentVector → Result<ArgumentVector, DispatchError>; dispatch_projection : DispatchEnvelope × DispatchLogging × EventLogTail → JSON; codex_terminal_usage : CodexTerminalObservation* × DispatchExitStatus → DispatchTokenUsage; claude_result_usage : ClaudeResultEnvelope × DispatchExitStatus → DispatchTokenUsage; SeatbeltCapability = classify(permissive_profile_probe_status); planning_role_frame : DispatchRole × ActReversibility × ArgumentVector → ArgumentVector ∪ PlanningFrameError   (pure, deterministic)
//! This module describes complete shell-free child invocations; the binary adapter performs all I/O and process work.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;

use crate::contract_measurement::ObservedExitStatus;
use crate::dispatch_process_identity::AbsoluteRequiredArtifactPath;
use crate::event_log::{
    ArtifactOutcome, CacheCreationInputTokens, CacheReadInputTokens, CachedInputTokens,
    DispatchCompletionPayload, DispatchDuration, DispatchExitStatus, DispatchPayload, DispatchRef,
    DispatchRole, DispatchTokenUsage, EventLogTail, EventLogTailError, EventTimestamp, Evidence,
    InputTokens, NodeId, OutputTokens, ReasoningOutputTokens, Sequence, UsageAbsenceReason,
    WriteKind, successor_sequence,
};
use crate::gate_execution::{AbsoluteGateExecClientPath, GateExecutionRecorderConfig};

const SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";
const PERMISSIVE_SEATBELT_PROFILE: &str = "(version 1)(allow default)";
const PLANNING_FRAME_SEPARATOR: &str = "\n\n## Binary-owned reversibility obligation\n\n";
const REPEATABLE_PLANNING_OBLIGATION: &str = "The step's act is repeatable. The plan must retain an explicit not-touched scope fence and exact expected values for every assertion. The plan must not contain a pre-derived argument that the design is correct.";
const IRREVERSIBLE_PLANNING_OBLIGATION: &str = "The step's act cannot be repeated. The plan must retain the existing front-loaded pre-proof of correctness, an explicit not-touched scope fence, and exact expected values for every assertion.";

/// Whether the planning act can be performed again after this dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActReversibility {
    /// The act can be repeated, so the plan relies on exact assertions and later falsification.
    Repeatable,
    /// The act cannot be repeated, so the plan retains a front-loaded correctness proof.
    Irreversible,
}

impl ActReversibility {
    /// Parse the only two CLI spellings accepted for planning acts.
    ///
    /// # Errors
    ///
    /// Returns [`PlanningFrameError::UnsupportedPlanningAct`] for any other spelling.
    pub fn parse(raw: impl Into<String>) -> Result<Self, PlanningFrameError> {
        let act = raw.into();
        match act.as_str() {
            "repeatable" => Ok(Self::Repeatable),
            "irreversible" => Ok(Self::Irreversible),
            _ => Err(PlanningFrameError::UnsupportedPlanningAct { act }),
        }
    }

    const fn obligation(self) -> &'static str {
        match self {
            Self::Repeatable => REPEATABLE_PLANNING_OBLIGATION,
            Self::Irreversible => IRREVERSIBLE_PLANNING_OBLIGATION,
        }
    }
}

/// A binary-owned planning-role frame could not be composed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanningFrameError {
    /// The CLI supplied a planning-act spelling outside the closed enum.
    #[error("unsupported planning act `{act}`; expected `repeatable` or `irreversible`")]
    UnsupportedPlanningAct { act: String },
    /// The CLI supplied a planning act without the complete logging authority group.
    #[error("`--planning-act` requires complete dispatch logging metadata")]
    MissingLoggingMetadata,
    /// The logging role is not one of the two planning roles that own this frame.
    #[error(
        "planning act is supported only for roles `step-plan-writer` and `step-plan-critic`; rejected role `{role}`"
    )]
    UnsupportedRole { role: String },
    /// The planning role has no non-empty final caller argument to carry the frame.
    #[error(
        "planning role `{role}` requires a non-empty final caller argument to carry its binary-owned frame"
    )]
    MissingFinalCallerArgument { role: String },
}

/// Append the binary-owned reversibility obligation to the final caller argument only.
///
/// # Errors
///
/// Returns [`PlanningFrameError::UnsupportedRole`] for a non-planning role and
/// [`PlanningFrameError::MissingFinalCallerArgument`] when the final argument is absent or empty.
pub fn compose_planning_role_frame(
    role: &DispatchRole,
    reversibility: ActReversibility,
    mut arguments: ArgumentVector,
) -> Result<ArgumentVector, PlanningFrameError> {
    let role_name = role.as_str();
    if !matches!(role_name, "step-plan-writer" | "step-plan-critic") {
        return Err(PlanningFrameError::UnsupportedRole {
            role: role_name.to_owned(),
        });
    }
    let final_argument = arguments
        .0
        .last_mut()
        .filter(|argument| !argument.is_empty())
        .ok_or_else(|| PlanningFrameError::MissingFinalCallerArgument {
            role: role_name.to_owned(),
        })?;
    final_argument.push_str(PLANNING_FRAME_SEPARATOR);
    final_argument.push_str(reversibility.obligation());
    Ok(arguments)
}

/// Typed log metadata carried beside, rather than inside, a child envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchLogging {
    pub node: NodeId,
    pub role: DispatchRole,
    pub dispatch_ref: DispatchRef,
    pub evidence: Evidence,
    pub required_artifact_path: AbsoluteRequiredArtifactPath,
}

/// Construct the shared concrete dispatch issuance payload.
pub fn dispatch_payload(logging: &DispatchLogging) -> DispatchPayload {
    DispatchPayload {
        role: logging.role.clone(),
        r#ref: logging.dispatch_ref.clone(),
        evidence: logging.evidence.clone(),
    }
}

/// Construct the shared concrete m3 dispatch completion payload.
pub fn dispatch_completion_payload(
    issuance_sequence: Sequence,
    duration_ms: DispatchDuration,
    usage: DispatchTokenUsage,
    exit_status: DispatchExitStatus,
    artifact_outcome: ArtifactOutcome,
) -> DispatchCompletionPayload {
    DispatchCompletionPayload {
        issuance_sequence,
        duration_ms,
        usage,
        exit_status,
        artifact_outcome,
    }
}

/// A typed prospective value that cannot carry a fabricated observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deferred<T> {
    state: DeferredState,
    #[serde(skip)]
    marker: PhantomData<fn() -> T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DeferredState {
    Deferred,
}

impl<T> Default for Deferred<T> {
    fn default() -> Self {
        Self {
            state: DeferredState::Deferred,
            marker: PhantomData,
        }
    }
}

/// The exact stdin portion of an ordered shell-free invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchInvocationStdin {
    binding: &'static str,
    bytes: Option<Vec<u8>>,
}

/// The complete ordered shell-free child invocation shared by live and projection paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchInvocation {
    target: DispatchTarget,
    executable: String,
    argv: Vec<String>,
    cwd: String,
    environment: BTreeMap<String, String>,
    stdin: DispatchInvocationStdin,
    schema_path: Option<String>,
    output_path: Option<String>,
}

impl DispatchInvocation {
    /// Return the closed dispatch route.
    pub const fn target(&self) -> DispatchTarget {
        self.target
    }
    /// Return the program name passed directly to the process adapter.
    pub fn executable(&self) -> &str {
        &self.executable
    }
    /// Borrow the complete ordered child argument vector.
    pub fn argv(&self) -> &[String] {
        &self.argv
    }
    /// Return the exact working-directory spelling.
    pub fn cwd(&self) -> &str {
        &self.cwd
    }
    /// Borrow the complete explicit child environment.
    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }
    /// Borrow exact plan bytes, or return `None` for null stdin.
    pub fn stdin_bytes(&self) -> Option<&[u8]> {
        self.stdin.bytes.as_deref()
    }
}

/// Render the complete ordered child invocation from an envelope.
pub fn dispatch_invocation(envelope: &DispatchEnvelope) -> DispatchInvocation {
    let cwd = envelope.working_directory().as_path().display().to_string();
    let mut argv = envelope.target().argv_prefix(&cwd);
    if envelope.target() == DispatchTarget::Codex {
        if let Some(sandbox) = envelope.sandbox() {
            argv.extend(["--sandbox".to_owned(), sandbox.as_str().to_owned()]);
        }
        if let Some(path) = envelope.schema_path() {
            argv.extend([
                "--output-schema".to_owned(),
                path.as_path().display().to_string(),
            ]);
        }
        if let Some(path) = envelope.output_path() {
            argv.extend(["-o".to_owned(), path.as_path().display().to_string()]);
        }
    }
    argv.extend(envelope.arguments().as_slice().iter().cloned());
    let environment = envelope
        .environment()
        .iter()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect();
    let stdin = match envelope.stdin() {
        StdinBinding::Null => DispatchInvocationStdin {
            binding: "null",
            bytes: None,
        },
        StdinBinding::PlanBytes(bytes) => DispatchInvocationStdin {
            binding: "plan-bytes",
            bytes: Some(bytes.clone()),
        },
    };
    DispatchInvocation {
        target: envelope.target(),
        executable: envelope.executable().as_str().to_owned(),
        argv,
        cwd,
        environment,
        stdin,
        schema_path: envelope
            .schema_path()
            .map(|path| path.as_path().display().to_string()),
        output_path: envelope
            .output_path()
            .map(|path| path.as_path().display().to_string()),
    }
}

/// Read-only facts from which a dry-run projection can be computed.
pub struct DispatchProjectionInput<'a> {
    envelope: &'a DispatchEnvelope,
    logging: &'a DispatchLogging,
    log_tail: &'a EventLogTail,
}

impl<'a> DispatchProjectionInput<'a> {
    /// Construct a projection input containing exactly three immutable domain facts.
    ///
    /// ```
    /// use pce_core::{AbsoluteWorkingDirectory, DispatchEnvelope, DispatchLogging, DispatchProjectionInput, DispatchRef, DispatchRole, DispatchTarget, EventLogTail, Evidence, NodeId, StdinBinding};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let envelope = DispatchEnvelope::new(DispatchTarget::Codex, AbsoluteWorkingDirectory::parse("/tmp")?, StdinBinding::Null);
    /// let logging = DispatchLogging { node: NodeId::parse("m3-s2")?, role: DispatchRole::new("step-executor"), dispatch_ref: DispatchRef::new("ref"), evidence: Evidence::parse("fixture")?, required_artifact_path: pce_core::AbsoluteRequiredArtifactPath::parse("/workspace/result.json")? };
    /// let tail = EventLogTail::Empty;
    /// let _input = DispatchProjectionInput::new(&envelope, &logging, &tail);
    /// # Ok(()) }
    /// ```
    /// ```compile_fail
    /// use pce_core::{AbsoluteWorkingDirectory, DispatchEnvelope, DispatchLogging, DispatchProjectionInput, DispatchRef, DispatchRole, DispatchTarget, EventLogTail, Evidence, NodeId, StdinBinding};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let envelope = DispatchEnvelope::new(DispatchTarget::Codex, AbsoluteWorkingDirectory::parse("/tmp")?, StdinBinding::Null);
    /// let logging = DispatchLogging { node: NodeId::parse("m3-s2")?, role: DispatchRole::new("step-executor"), dispatch_ref: DispatchRef::new("ref"), evidence: Evidence::parse("fixture")?, required_artifact_path: pce_core::AbsoluteRequiredArtifactPath::parse("/workspace/result.json")? };
    /// let tail = EventLogTail::Empty;
    /// let _input = DispatchProjectionInput::new(&envelope, &logging, &tail, |_bytes: &[u8]| Ok::<(), std::io::Error>(()));
    /// # Ok(()) }
    /// ```
    pub fn new(
        envelope: &'a DispatchEnvelope,
        logging: &'a DispatchLogging,
        log_tail: &'a EventLogTail,
    ) -> Self {
        Self {
            envelope,
            logging,
            log_tail,
        }
    }
}

#[derive(Serialize)]
struct DispatchProjection<'a> {
    envelope: DispatchInvocation,
    issuance: ProjectedIssuance<'a>,
    completion: ProjectedCompletion<'a>,
}

#[derive(Serialize)]
struct ProjectedIssuance<'a> {
    sequence: Deferred<Sequence>,
    timestamp: Deferred<EventTimestamp>,
    kind: &'static str,
    node: &'a NodeId,
    payload: DispatchPayload,
}

#[derive(Serialize)]
struct ProjectedCompletion<'a> {
    sequence: Deferred<Sequence>,
    timestamp: Deferred<EventTimestamp>,
    kind: &'static str,
    node: &'a NodeId,
    payload: ProjectedCompletionPayload,
}

#[derive(Serialize)]
struct ProjectedCompletionPayload {
    issuance_sequence: Deferred<Sequence>,
    duration_ms: Deferred<DispatchDuration>,
    usage: Deferred<DispatchTokenUsage>,
    exit_status: Deferred<DispatchExitStatus>,
    artifact_outcome: ProjectedArtifactOutcome,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ProjectedArtifactOutcome {
    Observed(ArtifactOutcome),
    Deferred(Deferred<ArtifactOutcome>),
}

/// A dispatch projection could not be rendered from the supplied typed facts.
#[derive(Debug, Error)]
pub enum DispatchProjectionError {
    /// The immutable event-log tail is malformed or has no successor.
    #[error("dispatch projection event-log tail is invalid: {source}")]
    InvalidTail { source: EventLogTailError },
    /// The typed projection unexpectedly failed JSON serialization.
    #[error("dispatch projection could not be serialized: {source}")]
    SerializationFailed { source: serde_json::Error },
}

/// Render one compact machine-readable dry-run projection without a terminal newline.
///
/// # Errors
///
/// Returns [`DispatchProjectionError::InvalidTail`] if the immutable log tail is invalid and
/// [`DispatchProjectionError::SerializationFailed`] if serialization fails.
pub fn render_dispatch_projection(
    input: DispatchProjectionInput<'_>,
) -> Result<String, DispatchProjectionError> {
    successor_sequence(input.log_tail)
        .map_err(|source| DispatchProjectionError::InvalidTail { source })?;
    let artifact_outcome =
        if input.envelope.schema_path().is_some() && input.envelope.output_path().is_some() {
            ProjectedArtifactOutcome::Deferred(Deferred::default())
        } else {
            ProjectedArtifactOutcome::Observed(ArtifactOutcome::NotValidated)
        };
    let projection = DispatchProjection {
        envelope: dispatch_invocation(input.envelope),
        issuance: ProjectedIssuance {
            sequence: Deferred::default(),
            timestamp: Deferred::default(),
            kind: WriteKind::Dispatch.as_str(),
            node: &input.logging.node,
            payload: dispatch_payload(input.logging),
        },
        completion: ProjectedCompletion {
            sequence: Deferred::default(),
            timestamp: Deferred::default(),
            kind: WriteKind::DispatchCompletion.as_str(),
            node: &input.logging.node,
            payload: ProjectedCompletionPayload {
                issuance_sequence: Deferred::default(),
                duration_ms: Deferred::default(),
                usage: Deferred::default(),
                exit_status: Deferred::default(),
                artifact_outcome,
            },
        },
    };
    serde_json::to_string(&projection)
        .map_err(|source| DispatchProjectionError::SerializationFailed { source })
}

/// One adapter-observed JSONL fact relevant to terminal classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexTerminalObservation {
    /// The physical line did not parse as JSON.
    MalformedLine,
    /// A parseable JSON value other than an object, or an object with no known terminal `type`,
    /// is non-terminal rather than malformed.
    NonTerminal,
    /// A completed turn with either exact counters or malformed usage.
    TurnCompleted(Option<CodexTerminalUsage>),
    /// A failed turn, recording whether a forbidden usage key was present.
    TurnFailed { usage_present: bool },
}

/// The four exact counters from a well-formed completed turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodexTerminalUsage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

/// Classify terminal observations after the child exit is known.
///
/// Invalid-data rows take precedence over well-formed outcomes, in their documented order.
/// Signal termination counts as a nonzero exit: absent becomes `NoTerminalTurn`, completed is
/// contradictory, and failed remains `TurnFailed`.
pub fn classify_codex_terminal_usage(
    observations: &[CodexTerminalObservation],
    exit_status: DispatchExitStatus,
) -> Result<DispatchTokenUsage, UsageAbsenceReason> {
    let completed = observations
        .iter()
        .filter(|item| matches!(item, CodexTerminalObservation::TurnCompleted(_)))
        .collect::<Vec<_>>();
    let failed = observations
        .iter()
        .filter(|item| matches!(item, CodexTerminalObservation::TurnFailed { .. }))
        .collect::<Vec<_>>();
    // Precedence is specification: malformed, duplicate, contradictory, then valid outcomes.
    if observations
        .iter()
        .any(|item| matches!(item, CodexTerminalObservation::MalformedLine))
        || completed
            .iter()
            .any(|item| matches!(item, CodexTerminalObservation::TurnCompleted(None)))
        || failed.iter().any(|item| {
            matches!(
                item,
                CodexTerminalObservation::TurnFailed {
                    usage_present: true
                }
            )
        })
    {
        return Err(UsageAbsenceReason::MalformedTerminalData);
    }
    if completed.len() > 1 || failed.len() > 1 {
        return Err(UsageAbsenceReason::DuplicateTerminalData);
    }
    if !completed.is_empty() && !failed.is_empty() {
        return Err(UsageAbsenceReason::ContradictoryTerminalData);
    }
    let zero = matches!(exit_status, DispatchExitStatus::Exited { code } if code.get() == 0);
    if (!completed.is_empty() && !zero)
        || (!failed.is_empty() && zero)
        || (completed.is_empty() && failed.is_empty() && zero)
    {
        return Err(UsageAbsenceReason::ContradictoryTerminalData);
    }
    if let Some(CodexTerminalObservation::TurnCompleted(Some(usage))) = completed.first().copied() {
        return Ok(DispatchTokenUsage::Measured {
            input_tokens: InputTokens::new(usage.input_tokens),
            cached_input_tokens: CachedInputTokens::new(usage.cached_input_tokens),
            output_tokens: OutputTokens::new(usage.output_tokens),
            reasoning_output_tokens: ReasoningOutputTokens::new(usage.reasoning_output_tokens),
        });
    }
    if !failed.is_empty() {
        return Ok(DispatchTokenUsage::Absent {
            reason: UsageAbsenceReason::TurnFailed,
        });
    }
    Ok(DispatchTokenUsage::Absent {
        reason: UsageAbsenceReason::NoTerminalTurn,
    })
}

/// One parsed Claude result envelope, closed over the states relevant to usage classification.
#[derive(Debug, Clone)]
pub enum ClaudeResultEnvelope {
    /// The bytes did not form a valid Claude result envelope.
    Malformed,
    /// The result claims success and may contain all required typed usage counters.
    Success { usage: Option<ClaudeResultUsage> },
    /// The result claims failure; any supplied usage is intentionally discarded.
    Error,
}

/// The four exact counters from a complete Claude success result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaudeResultUsage {
    input_tokens: InputTokens,
    output_tokens: OutputTokens,
    cache_creation_input_tokens: CacheCreationInputTokens,
    cache_read_input_tokens: CacheReadInputTokens,
}

/// Parse one Claude result envelope without performing I/O.
pub fn parse_claude_result(bytes: &[u8]) -> ClaudeResultEnvelope {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return ClaudeResultEnvelope::Malformed;
    };
    let Some(object) = value.as_object() else {
        return ClaudeResultEnvelope::Malformed;
    };
    let Some(is_error) = object.get("is_error").and_then(serde_json::Value::as_bool) else {
        return ClaudeResultEnvelope::Malformed;
    };
    if is_error {
        return ClaudeResultEnvelope::Error;
    }
    let Some(usage) = object.get("usage") else {
        return ClaudeResultEnvelope::Success { usage: None };
    };
    let Some(usage) = usage.as_object() else {
        return ClaudeResultEnvelope::Malformed;
    };
    for name in [
        "input_tokens",
        "output_tokens",
        "cache_creation_input_tokens",
        "cache_read_input_tokens",
    ] {
        if usage
            .get(name)
            .is_some_and(|value| value.as_u64().is_none())
        {
            return ClaudeResultEnvelope::Malformed;
        }
    }
    let (
        Some(input_tokens),
        Some(output_tokens),
        Some(cache_creation_input_tokens),
        Some(cache_read_input_tokens),
    ) = (
        usage
            .get("input_tokens")
            .and_then(serde_json::Value::as_u64),
        usage
            .get("output_tokens")
            .and_then(serde_json::Value::as_u64),
        usage
            .get("cache_creation_input_tokens")
            .and_then(serde_json::Value::as_u64),
        usage
            .get("cache_read_input_tokens")
            .and_then(serde_json::Value::as_u64),
    )
    else {
        return ClaudeResultEnvelope::Success { usage: None };
    };
    ClaudeResultEnvelope::Success {
        usage: Some(ClaudeResultUsage {
            input_tokens: InputTokens::new(input_tokens),
            output_tokens: OutputTokens::new(output_tokens),
            cache_creation_input_tokens: CacheCreationInputTokens::new(cache_creation_input_tokens),
            cache_read_input_tokens: CacheReadInputTokens::new(cache_read_input_tokens),
        }),
    }
}

/// Classify a parsed Claude result against its process exit.
///
/// # Errors
///
/// Returns [`UsageAbsenceReason::ClaudeMalformedResult`] for malformed envelopes,
/// [`UsageAbsenceReason::ClaudeExitEnvelopeContradiction`] when the envelope meaning and exit
/// disagree, [`UsageAbsenceReason::ClaudeErrorEnvelope`] for an agreeing error result, and
/// [`UsageAbsenceReason::ClaudeMissingUsage`] for an agreeing success without all four counters.
pub fn classify_claude_result(
    envelope: &ClaudeResultEnvelope,
    exit_status: DispatchExitStatus,
) -> Result<DispatchTokenUsage, UsageAbsenceReason> {
    if matches!(envelope, ClaudeResultEnvelope::Malformed) {
        return Err(UsageAbsenceReason::ClaudeMalformedResult);
    }
    let zero = matches!(exit_status, DispatchExitStatus::Exited { code } if code.get() == 0);
    match envelope {
        ClaudeResultEnvelope::Malformed => Err(UsageAbsenceReason::ClaudeMalformedResult),
        ClaudeResultEnvelope::Error if zero => {
            Err(UsageAbsenceReason::ClaudeExitEnvelopeContradiction)
        }
        ClaudeResultEnvelope::Error => Err(UsageAbsenceReason::ClaudeErrorEnvelope),
        ClaudeResultEnvelope::Success { .. } if !zero => {
            Err(UsageAbsenceReason::ClaudeExitEnvelopeContradiction)
        }
        ClaudeResultEnvelope::Success { usage: None } => {
            Err(UsageAbsenceReason::ClaudeMissingUsage)
        }
        ClaudeResultEnvelope::Success { usage: Some(usage) } => {
            Ok(DispatchTokenUsage::ClaudeMeasured {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_creation_input_tokens: usage.cache_creation_input_tokens,
                cache_read_input_tokens: usage.cache_read_input_tokens,
            })
        }
    }
}

/// A program name passed directly to a process adapter, never to a shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executable(String);

impl Executable {
    /// Parse a non-empty program name without normalization or lookup.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::EmptyExecutable`] when `raw` has zero bytes.
    #[instrument]
    pub fn parse(raw: &str) -> Result<Self, DispatchError> {
        if raw.is_empty() {
            return Err(DispatchError::EmptyExecutable {
                executable: raw.to_owned(),
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the program name unchanged.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The closed set of dispatch routes and their fixed executables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DispatchTarget {
    /// A Codex headless execution.
    Codex,
    /// A direct Claude headless gate execution.
    Gate,
    /// A pce-authored Seatbelt profile invocation used by contract measurement.
    Seatbelt,
}

impl DispatchTarget {
    /// Return the route's fixed executable.
    pub fn executable(self) -> Executable {
        Executable(match self {
            Self::Codex => "codex".to_owned(),
            Self::Gate => "claude".to_owned(),
            Self::Seatbelt => SEATBELT_EXECUTABLE.to_owned(),
        })
    }

    fn argv_prefix(self, cwd: &str) -> Vec<String> {
        match self {
            Self::Codex => vec![
                "exec".to_owned(),
                "--json".to_owned(),
                "-C".to_owned(),
                cwd.to_owned(),
            ],
            Self::Gate => vec![
                "-p".to_owned(),
                "--output-format".to_owned(),
                "json".to_owned(),
            ],
            Self::Seatbelt => Vec::new(),
        }
    }
}

/// The caller-supplied argument tail in caller order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArgumentVector(Vec<String>);

impl ArgumentVector {
    /// Store complete child arguments without filtering or normalization.
    pub fn new(arguments: Vec<String>) -> Self {
        Self(arguments)
    }

    /// Borrow the ordered complete child arguments.
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Report whether the child has no arguments.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Compose the role-owned argument frame for a gate dispatch.
///
/// # Errors
///
/// Returns [`DispatchError::FalsificationCriticCallerSystemPrompt`] when the exact
/// `falsification-critic` role's caller tail attempts to supply the binary-owned system prompt.
pub fn compose_gate_arguments(
    role: Option<&DispatchRole>,
    output_path: &AbsoluteOutputPath,
    gate_exec_client: Option<&AbsoluteGateExecClientPath>,
    arguments: ArgumentVector,
) -> Result<ArgumentVector, DispatchError> {
    if role.is_none_or(|role| role.as_str() != "falsification-critic") {
        return Ok(arguments);
    }
    if let Some(argument) = arguments.as_slice().iter().find(|argument| {
        argument.as_str() == "--append-system-prompt"
            || argument.starts_with("--append-system-prompt=")
    }) {
        return Err(DispatchError::FalsificationCriticCallerSystemPrompt {
            argument: argument.clone(),
        });
    }
    if let Some(argument) = arguments.as_slice().iter().find(|argument| {
        argument.as_str() == "--allowedTools" || argument.starts_with("--allowedTools=")
    }) {
        return Err(DispatchError::FalsificationCriticCallerAllowedTools {
            argument: argument.clone(),
        });
    }
    let gate_exec_client =
        gate_exec_client.ok_or(DispatchError::FalsificationCriticMissingGateExecClient)?;
    let client = gate_exec_client.as_path().display();
    let mandate = format!(
        "You are the falsification critic. Judge the built artifact by executing probes, never by reviewing prose alone. Submit every stimulus and all of its setup through the harness command between the markers by writing exactly one request JSON object to its standard input: <gate-exec-command>{client} gate exec</gate-exec-command>. The harness alone executes the command and setup, observes the result, and returns its execution_ref and observed_result; an execution the harness did not perform is not admissible evidence. A blocking issue is admissible only for a demonstrated break. For every blocking_issues entry, summarize the exact input in input and the returned observed_result in observation, and put that issue's returned harness reference in execution_ref; do not block on style, naming, design preference, scope, or any other reading-based opinion. For every rejection probe, execute an acceptance probe on the same built artifact whose input differs only in the property under test, and identify that acceptance execution's exact input, returned observed_result, and returned harness reference in the same issue's input and observation. Put the exact replacement you executed in required_change, summarize its input and returned observed_result in replacement_execution, and put that replacement run's returned harness reference in replacement_execution.execution_ref. If you cannot demonstrate a break, emit no blocking issue. Treat every check as a claim: mutate the subject it claims to test and rerun the check; if it stays green, that demonstrated vacuity is blocking, including when the check belongs to this gate rather than to the subject. For every rule the delivered work adds, delete the configuration entry that activates it and rerun the rule's checks; if they stay green, block. Run each mutation, configuration deletion, check, and replacement through the harness, and give each blocking issue its own execution rather than reusing one issue's evidence for another. You may reference harness records but cannot author or edit them. Write exactly one conforming verdict JSON object to the absolute path between the markers below: <output-path>{}</output-path>",
        output_path.as_path().display(),
    );
    let mut composed = vec![
        "--allowedTools".to_owned(),
        format!("Bash({client} gate exec:*)"),
        "Read".to_owned(),
        "Glob".to_owned(),
        "Grep".to_owned(),
        "Write".to_owned(),
        "--append-system-prompt".to_owned(),
        mandate,
    ];
    composed.extend(arguments.as_slice().iter().cloned());
    Ok(ArgumentVector::new(composed))
}

/// The complete explicit child environment.
///
/// A later process adapter must clear the inherited environment before applying these entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChildEnvironment(BTreeMap<String, String>);

impl ChildEnvironment {
    /// Store the complete child environment.
    pub fn new(environment: BTreeMap<String, String>) -> Self {
        Self(environment)
    }

    /// Iterate over environment names and values in deterministic key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Report whether the explicit child environment has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A supported child sandbox capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sandbox {
    /// Permit writes within the configured workspace.
    WorkspaceWrite,
}

impl Sandbox {
    /// Return the tool-facing sandbox spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceWrite => "workspace-write",
        }
    }
}

/// The closed set of stdin sources expressible by a dispatch value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StdinBinding {
    /// Bind child stdin to a null source.
    Null,
    /// Supply approved plan bytes unchanged.
    PlanBytes(Vec<u8>),
}

/// An absolute working directory for the child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteWorkingDirectory(PathBuf);

impl AbsoluteWorkingDirectory {
    /// Parse a lexically absolute working directory without resolving it.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::RelativeWorkingDirectory`] when the supplied path is relative.
    #[instrument(skip(raw))]
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, DispatchError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(DispatchError::RelativeWorkingDirectory { path });
        }
        Ok(Self(path))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// An absolute output-schema path for a structured child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteSchemaPath(PathBuf);

impl AbsoluteSchemaPath {
    /// Parse a lexically absolute schema path without resolving it.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::RelativeSchemaPath`] when the supplied path is relative.
    #[instrument(skip(raw))]
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, DispatchError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(DispatchError::RelativeSchemaPath { path });
        }
        Ok(Self(path))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// An absolute result-output path for a child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteOutputPath(PathBuf);

impl AbsoluteOutputPath {
    /// Parse a lexically absolute output path without resolving it.
    ///
    /// # Errors
    ///
    /// Returns [`DispatchError::RelativeOutputPath`] when the supplied path is relative.
    #[instrument(skip(raw))]
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, DispatchError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(DispatchError::RelativeOutputPath { path });
        }
        Ok(Self(path))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A pure description of one shell-free child invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchEnvelope {
    target: DispatchTarget,
    executable: Executable,
    arguments: ArgumentVector,
    working_directory: AbsoluteWorkingDirectory,
    environment: ChildEnvironment,
    stdin: StdinBinding,
    sandbox: Option<Sandbox>,
    schema_path: Option<AbsoluteSchemaPath>,
    output_path: Option<AbsoluteOutputPath>,
    gate_execution_recorder: Option<GateExecutionRecorderConfig>,
}

impl DispatchEnvelope {
    /// Construct a minimal dispatch with empty arguments and explicit environment.
    pub fn new(
        target: DispatchTarget,
        working_directory: AbsoluteWorkingDirectory,
        stdin: StdinBinding,
    ) -> Self {
        Self {
            target,
            executable: target.executable(),
            arguments: ArgumentVector::default(),
            working_directory,
            environment: ChildEnvironment::default(),
            stdin,
            sandbox: None,
            schema_path: None,
            output_path: None,
            gate_execution_recorder: None,
        }
    }

    /// Return the closed dispatch route.
    pub const fn target(&self) -> DispatchTarget {
        self.target
    }

    /// Set the caller-supplied argument tail.
    pub fn with_arguments(mut self, arguments: ArgumentVector) -> Self {
        self.arguments = arguments;
        self
    }

    /// Set the complete explicit child environment.
    pub fn with_environment(mut self, environment: ChildEnvironment) -> Self {
        self.environment = environment;
        self
    }

    /// Set the optional sandbox capability.
    pub fn with_sandbox(mut self, sandbox: Sandbox) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    /// Set the optional absolute schema path.
    pub fn with_schema_path(mut self, schema_path: AbsoluteSchemaPath) -> Self {
        self.schema_path = Some(schema_path);
        self
    }

    /// Set the optional absolute output path.
    pub fn with_output_path(mut self, output_path: AbsoluteOutputPath) -> Self {
        self.output_path = Some(output_path);
        self
    }

    /// Attach the exact-role recorder and inject its two binary-owned environment entries.
    ///
    /// # Errors
    ///
    /// Returns a collision error when the caller supplied either reserved name.
    pub fn with_gate_execution_recorder(
        mut self,
        recorder: GateExecutionRecorderConfig,
    ) -> Result<Self, DispatchError> {
        if self.environment.0.contains_key("PCE_GATE_EXEC_CLIENT") {
            return Err(DispatchError::FalsificationCriticClientEnvironmentCollision);
        }
        if self.environment.0.contains_key("PCE_GATE_EXEC_SOCKET") {
            return Err(DispatchError::FalsificationCriticSocketEnvironmentCollision);
        }
        self.environment.0.insert(
            "PCE_GATE_EXEC_CLIENT".to_owned(),
            recorder.client().as_path().display().to_string(),
        );
        self.environment.0.insert(
            "PCE_GATE_EXEC_SOCKET".to_owned(),
            recorder.socket().as_path().display().to_string(),
        );
        self.gate_execution_recorder = Some(recorder);
        Ok(self)
    }

    /// Borrow the executable.
    pub fn executable(&self) -> &Executable {
        &self.executable
    }

    /// Borrow the complete child arguments.
    pub fn arguments(&self) -> &ArgumentVector {
        &self.arguments
    }

    /// Borrow the absolute working directory.
    pub fn working_directory(&self) -> &AbsoluteWorkingDirectory {
        &self.working_directory
    }

    /// Borrow the complete explicit child environment.
    pub fn environment(&self) -> &ChildEnvironment {
        &self.environment
    }

    /// Borrow the closed stdin binding.
    pub fn stdin(&self) -> &StdinBinding {
        &self.stdin
    }

    /// Return the optional sandbox capability.
    pub fn sandbox(&self) -> Option<Sandbox> {
        self.sandbox
    }

    /// Borrow the optional absolute schema path.
    pub fn schema_path(&self) -> Option<&AbsoluteSchemaPath> {
        self.schema_path.as_ref()
    }

    /// Borrow the optional absolute output path.
    pub fn output_path(&self) -> Option<&AbsoluteOutputPath> {
        self.output_path.as_ref()
    }

    /// Borrow the optional exact-role gate execution recorder configuration.
    pub fn gate_execution_recorder(&self) -> Option<&GateExecutionRecorderConfig> {
        self.gate_execution_recorder.as_ref()
    }
}

/// Whether the permissive-profile probe proved that this process may apply Seatbelt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatbeltCapability {
    /// The probe applied Seatbelt and `/usr/bin/true` completed successfully.
    Available,
    /// The probe completed with a nonzero status, including a nested `sandbox_apply` denial.
    Unavailable {
        /// The exact status returned by `sandbox-exec`.
        status: ObservedExitStatus,
    },
}

/// Construct the permissive-profile probe used before applying a gate's Seatbelt profile.
///
/// # Errors
///
/// Returns [`DispatchError`] if a fixed probe executable cannot be represented as an executable.
pub fn seatbelt_capability_probe(
    working_directory: AbsoluteWorkingDirectory,
) -> Result<DispatchEnvelope, DispatchError> {
    Ok(DispatchEnvelope::new(
        DispatchTarget::Seatbelt,
        working_directory,
        StdinBinding::Null,
    )
    .with_arguments(ArgumentVector::new(vec![
        "-p".to_owned(),
        PERMISSIVE_SEATBELT_PROFILE.to_owned(),
        "--".to_owned(),
        "/usr/bin/true".to_owned(),
    ]))
    .with_environment(ChildEnvironment::new(BTreeMap::new())))
}

/// Classify the observed status from [`seatbelt_capability_probe`].
pub const fn classify_seatbelt_capability(status: ObservedExitStatus) -> SeatbeltCapability {
    if status.code() == 0 {
        SeatbeltCapability::Available
    } else {
        SeatbeltCapability::Unavailable { status }
    }
}

/// A dispatch value failed pure parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchError {
    /// The exact falsification critic caller attempted to replace the binary-owned system prompt.
    #[error("falsification-critic caller arguments must not contain `--append-system-prompt`")]
    FalsificationCriticCallerSystemPrompt { argument: String },
    /// The exact falsification critic caller attempted to replace the binary-owned tool grant.
    #[error("falsification-critic caller arguments must not contain `--allowedTools`")]
    FalsificationCriticCallerAllowedTools { argument: String },
    /// Exact-role composition omitted the required absolute helper executable.
    #[error("falsification-critic requires an absolute pce gate-exec client path")]
    FalsificationCriticMissingGateExecClient,
    /// The caller supplied the binary-owned helper executable environment name.
    #[error("falsification-critic environment must not supply binary-owned `PCE_GATE_EXEC_CLIENT`")]
    FalsificationCriticClientEnvironmentCollision,
    /// The caller supplied the binary-owned recorder socket environment name.
    #[error("falsification-critic environment must not supply binary-owned `PCE_GATE_EXEC_SOCKET`")]
    FalsificationCriticSocketEnvironmentCollision,
    /// The executable parser received a zero-byte program name.
    #[error("executable must not be empty; rejected {executable:?}")]
    EmptyExecutable { executable: String },
    /// The working-directory parser received a relative path.
    #[error("working directory must be absolute; rejected {path:?}")]
    RelativeWorkingDirectory { path: PathBuf },
    /// The schema-path parser received a relative path.
    #[error("schema path must be absolute; rejected {path:?}")]
    RelativeSchemaPath { path: PathBuf },
    /// The output-path parser received a relative path.
    #[error("output path must be absolute; rejected {path:?}")]
    RelativeOutputPath { path: PathBuf },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use super::{
        AbsoluteOutputPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory, ActReversibility,
        ArgumentVector, ChildEnvironment, ClaudeResultEnvelope, CodexTerminalObservation,
        CodexTerminalUsage, DispatchEnvelope, DispatchError, DispatchTarget, Executable,
        PlanningFrameError, Sandbox, SeatbeltCapability, StdinBinding, classify_claude_result,
        classify_codex_terminal_usage, classify_seatbelt_capability, compose_gate_arguments,
        compose_planning_role_frame, dispatch_completion_payload, parse_claude_result,
        seatbelt_capability_probe,
    };
    use crate::contract_measurement::ObservedExitStatus;
    use crate::event_log::{
        ArtifactOutcome, DispatchDuration, DispatchExitStatus, DispatchRole, DispatchTokenUsage,
        ExitCode, Sequence, SignalNumber, UsageAbsenceReason,
    };
    use crate::gate_execution::AbsoluteGateExecClientPath;

    const REPEATABLE_FIXTURE: &str = "Plan the step.\n\n## Binary-owned reversibility obligation\n\nThe step's act is repeatable. The plan must retain an explicit not-touched scope fence and exact expected values for every assertion. The plan must not contain a pre-derived argument that the design is correct.";
    const IRREVERSIBLE_FIXTURE: &str = "Plan the step.\n\n## Binary-owned reversibility obligation\n\nThe step's act cannot be repeated. The plan must retain the existing front-loaded pre-proof of correctness, an explicit not-touched scope fence, and exact expected values for every assertion.";

    #[test]
    fn planning_role_frame_complete_role_reversibility_matrix_is_byte_exact() {
        for role in ["step-plan-writer", "step-plan-critic"] {
            for (act, expected) in [
                (ActReversibility::Repeatable, REPEATABLE_FIXTURE),
                (ActReversibility::Irreversible, IRREVERSIBLE_FIXTURE),
            ] {
                let arguments = ArgumentVector::new(vec![
                    "--append-system-prompt".to_owned(),
                    "/tmp/review.json".to_owned(),
                    "Plan the step.".to_owned(),
                ]);
                let composed =
                    compose_planning_role_frame(&DispatchRole::new(role), act, arguments)
                        .expect("accepted planning frame");
                assert_eq!(
                    composed.as_slice(),
                    ["--append-system-prompt", "/tmp/review.json", expected]
                );
                assert!(!composed.as_slice()[2].ends_with('\n'));
            }
        }
    }

    #[test]
    fn planning_role_frame_rejects_unsupported_roles_and_prompt_carriers() {
        assert_eq!(
            compose_planning_role_frame(
                &DispatchRole::new("step-executor"),
                ActReversibility::Repeatable,
                ArgumentVector::new(vec!["Plan the step.".to_owned()]),
            ),
            Err(PlanningFrameError::UnsupportedRole {
                role: "step-executor".to_owned()
            })
        );
        for arguments in [
            ArgumentVector::default(),
            ArgumentVector::new(vec![String::new()]),
        ] {
            let error = compose_planning_role_frame(
                &DispatchRole::new("step-plan-writer"),
                ActReversibility::Repeatable,
                arguments,
            )
            .expect_err("prompt carrier must be non-empty");
            assert_eq!(
                error.to_string(),
                "planning role `step-plan-writer` requires a non-empty final caller argument to carry its binary-owned frame"
            );
        }
        assert_eq!(
            ActReversibility::parse("destructive")
                .expect_err("unsupported planning act")
                .to_string(),
            "unsupported planning act `destructive`; expected `repeatable` or `irreversible`"
        );
    }

    fn exited(code: u64) -> DispatchExitStatus {
        DispatchExitStatus::Exited {
            code: ExitCode::new(code),
        }
    }

    #[test]
    fn dispatch_completion_payload_preserves_artifact_outcome() {
        let payload = dispatch_completion_payload(
            Sequence::parse(7).expect("positive sequence"),
            DispatchDuration::new(12),
            DispatchTokenUsage::Absent {
                reason: UsageAbsenceReason::NoTerminalTurn,
            },
            exited(0),
            ArtifactOutcome::SchemaViolating,
        );
        assert_eq!(payload.artifact_outcome, ArtifactOutcome::SchemaViolating);
    }

    #[test]
    fn codex_terminal_partition_and_precedence_are_exhaustive() {
        let usage = CodexTerminalUsage {
            input_tokens: 101,
            cached_input_tokens: 23,
            output_tokens: 17,
            reasoning_output_tokens: 5,
        };
        let cases = [
            (
                vec![CodexTerminalObservation::MalformedLine],
                exited(5),
                Err(UsageAbsenceReason::MalformedTerminalData),
            ),
            (
                vec![CodexTerminalObservation::TurnCompleted(None)],
                exited(0),
                Err(UsageAbsenceReason::MalformedTerminalData),
            ),
            (
                vec![CodexTerminalObservation::TurnFailed {
                    usage_present: true,
                }],
                exited(0),
                Err(UsageAbsenceReason::MalformedTerminalData),
            ),
            (
                vec![
                    CodexTerminalObservation::TurnFailed {
                        usage_present: false,
                    },
                    CodexTerminalObservation::TurnFailed {
                        usage_present: false,
                    },
                ],
                exited(41),
                Err(UsageAbsenceReason::DuplicateTerminalData),
            ),
            (
                vec![
                    CodexTerminalObservation::TurnCompleted(Some(usage)),
                    CodexTerminalObservation::TurnFailed {
                        usage_present: false,
                    },
                ],
                exited(43),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![CodexTerminalObservation::TurnCompleted(Some(usage))],
                exited(5),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![CodexTerminalObservation::TurnFailed {
                    usage_present: false,
                }],
                exited(0),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![CodexTerminalObservation::NonTerminal],
                exited(0),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![CodexTerminalObservation::TurnFailed {
                    usage_present: false,
                }],
                exited(41),
                Ok(DispatchTokenUsage::Absent {
                    reason: UsageAbsenceReason::TurnFailed,
                }),
            ),
            (
                vec![CodexTerminalObservation::NonTerminal],
                exited(42),
                Ok(DispatchTokenUsage::Absent {
                    reason: UsageAbsenceReason::NoTerminalTurn,
                }),
            ),
            (
                vec![],
                DispatchExitStatus::Signaled {
                    signal: SignalNumber::new(15),
                },
                Ok(DispatchTokenUsage::Absent {
                    reason: UsageAbsenceReason::NoTerminalTurn,
                }),
            ),
        ];
        for (observations, status, expected) in cases {
            assert_eq!(
                classify_codex_terminal_usage(&observations, status),
                expected
            );
        }
        let measured = classify_codex_terminal_usage(
            &[
                CodexTerminalObservation::NonTerminal,
                CodexTerminalObservation::TurnCompleted(Some(usage)),
            ],
            exited(0),
        );
        assert_eq!(
            measured,
            Ok(DispatchTokenUsage::Measured {
                input_tokens: crate::event_log::InputTokens::new(101),
                cached_input_tokens: crate::event_log::CachedInputTokens::new(23),
                output_tokens: crate::event_log::OutputTokens::new(17),
                reasoning_output_tokens: crate::event_log::ReasoningOutputTokens::new(5),
            })
        );
    }

    fn assert_claude_reason(
        source: &[u8],
        status: DispatchExitStatus,
        expected: UsageAbsenceReason,
    ) {
        let envelope = parse_claude_result(source);
        assert_eq!(classify_claude_result(&envelope, status), Err(expected));
    }

    const CLAUDE_ERROR: &[u8] =
        br#"{"type":"result","subtype":"error","is_error":true,"result":"failed"}"#;
    const CLAUDE_SUCCESS: &[u8] = br#"{"type":"result","subtype":"success","is_error":false,"result":"OK","usage":{"input_tokens":2,"output_tokens":4,"cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}}"#;
    const CLAUDE_MISSING: &[u8] =
        br#"{"type":"result","subtype":"success","is_error":false,"result":"OK"}"#;

    #[test]
    fn claude_malformed_result_is_distinct() {
        assert_claude_reason(
            b"not-json",
            exited(0),
            UsageAbsenceReason::ClaudeMalformedResult,
        );
    }

    #[test]
    fn claude_missing_usage_is_distinct() {
        assert_claude_reason(
            CLAUDE_MISSING,
            exited(0),
            UsageAbsenceReason::ClaudeMissingUsage,
        );
    }

    #[test]
    fn claude_error_envelope_is_distinct() {
        for status in [
            exited(1),
            DispatchExitStatus::Signaled {
                signal: SignalNumber::new(15),
            },
        ] {
            assert_claude_reason(
                CLAUDE_ERROR,
                status,
                UsageAbsenceReason::ClaudeErrorEnvelope,
            );
        }
    }

    #[test]
    fn claude_exit_envelope_contradiction_is_distinct() {
        for (source, status) in [
            (CLAUDE_SUCCESS, exited(42)),
            (CLAUDE_ERROR, exited(0)),
            (
                CLAUDE_SUCCESS,
                DispatchExitStatus::Signaled {
                    signal: SignalNumber::new(15),
                },
            ),
            (CLAUDE_MISSING, exited(42)),
        ] {
            assert_claude_reason(
                source,
                status,
                UsageAbsenceReason::ClaudeExitEnvelopeContradiction,
            );
        }
    }

    #[test]
    fn claude_classifier_completes_envelope_exit_product() {
        for status in [
            exited(42),
            DispatchExitStatus::Signaled {
                signal: SignalNumber::new(15),
            },
        ] {
            assert_claude_reason(
                b"not-json",
                status,
                UsageAbsenceReason::ClaudeMalformedResult,
            );
        }
        assert_claude_reason(
            CLAUDE_MISSING,
            DispatchExitStatus::Signaled {
                signal: SignalNumber::new(15),
            },
            UsageAbsenceReason::ClaudeExitEnvelopeContradiction,
        );
    }

    #[test]
    fn claude_parser_partitions_envelope_shapes() {
        enum Expected {
            Malformed,
            Missing,
            Error,
        }
        let cases: &[(&[u8], Expected)] = &[
            (b"5", Expected::Malformed),
            (br#""x""#, Expected::Malformed),
            (b"[]", Expected::Malformed),
            (b"{}", Expected::Malformed),
            (br#"{"is_error":"false"}"#, Expected::Malformed),
            (br#"{"is_error":false,"usage":5}"#, Expected::Malformed),
            (br#"{"is_error":false,"usage":{"input_tokens":"2","output_tokens":4,"cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}}"#, Expected::Malformed),
            (br#"{"is_error":false,"usage":{"input_tokens":2,"output_tokens":"4","cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}}"#, Expected::Malformed),
            (br#"{"is_error":false,"usage":{"input_tokens":2,"output_tokens":4,"cache_creation_input_tokens":"9572","cache_read_input_tokens":15410}}"#, Expected::Malformed),
            (br#"{"is_error":false,"usage":{"input_tokens":2,"output_tokens":4,"cache_creation_input_tokens":9572,"cache_read_input_tokens":"15410"}}"#, Expected::Malformed),
            (br#"{"is_error":false,"usage":{"input_tokens":2,"output_tokens":"4","cache_creation_input_tokens":9572}}"#, Expected::Malformed),
            (br#"{"is_error":false,"usage":{"output_tokens":4,"cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}}"#, Expected::Missing),
            (br#"{"is_error":false,"usage":{"input_tokens":2,"cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}}"#, Expected::Missing),
            (br#"{"is_error":false,"usage":{"input_tokens":2,"output_tokens":4,"cache_read_input_tokens":15410}}"#, Expected::Missing),
            (br#"{"is_error":false,"usage":{"input_tokens":2,"output_tokens":4,"cache_creation_input_tokens":9572}}"#, Expected::Missing),
            (br#"{"is_error":true,"usage":5}"#, Expected::Error),
            (br#"{"is_error":true,"usage":{"input_tokens":2,"output_tokens":4,"cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}}"#, Expected::Error),
        ];
        for (source, expected) in cases {
            let envelope = parse_claude_result(source);
            match expected {
                Expected::Malformed => assert!(matches!(envelope, ClaudeResultEnvelope::Malformed)),
                Expected::Missing => assert!(matches!(
                    envelope,
                    ClaudeResultEnvelope::Success { usage: None }
                )),
                Expected::Error => assert!(matches!(envelope, ClaudeResultEnvelope::Error)),
            }
        }
        assert_claude_reason(
            cases[14].0,
            exited(0),
            UsageAbsenceReason::ClaudeMissingUsage,
        );
        assert_claude_reason(
            cases[16].0,
            exited(1),
            UsageAbsenceReason::ClaudeErrorEnvelope,
        );
    }

    #[test]
    fn claude_measured_results_preserve_and_compare_all_counters() {
        let sources: [&[u8]; 2] = [
            br#"{"type":"result","subtype":"success","is_error":false,"result":"OK","duration_ms":9307,"total_cost_usd":0.104116,"usage":{"input_tokens":2,"output_tokens":4,"cache_creation_input_tokens":9572,"cache_read_input_tokens":15410}}"#,
            br#"{"type":"result","subtype":"success","is_error":false,"result":"OK","duration_ms":9307,"total_cost_usd":999.0,"modelUsage":{"input_tokens":999999},"usage":{"input_tokens":13,"output_tokens":21,"cache_creation_input_tokens":9606,"cache_read_input_tokens":15465}}"#,
        ];
        let first = classify_claude_result(&parse_claude_result(sources[0]), exited(0));
        let second = classify_claude_result(&parse_claude_result(sources[1]), exited(0));
        assert_eq!(
            first,
            Ok(DispatchTokenUsage::ClaudeMeasured {
                input_tokens: crate::event_log::InputTokens::new(2),
                output_tokens: crate::event_log::OutputTokens::new(4),
                cache_creation_input_tokens: crate::event_log::CacheCreationInputTokens::new(9572),
                cache_read_input_tokens: crate::event_log::CacheReadInputTokens::new(15410),
            })
        );
        assert_eq!(
            second,
            Ok(DispatchTokenUsage::ClaudeMeasured {
                input_tokens: crate::event_log::InputTokens::new(13),
                output_tokens: crate::event_log::OutputTokens::new(21),
                cache_creation_input_tokens: crate::event_log::CacheCreationInputTokens::new(9606),
                cache_read_input_tokens: crate::event_log::CacheReadInputTokens::new(15465),
            })
        );
        let (
            Ok(DispatchTokenUsage::ClaudeMeasured {
                input_tokens: first_input,
                output_tokens: first_output,
                cache_creation_input_tokens: first_creation,
                cache_read_input_tokens: first_read,
            }),
            Ok(DispatchTokenUsage::ClaudeMeasured {
                input_tokens: second_input,
                output_tokens: second_output,
                cache_creation_input_tokens: second_creation,
                cache_read_input_tokens: second_read,
            }),
        ) = (first, second)
        else {
            panic!("both results must be measured");
        };
        assert_eq!(second_input.get() - first_input.get(), 11);
        assert_eq!(second_output.get() - first_output.get(), 17);
        assert_eq!(second_creation.get() - first_creation.get(), 34);
        assert_eq!(second_read.get() - first_read.get(), 55);
    }

    #[test]
    fn constructs_minimal_dispatch_envelope() -> Result<(), DispatchError> {
        let envelope = DispatchEnvelope::new(
            DispatchTarget::Codex,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::Null,
        );

        assert_eq!(envelope.executable().as_str(), "codex");
        assert_eq!(envelope.target(), DispatchTarget::Codex);
        assert_eq!(
            envelope.working_directory().as_path(),
            Path::new("/workspace/project")
        );
        assert_eq!(envelope.stdin(), &StdinBinding::Null);
        assert!(envelope.arguments().is_empty());
        assert!(envelope.environment().is_empty());
        Ok(())
    }

    #[test]
    fn rejects_empty_executable() -> Result<(), DispatchError> {
        assert_eq!(
            Executable::parse(""),
            Err(DispatchError::EmptyExecutable {
                executable: String::new()
            })
        );
        assert_eq!(Executable::parse("codex")?.as_str(), "codex");
        Ok(())
    }

    #[test]
    fn absolute_path_types_accept_absolute_paths() -> Result<(), DispatchError> {
        let working_directory = AbsoluteWorkingDirectory::parse("/workspace/./project")?;
        let schema_path = AbsoluteSchemaPath::parse("/workspace/schema.json")?;
        let output_path = AbsoluteOutputPath::parse("/workspace/output.json")?;

        assert_eq!(
            working_directory.as_path(),
            Path::new("/workspace/./project")
        );
        assert_eq!(schema_path.as_path(), Path::new("/workspace/schema.json"));
        assert_eq!(output_path.as_path(), Path::new("/workspace/output.json"));
        Ok(())
    }

    #[test]
    fn absolute_path_types_reject_relative_paths() {
        assert_eq!(
            AbsoluteWorkingDirectory::parse("relative/workspace"),
            Err(DispatchError::RelativeWorkingDirectory {
                path: PathBuf::from("relative/workspace")
            })
        );
        assert_eq!(
            AbsoluteSchemaPath::parse("relative/schema.json"),
            Err(DispatchError::RelativeSchemaPath {
                path: PathBuf::from("relative/schema.json")
            })
        );
        assert_eq!(
            AbsoluteOutputPath::parse("relative/output.json"),
            Err(DispatchError::RelativeOutputPath {
                path: PathBuf::from("relative/output.json")
            })
        );
    }

    #[test]
    fn constructs_structured_dispatch_value() -> Result<(), DispatchError> {
        let arguments = vec![
            "exec".to_owned(),
            "--caller-option".to_owned(),
            "POSITIONAL_PROMPT_PLACEHOLDER".to_owned(),
            String::new(),
        ];
        let mut environment = BTreeMap::new();
        environment.insert("LANG".to_owned(), "C.UTF-8".to_owned());
        environment.insert("PCE_MODE".to_owned(), "dispatch".to_owned());
        let plan_bytes = vec![0, 1, 2, 255];

        let envelope = DispatchEnvelope::new(
            DispatchTarget::Codex,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::PlanBytes(plan_bytes.clone()),
        )
        .with_arguments(ArgumentVector::new(arguments.clone()))
        .with_environment(ChildEnvironment::new(environment.clone()));

        assert_eq!(envelope.executable().as_str(), "codex");
        assert_eq!(envelope.target(), DispatchTarget::Codex);
        assert_eq!(envelope.arguments().as_slice(), arguments.as_slice());
        assert_eq!(
            envelope.environment().iter().collect::<Vec<_>>(),
            environment
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            envelope.stdin(),
            &StdinBinding::PlanBytes(plan_bytes.clone())
        );
        assert_eq!(Sandbox::WorkspaceWrite.as_str(), "workspace-write");
        Ok(())
    }

    #[test]
    fn constructs_unstructured_reusable_dispatch_value() -> Result<(), DispatchError> {
        let arguments = ArgumentVector::new(vec!["positional prompt".to_owned()]);
        let envelope = DispatchEnvelope::new(
            DispatchTarget::Gate,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::Null,
        )
        .with_arguments(arguments);

        assert_eq!(envelope.target(), DispatchTarget::Gate);
        assert_eq!(envelope.executable().as_str(), "claude");

        assert_eq!(
            envelope.arguments().as_slice(),
            &["positional prompt".to_owned()]
        );
        assert_eq!(envelope.stdin(), &StdinBinding::Null);
        Ok(())
    }

    #[test]
    fn composes_only_the_exact_falsification_critic_frame() -> Result<(), Box<dyn std::error::Error>>
    {
        let output = AbsoluteOutputPath::parse("/workspace/path with spaces/verdict.json")?;
        let client = AbsoluteGateExecClientPath::parse("/workspace/bin/pce")?;
        let caller = ArgumentVector::new(vec!["probe".to_owned(), "--flag".to_owned()]);
        for role in [
            None,
            Some(DispatchRole::new("pr-reviewer")),
            Some(DispatchRole::new("step-plan-critic")),
            Some(DispatchRole::new("Falsification-Critic")),
            Some(DispatchRole::new("falsification_critic")),
        ] {
            assert_eq!(
                compose_gate_arguments(role.as_ref(), &output, None, caller.clone())?,
                caller
            );
        }

        let role = DispatchRole::new("falsification-critic");
        let composed = compose_gate_arguments(Some(&role), &output, Some(&client), caller.clone())?;
        assert_eq!(
            &composed.as_slice()[..7],
            [
                "--allowedTools",
                "Bash(/workspace/bin/pce gate exec:*)",
                "Read",
                "Glob",
                "Grep",
                "Write",
                "--append-system-prompt"
            ]
        );
        assert_eq!(&composed.as_slice()[8..], caller.as_slice());
        let mandate = &composed.as_slice()[7];
        assert_eq!(mandate.matches("<output-path>").count(), 1);
        assert_eq!(mandate.matches("</output-path>").count(), 1);
        assert!(
            mandate.contains("<output-path>/workspace/path with spaces/verdict.json</output-path>")
        );
        assert!(!mandate.contains("schema"));
        assert_eq!(
            mandate
                .matches("For every rejection probe, execute an acceptance probe on the same built artifact whose input differs only in the property under test, and identify that acceptance execution's exact input, returned observed_result, and returned harness reference in the same issue's input and observation.")
                .count(),
            1
        );

        for forbidden in [
            "--append-system-prompt",
            "--append-system-prompt=caller-value",
        ] {
            let error = compose_gate_arguments(
                Some(&role),
                &output,
                Some(&client),
                ArgumentVector::new(vec![forbidden.to_owned()]),
            )
            .expect_err("caller override must fail");
            assert_eq!(
                error.to_string(),
                "falsification-critic caller arguments must not contain `--append-system-prompt`"
            );
        }
        Ok(())
    }

    #[test]
    fn permissive_probe_success_reports_seatbelt_available() -> Result<(), DispatchError> {
        let envelope =
            seatbelt_capability_probe(AbsoluteWorkingDirectory::parse("/workspace/project")?)?;

        assert_eq!(envelope.executable().as_str(), "/usr/bin/sandbox-exec");
        assert_eq!(
            envelope.arguments().as_slice(),
            ["-p", "(version 1)(allow default)", "--", "/usr/bin/true"]
        );
        assert_eq!(
            classify_seatbelt_capability(ObservedExitStatus::from_code(0)),
            SeatbeltCapability::Available
        );
        assert_eq!(
            classify_seatbelt_capability(ObservedExitStatus::from_code(71)),
            SeatbeltCapability::Unavailable {
                status: ObservedExitStatus::from_code(71)
            }
        );
        Ok(())
    }
}
