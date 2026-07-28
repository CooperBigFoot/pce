use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::time::SystemTime;

use anyhow::{Context, Error, Result, anyhow, bail};
use pce_core::{
    AppendError, ArtifactPath, AuthorityFailure, BranchState, CanonicalNode as DispatchNode,
    CreationDate, CurrentArtifactObservation, CurrentArtifactState, DispatchCandidate,
    DispatchRoleClass, DispatchabilityResult, EventBodyRef, EventKindName, EventLogTail,
    EventLogTailLine, EventRecord, EventRecordFilter, EventTimestamp, ExactPullRequestIdentity,
    ExactPullRequestState, GitAuthorityObservation, GitHubAuthorityObservation,
    GitHubPullRequestObservation, GitMergeObservation, KnownPayload, MergeStatus, MergeSubject,
    MilestoneMergeSubject, MilestoneNode, NodeId, OrderingEdge, PullRequestNumber,
    PullRequestSelector, RecoveryLogPath, RepositoryBranchName, RepositoryFetchObservation,
    RepositoryName, RepositoryObservation, RepositoryObservationFailure, RepositoryObservationRef,
    RunSnapshot, Sha256Digest, SquashCommitOid, StepAuthorityObservation, StepNode, TagName,
    TagState, TagTarget, UnparsedPayload, VersionPolicy, VisionName, VisionSlug, WorktreeIdentity,
    WorktreeState, WriteKind, append_event, compute_dispatchability, create_vision,
    derive_merge_status, derive_milestone_merge_status, derive_run_state, event_record_matches,
    parse_event_line, render_human_snapshot,
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

const USAGE: &str = concat!(
    "usage: pce vision new \"<name>\"\n",
    "       pce log --file <LOG_PATH> --kind <KIND> --node <NODE>\n",
    "       pce log read --file <LOG_PATH> [--kind <KIND>] [--node <NODE>]\n",
    "       pce status --file <LOG_PATH> --vision-dir <VISION_DIR> [--human]\n",
    "       pce ready --file <LOG_PATH> --vision-dir <VISION_DIR> --policy <REPOSITORY>=<NONE|SERIALIZE_DISPATCHES> [--policy <REPOSITORY>=<NONE|SERIALIZE_DISPATCHES> ...]"
);
const RUN_SNAPSHOT_SCHEMA: &str = include_str!("../skills/pce/schemas/run-snapshot.schema.json");
const ORIGIN: &str = "origin";
const RELEASE_TAG: &str = "v0.1.16";

#[derive(Debug)]
enum Command {
    VisionNew {
        name: VisionName,
    },
    LogWrite {
        path: PathBuf,
        kind: WriteKind,
        node: NodeId,
    },
    LogRead {
        path: PathBuf,
        filter: EventRecordFilter,
    },
    Status {
        log_path: PathBuf,
        recovery_log_path: RecoveryLogPath,
        vision_dir: PathBuf,
        format: StatusFormat,
    },
    Ready {
        log_path: PathBuf,
        recovery_log_path: RecoveryLogPath,
        vision_dir: PathBuf,
        version_policies: Vec<(RepositoryName, VersionPolicy)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusFormat {
    Json,
    Human,
}

#[derive(Debug)]
struct RepositoryContract {
    name: RepositoryName,
    root: PathBuf,
}

#[derive(Debug)]
struct CanonicalNode {
    node: NodeId,
    subject: MergeSubject,
    latest_sequence: u64,
}

#[derive(Debug)]
struct RepositoryRuntime {
    name: RepositoryName,
    root: PathBuf,
    fetches: Vec<BranchFetch>,
}

#[derive(Debug)]
struct BranchFetch {
    branch: String,
    result: FetchResult,
}

#[derive(Debug)]
enum FetchResult {
    Observed { oid: String, fetched_at: SystemTime },
    Unavailable { detail: String },
}

#[derive(Debug)]
enum ProcessAttempt {
    SpawnFailed { detail: String },
    Completed(ProcessResult),
}

#[derive(Debug)]
struct ProcessResult {
    command: String,
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl ProcessResult {
    fn detail(&self) -> String {
        format!(
            "command `{}` exited {}; stdout: {}; stderr: {}",
            self.command,
            self.status,
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(&self.stderr)
        )
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let mut input = std::io::stdin().lock();
    run(std::env::args().skip(1), &mut input)
}

fn run(args: impl Iterator<Item = String>, input: &mut dyn Read) -> Result<()> {
    match parse_command(args)? {
        Command::VisionNew { name } => run_vision_new(&name),
        Command::LogWrite { path, kind, node } => run_log(&path, kind, node, input),
        Command::LogRead { path, filter } => {
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            run_log_read(&path, &filter, &mut output)
        }
        Command::Status {
            log_path,
            recovery_log_path,
            vision_dir,
            format,
        } => run_status(&log_path, &recovery_log_path, &vision_dir, format),
        Command::Ready {
            log_path,
            recovery_log_path,
            vision_dir,
            version_policies,
        } => run_ready(
            &log_path,
            &recovery_log_path,
            &vision_dir,
            &version_policies,
        ),
    }
}

fn parse_command(args: impl Iterator<Item = String>) -> Result<Command> {
    let args: Vec<String> = args.collect();
    match args.as_slice() {
        [verb, action, raw_name] if verb == "vision" && action == "new" => {
            let name = VisionName::parse(raw_name).context("failed to parse vision name")?;
            Ok(Command::VisionNew { name })
        }
        [verb, action, rest @ ..] if verb == "log" => parse_log_command(action, rest),
        [verb, action, rest @ ..] if verb == "status" => parse_status_command(action, rest),
        [verb, rest @ ..] if verb == "ready" => parse_ready_command(rest),
        _ => bail!(USAGE),
    }
}

fn parse_log_command(action: &str, rest: &[String]) -> Result<Command> {
    match (action, rest) {
        ("--file", [raw_path, kind_flag, raw_kind, node_flag, raw_node])
            if kind_flag == "--kind"
                && node_flag == "--node"
                && is_value(raw_path)
                && is_value(raw_kind)
                && is_value(raw_node) =>
        {
            let kind = WriteKind::parse(raw_kind).context("failed to parse event kind")?;
            let node = NodeId::parse(raw_node).context("failed to parse event node")?;
            Ok(Command::LogWrite {
                path: PathBuf::from(raw_path),
                kind,
                node,
            })
        }
        ("read", [file_flag, raw_path]) if file_flag == "--file" => Ok(Command::LogRead {
            path: PathBuf::from(raw_path),
            filter: EventRecordFilter::All,
        }),
        ("read", [file_flag, raw_path, kind_flag, raw_kind])
            if file_flag == "--file" && kind_flag == "--kind" =>
        {
            Ok(Command::LogRead {
                path: PathBuf::from(raw_path),
                filter: EventRecordFilter::Kind(EventKindName::new(raw_kind)),
            })
        }
        ("read", [file_flag, raw_path, node_flag, raw_node])
            if file_flag == "--file" && node_flag == "--node" =>
        {
            let node = NodeId::parse(raw_node).context("failed to parse log-read node filter")?;
            Ok(Command::LogRead {
                path: PathBuf::from(raw_path),
                filter: EventRecordFilter::Node(node),
            })
        }
        (
            "read",
            [
                file_flag,
                raw_path,
                kind_flag,
                raw_kind,
                node_flag,
                raw_node,
            ],
        ) if file_flag == "--file" && kind_flag == "--kind" && node_flag == "--node" => {
            let node = NodeId::parse(raw_node).context("failed to parse log-read node filter")?;
            Ok(Command::LogRead {
                path: PathBuf::from(raw_path),
                filter: EventRecordFilter::KindAndNode {
                    kind: EventKindName::new(raw_kind),
                    node,
                },
            })
        }
        _ => bail!(USAGE),
    }
}

fn parse_status_command(action: &str, rest: &[String]) -> Result<Command> {
    match (action, rest) {
        ("--file", [raw_path, vision_flag, raw_vision_dir, trailing @ ..])
            if vision_flag == "--vision-dir" && is_value(raw_path) && is_value(raw_vision_dir) =>
        {
            let format = parse_status_format(trailing)?;
            Ok(Command::Status {
                log_path: PathBuf::from(raw_path),
                recovery_log_path: RecoveryLogPath::new(raw_path),
                vision_dir: PathBuf::from(raw_vision_dir),
                format,
            })
        }
        _ => bail!(USAGE),
    }
}

fn parse_status_format(trailing: &[String]) -> Result<StatusFormat> {
    match trailing {
        [] => Ok(StatusFormat::Json),
        [mode] if mode == "--human" => Ok(StatusFormat::Human),
        _ => bail!(USAGE),
    }
}

fn parse_ready_command(args: &[String]) -> Result<Command> {
    let [
        file_flag,
        raw_path,
        vision_flag,
        raw_vision_dir,
        policies @ ..,
    ] = args
    else {
        bail!(USAGE);
    };
    if file_flag != "--file"
        || vision_flag != "--vision-dir"
        || !is_value(raw_path)
        || !is_value(raw_vision_dir)
        || policies.is_empty()
        || !policies.len().is_multiple_of(2)
    {
        bail!(USAGE);
    }

    let mut version_policies = Vec::<(RepositoryName, VersionPolicy)>::new();
    for pair in policies.chunks_exact(2) {
        if pair[0] != "--policy" || !is_value(&pair[1]) {
            bail!(USAGE);
        }
        let raw_policy = &pair[1];
        if raw_policy.matches('=').count() != 1 {
            bail!("ready policy must contain exactly one `=`");
        }
        let (raw_repository, raw_value) = raw_policy
            .split_once('=')
            .context("ready policy must contain exactly one `=`")?;
        if raw_repository.is_empty() {
            bail!("ready policy repository must not be empty");
        }
        let repository = RepositoryName::new(raw_repository);
        if version_policies
            .iter()
            .any(|(existing, _)| existing == &repository)
        {
            bail!(
                "duplicate ready policy for repository {}",
                repository.as_str()
            );
        }
        let policy = match raw_value {
            "NONE" => VersionPolicy::None,
            "SERIALIZE_DISPATCHES" => VersionPolicy::SerializeDispatches,
            _ => bail!("unknown ready policy {raw_value:?}"),
        };
        version_policies.push((repository, policy));
    }

    Ok(Command::Ready {
        log_path: PathBuf::from(raw_path),
        recovery_log_path: RecoveryLogPath::new(raw_path),
        vision_dir: PathBuf::from(raw_vision_dir),
        version_policies,
    })
}

fn is_value(raw: &str) -> bool {
    !raw.starts_with("--")
}

fn run_vision_new(name: &VisionName) -> Result<()> {
    let new_vision = create_vision(name, Path::new("planning"), CreationDate::today())
        .context("failed to create vision")?;
    println!("{}", new_vision.dir());

    Ok(())
}

fn run_log(path: &Path, kind: WriteKind, node: NodeId, input: &mut dyn Read) -> Result<()> {
    let mut payload = String::new();
    input
        .read_to_string(&mut payload)
        .context("failed to read event payload from stdin to EOF")?;

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open or create event log {}", path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", path.display()))?;

    let operation = append_locked(&mut file, payload, kind, node, path);
    let unlock = file
        .unlock()
        .with_context(|| format!("failed to unlock event log {}", path.display()));

    match (operation, unlock) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(unlock_error)) => Err(unlock_error),
        (Err(primary), Err(unlock_error)) => Err(primary.context(format!(
            "additionally, explicit event-log unlock failed: {unlock_error:#}"
        ))),
    }
}

fn run_status(
    log_path: &Path,
    recovery_log_path: &RecoveryLogPath,
    vision_dir: &Path,
    format: StatusFormat,
) -> Result<()> {
    let parsed_lines = read_event_log(log_path)?;
    let records = parsed_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let contracts = repository_contracts(&records)?;
    let primary_index = resolve_primary_repository(&contracts, log_path, vision_dir)?;
    let vision = vision_slug(vision_dir)?;
    let (artifacts, _) = current_artifacts(&records, &contracts[primary_index].root)?;
    let canonical_nodes = canonical_nodes(&records, &vision)?;
    let selected_index = canonical_nodes
        .iter()
        .enumerate()
        .max_by_key(|(_, item)| item.latest_sequence)
        .map(|(index, _)| index)
        .context("event log contains no canonical step node for repository projection")?;
    let selected = &canonical_nodes[selected_index];
    let integration_branches = integration_branches(&canonical_nodes, selected_index);

    let mut repositories = Vec::with_capacity(contracts.len());
    let mut runtimes = Vec::with_capacity(contracts.len());
    for contract in contracts {
        let (observation, runtime) =
            observe_repository(contract, &integration_branches, selected.subject.selector())?;
        repositories.push(observation);
        runtimes.push(runtime);
    }
    let primary = runtimes
        .get(primary_index)
        .context("resolved primary repository index is unavailable")?;
    let authorities = observe_authorities(&canonical_nodes, primary)?;
    let state = derive_run_state(
        &records,
        &vision,
        recovery_log_path,
        &artifacts,
        &repositories,
        &authorities,
    )
    .context("failed to derive run state")?;
    let snapshot = RunSnapshot::from(&state);
    let value = validated_snapshot_value(&snapshot)?;

    match format {
        StatusFormat::Json => write_json_stdout(&value),
        StatusFormat::Human => {
            let rendered = render_human_snapshot(&snapshot);
            write_human_stdout(&rendered)
        }
    }
}

#[derive(Debug)]
struct DispatchGraph {
    nodes: Vec<DispatchGraphNode>,
    edges: Vec<OrderingEdge>,
}

#[derive(Debug)]
struct DispatchGraphNode {
    node_id: NodeId,
    node: DispatchNode,
    repository: RepositoryName,
}

fn run_ready(
    log_path: &Path,
    recovery_log_path: &RecoveryLogPath,
    vision_dir: &Path,
    version_policies: &[(RepositoryName, VersionPolicy)],
) -> Result<()> {
    let parsed_lines = read_event_log(log_path)?;
    let records = parsed_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let contracts = repository_contracts(&records)?;
    let primary_index = resolve_primary_repository(&contracts, log_path, vision_dir)?;
    let vision = vision_slug(vision_dir)?;

    let mut approvals = records
        .iter()
        .filter_map(|record| {
            let EventBodyRef::Known(KnownPayload::PlanningArtifactApproved(payload)) =
                record.body_ref()
            else {
                return None;
            };
            Some((record.sequence(), payload.path.clone()))
        })
        .collect::<Vec<_>>();
    approvals.sort_by(|(left, _), (right, _)| right.get().cmp(&left.get()));

    let (artifacts, artifact_bytes) = current_artifacts(&records, &contracts[primary_index].root)?;
    let state = derive_run_state(&records, &vision, recovery_log_path, &artifacts, &[], &[])
        .context("failed to derive readiness provenance and dispatch history")?;
    let mut selected = None;
    for (approval_sequence, artifact_path) in approvals {
        let retained = artifact_bytes
            .iter()
            .find(|artifact| artifact.path == artifact_path)
            .context("approved artifact has no retained current observation")?;
        let Some(bytes) = retained.bytes.as_deref() else {
            tracing::info!(
                artifact_path = artifact_path.as_str(),
                reason = "artifact missing",
                "skipping approved artifact candidate"
            );
            continue;
        };
        match parse_dispatch_graph(bytes) {
            Ok(graph) => {
                selected = Some((approval_sequence, artifact_path, graph));
                break;
            }
            Err(error) => {
                tracing::info!(
                    artifact_path = artifact_path.as_str(),
                    reason = %format!("{error:#}"),
                    "skipping approved artifact candidate"
                );
            }
        }
    }
    let (approval_sequence, artifact_path, graph) =
        selected.context("no approved artifact is a conforming graph")?;
    let provenance = state
        .provenance()
        .iter()
        .find(|item| item.approval_sequence() == approval_sequence && item.path() == &artifact_path)
        .context("selected planning-artifact approval has no derived provenance")?;
    compute_dispatchability(provenance, &[], &[], &[], &[])
        .context("selected planning artifact failed preliminary provenance check")?;

    for graph_node in &graph.nodes {
        let matches = contracts
            .iter()
            .filter(|contract| contract.name == graph_node.repository)
            .count();
        if matches != 1 {
            bail!(
                "expected exactly one repository contract for graph repository {}, found {}",
                graph_node.repository.as_str(),
                matches
            );
        }
    }

    let candidates = graph
        .nodes
        .iter()
        .filter(|graph_node| !already_dispatched(graph_node, state.dispatches()))
        .map(|graph_node| {
            DispatchCandidate::new(graph_node.node.clone(), graph_node.repository.clone())
        })
        .collect::<Vec<_>>();

    let mut merge_statuses = Vec::<(DispatchNode, MergeStatus)>::with_capacity(graph.nodes.len());
    for graph_node in &graph.nodes {
        let contract = contracts
            .iter()
            .find(|contract| contract.name == graph_node.repository)
            .context("graph repository contract disappeared after resolution")?;
        let (selector, altitude) = match &graph_node.node {
            DispatchNode::Milestone(node) => {
                let subject = MilestoneMergeSubject::derive(node.clone());
                (
                    subject.selector().clone(),
                    ReadyAltitude::Milestone(subject),
                )
            }
            DispatchNode::Step(node) => {
                let subject = MergeSubject::derive(&vision, node.clone());
                (subject.selector().clone(), ReadyAltitude::Step(subject))
            }
        };
        let (_, runtime) = observe_repository(
            RepositoryContract {
                name: contract.name.clone(),
                root: contract.root.clone(),
            },
            &[selector.base().as_str().to_owned()],
            &selector,
        )
        .with_context(|| {
            format!(
                "failed to observe graph node {} in repository {}",
                graph_node.node_id.as_str(),
                graph_node.repository.as_str()
            )
        })?;
        let github = observe_github(&runtime.root, &selector)?;
        let git = observe_git(&runtime, &selector, &github)?;
        let status = match altitude {
            ReadyAltitude::Milestone(subject) => {
                derive_milestone_merge_status(&subject, &github, &git)
            }
            ReadyAltitude::Step(subject) => derive_merge_status(&subject, &github, &git),
        };
        merge_statuses.push((graph_node.node.clone(), status));
    }

    let results = compute_dispatchability(
        provenance,
        &candidates,
        &graph.edges,
        &merge_statuses,
        version_policies,
    )
    .context("failed to compute graph dispatchability")?;
    let rendered = results
        .iter()
        .map(|result| {
            let (classification, candidate) = match result {
                DispatchabilityResult::Dispatchable { candidate } => ("ready", candidate),
                DispatchabilityResult::Waiting { candidate } => ("waiting", candidate),
                DispatchabilityResult::DependencyInconclusive { candidate } => {
                    ("dependency-inconclusive", candidate)
                }
            };
            serde_json::json!({
                "classification": classification,
                "node": dispatch_node_id(candidate.node()),
                "repository": candidate.repository().as_str(),
            })
        })
        .collect::<Vec<_>>();
    write_json_stdout(&serde_json::json!({ "results": rendered }))
}

enum ReadyAltitude {
    Milestone(MilestoneMergeSubject),
    Step(MergeSubject),
}

fn already_dispatched(
    graph_node: &DispatchGraphNode,
    dispatches: &[pce_core::DispatchObservation],
) -> bool {
    match &graph_node.node {
        DispatchNode::Step(_) => dispatches.iter().any(|dispatch| {
            dispatch.node() == &graph_node.node_id
                && DispatchRoleClass::classify(dispatch.role()) == DispatchRoleClass::Execution
        }),
        DispatchNode::Milestone(node) => dispatches.iter().any(|dispatch| {
            DispatchRoleClass::classify(dispatch.role()) == DispatchRoleClass::PlanProducing
                && dispatch.role().as_str() == "step-planner"
                && StepNode::parse(dispatch.node())
                    .is_ok_and(|step| step.milestone() == node.milestone())
        }),
    }
}

fn dispatch_node_id(node: &DispatchNode) -> String {
    match node {
        DispatchNode::Milestone(node) => format!("m{}", node.milestone().get()),
        DispatchNode::Step(node) => {
            format!("m{}-s{}", node.milestone().get(), node.step().get())
        }
    }
}

fn parse_dispatch_graph(bytes: &[u8]) -> Result<DispatchGraph> {
    let value: Value = serde_json::from_slice(bytes).context("graph is not valid JSON")?;
    let root = value.as_object().context("graph root must be an object")?;
    require_exact_keys(root, &["nodes"]).context("graph root has invalid keys")?;
    let raw_nodes = root["nodes"]
        .as_array()
        .context("graph nodes must be an array")?;
    let mut nodes = Vec::<DispatchGraphNode>::with_capacity(raw_nodes.len());

    for (index, value) in raw_nodes.iter().enumerate() {
        let object = value
            .as_object()
            .with_context(|| format!("graph node {index} must be an object"))?;
        require_exact_keys(object, &["id", "title", "repo", "depends_on", "summary"])
            .with_context(|| format!("graph node {index} has invalid keys"))?;
        let raw_id = object["id"]
            .as_str()
            .with_context(|| format!("graph node {index} id must be a string"))?;
        let node_id = NodeId::parse(raw_id)
            .with_context(|| format!("graph node {index} id must be non-empty"))?;
        if nodes.iter().any(|node| node.node_id == node_id) {
            bail!("graph contains duplicate node id {raw_id}");
        }
        object["title"]
            .as_str()
            .with_context(|| format!("graph node {raw_id} title must be a string"))?;
        let raw_repository = object["repo"]
            .as_str()
            .with_context(|| format!("graph node {raw_id} repo must be a string"))?;
        if raw_repository.is_empty() {
            bail!("graph node {raw_id} repo must be non-empty");
        }
        object["summary"]
            .as_str()
            .with_context(|| format!("graph node {raw_id} summary must be a string"))?;
        object["depends_on"]
            .as_array()
            .with_context(|| format!("graph node {raw_id} depends_on must be an array"))?;

        let milestone = MilestoneNode::parse(&node_id);
        let step = StepNode::parse(&node_id);
        let node = match (milestone, step) {
            (Ok(node), Err(_)) => DispatchNode::Milestone(node),
            (Err(_), Ok(node)) => DispatchNode::Step(node),
            _ => bail!("graph node id {raw_id:?} is not exactly one canonical node form"),
        };
        nodes.push(DispatchGraphNode {
            node_id,
            node,
            repository: RepositoryName::new(raw_repository),
        });
    }

    let mut edges = Vec::<OrderingEdge>::new();
    for (index, value) in raw_nodes.iter().enumerate() {
        let object = value
            .as_object()
            .context("previously parsed graph node must remain an object")?;
        let dependencies = object["depends_on"]
            .as_array()
            .context("previously parsed dependencies must remain an array")?;
        for (dependency_index, dependency) in dependencies.iter().enumerate() {
            let dependency = dependency.as_object().with_context(|| {
                format!(
                    "graph node {} dependency {dependency_index} must be an object",
                    nodes[index].node_id.as_str()
                )
            })?;
            require_exact_keys(dependency, &["id", "reason"]).with_context(|| {
                format!(
                    "graph node {} dependency {dependency_index} has invalid keys",
                    nodes[index].node_id.as_str()
                )
            })?;
            let raw_id = dependency["id"].as_str().with_context(|| {
                format!(
                    "graph node {} dependency {dependency_index} id must be a string",
                    nodes[index].node_id.as_str()
                )
            })?;
            if raw_id.is_empty() {
                bail!(
                    "graph node {} dependency {dependency_index} id must be non-empty",
                    nodes[index].node_id.as_str()
                );
            }
            let reason = dependency["reason"].as_str().with_context(|| {
                format!(
                    "graph node {} dependency {dependency_index} reason must be a string",
                    nodes[index].node_id.as_str()
                )
            })?;
            if reason.is_empty() {
                bail!(
                    "graph node {} dependency {dependency_index} reason must be non-empty",
                    nodes[index].node_id.as_str()
                );
            }
            let dependency_node = nodes
                .iter()
                .find(|node| node.node_id.as_str() == raw_id)
                .with_context(|| {
                    format!(
                        "graph node {} has dangling dependency id {raw_id}",
                        nodes[index].node_id.as_str()
                    )
                })?;
            edges.push(OrderingEdge::new(
                nodes[index].node.clone(),
                dependency_node.node.clone(),
            ));
        }
    }
    Ok(DispatchGraph { nodes, edges })
}

#[derive(Debug)]
struct ParsedEventLine {
    raw: String,
    record: EventRecord,
}

fn read_event_log(path: &Path) -> Result<Vec<ParsedEventLine>> {
    let file =
        File::open(path).with_context(|| format!("failed to open event log {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::<ParsedEventLine>::new();
    let mut physical_line = 0_u64;
    let mut buffer = String::new();
    loop {
        buffer.clear();
        let bytes = reader.read_line(&mut buffer).with_context(|| {
            format!(
                "failed to read physical line {} from event log {}",
                physical_line + 1,
                path.display()
            )
        })?;
        if bytes == 0 {
            break;
        }
        physical_line += 1;
        let parsed = buffer.strip_suffix('\n').unwrap_or(&buffer);
        let record = parse_event_line(parsed).with_context(|| {
            format!(
                "failed to parse physical line {physical_line} from event log {}",
                path.display()
            )
        })?;
        if let Some(previous) = lines.last()
            && record.sequence().get() <= previous.record.sequence().get()
        {
            bail!(
                "event log {} has non-increasing sequence {} at physical line {} after {}",
                path.display(),
                record.sequence().get(),
                physical_line,
                previous.record.sequence().get()
            );
        }
        lines.push(ParsedEventLine {
            raw: buffer.clone(),
            record,
        });
    }
    Ok(lines)
}

fn run_log_read(path: &Path, filter: &EventRecordFilter, output: &mut dyn Write) -> Result<()> {
    let lines = read_event_log(path)?;
    for line in &lines {
        if event_record_matches(&line.record, filter) {
            output.write_all(line.raw.as_bytes()).with_context(|| {
                format!("failed to write selected event from {}", path.display())
            })?;
        }
    }
    output
        .flush()
        .with_context(|| format!("failed to flush selected events from {}", path.display()))
}

fn repository_contracts(records: &[EventRecord]) -> Result<Vec<RepositoryContract>> {
    let mut contracts = Vec::<RepositoryContract>::new();
    for record in records {
        if let EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) = record.body_ref() {
            let root = PathBuf::from(payload.repo_root.as_str());
            if root.as_os_str().is_empty() {
                bail!(
                    "repository contract at sequence {} has an empty repo_root",
                    record.sequence().get()
                );
            }
            if contracts
                .iter()
                .any(|item| item.name == payload.repository || item.root == root)
            {
                bail!(
                    "ambiguous duplicate repository contract at sequence {} for repository {} and root {}",
                    record.sequence().get(),
                    payload.repository.as_str(),
                    root.display()
                );
            }
            contracts.push(RepositoryContract {
                name: payload.repository.clone(),
                root,
            });
        }
    }
    if contracts.is_empty() {
        bail!("event log contains no repository-contract record");
    }
    Ok(contracts)
}

fn resolve_primary_repository(
    contracts: &[RepositoryContract],
    log_path: &Path,
    vision_dir: &Path,
) -> Result<usize> {
    let log = absolute_path(log_path)?;
    let vision = absolute_path(vision_dir)?;
    let mut matches = Vec::<usize>::new();
    for (index, contract) in contracts.iter().enumerate() {
        let root = absolute_path(&contract.root).with_context(|| {
            format!(
                "failed to resolve repository root {}",
                contract.root.display()
            )
        })?;
        if log.starts_with(&root) && vision.starts_with(&root) {
            matches.push(index);
        }
    }
    match matches.as_slice() {
        [index] => Ok(*index),
        _ => bail!(
            "expected exactly one primary repository containing both log {} and vision {}, found {}",
            log.display(),
            vision.display(),
            matches.len()
        ),
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("failed to resolve current directory")?
            .join(path))
    }
}

fn vision_slug(vision_dir: &Path) -> Result<VisionSlug> {
    let basename = vision_dir
        .file_name()
        .context("vision directory must have a final basename")?
        .to_str()
        .context("vision directory basename must be valid Unicode")?;
    VisionSlug::parse(basename).context("failed to parse vision directory basename")
}

fn current_artifacts(
    records: &[EventRecord],
    primary_root: &Path,
) -> Result<(Vec<CurrentArtifactObservation>, Vec<CurrentArtifactBytes>)> {
    let mut paths = Vec::<ArtifactPath>::new();
    for record in records {
        if let EventBodyRef::Known(KnownPayload::PlanningArtifactApproved(payload)) =
            record.body_ref()
            && !paths.iter().any(|path| path == &payload.path)
        {
            paths.push(payload.path.clone());
        }
    }

    let mut observations = Vec::with_capacity(paths.len());
    let mut retained_bytes = Vec::with_capacity(paths.len());
    for path in paths {
        let recorded = PathBuf::from(path.as_str());
        let resolved = if recorded.is_absolute() {
            recorded
        } else {
            primary_root.join(recorded)
        };
        let (state, bytes) = match std::fs::read(&resolved) {
            Ok(bytes) => {
                let digest = format!("{:x}", Sha256::digest(&bytes));
                let state = CurrentArtifactState::Present {
                    digest: Sha256Digest::parse(&digest).with_context(|| {
                        format!(
                            "failed to parse SHA-256 digest for artifact {}",
                            resolved.display()
                        )
                    })?,
                };
                (state, Some(bytes))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (CurrentArtifactState::Missing, None)
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to read approved artifact {}", resolved.display())
                });
            }
        };
        observations.push(CurrentArtifactObservation::new(path.clone(), state));
        retained_bytes.push(CurrentArtifactBytes { path, bytes });
    }
    Ok((observations, retained_bytes))
}

