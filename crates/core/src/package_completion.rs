//! package_completion : VisionDirectory × WorkPackageId × Sequence × WorktreePath* → PackageResultPath
//!
//! This module purely derives restart-stable result locations, defines the strict worker-result
//! document, and composes the shell-free wrapper argument vector. It performs no filesystem I/O.

use std::path::{Component, Path, PathBuf};

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AbsoluteRequiredArtifactPath, DispatchDuration, DispatchExitStatus, RequiredArtifactPresence,
    Sequence, WorkPackageId,
};

/// A lexically absolute, UTF-8 path for one durable package-worker result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsolutePackageResultPath(PathBuf);

impl Serialize for AbsolutePackageResultPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AbsolutePackageResultPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

impl AbsolutePackageResultPath {
    /// Parse an absolute UTF-8 result path.
    ///
    /// # Errors
    ///
    /// Returns [`PackageCompletionError`] when the path is relative or is not valid UTF-8.
    pub fn parse(path: impl Into<PathBuf>) -> Result<Self, PackageCompletionError> {
        let path = path.into();
        if !path.is_absolute() {
            return Err(PackageCompletionError::PathNotAbsolute {
                field: "package result",
                path,
            });
        }
        if path.to_str().is_none() {
            return Err(PackageCompletionError::PathNotUtf8 {
                field: "package result",
            });
        }
        Ok(Self(path))
    }

    /// Borrow the exact validated path.
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// Borrow the exact validated UTF-8 spelling.
    pub fn as_str(&self) -> &str {
        self.0
            .to_str()
            .unwrap_or_else(|| unreachable!("constructor guarantees UTF-8"))
    }
}

/// An RFC 3339 stopping timestamp preserved byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageWorkerStoppedAt(String);

impl PackageWorkerStoppedAt {
    /// Parse an RFC 3339 timestamp without normalizing its representation.
    ///
    /// # Errors
    ///
    /// Returns [`PackageCompletionError::MalformedStoppedAt`] when parsing fails.
    pub fn parse(value: impl Into<String>) -> Result<Self, PackageCompletionError> {
        let value = value.into();
        DateTime::parse_from_rfc3339(&value).map_err(|_| {
            PackageCompletionError::MalformedStoppedAt {
                value: value.clone(),
            }
        })?;
        Ok(Self(value))
    }

    /// Return the exact validated RFC 3339 representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for PackageWorkerStoppedAt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for PackageWorkerStoppedAt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(serde::de::Error::custom)
    }
}

/// Whether processes in the isolated package-worker process group survived direct-child exit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurvivingProcesses {
    /// A pre-observation result carried no surviving-process measurement.
    #[default]
    Unknown,
    /// No process remained in the isolated worker process group.
    Absent,
    /// At least one process remained in the isolated worker process group.
    Present,
}

/// The complete durable observation written after a package worker stops.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageWorkerResult {
    duration_ms: DispatchDuration,
    stopped_at: PackageWorkerStoppedAt,
    exit_status: DispatchExitStatus,
    required_artifact_presence: RequiredArtifactPresence,
    #[serde(default)]
    surviving_processes: SurvivingProcesses,
}

impl PackageWorkerResult {
    /// Construct one complete package-worker observation.
    pub const fn new(
        duration_ms: DispatchDuration,
        stopped_at: PackageWorkerStoppedAt,
        exit_status: DispatchExitStatus,
        required_artifact_presence: RequiredArtifactPresence,
        surviving_processes: SurvivingProcesses,
    ) -> Self {
        Self {
            duration_ms,
            stopped_at,
            exit_status,
            required_artifact_presence,
            surviving_processes,
        }
    }

    /// Return the measured elapsed duration.
    pub const fn duration_ms(&self) -> DispatchDuration {
        self.duration_ms
    }

    /// Return the worker stopping timestamp.
    pub const fn stopped_at(&self) -> &PackageWorkerStoppedAt {
        &self.stopped_at
    }

    /// Return the process termination observation.
    pub const fn exit_status(&self) -> DispatchExitStatus {
        self.exit_status
    }

    /// Return whether the required artifact was a regular file at worker exit.
    pub const fn required_artifact_presence(&self) -> RequiredArtifactPresence {
        self.required_artifact_presence
    }

    /// Return whether any process survived in the isolated worker process group.
    pub const fn surviving_processes(&self) -> SurvivingProcesses {
        self.surviving_processes
    }
}

