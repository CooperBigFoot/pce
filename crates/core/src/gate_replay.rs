//! repair_sensitive_replay : ExpectedVerdictOutcome × ReplayObservation² × ReplayObservation² → RepairSensitivity   (pure, deterministic)
//! Pure relocation, normalization, reproducibility, and closed replay classification.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::{
    GateObservedResult, GateProcessObservation, GateProcessStimulus, GateStimulus,
    GateTerminalStatus,
};

/// One user-supplied ref spelling retained for reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct NamedReplayRef(String);

impl NamedReplayRef {
    /// Preserve a non-empty ref spelling.
    pub fn parse(value: impl Into<String>) -> Result<Self, GateReplayError> {
        let value = value.into();
        if value.is_empty() {
            return Err(GateReplayError::EmptyReplayRef);
        }
        Ok(Self(value))
    }
    /// Borrow the original spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A path whose components are all ordinary repository-relative names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRelativePath(PathBuf);

impl RepositoryRelativePath {
    fn parse(value: impl Into<PathBuf>, kind: ReplayPathKind) -> Result<Self, GateReplayError> {
        let value = value.into();
        let valid = !value.as_os_str().is_empty()
            && !value.is_absolute()
            && value
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if !valid {
            return Err(match kind {
                ReplayPathKind::Schema => GateReplayError::InvalidSchemaPath,
                ReplayPathKind::Output => GateReplayError::InvalidOutputPath,
            });
        }
        Ok(Self(value))
    }
    /// Borrow the checked relative path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

enum ReplayPathKind {
    Schema,
    Output,
}

/// Parse a target-checkout schema path.
pub fn parse_replay_schema_path(
    value: impl Into<PathBuf>,
) -> Result<RepositoryRelativePath, GateReplayError> {
    RepositoryRelativePath::parse(value, ReplayPathKind::Schema)
}

/// Parse a target-checkout output path.
pub fn parse_replay_output_path(
    value: impl Into<PathBuf>,
) -> Result<RepositoryRelativePath, GateReplayError> {
    RepositoryRelativePath::parse(value, ReplayPathKind::Output)
}

/// Parent-owned expected verdict outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedVerdictOutcome {
    Conforming,
    Nonconforming,
}

impl ExpectedVerdictOutcome {
    /// Parse the two CLI spellings.
    pub fn parse(value: &str) -> Result<Self, GateReplayError> {
        match value {
            "conforming-verdict" => Ok(Self::Conforming),
            "nonconforming-verdict" => Ok(Self::Nonconforming),
            _ => Err(GateReplayError::UnsupportedExpectedOutcome {
                value: value.to_owned(),
            }),
        }
    }
}

/// Closed artifact observation with exact normalized readable bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReplayArtifactObservation {
    Conforming { bytes: Vec<u8> },
    Missing,
    InvalidJson { bytes: Vec<u8> },
    SchemaViolating { bytes: Vec<u8> },
}

impl ReplayArtifactObservation {
    /// Whether the artifact conforms to the retained target schema.
    pub const fn conformance(&self) -> ArtifactConformance {
        match self {
            Self::Conforming { .. } => ArtifactConformance::Conforming,
            _ => ArtifactConformance::Nonconforming,
        }
    }
}

/// Closed conformance state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactConformance {
    Conforming,
    Nonconforming,
}

/// One complete raw process and artifact observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReplayObservation {
    pub process: GateObservedResult,
    pub artifact: ReplayArtifactObservation,
}

/// Whether a reproducible observation meets the parent oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpectedMatch {
    Yes,
    No,
}

/// The two-run result for a successfully prepared ref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReplayRefOutcome {
    Reproducible {
        matches_expected: ExpectedMatch,
        observation: ReplayObservation,
    },
    NonReproducible {
        first: ReplayObservation,
        second: ReplayObservation,
    },
}