struct CurrentArtifactBytes {
    path: ArtifactPath,
    bytes: Option<Vec<u8>>,
}

fn canonical_nodes(records: &[EventRecord], vision: &VisionSlug) -> Result<Vec<CanonicalNode>> {
    let mut nodes = Vec::<CanonicalNode>::new();
    for record in records {
        let step = match StepNode::parse(record.node()) {
            Ok(step) => step,
            Err(
                pce_core::RunStateError::MalformedStepNode { .. }
                | pce_core::RunStateError::LeadingZeroStepNodeComponent { .. }
                | pce_core::RunStateError::ZeroStepNodeComponent { .. }
                | pce_core::RunStateError::StepNodeComponentOverflow { .. },
            ) => continue,
            Err(error) => return Err(error).context("failed to classify event-log step node"),
        };
        if let Some(existing) = nodes.iter_mut().find(|item| item.node == *record.node()) {
            existing.latest_sequence = record.sequence().get();
        } else {
            nodes.push(CanonicalNode {
                node: record.node().clone(),
                subject: MergeSubject::derive(vision, step),
                latest_sequence: record.sequence().get(),
            });
        }
    }
    Ok(nodes)
}

fn integration_branches(nodes: &[CanonicalNode], selected_index: usize) -> Vec<String> {
    let mut branches = Vec::<String>::new();
    for (index, node) in nodes.iter().enumerate() {
        if index == selected_index {
            continue;
        }
        let branch = node.subject.integration_branch().as_str();
        if !branches.iter().any(|existing| existing == branch) {
            branches.push(branch.to_owned());
        }
    }
    let selected = nodes[selected_index]
        .subject
        .integration_branch()
        .as_str()
        .to_owned();
    if let Some(index) = branches.iter().position(|branch| branch == &selected) {
        branches.remove(index);
    }
    branches.push(selected);
    branches
}

