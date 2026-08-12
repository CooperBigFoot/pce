//! readiness : WorkPackageGraph × RepositoryMergeObservations × RiskOrdering → ReadyReport
//!
//! A validated graph is a finite DAG whose hard ancestry remains connected when advisory edges
//! are overridden.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::instrument;

use crate::run_state::MergeStatus;

/// One immutable plan version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkPackageGraph {
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    schema: Option<String>,
    vision: String,
    plan_version: u64,
    authored_at_ref: String,
    packages: Vec<WorkPackage>,
}

impl WorkPackageGraph {
    /// Return the vision identity recorded by this plan.
    pub fn vision(&self) -> &str {
        &self.vision
    }
    /// Return the immutable plan version.
    pub const fn plan_version(&self) -> u64 {
        self.plan_version
    }
    /// Return the source ref against which the plan was authored.
    pub fn authored_at_ref(&self) -> &str {
        &self.authored_at_ref
    }
    /// Return the packages in author order.
    pub fn packages(&self) -> &[WorkPackage] {
        &self.packages
    }
}

/// A non-empty package identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct WorkPackageId(String);
impl WorkPackageId {
    /// Return the exact identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One independently completable unit of work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkPackage {
    id: WorkPackageId,
    title: String,
    repositories: Vec<String>,
    criteria: Vec<WorkPackageCriterion>,
    depends_on: Vec<WorkPackageDependency>,
}
impl WorkPackage {
    /// Return the package identifier.
    pub const fn id(&self) -> &WorkPackageId {
        &self.id
    }
    /// Return the package title.
    pub fn title(&self) -> &str {
        &self.title
    }
    /// Return the repositories touched by the package.
    pub fn repositories(&self) -> &[String] {
        &self.repositories
    }
    /// Return executable completion criteria without executing them.
    pub fn criteria(&self) -> &[WorkPackageCriterion] {
        &self.criteria
    }
    /// Return the package's typed dependencies.
    pub fn depends_on(&self) -> &[WorkPackageDependency] {
        &self.depends_on
    }
}

/// A human-readable observation paired with its driver command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkPackageCriterion {
    name: String,
    input: String,
    observation: String,
    command: String,
}
impl WorkPackageCriterion {
    /// Return the criterion name.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Return the described input.
    pub fn input(&self) -> &str {
        &self.input
    }
    /// Return the expected observation.
    pub fn observation(&self) -> &str {
        &self.observation
    }
    /// Return the command a driver may execute.
    pub fn command(&self) -> &str {
        &self.command
    }
}

/// The scheduler obligation imposed by an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    /// The dependent cannot compile or run first.
    Buildability,
    /// The dependency protects an irreversible act.
    Safety,
    /// Ordering makes a defect cheaper to discover.
    RiskOrdering,
}
impl DependencyKind {
    /// Whether the scheduler must enforce this edge.
    pub const fn is_binding(self) -> bool {
        matches!(self, Self::Buildability | Self::Safety)
    }
}

