//! brief : VisionGoal × VisionCriteria × WorkPackageGraph × WorkPackage × RepositoryWorktrees → WorkerBrief
//! outcome : OutcomeDocumentBytes → PackageOutcome
//!
//! A brief gives one worker global intent, graph-wide summary context, and exactly one package's
//! executable detail. An outcome is the worker's strict, non-self-certifying status report.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

use crate::{AcceptanceCriteria, DependencyKind, WorkPackageGraph};

/// The non-empty contents of the vision's `Goal / Why` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisionGoal(String);

impl VisionGoal {
    /// Parse the exact goal section from a vision document.
    ///
    /// # Errors
    ///
    /// Returns [`PackageWorkerError`] unless exactly one goal header has non-empty content.
    pub fn parse_document(document: &str) -> Result<Self, PackageWorkerError> {
        const HEADER: &str = "## Goal / Why";
        let starts = document.match_indices(HEADER).collect::<Vec<_>>();
        let [(start, _)] = starts.as_slice() else {
            return Err(PackageWorkerError::GoalSection);
        };
        let content_start = *start + HEADER.len();
        let rest = &document[content_start..];
        let content_end = rest
            .match_indices("\n## ")
            .next()
            .map_or(rest.len(), |(index, _)| index);
        let goal = rest[..content_end].trim();
        if goal.is_empty() {
            return Err(PackageWorkerError::GoalSection);
        }
        Ok(Self(goal.to_owned()))
    }

    /// Return the exact trimmed goal text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One graph repository and the absolute worktree assigned to this worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryWorktree {
    repository: String,
    path: PathBuf,
}

impl RepositoryWorktree {
    /// Parse one repository-to-worktree binding.
    ///
    /// # Errors
    ///
    /// Returns [`PackageWorkerError`] for a blank repository or relative worktree path.
    pub fn parse(repository: impl Into<String>, path: PathBuf) -> Result<Self, PackageWorkerError> {
        let repository = repository.into();
        if repository.trim().is_empty() {
            return Err(PackageWorkerError::EmptyRepository);
        }
        if !path.is_absolute() {
            return Err(PackageWorkerError::WorktreeNotAbsolute { path });
        }
        Ok(Self { repository, path })
    }

    /// Return the graph repository name.
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// Return the assigned absolute worktree path.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// A package-brief composition or outcome-parse failure.
#[derive(Debug, Error)]
pub enum PackageWorkerError {
    /// The vision does not have exactly one non-empty `Goal / Why` section.
    #[error("vision must contain exactly one non-empty `## Goal / Why` section")]
    GoalSection,
    /// The requested package is not in the graph.
    #[error("package {package} is absent from the work-package graph")]
    UnknownPackage { package: String },
    /// A repository name is empty.
    #[error("repository name must be non-empty")]
    EmptyRepository,
    /// A worktree path is relative.
    #[error("repository worktree path must be absolute: {path}")]
    WorktreeNotAbsolute { path: PathBuf },
    /// A supplied repository binding is duplicated or does not belong to the target package.
    #[error("repository worktree `{repository}` is not unique and exact for package {package}")]
    RepositoryWorktreeMismatch { repository: String, package: String },
    /// Outcome bytes are not syntactically valid for the typed carrier.
    #[error("package outcome is malformed: {source}")]
    MalformedOutcome { source: serde_json::Error },
    /// Outcome JSON has extra, absent, or mutually invalid fields.
    #[error("package outcome does not have one exact actionable outcome shape")]
    InvalidOutcomeShape,
}

fn dependency_kind(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Buildability => "buildability",
        DependencyKind::Safety => "safety",
        DependencyKind::RiskOrdering => "risk-ordering",
    }
}

