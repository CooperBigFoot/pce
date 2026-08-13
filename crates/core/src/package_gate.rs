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
    output.push_str("You may repair a defect that you find. Repair only the defect you named. Do not refactor or make adjacent improvements. Do not expand scope or change another package. Do not push, merge, or tag. If a repair requires cross-package work, report that necessity instead of performing it.\n");

    output.push_str("\n## Required outcome\n\n");
    output.push_str("Write exactly one strict JSON outcome document to the path in `PCE_PACKAGE_GATE_OUTCOME` before exiting. Finding nothing is valid and must be written as `{");
    output.push_str("\"findings\":[]}`. Each finding must contain exactly five non-empty strings: `description`, `repair`, `proposed_criterion_command`, `pre_repair_ref`, and `post_repair_ref`. The repair must already be applied and bounded to the described defect. The proposed criterion must fail at the pre-repair ref and pass at the post-repair ref. Produce the command and refs but do not execute the command; the driver will replay it independently.\n");
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

/// One gate finding, applied repair, and its independently replayable falsifier.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageGateFinding {
    description: NonEmptyString,
    repair: NonEmptyString,
    proposed_criterion_command: NonEmptyString,
    pre_repair_ref: NonEmptyString,
    post_repair_ref: NonEmptyString,
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
    /// Return the reference immediately before repair.
    pub fn pre_repair_ref(&self) -> &str {
        &self.pre_repair_ref.0
    }
    /// Return the reference containing the repair.
    pub fn post_repair_ref(&self) -> &str {
        &self.post_repair_ref.0
    }
}

/// The product emitted by a completed gate, including a valid no-findings result.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageGateOutcome {
    findings: Vec<PackageGateFinding>,
}

impl PackageGateOutcome {
    /// Return all findings. An empty slice means the gate completed without finding a defect.
    pub fn findings(&self) -> &[PackageGateFinding] {
        &self.findings
    }
}

/// Parse one strict gate outcome document.
///
/// # Errors
///
/// Returns [`PackageGateError`] for malformed JSON, unknown fields, or any missing, blank, or
/// mistyped finding field.
pub fn parse_package_gate_outcome(bytes: &[u8]) -> Result<PackageGateOutcome, PackageGateError> {
    serde_json::from_slice(bytes).map_err(|source| PackageGateError::MalformedOutcome { source })
}