fn observe_repository(
    contract: RepositoryContract,
    integration_branches: &[String],
    selector: &PullRequestSelector,
) -> Result<(RepositoryObservation, RepositoryRuntime)> {
    let remote = verify_origin(&contract.root);
    let mut fetches = Vec::with_capacity(integration_branches.len());
    for branch in integration_branches {
        let result = match &remote {
            Ok(()) => fetch_branch(&contract.root, branch),
            Err(detail) => FetchResult::Unavailable {
                detail: detail.clone(),
            },
        };
        fetches.push(BranchFetch {
            branch: branch.clone(),
            result,
        });
    }
    let selected_branch = selector.base().as_str();
    let selected_fetch = fetches
        .iter()
        .find(|fetch| fetch.branch == selected_branch)
        .context("selected integration branch has no fetch result")?;
    let fetch = match &selected_fetch.result {
        FetchResult::Observed { oid, fetched_at } => RepositoryFetchObservation::Observed {
            observation_ref: RepositoryObservationRef::parse(oid)
                .context("failed to parse repository observation ref")?,
            fetched_at: EventTimestamp::new(chrono::DateTime::<chrono::Utc>::from(*fetched_at)),
        },
        FetchResult::Unavailable { detail } => RepositoryFetchObservation::Unavailable {
            failure: RepositoryObservationFailure::parse(detail)
                .context("failed to parse repository fetch failure")?,
        },
    };
    let branch_state = probe_branch(&contract.root, selected_branch)?;
    let head = selector.head().as_str();
    let worktree_state = probe_worktree(&contract.root, head)?;
    let tag_state = probe_tag(&contract.root)?;
    let observation = RepositoryObservation::new(
        contract.name.clone(),
        fetch,
        RepositoryBranchName::parse(selected_branch)
            .context("failed to parse repository branch name")?,
        branch_state,
        WorktreeIdentity::parse(head).context("failed to parse worktree identity")?,
        worktree_state,
        TagName::parse(RELEASE_TAG).context("failed to parse release tag name")?,
        tag_state,
    );
    Ok((
        observation,
        RepositoryRuntime {
            name: contract.name,
            root: contract.root,
            fetches,
        },
    ))
}