fn render_criterion_summary(output: &mut String, criterion: &crate::WorkPackageCriterion) {
    let _ = writeln!(output, "  - {}", criterion.name());
    let _ = writeln!(output, "    Input: {}", criterion.input());
    let _ = writeln!(output, "    Observation: {}", criterion.observation());
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CriterionFileReference {
    repository: String,
    path: String,
}

fn criterion_file_references(
    command: &str,
    repositories: &[String],
) -> BTreeSet<CriterionFileReference> {
    command
        .split(|character: char| character.is_whitespace() || "|&;()<>[]{}".contains(character))
        .filter_map(|token| {
            let value = token.trim_matches(|character: char| "'\"`,:".contains(character));
            if value.is_empty()
                || value.starts_with('-')
                || value.starts_with('/')
                || value.contains("://")
                || value.contains('=')
                || value.contains(['\\', '*', '?', '~'])
            {
                return None;
            }

            let (repository, relative_path, explicitly_rooted) =
                if let Some(indexed) = value.strip_prefix("$PCE_WORKTREE_") {
                    let (index, path) = indexed.split_once('/')?;
                    if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
                        return None;
                    }
                    let repository = repositories.get(index.parse::<usize>().ok()?)?;
                    (repository.as_str(), path, true)
                } else {
                    if value.contains('$') {
                        return None;
                    }
                    (
                        repositories.first()?.as_str(),
                        value.trim_start_matches("./"),
                        false,
                    )
                };

            if relative_path.is_empty()
                || (!explicitly_rooted && !relative_path.contains('/'))
                || relative_path
                    .split('/')
                    .any(|component| component.is_empty() || component == "." || component == "..")
            {
                return None;
            }

            Some(CriterionFileReference {
                repository: repository.to_owned(),
                path: relative_path.to_owned(),
            })
        })
        .collect()
}

fn render_cross_package_file_references(
    output: &mut String,
    graph: &WorkPackageGraph,
    package: &crate::WorkPackage,
) {
    let own_paths = package
        .criteria()
        .iter()
        .flat_map(|criterion| {
            criterion_file_references(criterion.command(), package.repositories())
        })
        .collect::<BTreeSet<_>>();
    let mut other_references = BTreeMap::<CriterionFileReference, BTreeSet<&str>>::new();
    for candidate in graph
        .packages()
        .iter()
        .filter(|candidate| candidate.id() != package.id())
    {
        for reference in candidate.criteria().iter().flat_map(|criterion| {
            criterion_file_references(criterion.command(), candidate.repositories())
        }) {
            if own_paths.contains(&reference) {
                other_references
                    .entry(reference)
                    .or_default()
                    .insert(candidate.id().as_str());
            }
        }
    }

    output.push_str("\nHeuristic cross-package file references (syntactically derived from criterion commands):\n");
    if own_paths.is_empty() {
        output.push_str(
            "- No conservative path candidates were syntactically derived for this package.\n",
        );
    } else {
        for reference in own_paths {
            let CriterionFileReference { repository, path } = &reference;
            match other_references.get(&reference) {
                Some(packages) => {
                    let _ = writeln!(
                        output,
                        "- {repository}:{path}: syntactically derived match with {} (heuristic only)",
                        packages.iter().copied().collect::<Vec<_>>().join(", ")
                    );
                }
                None => {
                    let _ = writeln!(
                        output,
                        "- {repository}:{path}: no syntactically derived cross-package match (heuristic only)"
                    );
                }
            }
        }
    }
}

