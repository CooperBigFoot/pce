//! herdr_dispatch_plan : Vision × WorkPackage × DispatchAttempt × RepositoryDispatchInput* × WorkerEnvironment × WorkerArgv → HerdrWorkPackageDispatchPlan
//! pane_close : PaneId → HerdrInvocation
//!
//! The result is a pure, ordered description of Herdr worktree creation followed by one agent
//! start, plus exact cleanup composition for the pane returned by that start. This module performs
//! no I/O.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{WorkPackage, WorkPackageId};

/// A positive identity for one attempt to dispatch a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchAttempt(u64);

impl DispatchAttempt {
    /// Parse a first-based attempt identity.
    ///
    /// # Errors
    ///
    /// Returns [`HerdrDispatchPlanError::ZeroAttempt`] when `value` is zero.
    pub fn parse(value: u64) -> Result<Self, HerdrDispatchPlanError> {
        if value == 0 {
            return Err(HerdrDispatchPlanError::ZeroAttempt);
        }
        Ok(Self(value))
    }

    /// Return the numeric attempt identity.
    pub const fn get(self) -> u64 {
        self.0
    }
}

const ENV_EXECUTABLE: &str = "/usr/bin/env";
const HERDR_EXECUTABLE: &str = "herdr";

/// A non-empty vision identity used to derive package branches and dispatch names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchVisionSource(String);

impl DispatchVisionSource {
    /// Parse a non-empty vision identity without path separators.
    ///
    /// # Errors
    ///
    /// Returns [`HerdrDispatchPlanError::InvalidIdentity`] when the identity is unsafe in a branch.
    pub fn parse(value: impl Into<String>) -> Result<Self, HerdrDispatchPlanError> {
        let value = value.into();
        let valid = !value.is_empty()
            && !value.starts_with('-')
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        if !valid {
            return Err(HerdrDispatchPlanError::InvalidIdentity {
                field: "vision",
                value,
            });
        }
        Ok(Self(value))
    }

    /// Return the exact validated identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An absolute root under which package worktrees are placed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteWorktreeRoot(PathBuf);

impl AbsoluteWorktreeRoot {
    /// Parse an absolute worktree root.
    ///
    /// # Errors
    ///
    /// Returns [`HerdrDispatchPlanError::PathNotAbsolute`] for a relative path.
    pub fn parse(path: PathBuf) -> Result<Self, HerdrDispatchPlanError> {
        if !path.is_absolute() {
            return Err(HerdrDispatchPlanError::PathNotAbsolute {
                field: "worktree root",
                path,
            });
        }
        Ok(Self(path))
    }
}

/// An absolute binary-selected temporary directory passed to the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbsoluteDispatchTemporaryDirectory(PathBuf);

impl AbsoluteDispatchTemporaryDirectory {
    /// Parse an absolute binary-selected temporary directory.
    ///
    /// # Errors
    ///
    /// Returns [`HerdrDispatchPlanError::PathNotAbsolute`] for a relative path.
    pub fn parse(path: PathBuf) -> Result<Self, HerdrDispatchPlanError> {
        if !path.is_absolute() {
            return Err(HerdrDispatchPlanError::PathNotAbsolute {
                field: "dispatch temporary directory",
                path,
            });
        }
        Ok(Self(path))
    }
}

/// The repository authority needed to create one package worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryDispatchInput {
    repository: String,
    source_checkout: PathBuf,
    base_ref: String,
}

impl RepositoryDispatchInput {
    /// Construct one repository dispatch authority.
    ///
    /// # Errors
    ///
    /// Rejects empty names, branches, and refs, and relative source checkouts.
    pub fn parse(
        repository: impl Into<String>,
        source_checkout: PathBuf,
        base_ref: impl Into<String>,
    ) -> Result<Self, HerdrDispatchPlanError> {
        let repository = repository.into();
        let base_ref = base_ref.into();
        if repository.trim().is_empty() {
            return Err(HerdrDispatchPlanError::EmptyValue {
                field: "repository",
            });
        }
        if !source_checkout.is_absolute() {
            return Err(HerdrDispatchPlanError::PathNotAbsolute {
                field: "repository source checkout",
                path: source_checkout,
            });
        }
        if base_ref.trim().is_empty() {
            return Err(HerdrDispatchPlanError::EmptyValue { field: "base ref" });
        }
        Ok(Self {
            repository,
            source_checkout,
            base_ref,
        })
    }
}

