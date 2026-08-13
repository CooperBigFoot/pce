//! gate_brief : VisionGoal × VisionCriteria × WorkPackageGraph × WorkPackage × RepositoryWorktrees × BuiltArtifactRef → GateBrief
//! gate_outcome : OutcomeDocumentBytes → PackageGateOutcome
//!
//! A package gate independently attacks one built artifact. Its findings carry repairs and the
//! falsifiers that a later driver can replay; this module never executes those commands.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use serde::Deserialize;
use thiserror::Error;

use crate::{AcceptanceCriteria, DependencyKind, RepositoryWorktree, VisionGoal, WorkPackageGraph};

/// A non-empty reference identifying the built artifact presented to a gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltArtifactRef(String);

impl BuiltArtifactRef {
    /// Parse an artifact reference.
    ///
    /// # Errors
    ///
    /// Returns [`PackageGateError::EmptyArtifactRef`] for a blank reference.
    pub fn parse(value: impl Into<String>) -> Result<Self, PackageGateError> {
        let value = value.into();
        if value.trim().is_empty() {
            Err(PackageGateError::EmptyArtifactRef)
        } else {
            Ok(Self(value))
        }
    }

    /// Return the exact artifact reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A gate-brief composition or outcome-parse failure.
#[derive(Debug, Error)]
pub enum PackageGateError {
    /// The built artifact reference is blank.
    #[error("built artifact reference must be non-empty")]
    EmptyArtifactRef,
    /// The requested package is absent from the graph.
    #[error("package {package} is absent from the work-package graph")]
    UnknownPackage { package: String },
    /// A repository binding is duplicated, absent, or does not belong to the package.
    #[error("repository worktree `{repository}` is not unique and exact for package {package}")]
    RepositoryWorktreeMismatch { repository: String, package: String },
    /// Outcome bytes are not syntactically valid for the typed carrier.
    #[error("package gate outcome is malformed: {source}")]
    MalformedOutcome { source: serde_json::Error },
    /// A finding has no repository reference pair.
    #[error("package gate finding must name at least one repository ref pair")]
    MissingRepositoryRefs,
    /// A finding repeats one repository identity.
    #[error("package gate finding repeats repository `{repository}`")]
    DuplicateFindingRepository { repository: String },
    /// A finding names a repository outside the target package.
    #[error("package gate finding names untouched repository `{repository}`")]
    UntouchedFindingRepository { repository: String },
}

fn dependency_kind(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Buildability => "buildability",
        DependencyKind::Safety => "safety",
        DependencyKind::RiskOrdering => "risk-ordering",
    }
}