/// Compose a deterministic worker brief without executing any criterion command.
///
/// # Errors
///
/// Returns [`PackageWorkerError`] if the package is absent, an owned repository is missing, or a
/// repository worktree is repeated. Extra worktrees carry binding dependency repositories.
pub fn compose_package_worker_brief(
    goal: &VisionGoal,
    vision_criteria: &AcceptanceCriteria,
    graph: &WorkPackageGraph,
    package_id: &str,
    worktrees: &[RepositoryWorktree],
) -> Result<String, PackageWorkerError> {
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .ok_or_else(|| PackageWorkerError::UnknownPackage {
            package: package_id.to_owned(),
        })?;
    let mut by_repository = HashMap::new();
    for worktree in worktrees {
        if by_repository
            .insert(worktree.repository.as_str(), worktree)
            .is_some()
        {
            return Err(PackageWorkerError::RepositoryWorktreeMismatch {
                repository: worktree.repository.clone(),
                package: package_id.to_owned(),
            });
        }
    }
    for repository in package.repositories() {
        if !by_repository.contains_key(repository.as_str()) {
            return Err(PackageWorkerError::RepositoryWorktreeMismatch {
                repository: repository.clone(),
                package: package_id.to_owned(),
            });
        }
    }

    let mut output = String::new();
    output.push_str("# Work-package worker brief\n\n");
    output.push_str("## 1. Vision goal and acceptance criteria\n\n");
    let _ = writeln!(output, "Goal: {}\n", goal.as_str());
    output.push_str("Vision acceptance criteria:\n");
    for criterion in vision_criteria.as_slice() {
        let _ = writeln!(output, "- {}", criterion.name().as_str());
        let _ = writeln!(output, "  Input: {}", criterion.input().as_str());
        let _ = writeln!(
            output,
            "  Observation: {}",
            criterion.observation().as_str()
        );
    }

    output.push_str("\n## 2. Whole work-package graph summary\n\n");
    output.push_str("Use this graph to preserve global coherence and to identify work that is not yours. Each package summary intentionally contains only its id, title, and criteria.\n\n");
    for graph_package in graph.packages() {
        let _ = writeln!(
            output,
            "### {}: {}",
            graph_package.id().as_str(),
            graph_package.title()
        );
        output.push_str("Criteria:\n");
        for criterion in graph_package.criteria() {
            render_criterion_summary(&mut output, criterion);
        }
        output.push('\n');
    }

    output.push_str("## 3. Your package in full\n\n");
    let _ = writeln!(
        output,
        "Package: {}: {}",
        package.id().as_str(),
        package.title()
    );
    output.push_str("\nRepositories and assigned worktrees:\n");
    for repository in package.repositories() {
        let worktree = by_repository[repository.as_str()];
        let _ = writeln!(output, "- {}: {}", repository, worktree.path.display());
    }
    output.push_str("\nDependencies:\n");
    if package.depends_on().is_empty() {
        output.push_str("- None\n");
    } else {
        for dependency in package.depends_on() {
            let _ = writeln!(
                output,
                "- {} [{}]: {}",
                dependency.id().as_str(),
                dependency_kind(dependency.kind()),
                dependency.reason()
            );
        }
    }
    output.push_str("\nCriteria and driver commands:\n");
    for criterion in package.criteria() {
        let _ = writeln!(output, "- {}", criterion.name());
        let _ = writeln!(output, "  Input: {}", criterion.input());
        let _ = writeln!(output, "  Observation: {}", criterion.observation());
        let _ = writeln!(output, "  Command: {}", criterion.command());
    }
    render_cross_package_file_references(&mut output, graph, package);
    output.push_str("\nAttempts do not accumulate. This attempt starts from the composed base, and the composed base is the only inheritance from prior work. No uncompleted change from a previous attempt of this package survives. Satisfy every criterion in this attempt.\n");
    output.push_str("\nThe commands above are exposed as targets only. Do not execute them as a substitute for independent judgement. Your own assessment is not the judgement: the criteria will be executed independently by the driver, which will use each command's exit status to judge the package.\n");

    output.push_str("\n## 4. Scope boundary\n\n");
    let others = graph
        .packages()
        .iter()
        .filter(|candidate| candidate.id() != package.id())
        .map(|candidate| candidate.id().as_str())
        .collect::<Vec<_>>();
    match others.as_slice() {
        [] => output.push_str("There are no other packages in this graph.\n"),
        [only] => {
            let _ = writeln!(
                output,
                "{only} is out of bounds. It is someone else's work."
            );
        }
        _ => {
            let (last, initial) = others.split_last().unwrap_or_else(|| unreachable!());
            let _ = writeln!(
                output,
                "{}, and {} are out of bounds. They are someone else's work.",
                initial.join(", "),
                last
            );
        }
    }
    output.push_str("The graph is context, not permission. Use neighbouring packages to avoid stranding their work, but do not implement, refactor, or opportunistically complete them even when doing so is convenient. If this package cannot serve the vision without changing another package, report a missing dependency or other mis-specification instead of crossing the boundary.\n");
    output.push_str("Only the repositories under change listed in this brief may be modified. Cloud, IAM, and account-level mutations are out of scope even when the credentials you hold permit them, unless this package has a criterion that names that act. Infrastructure changes may be made only through a criterion that names the act. This boundary is informational; credentials remain the enforcement fence.\n");

    output.push_str("\n## 5. Act ownership and long-running commands\n\nRun every act to completion in the foreground and write the outcome before exiting. Ending your turn is exiting. A process you background is orphaned the moment you stop, so never leave one running. Do not use shell backgrounding, `nohup`, `disown`, or a detached wrapper.\n\nFor a long-running command, start one foreground tool invocation with its timeout or yield interval configured to permit the command to finish, then keep the turn open and wait on that same invocation until it returns. If the tool returns a live handle, inspect that same handle until it reaches a terminal state. Do not start a child, report its PID or log path, and end the turn to wait. After the act reaches a terminal state, inspect its evidence and write the required outcome before exiting.\n");

    output.push_str("\n## Required outcome\n\nWrite exactly one strict JSON outcome document to the path in `PCE_PACKAGE_OUTCOME` before exiting:\n- done: `{\"outcome\":\"done\"}`\n- failed: `{\"outcome\":\"failed\",\"blocked_by\":\"specific blocker\"}`\n- mis-specified criterion: `{\"outcome\":\"mis-specified\",\"fault\":{\"kind\":\"criterion\",\"name\":\"criterion name\"}}`\n- mis-specified missing dependency: `{\"outcome\":\"mis-specified\",\"fault\":{\"kind\":\"missing-dependency\",\"id\":{\"missing\":\"capability or artifact\",\"checked\":[\"path or symbol\"],\"command\":\"command used to check\"}}}`\nA missing-dependency fault must be falsifiable: name what is missing, identify at least one path or symbol you checked, and give the exact non-empty command used to check. A package identifier alone is invalid. If you cannot identify and test a missing prerequisite, investigate further or report the specific blocker as `failed`; do not invent a mis-specification.\nDo not self-certify criterion results in this file. `done` means only that you believe the implementation work is complete. Commit all completed work before reporting done. Criteria run against the resulting commit, not the working tree, so uncommitted work will not be judged.\n");
    Ok(output)
}

