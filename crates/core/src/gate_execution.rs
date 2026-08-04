//! execution_record : GateExecutionRef × GateStimulus × GateObservedResult → GateExecutionRecord; valid_references = VerdictReferences ⊆ GateExecutionRefs   (pure, deterministic)
//! Closed, shell-free falsification stimuli and their parent-observed execution evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

const MAX_SOCKET_PATH_BYTES: usize = 103;

/// One positive, sequential reference assigned by the parent recorder.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GateExecutionRef(String);

impl GateExecutionRef {
    /// Construct the reference for a positive accepted-request sequence.
    ///
    /// # Errors
    ///
    /// Returns [`GateExecutionError::ReferenceSpaceExhausted`] after sequence 999999.
    pub fn from_sequence(sequence: u32) -> Result<Self, GateExecutionError> {
        if !(1..=999_999).contains(&sequence) {
            return Err(GateExecutionError::ReferenceSpaceExhausted);
        }
        Ok(Self(format!("execution-{sequence:06}")))
    }

    /// Parse a canonical positive execution reference.
    ///
    /// # Errors
    ///
    /// Returns [`GateExecutionError::InvalidReference`] for any other spelling.
    pub fn parse(raw: impl Into<String>) -> Result<Self, GateExecutionError> {
        let raw = raw.into();
        let valid_shape = raw.len() == 16
            && raw.starts_with("execution-")
            && raw[10..].bytes().all(|byte| byte.is_ascii_digit());
        if !valid_shape || raw == "execution-000000" {
            return Err(GateExecutionError::InvalidReference { reference: raw });
        }
        Ok(Self(raw))
    }

    /// Borrow the canonical reference spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GateExecutionRef {
    type Error = GateExecutionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<GateExecutionRef> for String {
    fn from(value: GateExecutionRef) -> Self {
        value.0
    }
}

/// An absolute executable path for the nested `pce gate exec` client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteGateExecClientPath(PathBuf);

impl AbsoluteGateExecClientPath {
    /// Parse a lexically absolute executable path that Claude can represent in a Bash grant.
    ///
    /// # Errors
    ///
    /// Returns an error for relative paths or paths containing a space or parenthesis.
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, GateExecutionError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(GateExecutionError::RelativeClientPath { path });
        }
        let spelling = path.as_os_str().to_string_lossy();
        if spelling.contains([' ', '(', ')']) {
            return Err(GateExecutionError::UnrepresentableClientPath);
        }
        Ok(Self(path))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// The absolute immutable evidence-sidecar path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteGateExecutionEvidencePath(PathBuf);

impl AbsoluteGateExecutionEvidencePath {
    /// Derive the evidence path by appending the literal suffix to the complete verdict filename.
    pub fn from_verdict_path(verdict: &Path) -> Self {
        let mut value = verdict.as_os_str().to_os_string();
        value.push(".executions.json");
        Self(PathBuf::from(value))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A bounded absolute Unix-domain socket path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteGateExecutionSocketPath(PathBuf);

impl AbsoluteGateExecutionSocketPath {
    /// Parse an absolute socket path and enforce the platform pathname-byte limit.
    ///
    /// # Errors
    ///
    /// Returns an error when the path is relative, non-UTF-8, or longer than 103 bytes.
    pub fn parse(raw: impl Into<PathBuf>) -> Result<Self, GateExecutionError> {
        let path = raw.into();
        if !path.is_absolute() {
            return Err(GateExecutionError::RelativeSocketPath { path });
        }
        let bytes = path
            .to_str()
            .ok_or(GateExecutionError::NonUtf8SocketPath)?
            .len();
        if bytes > MAX_SOCKET_PATH_BYTES {
            return Err(GateExecutionError::SocketPathTooLong);
        }
        Ok(Self(path))
    }

    /// Construct the parent-owned socket filename beneath a supplied temp directory.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::parse`].
    pub fn construct(
        temp_directory: &Path,
        pid: u32,
        issuance: u64,
    ) -> Result<Self, GateExecutionError> {
        Self::parse(temp_directory.join(format!("pce-gate-exec-{pid}-{issuance}.sock")))
    }

    /// Borrow the absolute path unchanged.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Exact-role recorder paths and nested helper executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateExecutionRecorderConfig {
    client: AbsoluteGateExecClientPath,
    evidence: AbsoluteGateExecutionEvidencePath,
    socket: AbsoluteGateExecutionSocketPath,
}

impl GateExecutionRecorderConfig {
    /// Construct a complete recorder configuration.
    pub fn new(
        client: AbsoluteGateExecClientPath,
        evidence: AbsoluteGateExecutionEvidencePath,
        socket: AbsoluteGateExecutionSocketPath,
    ) -> Self {
        Self {
            client,
            evidence,
            socket,
        }
    }

    /// Borrow the helper executable path.
    pub fn client(&self) -> &AbsoluteGateExecClientPath {
        &self.client
    }
    /// Borrow the evidence sidecar path.
    pub fn evidence(&self) -> &AbsoluteGateExecutionEvidencePath {
        &self.evidence
    }
    /// Borrow the socket path.
    pub fn socket(&self) -> &AbsoluteGateExecutionSocketPath {
        &self.socket
    }
}

/// One exact shell-free process stimulus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateProcessStimulus {
    program: String,
    arguments: Vec<String>,
    input: Vec<u8>,
    environment: BTreeMap<String, String>,
}

impl GateProcessStimulus {
    pub(crate) fn reconstruct(
        program: String,
        arguments: Vec<String>,
        input: Vec<u8>,
        environment: BTreeMap<String, String>,
    ) -> Self {
        Self {
            program,
            arguments,
            input,
            environment,
        }
    }
    /// Borrow the program spelling.
    pub fn program(&self) -> &str {
        &self.program
    }
    /// Borrow the ordered arguments.
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
    /// Borrow exact stdin bytes.
    pub fn input(&self) -> &[u8] {
        &self.input
    }
    /// Borrow the complete explicit environment.
    pub fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }
}

/// One complete request submitted to the parent recorder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateStimulus {
    working_directory: PathBuf,
    setup: Vec<GateProcessStimulus>,
    command: GateProcessStimulus,
}