fn verify_origin(root: &Path) -> std::result::Result<(), String> {
    let args = git_args(root, ["remote", "get-url", ORIGIN]);
    match execute_process("git", &args, None) {
        ProcessAttempt::SpawnFailed { detail } => Err(detail),
        ProcessAttempt::Completed(result) if result.status.success() => {
            match std::str::from_utf8(&result.stdout) {
                Ok(url) if !url.trim().is_empty() => Ok(()),
                Ok(_) => Err(format!(
                    "{}; origin remote URL output was empty",
                    result.detail()
                )),
                Err(error) => Err(format!(
                    "{}; origin remote URL output was not UTF-8: {error}",
                    result.detail()
                )),
            }
        }
        ProcessAttempt::Completed(result) => Err(result.detail()),
    }
}

fn fetch_branch(root: &Path, branch: &str) -> FetchResult {
    let refspec = format!("refs/heads/{branch}");
    let args = git_args(root, ["fetch", "--no-tags", ORIGIN, refspec.as_str()]);
    let fetch = match execute_process("git", &args, None) {
        ProcessAttempt::SpawnFailed { detail } => {
            return FetchResult::Unavailable { detail };
        }
        ProcessAttempt::Completed(result) if result.status.success() => result,
        ProcessAttempt::Completed(result) => {
            return FetchResult::Unavailable {
                detail: result.detail(),
            };
        }
    };
    let fetched_at = SystemTime::now();
    let args = git_args(root, ["rev-parse", "--verify", "FETCH_HEAD^{commit}"]);
    match execute_process("git", &args, None) {
        ProcessAttempt::SpawnFailed { detail } => FetchResult::Unavailable { detail },
        ProcessAttempt::Completed(result) if result.status.success() => {
            match one_nonempty_line(&result.stdout) {
                Ok(oid) => FetchResult::Observed { oid, fetched_at },
                Err(error) => FetchResult::Unavailable {
                    detail: format!(
                        "{}; successful fetch `{}` had invalid FETCH_HEAD resolution: {error}",
                        result.detail(),
                        fetch.command
                    ),
                },
            }
        }
        ProcessAttempt::Completed(result) => FetchResult::Unavailable {
            detail: result.detail(),
        },
    }
}

fn probe_branch(root: &Path, branch: &str) -> Result<BranchState> {
    let reference = format!("refs/heads/{branch}");
    let args = git_args(
        root,
        ["show-ref", "--verify", "--quiet", reference.as_str()],
    );
    match require_spawn(execute_process("git", &args, None))? {
        result if result.status.success() => Ok(BranchState::Present),
        result if result.status.code() == Some(1) => Ok(BranchState::Absent),
        result => bail!("failed to probe repository branch: {}", result.detail()),
    }
}

fn probe_worktree(root: &Path, head: &str) -> Result<WorktreeState> {
    let args = git_args(root, ["worktree", "list", "--porcelain"]);
    let result = require_spawn(execute_process("git", &args, None))?;
    if !result.status.success() {
        bail!("failed to list repository worktrees: {}", result.detail());
    }
    let text = std::str::from_utf8(&result.stdout)
        .with_context(|| format!("worktree output is not UTF-8: {}", result.detail()))?;
    let stanzas: Vec<&str> = text
        .trim_end_matches('\n')
        .split("\n\n")
        .filter(|stanza| !stanza.is_empty())
        .collect();
    if stanzas.is_empty()
        || stanzas
            .iter()
            .any(|stanza| !stanza.starts_with("worktree "))
    {
        bail!("malformed successful worktree output: {}", result.detail());
    }
    for stanza in &stanzas {
        validate_worktree_stanza(stanza).with_context(|| {
            format!("malformed successful worktree output: {}", result.detail())
        })?;
    }
    let expected = format!("branch refs/heads/{head}");
    let matches = stanzas
        .iter()
        .filter(|stanza| stanza.lines().any(|line| line == expected))
        .count();
    match matches {
        0 => Ok(WorktreeState::Absent),
        1 => Ok(WorktreeState::Present),
        _ => bail!(
            "multiple worktrees match exact branch {head}: {}",
            result.detail()
        ),
    }
}

fn validate_worktree_stanza(stanza: &str) -> Result<()> {
    let mut lines = stanza.lines();
    let worktree = lines
        .next()
        .context("worktree stanza is missing worktree line")?;
    if worktree.strip_prefix("worktree ").is_none_or(str::is_empty) {
        bail!("worktree stanza has an empty worktree path");
    }
    let mut head_count = 0_u8;
    let mut location_count = 0_u8;
    for line in lines {
        if line
            .strip_prefix("HEAD ")
            .is_some_and(|value| !value.is_empty())
        {
            head_count += 1;
        } else if line
            .strip_prefix("branch ")
            .is_some_and(|value| !value.is_empty())
            || line == "detached"
            || line == "bare"
        {
            location_count += 1;
        } else if line == "locked" || line.starts_with("locked ") || line.starts_with("prunable ") {
        } else {
            bail!("unrecognized or empty worktree porcelain line {line:?}");
        }
    }
    if head_count > 1 || location_count != 1 {
        bail!("worktree stanza has invalid HEAD or branch cardinality");
    }
    Ok(())
}

fn probe_tag(root: &Path) -> Result<TagState> {
    let reference = format!("refs/tags/{RELEASE_TAG}^{{}}");
    let args = git_args(
        root,
        ["rev-parse", "--verify", "--quiet", reference.as_str()],
    );
    match require_spawn(execute_process("git", &args, None))? {
        result if result.status.success() => {
            let target = one_nonempty_line(&result.stdout)
                .with_context(|| format!("malformed successful tag output: {}", result.detail()))?;
            Ok(TagState::PointsTo {
                target: TagTarget::parse(&target).context("failed to parse tag target")?,
            })
        }
        result if result.status.code() == Some(1) => Ok(TagState::Absent),
        result => bail!("failed to probe repository tag: {}", result.detail()),
    }
}

fn observe_authorities(
    nodes: &[CanonicalNode],
    primary: &RepositoryRuntime,
) -> Result<Vec<StepAuthorityObservation>> {
    let mut observations = Vec::with_capacity(nodes.len());
    for node in nodes {
        let selector = node.subject.selector();
        let github = observe_github(&primary.root, selector)?;
        let git = observe_git(primary, selector, &github)?;
        observations.push(StepAuthorityObservation::new(
            node.node.clone(),
            github,
            git,
        ));
    }
    Ok(observations)
}

fn github_pull_request_list_args(selector: &PullRequestSelector) -> Vec<OsString> {
    vec![
        OsString::from("pr"),
        OsString::from("list"),
        OsString::from("--head"),
        OsString::from(selector.head().as_str()),
        OsString::from("--base"),
        OsString::from(selector.base().as_str()),
        OsString::from("--state"),
        OsString::from("all"),
        OsString::from("--limit"),
        OsString::from("1000"),
        OsString::from("--json"),
        OsString::from("number,headRefName,baseRefName,state,mergeCommit"),
    ]
}

fn observe_github(
    root: &Path,
    selector: &PullRequestSelector,
) -> Result<GitHubAuthorityObservation> {
    let args = github_pull_request_list_args(selector);
    let result = match execute_process("gh", &args, Some(root)) {
        ProcessAttempt::SpawnFailed { detail } => {
            return unreachable_github(&detail);
        }
        ProcessAttempt::Completed(result) if result.status.success() => result,
        ProcessAttempt::Completed(result) => return unreachable_github(&result.detail()),
    };
    let value: Value = serde_json::from_slice(&result.stdout)
        .with_context(|| format!("malformed successful gh JSON output: {}", result.detail()))?;
    let array = value.as_array().with_context(|| {
        format!(
            "successful gh output is not a JSON array: {}",
            result.detail()
        )
    })?;
    let mut exact = Vec::<ParsedPullRequest>::new();
    for item in array {
        let parsed = parse_pull_request(item)
            .with_context(|| format!("malformed successful gh output: {}", result.detail()))?;
        if parsed.head == selector.head().as_str() && parsed.base == selector.base().as_str() {
            exact.push(parsed);
        }
    }
    let observation = match exact.as_slice() {
        [] => GitHubPullRequestObservation::ZeroExactMatches,
        [pull_request] => {
            let number = PullRequestNumber::parse(pull_request.number)
                .context("failed to parse GitHub pull-request number")?;
            let identity = ExactPullRequestIdentity::from_selector(number, selector);
            let state = match &pull_request.state {
                ParsedPullRequestState::Merged { oid } => ExactPullRequestState::Merged {
                    squash_commit: SquashCommitOid::parse(oid)
                        .context("failed to parse GitHub merge commit OID")?,
                },
                ParsedPullRequestState::NotMerged => ExactPullRequestState::NotMerged,
            };
            GitHubPullRequestObservation::OneExactMatch { identity, state }
        }
        _ => GitHubPullRequestObservation::MultipleExactMatches,
    };
    Ok(GitHubAuthorityObservation::Reachable { observation })
}