/// Compose a deterministic gate brief without executing criteria or proposed falsifiers.
///
/// # Errors
///
/// Returns [`PackageGateError`] if the package is absent or repository bindings are not an exact
/// one-to-one match for it.
pub fn compose_package_gate_brief(
    goal: &VisionGoal,
    vision_criteria: &AcceptanceCriteria,
    graph: &WorkPackageGraph,
    package_id: &str,
    worktrees: &[RepositoryWorktree],
    artifact_ref: &BuiltArtifactRef,
) -> Result<String, PackageGateError> {
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .ok_or_else(|| PackageGateError::UnknownPackage {
            package: package_id.to_owned(),
        })?;
    let expected = package
        .repositories()
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut by_repository = HashMap::new();
    for worktree in worktrees {
        if !expected.contains(worktree.repository())
            || by_repository
                .insert(worktree.repository(), worktree)
                .is_some()
        {
            return Err(PackageGateError::RepositoryWorktreeMismatch {
                repository: worktree.repository().to_owned(),
                package: package_id.to_owned(),
            });
        }
    }
    for repository in package.repositories() {
        if !by_repository.contains_key(repository.as_str()) {
            return Err(PackageGateError::RepositoryWorktreeMismatch {
                repository: repository.clone(),
                package: package_id.to_owned(),
            });
        }
    }

    let mut output = String::new();
    output.push_str("# Work-package gate brief\n\n");
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
    output.push_str("Use the whole graph as context. Other packages are not permission to change their work.\n\n");
    for graph_package in graph.packages() {
        let _ = writeln!(
            output,
            "### {}: {}",
            graph_package.id().as_str(),
            graph_package.title()
        );
        output.push_str("Criteria:\n");
        for criterion in graph_package.criteria() {
            let _ = writeln!(output, "  - {}", criterion.name());
            let _ = writeln!(output, "    Input: {}", criterion.input());
            let _ = writeln!(output, "    Observation: {}", criterion.observation());
        }
        output.push('\n');
    }

    output.push_str("## 3. Artifact and target package in full\n\n");
    let _ = writeln!(output, "Built artifact ref: {}", artifact_ref.as_str());
    let _ = writeln!(
        output,
        "Package: {}: {}",
        package.id().as_str(),
        package.title()
    );
    output.push_str("\nRepositories and assigned worktrees:\n");
    for repository in package.repositories() {
        let worktree = by_repository[repository.as_str()];
        let _ = writeln!(output, "- {}: {}", repository, worktree.path().display());
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
    output.push_str("\nAlready-passed package criteria, shown as the floor, not targets:\n");
    for criterion in package.criteria() {
        let _ = writeln!(output, "- {}", criterion.name());
        let _ = writeln!(output, "  Input: {}", criterion.input());
        let _ = writeln!(output, "  Observation: {}", criterion.observation());
        let _ = writeln!(output, "  Command: {}", criterion.command());
    }
    output.push_str("\nThese criteria already passed. Do not merely repeat them. Attack the built artifact to find material defects that this known floor misses. Do not execute the listed criteria; the driver owns mechanical judgement.\n");

    output.push_str("\n## 4. Repair and scope boundary\n\n");
    output.push_str("You may repair a defect that you find, using exactly two commits in order in each repository the finding touches. Treat the repository-local commits as one coordinated witness phase followed by one coordinated repair phase. First author a witness commit that introduces the falsifier and nothing else; the proposed command must fail there. Second author a repair commit, descending from the witness, containing only the bounded fix; the same command must pass there. Repair only the defect you named and proved with the witness. A repair without its preceding witness cannot be reported as a finding. Do not refactor or make adjacent improvements. Do not expand scope or change another package. Do not push, merge, or tag. If a repair requires cross-package work, report that necessity instead of performing it. Do not execute the proposed command at either commit; the driver owns that semantic replay.\n");

    output.push_str("\n## Required outcome\n\n");
    output.push_str("Write exactly one strict JSON outcome document to the path in `PCE_PACKAGE_GATE_OUTCOME` before exiting. Finding nothing is valid and must be written as `{");
    output.push_str("\"findings\":[]}`. Each finding must contain non-empty `description`, `repair`, and `proposed_criterion_command` strings plus a non-empty `repository_refs` list. Every repository entry must contain exactly `repository`, `witness_ref`, and `repair_ref`; name every repository changed by the finding and no repository outside this package. The witness commit must contain the falsifier alone and the repair commit must descend from it. Produce the command and repository-qualified refs but do not execute the command; the driver will require failure at each witness and success at each repair.\n");
    Ok(output)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
struct NonEmptyString(String);

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

/// One repository-specific witness and repair commit pair.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageGateRepositoryRefs {
    repository: NonEmptyString,
    witness_ref: NonEmptyString,
    repair_ref: NonEmptyString,
}

impl PackageGateRepositoryRefs {
    /// Return the graph repository identity.
    pub fn repository(&self) -> &str {
        &self.repository.0
    }
    /// Return the gate-authored commit containing only the falsifier.
    pub fn witness_ref(&self) -> &str {
        &self.witness_ref.0
    }
    /// Return the descendant commit containing the bounded repair.
    pub fn repair_ref(&self) -> &str {
        &self.repair_ref.0
    }
}

/// One gate finding, applied repair, and its independently replayable falsifier.
///
/// WP7 must execute `proposed_criterion_command` at every witness ref and require failure, then
/// execute it at every repair ref and require success. If either condition does not hold, WP7 must
/// reject the finding without crediting it to the gate. WP7 must bridge these repository-qualified
/// refs and the command into the recorded stimulus and evidence consumed by `pce gate replay`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageGateFinding {
    description: NonEmptyString,
    repair: NonEmptyString,
    proposed_criterion_command: NonEmptyString,
    repository_refs: Vec<PackageGateRepositoryRefs>,
}