/// Fold two raw runs before consulting the expected outcome.
pub fn fold_replay_runs(
    first: ReplayObservation,
    second: ReplayObservation,
    expected: ExpectedVerdictOutcome,
) -> ReplayRefOutcome {
    if first != second {
        return ReplayRefOutcome::NonReproducible { first, second };
    }
    let matches = matches!(
        (first.artifact.conformance(), expected),
        (
            ArtifactConformance::Conforming,
            ExpectedVerdictOutcome::Conforming
        ) | (
            ArtifactConformance::Nonconforming,
            ExpectedVerdictOutcome::Nonconforming
        )
    );
    ReplayRefOutcome::Reproducible {
        matches_expected: if matches {
            ExpectedMatch::Yes
        } else {
            ExpectedMatch::No
        },
        observation: first,
    }
}

/// Checkout failure stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckoutStage {
    ResolveRef,
    CreateWorktree,
}

/// Oracle failure stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum OracleStage {
    PrepareOutput,
    ReadSchema,
}

/// Stable typed checkout failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CheckoutFailure {
    pub stage: CheckoutStage,
    pub diagnostic: String,
}

/// Stable typed oracle failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OracleFailure {
    pub stage: OracleStage,
    pub diagnostic: String,
}

/// Report-facing result for one requested ref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ReplayRefResult {
    Completed {
        requested_ref: NamedReplayRef,
        resolved_commit: String,
        outcome: ReplayRefOutcome,
    },
    CheckoutFailed {
        requested_ref: NamedReplayRef,
        checkout_failed: CheckoutFailure,
    },
    OracleFailed {
        requested_ref: NamedReplayRef,
        resolved_commit: String,
        oracle_failed: OracleFailure,
    },
}

/// Closed top-level replay classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepairSensitivity {
    RepairSensitive,
    NoRepairSignal,
    OppositeDirection,
    NonReproducible,
    CheckoutFailed,
    OracleFailed,
}

/// Classify every pair with typed-failure precedence.
pub fn classify_replay_pair(
    broken: &ReplayRefResult,
    repaired: &ReplayRefResult,
) -> RepairSensitivity {
    if matches!(broken, ReplayRefResult::CheckoutFailed { .. })
        || matches!(repaired, ReplayRefResult::CheckoutFailed { .. })
    {
        return RepairSensitivity::CheckoutFailed;
    }
    if matches!(broken, ReplayRefResult::OracleFailed { .. })
        || matches!(repaired, ReplayRefResult::OracleFailed { .. })
    {
        return RepairSensitivity::OracleFailed;
    }
    let (
        ReplayRefResult::Completed { outcome: left, .. },
        ReplayRefResult::Completed { outcome: right, .. },
    ) = (broken, repaired)
    else {
        unreachable!("failure variants handled")
    };
    if matches!(left, ReplayRefOutcome::NonReproducible { .. })
        || matches!(right, ReplayRefOutcome::NonReproducible { .. })
    {
        return RepairSensitivity::NonReproducible;
    }
    let (
        ReplayRefOutcome::Reproducible {
            matches_expected: left,
            ..
        },
        ReplayRefOutcome::Reproducible {
            matches_expected: right,
            ..
        },
    ) = (left, right)
    else {
        unreachable!("non-reproducible handled")
    };
    match (left, right) {
        (ExpectedMatch::No, ExpectedMatch::Yes) => RepairSensitivity::RepairSensitive,
        (ExpectedMatch::Yes, ExpectedMatch::No) => RepairSensitivity::OppositeDirection,
        _ => RepairSensitivity::NoRepairSignal,
    }
}

fn replace_bytes(bytes: &[u8], old: &[u8], new: &[u8]) -> Vec<u8> {
    if old.is_empty() {
        return bytes.to_vec();
    }
    let mut output = Vec::new();
    let mut remaining = bytes;
    while let Some(index) = remaining
        .windows(old.len())
        .position(|window| window == old)
    {
        output.extend_from_slice(&remaining[..index]);
        output.extend_from_slice(new);
        remaining = &remaining[index + old.len()..];
    }
    output.extend_from_slice(remaining);
    output
}

fn replace_string(value: &str, old: &str, new: &str) -> String {
    String::from_utf8(replace_bytes(
        value.as_bytes(),
        old.as_bytes(),
        new.as_bytes(),
    ))
    .unwrap_or_else(|_| value.to_owned())
}