#[derive(Debug)]
struct ParsedPullRequest {
    number: u64,
    head: String,
    base: String,
    state: ParsedPullRequestState,
}

#[derive(Debug)]
enum ParsedPullRequestState {
    Merged { oid: String },
    NotMerged,
}

fn parse_pull_request(value: &Value) -> Result<ParsedPullRequest> {
    let object = value
        .as_object()
        .context("pull-request entry must be an object")?;
    require_exact_keys(
        object,
        &[
            "number",
            "headRefName",
            "baseRefName",
            "state",
            "mergeCommit",
        ],
    )?;
    let number = object["number"]
        .as_u64()
        .context("pull-request number must be a positive JSON integer")?;
    if number == 0 {
        bail!("pull-request number must be positive");
    }
    let head = object["headRefName"]
        .as_str()
        .context("headRefName must be a string")?
        .to_owned();
    let base = object["baseRefName"]
        .as_str()
        .context("baseRefName must be a string")?
        .to_owned();
    let state = object["state"].as_str().context("state must be a string")?;
    let merge_commit = &object["mergeCommit"];
    let state = match state {
        "MERGED" => {
            let merge = merge_commit
                .as_object()
                .context("MERGED pull request requires mergeCommit object")?;
            require_exact_keys(merge, &["oid"])?;
            let oid = merge["oid"]
                .as_str()
                .context("mergeCommit.oid must be a string")?;
            if oid.is_empty() {
                bail!("MERGED pull request requires non-empty mergeCommit.oid");
            }
            ParsedPullRequestState::Merged {
                oid: oid.to_owned(),
            }
        }
        "OPEN" | "CLOSED" if merge_commit.is_null() => ParsedPullRequestState::NotMerged,
        "OPEN" | "CLOSED" => bail!("{state} pull request requires mergeCommit null"),
        _ => bail!("unsupported pull-request state {state:?}"),
    };
    Ok(ParsedPullRequest {
        number,
        head,
        base,
        state,
    })
}

fn require_exact_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<()> {
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        bail!(
            "object keys must be exactly {}, got {}",
            expected.join(","),
            object.keys().cloned().collect::<Vec<_>>().join(",")
        );
    }
    Ok(())
}

fn observe_git(
    primary: &RepositoryRuntime,
    selector: &PullRequestSelector,
    github: &GitHubAuthorityObservation,
) -> Result<GitAuthorityObservation> {
    let branch = selector.base().as_str();
    let fetch = primary
        .fetches
        .iter()
        .find(|fetch| fetch.branch == branch)
        .with_context(|| {
            format!(
                "primary repository {} has no fetch result for branch {branch}",
                primary.name.as_str()
            )
        })?;
    let fetched_oid = match &fetch.result {
        FetchResult::Observed { oid, .. } => oid,
        FetchResult::Unavailable { detail } => return unreachable_git(detail),
    };
    match github {
        GitHubAuthorityObservation::Unreachable { failure } => unreachable_git(&format!(
            "GitHub authority unavailable before git reachability selection: {}",
            failure.as_str()
        )),
        GitHubAuthorityObservation::Reachable {
            observation:
                GitHubPullRequestObservation::ZeroExactMatches
                | GitHubPullRequestObservation::OneExactMatch {
                    state: ExactPullRequestState::NotMerged,
                    ..
                },
        } => Ok(GitAuthorityObservation::Reachable {
            observation: GitMergeObservation::NotMerged,
        }),
        GitHubAuthorityObservation::Reachable {
            observation: GitHubPullRequestObservation::MultipleExactMatches,
        } => unreachable_git("multiple exact GitHub pull requests prevent squash OID selection"),
        GitHubAuthorityObservation::Reachable {
            observation:
                GitHubPullRequestObservation::OneExactMatch {
                    state: ExactPullRequestState::Merged { squash_commit },
                    ..
                },
        } => {
            let args = git_args(
                &primary.root,
                [
                    "merge-base",
                    "--is-ancestor",
                    squash_commit.as_str(),
                    fetched_oid.as_str(),
                ],
            );
            match execute_process("git", &args, None) {
                ProcessAttempt::SpawnFailed { detail } => unreachable_git(&detail),
                ProcessAttempt::Completed(result) if result.status.success() => {
                    Ok(GitAuthorityObservation::Reachable {
                        observation: GitMergeObservation::SquashCommitReachable {
                            squash_commit: squash_commit.clone(),
                        },
                    })
                }
                ProcessAttempt::Completed(result) if result.status.code() == Some(1) => {
                    Ok(GitAuthorityObservation::Reachable {
                        observation: GitMergeObservation::NotMerged,
                    })
                }
                ProcessAttempt::Completed(result) => unreachable_git(&result.detail()),
            }
        }
    }
}

fn unreachable_github(detail: &str) -> Result<GitHubAuthorityObservation> {
    Ok(GitHubAuthorityObservation::Unreachable {
        failure: AuthorityFailure::parse(detail)
            .context("failed to parse GitHub authority failure")?,
    })
}

fn unreachable_git(detail: &str) -> Result<GitAuthorityObservation> {
    Ok(GitAuthorityObservation::Unreachable {
        failure: AuthorityFailure::parse(detail)
            .context("failed to parse git authority failure")?,
    })
}

fn git_args<'a>(root: &Path, args: impl IntoIterator<Item = &'a str>) -> Vec<OsString> {
    let mut result = vec![OsString::from("-C"), root.as_os_str().to_owned()];
    result.extend(args.into_iter().map(OsString::from));
    result
}

fn execute_process(program: &str, args: &[OsString], current_dir: Option<&Path>) -> ProcessAttempt {
    let mut command = std::process::Command::new(program);
    command.args(args);
    if let Some(directory) = current_dir {
        command.current_dir(directory);
    }
    let rendered = render_command(program, args, current_dir);
    match command.output() {
        Ok(Output {
            status,
            stdout,
            stderr,
        }) => ProcessAttempt::Completed(ProcessResult {
            command: rendered,
            status,
            stdout,
            stderr,
        }),
        Err(error) => ProcessAttempt::SpawnFailed {
            detail: format!("failed to spawn command `{rendered}`: {error}"),
        },
    }
}

fn render_command(program: &str, args: &[OsString], current_dir: Option<&Path>) -> String {
    let command = std::iter::once(program.to_owned())
        .chain(args.iter().map(|arg| arg.to_string_lossy().into_owned()))
        .collect::<Vec<_>>()
        .join(" ");
    match current_dir {
        Some(directory) => format!("{command} (cwd {})", directory.display()),
        None => command,
    }
}

fn require_spawn(attempt: ProcessAttempt) -> Result<ProcessResult> {
    match attempt {
        ProcessAttempt::SpawnFailed { detail } => bail!("{detail}"),
        ProcessAttempt::Completed(result) => Ok(result),
    }
}

fn one_nonempty_line(bytes: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(bytes).context("command output is not UTF-8")?;
    let lines: Vec<&str> = text.lines().collect();
    match lines.as_slice() {
        [line] if !line.is_empty() => Ok((*line).to_owned()),
        _ => bail!("expected exactly one non-empty output line"),
    }
}

fn validated_snapshot_value(snapshot: &RunSnapshot<'_>) -> Result<Value> {
    let value = serde_json::to_value(snapshot).context("failed to serialize typed run snapshot")?;
    let schema: Value =
        serde_json::from_str(RUN_SNAPSHOT_SCHEMA).context("failed to parse compiled run schema")?;
    let validator =
        jsonschema::validator_for(&schema).context("failed to compile run snapshot schema")?;
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|error| error.to_string())
        .collect();
    if !errors.is_empty() {
        bail!(
            "run snapshot failed schema validation: {}",
            errors.join("; ")
        );
    }
    Ok(value)
}

fn write_json_stdout(value: &Value) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, value).context("failed to write JSON status to stdout")?;
    output
        .write_all(b"\n")
        .context("failed to terminate JSON status with newline")?;
    output.flush().context("failed to flush JSON status")
}

fn write_human_stdout(rendered: &str) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(rendered.as_bytes())
        .context("failed to write human status to stdout")?;
    output.flush().context("failed to flush human status")
}

fn append_locked(
    file: &mut File,
    payload: String,
    kind: WriteKind,
    node: NodeId,
    path: &Path,
) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("failed to seek event log {} for tail read", path.display()))?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .with_context(|| format!("failed to read event-log tail from {}", path.display()))?;
    let content = String::from_utf8(content)
        .with_context(|| format!("event-log tail in {} is not valid UTF-8", path.display()))?;
    let tail = match content.lines().last() {
        Some(line) => EventLogTail::Present(EventLogTailLine::new(line)),
        None if content.is_empty() => EventLogTail::Empty,
        None => EventLogTail::Present(EventLogTailLine::new("")),
    };

    append_event::<std::io::Error, _>(
        kind,
        UnparsedPayload::new(payload),
        tail,
        node,
        SystemTime::now(),
        |bytes| {
            file.seek(SeekFrom::End(0))?;
            file.write_all(bytes)
        },
    )
    .map_err(classify_append_error)?;

    file.sync_all()
        .with_context(|| format!("failed to sync event log {}", path.display()))?;
    Ok(())
}

