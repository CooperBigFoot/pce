//! herdr_dispatch_plan : Vision × WorkPackage × DispatchAttempt × HerdrSession? × RepositoryDispatchInput* × WorkerEnvironment × WorkerArgv → HerdrWorkPackageDispatchPlan
//! pane_close : PaneId × HerdrSession? → HerdrInvocation
//! workspace_close : WorkspaceId × HerdrSession? → HerdrInvocation
//!
//! The result is a pure, ordered description of Herdr worktree creation followed by one command
//! run in the first worktree's root pane, plus exact cleanup composition for that pane. This module
//! performs no I/O.

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
const MAX_HERDR_SESSION_NAME_LEN: usize = 64;
// macOS has the smaller supported sun_path field (104 bytes versus Linux's 108). Reserve one
// byte for the terminating NUL so a session remains usable when a repository moves between them.
const PORTABLE_UNIX_SOCKET_PATH_CAPACITY: usize = 104;

/// A validated Herdr session selected for all objects and observations in one dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrSessionName(String);

impl HerdrSessionName {
    /// Parse the session-name grammar supported by Herdr 0.8.x and its derived socket path.
    ///
    /// # Errors
    ///
    /// Returns [`HerdrDispatchPlanError::InvalidSessionName`] for a name Herdr would reject, or
    /// [`HerdrDispatchPlanError::SessionSocketPathTooLong`] when Herdr could not bind the path produced by the supplied
    /// derivation on every supported Unix platform.
    pub fn parse(
        value: impl Into<String>,
        socket_path_for: impl FnOnce(&str) -> PathBuf,
    ) -> Result<Self, HerdrDispatchPlanError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= MAX_HERDR_SESSION_NAME_LEN
            && value != "."
            && value != ".."
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        if !valid {
            return Err(HerdrDispatchPlanError::InvalidSessionName { value });
        }
        let socket_path = socket_path_for(&value);
        let path_length = socket_path.as_os_str().as_encoded_bytes().len();
        if path_length.saturating_add(1) > PORTABLE_UNIX_SOCKET_PATH_CAPACITY {
            return Err(HerdrDispatchPlanError::SessionSocketPathTooLong {
                value,
                socket_path,
                path_length,
                limit: PORTABLE_UNIX_SOCKET_PATH_CAPACITY,
            });
        }
        Ok(Self(value))
    }

    /// Return the exact validated session name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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
            let mut bytes = name.bytes();
            let valid_start = bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
            if !valid_start || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
                return Err(HerdrDispatchPlanError::InvalidEnvironmentName { name: name.clone() });
            }
            if matches!(
                name.as_str(),
                "TMPDIR" | "PCE_DISPATCH_TMPDIR" | "PCE_WORKTREES"
            ) || name.starts_with("PCE_WORKTREE_")
            {
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
    pub fn close_invocation(&self, session: Option<&HerdrSessionName>) -> HerdrInvocation {
        HerdrInvocation::compose(
            session,
            vec!["workspace".to_owned(), "close".to_owned(), self.0.clone()],
        )
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
    pub fn close_invocation(&self, session: Option<&HerdrSessionName>) -> HerdrInvocation {
        HerdrInvocation::compose(
            session,
            vec!["pane".to_owned(), "close".to_owned(), self.0.clone()],
        )
    }
}

/// The runtime location read from a Herdr worktree-create JSON response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrAgentLocation {
    workspace: HerdrWorkspaceId,
    tab: HerdrTabId,
    pane: HerdrPaneId,
}

impl HerdrAgentLocation {
    /// Bind the opaque workspace, tab, and root pane returned by Herdr.
    pub const fn new(workspace: HerdrWorkspaceId, tab: HerdrTabId, pane: HerdrPaneId) -> Self {
        Self {
            workspace,
            tab,
            pane,
        }
    }

    /// Return the exact workspace identifier.
    pub fn workspace_id(&self) -> &str {
        &self.workspace.0
    }

    /// Return the exact tab identifier.
    pub fn tab_id(&self) -> &str {
        &self.tab.0
    }