/// A typed edge from the named prerequisite to its containing package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkPackageDependency {
    id: WorkPackageId,
    kind: DependencyKind,
    reason: String,
}
impl WorkPackageDependency {
    /// Return the prerequisite package.
    pub const fn id(&self) -> &WorkPackageId {
        &self.id
    }
    /// Return the edge kind.
    pub const fn kind(&self) -> DependencyKind {
        self.kind
    }
    /// Return the kind-specific justification.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGraph {
    #[serde(rename = "$schema")]
    schema: Option<String>,
    vision: String,
    plan_version: u64,
    authored_at_ref: String,
    packages: Vec<RawPackage>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPackage {
    id: String,
    title: String,
    repositories: Vec<String>,
    criteria: Vec<RawCriterion>,
    depends_on: Vec<RawDependency>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCriterion {
    name: String,
    input: String,
    observation: String,
    command: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDependency {
    id: String,
    kind: DependencyKind,
    reason: String,
}

/// A graph parse or semantic-validation failure.
#[derive(Debug, Error)]
pub enum WorkPackageGraphError {
    /// The bytes are not exactly one graph JSON document of the typed shape.
    #[error("work-package graph is not valid typed JSON: {source}")]
    InvalidJson { source: serde_json::Error },
    /// A required string is empty or whitespace-only.
    #[error("{field} must be non-empty in {location}")]
    EmptyField {
        field: &'static str,
        location: String,
    },
    /// The plan version is zero.
    #[error("plan_version must be greater than zero")]
    ZeroPlanVersion,
    /// A package owns no executable criterion.
    #[error("package {package} has no criteria")]
    NoCriteria { package: String },
    /// A package identifier occurs more than once.
    #[error("duplicate package id {package}")]
    DuplicatePackage { package: String },
    /// A dependency names no package in this graph.
    #[error("package {package} references unknown dependency {dependency}")]
    UnknownDependency { package: String, dependency: String },
    /// A package depends on itself.
    #[error("package {package} depends on itself")]
    SelfDependency { package: String },
    /// The same typed endpoint is declared more than once.
    #[error("package {package} declares dependency {dependency} more than once")]
    DuplicateDependency { package: String, dependency: String },
    /// The dependency graph contains a cycle.
    #[error("work-package graph contains a cycle involving {package}")]
    Cycle { package: String },
    /// A mixed advisory/binding path has no all-binding alternative.
    #[error(
        "soft dependency path from {from_package} to {to_package} mixes advisory and binding edges without an all-binding alternative"
    )]
    SoftEdgeSeversBinding {
        from_package: String,
        to_package: String,
    },
    /// A repository occurs more than once in one package.
    #[error("package {package} declares repository {repository} more than once")]
    DuplicateRepository { package: String, repository: String },
    /// A merge observation names a package outside this graph.
    #[error("merge observation package {package} is unknown")]
    UnknownMergeObservationPackage { package: String },
    /// A merge observation names a repository outside its package.
    #[error("merge observation repository {repository} is unknown for package {package}")]
    UnknownMergeObservationRepository { package: String, repository: String },
    /// The same package/repository merge state was observed more than once.
    #[error("duplicate merge observation for repository {repository} of package {package}")]
    DuplicateMergeObservation { package: String, repository: String },
    /// No merge state was supplied for one repository touched by a package.
    #[error("missing merge observation for repository {repository} of package {package}")]
    MissingMergeObservation { package: String, repository: String },
}

fn nonempty(
    value: &str,
    field: &'static str,
    location: impl Into<String>,
) -> Result<(), WorkPackageGraphError> {
    if value.trim().is_empty() {
        Err(WorkPackageGraphError::EmptyField {
            field,
            location: location.into(),
        })
    } else {
        Ok(())
    }
}

/// Parse and semantically validate exact graph bytes.
///
/// # Errors
///
/// Returns [`WorkPackageGraphError`] for malformed JSON, invalid carriers, dangling or duplicate
/// edges, cycles, and advisory reductions that would hide binding ancestry.
#[instrument(skip(bytes))]
pub fn parse_work_package_graph(bytes: &[u8]) -> Result<WorkPackageGraph, WorkPackageGraphError> {
    let raw: RawGraph = serde_json::from_slice(bytes)
        .map_err(|source| WorkPackageGraphError::InvalidJson { source })?;
    if let Some(schema) = &raw.schema {
        nonempty(schema, "$schema", "graph")?;
    }
    nonempty(&raw.vision, "vision", "graph")?;
    nonempty(&raw.authored_at_ref, "authored_at_ref", "graph")?;
    if raw.packages.is_empty() {
        return Err(WorkPackageGraphError::EmptyField {
            field: "packages",
            location: "graph".to_owned(),
        });
    }
    if raw.plan_version == 0 {
        return Err(WorkPackageGraphError::ZeroPlanVersion);
    }
    let mut ids = HashSet::new();
    for package in &raw.packages {
        nonempty(&package.id, "id", "package")?;
        nonempty(&package.title, "title", format!("package {}", package.id))?;
        if !ids.insert(package.id.as_str()) {
            return Err(WorkPackageGraphError::DuplicatePackage {
                package: package.id.clone(),
            });
        }
        if package.repositories.is_empty() {
            return Err(WorkPackageGraphError::EmptyField {
                field: "repositories",
                location: format!("package {}", package.id),
            });
        }
        let mut repositories = HashSet::new();
        for repository in &package.repositories {
            nonempty(repository, "repository", format!("package {}", package.id))?;
            if !repositories.insert(repository.as_str()) {
                return Err(WorkPackageGraphError::DuplicateRepository {
                    package: package.id.clone(),
                    repository: repository.clone(),
                });
            }
        }
        if package.criteria.is_empty() {
            return Err(WorkPackageGraphError::NoCriteria {
                package: package.id.clone(),
            });
        }
        for (index, criterion) in package.criteria.iter().enumerate() {
            let location = format!("criterion {} of package {}", index + 1, package.id);
            nonempty(&criterion.name, "name", &location)?;
            nonempty(&criterion.input, "input", &location)?;
            nonempty(&criterion.observation, "observation", &location)?;
            nonempty(&criterion.command, "command", &location)?;
        }
    }
    let known = raw
        .packages
        .iter()
        .map(|p| p.id.as_str())
        .collect::<HashSet<_>>();
    for package in &raw.packages {
        let mut dependencies = HashSet::new();
        for dependency in &package.depends_on {
            nonempty(
                &dependency.id,
                "id",
                format!("dependency of package {}", package.id),
            )?;
            let obligation = match dependency.kind {
                DependencyKind::Buildability => "buildability reason (code fact)",
                DependencyKind::Safety => "safety reason (irreversible act)",
                DependencyKind::RiskOrdering => "risk-ordering reason (learning)",
            };
            nonempty(
                &dependency.reason,
                obligation,
                format!("dependency {} -> {}", dependency.id, package.id),
            )?;
            if dependency.id == package.id {
                return Err(WorkPackageGraphError::SelfDependency {
                    package: package.id.clone(),
                });
            }
            if !known.contains(dependency.id.as_str()) {
                return Err(WorkPackageGraphError::UnknownDependency {
                    package: package.id.clone(),
                    dependency: dependency.id.clone(),
                });
            }
            if !dependencies.insert(dependency.id.as_str()) {
                return Err(WorkPackageGraphError::DuplicateDependency {
                    package: package.id.clone(),
                    dependency: dependency.id.clone(),
                });
            }
        }
    }
    let graph = WorkPackageGraph {
        schema: raw.schema,
        vision: raw.vision,
        plan_version: raw.plan_version,
        authored_at_ref: raw.authored_at_ref,
        packages: raw
            .packages
            .into_iter()
            .map(|p| WorkPackage {
                id: WorkPackageId(p.id),
                title: p.title,
                repositories: p.repositories,
                criteria: p
                    .criteria
                    .into_iter()
                    .map(|c| WorkPackageCriterion {
                        name: c.name,
                        input: c.input,
                        observation: c.observation,
                        command: c.command,
                    })
                    .collect(),
                depends_on: p
                    .depends_on
                    .into_iter()
                    .map(|d| WorkPackageDependency {
                        id: WorkPackageId(d.id),
                        kind: d.kind,
                        reason: d.reason,
                    })
                    .collect(),
            })
            .collect(),
    };
    validate_acyclic(&graph)?;
    validate_soft_reduction(&graph)?;
    Ok(graph)
}

fn indexes(graph: &WorkPackageGraph) -> HashMap<&str, usize> {
    graph
        .packages
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.as_str(), i))
        .collect()
}
fn validate_acyclic(graph: &WorkPackageGraph) -> Result<(), WorkPackageGraphError> {
    fn visit(
        index: usize,
        graph: &WorkPackageGraph,
        map: &HashMap<&str, usize>,
        marks: &mut [u8],
    ) -> Result<(), WorkPackageGraphError> {
        if marks[index] == 1 {
            return Err(WorkPackageGraphError::Cycle {
                package: graph.packages[index].id.0.clone(),
            });
        }
        if marks[index] == 2 {
            return Ok(());
        }
        marks[index] = 1;
        for dep in &graph.packages[index].depends_on {
            visit(map[dep.id.as_str()], graph, map, marks)?;
        }
        marks[index] = 2;
        Ok(())
    }
    let map = indexes(graph);
    let mut marks = vec![0; graph.packages.len()];
    for index in 0..graph.packages.len() {
        visit(index, graph, &map, &mut marks)?;
    }
    Ok(())
}
fn binding_reachable(
    from: usize,
    to: usize,
    graph: &WorkPackageGraph,
    map: &HashMap<&str, usize>,
) -> bool {
    // Edges point prerequisite -> dependent; walk dependent packages that bind to the current node.
    let mut stack = vec![from];
    let mut seen = HashSet::new();
    while let Some(current) = stack.pop() {
        if current == to {
            return true;
        }
        if !seen.insert(current) {
            continue;
        }
        for (index, p) in graph.packages.iter().enumerate() {
            if p.depends_on
                .iter()
                .any(|d| d.kind.is_binding() && map[d.id.as_str()] == current)
            {
                stack.push(index);
            }
        }
    }
    false
}
fn validate_soft_reduction(graph: &WorkPackageGraph) -> Result<(), WorkPackageGraphError> {
    let map = indexes(graph);
    let mut outgoing = vec![Vec::<(usize, DependencyKind)>::new(); graph.packages.len()];
    for (dependent, package) in graph.packages.iter().enumerate() {
        for dependency in &package.depends_on {
            outgoing[map[dependency.id.as_str()]].push((dependent, dependency.kind));
        }
    }
    for source in 0..graph.packages.len() {
        let mut stack = vec![(source, false, false)];
        let mut seen = HashSet::<(usize, bool, bool)>::new();
        while let Some((current, saw_binding, saw_soft)) = stack.pop() {
            if !seen.insert((current, saw_binding, saw_soft)) {
                continue;
            }
            if saw_binding && saw_soft && !binding_reachable(source, current, graph, &map) {
                return Err(WorkPackageGraphError::SoftEdgeSeversBinding {
                    from_package: graph.packages[source].id.0.clone(),
                    to_package: graph.packages[current].id.0.clone(),
                });
            }
            for (next, kind) in &outgoing[current] {
                stack.push((
                    *next,
                    saw_binding || kind.is_binding(),
                    saw_soft || *kind == DependencyKind::RiskOrdering,
                ));
            }
        }
    }
    Ok(())
}