/// A pure package-completion carrier or composition failure.
#[derive(Debug, Error)]
pub enum PackageCompletionError {
    /// A required domain path was relative.
    #[error("{field} path must be absolute: {path}")]
    PathNotAbsolute { field: &'static str, path: PathBuf },
    /// A required path cannot be represented exactly in an argument vector or JSON string.
    #[error("{field} path must be valid UTF-8")]
    PathNotUtf8 { field: &'static str },
    /// A package identifier would escape its stable package-results directory.
    #[error("package id `{package}` is not one safe path component")]
    UnsafePackageId { package: String },
    /// The derived durable result path is located in a disposable worktree.
    #[error("package result path {result} is inside worktree {worktree}")]
    ResultInsideWorktree { result: PathBuf, worktree: PathBuf },
    /// The stopping timestamp is not RFC 3339.
    #[error("stopped_at is not RFC 3339: {value}")]
    MalformedStoppedAt { value: String },
    /// Package-worker result bytes are not the exact strict JSON carrier.
    #[error("package worker result is invalid JSON: {source}")]
    InvalidJson { source: serde_json::Error },
    /// Package-worker result serialization unexpectedly failed.
    #[error("package worker result serialization failed: {source}")]
    SerializeJson { source: serde_json::Error },
    /// The wrapper worker command omitted its executable.
    #[error("worker argv must begin with a non-empty executable")]
    EmptyWorkerArguments,
}

fn normalized_absolute(
    path: &Path,
    field: &'static str,
) -> Result<PathBuf, PackageCompletionError> {
    if !path.is_absolute() {
        return Err(PackageCompletionError::PathNotAbsolute {
            field,
            path: path.to_owned(),
        });
    }
    if path.to_str().is_none() {
        return Err(PackageCompletionError::PathNotUtf8 { field });
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    Ok(normalized)
}

/// Derive one restart-stable package result path outside every supplied worktree.
///
/// # Errors
///
/// Returns [`PackageCompletionError`] when any path is relative or non-UTF-8, the package ID is not
/// one safe path component, or the derived result is equal to or below a supplied worktree path.
pub fn derive_package_result_path(
    vision_directory: &Path,
    package: &WorkPackageId,
    sequence: Sequence,
    worktree_paths: &[PathBuf],
) -> Result<AbsolutePackageResultPath, PackageCompletionError> {
    let vision_directory = normalized_absolute(vision_directory, "vision directory")?;
    let package_id = package.as_str();
    if package_id.is_empty()
        || Path::new(package_id).components().count() != 1
        || matches!(
            Path::new(package_id).components().next(),
            Some(
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        )
    {
        return Err(PackageCompletionError::UnsafePackageId {
            package: package_id.to_owned(),
        });
    }
    let result = vision_directory
        .join(".pce/package-results")
        .join(package_id)
        .join(format!("{}.json", sequence.get()));
    for worktree in worktree_paths {
        let worktree = normalized_absolute(worktree, "worktree")?;
        if result.starts_with(&worktree) {
            return Err(PackageCompletionError::ResultInsideWorktree { result, worktree });
        }
    }
    AbsolutePackageResultPath::parse(result)
}

/// Parse one strict package-worker result JSON document.
///
/// # Errors
///
/// Returns [`PackageCompletionError`] for malformed JSON, unknown fields, invalid enum carriers,
/// or a non-RFC-3339 `stopped_at` value.
pub fn parse_package_worker_result(
    bytes: &[u8],
) -> Result<PackageWorkerResult, PackageCompletionError> {
    serde_json::from_slice(bytes).map_err(|source| PackageCompletionError::InvalidJson { source })
}

/// Serialize one package-worker result to compact JSON bytes.
///
/// # Errors
///
/// Returns [`PackageCompletionError::SerializeJson`] if serialization fails.
pub fn serialize_package_worker_result(
    result: &PackageWorkerResult,
) -> Result<Vec<u8>, PackageCompletionError> {
    serde_json::to_vec(result).map_err(|source| PackageCompletionError::SerializeJson { source })
}

/// Compose the exact shell-free hidden package-worker argument vector, including `argv[0]`.
///
/// # Errors
///
/// Returns [`PackageCompletionError`] when the wrapper is not an absolute UTF-8 path or the worker
/// argument vector has no non-empty executable.
pub fn compose_package_worker_argv(
    wrapper_executable: &Path,
    result_path: &AbsolutePackageResultPath,
    required_artifact_path: &AbsoluteRequiredArtifactPath,
    worker_argv: &[String],
) -> Result<Vec<String>, PackageCompletionError> {
    let wrapper = normalized_absolute(wrapper_executable, "wrapper executable")?;
    let wrapper = wrapper
        .to_str()
        .ok_or(PackageCompletionError::PathNotUtf8 {
            field: "wrapper executable",
        })?;
    if worker_argv
        .first()
        .is_none_or(|argument| argument.is_empty())
    {
        return Err(PackageCompletionError::EmptyWorkerArguments);
    }
    let mut argv = vec![
        wrapper.to_owned(),
        "dispatch".to_owned(),
        "package-worker".to_owned(),
        "--result".to_owned(),
        result_path.as_str().to_owned(),
        "--required-artifact".to_owned(),
        required_artifact_path.as_str().to_owned(),
        "--".to_owned(),
    ];
    argv.extend(worker_argv.iter().cloned());
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::{
        AbsoluteRequiredArtifactPath, DispatchExitStatus, ExitCode, RequiredArtifactPresence,
        Sequence, SignalNumber, parse_work_package_graph,
    };

    use super::{
        AbsolutePackageResultPath, PackageCompletionError, SurvivingProcesses,
        compose_package_worker_argv, derive_package_result_path, parse_package_worker_result,
        serialize_package_worker_result,
    };

    fn package() -> crate::WorkPackageId {
        let graph = parse_work_package_graph(br#"{"vision":"v","plan_version":1,"authored_at_ref":"main","packages":[{"id":"WP4","title":"completion","repositories":["pce"],"criteria":[{"name":"test","input":"tree","observation":"green","command":"true"}],"depends_on":[]}]}"#)
            .unwrap_or_else(|error| panic!("{error}"));
        graph.packages()[0].id().clone()
    }

    #[test]
    fn derives_path_outside_two_disposable_worktrees() {
        let path = derive_package_result_path(
            Path::new("/visions/2026-08-12-dispatch"),
            &package(),
            Sequence::parse(17).unwrap_or_else(|error| panic!("{error}")),
            &[
                PathBuf::from("/tmp/worktrees/pce"),
                PathBuf::from("/tmp/worktrees/herdr"),
            ],
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            path.as_path(),
            Path::new("/visions/2026-08-12-dispatch/.pce/package-results/WP4/17.json")
        );
    }

    #[test]
    fn rejects_result_below_either_supplied_worktree() {
        let error = derive_package_result_path(
            Path::new("/tmp/worktrees/herdr/vision"),
            &package(),
            Sequence::first(),
            &[
                PathBuf::from("/tmp/worktrees/pce"),
                PathBuf::from("/tmp/worktrees/herdr"),
            ],
        )
        .expect_err("durable result must remain outside both worktrees");
        assert!(matches!(
            error,
            PackageCompletionError::ResultInsideWorktree { .. }
        ));
    }

    #[test]
    fn parses_both_exit_status_carriers_and_rejects_unknown_fields() {
        let exited = parse_package_worker_result(br#"{"duration_ms":9,"stopped_at":"2026-08-12T10:11:12Z","exit_status":{"kind":"exited","code":3},"required_artifact_presence":"present","surviving_processes":"absent"}"#)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            exited.exit_status(),
            DispatchExitStatus::Exited {
                code: ExitCode::new(3)
            }
        );
        assert_eq!(
            exited.required_artifact_presence(),
            RequiredArtifactPresence::Present
        );
        assert_eq!(exited.surviving_processes(), SurvivingProcesses::Absent);

        let signaled = parse_package_worker_result(br#"{"duration_ms":10,"stopped_at":"2026-08-12T10:11:12+00:00","exit_status":{"kind":"signaled","signal":15},"required_artifact_presence":"absent","surviving_processes":"present"}"#)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            signaled.exit_status(),
            DispatchExitStatus::Signaled {
                signal: SignalNumber::new(15)
            }
        );
        assert_eq!(signaled.surviving_processes(), SurvivingProcesses::Present);
        let serialized =
            serialize_package_worker_result(&signaled).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            parse_package_worker_result(&serialized).unwrap_or_else(|error| panic!("{error}")),
            signaled
        );
        let legacy = parse_package_worker_result(br#"{"duration_ms":9,"stopped_at":"2026-08-12T10:11:12Z","exit_status":{"kind":"exited","code":0},"required_artifact_presence":"present"}"#)
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(legacy.surviving_processes(), SurvivingProcesses::Unknown);

        assert!(parse_package_worker_result(br#"{"duration_ms":9,"stopped_at":"2026-08-12T10:11:12Z","exit_status":{"kind":"exited","code":0},"required_artifact_presence":"present","surviving_processes":"absent","extra":true}"#).is_err());
    }

    #[test]
    fn composes_exact_hidden_wrapper_argv() {
        let result = AbsolutePackageResultPath::parse("/vision/.pce/package-results/WP4/17.json")
            .unwrap_or_else(|error| panic!("{error}"));
        let artifact = AbsoluteRequiredArtifactPath::parse("/tmp/worktrees/pce/verdict.json")
            .unwrap_or_else(|error| panic!("{error}"));
        let argv = compose_package_worker_argv(
            Path::new("/usr/local/bin/pce"),
            &result,
            &artifact,
            &["prime-agent".to_owned(), "-p".to_owned(), "work".to_owned()],
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            argv,
            [
                "/usr/local/bin/pce",
                "dispatch",
                "package-worker",
                "--result",
                "/vision/.pce/package-results/WP4/17.json",
                "--required-artifact",
                "/tmp/worktrees/pce/verdict.json",
                "--",
                "prime-agent",
                "-p",
                "work",
            ]
        );
    }
}