fn rebase_process(process: &GateProcessStimulus, old: &str, new: &str) -> GateProcessStimulus {
    let environment = process
        .environment()
        .iter()
        .map(|(name, value)| (name.clone(), replace_string(value, old, new)))
        .collect::<BTreeMap<_, _>>();
    GateProcessStimulus::reconstruct(
        replace_string(process.program(), old, new),
        process
            .arguments()
            .iter()
            .map(|argument| replace_string(argument, old, new))
            .collect(),
        replace_bytes(process.input(), old.as_bytes(), new.as_bytes()),
        environment,
    )
}

/// Relocate every recorded-root occurrence and reject a cwd outside that root.
pub fn rebase_gate_stimulus(
    stimulus: &GateStimulus,
    recorded_root: &Path,
    checkout_root: &Path,
) -> Result<GateStimulus, GateReplayError> {
    let relative = stimulus
        .working_directory()
        .strip_prefix(recorded_root)
        .map_err(|_| GateReplayError::WorkingDirectoryOutsideRoot)?;
    if !relative
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
        && !relative.as_os_str().is_empty()
    {
        return Err(GateReplayError::WorkingDirectoryOutsideRoot);
    }
    let old = recorded_root.to_str().ok_or(GateReplayError::NonUtf8Root)?;
    let new = checkout_root.to_str().ok_or(GateReplayError::NonUtf8Root)?;
    Ok(GateStimulus::reconstruct(
        PathBuf::from(replace_string(
            stimulus
                .working_directory()
                .to_str()
                .ok_or(GateReplayError::NonUtf8Root)?,
            old,
            new,
        )),
        stimulus
            .setup()
            .iter()
            .map(|process| rebase_process(process, old, new))
            .collect(),
        rebase_process(stimulus.command(), old, new),
    ))
}

fn normalize_process(
    mut observation: GateProcessObservation,
    old: &[u8],
    new: &[u8],
) -> GateProcessObservation {
    observation.stdout = replace_bytes(&observation.stdout, old, new);
    observation.stderr = replace_bytes(&observation.stderr, old, new);
    if let GateTerminalStatus::SpawnFailed { detail } = &mut observation.status {
        *detail = String::from_utf8(replace_bytes(detail.as_bytes(), old, new))
            .unwrap_or_else(|_| detail.clone());
    }
    observation
}

/// Normalize checkout and parent-root occurrences back to recorded-root identity.
pub fn normalize_replay_observation(
    mut observation: ReplayObservation,
    recorded_root: &Path,
    checkout_root: &Path,
    replay_parent: &Path,
) -> Result<ReplayObservation, GateReplayError> {
    let checkout = checkout_root
        .to_str()
        .ok_or(GateReplayError::NonUtf8Root)?
        .as_bytes();
    let parent = replay_parent
        .to_str()
        .ok_or(GateReplayError::NonUtf8Root)?
        .as_bytes();
    let recorded = recorded_root
        .to_str()
        .ok_or(GateReplayError::NonUtf8Root)?
        .as_bytes();
    for (old, new) in [(checkout, recorded), (parent, recorded)] {
        observation.process.setup = observation
            .process
            .setup
            .into_iter()
            .map(|item| normalize_process(item, old, new))
            .collect();
        observation.process.command = observation
            .process
            .command
            .map(|item| normalize_process(item, old, new));
        match &mut observation.artifact {
            ReplayArtifactObservation::Conforming { bytes }
            | ReplayArtifactObservation::InvalidJson { bytes }
            | ReplayArtifactObservation::SchemaViolating { bytes } => {
                *bytes = replace_bytes(bytes, old, new)
            }
            ReplayArtifactObservation::Missing => {}
        }
    }
    Ok(observation)
}