/// The only three worker-reported outcomes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case", deny_unknown_fields)]
pub enum PackageOutcome {
    /// The worker believes implementation is complete; the driver still judges it.
    Done,
    /// The worker could not complete the work and names the blocker.
    Failed { blocked_by: NonEmptyString },
    /// The graph package is wrong and identifies the faulty criterion or absent dependency.
    MisSpecified { fault: MisSpecificationFault },
}

/// A non-empty string in an outcome document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Return the exact non-empty value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmptyString {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.trim().is_empty() {
            Err("value must be non-empty")
        } else {
            Ok(Self(value))
        }
    }
}

/// Falsifiable evidence for a prerequisite believed to be absent from the graph.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissingDependencyEvidence {
    missing: NonEmptyString,
    #[serde(deserialize_with = "deserialize_non_empty_checked")]
    checked: Vec<NonEmptyString>,
    command: NonEmptyString,
}

impl MissingDependencyEvidence {
    /// Return the capability or artifact believed to be missing.
    pub fn as_str(&self) -> &str {
        self.missing.as_str()
    }

    /// Return the paths or symbols examined for the missing prerequisite.
    pub fn checked(&self) -> &[NonEmptyString] {
        &self.checked
    }

    /// Return the command used to test for the prerequisite.
    pub fn command(&self) -> &str {
        self.command.as_str()
    }
}

fn deserialize_non_empty_checked<'de, D>(deserializer: D) -> Result<Vec<NonEmptyString>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let checked = Vec::<NonEmptyString>::deserialize(deserializer)?;
    if checked.is_empty() {
        return Err(serde::de::Error::custom(
            "checked paths or symbols must be non-empty",
        ));
    }
    Ok(checked)
}