    /// Return the exact root pane identifier.
    pub const fn pane(&self) -> &HerdrPaneId {
        &self.pane
    }
}

/// One direct Herdr CLI invocation; `pane run` carries quoted shell text as one argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HerdrInvocation {
    argv: Vec<String>,
}

impl HerdrInvocation {
    fn compose(session: Option<&HerdrSessionName>, arguments: Vec<String>) -> Self {
        let mut argv = Vec::with_capacity(arguments.len() + usize::from(session.is_some()) * 2);
        if let Some(session) = session {
            argv.extend(["--session".to_owned(), session.as_str().to_owned()]);
        }
        argv.extend(arguments);
        Self { argv }
    }

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
    session: Option<HerdrSessionName>,
    worktrees: Vec<HerdrWorktreeSpec>,
    environment: BTreeMap<String, String>,
    worker_arguments: WorkerArgumentVector,
    launch_script_path: PathBuf,
}

impl HerdrWorkPackageDispatchPlan {
    /// Return the restart-stable agent identity.
    pub const fn agent_name(&self) -> &HerdrAgentName {
        &self.agent_name
    }
    /// Return the selected Herdr session, or `None` for the default session.
    pub const fn session(&self) -> Option<&HerdrSessionName> {
        self.session.as_ref()
    }
    /// Return one worktree spec per package repository, in package order.
    pub fn worktrees(&self) -> &[HerdrWorktreeSpec] {
        &self.worktrees
    }
    /// Return the complete constructed worker environment.
    pub const fn environment(&self) -> &BTreeMap<String, String> {
        &self.environment
    }
    /// Return the binary-owned launch script path for execution-time materialization.
    pub fn launch_script_path(&self) -> &Path {
        &self.launch_script_path
    }

    /// Compose the launch script bytes without performing I/O.
    ///
    /// The script deletes itself before replacing its process with the scrubbed worker. This keeps
    /// long or sensitive environment values out of terminal input and scrollback.
    pub fn launch_script(&self) -> String {
        let mut command = vec![shell_quote(ENV_EXECUTABLE), "-i".to_owned()];
        command.extend(
            self.environment
                .iter()
                .map(|(name, value)| shell_quote(&format!("{name}={value}"))),
        );
        command.extend(
            self.worker_arguments
                .0
                .iter()
                .map(|argument| shell_quote(argument)),
        );
        format!(
            "#!/bin/sh\nset -eu\nrm -f -- \"$0\"\ncd {}\nexec {}\n",
            shell_quote(&self.worktrees[0].path.display().to_string()),
            command.join(" "),
        )
    }

    /// Compose the Herdr 0.8.2 pane-run command for the first worktree's root pane.
    pub fn pane_run(&self, location: &HerdrAgentLocation) -> HerdrInvocation {
        HerdrInvocation::compose(
            self.session(),
            vec![
                "pane".to_owned(),
                "run".to_owned(),
                location.pane.0.clone(),
                shell_quote(&self.launch_script_path.display().to_string()),
            ],
        )
    }
}

fn shell_quote(argument: &str) -> String {
    if !argument.is_empty()
        && argument
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
    {
        return argument.to_owned();
    }
    format!("'{}'", argument.replace('\'', "'\"'\"'"))
}

