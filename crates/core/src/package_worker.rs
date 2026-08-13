//! brief : VisionGoal × VisionCriteria × WorkPackageGraph × WorkPackage × RepositoryWorktrees → WorkerBrief
//! outcome : OutcomeDocumentBytes → PackageOutcome
//!
//! A brief gives one worker global intent, graph-wide summary context, and exactly one package's
//! executable detail. An outcome is the worker's strict, non-self-certifying status report.

use std::collections::{HashMap, HashSet};
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

/// Compose a deterministic worker brief without executing any criterion command.
///
/// # Errors
///
/// Returns [`PackageWorkerError`] if the package is absent or its repository worktrees are not an
/// exact one-to-one match.
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
    let expected = package
        .repositories()
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut by_repository = HashMap::new();
    for worktree in worktrees {
        if !expected.contains(worktree.repository.as_str())
            || by_repository
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

    output.push_str("\n## Required outcome\n\nWrite exactly one strict JSON outcome document to the path in `PCE_PACKAGE_OUTCOME` before exiting:\n- done: `{\"outcome\":\"done\"}`\n- failed: `{\"outcome\":\"failed\",\"blocked_by\":\"specific blocker\"}`\n- mis-specified criterion: `{\"outcome\":\"mis-specified\",\"fault\":{\"kind\":\"criterion\",\"name\":\"criterion name\"}}`\n- mis-specified missing dependency: `{\"outcome\":\"mis-specified\",\"fault\":{\"kind\":\"missing-dependency\",\"id\":\"dependency identity\"}}`\nDo not self-certify criterion results in this file. `done` means only that you believe the implementation work is complete.\n");
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

/// The actionable identity of a package mis-specification.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum MisSpecificationFault {
    /// One named package criterion does not serve the vision goal.
    Criterion { name: NonEmptyString },
    /// One named prerequisite is absent from the graph.
    MissingDependency { id: NonEmptyString },
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
                br#"{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"WP0"}}"#
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