/// Whether advisory risk-ordering dependencies constrain readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskOrdering {
    /// Enforce advisory ordering.
    Honour,
    /// Ignore advisory ordering explicitly.
    Override,
}

/// One repository's three-valued merge state for one work package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkPackageMergeObservation {
    package: WorkPackageId,
    repository: String,
    status: MergeStatus,
}

impl WorkPackageMergeObservation {
    /// Construct an observation using identities obtained from a validated graph.
    pub fn new(package: WorkPackageId, repository: String, status: MergeStatus) -> Self {
        Self {
            package,
            repository,
            status,
        }
    }

    /// Return the observed package identifier.
    pub const fn package(&self) -> &WorkPackageId {
        &self.package
    }

    /// Return the observed repository name.
    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// Return the repository's derived merge state.
    pub const fn status(&self) -> MergeStatus {
        self.status
    }
}

/// The readiness classification of one package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkPackageClassification {
    /// Every repository touched by this package proves it merged.
    Merged,
    /// The package is not merged and every applicable dependency is merged.
    Ready,
    /// At least one applicable dependency proves it is not merged.
    Waiting,
    /// The package itself or at least one applicable dependency is inconclusive.
    DependencyInconclusive,
}

/// One package's aggregate merge state and readiness classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedWorkPackage {
    package: WorkPackageId,
    merge_status: MergeStatus,
    classification: WorkPackageClassification,
}