impl GateStimulus {
    pub(crate) fn reconstruct(
        working_directory: PathBuf,
        setup: Vec<GateProcessStimulus>,
        command: GateProcessStimulus,
    ) -> Self {
        Self {
            working_directory,
            setup,
            command,
        }
    }
    /// Borrow the absolute working directory.
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }
    /// Borrow ordered setup actions.
    pub fn setup(&self) -> &[GateProcessStimulus] {
        &self.setup
    }
    /// Borrow the final command.
    pub fn command(&self) -> &GateProcessStimulus {
        &self.command
    }

    fn validate(self) -> Result<Self, GateExecutionError> {
        if !self.working_directory.is_absolute() {
            return Err(GateExecutionError::RelativeExecutionWorkingDirectory);
        }
        for process in self.setup.iter().chain(std::iter::once(&self.command)) {
            if process.program.is_empty() {
                return Err(GateExecutionError::EmptyProgram);
            }
            if process.environment.keys().any(String::is_empty) {
                return Err(GateExecutionError::EmptyEnvironmentName);
            }
        }
        Ok(self)
    }
}

/// Parse and validate exactly one request JSON value with no trailing JSON.
///
/// # Errors
///
/// Returns a typed validation diagnostic or the JSON parser diagnostic.
pub fn parse_gate_stimulus(bytes: &[u8]) -> Result<GateStimulus, GateExecutionError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let stimulus = GateStimulus::deserialize(&mut deserializer)
        .map_err(|source| GateExecutionError::InvalidRequestJson { source })?;
    deserializer
        .end()
        .map_err(|source| GateExecutionError::InvalidRequestJson { source })?;
    stimulus.validate()
}