/// Explicit worker environment entries supplied by the composition root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerEnvironment(BTreeMap<String, String>);

impl WorkerEnvironment {
    /// Construct an explicit environment, reserving package-owned bindings.
    ///
    /// # Errors
    ///
    /// Rejects empty names and binary-owned `TMPDIR` or `PCE_WORKTREE_*` entries.
    pub fn parse(entries: BTreeMap<String, String>) -> Result<Self, HerdrDispatchPlanError> {
        for name in entries.keys() {
            if name.is_empty() {
                return Err(HerdrDispatchPlanError::EmptyValue {
                    field: "environment name",
                });
            }
            if !matches!(name.as_str(), "PATH" | "HOME" | "USER") {
                return Err(HerdrDispatchPlanError::ReservedEnvironment { name: name.clone() });
            }
        }
        Ok(Self(entries))
    }
}

/// A non-empty worker program and argument vector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerArgumentVector(Vec<String>);

impl WorkerArgumentVector {
    /// Parse a worker argv whose first element names its executable.
    ///
    /// # Errors
    ///
    /// Rejects an absent or empty executable.
    pub fn parse(arguments: Vec<String>) -> Result<Self, HerdrDispatchPlanError> {
        if arguments.first().is_none_or(|argument| argument.is_empty()) {
            return Err(HerdrDispatchPlanError::EmptyWorkerArguments);
        }
        Ok(Self(arguments))
    }
}

/// A deterministic Herdr-valid live-agent identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrAgentName(String);

impl HerdrAgentName {
    /// Return the exact Herdr spelling.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An opaque workspace identifier returned by Herdr worktree creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrWorkspaceId(String);

impl HerdrWorkspaceId {
    /// Parse a non-empty opaque Herdr workspace identifier.
    ///
    /// # Errors
    ///
    /// Rejects an empty identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, HerdrDispatchPlanError> {
        let value = value.into();
        if value.is_empty() {
            return Err(HerdrDispatchPlanError::EmptyValue {
                field: "Herdr workspace id",
            });
        }
        Ok(Self(value))
    }

    /// Return the exact workspace identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compose closure of the exact worktree workspace containing the run-created root pane.
    pub fn close_invocation(&self) -> HerdrInvocation {
        HerdrInvocation {
            argv: vec!["workspace".to_owned(), "close".to_owned(), self.0.clone()],
        }
    }
}

/// An opaque tab identifier returned by Herdr worktree creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrTabId(String);

impl HerdrTabId {
    /// Parse a non-empty opaque Herdr tab identifier.
    ///
    /// # Errors
    ///
    /// Rejects an empty identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, HerdrDispatchPlanError> {
        let value = value.into();
        if value.is_empty() {
            return Err(HerdrDispatchPlanError::EmptyValue {
                field: "Herdr tab id",
            });
        }
        Ok(Self(value))
    }
}

/// An opaque pane identifier observed in Herdr's pane inventory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HerdrPaneId(String);

impl HerdrPaneId {
    /// Parse a non-empty opaque pane identifier.
    ///
    /// # Errors
    ///
    /// Rejects an empty identifier.
    pub fn parse(value: impl Into<String>) -> Result<Self, HerdrDispatchPlanError> {
        let value = value.into();
        if value.is_empty() {
            return Err(HerdrDispatchPlanError::EmptyValue {
                field: "Herdr pane id",
            });
        }
        Ok(Self(value))
    }

    /// Return the exact identifier accepted by `herdr pane close`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Compose the exact pane-close invocation.
    pub fn close_invocation(&self) -> HerdrInvocation {
        HerdrInvocation {
            argv: vec!["pane".to_owned(), "close".to_owned(), self.0.clone()],
        }
    }
}

/// The runtime location read from a Herdr worktree-create JSON response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrAgentLocation {
    workspace: HerdrWorkspaceId,
    tab: HerdrTabId,
}

impl HerdrAgentLocation {
    /// Bind the opaque workspace and tab returned by Herdr.
    pub const fn new(workspace: HerdrWorkspaceId, tab: HerdrTabId) -> Self {
        Self { workspace, tab }
    }

    /// Return the exact workspace identifier.
    pub fn workspace_id(&self) -> &str {
        &self.workspace.0
    }

    /// Return the exact tab identifier.
    pub fn tab_id(&self) -> &str {
        &self.tab.0
    }
}