fn classify_append_error(error: AppendError<std::io::Error>) -> Error {
    let context = match &error {
        AppendError::MalformedSubmittedPayload { .. } => {
            "failed to parse submitted event payload JSON"
        }
        AppendError::InvalidSubmittedPayload { .. } => "failed to validate submitted event payload",
        AppendError::InvalidTail { .. } => "failed to validate current event-log tail",
        AppendError::SequenceOverflow { .. } => {
            "failed to derive successor from current event-log tail"
        }
        AppendError::SerializationFailed { .. } => "failed to serialize validated event record",
        AppendError::CapabilityFailed { .. } => "failed to append event record",
    };
    anyhow!(error).context(context)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::io::{Cursor, Read};
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    use pce_core::{
        ArtifactPath, BranchState, CurrentArtifactObservation, CurrentArtifactState, EventKindName,
        EventRecordFilter, GitAuthorityObservation, GitHubAuthorityObservation,
        GitHubPullRequestObservation, GitMergeObservation, KnownPayload, MilestoneMergeSubject,
        MilestoneNode, NodeId, ReadKind, ReadPayload, RecoveryLogPath, RepositoryBranchName,
        RepositoryFetchObservation, RepositoryName, RepositoryObservation,
        RepositoryObservationFailure, RunSnapshot, Sha256Digest, StepAuthorityObservation, TagName,
        TagState, VersionPolicy, VisionSlug, WorktreeIdentity, WorktreeState, WriteKind,
        derive_run_state, parse_event_line, render_human_snapshot,
    };
    use tempfile::tempdir;

    use crate::{
        BranchFetch, Command, DispatchGraphNode, DispatchNode, FetchResult, RepositoryRuntime,
        StatusFormat, already_dispatched, github_pull_request_list_args, observe_git,
        parse_command, parse_dispatch_graph, run, run_log_read, validated_snapshot_value,
    };

    const DELTA_PAYLOAD: &str = r#"{"message":"append one validated event"}"#;
    const UNKNOWN_TAIL: &str = r#"{"sequence":41,"timestamp":"2026-07-27T12:34:55.000Z","kind":"future-kind","node":"m1-s1","payload":{"nested":{"answer":42},"items":[true,null,"kept"]}}"#;
    const MULTILINE_PAYLOAD: &str = "{\n  \"finding\":\"the measured fact\",\n  \"evidence\":\"git rev-parse HEAD\\ncargo test --workspace\"\n}";
    const RAW_READ_FIXTURE: &str = concat!(
        r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m2-s1","payload":{"message":"first"}}"#,
        "\n",
        r#"{"sequence":2,"timestamp":"2026-07-27T12:34:57.000Z","kind":"future-kind","node":"m2-s1","payload":{"nested":{"answer":42},"items":[true,null,"kept"]}}"#,
        "\n",
        r#"{"sequence":3,"timestamp":"2026-07-27T12:34:58.000Z","kind":"delta","node":"m2-s2","payload":{"message":"third"}}"#,
    );

    fn log_args(path: &Path, kind: &str) -> Vec<String> {
        vec![
            "log".to_owned(),
            "--file".to_owned(),
            path.to_string_lossy().into_owned(),
            "--kind".to_owned(),
            kind.to_owned(),
            "--node".to_owned(),
            "m1-s3".to_owned(),
        ]
    }

    fn invoke(path: &Path, kind: &str, payload: &str) -> anyhow::Result<()> {
        let mut input = Cursor::new(payload.as_bytes());
        run(log_args(path, kind).into_iter(), &mut input)
    }

    fn bytes_if_present(path: &Path) -> Vec<u8> {
        if path.exists() {
            fs::read(path).expect("log should be readable")
        } else {
            Vec::new()
        }
    }

    #[test]
    fn github_command_routing_uses_selector_exact_ordered_pair() {
        let node = NodeId::parse("m7").expect("milestone node fixture should parse");
        let milestone =
            MilestoneNode::parse(&node).expect("milestone node fixture should classify");
        let subject = MilestoneMergeSubject::derive(milestone);
        let selector = subject.selector();

        assert_eq!(
            github_pull_request_list_args(selector),
            [
                "pr",
                "list",
                "--head",
                "milestone-7",
                "--base",
                "main",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn git_fetch_selection_uses_selector_base() {
        let node = NodeId::parse("m7").expect("milestone node fixture should parse");
        let milestone =
            MilestoneNode::parse(&node).expect("milestone node fixture should classify");
        let subject = MilestoneMergeSubject::derive(milestone);
        let selector = subject.selector();
        let runtime = RepositoryRuntime {
            name: RepositoryName::new("primary"),
            root: PathBuf::from("unused"),
            fetches: vec![
                BranchFetch {
                    branch: "milestone-7".to_owned(),
                    result: FetchResult::Observed {
                        oid: "decoy-head-fetch".to_owned(),
                        fetched_at: SystemTime::UNIX_EPOCH,
                    },
                },
                BranchFetch {
                    branch: "main".to_owned(),
                    result: FetchResult::Unavailable {
                        detail: "selected-main-fetch".to_owned(),
                    },
                },
            ],
        };
        let github = GitHubAuthorityObservation::Reachable {
            observation: GitHubPullRequestObservation::ZeroExactMatches,
        };

        let observation =
            observe_git(&runtime, selector, &github).expect("git observation should derive");

        match observation {
            GitAuthorityObservation::Unreachable { failure } => {
                assert_eq!(failure.as_str(), "selected-main-fetch");
            }
            other => panic!("expected selected base fetch failure, got {other:?}"),
        }
    }

    #[test]
    fn consecutive_writes_append_exact_five_key_envelopes() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("events.jsonl");

        invoke(&path, "delta", DELTA_PAYLOAD).expect("first append should succeed");
        invoke(&path, "delta", DELTA_PAYLOAD).expect("second append should succeed");

        let content = fs::read_to_string(&path).expect("log should be UTF-8");
        assert!(content.ends_with('\n'));
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        for (line, sequence) in lines.iter().zip([1, 2]) {
            assert!(line.starts_with(&format!(r#"{{"sequence":{sequence},"timestamp":""#)));
            assert!(line.ends_with(
                r#"","kind":"delta","node":"m1-s3","payload":{"message":"append one validated event"}}"#
            ));
            parse_event_line(line).expect("written line should parse");
        }
    }

    #[test]
    fn accepts_all_seven_registered_payload_schemas() {
        let directory = tempdir().expect("temporary directory should create");
        let fixtures = [
            (
                "dispatch",
                r#"{"role":"step-executor","ref":"ca9788ded3daec9b9e9fd7679caa24e7c64a8193","evidence":"git rev-parse HEAD"}"#,
                WriteKind::Dispatch,
            ),
            (
                "delta",
                r#"{"message":"Require exact UTC timestamp spelling in the event envelope."}"#,
                WriteKind::Delta,
            ),
            (
                "escalation-open",
                r#"{"key":"timestamp-precision","question":"Which RFC 3339 sub-second precision is canonical?"}"#,
                WriteKind::EscalationOpen,
            ),
            (
                "escalation-close",
                r#"{"key":"timestamp-precision","resolution":"Use milliseconds and a Z suffix."}"#,
                WriteKind::EscalationClose,
            ),
            (
                "key-finding",
                r##"{"finding":"The repository has exactly five tests at the ground-truth ref.","evidence":"git grep -n '#[test]' ca9788ded3daec9b9e9fd7679caa24e7c64a8193 -- crates/core/src/vision.rs"}"##,
                WriteKind::KeyFinding,
            ),
            (
                "repository-contract",
                r#"{"repository":"pce","repo_root":"/workspace/pce","stack":"Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)","format":"cargo fmt --all --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --workspace","preflight":"cargo check --workspace --all-targets","gates_rule":"From the repo root, all four gates must exit zero before committing.","install":"None required for gates.","evidence":"rustc --version\ncargo --version\ngit rev-parse --show-toplevel"}"#,
                WriteKind::RepositoryContract,
            ),
            (
                "planning-artifact-approved",
                r#"{"path":"planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","evidence":"shasum -a 256 planning/2026-07-27-event-log-and-derived-run-state/milestone-1/steps.json"}"#,
                WriteKind::PlanningArtifactApproved,
            ),
        ];

        for (index, (kind, payload, expected_kind)) in fixtures.into_iter().enumerate() {
            let path = directory.path().join(format!("event-{index}.jsonl"));
            invoke(&path, kind, payload).expect("registered payload should append");
            let content = fs::read_to_string(path).expect("log should be readable");
            let record = parse_event_line(content.trim_end_matches('\n'))
                .expect("registered output should parse");
            assert_eq!(record.kind(), ReadKind::Known(expected_kind));
        }
    }

    #[test]
    fn rejected_kinds_and_payloads_append_no_bytes() {
        let directory = tempdir().expect("temporary directory should create");
        let fixtures = [
            ("key-finding", r#"{"finding":"no evidence supplied"}"#),
            (
                "delta",
                r#"{"message":"wrong evidence","evidence":"must be absent"}"#,
            ),
            ("future-kind", DELTA_PAYLOAD),
            ("delta", "{"),
            ("delta", "{}"),
        ];

        for (index, (kind, payload)) in fixtures.into_iter().enumerate() {
            let path = directory.path().join(format!("rejected-{index}.jsonl"));
            assert!(invoke(&path, kind, payload).is_err());
            assert_eq!(bytes_if_present(&path), Vec::<u8>::new());
        }
    }

    #[test]
    fn malformed_tails_are_preserved_byte_for_byte() {
        let directory = tempdir().expect("temporary directory should create");
        let fixtures = [
            "{",
            r#"{"sequence":41,"timestamp":"2026-07-27T12:34:55.000Z","kind":"delta","node":"m1-s1"}"#,
        ];

        for (index, tail) in fixtures.into_iter().enumerate() {
            let path = directory
                .path()
                .join(format!("malformed-tail-{index}.jsonl"));
            fs::write(&path, tail).expect("tail fixture should seed");
            let before = fs::read(&path).expect("seed should read");
            assert!(invoke(&path, "delta", DELTA_PAYLOAD).is_err());
            assert_eq!(fs::read(&path).expect("log should read"), before);
        }
    }

    #[test]
    fn unknown_kind_tail_remains_readable_and_yields_successor_42() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("unknown-tail.jsonl");
        let seeded = format!("{UNKNOWN_TAIL}\n");
        fs::write(&path, seeded.as_bytes()).expect("unknown tail should seed");

        invoke(&path, "delta", DELTA_PAYLOAD).expect("successor should append");

        let content = fs::read_to_string(&path).expect("log should read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(format!("{}\n", lines[0]).as_bytes(), seeded.as_bytes());
        let original = parse_event_line(lines[0]).expect("unknown record should remain readable");
        assert_eq!(original.kind(), ReadKind::Unknown("future-kind".to_owned()));
        let successor = parse_event_line(lines[1]).expect("successor should parse");
        assert_eq!(successor.sequence().get(), 42);
        assert_eq!(successor.kind(), ReadKind::Known(WriteKind::Delta));
    }

    #[test]
    fn multiline_stdin_compacts_and_round_trips_logical_newline() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("multiline.jsonl");

        invoke(&path, "key-finding", MULTILINE_PAYLOAD).expect("multiline payload should append");

        let content = fs::read_to_string(&path).expect("log should read");
        assert!(content.ends_with('\n'));
        assert_eq!(content.lines().count(), 1);
        let record = parse_event_line(content.trim_end_matches('\n')).expect("record should parse");
        let ReadPayload::Known(KnownPayload::KeyFinding(payload)) = record.payload() else {
            panic!("record should contain a key-finding payload");
        };
        assert_eq!(
            payload.evidence.as_str(),
            "git rev-parse HEAD\ncargo test --workspace"
        );
    }

    #[test]
    fn vision_routing_does_not_read_injected_input() {
        struct FailingReader;

        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                panic!("vision command must not read stdin")
            }
        }

        let args = vec!["vision".to_owned(), "new".to_owned(), String::new()];
        let error = run(args.into_iter(), &mut FailingReader)
            .expect_err("empty vision name should fail before filesystem work");
        assert!(error.to_string().contains("failed to parse vision name"));
    }

    #[test]
    fn log_parser_requires_literal_flag_order_and_no_payload_argument() {
        let directory = tempdir().expect("temporary directory should create");
        let path: PathBuf = directory.path().join("events.jsonl");
        let mut reordered = log_args(&path, "delta");
        reordered.swap(1, 3);
        let mut input = Cursor::new(DELTA_PAYLOAD.as_bytes());
        assert!(run(reordered.into_iter(), &mut input).is_err());

        let mut extra = log_args(&path, "delta");
        extra.push(DELTA_PAYLOAD.to_owned());
        let mut input = Cursor::new(Vec::<u8>::new());
        assert!(run(extra.into_iter(), &mut input).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn typed_parser_accepts_exact_vision_log_and_status_forms() {
        assert!(matches!(
            parse_command(
                ["vision", "new", "Event log"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .expect("vision command should parse"),
            Command::VisionNew { .. }
        ));

        let command = parse_command(
            [
                "log",
                "--file",
                "events.jsonl",
                "--kind",
                "delta",
                "--node",
                "m2-s4",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("log command should parse");
        let Command::LogWrite { path, kind, node } = command else {
            panic!("typed log-write command expected");
        };
        assert_eq!(path, PathBuf::from("events.jsonl"));
        assert_eq!(kind, WriteKind::Delta);
        assert_eq!(
            node,
            NodeId::parse("m2-s4").expect("node fixture should parse")
        );

        let command = parse_command(
            [
                "status",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-27-event-log",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("status command should parse without reaching adapters");
        let Command::Status {
            log_path,
            recovery_log_path,
            vision_dir,
            format,
        } = command
        else {
            panic!("typed status command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(recovery_log_path.as_str(), "events.jsonl");
        assert_eq!(vision_dir, PathBuf::from("planning/2026-07-27-event-log"));
        assert_eq!(format, StatusFormat::Json);

        let command = parse_command(
            [
                "status",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-27-event-log",
                "--human",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("trailing human status command should parse");
        assert!(matches!(
            command,
            Command::Status {
                format: StatusFormat::Human,
                ..
            }
        ));
    }

    #[test]
    fn ready_parser_accepts_exact_ordered_typed_policies() {
        let command = parse_command(
            [
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "pce=NONE",
                "--policy",
                "docs=SERIALIZE_DISPATCHES",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("exact ready command should parse");
        let Command::Ready {
            log_path,
            recovery_log_path,
            vision_dir,
            version_policies,
        } = command
        else {
            panic!("typed ready command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(recovery_log_path.as_str(), "events.jsonl");
        assert_eq!(
            VisionSlug::parse(
                vision_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("vision basename")
            )
            .expect("vision slug")
            .as_str(),
            "example"
        );
        assert_eq!(
            version_policies,
            vec![
                (RepositoryName::new("pce"), VersionPolicy::None),
                (
                    RepositoryName::new("docs"),
                    VersionPolicy::SerializeDispatches
                ),
            ]
        );
    }

    #[test]
    fn ready_parser_rejects_every_non_contract_shape() {
        let invalid = [
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
            ],
            vec![
                "ready",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--file",
                "events.jsonl",
                "--policy",
                "pce=NONE",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "=NONE",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "pceNONE",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "pce=NONE=NONE",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "pce=none",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "pce=NONE",
                "--policy",
                "pce=SERIALIZE_DISPATCHES",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "pce=NONE",
                "extra",
            ],
            vec![
                "ready",
                "--file",
                "--events",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "pce=NONE",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "--vision",
                "--policy",
                "pce=NONE",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--policy",
                "--value",
            ],
        ];
        for args in invalid {
            assert!(
                parse_command(args.into_iter().map(str::to_owned)).is_err(),
                "invalid ready arguments must fail"
            );
        }
    }

    #[test]
    fn dispatch_graph_parser_preserves_altitudes_and_reason_edges() {
        let graph = parse_dispatch_graph(
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[],"summary":"M"},{"id":"m1-s1","title":"S1","repo":"pce","depends_on":[],"summary":"S1"},{"id":"m1-s2","title":"S2","repo":"docs","depends_on":[{"id":"m1","reason":"milestone base"},{"id":"m1-s1","reason":"required API"}],"summary":"S2"}]}"#,
        )
        .expect("closed graph should parse");
        assert!(matches!(graph.nodes[0].node, DispatchNode::Milestone(_)));
        assert!(matches!(graph.nodes[1].node, DispatchNode::Step(_)));
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.edges[0].dependent(), &graph.nodes[2].node);
        assert_eq!(graph.edges[0].dependency(), &graph.nodes[0].node);
        assert_eq!(graph.edges[1].dependent(), &graph.nodes[2].node);
        assert_eq!(graph.edges[1].dependency(), &graph.nodes[1].node);
    }

    #[test]
    fn dispatch_graph_parser_rejects_closed_shape_violations() {
        let fixtures: &[&[u8]] = &[
            b"{",
            b"[]",
            br#"{"nodes":[],"extra":1}"#,
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[],"summary":"M","extra":1}]}"#,
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[]}]}"#,
            br#"{"nodes":[{"id":"m1","title":1,"repo":"pce","depends_on":[],"summary":"M"}]}"#,
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[{"id":"","reason":"x"}],"summary":"M"}]}"#,
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[{"id":"m1","reason":""}],"summary":"M"}]}"#,
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[{"id":"m1","reason":"x","extra":1}],"summary":"M"}]}"#,
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[],"summary":"M"},{"id":"m1","title":"M","repo":"pce","depends_on":[],"summary":"M"}]}"#,
            br#"{"nodes":[{"id":"m1","title":"M","repo":"pce","depends_on":[{"id":"m2","reason":"x"}],"summary":"M"}]}"#,
            br#"{"nodes":[{"id":"not-canonical","title":"M","repo":"pce","depends_on":[],"summary":"M"}]}"#,
        ];
        for fixture in fixtures {
            assert!(
                parse_dispatch_graph(fixture).is_err(),
                "malformed graph fixture must fail: {}",
                String::from_utf8_lossy(fixture)
            );
        }
    }

    #[test]
    fn dispatch_history_filter_is_altitude_and_role_specific() {
        let lines = [
            r#"{"sequence":1,"timestamp":"2026-07-28T12:00:00.000Z","kind":"dispatch","node":"m3-s1","payload":{"role":"step-executor","ref":"1111111111111111111111111111111111111111","evidence":"execution"}}"#,
            r#"{"sequence":2,"timestamp":"2026-07-28T12:00:01.000Z","kind":"dispatch","node":"m3-s2","payload":{"role":"step-plan-writer","ref":"2222222222222222222222222222222222222222","evidence":"planning"}}"#,
            r#"{"sequence":3,"timestamp":"2026-07-28T12:00:02.000Z","kind":"dispatch","node":"m3-s2","payload":{"role":"step-critic","ref":"3333333333333333333333333333333333333333","evidence":"critique"}}"#,
            r#"{"sequence":4,"timestamp":"2026-07-28T12:00:03.000Z","kind":"dispatch","node":"m4-s1","payload":{"role":"step-planner","ref":"4444444444444444444444444444444444444444","evidence":"descent"}}"#,
            r#"{"sequence":5,"timestamp":"2026-07-28T12:00:04.000Z","kind":"dispatch","node":"m4-s2","payload":{"role":"milestone-planner","ref":"5555555555555555555555555555555555555555","evidence":"other planning"}}"#,
            r#"{"sequence":6,"timestamp":"2026-07-28T12:00:05.000Z","kind":"dispatch","node":"m5-s1","payload":{"role":"step-plan-writer","ref":"6666666666666666666666666666666666666666","evidence":"other planning"}}"#,
            r#"{"sequence":7,"timestamp":"2026-07-28T12:00:06.000Z","kind":"dispatch","node":"m6-s1","payload":{"role":"step-critic","ref":"7777777777777777777777777777777777777777","evidence":"critique"}}"#,
        ];
        let records = lines
            .iter()
            .map(|line| parse_event_line(line).expect("dispatch fixture should parse"))
            .collect::<Vec<_>>();
        let state = derive_run_state(
            &records,
            &VisionSlug::parse("2026-07-28-example").expect("vision"),
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )
        .expect("dispatch projection should derive");
        let graph_node = |raw: &str, node: DispatchNode| DispatchGraphNode {
            node_id: NodeId::parse(raw).expect("node id"),
            node,
            repository: RepositoryName::new("pce"),
        };
        assert!(already_dispatched(
            &graph_node(
                "m3-s1",
                DispatchNode::Step(
                    pce_core::StepNode::parse(&NodeId::parse("m3-s1").expect("node"))
                        .expect("step")
                )
            ),
            state.dispatches()
        ));
        assert!(!already_dispatched(
            &graph_node(
                "m3-s2",
                DispatchNode::Step(
                    pce_core::StepNode::parse(&NodeId::parse("m3-s2").expect("node"))
                        .expect("step")
                )
            ),
            state.dispatches()
        ));
        for milestone in [4_u64, 5, 6] {
            let raw = format!("m{milestone}");
            let node_id = NodeId::parse(&raw).expect("node");
            let graph_node = graph_node(
                &raw,
                DispatchNode::Milestone(MilestoneNode::parse(&node_id).expect("milestone")),
            );
            assert_eq!(
                already_dispatched(&graph_node, state.dispatches()),
                milestone == 4
            );
        }
    }

    #[test]
    fn status_parser_rejects_every_non_contract_shape() {
        let invalid = [
            vec!["status", "--vision-dir", "vision", "--file", "log"],
            vec!["status", "--file", "log"],
            vec!["status", "--file", "--vision-dir", "vision"],
            vec!["status", "--file", "--file", "--vision-dir", "vision"],
            vec!["status", "--file", "log", "--file", "vision"],
            vec!["status", "--file", "log", "--vision-dir"],
            vec![
                "status",
                "--vision-dir",
                "vision",
                "--human",
                "--file",
                "log",
            ],
            vec![
                "status",
                "--human",
                "--file",
                "log",
                "--vision-dir",
                "vision",
            ],
            vec![
                "status",
                "--file",
                "log",
                "--human",
                "--vision-dir",
                "vision",
            ],
            vec!["status", "--file", "log", "--vision-dir", "vision", "extra"],
            vec![
                "status",
                "--file",
                "log",
                "--vision-dir",
                "vision",
                "--human",
                "--human",
            ],
            vec![
                "status",
                "--file",
                "log",
                "--vision-dir",
                "vision",
                "--human",
                "value",
            ],
            vec![
                "status",
                "--file",
                "log",
                "--vision-dir",
                "vision",
                "--yaml",
            ],
        ];
        for args in invalid {
            let error = parse_command(args.into_iter().map(str::to_owned))
                .expect_err("invalid status shape must be rejected");
            assert_eq!(error.to_string(), super::USAGE);
        }
    }

    #[test]
    fn log_action_seam_rejects_read_reordering_and_payload_arguments() {
        let invalid = [
            vec![
                "log",
                "read",
                "--file",
                "events.jsonl",
                "--node",
                "m2-s4",
                "--kind",
                "delta",
            ],
            vec![
                "log",
                "read",
                "--file",
                "events.jsonl",
                "--kind",
                "delta",
                "--kind",
                "delta",
            ],
            vec!["log", "read", "--file", "events.jsonl", "extra"],
            vec![
                "log",
                "--kind",
                "delta",
                "--file",
                "events.jsonl",
                "--node",
                "m2-s4",
            ],
            vec![
                "log",
                "--file",
                "events.jsonl",
                "--kind",
                "delta",
                "--node",
                "m2-s4",
                "{}",
            ],
        ];
        for args in invalid {
            assert!(parse_command(args.into_iter().map(str::to_owned)).is_err());
        }
    }

    #[test]
    fn log_read_parser_accepts_exact_filters_and_preserves_domain_errors() {
        let legal = [
            (
                vec!["log", "read", "--file", "events.jsonl"],
                EventRecordFilter::All,
            ),
            (
                vec![
                    "log",
                    "read",
                    "--file",
                    "events.jsonl",
                    "--kind",
                    "--future",
                ],
                EventRecordFilter::Kind(EventKindName::new("--future")),
            ),
            (
                vec![
                    "log",
                    "read",
                    "--file",
                    "events.jsonl",
                    "--node",
                    "--node-value",
                ],
                EventRecordFilter::Node(NodeId::parse("--node-value").expect("node")),
            ),
            (
                vec![
                    "log",
                    "read",
                    "--file",
                    "events.jsonl",
                    "--kind",
                    "delta",
                    "--node",
                    "m2-s2",
                ],
                EventRecordFilter::KindAndNode {
                    kind: EventKindName::new("delta"),
                    node: NodeId::parse("m2-s2").expect("node"),
                },
            ),
        ];
        for (args, expected) in legal {
            let Command::LogRead { path, filter } =
                parse_command(args.into_iter().map(str::to_owned)).expect("legal read shape")
            else {
                panic!("log-read command expected");
            };
            assert_eq!(path, PathBuf::from("events.jsonl"));
            assert_eq!(filter, expected);
        }

        let shape_error = parse_command(
            ["log", "read", "--kind", "delta", "--file", "events.jsonl"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect_err("wrong order should fail");
        assert_eq!(shape_error.to_string(), super::USAGE);

        let node_error = parse_command(
            ["log", "read", "--file", "events.jsonl", "--node", ""]
                .into_iter()
                .map(str::to_owned),
        )
        .expect_err("empty node should fail");
        assert_eq!(
            node_error.to_string(),
            "failed to parse log-read node filter"
        );
        assert!(!node_error.to_string().contains("usage:"));

        let write_error = parse_command(
            [
                "log",
                "--file",
                "events.jsonl",
                "--kind",
                "future-kind",
                "--node",
                "m2-s2",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect_err("unknown write kind should fail");
        assert_eq!(write_error.to_string(), "failed to parse event kind");
    }

    #[test]
    fn log_read_preserves_exact_selected_raw_bytes() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("events.jsonl");
        fs::write(&path, RAW_READ_FIXTURE).expect("fixture should seed");
        let fixtures = [
            (EventRecordFilter::All, RAW_READ_FIXTURE.as_bytes().to_vec()),
            (
                EventRecordFilter::Kind(EventKindName::new("delta")),
                concat!(
                    r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m2-s1","payload":{"message":"first"}}"#,
                    "\n",
                    r#"{"sequence":3,"timestamp":"2026-07-27T12:34:58.000Z","kind":"delta","node":"m2-s2","payload":{"message":"third"}}"#
                )
                .as_bytes()
                .to_vec(),
            ),
            (
                EventRecordFilter::Node(NodeId::parse("m2-s1").expect("node")),
                concat!(
                    r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m2-s1","payload":{"message":"first"}}"#,
                    "\n",
                    r#"{"sequence":2,"timestamp":"2026-07-27T12:34:57.000Z","kind":"future-kind","node":"m2-s1","payload":{"nested":{"answer":42},"items":[true,null,"kept"]}}"#,
                    "\n"
                )
                .as_bytes()
                .to_vec(),
            ),
            (
                EventRecordFilter::KindAndNode {
                    kind: EventKindName::new("delta"),
                    node: NodeId::parse("m2-s2").expect("node"),
                },
                r#"{"sequence":3,"timestamp":"2026-07-27T12:34:58.000Z","kind":"delta","node":"m2-s2","payload":{"message":"third"}}"#
                    .as_bytes()
                    .to_vec(),
            ),
        ];
        for (filter, expected) in fixtures {
            let mut output = Vec::new();
            run_log_read(&path, &filter, &mut output).expect("raw read should succeed");
            assert_eq!(output, expected);
        }
    }

    #[test]
    fn log_read_validates_complete_file_before_writing() {
        let directory = tempdir().expect("temporary directory should create");
        let path = directory.path().join("events.jsonl");
        for suffix in [
            "\n{",
            concat!(
                "\n",
                r#"{"sequence":1,"timestamp":"2026-07-27T12:34:59.000Z","kind":"delta","node":"m2-s3","payload":{"message":"decreasing"}}"#
            ),
        ] {
            fs::write(&path, format!("{RAW_READ_FIXTURE}{suffix}")).expect("fixture should seed");
            let mut output = Vec::new();
            assert!(
                run_log_read(&path, &EventRecordFilter::All, &mut output).is_err(),
                "invalid complete log should fail"
            );
            assert!(output.is_empty());
        }
    }

    #[test]
    fn typed_empty_snapshot_uses_compiled_schema_identity() {
        let vision =
            VisionSlug::parse("2026-07-27-event-log").expect("vision fixture should parse");
        let state = derive_run_state(
            &[],
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )
        .expect("empty typed state should derive");
        let snapshot = RunSnapshot::from(&state);
        let value =
            validated_snapshot_value(&snapshot).expect("typed empty snapshot should validate");
        assert_eq!(
            render_human_snapshot(&snapshot),
            concat!(
                "pce status (pce.run-snapshot v1)\n",
                "repositories (0)\nsteps (0)\ndispatches (0)\nrounds (0)\nholds (0)\n",
                "provenance (0)\nresume state=no-log-visible-candidate\nrecovery-digest\n",
                "  rounds (entries=0, elisions=0)\n",
                "  open-holds (entries=0, elisions=0)\n",
                "  deltas (entries=0, elisions=0)\n",
                "  facts (entries=0, elisions=0)\n",
            )
        );
        assert_eq!(value["schema_id"], "pce.run-snapshot");
        assert_eq!(value["schema_version"], 1);
        for category in ["rounds", "open_holds", "deltas", "facts"] {
            assert_eq!(
                value["recovery_digest"][category]["entries"],
                serde_json::json!([])
            );
        }
    }

    #[test]
    fn typed_rich_snapshot_conforms_to_compiled_schema() {
        let records = [
            r#"{"sequence":1,"timestamp":"2026-07-27T12:34:56.000Z","kind":"delta","node":"m2-s4","payload":{"message":"changed"}}"#,
            r#"{"sequence":2,"timestamp":"2026-07-27T12:34:57.000Z","kind":"planning-artifact-approved","node":"m2-s4","payload":{"path":"planning/steps.json","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","evidence":"digest evidence"}}"#,
        ]
        .into_iter()
        .map(|line| parse_event_line(line).expect("record fixture should parse"))
        .collect::<Vec<_>>();
        let digest =
            Sha256Digest::parse("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
                .expect("digest fixture should parse");
        let artifacts = [CurrentArtifactObservation::new(
            ArtifactPath::new("planning/steps.json"),
            CurrentArtifactState::Present { digest },
        )];
        let repositories = [RepositoryObservation::new(
            RepositoryName::new("pce"),
            RepositoryFetchObservation::Unavailable {
                failure: RepositoryObservationFailure::parse("offline")
                    .expect("failure fixture should parse"),
            },
            RepositoryBranchName::parse("milestone-2").expect("branch fixture should parse"),
            BranchState::Present,
            WorktreeIdentity::parse("pce/event-log/m2-s4").expect("worktree fixture should parse"),
            WorktreeState::Present,
            TagName::parse("v0.1.16").expect("tag fixture should parse"),
            TagState::Absent,
        )];
        let authorities = [StepAuthorityObservation::new(
            NodeId::parse("m2-s4").expect("node fixture should parse"),
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        )];
        let vision =
            VisionSlug::parse("2026-07-27-event-log").expect("vision fixture should parse");
        let state = derive_run_state(
            &records,
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &artifacts,
            &repositories,
            &authorities,
        )
        .expect("rich typed state should derive");
        let snapshot = RunSnapshot::from(&state);
        let value =
            validated_snapshot_value(&snapshot).expect("typed rich snapshot should validate");
        let human = render_human_snapshot(&snapshot);
        assert!(human.contains("repository 1: name=\"pce\""));
        assert!(human.contains("step 1: node=\"m2-s4\" merge-status=not-merged"));
        assert!(human.ends_with('\n'));
        assert_eq!(value["repositories"][0]["repository"], "pce");
        assert_eq!(value["steps"][0]["merge_status"], "not-merged");
        assert_eq!(
            value["provenance"][0]["condition"]["state"],
            "digest-matches"
        );
        assert_eq!(
            value["recovery_digest"]["deltas"]["entries"][0]["sequence"],
            1
        );
        assert_eq!(
            value["recovery_digest"]["facts"]["entries"][0]["kind"],
            "planning-artifact-approved"
        );
    }
}