/// Pure replay-domain validation failure.
#[derive(Debug, Error)]
pub enum GateReplayError {
    /// A named ref was empty.
    #[error("replay ref must not be empty")]
    EmptyReplayRef,
    /// The schema path was not a strict repository-relative path.
    #[error("replay schema path must be repository-relative and contain no `.` or `..` component")]
    InvalidSchemaPath,
    /// The output path was not a strict repository-relative path.
    #[error("replay output path must be repository-relative and contain no `.` or `..` component")]
    InvalidOutputPath,
    /// The expected outcome spelling is closed.
    #[error("unsupported expected verdict outcome `{value}`")]
    UnsupportedExpectedOutcome { value: String },
    /// The recorded cwd lies outside the supplied canonical repository root.
    #[error("recorded gate execution working directory is outside replay repository root")]
    WorkingDirectoryOutsideRoot,
    /// A repository root could not be represented in the UTF-8 stimulus.
    #[error("replay repository root must be valid UTF-8")]
    NonUtf8Root,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GateObservedResult, parse_gate_stimulus};
    use std::path::Path;

    fn raw(bytes: &[u8]) -> ReplayObservation {
        ReplayObservation {
            process: GateObservedResult {
                setup: vec![],
                command: None,
            },
            artifact: ReplayArtifactObservation::Conforming {
                bytes: bytes.to_vec(),
            },
        }
    }

    #[test]
    fn raw_equality_precedes_expected_outcome() {
        assert!(matches!(
            fold_replay_runs(
                raw(b"first"),
                raw(b"second"),
                ExpectedVerdictOutcome::Conforming
            ),
            ReplayRefOutcome::NonReproducible { .. }
        ));
    }

    #[test]
    fn relocation_rewrites_occurrences_but_not_environment_names() {
        let stimulus = parse_gate_stimulus(br#"{"working_directory":"/old/sub/old-suffix","setup":[],"command":{"program":"/old/bin","arguments":["/old-suffix","/tmp/not-the-recorded-root"],"input":[47,111,108,100,10],"environment":{"/old":"prefix/old","ROOT":"/old"}}}"#).expect("stimulus");
        let rebased =
            rebase_gate_stimulus(&stimulus, Path::new("/old"), Path::new("/new")).expect("rebase");
        assert_eq!(
            rebased.working_directory(),
            Path::new("/new/sub/new-suffix")
        );
        assert_eq!(rebased.command().program(), "/new/bin");
        assert_eq!(
            rebased.command().arguments(),
            &["/new-suffix", "/tmp/not-the-recorded-root"]
        );
        assert!(rebased.command().environment().contains_key("/old"));
        assert_eq!(rebased.command().environment()["ROOT"], "/new");
    }

    #[test]
    fn paths_are_strict_and_classification_precedence_is_total() {
        for path in ["", ".", "a/../b", "/absolute"] {
            assert!(parse_replay_schema_path(path).is_err());
        }
        assert!(parse_replay_output_path("verdict.json").is_ok());
        let requested = NamedReplayRef::parse("ref").expect("ref");
        let checkout = ReplayRefResult::CheckoutFailed {
            requested_ref: requested.clone(),
            checkout_failed: CheckoutFailure {
                stage: CheckoutStage::ResolveRef,
                diagnostic: "x".to_owned(),
            },
        };
        let oracle = ReplayRefResult::OracleFailed {
            requested_ref: requested,
            resolved_commit: "a".repeat(40),
            oracle_failed: OracleFailure {
                stage: OracleStage::ReadSchema,
                diagnostic: "x".to_owned(),
            },
        };
        assert_eq!(
            classify_replay_pair(&checkout, &oracle),
            RepairSensitivity::CheckoutFailed
        );
    }

    fn completed(matches_expected: ExpectedMatch) -> ReplayRefResult {
        ReplayRefResult::Completed {
            requested_ref: NamedReplayRef::parse("ref").expect("ref"),
            resolved_commit: "a".repeat(40),
            outcome: ReplayRefOutcome::Reproducible {
                matches_expected,
                observation: raw(b"same"),
            },
        }
    }

    fn non_reproducible() -> ReplayRefResult {
        ReplayRefResult::Completed {
            requested_ref: NamedReplayRef::parse("ref").expect("ref"),
            resolved_commit: "a".repeat(40),
            outcome: ReplayRefOutcome::NonReproducible {
                first: raw(b"first"),
                second: raw(b"second"),
            },
        }
    }

    #[test]
    fn classification_is_total_over_every_per_ref_result_kind() {
        let requested = NamedReplayRef::parse("ref").expect("ref");
        let values = [
            ReplayRefResult::CheckoutFailed {
                requested_ref: requested.clone(),
                checkout_failed: CheckoutFailure {
                    stage: CheckoutStage::ResolveRef,
                    diagnostic: "checkout".to_owned(),
                },
            },
            ReplayRefResult::OracleFailed {
                requested_ref: requested,
                resolved_commit: "a".repeat(40),
                oracle_failed: OracleFailure {
                    stage: OracleStage::ReadSchema,
                    diagnostic: "oracle".to_owned(),
                },
            },
            non_reproducible(),
            completed(ExpectedMatch::No),
            completed(ExpectedMatch::Yes),
        ];
        for left in &values {
            for right in &values {
                let expected = if matches!(left, ReplayRefResult::CheckoutFailed { .. })
                    || matches!(right, ReplayRefResult::CheckoutFailed { .. })
                {
                    RepairSensitivity::CheckoutFailed
                } else if matches!(left, ReplayRefResult::OracleFailed { .. })
                    || matches!(right, ReplayRefResult::OracleFailed { .. })
                {
                    RepairSensitivity::OracleFailed
                } else if matches!(
                    left,
                    ReplayRefResult::Completed {
                        outcome: ReplayRefOutcome::NonReproducible { .. },
                        ..
                    }
                ) || matches!(
                    right,
                    ReplayRefResult::Completed {
                        outcome: ReplayRefOutcome::NonReproducible { .. },
                        ..
                    }
                ) {
                    RepairSensitivity::NonReproducible
                } else {
                    match (left, right) {
                        (
                            ReplayRefResult::Completed {
                                outcome:
                                    ReplayRefOutcome::Reproducible {
                                        matches_expected: ExpectedMatch::No,
                                        ..
                                    },
                                ..
                            },
                            ReplayRefResult::Completed {
                                outcome:
                                    ReplayRefOutcome::Reproducible {
                                        matches_expected: ExpectedMatch::Yes,
                                        ..
                                    },
                                ..
                            },
                        ) => RepairSensitivity::RepairSensitive,
                        (
                            ReplayRefResult::Completed {
                                outcome:
                                    ReplayRefOutcome::Reproducible {
                                        matches_expected: ExpectedMatch::Yes,
                                        ..
                                    },
                                ..
                            },
                            ReplayRefResult::Completed {
                                outcome:
                                    ReplayRefOutcome::Reproducible {
                                        matches_expected: ExpectedMatch::No,
                                        ..
                                    },
                                ..
                            },
                        ) => RepairSensitivity::OppositeDirection,
                        _ => RepairSensitivity::NoRepairSignal,
                    }
                };
                assert_eq!(classify_replay_pair(left, right), expected);
            }
        }
    }

    #[test]
    fn normalization_is_only_inverse_root_relocation() {
        let observation = ReplayObservation {
            process: GateObservedResult {
                setup: vec![GateProcessObservation {
                    status: GateTerminalStatus::SpawnFailed {
                        detail: "/opaque/checkout/program".to_owned(),
                    },
                    stdout: b"prefix/opaque".to_vec(),
                    stderr: b"/unrelated".to_vec(),
                }],
                command: None,
            },
            artifact: ReplayArtifactObservation::Conforming {
                bytes: b"/opaque/checkout/artifact".to_vec(),
            },
        };
        let normalized = normalize_replay_observation(
            observation,
            Path::new("/recorded"),
            Path::new("/opaque/checkout"),
            Path::new("/opaque"),
        )
        .expect("normalize");
        assert_eq!(normalized.process.setup[0].stdout, b"prefix/recorded");
        assert_eq!(normalized.process.setup[0].stderr, b"/unrelated");
        assert!(matches!(
            &normalized.process.setup[0].status,
            GateTerminalStatus::SpawnFailed { detail } if detail == "/recorded/program"
        ));
        assert_eq!(
            normalized.artifact,
            ReplayArtifactObservation::Conforming {
                bytes: b"/recorded/artifact".to_vec()
            }
        );
    }
}