/// One direct, shell-free CLI invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrInvocation {
    argv: Vec<String>,
}

impl HerdrInvocation {
    /// The Herdr control executable selected by this protocol.
    pub const fn executable(&self) -> &'static str {
        HERDR_EXECUTABLE
    }
    /// Borrow arguments excluding argv[0].
    pub fn argv(&self) -> &[String] {
        &self.argv
    }
}

/// One repository worktree and its exact creation invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrWorktreeSpec {
    repository: String,
    path: PathBuf,
    invocation: HerdrInvocation,
}

impl HerdrWorktreeSpec {
    /// Return the graph repository name.
    pub fn repository(&self) -> &str {
        &self.repository
    }
    /// Return the binary-selected package worktree path.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Return the exact `herdr worktree create` invocation.
    pub const fn invocation(&self) -> &HerdrInvocation {
        &self.invocation
    }
}

/// Pure two-phase dispatch composition: create all worktrees, then start in returned location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrWorkPackageDispatchPlan {
    agent_name: HerdrAgentName,
    worktrees: Vec<HerdrWorktreeSpec>,
    environment: BTreeMap<String, String>,
    worker_arguments: WorkerArgumentVector,
}

impl HerdrWorkPackageDispatchPlan {
    /// Return the restart-stable agent identity.
    pub const fn agent_name(&self) -> &HerdrAgentName {
        &self.agent_name
    }
    /// Return one worktree spec per package repository, in package order.
    pub fn worktrees(&self) -> &[HerdrWorktreeSpec] {
        &self.worktrees
    }
    /// Return the complete constructed worker environment.
    pub const fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }
    /// Compose the installed Herdr 0.7.1 agent-start command using opaque returned IDs.
    ///
    /// No `--split` is emitted: worktree creation established the location.
    pub fn agent_start(&self, location: &HerdrAgentLocation) -> HerdrInvocation {
        let assignments = self
            .environment
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>();
        let mut argv = vec![
            "agent".to_owned(),
            "start".to_owned(),
            self.agent_name.0.clone(),
            "--cwd".to_owned(),
            self.worktrees[0].path.display().to_string(),
            "--workspace".to_owned(),
            location.workspace.0.clone(),
            "--tab".to_owned(),
            location.tab.0.clone(),
            "--no-focus".to_owned(),
            "--".to_owned(),
            ENV_EXECUTABLE.to_owned(),
            "-i".to_owned(),
        ];
        argv.extend(assignments);
        argv.extend(self.worker_arguments.0.iter().cloned());
        HerdrInvocation { argv }
    }
}

