//! dispatch projection : DispatchEnvelope × DispatchLogging × EventLogTail → JSON; terminal_usage : JSONL observations × ExitStatus → CodexTokenUsage   (pure, deterministic)
//! This provisional module describes child invocations; the binary adapter performs all I/O and process work and will test the shape against the real tool surface in m2-s2.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;

use crate::event_log::{
    ArtifactOutcome, CachedInputTokens, CodexTokenUsage, DispatchCompletionPayload,
    DispatchDuration, DispatchExitStatus, DispatchPayload, DispatchRef, DispatchRole, EventLogTail,
    EventLogTailError, EventTimestamp, Evidence, InputTokens, NodeId, OutputTokens,
    ReasoningOutputTokens, Sequence, UsageAbsenceReason, WriteKind, successor_sequence,
};

/// Typed log metadata carried beside, rather than inside, a child envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchLogging {
    pub node: NodeId,
    pub role: DispatchRole,
    pub dispatch_ref: DispatchRef,
    pub evidence: Evidence,
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
    usage: CodexTokenUsage,
    exit_status: DispatchExitStatus,
) -> DispatchCompletionPayload {
    DispatchCompletionPayload {
        issuance_sequence,
        duration_ms,
        usage,
        exit_status,
        artifact_outcome: ArtifactOutcome::NotValidated,
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
    executable: String,
    argv: Vec<String>,
    cwd: String,
    environment: BTreeMap<String, String>,
    stdin: DispatchInvocationStdin,
    schema_path: Option<String>,
    output_path: Option<String>,
}

impl DispatchInvocation {
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
    let mut argv = vec![
        "exec".to_owned(),
        "--json".to_owned(),
        "-C".to_owned(),
        cwd.clone(),
    ];
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
    /// use pce_core::{AbsoluteWorkingDirectory, DispatchEnvelope, DispatchLogging, DispatchProjectionInput, DispatchRef, DispatchRole, EventLogTail, Evidence, Executable, NodeId, StdinBinding};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let envelope = DispatchEnvelope::new(Executable::parse("codex")?, AbsoluteWorkingDirectory::parse("/tmp")?, StdinBinding::Null);
    /// let logging = DispatchLogging { node: NodeId::parse("m3-s2")?, role: DispatchRole::new("step-executor"), dispatch_ref: DispatchRef::new("ref"), evidence: Evidence::parse("fixture")? };
    /// let tail = EventLogTail::Empty;
    /// let _input = DispatchProjectionInput::new(&envelope, &logging, &tail);
    /// # Ok(()) }
    /// ```
    /// ```compile_fail
    /// use pce_core::{AbsoluteWorkingDirectory, DispatchEnvelope, DispatchLogging, DispatchProjectionInput, DispatchRef, DispatchRole, EventLogTail, Evidence, Executable, NodeId, StdinBinding};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let envelope = DispatchEnvelope::new(Executable::parse("codex")?, AbsoluteWorkingDirectory::parse("/tmp")?, StdinBinding::Null);
    /// let logging = DispatchLogging { node: NodeId::parse("m3-s2")?, role: DispatchRole::new("step-executor"), dispatch_ref: DispatchRef::new("ref"), evidence: Evidence::parse("fixture")? };
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
    usage: Deferred<CodexTokenUsage>,
    exit_status: Deferred<DispatchExitStatus>,
    artifact_outcome: ArtifactOutcome,
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
                artifact_outcome: ArtifactOutcome::NotValidated,
            },
        },
    };
    serde_json::to_string(&projection)
        .map_err(|source| DispatchProjectionError::SerializationFailed { source })
}

/// One adapter-observed JSONL fact relevant to terminal classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalObservation {
    /// The physical line did not parse as JSON.
    MalformedLine,
    /// A parseable JSON value other than an object, or an object with no known terminal `type`,
    /// is non-terminal rather than malformed.
    NonTerminal,
    /// A completed turn with either exact counters or malformed usage.
    TurnCompleted(Option<TerminalUsage>),
    /// A failed turn, recording whether a forbidden usage key was present.
    TurnFailed { usage_present: bool },
}