impl ClassifiedWorkPackage {
    /// Return the classified package identifier.
    pub const fn package(&self) -> &WorkPackageId {
        &self.package
    }

    /// Return the aggregate state across every repository touched by the package.
    pub const fn merge_status(&self) -> MergeStatus {
        self.merge_status
    }

    /// Return the dependency-aware readiness classification.
    pub const fn classification(&self) -> WorkPackageClassification {
        self.classification
    }
}

/// The computed package classifications and whether advisory ordering was overridden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyWorkPackages {
    packages: Vec<ClassifiedWorkPackage>,
    override_applied: bool,
}
impl ReadyWorkPackages {
    /// Return package results in graph order.
    pub fn packages(&self) -> &[ClassifiedWorkPackage] {
        &self.packages
    }

    /// Return ready package identifiers in graph order.
    pub fn ready(&self) -> Vec<&WorkPackageId> {
        self.packages
            .iter()
            .filter(|package| package.classification == WorkPackageClassification::Ready)
            .map(ClassifiedWorkPackage::package)
            .collect()
    }

    /// Whether risk ordering was explicitly overridden.
    pub const fn override_applied(&self) -> bool {
        self.override_applied
    }
}

fn aggregate_merge_status(statuses: impl IntoIterator<Item = MergeStatus>) -> MergeStatus {
    let mut saw_not_merged = false;
    for status in statuses {
        match status {
            MergeStatus::Inconclusive => return MergeStatus::Inconclusive,
            MergeStatus::NotMerged => saw_not_merged = true,
            MergeStatus::Merged => {}
        }
    }
    if saw_not_merged {
        MergeStatus::NotMerged
    } else {
        MergeStatus::Merged
    }
}