impl PackageGateFinding {
    /// Return the concrete defect description.
    pub fn description(&self) -> &str {
        &self.description.0
    }
    /// Return the description of the applied repair.
    pub fn repair(&self) -> &str {
        &self.repair.0
    }
    /// Return the proposed criterion command without executing it.
    pub fn proposed_criterion_command(&self) -> &str {
        &self.proposed_criterion_command.0
    }
    /// Return the witness/repair pair for every repository changed by this finding.
    pub fn repository_refs(&self) -> &[PackageGateRepositoryRefs] {
        &self.repository_refs
    }
}

/// A syntactically parsed gate document that is not yet creditable as an outcome.
///
/// The composition root must validate package repository scope, ref resolution, reachability, and
/// direct witness-to-repair ancestry before treating this document as a gate product.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedPackageGateOutcome {
    findings: Vec<PackageGateFinding>,
}

impl ParsedPackageGateOutcome {
    /// Return all findings. An empty slice means the gate completed without finding a defect.
    pub fn findings(&self) -> &[PackageGateFinding] {
        &self.findings
    }
}

/// Parse one strict gate outcome document.
///
/// # Errors
///
/// Returns [`PackageGateError`] for malformed JSON, unknown fields, or any missing, blank, mistyped,
/// empty, or duplicate repository-ref entry.
pub fn parse_package_gate_outcome(
    bytes: &[u8],
) -> Result<ParsedPackageGateOutcome, PackageGateError> {
    let outcome: ParsedPackageGateOutcome = serde_json::from_slice(bytes)
        .map_err(|source| PackageGateError::MalformedOutcome { source })?;
    for finding in &outcome.findings {
        if finding.repository_refs.is_empty() {
            return Err(PackageGateError::MissingRepositoryRefs);
        }
        let mut repositories = HashSet::new();
        for refs in &finding.repository_refs {
            if !repositories.insert(refs.repository()) {
                return Err(PackageGateError::DuplicateFindingRepository {
                    repository: refs.repository().to_owned(),
                });
            }
        }
    }
    Ok(outcome)
}

/// Ensure every finding names only repositories belonging to the target package.
///
/// This checks graph scope only. Ref resolution and ancestry are checked by the composition root
/// against the assigned repository worktrees, without executing proposed criteria.
///
/// # Errors
///
/// Returns [`PackageGateError::UntouchedFindingRepository`] for an out-of-package repository.
pub fn validate_package_gate_repositories(
    outcome: &ParsedPackageGateOutcome,
    package_repositories: &[String],
) -> Result<(), PackageGateError> {
    for finding in &outcome.findings {
        validate_package_gate_finding_repositories(finding, package_repositories)?;
    }
    Ok(())
}

/// Ensure one finding names only repositories belonging to the target package.
///
/// # Errors
///
/// Returns [`PackageGateError::UntouchedFindingRepository`] for an out-of-package repository.
pub fn validate_package_gate_finding_repositories(
    finding: &PackageGateFinding,
    package_repositories: &[String],
) -> Result<(), PackageGateError> {
    let allowed = package_repositories
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    for refs in &finding.repository_refs {
        if !allowed.contains(refs.repository()) {
            return Err(PackageGateError::UntouchedFindingRepository {
                repository: refs.repository().to_owned(),
            });
        }
    }
    Ok(())
}