/// The four exact counters from a well-formed completed turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalUsage {
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
pub fn classify_terminal_usage(
    observations: &[TerminalObservation],
    exit_status: DispatchExitStatus,
) -> Result<CodexTokenUsage, UsageAbsenceReason> {
    let completed = observations
        .iter()
        .filter(|item| matches!(item, TerminalObservation::TurnCompleted(_)))
        .collect::<Vec<_>>();
    let failed = observations
        .iter()
        .filter(|item| matches!(item, TerminalObservation::TurnFailed { .. }))
        .collect::<Vec<_>>();
    // Precedence is specification: malformed, duplicate, contradictory, then valid outcomes.
    if observations
        .iter()
        .any(|item| matches!(item, TerminalObservation::MalformedLine))
        || completed
            .iter()
            .any(|item| matches!(item, TerminalObservation::TurnCompleted(None)))
        || failed.iter().any(|item| {
            matches!(
                item,
                TerminalObservation::TurnFailed {
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
    if let Some(TerminalObservation::TurnCompleted(Some(usage))) = completed.first().copied() {
        return Ok(CodexTokenUsage::Measured {
            input_tokens: InputTokens::new(usage.input_tokens),
            cached_input_tokens: CachedInputTokens::new(usage.cached_input_tokens),
            output_tokens: OutputTokens::new(usage.output_tokens),
            reasoning_output_tokens: ReasoningOutputTokens::new(usage.reasoning_output_tokens),
        });
    }
    if !failed.is_empty() {
        return Ok(CodexTokenUsage::Absent {
            reason: UsageAbsenceReason::TurnFailed,
        });
    }
    Ok(CodexTokenUsage::Absent {
        reason: UsageAbsenceReason::NoTerminalTurn,
    })
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

/// The caller-supplied argument tail in caller order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArgumentVector(Vec<String>);

impl ArgumentVector {
    /// Store caller-supplied arguments without filtering or normalization.
    pub fn new(arguments: Vec<String>) -> Self {
        Self(arguments)
    }

    /// Borrow the ordered caller-supplied arguments.
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Report whether the caller supplied no arguments.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
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
    executable: Executable,
    arguments: ArgumentVector,
    working_directory: AbsoluteWorkingDirectory,
    environment: ChildEnvironment,
    stdin: StdinBinding,
    sandbox: Option<Sandbox>,
    schema_path: Option<AbsoluteSchemaPath>,
    output_path: Option<AbsoluteOutputPath>,
}

impl DispatchEnvelope {
    /// Construct a minimal dispatch with empty arguments and explicit environment.
    pub fn new(
        executable: Executable,
        working_directory: AbsoluteWorkingDirectory,
        stdin: StdinBinding,
    ) -> Self {
        Self {
            executable,
            arguments: ArgumentVector::default(),
            working_directory,
            environment: ChildEnvironment::default(),
            stdin,
            sandbox: None,
            schema_path: None,
            output_path: None,
        }
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

    /// Borrow the executable.
    pub fn executable(&self) -> &Executable {
        &self.executable
    }

    /// Borrow the caller-supplied arguments.
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
}

/// A dispatch value failed pure parsing.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchError {
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
        AbsoluteOutputPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory, ArgumentVector,
        ChildEnvironment, DispatchEnvelope, DispatchError, Executable, Sandbox, StdinBinding,
        TerminalObservation, TerminalUsage, classify_terminal_usage,
    };
    use crate::event_log::{
        CodexTokenUsage, DispatchExitStatus, ExitCode, SignalNumber, UsageAbsenceReason,
    };

    fn exited(code: u64) -> DispatchExitStatus {
        DispatchExitStatus::Exited {
            code: ExitCode::new(code),
        }
    }

    #[test]
    fn terminal_partition_and_precedence_are_exhaustive() {
        let usage = TerminalUsage {
            input_tokens: 101,
            cached_input_tokens: 23,
            output_tokens: 17,
            reasoning_output_tokens: 5,
        };
        let cases = [
            (
                vec![TerminalObservation::MalformedLine],
                exited(5),
                Err(UsageAbsenceReason::MalformedTerminalData),
            ),
            (
                vec![TerminalObservation::TurnCompleted(None)],
                exited(0),
                Err(UsageAbsenceReason::MalformedTerminalData),
            ),
            (
                vec![TerminalObservation::TurnFailed {
                    usage_present: true,
                }],
                exited(0),
                Err(UsageAbsenceReason::MalformedTerminalData),
            ),
            (
                vec![
                    TerminalObservation::TurnFailed {
                        usage_present: false,
                    },
                    TerminalObservation::TurnFailed {
                        usage_present: false,
                    },
                ],
                exited(41),
                Err(UsageAbsenceReason::DuplicateTerminalData),
            ),
            (
                vec![
                    TerminalObservation::TurnCompleted(Some(usage)),
                    TerminalObservation::TurnFailed {
                        usage_present: false,
                    },
                ],
                exited(43),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![TerminalObservation::TurnCompleted(Some(usage))],
                exited(5),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![TerminalObservation::TurnFailed {
                    usage_present: false,
                }],
                exited(0),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![TerminalObservation::NonTerminal],
                exited(0),
                Err(UsageAbsenceReason::ContradictoryTerminalData),
            ),
            (
                vec![TerminalObservation::TurnFailed {
                    usage_present: false,
                }],
                exited(41),
                Ok(CodexTokenUsage::Absent {
                    reason: UsageAbsenceReason::TurnFailed,
                }),
            ),
            (
                vec![TerminalObservation::NonTerminal],
                exited(42),
                Ok(CodexTokenUsage::Absent {
                    reason: UsageAbsenceReason::NoTerminalTurn,
                }),
            ),
            (
                vec![],
                DispatchExitStatus::Signaled {
                    signal: SignalNumber::new(15),
                },
                Ok(CodexTokenUsage::Absent {
                    reason: UsageAbsenceReason::NoTerminalTurn,
                }),
            ),
        ];
        for (observations, status, expected) in cases {
            assert_eq!(classify_terminal_usage(&observations, status), expected);
        }
        let measured = classify_terminal_usage(
            &[
                TerminalObservation::NonTerminal,
                TerminalObservation::TurnCompleted(Some(usage)),
            ],
            exited(0),
        );
        assert_eq!(
            measured,
            Ok(CodexTokenUsage::Measured {
                input_tokens: crate::event_log::InputTokens::new(101),
                cached_input_tokens: crate::event_log::CachedInputTokens::new(23),
                output_tokens: crate::event_log::OutputTokens::new(17),
                reasoning_output_tokens: crate::event_log::ReasoningOutputTokens::new(5),
            })
        );
    }

    #[test]
    fn constructs_minimal_dispatch_envelope() -> Result<(), DispatchError> {
        let envelope = DispatchEnvelope::new(
            Executable::parse("codex")?,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::Null,
        );

        assert_eq!(envelope.executable().as_str(), "codex");
        assert_eq!(
            envelope.working_directory().as_path(),
            Path::new("/workspace/project")
        );
        assert_eq!(envelope.stdin(), &StdinBinding::Null);
        assert!(envelope.arguments().is_empty());
        assert!(envelope.environment().is_empty());
        assert_eq!(envelope.sandbox(), None);
        assert_eq!(envelope.schema_path(), None);
        assert_eq!(envelope.output_path(), None);
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
            "--caller-option".to_owned(),
            "POSITIONAL_PROMPT_PLACEHOLDER".to_owned(),
            String::new(),
        ];
        let mut environment = BTreeMap::new();
        environment.insert("LANG".to_owned(), "C.UTF-8".to_owned());
        environment.insert("PCE_MODE".to_owned(), "dispatch".to_owned());
        let plan_bytes = vec![0, 1, 2, 255];

        let envelope = DispatchEnvelope::new(
            Executable::parse("codex")?,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::PlanBytes(plan_bytes.clone()),
        )
        .with_arguments(ArgumentVector::new(arguments.clone()))
        .with_environment(ChildEnvironment::new(environment.clone()))
        .with_sandbox(Sandbox::WorkspaceWrite)
        .with_schema_path(AbsoluteSchemaPath::parse("/workspace/schema.json")?)
        .with_output_path(AbsoluteOutputPath::parse("/workspace/output.json")?);

        assert_eq!(envelope.executable().as_str(), "codex");
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
        assert_eq!(envelope.sandbox(), Some(Sandbox::WorkspaceWrite));
        assert_eq!(Sandbox::WorkspaceWrite.as_str(), "workspace-write");
        assert_eq!(
            envelope.schema_path().map(AbsoluteSchemaPath::as_path),
            Some(Path::new("/workspace/schema.json"))
        );
        assert_eq!(
            envelope.output_path().map(AbsoluteOutputPath::as_path),
            Some(Path::new("/workspace/output.json"))
        );
        Ok(())
    }

    #[test]
    fn constructs_unstructured_reusable_dispatch_value() -> Result<(), DispatchError> {
        let arguments = ArgumentVector::new(vec!["positional prompt".to_owned()]);
        let envelope = DispatchEnvelope::new(
            Executable::parse("claude")?,
            AbsoluteWorkingDirectory::parse("/workspace/project")?,
            StdinBinding::Null,
        )
        .with_arguments(arguments);

        assert_eq!(
            envelope.arguments().as_slice(),
            &["positional prompt".to_owned()]
        );
        assert_eq!(envelope.stdin(), &StdinBinding::Null);
        assert_eq!(envelope.sandbox(), None);
        assert_eq!(envelope.schema_path(), None);
        assert_eq!(envelope.output_path(), None);
        Ok(())
    }
}