/// Compute package merge states and dependency-aware readiness.
///
/// # Errors
///
/// Returns [`WorkPackageGraphError`] when observations name an unknown package or repository, repeat
/// a package/repository pair, or omit any repository touched by the graph.
#[instrument(skip(graph, observations))]
pub fn ready_work_packages(
    graph: &WorkPackageGraph,
    observations: &[WorkPackageMergeObservation],
    risk: RiskOrdering,
) -> Result<ReadyWorkPackages, WorkPackageGraphError> {
    let packages = graph
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package))
        .collect::<HashMap<_, _>>();
    let mut observed = HashMap::<(&str, &str), MergeStatus>::new();
    for observation in observations {
        let Some(package) = packages.get(observation.package.as_str()) else {
            return Err(WorkPackageGraphError::UnknownMergeObservationPackage {
                package: observation.package.0.clone(),
            });
        };
        if !package
            .repositories
            .iter()
            .any(|repository| repository == &observation.repository)
        {
            return Err(WorkPackageGraphError::UnknownMergeObservationRepository {
                package: observation.package.0.clone(),
                repository: observation.repository.clone(),
            });
        }
        let key = (
            observation.package.as_str(),
            observation.repository.as_str(),
        );
        if observed.insert(key, observation.status).is_some() {
            return Err(WorkPackageGraphError::DuplicateMergeObservation {
                package: observation.package.0.clone(),
                repository: observation.repository.clone(),
            });
        }
    }
    for package in &graph.packages {
        for repository in &package.repositories {
            if !observed.contains_key(&(package.id.as_str(), repository.as_str())) {
                return Err(WorkPackageGraphError::MissingMergeObservation {
                    package: package.id.0.clone(),
                    repository: repository.clone(),
                });
            }
        }
    }

    let merge_statuses = graph
        .packages
        .iter()
        .map(|package| {
            let statuses = package
                .repositories
                .iter()
                .map(|repository| observed[&(package.id.as_str(), repository.as_str())]);
            (package.id.as_str(), aggregate_merge_status(statuses))
        })
        .collect::<HashMap<_, _>>();

    let packages = graph
        .packages
        .iter()
        .map(|package| {
            let merge_status = merge_statuses[package.id.as_str()];
            let classification = match merge_status {
                MergeStatus::Merged => WorkPackageClassification::Merged,
                MergeStatus::Inconclusive => WorkPackageClassification::DependencyInconclusive,
                MergeStatus::NotMerged => {
                    let dependency_statuses = package.depends_on.iter().filter_map(|dependency| {
                        if risk == RiskOrdering::Override
                            && dependency.kind == DependencyKind::RiskOrdering
                        {
                            None
                        } else {
                            Some(merge_statuses[dependency.id.as_str()])
                        }
                    });
                    let mut saw_inconclusive = false;
                    let mut saw_not_merged = false;
                    for status in dependency_statuses {
                        match status {
                            MergeStatus::Merged => {}
                            MergeStatus::NotMerged => saw_not_merged = true,
                            MergeStatus::Inconclusive => saw_inconclusive = true,
                        }
                    }
                    if saw_inconclusive {
                        WorkPackageClassification::DependencyInconclusive
                    } else if saw_not_merged {
                        WorkPackageClassification::Waiting
                    } else {
                        WorkPackageClassification::Ready
                    }
                }
            };
            ClassifiedWorkPackage {
                package: package.id.clone(),
                merge_status,
                classification,
            }
        })
        .collect();
    Ok(ReadyWorkPackages {
        packages,
        override_applied: risk == RiskOrdering::Override,
    })
}
