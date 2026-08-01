use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output, Stdio};
use std::time::{Instant, SystemTime};

use anyhow::{Context, Error, Result, anyhow, bail};
use pce_core::GateCommand;
use pce_core::{
    AbsoluteOutputPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory, AppendError,
    AppendableCategory, AppendableFinding, ArgumentVector, ArtifactOutcome, ArtifactPath,
    AuthorityFailure, BranchState, CanonicalNode as DispatchNode, ChildEnvironment,
    CodexTokenUsage, CreationDate, CurrentArtifactObservation, CurrentArtifactState,
    DispatchCandidate, DispatchDuration, DispatchEnvelope, DispatchExitStatus, DispatchLogging,
    DispatchProjectionInput, DispatchRef, DispatchRole, DispatchRoleClass, DispatchabilityResult,
    EventBodyRef, EventKindName, EventLogTail, EventLogTailLine, EventRecord, EventRecordFilter,
    EventTimestamp, Evidence, ExactPullRequestIdentity, ExactPullRequestState, Executable,
    ExitCode, FindingAdmission, GateObservations, GitAuthorityObservation,
    GitHubAuthorityObservation, GitHubPullRequestObservation, GitMergeObservation, KnownPayload,
    LegacyRepositoryContractPayload, MeasuredContractSnapshot, MergeStatus, MergeSubject,
    MilestoneMergeSubject, MilestoneNode, NodeId, ObservedExitStatus, ObservedWorkflowName,
    OrderingEdge, PullRequestNumber, PullRequestSelector, RecoveryLogPath, RepositoryBranchName,
    RepositoryContractPayload, RepositoryFetchObservation, RepositoryName, RepositoryObservation,
    RepositoryObservationFailure, RepositoryObservationRef, RepositoryRoot, RunSnapshot, Sandbox,
    Sha256Digest, SignalNumber, SquashCommitOid, StdinBinding, StepAuthorityObservation, StepNode,
    TagName, TagState, TagTarget, TerminalObservation, TerminalUsage, TrackedRepositoryContract,
    UnparsedPayload, UsageAbsenceReason, VersionPolicy, VisionName, VisionSlug, WorktreeIdentity,
    WorktreeState, WriteKind, admit_recurrent_finding, append_event, classify_terminal_usage,
    compute_dispatchability, create_vision, derive_merge_status, derive_milestone_merge_status,
    derive_run_state, dispatch_completion_payload, dispatch_invocation, dispatch_payload,
    event_record_matches, measure_contract_snapshot, parse_event_line,
    parse_tracked_repository_contract, render_dispatch_projection, render_human_snapshot,
    serialize_tracked_repository_contract, validate_workflow_coverage,
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const USAGE: &str = concat!(
    "usage: pce vision new \"<name>\"\n",
    "       pce log --file <LOG_PATH> --kind <KIND> --node <NODE>\n",
    "       pce log read --file <LOG_PATH> [--kind <KIND>] [--node <NODE>]\n",
    "       pce status --file <LOG_PATH> --vision-dir <VISION_DIR> [--human]\n",
    "       pce ready --file <LOG_PATH> --vision-dir <VISION_DIR> [--graph <APPROVED_ARTIFACT_PATH>]\n",
    "       pce contract check --file <CONTRACT_PATH> --repo-root <REPOSITORY_ROOT>\n",
    "       pce contract bootstrap --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --repository <REPOSITORY> --node <NODE>\n",
    "       pce contract refresh --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE>\n",
    "       pce contract learn --file <CURRENT_LOG_PATH> --prior-file <PRIOR_LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE> --category <environment-hazard|gate-ordering|lockfile-rule> --finding <FINDING>\n",
    "       pce dispatch codex --cwd <ABSOLUTE_WORKING_DIRECTORY> --sandbox workspace-write [--env <NAME=VALUE>]... [--output-schema <ABSOLUTE_SCHEMA_PATH> -o <ABSOLUTE_OUTPUT_PATH>] [--plan-file <PLAN_PATH>] [--log-file <LOG_PATH> --node <NODE> --role <ROLE> --ref <REF> --evidence <EVIDENCE> [--dry-run]] -- <CODEX_ARGUMENT>..."
);
const RUN_SNAPSHOT_SCHEMA: &str = include_str!("../skills/pce/schemas/run-snapshot.schema.json");
const ORIGIN: &str = "origin";
const RELEASE_TAG: &str = "v0.1.16";
const TRACKED_REPOSITORY_CONTRACT_PATH: &str = ".pce/repository-contract.json";
const FORMAT_BOOTSTRAP_CANDIDATES: &[&str] = &["cargo fmt --all --check", "cargo fmt --check"];
const LINT_BOOTSTRAP_CANDIDATES: &[&str] = &[
    "cargo clippy --workspace --all-targets",
    "cargo clippy --all-targets",
    "cargo clippy",
];
const TYPECHECK_BOOTSTRAP_CANDIDATES: &[&str] = &[
    "cargo check --workspace --all-targets",
    "cargo check --all-targets",
    "cargo check",
];
const TEST_BOOTSTRAP_CANDIDATES: &[&str] = &[
    "cargo test --workspace",
    "cargo test --all-targets",
    "cargo test --lib",
];
const BUILD_BOOTSTRAP_CANDIDATES: &[&str] = &[
    "cargo build --workspace",
    "cargo build --all-targets",
    "cargo build",
];

#[derive(Debug)]
enum Command {
    Dispatch {
        envelope: DispatchEnvelope,
        logging: Option<DispatchLoggingMode>,
    },
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
        graph_path: Option<ArtifactPath>,
    },
    ContractCheck {
        contract_path: PathBuf,
        repository_root: PathBuf,
    },
    ContractRefresh {
        log_path: PathBuf,
        repository_root: PathBuf,
        node: NodeId,
    },
    ContractLearn {
        log_path: PathBuf,
        prior_log_path: PathBuf,
        repository_root: PathBuf,
        node: NodeId,
        finding: AppendableFinding,
    },
    ContractBootstrap {
        log_path: PathBuf,
        repository_root: PathBuf,
        repository: RepositoryName,
        node: NodeId,
    },
}

#[derive(Debug)]
enum DispatchLoggingMode {
    Live {
        path: PathBuf,
        metadata: DispatchLogging,
    },
    DryRun {
        path: PathBuf,
        metadata: DispatchLogging,
    },
}

struct LiveDispatchLog<'a> {
    path: &'a Path,
    metadata: &'a DispatchLogging,
}

enum DefaultBranchContract {
    Present(Vec<u8>),
    Absent,
}

enum BootstrapWorkflowStandIn {
    Command(String),
    None,
}

struct BootstrapWorkflow {
    name: String,
    stand_in: BootstrapWorkflowStandIn,
    run_scripts: Vec<String>,
}

struct BootstrapGateCommands {
    format: String,
    lint: String,
    typecheck: String,
    test: String,
    build: String,
}

enum BootstrapDerivation {
    Ci {
        gates: BootstrapGateCommands,
        workflows: Vec<BootstrapWorkflow>,
    },
    CargoFallback {
        gates: BootstrapGateCommands,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusFormat {
    Json,
    Human,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepositoryContract {
    Current(RepositoryContractPayload),
    Legacy(LegacyRepositoryContractPayload),
}

impl RepositoryContract {
    fn name(&self) -> &RepositoryName {
        match self {
            Self::Current(payload) => &payload.repository,
            Self::Legacy(payload) => &payload.repository,
        }
    }

    fn root(&self) -> &str {
        match self {
            Self::Current(payload) => payload.repo_root.as_str(),
            Self::Legacy(payload) => payload.repo_root.as_str(),
        }
    }
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
        Command::Dispatch { envelope, logging } => match logging.as_ref() {
            Some(DispatchLoggingMode::DryRun { path, metadata }) => {
                run_dispatch_projection(&envelope, path, metadata)
            }
            Some(DispatchLoggingMode::Live { path, metadata }) => {
                spawn_dispatch(&envelope, Some(LiveDispatchLog { path, metadata }))
            }
            None => spawn_dispatch(&envelope, None),
        },
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
            graph_path,
        } => run_ready(
            &log_path,
            &recovery_log_path,
            &vision_dir,
            graph_path.as_ref(),
        ),
        Command::ContractCheck {
            contract_path,
            repository_root,
        } => run_contract_check(&contract_path, &repository_root),
        Command::ContractRefresh {
            log_path,
            repository_root,
            node,
        } => run_contract_refresh(&log_path, &repository_root, node),
        Command::ContractLearn {
            log_path,
            prior_log_path,
            repository_root,
            node,
            finding,
        } => run_contract_learn(&log_path, &prior_log_path, &repository_root, node, finding),
        Command::ContractBootstrap {
            log_path,
            repository_root,
            repository,
            node,
        } => run_contract_bootstrap(&log_path, &repository_root, repository, node),
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
        [verb, action, rest @ ..] if verb == "contract" => parse_contract_command(action, rest),
        [verb, target, rest @ ..] if verb == "dispatch" => {
            parse_codex_dispatch(target, rest).with_context(|| USAGE)
        }
        _ => bail!(USAGE),
    }
}

fn parse_codex_dispatch(target: &str, rest: &[String]) -> Result<Command> {
    if target != "codex" {
        bail!("unsupported dispatch target `{target}`");
    }

    let mut position = 0;
    let raw_cwd = required_option(rest, &mut position, "--cwd")?;
    let sandbox = required_option(rest, &mut position, "--sandbox")?;
    if sandbox != "workspace-write" {
        bail!("unsupported sandbox `{sandbox}`");
    }

    let mut environment = BTreeMap::new();
    while rest.get(position).is_some_and(|value| value == "--env") {
        let raw_entry = required_option(rest, &mut position, "--env")?;
        let (name, value) = raw_entry
            .split_once('=')
            .ok_or_else(|| anyhow!("environment entry must contain `=`: `{raw_entry}`"))?;
        if name.is_empty() {
            bail!("environment name must not be empty");
        }
        if environment
            .insert(name.to_owned(), value.to_owned())
            .is_some()
        {
            bail!("duplicate environment name `{name}`");
        }
    }

    let structured = if rest
        .get(position)
        .is_some_and(|value| value == "--output-schema")
    {
        let schema = required_option(rest, &mut position, "--output-schema")?;
        let output = required_option(rest, &mut position, "-o")?;
        Some((schema, output))
    } else {
        None
    };

    let plan_path = if rest
        .get(position)
        .is_some_and(|value| value == "--plan-file")
    {
        Some(required_option(rest, &mut position, "--plan-file")?)
    } else {
        None
    };

    const LOGGING_DIAGNOSTIC: &str = "dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence";
    let logging_raw = if rest
        .get(position)
        .is_some_and(|value| value == "--log-file")
    {
        let parsed = (|| -> Result<(&str, &str, &str, &str, &str)> {
            Ok((
                required_option(rest, &mut position, "--log-file")?,
                required_option(rest, &mut position, "--node")?,
                required_option(rest, &mut position, "--role")?,
                required_option(rest, &mut position, "--ref")?,
                required_option(rest, &mut position, "--evidence")?,
            ))
        })();
        match parsed {
            Ok(values) => Some(values),
            Err(_) => bail!(LOGGING_DIAGNOSTIC),
        }
    } else if rest.get(position).is_some_and(|value| {
        matches!(
            value.as_str(),
            "--node" | "--role" | "--ref" | "--evidence" | "--dry-run"
        )
    }) {
        bail!(LOGGING_DIAGNOSTIC)
    } else {
        None
    };

    let dry_run =
        if logging_raw.is_some() && rest.get(position).is_some_and(|value| value == "--dry-run") {
            position += 1;
            true
        } else {
            false
        };
    if rest.get(position).is_none_or(|value| value != "--") {
        bail!("dispatch arguments require the `--` delimiter");
    }
    position += 1;
    let arguments = rest[position..].to_vec();

    let executable = Executable::parse("codex").context("failed to parse Codex executable")?;
    let working_directory = AbsoluteWorkingDirectory::parse(PathBuf::from(raw_cwd))
        .context("failed to parse dispatch working directory")?;
    let stdin = match plan_path {
        Some(path) => StdinBinding::PlanBytes(
            std::fs::read(path).with_context(|| format!("failed to read plan file `{path}`"))?,
        ),
        None => StdinBinding::Null,
    };
    let mut envelope = DispatchEnvelope::new(executable, working_directory, stdin)
        .with_arguments(ArgumentVector::new(arguments))
        .with_environment(ChildEnvironment::new(environment))
        .with_sandbox(Sandbox::WorkspaceWrite);
    if let Some((schema, output)) = structured {
        envelope = envelope
            .with_schema_path(
                AbsoluteSchemaPath::parse(PathBuf::from(schema))
                    .context("failed to parse dispatch schema path")?,
            )
            .with_output_path(
                AbsoluteOutputPath::parse(PathBuf::from(output))
                    .context("failed to parse dispatch output path")?,
            );
    }
    let logging = logging_raw
        .map(|(path, node, role, dispatch_ref, evidence)| -> Result<_> {
            let path = PathBuf::from(path);
            let metadata = DispatchLogging {
                node: NodeId::parse(node).context("failed to parse dispatch logging node")?,
                role: DispatchRole::new(role),
                dispatch_ref: DispatchRef::new(dispatch_ref),
                evidence: Evidence::parse(evidence)
                    .context("failed to parse dispatch logging evidence")?,
            };
            Ok(if dry_run {
                DispatchLoggingMode::DryRun { path, metadata }
            } else {
                DispatchLoggingMode::Live { path, metadata }
            })
        })
        .transpose()?;
    Ok(Command::Dispatch { envelope, logging })
}

fn required_option<'a>(rest: &'a [String], position: &mut usize, flag: &str) -> Result<&'a str> {
    if rest.get(*position).is_none_or(|value| value != flag) {
        bail!("expected `{flag}`");
    }
    let value = rest
        .get(*position + 1)
        .ok_or_else(|| anyhow!("missing value for `{flag}`"))?;
    if value.is_empty() {
        bail!("empty value for `{flag}`");
    }
    *position += 2;
    Ok(value)
}