/// Pure dispatch composition failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HerdrDispatchPlanError {
    /// A branch-bearing identity is outside the closed safe grammar.
    #[error("{field} identity `{value}` must contain only ASCII letters, digits, `_`, or `-`")]
    InvalidIdentity { field: &'static str, value: String },
    /// A Herdr session name is outside the Herdr 0.8.x grammar.
    #[error(
        "invalid Herdr session name `{value}`; expected 1..=64 ASCII letters, digits, `.`, `_`, or `-`, excluding `.` and `..`"
    )]
    InvalidSessionName { value: String },
    /// A derived Herdr session socket path cannot fit every supported Unix `sun_path` field.
    #[error(
        "Herdr session name `{value}` produces socket path `{socket_path}` with length {path_length} bytes; with its terminating NUL it exceeds the portable Unix socket path limit of {limit} bytes; shorten the session name"
    )]
    SessionSocketPathTooLong {
        value: String,
        socket_path: PathBuf,
        path_length: usize,
        limit: usize,
    },
    /// A required string was empty.
    #[error("{field} must be non-empty")]
    EmptyValue { field: &'static str },
    /// A domain path was not absolute.
    #[error("{field} must be absolute: {path}")]
    PathNotAbsolute { field: &'static str, path: PathBuf },
    /// An environment name cannot be represented as one portable process assignment.
    #[error("environment name `{name}` must match [A-Za-z_][A-Za-z0-9_]*")]
    InvalidEnvironmentName { name: String },
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
}

/// Derive the stable agent name from length-framed vision and package identity.
pub fn derive_herdr_agent_name(
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
/// Returns [`HerdrDispatchPlanError`] unless inputs uniquely contain every package repository.
/// Additional inputs are dependency repositories delivered after the package-owned repositories.
pub fn compose_herdr_work_package_dispatch(
    vision: &DispatchVisionSource,
    package: &WorkPackage,
    attempt: DispatchAttempt,
    session: Option<HerdrSessionName>,
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

    let agent_name = derive_herdr_agent_name(vision, package.id(), attempt);
    let package_root = worktree_root.0.join(agent_name.as_str());
    let mut ordered_repositories = package.repositories().iter().collect::<Vec<_>>();
    ordered_repositories.extend(repositories.iter().filter_map(|input| {
        (!package_repositories.contains(input.repository.as_str())).then_some(&input.repository)
    }));
    let mut worktrees = Vec::with_capacity(ordered_repositories.len());
    for (index, repository_name) in ordered_repositories.into_iter().enumerate() {
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
            invocation: HerdrInvocation::compose(session.as_ref(), argv),
        });
    }

    let mut environment = environment.0;
    environment.insert(
        "TMPDIR".to_owned(),
        temporary_directory.0.display().to_string(),
    );
    environment.insert(
        "PCE_DISPATCH_TMPDIR".to_owned(),
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

    let mut launch_digest = Sha256::new();
    for argument in &worker_arguments.0 {
        launch_digest.update(argument.len().to_be_bytes());
        launch_digest.update(argument.as_bytes());
    }
    let launch_suffix = launch_digest.finalize()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(HerdrWorkPackageDispatchPlan {
        agent_name,
        session,
        worktrees,
        environment,
        worker_arguments,
        launch_script_path: temporary_directory
            .0
            .join(format!("worker-launch-{launch_suffix}.sh")),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use crate::parse_work_package_graph;

    use super::{
        AbsoluteDispatchTemporaryDirectory, AbsoluteWorktreeRoot, DispatchAttempt,
        DispatchVisionSource, HerdrAgentLocation, HerdrSessionName, HerdrTabId, HerdrWorkspaceId,
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
        compose_with_session(repositories, vision, attempt, None)
    }

    fn compose_with_session(
        repositories: &[&str],
        vision: &str,
        attempt: u64,
        session: Option<HerdrSessionName>,
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
            session,
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
                ("PCE_DISPATCH_TMPDIR".to_owned(), "/binary/tmp".to_owned()),
                ("PCE_WORKTREES".to_owned(), format!("[\"{target}\"]")),
                ("PCE_WORKTREE_0".to_owned(), target.clone()),
                ("TMPDIR".to_owned(), "/binary/tmp".to_owned()),
                ("USER".to_owned(), "worker".to_owned()),
            ])
        );
        let run = plan.pane_run(&HerdrAgentLocation::new(
            HerdrWorkspaceId::parse("w9").unwrap_or_else(|error| panic!("{error}")),
            HerdrTabId::parse("w9:t2").unwrap_or_else(|error| panic!("{error}")),
            super::HerdrPaneId::parse("w9:p7").unwrap_or_else(|error| panic!("{error}")),
        ));
        assert_eq!(run.executable(), "herdr");
        assert_eq!(
            run.argv(),
            [
                "pane",
                "run",
                "w9:p7",
                plan.launch_script_path().to_str().unwrap_or("invalid path"),
            ]
        );
        assert!(
            plan.launch_script_path()
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("worker-launch-") && name.ends_with(".sh"))
        );
        assert_eq!(
            plan.launch_script(),
            format!(
                "#!/bin/sh\nset -eu\nrm -f -- \"$0\"\ncd {target}\nexec /usr/bin/env -i HOME=/home/worker PATH=/usr/bin PCE_DISPATCH_TMPDIR=/binary/tmp 'PCE_WORKTREES=[\"{target}\"]' PCE_WORKTREE_0={target} TMPDIR=/binary/tmp USER=worker prime-agent -p\n"
            )
        );
        assert!(!run.argv().iter().any(|argument| argument == "agent"));
    }

    #[test]
    fn configured_session_prefixes_every_composed_invocation() {
        let session = HerdrSessionName::parse("pce-work", |_| {
            PathBuf::from("/Users/operator/.config/herdr/sessions/pce-work/herdr.sock")
        })
        .unwrap_or_else(|error| panic!("{error}"));
        let plan = compose_with_session(&["pce"], "vision-one", 1, Some(session.clone()));
        let target = format!("/worktrees/{}/00-pce", plan.agent_name().as_str());
        assert_eq!(
            plan.worktrees()[0].invocation().argv(),
            [
                "--session",
                "pce-work",
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
        let location = HerdrAgentLocation::new(
            HerdrWorkspaceId::parse("w9").unwrap_or_else(|error| panic!("{error}")),
            HerdrTabId::parse("w9:t2").unwrap_or_else(|error| panic!("{error}")),
            super::HerdrPaneId::parse("w9:p7").unwrap_or_else(|error| panic!("{error}")),
        );
        assert_eq!(
            &plan.pane_run(&location).argv()[..4],
            ["--session", "pce-work", "pane", "run"]
        );
        assert_eq!(
            location.pane().close_invocation(Some(&session)).argv(),
            ["--session", "pce-work", "pane", "close", "w9:p7"]
        );
        let workspace = HerdrWorkspaceId::parse("w9").unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            workspace.close_invocation(Some(&session)).argv(),
            ["--session", "pce-work", "workspace", "close", "w9"]
        );
    }

    #[test]
    fn invalid_session_names_are_refused() {
        for name in ["", ".", "..", "has/slash", &"x".repeat(65)] {
            assert!(
                HerdrSessionName::parse(name, |_| PathBuf::from("/short/herdr.sock")).is_err(),
                "accepted {name:?}"
            );
        }
    }

    #[test]
    fn session_name_is_refused_when_derived_socket_path_exceeds_portable_capacity() {
        let name = "pce-workers-2026-08-20-silence-means-the-run-has-stalled";
        let socket_path = PathBuf::from(format!(
            "/Users/nicolaslazaro/.config/herdr/sessions/{name}/herdr.sock"
        ));
        let error = HerdrSessionName::parse(name, |_| socket_path.clone())
            .expect_err("socket path exceeds macOS sun_path capacity");
        assert_eq!(
            error.to_string(),
            format!(
                "Herdr session name `{name}` produces socket path `{}` with length 111 bytes; with its terminating NUL it exceeds the portable Unix socket path limit of 104 bytes; shorten the session name",
                socket_path.display()
            )
        );
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
    fn explicit_operator_environment_accepts_custom_names_and_reserves_binary_names() {
        let environment = WorkerEnvironment::parse(BTreeMap::from([(
            "CAMPAIGN_TOKEN".to_owned(),
            "secret".to_owned(),
        )]))
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(environment.0["CAMPAIGN_TOKEN"], "secret");

        for name in [
            "TMPDIR",
            "PCE_DISPATCH_TMPDIR",
            "PCE_WORKTREES",
            "PCE_WORKTREE_0",
        ] {
            let error =
                WorkerEnvironment::parse(BTreeMap::from([(name.to_owned(), "value".to_owned())]))
                    .expect_err("binary-owned environment must be refused");
            assert_eq!(
                error.to_string(),
                format!("environment name `{name}` is binary-owned")
            );
        }
    }
    #[test]
    fn pane_run_shell_quotes_every_non_portable_worker_word() {
        let graph = package_graph(&["pce"]);
        let plan = compose_herdr_work_package_dispatch(
            &DispatchVisionSource::parse("vision-one").unwrap_or_else(|error| panic!("{error}")),
            &graph.packages()[0],
            DispatchAttempt::parse(1).unwrap_or_else(|error| panic!("{error}")),
            None,
            &[repository("pce")],
            &AbsoluteWorktreeRoot::parse(PathBuf::from("/worktrees"))
                .unwrap_or_else(|error| panic!("{error}")),
            &AbsoluteDispatchTemporaryDirectory::parse(PathBuf::from("/binary/tmp"))
                .unwrap_or_else(|error| panic!("{error}")),
            WorkerEnvironment::parse(BTreeMap::from([(
                "TOKEN".to_owned(),
                "space and ' quote".to_owned(),
            )]))
            .unwrap_or_else(|error| panic!("{error}")),
            WorkerArgumentVector::parse(vec![
                "worker command".to_owned(),
                "it's safe".to_owned(),
                "$HOME; echo bad".to_owned(),
                String::new(),
            ])
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let run = plan.pane_run(&HerdrAgentLocation::new(
            HerdrWorkspaceId::parse("w9").unwrap_or_else(|error| panic!("{error}")),
            HerdrTabId::parse("w9:t2").unwrap_or_else(|error| panic!("{error}")),
            super::HerdrPaneId::parse("w9:p7").unwrap_or_else(|error| panic!("{error}")),
        ));
        assert_eq!(
            run.argv()[3],
            plan.launch_script_path().display().to_string()
        );
        let script = plan.launch_script();
        assert!(script.contains("'TOKEN=space and '\"'\"' quote'"));
        assert!(script.contains("'worker command' 'it'\"'\"'s safe' '$HOME; echo bad' ''"));
    }

    #[test]
    fn pane_close_targets_only_the_exact_returned_opaque_identity() {
        let pane = super::HerdrPaneId::parse("w9:p73").unwrap_or_else(|error| panic!("{error}"));
        let close = pane.close_invocation(None);
        assert_eq!(close.executable(), "herdr");
        assert_eq!(close.argv(), ["pane", "close", "w9:p73"]);
        assert!(!close.argv().iter().any(|argument| argument == "unrelated"));

        let workspace =
            super::HerdrWorkspaceId::parse("w9").unwrap_or_else(|error| panic!("{error}"));
        let workspace_close = workspace.close_invocation(None);
        assert_eq!(workspace_close.argv(), ["workspace", "close", "w9"]);
        assert!(
            !workspace_close
                .argv()
                .iter()
                .any(|argument| argument == "w8")
        );
    }
    #[test]
    fn dependency_repository_input_is_delivered_after_owned_repositories() {
        let graph = package_graph(&["app"]);
        let inputs = vec![repository("app"), repository("library")];
        let plan = compose_herdr_work_package_dispatch(
            &DispatchVisionSource::parse("vision-one").unwrap_or_else(|error| panic!("{error}")),
            &graph.packages()[0],
            DispatchAttempt::parse(1).unwrap_or_else(|error| panic!("{error}")),
            None,
            &inputs,
            &AbsoluteWorktreeRoot::parse(PathBuf::from("/worktrees"))
                .unwrap_or_else(|error| panic!("{error}")),
            &AbsoluteDispatchTemporaryDirectory::parse(PathBuf::from("/binary/tmp"))
                .unwrap_or_else(|error| panic!("{error}")),
            WorkerEnvironment::parse(BTreeMap::new()).unwrap_or_else(|error| panic!("{error}")),
            WorkerArgumentVector::parse(vec!["prime-agent".to_owned()])
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            plan.worktrees()
                .iter()
                .map(|worktree| worktree.repository())
                .collect::<Vec<_>>(),
            vec!["app", "library"]
        );
        assert!(
            plan.environment()
                .iter()
                .any(|(name, value)| name == "PCE_WORKTREE_1" && value.ends_with("/01-library"))
        );
    }
}