/// Pure dispatch composition failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HerdrDispatchPlanError {
    /// A branch-bearing identity is outside the closed safe grammar.
    #[error("{field} identity `{value}` must contain only ASCII letters, digits, `_`, or `-`")]
    InvalidIdentity { field: &'static str, value: String },
    /// A required string was empty.
    #[error("{field} must be non-empty")]
    EmptyValue { field: &'static str },
    /// A domain path was not absolute.
    #[error("{field} must be absolute: {path}")]
    PathNotAbsolute { field: &'static str, path: PathBuf },
    /// The caller attempted to replace a binary-owned environment binding.
    #[error("environment name `{name}` is binary-owned")]
    ReservedEnvironment { name: String },
    /// The deterministic PCE_WORKTREES JSON projection unexpectedly failed.
    #[error("PCE_WORKTREES JSON serialization failed: {message}")]
    WorktreesJson { message: String },
    /// A dispatch attempt was zero, which cannot identify an attempt.
    #[error("dispatch attempt must be positive")]
    ZeroAttempt,
    /// The worker argv had no executable.
    #[error("worker arguments must begin with a non-empty executable")]
    EmptyWorkerArguments,
    /// A repository authority did not correspond exactly to one package repository.
    #[error("repository dispatch input `{repository}` is not unique and exact for the package")]
    RepositoryInputMismatch { repository: String },
    /// A package repeated a repository, which would violate one-worktree-per-repository identity.
    #[error("package repeats repository `{repository}`")]
    DuplicatePackageRepository { repository: String },
    /// More than one newly observed pane matched the exact agent-start location and cwd.
    #[error("{count} newly observed panes match the run-owned agent-start identity")]
    AmbiguousCreatedPane { count: usize },
}

/// Derive the stable agent name from length-framed vision and package identity.
fn derive_agent_name(
    vision: &DispatchVisionSource,
    package: &WorkPackageId,
    attempt: DispatchAttempt,
) -> HerdrAgentName {
    let mut digest = Sha256::new();
    digest.update(vision.as_str().len().to_be_bytes());
    digest.update(vision.as_str().as_bytes());
    digest.update(package.as_str().len().to_be_bytes());
    digest.update(package.as_str().as_bytes());
    digest.update(attempt.get().to_be_bytes());
    let bytes = digest.finalize();
    let suffix = bytes[..14]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    HerdrAgentName(format!("pce-{suffix}"))
}

fn worktree_component(index: usize, repository: &str) -> String {
    let slug = repository
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("{index:02}-{}", slug.trim_matches('-'))
}

/// Compose the exact offline-checkable Herdr dispatch protocol for one work package.
///
/// # Errors
///
/// Returns [`HerdrDispatchPlanError`] unless repository inputs are a one-to-one match for the
/// package repository set.
pub fn compose_herdr_work_package_dispatch(
    vision: &DispatchVisionSource,
    package: &WorkPackage,
    attempt: DispatchAttempt,
    repositories: &[RepositoryDispatchInput],
    worktree_root: &AbsoluteWorktreeRoot,
    temporary_directory: &AbsoluteDispatchTemporaryDirectory,
    environment: WorkerEnvironment,
    worker_arguments: WorkerArgumentVector,
) -> Result<HerdrWorkPackageDispatchPlan, HerdrDispatchPlanError> {
    let mut package_repositories = HashSet::new();
    for repository in package.repositories() {
        if !package_repositories.insert(repository.as_str()) {
            return Err(HerdrDispatchPlanError::DuplicatePackageRepository {
                repository: repository.clone(),
            });
        }
    }
    let mut by_name = HashMap::new();
    for repository in repositories {
        if by_name
            .insert(repository.repository.as_str(), repository)
            .is_some()
            || !package_repositories.contains(repository.repository.as_str())
        {
            return Err(HerdrDispatchPlanError::RepositoryInputMismatch {
                repository: repository.repository.clone(),
            });
        }
    }
    for repository in package.repositories() {
        if !by_name.contains_key(repository.as_str()) {
            return Err(HerdrDispatchPlanError::RepositoryInputMismatch {
                repository: repository.clone(),
            });
        }
    }

    let agent_name = derive_agent_name(vision, package.id(), attempt);
    let package_root = worktree_root.0.join(agent_name.as_str());
    let mut worktrees = Vec::with_capacity(package.repositories().len());
    for (index, repository_name) in package.repositories().iter().enumerate() {
        let repository = by_name[repository_name.as_str()];
        let path = package_root.join(worktree_component(index, repository_name));
        let label = format!(
            "{}:{}:attempt-{}",
            package.id().as_str(),
            repository_name,
            attempt.get()
        );
        let argv = vec![
            "worktree".to_owned(),
            "create".to_owned(),
            "--cwd".to_owned(),
            repository.source_checkout.display().to_string(),
            "--branch".to_owned(),
            format!(
                "pce/{}/{}/attempt-{}",
                vision.as_str(),
                package.id().as_str(),
                attempt.get()
            ),
            "--base".to_owned(),
            repository.base_ref.clone(),
            "--path".to_owned(),
            path.display().to_string(),
            "--label".to_owned(),
            label,
            "--no-focus".to_owned(),
            "--json".to_owned(),
        ];
        worktrees.push(HerdrWorktreeSpec {
            repository: repository_name.clone(),
            path,
            invocation: HerdrInvocation { argv },
        });
    }

    let mut environment = environment.0;
    environment.insert(
        "TMPDIR".to_owned(),
        temporary_directory.0.display().to_string(),
    );
    let paths = worktrees
        .iter()
        .map(|worktree| worktree.path.display().to_string())
        .collect::<Vec<_>>();
    // Serializing a vector of strings cannot fail; retain the impossible case as a loud domain error.
    let worktrees_json =
        serde_json::to_string(&paths).map_err(|source| HerdrDispatchPlanError::WorktreesJson {
            message: source.to_string(),
        })?;
    environment.insert("PCE_WORKTREES".to_owned(), worktrees_json);
    for (index, path) in paths.iter().enumerate() {
        environment.insert(format!("PCE_WORKTREE_{index}"), path.clone());
    }

    Ok(HerdrWorkPackageDispatchPlan {
        agent_name,
        worktrees,
        environment,
        worker_arguments,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use crate::parse_work_package_graph;

    use super::{
        AbsoluteDispatchTemporaryDirectory, AbsoluteWorktreeRoot, DispatchAttempt,
        DispatchVisionSource, HerdrAgentLocation, HerdrTabId, HerdrWorkspaceId,
        RepositoryDispatchInput, WorkerArgumentVector, WorkerEnvironment,
        compose_herdr_work_package_dispatch,
    };

    fn package_graph(repositories: &[&str]) -> crate::WorkPackageGraph {
        let repositories = repositories
            .iter()
            .map(|repository| format!(r#""{repository}""#))
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(
            r#"{{"vision":"vision-one","plan_version":1,"authored_at_ref":"main","packages":[{{"id":"WP4","title":"dispatch","repositories":[{repositories}],"criteria":[{{"name":"test","input":"tree","observation":"green","command":"true"}}],"depends_on":[]}}]}}"#
        );
        parse_work_package_graph(json.as_bytes()).unwrap_or_else(|error| panic!("{error}"))
    }

    fn repository(name: &str) -> RepositoryDispatchInput {
        RepositoryDispatchInput::parse(name, PathBuf::from(format!("/repos/{name}")), "main")
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn compose(
        repositories: &[&str],
        vision: &str,
        attempt: u64,
    ) -> super::HerdrWorkPackageDispatchPlan {
        let graph = package_graph(repositories);
        let inputs = repositories
            .iter()
            .map(|name| repository(name))
            .collect::<Vec<_>>();
        compose_herdr_work_package_dispatch(
            &DispatchVisionSource::parse(vision).unwrap_or_else(|error| panic!("{error}")),
            &graph.packages()[0],
            DispatchAttempt::parse(attempt).unwrap_or_else(|error| panic!("{error}")),
            &inputs,
            &AbsoluteWorktreeRoot::parse(PathBuf::from("/worktrees"))
                .unwrap_or_else(|error| panic!("{error}")),
            &AbsoluteDispatchTemporaryDirectory::parse(PathBuf::from("/binary/tmp"))
                .unwrap_or_else(|error| panic!("{error}")),
            WorkerEnvironment::parse(BTreeMap::from([
                ("HOME".to_owned(), "/home/worker".to_owned()),
                ("PATH".to_owned(), "/usr/bin".to_owned()),
                ("USER".to_owned(), "worker".to_owned()),
            ]))
            .unwrap_or_else(|error| panic!("{error}")),
            WorkerArgumentVector::parse(vec!["prime-agent".to_owned(), "-p".to_owned()])
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn single_repository_composes_exact_argv_environment_name_and_tmpdir() {
        let plan = compose(&["pce"], "vision-one", 1);
        let target = format!("/worktrees/{}/00-pce", plan.agent_name().as_str());
        assert_eq!(plan.worktrees().len(), 1);
        assert_eq!(plan.worktrees()[0].path(), Path::new(&target));
        assert_eq!(plan.worktrees()[0].invocation().executable(), "herdr");
        assert_eq!(
            plan.worktrees()[0].invocation().argv(),
            [
                "worktree",
                "create",
                "--cwd",
                "/repos/pce",
                "--branch",
                "pce/vision-one/WP4/attempt-1",
                "--base",
                "main",
                "--path",
                &target,
                "--label",
                "WP4:pce:attempt-1",
                "--no-focus",
                "--json",
            ]
        );
        assert_eq!(
            plan.environment(),
            &BTreeMap::from([
                ("HOME".to_owned(), "/home/worker".to_owned()),
                ("PATH".to_owned(), "/usr/bin".to_owned()),
                ("PCE_WORKTREES".to_owned(), format!("[\"{target}\"]")),
                ("PCE_WORKTREE_0".to_owned(), target.clone()),
                ("TMPDIR".to_owned(), "/binary/tmp".to_owned()),
                ("USER".to_owned(), "worker".to_owned()),
            ])
        );
        let start = plan.agent_start(&HerdrAgentLocation::new(
            HerdrWorkspaceId::parse("w9").unwrap_or_else(|error| panic!("{error}")),
            HerdrTabId::parse("w9:t2").unwrap_or_else(|error| panic!("{error}")),
        ));
        assert_eq!(start.executable(), "herdr");
        assert_eq!(
            start.argv(),
            [
                "agent",
                "start",
                plan.agent_name().as_str(),
                "--cwd",
                &target,
                "--workspace",
                "w9",
                "--tab",
                "w9:t2",
                "--no-focus",
                "--",
                "/usr/bin/env",
                "-i",
                "HOME=/home/worker",
                "PATH=/usr/bin",
                &format!("PCE_WORKTREES=[\"{target}\"]"),
                &format!("PCE_WORKTREE_0={target}"),
                "TMPDIR=/binary/tmp",
                "USER=worker",
                "prime-agent",
                "-p",
            ]
        );
        assert!(!start.argv().iter().any(|argument| argument == "--split"));
    }

    #[test]
    fn two_repositories_have_exact_paths_json_and_indexed_environment() {
        let plan = compose(&["pce", "herdr"], "vision-one", 1);
        let first = format!("/worktrees/{}/00-pce", plan.agent_name().as_str());
        let second = format!("/worktrees/{}/01-herdr", plan.agent_name().as_str());
        assert_eq!(
            plan.worktrees()
                .iter()
                .map(|worktree| worktree.repository())
                .collect::<Vec<_>>(),
            ["pce", "herdr"]
        );
        assert_eq!(
            plan.environment()["PCE_WORKTREES"],
            format!("[\"{first}\",\"{second}\"]")
        );
        assert_eq!(plan.environment()["PCE_WORKTREE_0"], first);
        assert_eq!(plan.environment()["PCE_WORKTREE_1"], second);
        assert_eq!(plan.environment()["TMPDIR"], "/binary/tmp");
        assert_eq!(
            plan.worktrees()[1].invocation().argv(),
            [
                "worktree",
                "create",
                "--cwd",
                "/repos/herdr",
                "--branch",
                "pce/vision-one/WP4/attempt-1",
                "--base",
                "main",
                "--path",
                &format!("/worktrees/{}/01-herdr", plan.agent_name().as_str()),
                "--label",
                "WP4:herdr:attempt-1",
                "--no-focus",
                "--json",
            ]
        );
    }

    #[test]
    fn agent_name_is_sha256_hex_truncated_grammar_bounded_and_attempt_distinct() {
        let first = compose(&["pce"], "vision-one", 1);
        let repeated = compose(&["pce"], "vision-one", 1);
        let retry = compose(&["pce"], "vision-one", 2);
        let other = compose(&["pce"], "vision-two", 1);
        assert_eq!(first.agent_name(), repeated.agent_name());
        assert_ne!(first.agent_name(), retry.agent_name());
        assert_ne!(first.agent_name(), other.agent_name());
        assert_ne!(first.worktrees()[0].path(), retry.worktrees()[0].path());
        assert_eq!(
            retry.worktrees()[0].invocation().argv()[5],
            "pce/vision-one/WP4/attempt-2"
        );
        let name = first.agent_name().as_str();
        assert_eq!(name.len(), 32);
        assert!(name.starts_with("pce-"));
        assert!(
            name[4..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        );
        assert_ne!(first.worktrees()[0].path(), other.worktrees()[0].path());
    }

    #[test]
    fn only_explicit_operator_environment_names_are_accepted() {
        let error = WorkerEnvironment::parse(BTreeMap::from([(
            "SHELL".to_owned(),
            "/bin/zsh".to_owned(),
        )]))
        .expect_err("operator environment must not leak");
        assert_eq!(
            error.to_string(),
            "environment name `SHELL` is binary-owned"
        );
        let empty =
            WorkerEnvironment::parse(BTreeMap::new()).unwrap_or_else(|error| panic!("{error}"));
        assert!(empty.0.is_empty());
    }
    #[test]
    fn pane_close_targets_only_the_exact_returned_opaque_identity() {
        let pane = super::HerdrPaneId::parse("w9:p73").unwrap_or_else(|error| panic!("{error}"));
        let close = pane.close_invocation();
        assert_eq!(close.executable(), "herdr");
        assert_eq!(close.argv(), ["pane", "close", "w9:p73"]);
        assert!(!close.argv().iter().any(|argument| argument == "unrelated"));

        let workspace =
            super::HerdrWorkspaceId::parse("w9").unwrap_or_else(|error| panic!("{error}"));
        let workspace_close = workspace.close_invocation();
        assert_eq!(workspace_close.argv(), ["workspace", "close", "w9"]);
        assert!(
            !workspace_close
                .argv()
                .iter()
                .any(|argument| argument == "w8")
        );
    }
}