fn parse_contract_command(action: &str, rest: &[String]) -> Result<Command> {
    match (action, rest) {
        ("check", [file_flag, raw_contract_path, root_flag, raw_repository_root])
            if file_flag == "--file"
                && root_flag == "--repo-root"
                && is_value(raw_contract_path)
                && is_value(raw_repository_root) =>
        {
            Ok(Command::ContractCheck {
                contract_path: PathBuf::from(raw_contract_path),
                repository_root: PathBuf::from(raw_repository_root),
            })
        }
        (
            "bootstrap",
            [
                file_flag,
                raw_log_path,
                root_flag,
                raw_repository_root,
                repository_flag,
                raw_repository,
                node_flag,
                raw_node,
            ],
        ) if file_flag == "--file"
            && root_flag == "--repo-root"
            && repository_flag == "--repository"
            && node_flag == "--node"
            && is_value(raw_log_path)
            && is_value(raw_repository_root)
            && is_value(raw_repository)
            && !raw_repository.is_empty()
            && is_value(raw_node) =>
        {
            let node =
                NodeId::parse(raw_node).context("failed to parse contract-bootstrap node")?;
            Ok(Command::ContractBootstrap {
                log_path: PathBuf::from(raw_log_path),
                repository_root: PathBuf::from(raw_repository_root),
                repository: RepositoryName::new(raw_repository),
                node,
            })
        }
        (
            "refresh",
            [
                file_flag,
                raw_log_path,
                root_flag,
                raw_repository_root,
                node_flag,
                raw_node,
            ],
        ) if file_flag == "--file"
            && root_flag == "--repo-root"
            && node_flag == "--node"
            && is_value(raw_log_path)
            && is_value(raw_repository_root)
            && is_value(raw_node) =>
        {
            let node = NodeId::parse(raw_node).context("failed to parse contract-refresh node")?;
            Ok(Command::ContractRefresh {
                log_path: PathBuf::from(raw_log_path),
                repository_root: PathBuf::from(raw_repository_root),
                node,
            })
        }
        (
            "learn",
            [
                file_flag,
                raw_log_path,
                prior_file_flag,
                raw_prior_log_path,
                root_flag,
                raw_repository_root,
                node_flag,
                raw_node,
                category_flag,
                raw_category,
                finding_flag,
                raw_finding,
            ],
        ) if file_flag == "--file"
            && prior_file_flag == "--prior-file"
            && root_flag == "--repo-root"
            && node_flag == "--node"
            && category_flag == "--category"
            && finding_flag == "--finding"
            && is_value(raw_log_path)
            && is_value(raw_prior_log_path)
            && is_value(raw_repository_root)
            && is_value(raw_node)
            && is_value(raw_category)
            && is_value(raw_finding) =>
        {
            let node = NodeId::parse(raw_node).context("failed to parse contract-learn node")?;
            let category = AppendableCategory::parse(raw_category)
                .context("failed to parse contract-learn category")?;
            let finding = AppendableFinding::parse(category, raw_finding.clone())
                .context("failed to parse contract-learn finding")?;
            Ok(Command::ContractLearn {
                log_path: PathBuf::from(raw_log_path),
                prior_log_path: PathBuf::from(raw_prior_log_path),
                repository_root: PathBuf::from(raw_repository_root),
                node,
                finding,
            })
        }
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
        trailing @ ..,
    ] = args
    else {
        bail!(USAGE);
    };
    if file_flag != "--file"
        || vision_flag != "--vision-dir"
        || !is_value(raw_path)
        || !is_value(raw_vision_dir)
    {
        bail!(USAGE);
    }

    let graph_path = match trailing {
        [] => None,
        [graph_flag, raw_graph_path]
            if graph_flag == "--graph"
                && is_value(raw_graph_path)
                && !raw_graph_path.is_empty() =>
        {
            Some(ArtifactPath::new(raw_graph_path))
        }
        _ => bail!(USAGE),
    };

    Ok(Command::Ready {
        log_path: PathBuf::from(raw_path),
        recovery_log_path: RecoveryLogPath::new(raw_path),
        vision_dir: PathBuf::from(raw_vision_dir),
        graph_path,
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

fn append_one(path: &Path, kind: WriteKind, node: NodeId, payload: String) -> Result<EventRecord> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open or create event log {}", path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", path.display()))?;
    let operation = append_locked_record(&mut file, payload, kind, node, path);
    let unlock = file
        .unlock()
        .with_context(|| format!("failed to unlock event log {}", path.display()));
    match (operation, unlock) {
        (Ok(record), Ok(())) => Ok(record),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(_), Err(error)) => Err(error),
        (Err(primary), Err(error)) => Err(primary.context(format!(
            "additionally, explicit event-log unlock failed: {error:#}"
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
    let (artifacts, _) = current_artifacts(&records, Path::new(contracts[primary_index].root()))?;
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
        let (observation, runtime) = observe_repository(
            contract.name().clone(),
            PathBuf::from(contract.root()),
            &integration_branches,
            selected.subject.selector(),
        )?;
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
    graph_path: Option<&ArtifactPath>,
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

    let (artifacts, artifact_bytes) =
        current_artifacts(&records, Path::new(contracts[primary_index].root()))?;
    let state = derive_run_state(&records, &vision, recovery_log_path, &artifacts, &[], &[])
        .context("failed to derive readiness provenance and dispatch history")?;
    let (approval_sequence, artifact_path, parsed_graph) = if let Some(requested_path) = graph_path
    {
        let (approval_sequence, artifact_path) = approvals
            .iter()
            .find(|(_, artifact_path)| artifact_path == requested_path)
            .cloned()
            .with_context(|| {
                format!(
                    "no planning-artifact-approved record for graph path {}",
                    requested_path.as_str()
                )
            })?;
        artifact_bytes
            .iter()
            .find(|artifact| artifact.path == artifact_path)
            .context("approved artifact has no retained current observation")?;
        (approval_sequence, artifact_path, None)
    } else {
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
        (approval_sequence, artifact_path, Some(graph))
    };
    let provenance = state
        .provenance()
        .iter()
        .find(|item| item.approval_sequence() == approval_sequence && item.path() == &artifact_path)
        .context("selected planning-artifact approval has no derived provenance")?;
    compute_dispatchability(provenance, &[], &[], &[], &[])
        .context("selected planning artifact failed preliminary provenance check")?;
    let graph = match parsed_graph {
        Some(graph) => graph,
        None => {
            let retained = artifact_bytes
                .iter()
                .find(|artifact| artifact.path == artifact_path)
                .context("approved artifact has no retained current observation")?;
            let bytes = retained
                .bytes
                .as_deref()
                .context("approved artifact has no retained current observation")?;
            parse_dispatch_graph(bytes).map_err(|error| {
                anyhow!(
                    "approved graph path {} is not a conforming graph: {error:#}",
                    artifact_path.as_str()
                )
            })?
        }
    };

    for graph_node in &graph.nodes {
        let matches = contracts
            .iter()
            .filter(|contract| contract.name() == &graph_node.repository)
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
    let effective_version_policies = readiness_version_policies(&candidates, &contracts)?;

    let mut merge_statuses = Vec::<(DispatchNode, MergeStatus)>::with_capacity(graph.nodes.len());
    for graph_node in &graph.nodes {
        let contract = contracts
            .iter()
            .find(|contract| contract.name() == &graph_node.repository)
            .context("graph repository contract disappeared after resolution")?;
        let (selector, altitude) = match &graph_node.node {
            DispatchNode::Milestone(node) => {
                let subject = MilestoneMergeSubject::derive(&vision, node.clone());
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
            contract.name().clone(),
            PathBuf::from(contract.root()),
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
        &effective_version_policies,
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

fn readiness_version_policies(
    candidates: &[DispatchCandidate],
    contracts: &[RepositoryContract],
) -> Result<Vec<(RepositoryName, VersionPolicy)>> {
    let mut policies = Vec::<(RepositoryName, VersionPolicy)>::new();
    for candidate in candidates {
        if policies
            .iter()
            .any(|(repository, _)| repository == candidate.repository())
        {
            continue;
        }
        let contract = contracts
            .iter()
            .find(|contract| contract.name() == candidate.repository())
            .with_context(|| {
                format!(
                    "candidate repository {} has no projected repository contract",
                    candidate.repository().as_str()
                )
            })?;
        let policy = match contract {
            RepositoryContract::Current(payload) => payload.stated.version_policy.clone(),
            RepositoryContract::Legacy(payload) => {
                match read_at_default_branch_head(
                    Path::new(payload.repo_root.as_str()),
                    ".pce/repository-contract.json",
                ) {
                    Ok(bytes) => parse_tracked_contract(&bytes)
                        .with_context(|| {
                            format!(
                                "failed to resolve readiness policy for repository {} from \
                                 .pce/repository-contract.json at default-branch HEAD",
                                payload.repository.as_str()
                            )
                        })?
                        .stated()
                        .version_policy()
                        .clone(),
                    Err(_) => bail!(
                        "repository {} has neither a current repository-contract record nor .pce/repository-contract.json at default-branch HEAD",
                        payload.repository.as_str()
                    ),
                }
            }
        };
        policies.push((candidate.repository().clone(), policy));
    }
    Ok(policies)
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

pub fn parse_tracked_contract(bytes: &[u8]) -> Result<TrackedRepositoryContract> {
    parse_tracked_repository_contract(bytes).context("failed to parse tracked repository contract")
}

fn run_contract_check(contract_path: &Path, repository_root: &Path) -> Result<()> {
    measure_tracked_contract_at_root(contract_path, repository_root, None)?;
    Ok(())
}

fn run_contract_refresh(log_path: &Path, repository_root: &Path, node: NodeId) -> Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(log_path)
        .with_context(|| format!("failed to open event log {}", log_path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", log_path.display()))?;

    let operation = refresh_locked(&mut file, log_path, repository_root, node);
    let unlock = file
        .unlock()
        .with_context(|| format!("failed to unlock event log {}", log_path.display()));

    match (operation, unlock) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(unlock_error)) => Err(unlock_error),
        (Err(primary), Err(unlock_error)) => Err(primary.context(format!(
            "additionally, explicit event-log unlock failed: {unlock_error:#}"
        ))),
    }
}

fn run_contract_learn(
    log_path: &Path,
    prior_log_path: &Path,
    repository_root: &Path,
    node: NodeId,
    finding: AppendableFinding,
) -> Result<()> {
    if log_path == prior_log_path {
        bail!("current and prior run logs must be distinct paths");
    }

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(log_path)
        .with_context(|| format!("failed to open event log {}", log_path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", log_path.display()))?;

    let operation = learn_locked(
        &mut file,
        log_path,
        prior_log_path,
        repository_root,
        node,
        finding,
    );
    let unlock = file
        .unlock()
        .with_context(|| format!("failed to unlock event log {}", log_path.display()));

    match (operation, unlock) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(unlock_error)) => Err(unlock_error),
        (Err(primary), Err(unlock_error)) => Err(primary.context(format!(
            "additionally, explicit event-log unlock failed: {unlock_error:#}"
        ))),
    }
}

fn run_contract_bootstrap(
    log_path: &Path,
    repository_root: &Path,
    repository: RepositoryName,
    node: NodeId,
) -> Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(log_path)
        .with_context(|| format!("failed to open event log {}", log_path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", log_path.display()))?;

    let operation = bootstrap_locked(&mut file, log_path, repository_root, repository, node);
    let unlock = file
        .unlock()
        .with_context(|| format!("failed to unlock event log {}", log_path.display()));

    match (operation, unlock) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(()), Err(unlock_error)) => Err(unlock_error),
        (Err(primary), Err(unlock_error)) => Err(primary.context(format!(
            "additionally, explicit event-log unlock failed: {unlock_error:#}"
        ))),
    }
}

fn bootstrap_locked(
    file: &mut File,
    log_path: &Path,
    repository_root: &Path,
    repository: RepositoryName,
    node: NodeId,
) -> Result<()> {
    let repository_root_argument = repository_root.to_str().with_context(|| {
        format!(
            "raw --repo-root argument is not valid UTF-8: {}",
            repository_root.display()
        )
    })?;
    let requested_repository_root = RepositoryRoot::new(repository_root_argument.to_owned());
    let normalized_repository_root =
        lexically_normalized_repository_root(requested_repository_root.as_str());

    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("failed to seek event log {} for read", log_path.display()))?;
    let parsed_lines = {
        let mut reader = BufReader::new(&mut *file);
        read_event_log_lines(&mut reader, log_path)?
    };
    let previous =
        matching_bootstrap_contract(&parsed_lines, &repository, &normalized_repository_root)?;

    let branch = resolve_default_branch(repository_root)?;
    match tracked_contract_at_branch_head(repository_root, &branch)? {
        DefaultBranchContract::Present(bytes) => {
            let _tracked_bytes_at_default_branch_head = bytes;
            bail!(
                "tracked repository contract .pce/repository-contract.json is present at default-branch HEAD; use `pce contract refresh`"
            )
        }
        DefaultBranchContract::Absent => {}
    }
    let workflow_paths = default_branch_paths(repository_root, &branch, ".github/workflows")?
        .into_iter()
        .filter(|path| path.ends_with(".yml") || path.ends_with(".yaml"))
        .collect::<Vec<_>>();
    let workflow_identities = workflow_paths
        .iter()
        .map(|path| {
            path.strip_prefix(".github/workflows/")
                .with_context(|| format!("workflow path `{path}` lacks expected prefix"))
                .and_then(|identity| {
                    ObservedWorkflowName::parse(identity)
                        .with_context(|| format!("failed to parse workflow path `{path}`"))
                })
        })
        .collect::<Result<Vec<_>>>()?;

    let tracked_path = repository_root.join(TRACKED_REPOSITORY_CONTRACT_PATH);
    let (tracked, measured) = if tracked_path.exists() {
        let bytes = std::fs::read(&tracked_path).with_context(|| {
            format!(
                "failed to read tracked repository contract {}",
                tracked_path.display()
            )
        })?;
        let tracked = parse_tracked_contract(&bytes)?;
        validate_workflow_coverage(tracked.stated().workflows(), &workflow_identities)
            .context("failed to validate tracked workflow coverage")?;
        let previous_snapshot = previous
            .as_ref()
            .map(|prior| previous_measured_snapshot(&tracked, prior))
            .transpose()?;
        let measured =
            measure_contract_snapshot(tracked.stated(), previous_snapshot.as_ref(), |command| {
                execute_gate_command(repository_root, command)
            })
            .context("failed to measure tracked repository contract")?;
        (tracked, measured)
    } else {
        derive_bootstrap_contract(
            repository_root,
            &branch,
            &workflow_paths,
            &workflow_identities,
        )?
    };

    let evidence_text = measured
        .gates()
        .iter()
        .map(|measurement| measurement.command().as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let evidence = Evidence::parse(&evidence_text).context("failed to parse contract evidence")?;
    let payload = RepositoryContractPayload::from_tracked_measurement(
        repository,
        requested_repository_root,
        &tracked,
        measured.gates(),
        evidence,
    );

    let tracked_directory = repository_root.join(".pce");
    std::fs::create_dir_all(&tracked_directory).with_context(|| {
        format!(
            "failed to create tracked contract directory {}",
            tracked_directory.display()
        )
    })?;
    let canonical_bytes = serialize_tracked_repository_contract(&tracked)
        .context("failed to serialize tracked repository contract")?;
    std::fs::write(&tracked_path, canonical_bytes).with_context(|| {
        format!(
            "failed to persist tracked repository contract {}",
            tracked_path.display()
        )
    })?;
    let payload_json = serde_json::to_string(&payload)
        .context("failed to serialize repository contract payload")?;
    append_locked(
        file,
        payload_json,
        WriteKind::RepositoryContract,
        node,
        log_path,
    )
}

fn matching_bootstrap_contract(
    lines: &[ParsedEventLine],
    repository: &RepositoryName,
    normalized_root: &Path,
) -> Result<Option<RepositoryContractPayload>> {
    let mut matching = None;
    for line in lines {
        let (name, root, current) = match line.record.body_ref() {
            EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) => (
                &payload.repository,
                payload.repo_root.as_str(),
                Some(payload),
            ),
            EventBodyRef::Known(KnownPayload::LegacyRepositoryContract(payload)) => {
                (&payload.repository, payload.repo_root.as_str(), None)
            }
            _ => continue,
        };
        let same_name = name == repository;
        let same_root = lexically_normalized_repository_root(root) == normalized_root;
        if same_name != same_root {
            bail!(
                "ambiguous duplicate repository contract at sequence {} for repository {} and root {}",
                line.record.sequence().get(),
                name.as_str(),
                root
            );
        }
        if same_name
            && same_root
            && let Some(payload) = current
        {
            matching = Some(payload.clone());
        }
    }
    Ok(matching)
}

fn derive_bootstrap_contract(
    repository_root: &Path,
    branch: &str,
    workflow_paths: &[String],
    workflow_identities: &[ObservedWorkflowName],
) -> Result<(TrackedRepositoryContract, MeasuredContractSnapshot)> {
    let derivation = if workflow_paths.is_empty() {
        if !path_exists_at_default_branch_head(repository_root, branch, "Cargo.toml")? {
            bail!("cannot bootstrap CI-less repository without Cargo.toml at default-branch HEAD");
        }
        BootstrapDerivation::CargoFallback {
            gates: BootstrapGateCommands {
                format: select_bootstrap_candidate(
                    "format",
                    FORMAT_BOOTSTRAP_CANDIDATES,
                    |command| execute_shell_gate_command(repository_root, command),
                )?,
                lint: select_bootstrap_candidate("lint", LINT_BOOTSTRAP_CANDIDATES, |command| {
                    execute_shell_gate_command(repository_root, command)
                })?,
                typecheck: select_bootstrap_candidate(
                    "typecheck",
                    TYPECHECK_BOOTSTRAP_CANDIDATES,
                    |command| execute_shell_gate_command(repository_root, command),
                )?,
                test: select_bootstrap_candidate("test", TEST_BOOTSTRAP_CANDIDATES, |command| {
                    execute_shell_gate_command(repository_root, command)
                })?,
                build: select_bootstrap_candidate(
                    "build",
                    BUILD_BOOTSTRAP_CANDIDATES,
                    |command| execute_shell_gate_command(repository_root, command),
                )?,
            },
        }
    } else {
        let workflows = workflow_paths
            .iter()
            .map(|path| parse_bootstrap_workflow(repository_root, branch, path))
            .collect::<Result<Vec<_>>>()?;
        let names = workflows
            .iter()
            .map(|workflow| workflow.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let first = |role: &str, prefix: &str| -> Result<String> {
            workflows
                .iter()
                .flat_map(|workflow| workflow.run_scripts.iter())
                .flat_map(|script| script.lines())
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .find(|line| {
                    let mut words = line.split_whitespace();
                    words.next() == Some("cargo") && words.next() == Some(prefix)
                })
                .map(str::to_owned)
                .with_context(|| {
                    format!(
                        "CI-derived bootstrap found no {role} gate command in workflows: {names}"
                    )
                })
        };
        BootstrapDerivation::Ci {
            gates: BootstrapGateCommands {
                format: first("format", "fmt")?,
                lint: first("lint", "clippy")?,
                typecheck: first("typecheck", "check")?,
                test: first("test", "test")?,
                build: first("build", "build")?,
            },
            workflows,
        }
    };

    let (gates, workflows) = match &derivation {
        BootstrapDerivation::Ci { gates, workflows } => (gates, workflows.as_slice()),
        BootstrapDerivation::CargoFallback { gates } => (gates, &[][..]),
    };
    let workflow_values = workflows
        .iter()
        .map(|workflow| match &workflow.stand_in {
            BootstrapWorkflowStandIn::Command(command) => json!({
                "workflow": workflow.name,
                "stand_in": {"kind": "COMMAND", "command": command}
            }),
            BootstrapWorkflowStandIn::None => json!({
                "workflow": workflow.name,
                "stand_in": {"kind": "NONE"}
            }),
        })
        .collect::<Vec<_>>();
    let raw = json!({
        "stated": {
            "gates": {
                "format": gates.format,
                "lint": gates.lint,
                "typecheck": gates.typecheck,
                "test": gates.test,
                "build": gates.build
            },
            "version_policy": "NONE",
            "branches": {
                "default": branch,
                "milestone": "pce/{vision}/milestone-{milestone}",
                "step": "pce/{vision}/m{milestone}-s{step}"
            },
            "pull_requests": {
                "step_base": "MILESTONE",
                "milestone_base": "DEFAULT",
                "merge_method": "SQUASH"
            },
            "workflows": workflow_values
        },
        "appendable": {
            "environment_hazards": [],
            "gate_orderings": [],
            "lockfile_rules": []
        }
    });
    let bytes = serde_json::to_vec(&raw).context("failed to serialize bootstrap contract")?;
    let tracked = parse_tracked_contract(&bytes)?;
    validate_workflow_coverage(tracked.stated().workflows(), workflow_identities)
        .context("failed to validate tracked workflow coverage")?;
    let measured = match derivation {
        BootstrapDerivation::Ci { .. } => {
            measure_contract_snapshot(tracked.stated(), None, |command| {
                execute_gate_command(repository_root, command)
            })
            .context("failed to measure tracked repository contract")?
        }
        BootstrapDerivation::CargoFallback { .. } => {
            let observations = GateObservations {
                format: ObservedExitStatus::from_code(0),
                lint: ObservedExitStatus::from_code(0),
                typecheck: ObservedExitStatus::from_code(0),
                test: ObservedExitStatus::from_code(0),
                build: ObservedExitStatus::from_code(0),
            };
            MeasuredContractSnapshot::from_observations(tracked.stated(), &observations)
                .context("failed to construct bootstrap observations")?
        }
    };
    Ok((tracked, measured))
}

fn parse_bootstrap_workflow(
    repository_root: &Path,
    branch: &str,
    path: &str,
) -> Result<BootstrapWorkflow> {
    let bytes = read_at_branch_head(repository_root, branch, path)?;
    let value: serde_yaml::Value = serde_yaml::from_slice(&bytes)
        .with_context(|| format!("failed to parse workflow `{path}` as YAML"))?;
    let mut run_scripts = Vec::new();
    if let Some(root) = value.as_mapping()
        && let Some(jobs_value) = root.get(serde_yaml::Value::String("jobs".to_owned()))
    {
        let jobs = jobs_value
            .as_mapping()
            .with_context(|| format!("workflow `{path}` jobs must be a mapping"))?;
        let mut sorted_jobs = jobs
            .iter()
            .map(|(key, value)| {
                key.as_str()
                    .map(|name| (name, value))
                    .with_context(|| format!("workflow `{path}` contains a non-string job key"))
            })
            .collect::<Result<Vec<_>>>()?;
        sorted_jobs.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (_, job) in sorted_jobs {
            let Some(job) = job.as_mapping() else {
                continue;
            };
            let Some(steps) = job.get(serde_yaml::Value::String("steps".to_owned())) else {
                continue;
            };
            let steps = steps
                .as_sequence()
                .with_context(|| format!("workflow `{path}` steps must be a sequence"))?;
            for step in steps {
                let Some(step) = step.as_mapping() else {
                    continue;
                };
                let Some(run) = step.get(serde_yaml::Value::String("run".to_owned())) else {
                    continue;
                };
                let command = run.as_str().with_context(|| {
                    format!("workflow `{path}` contains a non-string run command")
                })?;
                run_scripts.push(command.to_owned());
            }
        }
    }
    let name = path
        .strip_prefix(".github/workflows/")
        .with_context(|| format!("workflow path `{path}` lacks expected prefix"))?
        .to_owned();
    let stand_in = if run_scripts.is_empty() {
        BootstrapWorkflowStandIn::None
    } else {
        BootstrapWorkflowStandIn::Command(run_scripts.join("\n"))
    };
    Ok(BootstrapWorkflow {
        name,
        stand_in,
        run_scripts,
    })
}

fn select_bootstrap_candidate(
    role: &str,
    candidates: &[&str],
    mut execute: impl FnMut(&str) -> std::io::Result<ObservedExitStatus>,
) -> Result<String> {
    let mut attempted = Vec::new();
    for candidate in candidates {
        attempted.push(*candidate);
        let status = execute(candidate)
            .with_context(|| format!("failed to execute bootstrap candidate `{candidate}`"))?;
        if status.code() == 0 {
            return Ok((*candidate).to_owned());
        }
    }
    let rendered = attempted
        .iter()
        .map(|command| format!("`{command}`"))
        .collect::<Vec<_>>()
        .join(", ");
    bail!("no passing bootstrap candidate for {role}; attempted commands: {rendered}")
}

fn refresh_locked(
    file: &mut File,
    log_path: &Path,
    repository_root: &Path,
    node: NodeId,
) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("failed to seek event log {} for read", log_path.display()))?;
    let parsed_lines = {
        let mut reader = BufReader::new(&mut *file);
        read_event_log_lines(&mut reader, log_path)?
    };
    let records = parsed_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let (selected_projection, normalized_repository_root) =
        contract_for_repository_root(&records, repository_root)?;
    let RepositoryContract::Current(previous) = selected_projection else {
        bail!(
            "event log repository contract for lexically normalized --repo-root {} is legacy and cannot be refreshed",
            normalized_repository_root.display()
        );
    };

    let tracked_bytes = match tracked_contract_at_default_branch_head(repository_root)? {
        DefaultBranchContract::Present(bytes) => bytes,
        DefaultBranchContract::Absent => {
            read_at_default_branch_head(repository_root, TRACKED_REPOSITORY_CONTRACT_PATH)?
        }
    };
    let tracked = parse_tracked_contract(&tracked_bytes)?;
    persist_refreshed_contract_locked(file, log_path, repository_root, node, &previous, &tracked)
}

fn learn_locked(
    file: &mut File,
    log_path: &Path,
    prior_log_path: &Path,
    repository_root: &Path,
    node: NodeId,
    finding: AppendableFinding,
) -> Result<()> {
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("failed to seek event log {} for read", log_path.display()))?;
    let current_lines = {
        let mut reader = BufReader::new(&mut *file);
        read_event_log_lines(&mut reader, log_path)?
    };
    let current_records = current_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let (current_contract, current_normalized_root) =
        contract_for_repository_root(&current_records, repository_root)?;
    let RepositoryContract::Current(current_contract) = current_contract else {
        bail!(
            "event log repository contract for lexically normalized --repo-root {} is legacy and cannot be refreshed",
            current_normalized_root.display()
        );
    };

    let prior_lines = read_event_log(prior_log_path)?;
    let prior_records = prior_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let (prior_contract, prior_normalized_root) =
        contract_for_repository_root(&prior_records, repository_root)?;
    if prior_contract.name() != &current_contract.repository
        || prior_normalized_root != current_normalized_root
    {
        bail!(
            "prior run log repository identity does not match current run repository {}",
            current_contract.repository.as_str()
        );
    }

    let tracked_bytes =
        read_at_default_branch_head(repository_root, TRACKED_REPOSITORY_CONTRACT_PATH)?;
    let mut tracked = parse_tracked_contract(&tracked_bytes)?;
    let current_findings = current_records
        .iter()
        .filter_map(|record| match record.body_ref() {
            EventBodyRef::Known(KnownPayload::KeyFinding(payload)) => {
                Some(payload.finding.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let prior_findings = prior_records
        .iter()
        .filter_map(|record| match record.body_ref() {
            EventBodyRef::Known(KnownPayload::KeyFinding(payload)) => {
                Some(payload.finding.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let finding_text = finding.as_str().to_owned();
    match admit_recurrent_finding(&mut tracked, finding, &current_findings, &prior_findings) {
        FindingAdmission::CurrentOccurrenceMissing => {
            bail!("current run log contains no byte-exact key-finding {finding_text:?}")
        }
        FindingAdmission::FirstOccurrence => Ok(()),
        FindingAdmission::Appended | FindingAdmission::AlreadyPresent => {
            persist_refreshed_contract_locked(
                file,
                log_path,
                repository_root,
                node,
                &current_contract,
                &tracked,
            )
        }
    }
}

fn persist_refreshed_contract_locked(
    file: &mut File,
    log_path: &Path,
    repository_root: &Path,
    node: NodeId,
    previous: &RepositoryContractPayload,
    tracked: &TrackedRepositoryContract,
) -> Result<()> {
    let observed = observed_workflows(repository_root)?;
    validate_workflow_coverage(tracked.stated().workflows(), &observed)
        .context("failed to validate tracked workflow coverage")?;
    let previous_snapshot = previous_measured_snapshot(tracked, previous)?;
    let measured =
        measure_contract_snapshot(tracked.stated(), Some(&previous_snapshot), |command| {
            execute_gate_command(repository_root, command)
        })
        .context("failed to measure tracked repository contract")?;
    let evidence_text = measured
        .gates()
        .iter()
        .map(|measurement| measurement.command().as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let evidence = Evidence::parse(&evidence_text).context("failed to parse contract evidence")?;
    let payload = RepositoryContractPayload::from_tracked_measurement(
        previous.repository.clone(),
        previous.repo_root.clone(),
        tracked,
        measured.gates(),
        evidence,
    );
    let payload_json = serde_json::to_string(&payload)
        .context("failed to serialize repository contract payload")?;
    let canonical_bytes = serialize_tracked_repository_contract(tracked)
        .context("failed to serialize tracked repository contract")?;
    let tracked_path = repository_root.join(TRACKED_REPOSITORY_CONTRACT_PATH);
    std::fs::write(&tracked_path, canonical_bytes).with_context(|| {
        format!(
            "failed to persist tracked repository contract {}",
            tracked_path.display()
        )
    })?;
    append_locked(
        file,
        payload_json,
        WriteKind::RepositoryContract,
        node,
        log_path,
    )
}

fn previous_measured_snapshot(
    tracked: &TrackedRepositoryContract,
    previous: &RepositoryContractPayload,
) -> Result<MeasuredContractSnapshot> {
    (|| {
        let bytes = serialize_tracked_repository_contract(tracked)?;
        let mut value: Value = serde_json::from_slice(&bytes)?;
        for (pointer, command) in [
            ("/stated/gates/format", previous.stated.format.as_str()),
            ("/stated/gates/lint", previous.stated.lint.as_str()),
            (
                "/stated/gates/typecheck",
                previous.stated.typecheck.as_str(),
            ),
            ("/stated/gates/test", previous.stated.test.as_str()),
            ("/stated/gates/build", previous.stated.build.as_str()),
        ] {
            let slot = value
                .pointer_mut(pointer)
                .with_context(|| format!("canonical tracked contract lacks {pointer}"))?;
            *slot = Value::String(command.to_owned());
        }
        let prior_bytes = serde_json::to_vec(&value)?;
        let prior_tracked = parse_tracked_contract(&prior_bytes)?;
        MeasuredContractSnapshot::from_observations(prior_tracked.stated(), &previous.observations)
            .map_err(Error::from)
    })()
    .context("failed to reconstruct previous measured contract snapshot")
}

fn measure_tracked_contract_at_root(
    contract_path: &Path,
    repository_root: &Path,
    previous: Option<&MeasuredContractSnapshot>,
) -> Result<MeasuredContractSnapshot> {
    let bytes = std::fs::read(contract_path).with_context(|| {
        format!(
            "failed to read tracked repository contract {}",
            contract_path.display()
        )
    })?;
    let contract = parse_tracked_contract(&bytes)?;
    let observed = observed_workflows(repository_root)?;
    validate_workflow_coverage(contract.stated().workflows(), &observed)
        .context("failed to validate tracked workflow coverage")?;
    measure_contract_snapshot(contract.stated(), previous, |command| {
        execute_gate_command(repository_root, command)
    })
    .context("failed to measure tracked repository contract")
}

fn observed_workflows(repository_root: &Path) -> Result<Vec<ObservedWorkflowName>> {
    let workflows_directory = repository_root.join(".github/workflows");
    let entries = match std::fs::read_dir(&workflows_directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to read workflow directory {}",
                    workflows_directory.display()
                )
            });
        }
    };

    let mut observed = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to read entry in workflow directory {}",
                workflows_directory.display()
            )
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("failed to read workflow file type {}", path.display()))?;
        if !file_type.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let extension = Path::new(&file_name).extension();
        if extension != Some(std::ffi::OsStr::new("yml"))
            && extension != Some(std::ffi::OsStr::new("yaml"))
        {
            continue;
        }
        let file_name = file_name
            .into_string()
            .map_err(|_| anyhow!("workflow filename at {} is not valid UTF-8", path.display()))?;
        observed
            .push(ObservedWorkflowName::parse(&file_name).with_context(|| {
                format!("failed to parse workflow filename {}", path.display())
            })?);
    }
    observed.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    Ok(observed)
}

fn execute_gate_command(
    repository_root: &Path,
    command: &GateCommand,
) -> std::io::Result<ObservedExitStatus> {
    execute_shell_gate_command(repository_root, command.as_str())
}

fn execute_shell_gate_command(
    repository_root: &Path,
    command: &str,
) -> std::io::Result<ObservedExitStatus> {
    let args = [OsString::from("-c"), OsString::from(command)];
    match execute_process("/bin/sh", &args, Some(repository_root)) {
        ProcessAttempt::SpawnFailed { detail } => Err(std::io::Error::other(detail)),
        ProcessAttempt::Completed(result) => result
            .status
            .code()
            .map(ObservedExitStatus::from_code)
            .ok_or_else(|| {
                std::io::Error::other(format!(
                    "stated gate command `{}` terminated without an exit-status code",
                    command
                ))
            }),
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
    read_event_log_lines(&mut reader, path)
}

fn read_event_log_lines(reader: &mut dyn BufRead, path: &Path) -> Result<Vec<ParsedEventLine>> {
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

fn lexically_normalized_repository_root(root: &str) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    let mut rooted = false;
    for component in Path::new(root).components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                normalized.push(component.as_os_str());
                rooted = true;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(
                    normalized.components().next_back(),
                    Some(Component::Normal(_))
                ) {
                    normalized.pop();
                } else if !rooted {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if !rooted && normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

fn repository_contracts(records: &[EventRecord]) -> Result<Vec<RepositoryContract>> {
    let mut contracts = Vec::<RepositoryContract>::new();
    for record in records {
        let contract = match record.body_ref() {
            EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) => {
                RepositoryContract::Current(payload.clone())
            }
            EventBodyRef::Known(KnownPayload::LegacyRepositoryContract(payload)) => {
                RepositoryContract::Legacy(payload.clone())
            }
            _ => continue,
        };
        let root = Path::new(contract.root());
        if root.as_os_str().is_empty() {
            bail!(
                "repository contract at sequence {} has an empty repo_root",
                record.sequence().get()
            );
        }
        let normalized_root = lexically_normalized_repository_root(contract.root());
        let mut replacement = None;
        for (index, projected) in contracts.iter().enumerate() {
            let same_name = projected.name() == contract.name();
            let same_root =
                lexically_normalized_repository_root(projected.root()) == normalized_root;
            if same_name && same_root {
                replacement = Some(index);
                break;
            }
            if same_name || same_root {
                bail!(
                    "ambiguous duplicate repository contract at sequence {} for repository {} and root {}",
                    record.sequence().get(),
                    contract.name().as_str(),
                    root.display()
                );
            }
        }
        if let Some(index) = replacement {
            if !matches!(
                (&contracts[index], &contract),
                (
                    RepositoryContract::Current(_),
                    RepositoryContract::Legacy(_)
                )
            ) {
                contracts[index] = contract;
            }
        } else {
            contracts.push(contract);
        }
    }
    if contracts.is_empty() {
        bail!("event log contains no repository-contract record");
    }
    Ok(contracts)
}

fn contract_for_repository_root(
    records: &[EventRecord],
    repository_root: &Path,
) -> Result<(RepositoryContract, PathBuf)> {
    let repository_root_argument = repository_root.to_str().with_context(|| {
        format!(
            "raw --repo-root argument is not valid UTF-8: {}",
            repository_root.display()
        )
    })?;
    let requested_repository_root = RepositoryRoot::new(repository_root_argument.to_owned());
    let normalized_repository_root =
        lexically_normalized_repository_root(requested_repository_root.as_str());
    let contracts = repository_contracts(records)?;
    let mut matching = contracts.into_iter().filter(|contract| {
        lexically_normalized_repository_root(contract.root()) == normalized_repository_root
    });
    let selected = matching.next().with_context(|| {
        format!(
            "event log contains no repository contract for lexically normalized --repo-root {}",
            normalized_repository_root.display()
        )
    })?;
    if matching.next().is_some() {
        bail!(
            "event log contains multiple repository contracts for lexically normalized --repo-root {}",
            normalized_repository_root.display()
        );
    }
    Ok((selected, normalized_repository_root))
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
        let contract_root = Path::new(contract.root());
        let root = absolute_path(contract_root).with_context(|| {
            format!(
                "failed to resolve repository root {}",
                contract_root.display()
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
    name: RepositoryName,
    root: PathBuf,
    integration_branches: &[String],
    selector: &PullRequestSelector,
) -> Result<(RepositoryObservation, RepositoryRuntime)> {
    let remote = verify_origin(&root);
    let mut fetches = Vec::with_capacity(integration_branches.len());
    for branch in integration_branches {
        let result = match &remote {
            Ok(()) => fetch_branch(&root, branch),
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
    let branch_state = probe_branch(&root, selected_branch)?;
    let head = selector.head().as_str();
    let worktree_state = probe_worktree(&root, head)?;
    let tag_state = probe_tag(&root)?;
    let observation = RepositoryObservation::new(
        name.clone(),
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
            name,
            root,
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

fn resolve_default_branch(repo_root: &Path) -> Result<String> {
    (|| {
        let args = git_args(repo_root, ["symbolic-ref", "refs/remotes/origin/HEAD"]);
        let result = require_spawn(execute_process("git", &args, None))?;
        if !result.status.success() {
            bail!("{}", result.detail());
        }
        let symbolic_ref = one_nonempty_line(&result.stdout)?;
        let remote_branch = symbolic_ref
            .strip_prefix("refs/remotes/origin/")
            .context("default-branch symbolic ref has unexpected shape")?;
        remote_branch
            .split('/')
            .rfind(|segment| !segment.is_empty())
            .map(str::to_owned)
            .context("default-branch symbolic ref has no branch segment")
    })()
    .context("failed to resolve default branch from refs/remotes/origin/HEAD")
}

fn read_at_branch_head(repo_root: &Path, branch: &str, relative_path: &str) -> Result<Vec<u8>> {
    (|| {
        let revision_path = format!("{branch}:{relative_path}");
        let args = git_args(repo_root, ["show", revision_path.as_str()]);
        let result = require_spawn(execute_process("git", &args, None))?;
        if !result.status.success() {
            bail!("{}", result.detail());
        }
        Ok(result.stdout)
    })()
    .with_context(|| format!("failed to read {relative_path} at default-branch HEAD"))
}

fn read_at_default_branch_head(repo_root: &Path, relative_path: &str) -> Result<Vec<u8>> {
    let branch = resolve_default_branch(repo_root)?;
    read_at_branch_head(repo_root, &branch, relative_path)
}

fn default_branch_paths(repository_root: &Path, branch: &str, prefix: &str) -> Result<Vec<String>> {
    let args = git_args(
        repository_root,
        ["ls-tree", "-r", "--name-only", branch, "--", prefix],
    );
    let result = require_spawn(execute_process("git", &args, None))?;
    if !result.status.success() {
        bail!("{}", result.detail());
    }
    let stdout = std::str::from_utf8(&result.stdout).context("command output is not UTF-8")?;
    let mut paths = stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn path_exists_at_default_branch_head(
    repository_root: &Path,
    branch: &str,
    relative_path: &str,
) -> Result<bool> {
    Ok(
        default_branch_paths(repository_root, branch, relative_path)?
            .iter()
            .any(|path| path == relative_path),
    )
}

fn tracked_contract_at_default_branch_head(
    repository_root: &Path,
) -> Result<DefaultBranchContract> {
    let branch = resolve_default_branch(repository_root)?;
    tracked_contract_at_branch_head(repository_root, &branch)
}

fn tracked_contract_at_branch_head(
    repository_root: &Path,
    branch: &str,
) -> Result<DefaultBranchContract> {
    if path_exists_at_default_branch_head(
        repository_root,
        branch,
        TRACKED_REPOSITORY_CONTRACT_PATH,
    )? {
        Ok(DefaultBranchContract::Present(read_at_branch_head(
            repository_root,
            branch,
            TRACKED_REPOSITORY_CONTRACT_PATH,
        )?))
    } else {
        Ok(DefaultBranchContract::Absent)
    }
}

fn run_dispatch_projection(
    envelope: &DispatchEnvelope,
    log_path: &Path,
    metadata: &DispatchLogging,
) -> Result<()> {
    let content = match std::fs::read(log_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to read event-log tail from {}", log_path.display())
            });
        }
    };
    let content = String::from_utf8(content).with_context(|| {
        format!(
            "event-log tail in {} is not valid UTF-8",
            log_path.display()
        )
    })?;
    let tail = match content.lines().last() {
        Some(line) => EventLogTail::Present(EventLogTailLine::new(line)),
        None if content.is_empty() => EventLogTail::Empty,
        None => EventLogTail::Present(EventLogTailLine::new("")),
    };
    let rendered =
        render_dispatch_projection(DispatchProjectionInput::new(envelope, metadata, &tail))
            .context("failed to render dispatch projection")?;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    output
        .write_all(rendered.as_bytes())
        .context("failed to write dispatch projection to stdout")?;
    output
        .write_all(b"\n")
        .context("failed to terminate dispatch projection with newline")?;
    output
        .flush()
        .context("failed to flush dispatch projection")
}

fn spawn_dispatch(envelope: &DispatchEnvelope, logging: Option<LiveDispatchLog<'_>>) -> Result<()> {
    let issuance = logging
        .as_ref()
        .map(|logging| {
            let payload = dispatch_payload(logging.metadata);
            append_one(
                logging.path,
                WriteKind::Dispatch,
                logging.metadata.node.clone(),
                serde_json::to_string(&payload).context("failed to serialize dispatch issuance")?,
            )
        })
        .transpose()?;
    let invocation = dispatch_invocation(envelope);
    let executable = invocation.executable();
    let mut command = std::process::Command::new(executable);
    command.args(invocation.argv());
    command.current_dir(invocation.cwd());
    command.env_clear();
    command.envs(invocation.environment());
    match envelope.stdin() {
        StdinBinding::Null => {
            command.stdin(Stdio::null());
        }
        StdinBinding::PlanBytes(_) => {
            command.stdin(Stdio::piped());
        }
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::inherit());

    let started = Instant::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn `{executable}`"))?;
    let stdin_writer = if let StdinBinding::PlanBytes(bytes) = envelope.stdin() {
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("`{executable}` child stdin was not piped"))?;
        let bytes = bytes.clone();
        let executable = executable.to_owned();
        Some(std::thread::spawn(move || -> Result<()> {
            child_stdin
                .write_all(&bytes)
                .with_context(|| format!("failed to write plan bytes to `{executable}`"))?;
            drop(child_stdin);
            Ok(())
        }))
    } else {
        None
    };
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("`{executable}` child stdout was not piped"))?;
    let mut reader = BufReader::new(child_stdout);
    let stdout = std::io::stdout();
    let mut parent_stdout = stdout.lock();
    let mut observations = Vec::new();
    loop {
        let mut line = Vec::new();
        let count = reader
            .read_until(b'\n', &mut line)
            .with_context(|| format!("failed to read JSONL from `{executable}`"))?;
        if count == 0 {
            break;
        }
        parent_stdout
            .write_all(&line)
            .context("failed to tee Codex JSONL to stdout")?;
        parent_stdout
            .flush()
            .context("failed to flush Codex JSONL")?;
        if !line.iter().all(u8::is_ascii_whitespace) {
            observations.push(observe_terminal_line(&line));
        }
    }
    if let Some(writer) = stdin_writer {
        writer
            .join()
            .map_err(|_| anyhow!("plan stdin writer thread panicked"))??;
    }
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for `{executable}`"))?;
    let duration_ms = u64::try_from(started.elapsed().as_millis())
        .context("dispatch duration in milliseconds exceeds u64")?;
    let exit_status = dispatch_exit_status(status)?;
    let classification = classify_terminal_usage(&observations, exit_status);
    if let (Some(logging), Some(issuance)) = (logging, issuance) {
        let usage = classification
            .clone()
            .unwrap_or_else(|reason| CodexTokenUsage::Absent { reason });
        let completion = dispatch_completion_payload(
            issuance.sequence(),
            DispatchDuration::new(duration_ms),
            usage,
            exit_status,
            ArtifactOutcome::NotValidated,
        );
        append_one(
            logging.path,
            WriteKind::DispatchCompletion,
            logging.metadata.node.clone(),
            serde_json::to_string(&completion)
                .context("failed to serialize dispatch completion")?,
        )
        .context("failed to append dispatch completion after child exit")?;
    }
    if let Err(reason) = classification {
        bail!(
            "invalid Codex terminal data: {}",
            usage_absence_name(reason)
        );
    }
    if !status.success() {
        bail!("`{executable}` child exited with status {status}");
    }
    Ok(())
}

fn observe_terminal_line(line: &[u8]) -> TerminalObservation {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return TerminalObservation::MalformedLine;
    };
    let Some(object) = value.as_object() else {
        return TerminalObservation::NonTerminal;
    };
    match object.get("type").and_then(Value::as_str) {
        Some("turn.completed") => {
            let usage = object.get("usage").and_then(Value::as_object);
            let usage = usage.and_then(|usage| {
                Some(TerminalUsage {
                    input_tokens: usage.get("input_tokens")?.as_u64()?,
                    cached_input_tokens: usage.get("cached_input_tokens")?.as_u64()?,
                    output_tokens: usage.get("output_tokens")?.as_u64()?,
                    reasoning_output_tokens: usage.get("reasoning_output_tokens")?.as_u64()?,
                })
            });
            TerminalObservation::TurnCompleted(usage)
        }
        Some("turn.failed") => TerminalObservation::TurnFailed {
            usage_present: object.contains_key("usage"),
        },
        _ => TerminalObservation::NonTerminal,
    }
}

fn dispatch_exit_status(status: ExitStatus) -> Result<DispatchExitStatus> {
    if let Some(code) = status.code() {
        let code = u64::try_from(code).context("negative child exit code cannot be represented")?;
        return Ok(DispatchExitStatus::Exited {
            code: ExitCode::new(code),
        });
    }
    if let Some(signal) = status.signal() {
        let signal =
            u64::try_from(signal).context("negative signal number cannot be represented")?;
        return Ok(DispatchExitStatus::Signaled {
            signal: SignalNumber::new(signal),
        });
    }
    bail!("child exit status has neither an exit code nor a Unix signal")
}

fn usage_absence_name(reason: UsageAbsenceReason) -> &'static str {
    match reason {
        UsageAbsenceReason::TurnFailed => "turn-failed",
        UsageAbsenceReason::NoTerminalTurn => "no-terminal-turn",
        UsageAbsenceReason::MalformedTerminalData => "malformed-terminal-data",
        UsageAbsenceReason::DuplicateTerminalData => "duplicate-terminal-data",
        UsageAbsenceReason::ContradictoryTerminalData => "contradictory-terminal-data",
    }
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
    append_locked_record(file, payload, kind, node, path).map(|_| ())
}

fn append_locked_record(
    file: &mut File,
    payload: String,
    kind: WriteKind,
    node: NodeId,
    path: &Path,
) -> Result<EventRecord> {
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

    let intent = append_event::<std::io::Error, _>(
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
    let line = std::str::from_utf8(intent.as_bytes())
        .context("new event record is not UTF-8")?
        .trim_end_matches('\n');
    parse_event_line(line).context("failed to parse just-appended production event record")
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
    use std::process::Command as ProcessCommand;
    use std::time::SystemTime;

    use pce_core::{
        AppendableFinding, ArtifactPath, BranchState, CachedInputTokens, CodexTokenUsage,
        CurrentArtifactObservation, CurrentArtifactState, DispatchExitStatus, EventKindName,
        EventRecord, EventRecordFilter, ExitCode, GitAuthorityObservation,
        GitHubAuthorityObservation, GitHubPullRequestObservation, GitMergeObservation, InputTokens,
        KnownPayload, MilestoneMergeSubject, MilestoneNode, NodeId, ObservedExitStatus,
        OutputTokens, ReadKind, ReadPayload, ReasoningOutputTokens, RecoveryLogPath,
        RepositoryBranchName, RepositoryFetchObservation, RepositoryName, RepositoryObservation,
        RepositoryObservationFailure, RunSnapshot, Sha256Digest, StepAuthorityObservation,
        StepNode, TagName, TagState, TerminalObservation, VersionPolicy, VisionSlug,
        WorktreeIdentity, WorktreeState, WriteKind, classify_terminal_usage, derive_run_state,
        parse_event_line, render_human_snapshot,
    };
    use serde_json::json;
    use tempfile::tempdir;

    use crate::{
        BranchFetch, Command, DispatchGraphNode, DispatchLoggingMode, DispatchNode,
        FORMAT_BOOTSTRAP_CANDIDATES, FetchResult, RepositoryContract, RepositoryRuntime,
        StatusFormat, USAGE, already_dispatched, github_pull_request_list_args,
        lexically_normalized_repository_root, measure_tracked_contract_at_root, observe_git,
        observe_terminal_line, parse_command, parse_dispatch_graph, parse_tracked_contract,
        read_at_default_branch_head, read_event_log, readiness_version_policies,
        repository_contracts, run, run_log_read, select_bootstrap_candidate,
        validated_snapshot_value,
    };

    const VALID_TRACKED_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "cargo fmt --check",
      "lint": "cargo clippy --workspace --all-targets",
      "typecheck": "cargo check --workspace --all-targets",
      "test": "cargo test --workspace",
      "build": "cargo build --release"
    },
    "version_policy": "NONE",
    "branches": {
      "default": "main",
      "milestone": "pce/{vision}/milestone-{milestone}",
      "step": "pce/{vision}/m{milestone}-s{step}"
    },
    "pull_requests": {
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    },
    "workflows": [
      {
        "workflow": "ci.yml",
        "stand_in": {
          "kind": "COMMAND",
          "command": "cargo test --workspace"
        }
      },
      {
        "workflow": "release.yml",
        "stand_in": {
          "kind": "NONE"
        }
      }
    ]
  },
  "appendable": {
    "environment_hazards": [
      "pipe Codex stdin from /dev/null"
    ],
    "gate_orderings": [
      "run cargo fmt --check before clippy"
    ],
    "lockfile_rules": [
      "commit Cargo.lock when dependency resolution changes"
    ]
  }
}"#;

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

    fn tracked_contract_bytes(commands: [&str; 5]) -> Vec<u8> {
        let value = json!({
            "stated": {
                "gates": {
                    "format": commands[0],
                    "lint": commands[1],
                    "typecheck": commands[2],
                    "test": commands[3],
                    "build": commands[4]
                },
                "version_policy": "NONE",
                "branches": {
                    "default": "main",
                    "milestone": "pce/{vision}/milestone-{milestone}",
                    "step": "pce/{vision}/m{milestone}-s{step}"
                },
                "pull_requests": {
                    "step_base": "MILESTONE",
                    "milestone_base": "DEFAULT",
                    "merge_method": "SQUASH"
                },
                "workflows": []
            },
            "appendable": {
                "environment_hazards": [],
                "gate_orderings": [],
                "lockfile_rules": []
            }
        });
        let bytes = serde_json::to_vec(&value).expect("tracked contract value should serialize");
        let tracked = parse_tracked_contract(&bytes).expect("tracked contract should parse");
        pce_core::serialize_tracked_repository_contract(&tracked)
            .expect("tracked contract should serialize canonically")
    }

    #[test]
    fn contract_bootstrap_parses_exact_ordered_arguments() {
        let command = parse_command(
            [
                "contract",
                "bootstrap",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--repository",
                "pce",
                "--node",
                "m3-s3",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("exact bootstrap arguments should parse");
        let Command::ContractBootstrap {
            log_path,
            repository_root,
            repository,
            node,
        } = command
        else {
            panic!("expected contract-bootstrap command");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(repository_root, PathBuf::from("/workspace/pce"));
        assert_eq!(repository.as_str(), "pce");
        assert_eq!(node.as_str(), "m3-s3");

        for rejected in [
            vec![
                "contract",
                "bootstrap",
                "--repo-root",
                "/workspace/pce",
                "--file",
                "events.jsonl",
                "--repository",
                "pce",
                "--node",
                "m3-s3",
            ],
            vec![
                "contract",
                "bootstrap",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--node",
                "m3-s3",
            ],
            vec![
                "contract",
                "bootstrap",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--repository",
                "",
                "--node",
                "m3-s3",
            ],
            vec![
                "contract",
                "bootstrap",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--repository",
                "--node",
                "--node",
                "m3-s3",
            ],
            vec![
                "contract",
                "bootstrap",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--repository",
                "pce",
            ],
            vec![
                "contract",
                "bootstrap",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--repository",
                "pce",
                "--node",
                "m3-s3",
                "trailing",
            ],
        ] {
            let err = parse_command(rejected.into_iter().map(str::to_owned))
                .expect_err("invalid bootstrap arguments should be rejected");
            assert_eq!(err.to_string(), USAGE);
        }
    }

    #[test]
    fn cargo_bootstrap_candidates_stop_at_first_passing_command() {
        let mut calls = Vec::new();
        let selected =
            select_bootstrap_candidate("format", FORMAT_BOOTSTRAP_CANDIDATES, |command| {
                calls.push(command.to_owned());
                match command {
                    "cargo fmt --all --check" => Ok(ObservedExitStatus::from_code(31)),
                    "cargo fmt --check" => Ok(ObservedExitStatus::from_code(0)),
                    _ => panic!("candidate selection continued after success"),
                }
            })
            .expect("second format candidate should pass");
        assert_eq!(selected, "cargo fmt --check");
        assert_eq!(calls, vec!["cargo fmt --all --check", "cargo fmt --check"]);
    }

    #[test]
    fn cargo_bootstrap_candidates_name_role_and_attempts_when_none_pass() {
        let err = select_bootstrap_candidate("format", FORMAT_BOOTSTRAP_CANDIDATES, |_| {
            Ok(ObservedExitStatus::from_code(31))
        })
        .expect_err("all format candidates should fail");
        assert_eq!(
            err.to_string(),
            "no passing bootstrap candidate for format; attempted commands: `cargo fmt --all --check`, `cargo fmt --check`"
        );
    }

    fn current_contract_line(root: &Path, commands: [&str; 5]) -> String {
        let value = json!({
            "sequence": 1,
            "timestamp": "2026-07-29T12:00:00.000Z",
            "kind": "repository-contract",
            "node": "m3-s1",
            "payload": {
                "repository": "pce",
                "repo_root": root.to_str().expect("fixture root should be UTF-8"),
                "stated": {
                    "format": commands[0],
                    "lint": commands[1],
                    "typecheck": commands[2],
                    "test": commands[3],
                    "build": commands[4],
                    "version_policy": "NONE",
                    "branch_convention": "default=main; milestone=pce/{vision}/milestone-{milestone}; step=pce/{vision}/m{milestone}-s{step}",
                    "pull_request_convention": "step_base=MILESTONE; milestone_base=DEFAULT; merge_method=SQUASH"
                },
                "observations": {
                    "format": 0,
                    "lint": 0,
                    "typecheck": 0,
                    "test": 0,
                    "build": 0
                },
                "workflow_map": {},
                "appendable": {
                    "environment_hazards": [],
                    "gate_orderings": [],
                    "lockfile_rules": []
                },
                "evidence": commands.join("\n")
            }
        });
        format!("{value}\n")
    }

    fn legacy_contract_line(root: &Path) -> String {
        let value = json!({
            "sequence": 6,
            "timestamp": "2026-07-27T12:35:01.000Z",
            "kind": "repository-contract",
            "node": "m1-s1",
            "payload": {
                "repository": "pce",
                "repo_root": root.to_str().expect("fixture root should be UTF-8"),
                "stack": "Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)",
                "format": "cargo fmt --all --check",
                "lint": "cargo clippy --workspace --all-targets",
                "typecheck": "cargo check --workspace --all-targets",
                "test": "cargo test --workspace",
                "build": "cargo build --workspace",
                "preflight": "cargo check --workspace --all-targets",
                "gates_rule": "From the repo root, all four gates must exit zero before committing.",
                "install": "None required for gates.",
                "evidence": "rustc --version\ncargo --version\ngit rev-parse --show-toplevel"
            }
        });
        format!("{value}\n")
    }

    fn git(root: &Path, args: &[&str]) {
        let output = ProcessCommand::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .expect("git fixture command should spawn");
        assert!(
            output.status.success(),
            "git fixture command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn initialize_git_repository(root: &Path) {
        let output = ProcessCommand::new("git")
            .args(["init", "-b", "main"])
            .arg(root)
            .output()
            .expect("git init should spawn");
        assert!(output.status.success(), "git init should succeed");
        git(root, &["config", "user.name", "PCE Test"]);
        git(root, &["config", "user.email", "pce-test@example.invalid"]);
    }

    fn commit_contract(root: &Path, bytes: &[u8], message: &str) {
        let contract_path = root.join(".pce/repository-contract.json");
        fs::create_dir_all(contract_path.parent().expect("contract should have parent"))
            .expect("contract directory should create");
        fs::write(&contract_path, bytes).expect("contract fixture should write");
        git(root, &["add", ".pce/repository-contract.json"]);
        git(root, &["commit", "-m", message]);
    }

    fn establish_remote_head(root: &Path) {
        git(
            root,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
    }

    fn refresh_args(log_path: &Path, root: &Path) -> Vec<String> {
        vec![
            "contract".to_owned(),
            "refresh".to_owned(),
            "--file".to_owned(),
            log_path.display().to_string(),
            "--repo-root".to_owned(),
            root.display().to_string(),
            "--node".to_owned(),
            "m3-s2".to_owned(),
        ]
    }

    fn invoke_refresh(log_path: &Path, root: &Path) -> anyhow::Result<()> {
        run(
            refresh_args(log_path, root).into_iter(),
            &mut Cursor::new([]),
        )
    }

    #[test]
    fn contract_learn_parser_accepts_only_the_exact_ordered_shape() {
        let exact = [
            "contract",
            "learn",
            "--file",
            "current.jsonl",
            "--prior-file",
            "prior.jsonl",
            "--repo-root",
            "repository",
            "--node",
            "m3-s5",
            "--category",
            "lockfile-rule",
            "--finding",
            "Cargo.lock must be regenerated before cargo test",
        ];
        let command = parse_command(exact.into_iter().map(str::to_owned))
            .expect("exact learn command should parse");
        let Command::ContractLearn {
            log_path,
            prior_log_path,
            repository_root,
            node,
            finding,
        } = command
        else {
            panic!("expected contract learn command");
        };
        assert_eq!(log_path, PathBuf::from("current.jsonl"));
        assert_eq!(prior_log_path, PathBuf::from("prior.jsonl"));
        assert_eq!(repository_root, PathBuf::from("repository"));
        assert_eq!(node, NodeId::parse("m3-s5").expect("node should parse"));
        assert!(matches!(
            finding,
            AppendableFinding::LockfileRule(rule)
                if rule.as_str() == "Cargo.lock must be regenerated before cargo test"
        ));

        for rejected in [
            vec![
                "contract",
                "learn",
                "--prior-file",
                "prior.jsonl",
                "--file",
                "current.jsonl",
                "--repo-root",
                "repository",
                "--node",
                "m3-s5",
                "--category",
                "lockfile-rule",
                "--finding",
                "finding",
            ],
            vec![
                "contract",
                "learn",
                "--file",
                "current.jsonl",
                "--repo-root",
                "repository",
                "--node",
                "m3-s5",
                "--category",
                "lockfile-rule",
                "--finding",
                "finding",
            ],
            vec![
                "contract",
                "learn",
                "--file",
                "current.jsonl",
                "--file",
                "prior.jsonl",
                "--repo-root",
                "repository",
                "--node",
                "m3-s5",
                "--category",
                "lockfile-rule",
                "--finding",
                "finding",
            ],
            vec![
                "contract",
                "learn",
                "--file",
                "current.jsonl",
                "--prior-file",
                "prior.jsonl",
                "--repo-root",
                "repository",
                "--node",
                "m3-s5",
                "--category",
                "lockfile-rule",
                "--finding",
                "finding",
                "extra",
            ],
            vec![
                "contract",
                "learn",
                "--file",
                "--prior-file",
                "prior.jsonl",
                "--repo-root",
                "repository",
                "--node",
                "m3-s5",
                "--category",
                "lockfile-rule",
                "--finding",
                "finding",
            ],
        ] {
            let err = parse_command(rejected.into_iter().map(str::to_owned))
                .expect_err("non-exact learn command should fail");
            assert_eq!(err.to_string(), USAGE);
        }

        let unknown = exact.into_iter().map(str::to_owned).map(|value| {
            if value == "lockfile-rule" {
                "lockfile_rules".to_owned()
            } else {
                value
            }
        });
        let err = parse_command(unknown).expect_err("unknown category should fail");
        assert_eq!(err.to_string(), "failed to parse contract-learn category");
        assert!(format!("{err:#}").contains(
            "unknown appendable category \"lockfile_rules\"; expected environment-hazard, \
             gate-ordering, or lockfile-rule"
        ));

        let mut empty = exact.map(str::to_owned);
        empty[13] = String::new();
        let err = parse_command(empty.into_iter()).expect_err("empty finding should fail");
        assert_eq!(err.to_string(), "failed to parse contract-learn finding");
        assert!(format!("{err:#}").contains(
            "tracked repository contract field appendable.lockfile_rules[] cannot be empty"
        ));
    }

    #[test]
    fn contract_refresh_reads_default_branch_head_not_feature_worktree() {
        let directory = tempdir().expect("temporary directory should create");
        let root = directory.path().join("repository");
        let log_path = directory.path().join("events.jsonl");
        let strict = ["true", "true", "true", "exit 23", "true"];
        let relaxed = ["true", "true", "true", "true", "true"];
        initialize_git_repository(&root);
        commit_contract(&root, &tracked_contract_bytes(strict), "initial contract");
        establish_remote_head(&root);
        fs::write(&log_path, current_contract_line(&root, relaxed)).expect("event log should seed");
        git(&root, &["checkout", "-b", "feature"]);
        commit_contract(&root, &tracked_contract_bytes(relaxed), "relax test gate");

        let worktree_bytes = fs::read(root.join(".pce/repository-contract.json"))
            .expect("worktree contract should read");
        let worktree =
            parse_tracked_contract(&worktree_bytes).expect("worktree contract should parse");
        assert_eq!(worktree.stated().gates().test().as_str(), "true");
        let main_bytes = {
            let output = ProcessCommand::new("git")
                .arg("-C")
                .arg(&root)
                .args(["show", "main:.pce/repository-contract.json"])
                .output()
                .expect("git show should spawn");
            assert!(output.status.success(), "git show should succeed");
            output.stdout
        };
        let main = parse_tracked_contract(&main_bytes).expect("main contract should parse");
        assert_eq!(main.stated().gates().test().as_str(), "exit 23");
        let before = fs::read(&log_path).expect("event log should read");

        let err = invoke_refresh(&log_path, &root).expect_err("strict test gate should fail");

        let rendered = format!("{err:#}");
        assert!(rendered.contains("failed to measure tracked repository contract"));
        assert!(rendered.contains("stated gate command `exit 23` exited with status 23"));
        assert_eq!(fs::read(&log_path).expect("event log should read"), before);
        assert_eq!(
            read_event_log(&log_path)
                .expect("event log should parse")
                .len(),
            1
        );
    }

    #[test]
    fn contract_refresh_rereads_post_merge_command_and_reuses_unchanged_observations() {
        let directory = tempdir().expect("temporary directory should create");
        let root = directory.path().join("repository");
        let log_path = directory.path().join("events.jsonl");
        let initial = [
            "touch format-ran",
            "touch lint-ran",
            "touch typecheck-ran",
            "touch old-test-ran",
            "touch build-ran",
        ];
        initialize_git_repository(&root);
        commit_contract(&root, &tracked_contract_bytes(initial), "initial contract");
        establish_remote_head(&root);
        fs::write(&log_path, current_contract_line(&root, initial)).expect("event log should seed");

        invoke_refresh(&log_path, &root).expect("unchanged refresh should succeed");
        assert_eq!(
            read_event_log(&log_path)
                .expect("event log should parse")
                .len(),
            2
        );
        for marker in [
            "format-ran",
            "lint-ran",
            "typecheck-ran",
            "old-test-ran",
            "build-ran",
        ] {
            assert!(!root.join(marker).exists(), "{marker} must not exist");
        }

        let changed = [
            "touch format-ran",
            "touch lint-ran",
            "touch typecheck-ran",
            "touch new-test-ran",
            "touch build-ran",
        ];
        commit_contract(
            &root,
            &tracked_contract_bytes(changed),
            "change test command",
        );
        invoke_refresh(&log_path, &root).expect("changed refresh should succeed");

        let lines = read_event_log(&log_path).expect("event log should parse");
        assert_eq!(lines.len(), 3);
        assert!(root.join("new-test-ran").exists());
        for marker in [
            "format-ran",
            "lint-ran",
            "typecheck-ran",
            "old-test-ran",
            "build-ran",
        ] {
            assert!(!root.join(marker).exists(), "{marker} must not exist");
        }
        let records = lines
            .iter()
            .map(|line| line.record.clone())
            .collect::<Vec<_>>();
        let contracts = repository_contracts(&records).expect("contracts should project");
        let [RepositoryContract::Current(latest)] = contracts.as_slice() else {
            panic!("one latest current contract expected");
        };
        assert_eq!(latest.stated.test, "touch new-test-ran");
        assert_eq!(latest.observations.format.get(), 0);
        assert_eq!(latest.observations.lint.get(), 0);
        assert_eq!(latest.observations.typecheck.get(), 0);
        assert_eq!(latest.observations.test.get(), 0);
        assert_eq!(latest.observations.build.get(), 0);
        assert_eq!(
            fs::read(root.join(".pce/repository-contract.json"))
                .expect("worktree contract should read"),
            read_at_default_branch_head(&root, ".pce/repository-contract.json")
                .expect("default contract should read")
        );
    }

    #[test]
    fn contract_refresh_rejects_absent_default_branch_contract_without_worktree_fallback() {
        let directory = tempdir().expect("temporary directory should create");
        let root = directory.path().join("repository");
        let log_path = directory.path().join("events.jsonl");
        let commands = [
            "touch format-ran",
            "touch lint-ran",
            "touch typecheck-ran",
            "touch old-test-ran",
            "touch build-ran",
        ];
        initialize_git_repository(&root);
        fs::create_dir_all(root.join(".pce")).expect("contract directory should create");
        fs::write(root.join(".pce/.gitkeep"), []).expect("gitkeep should write");
        git(&root, &["add", ".pce/.gitkeep"]);
        git(&root, &["commit", "-m", "initial repository"]);
        establish_remote_head(&root);
        git(&root, &["checkout", "-b", "feature"]);
        let worktree_contract = tracked_contract_bytes(commands);
        fs::write(
            root.join(".pce/repository-contract.json"),
            &worktree_contract,
        )
        .expect("worktree contract should write");
        fs::write(&log_path, current_contract_line(&root, commands))
            .expect("event log should seed");
        let log_before = fs::read(&log_path).expect("event log should read");

        let err =
            invoke_refresh(&log_path, &root).expect_err("missing default contract should fail");

        assert!(
            format!("{err:#}")
                .contains("failed to read .pce/repository-contract.json at default-branch HEAD")
        );
        assert_eq!(
            fs::read(&log_path).expect("event log should read"),
            log_before
        );
        assert_eq!(
            fs::read(root.join(".pce/repository-contract.json"))
                .expect("worktree contract should read"),
            worktree_contract
        );
    }

    #[test]
    fn contract_refresh_rejects_legacy_only_prior_event_without_append() {
        let directory = tempdir().expect("temporary directory should create");
        let root = directory.path().join("repository");
        let log_path = directory.path().join("events.jsonl");
        let commands = [
            "touch format-ran",
            "touch lint-ran",
            "touch typecheck-ran",
            "touch old-test-ran",
            "touch build-ran",
        ];
        initialize_git_repository(&root);
        commit_contract(&root, &tracked_contract_bytes(commands), "initial contract");
        establish_remote_head(&root);
        fs::write(&log_path, legacy_contract_line(&root)).expect("event log should seed");
        let before = fs::read(&log_path).expect("event log should read");
        let normalized_repository_root =
            lexically_normalized_repository_root(root.to_str().expect("root should be UTF-8"));

        let err = invoke_refresh(&log_path, &root).expect_err("legacy contract should not refresh");

        assert_eq!(
            format!("{err:#}"),
            format!(
                "event log repository contract for lexically normalized --repo-root {} is legacy and cannot be refreshed",
                normalized_repository_root.display()
            )
        );
        assert_eq!(fs::read(&log_path).expect("event log should read"), before);
    }

    #[test]
    fn contract_refresh_parses_exact_ordered_arguments() {
        let command = parse_command(
            [
                "contract",
                "refresh",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--node",
                "m3-s2",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("exact refresh arguments should parse");
        let Command::ContractRefresh {
            log_path,
            repository_root,
            node,
        } = command
        else {
            panic!("contract refresh command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(repository_root, PathBuf::from("/workspace/pce"));
        assert_eq!(node.as_str(), "m3-s2");

        for args in [
            vec![
                "contract",
                "refresh",
                "--repo-root",
                "/workspace/pce",
                "--file",
                "events.jsonl",
                "--node",
                "m3-s2",
            ],
            vec![
                "contract",
                "refresh",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
            ],
            vec![
                "contract",
                "refresh",
                "--file",
                "events.jsonl",
                "--repo-root",
                "/workspace/pce",
                "--node",
                "m3-s2",
                "--extra",
            ],
        ] {
            let err = parse_command(args.into_iter().map(str::to_owned))
                .expect_err("non-exact refresh arguments should fail");
            assert_eq!(err.to_string(), USAGE);
        }
    }

    // Preservation guard for the orchestrator's existing status and ready vectors.
    #[test]
    fn status_and_policy_free_ready_orchestrator_arguments_are_accepted() {
        let status = parse_command(
            [
                "status",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/example",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("status orchestrator arguments should remain accepted");
        let Command::Status {
            log_path,
            vision_dir,
            format,
            ..
        } = status
        else {
            panic!("status command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(vision_dir, PathBuf::from("planning/example"));
        assert_eq!(format, StatusFormat::Json);

        let ready = parse_command(
            [
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/example",
                "--graph",
                "planning/example/milestone-3/steps.json",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("ready orchestrator arguments should remain accepted");
        let Command::Ready {
            log_path,
            vision_dir,
            graph_path,
            ..
        } = ready
        else {
            panic!("ready command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(vision_dir, PathBuf::from("planning/example"));
        assert_eq!(
            graph_path.as_ref().expect("graph should parse").as_str(),
            "planning/example/milestone-3/steps.json"
        );
    }

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
    fn composition_root_parses_exact_tracked_contract_bytes() {
        let contract =
            parse_tracked_contract(VALID_TRACKED_CONTRACT).expect("tracked contract should parse");
        assert_eq!(
            contract.stated().gates().test().as_str(),
            "cargo test --workspace"
        );
        assert_eq!(contract.stated().version_policy(), &VersionPolicy::None);
        assert!(matches!(
            contract.stated().workflows().as_slice()[1].stand_in(),
            pce_core::LocalWorkflowStandIn::None
        ));
        assert_eq!(
            contract.appendable().lockfile_rules()[0].as_str(),
            "commit Cargo.lock when dependency resolution changes"
        );
    }

    #[test]
    fn composition_root_preserves_tracked_contract_error_context() {
        let err = parse_tracked_contract(b"{\"stated\":{}}")
            .expect_err("incomplete contract should fail");
        let message = format!("{err:#}");
        assert!(message.contains("failed to parse tracked repository contract"));
        assert!(message.contains("malformed tracked repository contract:"));
    }

    #[test]
    fn github_command_routing_uses_selector_exact_ordered_pair() {
        let vision =
            VisionSlug::parse("2026-07-27-example").expect("vision slug fixture should parse");
        let node = NodeId::parse("m7").expect("milestone node fixture should parse");
        let milestone =
            MilestoneNode::parse(&node).expect("milestone node fixture should classify");
        let subject = MilestoneMergeSubject::derive(&vision, milestone);
        let selector = subject.selector();

        assert_eq!(
            github_pull_request_list_args(selector),
            [
                "pr",
                "list",
                "--head",
                "pce/example/milestone-7",
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
        let vision =
            VisionSlug::parse("2026-07-27-example").expect("vision slug fixture should parse");
        let node = NodeId::parse("m7").expect("milestone node fixture should parse");
        let milestone =
            MilestoneNode::parse(&node).expect("milestone node fixture should classify");
        let subject = MilestoneMergeSubject::derive(&vision, milestone);
        let selector = subject.selector();
        let runtime = RepositoryRuntime {
            name: RepositoryName::new("primary"),
            root: PathBuf::from("unused"),
            fetches: vec![
                BranchFetch {
                    branch: "pce/example/milestone-7".to_owned(),
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
    fn accepts_all_eight_registered_payload_schemas() {
        let directory = tempdir().expect("temporary directory should create");
        let fixtures = [
            (
                "dispatch",
                r#"{"role":"step-executor","ref":"ca9788ded3daec9b9e9fd7679caa24e7c64a8193","evidence":"git rev-parse HEAD"}"#,
                WriteKind::Dispatch,
            ),
            (
                "dispatch-completion",
                r#"{"issuance_sequence":1,"duration_ms":200,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":42},"artifact_outcome":"not-validated"}"#,
                WriteKind::DispatchCompletion,
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
                r#"{"repository":"pce","repo_root":"/workspace/pce","stated":{"format":"cargo fmt --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --release","version_policy":"NONE","branch_convention":"pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>","pull_request_convention":"step head targets the matching milestone integration branch"},"observations":{"format":0,"lint":0,"typecheck":0,"test":0,"build":0},"workflow_map":{"ci.yml":"cargo test --workspace","docs.yml":null},"appendable":{"environment_hazards":["stdin is reserved for event payload input"],"gate_orderings":["format before lint before typecheck before test before build"],"lockfile_rules":["Cargo.lock must remain synchronized with Cargo.toml"]},"evidence":"cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"}"#,
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

    fn parsed_current_contract_record(
        sequence: u64,
        repository: &str,
        repo_root: &str,
        test_command: &str,
    ) -> EventRecord {
        let evidence = format!(
            "cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\n{test_command}\ncargo build --release"
        );
        let line = json!({
            "sequence": sequence,
            "timestamp": "2026-07-27T12:35:04.000Z",
            "kind": "repository-contract",
            "node": "m3-s1",
            "payload": {
                "repository": repository,
                "repo_root": repo_root,
                "stated": {
                    "format": "cargo fmt --check",
                    "lint": "cargo clippy --workspace --all-targets",
                    "typecheck": "cargo check --workspace --all-targets",
                    "test": test_command,
                    "build": "cargo build --release",
                    "version_policy": "NONE",
                    "branch_convention": "default=main; milestone=pce/{vision}/milestone-{milestone}; step=pce/{vision}/m{milestone}-s{step}",
                    "pull_request_convention": "step_base=MILESTONE; milestone_base=DEFAULT; merge_method=SQUASH"
                },
                "observations": {
                    "format": 0,
                    "lint": 0,
                    "typecheck": 0,
                    "test": 0,
                    "build": 0
                },
                "workflow_map": {
                    "ci.yml": "cargo test --workspace",
                    "release.yml": null
                },
                "appendable": {
                    "environment_hazards": ["pipe Codex stdin from /dev/null"],
                    "gate_orderings": ["run cargo fmt --check before clippy"],
                    "lockfile_rules": ["commit Cargo.lock when dependency resolution changes"]
                },
                "evidence": evidence
            }
        })
        .to_string();
        parse_event_line(&line).expect("current contract fixture should parse")
    }

    fn parsed_legacy_contract_record(
        sequence: u64,
        repository: &str,
        repo_root: &str,
    ) -> EventRecord {
        let line = json!({
            "sequence": sequence,
            "timestamp": "2026-07-27T12:35:04.000Z",
            "kind": "repository-contract",
            "node": "m3-s1",
            "payload": {
                "repository": repository,
                "repo_root": repo_root,
                "stack": "Rust 2024-edition Cargo workspace",
                "format": "cargo fmt --all --check",
                "lint": "cargo clippy --workspace --all-targets",
                "typecheck": "cargo check --workspace --all-targets",
                "test": "cargo test --workspace",
                "build": "cargo build --workspace",
                "preflight": "cargo check --workspace --all-targets",
                "gates_rule": "all gates must exit zero",
                "install": "None required for gates.",
                "evidence": "rustc --version\ncargo --version\ngit rev-parse --show-toplevel"
            }
        })
        .to_string();
        parse_event_line(&line).expect("legacy contract fixture should parse")
    }

    #[test]
    fn legacy_contract_without_tracked_file_fails_readiness_loudly() {
        let repository_root = tempdir().expect("temporary repository root should exist");
        let records = [parsed_legacy_contract_record(
            1,
            "pce",
            repository_root
                .path()
                .to_str()
                .expect("UTF-8 repository root"),
        )];
        let contracts = repository_contracts(&records).expect("legacy contract should project");
        let candidate = pce_core::DispatchCandidate::new(
            DispatchNode::Step(
                StepNode::parse(&NodeId::parse("m1-s1").expect("canonical node id"))
                    .expect("canonical step node"),
            ),
            RepositoryName::new("pce"),
        );

        let error = readiness_version_policies(&[candidate], &contracts)
            .expect_err("missing current and tracked contracts must fail");
        assert_eq!(
            error.to_string(),
            "repository pce has neither a current repository-contract record nor .pce/repository-contract.json at default-branch HEAD"
        );
    }

    #[test]
    fn repository_contracts_replaces_same_identity_with_latest_current_record() {
        let records = [
            parsed_legacy_contract_record(8, "pce", "/nonexistent/pce/"),
            parsed_current_contract_record(
                9,
                "pce",
                "/nonexistent/./pce",
                "cargo test --workspace --first",
            ),
            parsed_current_contract_record(
                10,
                "pce",
                "/nonexistent/cache/../pce",
                "cargo test --workspace --latest",
            ),
        ];

        let contracts =
            repository_contracts(&records).expect("same identity should replace in place");
        let [RepositoryContract::Current(payload)] = contracts.as_slice() else {
            panic!("one current repository contract expected");
        };
        assert_eq!(payload.stated.test, "cargo test --workspace --latest");
    }

    #[test]
    fn repository_contracts_normalizes_nonexistent_roots_without_filesystem_access() {
        for (left, right) in [
            ("/nonexistent/pce", "/nonexistent/pce/"),
            ("/nonexistent/pce", "/nonexistent/./pce"),
            ("/nonexistent/pce", "/nonexistent/cache/../pce"),
            ("/nonexistent/pce", "//nonexistent//pce///"),
        ] {
            assert_eq!(
                lexically_normalized_repository_root(left),
                lexically_normalized_repository_root(right)
            );
        }
        assert_ne!(
            lexically_normalized_repository_root("/nonexistent/pce"),
            lexically_normalized_repository_root("/nonexistent/other")
        );

        let records = [
            parsed_current_contract_record(
                9,
                "pce",
                "/this/path/must/not/exist/pce/",
                "cargo test --workspace --first",
            ),
            parsed_current_contract_record(
                10,
                "pce",
                "/this/path/must/not/exist/cache/../pce",
                "cargo test --workspace --latest",
            ),
        ];
        let contracts =
            repository_contracts(&records).expect("lexically identical roots should replace");
        let [RepositoryContract::Current(payload)] = contracts.as_slice() else {
            panic!("one current repository contract expected");
        };
        assert_eq!(payload.stated.test, "cargo test --workspace --latest");
    }

    #[test]
    fn repository_contracts_rejects_genuine_identity_conflicts() {
        let same_name = [
            parsed_current_contract_record(
                9,
                "pce",
                "/nonexistent/pce",
                "cargo test --workspace --first",
            ),
            parsed_current_contract_record(
                10,
                "pce",
                "/nonexistent/other",
                "cargo test --workspace --latest",
            ),
        ];
        let error =
            repository_contracts(&same_name).expect_err("same name at another root is ambiguous");
        assert_eq!(
            error.to_string(),
            "ambiguous duplicate repository contract at sequence 10 for repository pce and root /nonexistent/other"
        );

        let same_root = [
            parsed_current_contract_record(
                9,
                "pce",
                "/nonexistent/pce",
                "cargo test --workspace --first",
            ),
            parsed_current_contract_record(
                10,
                "consumer",
                "/nonexistent/pce",
                "cargo test --workspace --latest",
            ),
        ];
        let error = repository_contracts(&same_root)
            .expect_err("same root under another name is ambiguous");
        assert_eq!(
            error.to_string(),
            "ambiguous duplicate repository contract at sequence 10 for repository consumer and root /nonexistent/pce"
        );
    }

    #[test]
    fn repository_contracts_does_not_demote_current_when_later_legacy_matches_identity() {
        let records = [
            parsed_current_contract_record(
                9,
                "pce",
                "/nonexistent/pce",
                "cargo test --workspace --latest",
            ),
            parsed_legacy_contract_record(10, "pce", "/nonexistent/./pce/"),
        ];

        let contracts =
            repository_contracts(&records).expect("later legacy must not demote current");
        let [RepositoryContract::Current(payload)] = contracts.as_slice() else {
            panic!("one current repository contract expected");
        };
        assert_eq!(payload.stated.test, "cargo test --workspace --latest");
    }

    #[test]
    fn repository_contracts_retains_current_fields_and_legacy_identity() {
        let current_line = r#"{"sequence":9,"timestamp":"2026-07-27T12:35:04.000Z","kind":"repository-contract","node":"m2-s1","payload":{"repository":"pce","repo_root":"/workspace/pce","stated":{"format":"cargo fmt --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --release","version_policy":"NONE","branch_convention":"pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>","pull_request_convention":"step head targets the matching milestone integration branch"},"observations":{"format":0,"lint":0,"typecheck":0,"test":0,"build":0},"workflow_map":{"ci.yml":"cargo test --workspace","docs.yml":null},"appendable":{"environment_hazards":["stdin is reserved for event payload input"],"gate_orderings":["format before lint before typecheck before test before build"],"lockfile_rules":["Cargo.lock must remain synchronized with Cargo.toml"]},"evidence":"cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"}}"#;
        let legacy_line = r#"{"sequence":6,"timestamp":"2026-07-27T12:35:01.000Z","kind":"repository-contract","node":"m1-s1","payload":{"repository":"pce","repo_root":"/workspace/pce","stack":"Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)","format":"cargo fmt --all --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --workspace","preflight":"cargo check --workspace --all-targets","gates_rule":"From the repo root, all four gates must exit zero before committing.","install":"None required for gates.","evidence":"rustc --version\ncargo --version\ngit rev-parse --show-toplevel"}}"#;

        let current_record = parse_event_line(current_line).expect("current contract should parse");
        let current =
            repository_contracts(&[current_record]).expect("current projection should succeed");
        let [RepositoryContract::Current(payload)] = current.as_slice() else {
            panic!("current repository contract expected");
        };
        assert_eq!(payload.repository.as_str(), "pce");
        assert_eq!(payload.repo_root.as_str(), "/workspace/pce");
        assert_eq!(payload.stated.format, "cargo fmt --check");
        assert_eq!(
            payload.stated.lint,
            "cargo clippy --workspace --all-targets"
        );
        assert_eq!(
            payload.stated.typecheck,
            "cargo check --workspace --all-targets"
        );
        assert_eq!(payload.stated.test, "cargo test --workspace");
        assert_eq!(payload.stated.build, "cargo build --release");
        assert_eq!(payload.stated.version_policy, VersionPolicy::None);
        assert_eq!(
            payload.stated.branch_convention,
            "pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>"
        );
        assert_eq!(
            payload.stated.pull_request_convention,
            "step head targets the matching milestone integration branch"
        );
        assert_eq!(payload.observations.format.get(), 0);
        assert_eq!(payload.observations.lint.get(), 0);
        assert_eq!(payload.observations.typecheck.get(), 0);
        assert_eq!(payload.observations.test.get(), 0);
        assert_eq!(payload.observations.build.get(), 0);
        assert_eq!(
            payload.workflow_map.as_map().get("ci.yml"),
            Some(&Some("cargo test --workspace".to_owned()))
        );
        assert_eq!(payload.workflow_map.as_map().get("docs.yml"), Some(&None));
        assert_eq!(
            payload.appendable.environment_hazards,
            ["stdin is reserved for event payload input"]
        );
        assert_eq!(
            payload.appendable.gate_orderings,
            ["format before lint before typecheck before test before build"]
        );
        assert_eq!(
            payload.appendable.lockfile_rules,
            ["Cargo.lock must remain synchronized with Cargo.toml"]
        );
        assert_eq!(
            payload.evidence.as_str(),
            "cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"
        );

        let legacy_record = parse_event_line(legacy_line).expect("legacy contract should parse");
        let legacy =
            repository_contracts(&[legacy_record]).expect("legacy projection should succeed");
        let [RepositoryContract::Legacy(payload)] = legacy.as_slice() else {
            panic!("legacy repository contract expected");
        };
        assert_eq!(payload.repository.as_str(), "pce");
        assert_eq!(payload.repo_root.as_str(), "/workspace/pce");
        assert_eq!(
            payload.evidence.as_str(),
            "rustc --version\ncargo --version\ngit rev-parse --show-toplevel"
        );
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
    fn contract_parser_accepts_only_exact_ordered_paths() {
        let command = parse_command(
            [
                "contract",
                "check",
                "--file",
                "contract.json",
                "--repo-root",
                "repo",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("exact contract check command should parse");
        let Command::ContractCheck {
            contract_path,
            repository_root,
        } = command
        else {
            panic!("typed contract-check command expected");
        };
        assert_eq!(contract_path, PathBuf::from("contract.json"));
        assert_eq!(repository_root, PathBuf::from("repo"));

        let invalid = [
            vec![
                "contract",
                "check",
                "--repo-root",
                "repo",
                "--file",
                "contract.json",
            ],
            vec!["contract", "check", "--file", "contract.json"],
            vec![
                "contract",
                "check",
                "--file",
                "contract.json",
                "--file",
                "contract.json",
            ],
            vec![
                "contract",
                "check",
                "--file",
                "contract.json",
                "--repo-root",
                "repo",
                "extra",
            ],
            vec![
                "contract",
                "inspect",
                "--file",
                "contract.json",
                "--repo-root",
                "repo",
            ],
            vec![
                "contract",
                "check",
                "--file",
                "--contract",
                "--repo-root",
                "repo",
            ],
            vec![
                "contract",
                "check",
                "--file",
                "contract.json",
                "--repo-root",
                "--repo",
            ],
        ];
        for args in invalid {
            let err = parse_command(args.into_iter().map(str::to_owned))
                .expect_err("non-contract CLI shape should fail");
            assert_eq!(err.to_string(), USAGE);
        }
    }

    #[test]
    fn successive_contract_measurements_do_not_reexecute_unchanged_gate_commands() {
        let directory = tempdir().expect("temporary directory should create");
        let repository_root = directory.path().join("repo");
        fs::create_dir(&repository_root).expect("repository fixture should create");
        let contract_path = directory.path().join("contract.json");
        fs::write(
            &contract_path,
            br#"{
  "stated": {
    "gates": {
      "format": "printf x >> gate-runs",
      "lint": "printf x >> gate-runs",
      "typecheck": "printf x >> gate-runs",
      "test": "printf x >> gate-runs",
      "build": "printf x >> gate-runs"
    },
    "version_policy": "NONE",
    "branches": {
      "default": "main",
      "milestone": "pce/{vision}/milestone-{milestone}",
      "step": "pce/{vision}/m{milestone}-s{step}"
    },
    "pull_requests": {
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    },
    "workflows": []
  },
  "appendable": {
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }
}"#,
        )
        .expect("contract fixture should write");

        let first = measure_tracked_contract_at_root(&contract_path, &repository_root, None)
            .expect("first measurement should succeed");
        let marker_path = repository_root.join("gate-runs");
        assert_eq!(
            fs::read_to_string(&marker_path).expect("marker should read"),
            "xxxxx"
        );

        measure_tracked_contract_at_root(&contract_path, &repository_root, Some(&first))
            .expect("second measurement should reuse prior observations");
        assert_eq!(
            fs::read_to_string(marker_path).expect("marker should read"),
            "xxxxx"
        );
    }

    #[test]
    fn ready_parser_accepts_policy_free_shapes() {
        let command = parse_command(
            [
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("exact ready command should parse");
        let Command::Ready {
            log_path,
            recovery_log_path,
            vision_dir,
            graph_path,
        } = command
        else {
            panic!("typed ready command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(recovery_log_path.as_str(), "events.jsonl");
        assert_eq!(graph_path, None);
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
        let with_graph = parse_command(
            [
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--graph",
                "planning/2026-07-28-example/milestone-3/steps.json",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("ready command with graph should parse");
        let Command::Ready {
            graph_path: Some(graph_path),
            ..
        } = with_graph
        else {
            panic!("typed ready command expected");
        };
        assert_eq!(
            graph_path.as_str(),
            "planning/2026-07-28-example/milestone-3/steps.json"
        );
    }

    #[test]
    fn ready_parser_rejects_policy_and_non_contract_shapes() {
        let invalid = [
            vec![
                "ready",
                "--file",
                "events.jsonl",
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
                "planning/2026-07-28-example",
                "--graph",
            ],
            vec![
                "ready",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--file",
                "events.jsonl",
            ],
            vec![
                "ready",
                "--file",
                "--events",
                "--vision-dir",
                "planning/2026-07-28-example",
            ],
            vec![
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "--vision",
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
            RepositoryBranchName::parse("pce/event-log/milestone-2")
                .expect("branch fixture should parse"),
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

    #[test]
    fn dispatch_logging_group_is_exact_ordered_and_all_or_none() {
        let prefix = [
            "dispatch",
            "codex",
            "--cwd",
            "/tmp",
            "--sandbox",
            "workspace-write",
        ];
        let complete = prefix
            .into_iter()
            .chain([
                "--log-file",
                "/tmp/events.jsonl",
                "--node",
                "m3-s1",
                "--role",
                "step-executor",
                "--ref",
                "abc",
                "--evidence",
                "fixture",
                "--",
                "PROMPT",
            ])
            .map(str::to_owned);
        let command = parse_command(complete).expect("parse complete logging group");
        let Command::Dispatch { logging, .. } = command else {
            panic!("parsed another command")
        };
        let Some(DispatchLoggingMode::Live { path, metadata }) = logging else {
            panic!("expected live logging metadata")
        };
        assert_eq!(path, PathBuf::from("/tmp/events.jsonl"));
        assert_eq!(metadata.node.as_str(), "m3-s1");
        assert_eq!(metadata.role.as_str(), "step-executor");

        let dry = prefix
            .into_iter()
            .chain([
                "--output-schema",
                "/tmp/schema.json",
                "-o",
                "/tmp/output.json",
                "--plan-file",
                "/dev/null",
                "--log-file",
                "/tmp/events.jsonl",
                "--node",
                "m3-s2",
                "--role",
                "step-executor",
                "--ref",
                "abc",
                "--evidence",
                "fixture",
                "--dry-run",
                "--",
                "PROMPT",
            ])
            .map(str::to_owned);
        let command = parse_command(dry).expect("parse complete dry-run logging group");
        let Command::Dispatch { logging, .. } = command else {
            panic!("parsed another command")
        };
        let Some(DispatchLoggingMode::DryRun { path, metadata }) = logging else {
            panic!("expected dry-run logging metadata")
        };
        assert_eq!(path, PathBuf::from("/tmp/events.jsonl"));
        assert_eq!(metadata.node.as_str(), "m3-s2");
        assert_eq!(metadata.role.as_str(), "step-executor");

        for suffix in [
            vec!["--log-file", "/tmp/events.jsonl", "--"],
            vec!["--node", "m3-s1", "--"],
            vec![
                "--log-file",
                "/tmp/events.jsonl",
                "--role",
                "step-executor",
                "--",
            ],
            vec!["--evidence", "fixture", "--"],
        ] {
            let args = prefix.into_iter().chain(suffix).map(str::to_owned);
            let error = parse_command(args).expect_err("partial group must fail");
            assert!(format!("{error:#}").contains("dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence"));
        }

        let misordered = prefix
            .into_iter()
            .chain([
                "--log-file",
                "/tmp/events.jsonl",
                "--role",
                "step-executor",
                "--node",
                "m3-s1",
                "--ref",
                "abc",
                "--evidence",
                "fixture",
                "--",
                "PROMPT",
            ])
            .map(str::to_owned);
        let error = parse_command(misordered).expect_err("misordered complete group must fail");
        assert!(format!("{error:#}").contains("dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence"));

        let before_plan = prefix
            .into_iter()
            .chain([
                "--log-file",
                "/tmp/events.jsonl",
                "--node",
                "m3-s1",
                "--role",
                "step-executor",
                "--ref",
                "abc",
                "--evidence",
                "fixture",
                "--plan-file",
                "/tmp/plan.md",
                "--",
                "-",
            ])
            .map(str::to_owned);
        let error = parse_command(before_plan).expect_err("logging before plan must fail");
        assert!(format!("{error:#}").contains("dispatch arguments require the `--` delimiter"));
    }

    #[test]
    fn terminal_observation_ignores_additive_usage_and_non_object_json() {
        let observation = observe_terminal_line(br#"{"type":"turn.completed","usage":{"total_tokens":146,"input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5}}"#);
        assert_eq!(
            classify_terminal_usage(
                &[observation],
                DispatchExitStatus::Exited {
                    code: ExitCode::new(0)
                }
            ),
            Ok(CodexTokenUsage::Measured {
                input_tokens: InputTokens::new(101),
                cached_input_tokens: CachedInputTokens::new(23),
                output_tokens: OutputTokens::new(17),
                reasoning_output_tokens: ReasoningOutputTokens::new(5),
            })
        );
        for fixture in [b"5".as_slice(), b"\"x\"".as_slice(), b"[]".as_slice()] {
            assert_eq!(
                observe_terminal_line(fixture),
                TerminalObservation::NonTerminal
            );
        }
    }
}