/// A closed terminal process observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GateTerminalStatus {
    /// The process exited normally with the supplied code.
    Exited { code: i32 },
    /// The process was terminated by the supplied signal.
    Signaled { signal: i32 },
    /// The process could not be spawned.
    SpawnFailed { detail: String },
}

/// Captured bytes and terminal status for one process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateProcessObservation {
    pub status: GateTerminalStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Ordered setup observations and the optional command observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateObservedResult {
    pub setup: Vec<GateProcessObservation>,
    pub command: Option<GateProcessObservation>,
}

/// One immutable parent-authored execution record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateExecutionRecord {
    pub execution_ref: GateExecutionRef,
    pub stimulus: GateStimulus,
    pub observed_result: GateObservedResult,
}

/// The complete immutable evidence-sidecar document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateExecutionEvidence {
    schema_id: String,
    schema_version: u32,
    executions: Vec<GateExecutionRecord>,
}

impl GateExecutionEvidence {
    /// Construct the canonical document from every retained execution.
    pub fn new(executions: Vec<GateExecutionRecord>) -> Self {
        Self {
            schema_id: "pce.gate-execution-evidence".to_owned(),
            schema_version: 1,
            executions,
        }
    }

    /// Borrow retained records in accepted-request order.
    pub fn executions(&self) -> &[GateExecutionRecord] {
        &self.executions
    }

    /// Look up one retained record by canonical execution reference.
    ///
    /// # Errors
    ///
    /// Returns [`GateExecutionError::MissingEvidenceReference`] when the reference is absent.
    pub fn record(
        &self,
        execution_ref: &GateExecutionRef,
    ) -> Result<&GateExecutionRecord, GateExecutionError> {
        self.executions
            .iter()
            .find(|record| &record.execution_ref == execution_ref)
            .ok_or_else(|| GateExecutionError::MissingEvidenceReference {
                reference: execution_ref.as_str().to_owned(),
            })
    }
}

/// Parse and validate one complete immutable evidence document.
///
/// # Errors
///
/// Returns a stable schema/reference diagnostic or the JSON parser error.
pub fn parse_gate_execution_evidence(
    bytes: &[u8],
) -> Result<GateExecutionEvidence, GateExecutionError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let evidence = GateExecutionEvidence::deserialize(&mut deserializer)
        .map_err(|source| GateExecutionError::InvalidEvidenceJson { source })?;
    deserializer
        .end()
        .map_err(|source| GateExecutionError::InvalidEvidenceJson { source })?;
    if evidence.schema_id != "pce.gate-execution-evidence" {
        return Err(GateExecutionError::InvalidEvidenceSchemaId {
            found: evidence.schema_id,
        });
    }
    if evidence.schema_version != 1 {
        return Err(GateExecutionError::InvalidEvidenceSchemaVersion {
            found: evidence.schema_version,
        });
    }
    let mut references = BTreeSet::new();
    for record in &evidence.executions {
        if !references.insert(record.execution_ref.as_str()) {
            return Err(GateExecutionError::DuplicateEvidenceReference {
                reference: record.execution_ref.as_str().to_owned(),
            });
        }
    }
    Ok(evidence)
}

/// Successful parent-to-client response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateExecutionResponse {
    pub execution_ref: GateExecutionRef,
    pub observed_result: GateObservedResult,
}