/// The actionable identity of a package mis-specification.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MisSpecificationFault {
    /// One named package criterion does not serve the vision goal.
    Criterion { name: NonEmptyString },
    /// One prerequisite is absent, with evidence that lets a supervisor test that claim.
    MissingDependency { id: MissingDependencyEvidence },
}

/// Parse exactly one strict worker outcome JSON document.
///
/// # Errors
///
/// Returns [`PackageWorkerError`] for malformed JSON, extra fields, absent blocker details, or a
/// `mis-specified` outcome that names neither a criterion nor a missing dependency.
pub fn parse_package_outcome(bytes: &[u8]) -> Result<PackageOutcome, PackageWorkerError> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|source| PackageWorkerError::MalformedOutcome { source })?;
    let strict_shape = value.as_object().is_some_and(|object| {
        match object.get("outcome").and_then(serde_json::Value::as_str) {
            Some("done") => object.len() == 1,
            Some("failed") => object.len() == 2 && object.contains_key("blocked_by"),
            Some("mis-specified") => {
                object.len() == 2
                    && object
                        .get("fault")
                        .and_then(serde_json::Value::as_object)
                        .is_some_and(|fault| {
                            fault.len() == 2
                                && match fault.get("kind").and_then(serde_json::Value::as_str) {
                                    Some("criterion") => fault.contains_key("name"),
                                    Some("missing-dependency") => fault.contains_key("id"),
                                    _ => false,
                                }
                        })
            }
            _ => false,
        }
    });
    if !strict_shape {
        return Err(PackageWorkerError::InvalidOutcomeShape);
    }
    serde_json::from_value(value).map_err(|source| PackageWorkerError::MalformedOutcome { source })
}

#[cfg(test)]
mod tests {
    use super::{PackageOutcome, parse_package_outcome};

    #[test]
    fn missing_dependency_fault_requires_falsifiable_evidence() {
        assert!(matches!(
            parse_package_outcome(
                br#"{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":{"missing":"released-reader HTTPS transport","checked":["src/reader.py::ReleasedReader","tests/test_reader.py"],"command":"rg ReleasedReader src/reader.py tests/test_reader.py"}}}"#
            ),
            Ok(PackageOutcome::MisSpecified { .. })
        ));
        for unfalsifiable in [
            br#"{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"GD2"}}"#.as_slice(),
            br#"{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":{"missing":"released-reader HTTPS transport","checked":[],"command":"rg ReleasedReader src/reader.py"}}}"#.as_slice(),
            br#"{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":{"missing":"released-reader HTTPS transport","checked":["src/reader.py"],"command":" "}}}"#.as_slice(),
        ] {
            assert!(parse_package_outcome(unfalsifiable).is_err());
        }
    }

    #[test]
    fn parses_exactly_three_strict_actionable_outcomes() {
        assert!(matches!(
            parse_package_outcome(br#"{"outcome":"done"}"#),
            Ok(PackageOutcome::Done)
        ));
        assert!(matches!(
            parse_package_outcome(br#"{"outcome":"failed","blocked_by":"no compiler"}"#),
            Ok(PackageOutcome::Failed { .. })
        ));
        assert!(matches!(parse_package_outcome(br#"{"outcome":"mis-specified","fault":{"kind":"criterion","name":"wrong oracle"}}"#), Ok(PackageOutcome::MisSpecified { .. })));
        assert!(matches!(
            parse_package_outcome(
                br#"{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":{"missing":"compiler capability","checked":["tools/compiler.rs::compile"],"command":"rg compile tools/compiler.rs"}}}"#
            ),
            Ok(PackageOutcome::MisSpecified { .. })
        ));
        for malformed in [
            br#"{"outcome":"mis-specified"}"#.as_slice(),
            br#"{"outcome":"mis-specified","fault":{"kind":"criterion","name":" "}}"#.as_slice(),
            br#"{"outcome":"failed","blocked_by":""}"#.as_slice(),
            br#"{"outcome":"done","blocked_by":"extra"}"#.as_slice(),
            br#"{"outcome":"unknown"}"#.as_slice(),
        ] {
            assert!(parse_package_outcome(malformed).is_err());
        }
    }
}