/// Parent rejection response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateExecutionRejection {
    pub error: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerdictDocument {
    #[serde(rename = "verdict")]
    _verdict: Value,
    #[serde(rename = "self_sufficiency")]
    _self_sufficiency: Value,
    #[serde(rename = "root_cause")]
    _root_cause: Value,
    blocking_issues: Vec<VerdictBlockingIssue>,
    #[serde(rename = "non_blocking_notes")]
    _non_blocking_notes: Value,
    #[serde(rename = "summary")]
    _summary: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerdictBlockingIssue {
    id: String,
    #[serde(rename = "severity")]
    _severity: Value,
    #[serde(rename = "location")]
    _location: Value,
    #[serde(rename = "problem")]
    _problem: Value,
    #[serde(rename = "input")]
    _input: Value,
    #[serde(rename = "observation")]
    _observation: Value,
    execution_ref: Option<String>,
    #[serde(rename = "required_change")]
    _required_change: Value,
    replacement_execution: VerdictReplacementExecution,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerdictReplacementExecution {
    #[serde(rename = "input")]
    _input: Value,
    #[serde(rename = "observation")]
    _observation: Value,
    execution_ref: Option<String>,
}

/// Require every blocking issue and replacement to cite the retained same-dispatch collection.
///
/// # Errors
///
/// Returns the first missing, malformed, or unknown reference diagnostic in issue order.
pub fn validate_verdict_references(
    verdict_bytes: &[u8],
    records: &[GateExecutionRecord],
) -> Result<(), GateExecutionError> {
    let verdict: VerdictDocument = serde_json::from_slice(verdict_bytes)
        .map_err(|source| GateExecutionError::InvalidVerdictJson { source })?;
    let known = records
        .iter()
        .map(|record| record.execution_ref.as_str())
        .collect::<BTreeSet<_>>();
    for issue in verdict.blocking_issues {
        let primary_raw =
            issue
                .execution_ref
                .ok_or_else(|| GateExecutionError::MissingPrimaryReference {
                    issue_id: issue.id.clone(),
                })?;
        let primary = GateExecutionRef::parse(primary_raw)?;
        if !known.contains(primary.as_str()) {
            return Err(GateExecutionError::UnknownPrimaryReference {
                issue_id: issue.id.clone(),
                reference: primary.as_str().to_owned(),
            });
        }
        let replacement_raw = issue.replacement_execution.execution_ref.ok_or_else(|| {
            GateExecutionError::MissingReplacementReference {
                issue_id: issue.id.clone(),
            }
        })?;
        let replacement = GateExecutionRef::parse(replacement_raw)?;
        if !known.contains(replacement.as_str()) {
            return Err(GateExecutionError::UnknownReplacementReference {
                issue_id: issue.id,
                reference: replacement.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

/// Pure gate-execution parsing or reference-validation failure.
#[derive(Debug, Error)]
pub enum GateExecutionError {
    /// Evidence JSON was malformed, trailing, or structurally invalid.
    #[error("failed to parse gate execution evidence")]
    InvalidEvidenceJson {
        #[source]
        source: serde_json::Error,
    },
    /// Evidence carried another schema identity.
    #[error(
        "gate execution evidence schema id must be `pce.gate-execution-evidence`; found `{found}`"
    )]
    InvalidEvidenceSchemaId { found: String },
    /// Evidence carried another schema version.
    #[error("gate execution evidence schema version must be `1`; found `{found}`")]
    InvalidEvidenceSchemaVersion { found: u32 },
    /// Evidence repeated a canonical execution reference.
    #[error("gate execution evidence contains duplicate reference `{reference}`")]
    DuplicateEvidenceReference { reference: String },
    /// Evidence omitted the requested canonical execution reference.
    #[error("gate execution evidence does not contain reference `{reference}`")]
    MissingEvidenceReference { reference: String },
    /// A helper executable path was not absolute.
    #[error("pce gate-exec client path must be absolute; rejected {path:?}")]
    RelativeClientPath { path: PathBuf },
    /// Claude's Bash permission grammar cannot represent the executable path safely.
    #[error("pce executable path cannot be represented in Claude Bash permission")]
    UnrepresentableClientPath,
    /// A Unix socket path was not absolute.
    #[error("gate execution socket path must be absolute; rejected {path:?}")]
    RelativeSocketPath { path: PathBuf },
    /// A Unix socket path was not UTF-8 and cannot be byte-counted portably.
    #[error("gate execution socket path must be valid UTF-8")]
    NonUtf8SocketPath,
    /// A Unix socket pathname exceeded the accepted 103 encoded bytes.
    #[error("gate execution socket path exceeds the platform limit")]
    SocketPathTooLong,
    /// A request working directory was relative.
    #[error("gate execution working directory must be absolute")]
    RelativeExecutionWorkingDirectory,
    /// A request contained an empty process program.
    #[error("gate execution program must not be empty")]
    EmptyProgram,
    /// A request contained an empty environment name.
    #[error("gate execution environment name must not be empty")]
    EmptyEnvironmentName,
    /// Request JSON was malformed, trailing, or structurally invalid.
    #[error("{source}")]
    InvalidRequestJson { source: serde_json::Error },
    /// Verdict JSON could not be decoded at the typed exact-role boundary.
    #[error("{source}")]
    InvalidVerdictJson { source: serde_json::Error },
    /// The positive six-digit execution reference space was exhausted.
    #[error("gate execution reference space exhausted")]
    ReferenceSpaceExhausted,
    /// An execution reference was not canonical and positive.
    #[error("invalid gate execution reference `{reference}`")]
    InvalidReference { reference: String },
    /// A blocking issue omitted its primary reference.
    #[error("blocking issue `{issue_id}` is missing gate execution reference")]
    MissingPrimaryReference { issue_id: String },
    /// A blocking issue omitted its replacement reference.
    #[error("blocking issue `{issue_id}` replacement is missing gate execution reference")]
    MissingReplacementReference { issue_id: String },
    /// A blocking issue cited a primary reference absent from retained records.
    #[error("blocking issue `{issue_id}` references unknown gate execution `{reference}`")]
    UnknownPrimaryReference { issue_id: String, reference: String },
    /// A blocking issue cited a replacement reference absent from retained records.
    #[error(
        "blocking issue `{issue_id}` replacement references unknown gate execution `{reference}`"
    )]
    UnknownReplacementReference { issue_id: String, reference: String },
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::{
        AbsoluteGateExecutionSocketPath, GateExecutionEvidence, GateExecutionRecord,
        GateExecutionRef, GateObservedResult, GateProcessObservation, GateTerminalStatus,
        parse_gate_execution_evidence, parse_gate_stimulus, validate_verdict_references,
    };

    const EXAMPLE: &str = r#"{"working_directory":"/fixture/worktree","setup":[{"program":"/fixture/bin/setup","arguments":["--prepare","value with spaces"],"input":[115,101,116,117,112],"environment":{"LANG":"C","TOKEN":"setup"}}],"command":{"program":"/fixture/bin/probe","arguments":["--check","value with spaces"],"input":[0,255,10],"environment":{"LANG":"C","TOKEN":"probe"}}}"#;

    fn example_record() -> GateExecutionRecord {
        GateExecutionRecord {
            execution_ref: GateExecutionRef::from_sequence(1).expect("reference"),
            stimulus: parse_gate_stimulus(EXAMPLE.as_bytes()).expect("stimulus"),
            observed_result: GateObservedResult {
                setup: vec![GateProcessObservation {
                    status: GateTerminalStatus::Exited { code: 0 },
                    stdout: b"setup-out".to_vec(),
                    stderr: b"setup-err".to_vec(),
                }],
                command: Some(GateProcessObservation {
                    status: GateTerminalStatus::Exited { code: 7 },
                    stdout: vec![0, 255, 10],
                    stderr: b"probe-err".to_vec(),
                }),
            },
        }
    }

    #[test]
    fn compact_request_and_evidence_are_canonical() {
        let stimulus = parse_gate_stimulus(EXAMPLE.as_bytes()).expect("stimulus");
        assert_eq!(
            serde_json::to_string(&stimulus).expect("request JSON"),
            EXAMPLE
        );
        assert_eq!(
            serde_json::to_string(&GateExecutionEvidence::new(vec![example_record()]))
                .expect("evidence JSON"),
            r#"{"schema_id":"pce.gate-execution-evidence","schema_version":1,"executions":[{"execution_ref":"execution-000001","stimulus":{"working_directory":"/fixture/worktree","setup":[{"program":"/fixture/bin/setup","arguments":["--prepare","value with spaces"],"input":[115,101,116,117,112],"environment":{"LANG":"C","TOKEN":"setup"}}],"command":{"program":"/fixture/bin/probe","arguments":["--check","value with spaces"],"input":[0,255,10],"environment":{"LANG":"C","TOKEN":"probe"}}},"observed_result":{"setup":[{"status":{"kind":"exited","code":0},"stdout":[115,101,116,117,112,45,111,117,116],"stderr":[115,101,116,117,112,45,101,114,114]}],"command":{"status":{"kind":"exited","code":7},"stdout":[0,255,10],"stderr":[112,114,111,98,101,45,101,114,114]}}}]}"#
        );
        assert_eq!(
            serde_json::to_string(&GateExecutionEvidence::new(Vec::new())).expect("empty evidence"),
            r#"{"schema_id":"pce.gate-execution-evidence","schema_version":1,"executions":[]}"#
        );
    }

    #[test]
    fn evidence_read_boundary_rejects_schema_and_reference_failures() {
        let wire = serde_json::to_vec(&GateExecutionEvidence::new(vec![example_record()]))
            .expect("evidence JSON");
        let parsed = parse_gate_execution_evidence(&wire).expect("evidence should parse");
        assert_eq!(
            parsed
                .record(&GateExecutionRef::from_sequence(1).expect("reference"))
                .expect("record")
                .execution_ref
                .as_str(),
            "execution-000001"
        );
        assert_eq!(
            parsed
                .record(&GateExecutionRef::from_sequence(2).expect("reference"))
                .expect_err("missing record")
                .to_string(),
            "gate execution evidence does not contain reference `execution-000002`"
        );
        let duplicate = serde_json::to_vec(&GateExecutionEvidence::new(vec![
            example_record(),
            example_record(),
        ]))
        .expect("duplicate evidence JSON");
        assert_eq!(
            parse_gate_execution_evidence(&duplicate)
                .expect_err("duplicate reference")
                .to_string(),
            "gate execution evidence contains duplicate reference `execution-000001`"
        );

        let mut value: serde_json::Value = serde_json::from_slice(&wire).expect("evidence value");
        value["schema_id"] = serde_json::json!("wrong");
        assert_eq!(
            parse_gate_execution_evidence(
                &serde_json::to_vec(&value).expect("wrong schema id JSON")
            )
            .expect_err("wrong schema id")
            .to_string(),
            "gate execution evidence schema id must be `pce.gate-execution-evidence`; found `wrong`"
        );
        value["schema_id"] = serde_json::json!("pce.gate-execution-evidence");
        value["schema_version"] = serde_json::json!(2);
        assert_eq!(
            parse_gate_execution_evidence(
                &serde_json::to_vec(&value).expect("wrong schema version JSON")
            )
            .expect_err("wrong schema version")
            .to_string(),
            "gate execution evidence schema version must be `1`; found `2`"
        );
        assert_eq!(
            parse_gate_execution_evidence(b"{")
                .expect_err("malformed evidence")
                .to_string(),
            "failed to parse gate execution evidence"
        );
    }

    #[test]
    fn request_boundary_returns_exact_domain_diagnostics() {
        let stimulus = parse_gate_stimulus(EXAMPLE.as_bytes()).expect("stimulus");
        assert_eq!(stimulus.command().input(), [0, 255, 10]);
        assert_eq!(stimulus.command().environment()["TOKEN"], "probe");
        for (value, diagnostic) in [
            (
                json!({"working_directory":"relative","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{}}}),
                "gate execution working directory must be absolute",
            ),
            (
                json!({"working_directory":"/tmp","setup":[],"command":{"program":"","arguments":[],"input":[],"environment":{}}}),
                "gate execution program must not be empty",
            ),
            (
                json!({"working_directory":"/tmp","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{"":"value"}}}),
                "gate execution environment name must not be empty",
            ),
        ] {
            let bytes = serde_json::to_vec(&value).expect("case JSON");
            assert_eq!(
                parse_gate_stimulus(&bytes)
                    .expect_err("invalid request")
                    .to_string(),
                diagnostic
            );
        }
    }

    #[test]
    fn references_are_positive_six_digit_values() {
        for invalid in ["execution-000000", "execution-1", "execution-000001x", ""] {
            assert!(GateExecutionRef::parse(invalid).is_err(), "{invalid}");
        }
        assert_eq!(
            GateExecutionRef::parse("execution-000001")
                .expect("valid")
                .as_str(),
            "execution-000001"
        );
    }

    #[test]
    fn socket_path_boundary_and_production_shape_are_exact() {
        let accepted = PathBuf::from(format!("/{}", "a".repeat(102)));
        assert!(AbsoluteGateExecutionSocketPath::parse(accepted).is_ok());
        let rejected = PathBuf::from(format!("/{}", "a".repeat(103)));
        assert_eq!(
            AbsoluteGateExecutionSocketPath::parse(rejected)
                .expect_err("104 bytes")
                .to_string(),
            "gate execution socket path exceeds the platform limit"
        );
        let production =
            AbsoluteGateExecutionSocketPath::construct(PathBuf::from("/tmp").as_path(), 42, 7)
                .expect("production path");
        assert_eq!(
            production.as_path(),
            PathBuf::from("/tmp/pce-gate-exec-42-7.sock")
        );
    }

    #[test]
    fn verdict_references_must_belong_to_issue_and_replacement() {
        let records = vec![example_record()];
        let approval = br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"none"}"#;
        validate_verdict_references(approval, &[]).expect("empty approval");
        let unknown = br#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"x","problem":"x","input":"x","observation":"x","execution_ref":"execution-000001","required_change":"x","replacement_execution":{"input":"x","observation":"x","execution_ref":"execution-000002"}}],"non_blocking_notes":[],"summary":"x"}"#;
        assert_eq!(
            validate_verdict_references(unknown, &records)
                .expect_err("unknown replacement")
                .to_string(),
            "blocking issue `F-1` replacement references unknown gate execution `execution-000002`"
        );

        let missing_primary = br#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"x","problem":"x","input":"x","observation":"x","required_change":"x","replacement_execution":{"input":"x","observation":"x","execution_ref":"execution-000001"}}],"non_blocking_notes":[],"summary":"x"}"#;
        assert_eq!(
            validate_verdict_references(missing_primary, &records)
                .expect_err("missing primary")
                .to_string(),
            "blocking issue `F-1` is missing gate execution reference"
        );
        let missing_replacement = br#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"x","problem":"x","input":"x","observation":"x","execution_ref":"execution-000001","required_change":"x","replacement_execution":{"input":"x","observation":"x"}}],"non_blocking_notes":[],"summary":"x"}"#;
        assert_eq!(
            validate_verdict_references(missing_replacement, &records)
                .expect_err("missing replacement")
                .to_string(),
            "blocking issue `F-1` replacement is missing gate execution reference"
        );
        let zero = br#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"x","problem":"x","input":"x","observation":"x","execution_ref":"execution-000000","required_change":"x","replacement_execution":{"input":"x","observation":"x","execution_ref":"execution-000001"}}],"non_blocking_notes":[],"summary":"x"}"#;
        assert_eq!(
            validate_verdict_references(zero, &records)
                .expect_err("zero reference")
                .to_string(),
            "invalid gate execution reference `execution-000000`"
        );
        let valid_then_unknown = br#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"x","problem":"x","input":"x","observation":"x","execution_ref":"execution-000001","required_change":"x","replacement_execution":{"input":"x","observation":"x","execution_ref":"execution-000001"}},{"id":"F-2","severity":"major","location":"y","problem":"y","input":"y","observation":"y","execution_ref":"execution-000003","required_change":"y","replacement_execution":{"input":"y","observation":"y","execution_ref":"execution-000001"}}],"non_blocking_notes":[],"summary":"x"}"#;
        assert_eq!(
            validate_verdict_references(valid_then_unknown, &records)
                .expect_err("unknown neighboring issue")
                .to_string(),
            "blocking issue `F-2` references unknown gate execution `execution-000003`"
        );
    }
}
