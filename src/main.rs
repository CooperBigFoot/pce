use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::ffi::{CString, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::Shutdown;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Error, Result, anyhow, bail};
use notify::{RecursiveMode, Watcher};
use pce_core::GateCommand;
use pce_core::tracked_contract::parse_gate_command;
use pce_core::{
    AbsoluteDispatchTemporaryDirectory, AbsoluteGateExecClientPath,
    AbsoluteGateExecutionEvidencePath, AbsoluteGateExecutionSocketPath, AbsoluteOutputPath,
    AbsoluteRequiredArtifactPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory,
    AbsoluteWorktreeRoot, AcceptanceCriteria, ActReversibility, AmendmentProof,
    AmendmentRepositoryRefs, AppendError, AppendableCategory, AppendableFinding, ArgumentVector,
    ArtifactOutcome, ArtifactPath, ArtifactProduction, AuthorityFailure, BaseCurrencyRiskEntry,
    BaseCurrencyRiskMode, BranchState, BuiltArtifactRef, CanonicalNode as DispatchNode,
    CheckoutFailure, CheckoutStage, ChildEnvironment, CodexTerminalObservation, CodexTerminalUsage,
    CommandExitStatus, CompletionCriterionStatus, CompletionDecision, CompositionInput,
    CreationDate, CriterionChangeDecision, CriterionExecution, CurrentArtifactObservation,
    CurrentArtifactState, DispatchAdmission, DispatchAttempt, DispatchCandidate,
    DispatchCompletionPayload, DispatchDuration, DispatchEnvelope, DispatchEnvironmentObservation,
    DispatchExitStatus, DispatchIdentityObservation, DispatchLedger, DispatchLedgerCompletion,
    DispatchLogging, DispatchPayload, DispatchProcessIdentity, DispatchProjectionInput,
    DispatchRef, DispatchRequiredArtifactObservation, DispatchRole, DispatchRoleClass,
    DispatchRootCause, DispatchTarget, DispatchTokenUsage, DispatchVisionSource,
    DispatchWorkerProcessObservation, DispatchabilityResult, DriverAssemblyState, DriverEvent,
    DriverRefProduct, EnvironmentFailureLimit, EnvironmentPreparationOutcome, EventBodyRef,
    EventKindName, EventLogTail, EventLogTailLine, EventRecord, EventRecordFilter, EventTimestamp,
    Evidence, ExactPullRequestIdentity, ExactPullRequestState, ExceptionalMergeChain,
    ExceptionalMergeChainObservation, Executable, ExitCode, ExpectedVerdictOutcome,
    FileObservation, FindingAdmission, FindingRejectionReason, FindingReplayDecision,
    FinishedResult, GateExecutionEvidence, GateExecutionRecord, GateExecutionRecorderConfig,
    GateExecutionRef, GateExecutionRejection, GateExecutionResponse, GateFailureLimit,
    GateObservedResult, GateProcessObservation, GateProcessStimulus, GateStimulus,
    GateTerminalStatus, GitAuthorityObservation, GitHubAuthorityObservation,
    GitHubPullRequestObservation, GitMergeObservation, HerdrAgentLocation, HerdrInvocation,
    HerdrPaneId, HerdrSessionName, HerdrTabId, HerdrWorkspaceId, HerdrWorktreeSpec, KnownPayload,
    LandingReadinessDecision, LegacyRepositoryContractPayload, LocalPatchLimit,
    MeasuredContractSnapshot, MergeStatus, MergeSubject, MilestoneMergeSubject, MilestoneNode,
    NamedReplayRef, NodeId, NonProductionHoldOpenPayload, NonProductionKey, ObservedExitStatus,
    ObservedWorkflowName, OracleFailure, OracleStage, OrderingEdge, PackageGateChallenge,
    PackageWorkerResult, PackageWorkerStoppedAt, PairedCampaign, PairedExecutionProofError,
    PairedReplayClassification, PaneCleanupOutcome, ProcessIdentityObservation, ProcessNumber,
    ProcessStartIdentity, PullRequestAuthorityObservation, PullRequestNumber, PullRequestSelector,
    ReconciledDeadDispatchCompletionPayload, ReconciledDispatchOutcome, RecordedProcessIdentity,
    RecoveryLimits, RecoveryLogPath, RecoveryRung, ReferenceValidation, ReplayArtifactObservation,
    ReplayClassifications, ReplayObservation, ReplayRefResult, RepositoryBranchName,
    RepositoryContractPayload, RepositoryDispatchInput, RepositoryFetchObservation, RepositoryName,
    RepositoryObservation, RepositoryObservationFailure, RepositoryObservationRef,
    RepositoryRelativePath, RepositoryRoot, RepositoryWorktree, RequiredArtifactPresence,
    RetryLimit, RiskOrdering, RunSnapshot, Sandbox, SeatbeltCapability, Sequence, Sha256Digest,
    SignalNumber, SpawnDispatchOutcome, SpawnFailedDispatchCompletionPayload, SquashCommitOid,
    StdinBinding, StepAuthorityObservation, StepNode, StructuredArtifactObservation,
    SurvivingProcesses, TagName, TagState, TagTarget, TrackedRepositoryContract, UnparsedPayload,
    UsageAbsenceReason, VersionPolicy, VisionGoal, VisionName, VisionSlug,
    WorkPackageClassification, WorkPackageGraph, WorkPackageId, WorkPackageMergeObservation,
    WorkPackageMergeSubject, WorkerArgumentVector, WorkerEnvironment, WorktreeIdentity,
    WorktreeState, WriteKind, admit_recurrent_finding, append_event, charged_failure_count,
    classify_claude_result, classify_codex_terminal_usage, classify_dispatch_admission,
    classify_dispatch_check_in, classify_replay_pair, classify_seatbelt_capability,
    compose_gate_arguments, compose_herdr_work_package_dispatch, compose_local_patch_brief,
    compose_package_gate_brief, compose_package_worker_argv, compose_package_worker_brief,
    compose_planning_role_frame, compute_dispatchability, create_vision,
    criteria_invariance_violations, derive_dispatch_outcome_state, derive_driver_snapshot,
    derive_herdr_agent_name, derive_merge_status, derive_milestone_merge_status,
    derive_package_result_path, derive_run_state, derive_run_state_with_dispatch_artifacts,
    derive_run_state_with_exceptional_merge_chains, derive_work_package_merge_status,
    dispatch_completion_payload, dispatch_invocation, dispatch_payload, effective_criteria,
    evaluate_completion, evaluate_landing_readiness, event_record_matches,
    extract_conservative_artifact_references, fold_dispatch_ledger, fold_paired_execution_proof,
    fold_replay_runs, gate_failure_outcome, judge_finding_replay,
    latest_criterion_failure_evidence, measure_contract_snapshot, meter_dispatches,
    next_gate_attempt, normalize_replay_observation, paired_stimulus_identity,
    parse_acceptance_criteria, parse_claude_result, parse_criterion_revision_manifest,
    parse_dispatch_process_identity, parse_event_line, parse_gate_execution_evidence,
    parse_gate_stimulus, parse_package_gate_outcome, parse_package_worker_result,
    parse_paired_falsification_verdict, parse_replay_output_path, parse_replay_schema_path,
    parse_tracked_repository_contract, parse_work_package_graph, pending_completed_pane_cleanups,
    pending_gate_challenges, ready_work_packages, rebase_gate_stimulus, recovery_attempt_records,
    recovery_base_brief, recovery_budget, render_dispatch_projection, render_human_snapshot,
    render_package_run, repeated_identical_worker_blocker, seatbelt_capability_probe,
    serialize_dispatch_check_in, serialize_dispatch_process_identity,
    serialize_package_worker_result, serialize_tracked_repository_contract,
    titles_conservatively_overlap, unchanged_package_ids, validate_artifact,
    validate_criterion_revisions, validate_package_gate_finding_repositories,
    validate_package_gate_repositories, validate_verdict_references, validate_workflow_coverage,
    validated_dispatch_completion_payload, verify_criterion_change, verify_mechanical_freeze,
    worker_environment_outcome,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const USAGE: &str = concat!(
    "usage: pce vision new \"<name>\"\n",
    "       pce vision check < vision.md\n",
    "       pce log --file <LOG_PATH> --kind <KIND> --node <NODE>\n",
    "       pce log read --file <LOG_PATH> [--kind <KIND>] [--node <NODE>]\n",
    "       pce log meter\n",
    "       pce status --file <LOG_PATH> --vision-dir <VISION_DIR> [--human]\n",
    "       pce ready --file <LOG_PATH> --vision-dir <VISION_DIR> [--graph <APPROVED_ARTIFACT_PATH>] [--override-risk-ordering]\n",
    "       pce graph check --file <GRAPH_PATH> [--strict] [--repository <NAME=SOURCE_WORKTREE>]...\n",
    "       pce graph freeze --vision-dir <VISION_DIR> [--mechanical] [--criterion-revisions <HUMAN_RECORD_PATH>] [--accept-base-currency-risk <HUMAN_RECORD_PATH>] --repository <NAME=SOURCE_WORKTREE> [--repository <NAME=SOURCE_WORKTREE>]...\n",
    "       pce package brief --vision <VISION_PATH> --graph <GRAPH_PATH> --package <PACKAGE_ID> --worktree <NAME=ABSOLUTE_PATH>...\n",
    "       pce package agent --vision <VISION_PATH> --graph <GRAPH_PATH> --package <PACKAGE_ID> --outcome <ABSOLUTE_OUTCOME_PATH> [--brief <ABSOLUTE_BRIEF_PATH>] -- <WORKER_ARG>...\n",
    "       pce package gate-brief --vision <VISION_PATH> --graph <GRAPH_PATH> --package <PACKAGE_ID> --artifact-ref <REF> --worktree <NAME=ABSOLUTE_PATH>...\n",
    "       pce package gate-agent --vision <VISION_PATH> --graph <GRAPH_PATH> --package <PACKAGE_ID> --artifact-ref <REF> --outcome <ABSOLUTE_OUTCOME_PATH> [--issuance <N>] [--attempt <N>] [--challenges <ABSOLUTE_PATH>] [--defer-finding-validation] -- <WORKER_ARG>...\n",
    "       pce package render --graph <GRAPH_PATH> [--journal <DRIVER_JOURNAL>] --output <HTML_PATH>\n",
    "       pce package driver-status --graph <GRAPH_PATH> --journal <DRIVER_JOURNAL> [--override-risk-ordering]\n",
    "       pce package materialize-refs --graph <GRAPH_PATH> --journal <DRIVER_JOURNAL> --repository <NAME=SOURCE_WORKTREE>...\n",
    "       pce package driver-overrule --graph <GRAPH_PATH> --journal <DRIVER_JOURNAL> --package <PACKAGE_ID> --rationale <TEXT>\n",
    "       pce package driver-run --graph <GRAPH_PATH> --journal <DRIVER_JOURNAL> --repository <NAME=SOURCE_WORKTREE>... [--prepare <NAME=COMMAND>]... [--worker-env <NAME>]... [--herdr-session <NAME>] [--override-risk-ordering] [--retry-limit <N>] [--local-patch-limit <N>] [--environment-failure-limit <N>] [--gate-failure-limit <N>] [--wait-timeout-ms <N>] [--worker-override -- <WORKER_OVERRIDE_ARG>...]\n",
    "       pce package criteria-run --graph <GRAPH_PATH> --journal <DRIVER_JOURNAL> --package <PACKAGE_ID> --repository <NAME=SOURCE_WORKTREE>... [--prepare <NAME=COMMAND>]...\n",
    "       pce package replay-finding --graph <GRAPH_PATH> --journal <DRIVER_JOURNAL> --package <PACKAGE_ID> --gate <GATE_ID> --finding <INDEX> --outcome <GATE_OUTCOME> --repository <NAME=SOURCE_WORKTREE>... [--prepare <NAME=COMMAND>]...\n",
    "       pce criteria check --file <LOG_PATH> --vision-dir <VISION_DIR>\n",
    "       pce completion check --file <LOG_PATH> --vision-dir <VISION_DIR> --finished-result <FINISHED_RESULT>\n",
    "       pce landing check --file <LOG_PATH> --vision-dir <VISION_DIR> --finished-result <FINISHED_RESULT>\n",
    "       pce contract check --file <CONTRACT_PATH> --repo-root <REPOSITORY_ROOT>\n",
    "       pce contract bootstrap --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --repository <REPOSITORY> --node <NODE>\n",
    "       pce contract refresh --file <LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE>\n",
    "       pce contract learn --file <CURRENT_LOG_PATH> --prior-file <PRIOR_LOG_PATH> --repo-root <REPOSITORY_ROOT> --node <NODE> --category <environment-hazard|gate-ordering|lockfile-rule> --finding <FINDING>\n",
    "       pce dispatch package --file <ABSOLUTE_LOG_PATH> --vision-dir <ABSOLUTE_VISION_DIR> --graph <GRAPH_PATH> --package <PACKAGE_ID> --required-artifact <ABSOLUTE_ARTIFACT_PATH> --repository <NAME=ABSOLUTE_ROOT>... --env <NAME=VALUE> --env <NAME=VALUE> --env <NAME=VALUE> -- <WORKER_ARG>...\n",
    "       pce dispatch codex --cwd <ABSOLUTE_WORKING_DIRECTORY> --sandbox workspace-write [--env <NAME=VALUE>]... [--output-schema <ABSOLUTE_SCHEMA_PATH> -o <ABSOLUTE_OUTPUT_PATH>] [--plan-file <PLAN_PATH>] [--log-file <ABSOLUTE_LOG_PATH> --node <NODE> --role <ROLE> --ref <REF> --evidence <EVIDENCE> --required-artifact <ABSOLUTE_ARTIFACT_PATH> [--planning-act <repeatable|irreversible>] [--dry-run]] -- <CODEX_ARGUMENT>...\n",
    "       pce dispatch gate --cwd <ABSOLUTE_WORKING_DIRECTORY> [--env <NAME=VALUE>]... --output-schema <ABSOLUTE_SCHEMA_PATH> -o <ABSOLUTE_OUTPUT_PATH> [--plan-file <PLAN_PATH>] [--log-file <ABSOLUTE_LOG_PATH> --node <NODE> --role <ROLE> --ref <REF> --evidence <EVIDENCE> --required-artifact <ABSOLUTE_ARTIFACT_PATH> [--planning-act <repeatable|irreversible>] [--dry-run]] -- <CLAUDE_ARGUMENT>...\n",
    "       pce dispatch check-in --file <LOG_PATH>\n",
    "       pce dispatch reconcile --file <ABSOLUTE_LOG_PATH> --issuance <ISSUANCE_SEQUENCE> --node <NODE>\n",
    "       pce gate exec\n",
    "       pce gate replay --repo-root <ABSOLUTE_REPOSITORY_ROOT> --evidence <ABSOLUTE_EVIDENCE_PATH> --execution-ref <EXECUTION_REF> --broken-ref <REF> --repaired-ref <REF> --schema <REPOSITORY_RELATIVE_SCHEMA_PATH> --output <REPOSITORY_RELATIVE_OUTPUT_PATH> --expected <conforming-verdict|nonconforming-verdict>\n",
    "       pce gate execution-subject-probe --output <REPOSITORY_RELATIVE_OUTPUT_PATH>\n",
    "       pce gate paired-execution-proof --repo-root <ABSOLUTE_REPOSITORY_ROOT> --artifacts <ABSOLUTE_EMPTY_DIRECTORY> --env <NAME=VALUE> --env <NAME=VALUE> --env <NAME=VALUE>\n",
    "       --worker-env forwards that named driver variable only to package and gate worker workspaces; criteria and --prepare commands continue to inherit the driver's full launch environment."
);
static GRAPH_FREEZE_NONCE: AtomicU64 = AtomicU64::new(0);
const GATE_REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(2);
const GATE_RESPONSE_WRITE_TIMEOUT: Duration = Duration::from_secs(1);
// A silent child may make no observable progress for this long before it is terminated.
const GATE_EXECUTION_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(5 * 60);
// A child is never allowed to occupy a recorder worker longer than this, even while producing I/O.
const GATE_EXECUTION_OVERALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const GATE_PROCESS_TERMINATION_TIMEOUT: Duration = Duration::from_secs(1);
// After child exit, open output pipes may remain idle this long before their drainers are abandoned.
const GATE_OUTPUT_DRAIN_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(30);
// Output drainers may run for at most this long after child exit, even while bytes keep arriving.
const GATE_OUTPUT_DRAIN_OVERALL_TIMEOUT: Duration = Duration::from_secs(2 * 60);
const GATE_ACCEPT_POLL_INTERVAL: Duration = Duration::from_millis(10);
const GATE_SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(12);
const GATE_MAX_CONNECTION_WORKERS: usize = 32;
const GATE_MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
// Replay only asks git to resolve or locally materialize an already-present commit. These bounded,
// metadata-local operations are expected to be fast and do not inherit a stimulus-sized timeout.
const REPLAY_GIT_TIMEOUT: Duration = Duration::from_secs(5);
// A quiet production stimulus may legitimately spend minutes in a nested agent before producing
// output; only five minutes without stdout/stderr progress makes that execution inactive.
const REPLAY_RUN_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(5 * 60);
// Even a continuously active replay process must finish, so one setup action or command gets a
// generous cap equal to the recorder's production execution cap.
const REPLAY_RUN_OVERALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
// One replay performs four clean runs plus materialization. This cap bounds the whole worker while
// leaving room for production stimuli instead of assuming unit-test runtimes.
const REPLAY_OVERALL_TIMEOUT: Duration = Duration::from_secs(40 * 60);
// This wait exists only when the test-only pause seam is explicitly enabled; production replay
// never waits here, so a short synchronization timeout is intentional.
const REPLAY_PAUSE_TIMEOUT: Duration = Duration::from_secs(10);
const REPLAY_PAUSE_ENV: &str = "PCE_REPLAY_PAUSE_AFTER_RESOLVE";
const RUN_SNAPSHOT_SCHEMA: &str = include_str!("../skills/pce/schemas/run-snapshot.schema.json");
const VERDICT_SCHEMA: &str = include_str!("../skills/pce/schemas/verdict.schema.json");
const PAIRED_BROKEN_REF: &str = "a8a87cb44f84988fa61904bfba48614401482851";
const PAIRED_BROKEN_OID: &str = "a8a87cb44f84988fa61904bfba48614401482851";
const PAIRED_REPAIRED_REF: &str = "cbe499ef94864d223518ad328db2a396551fa85b";
const PAIRED_REPAIRED_OID: &str = "cbe499ef94864d223518ad328db2a396551fa85b";
const PAIRED_DELIVERY_BASE_OID: &str = "6afc64eb9171350dde36d0def74bdb2ced0e5293";
const PAIRED_SUBJECT_ARTIFACT: &str = "artifact";
const PAIRED_SUBJECT_GATE: &str = "gate";
const PAIRED_SUBJECT_FIXTURE: &str = "tests/fixtures/execution-subject";
const PAIRED_REPLAY_SCHEMA: &str = "verdict.schema.json";
const PAIRED_REPLAY_OUTPUT: &str = ".pce-execution-subject-verdict.json";
const PAIRED_REPLAY_EXPECTED: &str = "conforming-verdict";
const PAIRED_PROGRAM_RELATIVE: &str = "target/pce-execution-subject-probe/pce";
const PAIRED_REPLAY_PROGRAM_ENV: &str = "PCE_PAIRED_REPLAY_PROGRAM";
const PAIRED_RECORDED_ROOT_ENV: &str = "PCE_PAIRED_RECORDED_ROOT";
const PAIRED_GATE_EXEC_CLIENT_ENV: &str = "PCE_PAIRED_GATE_EXEC_CLIENT";
const PAIRED_CRITIC_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const PAIRED_CAMPAIGN_TIMEOUT: Duration = Duration::from_secs(40 * 60);
const PAIRED_PROVENANCE_LIMITATION: &str = "Live authentication and free-running gate provenance cannot be reproduced from retained bytes alone; the immutable execution records and replay classifications are reproducible, while the claim that these bytes came from the recorded live campaign depends on the retained binary digest and invocation manifest.";
const PAIRED_CRITIC_TASK: &str = "Falsify the delivered behavior in exactly artifact and gate. Judge it by executions, not review alone. You must submit this ordinary probe through the harness before deciding: program is exactly \"target/pce-execution-subject-probe/pce\"; arguments are [\"gate\",\"execution-subject-probe\",\"--output\",\".pce-execution-subject-verdict.json\"]; input is empty; setup is empty; working_directory is this checkout; environment contains exactly PATH, HOME, and USER from your own explicit child environment. You may submit additional stimuli. A blocking issue must cite its own demonstrated primary execution and its executed replacement. Write the verdict required by your binary-owned role frame.";

fn paired_critic_task() -> String {
    format!(
        "{PAIRED_CRITIC_TASK} The verdict output must conform exactly to this compiled JSON Schema, quoted verbatim:\n<verdict-schema>\n{VERDICT_SCHEMA}</verdict-schema>"
    )
}
const ORIGIN: &str = "origin";
const LEGACY_RELEASE_TAG: &str = "v0.1.16";
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
struct PackageBriefCommand {
    vision_path: PathBuf,
    graph_path: PathBuf,
    package_id: String,
    worktrees: Vec<(String, PathBuf)>,
}

#[derive(Debug)]
struct PackageAgentCommand {
    vision_path: PathBuf,
    graph_path: PathBuf,
    package_id: String,
    outcome_path: PathBuf,
    brief_path: Option<PathBuf>,
    worker_arguments: Vec<String>,
}

#[derive(Debug)]
struct PackageGateBriefCommand {
    vision_path: PathBuf,
    graph_path: PathBuf,
    package_id: String,
    artifact_ref: BuiltArtifactRef,
    worktrees: Vec<(String, PathBuf)>,
}

#[derive(Debug)]
struct PackageGateAgentCommand {
    vision_path: PathBuf,
    graph_path: PathBuf,
    package_id: String,
    artifact_ref: BuiltArtifactRef,
    outcome_path: PathBuf,
    defer_finding_validation: bool,
    issuance: u64,
    attempt: u32,
    challenges_path: Option<PathBuf>,
    worker_arguments: Vec<String>,
}

#[derive(Debug)]
struct PackageDispatchCommand {
    log_path: PathBuf,
    vision_dir: PathBuf,
    graph_path: PathBuf,
    require_graph_at_vision_root: bool,
    package_id: String,
    attempt: Option<DispatchAttempt>,
    required_artifact_path: AbsoluteRequiredArtifactPath,
    repositories: Vec<(String, PathBuf)>,
    base_refs: BTreeMap<String, String>,
    conflicted_joins: BTreeMap<String, (CompositionInput, Vec<String>)>,
    herdr_session: Option<HerdrSessionName>,
    environment: BTreeMap<String, String>,
    worker_arguments: Vec<String>,
}

#[derive(Debug)]
struct PackageRenderCommand {
    graph_path: PathBuf,
    journal_path: Option<PathBuf>,
    output_path: PathBuf,
}

#[derive(Debug)]
struct DriverStatusCommand {
    graph_path: PathBuf,
    journal_path: PathBuf,
    override_risk_ordering: bool,
}

#[derive(Debug)]
struct MaterializeRefsCommand {
    graph_path: PathBuf,
    journal_path: PathBuf,
    repositories: Vec<(String, PathBuf)>,
}

#[derive(Debug)]
struct DriverCriteriaCommand {
    graph_path: PathBuf,
    journal_path: PathBuf,
    package_id: String,
    repositories: Vec<(String, PathBuf)>,
    preparations: BTreeMap<String, String>,
}

#[derive(Debug)]
struct DriverReplayCommand {
    graph_path: PathBuf,
    journal_path: PathBuf,
    package_id: String,
    gate: String,
    finding: usize,
    outcome_path: PathBuf,
    repositories: Vec<(String, PathBuf)>,
    preparations: BTreeMap<String, String>,
}

#[derive(Debug)]
struct DriverOverruleCommand {
    graph_path: PathBuf,
    journal_path: PathBuf,
    package_id: String,
    rationale: String,
}

#[derive(Debug)]
struct DriverRunCommand {
    graph_path: PathBuf,
    journal_path: PathBuf,
    repositories: Vec<(String, PathBuf)>,
    preparations: BTreeMap<String, String>,
    worker_environment: BTreeMap<String, String>,
    herdr_session: Option<HerdrSessionName>,
    override_risk_ordering: bool,
    recovery_limits: RecoveryLimits,
    worker_override: Option<Vec<String>>,
    wait_timeout: Option<Duration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FreezeAuthority {
    Human,
    Mechanical,
}

#[derive(Debug)]
enum Command {
    DispatchContinuation,
    GateExec,
    GateReplay(GateReplayCommand),
    GateReplayWorker,
    ExecutionSubjectProbe {
        output: PathBuf,
    },
    PairedExecutionProof(PairedExecutionProofCommand),
    PackageBrief(PackageBriefCommand),
    PackageAgent(PackageAgentCommand),
    PackageGateBrief(PackageGateBriefCommand),
    PackageGateAgent(PackageGateAgentCommand),
    PackageDispatch(PackageDispatchCommand),
    PackageRender(PackageRenderCommand),
    DriverStatus(DriverStatusCommand),
    MaterializeRefs(MaterializeRefsCommand),
    DriverCriteria(DriverCriteriaCommand),
    DriverReplay(DriverReplayCommand),
    DriverOverrule(DriverOverruleCommand),
    DriverRun(DriverRunCommand),
    PackageWorker {
        result_path: PathBuf,
        required_artifact_path: PathBuf,
        worker_arguments: Vec<String>,
    },
    PackageCompletions {
        log_path: PathBuf,
        vision_dir: PathBuf,
    },
    Dispatch {
        envelope: DispatchEnvelope,
        logging: Option<DispatchLoggingMode>,
    },
    DispatchCheckIn {
        log_path: PathBuf,
    },
    DispatchReconcile {
        log_path: PathBuf,
        issuance_sequence: Sequence,
        node: NodeId,
    },
    VisionNew {
        name: VisionName,
    },
    VisionCheck,
    GraphCheck {
        path: PathBuf,
        repositories: Vec<(String, PathBuf)>,
        strict: bool,
    },
    GraphFreeze {
        vision_dir: PathBuf,
        repositories: Vec<(String, PathBuf)>,
        authority: FreezeAuthority,
        criterion_revisions: Option<PathBuf>,
        base_currency_acceptance: Option<PathBuf>,
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
    LogMeter,
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
        risk_ordering: RiskOrdering,
    },
    CriteriaCheck {
        log_path: PathBuf,
        recovery_log_path: RecoveryLogPath,
        vision_dir: PathBuf,
    },
    CompletionCheck {
        log_path: PathBuf,
        recovery_log_path: RecoveryLogPath,
        vision_dir: PathBuf,
        finished_result: FinishedResult,
    },
    LandingCheck {
        log_path: PathBuf,
        recovery_log_path: RecoveryLogPath,
        vision_dir: PathBuf,
        finished_result: FinishedResult,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchContinuationRequest {
    envelope: ContinuationEnvelope,
    completion: ContinuationCompletion,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuationEnvelope {
    target: DispatchTarget,
    arguments: Vec<String>,
    working_directory: PathBuf,
    environment: BTreeMap<String, String>,
    stdin: ContinuationStdin,
    sandbox: Option<String>,
    schema_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    gate_execution_recorder: Option<ContinuationGateRecorder>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "binding", rename_all = "kebab-case", deny_unknown_fields)]
enum ContinuationStdin {
    Null,
    PlanBytes { bytes: Vec<u8> },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuationGateRecorder {
    client_path: PathBuf,
    evidence_path: PathBuf,
    socket_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinuationCompletion {
    log_path: PathBuf,
    node: NodeId,
    issuance_sequence: Sequence,
    required_artifact_path: PathBuf,
}

#[derive(Debug)]
struct PairedExecutionProofCommand {
    repository_root: PathBuf,
    artifact_directory: PathBuf,
    environment: ChildEnvironment,
}

#[derive(Debug)]
struct GateReplayCommand {
    repository_root: PathBuf,
    recorded_root: Option<PathBuf>,
    evidence_path: PathBuf,
    execution_ref: GateExecutionRef,
    broken_ref: NamedReplayRef,
    repaired_ref: NamedReplayRef,
    schema_path: RepositoryRelativePath,
    output_path: RepositoryRelativePath,
    expected: ExpectedVerdictOutcome,
    pause_directory: Option<PathBuf>,
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

struct LiveDispatchCompletion<'a> {
    path: &'a Path,
    node: &'a NodeId,
    issuance_sequence: Sequence,
    required_artifact_path: &'a AbsoluteRequiredArtifactPath,
    continuation_process_identity: RecordedProcessIdentity,
}

enum OwnedFileObservation {
    Missing,
    Unreadable(String),
    Readable(Vec<u8>),
}

impl OwnedFileObservation {
    fn as_observation(&self) -> FileObservation<'_> {
        match self {
            Self::Missing => FileObservation::Missing,
            Self::Unreadable(detail) => FileObservation::Unreadable {
                detail: detail.as_str(),
            },
            Self::Readable(bytes) => FileObservation::Readable {
                bytes: bytes.as_slice(),
            },
        }
    }
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

    fn release_tag(&self) -> &str {
        match self {
            Self::Current(payload) => payload
                .stated
                .release_tag
                .as_deref()
                .unwrap_or(LEGACY_RELEASE_TAG),
            Self::Legacy(_) => LEGACY_RELEASE_TAG,
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

struct SynthesizedTemporaryDirectory(PathBuf);

impl SynthesizedTemporaryDirectory {
    fn create() -> Result<Self> {
        // `/tmp` is an explicit writable root in both dispatched-agent sandboxes and
        // the contract-measurement profile. Do not inherit the operator's `TMPDIR`:
        // doing so makes identical routes depend on an unforwarded shell variable.
        let parent = Path::new("/tmp");
        for attempt in 0..128_u64 {
            let path = parent.join(format!(
                "pce-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .context("system clock before epoch while creating TMPDIR")?
                    .as_nanos(),
                attempt
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error).context("failed to create binary-owned TMPDIR"),
            }
        }
        bail!("failed to allocate a unique binary-owned TMPDIR")
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for SynthesizedTemporaryDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            tracing::warn!(path = %self.0.display(), error = ?error, "failed to remove binary-owned TMPDIR");
        }
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
        Command::DispatchContinuation => run_dispatch_continuation(input),
        Command::GateExec => run_gate_exec(input),
        Command::GateReplay(command) => exec_gate_replay_worker(command),
        Command::GateReplayWorker => {
            read_gate_replay_worker_request(input).and_then(run_gate_replay)
        }
        Command::ExecutionSubjectProbe { output } => run_execution_subject_probe(&output),
        Command::PairedExecutionProof(command) => run_paired_execution_proof(command),
        Command::PackageBrief(command) => run_package_brief(command),
        Command::PackageAgent(command) => run_package_agent(command),
        Command::PackageGateBrief(command) => run_package_gate_brief(command),
        Command::PackageGateAgent(command) => run_package_gate_agent(command),
        Command::PackageDispatch(command) => run_package_dispatch(command),
        Command::PackageRender(command) => run_package_render(command),
        Command::DriverStatus(command) => run_driver_status(command),
        Command::MaterializeRefs(command) => run_materialize_refs(command),
        Command::DriverCriteria(command) => run_driver_criteria(command),
        Command::DriverReplay(command) => run_driver_replay(command),
        Command::DriverOverrule(command) => run_driver_overrule(command),
        Command::DriverRun(command) => run_driver_loop(command),
        Command::PackageWorker {
            result_path,
            required_artifact_path,
            worker_arguments,
        } => run_package_worker(&result_path, &required_artifact_path, &worker_arguments),
        Command::PackageCompletions {
            log_path,
            vision_dir,
        } => run_package_completions(&log_path, &vision_dir),
        Command::Dispatch { envelope, logging } => {
            if dispatch_invocation(&envelope).target() == DispatchTarget::Codex
                && let Some(schema) = envelope.schema_path()
                && matches!(
                    schema.as_path().file_name().and_then(|name| name.to_str()),
                    Some("graph.schema.json" | "verdict.schema.json")
                )
            {
                validate_codex_output_schema(schema.as_path())?;
            }
            match logging.as_ref() {
                Some(DispatchLoggingMode::DryRun { path, metadata }) => {
                    run_dispatch_projection(&envelope, path, metadata)
                }
                Some(DispatchLoggingMode::Live { path, metadata }) => {
                    start_logged_dispatch(&envelope, LiveDispatchLog { path, metadata })
                }
                None => execute_dispatch(&envelope, None),
            }
        }
        Command::DispatchCheckIn { log_path } => {
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            run_dispatch_check_in(&log_path, &mut output)
        }
        Command::DispatchReconcile {
            log_path,
            issuance_sequence,
            node,
        } => run_dispatch_reconcile(&log_path, issuance_sequence, node),
        Command::VisionNew { name } => run_vision_new(&name),
        Command::VisionCheck => run_vision_check(input),
        Command::GraphCheck {
            path,
            repositories,
            strict,
        } => run_graph_check(&path, &repositories, strict),
        Command::GraphFreeze {
            vision_dir,
            repositories,
            authority,
            criterion_revisions,
            base_currency_acceptance,
        } => run_graph_freeze(
            &vision_dir,
            &repositories,
            authority,
            criterion_revisions.as_deref(),
            base_currency_acceptance.as_deref(),
        ),
        Command::LogWrite { path, kind, node } => run_log(&path, kind, node, input),
        Command::LogRead { path, filter } => {
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            run_log_read(&path, &filter, &mut output)
        }
        Command::LogMeter => {
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            run_log_meter(input, &mut output)
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
            risk_ordering,
        } => run_ready(
            &log_path,
            &recovery_log_path,
            &vision_dir,
            graph_path.as_ref(),
            risk_ordering,
        ),
        Command::CriteriaCheck {
            log_path,
            recovery_log_path,
            vision_dir,
        } => run_criteria_check(&log_path, &recovery_log_path, &vision_dir, input),
        Command::CompletionCheck {
            log_path,
            recovery_log_path,
            vision_dir,
            finished_result,
        } => run_completion_check(&log_path, &recovery_log_path, &vision_dir, &finished_result),
        Command::LandingCheck {
            log_path,
            recovery_log_path,
            vision_dir,
            finished_result,
        } => run_landing_check(&log_path, &recovery_log_path, &vision_dir, &finished_result),
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
        [command] if command == "__dispatch-continuation" => Ok(Command::DispatchContinuation),
        [verb, action] if verb == "gate" && action == "exec" => Ok(Command::GateExec),
        [verb, action, rest @ ..] if verb == "gate" && action == "replay" => {
            parse_gate_replay(rest).with_context(|| USAGE)
        }
        [verb, action] if verb == "gate" && action == "replay-worker" => {
            Ok(Command::GateReplayWorker)
        }
        [verb, action, output_flag, output]
            if verb == "gate"
                && action == "execution-subject-probe"
                && output_flag == "--output" =>
        {
            Ok(Command::ExecutionSubjectProbe {
                output: parse_paired_probe_output(output)?,
            })
        }
        [verb, action, rest @ ..] if verb == "gate" && action == "paired-execution-proof" => {
            parse_paired_execution_proof(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "brief" => {
            parse_package_brief(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "agent" => {
            parse_package_agent(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "gate-brief" => {
            parse_package_gate_brief(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "gate-agent" => {
            parse_package_gate_agent(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "render" => {
            parse_package_render(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "driver-status" => {
            parse_driver_status(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "materialize-refs" => {
            parse_materialize_refs(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "criteria-run" => {
            parse_driver_criteria(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "replay-finding" => {
            parse_driver_replay(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "driver-overrule" => {
            parse_driver_overrule(rest)
        }
        [verb, action, rest @ ..] if verb == "package" && action == "driver-run" => {
            parse_driver_run(rest)
        }
        [verb, action] if verb == "vision" && action == "check" => Ok(Command::VisionCheck),
        [verb, action, raw_name] if verb == "vision" && action == "new" => {
            let name = VisionName::parse(raw_name).context("failed to parse vision name")?;
            Ok(Command::VisionNew { name })
        }
        [verb, action, rest @ ..] if verb == "graph" => parse_graph_command(action, rest),
        [verb, action, rest @ ..] if verb == "log" => parse_log_command(action, rest),
        [verb, action, rest @ ..] if verb == "status" => parse_status_command(action, rest),
        [verb, rest @ ..] if verb == "ready" => parse_ready_command(rest),
        [verb, action, rest @ ..] if verb == "criteria" => parse_criteria_command(action, rest),
        [verb, action, rest @ ..] if verb == "completion" => parse_completion_command(action, rest),
        [verb, action, rest @ ..] if verb == "landing" => parse_landing_command(action, rest),
        [verb, action, rest @ ..] if verb == "contract" => parse_contract_command(action, rest),
        [verb, action, rest @ ..] if verb == "dispatch" && action == "package-worker" => {
            parse_package_worker(rest)
        }
        [verb, action, rest @ ..] if verb == "dispatch" && action == "package-completions" => {
            parse_package_completions(rest)
        }
        [verb, action, rest @ ..] if verb == "dispatch" && action == "package" => {
            parse_package_dispatch(rest)
        }
        [verb, action, rest @ ..] if verb == "dispatch" && action == "check-in" => {
            parse_dispatch_check_in(rest)
        }
        [verb, action, rest @ ..] if verb == "dispatch" && action == "reconcile" => {
            parse_dispatch_reconcile(rest)
        }
        [verb, target, rest @ ..] if verb == "dispatch" => match target.as_str() {
            "codex" => parse_codex_dispatch(target, rest),
            "gate" => parse_gate_dispatch(rest),
            _ => bail!("unsupported dispatch target `{target}`"),
        }
        .with_context(|| USAGE),
        _ => bail!(USAGE),
    }
}

fn parse_package_gate_agent(rest: &[String]) -> Result<Command> {
    let delimiter = rest
        .iter()
        .position(|argument| argument == "--")
        .context(USAGE)?;
    let (options, worker_with_delimiter) = rest.split_at(delimiter);
    let worker = &worker_with_delimiter[1..];
    let [
        vision_flag,
        vision,
        graph_flag,
        graph,
        package_flag,
        package,
        artifact_flag,
        artifact_ref,
        outcome_flag,
        outcome,
        trailing @ ..,
    ] = options
    else {
        bail!(USAGE);
    };
    let mut defer_finding_validation = false;
    let mut issuance = 1_u64;
    let mut attempt = 1_u32;
    let mut challenges_path = None;
    let mut trailing_index = 0;
    while trailing_index < trailing.len() {
        match trailing[trailing_index].as_str() {
            "--defer-finding-validation" => {
                defer_finding_validation = true;
                trailing_index += 1;
            }
            "--issuance" | "--attempt" | "--challenges" => {
                let value = trailing.get(trailing_index + 1).context(USAGE)?;
                match trailing[trailing_index].as_str() {
                    "--issuance" => {
                        issuance = value
                            .parse()
                            .context("gate issuance must be an unsigned integer")?
                    }
                    "--attempt" => {
                        attempt = value
                            .parse()
                            .context("gate attempt must be an unsigned integer")?
                    }
                    "--challenges" => challenges_path = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
                trailing_index += 2;
            }
            _ => bail!(USAGE),
        }
    }
    if issuance == 0 || attempt == 0 {
        bail!("gate issuance and attempt must be positive");
    }
    if vision_flag != "--vision"
        || graph_flag != "--graph"
        || package_flag != "--package"
        || artifact_flag != "--artifact-ref"
        || outcome_flag != "--outcome"
        || worker.is_empty()
    {
        bail!(USAGE);
    }
    let vision_path = PathBuf::from(vision);
    let graph_path = PathBuf::from(graph);
    let outcome_path = PathBuf::from(outcome);
    if !vision_path.is_absolute() || !graph_path.is_absolute() || !outcome_path.is_absolute() {
        bail!("package gate agent vision, graph, and outcome paths must be absolute");
    }
    Ok(Command::PackageGateAgent(PackageGateAgentCommand {
        vision_path,
        graph_path,
        package_id: package.clone(),
        artifact_ref: BuiltArtifactRef::parse(artifact_ref.clone())?,
        outcome_path,
        defer_finding_validation,
        issuance,
        attempt,
        challenges_path,
        worker_arguments: worker.to_vec(),
    }))
}

fn run_package_gate_agent(command: PackageGateAgentCommand) -> Result<()> {
    let graph_bytes = fs::read(&command.graph_path).with_context(|| {
        format!(
            "failed to read package graph {}",
            command.graph_path.display()
        )
    })?;
    let graph = parse_work_package_graph(&graph_bytes).context("failed to parse package graph")?;
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == command.package_id)
        .with_context(|| format!("package {} is absent from graph", command.package_id))?;
    let vision_identity = DispatchVisionSource::parse(graph.vision().to_owned())?;
    let temporary_directory = std::env::var_os("PCE_DISPATCH_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or(standalone_package_temporary_directory(
            &vision_identity,
            package.id(),
        ));
    prepare_private_dispatch_directory(&temporary_directory)?;
    let worktrees = package
        .repositories()
        .iter()
        .enumerate()
        .map(|(index, repository)| {
            let name = format!("PCE_WORKTREE_{index}");
            let path = std::env::var_os(&name)
                .with_context(|| format!("package gate agent environment omitted {name}"))?;
            RepositoryWorktree::parse(repository.clone(), PathBuf::from(path))
                .context("failed to parse package gate worktree environment")
        })
        .collect::<Result<Vec<_>>>()?;
    let vision = fs::read_to_string(&command.vision_path)
        .with_context(|| format!("failed to read vision {}", command.vision_path.display()))?;
    let goal = VisionGoal::parse_document(&vision).context("failed to parse vision goal")?;
    let criteria =
        parse_acceptance_criteria(&vision).context("failed to parse vision acceptance criteria")?;
    let challenges = command
        .challenges_path
        .as_ref()
        .map(|path| {
            fs::read(path)
                .with_context(|| format!("failed to read gate challenges {}", path.display()))
                .and_then(|bytes| {
                    serde_json::from_slice::<Vec<PackageGateChallenge>>(&bytes)
                        .context("failed to parse gate challenges")
                })
        })
        .transpose()?
        .unwrap_or_default();
    let brief = compose_package_gate_brief(
        &goal,
        &criteria,
        &graph,
        &command.package_id,
        &worktrees,
        &command.artifact_ref,
        &challenges,
    )
    .context("failed to compose package gate brief")?;
    let (program, arguments) = command
        .worker_arguments
        .split_first()
        .context("package gate agent worker command is empty")?;
    let mut child = std::process::Command::new(program)
        .args(arguments)
        .env("PCE_PACKAGE_GATE_OUTCOME", &command.outcome_path)
        .env("TMPDIR", &temporary_directory)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn package gate agent worker `{program}`"))?;
    child
        .stdin
        .as_mut()
        .context("package gate agent worker stdin unavailable")?
        .write_all(brief.as_bytes())
        .context("failed to pipe package gate brief to worker")?;
    drop(child.stdin.take());
    let status = child
        .wait()
        .context("failed to wait for package gate agent worker")?;
    match status.code() {
        Some(0) => {}
        Some(code) => std::process::exit(code),
        None => {
            let signal = status
                .signal()
                .context("package gate agent worker has no exit code or signal")?;
            std::process::exit(128_i32.saturating_add(signal));
        }
    }
    let outcome_bytes = fs::read(&command.outcome_path).with_context(|| {
        format!(
            "successful package gate wrote no readable outcome at {}",
            command.outcome_path.display()
        )
    })?;
    let outcome = parse_package_gate_outcome(&outcome_bytes)
        .context("failed to parse package gate outcome")?;
    if !command.defer_finding_validation {
        validate_package_gate_repositories(&outcome, package.repositories())
            .context("package gate outcome exceeded package repository scope")?;
    }
    anchor_package_gate_refs(
        &outcome,
        &worktrees,
        &command.package_id,
        command.issuance,
        command.attempt,
        command.defer_finding_validation,
    )?;
    if command.defer_finding_validation {
        return Ok(());
    }
    validate_package_gate_repositories(&outcome, package.repositories())
        .context("package gate outcome exceeded package repository scope")?;
    validate_package_gate_refs(&outcome, &worktrees)
}

fn resolve_package_gate_commit_oid(worktree: &Path, reference: &str) -> Result<String> {
    let commit = format!("{reference}^{{commit}}");
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(worktree)
        .args(["rev-parse", "--verify", "--end-of-options"])
        .arg(&commit)
        .output()
        .with_context(|| format!("failed to resolve package gate ref `{reference}`"))?;
    if !output.status.success() {
        bail!(
            "package gate ref `{reference}` does not resolve to a commit in {}: {}",
            worktree.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let oid = String::from_utf8(output.stdout).context("git returned a non-UTF-8 commit id")?;
    let oid = oid.trim();
    if oid.is_empty() {
        bail!("git returned an empty commit id for package gate ref `{reference}`");
    }
    Ok(oid.to_owned())
}

fn anchor_package_gate_refs(
    outcome: &pce_core::ParsedPackageGateOutcome,
    worktrees: &[RepositoryWorktree],
    package: &str,
    issuance: u64,
    attempt: u32,
    defer_invalid: bool,
) -> Result<()> {
    let by_repository = worktrees
        .iter()
        .map(|worktree| (worktree.repository(), worktree.path()))
        .collect::<BTreeMap<_, _>>();
    for (finding_index, finding) in outcome.findings().iter().enumerate() {
        for refs in finding.repository_refs() {
            let Some(worktree) = by_repository.get(refs.repository()) else {
                if defer_invalid {
                    continue;
                }
                bail!("no assigned worktree for `{}`", refs.repository());
            };
            let witness = match resolve_package_gate_commit_oid(worktree, refs.witness_ref()) {
                Ok(oid) => oid,
                Err(_) if defer_invalid => continue,
                Err(source) => return Err(source),
            };
            let repair = match resolve_package_gate_commit_oid(worktree, refs.repair_ref()) {
                Ok(oid) => oid,
                Err(_) if defer_invalid => continue,
                Err(source) => return Err(source),
            };
            for (kind, oid) in [("witness", witness), ("repair", repair)] {
                let reference =
                    format!("refs/pce-gate/{package}/{issuance}/{attempt}/{finding_index}/{kind}");
                let update = std::process::Command::new("git")
                    .args(["-C"])
                    .arg(worktree)
                    .args(["update-ref", &reference, &oid])
                    .output()
                    .with_context(|| format!("failed to anchor package gate ref `{reference}`"))?;
                if !update.status.success() {
                    bail!(
                        "failed to anchor package gate ref `{reference}`: {}",
                        String::from_utf8_lossy(&update.stderr).trim()
                    );
                }
            }
        }
    }
    Ok(())
}

fn resolve_package_gate_commit(worktree: &Path, reference: &str) -> Result<String> {
    let oid = resolve_package_gate_commit_oid(worktree, reference)?;
    let reachable = std::process::Command::new("git")
        .args(["-C"])
        .arg(worktree)
        .args(["for-each-ref", "--contains", &oid, "--format=%(refname)"])
        .output()
        .with_context(|| {
            format!("failed to inspect reachability of package gate ref `{reference}`")
        })?;
    if !reachable.status.success() {
        bail!(
            "failed to inspect reachability of package gate ref `{reference}` in {}: {}",
            worktree.display(),
            String::from_utf8_lossy(&reachable.stderr).trim()
        );
    }
    if reachable.stdout.is_empty() {
        let from_head = std::process::Command::new("git")
            .args(["-C"])
            .arg(worktree)
            .args(["merge-base", "--is-ancestor", &oid, "HEAD"])
            .output()
            .with_context(|| format!("failed to inspect HEAD reachability of `{reference}`"))?;
        if from_head.status.code() != Some(0) {
            bail!(
                "package gate ref `{reference}` resolves but is not reachable from any ref or HEAD in {}",
                worktree.display()
            );
        }
    }
    Ok(oid.to_owned())
}

fn validate_package_gate_refs(
    outcome: &pce_core::ParsedPackageGateOutcome,
    worktrees: &[RepositoryWorktree],
) -> Result<()> {
    for finding in outcome.findings() {
        validate_package_gate_finding_refs(finding, worktrees)?;
    }
    Ok(())
}

fn validate_package_gate_finding_refs(
    finding: &pce_core::PackageGateFinding,
    worktrees: &[RepositoryWorktree],
) -> Result<()> {
    let by_repository = worktrees
        .iter()
        .map(|worktree| (worktree.repository(), worktree.path()))
        .collect::<BTreeMap<_, _>>();
    for refs in finding.repository_refs() {
        let worktree = by_repository
            .get(refs.repository())
            .with_context(|| format!("no assigned worktree for `{}`", refs.repository()))?;
        let witness = resolve_package_gate_commit(worktree, refs.witness_ref())?;
        let repair = resolve_package_gate_commit(worktree, refs.repair_ref())?;
        if witness == repair {
            bail!(
                "package gate witness ref `{}` and repair ref `{}` identify the same commit in repository `{}`",
                refs.witness_ref(),
                refs.repair_ref(),
                refs.repository()
            );
        }
        let parents = std::process::Command::new("git")
            .args(["-C"])
            .arg(worktree)
            .args(["rev-list", "--parents", "-n", "1", &repair])
            .output()
            .with_context(|| {
                format!(
                    "failed to inspect repair parents in `{}`",
                    refs.repository()
                )
            })?;
        if !parents.status.success() {
            bail!(
                "failed to inspect repair parents in repository `{}`",
                refs.repository()
            );
        }
        let parent_line =
            String::from_utf8(parents.stdout).context("git returned non-UTF-8 repair ancestry")?;
        let identities = parent_line.split_whitespace().collect::<Vec<_>>();
        if identities.as_slice() != [repair.as_str(), witness.as_str()] {
            bail!(
                "package gate repair ref `{}` must have witness ref `{}` as its sole direct parent in repository `{}`",
                refs.repair_ref(),
                refs.witness_ref(),
                refs.repository()
            );
        }
        let relationship = std::process::Command::new("git")
            .args(["-C"])
            .arg(worktree)
            .args(["merge-base", "--is-ancestor", &witness, &repair])
            .output()
            .with_context(|| {
                format!(
                    "failed to inspect package gate ancestry in {}",
                    worktree.display()
                )
            })?;
        match relationship.status.code() {
            Some(0) => {}
            Some(1) => bail!(
                "package gate repair ref `{}` does not descend from witness ref `{}` in repository `{}`",
                refs.repair_ref(),
                refs.witness_ref(),
                refs.repository()
            ),
            code => bail!(
                "git ancestry inspection failed in repository `{}` with status {code:?}: {}",
                refs.repository(),
                String::from_utf8_lossy(&relationship.stderr).trim()
            ),
        }
    }
    Ok(())
}

fn parse_package_gate_brief(rest: &[String]) -> Result<Command> {
    let [
        vision_flag,
        vision,
        graph_flag,
        graph,
        package_flag,
        package,
        artifact_flag,
        artifact_ref,
        trailing @ ..,
    ] = rest
    else {
        bail!(USAGE);
    };
    if vision_flag != "--vision"
        || graph_flag != "--graph"
        || package_flag != "--package"
        || artifact_flag != "--artifact-ref"
        || trailing.is_empty()
        || trailing.len() % 2 != 0
    {
        bail!(USAGE);
    }
    let vision_path = PathBuf::from(vision);
    let graph_path = PathBuf::from(graph);
    if !vision_path.is_absolute() || !graph_path.is_absolute() {
        bail!("package gate brief vision and graph paths must be absolute");
    }
    let mut worktrees = Vec::new();
    for pair in trailing.chunks_exact(2) {
        if pair[0] != "--worktree" {
            bail!(USAGE);
        }
        let Some((repository, path)) = pair[1].split_once('=') else {
            bail!(USAGE);
        };
        if repository.is_empty() {
            bail!("package gate worktree mapping requires NAME=ABSOLUTE_PATH");
        }
        worktrees.push((repository.to_owned(), PathBuf::from(path)));
    }
    Ok(Command::PackageGateBrief(PackageGateBriefCommand {
        vision_path,
        graph_path,
        package_id: package.clone(),
        artifact_ref: BuiltArtifactRef::parse(artifact_ref.clone())?,
        worktrees,
    }))
}

fn run_package_gate_brief(command: PackageGateBriefCommand) -> Result<()> {
    let vision = fs::read_to_string(&command.vision_path)
        .with_context(|| format!("failed to read vision {}", command.vision_path.display()))?;
    let graph_bytes = fs::read(&command.graph_path).with_context(|| {
        format!(
            "failed to read package graph {}",
            command.graph_path.display()
        )
    })?;
    let graph = parse_work_package_graph(&graph_bytes).context("failed to parse package graph")?;
    let goal = VisionGoal::parse_document(&vision).context("failed to parse vision goal")?;
    let criteria =
        parse_acceptance_criteria(&vision).context("failed to parse vision acceptance criteria")?;
    let worktrees = command
        .worktrees
        .into_iter()
        .map(|(repository, path)| RepositoryWorktree::parse(repository, path))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let brief = compose_package_gate_brief(
        &goal,
        &criteria,
        &graph,
        &command.package_id,
        &worktrees,
        &command.artifact_ref,
        &[],
    )
    .context("failed to compose package gate brief")?;
    std::io::stdout()
        .lock()
        .write_all(brief.as_bytes())
        .context("failed to write package gate brief")
}

fn parse_package_agent(rest: &[String]) -> Result<Command> {
    if rest.len() < 10
        || rest[0] != "--vision"
        || rest[2] != "--graph"
        || rest[4] != "--package"
        || rest[6] != "--outcome"
    {
        bail!(USAGE);
    }
    let delimiter = rest
        .iter()
        .position(|value| value == "--")
        .context("package agent requires -- before worker arguments")?;
    let worker = &rest[delimiter + 1..];
    if worker.is_empty() {
        bail!(USAGE);
    }
    let brief_path = match &rest[8..delimiter] {
        [] => None,
        [flag, path] if flag == "--brief" => Some(PathBuf::from(path)),
        _ => bail!(USAGE),
    };
    let vision_path = PathBuf::from(&rest[1]);
    let graph_path = PathBuf::from(&rest[3]);
    let outcome_path = PathBuf::from(&rest[7]);
    if !vision_path.is_absolute()
        || !graph_path.is_absolute()
        || !outcome_path.is_absolute()
        || brief_path.as_ref().is_some_and(|path| !path.is_absolute())
    {
        bail!("package agent vision, graph, outcome, and brief paths must be absolute");
    }
    Ok(Command::PackageAgent(PackageAgentCommand {
        vision_path,
        graph_path,
        package_id: rest[5].clone(),
        outcome_path,
        brief_path,
        worker_arguments: worker.to_vec(),
    }))
}

fn run_package_agent(command: PackageAgentCommand) -> Result<()> {
    let graph_bytes = fs::read(&command.graph_path).with_context(|| {
        format!(
            "failed to read package graph {}",
            command.graph_path.display()
        )
    })?;
    let graph = parse_work_package_graph(&graph_bytes).context("failed to parse package graph")?;
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == command.package_id)
        .with_context(|| format!("package {} is absent from graph", command.package_id))?;
    let vision_identity = DispatchVisionSource::parse(graph.vision().to_owned())?;
    let temporary_directory = std::env::var_os("PCE_DISPATCH_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or(standalone_package_temporary_directory(
            &vision_identity,
            package.id(),
        ));
    fs::create_dir_all(&temporary_directory).context("failed to create binary-owned TMPDIR")?;
    let worktrees = package
        .repositories()
        .iter()
        .enumerate()
        .map(|(index, repository)| {
            let name = format!("PCE_WORKTREE_{index}");
            let path = std::env::var_os(&name)
                .with_context(|| format!("package agent environment omitted {name}"))?;
            RepositoryWorktree::parse(repository.clone(), PathBuf::from(path))
                .context("failed to parse package worktree environment")
        })
        .collect::<Result<Vec<_>>>()?;
    let brief = if let Some(path) = &command.brief_path {
        fs::read_to_string(path)
            .with_context(|| format!("failed to read composed package brief {}", path.display()))?
    } else {
        let vision = fs::read_to_string(&command.vision_path)
            .with_context(|| format!("failed to read vision {}", command.vision_path.display()))?;
        let goal = VisionGoal::parse_document(&vision).context("failed to parse vision goal")?;
        let criteria = parse_acceptance_criteria(&vision)
            .context("failed to parse vision acceptance criteria")?;
        compose_package_worker_brief(&goal, &criteria, &graph, &command.package_id, &worktrees)
            .context("failed to compose package worker brief")?
    };
    let (program, arguments) = command
        .worker_arguments
        .split_first()
        .context("package agent worker command is empty")?;
    let mut child = std::process::Command::new(program)
        .args(arguments)
        .env("PCE_PACKAGE_OUTCOME", &command.outcome_path)
        .env("TMPDIR", &temporary_directory)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn package agent worker `{program}`"))?;
    child
        .stdin
        .as_mut()
        .context("package agent worker stdin unavailable")?
        .write_all(brief.as_bytes())
        .context("failed to pipe package brief to worker")?;
    drop(child.stdin.take());
    let status = child
        .wait()
        .context("failed to wait for package agent worker")?;
    if let Some(code) = status.code() {
        std::process::exit(code);
    }
    let signal = status
        .signal()
        .context("package agent worker has no exit code or signal")?;
    std::process::exit(128_i32.saturating_add(signal));
}

fn parse_package_brief(rest: &[String]) -> Result<Command> {
    let [
        vision_flag,
        vision,
        graph_flag,
        graph,
        package_flag,
        package,
        trailing @ ..,
    ] = rest
    else {
        bail!(USAGE);
    };
    if vision_flag != "--vision"
        || graph_flag != "--graph"
        || package_flag != "--package"
        || [vision, graph, package]
            .iter()
            .any(|value| !is_value(value))
        || trailing.is_empty()
        || trailing.len() % 2 != 0
    {
        bail!(USAGE);
    }
    let vision_path = PathBuf::from(vision);
    let graph_path = PathBuf::from(graph);
    if !vision_path.is_absolute() || !graph_path.is_absolute() {
        bail!("package brief vision and graph paths must be absolute");
    }
    let mut worktrees = Vec::new();
    for pair in trailing.chunks_exact(2) {
        if pair[0] != "--worktree" {
            bail!(USAGE);
        }
        let Some((repository, path)) = pair[1].split_once('=') else {
            bail!(USAGE);
        };
        if repository.is_empty() {
            bail!("package worktree mapping requires NAME=ABSOLUTE_PATH");
        }
        worktrees.push((repository.to_owned(), PathBuf::from(path)));
    }
    Ok(Command::PackageBrief(PackageBriefCommand {
        vision_path,
        graph_path,
        package_id: package.clone(),
        worktrees,
    }))
}

fn run_package_brief(command: PackageBriefCommand) -> Result<()> {
    let vision = fs::read_to_string(&command.vision_path)
        .with_context(|| format!("failed to read vision {}", command.vision_path.display()))?;
    let graph_bytes = fs::read(&command.graph_path).with_context(|| {
        format!(
            "failed to read package graph {}",
            command.graph_path.display()
        )
    })?;
    let graph = parse_work_package_graph(&graph_bytes).context("failed to parse package graph")?;
    let goal = VisionGoal::parse_document(&vision).context("failed to parse vision goal")?;
    let criteria =
        parse_acceptance_criteria(&vision).context("failed to parse vision acceptance criteria")?;
    let worktrees = command
        .worktrees
        .into_iter()
        .map(|(repository, path)| RepositoryWorktree::parse(repository, path))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let brief =
        compose_package_worker_brief(&goal, &criteria, &graph, &command.package_id, &worktrees)
            .context("failed to compose package worker brief")?;
    std::io::stdout()
        .lock()
        .write_all(brief.as_bytes())
        .context("failed to write package worker brief")
}

fn parse_package_dispatch(rest: &[String]) -> Result<Command> {
    let [
        file_flag,
        raw_log,
        vision_flag,
        raw_vision,
        graph_flag,
        raw_graph,
        package_flag,
        package_id,
        artifact_flag,
        raw_artifact,
        trailing @ ..,
    ] = rest
    else {
        bail!(USAGE);
    };
    if file_flag != "--file"
        || vision_flag != "--vision-dir"
        || graph_flag != "--graph"
        || package_flag != "--package"
        || artifact_flag != "--required-artifact"
        || [raw_log, raw_vision, raw_graph, package_id, raw_artifact]
            .iter()
            .any(|value| !is_value(value))
    {
        bail!(USAGE);
    }
    let log_path = PathBuf::from(raw_log);
    let vision_dir = PathBuf::from(raw_vision);
    let graph_path = PathBuf::from(raw_graph);
    if !log_path.is_absolute() || !vision_dir.is_absolute() || !graph_path.is_absolute() {
        bail!("package dispatch log, vision, and graph paths must be absolute");
    }
    let required_artifact_path = AbsoluteRequiredArtifactPath::parse(raw_artifact)
        .context("failed to parse package required artifact path")?;
    let mut repositories = Vec::new();
    let mut environment = BTreeMap::new();
    let mut herdr_session = None;
    let mut index = 0;
    while index < trailing.len() && trailing[index] != "--" {
        if index + 1 >= trailing.len() {
            bail!(USAGE);
        }
        match trailing[index].as_str() {
            "--repository" => {
                let Some((name, root)) = trailing[index + 1].split_once('=') else {
                    bail!(USAGE);
                };
                let root = PathBuf::from(root);
                if name.is_empty() || !root.is_absolute() {
                    bail!("package repository mappings require NAME=ABSOLUTE_ROOT");
                }
                repositories.push((name.to_owned(), root));
            }
            "--env" => {
                let Some((name, value)) = trailing[index + 1].split_once('=') else {
                    bail!(USAGE);
                };
                if environment
                    .insert(name.to_owned(), value.to_owned())
                    .is_some()
                {
                    bail!("package dispatch environment names must be unique");
                }
            }
            "--herdr-session" => {
                if herdr_session.is_some() {
                    bail!("package dispatch Herdr session is repeated");
                }
                herdr_session = Some(parse_herdr_session_name(trailing[index + 1].clone())?);
            }
            _ => bail!(USAGE),
        }
        index += 2;
    }
    if repositories.is_empty() || index >= trailing.len() || trailing[index] != "--" {
        bail!(USAGE);
    }
    let worker_arguments = trailing[index + 1..].to_vec();
    if worker_arguments.is_empty() {
        bail!(USAGE);
    }
    Ok(Command::PackageDispatch(PackageDispatchCommand {
        log_path,
        vision_dir,
        graph_path,
        require_graph_at_vision_root: true,
        package_id: package_id.clone(),
        attempt: None,
        required_artifact_path,
        repositories,
        base_refs: BTreeMap::new(),
        conflicted_joins: BTreeMap::new(),
        herdr_session,
        environment,
        worker_arguments,
    }))
}

fn parse_package_worker(rest: &[String]) -> Result<Command> {
    let [
        result_flag,
        result,
        artifact_flag,
        artifact,
        delimiter,
        worker @ ..,
    ] = rest
    else {
        bail!(USAGE);
    };
    if result_flag != "--result"
        || artifact_flag != "--required-artifact"
        || delimiter != "--"
        || worker.is_empty()
    {
        bail!(USAGE);
    }
    let result_path = PathBuf::from(result);
    let required_artifact_path = PathBuf::from(artifact);
    if !result_path.is_absolute() || !required_artifact_path.is_absolute() {
        bail!("package worker paths must be absolute");
    }
    Ok(Command::PackageWorker {
        result_path,
        required_artifact_path,
        worker_arguments: worker.to_vec(),
    })
}

fn parse_package_completions(rest: &[String]) -> Result<Command> {
    let [file_flag, log, vision_flag, vision] = rest else {
        bail!(USAGE);
    };
    if file_flag != "--file" || vision_flag != "--vision-dir" {
        bail!(USAGE);
    }
    let log_path = PathBuf::from(log);
    let vision_dir = PathBuf::from(vision);
    if !log_path.is_absolute() || !vision_dir.is_absolute() {
        bail!("package completion paths must be absolute");
    }
    Ok(Command::PackageCompletions {
        log_path,
        vision_dir,
    })
}

fn parse_package_render(rest: &[String]) -> Result<Command> {
    let (graph, journal, output) = match rest {
        [graph_flag, graph, output_flag, output]
            if graph_flag == "--graph" && output_flag == "--output" =>
        {
            (graph, None, output)
        }
        [
            graph_flag,
            graph,
            journal_flag,
            journal,
            output_flag,
            output,
        ] if graph_flag == "--graph"
            && journal_flag == "--journal"
            && output_flag == "--output" =>
        {
            (graph, Some(PathBuf::from(journal)), output)
        }
        _ => bail!(USAGE),
    };
    Ok(Command::PackageRender(PackageRenderCommand {
        graph_path: PathBuf::from(graph),
        journal_path: journal,
        output_path: PathBuf::from(output),
    }))
}

fn parse_driver_status(rest: &[String]) -> Result<Command> {
    let override_risk_ordering = rest
        .last()
        .is_some_and(|value| value == "--override-risk-ordering");
    let values = if override_risk_ordering {
        &rest[..rest.len() - 1]
    } else {
        rest
    };
    let [graph_flag, graph, journal_flag, journal] = values else {
        bail!(USAGE);
    };
    if graph_flag != "--graph" || journal_flag != "--journal" {
        bail!(USAGE);
    }
    Ok(Command::DriverStatus(DriverStatusCommand {
        graph_path: PathBuf::from(graph),
        journal_path: PathBuf::from(journal),
        override_risk_ordering,
    }))
}

fn parse_materialize_refs(rest: &[String]) -> Result<Command> {
    let [graph_flag, graph, journal_flag, journal, trailing @ ..] = rest else {
        bail!(USAGE);
    };
    if graph_flag != "--graph" || journal_flag != "--journal" {
        bail!(USAGE);
    }
    let (repositories, preparations) = parse_driver_repository_options(trailing)?;
    if !preparations.is_empty() {
        bail!("ref materialization accepts repository mappings only");
    }
    Ok(Command::MaterializeRefs(MaterializeRefsCommand {
        graph_path: PathBuf::from(graph),
        journal_path: PathBuf::from(journal),
        repositories,
    }))
}

fn parse_driver_repository_options(
    rest: &[String],
) -> Result<(Vec<(String, PathBuf)>, BTreeMap<String, String>)> {
    if rest.is_empty() || !rest.len().is_multiple_of(2) {
        bail!(USAGE);
    }
    let mut repositories = Vec::new();
    let mut preparations = BTreeMap::new();
    for pair in rest.chunks_exact(2) {
        let (name, value) = pair[1]
            .split_once('=')
            .context("driver repository option requires NAME=VALUE")?;
        if name.is_empty() || value.is_empty() {
            bail!("driver repository option name and value must be non-empty");
        }
        match pair[0].as_str() {
            "--repository" => repositories.push((name.to_owned(), PathBuf::from(value))),
            "--prepare" => {
                if preparations
                    .insert(name.to_owned(), value.to_owned())
                    .is_some()
                {
                    bail!("preparation command for repository `{name}` is repeated");
                }
            }
            _ => bail!(USAGE),
        }
    }
    if repositories.is_empty() {
        bail!(USAGE);
    }
    let repository_names = repositories
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<std::collections::HashSet<_>>();
    for name in preparations.keys() {
        if !repository_names.contains(name.as_str()) {
            bail!("preparation command names unmapped repository `{name}`");
        }
    }
    Ok((repositories, preparations))
}

fn parse_driver_criteria(rest: &[String]) -> Result<Command> {
    let [
        graph_flag,
        graph,
        journal_flag,
        journal,
        package_flag,
        package,
        trailing @ ..,
    ] = rest
    else {
        bail!(USAGE);
    };
    if graph_flag != "--graph" || journal_flag != "--journal" || package_flag != "--package" {
        bail!(USAGE);
    }
    let (repositories, preparations) = parse_driver_repository_options(trailing)?;
    Ok(Command::DriverCriteria(DriverCriteriaCommand {
        graph_path: PathBuf::from(graph),
        journal_path: PathBuf::from(journal),
        package_id: package.clone(),
        repositories,
        preparations,
    }))
}

fn parse_driver_replay(rest: &[String]) -> Result<Command> {
    let [
        graph_flag,
        graph,
        journal_flag,
        journal,
        package_flag,
        package,
        gate_flag,
        gate,
        finding_flag,
        finding,
        outcome_flag,
        outcome,
        trailing @ ..,
    ] = rest
    else {
        bail!(USAGE);
    };
    if graph_flag != "--graph"
        || journal_flag != "--journal"
        || package_flag != "--package"
        || gate_flag != "--gate"
        || finding_flag != "--finding"
        || outcome_flag != "--outcome"
    {
        bail!(USAGE);
    }
    let (repositories, preparations) = parse_driver_repository_options(trailing)?;
    Ok(Command::DriverReplay(DriverReplayCommand {
        graph_path: PathBuf::from(graph),
        journal_path: PathBuf::from(journal),
        package_id: package.clone(),
        gate: gate.clone(),
        finding: finding
            .parse()
            .context("finding index must be an unsigned integer")?,
        outcome_path: PathBuf::from(outcome),
        repositories,
        preparations,
    }))
}

fn parse_driver_overrule(rest: &[String]) -> Result<Command> {
    let [
        graph_flag,
        graph,
        journal_flag,
        journal,
        package_flag,
        package,
        rationale_flag,
        rationale,
    ] = rest
    else {
        bail!(USAGE);
    };
    if graph_flag != "--graph"
        || journal_flag != "--journal"
        || package_flag != "--package"
        || rationale_flag != "--rationale"
    {
        bail!(USAGE);
    }
    if package.trim().is_empty() {
        bail!("driver overrule package must be non-empty");
    }
    if rationale.trim().is_empty() {
        bail!("driver overrule rationale must be non-empty");
    }
    Ok(Command::DriverOverrule(DriverOverruleCommand {
        graph_path: PathBuf::from(graph),
        journal_path: PathBuf::from(journal),
        package_id: package.clone(),
        rationale: rationale.clone(),
    }))
}

fn parse_driver_run(rest: &[String]) -> Result<Command> {
    let delimiter = rest.iter().position(|value| value == "--");
    let (options, worker_override) = match delimiter {
        Some(index) if index > 0 && rest[index - 1] == "--worker-override" => {
            let worker = rest[index + 1..].to_vec();
            if worker.is_empty() {
                bail!("driver run worker override is empty");
            }
            (&rest[..index - 1], Some(worker))
        }
        Some(_) => bail!("driver worker override requires --worker-override -- <ARGV>"),
        None => (rest, None),
    };
    if options.len() < 6 || options[0] != "--graph" || options[2] != "--journal" {
        bail!(USAGE);
    }
    let mut override_risk_ordering = false;
    let mut retry_limit = 1_u32;
    let mut local_patch_limit = 1_u32;
    let mut environment_failure_limit = 6_u32;
    let mut gate_failure_limit = 3_u32;
    let mut wait_timeout = None;
    let mut worker_environment = BTreeMap::new();
    let mut herdr_session = None;
    let mut mapping_args = Vec::new();
    let mut index = 4;
    while index < options.len() {
        if options[index] == "--override-risk-ordering" {
            override_risk_ordering = true;
            index += 1;
        } else if options[index] == "--retry-limit"
            || options[index] == "--local-patch-limit"
            || options[index] == "--environment-failure-limit"
            || options[index] == "--gate-failure-limit"
        {
            let value = options
                .get(index + 1)
                .context("driver limit requires a value")?
                .parse::<u32>()
                .context("driver limit must be an unsigned integer")?;
            if options[index] == "--retry-limit" {
                retry_limit = value;
            } else if options[index] == "--local-patch-limit" {
                local_patch_limit = value;
            } else if options[index] == "--environment-failure-limit" {
                environment_failure_limit = value;
            } else {
                gate_failure_limit = value;
            }
            index += 2;
        } else if options[index] == "--worker-env" {
            let name = options
                .get(index + 1)
                .context("--worker-env requires an environment name")?;
            validate_worker_environment_name(name)?;
            let value = std::env::var(name).with_context(|| {
                format!("declared worker environment variable `{name}` is unset or not Unicode")
            })?;
            if worker_environment.insert(name.clone(), value).is_some() {
                bail!("worker environment name `{name}` is repeated");
            }
            index += 2;
        } else if options[index] == "--herdr-session" {
            if herdr_session.is_some() {
                bail!("driver Herdr session is repeated");
            }
            let value = options
                .get(index + 1)
                .context("--herdr-session requires a session name")?;
            herdr_session = Some(parse_herdr_session_name(value.clone())?);
            index += 2;
        } else if options[index] == "--wait-timeout-ms" {
            let value = options
                .get(index + 1)
                .context("wait timeout requires a value")?
                .parse::<u64>()
                .context("wait timeout must be an unsigned integer")?;
            wait_timeout = Some(Duration::from_millis(value));
            index += 2;
        } else {
            if index + 1 >= options.len() {
                bail!(USAGE);
            }
            mapping_args.extend_from_slice(&options[index..index + 2]);
            index += 2;
        }
    }
    let (repositories, preparations) = parse_driver_repository_options(&mapping_args)?;
    Ok(Command::DriverRun(DriverRunCommand {
        graph_path: PathBuf::from(&options[1]),
        journal_path: PathBuf::from(&options[3]),
        repositories,
        preparations,
        worker_environment,
        herdr_session,
        override_risk_ordering,
        recovery_limits: RecoveryLimits::new(
            RetryLimit::new(retry_limit),
            LocalPatchLimit::new(local_patch_limit),
        )
        .with_environment_failure_limit(EnvironmentFailureLimit::new(environment_failure_limit))
        .with_gate_failure_limit(GateFailureLimit::new(gate_failure_limit)),
        worker_override,
        wait_timeout,
    }))
}

fn driver_outcome_path(journal: &Path, package: &str, issuance: u64) -> Result<PathBuf> {
    let parent = journal.parent().context("driver journal has no parent")?;
    Ok(parent
        .join("package-outcomes")
        .join(package)
        .join(format!("{issuance}.json")))
}

fn ensure_worker_environment_contract(
    command: &DriverRunCommand,
    events: &[DriverEvent],
) -> Result<()> {
    let names = command
        .worker_environment
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut configured = None::<BTreeSet<String>>;
    for event in events {
        match event {
            DriverEvent::WorkerEnvironmentDeclared { names } => {
                configured = Some(names.iter().cloned().collect());
            }
            DriverEvent::WorkerEnvironmentExtended { added_names, .. } => {
                configured
                    .get_or_insert_with(BTreeSet::new)
                    .extend(added_names.iter().cloned());
            }
            _ => {}
        }
    }
    let Some(configured) = configured else {
        return append_driver_event(
            &command.journal_path,
            &DriverEvent::WorkerEnvironmentDeclared {
                names: names.into_iter().collect(),
            },
        );
    };
    if configured == names {
        return Ok(());
    }
    if !configured.is_subset(&names) {
        bail!(
            "driver worker environment is already declared as {:?}, not {:?}; cannot remove declared names because the contract is additions-only",
            configured,
            names
        );
    }
    let boundary_plan_version = events
        .iter()
        .rposition(|event| matches!(event, DriverEvent::PlanVersionAdvanced { .. }))
        .and_then(|index| {
            let plan_version = match &events[index] {
                DriverEvent::PlanVersionAdvanced {
                    to_plan_version, ..
                } => *to_plan_version,
                _ => return None,
            };
            events[index + 1..]
                .iter()
                .all(|event| {
                    matches!(
                        event,
                        DriverEvent::DriverAborted { .. }
                            | DriverEvent::DriverResumed
                            | DriverEvent::WorkerEnvironmentDeclared { .. }
                            | DriverEvent::WorkerEnvironmentExtended { .. }
                            | DriverEvent::RecoveryConfigured { .. }
                    )
                })
                .then_some(plan_version)
        });
    let Some(plan_version) = boundary_plan_version else {
        bail!(
            "driver worker environment additions are refused mid-plan; extend at the next plan-version boundary (current {:?}, supplied {:?})",
            configured,
            names
        );
    };
    let added_names = names.difference(&configured).cloned().collect();
    append_driver_event(
        &command.journal_path,
        &DriverEvent::WorkerEnvironmentExtended {
            plan_version,
            added_names,
        },
    )
}

fn ensure_recovery_configuration(command: &DriverRunCommand, events: &[DriverEvent]) -> Result<()> {
    let configured = events.iter().find_map(|event| match event {
        DriverEvent::RecoveryConfigured { limits } => Some(*limits),
        _ => None,
    });
    match configured {
        Some(limits) if limits != command.recovery_limits => bail!(
            "driver limits are already retry={} local-patch={} environment-failure={} gate-failure={}, not retry={} local-patch={} environment-failure={} gate-failure={}",
            limits.retry_attempts(),
            limits.local_patch_attempts(),
            limits.environment_failures(),
            limits.gate_failures(),
            command.recovery_limits.retry_attempts(),
            command.recovery_limits.local_patch_attempts(),
            command.recovery_limits.environment_failures(),
            command.recovery_limits.gate_failures()
        ),
        Some(_) => Ok(()),
        None => append_driver_event(
            &command.journal_path,
            &DriverEvent::RecoveryConfigured {
                limits: command.recovery_limits,
            },
        ),
    }
}

fn append_worker_environment_outcome(
    command: &DriverRunCommand,
    package: String,
    issuance: u64,
    reason: String,
) -> Result<()> {
    let events = read_driver_journal(&command.journal_path)?;
    let event =
        worker_environment_outcome(&events, command.recovery_limits, package, issuance, reason);
    append_driver_event(&command.journal_path, &event)
}

fn park_if_recovery_exhausted(
    command: &DriverRunCommand,
    package_id: &str,
    blocked_by: Option<&str>,
) -> Result<()> {
    let events = read_driver_journal(&command.journal_path)?;
    let charged = charged_failure_count(&events, package_id);
    let budget = recovery_budget(command.recovery_limits, charged);
    if budget.next_rung != RecoveryRung::Replan {
        return Ok(());
    }
    let evidence = latest_criterion_failure_evidence(&events, package_id);
    let attempts = recovery_attempt_records(&events, package_id, evidence);
    append_driver_event(
        &command.journal_path,
        &DriverEvent::RecoveryParked {
            package: package_id.to_owned(),
            reason: format!(
                "recovery spending exhausted after {charged} attributable failures; re-author as plan version n+1"
            ),
            blocked_by: blocked_by.map(str::to_owned),
            attempts,
        },
    )
}

fn park_if_worker_blocker_repeated(command: &DriverRunCommand, package_id: &str) -> Result<bool> {
    let events = read_driver_journal(&command.journal_path)?;
    let Some(blocked_by) =
        repeated_identical_worker_blocker(&events, package_id).map(str::to_owned)
    else {
        return Ok(false);
    };
    let evidence = latest_criterion_failure_evidence(&events, package_id);
    let attempts = recovery_attempt_records(&events, package_id, evidence);
    append_driver_event(
        &command.journal_path,
        &DriverEvent::RecoveryParked {
            package: package_id.to_owned(),
            reason: "repeated identical worker blocker; package work cannot resolve it".to_owned(),
            blocked_by: Some(blocked_by),
            attempts,
        },
    )?;
    Ok(true)
}

const GATE_STOPPED_REASON: &str = "gate dispatch stopped without an outcome";
const GATE_INCOMPLETE_REASON: &str = "gate outcome incomplete";

fn append_gate_failure_outcome(
    command: &DriverRunCommand,
    package: &str,
    issuance: u64,
    gate: String,
    reason: &'static str,
    detail: String,
    challenges: Vec<PackageGateChallenge>,
) -> Result<()> {
    let events = read_driver_journal(&command.journal_path)?;
    let event = gate_failure_outcome(
        &events,
        command.recovery_limits,
        package.to_owned(),
        issuance,
        gate,
        reason.to_owned(),
        detail,
        challenges,
    );
    append_driver_event(&command.journal_path, &event)
}

fn run_composed_driver_gate(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: &str,
    issuance: u64,
) -> Result<()> {
    let events = read_driver_journal(&command.journal_path)?;
    let attempt = next_gate_attempt(&events, package_id, issuance);
    let gate_name = format!("package-gate-{issuance}-{attempt}");
    let challenges = pending_gate_challenges(&events, package_id, issuance);
    let implementation_worktrees = driver_package_worktrees(command, graph, package_id, issuance)?;
    let package_ref = package_branch(graph, package_id, issuance);
    let gate_base_refs = implementation_worktrees
        .iter()
        .map(|worktree| {
            let source = command
                .repositories
                .iter()
                .find(|(repository, _)| repository == worktree.repository())
                .map(|(_, path)| path)
                .with_context(|| {
                    format!(
                        "missing source repository for gate worktree `{}`",
                        worktree.repository()
                    )
                })?;
            Ok((
                worktree.repository().to_owned(),
                git_oid(source, &package_ref)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let first = implementation_worktrees
        .first()
        .context("package gate has no implementation worktree")?;
    let artifact_ref = gate_base_refs
        .get(first.repository())
        .cloned()
        .context("package gate has no artifact ref for its first repository")?;
    let vision_dir = driver_vision_directory(command)?;
    let gate_outcome = vision_dir
        .join(".pce/package-gate-outcomes")
        .join(package_id)
        .join(issuance.to_string())
        .join(format!("{attempt}.json"));
    fs::create_dir_all(
        gate_outcome
            .parent()
            .context("gate outcome has no parent")?,
    )?;

    let mut dispatch_graph: Value = serde_json::from_slice(
        &fs::read(&command.graph_path).context("failed to read gate dispatch graph")?,
    )
    .context("failed to parse gate dispatch graph JSON")?;
    dispatch_graph["vision"] =
        Value::String(format!("{}-gate-{issuance}-{attempt}", graph.vision()));
    if dispatch_graph.get("authored_at_refs").is_some() {
        dispatch_graph["authored_at_refs"] = serde_json::to_value(
            graph
                .authored_refs()
                .keys()
                .map(|repository| (repository.clone(), "HEAD".to_owned()))
                .collect::<BTreeMap<_, _>>(),
        )?;
    } else {
        dispatch_graph["authored_at_ref"] = Value::String("HEAD".to_owned());
    }
    let dispatch_graph_path = vision_dir
        .join(".pce/gate-dispatch-graphs")
        .join(package_id)
        .join(issuance.to_string())
        .join(format!("{attempt}.json"));
    fs::create_dir_all(
        dispatch_graph_path
            .parent()
            .context("gate dispatch graph has no parent")?,
    )?;
    fs::write(&dispatch_graph_path, serde_json::to_vec(&dispatch_graph)?)?;
    let challenges_path = vision_dir
        .join(".pce/package-gate-challenges")
        .join(package_id)
        .join(issuance.to_string())
        .join(format!("{attempt}.json"));
    fs::create_dir_all(
        challenges_path
            .parent()
            .context("gate challenges have no parent")?,
    )?;
    fs::write(&challenges_path, serde_json::to_vec(&challenges)?)?;

    let executable = std::env::current_exe().context("failed to resolve gate driver executable")?;
    let worker_arguments = vec![
        executable.display().to_string(),
        "package".to_owned(),
        "gate-agent".to_owned(),
        "--vision".to_owned(),
        vision_dir.join("vision.md").display().to_string(),
        "--graph".to_owned(),
        absolute_path(&command.graph_path)?.display().to_string(),
        "--package".to_owned(),
        package_id.to_owned(),
        "--artifact-ref".to_owned(),
        artifact_ref,
        "--outcome".to_owned(),
        gate_outcome.display().to_string(),
        "--issuance".to_owned(),
        issuance.to_string(),
        "--attempt".to_owned(),
        attempt.to_string(),
        "--challenges".to_owned(),
        challenges_path.display().to_string(),
        "--defer-finding-validation".to_owned(),
        "--".to_owned(),
        "prime-agent".to_owned(),
        "-p".to_owned(),
    ];
    append_driver_event(
        &command.journal_path,
        &DriverEvent::GateDispatched {
            package: package_id.to_owned(),
            issuance,
            attempt,
            gate: gate_name.clone(),
        },
    )?;
    let response = issue_package_dispatch(PackageDispatchCommand {
        log_path: driver_dispatch_log(command)?,
        vision_dir: vision_dir.clone(),
        graph_path: dispatch_graph_path,
        require_graph_at_vision_root: false,
        package_id: package_id.to_owned(),
        attempt: None,
        required_artifact_path: AbsoluteRequiredArtifactPath::parse(gate_outcome.clone())?,
        repositories: command
            .repositories
            .iter()
            .filter(|(name, _)| {
                implementation_worktrees
                    .iter()
                    .any(|worktree| worktree.repository() == name)
            })
            .cloned()
            .collect(),
        base_refs: gate_base_refs,
        conflicted_joins: BTreeMap::new(),
        herdr_session: command.herdr_session.clone(),
        environment: route_environment(command)?,
        worker_arguments,
    })?;
    record_driver_dispatch_panes(&command.journal_path, package_id, issuance, &response)?;
    let result_path = PathBuf::from(
        response["result_path"]
            .as_str()
            .context("gate dispatch omitted result path")?,
    );
    wait_for_driver_results(std::slice::from_ref(&result_path), None)?;
    let _completions = collect_package_completions(&driver_dispatch_log(command)?, &vision_dir)?;
    let result = read_package_result(&result_path)?
        .context("gate dispatch notification had no durable result")?;
    let healthy = matches!(result.exit_status(), DispatchExitStatus::Exited { code } if code.get() == 0)
        && result.required_artifact_presence() == RequiredArtifactPresence::Present;
    if !healthy {
        append_gate_failure_outcome(
            command,
            package_id,
            issuance,
            gate_name,
            GATE_STOPPED_REASON,
            format!("gate dispatch exit status: {:?}", result.exit_status()),
            challenges,
        )?;
        return Ok(());
    }
    let outcome = parse_package_gate_outcome(&fs::read(&gate_outcome)?)
        .context("failed to parse composed gate outcome")?;
    let mut rejected_challenges = Vec::new();
    for finding in 0..outcome.findings().len() {
        let disposition = replay_driver_finding(
            DriverReplayCommand {
                graph_path: command.graph_path.clone(),
                journal_path: command.journal_path.clone(),
                package_id: package_id.to_owned(),
                gate: gate_name.clone(),
                finding,
                outcome_path: gate_outcome.clone(),
                repositories: implementation_worktrees
                    .iter()
                    .map(|worktree| {
                        (
                            worktree.repository().to_owned(),
                            worktree.path().to_path_buf(),
                        )
                    })
                    .collect(),
                preparations: command.preparations.clone(),
            },
            false,
        )?;
        if matches!(disposition, DriverFindingDisposition::StructurallyMalformed) {
            let finding = &outcome.findings()[finding];
            rejected_challenges.push(PackageGateChallenge::from_finding(finding));
        }
    }
    append_driver_event(
        &command.journal_path,
        &DriverEvent::GateFinished {
            package: package_id.to_owned(),
            gate: gate_name.clone(),
        },
    )?;
    let reproof_challenges = outcome
        .findings()
        .iter()
        .map(PackageGateChallenge::from_finding)
        .collect();
    if matches!(
        finalize_gate_repairs(
            graph,
            command,
            package_id,
            issuance,
            &gate_name,
            reproof_challenges,
        )?,
        GateReproofOutcome::Rejected
    ) {
        return Ok(());
    }
    if !rejected_challenges.is_empty() {
        append_gate_failure_outcome(
            command,
            package_id,
            issuance,
            gate_name,
            GATE_INCOMPLETE_REASON,
            format!(
                "gate outcome contained {} structurally rejected findings",
                rejected_challenges.len()
            ),
            rejected_challenges,
        )?;
        return Ok(());
    }
    append_driver_event(
        &command.journal_path,
        &DriverEvent::PackageCompleted {
            package: package_id.to_owned(),
        },
    )?;
    remove_clean_gate_worktrees(&response, &implementation_worktrees)?;
    remove_clean_driver_worktrees(command, graph, package_id, issuance)
}

fn run_join_parent_criteria(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: &str,
    issuance: u64,
) -> Result<bool> {
    let events = read_driver_journal(&command.journal_path)?;
    let conflicted_repositories = events
        .iter()
        .filter_map(|event| match event {
            DriverEvent::PackageJoinConflicted {
                package,
                repository,
                ..
            } if package == package_id => Some(repository.as_str()),
            _ => None,
        })
        .collect::<std::collections::HashSet<_>>();
    if conflicted_repositories.is_empty() {
        return Ok(true);
    }
    let dependent = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .with_context(|| format!("package {package_id} is absent from graph"))?;
    let worktrees = driver_package_worktrees(command, graph, package_id, issuance)?;
    let mut failed = Vec::new();
    for dependency in dependent
        .depends_on()
        .iter()
        .filter(|dependency| dependency.kind().is_binding())
    {
        let parent = graph
            .packages()
            .iter()
            .find(|candidate| candidate.id() == dependency.id())
            .with_context(|| format!("dependency {} is absent", dependency.id().as_str()))?;
        if !parent
            .repositories()
            .iter()
            .any(|repository| conflicted_repositories.contains(repository.as_str()))
        {
            continue;
        }
        let sources = worktrees
            .iter()
            .filter(|worktree| {
                parent
                    .repositories()
                    .contains(&worktree.repository().to_owned())
            })
            .map(|worktree| {
                (
                    worktree.repository().to_owned(),
                    worktree.path().to_path_buf(),
                )
            })
            .collect::<Vec<_>>();
        let refs = sources
            .iter()
            .map(|(repository, path)| Ok((repository.clone(), git_oid(path, "HEAD")?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let materialization_label =
            format!("join-{package_id}-{}-{issuance}", parent.id().as_str());
        let materialization = materialize_driver_state(
            &command.journal_path,
            &materialization_label,
            &sources,
            &refs,
        )?;
        if !prepare_driver_materialization(
            &command.journal_path,
            package_id,
            &materialization_label,
            &materialization,
            &sources,
            &command.preparations,
        )? {
            return Ok(false);
        }
        let paths = materialization.paths()?;
        let named_paths = materialization
            .named_paths(&sources)
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        for criterion in effective_criteria(graph, parent.id().as_str(), &events)? {
            let (execution, amendment_proof, passed) =
                execute_effective_criterion(&criterion, &paths, &named_paths)?;
            if !passed {
                failed.push(format!("{}:{}", parent.id().as_str(), criterion.name));
            }
            append_driver_event(
                &command.journal_path,
                &DriverEvent::JoinCriterionExecuted {
                    package: package_id.to_owned(),
                    parent: parent.id().as_str().to_owned(),
                    name: criterion.name,
                    origin: criterion.origin,
                    execution,
                    amendment_proof,
                },
            )?;
        }
    }
    if failed.is_empty() {
        Ok(true)
    } else {
        append_driver_event(
            &command.journal_path,
            &DriverEvent::PackageFailed {
                package: package_id.to_owned(),
                reason: format!(
                    "conflicted join broke parent criteria: {}",
                    failed.join(", ")
                ),
            },
        )?;
        Ok(false)
    }
}

fn observe_driver_worker_outcome(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: String,
    issuance: u64,
    outcome_path: &Path,
) -> Result<bool> {
    let outcome = match fs::read(outcome_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read {}", outcome_path.display()));
        }
    };
    match pce_core::parse_package_outcome(&outcome).context("worker outcome is invalid")? {
        pce_core::PackageOutcome::Done => {
            append_driver_event(
                &command.journal_path,
                &DriverEvent::WorkerDone {
                    package: package_id.clone(),
                    issuance,
                },
            )?;
            let mut repositories = command.repositories.clone();
            if command.worker_override.is_none() {
                for worktree in driver_package_worktrees(command, graph, &package_id, issuance)? {
                    if let Some((_, path)) = repositories
                        .iter_mut()
                        .find(|(name, _)| name == worktree.repository())
                    {
                        *path = worktree.path().to_path_buf();
                    }
                }
            }
            if !run_join_parent_criteria(graph, command, &package_id, issuance)? {
                park_if_recovery_exhausted(command, &package_id, None)?;
                return Ok(true);
            }
            let _ = execute_driver_criteria(DriverCriteriaCommand {
                graph_path: command.graph_path.clone(),
                journal_path: command.journal_path.clone(),
                package_id: package_id.clone(),
                repositories,
                preparations: command.preparations.clone(),
            })?;
            park_if_recovery_exhausted(command, &package_id, None)?;
            let refreshed = read_driver_journal(&command.journal_path)?;
            let state = derive_driver_snapshot(graph, &refreshed, command.override_risk_ordering)?;
            if state.packages().iter().any(|(name, package_state)| {
                name == &package_id
                    && matches!(package_state, pce_core::DriverPackageState::Judging { .. })
            }) {
                if command.worker_override.is_some() {
                    append_driver_event(
                        &command.journal_path,
                        &DriverEvent::GateFinished {
                            package: package_id.clone(),
                            gate: "worker-override-no-findings".to_owned(),
                        },
                    )?;
                    append_driver_event(
                        &command.journal_path,
                        &DriverEvent::PackageCompleted {
                            package: package_id,
                        },
                    )?;
                } else {
                    run_composed_driver_gate(graph, command, &package_id, issuance)?;
                }
            }
        }
        pce_core::PackageOutcome::Failed { blocked_by } => {
            append_driver_event(
                &command.journal_path,
                &DriverEvent::WorkerFailed {
                    package: package_id.clone(),
                    issuance,
                    reason: blocked_by.as_str().to_owned(),
                },
            )?;
            if !park_if_worker_blocker_repeated(command, &package_id)? {
                park_if_recovery_exhausted(command, &package_id, Some(blocked_by.as_str()))?;
            }
        }
        pce_core::PackageOutcome::MisSpecified { fault } => {
            let reason = match fault {
                pce_core::MisSpecificationFault::Criterion { name } => {
                    format!("replan: criterion: {}", name.as_str())
                }
                pce_core::MisSpecificationFault::MissingDependency { id } => {
                    let checked = id
                        .checked()
                        .iter()
                        .map(|item| item.as_str())
                        .collect::<Vec<_>>();
                    let checked = serde_json::to_string(&checked)
                        .context("failed to serialize missing-dependency checks")?;
                    format!(
                        "replan: missing dependency: {}; checked: {checked}; command: {}",
                        id.as_str(),
                        id.command()
                    )
                }
            };
            append_driver_event(
                &command.journal_path,
                &DriverEvent::PackageParked {
                    package: package_id,
                    issuance,
                    reason,
                },
            )?;
        }
    }
    Ok(true)
}

fn driver_vision_directory(command: &DriverRunCommand) -> Result<PathBuf> {
    absolute_path(
        command
            .graph_path
            .parent()
            .context("driver graph has no parent directory")?,
    )
}

fn driver_dispatch_log(command: &DriverRunCommand) -> Result<PathBuf> {
    Ok(driver_vision_directory(command)?.join(".pce/package-dispatch.jsonl"))
}

fn validate_worker_environment_name(name: &str) -> Result<()> {
    let mut bytes = name.bytes();
    let valid_start = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
    if !valid_start || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
        bail!("worker environment name `{name}` must match [A-Za-z_][A-Za-z0-9_]*");
    }
    if matches!(name, "TMPDIR" | "PCE_DISPATCH_TMPDIR" | "PCE_WORKTREES")
        || name.starts_with("PCE_WORKTREE_")
    {
        bail!("worker environment name `{name}` is binary-owned");
    }
    Ok(())
}

fn route_environment(command: &DriverRunCommand) -> Result<BTreeMap<String, String>> {
    let mut environment = command.worker_environment.clone();
    for name in ["PATH", "HOME", "USER"] {
        let value = std::env::var(name)
            .with_context(|| format!("driver route environment omitted {name}"))?;
        environment.insert(name.to_owned(), value);
    }
    Ok(environment)
}

fn active_driver_events(events: &[DriverEvent]) -> &[DriverEvent] {
    events
        .iter()
        .rposition(|event| matches!(event, DriverEvent::PlanVersionAdvanced { .. }))
        .map_or(events, |index| &events[index..])
}

fn active_assembly_events(events: &[DriverEvent]) -> &[DriverEvent] {
    events
        .iter()
        .rposition(|event| {
            matches!(
                event,
                DriverEvent::PlanVersionAdvanced { .. }
                    | DriverEvent::PackageHardeningInvalidated { .. }
            )
        })
        .map_or(events, |index| &events[index.saturating_add(1)..])
}

fn completed_package_issuance(events: &[DriverEvent], package_id: &str) -> Result<u64> {
    let completion = events
        .iter()
        .rposition(|event| matches!(event, DriverEvent::PackageCompleted { package } if package == package_id))
        .with_context(|| format!("package {package_id} has no durable completion"))?;
    events[..completion]
        .iter()
        .rev()
        .find_map(|event| match event {
            DriverEvent::WorkerDone { package, issuance } if package == package_id => {
                Some(*issuance)
            }
            _ => None,
        })
        .with_context(|| format!("package {package_id} completion has no worker issuance"))
}

fn package_branch(graph: &WorkPackageGraph, package: &str, issuance: u64) -> String {
    format!("pce/{}/{package}/attempt-{issuance}", graph.vision())
}

fn git_is_ancestor(source: &Path, ancestor: &str, descendant: &str) -> Result<bool> {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .status()?;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => bail!("failed to compare git commits {ancestor} and {descendant}"),
    }
}

fn ensure_hardened_package_lineage(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: &str,
) -> Result<()> {
    let events = read_driver_journal(&command.journal_path)?;
    let issuance = completed_package_issuance(&events, package_id)?;
    harden_package_lineage_for_issuance(graph, command, package_id, issuance)
}

fn harden_package_lineage_for_issuance(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: &str,
    issuance: u64,
) -> Result<()> {
    let events = read_driver_journal(&command.journal_path)?;
    let branch = package_branch(graph, package_id, issuance);
    for criterion in effective_criteria(graph, package_id, &events)? {
        let pce_core::CriterionOrigin::Amendment { gate, finding } = &criterion.origin else {
            continue;
        };
        for repository_refs in &criterion.repository_refs {
            let source = command
                .repositories
                .iter()
                .find(|(name, _)| name == &repository_refs.repository)
                .map(|(_, path)| path)
                .with_context(|| {
                    format!(
                        "missing repository mapping for amendment repository `{}`",
                        repository_refs.repository
                    )
                })?;
            let previous_oid = git_oid(source, &branch)?;
            let Some(repair_oid) = git_oid_if_available(source, &repository_refs.repair_ref)?
            else {
                append_driver_event(
                    &command.journal_path,
                    &DriverEvent::RepairCreditStale {
                        package: package_id.to_owned(),
                        issuance,
                        repository: repository_refs.repository.clone(),
                        gate: gate.clone(),
                        finding: *finding,
                        repair_ref: repository_refs.repair_ref.clone(),
                        lineage_oid: previous_oid,
                        reason: pce_core::StaleRepairCreditReason::RepairUnavailable,
                    },
                )?;
                continue;
            };
            if git_is_ancestor(source, &repair_oid, &previous_oid)? {
                continue;
            }
            if !git_is_ancestor(source, &previous_oid, &repair_oid)? {
                append_driver_event(
                    &command.journal_path,
                    &DriverEvent::RepairCreditStale {
                        package: package_id.to_owned(),
                        issuance,
                        repository: repository_refs.repository.clone(),
                        gate: gate.clone(),
                        finding: *finding,
                        repair_ref: repair_oid,
                        lineage_oid: previous_oid,
                        reason: pce_core::StaleRepairCreditReason::DivergentLineage,
                    },
                )?;
                continue;
            }
            let update = std::process::Command::new("git")
                .arg("-C")
                .arg(source)
                .args([
                    "update-ref",
                    &format!("refs/heads/{branch}"),
                    &repair_oid,
                    &previous_oid,
                ])
                .output()?;
            if !update.status.success() {
                bail!(
                    "failed to fast-forward package {} repair {}: {}",
                    package_id,
                    repair_oid,
                    String::from_utf8_lossy(&update.stderr).trim()
                );
            }
            append_driver_event(
                &command.journal_path,
                &DriverEvent::PackageRepairMerged {
                    package: package_id.to_owned(),
                    repository: repository_refs.repository.clone(),
                    gate: gate.clone(),
                    finding: *finding,
                    repair_ref: repair_oid.clone(),
                    previous_oid,
                    hardened_oid: repair_oid,
                },
            )?;
        }
    }
    Ok(())
}

const GATE_REPROOF_FAILED_REASON: &str = "gate repair re-proof failed";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateReproofOutcome {
    Proven,
    Rejected,
}

fn rollback_gate_repairs(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: &str,
    issuance: u64,
    merges: &[DriverEvent],
) -> Result<()> {
    let branch = package_branch(graph, package_id, issuance);
    let mut rolled_back = Vec::new();
    for event in merges.iter().rev() {
        let DriverEvent::PackageRepairMerged {
            package,
            repository,
            gate,
            finding,
            repair_ref,
            previous_oid,
            hardened_oid,
        } = event
        else {
            continue;
        };
        let source = command
            .repositories
            .iter()
            .find(|(name, _)| name == repository)
            .map(|(_, path)| path)
            .with_context(|| format!("missing repository mapping for rollback `{repository}`"))?;
        let update = std::process::Command::new("git")
            .arg("-C")
            .arg(source)
            .args([
                "update-ref",
                &format!("refs/heads/{branch}"),
                previous_oid,
                hardened_oid,
            ])
            .output()?;
        if !update.status.success() {
            bail!(
                "failed to roll back rejected gate repair {} for package {}: {}",
                repair_ref,
                package_id,
                String::from_utf8_lossy(&update.stderr).trim()
            );
        }
        rolled_back.push(DriverEvent::PackageRepairRolledBack {
            package: package.clone(),
            repository: repository.clone(),
            gate: gate.clone(),
            finding: *finding,
            repair_ref: repair_ref.clone(),
            hardened_oid: hardened_oid.clone(),
            restored_oid: previous_oid.clone(),
        });
    }
    for event in rolled_back {
        append_driver_event(&command.journal_path, &event)?;
    }
    Ok(())
}

fn finalize_gate_repairs(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: &str,
    issuance: u64,
    gate: &str,
    mut challenges: Vec<PackageGateChallenge>,
) -> Result<GateReproofOutcome> {
    let before = read_driver_journal(&command.journal_path)?;
    let before_len = before.len();
    harden_package_lineage_for_issuance(graph, command, package_id, issuance)?;
    let events = read_driver_journal(&command.journal_path)?;
    let criteria = effective_criteria(graph, package_id, &events)?;
    if !criteria.iter().any(|criterion| {
        matches!(
            criterion.origin,
            pce_core::CriterionOrigin::Amendment { .. }
        )
    }) {
        return Ok(GateReproofOutcome::Proven);
    }
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .with_context(|| format!("package {package_id} is absent from graph"))?;
    let sources = package_repository_sources(package, &command.repositories)?;
    let branch = package_branch(graph, package_id, issuance);
    let references = sources
        .iter()
        .map(|(name, source)| Ok((name.clone(), git_oid(source, &branch)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let materialization = materialize_driver_state(
        &command.journal_path,
        &format!("gate-reproof-{package_id}-{issuance}"),
        &sources,
        &references,
    )?;
    let paths = materialization.paths()?;
    for (repository, checkout) in materialization.named_paths(&sources) {
        let Some(preparation) = command.preparations.get(&repository) else {
            continue;
        };
        let execution = shell_execution_at(preparation, &checkout, &paths)?;
        if !execution.exit_status().is_success() {
            append_driver_event(
                &command.journal_path,
                &DriverEvent::GateReproofEnvironmentFailed {
                    package: package_id.to_owned(),
                    issuance,
                    gate: gate.to_owned(),
                    repository,
                    command: preparation.clone(),
                    execution,
                },
            )?;
            bail!("gate re-proof environment preparation failed without charging a failure budget");
        }
    }
    let named_paths = materialization
        .named_paths(&sources)
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    let mut failed = Vec::new();
    for criterion in criteria {
        let (execution, amendment_proof, passed) =
            execute_effective_criterion(&criterion, &paths, &named_paths)?;
        if !passed {
            challenges.push(PackageGateChallenge::from_reproof_failure(
                &criterion.name,
                &criterion.command,
            ));
            failed.push(format!(
                "{} ({:?})",
                criterion.name,
                execution.exit_status()
            ));
        }
        append_driver_event(
            &command.journal_path,
            &DriverEvent::GateReproofExecuted {
                package: package_id.to_owned(),
                gate: gate.to_owned(),
                name: criterion.name,
                origin: criterion.origin,
                execution,
                amendment_proof,
            },
        )?;
    }
    if failed.is_empty() {
        return Ok(GateReproofOutcome::Proven);
    }
    let merges = events[before_len..]
        .iter()
        .filter(|event| {
            matches!(
                event,
                DriverEvent::PackageRepairMerged { package, gate: merged_gate, .. }
                    if package == package_id && merged_gate == gate
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    rollback_gate_repairs(graph, command, package_id, issuance, &merges)?;
    append_gate_failure_outcome(
        command,
        package_id,
        issuance,
        gate.to_owned(),
        GATE_REPROOF_FAILED_REASON,
        format!("hardened lineage failed criteria: {}", failed.join(", ")),
        challenges,
    )?;
    Ok(GateReproofOutcome::Rejected)
}

fn driver_package_base_refs(
    command: &DriverRunCommand,
    package_id: &str,
) -> Result<BTreeMap<String, String>> {
    let events = read_driver_journal(&command.journal_path)?;
    let refs = active_driver_events(&events)
        .iter()
        .filter_map(|event| match event {
            DriverEvent::PackageBaseComposed {
                package,
                repository,
                base_oid,
                ..
            }
            | DriverEvent::PackageJoinConflicted {
                package,
                repository,
                base_oid,
                ..
            } if package == package_id => Some((repository.clone(), base_oid.clone())),
            _ => None,
        })
        .collect();
    Ok(refs)
}

fn composition_component(value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(value.as_bytes());
    digest.finalize()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug)]
enum GitComposition {
    Clean {
        base_oid: String,
    },
    Conflicted {
        base_oid: String,
        conflicting_input: CompositionInput,
        remaining_inputs: Vec<CompositionInput>,
        conflicted_paths: Vec<String>,
        reason: String,
    },
}

fn maximal_composition_inputs(
    source: &Path,
    inputs: Vec<CompositionInput>,
) -> Result<Vec<CompositionInput>> {
    let mut maximal = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let mut ancestor = false;
        for (other_index, other) in inputs.iter().enumerate() {
            if index == other_index {
                continue;
            }
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(source)
                .args(["merge-base", "--is-ancestor", &input.oid, &other.oid])
                .status()?;
            match status.code() {
                Some(0) => {
                    ancestor = true;
                    break;
                }
                Some(1) => {}
                _ => bail!(
                    "failed to compare composition commits {} and {}",
                    input.oid,
                    other.oid
                ),
            }
        }
        if !ancestor {
            maximal.push(input.clone());
        }
    }
    Ok(maximal)
}

fn compose_git_commits(
    journal: &Path,
    label: &str,
    source: &Path,
    authored_base: &str,
    inputs: &[CompositionInput],
) -> Result<GitComposition> {
    if inputs.is_empty() {
        return Ok(GitComposition::Clean {
            base_oid: git_oid(source, authored_base)?,
        });
    }
    let root = journal
        .parent()
        .context("driver journal has no parent")?
        .join(".pce/compositions");
    fs::create_dir_all(&root)?;
    let worktree = root.join(format!(
        "{}-{}-{}",
        std::process::id(),
        composition_component(label),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .context("clock precedes epoch")?
            .as_nanos()
    ));
    let add = std::process::Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["worktree", "add", "--quiet", "--detach"])
        .arg(&worktree)
        .arg(authored_base)
        .output()?;
    if !add.status.success() {
        bail!(
            "failed to create composition worktree: {}",
            String::from_utf8_lossy(&add.stderr).trim()
        );
    }
    let result = (|| {
        for (index, input) in inputs.iter().enumerate() {
            let merge = std::process::Command::new("git")
                .arg("-C")
                .arg(&worktree)
                .env("GIT_AUTHOR_DATE", "2000-01-01T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2000-01-01T00:00:00Z")
                .args([
                    "-c",
                    "user.name=PCE composition",
                    "-c",
                    "user.email=pce@localhost",
                    "merge",
                    "--no-edit",
                    "--no-ff",
                ])
                .arg(&input.oid)
                .output()?;
            if !merge.status.success() {
                let paths = std::process::Command::new("git")
                    .arg("-C")
                    .arg(&worktree)
                    .args(["diff", "--name-only", "--diff-filter=U"])
                    .output()?;
                if !paths.status.success() {
                    bail!(
                        "failed to inspect unsuccessful merge of package {} commit {}: {}",
                        input.package,
                        input.oid,
                        String::from_utf8_lossy(&paths.stderr).trim()
                    );
                }
                let conflicted_paths = String::from_utf8_lossy(&paths.stdout)
                    .lines()
                    .map(str::to_owned)
                    .filter(|path| !path.is_empty())
                    .collect::<Vec<_>>();
                let stdout = String::from_utf8_lossy(&merge.stdout);
                let stderr = String::from_utf8_lossy(&merge.stderr);
                let reason = [stdout.trim(), stderr.trim()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(
                        "
",
                    );
                if conflicted_paths.is_empty() {
                    bail!(
                        "failed to combine package {} commit {}: {}",
                        input.package,
                        input.oid,
                        reason
                    );
                }
                let base_oid = git_oid(&worktree, "HEAD")?;
                let abort = std::process::Command::new("git")
                    .arg("-C")
                    .arg(&worktree)
                    .args(["merge", "--abort"])
                    .output()?;
                if !abort.status.success() {
                    bail!(
                        "failed to abort inspected conflict for package {} commit {}: {}",
                        input.package,
                        input.oid,
                        String::from_utf8_lossy(&abort.stderr).trim()
                    );
                }
                return Ok(GitComposition::Conflicted {
                    base_oid,
                    conflicting_input: input.clone(),
                    remaining_inputs: inputs[index.saturating_add(1)..].to_vec(),
                    conflicted_paths,
                    reason,
                });
            }
        }
        Ok(GitComposition::Clean {
            base_oid: git_oid(&worktree, "HEAD")?,
        })
    })();
    let removal = std::process::Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["worktree", "remove", "--force"])
        .arg(&worktree)
        .output()?;
    if !removal.status.success() && result.is_ok() {
        bail!(
            "failed to remove composition worktree: {}",
            String::from_utf8_lossy(&removal.stderr).trim()
        );
    }
    result
}

fn ensure_driver_package_bases(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    package_id: &str,
    issuance: u64,
) -> Result<bool> {
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .with_context(|| format!("package {package_id} is absent from graph"))?;
    let events = read_driver_journal(&command.journal_path)?;
    for repository in package.repositories() {
        if active_driver_events(&events).iter().any(|event| {
            matches!(event,
            DriverEvent::PackageBaseComposed { package, repository: recorded, .. }
                | DriverEvent::PackageJoinConflicted { package, repository: recorded, .. }
                if package == package_id && recorded == repository)
        }) {
            continue;
        }
        let source = command
            .repositories
            .iter()
            .find(|(name, _)| name == repository)
            .map(|(_, path)| path)
            .with_context(|| format!("missing repository mapping for `{repository}`"))?;
        let mut dependencies = Vec::new();
        for dependency in package
            .depends_on()
            .iter()
            .filter(|dependency| dependency.kind().is_binding())
        {
            let dependency_package = graph
                .packages()
                .iter()
                .find(|candidate| candidate.id() == dependency.id())
                .with_context(|| format!("dependency {} is absent", dependency.id().as_str()))?;
            if !dependency_package.repositories().contains(repository) {
                continue;
            }
            ensure_hardened_package_lineage(graph, command, dependency.id().as_str())?;
            let issuance = completed_package_issuance(&events, dependency.id().as_str())?;
            dependencies.push(CompositionInput {
                package: dependency.id().as_str().to_owned(),
                oid: git_oid(
                    source,
                    &package_branch(graph, dependency.id().as_str(), issuance),
                )?,
            });
        }
        match compose_git_commits(
            &command.journal_path,
            &format!("package-{package_id}-{repository}"),
            source,
            graph_authored_ref(graph, repository)?,
            &dependencies,
        ) {
            Ok(GitComposition::Clean { base_oid }) => append_driver_event(
                &command.journal_path,
                &DriverEvent::PackageBaseComposed {
                    package: package_id.to_owned(),
                    repository: repository.clone(),
                    base_oid,
                    dependencies,
                },
            )?,
            Ok(GitComposition::Conflicted {
                base_oid,
                conflicting_input,
                remaining_inputs,
                conflicted_paths,
                reason,
            }) => append_driver_event(
                &command.journal_path,
                &DriverEvent::PackageJoinConflicted {
                    package: package_id.to_owned(),
                    repository: repository.clone(),
                    base_oid,
                    dependencies,
                    conflicting_input,
                    remaining_inputs,
                    conflicted_paths,
                    reason,
                },
            )?,
            Err(source) => {
                let reason = format!("{source:#}");
                append_driver_event(
                    &command.journal_path,
                    &DriverEvent::PackageCompositionFailed {
                        package: package_id.to_owned(),
                        repository: repository.clone(),
                        dependencies,
                        reason: reason.clone(),
                    },
                )?;
                append_driver_event(
                    &command.journal_path,
                    &DriverEvent::WorkerDispatched {
                        package: package_id.to_owned(),
                        issuance,
                    },
                )?;
                append_worker_environment_outcome(
                    command,
                    package_id.to_owned(),
                    issuance,
                    format!("package composition infrastructure failed in {repository}: {reason}"),
                )?;
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn driver_package_worktrees(
    command: &DriverRunCommand,
    graph: &WorkPackageGraph,
    package_id: &str,
    issuance: u64,
) -> Result<Vec<RepositoryWorktree>> {
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .with_context(|| format!("package {package_id} is absent from graph"))?;
    let vision = DispatchVisionSource::parse(graph.vision().to_owned())?;
    let base_refs = driver_package_base_refs(command, package_id)?;
    let inputs = command
        .repositories
        .iter()
        .filter(|(name, _)| package.repositories().contains(name))
        .map(|(name, root)| -> Result<RepositoryDispatchInput> {
            let base_ref = match base_refs.get(name) {
                Some(base_ref) => base_ref.clone(),
                None => graph_authored_ref(graph, name)?.to_owned(),
            };
            Ok(RepositoryDispatchInput::parse(
                name.clone(),
                root.clone(),
                base_ref,
            )?)
        })
        .collect::<Result<Vec<_>>>()?;
    let plan = compose_herdr_work_package_dispatch(
        &vision,
        package,
        DispatchAttempt::parse(issuance)?,
        command.herdr_session.clone(),
        &inputs,
        &AbsoluteWorktreeRoot::parse(package_worktree_root()?)?,
        &AbsoluteDispatchTemporaryDirectory::parse(package_temporary_directory(
            &vision,
            package.id(),
            DispatchAttempt::parse(issuance)?,
        ))?,
        WorkerEnvironment::parse(route_environment(command)?)?,
        WorkerArgumentVector::parse(vec!["prime-agent".to_owned(), "-p".to_owned()])?,
    )?;
    plan.worktrees()
        .iter()
        .map(|worktree| {
            RepositoryWorktree::parse(worktree.repository(), worktree.path().to_path_buf())
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to derive driver package worktrees")
}

fn compose_driver_worker_brief(
    command: &DriverRunCommand,
    graph: &WorkPackageGraph,
    package_id: &str,
    issuance: u64,
    local_patch_evidence: Option<&[pce_core::RecoveryCriterionEvidence]>,
) -> Result<String> {
    let vision_path = driver_vision_directory(command)?.join("vision.md");
    let vision = fs::read_to_string(&vision_path)
        .with_context(|| format!("failed to read vision {}", vision_path.display()))?;
    let goal = VisionGoal::parse_document(&vision).context("failed to parse vision goal")?;
    let criteria =
        parse_acceptance_criteria(&vision).context("failed to parse acceptance criteria")?;
    let worktrees = driver_package_worktrees(command, graph, package_id, issuance)?;
    let mut brief = compose_package_worker_brief(&goal, &criteria, graph, package_id, &worktrees)
        .context("failed to compose driver-owned worker brief")?;
    let events = read_driver_journal(&command.journal_path)?;
    let conflicts = events
        .iter()
        .filter_map(|event| match event {
            DriverEvent::PackageJoinConflicted {
                package,
                repository,
                dependencies,
                conflicting_input,
                remaining_inputs,
                conflicted_paths,
                reason,
                ..
            } if package == package_id => Some(json!({
                "repository": repository,
                "dependencies": dependencies,
                "conflicting_input": conflicting_input,
                "remaining_inputs": remaining_inputs,
                "conflicted_paths": conflicted_paths,
                "reason": reason,
            })),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !conflicts.is_empty() {
        brief.push_str("

## Conflicted join: resolve before dependent work

This worktree intentionally starts with an unresolved dependency merge. Resolve it first, retain every parent's guarantee, commit the merge, then perform this package's authored work. After completion, the driver will re-run every parent criterion before judging this package.
");
        for conflict in conflicts {
            brief.push_str(
                "
<!-- pce-conflicted-join:",
            );
            brief.push_str(&serde_json::to_string(&conflict)?);
            brief.push_str(
                " -->
",
            );
            brief.push_str(&format!(
                "
Git conflict evidence:
```text
{}
```
",
                conflict["reason"]
                    .as_str()
                    .unwrap_or("conflict details unavailable")
            ));
            brief.push_str(&format!(
                "Conflicted paths: `{}`. Parent refs: `{}`.
",
                conflict["conflicted_paths"], conflict["dependencies"]
            ));
            let remaining = conflict["remaining_inputs"].as_array().map_or(0, Vec::len);
            if remaining > 0 {
                brief.push_str("After committing this resolution, merge each remaining parent ref listed in the machine record above, resolving any further conflict before dependent work.
");
            }
        }
    }
    let follows_environment_closure = events.iter().any(|event| {
        matches!(
            event,
            DriverEvent::WorkerEnvironmentFailed {
                package,
                issuance: failed_issuance,
                reason,
            } if package == package_id
                && *failed_issuance < issuance
                && matches!(
                    reason.as_str(),
                    OBSERVED_DEAD_DISPATCH_REASON | INCONCLUSIVE_DISPATCH_REASON
                )
        )
    });
    if follows_environment_closure {
        brief.push_str(
            "

## Earlier environment-killed attempt

",
        );
        brief.push_str(KILLED_PREDECESSOR_SENTENCE);
        brief.push('\n');
    }
    Ok(local_patch_evidence.map_or(brief.clone(), |evidence| {
        compose_local_patch_brief(&brief, evidence)
    }))
}

fn record_driver_dispatch_panes(
    journal: &Path,
    package: &str,
    issuance: u64,
    response: &Value,
) -> Result<()> {
    let targets = response
        .get("pane_cleanup_targets")
        .and_then(Value::as_array)
        .context("package dispatch omitted pane_cleanup_targets")?;
    for target in targets {
        let pane_id = target
            .get("pane_id")
            .and_then(Value::as_str)
            .context("package dispatch pane target omitted pane_id")?;
        let workspace_id = target
            .get("workspace_id")
            .and_then(Value::as_str)
            .context("package dispatch pane target omitted workspace_id")?;
        let herdr_session = target
            .get("herdr_session")
            .and_then(Value::as_str)
            .map(str::to_owned);
        append_driver_event(
            journal,
            &DriverEvent::DispatchPaneOpened {
                package: package.to_owned(),
                issuance,
                pane_id: pane_id.to_owned(),
                workspace_id: workspace_id.to_owned(),
                herdr_session,
            },
        )?;
    }
    if let Some(detail) = response.get("pane_ownership_error").and_then(Value::as_str) {
        append_driver_event(
            journal,
            &DriverEvent::DispatchPaneOwnershipUnresolved {
                package: package.to_owned(),
                issuance,
                detail: detail.to_owned(),
            },
        )?;
    }
    Ok(())
}

fn record_driver_dispatch_identity(
    journal: &Path,
    package: &str,
    issuance: u64,
    response: &Value,
) -> Result<()> {
    let identity = response
        .get("dispatch_identity")
        .context("package dispatch omitted dispatch_identity")?;
    let dispatch_sequence = response
        .get("issuance_sequence")
        .and_then(Value::as_u64)
        .context("package dispatch omitted issuance_sequence")?;
    let agent_name = identity
        .get("agent_name")
        .and_then(Value::as_str)
        .context("package dispatch identity omitted agent_name")?;
    let pane_id = identity
        .get("pane_id")
        .and_then(Value::as_str)
        .context("package dispatch identity omitted pane_id")?;
    let workspace_id = identity
        .get("workspace_id")
        .and_then(Value::as_str)
        .context("package dispatch identity omitted workspace_id")?;
    let herdr_session = identity
        .get("herdr_session")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let process = serde_json::from_value(
        identity
            .get("process")
            .cloned()
            .context("package dispatch identity omitted process observation")?,
    )
    .context("package dispatch process observation is invalid")?;
    let session_path = identity
        .get("session_path")
        .and_then(Value::as_str)
        .map(str::to_owned);
    append_driver_event(
        journal,
        &DriverEvent::DispatchWorkerIdentified {
            package: package.to_owned(),
            issuance,
            dispatch_sequence,
            agent_name: agent_name.to_owned(),
            pane_id: pane_id.to_owned(),
            workspace_id: workspace_id.to_owned(),
            herdr_session,
            session_path,
            process,
        },
    )
}

fn issue_driver_package_dispatch(
    command: &DriverRunCommand,
    graph: &WorkPackageGraph,
    package_id: &str,
    issuance: u64,
    outcome_path: &Path,
    brief: &str,
) -> Result<PathBuf> {
    let executable = std::env::current_exe().context("failed to resolve driver executable")?;
    let vision_dir = driver_vision_directory(command)?;
    let vision_path = vision_dir.join("vision.md");
    let graph_path = absolute_path(&command.graph_path)?;
    let outcome_path = absolute_path(outcome_path)?;
    let brief_path = vision_dir
        .join(".pce/package-briefs")
        .join(package_id)
        .join(format!("{issuance}.md"));
    fs::create_dir_all(
        brief_path
            .parent()
            .context("package brief path has no parent")?,
    )?;
    fs::write(&brief_path, brief).with_context(|| {
        format!(
            "failed to write composed package brief {}",
            brief_path.display()
        )
    })?;
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .with_context(|| format!("package {package_id} is absent from graph"))?;
    let repositories = command
        .repositories
        .iter()
        .filter(|(name, _)| package.repositories().contains(name))
        .cloned()
        .collect();
    let worker_arguments = vec![
        executable.display().to_string(),
        "package".to_owned(),
        "agent".to_owned(),
        "--vision".to_owned(),
        vision_path.display().to_string(),
        "--graph".to_owned(),
        graph_path.display().to_string(),
        "--package".to_owned(),
        package_id.to_owned(),
        "--outcome".to_owned(),
        outcome_path.display().to_string(),
        "--brief".to_owned(),
        brief_path.display().to_string(),
        "--".to_owned(),
        "prime-agent".to_owned(),
        "-p".to_owned(),
    ];
    let response = issue_package_dispatch(PackageDispatchCommand {
        log_path: driver_dispatch_log(command)?,
        vision_dir,
        graph_path,
        require_graph_at_vision_root: true,
        package_id: package_id.to_owned(),
        attempt: Some(DispatchAttempt::parse(issuance)?),
        required_artifact_path: AbsoluteRequiredArtifactPath::parse(outcome_path)?,
        repositories,
        base_refs: driver_package_base_refs(command, package_id)?,
        conflicted_joins: read_driver_journal(&command.journal_path)?
            .into_iter()
            .filter_map(|event| match event {
                DriverEvent::PackageJoinConflicted {
                    package,
                    repository,
                    conflicting_input,
                    conflicted_paths,
                    ..
                } if package == package_id => {
                    Some((repository, (conflicting_input, conflicted_paths)))
                }
                _ => None,
            })
            .collect(),
        herdr_session: command.herdr_session.clone(),
        environment: route_environment(command)?,
        worker_arguments,
    })?;
    record_driver_dispatch_identity(&command.journal_path, package_id, issuance, &response)?;
    record_driver_dispatch_panes(&command.journal_path, package_id, issuance, &response)?;
    let result_path = response["result_path"]
        .as_str()
        .context("composed package dispatch omitted result_path")?;
    Ok(PathBuf::from(result_path))
}

const OBSERVED_DEAD_DISPATCH_REASON: &str =
    "worker environment ended the dispatch before completion";
const INCONCLUSIVE_DISPATCH_REASON: &str = "worker environment liveness evidence was inconclusive";
const KILLED_PREDECESSOR_SENTENCE: &str = "An earlier attempt was killed by the environment before finishing; nothing it produced was judged, and any durable side effects it may have left outside the repository must be verified against this package's own criteria rather than treated as corruption or redone.";

#[derive(Debug)]
enum DriverDispatchEnvironmentLiveness {
    Unknown,
    Alive,
    Dead { detail: String },
    Inconclusive { detail: String },
}

#[derive(Debug)]
struct DriverDispatchRuntimeIdentity {
    agent_name: String,
    pane_id: String,
    workspace_id: String,
    herdr_session: Option<String>,
    process: Option<DispatchWorkerProcessObservation>,
}

fn driver_dispatch_runtime_identity(
    graph: &WorkPackageGraph,
    events: &[DriverEvent],
    package: &str,
    issuance: u64,
) -> Result<Option<DriverDispatchRuntimeIdentity>> {
    if let Some(identity) = events.iter().rev().find_map(|event| match event {
        DriverEvent::DispatchWorkerIdentified {
            package: event_package,
            issuance: event_issuance,
            agent_name,
            pane_id,
            workspace_id,
            herdr_session,
            process,
            ..
        } if event_package == package && *event_issuance == issuance => {
            Some(DriverDispatchRuntimeIdentity {
                agent_name: agent_name.clone(),
                pane_id: pane_id.clone(),
                workspace_id: workspace_id.clone(),
                herdr_session: herdr_session.clone(),
                process: Some(process.clone()),
            })
        }
        _ => None,
    }) {
        return Ok(Some(identity));
    }
    let Some((pane_id, workspace_id, herdr_session)) =
        events.iter().rev().find_map(|event| match event {
            DriverEvent::DispatchPaneOpened {
                package: event_package,
                issuance: event_issuance,
                pane_id,
                workspace_id,
                herdr_session,
            } if event_package == package && *event_issuance == issuance => {
                Some((pane_id.clone(), workspace_id.clone(), herdr_session.clone()))
            }
            _ => None,
        })
    else {
        return Ok(None);
    };
    let work_package = graph
        .packages()
        .iter()
        .find(|candidate| candidate.id().as_str() == package)
        .with_context(|| format!("package {package} is absent from graph"))?;
    let vision = DispatchVisionSource::parse(graph.vision().to_owned())?;
    let agent_name = derive_herdr_agent_name(
        &vision,
        work_package.id(),
        DispatchAttempt::parse(issuance)?,
    );
    Ok(Some(DriverDispatchRuntimeIdentity {
        agent_name: agent_name.as_str().to_owned(),
        pane_id,
        workspace_id,
        herdr_session,
        process: None,
    }))
}

fn scoped_herdr_arguments(session: Option<&HerdrSessionName>, arguments: &[&str]) -> Vec<String> {
    let mut scoped = Vec::with_capacity(arguments.len() + usize::from(session.is_some()) * 2);
    if let Some(session) = session {
        scoped.extend(["--session".to_owned(), session.as_str().to_owned()]);
    }
    scoped.extend(arguments.iter().map(|argument| (*argument).to_owned()));
    scoped
}

fn herdr_command_output(session: Option<&HerdrSessionName>, arguments: &[&str]) -> Result<Value> {
    let scoped_arguments = scoped_herdr_arguments(session, arguments);
    let output = std::process::Command::new("herdr")
        .args(&scoped_arguments)
        .output()
        .context("failed to execute herdr liveness observation")?;
    if !output.status.success() {
        bail!(
            "herdr liveness observation {:?} failed with {}: {}",
            scoped_arguments,
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout)
        .context("herdr liveness observation returned invalid JSON")
}

fn foreground_is_only_shell(response: &Value) -> bool {
    let processes = response
        .pointer("/result/process_info/foreground_processes")
        .and_then(Value::as_array);
    let Some(processes) = processes else {
        return false;
    };
    if processes.is_empty() {
        return true;
    }
    let shell_pid = response
        .pointer("/result/process_info/shell_pid")
        .and_then(Value::as_u64);
    processes.iter().all(|process| {
        let pid = process.get("pid").and_then(Value::as_u64);
        let name = process
            .get("name")
            .and_then(Value::as_str)
            .or_else(|| process.get("argv0").and_then(Value::as_str));
        pid == shell_pid || matches!(name, Some("sh" | "bash" | "zsh" | "fish" | "dash" | "nu"))
    })
}

fn process_observation_matches(
    recorded: &DispatchWorkerProcessObservation,
    response: &Value,
) -> bool {
    let DispatchWorkerProcessObservation::Observed { process_id, .. } = recorded else {
        return false;
    };
    response
        .pointer("/result/process_info/foreground_processes")
        .and_then(Value::as_array)
        .is_some_and(|processes| {
            processes.iter().any(|process| {
                process.get("pid").and_then(Value::as_u64) == Some(u64::from(*process_id))
            })
        })
}

fn observe_driver_dispatch_environment(
    graph: &WorkPackageGraph,
    events: &[DriverEvent],
    package: &str,
    issuance: u64,
) -> Result<DriverDispatchEnvironmentLiveness> {
    let Some(identity) = driver_dispatch_runtime_identity(graph, events, package, issuance)? else {
        return Ok(DriverDispatchEnvironmentLiveness::Unknown);
    };
    let herdr_session = identity
        .herdr_session
        .clone()
        .map(parse_herdr_session_name)
        .transpose()?;
    let legacy_agent = identity.process.is_none().then(|| {
        herdr_command_output(
            herdr_session.as_ref(),
            &["agent", "get", &identity.agent_name],
        )
    });
    let legacy_agent_matches = legacy_agent.as_ref().is_some_and(|agent| {
        agent.as_ref().is_ok_and(|response| {
            let observed = response.pointer("/result/agent");
            observed.is_some_and(|observed| {
                observed.get("pane_id").and_then(Value::as_str) == Some(identity.pane_id.as_str())
                    && observed.get("workspace_id").and_then(Value::as_str)
                        == Some(identity.workspace_id.as_str())
                    && observed
                        .get("name")
                        .and_then(Value::as_str)
                        .or_else(|| observed.get("agent").and_then(Value::as_str))
                        == Some(identity.agent_name.as_str())
            })
        })
    });
    if legacy_agent_matches {
        return Ok(DriverDispatchEnvironmentLiveness::Alive);
    }
    let process = herdr_command_output(
        herdr_session.as_ref(),
        &["pane", "process-info", "--pane", &identity.pane_id],
    );
    let process = match process {
        Ok(response) => response,
        Err(source) => {
            let detail = format!("{source:#}");
            if detail.contains("pane_not_found") {
                return Ok(DriverDispatchEnvironmentLiveness::Dead { detail });
            }
            return Ok(DriverDispatchEnvironmentLiveness::Inconclusive { detail });
        }
    };
    if foreground_is_only_shell(&process) {
        return Ok(DriverDispatchEnvironmentLiveness::Dead {
            detail: format!(
                "worker pane {} contains only its login shell",
                identity.pane_id
            ),
        });
    }
    if identity
        .process
        .as_ref()
        .is_some_and(|recorded| process_observation_matches(recorded, &process))
    {
        return Ok(DriverDispatchEnvironmentLiveness::Alive);
    }
    let legacy_detail = legacy_agent.map_or_else(
        || "not consulted for pane-run dispatch".to_owned(),
        |agent| agent.map_or_else(|error| format!("{error:#}"), |_| "mismatched".to_owned()),
    );
    Ok(DriverDispatchEnvironmentLiveness::Inconclusive {
        detail: format!(
            "worker pane {} contains no process matching the recorded dispatch identity; legacy agent lookup: {}",
            identity.pane_id, legacy_detail
        ),
    })
}

fn close_driver_lost_dispatch(
    command: &DriverRunCommand,
    package: &str,
    issuance: u64,
    dispatch_sequence: Sequence,
    observation: DispatchEnvironmentObservation,
    detail: String,
    reason: &'static str,
) -> Result<()> {
    append_driver_event(
        &command.journal_path,
        &DriverEvent::DispatchEnvironmentObserved {
            package: package.to_owned(),
            issuance,
            dispatch_sequence: dispatch_sequence.get(),
            observation,
            detail,
        },
    )?;
    close_dispatch_conditionally(
        &driver_dispatch_log(command)?,
        DispatchClosureTarget {
            issuance_sequence: dispatch_sequence,
            node: NodeId::parse(package)?,
        },
        |_| {
            Ok(DispatchCompletionPayload::ReconciledDead(
                ReconciledDeadDispatchCompletionPayload {
                    issuance_sequence: dispatch_sequence,
                    outcome: ReconciledDispatchOutcome::ReconciledDead,
                    artifact_production: ArtifactProduction::NotProduced,
                },
            ))
        },
    )?;
    append_worker_environment_outcome(command, package.to_owned(), issuance, reason.to_owned())
}

fn repair_driver_environment_closures(
    command: &DriverRunCommand,
    events: &[DriverEvent],
) -> Result<usize> {
    let dispatch_log = driver_dispatch_log(command)?;
    if !dispatch_log.is_file() {
        return Ok(0);
    }
    let records = read_event_log(&dispatch_log)?
        .into_iter()
        .map(|line| line.record)
        .collect::<Vec<_>>();
    let ledger = fold_dispatch_ledger(&records)
        .context("failed to derive dispatch ledger while repairing driver closure")?;
    let mut repaired = 0;
    for event in events {
        let DriverEvent::DispatchEnvironmentObserved {
            package,
            issuance,
            dispatch_sequence,
            observation,
            ..
        } = event
        else {
            continue;
        };
        let already_recorded = events.iter().any(|candidate| match candidate {
            DriverEvent::WorkerEnvironmentFailed {
                package: candidate_package,
                issuance: candidate_issuance,
                reason,
            }
            | DriverEvent::PackageEnvironmentBlocked {
                package: candidate_package,
                issuance: candidate_issuance,
                reason,
                ..
            } => {
                candidate_package == package
                    && candidate_issuance == issuance
                    && matches!(
                        reason.as_str(),
                        OBSERVED_DEAD_DISPATCH_REASON | INCONCLUSIVE_DISPATCH_REASON
                    )
            }
            _ => false,
        });
        if already_recorded {
            continue;
        }
        let sequence = Sequence::parse(*dispatch_sequence)?;
        let driver_closed = ledger.issuance(sequence).is_some_and(|entry| {
            matches!(
                entry.completion(),
                Some(DispatchLedgerCompletion::ReconciledDead { .. })
            )
        });
        if !driver_closed {
            continue;
        }
        let reason = match observation {
            DispatchEnvironmentObservation::Dead => OBSERVED_DEAD_DISPATCH_REASON,
            DispatchEnvironmentObservation::Inconclusive => INCONCLUSIVE_DISPATCH_REASON,
        };
        append_worker_environment_outcome(command, package.clone(), *issuance, reason.to_owned())?;
        repaired += 1;
    }
    Ok(repaired)
}

fn wait_for_driver_results(paths: &[PathBuf], timeout: Option<Duration>) -> Result<bool> {
    if paths.iter().all(|path| path.is_file()) {
        return Ok(true);
    }
    let roots = paths
        .iter()
        .map(|path| path.parent().context("package result path has no parent"))
        .collect::<Result<Vec<_>>>()?;
    for root in &roots {
        fs::create_dir_all(root).with_context(|| {
            format!("failed to create result watch directory {}", root.display())
        })?;
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ignored = sender.send(event);
    })
    .context("failed to create package result watcher")?;
    for root in &roots {
        watcher
            .watch(root, RecursiveMode::NonRecursive)
            .with_context(|| format!("failed to watch {}", root.display()))?;
    }
    if paths.iter().all(|path| path.is_file()) {
        return Ok(true);
    }
    let started = Instant::now();
    loop {
        match timeout {
            Some(limit) => {
                let remaining = limit.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    return Ok(false);
                }
                match receiver.recv_timeout(remaining) {
                    Ok(event) => event.context("package result watcher failed")?,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return Ok(false),
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        bail!("package result watcher disconnected")
                    }
                };
            }
            None => {
                receiver
                    .recv()
                    .context("package result watcher disconnected")?
                    .context("package result watcher failed")?;
            }
        }
        if paths.iter().all(|path| path.is_file()) {
            return Ok(true);
        }
    }
}

fn reconcile_completed_dispatch_panes(journal: &Path, events: &[DriverEvent]) -> Result<usize> {
    let pending = pending_completed_pane_cleanups(events);
    for target in &pending {
        // A worktree root pane is the workspace's last pane, so close the exact workspace returned
        // alongside it rather than attempting a pattern-based or last-pane closure.
        let result = target
            .herdr_session()
            .map(parse_herdr_session_name)
            .transpose()
            .and_then(|session| {
                HerdrWorkspaceId::parse(target.workspace_id())
                    .map_err(Error::new)
                    .and_then(|workspace_id| {
                        execute_herdr(&workspace_id.close_invocation(session.as_ref()))
                    })
            });
        let (outcome, detail) = match result {
            Ok(_) => (
                PaneCleanupOutcome::Closed,
                "herdr confirmed closure of the exact workspace containing the run-created pane"
                    .to_owned(),
            ),
            Err(source) => (PaneCleanupOutcome::Failed, format!("{source:#}")),
        };
        append_driver_event(
            journal,
            &DriverEvent::DispatchPaneCleanup {
                package: target.package().to_owned(),
                issuance: target.issuance(),
                pane_id: target.pane_id().to_owned(),
                workspace_id: target.workspace_id().to_owned(),
                herdr_session: target.herdr_session().map(str::to_owned),
                outcome,
                detail,
            },
        )?;
    }
    Ok(pending.len())
}

fn ensure_assembly_checkout(source: &Path, path: &Path, oid: &str) -> Result<()> {
    if path.is_dir() && git_oid(path, "HEAD").is_ok_and(|current| current == oid) {
        return Ok(());
    }
    if path.exists() {
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(source)
            .args(["worktree", "remove", "--force"])
            .arg(path)
            .output();
        let _ = fs::remove_dir_all(path);
    }
    fs::create_dir_all(path.parent().context("assembly checkout has no parent")?)?;
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["worktree", "add", "--quiet", "--detach"])
        .arg(path)
        .arg(oid)
        .output()?;
    if !output.status.success() {
        bail!(
            "failed to materialize assembly checkout: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn anchor_assembly_resolution(
    source: &Path,
    repository: &str,
    oid: &str,
) -> Result<(String, bool)> {
    let resolution_ref = format!(
        "refs/pce-assembly-resolutions/{}/{}",
        composition_component(repository),
        oid
    );
    if let Some(current) = git_oid_if_available(source, &resolution_ref)? {
        if current != oid {
            bail!(
                "assembly resolution ref {} is at {}, expected {}; refusing to move it",
                resolution_ref,
                current,
                oid
            );
        }
        return Ok((resolution_ref, false));
    }
    let anchored = std::process::Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["update-ref", &resolution_ref, oid, ""])
        .output()?;
    if !anchored.status.success() {
        bail!(
            "failed to anchor assembly resolution {} without movement: {}",
            oid,
            String::from_utf8_lossy(&anchored.stderr).trim()
        );
    }
    Ok((resolution_ref, true))
}

fn assembly_resolution_prompt(
    repository: &str,
    conflicted_paths: &[String],
    packages: &[CompositionInput],
    reason: &str,
) -> String {
    format!(
        "Resolve this conflicted assembly join before any other work. Repository: {repository}. Conflicted paths: {conflicted_paths:?}. Package refs: {packages:?}. Git evidence:
{reason}
Retain every package guarantee, resolve all unmerged entries, and commit the merge. Modify only the exact conflicted paths listed above. Do not run repository-wide formatters, do not edit any other path, and refuse if resolving the conflict requires another path."
    )
}

fn resolve_assembly_conflict(
    command: &DriverRunCommand,
    repository: &str,
    source: &Path,
    checkout: &Path,
    base_oid: &str,
    conflicting_input: &CompositionInput,
    conflicted_paths: &[String],
    reason: &str,
    packages: &[CompositionInput],
) -> Result<String> {
    ensure_assembly_checkout(source, checkout, base_oid)?;
    let merge = std::process::Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["merge", "--no-edit", "--no-ff"])
        .arg(&conflicting_input.oid)
        .output()?;
    let actual_paths = std::process::Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["diff", "--name-only", "--diff-filter=U"])
        .output()?;
    let actual_paths = String::from_utf8_lossy(&actual_paths.stdout)
        .lines()
        .map(str::to_owned)
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    if merge.status.success() || actual_paths != conflicted_paths {
        bail!(
            "assembly conflict for repository {repository} did not reproduce: expected {:?}, observed {:?}",
            conflicted_paths,
            actual_paths
        );
    }
    append_driver_event(
        &command.journal_path,
        &DriverEvent::AssemblyResolutionDispatched {
            repository: repository.to_owned(),
        },
    )?;
    let prompt = assembly_resolution_prompt(repository, conflicted_paths, packages, reason);
    let mut child = std::process::Command::new("prime-agent")
        .arg("-p")
        .current_dir(checkout)
        .env("PCE_ASSEMBLY_RESOLUTION", repository)
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to spawn assembly resolution worker")?;
    child
        .stdin
        .as_mut()
        .context("assembly resolution worker stdin unavailable")?
        .write_all(prompt.as_bytes())?;
    drop(child.stdin.take());
    let status = child.wait()?;
    if !status.success() {
        bail!("assembly resolution worker exited with {status}");
    }
    let unresolved = std::process::Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(["diff", "--name-only", "--diff-filter=U"])
        .output()?;
    if !unresolved.stdout.is_empty() {
        bail!(
            "assembly resolution worker left unmerged paths: {}",
            String::from_utf8_lossy(&unresolved.stdout).trim()
        );
    }
    let oid = git_oid(checkout, "HEAD")?;
    if oid == base_oid {
        let add = std::process::Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(["add", "-A"])
            .output()?;
        if !add.status.success() {
            bail!("failed to stage assembly resolution");
        }
        let commit = std::process::Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args([
                "-c",
                "user.name=PCE assembly resolution",
                "-c",
                "user.email=pce@localhost",
                "commit",
                "-m",
                "Resolve assembly join",
            ])
            .output()?;
        if !commit.status.success() {
            bail!(
                "assembly resolution worker produced no committable resolution: {}",
                String::from_utf8_lossy(&commit.stderr).trim()
            );
        }
    }
    let base_oid = git_oid(checkout, "HEAD")?;
    let (resolution_ref, created) = anchor_assembly_resolution(source, repository, &base_oid)?;
    append_driver_event(
        &command.journal_path,
        &DriverEvent::AssemblyResolutionDone {
            repository: repository.to_owned(),
            base_oid: base_oid.clone(),
        },
    )?;
    if created {
        append_driver_event(
            &command.journal_path,
            &DriverEvent::DriverRefMaterialized {
                repository: repository.to_owned(),
                reference: resolution_ref,
                oid: base_oid.clone(),
                product: DriverRefProduct::AssemblyResolution,
            },
        )?;
    }
    Ok(base_oid)
}

fn assembly_rewrite_author(
    source: &Path,
    inputs: &[CompositionInput],
    hardened_package: &str,
    repair_ref: &str,
) -> Result<Option<String>> {
    let paths = std::process::Command::new("git")
        .arg("-C")
        .arg(source)
        .args([
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            repair_ref,
        ])
        .output()?;
    if !paths.status.success() {
        bail!(
            "failed to inspect repair {} paths: {}",
            repair_ref,
            String::from_utf8_lossy(&paths.stderr).trim()
        );
    }
    let changed_paths = String::from_utf8_lossy(&paths.stdout)
        .lines()
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for input in inputs
        .iter()
        .rev()
        .filter(|input| input.package != hardened_package)
    {
        let mut command = std::process::Command::new("git");
        command
            .arg("-C")
            .arg(source)
            .args(["log", "-1", "--format=%H", &input.oid, "--"])
            .args(&changed_paths);
        let latest = command.output()?;
        if !latest.status.success() {
            bail!(
                "failed to inspect package {} history for repair {}: {}",
                input.package,
                repair_ref,
                String::from_utf8_lossy(&latest.stderr).trim()
            );
        }
        let oid = String::from_utf8(latest.stdout)
            .context("git returned non-UTF-8 rewrite oid")?
            .trim()
            .to_owned();
        if !oid.is_empty() && !git_is_ancestor(source, &oid, repair_ref)? {
            return Ok(Some(input.package.clone()));
        }
    }
    Ok(None)
}

fn run_driver_assembly(graph: &WorkPackageGraph, command: &DriverRunCommand) -> Result<()> {
    let mut events = read_driver_journal(&command.journal_path)?;
    let assembly_root = command
        .journal_path
        .parent()
        .context("driver journal has no parent")?
        .join(".pce/assembly");
    let mut assembly_refs = active_assembly_events(&events)
        .iter()
        .filter_map(|event| match event {
            DriverEvent::AssemblyRepositoryComposed {
                repository,
                base_oid,
                ..
            } => Some((repository.clone(), base_oid.clone())),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    for (repository, source) in &command.repositories {
        let touched = graph
            .packages()
            .iter()
            .filter(|package| package.repositories().contains(repository))
            .collect::<Vec<_>>();
        if touched.is_empty() {
            continue;
        }
        let mut packages = Vec::new();
        for package in touched {
            if command.worker_override.is_none() {
                ensure_hardened_package_lineage(graph, command, package.id().as_str())?;
            }
            let oid = if command.worker_override.is_some() {
                git_oid(source, "HEAD")?
            } else {
                let issuance = completed_package_issuance(&events, package.id().as_str())?;
                git_oid(
                    source,
                    &package_branch(graph, package.id().as_str(), issuance),
                )?
            };
            packages.push(CompositionInput {
                package: package.id().as_str().to_owned(),
                oid,
            });
        }
        let packages = maximal_composition_inputs(source, packages)?;
        let oid = if let Some(oid) = assembly_refs.get(repository) {
            oid.clone()
        } else {
            match compose_git_commits(
                &command.journal_path,
                &format!("assembly-{repository}"),
                source,
                graph_authored_ref(graph, repository)?,
                &packages,
            ) {
                Ok(GitComposition::Clean { base_oid }) => {
                    append_driver_event(
                        &command.journal_path,
                        &DriverEvent::AssemblyRepositoryComposed {
                            repository: repository.clone(),
                            base_oid: base_oid.clone(),
                            packages: packages.clone(),
                        },
                    )?;
                    assembly_refs.insert(repository.clone(), base_oid.clone());
                    base_oid
                }
                Ok(GitComposition::Conflicted {
                    base_oid,
                    conflicting_input,
                    remaining_inputs,
                    conflicted_paths,
                    reason,
                }) => {
                    append_driver_event(
                        &command.journal_path,
                        &DriverEvent::AssemblyJoinConflicted {
                            repository: repository.clone(),
                            packages: packages.clone(),
                            base_oid: base_oid.clone(),
                            conflicting_input: conflicting_input.clone(),
                            remaining_inputs: remaining_inputs.clone(),
                            conflicted_paths: conflicted_paths.clone(),
                            reason: reason.clone(),
                        },
                    )?;
                    let checkout = assembly_root.join(composition_component(repository));
                    let mut resolved_oid = resolve_assembly_conflict(
                        command,
                        repository,
                        source,
                        &checkout,
                        &base_oid,
                        &conflicting_input,
                        &conflicted_paths,
                        &reason,
                        &packages,
                    )?;
                    if !remaining_inputs.is_empty() {
                        match compose_git_commits(
                            &command.journal_path,
                            &format!("assembly-{repository}-remaining"),
                            source,
                            &resolved_oid,
                            &remaining_inputs,
                        )? {
                            GitComposition::Clean { base_oid } => resolved_oid = base_oid,
                            GitComposition::Conflicted {
                                conflicted_paths,
                                reason,
                                ..
                            } => bail!(
                                "assembly resolution exposed a further conflict in {}: {}; {}",
                                repository,
                                conflicted_paths.join(", "),
                                reason
                            ),
                        }
                    }
                    append_driver_event(
                        &command.journal_path,
                        &DriverEvent::AssemblyRepositoryComposed {
                            repository: repository.clone(),
                            base_oid: resolved_oid.clone(),
                            packages: packages.clone(),
                        },
                    )?;
                    assembly_refs.insert(repository.clone(), resolved_oid.clone());
                    resolved_oid
                }
                Err(error) => {
                    append_driver_event(
                        &command.journal_path,
                        &DriverEvent::AssemblyCompositionFailed {
                            repository: repository.clone(),
                            packages,
                            reason: format!("{error:#}"),
                        },
                    )?;
                    return Ok(());
                }
            }
        };
        ensure_assembly_checkout(
            source,
            &assembly_root.join(composition_component(repository)),
            &oid,
        )?;
    }

    events = read_driver_journal(&command.journal_path)?;
    let prior_assembly = active_assembly_events(&events);
    let prior_criteria_are_green = prior_assembly.iter().all(|event| match event {
        DriverEvent::AssemblyCriterionExecuted {
            execution,
            amendment_proof,
            ..
        } => {
            execution.exit_status().is_success()
                && amendment_proof.as_ref().is_none_or(|proof| match proof {
                    AmendmentProof::Reverted { .. } => proof.proves_guard(),
                    AmendmentProof::Unconstructable { .. } => true,
                })
        }
        _ => true,
    });
    if prior_criteria_are_green {
        let stale_candidates = prior_assembly
            .iter()
            .filter_map(|event| match event {
                DriverEvent::AssemblyCriterionExecuted {
                    package,
                    origin: pce_core::CriterionOrigin::Amendment { gate, finding },
                    execution,
                    amendment_proof:
                        Some(AmendmentProof::Unconstructable {
                            repository,
                            repair_ref,
                            ..
                        }),
                    ..
                } if execution.exit_status().is_success() => Some((
                    package.clone(),
                    gate.clone(),
                    *finding,
                    repository.clone(),
                    repair_ref.clone(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (package, gate, finding, repository, repair_ref) in stale_candidates {
            if prior_assembly.iter().any(|event| {
                matches!(
                    event,
                    DriverEvent::RepairCreditStale {
                        package: stale_package,
                        gate: stale_gate,
                        finding: stale_finding,
                        repair_ref: stale_ref,
                        ..
                    } if stale_package == &package && stale_gate == &gate
                        && stale_finding == &finding && stale_ref == &repair_ref
                )
            }) {
                continue;
            }
            let lineage_oid = prior_assembly
                .iter()
                .rev()
                .find_map(|event| match event {
                    DriverEvent::AssemblyRepositoryComposed {
                        repository: composed_repository,
                        base_oid,
                        ..
                    } if composed_repository == &repository => Some(base_oid.clone()),
                    _ => None,
                })
                .with_context(|| format!("assembly omitted repository `{repository}`"))?;
            append_driver_event(
                &command.journal_path,
                &DriverEvent::RepairCreditStale {
                    issuance: completed_package_issuance(&events, &package)?,
                    package,
                    repository,
                    gate,
                    finding,
                    repair_ref,
                    lineage_oid,
                    reason: pce_core::StaleRepairCreditReason::CounterfactualUnconstructable,
                },
            )?;
        }
        events = read_driver_journal(&command.journal_path)?;
    }
    let active_assembly = active_assembly_events(&events);
    let mut remaining_executed = active_assembly
        .iter()
        .filter_map(|event| match event {
            DriverEvent::AssemblyCriterionExecuted {
                package,
                name,
                origin,
                ..
            } => Some((package.clone(), name.clone(), origin.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut failed = active_assembly.iter().any(|event| match event {
        DriverEvent::AssemblyCriterionExecuted {
            origin: pce_core::CriterionOrigin::Authored,
            execution,
            ..
        } => !execution.exit_status().is_success(),
        DriverEvent::AssemblyCriterionExecuted {
            package,
            origin: pce_core::CriterionOrigin::Amendment { gate, finding },
            execution,
            amendment_proof,
            ..
        } => {
            !execution.exit_status().is_success()
                || amendment_proof.as_ref().is_some_and(|proof| match proof {
                    AmendmentProof::Reverted { .. } => !proof.proves_guard(),
                    AmendmentProof::Unconstructable { repair_ref, .. } => {
                        !active_assembly.iter().any(|candidate| {
                            matches!(
                                candidate,
                                DriverEvent::RepairCreditStale {
                                    package: stale_package,
                                    gate: stale_gate,
                                    finding: stale_finding,
                                    repair_ref: stale_ref,
                                    ..
                                } if stale_package == package && stale_gate == gate
                                    && stale_finding == finding && stale_ref == repair_ref
                            )
                        })
                    }
                })
        }
        _ => false,
    });
    for package in graph.packages() {
        let sources = package_repository_sources(package, &command.repositories)?;
        let refs = sources
            .iter()
            .map(|(name, _)| {
                assembly_refs
                    .get(name)
                    .cloned()
                    .map(|oid| (name.clone(), oid))
                    .with_context(|| format!("assembly omitted repository `{name}`"))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let materialization = materialize_driver_state(
            &command.journal_path,
            &format!("assembly-{}", package.id().as_str()),
            &sources,
            &refs,
        )?;
        let paths = materialization.paths()?;
        let named_paths = materialization
            .named_paths(&sources)
            .into_iter()
            .collect::<BTreeMap<_, _>>();
        for (repository, checkout) in materialization.named_paths(&sources) {
            if let Some(preparation) = command.preparations.get(&repository) {
                let preparation_execution = shell_execution_at(preparation, &checkout, &paths)?;
                if !preparation_execution.exit_status().is_success() {
                    append_driver_event(
                        &command.journal_path,
                        &DriverEvent::AssemblyFailed {
                            reason: format!(
                                "assembly environment preparation failed for repository {repository}: {preparation}"
                            ),
                        },
                    )?;
                    return Ok(());
                }
            }
        }
        let current_events = read_driver_journal(&command.journal_path)?;
        for criterion in effective_criteria(graph, package.id().as_str(), &current_events)? {
            if let Some(position) =
                remaining_executed
                    .iter()
                    .position(|(recorded_package, name, origin)| {
                        recorded_package == package.id().as_str()
                            && name == &criterion.name
                            && origin == &criterion.origin
                    })
            {
                remaining_executed.remove(position);
                continue;
            }
            let (execution, mut amendment_proof, mut passed) =
                execute_effective_criterion(&criterion, &paths, &named_paths)?;
            let mut unconstructable = match (&criterion.origin, &amendment_proof) {
                (
                    pce_core::CriterionOrigin::Amendment { gate, finding },
                    Some(AmendmentProof::Unconstructable {
                        repository,
                        repair_ref,
                        detail,
                    }),
                ) => Some((
                    gate.clone(),
                    *finding,
                    repository.clone(),
                    repair_ref.clone(),
                    detail.clone(),
                )),
                _ => None,
            };
            if !failed
                && execution.exit_status().is_success()
                && let Some((gate, finding, repository, repair_ref, _)) = unconstructable.clone()
            {
                let lineage_oid = refs
                    .get(&repository)
                    .cloned()
                    .with_context(|| format!("assembly omitted repository `{repository}`"))?;
                append_driver_event(
                    &command.journal_path,
                    &DriverEvent::RepairCreditStale {
                        package: package.id().as_str().to_owned(),
                        issuance: completed_package_issuance(
                            &current_events,
                            package.id().as_str(),
                        )?,
                        repository,
                        gate,
                        finding,
                        repair_ref,
                        lineage_oid,
                        reason: pce_core::StaleRepairCreditReason::CounterfactualUnconstructable,
                    },
                )?;
                amendment_proof = None;
                passed = true;
                unconstructable = None;
            }
            failed |= !passed;
            append_driver_event(
                &command.journal_path,
                &DriverEvent::AssemblyCriterionExecuted {
                    package: package.id().as_str().to_owned(),
                    name: criterion.name,
                    origin: criterion.origin,
                    execution,
                    amendment_proof,
                },
            )?;
            if let Some((gate, finding, repository, repair_ref, detail)) = unconstructable {
                let source = command
                    .repositories
                    .iter()
                    .find(|(name, _)| name == &repository)
                    .map(|(_, path)| path)
                    .with_context(|| format!("missing repository mapping for `{repository}`"))?;
                let composition = active_assembly_events(&current_events)
                    .iter()
                    .rev()
                    .find_map(|event| match event {
                        DriverEvent::AssemblyRepositoryComposed {
                            repository: recorded,
                            packages,
                            ..
                        } if recorded == &repository => Some(packages.as_slice()),
                        _ => None,
                    })
                    .with_context(|| {
                        format!("assembly omitted composition inputs for `{repository}`")
                    })?;
                let author = assembly_rewrite_author(
                    source,
                    composition,
                    package.id().as_str(),
                    &repair_ref,
                )?
                .with_context(|| {
                    format!(
                        "could not attribute unconstructable repair {repair_ref} for {}",
                        package.id().as_str()
                    )
                })?;
                append_driver_event(
                    &command.journal_path,
                    &DriverEvent::PackageHardeningInvalidated {
                        package: author,
                        hardened_package: package.id().as_str().to_owned(),
                        repository,
                        gate,
                        finding,
                        repair_ref,
                        detail,
                    },
                )?;
                return Ok(());
            }
        }
    }
    if failed {
        append_driver_event(
            &command.journal_path,
            &DriverEvent::AssemblyFailed {
                reason: "one or more effective criteria failed against the composed assembly"
                    .to_owned(),
            },
        )
    } else {
        append_driver_event(&command.journal_path, &DriverEvent::AssemblyCompleted)
    }
}

fn ensure_base_currency_acceptance_imported(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    events: &[DriverEvent],
) -> Result<bool> {
    let version = graph.plan_version();
    let directory = command
        .graph_path
        .parent()
        .context("driver graph has no parent directory")?;
    let sidecar = directory.join(format!("graph.v{version}.base-currency-acceptance.json"));
    let bytes = match fs::read(&sidecar) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to read frozen base-currency acceptance {}",
                    sidecar.display()
                )
            });
        }
    };
    let acceptance = parse_base_currency_acceptance(&bytes)
        .context("failed to parse frozen base-currency acceptance")?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let expected = DriverEvent::BaseCurrencyRiskAccepted {
        plan_version: version,
        entries: acceptance.entries,
        accepted_by: acceptance.accepted_by,
        reason: acceptance.reason,
        sidecar_sha256: digest,
    };
    if events.iter().any(|event| event == &expected) {
        return Ok(false);
    }
    if events.iter().any(|event| {
        matches!(event, DriverEvent::BaseCurrencyRiskAccepted { plan_version, .. } if *plan_version == version)
    }) {
        bail!(
            "driver journal already contains a different base-currency acceptance for plan version {version}"
        );
    }
    let boundary_start = if version == 1 {
        0
    } else {
        events
            .iter()
            .rposition(|event| {
                matches!(event, DriverEvent::PlanVersionAdvanced { to_plan_version, .. } if *to_plan_version == version)
            })
            .context("base-currency acceptance cannot be imported before its plan-version boundary")?
            + 1
    };
    if events[boundary_start..].iter().any(|event| {
        !matches!(
            event,
            DriverEvent::DriverAborted { .. }
                | DriverEvent::DriverResumed
                | DriverEvent::WorkerEnvironmentDeclared { .. }
                | DriverEvent::WorkerEnvironmentExtended { .. }
                | DriverEvent::RecoveryConfigured { .. }
        )
    }) {
        bail!(
            "base-currency acceptance for plan version {version} cannot be imported after plan work began"
        );
    }
    append_driver_event(&command.journal_path, &expected)?;
    Ok(true)
}

fn ensure_driver_plan_version(
    graph: &WorkPackageGraph,
    command: &DriverRunCommand,
    events: &[DriverEvent],
) -> Result<bool> {
    let latest_version = events.iter().rev().find_map(|event| match event {
        DriverEvent::PlanVersionAdvanced {
            to_plan_version, ..
        } => Some(*to_plan_version),
        _ => None,
    });
    let has_plan_work = events.iter().any(|event| {
        !matches!(
            event,
            DriverEvent::RecoveryConfigured { .. } | DriverEvent::PlanVersionAdvanced { .. }
        )
    });
    let Some(from_plan_version) = latest_version
        .or(has_plan_work.then_some(1))
        .or((graph.plan_version() > 1).then_some(graph.plan_version() - 1))
    else {
        return Ok(false);
    };
    if graph.plan_version() < from_plan_version {
        bail!(
            "driver journal is already at plan version {}, newer than supplied graph version {}",
            from_plan_version,
            graph.plan_version()
        );
    }
    if graph.plan_version() == from_plan_version {
        return Ok(false);
    }
    let to_plan_version = from_plan_version
        .checked_add(1)
        .context("driver plan version overflow")?;
    let directory = command
        .graph_path
        .parent()
        .context("driver graph has no parent directory")?;
    let previous_path = directory.join(format!("graph.v{from_plan_version}.json"));
    let previous = read_driver_graph(&previous_path).with_context(|| {
        format!(
            "cannot advance driver journal without frozen predecessor {}",
            previous_path.display()
        )
    })?;
    if previous.plan_version() != from_plan_version || previous.vision() != graph.vision() {
        bail!("frozen predecessor does not match the active driver plan");
    }
    let next = if to_plan_version == graph.plan_version() {
        graph.clone()
    } else {
        let next_path = directory.join(format!("graph.v{to_plan_version}.json"));
        read_driver_graph(&next_path).with_context(|| {
            format!(
                "cannot advance through missing frozen graph {}",
                next_path.display()
            )
        })?
    };
    if next.plan_version() != to_plan_version || next.vision() != graph.vision() {
        bail!("next frozen graph does not form a sequential plan version");
    }
    let violations = criteria_invariance_violations(&previous, &next);
    let revision_path =
        directory.join(format!("graph.v{to_plan_version}.criterion-revisions.json"));
    let (criterion_revisions_ratified_by, criterion_revisions) = if violations.is_empty() {
        if revision_path.try_exists().with_context(|| {
            format!(
                "failed to inspect frozen criterion revision record {}",
                revision_path.display()
            )
        })? {
            bail!(
                "frozen criterion revision record {} exists, but no predecessor criterion changed or was removed",
                revision_path.display()
            );
        }
        (None, Vec::new())
    } else {
        let revision_bytes = fs::read(&revision_path).with_context(|| {
            let affected = violations
                .iter()
                .map(|violation| {
                    format!(
                        "{}::{:?}",
                        violation.previous_package().as_str(),
                        violation.criterion().name()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "criteria changed or were removed ({affected}); the real exits are additive world-conformance or a human-ratified revision recorded at freeze in {}",
                revision_path.display()
            )
        })?;
        let manifest = parse_criterion_revision_manifest(&revision_bytes)
            .context("failed to parse frozen human criterion revision record")?;
        validate_criterion_revisions(&previous, &next, manifest.revisions())
            .context("frozen human criterion revision record does not match the plan transition")?;
        (
            Some(manifest.ratified_by().to_owned()),
            manifest.revisions().to_vec(),
        )
    };
    let previous_snapshot =
        derive_driver_snapshot(&previous, events, command.override_risk_ordering)
            .context("failed to derive predecessor plan before advancing")?;
    if previous_snapshot.packages().iter().any(|(_, state)| {
        matches!(
            state,
            pce_core::DriverPackageState::Running { .. }
                | pce_core::DriverPackageState::Judging { .. }
        )
    }) {
        bail!("cannot advance a driver plan while a package attempt is running or judging");
    }
    let mut unchanged = unchanged_package_ids(&previous, &next)
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    loop {
        let revised_dependents = next
            .packages()
            .iter()
            .filter(|package| unchanged.contains(package.id().as_str()))
            .filter(|package| {
                package
                    .depends_on()
                    .iter()
                    .any(|dependency| !unchanged.contains(dependency.id().as_str()))
            })
            .map(|package| package.id().as_str().to_owned())
            .collect::<Vec<_>>();
        if revised_dependents.is_empty() {
            break;
        }
        for package in revised_dependents {
            unchanged.remove(&package);
        }
    }
    let carried_completions: Vec<String> = previous_snapshot
        .packages()
        .iter()
        .filter(|(package, state)| {
            unchanged.contains(package) && matches!(state, pce_core::DriverPackageState::Complete)
        })
        .map(|(package, _)| package.clone())
        .collect();
    let next_packages = next
        .packages()
        .iter()
        .map(|package| (package.id().as_str(), package))
        .collect::<BTreeMap<_, _>>();
    // A forfeited completion starts a new package attempt: amendment criteria carry, but the
    // prior attempt's repair commits do not. The fresh lineage must satisfy each carried command;
    // hardening records and drops any repair credit that cannot fast-forward that lineage.
    let mut carried_amendments = Vec::new();
    for (package, amendment) in previous_snapshot.amendments() {
        let successor_package = next_packages.get(package.as_str()).with_context(|| {
            format!(
                "cannot remove package {package} while its gate-earned amendments remain durable; an explicit amendment portability ruling is required"
            )
        })?;
        if amendment.repository_refs.iter().any(|reference| {
            !successor_package
                .repositories()
                .contains(&reference.repository)
        }) {
            bail!(
                "cannot narrow repository scope for package {package} while its gate-earned amendment references a removed repository"
            );
        }
        carried_amendments.push((package.clone(), amendment.clone()));
    }
    append_driver_event(
        &command.journal_path,
        &DriverEvent::PlanVersionAdvanced {
            from_plan_version,
            to_plan_version,
            carried_completions,
            carried_amendments,
            criterion_revisions_ratified_by,
            criterion_revisions,
        },
    )?;
    Ok(true)
}

fn run_driver_loop(command: DriverRunCommand) -> Result<()> {
    require_herdr_session_running(command.herdr_session.as_ref())?;
    let journal_path = command.journal_path.clone();
    let result = run_driver_loop_inner(command);
    if let Err(error) = &result {
        let reason = format!("{error:#}");
        if let Err(append_error) =
            append_driver_event(&journal_path, &DriverEvent::DriverAborted { reason })
        {
            tracing::error!(
                error = %append_error,
                original_error = %error,
                journal = %journal_path.display(),
                "failed to record driver abort"
            );
        }
    }
    result
}

fn run_driver_loop_inner(command: DriverRunCommand) -> Result<()> {
    let initial_events = read_driver_journal(&command.journal_path)?;
    let latest_driver_lifecycle = initial_events.iter().rev().find(|event| {
        matches!(
            event,
            DriverEvent::DriverAborted { .. } | DriverEvent::DriverResumed
        )
    });
    if matches!(
        latest_driver_lifecycle,
        Some(DriverEvent::DriverAborted { .. })
    ) {
        append_driver_event(&command.journal_path, &DriverEvent::DriverResumed)?;
    }
    let initial_graph = read_driver_graph(&command.graph_path)?;
    verify_graph_repository_refs(&initial_graph, &command.repositories)?;
    ensure_recovery_configuration(&command, &initial_events)?;
    let mut issued_this_launch = HashSet::new();
    loop {
        let graph = read_driver_graph(&command.graph_path)?;
        verify_graph_repository_refs(&graph, &command.repositories)?;
        let events = read_driver_journal(&command.journal_path)?;
        if ensure_driver_plan_version(&graph, &command, &events)? {
            continue;
        }
        if ensure_base_currency_acceptance_imported(&graph, &command, &events)? {
            continue;
        }
        ensure_worker_environment_contract(&command, &events)?;
        let events = read_driver_journal(&command.journal_path)?;
        if repair_driver_environment_closures(&command, &events)? > 0 {
            continue;
        }
        if reconcile_completed_dispatch_panes(&command.journal_path, &events)? > 0 {
            continue;
        }
        let snapshot = derive_driver_snapshot(&graph, &events, command.override_risk_ordering)
            .context("failed to derive driver loop state")?;
        let all_packages_complete = snapshot
            .packages()
            .iter()
            .all(|(_, state)| matches!(state, pce_core::DriverPackageState::Complete));
        if all_packages_complete
            && matches!(
                snapshot.assembly(),
                DriverAssemblyState::Pending | DriverAssemblyState::Gating
            )
        {
            run_driver_assembly(&graph, &command)?;
            continue;
        }
        let exhausted = snapshot
            .packages()
            .iter()
            .filter(|(_, state)| matches!(state, pce_core::DriverPackageState::Failed { .. }))
            .map(|(package, _)| package.clone())
            .collect::<Vec<_>>();
        if !exhausted.is_empty() {
            for package in exhausted {
                park_if_recovery_exhausted(&command, &package, None)?;
            }
            continue;
        }
        match snapshot.outcome() {
            pce_core::DriverLoopOutcome::Finished => {
                materialize_driver_refs(&graph, &command.journal_path, &command.repositories)
                    .context("failed to materialize journal-proven terminal refs")?;
                return run_driver_status(DriverStatusCommand {
                    graph_path: command.graph_path,
                    journal_path: command.journal_path,
                    override_risk_ordering: command.override_risk_ordering,
                });
            }
            pce_core::DriverLoopOutcome::Blocked => {
                return run_driver_status(DriverStatusCommand {
                    graph_path: command.graph_path,
                    journal_path: command.journal_path,
                    override_risk_ordering: command.override_risk_ordering,
                });
            }
            pce_core::DriverLoopOutcome::Running => {}
        }
        let ready = snapshot.ready().to_vec();
        if ready.is_empty() {
            if command.worker_override.is_none() {
                let vision_dir = driver_vision_directory(&command)?;
                let dispatch_log = driver_dispatch_log(&command)?;
                let records = read_event_log(&dispatch_log)?
                    .into_iter()
                    .map(|line| line.record)
                    .collect::<Vec<_>>();
                let ledger = fold_dispatch_ledger(&records)
                    .context("failed to derive driver dispatch ledger")?;
                let mut running_results = Vec::new();
                for (package, state) in snapshot.packages() {
                    if let pce_core::DriverPackageState::Running { issuance } = state
                        && let Some(entry) = ledger
                            .unaccounted()
                            .entries()
                            .iter()
                            .rev()
                            .find(|entry| entry.node().as_str() == package)
                    {
                        running_results.push((
                            package.clone(),
                            *issuance,
                            entry.sequence(),
                            driver_outcome_path(&command.journal_path, package, *issuance)?,
                            package_result_path(&vision_dir, entry.node(), entry.sequence())?,
                        ));
                    }
                }
                if !running_results.is_empty() {
                    let events = read_driver_journal(&command.journal_path)?;
                    let mut retained = Vec::new();
                    let mut closed = false;
                    for (package, issuance, dispatch_sequence, outcome, result_path) in
                        running_results
                    {
                        if result_path.is_file()
                            || issued_this_launch.contains(&(package.clone(), issuance))
                        {
                            retained.push((
                                package,
                                issuance,
                                dispatch_sequence,
                                outcome,
                                result_path,
                            ));
                            continue;
                        }
                        match observe_driver_dispatch_environment(
                            &graph, &events, &package, issuance,
                        )? {
                            DriverDispatchEnvironmentLiveness::Unknown
                            | DriverDispatchEnvironmentLiveness::Alive => retained.push((
                                package,
                                issuance,
                                dispatch_sequence,
                                outcome,
                                result_path,
                            )),
                            DriverDispatchEnvironmentLiveness::Dead { detail } => {
                                close_driver_lost_dispatch(
                                    &command,
                                    &package,
                                    issuance,
                                    dispatch_sequence,
                                    DispatchEnvironmentObservation::Dead,
                                    detail,
                                    OBSERVED_DEAD_DISPATCH_REASON,
                                )?;
                                closed = true;
                            }
                            DriverDispatchEnvironmentLiveness::Inconclusive { detail } => {
                                close_driver_lost_dispatch(
                                    &command,
                                    &package,
                                    issuance,
                                    dispatch_sequence,
                                    DispatchEnvironmentObservation::Inconclusive,
                                    detail,
                                    INCONCLUSIVE_DISPATCH_REASON,
                                )?;
                                closed = true;
                            }
                        }
                    }
                    if closed {
                        continue;
                    }
                    let paths = retained
                        .iter()
                        .map(|(_, _, _, _, path)| path.clone())
                        .collect::<Vec<_>>();
                    if !wait_for_driver_results(&paths, command.wait_timeout)? {
                        let waited_ms =
                            u64::try_from(command.wait_timeout.unwrap_or_default().as_millis())
                                .context("driver wait timeout exceeds u64")?;
                        let timeout_events = read_driver_journal(&command.journal_path)?;
                        let mut timeout_closed = false;
                        for (package, issuance, dispatch_sequence, _, path) in &retained {
                            if path.is_file() {
                                continue;
                            }
                            match observe_driver_dispatch_environment(
                                &graph,
                                &timeout_events,
                                package,
                                *issuance,
                            )? {
                                DriverDispatchEnvironmentLiveness::Unknown => {}
                                DriverDispatchEnvironmentLiveness::Alive => append_driver_event(
                                    &command.journal_path,
                                    &DriverEvent::DriverStoppedWaiting {
                                        package: package.clone(),
                                        issuance: *issuance,
                                        waited_ms,
                                    },
                                )?,
                                DriverDispatchEnvironmentLiveness::Dead { detail } => {
                                    close_driver_lost_dispatch(
                                        &command,
                                        package,
                                        *issuance,
                                        *dispatch_sequence,
                                        DispatchEnvironmentObservation::Dead,
                                        detail,
                                        OBSERVED_DEAD_DISPATCH_REASON,
                                    )?;
                                    timeout_closed = true;
                                }
                                DriverDispatchEnvironmentLiveness::Inconclusive { detail } => {
                                    close_driver_lost_dispatch(
                                        &command,
                                        package,
                                        *issuance,
                                        *dispatch_sequence,
                                        DispatchEnvironmentObservation::Inconclusive,
                                        detail,
                                        INCONCLUSIVE_DISPATCH_REASON,
                                    )?;
                                    timeout_closed = true;
                                }
                            }
                        }
                        if timeout_closed {
                            continue;
                        }
                        return run_driver_status(DriverStatusCommand {
                            graph_path: command.graph_path,
                            journal_path: command.journal_path,
                            override_risk_ordering: command.override_risk_ordering,
                        });
                    }
                    let _completions = collect_package_completions(&dispatch_log, &vision_dir)?;
                    for (package, issuance, _, outcome, result_path) in retained {
                        let result = read_package_result(&result_path)?
                            .context("durable dispatch result vanished")?;
                        let healthy = matches!(result.exit_status(), DispatchExitStatus::Exited { code } if code.get() == 0)
                            && result.required_artifact_presence()
                                == RequiredArtifactPresence::Present;
                        if healthy {
                            let _observed = observe_driver_worker_outcome(
                                &graph, &command, package, issuance, &outcome,
                            )?;
                        } else {
                            append_worker_environment_outcome(
                                &command,
                                package,
                                issuance,
                                format!(
                                    "package dispatch stopped without a successful required artifact: {:?}",
                                    result.exit_status()
                                ),
                            )?;
                        }
                    }
                    continue;
                }
                if let Some((package, pce_core::DriverPackageState::Judging { issuance })) =
                    snapshot.packages().iter().find(|(_, state)| {
                        matches!(state, pce_core::DriverPackageState::Judging { .. })
                    })
                {
                    run_composed_driver_gate(&graph, &command, package, *issuance)?;
                    continue;
                }
                return run_driver_status(DriverStatusCommand {
                    graph_path: command.graph_path,
                    journal_path: command.journal_path,
                    override_risk_ordering: command.override_risk_ordering,
                });
            }
            let mut observed = false;
            for (package, state) in snapshot.packages() {
                if let pce_core::DriverPackageState::Running { issuance } = state {
                    let outcome_path =
                        driver_outcome_path(&command.journal_path, package, *issuance)?;
                    observed |= observe_driver_worker_outcome(
                        &graph,
                        &command,
                        package.clone(),
                        *issuance,
                        &outcome_path,
                    )?;
                }
            }
            if !observed {
                return run_driver_status(DriverStatusCommand {
                    graph_path: command.graph_path,
                    journal_path: command.journal_path,
                    override_risk_ordering: command.override_risk_ordering,
                });
            }
            continue;
        }
        let next_issuance = events
            .iter()
            .filter_map(|event| match event {
                DriverEvent::WorkerDispatched { issuance, .. } => Some(*issuance),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .context("driver issuance space is exhausted")?;
        for (offset, package_id) in ready.iter().enumerate() {
            let issuance = next_issuance
                .checked_add(u64::try_from(offset).context("ready set exceeds u64")?)
                .context("driver issuance space is exhausted")?;
            let outcome = driver_outcome_path(&command.journal_path, package_id, issuance)?;
            match fs::symlink_metadata(&outcome) {
                Ok(_) => bail!(
                    "refusing to dispatch package {package_id} issuance {issuance}: outcome path already exists at {}",
                    outcome.display()
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to inspect outcome path {}", outcome.display())
                    });
                }
            }
        }
        let mut override_children = Vec::new();
        let mut composed_dispatches = Vec::new();
        let mut spawn_failed = false;
        // The full ready antichain is issued before waiting for any member.
        for (offset, package_id) in ready.iter().enumerate() {
            let issuance = next_issuance
                .checked_add(u64::try_from(offset).context("ready set exceeds u64")?)
                .context("driver issuance space is exhausted")?;
            let outcome = driver_outcome_path(&command.journal_path, package_id, issuance)?;
            fs::create_dir_all(outcome.parent().context("outcome path has no parent")?)?;
            if command.worker_override.is_none()
                && !ensure_driver_package_bases(&graph, &command, package_id, issuance)?
            {
                continue;
            }
            let charged = charged_failure_count(&events, package_id);
            let base_brief = recovery_base_brief(&graph, package_id)?;
            let (rung_name, worker_brief) = if charged == 0 {
                ("initial", base_brief)
            } else {
                let budget = recovery_budget(command.recovery_limits, charged);
                let evidence = latest_criterion_failure_evidence(&events, package_id);
                let brief = match budget.next_rung {
                    RecoveryRung::Retry => base_brief,
                    RecoveryRung::LocalPatch => compose_local_patch_brief(&base_brief, &evidence),
                    RecoveryRung::Replan => {
                        park_if_recovery_exhausted(&command, package_id, None)?;
                        continue;
                    }
                };
                append_driver_event(
                    &command.journal_path,
                    &DriverEvent::RecoveryRungAttempted {
                        package: package_id.clone(),
                        issuance,
                        rung: budget.next_rung,
                        evidence,
                        brief: brief.clone(),
                    },
                )?;
                (
                    match budget.next_rung {
                        RecoveryRung::Retry => "retry",
                        RecoveryRung::LocalPatch => "local-patch",
                        RecoveryRung::Replan => unreachable!("handled above"),
                    },
                    brief,
                )
            };
            append_driver_event(
                &command.journal_path,
                &DriverEvent::WorkerDispatched {
                    package: package_id.clone(),
                    issuance,
                },
            )?;
            if let Some(worker_arguments) = &command.worker_override {
                let (program, arguments) = worker_arguments
                    .split_first()
                    .context("driver worker override is empty")?;
                match std::process::Command::new(program)
                    .args(arguments)
                    .env("PCE_PACKAGE", package_id)
                    .env("PCE_PACKAGE_OUTCOME", &outcome)
                    .env("PCE_RECOVERY_RUNG", rung_name)
                    .env("PCE_PACKAGE_BRIEF", worker_brief)
                    .spawn()
                {
                    Ok(child) => {
                        override_children.push((package_id.clone(), issuance, outcome, child));
                    }
                    Err(source) => {
                        spawn_failed = true;
                        append_driver_event(
                            &command.journal_path,
                            &DriverEvent::WorkerSpawnFailed {
                                package: package_id.clone(),
                                issuance,
                                reason: format!("failed to spawn worker override: {source}"),
                            },
                        )?;
                    }
                }
            } else {
                let evidence = (rung_name == "local-patch")
                    .then(|| latest_criterion_failure_evidence(&events, package_id));
                let composed_brief = compose_driver_worker_brief(
                    &command,
                    &graph,
                    package_id,
                    issuance,
                    evidence.as_deref(),
                )?;
                match issue_driver_package_dispatch(
                    &command,
                    &graph,
                    package_id,
                    issuance,
                    &outcome,
                    &composed_brief,
                ) {
                    Ok(result) => {
                        issued_this_launch.insert((package_id.clone(), issuance));
                        composed_dispatches.push((package_id.clone(), issuance, outcome, result));
                    }
                    Err(source) => {
                        if source
                            .downcast_ref::<SpawnObservedDispatchError>()
                            .is_some()
                        {
                            return Err(source).context(format!(
                                "worker spawn for {package_id} was observed but could not be recorded"
                            ));
                        }
                        spawn_failed = true;
                        append_driver_event(
                            &command.journal_path,
                            &DriverEvent::WorkerSpawnFailed {
                                package: package_id.clone(),
                                issuance,
                                reason: format!("worker spawn failed: {source:#}"),
                            },
                        )?;
                    }
                }
            }
        }
        for (package_id, issuance, outcome_path, mut child) in override_children {
            let status = child
                .wait()
                .with_context(|| format!("failed to wait for worker {package_id}"))?;
            if !status.success() {
                append_worker_environment_outcome(
                    &command,
                    package_id,
                    issuance,
                    format!(
                        "worker process exited with {status}; no package judgement was produced"
                    ),
                )?;
                continue;
            }
            if !observe_driver_worker_outcome(
                &graph,
                &command,
                package_id,
                issuance,
                &outcome_path,
            )? {
                bail!(
                    "successful worker wrote no outcome at {}",
                    outcome_path.display()
                );
            }
        }
        if !composed_dispatches.is_empty() {
            let paths = composed_dispatches
                .iter()
                .map(|(_, _, _, result)| result.clone())
                .collect::<Vec<_>>();
            if !wait_for_driver_results(&paths, command.wait_timeout)? {
                let waited_ms = u64::try_from(command.wait_timeout.unwrap_or_default().as_millis())
                    .context("driver wait timeout exceeds u64")?;
                for (package, issuance, _, result) in &composed_dispatches {
                    if !result.is_file() {
                        append_driver_event(
                            &command.journal_path,
                            &DriverEvent::DriverStoppedWaiting {
                                package: package.clone(),
                                issuance: *issuance,
                                waited_ms,
                            },
                        )?;
                    }
                }
                return run_driver_status(DriverStatusCommand {
                    graph_path: command.graph_path,
                    journal_path: command.journal_path,
                    override_risk_ordering: command.override_risk_ordering,
                });
            }
            let vision_dir = driver_vision_directory(&command)?;
            let dispatch_log = driver_dispatch_log(&command)?;
            let _completions = collect_package_completions(&dispatch_log, &vision_dir)?;
            for (package_id, issuance, outcome_path, result_path) in composed_dispatches {
                let result = read_package_result(&result_path)?.with_context(|| {
                    format!(
                        "notified package result vanished: {}",
                        result_path.display()
                    )
                })?;
                let healthy = matches!(result.exit_status(), DispatchExitStatus::Exited { code } if code.get() == 0)
                    && result.required_artifact_presence() == RequiredArtifactPresence::Present;
                if !healthy {
                    append_worker_environment_outcome(
                        &command,
                        package_id,
                        issuance,
                        format!(
                            "package dispatch stopped without a successful required artifact: {:?}",
                            result.exit_status()
                        ),
                    )?;
                    continue;
                }
                if !observe_driver_worker_outcome(
                    &graph,
                    &command,
                    package_id,
                    issuance,
                    &outcome_path,
                )? {
                    bail!(
                        "package dispatch result exists without outcome {}",
                        outcome_path.display()
                    );
                }
            }
        }
        if spawn_failed {
            return run_driver_status(DriverStatusCommand {
                graph_path: command.graph_path,
                journal_path: command.journal_path,
                override_risk_ordering: command.override_risk_ordering,
            });
        }
    }
}

fn read_driver_journal(path: &Path) -> Result<Vec<DriverEvent>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read driver journal {}", path.display()));
        }
    };
    let mut events = Vec::new();
    let lines = bytes.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        match serde_json::from_slice::<DriverEvent>(line) {
            Ok(event) => events.push(event),
            Err(_) if index + 1 == lines.len() && !bytes.ends_with(b"\n") => break,
            Err(source) => {
                return Err(source).with_context(|| {
                    format!(
                        "invalid driver journal line {} in {}",
                        index + 1,
                        path.display()
                    )
                });
            }
        }
    }
    Ok(events)
}

fn append_driver_event(path: &Path, event: &DriverEvent) -> Result<()> {
    let parent = path.parent().context("driver journal path has no parent")?;
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let mut bytes = serde_json::to_vec(event).context("failed to serialize driver event")?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .with_context(|| format!("failed to open driver journal {}", path.display()))?;
    file.write_all(&bytes)
        .context("failed to append driver event")?;
    file.sync_all().context("failed to sync driver journal")
}

fn read_driver_graph(path: &Path) -> Result<WorkPackageGraph> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read graph {}", path.display()))?;
    parse_work_package_graph(&bytes).context("failed to parse driver graph")
}

fn run_package_render(command: PackageRenderCommand) -> Result<()> {
    let graph = read_driver_graph(&command.graph_path)?;
    let events = match &command.journal_path {
        Some(path) => {
            fs::metadata(path)
                .with_context(|| format!("failed to read render journal {}", path.display()))?;
            read_driver_journal(path)
                .with_context(|| format!("failed to read render journal {}", path.display()))?
        }
        None => Vec::new(),
    };
    let html = render_package_run(&graph, &events).context("failed to render package run")?;
    fs::write(&command.output_path, html).with_context(|| {
        format!(
            "failed to write rendered package run {}",
            command.output_path.display()
        )
    })
}

#[derive(Debug, Clone)]
struct DriverRefIntent {
    repository: String,
    reference: String,
    oid: String,
    product: DriverRefProduct,
}

fn derive_driver_ref_intents(
    graph: &WorkPackageGraph,
    events: &[DriverEvent],
) -> Result<Vec<DriverRefIntent>> {
    let snapshot = derive_driver_snapshot(graph, events, false)
        .context("cannot materialize refs from an invalid driver journal")?;
    if !matches!(snapshot.outcome(), pce_core::DriverLoopOutcome::Finished)
        || !matches!(snapshot.assembly(), DriverAssemblyState::Complete)
    {
        bail!("driver refs require a Finished journal with a complete assembly");
    }

    let active = active_assembly_events(events);
    let mut composed = BTreeMap::<String, (String, Vec<CompositionInput>)>::new();
    let mut resolutions = Vec::<(String, String)>::new();
    for event in active {
        match event {
            DriverEvent::AssemblyRepositoryComposed {
                repository,
                base_oid,
                packages,
            } => {
                composed.insert(repository.clone(), (base_oid.clone(), packages.clone()));
            }
            DriverEvent::AssemblyResolutionDone {
                repository,
                base_oid,
            } => resolutions.push((repository.clone(), base_oid.clone())),
            _ => {}
        }
    }
    if composed.is_empty() {
        bail!("finished driver journal has no final assembly repository");
    }

    let mut intents = Vec::new();
    for (repository, (_, packages)) in &composed {
        for package in packages {
            let issuance = completed_package_issuance(events, &package.package)?;
            intents.push(DriverRefIntent {
                repository: repository.clone(),
                reference: format!(
                    "refs/heads/{}",
                    package_branch(graph, &package.package, issuance)
                ),
                oid: package.oid.clone(),
                product: DriverRefProduct::PackageAttempt {
                    package: package.package.clone(),
                    issuance,
                },
            });
        }
    }
    for (repository, oid) in resolutions {
        if !composed.contains_key(&repository) {
            bail!("resolution journal names uncomposed repository `{repository}`");
        }
        intents.push(DriverRefIntent {
            reference: format!(
                "refs/pce-assembly-resolutions/{}/{}",
                composition_component(&repository),
                oid
            ),
            repository,
            oid,
            product: DriverRefProduct::AssemblyResolution,
        });
    }
    for (repository, (oid, _)) in composed {
        intents.push(DriverRefIntent {
            repository,
            reference: format!(
                "refs/heads/pce/{}/assembly-v{}",
                graph.vision(),
                graph.plan_version()
            ),
            oid,
            product: DriverRefProduct::Assembly,
        });
    }
    Ok(intents)
}

fn materialize_driver_refs(
    graph: &WorkPackageGraph,
    journal: &Path,
    repositories: &[(String, PathBuf)],
) -> Result<(usize, usize)> {
    let events = read_driver_journal(journal)?;
    let intents = derive_driver_ref_intents(graph, &events)?;
    let sources = repositories
        .iter()
        .map(|(name, path)| (name.as_str(), path.as_path()))
        .collect::<BTreeMap<_, _>>();
    if sources.len() != repositories.len() {
        bail!("ref materialization repository mappings must be unique");
    }

    let mut observed = Vec::with_capacity(intents.len());
    for intent in &intents {
        let source = sources
            .get(intent.repository.as_str())
            .with_context(|| format!("missing repository mapping for `{}`", intent.repository))?;
        let target = git_oid(source, &intent.oid).with_context(|| {
            format!(
                "journal-proven oid {} is unavailable in repository `{}`",
                intent.oid, intent.repository
            )
        })?;
        if target != intent.oid {
            bail!("journal oid {} did not resolve exactly", intent.oid);
        }
        let current = git_oid_if_available(source, &intent.reference)?;
        if let Some(current) = &current
            && current != &intent.oid
        {
            bail!(
                "ref {} in repository `{}` is at {}, expected {}; refusing to move it",
                intent.reference,
                intent.repository,
                current,
                intent.oid
            );
        }
        observed.push(current);
    }

    let mut created = 0_usize;
    let mut already_correct = 0_usize;
    for (intent, current) in intents.iter().zip(observed) {
        if current.is_some() {
            already_correct += 1;
            continue;
        }
        let source = sources
            .get(intent.repository.as_str())
            .with_context(|| format!("missing repository mapping for `{}`", intent.repository))?;
        let update = std::process::Command::new("git")
            .arg("-C")
            .arg(source)
            .args(["update-ref", &intent.reference, &intent.oid, ""])
            .output()?;
        if !update.status.success() {
            bail!(
                "failed to create ref {} at {} without movement: {}",
                intent.reference,
                intent.oid,
                String::from_utf8_lossy(&update.stderr).trim()
            );
        }
        append_driver_event(
            journal,
            &DriverEvent::DriverRefMaterialized {
                repository: intent.repository.clone(),
                reference: intent.reference.clone(),
                oid: intent.oid.clone(),
                product: intent.product.clone(),
            },
        )?;
        created += 1;
    }
    Ok((created, already_correct))
}

fn run_materialize_refs(command: MaterializeRefsCommand) -> Result<()> {
    let graph = read_driver_graph(&command.graph_path)?;
    let (created, already_correct) =
        materialize_driver_refs(&graph, &command.journal_path, &command.repositories)?;
    write_json_stdout(&json!({
        "created": created,
        "already_correct": already_correct,
        "changed": created > 0,
    }))
}

fn run_driver_status(command: DriverStatusCommand) -> Result<()> {
    let graph = read_driver_graph(&command.graph_path)?;
    let events = read_driver_journal(&command.journal_path)?;
    let snapshot = derive_driver_snapshot(&graph, &events, command.override_risk_ordering)
        .context("failed to derive driver state")?;
    write_json_stdout(&serde_json::to_value(snapshot).context("failed to serialize driver state")?)
}

fn run_driver_overrule(command: DriverOverruleCommand) -> Result<()> {
    let graph = read_driver_graph(&command.graph_path)?;
    let events = read_driver_journal(&command.journal_path)?;
    if graph.plan_version() > 1 {
        let active_version = events.iter().rev().find_map(|event| match event {
            DriverEvent::PlanVersionAdvanced {
                to_plan_version, ..
            } => Some(*to_plan_version),
            _ => None,
        });
        if active_version != Some(graph.plan_version()) {
            bail!(
                "plan version {} is not active in the journal; run driver-run before adjudicating",
                graph.plan_version()
            );
        }
    }
    let event = DriverEvent::PackageParkOverruled {
        package: command.package_id,
        plan_version: graph.plan_version(),
        rationale: command.rationale,
    };
    let mut candidate = events;
    candidate.push(event.clone());
    derive_driver_snapshot(&graph, &candidate, false).context(
        "park overrule refused; graph revision is the only exit after a repeated dispute",
    )?;
    append_driver_event(&command.journal_path, &event)?;
    run_driver_status(DriverStatusCommand {
        graph_path: command.graph_path,
        journal_path: command.journal_path,
        override_risk_ordering: false,
    })
}

fn package_repository_sources(
    package: &pce_core::WorkPackage,
    repositories: &[(String, PathBuf)],
) -> Result<Vec<(String, PathBuf)>> {
    let by_name = repositories
        .iter()
        .map(|(name, path)| (name.as_str(), path.as_path()))
        .collect::<BTreeMap<_, _>>();
    if by_name.len() != repositories.len() {
        bail!("driver repository mappings must be unique");
    }
    package
        .repositories()
        .iter()
        .map(|name| {
            by_name
                .get(name.as_str())
                .map(|path| (name.clone(), (*path).to_path_buf()))
                .with_context(|| format!("driver omitted repository mapping `{name}`"))
        })
        .collect()
}

fn graph_authored_ref<'a>(graph: &'a WorkPackageGraph, repository: &str) -> Result<&'a str> {
    graph
        .authored_ref(repository)
        .with_context(|| format!("repository `{repository}` has no authored ref"))
}

fn package_authored_refs(
    graph: &WorkPackageGraph,
    package: &pce_core::WorkPackage,
) -> Result<String> {
    package
        .repositories()
        .iter()
        .map(|repository| {
            graph_authored_ref(graph, repository)
                .map(|authored_ref| format!("{repository}={authored_ref}"))
        })
        .collect::<Result<Vec<_>>>()
        .map(|refs| refs.join(","))
}

fn verify_graph_repository_refs(
    graph: &WorkPackageGraph,
    repositories: &[(String, PathBuf)],
) -> Result<()> {
    if repositories.is_empty() {
        return Ok(());
    }
    for (name, _) in repositories {
        if !graph.authored_refs().contains_key(name) {
            bail!("repository mapping `{name}` is not named by the graph");
        }
    }
    for name in graph.authored_refs().keys() {
        let source = repositories
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, path)| path)
            .with_context(|| format!("missing repository mapping for `{name}`"))?;
        let authored_ref = graph
            .authored_ref(name)
            .with_context(|| format!("repository `{name}` has no authored ref"))?;
        git_oid(source, authored_ref).with_context(|| {
            format!("repository `{name}` authored ref `{authored_ref}` does not resolve")
        })?;
    }
    Ok(())
}

fn git_oid_if_available(repository: &Path, reference: &str) -> Result<Option<String>> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(repository)
        .args([
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ])
        .output()
        .with_context(|| {
            format!(
                "failed to inspect repair `{reference}` in {}",
                repository.display()
            )
        })?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8(output.stdout)
            .context("git returned non-UTF-8 oid")?
            .trim()
            .to_owned(),
    ))
}

fn git_oid(repository: &Path, reference: &str) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(repository)
        .args([
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ])
        .output()
        .with_context(|| {
            format!(
                "failed to resolve `{reference}` in {}",
                repository.display()
            )
        })?;
    if !output.status.success() {
        bail!(
            "git could not resolve `{reference}` in {}: {}",
            repository.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)
        .context("git returned non-UTF-8 oid")?
        .trim()
        .to_owned())
}

fn remove_clean_worktree(source: &Path, worktree: &Path) -> Result<()> {
    if !worktree.exists() {
        return Ok(());
    }
    let status = std::process::Command::new("git")
        .args(["-C"])
        .arg(worktree)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
        .with_context(|| format!("failed to inspect worktree {}", worktree.display()))?;
    if !status.status.success() {
        bail!(
            "git could not inspect worktree {}: {}",
            worktree.display(),
            String::from_utf8_lossy(&status.stderr).trim()
        );
    }
    if !status.stdout.is_empty() {
        return Ok(());
    }
    let removed = std::process::Command::new("git")
        .args(["-C"])
        .arg(source)
        .args(["worktree", "remove"])
        .arg(worktree)
        .output()
        .with_context(|| format!("failed to remove worktree {}", worktree.display()))?;
    if !removed.status.success() {
        bail!(
            "git could not remove clean worktree {}: {}",
            worktree.display(),
            String::from_utf8_lossy(&removed.stderr).trim()
        );
    }
    if let Some(attempt_root) = worktree.parent() {
        match fs::remove_dir(attempt_root) {
            Ok(()) => {}
            Err(source)
                if matches!(
                    source.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(source) => {
                return Err(source).with_context(|| {
                    format!(
                        "failed to remove empty attempt root {}",
                        attempt_root.display()
                    )
                });
            }
        }
    }
    Ok(())
}

fn remove_clean_created_worktrees(
    worktrees: &[HerdrWorktreeSpec],
    repositories: &[(String, PathBuf)],
) -> Result<()> {
    for worktree in worktrees {
        let source = repositories
            .iter()
            .find(|(name, _)| name == worktree.repository())
            .map(|(_, path)| path)
            .with_context(|| {
                format!(
                    "dispatch omitted repository mapping `{}`",
                    worktree.repository()
                )
            })?;
        remove_clean_worktree(source, worktree.path())?;
    }
    Ok(())
}

fn remove_clean_driver_worktrees(
    command: &DriverRunCommand,
    graph: &WorkPackageGraph,
    package_id: &str,
    issuance: u64,
) -> Result<()> {
    for worktree in driver_package_worktrees(command, graph, package_id, issuance)? {
        let source = command
            .repositories
            .iter()
            .find(|(name, _)| name == worktree.repository())
            .map(|(_, path)| path)
            .with_context(|| {
                format!(
                    "driver omitted repository mapping `{}`",
                    worktree.repository()
                )
            })?;
        remove_clean_worktree(source, worktree.path())?;
    }
    Ok(())
}

fn remove_clean_gate_worktrees(
    response: &Value,
    implementation_worktrees: &[RepositoryWorktree],
) -> Result<()> {
    let worktrees = response["worktrees"]
        .as_array()
        .context("gate dispatch omitted worktrees")?;
    for worktree in worktrees {
        let repository = worktree["repository"]
            .as_str()
            .context("gate dispatch worktree omitted repository")?;
        let path = Path::new(
            worktree["path"]
                .as_str()
                .context("gate dispatch worktree omitted path")?,
        );
        let source = implementation_worktrees
            .iter()
            .find(|candidate| candidate.repository() == repository)
            .map(RepositoryWorktree::path)
            .with_context(|| format!("gate worktree names unknown repository `{repository}`"))?;
        remove_clean_worktree(source, path)?;
    }
    Ok(())
}

struct DriverMaterialization {
    root: PathBuf,
}
impl Drop for DriverMaterialization {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
impl DriverMaterialization {
    fn named_paths(&self, sources: &[(String, PathBuf)]) -> Vec<(String, PathBuf)> {
        sources
            .iter()
            .enumerate()
            .map(|(index, (name, _))| {
                (
                    name.clone(),
                    self.root.join(format!("{index:02}-repository")),
                )
            })
            .collect()
    }

    fn paths(&self) -> Result<Vec<PathBuf>> {
        let mut entries = fs::read_dir(&self.root)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        Ok(entries)
    }
}

fn materialize_driver_state(
    journal: &Path,
    label: &str,
    sources: &[(String, PathBuf)],
    references: &BTreeMap<String, String>,
) -> Result<DriverMaterialization> {
    let parent = journal
        .parent()
        .context("driver journal has no parent")?
        .join("driver-materializations");
    let parent = std::path::absolute(&parent).with_context(|| {
        format!(
            "failed to absolutize driver materialization parent {}",
            parent.display()
        )
    })?;
    fs::create_dir_all(&parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("clock precedes epoch")?
        .as_nanos();
    let root = parent.join(format!("{}-{}-{nonce}", std::process::id(), label));
    fs::create_dir(&root).with_context(|| format!("failed to create {}", root.display()))?;
    let materialization = DriverMaterialization { root };
    for (index, (name, source)) in sources.iter().enumerate() {
        let checkout = materialization.root.join(format!("{index:02}-repository"));
        let status = std::process::Command::new("git")
            .args(["clone", "--quiet", "--no-checkout", "--shared"])
            .arg(source)
            .arg(&checkout)
            .status()
            .with_context(|| format!("failed to clone repository `{name}`"))?;
        if !status.success() {
            bail!("failed to materialize repository `{name}`");
        }
        let reference = references
            .get(name)
            .with_context(|| format!("missing materialization ref for `{name}`"))?;
        let output = std::process::Command::new("git")
            .args(["-C"])
            .arg(&checkout)
            .args(["checkout", "--quiet", "--detach", reference])
            .output()?;
        if !output.status.success() {
            bail!(
                "failed to checkout `{reference}` for `{name}`: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }
    Ok(materialization)
}

fn restore_counterfactual_checkouts(originals: &[(PathBuf, String)]) -> Result<()> {
    for (checkout, oid) in originals {
        let reset = std::process::Command::new("git")
            .arg("-C")
            .arg(checkout)
            .args(["reset", "--hard", oid])
            .output()?;
        if !reset.status.success() {
            bail!(
                "failed to restore amendment checkout {}: {}",
                checkout.display(),
                String::from_utf8_lossy(&reset.stderr).trim()
            );
        }
    }
    Ok(())
}

fn amendment_counterfactual(
    criterion: &pce_core::EffectiveCriterion,
    paths: &[PathBuf],
    named_paths: &BTreeMap<String, PathBuf>,
) -> Result<AmendmentProof> {
    let mut originals = Vec::new();
    for refs in &criterion.repository_refs {
        let Some(checkout) = named_paths.get(&refs.repository) else {
            restore_counterfactual_checkouts(&originals)?;
            return Ok(AmendmentProof::Unconstructable {
                repository: refs.repository.clone(),
                repair_ref: refs.repair_ref.clone(),
                detail: "composed materialization omitted amendment repository".to_owned(),
            });
        };
        originals.push((checkout.clone(), git_oid(checkout, "HEAD")?));
        let revert = std::process::Command::new("git")
            .arg("-C")
            .arg(checkout)
            .env("GIT_EDITOR", "true")
            .args(["revert", "--no-commit", &refs.repair_ref])
            .output()?;
        if !revert.status.success() {
            let detail = [
                String::from_utf8_lossy(&revert.stdout).trim().to_owned(),
                String::from_utf8_lossy(&revert.stderr).trim().to_owned(),
            ]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
            restore_counterfactual_checkouts(&originals)?;
            return Ok(AmendmentProof::Unconstructable {
                repository: refs.repository.clone(),
                repair_ref: refs.repair_ref.clone(),
                detail,
            });
        }
    }
    let execution = shell_execution(&criterion.command, paths);
    restore_counterfactual_checkouts(&originals)?;
    Ok(AmendmentProof::Reverted {
        execution: execution?,
    })
}

fn execute_effective_criterion(
    criterion: &pce_core::EffectiveCriterion,
    paths: &[PathBuf],
    named_paths: &BTreeMap<String, PathBuf>,
) -> Result<(CriterionExecution, Option<AmendmentProof>, bool)> {
    let amendment = matches!(
        criterion.origin,
        pce_core::CriterionOrigin::Amendment { .. }
    );
    let paired_amendment = amendment && !criterion.repository_refs.is_empty();
    let originals = paired_amendment
        .then(|| {
            named_paths
                .values()
                .map(|checkout| Ok((checkout.clone(), git_oid(checkout, "HEAD")?)))
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let execution = shell_execution(&criterion.command, paths)?;
    if let Some(originals) = &originals {
        restore_counterfactual_checkouts(originals)?;
    }
    let amendment_proof = paired_amendment
        .then(|| amendment_counterfactual(criterion, paths, named_paths))
        .transpose()?;
    let passed = execution.exit_status().is_success()
        && amendment_proof
            .as_ref()
            .is_none_or(AmendmentProof::proves_guard);
    Ok((execution, amendment_proof, passed))
}

fn shell_execution(command: &str, paths: &[PathBuf]) -> Result<CriterionExecution> {
    let cwd = paths
        .first()
        .context("package has no materialized repository")?;
    shell_execution_at(command, cwd, paths)
}

fn shell_execution_at(command: &str, cwd: &Path, paths: &[PathBuf]) -> Result<CriterionExecution> {
    let projection =
        serde_json::to_string(paths).context("failed to serialize coordinated worktrees")?;
    let mut child = std::process::Command::new("/bin/sh");
    child
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .env("PCE_WORKTREES", projection);
    for (index, path) in paths.iter().enumerate() {
        child.env(format!("PCE_WORKTREE_{index}"), path);
    }
    let output = child
        .output()
        .with_context(|| format!("failed to execute criterion `{command}`"))?;
    let exit_status = match output.status.code() {
        Some(code) => CommandExitStatus::Exited { code },
        None => CommandExitStatus::Signaled {
            signal: output
                .status
                .signal()
                .context("shell has neither exit code nor signal")?,
        },
    };
    Ok(CriterionExecution::new(
        command.to_owned(),
        cwd.display().to_string(),
        exit_status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

fn prepare_driver_materialization(
    journal: &Path,
    package: &str,
    materialization_label: &str,
    materialization: &DriverMaterialization,
    sources: &[(String, PathBuf)],
    preparations: &BTreeMap<String, String>,
) -> Result<bool> {
    let paths = materialization.paths()?;
    for (repository, checkout) in materialization.named_paths(sources) {
        let Some(command) = preparations.get(&repository) else {
            continue;
        };
        let execution = shell_execution_at(command, &checkout, &paths)?;
        let succeeded = execution.exit_status().is_success();
        append_driver_event(
            journal,
            &DriverEvent::EnvironmentPreparationExecuted {
                package: package.to_owned(),
                materialization: materialization_label.to_owned(),
                repository,
                command: command.clone(),
                outcome: if succeeded {
                    EnvironmentPreparationOutcome::Succeeded
                } else {
                    EnvironmentPreparationOutcome::Failed
                },
                execution,
            },
        )?;
        if !succeeded {
            return Ok(false);
        }
    }
    Ok(true)
}

fn execute_driver_criteria(command: DriverCriteriaCommand) -> Result<DriverStatusCommand> {
    let graph = read_driver_graph(&command.graph_path)?;
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == command.package_id)
        .with_context(|| format!("package {} is absent from graph", command.package_id))?;
    let sources = package_repository_sources(package, &command.repositories)?;
    let references = sources
        .iter()
        .map(|(name, source)| Ok((name.clone(), git_oid(source, "HEAD")?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let materialization =
        materialize_driver_state(&command.journal_path, "criteria", &sources, &references)?;
    if !prepare_driver_materialization(
        &command.journal_path,
        &command.package_id,
        "criteria",
        &materialization,
        &sources,
        &command.preparations,
    )? {
        return Ok(DriverStatusCommand {
            graph_path: command.graph_path,
            journal_path: command.journal_path,
            override_risk_ordering: false,
        });
    }
    let paths = materialization.paths()?;
    let events = read_driver_journal(&command.journal_path)?;
    let criteria = effective_criteria(&graph, &command.package_id, &events)
        .context("failed to derive effective criteria")?;
    let mut failed = Vec::new();
    for criterion in criteria {
        let execution = shell_execution(&criterion.command, &paths)?;
        if !execution.exit_status().is_success() {
            failed.push(criterion.name.clone());
        }
        append_driver_event(
            &command.journal_path,
            &DriverEvent::CriterionExecuted {
                package: command.package_id.clone(),
                name: criterion.name,
                origin: criterion.origin,
                execution,
            },
        )?;
    }
    if !failed.is_empty() {
        append_driver_event(
            &command.journal_path,
            &DriverEvent::PackageFailed {
                package: command.package_id.clone(),
                reason: format!("criteria failed: {}", failed.join(", ")),
            },
        )?;
    }
    Ok(DriverStatusCommand {
        graph_path: command.graph_path,
        journal_path: command.journal_path,
        override_risk_ordering: false,
    })
}

fn run_driver_criteria(command: DriverCriteriaCommand) -> Result<()> {
    run_driver_status(execute_driver_criteria(command)?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DriverFindingDisposition {
    StructurallyUsable,
    StructurallyMalformed,
}

fn run_driver_replay(command: DriverReplayCommand) -> Result<()> {
    replay_driver_finding(command, true).map(|_| ())
}

fn replay_driver_finding(
    command: DriverReplayCommand,
    emit_stdout: bool,
) -> Result<DriverFindingDisposition> {
    let graph = read_driver_graph(&command.graph_path)?;
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == command.package_id)
        .with_context(|| format!("package {} is absent from graph", command.package_id))?;
    let sources = package_repository_sources(package, &command.repositories)?;
    let outcome_bytes = fs::read(&command.outcome_path)
        .with_context(|| format!("failed to read {}", command.outcome_path.display()))?;
    let outcome =
        parse_package_gate_outcome(&outcome_bytes).context("failed to parse gate outcome")?;
    let finding = outcome
        .findings()
        .get(command.finding)
        .with_context(|| format!("gate finding {} is absent", command.finding))?;
    let worktrees = sources
        .iter()
        .map(|(name, path)| RepositoryWorktree::parse(name.clone(), path.clone()))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let structural_validation =
        validate_package_gate_finding_repositories(finding, package.repositories())
            .context("finding exceeds package repository scope")
            .and_then(|()| {
                validate_package_gate_finding_refs(finding, &worktrees)
                    .context("finding refs do not satisfy the WP6 witness/repair contract")
            });
    if let Err(source) = structural_validation {
        let detail = format!("{source:#}");
        let decision = FindingReplayDecision::Rejected {
            reason: FindingRejectionReason::StructurallyMalformed,
        };
        append_driver_event(
            &command.journal_path,
            &DriverEvent::FindingRejected {
                package: command.package_id.clone(),
                gate: command.gate,
                finding: u64::try_from(command.finding).context("finding index exceeds u64")?,
                command: finding.proposed_criterion_command().to_owned(),
                reason: FindingRejectionReason::StructurallyMalformed,
                detail,
            },
        )?;
        if emit_stdout {
            write_json_stdout(&json!({"decision": decision}))?;
        }
        return Ok(DriverFindingDisposition::StructurallyMalformed);
    }
    let by_refs = finding
        .repository_refs()
        .iter()
        .map(|refs| (refs.repository(), refs))
        .collect::<BTreeMap<_, _>>();
    let mut witness_refs = BTreeMap::new();
    let mut repair_refs = BTreeMap::new();
    let mut recorded_refs = Vec::new();
    for (name, source) in &sources {
        if let Some(refs) = by_refs.get(name.as_str()) {
            let witness = git_oid(source, refs.witness_ref())?;
            let repair = git_oid(source, refs.repair_ref())?;
            witness_refs.insert(name.clone(), witness.clone());
            repair_refs.insert(name.clone(), repair.clone());
            recorded_refs.push(AmendmentRepositoryRefs {
                repository: name.clone(),
                witness_ref: witness,
                repair_ref: repair,
            });
        } else {
            let head = git_oid(source, "HEAD")?;
            witness_refs.insert(name.clone(), head.clone());
            repair_refs.insert(name.clone(), head);
        }
    }
    let witness_state =
        materialize_driver_state(&command.journal_path, "witness", &sources, &witness_refs)?;
    let repair_state =
        materialize_driver_state(&command.journal_path, "repair", &sources, &repair_refs)?;
    let witness_prepared = prepare_driver_materialization(
        &command.journal_path,
        &command.package_id,
        "witness",
        &witness_state,
        &sources,
        &command.preparations,
    )?;
    if !witness_prepared {
        if emit_stdout {
            write_json_stdout(
                &json!({"decision": "not-judged", "reason": "environment-preparation-failed"}),
            )?;
        }
        return Ok(DriverFindingDisposition::StructurallyUsable);
    }
    let repair_prepared = prepare_driver_materialization(
        &command.journal_path,
        &command.package_id,
        "repair",
        &repair_state,
        &sources,
        &command.preparations,
    )?;
    if !repair_prepared {
        if emit_stdout {
            write_json_stdout(
                &json!({"decision": "not-judged", "reason": "environment-preparation-failed"}),
            )?;
        }
        return Ok(DriverFindingDisposition::StructurallyUsable);
    }
    let witness = shell_execution(
        finding.proposed_criterion_command(),
        &witness_state.paths()?,
    )?;
    let repair = shell_execution(finding.proposed_criterion_command(), &repair_state.paths()?)?;
    let decision = judge_finding_replay(&witness, &repair);
    append_driver_event(
        &command.journal_path,
        &DriverEvent::FindingReplayed {
            package: command.package_id.clone(),
            gate: command.gate,
            finding: u64::try_from(command.finding).context("finding index exceeds u64")?,
            command: finding.proposed_criterion_command().to_owned(),
            repository_refs: recorded_refs,
            witness,
            repair,
            decision: decision.clone(),
        },
    )?;
    if emit_stdout {
        write_json_stdout(&json!({"decision": decision}))?;
    }
    Ok(DriverFindingDisposition::StructurallyUsable)
}

fn package_result_path(vision_dir: &Path, package: &NodeId, issuance: Sequence) -> Result<PathBuf> {
    let component = Path::new(package.as_str());
    if component.components().count() != 1
        || !matches!(
            component.components().next(),
            Some(std::path::Component::Normal(_))
        )
    {
        bail!("package result identity is not one safe path component");
    }
    Ok(vision_dir
        .join(".pce")
        .join("package-results")
        .join(component)
        .join(format!("{}.json", issuance.get())))
}

fn write_package_result_atomic(path: &Path, result: &PackageWorkerResult) -> Result<()> {
    let parent = path.parent().context("package result path has no parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create package result directory {}",
            parent.display()
        )
    })?;
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .context("system clock precedes Unix epoch")?
        .as_nanos();
    let temporary = path.with_extension(format!("json.tmp.{}.{nonce}", std::process::id()));
    let bytes = serialize_package_worker_result(result)
        .context("failed to serialize package worker result")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .with_context(|| {
            format!(
                "failed to create package result temporary file {}",
                temporary.display()
            )
        })?;
    file.write_all(&bytes)
        .context("failed to write package worker result")?;
    file.sync_all()
        .context("failed to sync package worker result")?;
    fs::hard_link(&temporary, path)
        .with_context(|| format!("failed to publish package worker result {}", path.display()))?;
    fs::remove_file(&temporary).context("failed to remove package result temporary file")?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .context("failed to sync package result directory")?;
    Ok(())
}

#[cfg(unix)]
fn observe_surviving_package_worker_processes(process_group: u32) -> Result<SurvivingProcesses> {
    let process_group =
        i32::try_from(process_group).context("package worker process group exceeds pid_t")?;
    // SAFETY: the spawned child's positive PID is its isolated process-group ID. Signal zero
    // performs a presence/permission probe and does not signal or terminate any process.
    let observation = unsafe { libc::kill(-process_group, 0) };
    if observation == 0 {
        return Ok(SurvivingProcesses::Present);
    }
    let error = std::io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(SurvivingProcesses::Absent),
        Some(libc::EPERM) => Ok(SurvivingProcesses::Present),
        _ => Err(error).context("failed to inspect package worker process group"),
    }
}

fn run_package_worker(
    result_path: &Path,
    required_artifact_path: &Path,
    worker_arguments: &[String],
) -> Result<()> {
    let (program, arguments) = worker_arguments
        .split_first()
        .context("package worker command is empty")?;
    let started = Instant::now();
    let mut command = std::process::Command::new(program);
    command.args(arguments).process_group(0);
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn package worker `{program}`"))?;
    let process_group = child.id();
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for package worker `{program}`"))?;
    let duration_ms = u64::try_from(started.elapsed().as_millis())
        .context("package worker duration exceeds u64")?;
    let result = PackageWorkerResult::new(
        DispatchDuration::new(duration_ms),
        PackageWorkerStoppedAt::parse(
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        )?,
        dispatch_exit_status(status)?,
        observe_required_artifact_presence(required_artifact_path)?,
        observe_surviving_package_worker_processes(process_group)?,
    );
    write_package_result_atomic(result_path, &result)?;
    match result.exit_status() {
        DispatchExitStatus::Exited { code } => {
            let code = i32::try_from(code.get()).context("worker exit code exceeds i32")?;
            std::process::exit(code);
        }
        DispatchExitStatus::Signaled { signal } => {
            let signal = i32::try_from(signal.get()).context("worker signal exceeds i32")?;
            std::process::exit(128_i32.saturating_add(signal));
        }
    }
}

fn read_package_result(path: &Path) -> Result<Option<PackageWorkerResult>> {
    match fs::read(path) {
        Ok(bytes) => parse_package_worker_result(&bytes)
            .context("package result file is invalid")
            .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn collect_package_completions(log_path: &Path, vision_dir: &Path) -> Result<Value> {
    let records = read_event_log(log_path)?
        .into_iter()
        .map(|line| line.record)
        .collect::<Vec<_>>();
    let ledger =
        fold_dispatch_ledger(&records).context("failed to derive package dispatch ledger")?;
    let mut appended = Vec::new();
    for issuance in ledger.unaccounted().entries() {
        if issuance.role().as_str() != "work-package-worker" {
            continue;
        }
        let result_path = package_result_path(vision_dir, issuance.node(), issuance.sequence())?;
        let Some(result) = read_package_result(&result_path)? else {
            continue;
        };
        close_dispatch_conditionally(
            log_path,
            DispatchClosureTarget {
                issuance_sequence: issuance.sequence(),
                node: issuance.node().clone(),
            },
            |_| {
                Ok(dispatch_completion_payload(
                    issuance.sequence(),
                    result.duration_ms(),
                    DispatchTokenUsage::Absent {
                        reason: UsageAbsenceReason::NoTerminalTurn,
                    },
                    result.exit_status(),
                    ArtifactOutcome::NotValidated,
                    result.required_artifact_presence(),
                ))
            },
        )?;
        appended.push(issuance.sequence().get());
    }
    let records = read_event_log(log_path)?
        .into_iter()
        .map(|line| line.record)
        .collect::<Vec<_>>();
    let ledger =
        fold_dispatch_ledger(&records).context("failed to rederive package dispatch ledger")?;
    let packages = ledger
        .entries()
        .iter()
        .filter(|entry| entry.issuance().role().as_str() == "work-package-worker")
        .map(|entry| {
            let result_path = package_result_path(
                vision_dir,
                entry.issuance().node(),
                entry.issuance().sequence(),
            )?;
            let result = read_package_result(&result_path)?;
            Ok(json!({
                "issuance_sequence": entry.issuance().sequence().get(),
                "package": entry.issuance().node().as_str(),
                "state": if result.is_some() { "finished" } else { "unaccounted" },
                "result_path": result_path.display().to_string(),
                "exit_status": result.as_ref().map(PackageWorkerResult::exit_status),
                "surviving_processes": result.map(|result| result.surviving_processes()),
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({ "appended": appended, "packages": packages }))
}

fn run_package_completions(log_path: &Path, vision_dir: &Path) -> Result<()> {
    let completions = collect_package_completions(log_path, vision_dir)?;
    write_json_stdout(&completions)
}

#[derive(Debug)]
struct SpawnObservedDispatchError {
    source: Error,
}

impl std::fmt::Display for SpawnObservedDispatchError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "child spawn was observed but its response was unusable: {:#}",
            self.source
        )
    }
}

impl std::error::Error for SpawnObservedDispatchError {}

enum HerdrPaneRunOutcome {
    Started(Value),
    Refused(Error),
    SpawnObservedButUnusable(Error),
}

fn environment_assignment_name(argument: &str) -> Option<&str> {
    let (name, _) = argument.split_once('=')?;
    let mut bytes = name.bytes();
    let valid_start = bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
    (valid_start && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')).then_some(name)
}

fn redact_environment_assignments(arguments: &[String]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| {
            environment_assignment_name(argument)
                .map_or_else(|| argument.clone(), |name| format!("{name}=<redacted>"))
        })
        .collect()
}

#[cfg(test)]
#[test]
fn herdr_082_compatible_versions_are_named_and_bounded() {
    assert!(herdr_version_is_supported("herdr 0.8.2"));
    assert!(herdr_version_is_supported("herdr 0.8.9"));
    assert!(!herdr_version_is_supported("herdr 0.8.1"));
    assert!(!herdr_version_is_supported("herdr 0.9.0"));
    assert!(!herdr_version_is_supported("herdr 0.8.2-alpha"));
    assert!(!herdr_version_is_supported("herdr 0.8.2+build"));
}

#[cfg(test)]
#[test]
fn herdr_session_prefix_is_optional_and_precedes_the_subcommand() {
    assert_eq!(
        scoped_herdr_arguments(None, &["pane", "process-info"]),
        ["pane", "process-info"]
    );
    let session = parse_herdr_session_name("pce-work").expect("valid Herdr session");
    assert_eq!(
        scoped_herdr_arguments(Some(&session), &["pane", "process-info"]),
        ["--session", "pce-work", "pane", "process-info"]
    );
}

#[cfg(test)]
#[test]
fn driver_run_parser_accepts_a_named_herdr_session() {
    let arguments = [
        "--graph",
        "/tmp/graph.json",
        "--journal",
        "/tmp/journal.jsonl",
        "--repository",
        "pce=/tmp/pce",
        "--herdr-session",
        "pce-work",
    ]
    .map(str::to_owned);
    let Command::DriverRun(command) = parse_driver_run(&arguments).expect("driver command") else {
        panic!("unexpected command");
    };
    assert_eq!(
        command.herdr_session.as_ref().map(HerdrSessionName::as_str),
        Some("pce-work")
    );
}

#[cfg(test)]
#[test]
fn worker_launch_script_is_private_and_never_replaces_an_existing_file() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("worker-launch.sh");
    prepare_private_dispatch_directory(directory.path()).expect("private directory");
    materialize_worker_launch_script(&path, "original").expect("materialized script");
    assert!(materialize_worker_launch_script(&path, "replacement").is_err());
    assert_eq!(fs::read_to_string(&path).expect("script"), "original");
    assert_eq!(
        fs::metadata(&path).expect("metadata").permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(directory.path())
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[cfg(test)]
#[test]
fn herdr_environment_values_are_redacted_from_observations_and_errors() {
    let arguments = vec![
        "/usr/bin/env".to_owned(),
        "-i".to_owned(),
        "CAMPAIGN_TOKEN=super-secret".to_owned(),
        "worker".to_owned(),
    ];
    assert_eq!(
        redact_environment_assignments(&arguments),
        ["/usr/bin/env", "-i", "CAMPAIGN_TOKEN=<redacted>", "worker"]
    );
    let pane_stderr = redact_pane_run_stderr(
        &BTreeMap::from([
            ("SHORT".to_owned(), "sec".to_owned()),
            ("LONG".to_owned(), "secret".to_owned()),
        ]),
        b"SHORT=sec LONG=secret secret",
    );
    assert!(!pane_stderr.contains("sec"));
}

fn redact_pane_run_stderr(environment: &BTreeMap<String, String>, stderr: &[u8]) -> String {
    let mut redacted = String::from_utf8_lossy(stderr).into_owned();
    let mut values = environment
        .values()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));
    for value in values {
        redacted = redacted.replace(value, "<redacted>");
    }
    redacted
}

fn prepare_private_dispatch_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| {
        format!(
            "failed to create dispatch temporary directory {}",
            path.display()
        )
    })?;
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to inspect dispatch temporary directory {}",
            path.display()
        )
    })?;
    // SAFETY: geteuid has no preconditions and only reads the current process credential.
    let effective_uid = unsafe { libc::geteuid() };
    if metadata.file_type().is_symlink() || !metadata.is_dir() || metadata.uid() != effective_uid {
        bail!(
            "dispatch temporary directory must be a real directory owned by uid {}: {}",
            effective_uid,
            path.display()
        );
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).with_context(|| {
        format!(
            "failed to secure dispatch temporary directory {}",
            path.display()
        )
    })
}

fn materialize_worker_launch_script(path: &Path, contents: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(path)
        .with_context(|| {
            format!(
                "failed to create unique worker launch script {}",
                path.display()
            )
        })?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write worker launch script {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to sync worker launch script {}", path.display()))
}

fn execute_herdr_pane_run(
    invocation: &HerdrInvocation,
    environment: &BTreeMap<String, String>,
) -> HerdrPaneRunOutcome {
    let output = match std::process::Command::new(invocation.executable())
        .args(invocation.argv())
        .output()
    {
        Ok(output) => output,
        Err(source) => {
            return HerdrPaneRunOutcome::Refused(
                Error::new(source)
                    .context(format!("failed to execute {}", invocation.executable())),
            );
        }
    };
    if !output.status.success() {
        return HerdrPaneRunOutcome::Refused(anyhow!(
            "herdr pane-run failed with {} for {:?}: {}",
            output.status,
            ["pane", "run", "<pane-id>", "<launch-script>"],
            redact_pane_run_stderr(environment, &output.stderr).trim()
        ));
    }
    if output.stdout.is_empty() {
        return HerdrPaneRunOutcome::Started(json!({
            "result": {"type": "pane_input_sent"}
        }));
    }
    match serde_json::from_slice(&output.stdout) {
        Ok(response) => HerdrPaneRunOutcome::Started(response),
        Err(source) => HerdrPaneRunOutcome::SpawnObservedButUnusable(
            Error::new(source).context("herdr pane-run returned invalid JSON"),
        ),
    }
}

fn wait_for_worker_launch(script_path: &Path, result_path: &Path) -> bool {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if !script_path.exists() || result_path.is_file() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
}

fn parse_herdr_foreground_process(response: &Value) -> Option<DispatchWorkerProcessObservation> {
    let process = response
        .pointer("/result/process_info/foreground_processes")?
        .as_array()?
        .first()?;
    let process_id = u32::try_from(process.get("pid")?.as_u64()?).ok()?;
    let name = process
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| process.get("argv0").and_then(Value::as_str))?
        .to_owned();
    let argv = process
        .get("argv")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            process
                .get("cmdline")
                .and_then(Value::as_str)
                .map_or_else(Vec::new, |line| {
                    line.split_whitespace().map(str::to_owned).collect()
                })
        });
    let argv = redact_environment_assignments(&argv);
    Some(DispatchWorkerProcessObservation::Observed {
        process_id,
        name,
        argv,
    })
}

fn observe_spawned_herdr_process(
    session: Option<&HerdrSessionName>,
    pane_id: &str,
) -> DispatchWorkerProcessObservation {
    let started = Instant::now();
    let mut detail = "worker pane had no foreground process".to_owned();
    let arguments = scoped_herdr_arguments(session, &["pane", "process-info", "--pane", pane_id]);
    while started.elapsed() < Duration::from_millis(500) {
        match std::process::Command::new("herdr")
            .args(&arguments)
            .output()
        {
            Ok(output) if output.status.success() => match serde_json::from_slice(&output.stdout) {
                Ok(response) => {
                    if let Some(process) = parse_herdr_foreground_process(&response) {
                        return process;
                    }
                    detail =
                        "worker pane process-info contained no identifiable foreground process"
                            .to_owned();
                }
                Err(source) => {
                    detail = format!("worker pane process-info returned invalid JSON: {source}")
                }
            },
            Ok(output) => {
                detail = format!(
                    "worker pane process-info failed with {}: {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            Err(source) => detail = format!("failed to execute herdr pane process-info: {source}"),
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    DispatchWorkerProcessObservation::Inconclusive { detail }
}

fn is_prime_agent_dispatch(arguments: &[String]) -> bool {
    let executable = arguments
        .iter()
        .rposition(|argument| argument == "--")
        .and_then(|separator| arguments.get(separator + 1))
        .or_else(|| arguments.first());
    executable.is_some_and(|argument| {
        Path::new(argument)
            .file_name()
            .and_then(|name| name.to_str())
            == Some("prime-agent")
    })
}

fn matching_prime_sessions(worktree_path: &str) -> Vec<Option<String>> {
    let Ok(canonical_worktree_path) = Path::new(worktree_path).canonicalize() else {
        return Vec::new();
    };
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let descriptor_root = PathBuf::from(home).join(".prime/agent/daemon-workers");
    let Ok(daemon_entries) = fs::read_dir(descriptor_root) else {
        return Vec::new();
    };
    daemon_entries
        .filter_map(|entry| entry.ok())
        .flat_map(|daemon| {
            fs::read_dir(daemon.path())
                .into_iter()
                .flatten()
                .filter_map(|entry| entry.ok())
        })
        .filter(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some("json"))
        .filter_map(|entry| fs::read(entry.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|descriptor| {
            descriptor
                .pointer("/createCommand/config/cwd")
                .and_then(Value::as_str)
                .and_then(|cwd| Path::new(cwd).canonicalize().ok())
                .is_some_and(|cwd| cwd == canonical_worktree_path)
        })
        .map(|descriptor| {
            descriptor
                .pointer("/createCommand/sessionPath")
                .and_then(Value::as_str)
                .filter(|session_path| Path::new(session_path).is_absolute())
                .map(str::to_owned)
        })
        .collect()
}

fn observe_prime_session_path(worktree_path: &str) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < Duration::from_millis(500) {
        let matches = matching_prime_sessions(worktree_path);
        match matches.as_slice() {
            [Some(session_path)] => return Some(session_path.clone()),
            [None] | [_, _, ..] => return None,
            [] => std::thread::sleep(Duration::from_millis(25)),
        }
    }
    None
}

fn record_dispatch_spawn_failure(
    log_path: &Path,
    node: &NodeId,
    issuance_sequence: Sequence,
) -> Result<()> {
    close_dispatch_conditionally(
        log_path,
        DispatchClosureTarget {
            issuance_sequence,
            node: node.clone(),
        },
        |_| {
            Ok(DispatchCompletionPayload::SpawnFailed(
                SpawnFailedDispatchCompletionPayload {
                    issuance_sequence,
                    outcome: SpawnDispatchOutcome::SpawnFailed,
                    artifact_production: ArtifactProduction::NotProduced,
                },
            ))
        },
    )
    .map(|_| ())
}

fn execute_herdr(invocation: &HerdrInvocation) -> Result<Value> {
    let output = std::process::Command::new(invocation.executable())
        .args(invocation.argv())
        .output()
        .with_context(|| format!("failed to execute {}", invocation.executable()))?;
    if !output.status.success() {
        bail!(
            "herdr command failed with {} for {:?}: {}",
            output.status,
            invocation.argv(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("herdr command returned invalid JSON")
}

fn herdr_location(response: &Value) -> Result<HerdrAgentLocation> {
    let workspace = response
        .pointer("/result/workspace/workspace_id")
        .and_then(Value::as_str)
        .context("herdr worktree response omitted result.workspace.workspace_id")?;
    let tab = response
        .pointer("/result/tab/tab_id")
        .and_then(Value::as_str)
        .context("herdr worktree response omitted result.tab.tab_id")?;
    let pane = response
        .pointer("/result/root_pane/pane_id")
        .and_then(Value::as_str)
        .context("herdr worktree response omitted result.root_pane.pane_id")?;
    Ok(HerdrAgentLocation::new(
        HerdrWorkspaceId::parse(workspace)?,
        HerdrTabId::parse(tab)?,
        HerdrPaneId::parse(pane)?,
    ))
}

fn package_worktree_root() -> Result<PathBuf> {
    let path = std::env::var_os("PCE_WORK_PACKAGE_WORKTREE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/pce-work-package-worktrees"));
    if !path.is_absolute() {
        bail!(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT must be absolute: {}",
            path.display()
        );
    }
    Ok(path)
}

fn standalone_package_temporary_directory(
    vision: &DispatchVisionSource,
    package: &WorkPackageId,
) -> PathBuf {
    let mut digest = Sha256::new();
    digest.update(vision.as_str().len().to_be_bytes());
    digest.update(vision.as_str().as_bytes());
    digest.update(package.as_str().len().to_be_bytes());
    digest.update(package.as_str().as_bytes());
    let bytes = digest.finalize();
    let suffix = bytes[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    PathBuf::from("/tmp/pce-tmp").join(suffix)
}

fn package_temporary_directory(
    vision: &DispatchVisionSource,
    package: &WorkPackageId,
    attempt: DispatchAttempt,
) -> PathBuf {
    let mut digest = Sha256::new();
    digest.update(vision.as_str().len().to_be_bytes());
    digest.update(vision.as_str().as_bytes());
    digest.update(package.as_str().len().to_be_bytes());
    digest.update(package.as_str().as_bytes());
    digest.update(attempt.get().to_be_bytes());
    let bytes = digest.finalize();
    let suffix = bytes[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    PathBuf::from("/tmp/pce-tmp").join(suffix)
}

fn herdr_config_dir() -> PathBuf {
    if let Ok(directory) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(directory).join("herdr");
    }
    herdr_platform_config_dir()
}

#[cfg(windows)]
fn herdr_platform_config_dir() -> PathBuf {
    if let Ok(directory) = std::env::var("APPDATA") {
        return PathBuf::from(directory).join("herdr");
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile)
            .join("AppData")
            .join("Roaming")
            .join("herdr");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config").join("herdr");
    }
    std::env::temp_dir().join("herdr")
}

#[cfg(not(windows))]
fn herdr_platform_config_dir() -> PathBuf {
    std::env::var("HOME").map_or_else(
        |_| std::env::temp_dir().join("herdr"),
        |home| PathBuf::from(home).join(".config").join("herdr"),
    )
}

fn herdr_session_socket_path(value: &str) -> PathBuf {
    herdr_config_dir()
        .join("sessions")
        .join(value)
        .join("herdr.sock")
}

fn parse_herdr_session_name(value: impl Into<String>) -> Result<HerdrSessionName> {
    HerdrSessionName::parse(value, herdr_session_socket_path).map_err(Error::new)
}

fn herdr_version_is_supported(detected: &str) -> bool {
    let Some(version) = detected.strip_prefix("herdr ") else {
        return false;
    };
    let mut parts = version.split('.');
    let major = parts.next().and_then(|part| part.parse::<u64>().ok());
    let minor = parts.next().and_then(|part| part.parse::<u64>().ok());
    let patch = parts.next().and_then(|part| part.parse::<u64>().ok());
    parts.next().is_none()
        && matches!((major, minor, patch), (Some(0), Some(8), Some(patch)) if patch >= 2)
}

fn require_herdr_session_running(session: Option<&HerdrSessionName>) -> Result<()> {
    let Some(session) = session else {
        return Ok(());
    };
    let arguments = scoped_herdr_arguments(Some(session), &["workspace", "list"]);
    let output = std::process::Command::new("herdr")
        .args(&arguments)
        .output()
        .with_context(|| format!("failed to check Herdr session `{}`", session.as_str()))?;
    if !output.status.success() {
        bail!(
            "Herdr session `{}` is not available: {}",
            session.as_str(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn require_supported_herdr_version() -> Result<String> {
    let output = std::process::Command::new("herdr")
        .arg("--version")
        .output()
        .context("failed to detect Herdr version; PCE requires Herdr >=0.8.2,<0.9.0")?;
    let detected = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() {
        bail!(
            "failed to detect Herdr version (status {}): {}; PCE requires Herdr >=0.8.2,<0.9.0",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    if !herdr_version_is_supported(&detected) {
        bail!(
            "unsupported Herdr version `{detected}`; PCE requires Herdr >=0.8.2,<0.9.0 for the pane-run dispatch protocol"
        );
    }
    Ok(detected)
}

fn issue_package_dispatch(command: PackageDispatchCommand) -> Result<Value> {
    if std::env::var("HERDR_ENV").as_deref() != Ok("1") {
        bail!("package dispatch requires HERDR_ENV=1 inside a Herdr-managed pane");
    }
    require_herdr_session_running(command.herdr_session.as_ref())?;
    let herdr_version = require_supported_herdr_version()?;
    let graph_bytes = fs::read(&command.graph_path).with_context(|| {
        format!(
            "failed to read package graph {}",
            command.graph_path.display()
        )
    })?;
    let graph = parse_work_package_graph(&graph_bytes).context("failed to parse package graph")?;
    if command.require_graph_at_vision_root
        && absolute_path(&command.vision_dir)?
            != absolute_path(
                command
                    .graph_path
                    .parent()
                    .context("package graph has no parent")?,
            )?
    {
        bail!("package graph must be at the supplied vision directory root");
    }
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == command.package_id)
        .with_context(|| format!("package {} is absent from graph", command.package_id))?;
    let node = NodeId::parse(package.id().as_str())?;
    let metadata = DispatchLogging {
        node: node.clone(),
        role: DispatchRole::new("work-package-worker"),
        dispatch_ref: DispatchRef::new(package_authored_refs(&graph, package)?),
        evidence: Evidence::parse("pce-composed herdr work-package dispatch")?,
        required_artifact_path: command.required_artifact_path.clone(),
    };
    let issuance =
        admit_and_append_dispatch(&command.log_path, &metadata, dispatch_payload(&metadata))?;
    let attempt = command
        .attempt
        .unwrap_or(DispatchAttempt::parse(issuance.sequence().get())?);
    let vision = DispatchVisionSource::parse(graph.vision().to_owned())?;
    let repository_inputs = command
        .repositories
        .iter()
        .map(|(name, root)| -> Result<RepositoryDispatchInput> {
            let base_ref = match command.base_refs.get(name) {
                Some(base_ref) => base_ref.clone(),
                None => graph_authored_ref(&graph, name)?.to_owned(),
            };
            Ok(RepositoryDispatchInput::parse(
                name.clone(),
                root.clone(),
                base_ref,
            )?)
        })
        .collect::<Result<Vec<_>>>()?;
    let worktree_root = package_worktree_root()?;
    let temporary_directory = package_temporary_directory(&vision, package.id(), attempt);
    fs::create_dir_all(&worktree_root).context("failed to create binary-owned worktree root")?;
    fs::create_dir_all(&temporary_directory).context("failed to create binary-owned TMPDIR")?;
    let worktree_root = AbsoluteWorktreeRoot::parse(worktree_root)?;
    let temporary_directory = AbsoluteDispatchTemporaryDirectory::parse(temporary_directory)?;
    let environment = WorkerEnvironment::parse(command.environment)?;
    let provisional = compose_herdr_work_package_dispatch(
        &vision,
        package,
        attempt,
        command.herdr_session.clone(),
        &repository_inputs,
        &worktree_root,
        &temporary_directory,
        environment.clone(),
        WorkerArgumentVector::parse(command.worker_arguments.clone())?,
    )?;
    let worktree_paths = provisional
        .worktrees()
        .iter()
        .map(|worktree| worktree.path().to_path_buf())
        .collect::<Vec<_>>();
    let result_path = derive_package_result_path(
        &command.vision_dir,
        package.id(),
        issuance.sequence(),
        &worktree_paths,
    )?;
    if result_path.as_path().exists() {
        bail!(
            "package result path already exists: {}",
            result_path.as_str()
        );
    }
    let wrapper = std::env::current_exe().context("failed to resolve package worker wrapper")?;
    let wrapped_arguments = compose_package_worker_argv(
        &wrapper,
        &result_path,
        &command.required_artifact_path,
        &command.worker_arguments,
    )?;
    let plan = compose_herdr_work_package_dispatch(
        &vision,
        package,
        attempt,
        command.herdr_session.clone(),
        &repository_inputs,
        &worktree_root,
        &temporary_directory,
        environment,
        WorkerArgumentVector::parse(wrapped_arguments)?,
    )?;
    let mut first_location = None;
    let mut created = Vec::new();
    for worktree in plan.worktrees() {
        let response = match execute_herdr(worktree.invocation()) {
            Ok(response) => response,
            Err(source) => {
                record_dispatch_spawn_failure(&command.log_path, &node, issuance.sequence())?;
                remove_clean_created_worktrees(
                    &plan.worktrees()[..created.len()],
                    &command.repositories,
                )?;
                return Err(source);
            }
        };
        if first_location.is_none() {
            first_location = Some(herdr_location(&response)?);
        }
        created.push(json!({
            "repository": worktree.repository(),
            "path": worktree.path(),
            "response": response,
        }));
    }
    for worktree in plan.worktrees() {
        let Some((input, expected_paths)) = command.conflicted_joins.get(worktree.repository())
        else {
            continue;
        };
        let merge = std::process::Command::new("git")
            .arg("-C")
            .arg(worktree.path())
            .args(["merge", "--no-edit", "--no-ff"])
            .arg(&input.oid)
            .output()
            .with_context(|| {
                format!(
                    "failed to prepare conflicted join in repository {}",
                    worktree.repository()
                )
            })?;
        let paths = std::process::Command::new("git")
            .arg("-C")
            .arg(worktree.path())
            .args(["diff", "--name-only", "--diff-filter=U"])
            .output()?;
        let actual_paths = String::from_utf8_lossy(&paths.stdout)
            .lines()
            .map(str::to_owned)
            .filter(|path| !path.is_empty())
            .collect::<Vec<_>>();
        if merge.status.success() || actual_paths != *expected_paths {
            record_dispatch_spawn_failure(&command.log_path, &node, issuance.sequence())?;
            bail!(
                "conflicted join for repository {} did not reproduce: expected {:?}, observed {:?}; stdout: {}; stderr: {}",
                worktree.repository(),
                expected_paths,
                actual_paths,
                String::from_utf8_lossy(&merge.stdout).trim(),
                String::from_utf8_lossy(&merge.stderr).trim()
            );
        }
    }
    let location = match first_location.context("package dispatch composed no worktree") {
        Ok(location) => location,
        Err(source) => {
            record_dispatch_spawn_failure(&command.log_path, &node, issuance.sequence())?;
            return Err(source);
        }
    };
    if let Err(source) =
        materialize_worker_launch_script(plan.launch_script_path(), &plan.launch_script())
    {
        record_dispatch_spawn_failure(&command.log_path, &node, issuance.sequence())?;
        remove_clean_created_worktrees(plan.worktrees(), &command.repositories)?;
        return Err(source);
    }
    let pane_run_response =
        match execute_herdr_pane_run(&plan.pane_run(&location), plan.environment()) {
            HerdrPaneRunOutcome::Started(response) => response,
            HerdrPaneRunOutcome::Refused(source) => {
                record_dispatch_spawn_failure(&command.log_path, &node, issuance.sequence())?;
                let _ignored = fs::remove_file(plan.launch_script_path());
                remove_clean_created_worktrees(plan.worktrees(), &command.repositories)?;
                return Err(source);
            }
            HerdrPaneRunOutcome::SpawnObservedButUnusable(source) => {
                return Err(Error::new(SpawnObservedDispatchError { source }));
            }
        };
    if !wait_for_worker_launch(plan.launch_script_path(), result_path.as_path()) {
        record_dispatch_spawn_failure(&command.log_path, &node, issuance.sequence())?;
        let workspace = HerdrWorkspaceId::parse(location.workspace_id())?;
        let _ignored = execute_herdr(&workspace.close_invocation(plan.session()));
        let _ignored = fs::remove_file(plan.launch_script_path());
        remove_clean_created_worktrees(plan.worktrees(), &command.repositories)?;
        bail!(
            "worker launch timed out before the pane consumed {}",
            plan.launch_script_path().display()
        );
    }
    let worker_pane_id = location.pane().as_str().to_owned();
    let worker_workspace_id = location.workspace_id().to_owned();
    let worker_process =
        observe_spawned_herdr_process(command.herdr_session.as_ref(), &worker_pane_id);
    let mut pane_cleanup_targets = Vec::new();
    let mut pane_ownership_errors = Vec::new();
    for item in &created {
        let pane_id = item
            .pointer("/response/result/root_pane/pane_id")
            .and_then(Value::as_str)
            .context("herdr worktree response omitted result.root_pane.pane_id")
            .and_then(|value| HerdrPaneId::parse(value).map_err(Error::new));
        let workspace_id = item
            .pointer("/response/result/root_pane/workspace_id")
            .and_then(Value::as_str)
            .context("herdr worktree response omitted result.root_pane.workspace_id")
            .and_then(|value| HerdrWorkspaceId::parse(value).map_err(Error::new));
        match (pane_id, workspace_id) {
            (Ok(pane_id), Ok(workspace_id)) => pane_cleanup_targets.push(json!({
                "pane_id": pane_id.as_str(),
                "workspace_id": workspace_id.as_str(),
                "herdr_session": command.herdr_session.as_ref().map(HerdrSessionName::as_str),
            })),
            (pane, workspace) => pane_ownership_errors.push(format!(
                "pane identity: {}; workspace identity: {}",
                pane.err()
                    .map_or_else(|| "available".to_owned(), |error| format!("{error:#}")),
                workspace
                    .err()
                    .map_or_else(|| "available".to_owned(), |error| format!("{error:#}")),
            )),
        }
    }
    let pane_ownership_error =
        (!pane_ownership_errors.is_empty()).then(|| pane_ownership_errors.join("; "));
    let session_path = if is_prime_agent_dispatch(&command.worker_arguments) {
        created
            .first()
            .and_then(|worktree| worktree.get("path"))
            .and_then(Value::as_str)
            .and_then(observe_prime_session_path)
    } else {
        None
    };
    Ok(json!({
        "herdr_version": herdr_version,
        "agent_name": plan.agent_name().as_str(),
        "issuance_sequence": issuance.sequence().get(),
        "result_path": result_path.as_str(),
        "dispatch_identity": {
            "agent_name": plan.agent_name().as_str(),
            "pane_id": worker_pane_id,
            "workspace_id": worker_workspace_id,
            "herdr_session": command.herdr_session.as_ref().map(HerdrSessionName::as_str),
            "session_path": session_path,
            "process": worker_process,
        },
        "pane_cleanup_targets": pane_cleanup_targets,
        "pane_ownership_error": pane_ownership_error,
        "worktrees": created,
        "pane_run": pane_run_response,
    }))
}

fn run_package_dispatch(command: PackageDispatchCommand) -> Result<()> {
    let dispatched = issue_package_dispatch(command)?;
    write_json_stdout(&dispatched)
}

fn parse_dispatch_reconcile(rest: &[String]) -> Result<Command> {
    let [
        file_flag,
        raw_log_path,
        issuance_flag,
        raw_issuance,
        node_flag,
        raw_node,
    ] = rest
    else {
        bail!(USAGE);
    };
    if file_flag != "--file"
        || issuance_flag != "--issuance"
        || node_flag != "--node"
        || raw_log_path.is_empty()
        || raw_issuance.is_empty()
        || raw_node.is_empty()
    {
        bail!(USAGE);
    }
    let log_path = PathBuf::from(raw_log_path);
    if !log_path.is_absolute() {
        bail!(
            "dispatch reconcile event-log path must be absolute: {}",
            log_path.display()
        );
    }
    let raw = raw_issuance.parse::<u64>().map_err(|source| {
        anyhow!("failed to parse dispatch reconciliation issuance sequence: {source}")
    })?;
    let issuance_sequence = Sequence::parse(raw).map_err(|source| {
        anyhow!("failed to parse dispatch reconciliation issuance sequence: {source}")
    })?;
    let node = NodeId::parse(raw_node).context("failed to parse dispatch reconciliation node")?;
    Ok(Command::DispatchReconcile {
        log_path,
        issuance_sequence,
        node,
    })
}

fn parse_dispatch_check_in(rest: &[String]) -> Result<Command> {
    let [file_flag, raw_log_path] = rest else {
        bail!(USAGE);
    };
    if file_flag != "--file" || raw_log_path.is_empty() {
        bail!(USAGE);
    }
    let log_path = PathBuf::from(raw_log_path);
    Ok(Command::DispatchCheckIn { log_path })
}

fn parse_gate_replay(rest: &[String]) -> Result<Command> {
    let [
        repo_flag,
        repo,
        evidence_flag,
        evidence,
        execution_flag,
        execution_ref,
        broken_flag,
        broken_ref,
        repaired_flag,
        repaired_ref,
        schema_flag,
        schema,
        output_flag,
        output,
        expected_flag,
        expected,
    ] = rest
    else {
        bail!(USAGE)
    };
    if repo_flag != "--repo-root"
        || evidence_flag != "--evidence"
        || execution_flag != "--execution-ref"
        || broken_flag != "--broken-ref"
        || repaired_flag != "--repaired-ref"
        || schema_flag != "--schema"
        || output_flag != "--output"
        || expected_flag != "--expected"
    {
        bail!(USAGE);
    }
    let repository_root = PathBuf::from(repo);
    let evidence_path = PathBuf::from(evidence);
    if !repository_root.is_absolute() || !evidence_path.is_absolute() {
        bail!(USAGE);
    }
    Ok(Command::GateReplay(GateReplayCommand {
        repository_root,
        recorded_root: std::env::var_os(PAIRED_RECORDED_ROOT_ENV).map(PathBuf::from),
        evidence_path,
        execution_ref: GateExecutionRef::parse(execution_ref.clone())?,
        broken_ref: NamedReplayRef::parse(broken_ref.clone())?,
        repaired_ref: NamedReplayRef::parse(repaired_ref.clone())?,
        schema_path: parse_replay_schema_path(schema)?,
        output_path: parse_replay_output_path(output)?,
        expected: ExpectedVerdictOutcome::parse(expected)?,
        pause_directory: None,
    }))
}

fn parse_paired_probe_output(raw: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        bail!("execution subject probe output must be a repository-relative ordinary path");
    }
    Ok(path)
}

fn parse_paired_execution_proof(rest: &[String]) -> Result<Command> {
    if rest.len() != 10
        || rest[0] != "--repo-root"
        || rest[2] != "--artifacts"
        || rest[4] != "--env"
        || rest[6] != "--env"
        || rest[8] != "--env"
    {
        bail!(USAGE);
    }
    let mut environment = BTreeMap::new();
    for raw in [&rest[5], &rest[7], &rest[9]] {
        let Some((name, value)) = raw.split_once('=') else {
            bail!("paired execution proof requires exactly PATH, HOME, and USER");
        };
        if environment
            .insert(name.to_owned(), value.to_owned())
            .is_some()
        {
            bail!("paired execution proof requires exactly PATH, HOME, and USER");
        }
    }
    if environment
        .keys()
        .map(String::as_str)
        .ne(["HOME", "PATH", "USER"])
    {
        bail!("paired execution proof requires exactly PATH, HOME, and USER");
    }
    let repository_root = PathBuf::from(&rest[1]);
    if !repository_root.is_absolute() {
        bail!(USAGE);
    }
    let artifact_directory = PathBuf::from(&rest[3]);
    if !artifact_directory.is_absolute()
        || !artifact_directory.is_dir()
        || std::fs::read_dir(&artifact_directory)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(true)
    {
        bail!("paired execution artifact directory must be absolute, existing, and empty");
    }
    Ok(Command::PairedExecutionProof(PairedExecutionProofCommand {
        repository_root,
        artifact_directory,
        environment: ChildEnvironment::new(environment),
    }))
}

fn ordinary_file(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect execution subject {}", path.display()))?;
    if !metadata.file_type().is_file() {
        bail!("execution subject probe requires ordinary artifact and gate files");
    }
    Ok(())
}

fn remove_probe_file(path: &Path, cwd: &Path) -> Result<()> {
    if !path.starts_with(cwd) {
        bail!("execution subject probe output must be a repository-relative ordinary path");
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => std::fs::remove_file(path)
            .with_context(|| format!("failed to remove execution subject file {}", path.display())),
        Ok(_) => {
            bail!("execution subject probe output must be a repository-relative ordinary path")
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::new(error).context(format!(
            "failed to inspect execution subject file {}",
            path.display()
        ))),
    }
}

fn run_execution_subject_probe(relative_output: &Path) -> Result<()> {
    let cwd = std::env::current_dir()
        .context("failed to read execution subject current directory")?
        .canonicalize()
        .context("failed to canonicalize execution subject current directory")?;
    let artifact = cwd.join(PAIRED_SUBJECT_ARTIFACT);
    let gate = cwd.join(PAIRED_SUBJECT_GATE);
    ordinary_file(&artifact)?;
    ordinary_file(&gate)?;
    let output = cwd.join(relative_output);
    remove_probe_file(&output, &cwd)?;
    let path_value = std::env::var("PATH").context("execution subject probe requires PATH")?;
    let home_value = std::env::var("HOME").context("execution subject probe requires HOME")?;
    let user_value = std::env::var("USER").context("execution subject probe requires USER")?;
    let temporary_directory = SynthesizedTemporaryDirectory::create()?;
    let status = std::process::Command::new(gate)
        .current_dir(&cwd)
        .env_clear()
        .env("PATH", path_value)
        .env("HOME", home_value)
        .env("USER", user_value)
        .env("TMPDIR", temporary_directory.path())
        .stdin(Stdio::null())
        .status()
        .context("failed to execute the execution subject gate")?;
    if !status.success() {
        bail!("execution subject gate exited with status {status}");
    }
    std::fs::write(
        output,
        b"{\"verdict\":\"APPROVE\",\"self_sufficiency\":\"NOT_APPLICABLE\",\"root_cause\":\"execution\",\"blocking_issues\":[],\"non_blocking_notes\":[],\"summary\":\"ordinary gate passed\"}\n",
    )
    .context("failed to write execution subject probe output")?;
    Ok(())
}

fn resolve_paired_ref(repository_root: &Path, name: &str, expected: &str) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(repository_root)
        .args(["rev-parse", &format!("{name}^{{commit}}")])
        .output()
        .context("failed to resolve execution subject ref")?;
    let resolved = String::from_utf8(output.stdout)
        .context("execution subject ref was not UTF-8")?
        .trim()
        .to_owned();
    if !output.status.success() || resolved != expected {
        bail!(
            "execution subject ref `{name}` resolved to unexpected commit `{resolved}` (expected `{expected}`)"
        );
    }
    Ok(resolved)
}

fn run_subject_git(repository: &Path, arguments: &[&str], commit_date: Option<&str>) -> Result<()> {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(repository)
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgSign=false"])
        .args(arguments);
    if let Some(date) = commit_date {
        command
            .env("GIT_AUTHOR_NAME", "PCE Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
            .env("GIT_COMMITTER_NAME", "PCE Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date);
    }
    let output = command
        .output()
        .context("failed to construct paired execution subject repository")?;
    if !output.status.success() {
        bail!(
            "failed to construct paired execution subject repository with `git {}`: stdout: {}; stderr: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stdout).trim(),
            String::from_utf8_lossy(&output.stderr).trim(),
        );
    }
    Ok(())
}

fn copy_subject_file(source: &Path, destination: &Path) -> Result<()> {
    ordinary_file(source)?;
    match std::fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_file() => std::fs::remove_file(destination)
            .context("failed to replace execution subject fixture file")?,
        Ok(_) => bail!("execution subject fixture destination must be an ordinary file"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(Error::new(error).context("failed to inspect execution subject fixture"));
        }
    }
    std::fs::copy(source, destination).with_context(|| {
        format!(
            "failed to materialize execution subject fixture {}",
            source.display()
        )
    })?;
    Ok(())
}

fn materialize_subject_repository(source_root: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir(destination)
        .context("failed to create paired execution subject repository")?;
    run_subject_git(
        destination,
        &["init", "--quiet", "--initial-branch=subject"],
        None,
    )?;
    let fixture = source_root.join(PAIRED_SUBJECT_FIXTURE);
    for (source, target) in [
        (fixture.join("common/.gitignore"), ".gitignore"),
        (fixture.join("common/gate"), PAIRED_SUBJECT_GATE),
        (
            fixture.join("common/verdict.schema.json"),
            PAIRED_REPLAY_SCHEMA,
        ),
        (fixture.join("broken/artifact"), PAIRED_SUBJECT_ARTIFACT),
    ] {
        copy_subject_file(&source, &destination.join(target))?;
    }
    run_subject_git(
        destination,
        &[
            "add",
            ".gitignore",
            "artifact",
            "gate",
            "verdict.schema.json",
        ],
        None,
    )?;
    run_subject_git(
        destination,
        &["commit", "--quiet", "-m", "execution subject snapshot"],
        Some("2000-01-01T00:00:00Z"),
    )?;
    resolve_paired_ref(destination, PAIRED_BROKEN_REF, PAIRED_BROKEN_OID)?;
    run_subject_git(
        destination,
        &["checkout", "--quiet", "--orphan", "replacement"],
        None,
    )?;
    copy_subject_file(
        &fixture.join("repaired/artifact"),
        &destination.join(PAIRED_SUBJECT_ARTIFACT),
    )?;
    run_subject_git(
        destination,
        &[
            "add",
            ".gitignore",
            "artifact",
            "gate",
            "verdict.schema.json",
        ],
        None,
    )?;
    run_subject_git(
        destination,
        &["commit", "--quiet", "-m", "execution subject snapshot"],
        Some("2000-01-01T00:00:00Z"),
    )?;
    resolve_paired_ref(destination, PAIRED_REPAIRED_REF, PAIRED_REPAIRED_OID)?;
    run_subject_git(destination, &["branch", "-D", "subject"], None)?;
    run_subject_git(destination, &["branch", "-m", "subject"], None)?;
    Ok(())
}

fn materialize_subject_side_repository(
    source_root: &Path,
    destination: &Path,
    artifact_fixture: &str,
    expected_oid: &str,
) -> Result<()> {
    std::fs::create_dir(destination)
        .context("failed to create side-blind execution subject repository")?;
    run_subject_git(
        destination,
        &["init", "--quiet", "--initial-branch=subject"],
        None,
    )?;
    let fixture = source_root.join(PAIRED_SUBJECT_FIXTURE);
    for (source, target) in [
        (fixture.join("common/.gitignore"), ".gitignore"),
        (fixture.join("common/gate"), PAIRED_SUBJECT_GATE),
        (
            fixture.join("common/verdict.schema.json"),
            PAIRED_REPLAY_SCHEMA,
        ),
        (fixture.join(artifact_fixture), PAIRED_SUBJECT_ARTIFACT),
    ] {
        copy_subject_file(&source, &destination.join(target))?;
    }
    run_subject_git(
        destination,
        &[
            "add",
            ".gitignore",
            "artifact",
            "gate",
            "verdict.schema.json",
        ],
        None,
    )?;
    run_subject_git(
        destination,
        &["commit", "--quiet", "-m", "execution subject snapshot"],
        Some("2000-01-01T00:00:00Z"),
    )?;
    resolve_paired_ref(destination, "HEAD", expected_oid)?;
    Ok(())
}

struct PairedWorktrees {
    source_root: PathBuf,
    repository_root: PathBuf,
    parent: PathBuf,
    paths: Vec<PathBuf>,
    checkout_parents: Vec<PathBuf>,
}

impl PairedWorktrees {
    fn create(source_root: &Path) -> Result<Self> {
        let parent = allocate_opaque_temporary_path()?;
        let repository_root = parent.join("subject-repository");
        Ok(Self {
            source_root: source_root.to_path_buf(),
            repository_root,
            parent,
            paths: Vec::new(),
            checkout_parents: Vec::new(),
        })
    }

    fn materialize_replay_repository(&self) -> Result<()> {
        std::fs::create_dir(&self.parent)
            .context("failed to create paired replay repository parent")?;
        materialize_subject_repository(&self.source_root, &self.repository_root)
    }

    fn add(&mut self, artifact_fixture: &str, oid: &str) -> Result<PathBuf> {
        let checkout_parent = create_paired_checkout_parent()?;
        let path = checkout_parent.join("checkout");
        if let Err(error) =
            materialize_subject_side_repository(&self.source_root, &path, artifact_fixture, oid)
        {
            let _cleanup = std::fs::remove_dir_all(&checkout_parent);
            return Err(error);
        }
        let path = path
            .canonicalize()
            .context("failed to canonicalize isolated subject checkout")?;
        self.checkout_parents.push(checkout_parent);
        self.paths.push(path.clone());
        Ok(path)
    }

    fn remove(&mut self, path: &Path) -> Result<()> {
        let index = self
            .paths
            .iter()
            .position(|tracked| tracked == path)
            .ok_or_else(|| anyhow!("paired campaign cleanup target was not tracked"))?;
        self.paths.remove(index);
        let parent = self.checkout_parents.remove(index);
        std::fs::remove_dir_all(parent).context("paired campaign cleanup failed")
    }

    fn cleanup(&mut self) -> Result<()> {
        let mut failure = None;
        self.paths.clear();
        for parent in self.checkout_parents.drain(..).rev() {
            if let Err(error) = std::fs::remove_dir_all(parent) {
                failure = Some(Error::new(error).context("paired campaign cleanup failed"));
            }
        }
        if self.repository_root.exists()
            && let Err(error) = std::fs::remove_dir_all(&self.repository_root)
        {
            failure = Some(Error::new(error).context("paired campaign cleanup failed"));
        }
        if self.parent.exists()
            && let Err(error) = std::fs::remove_dir(&self.parent)
        {
            failure = Some(Error::new(error).context("paired campaign cleanup failed"));
        }
        failure.map_or(Ok(()), Err)
    }
}

fn create_paired_checkout_parent() -> Result<PathBuf> {
    create_opaque_temporary_directory()
}

fn create_opaque_temporary_directory() -> Result<PathBuf> {
    let path = allocate_opaque_temporary_path()?;
    std::fs::create_dir(&path).context("failed to create isolated subject parent")?;
    Ok(path)
}

fn allocate_opaque_temporary_path() -> Result<PathBuf> {
    for _attempt in 0..1000 {
        let mut opaque = [0_u8; 16];
        File::open("/dev/urandom")
            .context("failed to open operating-system randomness for paired checkout")?
            .read_exact(&mut opaque)
            .context("failed to read operating-system randomness for paired checkout")?;
        let identifier = opaque
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = std::env::temp_dir().join(identifier);
        if !path.exists() {
            return Ok(path);
        }
    }
    bail!("failed to allocate isolated subject parent")
}

struct PairedCriticCapture {
    verdict: Vec<u8>,
    evidence: Vec<u8>,
}

fn opaque_dispatch_paths(checkout: &Path) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let path = allocate_opaque_temporary_path()?;
    let identifier = path
        .file_name()
        .ok_or_else(|| anyhow!("opaque dispatch directory has no name"))?
        .to_owned();
    let base = checkout.join(identifier);
    Ok((
        base.with_extension("schema"),
        base.with_extension("json"),
        base.with_extension("jsonl"),
    ))
}

fn materialize_paired_gate_exec_client(checkout: &Path) -> Result<PathBuf> {
    let path = allocate_opaque_temporary_path()?;
    let identifier = path
        .file_name()
        .ok_or_else(|| anyhow!("opaque recorder client has no name"))?;
    let client = checkout.join(identifier);
    std::fs::write(
        &client,
        b"#!/bin/sh\nset -eu\n[ \"$#\" -eq 2 ]\n[ \"$1\" = gate ]\n[ \"$2\" = exec ]\nexec /usr/bin/nc -U \"$PCE_GATE_EXEC_SOCKET\"\n",
    )
    .context("failed to materialize isolated recorder client")?;
    std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o500))
        .context("failed to make isolated recorder client executable")?;
    client
        .canonicalize()
        .context("failed to canonicalize isolated recorder client")
}

fn capture_paired_critic(verdict: &Path) -> Result<PairedCriticCapture> {
    let evidence = AbsoluteGateExecutionEvidencePath::from_verdict_path(verdict);
    Ok(PairedCriticCapture {
        verdict: std::fs::read(verdict).context("failed to capture paired critic verdict")?,
        evidence: std::fs::read(evidence.as_path())
            .context("failed to capture paired critic execution evidence")?,
    })
}

fn retain_paired_critic_capture(capture: &PairedCriticCapture, destination: &Path) -> Result<()> {
    std::fs::write(destination, &capture.verdict)
        .context("failed to retain paired critic verdict")?;
    let evidence = AbsoluteGateExecutionEvidencePath::from_verdict_path(destination);
    std::fs::write(evidence.as_path(), &capture.evidence)
        .context("failed to retain paired critic execution evidence")
}

impl Drop for PairedWorktrees {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            tracing::error!(error = ?error, "paired campaign drop cleanup failed");
        }
    }
}

fn run_paired_child(
    command: &mut std::process::Command,
    deadline: Instant,
    timeout: Duration,
    timeout_diagnostic: &'static str,
) -> Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command.process_group(0);
    let mut child = command
        .spawn()
        .context("failed to spawn paired campaign child")?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture paired campaign child stdout"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("failed to capture paired campaign child stderr"))?;
    let stdout_worker = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_worker = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });
    let child_deadline = deadline.min(Instant::now() + timeout);
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .context("failed to wait for paired campaign child")?
        {
            break status;
        }
        if Instant::now() >= child_deadline {
            let _group_kill = std::process::Command::new("/bin/kill")
                .args(["-KILL", &format!("-{}", child.id())])
                .status();
            let _child_kill = child.kill();
            let _reap = child.wait();
            let _stdout = stdout_worker.join();
            let _stderr = stderr_worker.join();
            if Instant::now() >= deadline {
                bail!("paired execution proof exceeded its 40 minute overall deadline");
            }
            bail!(timeout_diagnostic);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout_worker
        .join()
        .map_err(|_| anyhow!("paired campaign stdout worker panicked"))??;
    let stderr = stderr_worker
        .join()
        .map_err(|_| anyhow!("paired campaign stderr worker panicked"))??;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn run_paired_critic(
    executable: &Path,
    checkout: &Path,
    schema: &Path,
    verdict: &Path,
    log: &Path,
    environment: &ChildEnvironment,
    deadline: Instant,
) -> Result<()> {
    let task = paired_critic_task();
    let gate_exec_client = materialize_paired_gate_exec_client(checkout)?;
    let mut command = std::process::Command::new(executable);
    command.env(PAIRED_GATE_EXEC_CLIENT_ENV, &gate_exec_client);
    command.args(["dispatch", "gate", "--cwd"]).arg(checkout);
    for (name, value) in environment.iter() {
        command.args(["--env", &format!("{name}={value}")]);
    }
    command
        .args(["--output-schema"])
        .arg(schema)
        .arg("-o")
        .arg(verdict)
        .args(["--log-file"])
        .arg(log)
        .args([
            "--node",
            "m1-s1",
            "--role",
            "falsification-critic",
            "--ref",
            "campaign-subject",
            "--evidence",
            "campaign-review",
        ])
        .arg("--required-artifact")
        .arg(verdict)
        .args(["--", &task]);
    let output = run_paired_child(
        &mut command,
        deadline,
        PAIRED_CRITIC_TIMEOUT,
        "paired execution critic timed out after 15 minutes",
    )?;
    if !output.status.success() {
        bail!(
            "paired execution critic failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    wait_for_paired_critic_completion(log, verdict, deadline)?;
    Ok(())
}

fn wait_for_paired_critic_completion(log: &Path, verdict: &Path, deadline: Instant) -> Result<()> {
    let evidence = AbsoluteGateExecutionEvidencePath::from_verdict_path(verdict);
    loop {
        match std::fs::read_to_string(log) {
            Ok(contents) => {
                let records = contents
                    .lines()
                    .map(parse_event_line)
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .context("paired critic event log is malformed")?;
                if records.len() == 2 {
                    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) =
                        records[1].body_ref()
                    else {
                        bail!("paired critic second lifecycle record is not a completion")
                    };
                    if completion.issuance_sequence() != records[0].sequence() {
                        bail!("paired critic completion does not name its issuance")
                    }
                    match std::fs::read(evidence.as_path()) {
                        Ok(bytes) if parse_gate_execution_evidence(&bytes).is_ok() => return Ok(()),
                        Ok(_) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => {
                            return Err(error)
                                .context("failed to read paired critic execution evidence");
                        }
                    }
                }
                if records.len() > 2 {
                    bail!("paired critic lifecycle contains more than two records")
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).context("failed to read paired critic event log");
            }
        }
        if Instant::now() >= deadline {
            bail!("paired execution campaign timed out before critic completion")
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn materialize_paired_probe(checkout: &Path) -> Result<PathBuf> {
    let executable = checkout.join(PAIRED_PROGRAM_RELATIVE);
    let parent = executable
        .parent()
        .ok_or_else(|| anyhow!("paired campaign executable path has no parent"))?;
    std::fs::create_dir_all(parent).context("failed to create paired campaign executable root")?;
    std::fs::write(
        &executable,
        b"#!/bin/sh\nset -eu\n[ \"$#\" -eq 4 ]\n[ \"$1\" = gate ]\n[ \"$2\" = execution-subject-probe ]\n[ \"$3\" = --output ]\ncase \"$4\" in /*|*../*|../*) exit 2;; esac\n./gate\nprintf '%s\\n' '{\"verdict\":\"APPROVE\",\"self_sufficiency\":\"NOT_APPLICABLE\",\"root_cause\":\"execution\",\"blocking_issues\":[],\"non_blocking_notes\":[],\"summary\":\"ordinary gate passed\"}' > \"$4\"\n",
    )
    .context("failed to materialize paired probe program")?;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
        .context("failed to make paired probe executable")?;
    Ok(executable)
}

fn replay_classification(report: &[u8]) -> Result<PairedReplayClassification> {
    let value: Value =
        serde_json::from_slice(report).context("failed to parse paired replay report")?;
    match value.get("classification").and_then(Value::as_str) {
        Some("repair-sensitive") => Ok(PairedReplayClassification::RepairSensitive),
        Some("no-repair-signal") => Ok(PairedReplayClassification::NoRepairSignal),
        Some("opposite-direction") => Ok(PairedReplayClassification::OppositeDirection),
        Some("non-reproducible") => Ok(PairedReplayClassification::NonReproducible),
        Some("checkout-failed") => Ok(PairedReplayClassification::CheckoutFailed),
        Some("oracle-failed") => Ok(PairedReplayClassification::OracleFailed),
        _ => bail!("paired replay report has an unknown classification"),
    }
}

fn run_paired_replay(
    executable: &Path,
    replay_repository: &Path,
    recorded_root: &Path,
    evidence: &Path,
    execution_ref: &GateExecutionRef,
    report_path: &Path,
    deadline: Instant,
) -> Result<PairedReplayClassification> {
    let mut command = std::process::Command::new(executable);
    command.args(paired_replay_arguments(
        replay_repository,
        evidence,
        execution_ref,
    ));
    command.env(PAIRED_REPLAY_PROGRAM_ENV, PAIRED_PROGRAM_RELATIVE);
    command.env(PAIRED_RECORDED_ROOT_ENV, recorded_root);
    let output = run_paired_child(
        &mut command,
        deadline,
        REPLAY_OVERALL_TIMEOUT + Duration::from_secs(5),
        "paired replay did not complete",
    )?;
    if !output.status.success() {
        bail!(
            "paired replay failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    std::fs::write(report_path, &output.stdout).context("failed to retain paired replay report")?;
    replay_classification(&output.stdout)
}

fn write_paired_unreplayable_report(
    report_path: &Path,
    execution_ref: &GateExecutionRef,
    reason: &str,
) -> Result<()> {
    let mut bytes = serde_json::to_vec(&json!({
        "schema_id": "pce.paired-replayability",
        "schema_version": 1,
        "execution_ref": execution_ref.as_str(),
        "replayability": "unreplayable",
        "reason": reason,
    }))
    .context("failed to serialize paired unreplayable report")?;
    bytes.push(b'\n');
    std::fs::write(report_path, bytes).context("failed to retain paired unreplayable report")
}

fn paired_record_is_replayable(
    record: &GateExecutionRecord,
    root: &Path,
    report_path: &Path,
) -> Result<bool> {
    match paired_stimulus_identity(&record.stimulus, root) {
        Ok(_) => Ok(true),
        Err(PairedExecutionProofError::StimulusOutsideCampaignRoot) => {
            write_paired_unreplayable_report(
                report_path,
                &record.execution_ref,
                "working-directory-outside-repository-root",
            )?;
            Ok(false)
        }
        Err(PairedExecutionProofError::StimulusNamesFixedAbsolutePath) => {
            write_paired_unreplayable_report(
                report_path,
                &record.execution_ref,
                "fixed-absolute-path-outside-repository-root",
            )?;
            Ok(false)
        }
        Err(error) => Err(error.into()),
    }
}

fn paired_replay_arguments(
    checkout: &Path,
    evidence: &Path,
    execution_ref: &GateExecutionRef,
) -> Vec<OsString> {
    [
        OsString::from("gate"),
        OsString::from("replay"),
        OsString::from("--repo-root"),
        checkout.as_os_str().to_owned(),
        OsString::from("--evidence"),
        evidence.as_os_str().to_owned(),
        OsString::from("--execution-ref"),
        OsString::from(execution_ref.as_str()),
        OsString::from("--broken-ref"),
        OsString::from(PAIRED_BROKEN_REF),
        OsString::from("--repaired-ref"),
        OsString::from(PAIRED_REPAIRED_REF),
        OsString::from("--schema"),
        OsString::from(PAIRED_REPLAY_SCHEMA),
        OsString::from("--output"),
        OsString::from(PAIRED_REPLAY_OUTPUT),
        OsString::from("--expected"),
        OsString::from(PAIRED_REPLAY_EXPECTED),
    ]
    .into()
}

struct PairedProofReport {
    schema_id: &'static str,
    schema_version: u32,
    broken_ref: &'static str,
    repaired_ref: &'static str,
    broken_verdict: &'static str,
    broken_blocking_issue_count: usize,
    broken_witnesses: Vec<String>,
    repaired_verdict: &'static str,
    repaired_blocking_issue_count: usize,
    repaired_probes: Vec<String>,
    decision: &'static str,
}

fn serialize_paired_proof_report(report: &PairedProofReport) -> Result<Vec<u8>> {
    let broken_witnesses = serde_json::to_string(&report.broken_witnesses)
        .context("failed to serialize broken paired witnesses")?;
    let repaired_probes = serde_json::to_string(&report.repaired_probes)
        .context("failed to serialize repaired paired probes")?;
    Ok(format!(
        "{{\"schema_id\":\"{}\",\"schema_version\":{},\"broken_ref\":\"{}\",\"repaired_ref\":\"{}\",\"broken_verdict\":\"{}\",\"broken_blocking_issue_count\":{},\"broken_witnesses\":{},\"repaired_verdict\":\"{}\",\"repaired_blocking_issue_count\":{},\"repaired_probes\":{},\"decision\":\"{}\"}}\n",
        report.schema_id,
        report.schema_version,
        report.broken_ref,
        report.repaired_ref,
        report.broken_verdict,
        report.broken_blocking_issue_count,
        broken_witnesses,
        report.repaired_verdict,
        report.repaired_blocking_issue_count,
        repaired_probes,
        report.decision,
    )
    .into_bytes())
}

fn paired_utc_now() -> Result<String> {
    let output = std::process::Command::new("/bin/date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .context("failed to read paired campaign UTC time")?;
    if !output.status.success() {
        bail!("failed to read paired campaign UTC time");
    }
    String::from_utf8(output.stdout)
        .context("paired campaign UTC time was not UTF-8")
        .map(|value| value.trim().to_owned())
}

fn paired_head_commit(repository_root: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["-C"])
        .arg(repository_root)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .context("failed to resolve paired campaign head commit")?;
    if !output.status.success() {
        bail!("failed to resolve paired campaign head commit");
    }
    String::from_utf8(output.stdout)
        .context("paired campaign head commit was not UTF-8")
        .map(|value| value.trim().to_owned())
}

fn paired_sha256(path: &Path) -> Result<(u64, String)> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to hash paired artifact {}", path.display()))?;
    let byte_count =
        u64::try_from(bytes.len()).context("paired artifact byte count exceeds u64")?;
    Ok((byte_count, format!("{:x}", Sha256::digest(&bytes))))
}

fn paired_retained_artifact_paths(artifact_directory: &Path) -> Result<Vec<PathBuf>> {
    fn visit(root: &Path, directory: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
        for entry in std::fs::read_dir(directory).with_context(|| {
            format!(
                "failed to enumerate paired artifacts in {}",
                directory.display()
            )
        })? {
            let entry = entry.context("failed to enumerate paired artifact")?;
            let file_type = entry
                .file_type()
                .context("failed to inspect paired artifact type")?;
            if file_type.is_dir() {
                visit(root, &entry.path(), paths)?;
            } else if file_type.is_file() {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .context("paired artifact escaped its artifact directory")?
                    .to_path_buf();
                if relative != Path::new("provenance-manifest.json") {
                    paths.push(relative);
                }
            } else {
                bail!("paired artifact must be an ordinary file");
            }
        }
        Ok(())
    }

    let mut paths = Vec::new();
    visit(artifact_directory, artifact_directory, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn write_paired_provenance_manifest(
    repository_root: &Path,
    executable: &Path,
    artifact_directory: &Path,
    started_at_utc: String,
) -> Result<()> {
    let artifact_paths = paired_retained_artifact_paths(artifact_directory)?;
    let mut artifacts = Vec::with_capacity(artifact_paths.len());
    for path in artifact_paths {
        let (byte_count, sha256) = paired_sha256(&artifact_directory.join(&path))?;
        artifacts.push(json!({
            "path": path
                .to_str()
                .context("paired artifact path was not UTF-8")?,
            "byte_count": byte_count,
            "sha256": sha256,
        }));
    }
    let (_, campaign_binary_sha256) = paired_sha256(executable)?;
    let manifest = json!({
        "schema_id": "pce.paired-execution-proof-provenance",
        "schema_version": 1,
        "campaign_binary_sha256": campaign_binary_sha256,
        "base_commit": PAIRED_DELIVERY_BASE_OID,
        "head_commit": paired_head_commit(repository_root)?,
        "started_at_utc": started_at_utc,
        "ended_at_utc": paired_utc_now()?,
        "direct_argv": [
            "<CAMPAIGN_BINARY>",
            "gate",
            "paired-execution-proof",
            "--repo-root",
            "<REPOSITORY_ROOT>",
            "--artifacts",
            "<ARTIFACT_DIRECTORY>",
            "--env",
            "PATH=<redacted>",
            "--env",
            "HOME=<redacted>",
            "--env",
            "USER=<redacted>",
        ],
        "broken_ref": PAIRED_BROKEN_OID,
        "repaired_ref": PAIRED_REPAIRED_OID,
        "artifacts": artifacts,
        "limitation": PAIRED_PROVENANCE_LIMITATION,
    });
    let mut bytes =
        serde_json::to_vec(&manifest).context("failed to serialize paired provenance manifest")?;
    bytes.push(b'\n');
    std::fs::write(artifact_directory.join("provenance-manifest.json"), bytes)
        .context("failed to write paired provenance manifest")
}

fn run_paired_execution_proof(command: PairedExecutionProofCommand) -> Result<()> {
    let started_at_utc = paired_utc_now()?;
    let overall_deadline = Instant::now() + PAIRED_CAMPAIGN_TIMEOUT;
    let repository_root = command
        .repository_root
        .canonicalize()
        .context("failed to canonicalize paired execution repository root")?;
    let artifact_directory = command
        .artifact_directory
        .canonicalize()
        .context("failed to canonicalize paired execution artifact directory")?;
    let executable = std::env::current_exe()
        .context("failed to locate paired campaign executable")?
        .canonicalize()
        .context("failed to canonicalize paired campaign executable")?;
    let mut worktrees = PairedWorktrees::create(&repository_root)?;
    let campaign_result = (|| -> Result<bool> {
        let broken_root = worktrees.add("broken/artifact", PAIRED_BROKEN_OID)?;
        materialize_paired_probe(&broken_root)?;
        let (first_schema, first_verdict, first_log) = opaque_dispatch_paths(&broken_root)?;
        std::fs::write(&first_schema, VERDICT_SCHEMA)
            .context("failed to materialize embedded paired verdict schema")?;
        run_paired_critic(
            &executable,
            &broken_root,
            &first_schema,
            &first_verdict,
            &first_log,
            &command.environment,
            overall_deadline,
        )?;
        let first_capture = capture_paired_critic(&first_verdict)?;
        worktrees.remove(&broken_root)?;
        let repaired_root = worktrees.add("repaired/artifact", PAIRED_REPAIRED_OID)?;
        materialize_paired_probe(&repaired_root)?;
        let (second_schema, second_verdict, second_log) = opaque_dispatch_paths(&repaired_root)?;
        std::fs::write(&second_schema, VERDICT_SCHEMA)
            .context("failed to materialize embedded paired verdict schema")?;
        run_paired_critic(
            &executable,
            &repaired_root,
            &second_schema,
            &second_verdict,
            &second_log,
            &command.environment,
            overall_deadline,
        )?;
        let second_capture = capture_paired_critic(&second_verdict)?;
        worktrees.remove(&repaired_root)?;
        worktrees.materialize_replay_repository()?;
        resolve_paired_ref(
            &worktrees.repository_root,
            PAIRED_BROKEN_REF,
            PAIRED_BROKEN_OID,
        )?;
        resolve_paired_ref(
            &worktrees.repository_root,
            PAIRED_REPAIRED_REF,
            PAIRED_REPAIRED_OID,
        )?;
        let broken_verdict_path = artifact_directory.join("broken-verdict.json");
        let repaired_verdict_path = artifact_directory.join("repaired-verdict.json");
        retain_paired_critic_capture(&first_capture, &broken_verdict_path)?;
        retain_paired_critic_capture(&second_capture, &repaired_verdict_path)?;
        let broken_bytes =
            std::fs::read(&broken_verdict_path).context("failed to read broken paired verdict")?;
        let repaired_bytes = std::fs::read(&repaired_verdict_path)
            .context("failed to read repaired paired verdict")?;
        let broken_evidence_path =
            AbsoluteGateExecutionEvidencePath::from_verdict_path(&broken_verdict_path);
        let repaired_evidence_path =
            AbsoluteGateExecutionEvidencePath::from_verdict_path(&repaired_verdict_path);
        let broken_evidence_bytes = std::fs::read(broken_evidence_path.as_path())
            .context("failed to read broken paired evidence")?;
        let repaired_evidence_bytes = std::fs::read(repaired_evidence_path.as_path())
            .context("failed to read repaired paired evidence")?;
        let broken_evidence = parse_gate_execution_evidence(&broken_evidence_bytes)?;
        let repaired_evidence = parse_gate_execution_evidence(&repaired_evidence_bytes)?;
        validate_verdict_references(&broken_bytes, broken_evidence.executions())?;
        validate_verdict_references(&repaired_bytes, repaired_evidence.executions())?;
        let broken_verdict = parse_paired_falsification_verdict(&broken_bytes)?;
        let repaired_verdict = parse_paired_falsification_verdict(&repaired_bytes)?;
        let replay_directory = artifact_directory.join("replays");
        std::fs::create_dir(&replay_directory)
            .context("failed to create paired replay directory")?;
        let mut broken_replays = BTreeMap::new();
        let mut broken_replay_paths = BTreeMap::new();
        let broken_primary_references = broken_verdict
            .blocking_issues()
            .iter()
            .filter_map(|issue| issue.primary().cloned())
            .collect::<Vec<_>>();
        for record in broken_evidence
            .executions()
            .iter()
            .filter(|record| broken_primary_references.contains(&record.execution_ref))
        {
            let reference = &record.execution_ref;
            let report = replay_directory.join(format!("broken-{}.json", reference.as_str()));
            if !paired_record_is_replayable(record, &broken_root, &report)? {
                continue;
            }
            let classification = run_paired_replay(
                &executable,
                &worktrees.repository_root,
                &broken_root,
                broken_evidence_path.as_path(),
                reference,
                &report,
                overall_deadline,
            )?;
            broken_replays.insert(reference.clone(), classification);
            broken_replay_paths.insert(reference.clone(), report);
        }
        let mut repaired_replays = BTreeMap::new();
        let mut repaired_replay_paths = BTreeMap::new();
        for record in repaired_evidence.executions() {
            let report =
                replay_directory.join(format!("repaired-{}.json", record.execution_ref.as_str()));
            if !paired_record_is_replayable(record, &repaired_root, &report)? {
                continue;
            }
            let classification = run_paired_replay(
                &executable,
                &worktrees.repository_root,
                &repaired_root,
                repaired_evidence_path.as_path(),
                &record.execution_ref,
                &report,
                overall_deadline,
            )?;
            repaired_replays.insert(record.execution_ref.clone(), classification);
            repaired_replay_paths.insert(record.execution_ref.clone(), report);
        }
        let folded = fold_paired_execution_proof(
            PairedCampaign {
                side: pce_core::CampaignSide::Broken,
                root: &broken_root,
                verdict: &broken_verdict,
                evidence: &broken_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &ReplayClassifications::new(broken_replays),
            },
            PairedCampaign {
                side: pce_core::CampaignSide::Repaired,
                root: &repaired_root,
                verdict: &repaired_verdict,
                evidence: &repaired_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &ReplayClassifications::new(repaired_replays),
            },
        );
        let approved = folded.decision == pce_core::PairedProofDecision::Approve;
        if approved {
            let broken_reference = folded
                .broken_witnesses
                .first()
                .ok_or_else(|| anyhow!("approved paired proof has no broken witness"))?;
            let repaired_reference = folded
                .repaired_probes
                .first()
                .ok_or_else(|| anyhow!("approved paired proof has no repaired probe"))?;
            let broken_report = broken_replay_paths
                .get(broken_reference)
                .ok_or_else(|| anyhow!("approved paired broken witness has no replay report"))?;
            let repaired_report = repaired_replay_paths
                .get(repaired_reference)
                .ok_or_else(|| anyhow!("approved paired repaired probe has no replay report"))?;
            std::fs::copy(broken_report, replay_directory.join("broken-witness.json"))
                .context("failed to retain fixed broken witness replay")?;
            std::fs::copy(
                repaired_report,
                replay_directory.join("repaired-probe.json"),
            )
            .context("failed to retain fixed repaired probe replay")?;
        }
        let report = PairedProofReport {
            schema_id: "pce.paired-execution-proof",
            schema_version: 1,
            broken_ref: PAIRED_BROKEN_OID,
            repaired_ref: PAIRED_REPAIRED_OID,
            broken_verdict: match broken_verdict.token() {
                pce_core::FalsificationVerdictToken::Approve => "APPROVE",
                pce_core::FalsificationVerdictToken::Revise => "REVISE",
                pce_core::FalsificationVerdictToken::Block => "BLOCK",
            },
            broken_blocking_issue_count: broken_verdict.blocking_issues().len(),
            broken_witnesses: folded
                .broken_witnesses
                .iter()
                .map(|reference| reference.as_str().to_owned())
                .collect(),
            repaired_verdict: match repaired_verdict.token() {
                pce_core::FalsificationVerdictToken::Approve => "APPROVE",
                pce_core::FalsificationVerdictToken::Revise => "REVISE",
                pce_core::FalsificationVerdictToken::Block => "BLOCK",
            },
            repaired_blocking_issue_count: repaired_verdict.blocking_issues().len(),
            repaired_probes: folded
                .repaired_probes
                .iter()
                .map(|reference| reference.as_str().to_owned())
                .collect(),
            decision: if approved { "APPROVE" } else { "REFUSE" },
        };
        let bytes = serialize_paired_proof_report(&report)?;
        std::fs::write(
            artifact_directory.join("paired-execution-proof.json"),
            &bytes,
        )
        .context("failed to write paired proof report")?;
        write_paired_provenance_manifest(
            &repository_root,
            &executable,
            &artifact_directory,
            started_at_utc.clone(),
        )?;
        let stdout = std::io::stdout();
        let mut output = stdout.lock();
        output
            .write_all(&bytes)
            .context("failed to write paired proof stdout")?;
        output
            .flush()
            .context("failed to flush paired proof stdout")?;
        Ok(approved)
    })();
    let cleanup_result = worktrees.cleanup();
    let approved = campaign_result?;
    cleanup_result?;
    if !approved {
        bail!("paired execution proof refused");
    }
    Ok(())
}

fn validate_codex_output_schema(path: &Path) -> Result<()> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read Codex output schema {}", path.display()))?;
    let schema: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("Codex output schema {} is not valid JSON", path.display()))?;
    validate_codex_schema_node(&schema, "$", true).with_context(|| {
        format!(
            "Codex output schema {} violates the strict structured-output dialect",
            path.display()
        )
    })
}

fn validate_codex_schema_node(schema: &Value, location: &str, root: bool) -> Result<()> {
    let object = schema
        .as_object()
        .with_context(|| format!("schema node {location} must be an object"))?;
    if root && object.get("type").and_then(Value::as_str) != Some("object") {
        bail!("root schema type must be object");
    }
    if object.get("type").and_then(Value::as_str) == Some("object") {
        let properties = object
            .get("properties")
            .and_then(Value::as_object)
            .with_context(|| format!("object schema {location} must declare properties"))?;
        if object.get("additionalProperties") != Some(&Value::Bool(false)) {
            bail!("object schema {location} must set additionalProperties to false");
        }
        let required = object
            .get("required")
            .and_then(Value::as_array)
            .with_context(|| format!("object schema {location} must declare required"))?;
        let required_names = required
            .iter()
            .map(|value| {
                value.as_str().map(str::to_owned).with_context(|| {
                    format!("object schema {location} has a non-string required entry")
                })
            })
            .collect::<Result<std::collections::BTreeSet<_>>>()?;
        let property_names = properties
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if required_names != property_names {
            bail!("object schema {location} required must contain every and only property name");
        }
        for (name, child) in properties {
            validate_codex_schema_node(child, &format!("{location}/properties/{name}"), false)?;
        }
    }
    if object.get("type").and_then(Value::as_str) == Some("array") {
        let items = object
            .get("items")
            .with_context(|| format!("array schema {location} must declare items"))?;
        validate_codex_schema_node(items, &format!("{location}/items"), false)?;
    }
    for keyword in ["anyOf", "oneOf"] {
        if let Some(branches) = object.get(keyword) {
            let branches = branches
                .as_array()
                .with_context(|| format!("schema {location}/{keyword} must be an array"))?;
            for (index, branch) in branches.iter().enumerate() {
                validate_codex_schema_node(
                    branch,
                    &format!("{location}/{keyword}/{index}"),
                    false,
                )?;
            }
        }
    }
    if let Some(definitions) = object.get("$defs") {
        let definitions = definitions
            .as_object()
            .with_context(|| format!("schema {location}/$defs must be an object"))?;
        for (name, definition) in definitions {
            validate_codex_schema_node(definition, &format!("{location}/$defs/{name}"), false)?;
        }
    }
    Ok(())
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

    const LOGGING_DIAGNOSTIC: &str = "dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence, --required-artifact";
    let logging_raw = if rest
        .get(position)
        .is_some_and(|value| value == "--log-file")
    {
        let parsed = (|| -> Result<(&str, &str, &str, &str, &str, &str)> {
            Ok((
                required_option(rest, &mut position, "--log-file")?,
                required_option(rest, &mut position, "--node")?,
                required_option(rest, &mut position, "--role")?,
                required_option(rest, &mut position, "--ref")?,
                required_option(rest, &mut position, "--evidence")?,
                required_option(rest, &mut position, "--required-artifact")?,
            ))
        })();
        match parsed {
            Ok(values) => Some(values),
            Err(_) => bail!(LOGGING_DIAGNOSTIC),
        }
    } else if rest
        .get(position)
        .is_some_and(|value| value == "--planning-act")
    {
        return Err(pce_core::PlanningFrameError::MissingLoggingMetadata.into());
    } else if rest.get(position).is_some_and(|value| {
        matches!(
            value.as_str(),
            "--node" | "--role" | "--ref" | "--evidence" | "--required-artifact" | "--dry-run"
        )
    }) {
        bail!(LOGGING_DIAGNOSTIC)
    } else {
        None
    };

    let planning_act = if logging_raw.is_some()
        && rest
            .get(position)
            .is_some_and(|value| value == "--planning-act")
    {
        Some(ActReversibility::parse(required_option(
            rest,
            &mut position,
            "--planning-act",
        )?)?)
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
    let caller_arguments = ArgumentVector::new(rest[position..].to_vec());

    let logging = logging_raw
        .map(
            |(path, node, role, dispatch_ref, evidence, required_artifact)| -> Result<_> {
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    bail!(
                        "logged dispatch event-log path must be absolute: {}",
                        path.display()
                    );
                }
                let required_artifact_path =
                    AbsoluteRequiredArtifactPath::parse(PathBuf::from(required_artifact))?;
                if let Some((_, output)) = structured {
                    let output = PathBuf::from(output);
                    if output.is_absolute()
                        && required_artifact_path.as_path().as_os_str() != output.as_os_str()
                    {
                        bail!(
                            "dispatch required artifact path `{}` does not match output path `{}`",
                            required_artifact_path.as_path().display(),
                            output.display()
                        );
                    }
                }
                Ok((
                    path,
                    DispatchLogging {
                        node: NodeId::parse(node)
                            .context("failed to parse dispatch logging node")?,
                        role: DispatchRole::new(role),
                        dispatch_ref: DispatchRef::new(dispatch_ref),
                        evidence: Evidence::parse(evidence)
                            .context("failed to parse dispatch logging evidence")?,
                        required_artifact_path,
                    },
                ))
            },
        )
        .transpose()?;
    let caller_arguments = match (planning_act, logging.as_ref()) {
        (Some(act), Some((_, metadata))) => {
            compose_planning_role_frame(&metadata.role, act, caller_arguments)?
        }
        (None, _) => caller_arguments,
        (Some(_), None) => return Err(pce_core::PlanningFrameError::MissingLoggingMetadata.into()),
    };

    let working_directory = AbsoluteWorkingDirectory::parse(PathBuf::from(raw_cwd))
        .context("failed to parse dispatch working directory")?;
    let stdin = match plan_path {
        Some(path) => StdinBinding::PlanBytes(
            std::fs::read(path).with_context(|| format!("failed to read plan file `{path}`"))?,
        ),
        None => StdinBinding::Null,
    };
    let mut envelope = DispatchEnvelope::new(DispatchTarget::Codex, working_directory, stdin)
        .with_arguments(caller_arguments)
        .with_environment(ChildEnvironment::new(environment))
        .with_sandbox(Sandbox::WorkspaceWrite);
    if let Some((schema, output)) = structured {
        let output_path = AbsoluteOutputPath::parse(PathBuf::from(output))
            .context("failed to parse dispatch output path")?;
        if logging
            .as_ref()
            .is_some_and(|(_, metadata)| metadata.role.as_str() == "step-executor")
            && output_is_inside_measured_worktree(
                envelope.working_directory().as_path(),
                output_path.as_path(),
            )?
        {
            bail!(
                "step-executor output path must be outside its measured worktree: {}",
                output_path.as_path().display()
            );
        }
        envelope = envelope
            .with_schema_path(
                AbsoluteSchemaPath::parse(PathBuf::from(schema))
                    .context("failed to parse dispatch schema path")?,
            )
            .with_output_path(output_path);
    }
    let logging = logging.map(|(path, metadata)| {
        if dry_run {
            DispatchLoggingMode::DryRun { path, metadata }
        } else {
            DispatchLoggingMode::Live { path, metadata }
        }
    });
    Ok(Command::Dispatch { envelope, logging })
}

fn parse_gate_dispatch(rest: &[String]) -> Result<Command> {
    let mut position = 0;
    let raw_cwd = required_option(rest, &mut position, "--cwd")?;
    let mut environment = BTreeMap::new();
    while rest.get(position).is_some_and(|value| value == "--env") {
        let raw_entry = required_option(rest, &mut position, "--env")?;
        let (name, value) = raw_entry
            .split_once('=')
            .ok_or_else(|| anyhow!("environment entry must contain `=`: `{raw_entry}`"))?;
        if name.is_empty() {
            bail!("environment name must not be empty");
        }
        if name == "ANTHROPIC_API_KEY" {
            bail!("gate dispatch environment must not contain `ANTHROPIC_API_KEY`");
        }
        if environment
            .insert(name.to_owned(), value.to_owned())
            .is_some()
        {
            bail!("duplicate environment name `{name}`");
        }
    }
    let schema = required_option(rest, &mut position, "--output-schema")?;
    let output = required_option(rest, &mut position, "-o")?;
    let plan_path = if rest
        .get(position)
        .is_some_and(|value| value == "--plan-file")
    {
        Some(required_option(rest, &mut position, "--plan-file")?)
    } else {
        None
    };
    const LOGGING_DIAGNOSTIC: &str = "dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence, --required-artifact";
    let logging_raw = if rest
        .get(position)
        .is_some_and(|value| value == "--log-file")
    {
        let parsed = (|| -> Result<(&str, &str, &str, &str, &str, &str)> {
            Ok((
                required_option(rest, &mut position, "--log-file")?,
                required_option(rest, &mut position, "--node")?,
                required_option(rest, &mut position, "--role")?,
                required_option(rest, &mut position, "--ref")?,
                required_option(rest, &mut position, "--evidence")?,
                required_option(rest, &mut position, "--required-artifact")?,
            ))
        })();
        match parsed {
            Ok(values) => Some(values),
            Err(_) => bail!(LOGGING_DIAGNOSTIC),
        }
    } else if rest
        .get(position)
        .is_some_and(|value| value == "--planning-act")
    {
        return Err(pce_core::PlanningFrameError::MissingLoggingMetadata.into());
    } else if rest.get(position).is_some_and(|value| {
        matches!(
            value.as_str(),
            "--node" | "--role" | "--ref" | "--evidence" | "--required-artifact" | "--dry-run"
        )
    }) {
        bail!(LOGGING_DIAGNOSTIC)
    } else {
        None
    };
    let planning_act = if logging_raw.is_some()
        && rest
            .get(position)
            .is_some_and(|value| value == "--planning-act")
    {
        Some(ActReversibility::parse(required_option(
            rest,
            &mut position,
            "--planning-act",
        )?)?)
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
    let caller_arguments = rest[position..].to_vec();
    if caller_arguments
        .iter()
        .any(|argument| argument == "--output-format" || argument.starts_with("--output-format="))
    {
        bail!("gate caller arguments must not contain `--output-format`");
    }
    let logging = logging_raw
        .map(
            |(path, node, role, dispatch_ref, evidence, required_artifact)| -> Result<_> {
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    bail!(
                        "logged dispatch event-log path must be absolute: {}",
                        path.display()
                    );
                }
                let required_artifact_path =
                    AbsoluteRequiredArtifactPath::parse(PathBuf::from(required_artifact))?;
                let output = PathBuf::from(output);
                if output.is_absolute()
                    && required_artifact_path.as_path().as_os_str() != output.as_os_str()
                {
                    bail!(
                        "dispatch required artifact path `{}` does not match output path `{}`",
                        required_artifact_path.as_path().display(),
                        output.display()
                    );
                }
                Ok((
                    path,
                    DispatchLogging {
                        node: NodeId::parse(node)
                            .context("failed to parse dispatch logging node")?,
                        role: DispatchRole::new(role),
                        dispatch_ref: DispatchRef::new(dispatch_ref),
                        evidence: Evidence::parse(evidence)
                            .context("failed to parse dispatch logging evidence")?,
                        required_artifact_path,
                    },
                ))
            },
        )
        .transpose()?;
    let arguments = ArgumentVector::new(caller_arguments);
    let arguments = match (planning_act, logging.as_ref()) {
        (Some(act), Some((_, metadata))) => {
            compose_planning_role_frame(&metadata.role, act, arguments)?
        }
        (None, _) => arguments,
        (Some(_), None) => return Err(pce_core::PlanningFrameError::MissingLoggingMetadata.into()),
    };
    let working_directory = AbsoluteWorkingDirectory::parse(PathBuf::from(raw_cwd))
        .context("failed to parse dispatch working directory")?;
    let schema_path = AbsoluteSchemaPath::parse(PathBuf::from(schema))
        .context("failed to parse dispatch schema path")?;
    let output_path = AbsoluteOutputPath::parse(PathBuf::from(output))
        .context("failed to parse dispatch output path")?;
    let stdin = match plan_path {
        Some(path) => StdinBinding::PlanBytes(
            std::fs::read(path).with_context(|| format!("failed to read plan file `{path}`"))?,
        ),
        None => StdinBinding::Null,
    };
    let role = logging.as_ref().map(|(_, metadata)| metadata.role.clone());
    let recorder = if role
        .as_ref()
        .is_some_and(|role| role.as_str() == "falsification-critic")
    {
        if environment.contains_key("PCE_GATE_EXEC_CLIENT") {
            bail!(
                "falsification-critic environment must not supply binary-owned `PCE_GATE_EXEC_CLIENT`"
            );
        }
        if environment.contains_key("PCE_GATE_EXEC_SOCKET") {
            bail!(
                "falsification-critic environment must not supply binary-owned `PCE_GATE_EXEC_SOCKET`"
            );
        }
        let client_path = match std::env::var_os(PAIRED_GATE_EXEC_CLIENT_ENV) {
            Some(path) => {
                let path = PathBuf::from(path)
                    .canonicalize()
                    .context("failed to resolve isolated recorder client")?;
                if !path.starts_with(working_directory.as_path()) || !path.is_file() {
                    bail!(
                        "isolated recorder client must be an ordinary file in the subject checkout"
                    );
                }
                path
            }
            None => std::env::current_exe().context("failed to resolve current pce executable")?,
        };
        let client = AbsoluteGateExecClientPath::parse(client_path)?;
        let evidence = AbsoluteGateExecutionEvidencePath::from_verdict_path(output_path.as_path());
        let issuance = u64::try_from(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .context("system clock before epoch")?
                .as_nanos()
                % 1_000_000_000_000,
        )
        .context("gate execution issuance exceeds u64")?;
        let socket = AbsoluteGateExecutionSocketPath::construct(
            &std::env::temp_dir(),
            std::process::id(),
            issuance,
        )?;
        Some(GateExecutionRecorderConfig::new(client, evidence, socket))
    } else {
        None
    };
    let arguments = compose_gate_arguments(
        role.as_ref(),
        &output_path,
        recorder.as_ref().map(GateExecutionRecorderConfig::client),
        arguments,
    )
    .context("failed to compose gate caller arguments")?;
    let envelope = DispatchEnvelope::new(DispatchTarget::Gate, working_directory, stdin)
        .with_arguments(arguments)
        .with_environment(ChildEnvironment::new(environment))
        .with_schema_path(schema_path)
        .with_output_path(output_path);
    let envelope = match recorder {
        Some(recorder) => envelope.with_gate_execution_recorder(recorder)?,
        None => envelope,
    };
    let logging = logging.map(|(path, metadata)| {
        if dry_run {
            DispatchLoggingMode::DryRun { path, metadata }
        } else {
            DispatchLoggingMode::Live { path, metadata }
        }
    });
    Ok(Command::Dispatch { envelope, logging })
}

fn output_is_inside_measured_worktree(cwd: &Path, output: &Path) -> Result<bool> {
    let Some(root) = cwd
        .ancestors()
        .find(|candidate| candidate.join(".git").exists())
    else {
        return Ok(false);
    };
    let root = root
        .to_str()
        .ok_or_else(|| anyhow!("measured Git worktree root is not UTF-8"))?;
    let output = output
        .to_str()
        .ok_or_else(|| anyhow!("dispatch output path is not UTF-8"))?;
    let normalized_root = lexically_normalized_repository_root(root);
    let normalized_output = lexically_normalized_repository_root(output);
    Ok(normalized_output.starts_with(normalized_root))
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
        ("meter", []) => Ok(Command::LogMeter),
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
    let mut graph_path = None;
    let mut risk_ordering = RiskOrdering::Honour;
    let mut index = 0;
    while index < trailing.len() {
        match trailing[index].as_str() {
            "--graph"
                if graph_path.is_none()
                    && index + 1 < trailing.len()
                    && is_value(&trailing[index + 1]) =>
            {
                graph_path = Some(ArtifactPath::new(&trailing[index + 1]));
                index += 2;
            }
            "--override-risk-ordering" if risk_ordering == RiskOrdering::Honour => {
                risk_ordering = RiskOrdering::Override;
                index += 1;
            }
            _ => bail!(USAGE),
        }
    }
    Ok(Command::Ready {
        log_path: PathBuf::from(raw_path),
        recovery_log_path: RecoveryLogPath::new(raw_path),
        vision_dir: PathBuf::from(raw_vision_dir),
        graph_path,
        risk_ordering,
    })
}

fn parse_graph_repositories(args: &[String]) -> Result<Vec<(String, PathBuf)>> {
    let mut repositories = Vec::new();
    let mut index = 0;
    while index < args.len() {
        if args[index] != "--repository" || index + 1 >= args.len() {
            bail!(USAGE);
        }
        let Some((name, path)) = args[index + 1].split_once('=') else {
            bail!("repository mapping must be NAME=SOURCE_WORKTREE");
        };
        if name.trim().is_empty() || path.trim().is_empty() {
            bail!("repository mapping must contain a non-empty name and path");
        }
        if repositories.iter().any(|(existing, _)| existing == name) {
            bail!("repository mapping `{name}` was supplied more than once");
        }
        repositories.push((name.to_owned(), PathBuf::from(path)));
        index += 2;
    }
    Ok(repositories)
}

fn parse_graph_command(action: &str, args: &[String]) -> Result<Command> {
    let [flag, raw_value, trailing @ ..] = args else {
        bail!(USAGE);
    };
    match action {
        "check" if flag == "--file" && is_value(raw_value) => {
            let strict = trailing.iter().any(|argument| argument == "--strict");
            let repository_arguments = trailing
                .iter()
                .filter(|argument| argument.as_str() != "--strict")
                .cloned()
                .collect::<Vec<_>>();
            Ok(Command::GraphCheck {
                path: PathBuf::from(raw_value),
                repositories: parse_graph_repositories(&repository_arguments)?,
                strict,
            })
        }
        "freeze" if flag == "--vision-dir" && is_value(raw_value) => {
            let mut repositories = Vec::new();
            let mut authority = FreezeAuthority::Human;
            let mut criterion_revisions = None;
            let mut base_currency_acceptance = None;
            let mut index = 0;
            while index < trailing.len() {
                match trailing[index].as_str() {
                    "--mechanical" if authority == FreezeAuthority::Human => {
                        authority = FreezeAuthority::Mechanical;
                        index += 1;
                    }
                    "--repository"
                        if index + 1 < trailing.len() && is_value(&trailing[index + 1]) =>
                    {
                        let Some((name, path)) = trailing[index + 1].split_once('=') else {
                            bail!("repository mapping must be NAME=SOURCE_WORKTREE");
                        };
                        if name.trim().is_empty() || path.trim().is_empty() {
                            bail!("repository mapping must contain a non-empty name and path");
                        }
                        if repositories.iter().any(|(existing, _)| existing == name) {
                            bail!("repository mapping `{name}` was supplied more than once");
                        }
                        repositories.push((name.to_owned(), PathBuf::from(path)));
                        index += 2;
                    }
                    "--criterion-revisions"
                        if criterion_revisions.is_none()
                            && index + 1 < trailing.len()
                            && is_value(&trailing[index + 1]) =>
                    {
                        criterion_revisions = Some(PathBuf::from(&trailing[index + 1]));
                        index += 2;
                    }
                    "--accept-base-currency-risk"
                        if base_currency_acceptance.is_none()
                            && index + 1 < trailing.len()
                            && is_value(&trailing[index + 1]) =>
                    {
                        base_currency_acceptance = Some(PathBuf::from(&trailing[index + 1]));
                        index += 2;
                    }
                    _ => bail!(USAGE),
                }
            }
            Ok(Command::GraphFreeze {
                vision_dir: PathBuf::from(raw_value),
                repositories,
                authority,
                criterion_revisions,
                base_currency_acceptance,
            })
        }
        _ => bail!(USAGE),
    }
}

fn parse_completion_command(action: &str, rest: &[String]) -> Result<Command> {
    let [
        file_flag,
        raw_path,
        vision_flag,
        raw_vision_dir,
        finished_flag,
        raw_finished_result,
    ] = rest
    else {
        bail!(USAGE);
    };
    if action != "check"
        || file_flag != "--file"
        || vision_flag != "--vision-dir"
        || finished_flag != "--finished-result"
        || !is_value(raw_path)
        || !is_value(raw_vision_dir)
        || !is_value(raw_finished_result)
    {
        bail!(USAGE);
    }
    let finished_result =
        FinishedResult::parse(raw_finished_result).context("failed to parse finished result")?;
    Ok(Command::CompletionCheck {
        log_path: PathBuf::from(raw_path),
        recovery_log_path: RecoveryLogPath::new(raw_path),
        vision_dir: PathBuf::from(raw_vision_dir),
        finished_result,
    })
}

fn parse_criteria_command(action: &str, rest: &[String]) -> Result<Command> {
    let [file_flag, raw_path, vision_flag, raw_vision_dir] = rest else {
        bail!(USAGE);
    };
    if action != "check"
        || file_flag != "--file"
        || vision_flag != "--vision-dir"
        || !is_value(raw_path)
        || !is_value(raw_vision_dir)
    {
        bail!(USAGE);
    }
    Ok(Command::CriteriaCheck {
        log_path: PathBuf::from(raw_path),
        recovery_log_path: RecoveryLogPath::new(raw_path),
        vision_dir: PathBuf::from(raw_vision_dir),
    })
}

fn parse_landing_command(action: &str, rest: &[String]) -> Result<Command> {
    let [
        file_flag,
        raw_path,
        vision_flag,
        raw_vision_dir,
        finished_flag,
        raw_finished_result,
    ] = rest
    else {
        bail!(USAGE);
    };
    if action != "check"
        || file_flag != "--file"
        || vision_flag != "--vision-dir"
        || finished_flag != "--finished-result"
        || !is_value(raw_path)
        || !is_value(raw_vision_dir)
        || !is_value(raw_finished_result)
    {
        bail!(USAGE);
    }
    let finished_result =
        FinishedResult::parse(raw_finished_result).context("failed to parse finished result")?;
    Ok(Command::LandingCheck {
        log_path: PathBuf::from(raw_path),
        recovery_log_path: RecoveryLogPath::new(raw_path),
        vision_dir: PathBuf::from(raw_vision_dir),
        finished_result,
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

fn run_vision_check(input: &mut dyn Read) -> Result<()> {
    (|| -> Result<()> {
        let mut document = String::new();
        input.read_to_string(&mut document)?;
        parse_acceptance_criteria(&document)?;
        Ok(())
    })()
    .context("failed to check vision acceptance criteria")
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum GraphAuthoringWarning {
    ArtifactProvenance {
        package: String,
        criterion: String,
        repository: String,
        path: String,
        strict_upstream_references: Vec<String>,
        message: String,
    },
    ActOwnershipArtifact {
        package: String,
        predecessor_package: String,
        repository: String,
        path: String,
        message: String,
    },
    ActOwnershipTitle {
        package: String,
        predecessor_package: String,
        title: String,
        message: String,
    },
    PredecessorUnavailable {
        path: String,
        message: String,
    },
}

fn package_artifact_references(package: &pce_core::WorkPackage) -> Vec<(String, String, String)> {
    let mut references = Vec::new();
    for criterion in package.criteria() {
        for artifact in extract_conservative_artifact_references(criterion.command()) {
            let repository = match artifact.repository_index() {
                Some(index) => package.repositories().get(index.position()),
                None if package.repositories().len() == 1 => package.repositories().first(),
                None => None,
            };
            if let Some(repository) = repository {
                references.push((
                    criterion.name().to_owned(),
                    repository.clone(),
                    artifact.path().to_owned(),
                ));
            }
        }
    }
    references.sort();
    references.dedup();
    references
}

fn strict_upstream_ids(graph: &WorkPackageGraph, package_id: &str) -> BTreeSet<String> {
    let by_id = graph
        .packages()
        .iter()
        .map(|package| (package.id().as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![package_id];
    let mut upstream = BTreeSet::new();
    while let Some(id) = pending.pop() {
        let Some(package) = by_id.get(id) else {
            continue;
        };
        for dependency in package.depends_on() {
            if upstream.insert(dependency.id().as_str().to_owned()) {
                pending.push(dependency.id().as_str());
            }
        }
    }
    upstream
}

fn artifact_exists_at_authored_ref(
    graph: &WorkPackageGraph,
    repositories: &[(String, PathBuf)],
    repository_name: &str,
    path: &str,
) -> Result<Option<bool>> {
    let Some((_, repository)) = repositories
        .iter()
        .find(|(name, _)| name == repository_name)
    else {
        return Ok(None);
    };
    let authored_ref = graph_authored_ref(graph, repository_name)?;
    let oid = git_oid(repository, authored_ref)?;
    let object = format!("{oid}:{path}");
    let output = git_output(repository, &["cat-file", "-e", &object])?;
    Ok(Some(output.status.success()))
}

fn graph_authoring_warnings(
    path: &Path,
    graph: &WorkPackageGraph,
    repositories: &[(String, PathBuf)],
) -> Result<Vec<GraphAuthoringWarning>> {
    let references = graph
        .packages()
        .iter()
        .map(|package| (package.id().as_str(), package_artifact_references(package)))
        .collect::<BTreeMap<_, _>>();
    let mut warnings = Vec::new();
    for package in graph.packages() {
        let upstream = strict_upstream_ids(graph, package.id().as_str());
        for (criterion, repository, artifact_path) in package_artifact_references(package) {
            if artifact_exists_at_authored_ref(graph, repositories, &repository, &artifact_path)?
                != Some(false)
            {
                continue;
            }
            let strict_upstream_references = upstream
                .iter()
                .filter(|upstream_id| {
                    references.get(upstream_id.as_str()).is_some_and(|items| {
                        items
                            .iter()
                            .any(|(_, candidate_repository, candidate_path)| {
                                candidate_repository == &repository
                                    && candidate_path == &artifact_path
                            })
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            if !strict_upstream_references.is_empty() {
                continue;
            }
            warnings.push(GraphAuthoringWarning::ArtifactProvenance {
                package: package.id().as_str().to_owned(),
                criterion,
                repository,
                path: artifact_path,
                strict_upstream_references,
                message: "artifact does not exist at the authored ref; a producer cannot be proven because the graph has no output declarations".to_owned(),
            });
        }
    }
    if graph.plan_version() > 1 {
        let predecessor_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("graph.v{}.json", graph.plan_version() - 1));
        match fs::read(&predecessor_path) {
            Ok(bytes) => {
                let predecessor = parse_work_package_graph(&bytes).with_context(|| format!("failed to validate predecessor graph {}", predecessor_path.display()))?;
                let predecessor_ids = predecessor.packages().iter().map(|package| package.id().as_str()).collect::<BTreeSet<_>>();
                let predecessor_references = predecessor.packages().iter().map(|package| (package, package_artifact_references(package))).collect::<Vec<_>>();
                for package in graph.packages().iter().filter(|package| !predecessor_ids.contains(package.id().as_str())) {
                    let current_references = package_artifact_references(package);
                    for (predecessor_package, frozen_references) in &predecessor_references {
                        for (_, repository, artifact_path) in &current_references {
                            if frozen_references.iter().any(|(_, frozen_repository, frozen_path)| frozen_repository == repository && frozen_path == artifact_path) {
                                warnings.push(GraphAuthoringWarning::ActOwnershipArtifact {
                                    package: package.id().as_str().to_owned(), predecessor_package: predecessor_package.id().as_str().to_owned(), repository: repository.clone(), path: artifact_path.clone(),
                                    message: "new package references the exact canonical artifact already referenced by a predecessor package".to_owned(),
                                });
                            }
                        }
                        if titles_conservatively_overlap(package.title(), predecessor_package.title()) {
                            warnings.push(GraphAuthoringWarning::ActOwnershipTitle {
                                package: package.id().as_str().to_owned(), predecessor_package: predecessor_package.id().as_str().to_owned(), title: package.title().to_owned(),
                                message: "new package title has conservative normalized overlap with a predecessor package title".to_owned(),
                            });
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => warnings.push(GraphAuthoringWarning::PredecessorUnavailable {
                path: predecessor_path.display().to_string(), message: "act ownership cannot be checked because the immediate predecessor graph is missing".to_owned(),
            }),
            Err(error) => return Err(error).with_context(|| format!("failed to read predecessor graph {}", predecessor_path.display())),
        }
    }
    Ok(warnings)
}

fn run_graph_check(path: &Path, repositories: &[(String, PathBuf)], strict: bool) -> Result<()> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read work-package graph {}", path.display()))?;
    let graph = parse_work_package_graph(&bytes)
        .with_context(|| format!("failed to validate work-package graph {}", path.display()))?;
    verify_graph_repository_refs(&graph, repositories)?;
    let warnings = graph_authoring_warnings(path, &graph, repositories)?;
    if strict && !warnings.is_empty() {
        bail!(
            "graph check strict mode refused {} authoring warning(s): {}",
            warnings.len(),
            serde_json::to_string(&warnings).context("failed to serialize authoring warnings")?
        );
    }
    write_json_stdout(&json!({
        "valid": true,
        "refs_verified": !repositories.is_empty(),
        "vision": graph.vision(),
        "plan_version": graph.plan_version(),
        "packages": graph.packages().len(),
        "warnings": warnings,
    }))
}

fn publish_frozen_graph(path: &Path, bytes: &[u8]) -> Result<bool> {
    match fs::read(path) {
        Ok(existing) => {
            if existing == bytes {
                return Ok(false);
            }
            bail!(
                "frozen plan artifact {} already contains different bytes",
                path.display()
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to read frozen graph {}", path.display()));
        }
    }
    let parent = path
        .parent()
        .context("frozen graph must have a parent directory")?;
    let nonce = GRAPH_FREEZE_NONCE.fetch_add(1, AtomicOrdering::Relaxed);
    let temporary = parent.join(format!(
        ".graph.freeze.{}.{}.tmp",
        std::process::id(),
        nonce
    ));
    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| {
                format!(
                    "failed to create temporary frozen graph {}",
                    temporary.display()
                )
            })?;
        file.write_all(bytes).with_context(|| {
            format!(
                "failed to write temporary frozen graph {}",
                temporary.display()
            )
        })?;
        file.sync_all().with_context(|| {
            format!(
                "failed to synchronize temporary frozen graph {}",
                temporary.display()
            )
        })?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    let linked = match fs::hard_link(&temporary, path) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let existing = fs::read(path).with_context(|| {
                format!(
                    "failed to read concurrently frozen graph {}",
                    path.display()
                )
            })?;
            if existing != bytes {
                let _ = fs::remove_file(&temporary);
                bail!(
                    "frozen plan artifact {} already contains different bytes",
                    path.display()
                );
            }
            false
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error)
                .with_context(|| format!("failed to publish frozen graph {}", path.display()));
        }
    };
    fs::remove_file(&temporary).with_context(|| {
        format!(
            "failed to remove temporary frozen graph {}",
            temporary.display()
        )
    })?;
    if linked {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| {
                format!(
                    "failed to synchronize vision directory {}",
                    parent.display()
                )
            })?;
    }
    Ok(linked)
}

fn highest_frozen_graph_version(vision_dir: &Path) -> Result<Option<u64>> {
    let mut highest = None;
    for entry in fs::read_dir(vision_dir)
        .with_context(|| format!("failed to list vision directory {}", vision_dir.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "failed to read an entry in vision directory {}",
                vision_dir.display()
            )
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(raw_version) = name
            .strip_prefix("graph.v")
            .and_then(|value| value.strip_suffix(".json"))
        else {
            continue;
        };
        let Ok(version) = raw_version.parse::<u64>() else {
            continue;
        };
        highest = Some(highest.map_or(version, |current: u64| current.max(version)));
    }
    Ok(highest)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BaseCurrencyAcceptance {
    schema_version: u64,
    entries: Vec<BaseCurrencyRiskEntry>,
    accepted_by: String,
    reason: String,
}

fn parse_base_currency_acceptance(bytes: &[u8]) -> Result<BaseCurrencyAcceptance> {
    let acceptance: BaseCurrencyAcceptance =
        serde_json::from_slice(bytes).context("base-currency acceptance must be valid JSON")?;
    if acceptance.schema_version != 1 {
        bail!("base-currency acceptance schema_version must be 1");
    }
    if acceptance.entries.is_empty()
        || acceptance
            .entries
            .iter()
            .any(|entry| entry.repository.trim().is_empty())
    {
        bail!("base-currency acceptance requires non-empty repository entries");
    }
    let unique = acceptance
        .entries
        .iter()
        .map(|entry| entry.repository.as_str())
        .collect::<BTreeSet<_>>();
    if unique.len() != acceptance.entries.len() {
        bail!("base-currency acceptance repository entries must be unique");
    }
    if acceptance.accepted_by.trim().is_empty() || acceptance.reason.trim().is_empty() {
        bail!("base-currency acceptance requires non-empty accepted_by and reason");
    }
    Ok(acceptance)
}

fn git_output(repository: &Path, arguments: &[&str]) -> Result<Output> {
    std::process::Command::new("git")
        .args(["-C"])
        .arg(repository)
        .args(arguments)
        .output()
        .with_context(|| format!("failed to execute git in {}", repository.display()))
}

#[derive(Debug)]
struct BaseCurrencyFailure {
    repository: String,
    mode: BaseCurrencyRiskMode,
    diagnostic: String,
}

fn remote_base_currency_failures(
    graph: &WorkPackageGraph,
    repositories: &[(String, PathBuf)],
) -> Result<Vec<BaseCurrencyFailure>> {
    let mut failures = Vec::new();
    for (name, repository) in repositories {
        let authored_ref = graph_authored_ref(graph, name)?;
        let authored_oid = git_oid(repository, authored_ref)?;
        let fetched = git_output(repository, &["fetch", "--no-tags", "origin", "HEAD"])?;
        if !fetched.status.success() {
            failures.push(BaseCurrencyFailure {
                repository: name.clone(),
                mode: BaseCurrencyRiskMode::Offline,
                diagnostic: format!(
                    "repository `{name}` remote base currency cannot be established: origin HEAD is missing or unreachable: {}; configure/reach origin or use an explicit attributed --accept-base-currency-risk record for an offline or historical freeze",
                    String::from_utf8_lossy(&fetched.stderr).trim()
                ),
            });
            continue;
        }
        let remote_oid = git_oid(repository, "FETCH_HEAD")?;
        let ancestor = git_output(
            repository,
            &["merge-base", "--is-ancestor", &remote_oid, &authored_oid],
        )?;
        if ancestor.status.success() {
            continue;
        }
        let divergence = git_output(
            repository,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("{authored_oid}...{remote_oid}"),
            ],
        )?;
        if !divergence.status.success() {
            bail!("repository `{name}` failed to measure authored/remote divergence");
        }
        let counts = String::from_utf8(divergence.stdout)
            .context("git returned non-UTF-8 divergence counts")?;
        let mut fields = counts.split_whitespace();
        let authored_only = fields
            .next()
            .context("git omitted authored-only divergence count")?;
        let remote_only = fields
            .next()
            .context("git omitted remote-only divergence count")?;
        failures.push(BaseCurrencyFailure {
            repository: name.clone(),
            mode: BaseCurrencyRiskMode::Historical,
            diagnostic: format!(
                "repository `{name}` authored oid {authored_oid} is not current with remote default oid {remote_oid} (authored-only {authored_only}, remote-only {remote_only}); update the source worktree with `git pull --ff-only` and re-author the graph"
            ),
        });
    }
    Ok(failures)
}

fn verify_remote_base_currency(
    graph: &WorkPackageGraph,
    repositories: &[(String, PathBuf)],
    acceptance: Option<&BaseCurrencyAcceptance>,
) -> Result<()> {
    let failures = remote_base_currency_failures(graph, repositories)?;
    let Some(acceptance) = acceptance else {
        if let Some(failure) = failures.first() {
            bail!(failure.diagnostic.clone());
        }
        return Ok(());
    };
    let accepted = acceptance
        .entries
        .iter()
        .map(|entry| entry.repository.as_str())
        .collect::<BTreeSet<_>>();
    let failed = failures
        .iter()
        .map(|failure| failure.repository.as_str())
        .collect::<BTreeSet<_>>();
    if accepted != failed {
        bail!(
            "base-currency acceptance repository scope {:?} does not exactly match repositories with accepted failures {:?}",
            acceptance
                .entries
                .iter()
                .map(|entry| entry.repository.as_str())
                .collect::<Vec<_>>(),
            failures
                .iter()
                .map(|failure| failure.repository.as_str())
                .collect::<Vec<_>>()
        );
    }
    if let Some((failure, entry)) = failures.iter().find_map(|failure| {
        acceptance
            .entries
            .iter()
            .find(|entry| entry.repository == failure.repository && entry.mode != failure.mode)
            .map(|entry| (failure, entry))
    }) {
        bail!(
            "repository `{}` has {:?} base-currency failure, not the accepted {:?} mode: {}",
            failure.repository,
            failure.mode,
            entry.mode,
            failure.diagnostic
        );
    }
    Ok(())
}

fn run_graph_freeze(
    vision_dir: &Path,
    repositories: &[(String, PathBuf)],
    authority: FreezeAuthority,
    criterion_revisions: Option<&Path>,
    base_currency_acceptance: Option<&Path>,
) -> Result<()> {
    if authority == FreezeAuthority::Mechanical && criterion_revisions.is_some() {
        bail!("mechanical freeze cannot carry a human criterion revision record");
    }
    if authority == FreezeAuthority::Mechanical && base_currency_acceptance.is_some() {
        bail!("mechanical freeze cannot accept base-currency risk");
    }
    if repositories.is_empty() {
        bail!(
            "graph freeze requires one --repository NAME=SOURCE_WORKTREE mapping per graph repository"
        );
    }
    let freeze_lock_path = vision_dir.join(".pce-graph-freeze.lock");
    let freeze_lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&freeze_lock_path)
        .with_context(|| {
            format!(
                "failed to open graph freeze lock {}",
                freeze_lock_path.display()
            )
        })?;
    freeze_lock.lock().with_context(|| {
        format!(
            "failed to lock graph freeze at {}",
            freeze_lock_path.display()
        )
    })?;
    let source = vision_dir.join("graph.json");
    let bytes = fs::read(&source)
        .with_context(|| format!("failed to read work-package graph {}", source.display()))?;
    let graph = parse_work_package_graph(&bytes)
        .with_context(|| format!("failed to validate work-package graph {}", source.display()))?;
    verify_graph_repository_refs(&graph, repositories)?;
    let acceptance = if authority == FreezeAuthority::Human {
        match base_currency_acceptance {
            Some(path) => {
                let bytes = fs::read(path).with_context(|| {
                    format!("failed to read base-currency acceptance {}", path.display())
                })?;
                let acceptance = parse_base_currency_acceptance(&bytes)?;
                Some((bytes, acceptance))
            }
            None => None,
        }
    } else {
        None
    };
    if authority == FreezeAuthority::Human {
        verify_remote_base_currency(
            &graph,
            repositories,
            acceptance.as_ref().map(|(_, acceptance)| acceptance),
        )?;
    }
    let acceptance_bytes = acceptance.map(|(bytes, _)| bytes);
    let version = graph.plan_version();
    let expected_vision = vision_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("vision directory must have a Unicode basename")?;
    if graph.vision() != expected_vision {
        bail!(
            "graph vision {} does not match vision directory {}",
            graph.vision(),
            expected_vision
        );
    }
    let frozen = vision_dir.join(format!("graph.v{version}.json"));
    let frozen_exists = frozen
        .try_exists()
        .with_context(|| format!("failed to inspect frozen graph {}", frozen.display()))?;
    if frozen_exists {
        let existing = fs::read(&frozen)
            .with_context(|| format!("failed to read frozen graph {}", frozen.display()))?;
        if existing != bytes {
            bail!(
                "frozen plan artifact {} already contains different bytes",
                frozen.display()
            );
        }
    }
    if authority == FreezeAuthority::Mechanical {
        let highest = highest_frozen_graph_version(vision_dir)?;
        let expected_highest = if frozen_exists {
            version
        } else {
            version
                .checked_sub(1)
                .context("mechanical freeze requires a frozen predecessor")?
        };
        if highest != Some(expected_highest) {
            bail!(
                "mechanical freeze requires the highest frozen version to be {expected_highest}, found {highest:?}"
            );
        }
    }
    let mut frozen_revision_digest = None;
    if version > 1 {
        let previous = vision_dir.join(format!("graph.v{}.json", version - 1));
        let previous_bytes = fs::read(&previous).with_context(|| {
            format!(
                "cannot freeze plan version {version} before readable version {} at {}",
                version - 1,
                previous.display()
            )
        })?;
        let previous_graph = parse_work_package_graph(&previous_bytes)
            .with_context(|| format!("frozen predecessor {} is invalid", previous.display()))?;
        if previous_graph.plan_version() != version - 1 || previous_graph.vision() != graph.vision()
        {
            bail!(
                "frozen predecessor {} does not record version {} of vision {}",
                previous.display(),
                version - 1,
                graph.vision()
            );
        }
        if authority == FreezeAuthority::Mechanical {
            verify_mechanical_freeze(&previous_graph, &graph)?;
        }
        let violations = criteria_invariance_violations(&previous_graph, &graph);
        if violations.is_empty() {
            if criterion_revisions.is_some() {
                bail!(
                    "criterion revision record supplied, but no predecessor criterion changed or was removed"
                );
            }
            let frozen_record =
                vision_dir.join(format!("graph.v{version}.criterion-revisions.json"));
            if frozen_record.try_exists().with_context(|| {
                format!(
                    "failed to inspect frozen criterion revision record {}",
                    frozen_record.display()
                )
            })? {
                bail!(
                    "frozen criterion revision record {} exists, but no predecessor criterion changed or was removed",
                    frozen_record.display()
                );
            }
        } else {
            let record_path = criterion_revisions.with_context(|| {
                let affected = violations
                    .iter()
                    .map(|violation| {
                        format!(
                            "{}::{:?}",
                            violation.previous_package().as_str(),
                            violation.criterion().name()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "criteria changed or were removed ({affected}); additive world-conformance requires no graph edit, otherwise the human must ratify the exact revision with --criterion-revisions <HUMAN_RECORD_PATH>"
                )
            })?;
            let revision_bytes = fs::read(record_path).with_context(|| {
                format!(
                    "failed to read human criterion revision record {}",
                    record_path.display()
                )
            })?;
            let manifest = parse_criterion_revision_manifest(&revision_bytes)
                .context("failed to parse human criterion revision record")?;
            validate_criterion_revisions(&previous_graph, &graph, manifest.revisions())
                .context("human criterion revision record does not match the graph transition")?;
            let frozen_record =
                vision_dir.join(format!("graph.v{version}.criterion-revisions.json"));
            publish_frozen_graph(&frozen_record, &revision_bytes).with_context(|| {
                format!("frozen criterion revision record for plan version {version} is immutable")
            })?;
            frozen_revision_digest = Some(format!("{:x}", Sha256::digest(&revision_bytes)));
        }
    } else if authority == FreezeAuthority::Mechanical {
        bail!("mechanical freeze requires a frozen predecessor");
    } else if criterion_revisions.is_some() {
        bail!("plan version 1 has no predecessor criteria to revise");
    }
    let frozen_base_currency_acceptance = if let Some(acceptance_bytes) = acceptance_bytes {
        let path = vision_dir.join(format!("graph.v{version}.base-currency-acceptance.json"));
        publish_frozen_graph(&path, &acceptance_bytes).with_context(|| {
            format!("frozen base-currency acceptance for plan version {version} is immutable")
        })?;
        Some((path, format!("{:x}", Sha256::digest(&acceptance_bytes))))
    } else {
        None
    };
    let wrote = publish_frozen_graph(&frozen, &bytes)
        .with_context(|| format!("frozen plan version {version} is immutable"))?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let mut output = json!({
        "path": frozen,
        "plan_version": version,
        "sha256": digest,
        "created": wrote
    });
    if authority == FreezeAuthority::Mechanical {
        output["mechanical"] = Value::from(true);
    }
    if let Some((acceptance_path, acceptance_digest)) = frozen_base_currency_acceptance {
        output["base_currency_acceptance"] = Value::from(acceptance_path.display().to_string());
        output["base_currency_acceptance_sha256"] = Value::from(acceptance_digest);
    }
    if let Some(revision_digest) = frozen_revision_digest {
        output["criterion_revisions"] = Value::from(
            vision_dir
                .join(format!("graph.v{version}.criterion-revisions.json"))
                .display()
                .to_string(),
        );
        output["criterion_revisions_sha256"] = Value::from(revision_digest);
    }
    write_json_stdout(&output)
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

fn dispatch_artifact_observations(
    log_path: &Path,
    records: &[EventRecord],
) -> Result<Vec<DispatchRequiredArtifactObservation>> {
    let directory = dispatch_identity_directory(log_path);
    let mut observations = Vec::new();
    for record in records {
        if !matches!(
            record.body_ref(),
            EventBodyRef::Known(KnownPayload::Dispatch(_))
        ) {
            continue;
        }
        let issuance_sequence = record.sequence();
        let sidecar = directory.join(format!("{}.json", issuance_sequence.get()));
        let Some(identity) = read_dispatch_identity_sidecar(&sidecar)? else {
            continue;
        };
        if identity.issuance_sequence() != issuance_sequence {
            bail!(
                "dispatch process identity sidecar issuance sequence {} does not match event issuance {}",
                identity.issuance_sequence().get(),
                issuance_sequence.get()
            );
        }
        observations.push(DispatchRequiredArtifactObservation::new(
            issuance_sequence,
            identity.required_artifact_path().clone(),
        ));
    }
    Ok(observations)
}

fn admit_and_append_dispatch(
    path: &Path,
    metadata: &DispatchLogging,
    payload: DispatchPayload,
) -> Result<EventRecord> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .with_context(|| format!("failed to open or create event log {}", path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", path.display()))?;
    let operation = (|| -> Result<EventRecord> {
        file.seek(SeekFrom::Start(0))
            .context("failed to seek locked event log")?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .context("failed to read locked event log")?;
        let mut reader = BufReader::new(bytes.as_slice());
        let records = read_event_log_lines(&mut reader, path)?
            .into_iter()
            .map(|line| line.record)
            .collect::<Vec<_>>();
        let observations = dispatch_artifact_observations(path, &records)?;
        let state = derive_dispatch_outcome_state(&records, &observations)?;
        let key = NonProductionKey {
            node: metadata.node.clone(),
            role: metadata.role.clone(),
            required_artifact_path: metadata.required_artifact_path.clone(),
        };
        match classify_dispatch_admission(&state, &key) {
            DispatchAdmission::Admit => append_locked_record(
                &mut file,
                serde_json::to_string(&payload).context("failed to serialize dispatch issuance")?,
                WriteKind::Dispatch,
                metadata.node.clone(),
                path,
            ),
            DispatchAdmission::OpenNonProductionHold { consecutive } => {
                append_locked_record(
                    &mut file,
                    serde_json::to_string(&NonProductionHoldOpenPayload { key: key.clone() })
                        .context("failed to serialize non-production hold")?,
                    WriteKind::NonProductionHoldOpen,
                    metadata.node.clone(),
                    path,
                )?;
                bail!(
                    "dispatch admission opened non-production hold for node {}, role {}, required artifact {} after {} consecutive non-production completions; resolve with retry, re-plan, or abandon",
                    key.node.as_str(),
                    key.role.as_str(),
                    key.required_artifact_path.as_str(),
                    consecutive.get()
                )
            }
            DispatchAdmission::NonProductionHoldOpen => bail!(
                "dispatch admission refused: non-production hold is open for node {}, role {}, required artifact {}; resolve with retry, re-plan, or abandon",
                key.node.as_str(),
                key.role.as_str(),
                key.required_artifact_path.as_str()
            ),
            DispatchAdmission::NonProductionResolutionClosesAdmission { resolution } => {
                let resolution = match resolution {
                    pce_core::NonProductionHoldResolution::Retry => "retry",
                    pce_core::NonProductionHoldResolution::RePlan => "re-plan",
                    pce_core::NonProductionHoldResolution::Abandon => "abandon",
                };
                bail!(
                    "dispatch admission refused: non-production hold for node {}, role {}, required artifact {} was resolved with {}",
                    key.node.as_str(),
                    key.role.as_str(),
                    key.required_artifact_path.as_str(),
                    resolution
                )
            }
            DispatchAdmission::DefectRoundCapExhausted { count } => bail!(
                "dispatch admission refused: defect-round cap {} is exhausted for node {} and role {}",
                count.get(),
                key.node.as_str(),
                key.role.as_str()
            ),
        }
    })();
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

#[derive(Debug, Clone)]
struct DispatchClosureTarget {
    issuance_sequence: Sequence,
    node: NodeId,
}

fn close_dispatch_conditionally(
    log_path: &Path,
    target: DispatchClosureTarget,
    prepare: impl FnOnce(&DispatchLedger) -> Result<DispatchCompletionPayload>,
) -> Result<EventRecord> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(false)
        .open(log_path)
        .with_context(|| format!("failed to open event log {}", log_path.display()))?;
    file.lock()
        .with_context(|| format!("failed to lock event log {}", log_path.display()))?;
    let operation = (|| -> Result<EventRecord> {
        file.seek(SeekFrom::Start(0))
            .context("failed to seek locked event log")?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .context("failed to read locked event log")?;
        let mut reader = BufReader::new(bytes.as_slice());
        let parsed = read_event_log_lines(&mut reader, log_path)
            .context("failed to read locked event log")?;
        let records = parsed
            .into_iter()
            .map(|line| line.record)
            .collect::<Vec<_>>();
        let ledger = fold_dispatch_ledger(&records).context("failed to read locked event log")?;
        let entry = ledger.issuance(target.issuance_sequence).ok_or_else(|| {
            if records
                .iter()
                .any(|record| record.sequence() == target.issuance_sequence)
            {
                anyhow!(
                    "dispatch closure sequence {} is not a dispatch issuance",
                    target.issuance_sequence.get()
                )
            } else {
                anyhow!(
                    "dispatch closure issuance {} is absent",
                    target.issuance_sequence.get()
                )
            }
        })?;
        if entry.issuance().node() != &target.node {
            bail!(
                "dispatch closure node {} does not match issuance {} node {}",
                target.node.as_str(),
                target.issuance_sequence.get(),
                entry.issuance().node().as_str()
            );
        }
        if let Some(completion) = entry.completion() {
            let (outcome, sequence) = match completion {
                DispatchLedgerCompletion::ObservedChild { sequence, .. } => {
                    ("observed-child", sequence)
                }
                DispatchLedgerCompletion::SpawnFailed { sequence, .. } => {
                    ("spawn-failed", sequence)
                }
                DispatchLedgerCompletion::ReconciledDead { sequence, .. } => {
                    ("reconciled-dead", sequence)
                }
            };
            bail!(
                "dispatch closure issuance {} already has {} completion {}",
                target.issuance_sequence.get(),
                outcome,
                sequence.get()
            );
        }
        let payload = prepare(&ledger)?;
        if payload.issuance_sequence() != target.issuance_sequence {
            bail!(
                "dispatch closure payload names issuance {}; expected {}",
                payload.issuance_sequence().get(),
                target.issuance_sequence.get()
            );
        }
        append_locked_record(
            &mut file,
            serde_json::to_string(&payload).context("failed to serialize dispatch completion")?,
            WriteKind::DispatchCompletion,
            target.node,
            log_path,
        )
    })();
    let unlock = file
        .unlock()
        .with_context(|| format!("failed to unlock event log {}", log_path.display()));
    match (operation, unlock) {
        (Ok(record), Ok(())) => Ok(record),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(primary), Err(error)) => Err(primary.context(format!(
            "additionally, explicit event-log unlock failed: {error:#}"
        ))),
    }
}

fn run_dispatch_reconcile(
    log_path: &Path,
    issuance_sequence: Sequence,
    node: NodeId,
) -> Result<()> {
    let target = DispatchClosureTarget {
        issuance_sequence,
        node,
    };
    close_dispatch_conditionally(log_path, target, |_| {
        let sidecar_path =
            dispatch_identity_directory(log_path).join(format!("{}.json", issuance_sequence.get()));
        let Some(identity) = read_dispatch_identity_sidecar(&sidecar_path)? else {
            bail!(
                "dispatch reconciliation refused for issuance {}: dispatch process identity sidecar is absent",
                issuance_sequence.get()
            );
        };
        if identity.issuance_sequence() != issuance_sequence {
            bail!(
                "dispatch process identity sidecar issuance sequence {} does not match event issuance {}",
                identity.issuance_sequence().get(),
                issuance_sequence.get()
            );
        }
        require_dead_identity(
            issuance_sequence,
            "child",
            identity.child_process_identity(),
        )?;
        if let Some(continuation) = identity.continuation_process_identity() {
            require_dead_identity(issuance_sequence, "continuation", continuation)?;
        }
        let artifact_production = artifact_production(observe_required_artifact_presence(
            identity.required_artifact_path().as_path(),
        )?);
        Ok(DispatchCompletionPayload::ReconciledDead(
            ReconciledDeadDispatchCompletionPayload {
                issuance_sequence,
                outcome: ReconciledDispatchOutcome::ReconciledDead,
                artifact_production,
            },
        ))
    })?;
    Ok(())
}

fn require_dead_identity(
    issuance_sequence: Sequence,
    label: &str,
    identity: RecordedProcessIdentity,
) -> Result<()> {
    match observe_darwin_process_number(identity.process_number())? {
        ProcessIdentityObservation::Absent => Ok(()),
        ProcessIdentityObservation::Present(observed)
            if observed != identity.process_start_identity() =>
        {
            Ok(())
        }
        ProcessIdentityObservation::Present(_) => bail!(
            "dispatch reconciliation refused for issuance {}: {} process identity is still alive",
            issuance_sequence.get(),
            label
        ),
        ProcessIdentityObservation::ForeignPresent => bail!(
            "dispatch reconciliation refused for issuance {}: {} process start identity is unreadable",
            issuance_sequence.get(),
            label
        ),
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
    let ratified_criteria = read_ratified_acceptance_criteria(vision_dir)?;
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
            contract.release_tag(),
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
    let dispatch_artifacts = dispatch_artifact_observations(log_path, &records)?;
    let state = derive_run_state_with_dispatch_artifacts(
        &records,
        &dispatch_artifacts,
        &ratified_criteria,
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

fn run_completion_check(
    log_path: &Path,
    recovery_log_path: &RecoveryLogPath,
    vision_dir: &Path,
    finished_result: &FinishedResult,
) -> Result<()> {
    let parsed_lines = read_event_log(log_path)?;
    let records = parsed_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let contracts = repository_contracts(&records)?;
    let primary_index = resolve_primary_repository(&contracts, log_path, vision_dir)?;
    let vision = vision_slug(vision_dir)?;
    let ratified_criteria = read_ratified_acceptance_criteria(vision_dir)?;
    let (artifacts, _) = current_artifacts(&records, Path::new(contracts[primary_index].root()))?;
    let state = derive_run_state(
        &records,
        &ratified_criteria,
        &vision,
        recovery_log_path,
        &artifacts,
        &[],
        &[],
    )
    .context("failed to derive completion state")?;
    let result = evaluate_completion(
        finished_result,
        state.blocking_criteria(),
        state.criterion_executions(),
    );
    let missing = result
        .criteria
        .iter()
        .filter(|report| matches!(report.status, CompletionCriterionStatus::Missing))
        .count();
    let failed = result
        .criteria
        .iter()
        .filter(|report| matches!(report.status, CompletionCriterionStatus::Failed { .. }))
        .count();
    let refused = result.decision == CompletionDecision::Refuse;

    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, &result).context("failed to serialize completion result")?;
    output
        .write_all(b"\n")
        .context("failed to write completion result newline")?;
    output
        .flush()
        .context("failed to flush completion result")?;

    if refused {
        bail!(
            "completion refused: {} blocking criteria ({} missing, {} failed)",
            missing + failed,
            missing,
            failed
        );
    }
    Ok(())
}

fn run_criteria_check(
    log_path: &Path,
    recovery_log_path: &RecoveryLogPath,
    vision_dir: &Path,
    input: &mut dyn Read,
) -> Result<()> {
    let mut document = String::new();
    input
        .read_to_string(&mut document)
        .context("failed to read proposed vision from stdin to EOF")?;
    let proposed_criteria = parse_acceptance_criteria(&document)
        .context("failed to parse proposed acceptance criteria")?;
    let parsed_lines = read_event_log(log_path)?;
    let records = parsed_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let contracts = repository_contracts(&records)?;
    let primary_index = resolve_primary_repository(&contracts, log_path, vision_dir)?;
    let vision = vision_slug(vision_dir)?;
    let ratified_criteria = read_ratified_acceptance_criteria(vision_dir)?;
    let (artifacts, _) = current_artifacts(&records, Path::new(contracts[primary_index].root()))?;
    let state = derive_run_state(
        &records,
        &ratified_criteria,
        &vision,
        recovery_log_path,
        &artifacts,
        &[],
        &[],
    )
    .context("failed to derive criterion-change state")?;
    let result = verify_criterion_change(state.blocking_criteria(), &proposed_criteria);
    let refused = result.decision == CriterionChangeDecision::Refuse;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, &result)
        .context("failed to serialize criterion-change verification")?;
    output
        .write_all(b"\n")
        .context("failed to write criterion-change verification newline")?;
    output
        .flush()
        .context("failed to flush criterion-change verification")?;
    if refused {
        bail!(
            "criterion change refused: proposed acceptance criteria must exactly match the ratified floor plus logged criterion-added events"
        );
    }
    Ok(())
}

fn run_landing_check(
    log_path: &Path,
    recovery_log_path: &RecoveryLogPath,
    vision_dir: &Path,
    finished_result: &FinishedResult,
) -> Result<()> {
    let parsed_lines = read_event_log(log_path)?;
    let records = parsed_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let contracts = repository_contracts(&records)?;
    let primary_index = resolve_primary_repository(&contracts, log_path, vision_dir)?;
    let vision = vision_slug(vision_dir)?;
    let ratified_criteria = read_ratified_acceptance_criteria(vision_dir)?;
    let (artifacts, _) = current_artifacts(&records, Path::new(contracts[primary_index].root()))?;
    let canonical_nodes = canonical_nodes(&records, &vision)?;
    let declared_chains = declared_exceptional_merge_chains(&records, &vision)?;

    let mut repositories = Vec::new();
    let mut authorities = Vec::new();
    let mut exceptional = Vec::new();
    if !canonical_nodes.is_empty() {
        let selected_index = canonical_nodes
            .iter()
            .enumerate()
            .max_by_key(|(_, item)| item.latest_sequence)
            .map(|(index, _)| index)
            .context("event log contains no canonical step node for repository projection")?;
        let selected = &canonical_nodes[selected_index];
        let selected_chain = declared_chains
            .iter()
            .find(|(node, _)| node == &selected.node)
            .map(|(_, chain)| chain);
        let selected_selector = selected_chain.map_or_else(
            || selected.subject.selector(),
            |chain| chain.step_pull_request().selector(),
        );
        let integration_branches = if declared_chains.is_empty() {
            integration_branches(&canonical_nodes, selected_index)
        } else {
            landing_integration_branches(&canonical_nodes, &declared_chains)
        };
        let mut runtimes = Vec::with_capacity(contracts.len());
        for contract in contracts {
            let (observation, runtime) = observe_repository(
                contract.name().clone(),
                PathBuf::from(contract.root()),
                contract.release_tag(),
                &integration_branches,
                selected_selector,
            )?;
            repositories.push(observation);
            runtimes.push(runtime);
        }
        let primary = runtimes
            .get(primary_index)
            .context("resolved primary repository index is unavailable")?;
        for node in &canonical_nodes {
            if let Some((_, chain)) = declared_chains
                .iter()
                .find(|(declared_node, _)| declared_node == &node.node)
            {
                let step_github =
                    observe_github(&primary.root, chain.step_pull_request().selector())?;
                let step_git =
                    observe_git(primary, chain.step_pull_request().selector(), &step_github)?;
                let promotion_github =
                    observe_github(&primary.root, chain.promotion_pull_request().selector())?;
                let promotion_git = observe_git(
                    primary,
                    chain.promotion_pull_request().selector(),
                    &promotion_github,
                )?;
                exceptional.push((
                    node.node.clone(),
                    ExceptionalMergeChainObservation::new(
                        PullRequestAuthorityObservation::new(step_github, step_git),
                        PullRequestAuthorityObservation::new(promotion_github, promotion_git),
                    ),
                ));
            } else {
                let selector = node.subject.selector();
                let github = observe_github(&primary.root, selector)?;
                let git = observe_git(primary, selector, &github)?;
                authorities.push(StepAuthorityObservation::new(
                    node.node.clone(),
                    github,
                    git,
                ));
            }
        }
    }

    let state = derive_run_state_with_exceptional_merge_chains(
        &records,
        &ratified_criteria,
        &vision,
        recovery_log_path,
        &artifacts,
        &repositories,
        &authorities,
        &exceptional,
    )
    .context("failed to derive landing state")?;
    let completion = evaluate_completion(
        finished_result,
        state.blocking_criteria(),
        state.criterion_executions(),
    );
    let result =
        evaluate_landing_readiness(&completion, state.steps(), state.criterion_executions());
    let refused = result.decision == LandingReadinessDecision::Refuse;
    let problem_count = result.problems.len();

    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, &result).context("failed to serialize landing result")?;
    output
        .write_all(b"\n")
        .context("failed to write landing result newline")?;
    output.flush().context("failed to flush landing result")?;

    if refused {
        bail!("landing refused: {problem_count} readiness problems");
    }
    Ok(())
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

#[derive(Debug)]
enum ReadyGraph {
    Legacy(DispatchGraph),
    WorkPackages(WorkPackageGraph),
}

fn frozen_graph_version(path: &ArtifactPath) -> Option<u64> {
    let name = Path::new(path.as_str()).file_name()?.to_str()?;
    let version = name.strip_prefix("graph.v")?.strip_suffix(".json")?;
    if version.is_empty()
        || version.starts_with('0')
        || !version.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    version.parse().ok()
}

fn validate_frozen_graph_identity(
    graph: &WorkPackageGraph,
    artifact_path: &ArtifactPath,
    vision_dir: &Path,
    primary_root: &Path,
) -> Result<()> {
    let path_version = frozen_graph_version(artifact_path).with_context(|| {
        format!(
            "work-package readiness requires a frozen graph.v<n>.json artifact, got {}",
            artifact_path.as_str()
        )
    })?;
    if path_version != graph.plan_version() {
        bail!(
            "frozen graph path {} names version {}, but its bytes record plan_version {}",
            artifact_path.as_str(),
            path_version,
            graph.plan_version()
        );
    }
    let recorded = Path::new(artifact_path.as_str());
    let resolved = if recorded.is_absolute() {
        recorded.to_path_buf()
    } else {
        primary_root.join(recorded)
    };
    let frozen_name = recorded
        .file_name()
        .context("frozen graph path must have a basename")?;
    let expected = vision_dir.join(frozen_name);
    if absolute_path(&resolved)? != absolute_path(&expected)? {
        bail!(
            "frozen work-package graph {} is not at vision root {}",
            resolved.display(),
            vision_dir.display()
        );
    }
    let expected_vision = vision_dir
        .file_name()
        .and_then(|name| name.to_str())
        .context("vision directory must have a Unicode basename")?;
    if graph.vision() != expected_vision {
        bail!(
            "frozen graph vision {} does not match requested vision {}",
            graph.vision(),
            expected_vision
        );
    }
    Ok(())
}

fn parse_ready_graph(bytes: &[u8]) -> Result<ReadyGraph> {
    match parse_dispatch_graph(bytes) {
        Ok(graph) => Ok(ReadyGraph::Legacy(graph)),
        Err(legacy_error) => parse_work_package_graph(bytes)
            .map(ReadyGraph::WorkPackages)
            .map_err(|package_error| anyhow!("neither a legacy milestone/step graph ({legacy_error:#}) nor a work-package graph ({package_error})")),
    }
}

fn merge_status_label(status: MergeStatus) -> &'static str {
    match status {
        MergeStatus::Merged => "merged",
        MergeStatus::NotMerged => "not-merged",
        MergeStatus::Inconclusive => "inconclusive",
    }
}

fn observe_work_package_merges(
    graph: &WorkPackageGraph,
    vision: &VisionSlug,
    contracts: &[RepositoryContract],
) -> Result<Vec<WorkPackageMergeObservation>> {
    let mut observations = Vec::new();
    for package in graph.packages() {
        let subject = WorkPackageMergeSubject::derive(vision, package.id().clone());
        for repository in package.repositories() {
            let matches = contracts
                .iter()
                .filter(|contract| contract.name().as_str() == repository)
                .collect::<Vec<_>>();
            let [contract] = matches.as_slice() else {
                bail!(
                    "expected exactly one repository contract for work package {} repository {}, found {}",
                    package.id().as_str(),
                    repository,
                    matches.len()
                );
            };
            let root = PathBuf::from(contract.root());
            let base = subject.selector().base().as_str().to_owned();
            let fetch = match verify_origin(&root) {
                Ok(()) => fetch_branch(&root, &base),
                Err(detail) => FetchResult::Unavailable { detail },
            };
            let runtime = RepositoryRuntime {
                name: contract.name().clone(),
                root,
                fetches: vec![BranchFetch {
                    branch: base,
                    result: fetch,
                }],
            };
            let github = observe_github(&runtime.root, subject.selector())?;
            let git = observe_git(&runtime, subject.selector(), &github)?;
            observations.push(WorkPackageMergeObservation::new(
                package.id().clone(),
                repository.clone(),
                derive_work_package_merge_status(&subject, &github, &git),
            ));
        }
    }
    Ok(observations)
}

fn render_work_package_readiness(
    graph: &WorkPackageGraph,
    observations: &[WorkPackageMergeObservation],
    risk_ordering: RiskOrdering,
) -> Result<()> {
    let report = ready_work_packages(graph, observations, risk_ordering)
        .context("failed to compute work-package readiness")?;
    let results = graph
        .packages()
        .iter()
        .zip(report.packages())
        .map(|(package, result)| {
            let criteria = package
                .criteria()
                .iter()
                .map(|criterion| {
                    json!({
                        "name": criterion.name(), "input": criterion.input(),
                        "observation": criterion.observation(), "command": criterion.command(),
                    })
                })
                .collect::<Vec<_>>();
            let repositories = observations
                .iter()
                .filter(|observation| observation.package() == package.id())
                .map(|observation| {
                    json!({
                        "repository": observation.repository(),
                        "merge_status": merge_status_label(observation.status()),
                    })
                })
                .collect::<Vec<_>>();
            let classification = match result.classification() {
                WorkPackageClassification::Merged => "merged",
                WorkPackageClassification::Ready => "ready",
                WorkPackageClassification::Waiting => "waiting",
                WorkPackageClassification::DependencyInconclusive => "dependency-inconclusive",
            };
            json!({
                "classification": classification,
                "merge_status": merge_status_label(result.merge_status()),
                "package": package.id().as_str(), "title": package.title(),
                "repositories": repositories, "criteria": criteria,
            })
        })
        .collect::<Vec<_>>();
    write_json_stdout(&json!({
        "plan_version": graph.plan_version(),
        "risk_ordering": if report.override_applied() { "overridden" } else { "honoured" },
        "override_applied": report.override_applied(),
        "results": results,
    }))
}

fn run_ready(
    log_path: &Path,
    recovery_log_path: &RecoveryLogPath,
    vision_dir: &Path,
    graph_path: Option<&ArtifactPath>,
    risk_ordering: RiskOrdering,
) -> Result<()> {
    let parsed_lines = read_event_log(log_path)?;
    let records = parsed_lines
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let contracts = repository_contracts(&records)?;
    let primary_index = resolve_primary_repository(&contracts, log_path, vision_dir)?;
    let vision = vision_slug(vision_dir)?;
    let ratified_criteria = read_ratified_acceptance_criteria(vision_dir)?;

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
    let state = derive_run_state(
        &records,
        &ratified_criteria,
        &vision,
        recovery_log_path,
        &artifacts,
        &[],
        &[],
    )
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
                if frozen_graph_version(&artifact_path).is_some() {
                    bail!(
                        "approved frozen graph {} is missing",
                        artifact_path.as_str()
                    );
                }
                tracing::info!(
                    artifact_path = artifact_path.as_str(),
                    reason = "artifact missing",
                    "skipping approved artifact candidate"
                );
                continue;
            };
            match parse_ready_graph(bytes) {
                Ok(graph) => {
                    selected = Some((approval_sequence, artifact_path, graph));
                    break;
                }
                Err(error) => {
                    if frozen_graph_version(&artifact_path).is_some() {
                        return Err(error).with_context(|| {
                            format!(
                                "approved frozen graph {} is invalid",
                                artifact_path.as_str()
                            )
                        });
                    }
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
    let ready_graph = match parsed_graph {
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
            parse_ready_graph(bytes).map_err(|error| {
                anyhow!(
                    "approved graph path {} is not a conforming graph: {error:#}",
                    artifact_path.as_str()
                )
            })?
        }
    };
    let graph = match ready_graph {
        ReadyGraph::WorkPackages(graph) => {
            validate_frozen_graph_identity(
                &graph,
                &artifact_path,
                vision_dir,
                Path::new(contracts[primary_index].root()),
            )?;
            let observations = observe_work_package_merges(&graph, &vision, &contracts)?;
            return render_work_package_readiness(&graph, &observations, risk_ordering);
        }
        ReadyGraph::Legacy(graph) => graph,
    };

    if risk_ordering == RiskOrdering::Override {
        bail!("--override-risk-ordering applies only to work-package graphs");
    }

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
            contract.release_tag(),
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
    measure_tracked_contract_at_root(contract_path, repository_root)?;
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
    let _previous =
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
        let measured = measure_stated_contract_at_root(tracked.stated(), repository_root)?;
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
    commit_bootstrapped_contract(repository_root)?;
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

fn commit_bootstrapped_contract(repository_root: &Path) -> Result<()> {
    for arguments in [
        vec!["add", "--", TRACKED_REPOSITORY_CONTRACT_PATH],
        vec![
            "commit",
            "--only",
            "-m",
            "chore: bootstrap PCE repository contract",
            "--",
            TRACKED_REPOSITORY_CONTRACT_PATH,
        ],
    ] {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repository_root)
            .args(&arguments)
            .output()
            .with_context(|| format!("failed to spawn `git {}`", arguments.join(" ")))?;
        if !output.status.success() {
            bail!(
                "failed to commit bootstrapped tracked repository contract with `git {}`: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
    }
    Ok(())
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
        verify_seatbelt_execution_capability(repository_root)?;
        BootstrapDerivation::CargoFallback {
            gates: BootstrapGateCommands {
                format: select_bootstrap_candidate(
                    "format",
                    FORMAT_BOOTSTRAP_CANDIDATES,
                    |command| execute_sandboxed_gate_text(repository_root, command),
                )?,
                lint: select_bootstrap_candidate("lint", LINT_BOOTSTRAP_CANDIDATES, |command| {
                    execute_sandboxed_gate_text(repository_root, command)
                })?,
                typecheck: select_bootstrap_candidate(
                    "typecheck",
                    TYPECHECK_BOOTSTRAP_CANDIDATES,
                    |command| execute_sandboxed_gate_text(repository_root, command),
                )?,
                test: select_bootstrap_candidate("test", TEST_BOOTSTRAP_CANDIDATES, |command| {
                    execute_sandboxed_gate_text(repository_root, command)
                })?,
                build: select_bootstrap_candidate(
                    "build",
                    BUILD_BOOTSTRAP_CANDIDATES,
                    |command| execute_sandboxed_gate_text(repository_root, command),
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
    let measured = measure_stated_contract_at_root(tracked.stated(), repository_root)?;
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
    let measured = measure_stated_contract_at_root(tracked.stated(), repository_root)?;
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
    if &payload == previous {
        return Ok(());
    }
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

fn measure_tracked_contract_at_root(
    contract_path: &Path,
    repository_root: &Path,
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
    measure_stated_contract_at_root(contract.stated(), repository_root)
}

fn measure_stated_contract_at_root(
    stated: &pce_core::StatedContract,
    repository_root: &Path,
) -> Result<MeasuredContractSnapshot> {
    verify_seatbelt_execution_capability(repository_root)?;
    measure_contract_snapshot(stated, |command| {
        execute_sandboxed_gate_command(repository_root, command)
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

fn execute_sandboxed_gate_command(
    repository_root: &Path,
    command: &GateCommand,
) -> std::io::Result<ObservedExitStatus> {
    execute_sandboxed_gate_text(repository_root, command.as_str())
}

fn verify_seatbelt_execution_capability(repository_root: &Path) -> std::io::Result<()> {
    if let SeatbeltCapability::Unavailable { status } =
        seatbelt_execution_capability(repository_root)?
    {
        return Err(std::io::Error::other(format!(
            "failed to verify Seatbelt execution capability: permissive profile probe exited with status {status}"
        )));
    }
    Ok(())
}

fn seatbelt_execution_capability(repository_root: &Path) -> std::io::Result<SeatbeltCapability> {
    let canonical_root = std::fs::canonicalize(repository_root).map_err(|error| {
        std::io::Error::other(format!(
            "failed to canonicalize Seatbelt probe working directory {}: {error}",
            repository_root.display()
        ))
    })?;
    let working_directory = AbsoluteWorkingDirectory::parse(canonical_root)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let envelope = seatbelt_capability_probe(working_directory)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let status = spawn_envelope(&envelope).map_err(|error| {
        std::io::Error::other(format!(
            "failed to verify Seatbelt execution capability: {error:#}"
        ))
    })?;
    Ok(classify_seatbelt_capability(status))
}

fn execute_sandboxed_gate_text(
    repository_root: &Path,
    command: &str,
) -> std::io::Result<ObservedExitStatus> {
    let parsed = parse_gate_command(command).map_err(|error| {
        std::io::Error::other(format!(
            "failed to parse stated gate command `{command}`: {error}"
        ))
    })?;
    let canonical_root = std::fs::canonicalize(repository_root).map_err(|error| {
        std::io::Error::other(format!(
            "failed to canonicalize gate repository root {}: {error}",
            repository_root.display()
        ))
    })?;
    let binary_temp_root = Path::new("/tmp");
    let canonical_temp = std::fs::canonicalize(binary_temp_root).map_err(|error| {
        std::io::Error::other(format!(
            "failed to canonicalize binary-owned temporary root {}: {error}",
            binary_temp_root.display()
        ))
    })?;
    let mut temporary_directories = std::collections::BTreeSet::new();
    temporary_directories.insert(canonical_temp);
    let profile = render_seatbelt_profile(&canonical_root, &temporary_directories)?;

    let environment = gate_child_environment();
    let resolved_executable = resolve_gate_executable(parsed.executable(), &environment)?;
    let mut arguments = vec![
        "-p".to_owned(),
        profile,
        "--".to_owned(),
        resolved_executable.as_str().to_owned(),
    ];
    arguments.extend(parsed.arguments().as_slice().iter().cloned());
    let envelope = DispatchEnvelope::new(
        DispatchTarget::Seatbelt,
        AbsoluteWorkingDirectory::parse(canonical_root)
            .map_err(|error| std::io::Error::other(error.to_string()))?,
        StdinBinding::Null,
    )
    .with_arguments(ArgumentVector::new(arguments))
    .with_environment(environment);
    spawn_envelope(&envelope).map_err(|error| {
        std::io::Error::other(format!(
            "failed to execute stated gate command `{command}` in Seatbelt: {error:#}"
        ))
    })
}

fn gate_child_environment() -> ChildEnvironment {
    let mut environment = BTreeMap::new();
    for name in ["PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME"] {
        if let Ok(value) = std::env::var(name) {
            environment.insert(name.to_owned(), value);
        }
    }
    ChildEnvironment::new(environment)
}

fn resolve_gate_executable(
    executable: &Executable,
    environment: &ChildEnvironment,
) -> std::io::Result<Executable> {
    let raw = Path::new(executable.as_str());
    if raw.is_absolute() {
        return executable_path(raw);
    }
    let path = environment
        .iter()
        .find_map(|(name, value)| (name == "PATH").then_some(value))
        .ok_or_else(|| {
            std::io::Error::other(format!(
                "cannot resolve gate executable `{}` without forwarded PATH",
                executable.as_str()
            ))
        })?;
    for directory in std::env::split_paths(path) {
        let candidate = directory.join(raw);
        if let Ok(resolved) = executable_path(&candidate) {
            return Ok(resolved);
        }
    }
    Err(std::io::Error::other(format!(
        "gate executable `{}` was not found as an executable file on forwarded PATH",
        executable.as_str()
    )))
}

fn executable_path(path: &Path) -> std::io::Result<Executable> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        std::io::Error::other(format!(
            "failed to inspect gate executable {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(std::io::Error::other(format!(
            "gate executable {} is not an executable file",
            path.display()
        )));
    }
    let raw = path.to_str().ok_or_else(|| {
        std::io::Error::other(format!(
            "gate executable path {} is not valid UTF-8",
            path.display()
        ))
    })?;
    Executable::parse(raw).map_err(|error| std::io::Error::other(error.to_string()))
}

fn render_seatbelt_profile(
    repository_root: &Path,
    temporary_directories: &std::collections::BTreeSet<PathBuf>,
) -> std::io::Result<String> {
    let repository_root = seatbelt_path(repository_root)?;
    let temporary_directories = temporary_directories
        .iter()
        .map(|directory| seatbelt_path(directory))
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut profile = concat!(
        "(version 1)\n",
        "(deny default)\n",
        "(import \"system.sb\")\n",
        "(allow process*)\n",
        "(allow signal (target children))\n",
        "(allow signal (target same-sandbox))\n",
        "(allow file-read*)\n"
    )
    .to_owned();
    profile.push_str("(deny file-write*\n  (require-all\n");
    profile.push_str(&format!(
        "    (require-not (subpath \"{repository_root}\"))\n"
    ));
    for directory in &temporary_directories {
        profile.push_str(&format!("    (require-not (subpath \"{directory}\"))\n"));
    }
    profile.push_str("  ))\n(allow file-write*\n");
    profile.push_str(&format!("  (subpath \"{repository_root}\")\n"));
    for directory in &temporary_directories {
        profile.push_str(&format!("  (subpath \"{directory}\")\n"));
    }
    profile.push_str(")\n(deny network*\n  (require-all\n");
    profile.push_str(&format!(
        "    (require-not (subpath \"{repository_root}\"))\n"
    ));
    for directory in &temporary_directories {
        profile.push_str(&format!("    (require-not (subpath \"{directory}\"))\n"));
    }
    profile.push_str("  ))\n(allow network-bind network-outbound\n");
    profile.push_str(&format!("  (subpath \"{repository_root}\")\n"));
    for directory in &temporary_directories {
        profile.push_str(&format!("  (subpath \"{directory}\")\n"));
    }
    profile.push(')');
    Ok(profile)
}

fn seatbelt_path(path: &Path) -> std::io::Result<String> {
    let raw = path.to_str().ok_or_else(|| {
        std::io::Error::other(format!(
            "Seatbelt path {} is not valid UTF-8",
            path.display()
        ))
    })?;
    Ok(raw.replace('\\', "\\\\").replace('"', "\\\""))
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

fn run_dispatch_check_in(log_path: &Path, output: &mut dyn Write) -> Result<()> {
    let parsed = read_event_log(log_path)?;
    let records = parsed
        .iter()
        .map(|line| line.record.clone())
        .collect::<Vec<_>>();
    let issuance_sequences = records
        .iter()
        .filter_map(|record| match record.body_ref() {
            EventBodyRef::Known(KnownPayload::Dispatch(_)) => Some(record.sequence()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let identity_directory = dispatch_identity_directory(log_path);
    let mut observations = Vec::with_capacity(issuance_sequences.len());
    for issuance_sequence in issuance_sequences {
        let sidecar_path = identity_directory.join(format!("{}.json", issuance_sequence.get()));
        let Some(identity) = read_dispatch_identity_sidecar(&sidecar_path)? else {
            observations.push(DispatchIdentityObservation::unrecorded(issuance_sequence));
            continue;
        };
        if identity.issuance_sequence() != issuance_sequence {
            bail!(
                "dispatch process identity sidecar issuance sequence {} does not match event issuance {}",
                identity.issuance_sequence().get(),
                issuance_sequence.get()
            );
        }
        let process_identity = observe_darwin_process_number(identity.process_number())?;
        let artifact_production = artifact_production(observe_required_artifact_presence(
            identity.required_artifact_path().as_path(),
        )?);
        observations.push(DispatchIdentityObservation::new(
            issuance_sequence,
            identity.process_start_identity(),
            process_identity,
            artifact_production,
        ));
    }
    let report = classify_dispatch_check_in(&records, &observations)?;
    let bytes = serialize_dispatch_check_in(&report)?;
    output
        .write_all(&bytes)
        .context("failed to write dispatch check-in report")?;
    output
        .flush()
        .context("failed to flush dispatch check-in report")
}

fn read_dispatch_identity_sidecar(path: &Path) -> Result<Option<DispatchProcessIdentity>> {
    const MAX_SIDECAR_BYTES: u64 = 65_536;

    let mut file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).context("failed to open dispatch process identity sidecar");
        }
    };
    let metadata = file
        .metadata()
        .context("failed to read dispatch process identity sidecar")?;
    if !metadata.file_type().is_file() {
        bail!("dispatch process identity sidecar is not a regular file");
    }
    if metadata.uid() != unsafe { libc::geteuid() } {
        bail!("dispatch process identity sidecar is not owned by the current user");
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_SIDECAR_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("failed to read dispatch process identity sidecar")?;
    if bytes.len() > MAX_SIDECAR_BYTES as usize {
        bail!("dispatch process identity sidecar exceeds 65536 bytes");
    }
    parse_dispatch_process_identity(&bytes)
        .map(Some)
        .context("failed to parse dispatch process identity sidecar")
}

fn observe_required_artifact_presence(path: &Path) -> Result<RequiredArtifactPresence> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(RequiredArtifactPresence::Present),
        Ok(_) => Ok(RequiredArtifactPresence::Absent),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(RequiredArtifactPresence::Absent)
        }
        Err(error) => Err(error).context("failed to inspect required dispatch artifact"),
    }
}

const fn artifact_production(presence: RequiredArtifactPresence) -> ArtifactProduction {
    match presence {
        RequiredArtifactPresence::Present => ArtifactProduction::Produced,
        RequiredArtifactPresence::Absent => ArtifactProduction::NotProduced,
    }
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

fn run_log_meter(input: &mut dyn Read, output: &mut dyn Write) -> Result<()> {
    // The meter accepts JSONL only on standard input so it has no path or transcript-read
    // capability, while `pce log` and `pce log read` retain path arguments for append and raw
    // retrieval.
    let mut reader = BufReader::new(input);
    let mut records = Vec::<EventRecord>::new();
    let mut physical_line = 0_u64;
    let mut buffer = String::new();
    loop {
        buffer.clear();
        let bytes = reader.read_line(&mut buffer).with_context(|| {
            format!(
                "failed to read physical line {} from meter stdin",
                physical_line + 1
            )
        })?;
        if bytes == 0 {
            break;
        }
        physical_line += 1;
        let parsed = buffer.strip_suffix('\n').unwrap_or(&buffer);
        let record = parse_event_line(parsed).with_context(|| {
            format!("failed to parse physical line {physical_line} from meter stdin")
        })?;
        if let Some(previous) = records.last()
            && record.sequence().get() <= previous.sequence().get()
        {
            bail!(
                "meter stdin has non-increasing sequence {} at physical line {} after {}",
                record.sequence().get(),
                physical_line,
                previous.sequence().get()
            );
        }
        records.push(record);
    }

    let report = meter_dispatches(records.as_slice())?;
    let mut serialized = Vec::new();
    for record in &report {
        serde_json::to_writer(&mut serialized, record)
            .context("failed to serialize dispatch meter report")?;
        serialized.push(b'\n');
    }
    output
        .write_all(&serialized)
        .context("failed to write dispatch meter report")?;
    output
        .flush()
        .context("failed to flush dispatch meter report")
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

fn read_ratified_acceptance_criteria(vision_dir: &Path) -> Result<AcceptanceCriteria> {
    let path = vision_dir.join("vision.md");
    let document = fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read ratified acceptance criteria from {}",
            path.display()
        )
    })?;
    parse_acceptance_criteria(&document).with_context(|| {
        format!(
            "failed to parse ratified acceptance criteria from {}",
            path.display()
        )
    })
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

fn declared_exceptional_merge_chains(
    records: &[EventRecord],
    vision: &VisionSlug,
) -> Result<Vec<(NodeId, ExceptionalMergeChain)>> {
    let mut chains = Vec::new();
    for record in records {
        let EventBodyRef::Known(KnownPayload::ExceptionalMergeChainDeclared(payload)) =
            record.body_ref()
        else {
            continue;
        };
        let Ok(step) = StepNode::parse(record.node()) else {
            continue;
        };
        chains.push((
            record.node().clone(),
            ExceptionalMergeChain::new(
                vision,
                step,
                payload.integration_branch.clone(),
                payload.step_pull_request_number,
                payload.promotion_pull_request_number,
            )
            .context("failed to assemble exceptional merge chain")?,
        ));
    }
    Ok(chains)
}

fn landing_integration_branches(
    nodes: &[CanonicalNode],
    exceptional: &[(NodeId, ExceptionalMergeChain)],
) -> Vec<String> {
    let mut branches = Vec::new();
    for node in nodes {
        if let Some((_, chain)) = exceptional
            .iter()
            .find(|(declared, _)| declared == &node.node)
        {
            for branch in [
                chain.integration_branch().as_str(),
                chain.promotion_pull_request().selector().base().as_str(),
            ] {
                if !branches.iter().any(|existing| existing == branch) {
                    branches.push(branch.to_owned());
                }
            }
        } else {
            let branch = node.subject.integration_branch().as_str();
            if !branches.iter().any(|existing| existing == branch) {
                branches.push(branch.to_owned());
            }
        }
    }
    branches
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
    release_tag: &str,
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
    let tag_state = probe_tag(&root, release_tag)?;
    let observation = RepositoryObservation::new(
        name.clone(),
        fetch,
        RepositoryBranchName::parse(selected_branch)
            .context("failed to parse repository branch name")?,
        branch_state,
        WorktreeIdentity::parse(head).context("failed to parse worktree identity")?,
        worktree_state,
        TagName::parse(release_tag).context("failed to parse release tag name")?,
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

fn probe_tag(root: &Path, release_tag: &str) -> Result<TagState> {
    let reference = format!("refs/tags/{release_tag}^{{}}");
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
    let result = match execute_process_without_force_color("gh", &args, Some(root)) {
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

fn run_gate_exec(input: &mut dyn Read) -> Result<()> {
    let mut request_bytes = Vec::new();
    input
        .read_to_end(&mut request_bytes)
        .context("failed to read gate execution request from stdin")?;
    let stimulus = match parse_gate_stimulus(&request_bytes) {
        Ok(stimulus) => stimulus,
        Err(error) => exit_gate_exec_rejection(&error.to_string()),
    };
    let socket = match std::env::var_os("PCE_GATE_EXEC_SOCKET") {
        Some(value) => PathBuf::from(value),
        None => exit_gate_exec_rejection("PCE_GATE_EXEC_SOCKET is required for `pce gate exec`"),
    };
    let wire =
        serde_json::to_vec(&stimulus).context("failed to serialize gate execution request")?;
    let mut stream = UnixStream::connect(&socket).with_context(|| {
        format!(
            "failed to connect to gate execution socket {}",
            socket.display()
        )
    })?;
    stream
        .write_all(&wire)
        .context("failed to write gate execution request")?;
    stream
        .shutdown(Shutdown::Write)
        .context("failed to finish gate execution request")?;
    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .context("failed to read gate execution response")?;
    if let Ok(rejection) = serde_json::from_slice::<GateExecutionRejection>(&response_bytes) {
        exit_gate_exec_rejection(&rejection.error);
    }
    let response: GateExecutionResponse = serde_json::from_slice(&response_bytes)
        .context("failed to decode gate execution response")?;
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &response)
        .context("failed to write gate execution response to stdout")?;
    stdout
        .write_all(b"\n")
        .context("failed to terminate gate execution response")?;
    stdout
        .flush()
        .context("failed to flush gate execution response")
}

enum ResolvedReplayRef {
    Commit(String),
    Failed(ReplayRefResult),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReplaySide {
    Broken,
    Repaired,
}

enum ReplayRefCampaign {
    Complete(Box<ReplayRefResult>),
    Running {
        requested_ref: NamedReplayRef,
        resolved_commit: String,
        observations: Vec<ReplayObservation>,
    },
}

impl ReplayRefCampaign {
    fn new(requested_ref: NamedReplayRef, resolved: ResolvedReplayRef) -> Self {
        match resolved {
            ResolvedReplayRef::Commit(resolved_commit) => Self::Running {
                requested_ref,
                resolved_commit,
                observations: Vec::with_capacity(2),
            },
            ResolvedReplayRef::Failed(failure) => Self::Complete(Box::new(failure)),
        }
    }

    fn finish(self, expected: ExpectedVerdictOutcome) -> Result<ReplayRefResult> {
        match self {
            Self::Complete(result) => Ok(*result),
            Self::Running {
                requested_ref,
                resolved_commit,
                mut observations,
            } => {
                let second = observations.pop().ok_or_else(|| {
                    anyhow!("clean replay did not produce its second observation")
                })?;
                let first = observations
                    .pop()
                    .ok_or_else(|| anyhow!("clean replay did not produce its first observation"))?;
                Ok(ReplayRefResult::Completed {
                    requested_ref,
                    resolved_commit,
                    outcome: fold_replay_runs(first, second, expected),
                })
            }
        }
    }
}

struct ReplayPauseGuard {
    resolved_marker: Option<PathBuf>,
}

struct ReplayParentGuard(PathBuf);

impl Drop for ReplayParentGuard {
    fn drop(&mut self) {
        let _result = std::fs::remove_dir_all(&self.0);
    }
}

impl Drop for ReplayPauseGuard {
    fn drop(&mut self) {
        if let Some(path) = self.resolved_marker.take() {
            let _result = std::fs::remove_file(path);
        }
    }
}

fn exec_gate_replay_worker(mut command: GateReplayCommand) -> Result<()> {
    command.pause_directory = parse_replay_pause_seam()?;
    let expected = match command.expected {
        ExpectedVerdictOutcome::Conforming => "conforming-verdict",
        ExpectedVerdictOutcome::Nonconforming => "nonconforming-verdict",
    };
    let wire = serde_json::to_vec(&json!({
        "repository_root": command.repository_root,
        "recorded_root": command.recorded_root,
        "evidence_path": command.evidence_path,
        "execution_ref": command.execution_ref.as_str(),
        "broken_ref": command.broken_ref.as_str(),
        "repaired_ref": command.repaired_ref.as_str(),
        "schema_path": command.schema_path.as_path(),
        "output_path": command.output_path.as_path(),
        "expected": expected,
        "pause_directory": command.pause_directory,
    }))
    .context("failed to encode private gate replay request")?;

    let parent = ReplayParentGuard(create_replay_parent()?);
    let request_path = parent.0.join("request");
    let mut request = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&request_path)
        .context("failed to create private gate replay request")?;
    request
        .write_all(&wire)
        .context("failed to write private gate replay request")?;
    request
        .seek(SeekFrom::Start(0))
        .context("failed to rewind private gate replay request")?;
    std::fs::remove_file(&request_path).context("failed to unlink private gate replay request")?;
    std::fs::remove_dir(&parent.0).context("failed to remove private gate replay request root")?;
    std::mem::forget(parent);

    let executable = std::env::current_exe().context("failed to locate gate replay worker")?;
    let error = std::process::Command::new(executable)
        .args(["gate", "replay-worker"])
        .env_remove(REPLAY_PAUSE_ENV)
        .stdin(Stdio::from(request))
        .exec();
    Err(Error::new(error).context("failed to replace gate replay launcher"))
}

fn read_gate_replay_worker_request(input: &mut dyn Read) -> Result<GateReplayCommand> {
    let mut wire = Vec::new();
    input
        .take(1024 * 1024)
        .read_to_end(&mut wire)
        .context("failed to read private gate replay request")?;
    let value: Value =
        serde_json::from_slice(&wire).context("failed to parse private gate replay request")?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("failed to parse private gate replay request"))?;
    require_exact_keys(
        object,
        &[
            "repository_root",
            "recorded_root",
            "evidence_path",
            "execution_ref",
            "broken_ref",
            "repaired_ref",
            "schema_path",
            "output_path",
            "expected",
            "pause_directory",
        ],
    )
    .context("failed to parse private gate replay request")?;
    let string = |key: &str| -> Result<&str> {
        object
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("failed to parse private gate replay request"))
    };
    let pause_directory = match object.get("pause_directory") {
        Some(Value::Null) => None,
        Some(value) => {
            Some(PathBuf::from(value.as_str().ok_or_else(|| {
                anyhow!("failed to parse private gate replay request")
            })?))
        }
        None => return Err(anyhow!("failed to parse private gate replay request")),
    };
    let recorded_root = match object.get("recorded_root") {
        Some(Value::Null) => None,
        Some(value) => {
            Some(PathBuf::from(value.as_str().ok_or_else(|| {
                anyhow!("failed to parse private gate replay request")
            })?))
        }
        None => return Err(anyhow!("failed to parse private gate replay request")),
    };
    Ok(GateReplayCommand {
        repository_root: PathBuf::from(string("repository_root")?),
        recorded_root,
        evidence_path: PathBuf::from(string("evidence_path")?),
        execution_ref: GateExecutionRef::parse(string("execution_ref")?.to_owned())?,
        broken_ref: NamedReplayRef::parse(string("broken_ref")?.to_owned())?,
        repaired_ref: NamedReplayRef::parse(string("repaired_ref")?.to_owned())?,
        schema_path: parse_replay_schema_path(string("schema_path")?)?,
        output_path: parse_replay_output_path(string("output_path")?)?,
        expected: ExpectedVerdictOutcome::parse(string("expected")?)?,
        pause_directory,
    })
}

fn run_gate_replay(command: GateReplayCommand) -> Result<()> {
    let started = Instant::now();
    let overall_deadline = started + REPLAY_OVERALL_TIMEOUT;
    let repository_root = std::fs::canonicalize(&command.repository_root)
        .context("failed to canonicalize replay repository root")?;
    let recorded_root = command
        .recorded_root
        .unwrap_or_else(|| repository_root.clone());
    if !recorded_root.is_absolute() {
        bail!("recorded replay root must be absolute");
    }
    let evidence_bytes =
        std::fs::read(&command.evidence_path).context("failed to read gate execution evidence")?;
    let evidence = parse_gate_execution_evidence(&evidence_bytes)?;
    let record = evidence.record(&command.execution_ref)?;
    rebase_gate_stimulus(&record.stimulus, &recorded_root, &repository_root)?;
    let broken_resolved =
        resolve_replay_ref(&repository_root, &command.broken_ref, overall_deadline)?;
    let repaired_resolved =
        resolve_replay_ref(&repository_root, &command.repaired_ref, overall_deadline)?;
    let _pause_guard = if matches!(broken_resolved, ResolvedReplayRef::Commit(_))
        && matches!(repaired_resolved, ResolvedReplayRef::Commit(_))
    {
        run_replay_pause_seam(command.pause_directory.as_deref(), overall_deadline)?
    } else {
        ReplayPauseGuard {
            resolved_marker: None,
        }
    };

    let stimulus = record.stimulus.clone();
    let mut broken = ReplayRefCampaign::new(command.broken_ref, broken_resolved);
    let mut repaired = ReplayRefCampaign::new(command.repaired_ref, repaired_resolved);
    for side in create_replay_schedule()? {
        let campaign = match side {
            ReplaySide::Broken => &mut broken,
            ReplaySide::Repaired => &mut repaired,
        };
        evaluate_replay_ref_run(
            campaign,
            &repository_root,
            &recorded_root,
            &stimulus,
            &command.schema_path,
            &command.output_path,
            overall_deadline,
        )?;
    }
    reconcile_identical_commit_observations(&mut broken, &mut repaired);
    let broken = broken.finish(command.expected)?;
    let repaired = repaired.finish(command.expected)?;
    require_replay_overall_deadline(overall_deadline)?;
    let classification = classify_replay_pair(&broken, &repaired);
    let report =
        serialize_replay_report(&command.execution_ref, &broken, &repaired, classification)?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&report)
        .context("failed to terminate gate replay report")?;
    stdout.flush().context("failed to flush gate replay report")
}

fn serialize_replay_report(
    execution_ref: &GateExecutionRef,
    broken: &ReplayRefResult,
    repaired: &ReplayRefResult,
    classification: pce_core::RepairSensitivity,
) -> Result<Vec<u8>> {
    let mut report = Vec::new();
    report.extend_from_slice(
        br#"{"schema_id":"pce.gate-replay-report","schema_version":1,"execution_ref":"#,
    );
    serde_json::to_writer(&mut report, execution_ref)?;
    report.extend_from_slice(b",\"broken\":");
    serde_json::to_writer(&mut report, broken)?;
    report.extend_from_slice(b",\"repaired\":");
    serde_json::to_writer(&mut report, repaired)?;
    report.extend_from_slice(b",\"classification\":");
    serde_json::to_writer(&mut report, &classification)?;
    report.extend_from_slice(b"}\n");
    Ok(report)
}

fn parse_replay_pause_seam() -> Result<Option<PathBuf>> {
    let Some(value) = std::env::var_os(REPLAY_PAUSE_ENV) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    let valid = path.is_absolute()
        && path.is_dir()
        && !path.join("resolved").exists()
        && !path.join("continue").exists();
    if !valid {
        bail!(
            "replay pause-after-resolve seam requires an existing absolute directory with no `resolved` or `continue` entry"
        );
    }
    Ok(Some(path))
}

fn run_replay_pause_seam(
    directory: Option<&Path>,
    overall_deadline: Instant,
) -> Result<ReplayPauseGuard> {
    let Some(directory) = directory else {
        return Ok(ReplayPauseGuard {
            resolved_marker: None,
        });
    };
    let resolved = directory.join("resolved");
    OpenOptions::new().write(true).create_new(true).open(&resolved)
        .context("replay pause-after-resolve seam requires an existing absolute directory with no `resolved` or `continue` entry")?;
    let guard = ReplayPauseGuard {
        resolved_marker: Some(resolved),
    };
    let deadline = Instant::now() + REPLAY_PAUSE_TIMEOUT;
    loop {
        if Instant::now() >= overall_deadline {
            bail!("gate replay exceeded its 40 minute overall deadline");
        }
        if Instant::now() >= deadline {
            bail!("replay pause-after-resolve seam timed out after 10 seconds");
        }
        let continuation = directory.join("continue");
        match std::fs::metadata(&continuation) {
            Ok(metadata) if metadata.is_file() && metadata.len() == 0 => return Ok(guard),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::debug!(error = ?error, "failed to inspect replay seam continuation")
            }
        }
        std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL);
    }
}

fn create_replay_parent() -> Result<PathBuf> {
    for _attempt in 0..1000 {
        let mut opaque = [0_u8; 16];
        File::open("/dev/urandom")
            .context("failed to open operating-system randomness for gate replay")?
            .read_exact(&mut opaque)
            .context("failed to read operating-system randomness for gate replay")?;
        let identifier = opaque
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = std::env::temp_dir().join(format!("pce-gate-replay-{identifier}"));
        match std::fs::create_dir(&path) {
            Ok(()) => {
                return std::fs::canonicalize(&path).map_err(|error| {
                    Error::new(error)
                        .context("failed to canonicalize gate replay temporary directory")
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(
                    Error::new(error).context("failed to create gate replay temporary directory")
                );
            }
        }
    }
    bail!("failed to create gate replay temporary directory")
}

fn create_replay_schedule() -> Result<[ReplaySide; 4]> {
    let mut random = File::open("/dev/urandom")
        .context("failed to open operating-system randomness for gate replay")?;
    create_replay_schedule_from(&mut random)
}

fn create_replay_schedule_from(random: &mut impl Read) -> Result<[ReplaySide; 4]> {
    let mut schedule = [
        ReplaySide::Broken,
        ReplaySide::Broken,
        ReplaySide::Repaired,
        ReplaySide::Repaired,
    ];
    for index in (1..schedule.len()).rev() {
        let selected = replay_random_index(random, index + 1)?;
        schedule.swap(index, selected);
    }
    Ok(schedule)
}

fn replay_random_index(random: &mut impl Read, upper_bound: usize) -> Result<usize> {
    let acceptance_limit = 256 - (256 % upper_bound);
    loop {
        let mut byte = [0_u8; 1];
        random
            .read_exact(&mut byte)
            .context("failed to read operating-system randomness for gate replay")?;
        if usize::from(byte[0]) < acceptance_limit {
            return Ok(usize::from(byte[0]) % upper_bound);
        }
    }
}

fn reconcile_identical_commit_observations(
    broken: &mut ReplayRefCampaign,
    repaired: &mut ReplayRefCampaign,
) {
    let (
        ReplayRefCampaign::Running {
            resolved_commit: broken_commit,
            observations: broken_observations,
            ..
        },
        ReplayRefCampaign::Running {
            resolved_commit: repaired_commit,
            observations: repaired_observations,
            ..
        },
    ) = (broken, repaired)
    else {
        return;
    };
    if broken_commit != repaired_commit {
        return;
    }
    let Some(first) = broken_observations.first().cloned() else {
        return;
    };
    let different = broken_observations
        .iter()
        .chain(repaired_observations.iter())
        .find(|observation| **observation != first)
        .cloned();
    if let Some(different) = different {
        *broken_observations = vec![first.clone(), different.clone()];
        *repaired_observations = vec![first, different];
    }
}

fn resolve_replay_ref(
    repository_root: &Path,
    requested: &NamedReplayRef,
    overall_deadline: Instant,
) -> Result<ResolvedReplayRef> {
    require_replay_overall_deadline(overall_deadline)?;
    let revision = format!("{}^{{commit}}", requested.as_str());
    let output = run_replay_git(
        repository_root,
        &["rev-parse", "--verify", "--end-of-options", &revision],
        overall_deadline,
    );
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            tracing::debug!(requested_ref = requested.as_str(), error = ?error, "replay ref resolution failed");
            return Ok(ResolvedReplayRef::Failed(ReplayRefResult::CheckoutFailed {
                requested_ref: requested.clone(),
                checkout_failed: CheckoutFailure {
                    stage: CheckoutStage::ResolveRef,
                    diagnostic: format!(
                        "failed to resolve replay ref `{}` to a commit",
                        requested.as_str()
                    ),
                },
            }));
        }
    };
    if !output.status.success() {
        tracing::debug!(requested_ref = requested.as_str(), stderr = %String::from_utf8_lossy(&output.stderr), "replay ref resolution failed");
        return Ok(ResolvedReplayRef::Failed(ReplayRefResult::CheckoutFailed {
            requested_ref: requested.clone(),
            checkout_failed: CheckoutFailure {
                stage: CheckoutStage::ResolveRef,
                diagnostic: format!(
                    "failed to resolve replay ref `{}` to a commit",
                    requested.as_str()
                ),
            },
        }));
    }
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Ok(ResolvedReplayRef::Failed(ReplayRefResult::CheckoutFailed {
            requested_ref: requested.clone(),
            checkout_failed: CheckoutFailure {
                stage: CheckoutStage::ResolveRef,
                diagnostic: format!(
                    "failed to resolve replay ref `{}` to a commit",
                    requested.as_str()
                ),
            },
        }));
    }
    Ok(ResolvedReplayRef::Commit(commit))
}

fn evaluate_replay_ref_run(
    campaign: &mut ReplayRefCampaign,
    repository_root: &Path,
    recorded_root: &Path,
    stimulus: &GateStimulus,
    schema_path: &RepositoryRelativePath,
    output_path: &RepositoryRelativePath,
    overall_deadline: Instant,
) -> Result<()> {
    let ReplayRefCampaign::Running {
        requested_ref,
        resolved_commit,
        observations,
    } = campaign
    else {
        return Ok(());
    };
    let parent = ReplayParentGuard(create_replay_parent()?);
    let checkout = parent.0.join("checkout");
    if let Err(error) = create_isolated_replay_checkout(
        repository_root,
        &checkout,
        resolved_commit,
        overall_deadline,
    ) {
        tracing::debug!(requested_ref = requested_ref.as_str(), error = ?error, "isolated replay checkout creation failed");
        let diagnostic = format!(
            "failed to create clean replay checkout for ref `{}`",
            requested_ref.as_str()
        );
        cleanup_replay_parent(&parent.0)?;
        std::mem::forget(parent);
        *campaign = ReplayRefCampaign::Complete(Box::new(ReplayRefResult::CheckoutFailed {
            requested_ref: requested_ref.clone(),
            checkout_failed: CheckoutFailure {
                stage: CheckoutStage::CreateWorktree,
                diagnostic,
            },
        }));
        return Ok(());
    }
    let run_result = run_clean_replay(
        recorded_root,
        &checkout,
        requested_ref,
        stimulus,
        schema_path,
        output_path,
        overall_deadline,
    );
    validate_replay_cleanup_target(&parent.0, &checkout)?;
    cleanup_replay_parent(&parent.0)?;
    std::mem::forget(parent);
    match run_result? {
        CleanReplayResult::Observed(observation) => observations.push(observation),
        CleanReplayResult::OracleFailed(failure) => {
            *campaign = ReplayRefCampaign::Complete(Box::new(ReplayRefResult::OracleFailed {
                requested_ref: requested_ref.clone(),
                resolved_commit: resolved_commit.clone(),
                oracle_failed: failure,
            }));
        }
    }
    Ok(())
}

fn create_isolated_replay_checkout(
    repository_root: &Path,
    checkout: &Path,
    resolved_commit: &str,
    overall_deadline: Instant,
) -> Result<()> {
    let replay_parent = checkout
        .parent()
        .ok_or_else(|| anyhow!("replay checkout must have a parent"))?;
    let private_repository = replay_parent.join("repository.git");
    let checkout_spelling = checkout
        .to_str()
        .ok_or_else(|| anyhow!("replay checkout path must be UTF-8"))?;
    let private_repository_spelling = private_repository
        .to_str()
        .ok_or_else(|| anyhow!("private replay repository path must be UTF-8"))?;
    let source_spelling = repository_root
        .to_str()
        .ok_or_else(|| anyhow!("replay repository path must be UTF-8"))?;
    for (cwd, arguments) in [
        (
            replay_parent,
            vec!["init", "--quiet", "--bare", private_repository_spelling],
        ),
        (
            replay_parent,
            vec![
                &format!("--git-dir={private_repository_spelling}"),
                "fetch",
                "--quiet",
                "--no-tags",
                "--no-write-fetch-head",
                source_spelling,
                resolved_commit,
            ],
        ),
    ] {
        let output = run_replay_git(cwd, &arguments, overall_deadline)?;
        if !output.status.success() {
            tracing::debug!(stderr = ?output.stderr, "isolated replay checkout git command failed");
            bail!("isolated replay checkout git command failed");
        }
    }
    std::fs::create_dir(checkout).context("failed to create replay checkout directory")?;
    let git_dir_argument = format!("--git-dir={private_repository_spelling}");
    let work_tree_argument = format!("--work-tree={checkout_spelling}");
    let arguments = [
        git_dir_argument.as_str(),
        work_tree_argument.as_str(),
        "checkout",
        "--quiet",
        "--force",
        resolved_commit,
        "--",
        ".",
    ];
    let output = run_replay_git(replay_parent, &arguments, overall_deadline)?;
    if !output.status.success() {
        tracing::debug!(stderr = ?output.stderr, "isolated replay tree materialization failed");
        bail!("isolated replay checkout git command failed");
    }
    std::fs::remove_dir_all(&private_repository)
        .context("failed to remove private replay repository after tree materialization")?;
    Ok(())
}

fn validate_replay_cleanup_target(parent: &Path, checkout: &Path) -> Result<()> {
    let valid = checkout.parent() == Some(parent)
        && checkout.file_name().is_some_and(|name| name == "checkout");
    if !valid {
        bail!("gate replay cleanup failed");
    }
    Ok(())
}

enum CleanReplayResult {
    Observed(ReplayObservation),
    OracleFailed(OracleFailure),
}

fn run_clean_replay(
    recorded_root: &Path,
    checkout: &Path,
    requested_ref: &NamedReplayRef,
    stimulus: &GateStimulus,
    schema_relative: &RepositoryRelativePath,
    output_relative: &RepositoryRelativePath,
    overall_deadline: Instant,
) -> Result<CleanReplayResult> {
    let replay_parent = checkout
        .parent()
        .ok_or_else(|| anyhow!("replay checkout must have a parent"))?;
    let output_path = checkout.join(output_relative.as_path());
    if std::fs::symlink_metadata(&output_path).is_ok() {
        return Ok(CleanReplayResult::OracleFailed(OracleFailure {
            stage: OracleStage::PrepareOutput,
            diagnostic: format!(
                "replay oracle output already exists at ref `{}`",
                requested_ref.as_str()
            ),
        }));
    }
    let schema_path = checkout.join(schema_relative.as_path());
    let schema_bytes = match std::fs::read(&schema_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::debug!(error = ?error, requested_ref = requested_ref.as_str(), "replay oracle schema read failed");
            return Ok(CleanReplayResult::OracleFailed(OracleFailure {
                stage: OracleStage::ReadSchema,
                diagnostic: format!(
                    "replay oracle schema is unavailable at ref `{}`",
                    requested_ref.as_str()
                ),
            }));
        }
    };
    if !replay_schema_compiles(&schema_path, &schema_bytes, &output_path)? {
        return Ok(CleanReplayResult::OracleFailed(OracleFailure {
            stage: OracleStage::ReadSchema,
            diagnostic: format!(
                "replay oracle schema is unavailable at ref `{}`",
                requested_ref.as_str()
            ),
        }));
    }
    require_replay_overall_deadline(overall_deadline)?;
    let rebased = rebase_gate_stimulus(stimulus, recorded_root, checkout)?;
    materialize_paired_replay_program(checkout, &rebased)?;
    let execution = execute_replay_stimulus_until(&rebased, overall_deadline);
    let process = execution.observed_result.ok_or_else(|| {
        anyhow!(
            execution
                .diagnostic
                .unwrap_or_else(|| "clean replay did not produce an observation".to_owned())
        )
    })?;
    require_replay_overall_deadline(overall_deadline)?;
    let artifact = observe_replay_artifact(&schema_path, &schema_bytes, &output_path)?;
    let normalized = normalize_replay_observation(
        ReplayObservation { process, artifact },
        recorded_root,
        checkout,
        replay_parent,
    )?;
    Ok(CleanReplayResult::Observed(normalized))
}

fn materialize_paired_replay_program(checkout: &Path, stimulus: &GateStimulus) -> Result<()> {
    let Some(relative) = std::env::var_os(PAIRED_REPLAY_PROGRAM_ENV) else {
        return Ok(());
    };
    let relative = parse_replay_output_path(
        relative
            .to_str()
            .ok_or_else(|| anyhow!("paired replay program path must be UTF-8"))?,
    )?;
    let destination = checkout.join(relative.as_path());
    let recorded_program = Path::new(stimulus.command().program());
    if recorded_program != relative.as_path() && recorded_program != destination {
        return Ok(());
    }
    let parent = destination
        .parent()
        .ok_or_else(|| anyhow!("paired replay program path has no parent"))?;
    std::fs::create_dir_all(parent).context("failed to create paired replay program root")?;
    let source = std::env::current_exe().context("failed to locate paired replay executable")?;
    std::fs::copy(source, destination).context("failed to materialize paired replay executable")?;
    Ok(())
}

fn replay_schema_compiles(schema_path: &Path, schema: &[u8], output_path: &Path) -> Result<bool> {
    let schema_path = AbsoluteSchemaPath::parse(schema_path.to_path_buf())?;
    let output_path = AbsoluteOutputPath::parse(output_path.to_path_buf())?;
    match validate_artifact(StructuredArtifactObservation::new(
        &schema_path,
        FileObservation::Readable { bytes: schema },
        &output_path,
        FileObservation::Readable { bytes: b"{}" },
    )) {
        Ok(_) => Ok(true),
        Err(error) => Ok(error.outcome() != ArtifactOutcome::SchemaInvalid),
    }
}

fn observe_replay_artifact(
    schema_path: &Path,
    schema: &[u8],
    output_path: &Path,
) -> Result<ReplayArtifactObservation> {
    let bytes = match std::fs::read(output_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ReplayArtifactObservation::Missing);
        }
        Err(error) => {
            tracing::debug!(error = ?error, "replay artifact read failed");
            return Ok(ReplayArtifactObservation::Missing);
        }
    };
    if serde_json::from_slice::<Value>(&bytes).is_err() {
        return Ok(ReplayArtifactObservation::InvalidJson { bytes });
    }
    let typed_schema = AbsoluteSchemaPath::parse(schema_path.to_path_buf())?;
    let typed_output = AbsoluteOutputPath::parse(output_path.to_path_buf())?;
    match validate_artifact(StructuredArtifactObservation::new(
        &typed_schema,
        FileObservation::Readable { bytes: schema },
        &typed_output,
        FileObservation::Readable { bytes: &bytes },
    )) {
        Ok(_) => Ok(ReplayArtifactObservation::Conforming { bytes }),
        Err(error) if error.outcome() == ArtifactOutcome::SchemaViolating => {
            Ok(ReplayArtifactObservation::SchemaViolating { bytes })
        }
        Err(error) if error.outcome() == ArtifactOutcome::Truncated => {
            Ok(ReplayArtifactObservation::InvalidJson { bytes })
        }
        Err(error) => Err(Error::new(error).context("replay oracle schema became unusable")),
    }
}

fn cleanup_replay_parent(parent: &Path) -> Result<()> {
    std::fs::remove_dir_all(parent).map_err(|error| {
        tracing::debug!(error = ?error, path = %parent.display(), "isolated replay cleanup failed");
        anyhow!("gate replay cleanup failed")
    })
}

fn require_replay_overall_deadline(deadline: Instant) -> Result<()> {
    if Instant::now() >= deadline {
        bail!("gate replay exceeded its 40 minute overall deadline");
    }
    Ok(())
}

fn run_replay_git(cwd: &Path, arguments: &[&str], overall_deadline: Instant) -> Result<Output> {
    require_replay_overall_deadline(overall_deadline)?;
    run_replay_git_bounded(cwd, arguments, Some(overall_deadline))
}

fn run_replay_git_bounded(
    cwd: &Path,
    arguments: &[&str],
    overall_deadline: Option<Instant>,
) -> Result<Output> {
    let mut command = std::process::Command::new("git");
    command
        .args(arguments)
        .current_dir(cwd)
        .env_remove(REPLAY_PAUSE_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .context("failed to spawn clean replay git command")?;
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture clean replay git stdout"))?;
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("failed to capture clean replay git stderr"))?;
    let stdout = Arc::new(Mutex::new(Vec::new()));
    let stderr = Arc::new(Mutex::new(Vec::new()));
    let stdout_capture = Arc::clone(&stdout);
    let stderr_capture = Arc::clone(&stderr);
    let stdout_worker = std::thread::spawn(move || {
        drain_gate_pipe(&mut stdout_pipe, &stdout_capture, "clean replay git stdout")
    });
    let stderr_worker = std::thread::spawn(move || {
        drain_gate_pipe(&mut stderr_pipe, &stderr_capture, "clean replay git stderr")
    });
    let process_deadline = Instant::now() + REPLAY_GIT_TIMEOUT;
    let deadline = overall_deadline
        .map(|overall| process_deadline.min(overall))
        .unwrap_or(process_deadline);
    let status = loop {
        match child
            .try_wait()
            .context("failed to wait for clean replay git command")?
        {
            Some(status) => break status,
            None if Instant::now() < deadline => std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL),
            None => {
                let _kill = child.kill();
                let _reap = child.wait();
                drop(stdout_worker);
                drop(stderr_worker);
                if overall_deadline.is_some_and(|overall| Instant::now() >= overall) {
                    bail!("gate replay exceeded its 40 minute overall deadline");
                }
                bail!("clean replay git command timed out after 5 seconds");
            }
        }
    };
    while (!stdout_worker.is_finished() || !stderr_worker.is_finished())
        && Instant::now() < deadline
    {
        std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL);
    }
    if !stdout_worker.is_finished() || !stderr_worker.is_finished() {
        drop(stdout_worker);
        drop(stderr_worker);
        if overall_deadline.is_some_and(|overall| Instant::now() >= overall) {
            bail!("gate replay exceeded its 40 minute overall deadline");
        }
        bail!("clean replay git command timed out after 5 seconds");
    }
    let stdout_diagnostic = stdout_worker
        .join()
        .map_err(|_| anyhow!("clean replay git stdout worker panicked"))?;
    let stderr_diagnostic = stderr_worker
        .join()
        .map_err(|_| anyhow!("clean replay git stderr worker panicked"))?;
    if let Some(diagnostic) = stdout_diagnostic.or(stderr_diagnostic) {
        bail!(diagnostic);
    }
    Ok(Output {
        status,
        stdout: with_gate_buffer(&stdout, |bytes| bytes.clone()),
        stderr: with_gate_buffer(&stderr, |bytes| bytes.clone()),
    })
}

fn exit_gate_exec_rejection(diagnostic: &str) -> ! {
    let mut stderr = std::io::stderr().lock();
    let _result = writeln!(stderr, "{diagnostic}").and_then(|()| stderr.flush());
    std::process::exit(2)
}

struct GateRecorderRuntime {
    stop: Arc<AtomicBool>,
    state: Arc<Mutex<GateRecorderState>>,
    server: Option<std::thread::JoinHandle<()>>,
    socket_path: PathBuf,
}

#[derive(Default)]
struct GateRecorderState {
    records: Vec<(u32, GateExecutionRecord)>,
    fatal_diagnostics: Vec<String>,
    live_workers: usize,
    next_sequence: u32,
    #[cfg(test)]
    process_bounds: Option<GateProcessBounds>,
}

struct GateRecorderStop {
    records: Vec<GateExecutionRecord>,
    error: Option<Error>,
}

enum GateConnectionOutcome {
    Recorded,
    Rejected { diagnostic: String, fatal: bool },
}

struct GateStimulusExecution {
    observed_result: Option<GateObservedResult>,
    diagnostic: Option<String>,
}

struct GateProcessExecution {
    observation: Option<GateProcessObservation>,
    diagnostic: Option<String>,
}

#[derive(Clone, Copy)]
struct GateProcessBounds {
    execution_inactivity: Duration,
    execution_overall: Duration,
    output_drain_inactivity: Duration,
    output_drain_overall: Duration,
}

impl GateProcessBounds {
    const PRODUCTION: Self = Self {
        execution_inactivity: GATE_EXECUTION_INACTIVITY_TIMEOUT,
        execution_overall: GATE_EXECUTION_OVERALL_TIMEOUT,
        output_drain_inactivity: GATE_OUTPUT_DRAIN_INACTIVITY_TIMEOUT,
        output_drain_overall: GATE_OUTPUT_DRAIN_OVERALL_TIMEOUT,
    };

    const REPLAY: Self = Self {
        execution_inactivity: REPLAY_RUN_INACTIVITY_TIMEOUT,
        execution_overall: REPLAY_RUN_OVERALL_TIMEOUT,
        output_drain_inactivity: GATE_OUTPUT_DRAIN_INACTIVITY_TIMEOUT,
        output_drain_overall: GATE_OUTPUT_DRAIN_OVERALL_TIMEOUT,
    };
}

#[cfg(test)]
std::thread_local! {
    static GATE_PROCESS_TEST_BOUNDS: std::cell::Cell<Option<GateProcessBounds>> =
        const { std::cell::Cell::new(None) };
    static REPLAY_PROCESS_TEST_BOUNDS: std::cell::Cell<Option<GateProcessBounds>> =
        const { std::cell::Cell::new(None) };
}

fn gate_process_bounds() -> GateProcessBounds {
    #[cfg(test)]
    if let Some(bounds) = GATE_PROCESS_TEST_BOUNDS.with(std::cell::Cell::get) {
        return bounds;
    }
    GateProcessBounds::PRODUCTION
}

fn replay_process_bounds() -> GateProcessBounds {
    #[cfg(test)]
    if let Some(bounds) = REPLAY_PROCESS_TEST_BOUNDS.with(std::cell::Cell::get) {
        return bounds;
    }
    GateProcessBounds::REPLAY
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GateOutputDrainTimeout {
    Inactivity,
    Overall,
}

impl GateRecorderRuntime {
    fn start(config: &GateExecutionRecorderConfig) -> Result<Self> {
        if config.evidence().as_path().exists() {
            bail!("gate execution evidence path already exists");
        }
        let socket_path = config.socket().as_path();
        match std::fs::symlink_metadata(socket_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(
                    Error::new(error).context("failed to inspect gate execution socket path")
                );
            }
            Ok(metadata) => {
                use std::os::unix::fs::FileTypeExt;

                if !metadata.file_type().is_socket() {
                    bail!("gate execution socket path already exists");
                }
                let stale = match UnixStream::connect(socket_path) {
                    Ok(mut probe) => {
                        let mut byte = [0_u8; 1];
                        matches!(
                            probe
                                .set_read_timeout(Some(GATE_ACCEPT_POLL_INTERVAL))
                                .and_then(|()| probe.read(&mut byte)),
                            Ok(0)
                        )
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => true,
                    Err(error) => {
                        return Err(
                            Error::new(error).context("failed to probe gate execution socket path")
                        );
                    }
                };
                if !stale {
                    bail!("gate execution socket path already exists");
                }
                std::fs::remove_file(socket_path).with_context(|| {
                    format!(
                        "failed to remove stale gate execution socket {}",
                        socket_path.display()
                    )
                })?;
            }
        }
        let listener = UnixListener::bind(config.socket().as_path()).with_context(|| {
            format!(
                "failed to bind gate execution socket {}",
                config.socket().as_path().display()
            )
        })?;
        listener
            .set_nonblocking(true)
            .context("failed to make gate execution socket nonblocking")?;
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(Mutex::new(GateRecorderState {
            next_sequence: 1,
            ..GateRecorderState::default()
        }));
        let server_stop = Arc::clone(&stop);
        let server_state = Arc::clone(&state);
        let server = std::thread::spawn(move || {
            let mut workers = Vec::new();
            while !server_stop.load(AtomicOrdering::Acquire) {
                match listener.accept() {
                    Ok((stream, _address)) => {
                        let admitted = with_gate_state(&server_state, |state| {
                            if state.live_workers < GATE_MAX_CONNECTION_WORKERS {
                                state.live_workers += 1;
                                true
                            } else {
                                false
                            }
                        });
                        if admitted {
                            let worker_state = Arc::clone(&server_state);
                            let worker_stop = Arc::clone(&server_stop);
                            workers.push(std::thread::spawn(move || {
                                let accepted_at = Instant::now();
                                let outcome = std::panic::catch_unwind(|| {
                                    serve_gate_execution(
                                        stream,
                                        &worker_state,
                                        &worker_stop,
                                        accepted_at,
                                    )
                                })
                                .unwrap_or_else(|_| GateConnectionOutcome::Rejected {
                                    diagnostic: "gate execution connection worker panicked"
                                        .to_owned(),
                                    fatal: true,
                                });
                                consume_gate_connection_outcome(&worker_state, outcome);
                                with_gate_state(&worker_state, |state| {
                                    state.live_workers = state.live_workers.saturating_sub(1);
                                });
                            }));
                        } else {
                            let outcome =
                                reject_gate_connection(stream, gate_connection_limit_diagnostic());
                            consume_gate_connection_outcome(&server_state, outcome);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL);
                    }
                    Err(error) => {
                        retain_gate_fatal(
                            &server_state,
                            format!("failed to accept gate execution request: {error}"),
                        );
                        break;
                    }
                }
            }
            let shutdown_deadline = Instant::now() + GATE_SERVER_SHUTDOWN_TIMEOUT;
            for worker in workers {
                while !worker.is_finished() && Instant::now() < shutdown_deadline {
                    std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL);
                }
                if worker.is_finished() {
                    if worker.join().is_err() {
                        retain_gate_fatal(
                            &server_state,
                            "gate execution connection worker panicked".to_owned(),
                        );
                    }
                } else {
                    retain_gate_fatal(&server_state, gate_worker_shutdown_diagnostic());
                    drop(worker);
                }
            }
        });
        Ok(Self {
            stop,
            state,
            server: Some(server),
            socket_path: config.socket().as_path().to_path_buf(),
        })
    }

    #[cfg(test)]
    fn start_with_bounds(
        config: &GateExecutionRecorderConfig,
        bounds: GateProcessBounds,
    ) -> Result<Self> {
        let runtime = Self::start(config)?;
        with_gate_state(&runtime.state, |state| state.process_bounds = Some(bounds));
        Ok(runtime)
    }

    fn stop(mut self) -> GateRecorderStop {
        self.stop.store(true, AtomicOrdering::Release);
        let mut error = self.stop_server_bounded();
        let (mut records, fatal_diagnostics) = with_gate_state(&self.state, |state| {
            (state.records.clone(), state.fatal_diagnostics.clone())
        });
        records.sort_by_key(|(sequence, _record)| *sequence);
        let records = records
            .into_iter()
            .map(|(_sequence, record)| record)
            .collect();
        if error.is_none() && !fatal_diagnostics.is_empty() {
            error = Some(anyhow!(fatal_diagnostics.join("; ")));
        }
        let remove_result = std::fs::remove_file(&self.socket_path).with_context(|| {
            format!(
                "failed to remove gate execution socket {}",
                self.socket_path.display()
            )
        });
        if let Err(remove_error) = remove_result
            && error.is_none()
        {
            error = Some(remove_error);
        }
        GateRecorderStop { records, error }
    }

    fn stop_server_bounded(&mut self) -> Option<Error> {
        self.stop.store(true, AtomicOrdering::Release);
        let deadline = Instant::now() + GATE_SERVER_SHUTDOWN_TIMEOUT;
        while self
            .server
            .as_ref()
            .is_some_and(|server| !server.is_finished())
            && Instant::now() < deadline
        {
            std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL);
        }
        let server = self.server.take()?;
        if !server.is_finished() {
            drop(server);
            return Some(anyhow!(gate_worker_shutdown_diagnostic()));
        }
        match server.join() {
            Ok(()) => None,
            Err(_) => Some(anyhow!("gate execution server thread panicked")),
        }
    }
}

impl Drop for GateRecorderRuntime {
    fn drop(&mut self) {
        let _server_error = self.stop_server_bounded();
        if self.socket_path.exists() {
            let _remove_result = std::fs::remove_file(&self.socket_path);
        }
    }
}

fn serve_gate_execution(
    mut stream: UnixStream,
    state: &Arc<Mutex<GateRecorderState>>,
    stop: &Arc<AtomicBool>,
    accepted_at: Instant,
) -> GateConnectionOutcome {
    if let Err(error) = stream.set_nonblocking(false) {
        return reject_gate_connection(
            stream,
            format!("failed to make gate execution connection blocking: {error}"),
        );
    }
    if let Err(error) = stream.set_write_timeout(Some(GATE_RESPONSE_WRITE_TIMEOUT)) {
        if error.kind() != std::io::ErrorKind::InvalidInput {
            return GateConnectionOutcome::Rejected {
                diagnostic: format!("failed to bound gate execution response write: {error}"),
                fatal: true,
            };
        }
        tracing::debug!(error = ?error, "gate execution peer closed before response bound was set");
    }
    let read_deadline = accepted_at + GATE_REQUEST_READ_TIMEOUT;
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let remaining = read_deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return deliver_gate_rejection(&mut stream, gate_request_timeout_diagnostic());
        }
        if let Err(error) = stream.set_read_timeout(Some(remaining)) {
            if error.kind() != std::io::ErrorKind::InvalidInput {
                return deliver_gate_rejection(
                    &mut stream,
                    format!("failed to bound gate execution request read: {error}"),
                );
            }
            tracing::debug!(error = ?error, "gate execution peer closed before request bound was set");
        }
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => {
                bytes.extend_from_slice(&chunk[..count]);
                if bytes.len() > GATE_MAX_REQUEST_BYTES {
                    return deliver_gate_rejection(
                        &mut stream,
                        gate_request_too_large_diagnostic(),
                    );
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                return deliver_gate_rejection(&mut stream, gate_request_timeout_diagnostic());
            }
            Err(error) => {
                return deliver_gate_rejection(
                    &mut stream,
                    format!("failed to read gate execution request: {error}"),
                );
            }
        }
    }
    if stop.load(AtomicOrdering::Acquire) {
        return deliver_gate_rejection(&mut stream, gate_recorder_stopping_diagnostic());
    }
    let stimulus = match parse_gate_stimulus(&bytes) {
        Ok(stimulus) => stimulus,
        Err(error) => {
            return deliver_gate_rejection(&mut stream, error.to_string());
        }
    };
    let (sequence, execution_ref) = match allocate_gate_execution_ref(state) {
        Ok(allocated) => allocated,
        Err(diagnostic) => return deliver_gate_rejection(&mut stream, diagnostic),
    };
    #[cfg(test)]
    let execution = with_gate_state(state, |state| state.process_bounds).map_or_else(
        || execute_gate_stimulus(&stimulus, stop),
        |bounds| execute_gate_stimulus_with_bounds(&stimulus, None, bounds, Some(stop)),
    );
    #[cfg(not(test))]
    let execution = execute_gate_stimulus(&stimulus, stop);
    let Some(observed_result) = execution.observed_result else {
        let diagnostic = execution
            .diagnostic
            .unwrap_or_else(|| "gate execution ended without an observation".to_owned());
        retain_gate_fatal(state, diagnostic.clone());
        return deliver_gate_rejection(&mut stream, diagnostic);
    };
    let record = GateExecutionRecord {
        execution_ref: execution_ref.clone(),
        stimulus,
        observed_result: observed_result.clone(),
    };
    with_gate_state(state, |state| state.records.push((sequence, record)));
    if let Some(diagnostic) = execution.diagnostic {
        retain_gate_fatal(state, diagnostic.clone());
        return deliver_gate_rejection(&mut stream, diagnostic);
    }
    let response = GateExecutionResponse {
        execution_ref,
        observed_result,
    };
    let wire = match serde_json::to_vec(&response) {
        Ok(wire) => wire,
        Err(error) => {
            let diagnostic = format!("failed to serialize gate execution response: {error}");
            retain_gate_fatal(state, diagnostic);
            return GateConnectionOutcome::Recorded;
        }
    };
    if let Err(error) = stream.write_all(&wire) {
        tracing::warn!(error = ?error, "failed to deliver retained gate execution response");
    }
    GateConnectionOutcome::Recorded
}

fn execute_gate_stimulus(stimulus: &GateStimulus, stop: &Arc<AtomicBool>) -> GateStimulusExecution {
    execute_gate_stimulus_with_bounds(stimulus, None, gate_process_bounds(), Some(stop))
}

fn execute_replay_stimulus_until(
    stimulus: &GateStimulus,
    overall_deadline: Instant,
) -> GateStimulusExecution {
    execute_gate_stimulus_with_bounds(
        stimulus,
        Some(overall_deadline),
        replay_process_bounds(),
        None,
    )
}

fn execute_gate_stimulus_with_bounds(
    stimulus: &GateStimulus,
    overall_deadline: Option<Instant>,
    bounds: GateProcessBounds,
    stop: Option<&AtomicBool>,
) -> GateStimulusExecution {
    let mut setup = Vec::with_capacity(stimulus.setup().len());
    for action in stimulus.setup() {
        if overall_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return GateStimulusExecution {
                observed_result: None,
                diagnostic: Some("gate replay exceeded its 40 minute overall deadline".to_owned()),
            };
        }
        let execution = execute_gate_process_with_bounds(
            stimulus.working_directory(),
            action,
            overall_deadline,
            bounds,
            stop,
        );
        let Some(observation) = execution.observation else {
            return GateStimulusExecution {
                observed_result: None,
                diagnostic: execution.diagnostic,
            };
        };
        let succeeded = matches!(observation.status, GateTerminalStatus::Exited { code: 0 });
        setup.push(observation);
        if !succeeded || execution.diagnostic.is_some() {
            return GateStimulusExecution {
                observed_result: Some(GateObservedResult {
                    setup,
                    command: None,
                }),
                diagnostic: execution.diagnostic,
            };
        }
    }
    if overall_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return GateStimulusExecution {
            observed_result: None,
            diagnostic: Some("gate replay exceeded its 40 minute overall deadline".to_owned()),
        };
    }
    let execution = execute_gate_process_with_bounds(
        stimulus.working_directory(),
        stimulus.command(),
        overall_deadline,
        bounds,
        stop,
    );
    GateStimulusExecution {
        observed_result: execution.observation.map(|command| GateObservedResult {
            setup,
            command: Some(command),
        }),
        diagnostic: execution.diagnostic,
    }
}

#[cfg(test)]
fn execute_gate_process_until(
    working_directory: &Path,
    stimulus: &GateProcessStimulus,
    overall_deadline: Option<Instant>,
) -> GateProcessExecution {
    execute_gate_process_with_bounds(
        working_directory,
        stimulus,
        overall_deadline,
        gate_process_bounds(),
        None,
    )
}

fn execute_gate_process_with_bounds(
    working_directory: &Path,
    stimulus: &GateProcessStimulus,
    overall_deadline: Option<Instant>,
    bounds: GateProcessBounds,
    stop: Option<&AtomicBool>,
) -> GateProcessExecution {
    let mut command = std::process::Command::new(stimulus.program());
    command.args(stimulus.arguments());
    command.current_dir(working_directory);
    command.env_clear();
    command.envs(stimulus.environment());
    let temporary_directory = match SynthesizedTemporaryDirectory::create() {
        Ok(directory) => directory,
        Err(error) => {
            return GateProcessExecution {
                observation: Some(GateProcessObservation {
                    status: GateTerminalStatus::SpawnFailed {
                        detail: format!("failed to create binary-owned TMPDIR: {error:#}"),
                    },
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
                diagnostic: None,
            };
        }
    };
    command.env("TMPDIR", temporary_directory.path());
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.process_group(0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return GateProcessExecution {
                observation: Some(GateProcessObservation {
                    status: GateTerminalStatus::SpawnFailed {
                        detail: error.to_string(),
                    },
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }),
                diagnostic: None,
            };
        }
    };
    let Some(mut child_stdin) = child.stdin.take() else {
        terminate_unobserved_gate_child(&mut child);
        return failed_gate_process("gate execution child stdin was not piped");
    };
    let Some(mut child_stdout) = child.stdout.take() else {
        terminate_unobserved_gate_child(&mut child);
        return failed_gate_process("gate execution child stdout was not piped");
    };
    let Some(mut child_stderr) = child.stderr.take() else {
        terminate_unobserved_gate_child(&mut child);
        return failed_gate_process("gate execution child stderr was not piped");
    };
    let input = stimulus.input().to_vec();
    let stdout = Arc::new(Mutex::new(Vec::new()));
    let stderr = Arc::new(Mutex::new(Vec::new()));
    let output_progress = Arc::new(AtomicU64::new(0));
    let stdout_capture = Arc::clone(&stdout);
    let stderr_capture = Arc::clone(&stderr);
    let stdout_progress = Arc::clone(&output_progress);
    let stderr_progress = Arc::clone(&output_progress);
    let stdin_worker = std::thread::spawn(move || match child_stdin.write_all(&input) {
        Ok(()) => None,
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => None,
        Err(error) => Some(format!("gate execution stdin write failed: {error}")),
    });
    let stdout_worker = std::thread::spawn(move || {
        drain_gate_pipe_with_progress(
            &mut child_stdout,
            &stdout_capture,
            "stdout",
            &stdout_progress,
        )
    });
    let stderr_worker = std::thread::spawn(move || {
        drain_gate_pipe_with_progress(
            &mut child_stderr,
            &stderr_capture,
            "stderr",
            &stderr_progress,
        )
    });
    let execution_started = Instant::now();
    let execution_overall_deadline = overall_deadline
        .map(|deadline| deadline.min(execution_started + bounds.execution_overall))
        .unwrap_or_else(|| execution_started + bounds.execution_overall);
    let mut execution_inactivity_deadline = execution_started + bounds.execution_inactivity;
    let mut observed_progress = output_progress.load(AtomicOrdering::Acquire);
    let mut terminal_status = None;
    let mut diagnostic = None;
    while Instant::now() < execution_overall_deadline
        && Instant::now() < execution_inactivity_deadline
        && !stop.is_some_and(|flag| flag.load(AtomicOrdering::Acquire))
    {
        match child.try_wait() {
            Ok(Some(status)) => {
                terminal_status = Some(status);
                break;
            }
            Ok(None) => std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL),
            Err(error) => {
                diagnostic = Some(format!("gate execution process wait failed: {error}"));
                break;
            }
        }
        let current_progress = output_progress.load(AtomicOrdering::Acquire);
        if current_progress != observed_progress {
            observed_progress = current_progress;
            execution_inactivity_deadline =
                (Instant::now() + bounds.execution_inactivity).min(execution_overall_deadline);
        }
    }
    if terminal_status.is_none() && diagnostic.is_none() {
        if let Err(error) = kill_gate_process_group(&mut child) {
            diagnostic = Some(format!("gate execution process kill failed: {error}"));
        } else {
            let termination_deadline = Instant::now() + GATE_PROCESS_TERMINATION_TIMEOUT;
            while Instant::now() < termination_deadline {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        terminal_status = Some(status);
                        break;
                    }
                    Ok(None) => std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL),
                    Err(error) => {
                        diagnostic = Some(format!("gate execution process wait failed: {error}"));
                        break;
                    }
                }
            }
            if terminal_status.is_none() && diagnostic.is_none() {
                diagnostic = Some(gate_process_termination_diagnostic());
            }
        }
    }
    if terminal_status.is_none() {
        drop(stdin_worker);
        drop(stdout_worker);
        drop(stderr_worker);
        return GateProcessExecution {
            observation: None,
            diagnostic,
        };
    }
    let drain_started = Instant::now();
    let drain_overall_deadline = drain_started + bounds.output_drain_overall;
    let mut drain_inactivity_deadline = drain_started + bounds.output_drain_inactivity;
    observed_progress = output_progress.load(AtomicOrdering::Acquire);
    let mut drain_timeout = None;
    while !stdout_worker.is_finished()
        || !stderr_worker.is_finished()
        || !stdin_worker.is_finished()
    {
        let now = Instant::now();
        if now >= drain_overall_deadline {
            drain_timeout = Some(GateOutputDrainTimeout::Overall);
            break;
        }
        let current_progress = output_progress.load(AtomicOrdering::Acquire);
        if current_progress != observed_progress {
            observed_progress = current_progress;
            drain_inactivity_deadline =
                (now + bounds.output_drain_inactivity).min(drain_overall_deadline);
        } else if now >= drain_inactivity_deadline {
            drain_timeout = Some(GateOutputDrainTimeout::Inactivity);
            break;
        }
        std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL);
    }
    if (!stdout_worker.is_finished() || !stderr_worker.is_finished()) && diagnostic.is_none() {
        let _group_kill = kill_gate_process_group(&mut child);
        diagnostic = Some(gate_output_drain_diagnostic(
            drain_timeout.unwrap_or(GateOutputDrainTimeout::Inactivity),
        ));
    }
    let drain_deadline = if drain_timeout.is_some() {
        Instant::now() + GATE_PROCESS_TERMINATION_TIMEOUT
    } else {
        drain_overall_deadline.min(drain_inactivity_deadline)
    };
    let stdin_diagnostic = join_gate_worker(
        stdin_worker,
        "gate execution stdin writer panicked",
        drain_deadline,
    );
    let stdout_diagnostic = join_gate_worker(
        stdout_worker,
        "gate execution stdout drainer panicked",
        drain_deadline,
    );
    let stderr_diagnostic = join_gate_worker(
        stderr_worker,
        "gate execution stderr drainer panicked",
        drain_deadline,
    );
    diagnostic = diagnostic
        .or(stdin_diagnostic)
        .or(stdout_diagnostic)
        .or(stderr_diagnostic);
    let Some(status) = terminal_status else {
        return GateProcessExecution {
            observation: None,
            diagnostic: Some("gate execution process had no exit code or signal".to_owned()),
        };
    };
    let status = match (status.code(), status.signal()) {
        (Some(code), _) => GateTerminalStatus::Exited { code },
        (None, Some(signal)) => GateTerminalStatus::Signaled { signal },
        (None, None) => {
            return GateProcessExecution {
                observation: None,
                diagnostic: Some("gate execution process had no exit code or signal".to_owned()),
            };
        }
    };
    GateProcessExecution {
        observation: Some(GateProcessObservation {
            status,
            stdout: with_gate_buffer(&stdout, |bytes| bytes.clone()),
            stderr: with_gate_buffer(&stderr, |bytes| bytes.clone()),
        }),
        diagnostic,
    }
}

fn failed_gate_process(diagnostic: &str) -> GateProcessExecution {
    GateProcessExecution {
        observation: None,
        diagnostic: Some(diagnostic.to_owned()),
    }
}

fn terminate_unobserved_gate_child(child: &mut std::process::Child) {
    let _kill_result = kill_gate_process_group(child);
    let deadline = Instant::now() + GATE_PROCESS_TERMINATION_TIMEOUT;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break,
            Ok(None) => std::thread::sleep(GATE_ACCEPT_POLL_INTERVAL),
        }
    }
}

fn kill_gate_process_group(child: &mut std::process::Child) -> std::io::Result<()> {
    let group = format!("-{}", child.id());
    let status = std::process::Command::new("/bin/kill")
        .args(["-KILL", &group])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        child.kill()
    }
}

fn drain_gate_pipe(
    pipe: &mut dyn Read,
    capture: &Arc<Mutex<Vec<u8>>>,
    name: &str,
) -> Option<String> {
    let progress = AtomicU64::new(0);
    drain_gate_pipe_with_progress(pipe, capture, name, &progress)
}

fn drain_gate_pipe_with_progress(
    pipe: &mut dyn Read,
    capture: &Arc<Mutex<Vec<u8>>>,
    name: &str,
    progress: &AtomicU64,
) -> Option<String> {
    let mut buffer = [0_u8; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => return None,
            Ok(count) => with_gate_buffer(capture, |bytes| {
                bytes.extend_from_slice(&buffer[..count]);
                progress.fetch_add(1, AtomicOrdering::Release);
            }),
            Err(error) => {
                return Some(format!("gate execution {name} read failed: {error}"));
            }
        }
    }
}

fn join_gate_worker(
    worker: std::thread::JoinHandle<Option<String>>,
    panic_diagnostic: &str,
    deadline: Instant,
) -> Option<String> {
    if !worker.is_finished() || Instant::now() > deadline {
        drop(worker);
        return None;
    }
    match worker.join() {
        Ok(diagnostic) => diagnostic,
        Err(_) => Some(panic_diagnostic.to_owned()),
    }
}

fn allocate_gate_execution_ref(
    state: &Arc<Mutex<GateRecorderState>>,
) -> std::result::Result<(u32, GateExecutionRef), String> {
    with_gate_state(state, |state| {
        let sequence = state.next_sequence;
        let reference =
            GateExecutionRef::from_sequence(sequence).map_err(|error| error.to_string())?;
        state.next_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| "gate execution sequence exceeds u32".to_owned())?;
        Ok((sequence, reference))
    })
}

fn reject_gate_connection(mut stream: UnixStream, diagnostic: String) -> GateConnectionOutcome {
    if let Err(error) = stream.set_nonblocking(false) {
        return GateConnectionOutcome::Rejected {
            diagnostic: format!("failed to make gate execution connection blocking: {error}"),
            fatal: true,
        };
    }
    if let Err(error) = stream.set_write_timeout(Some(GATE_RESPONSE_WRITE_TIMEOUT)) {
        return GateConnectionOutcome::Rejected {
            diagnostic: format!("failed to bound gate execution response write: {error}"),
            fatal: true,
        };
    }
    deliver_gate_rejection(&mut stream, diagnostic)
}

fn deliver_gate_rejection(stream: &mut UnixStream, diagnostic: String) -> GateConnectionOutcome {
    let rejection = GateExecutionRejection {
        error: diagnostic.clone(),
    };
    let wire = match serde_json::to_vec(&rejection) {
        Ok(wire) => wire,
        Err(error) => {
            return GateConnectionOutcome::Rejected {
                diagnostic: format!("failed to serialize gate execution rejection: {error}"),
                fatal: true,
            };
        }
    };
    match stream.write_all(&wire) {
        Ok(()) => GateConnectionOutcome::Rejected {
            diagnostic,
            fatal: false,
        },
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            ) =>
        {
            tracing::warn!(error = ?error, diagnostic, "gate execution client departed before rejection delivery");
            GateConnectionOutcome::Rejected {
                diagnostic,
                fatal: false,
            }
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            GateConnectionOutcome::Rejected {
                diagnostic: gate_response_timeout_diagnostic(),
                fatal: true,
            }
        }
        Err(error) => GateConnectionOutcome::Rejected {
            diagnostic: format!("failed to write gate execution response: {error}"),
            fatal: true,
        },
    }
}

fn consume_gate_connection_outcome(
    state: &Arc<Mutex<GateRecorderState>>,
    outcome: GateConnectionOutcome,
) {
    if let GateConnectionOutcome::Rejected {
        diagnostic,
        fatal: true,
    } = outcome
    {
        retain_gate_fatal(state, diagnostic);
    }
}

fn retain_gate_fatal(state: &Arc<Mutex<GateRecorderState>>, diagnostic: String) {
    with_gate_state(state, |state| state.fatal_diagnostics.push(diagnostic));
}

fn with_gate_state<T>(
    state: &Arc<Mutex<GateRecorderState>>,
    operation: impl FnOnce(&mut GateRecorderState) -> T,
) -> T {
    match state.lock() {
        Ok(mut state) => operation(&mut state),
        Err(poisoned) => operation(&mut poisoned.into_inner()),
    }
}

fn with_gate_buffer<T>(
    buffer: &Arc<Mutex<Vec<u8>>>,
    operation: impl FnOnce(&mut Vec<u8>) -> T,
) -> T {
    match buffer.lock() {
        Ok(mut buffer) => operation(&mut buffer),
        Err(poisoned) => operation(&mut poisoned.into_inner()),
    }
}

fn gate_request_timeout_diagnostic() -> String {
    "gate execution request read timed out after 2 seconds".to_owned()
}

fn gate_request_too_large_diagnostic() -> String {
    "gate execution request exceeds 16777216 bytes".to_owned()
}

fn gate_response_timeout_diagnostic() -> String {
    "gate execution response write timed out after 1 second".to_owned()
}

fn gate_process_termination_diagnostic() -> String {
    "gate execution process did not terminate within 1 second after kill".to_owned()
}

fn gate_output_drain_diagnostic(timeout: GateOutputDrainTimeout) -> String {
    match timeout {
        GateOutputDrainTimeout::Inactivity => {
            "gate execution output drain inactive for 30 seconds".to_owned()
        }
        GateOutputDrainTimeout::Overall => {
            "gate execution output drain exceeded 2 minute overall cap".to_owned()
        }
    }
}

fn gate_recorder_stopping_diagnostic() -> String {
    "gate execution recorder is stopping".to_owned()
}

fn gate_worker_shutdown_diagnostic() -> String {
    "gate execution worker exceeded 12 second shutdown deadline".to_owned()
}

fn gate_connection_limit_diagnostic() -> String {
    "gate execution recorder connection limit reached".to_owned()
}

fn persist_gate_execution_evidence(
    path: &AbsoluteGateExecutionEvidencePath,
    records: Vec<GateExecutionRecord>,
) -> Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path.as_path())
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                anyhow!("gate execution evidence path already exists")
            } else {
                Error::new(error).context(format!(
                    "failed to create gate execution evidence {}",
                    path.as_path().display()
                ))
            }
        })?;
    let mut writer = std::io::BufWriter::new(file);
    serde_json::to_writer(&mut writer, &GateExecutionEvidence::new(records))
        .context("failed to serialize gate execution evidence")?;
    writer
        .write_all(b"\n")
        .context("failed to terminate gate execution evidence")?;
    writer
        .flush()
        .context("failed to flush gate execution evidence")?;
    drop(writer);
    std::fs::set_permissions(path.as_path(), std::fs::Permissions::from_mode(0o444))
        .context("failed to make gate execution evidence read-only")
}

fn dispatch_identity_directory(log_path: &Path) -> PathBuf {
    let mut directory = log_path.as_os_str().to_os_string();
    directory.push(".dispatches");
    PathBuf::from(directory)
}

#[cfg(target_os = "macos")]
fn observe_darwin_process_number(
    process_number: ProcessNumber,
) -> Result<ProcessIdentityObservation> {
    let process_number_value = process_number.get();
    let pid = i32::try_from(process_number_value)
        .context("dispatch process number exceeds Darwin pid_t")?;
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let expected_size = std::mem::size_of::<libc::proc_bsdinfo>();
    let expected_size_i32 = i32::try_from(expected_size)
        .context("Darwin proc_bsdinfo size exceeds proc_pidinfo input range")?;
    let observed_size = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            expected_size_i32,
        )
    };
    if observed_size == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(ProcessIdentityObservation::Absent);
        }
        if error.raw_os_error() == Some(libc::EPERM) {
            return Ok(ProcessIdentityObservation::ForeignPresent);
        }
        bail!(
            "failed to observe Darwin process identity for PID {process_number_value}: proc_pidinfo returned 0 bytes: {error}"
        );
    }
    if observed_size != expected_size_i32 || info.pbi_pid != process_number_value {
        bail!(
            "failed to observe Darwin process identity for PID {process_number_value}: proc_pidinfo returned {observed_size} bytes with PID {}; expected {expected_size_i32} bytes and PID {process_number_value}: {}",
            info.pbi_pid,
            std::io::Error::last_os_error()
        );
    }
    let seconds_since_unix_epoch = info.pbi_start_tvsec;
    let microseconds = u32::try_from(info.pbi_start_tvusec)
        .context("Darwin process start microseconds exceed u32")?;
    let process_start_identity = ProcessStartIdentity::new(seconds_since_unix_epoch, microseconds)?;
    Ok(ProcessIdentityObservation::Present(process_start_identity))
}

#[cfg(not(target_os = "macos"))]
fn observe_darwin_process_number(
    _process_number: ProcessNumber,
) -> Result<ProcessIdentityObservation> {
    bail!("dispatch process identity observation is unsupported outside macOS/Darwin")
}

fn observe_darwin_process_identity(
    child: &std::process::Child,
) -> Result<(ProcessNumber, ProcessStartIdentity)> {
    let child_id = child.id();
    let process_number = ProcessNumber::new(child_id)?;
    let observation = observe_darwin_process_number(process_number)?;
    let process_start_identity = match observation {
        ProcessIdentityObservation::Present(identity) => identity,
        ProcessIdentityObservation::Absent => {
            bail!(
                "failed to observe Darwin process identity for child PID {child_id}: process is absent"
            )
        }
        ProcessIdentityObservation::ForeignPresent => {
            bail!(
                "failed to observe Darwin process identity for child PID {child_id}: process identity is unreadable"
            )
        }
    };
    Ok((process_number, process_start_identity))
}

fn ensure_dispatch_identity_directory(path: &Path) -> Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to create dispatch process identity directory {}",
                    path.display()
                )
            });
        }
    }
    let metadata = fs::symlink_metadata(path).with_context(|| {
        format!(
            "failed to inspect dispatch process identity directory {}",
            path.display()
        )
    })?;
    if !metadata.file_type().is_dir() {
        bail!(
            "dispatch process identity directory is not a real directory: {}",
            path.display()
        );
    }
    #[cfg(target_os = "macos")]
    if metadata.uid() != unsafe { libc::geteuid() } {
        bail!(
            "dispatch process identity directory is not owned by the current user: {}",
            path.display()
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn rename_dispatch_identity_exclusive(temporary: &Path, final_path: &Path) -> Result<()> {
    let temporary_c = CString::new(temporary.as_os_str().as_bytes())
        .context("dispatch process identity temporary path contains an embedded NUL")?;
    let final_c = CString::new(final_path.as_os_str().as_bytes())
        .context("dispatch process identity final path contains an embedded NUL")?;
    let result =
        unsafe { libc::renamex_np(temporary_c.as_ptr(), final_c.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        bail!("dispatch process identity sidecar already exists");
    }
    Err(error).with_context(|| {
        format!(
            "failed to publish dispatch process identity sidecar {}",
            final_path.display()
        )
    })
}

#[cfg(not(target_os = "macos"))]
fn rename_dispatch_identity_exclusive(_temporary: &Path, _final_path: &Path) -> Result<()> {
    bail!("exclusive dispatch process identity publication is unsupported outside macOS/Darwin")
}

fn persist_dispatch_process_identity(
    log_path: &Path,
    identity: &DispatchProcessIdentity,
) -> Result<()> {
    let directory = dispatch_identity_directory(log_path);
    ensure_dispatch_identity_directory(&directory)?;
    let sequence = identity.issuance_sequence().get();
    let final_path = directory.join(format!("{sequence}.json"));
    let temporary = directory.join(format!(".{sequence}.{}.tmp", std::process::id()));
    let mut temporary_created = false;
    let publication = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc_o_nofollow())
            .open(&temporary)
            .with_context(|| {
                format!(
                    "failed to create dispatch process identity temporary file {}",
                    temporary.display()
                )
            })?;
        temporary_created = true;
        let bytes = serialize_dispatch_process_identity(identity)?;
        file.write_all(&bytes)
            .context("failed to write dispatch process identity temporary file")?;
        file.flush()
            .context("failed to flush dispatch process identity temporary file")?;
        file.sync_all()
            .context("failed to sync dispatch process identity temporary file")?;
        file.set_permissions(fs::Permissions::from_mode(0o444))
            .context("failed to make dispatch process identity sidecar immutable")?;
        drop(file);
        rename_dispatch_identity_exclusive(&temporary, &final_path)?;
        File::open(&directory)
            .with_context(|| {
                format!(
                    "failed to open dispatch process identity directory {} for sync",
                    directory.display()
                )
            })?
            .sync_all()
            .context("failed to sync dispatch process identity directory")?;
        Ok(())
    })();
    if publication.is_err() && temporary_created {
        match fs::remove_file(&temporary) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => tracing::error!(
                error = ?error,
                path = %temporary.display(),
                "failed to remove dispatch process identity temporary file"
            ),
        }
    }
    publication
}

#[cfg(target_os = "macos")]
const fn libc_o_nofollow() -> i32 {
    libc::O_NOFOLLOW
}

#[cfg(not(target_os = "macos"))]
const fn libc_o_nofollow() -> i32 {
    0
}

fn cleanup_failed_dispatch_setup(
    child: &mut std::process::Child,
    recorder: Option<GateRecorderRuntime>,
    envelope: &DispatchEnvelope,
    completion: Option<&LiveDispatchCompletion<'_>>,
    started: Instant,
    primary: Error,
) -> Error {
    let mut cleanup_errors = Vec::new();
    if let Err(error) = child.kill() {
        cleanup_errors.push(format!("failed to kill just-spawned child: {error}"));
    }
    let status = match child.wait() {
        Ok(status) => Some(status),
        Err(error) => {
            cleanup_errors.push(format!("failed to reap just-spawned child: {error}"));
            None
        }
    };
    let recorder_stop = recorder.map(GateRecorderRuntime::stop);
    let gate_execution_records = recorder_stop
        .as_ref()
        .map_or_else(Vec::new, |stopped| stopped.records.clone());
    if let Some(error) = recorder_stop.and_then(|stopped| stopped.error) {
        cleanup_errors.push(format!("failed to stop gate recorder: {error:#}"));
    }
    if let Some(config) = envelope.gate_execution_recorder()
        && let Err(error) =
            persist_gate_execution_evidence(config.evidence(), gate_execution_records)
    {
        cleanup_errors.push(format!(
            "failed to persist gate execution evidence: {error:#}"
        ));
    }
    if let (Some(status), Some(completion)) = (status, completion) {
        let completion_result = (|| -> Result<()> {
            let duration_ms = u64::try_from(started.elapsed().as_millis())
                .context("dispatch duration in milliseconds exceeds u64")?;
            let payload = dispatch_completion_payload(
                completion.issuance_sequence,
                DispatchDuration::new(duration_ms),
                DispatchTokenUsage::Absent {
                    reason: UsageAbsenceReason::NoTerminalTurn,
                },
                dispatch_exit_status(status)?,
                ArtifactOutcome::NotValidated,
                observe_required_artifact_presence(completion.required_artifact_path.as_path())?,
            );
            close_dispatch_conditionally(
                completion.path,
                DispatchClosureTarget {
                    issuance_sequence: completion.issuance_sequence,
                    node: completion.node.clone(),
                },
                |_| Ok(payload),
            )?;
            Ok(())
        })();
        if let Err(error) = completion_result {
            cleanup_errors.push(format!("failed to append dispatch completion: {error:#}"));
        }
    }
    if cleanup_errors.is_empty() {
        primary
    } else {
        let detail = cleanup_errors.join("; ");
        tracing::error!(cleanup = %detail, "dispatch identity setup cleanup was incomplete");
        primary.context(format!(
            "additionally, dispatch setup cleanup failed: {detail}"
        ))
    }
}

fn start_logged_dispatch(envelope: &DispatchEnvelope, logging: LiveDispatchLog<'_>) -> Result<()> {
    let payload = dispatch_payload(logging.metadata);
    let issuance = admit_and_append_dispatch(logging.path, logging.metadata, payload)?;
    let request = DispatchContinuationRequest {
        envelope: ContinuationEnvelope::from(envelope),
        completion: ContinuationCompletion {
            log_path: logging.path.to_path_buf(),
            node: logging.metadata.node.clone(),
            issuance_sequence: issuance.sequence(),
            required_artifact_path: logging
                .metadata
                .required_artifact_path
                .as_path()
                .to_path_buf(),
        },
    };
    let suffix = format!("dispatch-{:06}", issuance.sequence().get());
    let stdout_path = PathBuf::from(format!("{}.{}.stdout", logging.path.display(), suffix));
    let stderr_path = PathBuf::from(format!("{}.{}.stderr", logging.path.display(), suffix));
    launch_dispatch_continuation(&request, &stdout_path, &stderr_path)
}

impl From<&DispatchEnvelope> for ContinuationEnvelope {
    fn from(envelope: &DispatchEnvelope) -> Self {
        let stdin = match envelope.stdin() {
            StdinBinding::Null => ContinuationStdin::Null,
            StdinBinding::PlanBytes(bytes) => ContinuationStdin::PlanBytes {
                bytes: bytes.clone(),
            },
        };
        let gate_execution_recorder =
            envelope
                .gate_execution_recorder()
                .map(|config| ContinuationGateRecorder {
                    client_path: config.client().as_path().to_path_buf(),
                    evidence_path: config.evidence().as_path().to_path_buf(),
                    socket_path: config.socket().as_path().to_path_buf(),
                });
        Self {
            target: envelope.target(),
            arguments: envelope.arguments().as_slice().to_vec(),
            working_directory: envelope.working_directory().as_path().to_path_buf(),
            environment: envelope
                .environment()
                .iter()
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
            stdin,
            sandbox: envelope
                .sandbox()
                .map(|sandbox| sandbox.as_str().to_owned()),
            schema_path: envelope
                .schema_path()
                .map(|path| path.as_path().to_path_buf()),
            output_path: envelope
                .output_path()
                .map(|path| path.as_path().to_path_buf()),
            gate_execution_recorder,
        }
    }
}

fn launch_dispatch_continuation(
    request: &DispatchContinuationRequest,
    stdout_path: &Path,
    stderr_path: &Path,
) -> Result<()> {
    let stdout = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(stdout_path)
        .with_context(|| format!("failed to create `{}`", stdout_path.display()))?;
    let stderr = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(stderr_path)
        .with_context(|| format!("failed to create `{}`", stderr_path.display()))?;
    let executable = std::env::current_exe().context("failed to resolve current pce executable")?;
    let mut command = std::process::Command::new(&executable);
    command.arg("__dispatch-continuation");
    command.stdin(Stdio::piped());
    command.stdout(stdout);
    command.stderr(stderr);
    command.process_group(0);
    let mut continuation = command.spawn().with_context(|| {
        format!(
            "failed to spawn dispatch continuation `{}`",
            executable.display()
        )
    })?;
    let request_bytes =
        serde_json::to_vec(request).context("failed to serialize dispatch continuation request")?;
    let mut stdin = continuation
        .stdin
        .take()
        .ok_or_else(|| anyhow!("dispatch continuation stdin was not piped"))?;
    stdin
        .write_all(&request_bytes)
        .context("failed to write dispatch continuation request")?;
    drop(stdin);
    Ok(())
}

fn run_dispatch_continuation(input: &mut dyn Read) -> Result<()> {
    let mut deserializer = serde_json::Deserializer::from_reader(input);
    let request = DispatchContinuationRequest::deserialize(&mut deserializer)
        .context("failed to parse dispatch continuation request")?;
    deserializer
        .end()
        .context("dispatch continuation request has trailing non-whitespace bytes")?;
    let envelope = reconstruct_dispatch_envelope(request.envelope)?;
    let required_artifact_path =
        AbsoluteRequiredArtifactPath::parse(request.completion.required_artifact_path)?;
    let continuation_process_identity = observe_current_process_identity()?;
    execute_dispatch(
        &envelope,
        Some(LiveDispatchCompletion {
            path: &request.completion.log_path,
            node: &request.completion.node,
            issuance_sequence: request.completion.issuance_sequence,
            required_artifact_path: &required_artifact_path,
            continuation_process_identity,
        }),
    )
}

fn observe_current_process_identity() -> Result<RecordedProcessIdentity> {
    let process_number = ProcessNumber::new(std::process::id())?;
    match observe_darwin_process_number(process_number)? {
        ProcessIdentityObservation::Present(start) => {
            Ok(RecordedProcessIdentity::new(process_number, start))
        }
        ProcessIdentityObservation::Absent => {
            bail!("failed to observe dispatch continuation process identity: process is absent")
        }
        ProcessIdentityObservation::ForeignPresent => bail!(
            "failed to observe dispatch continuation process identity: process identity is unreadable"
        ),
    }
}

fn reconstruct_dispatch_envelope(wire: ContinuationEnvelope) -> Result<DispatchEnvelope> {
    if wire.target == DispatchTarget::Seatbelt {
        bail!("Seatbelt envelopes must use the contract-measurement process adapter");
    }
    let working_directory = AbsoluteWorkingDirectory::parse(wire.working_directory)?;
    let stdin = match wire.stdin {
        ContinuationStdin::Null => StdinBinding::Null,
        ContinuationStdin::PlanBytes { bytes } => StdinBinding::PlanBytes(bytes),
    };
    let mut environment = wire.environment;
    let recorder = wire
        .gate_execution_recorder
        .map(|config| -> Result<GateExecutionRecorderConfig> {
            let expected_client = config.client_path.display().to_string();
            let expected_socket = config.socket_path.display().to_string();
            if environment.remove("PCE_GATE_EXEC_CLIENT").as_deref() != Some(&expected_client)
                || environment.remove("PCE_GATE_EXEC_SOCKET").as_deref() != Some(&expected_socket)
            {
                bail!("dispatch continuation gate recorder environment does not match its paths");
            }
            let evidence = AbsoluteGateExecutionEvidencePath::from_verdict_path(
                wire.output_path
                    .as_deref()
                    .ok_or_else(|| anyhow!("gate recorder requires an output path"))?,
            );
            if evidence.as_path() != config.evidence_path {
                bail!("dispatch continuation gate recorder evidence path is not derived from its output path");
            }
            Ok(GateExecutionRecorderConfig::new(
                AbsoluteGateExecClientPath::parse(config.client_path)?,
                evidence,
                AbsoluteGateExecutionSocketPath::parse(config.socket_path)?,
            ))
        })
        .transpose()?;
    let mut envelope = DispatchEnvelope::new(wire.target, working_directory, stdin)
        .with_arguments(ArgumentVector::new(wire.arguments))
        .with_environment(ChildEnvironment::new(environment));
    if let Some(sandbox) = wire.sandbox {
        if sandbox != Sandbox::WorkspaceWrite.as_str() {
            bail!("unsupported dispatch continuation sandbox `{sandbox}`");
        }
        envelope = envelope.with_sandbox(Sandbox::WorkspaceWrite);
    }
    if let Some(path) = wire.schema_path {
        envelope = envelope.with_schema_path(AbsoluteSchemaPath::parse(path)?);
    }
    if let Some(path) = wire.output_path {
        envelope = envelope.with_output_path(AbsoluteOutputPath::parse(path)?);
    }
    if let Some(recorder) = recorder {
        envelope = envelope.with_gate_execution_recorder(recorder)?;
    }
    Ok(envelope)
}

fn execute_dispatch(
    envelope: &DispatchEnvelope,
    completion: Option<LiveDispatchCompletion<'_>>,
) -> Result<()> {
    let recorder = envelope
        .gate_execution_recorder()
        .map(GateRecorderRuntime::start)
        .transpose()?;
    let invocation = dispatch_invocation(envelope);
    let executable = invocation.executable();
    let mut command = std::process::Command::new(executable);
    command.args(invocation.argv());
    command.current_dir(invocation.cwd());
    command.env_clear();
    command.envs(invocation.environment());
    let temporary_directory = SynthesizedTemporaryDirectory::create()?;
    command.env("TMPDIR", temporary_directory.path());
    match invocation.stdin_bytes() {
        None => {
            command.stdin(Stdio::null());
        }
        Some(_) => {
            command.stdin(Stdio::piped());
        }
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::inherit());

    let started = Instant::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn `{executable}`"))?;
    let identity_setup = (|| -> Result<()> {
        if let Some(completion) = completion.as_ref() {
            let (process_number, process_start_identity) = observe_darwin_process_identity(&child)?;
            let identity = DispatchProcessIdentity::new(
                completion.issuance_sequence,
                RecordedProcessIdentity::new(process_number, process_start_identity),
                completion.continuation_process_identity,
                completion.required_artifact_path.clone(),
            );
            persist_dispatch_process_identity(completion.path, &identity).with_context(|| {
                format!(
                    "failed to persist dispatch process identity for child PID {}",
                    process_number.get()
                )
            })?;
        }
        Ok(())
    })();
    if let Err(error) = identity_setup {
        return Err(cleanup_failed_dispatch_setup(
            &mut child,
            recorder,
            envelope,
            completion.as_ref(),
            started,
            error,
        ));
    }
    let stdin_writer = if let Some(bytes) = invocation.stdin_bytes() {
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("`{executable}` child stdin was not piped"))?;
        let bytes = bytes.to_vec();
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
    let stdout = std::io::stdout();
    let mut parent_stdout = stdout.lock();
    let mut observations = Vec::new();
    let mut claude_stdout = Vec::new();
    match invocation.target() {
        DispatchTarget::Codex => {
            let mut reader = BufReader::new(child_stdout);
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
        }
        DispatchTarget::Gate => {
            let mut reader = BufReader::new(child_stdout);
            let mut buffer = [0_u8; 8192];
            loop {
                let count = reader
                    .read(&mut buffer)
                    .with_context(|| format!("failed to read Claude result from `{executable}`"))?;
                if count == 0 {
                    break;
                }
                let bytes = &buffer[..count];
                parent_stdout
                    .write_all(bytes)
                    .context("failed to tee Claude result to stdout")?;
                parent_stdout
                    .flush()
                    .context("failed to flush Claude result")?;
                claude_stdout.extend_from_slice(bytes);
            }
        }
        DispatchTarget::Seatbelt => {
            bail!("Seatbelt envelopes must use the contract-measurement process adapter")
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
    let recorder_stop = recorder.map(GateRecorderRuntime::stop);
    let gate_execution_records = recorder_stop
        .as_ref()
        .map_or_else(Vec::new, |stopped| stopped.records.clone());
    let recorder_error = recorder_stop.and_then(|stopped| stopped.error);
    let evidence_persistence = match envelope.gate_execution_recorder() {
        Some(config) => {
            persist_gate_execution_evidence(config.evidence(), gate_execution_records.clone())
        }
        None => Ok(()),
    };
    let post_stop_result = (|| -> Result<_> {
        let duration_ms = u64::try_from(started.elapsed().as_millis())
            .context("dispatch duration in milliseconds exceeds u64")?;
        let exit_status = dispatch_exit_status(status)?;
        let classification = match invocation.target() {
            DispatchTarget::Codex => classify_codex_terminal_usage(&observations, exit_status),
            DispatchTarget::Gate => {
                classify_claude_result(&parse_claude_result(&claude_stdout), exit_status)
            }
            DispatchTarget::Seatbelt => {
                bail!("Seatbelt envelopes must use the contract-measurement process adapter")
            }
        };
        let artifact_validation: Result<ArtifactOutcome, pce_core::ArtifactValidationError> =
            match (envelope.schema_path(), envelope.output_path()) {
                (Some(schema_path), Some(output_path)) => {
                    let artifact = read_file_observation(output_path.as_path());
                    // The falsification role frame and its reference validator are compiled into
                    // this binary, so its verdict schema must come from the same build as well.
                    let filesystem_schema = if envelope.gate_execution_recorder().is_some() {
                        None
                    } else {
                        Some(read_file_observation(schema_path.as_path()))
                    };
                    let schema_observation = match filesystem_schema.as_ref() {
                        Some(schema) => schema.as_observation(),
                        None => FileObservation::Readable {
                            bytes: VERDICT_SCHEMA.as_bytes(),
                        },
                    };
                    validate_artifact(StructuredArtifactObservation::new(
                        schema_path,
                        schema_observation,
                        output_path,
                        artifact.as_observation(),
                    ))
                }
                _ => Ok(ArtifactOutcome::NotValidated),
            };
        let reference_validation =
            if matches!(artifact_validation.as_ref(), Ok(ArtifactOutcome::Validated))
                && envelope.gate_execution_recorder().is_some()
            {
                let output_path = envelope.output_path().ok_or_else(|| {
                    anyhow!("falsification recorder requires a verdict output path")
                })?;
                match std::fs::read(output_path.as_path()) {
                    Ok(bytes) => validate_verdict_references(&bytes, &gate_execution_records)
                        .map_err(Error::new),
                    Err(error) => Err(Error::new(error)
                        .context("failed to reread validated falsification verdict")),
                }
            } else {
                Ok(())
            };
        let artifact_outcome = if reference_validation.is_err() {
            ArtifactOutcome::SchemaViolating
        } else {
            artifact_validation
                .as_ref()
                .copied()
                .unwrap_or_else(|error| error.outcome())
        };
        if let Some(completion_context) = completion.as_ref() {
            let usage = classification
                .clone()
                .unwrap_or_else(|reason| DispatchTokenUsage::Absent { reason });
            let required_artifact_presence = observe_required_artifact_presence(
                completion_context.required_artifact_path.as_path(),
            )?;
            let root_cause = validated_artifact_root_cause(envelope, artifact_outcome)?;
            let completion = match root_cause {
                Some(root_cause) => validated_dispatch_completion_payload(
                    completion_context.issuance_sequence,
                    DispatchDuration::new(duration_ms),
                    usage,
                    exit_status,
                    required_artifact_presence,
                    root_cause,
                ),
                None => dispatch_completion_payload(
                    completion_context.issuance_sequence,
                    DispatchDuration::new(duration_ms),
                    usage,
                    exit_status,
                    artifact_outcome,
                    required_artifact_presence,
                ),
            };
            close_dispatch_conditionally(
                completion_context.path,
                DispatchClosureTarget {
                    issuance_sequence: completion_context.issuance_sequence,
                    node: completion_context.node.clone(),
                },
                |_| Ok(completion),
            )
            .context("failed to append dispatch completion after child exit")?;
        }
        Ok((classification, artifact_validation, reference_validation))
    })();
    let (classification, artifact_validation, reference_validation) = match post_stop_result {
        Ok(results) => results,
        Err(error) => {
            if let Err(persistence_error) = evidence_persistence {
                tracing::error!(error = ?persistence_error, "failed to persist gate execution evidence after dispatch finalization failure");
            }
            return Err(error);
        }
    };
    evidence_persistence?;
    if let Some(error) = recorder_error {
        return Err(error);
    }
    if let Err(reason) = classification {
        match invocation.target() {
            DispatchTarget::Codex => {
                bail!(
                    "invalid Codex terminal data: {}",
                    usage_absence_name(reason)
                );
            }
            DispatchTarget::Gate => {
                bail!("invalid Claude result data: {}", usage_absence_name(reason));
            }
            DispatchTarget::Seatbelt => {
                bail!("Seatbelt envelopes must use the contract-measurement process adapter");
            }
        }
    }
    if !status.success() {
        bail!("`{executable}` child exited with status {status}");
    }
    artifact_validation.map_err(Error::new)?;
    reference_validation?;
    Ok(())
}

fn validated_artifact_root_cause(
    envelope: &DispatchEnvelope,
    artifact_outcome: ArtifactOutcome,
) -> Result<Option<DispatchRootCause>> {
    if artifact_outcome != ArtifactOutcome::Validated {
        return Ok(None);
    }
    let Some(output_path) = envelope.output_path() else {
        return Ok(None);
    };
    let bytes = std::fs::read(output_path.as_path()).with_context(|| {
        format!(
            "failed to reread validated structured artifact {} for root-cause attribution",
            output_path.as_path().display()
        )
    })?;
    let value = serde_json::from_slice::<Value>(&bytes).with_context(|| {
        format!(
            "validated structured artifact {} became malformed during root-cause attribution",
            output_path.as_path().display()
        )
    })?;
    Ok(match value.get("root_cause").and_then(Value::as_str) {
        Some("execution") => Some(DispatchRootCause::Execution),
        Some("step_plan") => Some(DispatchRootCause::StepPlan),
        Some("milestone_plan") => Some(DispatchRootCause::MilestonePlan),
        Some("vision") => Some(DispatchRootCause::Vision),
        _ => None,
    })
}

fn spawn_envelope(envelope: &DispatchEnvelope) -> Result<ObservedExitStatus> {
    let invocation = dispatch_invocation(envelope);
    let executable = invocation.executable();
    let mut command = std::process::Command::new(executable);
    command.args(invocation.argv());
    command.current_dir(invocation.cwd());
    command.env_clear();
    command.envs(invocation.environment());
    let temporary_directory = SynthesizedTemporaryDirectory::create()?;
    command.env("TMPDIR", temporary_directory.path());
    match invocation.stdin_bytes() {
        None => {
            command.stdin(Stdio::null());
        }
        Some(_) => {
            command.stdin(Stdio::piped());
        }
    }
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let mut child = command
        .spawn()
        .with_context(|| format!("failed to spawn `{executable}`"))?;
    if let Some(bytes) = invocation.stdin_bytes() {
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("`{executable}` child stdin was not piped"))?;
        child_stdin
            .write_all(bytes)
            .with_context(|| format!("failed to write plan bytes to `{executable}`"))?;
        drop(child_stdin);
    }
    let status = child
        .wait()
        .with_context(|| format!("failed to wait for `{executable}`"))?;
    status
        .code()
        .map(ObservedExitStatus::from_code)
        .ok_or_else(|| {
            anyhow!("`{executable}` child terminated without an exit-status code: {status}")
        })
}

fn read_file_observation(path: &Path) -> OwnedFileObservation {
    match std::fs::read(path) {
        Ok(bytes) => OwnedFileObservation::Readable(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => OwnedFileObservation::Missing,
        Err(error) => OwnedFileObservation::Unreadable(error.to_string()),
    }
}

fn observe_terminal_line(line: &[u8]) -> CodexTerminalObservation {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return CodexTerminalObservation::MalformedLine;
    };
    let Some(object) = value.as_object() else {
        return CodexTerminalObservation::NonTerminal;
    };
    match object.get("type").and_then(Value::as_str) {
        Some("turn.completed") => {
            let usage = object.get("usage").and_then(Value::as_object);
            let usage = usage.and_then(|usage| {
                Some(CodexTerminalUsage {
                    input_tokens: usage.get("input_tokens")?.as_u64()?,
                    cached_input_tokens: usage.get("cached_input_tokens")?.as_u64()?,
                    output_tokens: usage.get("output_tokens")?.as_u64()?,
                    reasoning_output_tokens: usage.get("reasoning_output_tokens")?.as_u64()?,
                })
            });
            CodexTerminalObservation::TurnCompleted(usage)
        }
        Some("turn.failed") => CodexTerminalObservation::TurnFailed {
            usage_present: object.contains_key("usage"),
        },
        _ => CodexTerminalObservation::NonTerminal,
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
        UsageAbsenceReason::ClaudeMalformedResult => "claude-malformed-result",
        UsageAbsenceReason::ClaudeMissingUsage => "claude-missing-usage",
        UsageAbsenceReason::ClaudeErrorEnvelope => "claude-error-envelope",
        UsageAbsenceReason::ClaudeExitEnvelopeContradiction => "claude-exit-envelope-contradiction",
    }
}

fn execute_process(program: &str, args: &[OsString], current_dir: Option<&Path>) -> ProcessAttempt {
    execute_process_with_command(program, args, current_dir, |_| {})
}

fn execute_process_without_force_color(
    program: &str,
    args: &[OsString],
    current_dir: Option<&Path>,
) -> ProcessAttempt {
    execute_process_with_command(program, args, current_dir, |command| {
        for name in ["CLICOLOR", "CLICOLOR_FORCE", "FORCE_COLOR", "GH_FORCE_TTY"] {
            command.env_remove(name);
        }
    })
}

fn execute_process_with_command(
    program: &str,
    args: &[OsString],
    current_dir: Option<&Path>,
    configure: impl FnOnce(&mut std::process::Command),
) -> ProcessAttempt {
    let mut command = std::process::Command::new(program);
    command.args(args);
    configure(&mut command);
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
    use std::collections::BTreeSet;
    use std::ffi::OsString;
    use std::fs;
    use std::io::{Cursor, Read};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::Command as ProcessCommand;
    use std::time::{Duration, SystemTime};

    use pce_core::{
        AcceptanceCriteria, AmendmentRepositoryRefs, AppendableFinding, ArtifactPath, BranchState,
        CachedInputTokens, CodexTerminalObservation, CriterionOrigin, CurrentArtifactObservation,
        CurrentArtifactState, DispatchExitStatus, DispatchTokenUsage, EffectiveCriterion,
        EventBodyRef, EventKindName, EventRecord, EventRecordFilter, ExactPullRequestIdentity,
        ExactPullRequestState, ExceptionalMergeChain, ExceptionalMergeChainObservation, ExitCode,
        GitAuthorityObservation, GitHubAuthorityObservation, GitHubPullRequestObservation,
        GitMergeObservation, InputTokens, KnownPayload, MilestoneMergeSubject, MilestoneNode,
        NESTED_SEATBELT_SKIP_MARKER, NodeId, ObservedExitStatus, OutputTokens,
        PullRequestAuthorityObservation, PullRequestNumber, ReadKind, ReadPayload,
        ReasoningOutputTokens, RecoveryLogPath, RepositoryBranchName, RepositoryFetchObservation,
        RepositoryName, RepositoryObservation, RepositoryObservationFailure, RunSnapshot,
        SeatbeltCapability, Sha256Digest, SquashCommitOid, StepAuthorityObservation, StepNode,
        TagName, TagState, VersionPolicy, VisionSlug, WorktreeIdentity, WorktreeState, WriteKind,
        classify_codex_terminal_usage, derive_run_state,
        derive_run_state_with_exceptional_merge_chains, parse_acceptance_criteria,
        parse_event_line, render_human_snapshot,
    };
    use serde_json::json;
    use tempfile::tempdir;

    use crate::{
        BranchFetch, Command, DispatchGraphNode, DispatchLoggingMode, DispatchNode,
        FORMAT_BOOTSTRAP_CANDIDATES, FetchResult, RepositoryContract, RepositoryRuntime,
        StatusFormat, USAGE, already_dispatched, execute_effective_criterion,
        github_pull_request_list_args, lexically_normalized_repository_root,
        measure_tracked_contract_at_root, observe_git, observe_terminal_line, parse_command,
        parse_dispatch_graph, parse_tracked_contract, read_at_default_branch_head, read_event_log,
        read_ratified_acceptance_criteria, readiness_version_policies, render_seatbelt_profile,
        repository_contracts, run, run_log_read, seatbelt_execution_capability,
        select_bootstrap_candidate, validated_snapshot_value,
    };

    fn ratified_floor() -> AcceptanceCriteria {
        parse_acceptance_criteria(
            r#"# Vision: fixture

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Ratified floor","input":"Run the finished thing.","observation":"It reports success."}]}
```

## Decomposition hints

None.
"#,
        )
        .expect("ratified floor fixture")
    }

    #[test]
    fn composition_root_loads_ratified_acceptance_criteria_with_exact_context() {
        let directory = tempdir().expect("temporary directory");
        let vision_dir = directory.path().join("planning/2026-08-03-fixture");
        fs::create_dir_all(&vision_dir).expect("vision directory");
        fs::write(
            vision_dir.join("vision.md"),
            r#"# Vision: fixture

## Acceptance criteria (vision-level "done")

```json
{
  "criteria": [
    {
      "name": "Ratified floor",
      "input": "Run the finished thing.",
      "observation": "It reports success."
    }
  ]
}
```

## Decomposition hints

None.
"#,
        )
        .expect("vision fixture");
        let criteria = read_ratified_acceptance_criteria(&vision_dir).expect("ratified criteria");
        assert_eq!(criteria.len(), 1);
        assert_eq!(criteria.as_slice()[0].name().as_str(), "Ratified floor");
        assert_eq!(
            criteria.as_slice()[0].input().as_str(),
            "Run the finished thing."
        );
        assert_eq!(
            criteria.as_slice()[0].observation().as_str(),
            "It reports success."
        );

        let missing = directory.path().join("missing");
        assert_eq!(
            read_ratified_acceptance_criteria(&missing)
                .expect_err("missing vision")
                .to_string(),
            format!(
                "failed to read ratified acceptance criteria from {}",
                missing.join("vision.md").display()
            )
        );
        fs::write(vision_dir.join("vision.md"), "malformed").expect("malformed fixture");
        assert_eq!(
            read_ratified_acceptance_criteria(&vision_dir)
                .expect_err("malformed vision")
                .to_string(),
            format!(
                "failed to parse ratified acceptance criteria from {}",
                vision_dir.join("vision.md").display()
            )
        );
    }

    #[test]
    fn seatbelt_profile_allows_only_sandbox_tree_signals_and_path_filtered_unix_sockets() {
        let temporary_directories = BTreeSet::from([PathBuf::from("/private/tmp")]);

        let profile =
            render_seatbelt_profile(Path::new("/workspace/repository"), &temporary_directories)
                .expect("fixed Seatbelt paths should render");

        assert_eq!(
            profile,
            concat!(
                "(version 1)\n",
                "(deny default)\n",
                "(import \"system.sb\")\n",
                "(allow process*)\n",
                "(allow signal (target children))\n",
                "(allow signal (target same-sandbox))\n",
                "(allow file-read*)\n",
                "(deny file-write*\n",
                "  (require-all\n",
                "    (require-not (subpath \"/workspace/repository\"))\n",
                "    (require-not (subpath \"/private/tmp\"))\n",
                "  ))\n",
                "(allow file-write*\n",
                "  (subpath \"/workspace/repository\")\n",
                "  (subpath \"/private/tmp\")\n",
                ")\n",
                "(deny network*\n",
                "  (require-all\n",
                "    (require-not (subpath \"/workspace/repository\"))\n",
                "    (require-not (subpath \"/private/tmp\"))\n",
                "  ))\n",
                "(allow network-bind network-outbound\n",
                "  (subpath \"/workspace/repository\")\n",
                "  (subpath \"/private/tmp\")\n",
                ")"
            )
        );
    }

    fn record_nested_seatbelt_skip() {
        let status = ProcessCommand::new("/bin/sh")
            .args([
                "-c",
                "printf '%s\\n' \"$1\" >&2",
                "pce-test-skip",
                NESTED_SEATBELT_SKIP_MARKER,
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .status()
            .expect("skip marker process should spawn");
        assert!(status.success(), "skip marker process should succeed");
    }

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
        if let SeatbeltCapability::Unavailable { .. } =
            seatbelt_execution_capability(Path::new(".")).expect("Seatbelt probe should execute")
        {
            record_nested_seatbelt_skip();
            return;
        }
        let directory = tempdir().expect("temporary directory should create");
        let root = directory.path().join("repository");
        let log_path = directory.path().join("events.jsonl");
        initialize_git_repository(&root);
        let strict_test = root.join("strict-test");
        fs::write(&strict_test, "#!/bin/sh\nexit 23\n").expect("strict shim should write");
        let mut permissions = fs::metadata(&strict_test)
            .expect("strict shim metadata should read")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&strict_test, permissions).expect("strict shim should be executable");
        let strict_command = strict_test.display().to_string();
        let strict = ["true", "true", "true", strict_command.as_str(), "true"];
        let relaxed = ["true", "true", "true", "true", "true"];
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
        assert_eq!(main.stated().gates().test().as_str(), strict_command);
        let before = fs::read(&log_path).expect("event log should read");

        let err = invoke_refresh(&log_path, &root).expect_err("strict test gate should fail");

        let rendered = format!("{err:#}");
        assert!(rendered.contains("failed to measure tracked repository contract"));
        assert!(rendered.contains(&format!(
            "stated gate command `{strict_command}` exited with status 23"
        )));
        assert_eq!(fs::read(&log_path).expect("event log should read"), before);
        assert_eq!(
            read_event_log(&log_path)
                .expect("event log should parse")
                .len(),
            1
        );
    }

    #[test]
    fn contract_refresh_rereads_post_merge_command_and_reexecutes_all_gates() {
        if let SeatbeltCapability::Unavailable { .. } =
            seatbelt_execution_capability(Path::new(".")).expect("Seatbelt probe should execute")
        {
            record_nested_seatbelt_skip();
            return;
        }
        let directory = tempdir().expect("temporary directory should create");
        let root = directory.path().join("repository");
        let log_path = directory.path().join("events.jsonl");
        initialize_git_repository(&root);
        let invocation_log = root.join("gate-invocations");
        let make_shim = |name: &str| {
            let path = root.join(name);
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' '{}' >> '{}' || exit 74\n",
                    path.display(),
                    invocation_log.display()
                ),
            )
            .expect("recording shim should write");
            let mut permissions = fs::metadata(&path)
                .expect("recording shim metadata should read")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("recording shim should be executable");
            path.display().to_string()
        };
        let format = make_shim("format-gate");
        let lint = make_shim("lint-gate");
        let typecheck = make_shim("typecheck-gate");
        let old_test = make_shim("old-test-gate");
        let new_test = make_shim("new-test-gate");
        let build = make_shim("build-gate");
        let initial = [
            format.as_str(),
            lint.as_str(),
            typecheck.as_str(),
            old_test.as_str(),
            build.as_str(),
        ];
        commit_contract(&root, &tracked_contract_bytes(initial), "initial contract");
        establish_remote_head(&root);
        fs::write(&log_path, current_contract_line(&root, initial)).expect("event log should seed");

        invoke_refresh(&log_path, &root).expect("unchanged refresh should succeed");
        let first_invocations =
            fs::read_to_string(&invocation_log).expect("first invocation log should read");
        let expected_first = [
            format.as_str(),
            lint.as_str(),
            typecheck.as_str(),
            old_test.as_str(),
            build.as_str(),
        ]
        .join("\n")
            + "\n";
        assert_eq!(first_invocations, expected_first);
        assert_eq!(
            read_event_log(&log_path)
                .expect("event log should parse")
                .len(),
            1,
            "semantically unchanged refresh must not append a duplicate contract"
        );
        let changed = [
            format.as_str(),
            lint.as_str(),
            typecheck.as_str(),
            new_test.as_str(),
            build.as_str(),
        ];
        commit_contract(
            &root,
            &tracked_contract_bytes(changed),
            "change test command",
        );
        invoke_refresh(&log_path, &root).expect("changed refresh should succeed");

        let lines = read_event_log(&log_path).expect("event log should parse");
        assert_eq!(lines.len(), 2);
        let all_invocations =
            fs::read_to_string(&invocation_log).expect("second invocation log should read");
        let expected_second = [
            format.as_str(),
            lint.as_str(),
            typecheck.as_str(),
            new_test.as_str(),
            build.as_str(),
        ]
        .join("\n")
            + "\n";
        assert_eq!(
            all_invocations,
            format!("{first_invocations}{expected_second}")
        );
        let records = lines
            .iter()
            .map(|line| line.record.clone())
            .collect::<Vec<_>>();
        let contracts = repository_contracts(&records).expect("contracts should project");
        let [RepositoryContract::Current(latest)] = contracts.as_slice() else {
            panic!("one latest current contract expected");
        };
        assert_eq!(latest.stated.test, new_test);
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
    fn accepts_all_twelve_registered_payload_schemas() {
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
            (
                "criterion-execution",
                r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
                WriteKind::CriterionExecution,
            ),
            (
                "criterion-added",
                r#"{"criterion":{"name":"New blocking criterion","input":"Run the new probe.","observation":"The probe exits 0."},"change_of_course":"Reality exposed an uncovered failure."}"#,
                WriteKind::CriterionAdded,
            ),
            (
                "non-production-hold-open",
                r#"{"key":{"node":"m4-s1","role":"step-plan-writer","required_artifact_path":"/workspace/plan.md"}}"#,
                WriteKind::NonProductionHoldOpen,
            ),
            (
                "non-production-hold-close",
                r#"{"key":{"node":"m4-s1","role":"step-plan-writer","required_artifact_path":"/workspace/plan.md"},"resolution":"retry"}"#,
                WriteKind::NonProductionHoldClose,
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
            parse_command(["vision", "check"].into_iter().map(str::to_owned))
                .expect("vision check command should parse"),
            Command::VisionCheck
        ));

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
    fn completion_parser_accepts_only_the_canonical_ordered_shape() {
        let canonical = [
            "completion",
            "check",
            "--file",
            "events.jsonl",
            "--vision-dir",
            "planning/2026-08-03-a-gate-runs-what-was-built",
            "--finished-result",
            "main@0123456789abcdef",
        ];
        let command = parse_command(canonical.into_iter().map(str::to_owned))
            .expect("completion command should parse");
        let Command::CompletionCheck {
            log_path,
            recovery_log_path,
            vision_dir,
            finished_result,
        } = command
        else {
            panic!("completion command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(recovery_log_path.as_str(), "events.jsonl");
        assert_eq!(
            vision_dir,
            PathBuf::from("planning/2026-08-03-a-gate-runs-what-was-built")
        );
        assert_eq!(finished_result.as_str(), "main@0123456789abcdef");

        const LINE: &str = "pce completion check --file <LOG_PATH> --vision-dir <VISION_DIR> --finished-result <FINISHED_RESULT>";
        assert_eq!(USAGE.matches(LINE).count(), 1);

        let invalid = [
            vec![
                "completion",
                "check",
                "--vision-dir",
                "vision",
                "--file",
                "events.jsonl",
                "--finished-result",
                "result",
            ],
            vec![
                "completion",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
                "--finished-result",
            ],
            vec![
                "completion",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
            ],
            vec!["completion", "check", "--file", "events.jsonl"],
            vec!["completion", "check"],
            vec![
                "completion",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
                "--finished-result",
                "result",
                "extra",
            ],
            vec!["completion"],
        ];
        for args in invalid {
            let error = parse_command(args.into_iter().map(str::to_owned))
                .expect_err("invalid completion shape must fail");
            assert_eq!(error.to_string(), USAGE);
        }
        for missing_index in 2..canonical.len() {
            let args = canonical
                .iter()
                .enumerate()
                .filter_map(|(index, argument)| (index != missing_index).then_some(*argument));
            let error = parse_command(args.map(str::to_owned))
                .expect_err("each missing flag or value must fail");
            assert_eq!(error.to_string(), USAGE);
        }
    }

    #[test]
    fn criteria_parser_accepts_only_the_canonical_ordered_shape() {
        let canonical = [
            "criteria",
            "check",
            "--file",
            "events.jsonl",
            "--vision-dir",
            "planning/2026-08-03-a-gate-runs-what-was-built",
        ];
        let command = parse_command(canonical.into_iter().map(str::to_owned))
            .expect("criteria command should parse");
        let Command::CriteriaCheck {
            log_path,
            recovery_log_path,
            vision_dir,
        } = command
        else {
            panic!("criteria command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(recovery_log_path.as_str(), "events.jsonl");
        assert_eq!(
            vision_dir,
            PathBuf::from("planning/2026-08-03-a-gate-runs-what-was-built")
        );
        const LINE: &str = "pce criteria check --file <LOG_PATH> --vision-dir <VISION_DIR>";
        assert_eq!(USAGE.matches(LINE).count(), 1);

        let invalid = [
            vec![
                "criteria",
                "check",
                "--vision-dir",
                "vision",
                "--file",
                "events.jsonl",
            ],
            vec!["criteria", "check"],
            vec![
                "criteria",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
                "extra",
            ],
            vec![
                "criteria",
                "validate",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
            ],
            vec!["criteria"],
        ];
        for args in invalid {
            let error = parse_command(args.into_iter().map(str::to_owned))
                .expect_err("invalid criteria shape must fail");
            assert_eq!(error.to_string(), USAGE);
        }
        for missing_index in 2..canonical.len() {
            let args = canonical
                .iter()
                .enumerate()
                .filter_map(|(index, argument)| (index != missing_index).then_some(*argument));
            let error = parse_command(args.map(str::to_owned))
                .expect_err("each missing flag or value must fail");
            assert_eq!(error.to_string(), USAGE);
        }
    }

    #[test]
    fn criteria_check_reports_stdin_read_failure_before_filesystem_access() {
        struct BrokenReader;
        impl std::io::Read for BrokenReader {
            fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("reader failed"))
            }
        }
        let error = super::run_criteria_check(
            Path::new("missing-events.jsonl"),
            &RecoveryLogPath::new("missing-events.jsonl"),
            Path::new("missing-vision"),
            &mut BrokenReader,
        )
        .expect_err("reader failure should stop the command");
        let chain = error.chain().map(ToString::to_string).collect::<Vec<_>>();
        assert!(
            chain
                .iter()
                .any(|message| message == "failed to read proposed vision from stdin to EOF")
        );
    }

    #[test]
    fn landing_parser_accepts_only_the_canonical_ordered_shape() {
        let canonical = [
            "landing",
            "check",
            "--file",
            "events.jsonl",
            "--vision-dir",
            "planning/2026-08-03-a-gate-runs-what-was-built",
            "--finished-result",
            "main@0123456789abcdef",
        ];
        let command = parse_command(canonical.into_iter().map(str::to_owned))
            .expect("landing command should parse");
        let Command::LandingCheck {
            log_path,
            recovery_log_path,
            vision_dir,
            finished_result,
        } = command
        else {
            panic!("landing command expected");
        };
        assert_eq!(log_path, PathBuf::from("events.jsonl"));
        assert_eq!(recovery_log_path.as_str(), "events.jsonl");
        assert_eq!(
            vision_dir,
            PathBuf::from("planning/2026-08-03-a-gate-runs-what-was-built")
        );
        assert_eq!(finished_result.as_str(), "main@0123456789abcdef");

        const LINE: &str = "pce landing check --file <LOG_PATH> --vision-dir <VISION_DIR> --finished-result <FINISHED_RESULT>";
        assert_eq!(USAGE.matches(LINE).count(), 1);

        let invalid = [
            vec![
                "landing",
                "check",
                "--vision-dir",
                "vision",
                "--file",
                "events.jsonl",
                "--finished-result",
                "result",
            ],
            vec![
                "landing",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
                "--finished-result",
            ],
            vec![
                "landing",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
            ],
            vec!["landing", "check", "--file", "events.jsonl"],
            vec!["landing", "check"],
            vec!["landing"],
            vec![
                "landing",
                "inspect",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
                "--finished-result",
                "result",
            ],
            vec![
                "landing",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
                "--finished-result",
                "result",
                "extra",
            ],
        ];
        for arguments in invalid {
            let error = parse_command(arguments.into_iter().map(str::to_owned))
                .expect_err("non-canonical landing command must fail");
            assert_eq!(error.to_string(), USAGE);
        }

        let error = parse_command(
            [
                "landing",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "vision",
                "--finished-result",
                "   ",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect_err("blank finished result must fail");
        let chain = format!("{error:#}");
        assert!(chain.contains("failed to parse finished result"));
        assert!(chain.contains("finished result cannot be blank"));
    }

    #[test]
    fn completion_parser_preserves_blank_finished_result_error_chain() {
        let error = parse_command(
            [
                "completion",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-08-03-a-gate-runs-what-was-built",
                "--finished-result",
                "   ",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect_err("blank finished result must fail");
        let chain = error.chain().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(chain[0], "failed to parse finished result");
        assert_eq!(chain[1], "finished result cannot be blank");
    }

    #[test]
    fn vision_check_parser_rejects_arguments_and_unknown_actions_with_usage() {
        assert!(USAGE.contains("pce vision check < vision.md"));
        for args in [
            vec!["vision", "check", "extra"],
            vec!["vision", "check", "--file", "vision.md"],
            vec!["vision", "validate"],
        ] {
            let error = parse_command(args.into_iter().map(str::to_owned))
                .expect_err("invalid vision check shape must be rejected");
            assert_eq!(error.to_string(), super::USAGE);
        }
    }

    #[test]
    fn vision_check_run_reads_stdin_and_adds_stable_refusal_context() {
        let conforming = r#"# Vision: example

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Runs","input":"Run it.","observation":"It exits zero."}]}
```
"#;
        let mut input = Cursor::new(conforming.as_bytes());
        run(
            ["vision", "check"].into_iter().map(str::to_owned),
            &mut input,
        )
        .expect("conforming vision should pass");

        let mut input = Cursor::new("# Vision: missing criteria".as_bytes());
        let error = run(
            ["vision", "check"].into_iter().map(str::to_owned),
            &mut input,
        )
        .expect_err("missing acceptance section should fail");
        assert_eq!(
            error.to_string(),
            "failed to check vision acceptance criteria"
        );
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
    fn successive_contract_measurements_reexecute_every_gate() {
        if let SeatbeltCapability::Unavailable { .. } =
            seatbelt_execution_capability(Path::new(".")).expect("Seatbelt probe should execute")
        {
            record_nested_seatbelt_skip();
            return;
        }
        let directory = tempdir().expect("temporary directory should create");
        let repository_root = directory.path().join("repo");
        fs::create_dir(&repository_root).expect("repository fixture should create");
        let contract_path = directory.path().join("contract.json");
        let marker_path = repository_root.join("gate-runs");
        let shim_path = repository_root.join("record-gate");
        fs::write(
            &shim_path,
            format!(
                "#!/bin/sh\nprintf x >> '{}' || exit 74\n",
                marker_path.display()
            ),
        )
        .expect("gate shim should write");
        let mut permissions = fs::metadata(&shim_path)
            .expect("gate shim metadata should read")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim_path, permissions).expect("gate shim should be executable");
        let command = shim_path.display().to_string();
        fs::write(
            &contract_path,
            format!(
                r#"{{
  "stated": {{
    "gates": {{
      "format": "{command}",
      "lint": "{command}",
      "typecheck": "{command}",
      "test": "{command}",
      "build": "{command}"
    }},
    "version_policy": "NONE",
    "branches": {{
      "default": "main",
      "milestone": "pce/{{vision}}/milestone-{{milestone}}",
      "step": "pce/{{vision}}/m{{milestone}}-s{{step}}"
    }},
    "pull_requests": {{
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    }},
    "workflows": []
  }},
  "appendable": {{
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }}
}}"#
            ),
        )
        .expect("contract fixture should write");

        let _first = measure_tracked_contract_at_root(&contract_path, &repository_root)
            .expect("first measurement should succeed");
        assert_eq!(
            fs::read_to_string(&marker_path).expect("marker should read"),
            "xxxxx"
        );

        let first_invocations =
            fs::read_to_string(&marker_path).expect("first invocation log should read");
        measure_tracked_contract_at_root(&contract_path, &repository_root)
            .expect("second measurement should freshly execute every gate");
        assert_eq!(
            fs::read_to_string(marker_path).expect("marker should read"),
            format!("{first_invocations}{first_invocations}")
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
            ..
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
    fn package_dispatch_parser_keeps_legacy_routes_additive() {
        let command = parse_command(
            [
                "dispatch",
                "package",
                "--file",
                "/tmp/events.jsonl",
                "--vision-dir",
                "/tmp",
                "--graph",
                "/tmp/graph.json",
                "--package",
                "WP4",
                "--required-artifact",
                "/tmp/result.json",
                "--repository",
                "pce=/repos/pce",
                "--env",
                "PATH=/usr/bin",
                "--env",
                "HOME=/home/worker",
                "--env",
                "USER=worker",
                "--",
                "/usr/bin/true",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("package dispatch should parse");
        let Command::PackageDispatch(command) = command else {
            panic!("package dispatch expected");
        };
        assert_eq!(command.package_id, "WP4");
        assert_eq!(
            command.repositories,
            [("pce".to_owned(), PathBuf::from("/repos/pce"))]
        );
        assert_eq!(command.worker_arguments, ["/usr/bin/true"]);

        assert!(
            parse_command(
                ["dispatch", "codex", "--cwd"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .is_err()
        );
        assert!(USAGE.contains("pce dispatch codex"));
        assert!(USAGE.contains("pce dispatch gate"));
    }

    #[test]
    fn ready_parser_rejects_caller_supplied_merge_state() {
        let error = parse_command(
            [
                "ready",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-07-28-example",
                "--merged",
                "WP1",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect_err("ready must derive merge state rather than accept it");
        assert_eq!(error.to_string(), USAGE);
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
            &ratified_floor(),
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
            &ratified_floor(),
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
                "pce status (pce.run-snapshot v2)\n",
                "repositories (0)\nsteps (0)\ndispatch-accounting state=all-accounted issuance-sequences=-\ndispatches (0)\nissuance-ordinals (0)\nrounds (0)\nvalidated-production-spending (limit-per-node-role=12, series=0)\nnon-production-streaks (0)\nnon-production-holds (0)\nholds (0)\n",
                "provenance (0)\nresume state=no-log-visible-candidate\nrecovery-digest\n",
                "  rounds (entries=0, elisions=0)\n",
                "  open-holds (entries=0, elisions=0)\n",
                "  deltas (entries=0, elisions=0)\n",
                "  facts (entries=0, elisions=0)\n",
            )
        );
        assert_eq!(value["schema_id"], "pce.run-snapshot");
        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["issuance_ordinals"], serde_json::json!([]));
        assert_eq!(value["non_production_streaks"], serde_json::json!([]));
        assert_eq!(value["non_production_holds"], serde_json::json!([]));
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
            &ratified_floor(),
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
    fn dispatch_check_in_parser_accepts_absolute_and_relative_paths() {
        let command = parse_command(
            ["dispatch", "check-in", "--file", "/tmp/events.jsonl"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect("canonical check-in command");
        let Command::DispatchCheckIn { log_path } = command else {
            panic!("parsed another command")
        };
        assert_eq!(log_path, PathBuf::from("/tmp/events.jsonl"));
        let relative = parse_command(
            ["dispatch", "check-in", "--file", "planning/events.jsonl"]
                .into_iter()
                .map(str::to_owned),
        )
        .expect("relative check-in command");
        let Command::DispatchCheckIn { log_path } = relative else {
            panic!("parsed another command")
        };
        assert_eq!(log_path, PathBuf::from("planning/events.jsonl"));
        assert_eq!(
            USAGE
                .matches("pce dispatch check-in --file <LOG_PATH>")
                .count(),
            1
        );
    }

    #[test]
    fn dispatch_check_in_parser_rejects_every_noncanonical_shape_with_usage() {
        for rejected in [
            vec!["dispatch", "check-in"],
            vec![
                "dispatch",
                "check-in",
                "--file",
                "/tmp/events.jsonl",
                "extra",
            ],
            vec!["dispatch", "check-in", "/tmp/events.jsonl", "--file"],
            vec!["dispatch", "check-in", "--file", ""],
            vec!["dispatch", "check-in", "--unknown", "/tmp/events.jsonl"],
        ] {
            let error = parse_command(rejected.into_iter().map(str::to_owned))
                .expect_err("noncanonical check-in command");
            assert_eq!(error.to_string(), USAGE);
        }
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
                "--required-artifact",
                "/tmp/output.json",
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
                "--required-artifact",
                "/tmp/output.json",
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
            assert!(format!("{error:#}").contains("dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence, --required-artifact"));
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
                "--required-artifact",
                "/tmp/output.json",
                "--",
                "PROMPT",
            ])
            .map(str::to_owned);
        let error = parse_command(misordered).expect_err("misordered complete group must fail");
        assert!(format!("{error:#}").contains("dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence, --required-artifact"));

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
                "--required-artifact",
                "/tmp/output.json",
                "--plan-file",
                "/tmp/plan.md",
                "--",
                "-",
            ])
            .map(str::to_owned);
        let error = parse_command(before_plan).expect_err("logging before plan must fail");
        assert!(format!("{error:#}").contains("dispatch arguments require the `--` delimiter"));

        let gate_prefix = [
            "dispatch",
            "gate",
            "--cwd",
            "/tmp",
            "--output-schema",
            "/tmp/schema.json",
            "-o",
            "/tmp/output.json",
        ];
        let no_logging = gate_prefix.into_iter().chain(["--"]).map(str::to_owned);
        let Command::Dispatch { logging, .. } =
            parse_command(no_logging).expect("gate logging is optional")
        else {
            panic!("parsed another command")
        };
        assert!(logging.is_none());
        for dry_run in [false, true] {
            let mut suffix = vec![
                "--log-file",
                "/tmp/gate.jsonl",
                "--node",
                "m6-s2",
                "--role",
                "critic",
                "--ref",
                "ref",
                "--evidence",
                "evidence",
                "--required-artifact",
                "/tmp/output.json",
            ];
            if dry_run {
                suffix.push("--dry-run");
            }
            suffix.push("--");
            let command = parse_command(gate_prefix.into_iter().chain(suffix).map(str::to_owned))
                .expect("complete gate logging group");
            let Command::Dispatch { logging, .. } = command else {
                panic!("parsed another command")
            };
            assert!(matches!(
                (dry_run, logging),
                (false, Some(DispatchLoggingMode::Live { .. }))
                    | (true, Some(DispatchLoggingMode::DryRun { .. }))
            ));
        }
        let members = [
            "--log-file",
            "--node",
            "--role",
            "--ref",
            "--evidence",
            "--required-artifact",
        ];
        for missing in members {
            let values = [
                ("--log-file", "/tmp/gate.jsonl"),
                ("--node", "m6-s2"),
                ("--role", "critic"),
                ("--ref", "ref"),
                ("--evidence", "evidence"),
                ("--required-artifact", "/tmp/output.json"),
            ];
            let suffix = values
                .into_iter()
                .filter(|(flag, _)| *flag != missing)
                .flat_map(|(flag, value)| [flag, value])
                .chain(["--"]);
            let error = parse_command(gate_prefix.into_iter().chain(suffix).map(str::to_owned))
                .expect_err("missing gate logging member must fail");
            assert!(
                format!("{error:#}").contains("dispatch logging options must be supplied together"),
                "missing {missing}"
            );
        }
        let stray_dry = gate_prefix
            .into_iter()
            .chain(["--dry-run", "--"])
            .map(str::to_owned);
        let error = parse_command(stray_dry).expect_err("gate dry-run without logging must fail");
        assert!(
            format!("{error:#}").contains("dispatch logging options must be supplied together")
        );

        let relative_artifact = prefix
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
                "--required-artifact",
                "relative/result.json",
                "--",
                "PROMPT",
            ])
            .map(str::to_owned);
        let error = parse_command(relative_artifact).expect_err("relative artifact must fail");
        assert!(
            format!("{error:#}")
                .contains("required artifact path must be absolute: relative/result.json")
        );

        let relative_log = prefix
            .into_iter()
            .chain([
                "--log-file",
                "relative/events.jsonl",
                "--node",
                "m3-s1",
                "--role",
                "step-executor",
                "--ref",
                "abc",
                "--evidence",
                "fixture",
                "--required-artifact",
                "/tmp/output.json",
                "--",
                "PROMPT",
            ])
            .map(str::to_owned);
        let error = parse_command(relative_log).expect_err("relative log must fail");
        assert!(
            format!("{error:#}")
                .contains("logged dispatch event-log path must be absolute: relative/events.jsonl")
        );

        let mismatched_artifact = prefix
            .into_iter()
            .chain([
                "--output-schema",
                "/tmp/schema.json",
                "-o",
                "/tmp/output.json",
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
                "--required-artifact",
                "/tmp/other.json",
                "--",
                "PROMPT",
            ])
            .map(str::to_owned);
        let error = parse_command(mismatched_artifact).expect_err("mismatch must fail");
        assert!(format!("{error:#}").contains(
            "dispatch required artifact path `/tmp/other.json` does not match output path `/tmp/output.json`"
        ));
    }

    #[test]
    fn planning_act_parser_accepts_both_values_for_both_planning_roles() {
        for target in ["codex", "gate"] {
            for (role, act) in [
                ("step-plan-writer", "repeatable"),
                ("step-plan-critic", "irreversible"),
            ] {
                let mut args = vec!["dispatch", target, "--cwd", "/tmp"];
                if target == "codex" {
                    args.extend(["--sandbox", "workspace-write"]);
                } else {
                    args.extend([
                        "--output-schema",
                        "/tmp/schema.json",
                        "-o",
                        "/tmp/output.json",
                    ]);
                }
                args.extend([
                    "--log-file",
                    "/tmp/events.jsonl",
                    "--node",
                    "m5-s1",
                    "--role",
                    role,
                    "--ref",
                    "abc",
                    "--evidence",
                    "fixture",
                    "--required-artifact",
                    "/tmp/output.json",
                    "--planning-act",
                    act,
                    "--",
                    "PROMPT",
                ]);
                parse_command(args.into_iter().map(str::to_owned))
                    .expect("planning act should parse");
            }
        }
    }

    #[test]
    fn planning_act_parser_rejects_exact_invalid_shapes_and_preserves_absence() {
        let prefix = [
            "dispatch",
            "codex",
            "--cwd",
            "/tmp",
            "--sandbox",
            "workspace-write",
        ];
        let logging = [
            "--log-file",
            "/tmp/events.jsonl",
            "--node",
            "m5-s1",
            "--role",
            "step-plan-writer",
            "--ref",
            "abc",
            "--evidence",
            "fixture",
            "--required-artifact",
            "/tmp/output.json",
        ];
        for (suffix, diagnostic) in [
            (
                logging
                    .into_iter()
                    .chain(["--planning-act", "destructive", "--", "Plan the step."])
                    .collect::<Vec<_>>(),
                "unsupported planning act `destructive`; expected `repeatable` or `irreversible`",
            ),
            (
                vec!["--planning-act", "repeatable", "--", "Plan the step."],
                "`--planning-act` requires complete dispatch logging metadata",
            ),
            (
                logging
                    .into_iter()
                    .map(|value| {
                        if value == "step-plan-writer" {
                            "step-executor"
                        } else {
                            value
                        }
                    })
                    .chain(["--planning-act", "repeatable", "--", "Plan the step."])
                    .collect(),
                "planning act is supported only for roles `step-plan-writer` and `step-plan-critic`; rejected role `step-executor`",
            ),
            (
                logging
                    .into_iter()
                    .chain(["--planning-act", "repeatable", "--"])
                    .collect(),
                "planning role `step-plan-writer` requires a non-empty final caller argument to carry its binary-owned frame",
            ),
            (
                logging
                    .into_iter()
                    .chain([
                        "--dry-run",
                        "--planning-act",
                        "repeatable",
                        "--",
                        "Plan the step.",
                    ])
                    .collect(),
                "dispatch arguments require the `--` delimiter",
            ),
            (
                vec![
                    "--log-file",
                    "/tmp/events.jsonl",
                    "--node",
                    "m5-s1",
                    "--role",
                    "step-plan-writer",
                    "--ref",
                    "abc",
                    "--planning-act",
                    "repeatable",
                    "--evidence",
                    "fixture",
                    "--",
                    "Plan the step.",
                ],
                "dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence, --required-artifact",
            ),
        ] {
            let error = parse_command(prefix.into_iter().chain(suffix).map(str::to_owned))
                .expect_err("planning act shape must fail");
            assert!(format!("{error:#}").contains(diagnostic));
        }

        let Command::Dispatch { envelope, .. } = parse_command(
            prefix
                .into_iter()
                .chain(logging)
                .chain(["--", "Plan the step."])
                .map(str::to_owned),
        )
        .expect("absence preserves dispatch") else {
            panic!("parsed another command")
        };
        assert_eq!(envelope.arguments().as_slice(), ["Plan the step."]);
    }

    #[test]
    fn terminal_observation_ignores_additive_usage_and_non_object_json() {
        let observation = observe_terminal_line(br#"{"type":"turn.completed","usage":{"total_tokens":146,"input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5}}"#);
        assert_eq!(
            classify_codex_terminal_usage(
                &[observation],
                DispatchExitStatus::Exited {
                    code: ExitCode::new(0)
                }
            ),
            Ok(DispatchTokenUsage::Measured {
                input_tokens: InputTokens::new(101),
                cached_input_tokens: CachedInputTokens::new(23),
                output_tokens: OutputTokens::new(17),
                reasoning_output_tokens: ReasoningOutputTokens::new(5),
            })
        );
        for fixture in [b"5".as_slice(), b"\"x\"".as_slice(), b"[]".as_slice()] {
            assert_eq!(
                observe_terminal_line(fixture),
                CodexTerminalObservation::NonTerminal
            );
        }
    }

    #[test]
    fn gate_exec_parser_accepts_only_the_exact_helper_command() {
        assert!(matches!(
            parse_command(["gate", "exec"].into_iter().map(str::to_owned))
                .expect("exact helper command"),
            super::Command::GateExec
        ));
        let error = parse_command(["gate", "exec", "extra"].into_iter().map(str::to_owned))
            .expect_err("extra helper argument");
        assert_eq!(error.to_string(), super::USAGE);
        assert!(super::USAGE.contains("\n       pce gate exec\n"));
        assert!(super::USAGE.ends_with(concat!(
            "       pce gate exec\n",
            "       pce gate replay --repo-root <ABSOLUTE_REPOSITORY_ROOT> --evidence <ABSOLUTE_EVIDENCE_PATH> --execution-ref <EXECUTION_REF> --broken-ref <REF> --repaired-ref <REF> --schema <REPOSITORY_RELATIVE_SCHEMA_PATH> --output <REPOSITORY_RELATIVE_OUTPUT_PATH> --expected <conforming-verdict|nonconforming-verdict>\n",
            "       pce gate execution-subject-probe --output <REPOSITORY_RELATIVE_OUTPUT_PATH>\n",
            "       pce gate paired-execution-proof --repo-root <ABSOLUTE_REPOSITORY_ROOT> --artifacts <ABSOLUTE_EMPTY_DIRECTORY> --env <NAME=VALUE> --env <NAME=VALUE> --env <NAME=VALUE>\n",
            "       --worker-env forwards that named driver variable only to package and gate worker workspaces; criteria and --prepare commands continue to inherit the driver's full launch environment."
        )));
    }

    #[test]
    fn gate_replay_parser_is_exact_ordered_and_typed() {
        let arguments = [
            "gate",
            "replay",
            "--repo-root",
            "/tmp/repository",
            "--evidence",
            "/tmp/evidence.json",
            "--execution-ref",
            "execution-000001",
            "--broken-ref",
            "broken",
            "--repaired-ref",
            "repaired",
            "--schema",
            "schemas/verdict.json",
            "--output",
            "verdict.json",
            "--expected",
            "conforming-verdict",
        ];
        assert!(matches!(
            parse_command(arguments.into_iter().map(str::to_owned)).expect("replay command"),
            super::Command::GateReplay(_)
        ));
        let mut reordered = arguments;
        reordered.swap(2, 4);
        assert!(parse_command(reordered.into_iter().map(str::to_owned)).is_err());
        let mut extra = arguments.to_vec();
        extra.push("extra");
        assert!(parse_command(extra.into_iter().map(str::to_owned)).is_err());
    }

    #[test]
    fn paired_replay_classification_keeps_no_repair_signal_out_of_repair_sensitive() {
        assert_eq!(
            super::replay_classification(br#"{"classification":"no-repair-signal"}"#)
                .expect("known replay classification"),
            pce_core::PairedReplayClassification::NoRepairSignal
        );
        assert_eq!(
            super::replay_classification(br#"{"classification":"repair-sensitive"}"#)
                .expect("known replay classification"),
            pce_core::PairedReplayClassification::RepairSensitive
        );
    }

    #[test]
    fn gate_replay_bounds_diagnostics_and_cleanup_targets_are_stable() {
        assert_eq!(super::REPLAY_GIT_TIMEOUT, Duration::from_secs(5));
        assert_eq!(
            super::REPLAY_RUN_INACTIVITY_TIMEOUT,
            Duration::from_secs(5 * 60)
        );
        assert_eq!(
            super::REPLAY_RUN_OVERALL_TIMEOUT,
            Duration::from_secs(15 * 60)
        );
        assert_eq!(super::REPLAY_OVERALL_TIMEOUT, Duration::from_secs(40 * 60));
        assert_eq!(super::REPLAY_PAUSE_TIMEOUT, Duration::from_secs(10));
        let parent = Path::new("/private/tmp/pce-gate-replay-test");
        assert!(super::validate_replay_cleanup_target(parent, &parent.join("checkout")).is_ok());
        for rejected in [
            parent.to_path_buf(),
            parent.join("unknown"),
            parent.join("nested/checkout"),
            PathBuf::from("/private/tmp/other/checkout"),
        ] {
            assert_eq!(
                super::validate_replay_cleanup_target(parent, &rejected)
                    .expect_err("broad cleanup target")
                    .to_string(),
                "gate replay cleanup failed"
            );
        }
    }

    #[test]
    fn replay_schedule_covers_every_assignment_and_balances_each_position() {
        let mut distinct = std::collections::BTreeSet::new();
        let mut broken_by_position = [0_usize; 4];
        for fourth in 0_u8..4 {
            for third in 0_u8..3 {
                for second in 0_u8..2 {
                    let mut random = std::io::Cursor::new([fourth, third, second]);
                    let schedule = super::create_replay_schedule_from(&mut random)
                        .expect("deterministic schedule");
                    let spelling = schedule
                        .iter()
                        .map(|side| match side {
                            super::ReplaySide::Broken => 'B',
                            super::ReplaySide::Repaired => 'R',
                        })
                        .collect::<String>();
                    distinct.insert(spelling);
                    for (position, side) in schedule.iter().enumerate() {
                        if *side == super::ReplaySide::Broken {
                            broken_by_position[position] += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(distinct.len(), 6);
        assert_eq!(broken_by_position, [12, 12, 12, 12]);
    }

    #[test]
    fn production_replay_schedule_uses_operating_system_randomness() {
        let first = super::create_replay_schedule().expect("production replay schedule");
        assert!((0..63).any(|_| {
            super::create_replay_schedule().expect("production replay schedule") != first
        }));
    }

    #[test]
    fn gate_replay_report_has_closed_order_and_omits_parent_inputs() {
        let requested = pce_core::NamedReplayRef::parse("broken-secret").expect("replay ref");
        let broken = pce_core::ReplayRefResult::CheckoutFailed {
            requested_ref: requested.clone(),
            checkout_failed: pce_core::CheckoutFailure {
                stage: pce_core::CheckoutStage::ResolveRef,
                diagnostic: "failed to resolve replay ref `broken-secret` to a commit".to_owned(),
            },
        };
        let repaired = broken.clone();
        let reference = pce_core::GateExecutionRef::parse("execution-000001").expect("reference");
        let report = super::serialize_replay_report(
            &reference,
            &broken,
            &repaired,
            pce_core::RepairSensitivity::CheckoutFailed,
        )
        .expect("report");
        let text = String::from_utf8(report).expect("report UTF-8");
        assert!(text.starts_with("{\"schema_id\":\"pce.gate-replay-report\",\"schema_version\":1,\"execution_ref\":\"execution-000001\",\"broken\":"));
        assert!(text.ends_with("\"classification\":\"checkout-failed\"}\n"));
        for absent in [
            "conforming-verdict",
            "schemas/verdict.json",
            "verdict.json",
            "PCE_REPLAY_PAUSE_AFTER_RESOLVE",
            "/private/tmp/pce-gate-replay-",
        ] {
            assert!(!text.contains(absent), "report leaked {absent}");
        }
    }

    #[test]
    fn gate_execution_deadline_diagnostics_are_exact() {
        assert_eq!(super::GATE_REQUEST_READ_TIMEOUT, Duration::from_secs(2));
        assert_eq!(super::GATE_RESPONSE_WRITE_TIMEOUT, Duration::from_secs(1));
        assert_eq!(
            super::GATE_EXECUTION_INACTIVITY_TIMEOUT,
            Duration::from_secs(5 * 60)
        );
        assert_eq!(
            super::GATE_EXECUTION_OVERALL_TIMEOUT,
            Duration::from_secs(15 * 60)
        );
        assert_eq!(
            super::GATE_PROCESS_TERMINATION_TIMEOUT,
            Duration::from_secs(1)
        );
        assert_eq!(
            super::GATE_OUTPUT_DRAIN_INACTIVITY_TIMEOUT,
            Duration::from_secs(30)
        );
        assert_eq!(
            super::GATE_OUTPUT_DRAIN_OVERALL_TIMEOUT,
            Duration::from_secs(2 * 60)
        );
        assert_eq!(super::GATE_ACCEPT_POLL_INTERVAL, Duration::from_millis(10));
        assert_eq!(super::GATE_SERVER_SHUTDOWN_TIMEOUT, Duration::from_secs(12));
        assert_eq!(super::GATE_MAX_CONNECTION_WORKERS, 32);
        assert_eq!(super::GATE_MAX_REQUEST_BYTES, 16 * 1024 * 1024);
        assert_eq!(
            super::gate_request_timeout_diagnostic(),
            "gate execution request read timed out after 2 seconds"
        );
        assert_eq!(
            super::gate_request_too_large_diagnostic(),
            "gate execution request exceeds 16777216 bytes"
        );
        assert_eq!(
            super::gate_response_timeout_diagnostic(),
            "gate execution response write timed out after 1 second"
        );
        assert_eq!(
            super::gate_process_termination_diagnostic(),
            "gate execution process did not terminate within 1 second after kill"
        );
        assert_eq!(
            super::gate_output_drain_diagnostic(super::GateOutputDrainTimeout::Inactivity),
            "gate execution output drain inactive for 30 seconds"
        );
        assert_eq!(
            super::gate_output_drain_diagnostic(super::GateOutputDrainTimeout::Overall),
            "gate execution output drain exceeded 2 minute overall cap"
        );
        assert_eq!(
            super::gate_recorder_stopping_diagnostic(),
            "gate execution recorder is stopping"
        );
        assert_eq!(
            super::gate_worker_shutdown_diagnostic(),
            "gate execution worker exceeded 12 second shutdown deadline"
        );
    }

    fn gate_process_fixture(program: &str, arguments: &[&str]) -> pce_core::GateProcessStimulus {
        let request = serde_json::to_vec(&json!({
            "working_directory": "/tmp",
            "setup": [],
            "command": {
                "program": program,
                "arguments": arguments,
                "input": [],
                "environment": {}
            }
        }))
        .expect("gate process fixture should serialize");
        pce_core::parse_gate_stimulus(&request)
            .expect("gate process fixture should parse")
            .command()
            .clone()
    }

    fn gate_stimulus_fixture(program: &str, arguments: &[&str]) -> pce_core::GateStimulus {
        let request = serde_json::to_vec(&json!({
            "working_directory": "/tmp",
            "setup": [],
            "command": {
                "program": program,
                "arguments": arguments,
                "input": [],
                "environment": {}
            }
        }))
        .expect("gate stimulus fixture should serialize");
        pce_core::parse_gate_stimulus(&request).expect("gate stimulus fixture should parse")
    }

    fn short_gate_process_bounds() -> super::GateProcessBounds {
        super::GateProcessBounds {
            execution_inactivity: Duration::from_millis(60),
            execution_overall: Duration::from_millis(180),
            output_drain_inactivity: Duration::from_millis(60),
            output_drain_overall: Duration::from_millis(180),
        }
    }

    fn execute_gate_process_with_test_bounds(
        stimulus: &pce_core::GateProcessStimulus,
        bounds: super::GateProcessBounds,
    ) -> super::GateProcessExecution {
        super::GATE_PROCESS_TEST_BOUNDS.with(|configured| {
            let previous = configured.replace(Some(bounds));
            let execution = super::execute_gate_process_until(Path::new("/tmp"), stimulus, None);
            configured.set(previous);
            execution
        })
    }

    #[test]
    fn production_process_entry_terminates_an_inactive_execution() {
        let stimulus = gate_process_fixture("/bin/sleep", &["0.30"]);
        let started = std::time::Instant::now();
        let execution =
            execute_gate_process_with_test_bounds(&stimulus, short_gate_process_bounds());
        assert!(started.elapsed() < Duration::from_millis(180));
        assert_eq!(
            execution.observation.map(|observation| observation.status),
            Some(pce_core::GateTerminalStatus::Signaled { signal: 9 })
        );
    }

    #[test]
    fn production_process_entry_caps_a_continuously_active_execution() {
        let stimulus = gate_process_fixture(
            "/bin/sh",
            &[
                "-c",
                "i=0; while [ $i -lt 50 ]; do echo x; /bin/sleep 0.01; i=$((i+1)); done",
            ],
        );
        let mut bounds = short_gate_process_bounds();
        bounds.execution_inactivity = Duration::from_millis(100);
        let execution = execute_gate_process_with_test_bounds(&stimulus, bounds);
        let observation = execution.observation.expect("execution observation");
        assert!(matches!(
            observation.status,
            pce_core::GateTerminalStatus::Signaled { .. }
        ));
        assert!(!observation.stdout.is_empty());
    }

    #[test]
    fn production_replay_entry_terminates_an_inactive_execution() {
        let stimulus = gate_stimulus_fixture("/bin/sleep", &["0.30"]);
        let started = std::time::Instant::now();
        let execution = super::REPLAY_PROCESS_TEST_BOUNDS.with(|configured| {
            let previous = configured.replace(Some(short_gate_process_bounds()));
            let execution = super::execute_replay_stimulus_until(
                &stimulus,
                std::time::Instant::now() + Duration::from_secs(1),
            );
            configured.set(previous);
            execution
        });
        assert!(started.elapsed() < Duration::from_millis(180));
        assert!(matches!(
            execution
                .observed_result
                .and_then(|result| result.command)
                .map(|observation| observation.status),
            Some(pce_core::GateTerminalStatus::Signaled { .. })
        ));
    }

    #[test]
    fn production_replay_entry_caps_a_continuously_active_execution() {
        let stimulus = gate_stimulus_fixture(
            "/bin/sh",
            &[
                "-c",
                "i=0; while [ $i -lt 50 ]; do echo x; /bin/sleep 0.01; i=$((i+1)); done",
            ],
        );
        let mut bounds = short_gate_process_bounds();
        bounds.execution_inactivity = Duration::from_millis(100);
        let execution = super::REPLAY_PROCESS_TEST_BOUNDS.with(|configured| {
            let previous = configured.replace(Some(bounds));
            let execution = super::execute_replay_stimulus_until(
                &stimulus,
                std::time::Instant::now() + Duration::from_secs(1),
            );
            configured.set(previous);
            execution
        });
        let observation = execution
            .observed_result
            .and_then(|result| result.command)
            .expect("replay execution observation");
        assert!(matches!(
            observation.status,
            pce_core::GateTerminalStatus::Signaled { .. }
        ));
        assert!(!observation.stdout.is_empty());
    }

    #[test]
    fn production_process_entry_detects_inactive_output_drain() {
        let stimulus = gate_process_fixture("/bin/sh", &["-c", "/bin/sleep 0.30 &"]);
        let mut bounds = short_gate_process_bounds();
        bounds.execution_inactivity = Duration::from_millis(500);
        bounds.execution_overall = Duration::from_millis(500);
        let execution = execute_gate_process_with_test_bounds(&stimulus, bounds);
        assert_eq!(
            execution.diagnostic.as_deref(),
            Some("gate execution output drain inactive for 30 seconds")
        );
    }

    fn process_is_gone(pid: &str) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while std::time::Instant::now() < deadline {
            let status = std::process::Command::new("/bin/kill")
                .args(["-0", pid])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("process liveness probe should run");
            if !status.success() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn production_process_entry_reaps_the_inactive_process_group() {
        let stimulus =
            gate_process_fixture("/bin/sh", &["-c", "sleep 30 & child=$!; echo $child; wait"]);
        let execution =
            execute_gate_process_with_test_bounds(&stimulus, short_gate_process_bounds());
        let observation = execution
            .observation
            .expect("inactive execution observation");
        assert_eq!(
            observation.status,
            pce_core::GateTerminalStatus::Signaled { signal: 9 }
        );
        let pid = String::from_utf8(observation.stdout)
            .expect("child pid should be UTF-8")
            .trim()
            .to_owned();
        assert!(process_is_gone(&pid), "inactive descendant {pid} survived");
    }

    #[test]
    fn production_process_entry_reaps_the_inactive_output_drain_group() {
        let stimulus = gate_process_fixture("/bin/sh", &["-c", "sleep 30 & child=$!; echo $child"]);
        let mut bounds = short_gate_process_bounds();
        bounds.execution_inactivity = Duration::from_millis(500);
        bounds.execution_overall = Duration::from_millis(500);
        let execution = execute_gate_process_with_test_bounds(&stimulus, bounds);
        assert_eq!(
            execution.diagnostic.as_deref(),
            Some("gate execution output drain inactive for 30 seconds")
        );
        let observation = execution.observation.expect("drain execution observation");
        assert_eq!(
            observation.status,
            pce_core::GateTerminalStatus::Exited { code: 0 }
        );
        let pid = String::from_utf8(observation.stdout)
            .expect("child pid should be UTF-8")
            .trim()
            .to_owned();
        assert!(process_is_gone(&pid), "drain descendant {pid} survived");
    }

    #[test]
    fn recorder_kills_and_records_an_inactive_process_group_then_cleans_up() {
        let directory = tempdir().expect("temporary recorder directory should create");
        let verdict = directory.path().join("verdict.json");
        let evidence = pce_core::AbsoluteGateExecutionEvidencePath::from_verdict_path(&verdict);
        let socket = pce_core::AbsoluteGateExecutionSocketPath::construct(
            directory.path(),
            std::process::id(),
            2,
        )
        .expect("socket path should construct");
        let client = pce_core::AbsoluteGateExecClientPath::parse(
            std::env::current_exe().expect("current executable should resolve"),
        )
        .expect("client path should parse");
        let config = pce_core::GateExecutionRecorderConfig::new(client, evidence, socket.clone());
        let runtime =
            super::GateRecorderRuntime::start_with_bounds(&config, short_gate_process_bounds())
                .expect("bounded recorder should start");
        let request = serde_json::to_vec(&json!({
            "working_directory": directory.path(),
            "setup": [],
            "command": {
                "program": "/bin/sh",
                "arguments": ["-c", "/bin/sleep 30 & child=$!; echo $child; wait"],
                "input": [],
                "environment": {}
            }
        }))
        .expect("bounded recorder request should serialize");
        let mut stream = std::os::unix::net::UnixStream::connect(socket.as_path())
            .expect("bounded recorder client should connect");
        std::io::Write::write_all(&mut stream, &request)
            .expect("bounded recorder request should write");
        stream
            .shutdown(std::net::Shutdown::Write)
            .expect("bounded recorder request should finish");
        let mut response = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut response)
            .expect("bounded recorder response should read");
        let response: serde_json::Value =
            serde_json::from_slice(&response).expect("bounded recorder response should parse");
        assert_eq!(
            response["observed_result"]["command"]["status"],
            json!({"kind":"signaled","signal":9})
        );
        let pid = response["observed_result"]["command"]["stdout"]
            .as_array()
            .expect("recorded stdout should be bytes")
            .iter()
            .map(|byte| byte.as_u64().expect("stdout byte") as u8)
            .collect::<Vec<_>>();
        let pid = String::from_utf8(pid)
            .expect("recorded child pid should be UTF-8")
            .trim()
            .to_owned();
        let stopped = runtime.stop();
        assert!(stopped.error.is_none());
        assert_eq!(stopped.records.len(), 1);
        assert!(!socket.as_path().exists());
        assert!(process_is_gone(&pid), "recorded descendant {pid} survived");
    }

    #[test]
    fn production_process_entry_caps_continuously_active_output_drain() {
        let stimulus = gate_process_fixture(
            "/bin/sh",
            &[
                "-c",
                "(i=0; while [ $i -lt 50 ]; do echo x; /bin/sleep 0.01; i=$((i+1)); done) &",
            ],
        );
        let mut bounds = short_gate_process_bounds();
        bounds.execution_inactivity = Duration::from_millis(500);
        bounds.execution_overall = Duration::from_millis(500);
        bounds.output_drain_inactivity = Duration::from_millis(100);
        let execution = execute_gate_process_with_test_bounds(&stimulus, bounds);
        assert_eq!(
            execution.diagnostic.as_deref(),
            Some("gate execution output drain exceeded 2 minute overall cap")
        );
        assert!(
            execution
                .observation
                .is_some_and(|observation| !observation.stdout.is_empty())
        );
    }

    #[test]
    fn gate_execution_recorder_reclaims_stale_socket_endpoint() {
        let directory = tempdir().expect("temporary directory should create");
        let verdict = directory.path().join("verdict.json");
        let evidence = pce_core::AbsoluteGateExecutionEvidencePath::from_verdict_path(&verdict);
        let socket = pce_core::AbsoluteGateExecutionSocketPath::construct(
            directory.path(),
            std::process::id(),
            1,
        )
        .expect("socket path should construct");
        let stale_listener = std::os::unix::net::UnixListener::bind(socket.as_path())
            .expect("stale socket should bind");
        drop(stale_listener);
        assert!(socket.as_path().exists());
        let client = pce_core::AbsoluteGateExecClientPath::parse(
            std::env::current_exe().expect("current executable should resolve"),
        )
        .expect("client path should parse");
        let config = pce_core::GateExecutionRecorderConfig::new(client, evidence, socket.clone());

        let runtime = super::GateRecorderRuntime::start(&config)
            .expect("stale socket endpoint should be reclaimed");
        assert!(socket.as_path().exists());
        let stopped = runtime.stop();
        assert!(stopped.error.is_none());
        assert!(!socket.as_path().exists());
    }

    #[test]
    fn paired_dispatch_parser_is_strict_and_requires_exact_environment() {
        let directory = tempdir().expect("temporary directory should create");
        let root = directory.path().display().to_string();
        let valid = [
            "gate",
            "paired-execution-proof",
            "--repo-root",
            "/repository",
            "--artifacts",
            &root,
            "--env",
            "PATH=/bin",
            "--env",
            "HOME=/home/operator",
            "--env",
            "USER=operator",
        ];
        assert!(super::parse_command(valid.into_iter().map(str::to_owned)).is_ok());
        for invalid in [
            ["PATH=/bin", "HOME=/home/operator", "HOME=duplicate"],
            ["PATH=/bin", "HOME=/home/operator", "SHELL=/bin/sh"],
        ] {
            let args = [
                "gate",
                "paired-execution-proof",
                "--repo-root",
                "/repository",
                "--artifacts",
                &root,
                "--env",
                invalid[0],
                "--env",
                invalid[1],
                "--env",
                invalid[2],
            ];
            let error = super::parse_command(args.into_iter().map(str::to_owned))
                .expect_err("invalid paired environment must fail");
            assert_eq!(
                error.to_string(),
                "paired execution proof requires exactly PATH, HOME, and USER"
            );
        }
    }

    #[test]
    fn paired_constants_and_direct_replay_argv_are_exact() {
        assert_eq!(
            super::PAIRED_BROKEN_REF,
            "a8a87cb44f84988fa61904bfba48614401482851"
        );
        assert_eq!(
            super::PAIRED_BROKEN_OID,
            "a8a87cb44f84988fa61904bfba48614401482851"
        );
        assert_eq!(
            super::PAIRED_REPAIRED_REF,
            "cbe499ef94864d223518ad328db2a396551fa85b"
        );
        assert_eq!(
            super::PAIRED_REPAIRED_OID,
            "cbe499ef94864d223518ad328db2a396551fa85b"
        );
        assert!(
            super::PAIRED_CRITIC_TASK
                .contains("arguments are [\"gate\",\"execution-subject-probe\"")
        );
        let critic_task = super::paired_critic_task();
        assert!(critic_task.contains(&format!(
            "<verdict-schema>\n{}</verdict-schema>",
            super::VERDICT_SCHEMA
        )));
        for hidden_or_side_specific in [
            super::PAIRED_BROKEN_REF,
            super::PAIRED_REPAIRED_REF,
            super::PAIRED_REPLAY_EXPECTED,
            "repair-sensitive",
            "approval",
            "broken",
            "repaired",
        ] {
            assert!(!critic_task.contains(hidden_or_side_specific));
        }
        let reference =
            pce_core::GateExecutionRef::parse("execution-000001").expect("reference should parse");
        let argv = super::paired_replay_arguments(
            Path::new("/checkout"),
            Path::new("/artifacts/evidence.json"),
            &reference,
        );
        assert_eq!(argv[0], "gate");
        assert_eq!(argv[1], "replay");
        assert!(
            !argv
                .iter()
                .any(|argument| argument == "sh" || argument == "-c")
        );
        assert_eq!(argv.last(), Some(&OsString::from("conforming-verdict")));
    }

    #[test]
    fn paired_report_key_order_is_stable_and_redacted() {
        let report = super::PairedProofReport {
            schema_id: "pce.paired-execution-proof",
            schema_version: 1,
            broken_ref: super::PAIRED_BROKEN_OID,
            repaired_ref: super::PAIRED_REPAIRED_OID,
            broken_verdict: "BLOCK",
            broken_blocking_issue_count: 1,
            broken_witnesses: vec!["execution-000001".to_owned()],
            repaired_verdict: "APPROVE",
            repaired_blocking_issue_count: 0,
            repaired_probes: vec!["execution-000001".to_owned()],
            decision: "APPROVE",
        };
        let bytes = super::serialize_paired_proof_report(&report)
            .expect("report serialization should succeed");
        let text = String::from_utf8(bytes).expect("report should be UTF-8");
        assert!(text.starts_with(
            "{\"schema_id\":\"pce.paired-execution-proof\",\"schema_version\":1,\"broken_ref\":"
        ));
        assert!(text.ends_with("\"decision\":\"APPROVE\"}\n"));
        for secret in [
            "HOME=",
            "USER=",
            "PATH=",
            "/private/tmp",
            "conforming-verdict",
        ] {
            assert!(!text.contains(secret));
        }
    }

    #[test]
    fn paired_probe_cleanup_rejects_nonordinary_targets() {
        let directory = tempdir().expect("temporary directory should create");
        let child = directory.path().join("child");
        std::fs::create_dir(&child).expect("child directory should create");
        let error = super::remove_probe_file(&child, directory.path())
            .expect_err("directory target must be rejected");
        assert_eq!(
            error.to_string(),
            "execution subject probe output must be a repository-relative ordinary path"
        );
    }

    #[test]
    fn paired_critics_receive_sequential_single_commit_repositories_and_probe_only_programs() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut campaign = super::PairedWorktrees::create(source_root)
            .expect("paired repository should materialize");
        assert!(!campaign.parent.exists());
        assert!(!campaign.repository_root.exists());
        let broken = campaign
            .add("broken/artifact", super::PAIRED_BROKEN_OID)
            .expect("broken side should materialize");
        super::materialize_paired_probe(&broken).expect("probe should materialize");
        let rev_list = std::process::Command::new("git")
            .args(["-C"])
            .arg(&broken)
            .args(["rev-list", "--all", "--count"])
            .output()
            .expect("single-side rev-list should run");
        assert!(rev_list.status.success());
        assert_eq!(rev_list.stdout, b"1\n");
        let parent = broken.parent().expect("checkout has an isolated parent");
        assert_eq!(
            std::fs::read_dir(parent)
                .expect("isolated parent should be readable")
                .count(),
            1
        );
        let probe = std::fs::read(broken.join(super::PAIRED_PROGRAM_RELATIVE))
            .expect("probe bytes should be readable");
        let probe = String::from_utf8(probe).expect("probe should be UTF-8");
        for campaign_secret in [
            super::PAIRED_BROKEN_OID,
            super::PAIRED_REPAIRED_OID,
            "repair-sensitive",
            "conforming-verdict",
            "paired-execution-proof",
            super::PAIRED_CRITIC_TASK,
        ] {
            assert!(!probe.contains(campaign_secret));
        }
        campaign
            .remove(&broken)
            .expect("broken checkout should be removed before repaired dispatch");
        assert!(!broken.exists());
        let repaired = campaign
            .add("repaired/artifact", super::PAIRED_REPAIRED_OID)
            .expect("repaired side should materialize");
        assert!(!broken.exists());
        assert!(!campaign.parent.exists());
        assert!(!campaign.repository_root.exists());
        campaign
            .remove(&repaired)
            .expect("repaired checkout should be removed after dispatch");
        campaign
            .materialize_replay_repository()
            .expect("two-ref replay repository should materialize after both critics exit");
        assert!(campaign.repository_root.exists());
        campaign.cleanup().expect("campaign should clean up");
    }

    #[test]
    fn exceptional_step_snapshot_variant_is_schema_valid_and_exact() {
        let declaration = parse_event_line(r#"{"sequence":1,"timestamp":"2026-08-09T12:00:00.000Z","kind":"exceptional-merge-chain-declared","node":"m1-s3","payload":{"integration_branch":"pce/a-dispatch-outlives-the-call-that-started-it/milestone-1b","step_pull_request_number":179,"promotion_pull_request_number":180,"evidence":"gh pr view 179 --json number,headRefName,baseRefName,state,mergeCommit && gh pr view 180 --json number,headRefName,baseRefName,state,mergeCommit"}}"#).expect("typed declaration");
        assert!(matches!(
            declaration.body_ref(),
            EventBodyRef::Known(KnownPayload::ExceptionalMergeChainDeclared(_))
        ));
        let vision = VisionSlug::parse("2026-08-09-a-dispatch-outlives-the-call-that-started-it")
            .expect("vision");
        let node = NodeId::parse("m1-s3").expect("node");
        let chain = ExceptionalMergeChain::new(
            &vision,
            StepNode::parse(&node).expect("step"),
            pce_core::DeclaredIntegrationBranch::parse(
                "pce/a-dispatch-outlives-the-call-that-started-it/milestone-1b",
            )
            .expect("branch"),
            PullRequestNumber::parse(179).expect("step PR"),
            PullRequestNumber::parse(180).expect("promotion PR"),
        )
        .expect("chain");
        let hop = |identity: ExactPullRequestIdentity, oid: &str| {
            let squash = SquashCommitOid::parse(oid).expect("squash");
            PullRequestAuthorityObservation::new(
                GitHubAuthorityObservation::Reachable {
                    observation: GitHubPullRequestObservation::OneExactMatch {
                        identity,
                        state: ExactPullRequestState::Merged {
                            squash_commit: squash.clone(),
                        },
                    },
                },
                GitAuthorityObservation::Reachable {
                    observation: GitMergeObservation::SquashCommitReachable {
                        squash_commit: squash,
                    },
                },
            )
        };
        let observation = ExceptionalMergeChainObservation::new(
            hop(
                chain.step_pull_request().clone(),
                "798ca422c9576983c8be362481169632952c578b",
            ),
            hop(
                chain.promotion_pull_request().clone(),
                "e9a3eed73d6852ca9652b1af083127ba11126cc1",
            ),
        );
        let criteria = parse_acceptance_criteria("# Vision\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"x\",\"input\":\"x\",\"observation\":\"x\"}]}\n```\n").expect("criteria");
        let state = derive_run_state_with_exceptional_merge_chains(
            &[declaration],
            &criteria,
            &vision,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
            &[(node, observation)],
        )
        .expect("exceptional state");
        let snapshot = RunSnapshot::from(&state);
        let value =
            validated_snapshot_value(&snapshot).expect("run snapshot failed schema validation");
        let member = &value["steps"][0];
        assert_eq!(member["node"], "m1-s3");
        assert_eq!(
            member["subject"]["integration_branch"],
            "pce/a-dispatch-outlives-the-call-that-started-it/milestone-1b"
        );
        assert_eq!(
            member["exceptional_merge_chain"]["step_pull_request_number"],
            179
        );
        assert_eq!(
            member["exceptional_merge_chain"]["promotion_pull_request_number"],
            180
        );
        assert_eq!(
            member["exceptional_merge_chain"]["step_to_integration"]["merge_status"],
            "merged"
        );
        assert_eq!(
            member["exceptional_merge_chain"]["integration_to_default"]["merge_status"],
            "merged"
        );
        assert_eq!(member["merge_status"], "merged");
        assert_eq!(member.as_object().expect("step object").len(), 4);
    }

    #[test]
    fn codex_dialect_rejects_object_properties_omitted_from_required() {
        let directory = tempfile::tempdir().expect("schema fixture directory");
        let path = directory.path().join("invalid.schema.json");
        fs::write(
            &path,
            br#"{"type":"object","properties":{"kept":{"type":"string"},"omitted":{"type":"string"}},"required":["kept"],"additionalProperties":false}"#,
        )
        .expect("write schema fixture");

        let error =
            crate::validate_codex_output_schema(&path).expect_err("dialect mismatch must fail");
        assert!(
            format!("{error:#}").contains("required must contain every and only property name")
        );
    }

    #[test]
    fn herdr_shell_only_process_info_is_dead_evidence() {
        let shell = serde_json::json!({
            "result": {"process_info": {
                "shell_pid": 41,
                "foreground_processes": [{"pid": 41, "name": "zsh", "argv": ["-zsh"]}]
            }}
        });
        assert!(crate::foreground_is_only_shell(&shell));
        let worker = serde_json::json!({
            "result": {"process_info": {
                "shell_pid": 41,
                "foreground_processes": [{"pid": 42, "name": "pce", "argv": ["pce", "package"]}]
            }}
        });
        assert!(!crate::foreground_is_only_shell(&worker));
        assert!(crate::process_observation_matches(
            &pce_core::DispatchWorkerProcessObservation::Observed {
                process_id: 42,
                name: "pce".to_owned(),
                argv: vec!["pce".to_owned()],
            },
            &worker,
        ));
    }

    #[test]
    fn package_tmpdir_is_attempt_scoped_and_leaves_darwin_socket_headroom() {
        let vision = pce_core::DispatchVisionSource::parse(
            "2026-08-11-the-store-is-the-only-copy".to_owned(),
        )
        .expect("vision");
        let graph = pce_core::parse_work_package_graph(include_bytes!(
            "../crates/core/tests/data/rivretrieve-work-package-graph.json"
        ))
        .expect("graph");
        let package = graph
            .packages()
            .iter()
            .find(|package| package.id().as_str() == "RR2")
            .expect("RR2");
        let path = crate::package_temporary_directory(
            &vision,
            package.id(),
            pce_core::DispatchAttempt::parse(1).expect("attempt"),
        );
        let retry_path = crate::package_temporary_directory(
            &vision,
            package.id(),
            pce_core::DispatchAttempt::parse(2).expect("attempt"),
        );
        assert_ne!(path, retry_path);
        let rendered = path.to_str().expect("ASCII temporary path");
        assert!(rendered.starts_with("/tmp/pce-tmp/"));
        assert_eq!(rendered.len(), 25);
        assert!(rendered[13..].bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert!(104_usize - rendered.len() >= 40);
    }

    #[test]
    fn synthesized_tmpdir_uses_binary_owned_fixed_root() {
        use std::os::unix::fs::PermissionsExt;

        let directory = crate::SynthesizedTemporaryDirectory::create()
            .expect("binary-owned TMPDIR should create");
        assert!(directory.path().starts_with("/tmp"));
        assert_eq!(
            fs::metadata(directory.path())
                .expect("TMPDIR metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    fn worker_environment_command(
        journal_path: PathBuf,
        names: &[(&str, &str)],
    ) -> crate::DriverRunCommand {
        crate::DriverRunCommand {
            graph_path: journal_path.with_file_name("graph.json"),
            journal_path,
            repositories: Vec::new(),
            preparations: std::collections::BTreeMap::new(),
            worker_environment: names
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        }
    }

    fn plan_advanced_event() -> pce_core::DriverEvent {
        pce_core::DriverEvent::PlanVersionAdvanced {
            from_plan_version: 6,
            to_plan_version: 7,
            carried_completions: Vec::new(),
            carried_amendments: Vec::new(),
            criterion_revisions_ratified_by: None,
            criterion_revisions: Vec::new(),
        }
    }

    #[test]
    fn worker_environment_strict_superset_is_recorded_at_plan_boundary() {
        let directory = tempdir().expect("temporary directory");
        let journal_path = directory.path().join("events.jsonl");
        let events = vec![
            pce_core::DriverEvent::WorkerEnvironmentDeclared { names: Vec::new() },
            plan_advanced_event(),
        ];
        let command = worker_environment_command(
            journal_path.clone(),
            &[
                ("POURPOINT_LIVE_READ_AUTHORIZATION", "authorized"),
                ("POURPOINT_RELEASE_WHEEL", "/tmp/pourpoint.whl"),
            ],
        );

        crate::ensure_worker_environment_contract(&command, &events)
            .expect("a strict superset must extend at the plan boundary");

        let line = fs::read_to_string(&journal_path).expect("extension event");
        let event: serde_json::Value = serde_json::from_str(line.trim()).expect("event JSON");
        assert_eq!(event["event"], "worker-environment-extended");
        assert_eq!(event["plan_version"], 7);
        assert_eq!(
            event["added_names"],
            json!([
                "POURPOINT_LIVE_READ_AUTHORIZATION",
                "POURPOINT_RELEASE_WHEEL"
            ])
        );
        let routed = crate::route_environment(&command).expect("routed environment");
        assert_eq!(
            routed
                .get("POURPOINT_LIVE_READ_AUTHORIZATION")
                .map(String::as_str),
            Some("authorized")
        );
        assert_eq!(
            routed.get("POURPOINT_RELEASE_WHEEL").map(String::as_str),
            Some("/tmp/pourpoint.whl")
        );
    }

    #[test]
    fn worker_environment_extension_refuses_removal_and_mid_plan_addition() {
        let directory = tempdir().expect("temporary directory");
        let declared = pce_core::DriverEvent::WorkerEnvironmentDeclared {
            names: vec!["EXISTING".to_owned()],
        };
        let removal = worker_environment_command(directory.path().join("removal.jsonl"), &[]);
        let error =
            crate::ensure_worker_environment_contract(&removal, std::slice::from_ref(&declared))
                .expect_err("removal must stay refused");
        assert!(format!("{error:#}").contains("cannot remove declared names"));
        let replacement = worker_environment_command(
            directory.path().join("replacement.jsonl"),
            &[("REPLACEMENT", "new")],
        );
        let error = crate::ensure_worker_environment_contract(
            &replacement,
            std::slice::from_ref(&declared),
        )
        .expect_err("an incomparable set must stay refused");
        assert!(format!("{error:#}").contains("cannot remove declared names"));

        let addition = worker_environment_command(
            directory.path().join("addition.jsonl"),
            &[("EXISTING", "old"), ("ADDED", "new")],
        );
        let error = crate::ensure_worker_environment_contract(
            &addition,
            &[
                declared,
                pce_core::DriverEvent::WorkerDispatched {
                    package: "GD2".to_owned(),
                    issuance: 1,
                },
            ],
        )
        .expect_err("mid-plan addition must stay refused");
        assert!(format!("{error:#}").contains("extend at the next plan-version boundary"));
    }

    #[test]
    fn worker_environment_absence_keeps_deriving_the_initial_declaration() {
        let directory = tempdir().expect("temporary directory");
        let journal_path = directory.path().join("events.jsonl");
        let command =
            worker_environment_command(journal_path.clone(), &[("FIRST_DECLARATION", "value")]);

        let historical_events = [pce_core::DriverEvent::WorkerDispatched {
            package: "LEGACY".to_owned(),
            issuance: 1,
        }];
        crate::ensure_worker_environment_contract(&command, &historical_events)
            .expect("old journal without a declaration must derive one");

        let line = fs::read_to_string(journal_path).expect("declaration event");
        let event: serde_json::Value = serde_json::from_str(line.trim()).expect("event JSON");
        assert_eq!(event["event"], "worker-environment-declared");
        assert_eq!(event["names"], json!(["FIRST_DECLARATION"]));
    }

    #[test]
    fn driver_run_error_appends_the_same_abort_diagnostic() {
        let directory = tempdir().expect("temporary directory");
        let graph_path = directory.path().join("graph.json");
        let journal_path = directory.path().join("events.jsonl");
        fs::write(&graph_path, b"{}").expect("invalid graph fixture");
        fs::write(&journal_path, b"").expect("empty journal");
        let command = crate::DriverRunCommand {
            graph_path: graph_path.clone(),
            journal_path: journal_path.clone(),
            repositories: Vec::new(),
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };

        let error = crate::run_driver_loop(command).expect_err("invalid graph must abort");
        let diagnostic = format!("{error:#}");
        let events = crate::read_driver_journal(&journal_path).expect("abort journal");
        assert!(matches!(
            events.last(),
            Some(pce_core::DriverEvent::DriverAborted { reason }) if reason == &diagnostic
        ));

        crate::append_driver_event(
            &journal_path,
            &pce_core::DriverEvent::WorkerEnvironmentDeclared {
                names: vec!["PATH".to_owned()],
            },
        )
        .expect("intervening legal event");
        let resumed = crate::DriverRunCommand {
            graph_path,
            journal_path: journal_path.clone(),
            repositories: Vec::new(),
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };
        crate::run_driver_loop(resumed).expect_err("invalid graph must abort after resuming");
        let events = crate::read_driver_journal(&journal_path).expect("resumed abort journal");
        assert!(matches!(
            events.last(),
            Some(pce_core::DriverEvent::DriverAborted { .. })
        ));
        assert!(matches!(
            events.get(events.len() - 2),
            Some(pce_core::DriverEvent::DriverResumed)
        ));
    }

    #[test]
    fn assembly_path_error_is_recorded_as_driver_abort() {
        let directory = tempdir().expect("temporary directory");
        let repository = directory.path().join("repository");
        fs::create_dir(&repository).expect("repository directory");
        initialize_git_repository(&repository);
        fs::write(repository.join("base"), "base\n").expect("base file");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "base"]);
        let base = crate::git_oid(&repository, "HEAD").expect("base oid");
        let graph_path = directory.path().join("graph.json");
        fs::write(
            &graph_path,
            serde_json::to_vec(&json!({
                "vision":"assembly-abort", "plan_version":1,
                "authored_at_refs":{"repo":base},
                "packages":[{
                    "id":"A", "title":"A", "repositories":["repo"],
                    "criteria":[{
                        "name":"base", "input":"package", "observation":"passes", "command":"true"
                    }],
                    "depends_on":[]
                }]
            }))
            .expect("graph JSON"),
        )
        .expect("graph write");
        let journal_path = directory.path().join("events.jsonl");
        let limits = pce_core::RecoveryLimits::default();
        for event in [
            pce_core::DriverEvent::WorkerEnvironmentDeclared { names: Vec::new() },
            pce_core::DriverEvent::RecoveryConfigured { limits },
            pce_core::DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::WorkerDone {
                package: "A".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::PackageCompleted {
                package: "A".to_owned(),
            },
        ] {
            crate::append_driver_event(&journal_path, &event).expect("journal event");
        }
        let command = crate::DriverRunCommand {
            graph_path,
            journal_path: journal_path.clone(),
            repositories: vec![("repo".to_owned(), repository)],
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: limits,
            worker_override: None,
            wait_timeout: None,
        };

        let error = crate::run_driver_loop(command).expect_err("missing package ref must abort");
        let events = crate::read_driver_journal(&journal_path).expect("abort journal");
        assert!(matches!(
            events.last(),
            Some(pce_core::DriverEvent::DriverAborted { reason })
                if reason == &format!("{error:#}")
        ));
    }

    #[test]
    fn driver_run_keeps_the_original_error_when_abort_journal_is_unwritable() {
        let directory = tempdir().expect("temporary directory");
        let graph_path = directory.path().join("graph.json");
        fs::write(&graph_path, b"{}").expect("invalid graph fixture");
        let journal_path = directory.path().join("journal-directory");
        fs::create_dir(&journal_path).expect("unwritable journal fixture");
        let command = crate::DriverRunCommand {
            graph_path,
            journal_path,
            repositories: Vec::new(),
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };

        let error = crate::run_driver_loop(command).expect_err("journal directory must abort");
        assert!(
            format!("{error:#}").contains("failed to read driver journal"),
            "the abort append failure must not replace the original error: {error:#}"
        );
    }

    #[test]
    fn revised_multi_repository_package_drops_stale_repair_credit_and_completes() {
        let directory = tempdir().expect("temporary directory");
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        for repository in [&first, &second] {
            fs::create_dir(repository).expect("repository directory");
            initialize_git_repository(repository);
            fs::write(repository.join("base"), "base\n").expect("base file");
            git(repository, &["add", "."]);
            git(repository, &["commit", "-m", "base"]);
        }
        let oid = |repository: &Path, reference: &str| {
            crate::git_oid(repository, reference).expect("git oid")
        };
        let first_base = oid(&first, "HEAD");
        let second_base = oid(&second, "HEAD");
        git(&second, &["checkout", "-b", "old-repair"]);
        fs::write(second.join("guard"), "present\n").expect("old repair guard");
        git(&second, &["add", "guard"]);
        git(&second, &["commit", "-m", "old repair"]);
        let stale_repair = oid(&second, "HEAD");
        git(&second, &["checkout", "main"]);
        fs::write(second.join("guard"), "present\n").expect("recreated guard");
        fs::write(second.join("revision"), "fresh lineage\n").expect("revision marker");
        git(&second, &["add", "guard", "revision"]);
        git(
            &second,
            &["commit", "-m", "recreate criterion on revised lineage"],
        );
        let second_lineage = oid(&second, "HEAD");

        let branch = "pce/stale-credit/W7/attempt-1";
        git(&first, &["branch", branch, "HEAD"]);
        git(&second, &["branch", branch, "HEAD"]);
        let graph_path = directory.path().join("graph.v2.json");
        fs::write(
            &graph_path,
            serde_json::to_vec(&json!({
                "vision":"stale-credit", "plan_version":2,
                "authored_at_refs":{"first":first_base,"second":second_base},
                "packages":[{
                    "id":"W7", "title":"revised package",
                    "repositories":["first","second"],
                    "criteria":[{
                        "name":"fresh implementation passes", "input":"recomposed lineage",
                        "observation":"criterion passes", "command":"true"
                    }],
                    "depends_on":[]
                }]
            }))
            .expect("graph JSON"),
        )
        .expect("graph write");
        let journal_path = directory.path().join("events.jsonl");
        let amendment = pce_core::EffectiveCriterion {
            name: "gate:package-gate-20-1:finding:0".to_owned(),
            command: r#"test -f "$PCE_WORKTREE_1/guard""#.to_owned(),
            origin: pce_core::CriterionOrigin::Amendment {
                gate: "package-gate-20-1".to_owned(),
                finding: 0,
            },
            repository_refs: vec![pce_core::AmendmentRepositoryRefs {
                repository: "second".to_owned(),
                witness_ref: second_base.clone(),
                repair_ref: stale_repair.clone(),
            }],
        };
        for event in [
            pce_core::DriverEvent::PlanVersionAdvanced {
                from_plan_version: 1,
                to_plan_version: 2,
                carried_completions: Vec::new(),
                carried_amendments: vec![("W7".to_owned(), amendment)],
                criterion_revisions_ratified_by: None,
                criterion_revisions: Vec::new(),
            },
            pce_core::DriverEvent::WorkerDispatched {
                package: "W7".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::WorkerDone {
                package: "W7".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::CriterionExecuted {
                package: "W7".to_owned(),
                name: "gate:package-gate-20-1:finding:0".to_owned(),
                origin: pce_core::CriterionOrigin::Amendment {
                    gate: "package-gate-20-1".to_owned(),
                    finding: 0,
                },
                execution: pce_core::CriterionExecution::new(
                    "test -f guard".to_owned(),
                    "second".to_owned(),
                    pce_core::CommandExitStatus::Exited { code: 0 },
                    String::new(),
                    String::new(),
                ),
            },
            pce_core::DriverEvent::GateFinished {
                package: "W7".to_owned(),
                gate: "package-gate-26-1".to_owned(),
            },
        ] {
            crate::append_driver_event(&journal_path, &event).expect("journal event");
        }
        let command = crate::DriverRunCommand {
            graph_path: graph_path.clone(),
            journal_path: journal_path.clone(),
            repositories: vec![("first".to_owned(), first), ("second".to_owned(), second)],
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };
        let graph = crate::read_driver_graph(&graph_path).expect("graph");

        let reproof = crate::finalize_gate_repairs(
            &graph,
            &command,
            "W7",
            1,
            "package-gate-26-1",
            Vec::new(),
        )
        .expect("stale credit must degrade during gate re-proof");
        assert_eq!(reproof, crate::GateReproofOutcome::Proven);
        crate::append_driver_event(
            &journal_path,
            &pce_core::DriverEvent::PackageCompleted {
                package: "W7".to_owned(),
            },
        )
        .expect("completion event");

        let events = crate::read_driver_journal(&journal_path).expect("journal");
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::RepairCreditStale {
                package, repository, repair_ref, lineage_oid, ..
            } if package == "W7" && repository == "second"
                && repair_ref == &stale_repair && lineage_oid == &second_lineage
        )));
        let snapshot = pce_core::derive_driver_snapshot(&graph, &events, false).expect("snapshot");
        assert!(matches!(
            snapshot.packages()[0].1,
            pce_core::DriverPackageState::Complete
        ));
        assert_eq!(
            snapshot.amendments().len(),
            1,
            "the criterion carries; only repair credit drops"
        );
        assert!(snapshot.amendments()[0].1.repository_refs.is_empty());
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::GateReproofExecuted {
                name,
                amendment_proof: None,
                execution,
                ..
            } if name == "gate:package-gate-20-1:finding:0"
                && execution.exit_status().is_success()
        )));

        // Assembly invalidation can rebuild a package lineage without a plan revision. Start a
        // separate journal where the original repair was credited, then invalidate that completed
        // package and recompose its next attempt from the independent fresh lineage.
        let no_revision_journal = directory.path().join("no-revision-events.jsonl");
        git(
            &command.repositories[0].1,
            &["branch", "-f", "pce/stale-credit/W7/attempt-2", "HEAD"],
        );
        let second_source = &command.repositories[1].1;
        git(
            second_source,
            &[
                "branch",
                "-f",
                "pce/stale-credit/W7/attempt-1",
                &stale_repair,
            ],
        );
        git(
            second_source,
            &[
                "branch",
                "-f",
                "pce/stale-credit/W7/attempt-2",
                &second_lineage,
            ],
        );
        let amendment_command = r#"test -f "$PCE_WORKTREE_1/guard""#;
        let replay_execution = |code| {
            pce_core::CriterionExecution::new(
                amendment_command.to_owned(),
                "second".to_owned(),
                pce_core::CommandExitStatus::Exited { code },
                String::new(),
                String::new(),
            )
        };
        for event in [
            pce_core::DriverEvent::WorkerDispatched {
                package: "W7".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::WorkerDone {
                package: "W7".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::FindingReplayed {
                package: "W7".to_owned(),
                gate: "package-gate-before-invalidation".to_owned(),
                finding: 0,
                command: amendment_command.to_owned(),
                repository_refs: vec![pce_core::AmendmentRepositoryRefs {
                    repository: "second".to_owned(),
                    witness_ref: second_base.clone(),
                    repair_ref: stale_repair.clone(),
                }],
                witness: replay_execution(1),
                repair: replay_execution(0),
                decision: pce_core::FindingReplayDecision::Accepted,
            },
            pce_core::DriverEvent::GateFinished {
                package: "W7".to_owned(),
                gate: "package-gate-before-invalidation".to_owned(),
            },
            pce_core::DriverEvent::PackageRepairMerged {
                package: "W7".to_owned(),
                repository: "second".to_owned(),
                gate: "package-gate-before-invalidation".to_owned(),
                finding: 0,
                repair_ref: stale_repair.clone(),
                previous_oid: second_base.clone(),
                hardened_oid: stale_repair.clone(),
            },
            pce_core::DriverEvent::PackageCompleted {
                package: "W7".to_owned(),
            },
            pce_core::DriverEvent::PackageHardeningInvalidated {
                package: "W7".to_owned(),
                hardened_package: "W7".to_owned(),
                repository: "second".to_owned(),
                gate: "package-gate-before-invalidation".to_owned(),
                finding: 0,
                repair_ref: stale_repair.clone(),
                detail: "assembly rewrite requires a fresh attempt".to_owned(),
            },
            pce_core::DriverEvent::WorkerDispatched {
                package: "W7".to_owned(),
                issuance: 2,
            },
            pce_core::DriverEvent::WorkerDone {
                package: "W7".to_owned(),
                issuance: 2,
            },
            pce_core::DriverEvent::GateFinished {
                package: "W7".to_owned(),
                gate: "package-gate-without-plan-revision".to_owned(),
            },
        ] {
            crate::append_driver_event(&no_revision_journal, &event).expect("non-revision event");
        }
        let no_revision_command = crate::DriverRunCommand {
            graph_path: graph_path.clone(),
            journal_path: no_revision_journal.clone(),
            repositories: command.repositories.clone(),
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };
        let reproof = crate::finalize_gate_repairs(
            &graph,
            &no_revision_command,
            "W7",
            2,
            "package-gate-without-plan-revision",
            Vec::new(),
        )
        .expect("non-revision stale credit must degrade during gate re-proof");
        assert_eq!(reproof, crate::GateReproofOutcome::Proven);
        let events =
            crate::read_driver_journal(&no_revision_journal).expect("non-revision journal");
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::RepairCreditStale { issuance: 2, .. }
        )));

        let unavailable_journal = directory.path().join("unavailable-repair-events.jsonl");
        let unavailable_ref = "ffffffffffffffffffffffffffffffffffffffff";
        let unavailable_amendment = pce_core::EffectiveCriterion {
            name: "gate:unavailable:finding:0".to_owned(),
            command: "true".to_owned(),
            origin: pce_core::CriterionOrigin::Amendment {
                gate: "unavailable".to_owned(),
                finding: 0,
            },
            repository_refs: vec![pce_core::AmendmentRepositoryRefs {
                repository: "second".to_owned(),
                witness_ref: second_base,
                repair_ref: unavailable_ref.to_owned(),
            }],
        };
        for event in [
            pce_core::DriverEvent::PlanVersionAdvanced {
                from_plan_version: 1,
                to_plan_version: 2,
                carried_completions: Vec::new(),
                carried_amendments: vec![("W7".to_owned(), unavailable_amendment)],
                criterion_revisions_ratified_by: None,
                criterion_revisions: Vec::new(),
            },
            pce_core::DriverEvent::WorkerDispatched {
                package: "W7".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::WorkerDone {
                package: "W7".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::GateFinished {
                package: "W7".to_owned(),
                gate: "unavailable-gate".to_owned(),
            },
        ] {
            crate::append_driver_event(&unavailable_journal, &event)
                .expect("unavailable-repair event");
        }
        let unavailable_command = crate::DriverRunCommand {
            graph_path,
            journal_path: unavailable_journal.clone(),
            repositories: command.repositories.clone(),
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };
        let reproof = crate::finalize_gate_repairs(
            &graph,
            &unavailable_command,
            "W7",
            1,
            "unavailable-gate",
            Vec::new(),
        )
        .expect("unavailable repair must degrade during gate re-proof");
        assert_eq!(reproof, crate::GateReproofOutcome::Proven);
        let events =
            crate::read_driver_journal(&unavailable_journal).expect("unavailable-repair journal");
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::RepairCreditStale {
                repair_ref,
                reason: pce_core::StaleRepairCreditReason::RepairUnavailable,
                ..
            } if repair_ref == unavailable_ref
        )));
    }

    #[test]
    fn assembly_resolution_is_path_scoped_and_anchored() {
        let prompt = crate::assembly_resolution_prompt(
            "repo",
            &["conflicted.rs".to_owned()],
            &[pce_core::CompositionInput {
                package: "A".to_owned(),
                oid: "abc".to_owned(),
            }],
            "content conflict",
        );
        assert!(prompt.contains("Modify only the exact conflicted paths listed above"));
        assert!(prompt.contains("Do not run repository-wide formatters"));
        assert!(prompt.contains("refuse if resolving the conflict requires another path"));

        let directory = tempdir().expect("temporary directory");
        let repository = directory.path().join("repository");
        fs::create_dir(&repository).expect("repository directory");
        initialize_git_repository(&repository);
        fs::write(repository.join("conflicted.rs"), "resolved\n").expect("resolution");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "resolved assembly"]);
        let oid = crate::git_oid(&repository, "HEAD").expect("resolution oid");

        let (reference, created) =
            crate::anchor_assembly_resolution(&repository, "repo", &oid).expect("resolution ref");

        assert!(created);
        assert!(reference.starts_with("refs/pce-assembly-resolutions/"));
        assert!(reference.ends_with(&oid));
        assert_eq!(
            crate::git_oid(&repository, &reference).expect("anchored oid"),
            oid
        );
        let (same_reference, created_again) =
            crate::anchor_assembly_resolution(&repository, "repo", &oid).expect("existing ref");
        assert_eq!(same_reference, reference);
        assert!(!created_again);
    }

    #[test]
    fn ast_identical_reformat_drops_byte_exact_credit_and_completes_assembly() {
        let directory = tempdir().expect("temporary directory");
        let repository = directory.path().join("repository");
        fs::create_dir(&repository).expect("repository directory");
        initialize_git_repository(&repository);
        fs::write(repository.join("boundary_probes.py"), "VALUE = 1\n").expect("base source");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "base"]);
        let base = crate::git_oid(&repository, "HEAD").expect("base oid");
        fs::write(
            repository.join("boundary_probes.py"),
            "VALUE = (\n    1\n    + 1\n)\n",
        )
        .expect("repair source");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "repair fixture"]);
        let repair = crate::git_oid(&repository, "HEAD").expect("repair oid");
        fs::write(repository.join("boundary_probes.py"), "VALUE = 1 + 1\n")
            .expect("formatted source");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "format AST-identically"]);
        let formatted = crate::git_oid(&repository, "HEAD").expect("formatted oid");
        git(
            &repository,
            &["branch", "pce/ast-reflow/REC3/attempt-1", &formatted],
        );

        let graph_path = directory.path().join("graph.json");
        fs::write(
            &graph_path,
            serde_json::to_vec(&json!({
                "vision":"ast-reflow", "plan_version":1,
                "authored_at_refs":{"repo":base},
                "packages":[{
                    "id":"REC3", "title":"record fixture", "repositories":["repo"],
                    "criteria":[{
                        "name":"module remains valid", "input":"composed tree",
                        "observation":"module imports", "command":"true"
                    }],
                    "depends_on":[]
                }]
            }))
            .expect("graph JSON"),
        )
        .expect("graph write");
        let journal_path = directory.path().join("events.jsonl");
        let amendment_command = "grep -qx 'VALUE = 1 + 1' boundary_probes.py";
        let execution = |code| {
            pce_core::CriterionExecution::new(
                amendment_command.to_owned(),
                "repo".to_owned(),
                pce_core::CommandExitStatus::Exited { code },
                String::new(),
                String::new(),
            )
        };
        for event in [
            pce_core::DriverEvent::WorkerDispatched {
                package: "REC3".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::WorkerDone {
                package: "REC3".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::FindingReplayed {
                package: "REC3".to_owned(),
                gate: "package-gate-1".to_owned(),
                finding: 0,
                command: amendment_command.to_owned(),
                repository_refs: vec![pce_core::AmendmentRepositoryRefs {
                    repository: "repo".to_owned(),
                    witness_ref: base.clone(),
                    repair_ref: repair.clone(),
                }],
                witness: execution(1),
                repair: execution(0),
                decision: pce_core::FindingReplayDecision::Accepted,
            },
            pce_core::DriverEvent::GateFinished {
                package: "REC3".to_owned(),
                gate: "package-gate-1".to_owned(),
            },
            pce_core::DriverEvent::PackageRepairMerged {
                package: "REC3".to_owned(),
                repository: "repo".to_owned(),
                gate: "package-gate-1".to_owned(),
                finding: 0,
                repair_ref: repair.clone(),
                previous_oid: base,
                hardened_oid: repair.clone(),
            },
            pce_core::DriverEvent::PackageCompleted {
                package: "REC3".to_owned(),
            },
        ] {
            crate::append_driver_event(&journal_path, &event).expect("journal event");
        }
        let command = crate::DriverRunCommand {
            graph_path: graph_path.clone(),
            journal_path: journal_path.clone(),
            repositories: vec![("repo".to_owned(), repository)],
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };
        let graph = crate::read_driver_graph(&graph_path).expect("graph");

        crate::run_driver_assembly(&graph, &command)
            .expect("AST-identical reformat must not abort attribution");

        let events = crate::read_driver_journal(&journal_path).expect("assembly journal");
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::RepairCreditStale {
                reason: pce_core::StaleRepairCreditReason::CounterfactualUnconstructable,
                repair_ref,
                ..
            } if repair_ref == &repair
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::AssemblyCriterionExecuted {
                origin: pce_core::CriterionOrigin::Amendment { .. },
                amendment_proof: None,
                execution,
                ..
            } if execution.exit_status().is_success()
        )));
        assert!(
            matches!(
                events.last(),
                Some(pce_core::DriverEvent::AssemblyCompleted)
            ),
            "unexpected terminal events: {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, pce_core::DriverEvent::DriverAborted { .. }))
        );

        // A cold relaunch must also repair the pre-fix journal shape: the green assembly
        // execution was already recorded with an unconstructable byte-exact counterfactual.
        let legacy_journal = directory.path().join("legacy-assembly-events.jsonl");
        for event in &events {
            let replay = match event {
                pce_core::DriverEvent::RepairCreditStale { .. }
                | pce_core::DriverEvent::AssemblyCompleted => continue,
                pce_core::DriverEvent::AssemblyCriterionExecuted {
                    package,
                    name,
                    origin: pce_core::CriterionOrigin::Amendment { gate, finding },
                    execution,
                    ..
                } => pce_core::DriverEvent::AssemblyCriterionExecuted {
                    package: package.clone(),
                    name: name.clone(),
                    origin: pce_core::CriterionOrigin::Amendment {
                        gate: gate.clone(),
                        finding: *finding,
                    },
                    execution: execution.clone(),
                    amendment_proof: Some(pce_core::AmendmentProof::Unconstructable {
                        repository: "repo".to_owned(),
                        repair_ref: repair.clone(),
                        detail: "formatted hunk no longer matches bytes".to_owned(),
                    }),
                },
                other => other.clone(),
            };
            crate::append_driver_event(&legacy_journal, &replay).expect("legacy event");
        }
        let legacy_command = crate::DriverRunCommand {
            graph_path,
            journal_path: legacy_journal.clone(),
            repositories: command.repositories.clone(),
            preparations: std::collections::BTreeMap::new(),
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
            override_risk_ordering: false,
            recovery_limits: pce_core::RecoveryLimits::default(),
            worker_override: None,
            wait_timeout: None,
        };
        crate::run_driver_assembly(&graph, &legacy_command)
            .expect("cold assembly resume must degrade historical byte credit");
        let legacy = crate::read_driver_journal(&legacy_journal).expect("legacy journal");
        assert!(legacy.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::RepairCreditStale {
                reason: pce_core::StaleRepairCreditReason::CounterfactualUnconstructable,
                ..
            }
        )));
        assert!(matches!(
            legacy.last(),
            Some(pce_core::DriverEvent::AssemblyCompleted)
        ));
    }

    #[test]
    fn amendment_with_deleted_guard_cannot_pass_on_zero_executed_tests() {
        let temp = tempdir().expect("tempdir");
        let repository = temp.path().join("repository");
        initialize_git_repository(&repository);
        fs::write(repository.join("defect"), "broken\n").expect("defect");
        git(&repository, &["add", "."]);
        git(&repository, &["commit", "-m", "base"]);
        fs::write(
            repository.join("guard.sh"),
            "#!/bin/sh\nif grep -qx hardened defect; then echo '1 passed'; exit 0; fi\necho failed; exit 1\n",
        )
        .expect("guard");
        let mut permissions = fs::metadata(repository.join("guard.sh"))
            .expect("guard metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(repository.join("guard.sh"), permissions).expect("guard mode");
        git(&repository, &["add", "guard.sh"]);
        git(&repository, &["commit", "-m", "witness"]);
        fs::write(repository.join("defect"), "hardened\n").expect("repair");
        git(&repository, &["add", "defect"]);
        git(&repository, &["commit", "-m", "repair"]);
        let repair = crate::git_oid(&repository, "HEAD").expect("repair oid");
        fs::write(
            repository.join("guard.sh"),
            "#!/bin/sh\necho 'running 0 tests ... test result: ok'\nexit 0\n",
        )
        .expect("deleted guard");
        git(&repository, &["add", "guard.sh"]);
        git(&repository, &["commit", "-m", "delete guarded case"]);
        let criterion = EffectiveCriterion {
            name: "gate:g:finding:0".to_owned(),
            command: "./guard.sh".to_owned(),
            origin: CriterionOrigin::Amendment {
                gate: "g".to_owned(),
                finding: 0,
            },
            repository_refs: vec![AmendmentRepositoryRefs {
                repository: "repo".to_owned(),
                witness_ref: "HEAD~2".to_owned(),
                repair_ref: repair,
            }],
        };
        let paths = vec![repository.clone()];
        let named_paths = [("repo".to_owned(), repository)].into_iter().collect();
        let (execution, proof, passed) =
            execute_effective_criterion(&criterion, &paths, &named_paths).expect("paired proof");
        assert!(execution.exit_status().is_success());
        assert!(execution.stdout().contains("0 tests"));
        let pce_core::AmendmentProof::Reverted {
            execution: reverted,
        } = proof.expect("amendment proof")
        else {
            panic!("revert should construct");
        };
        assert!(reverted.exit_status().is_success());
        assert!(reverted.stdout().contains("0 tests"));
        assert!(!passed);
    }

    #[test]
    fn gate_reproof_rejects_a_repair_that_breaks_an_authored_criterion_without_charging_package() {
        let directory = tempdir().expect("temporary directory");
        let repository = directory.path().join("repo");
        fs::create_dir(&repository).expect("repository directory");
        initialize_git_repository(&repository);
        fs::write(repository.join("value"), "sealed-order").expect("baseline value");
        git(&repository, &["add", "value"]);
        git(&repository, &["commit", "-m", "worker result"]);
        let oid = |reference: &str| {
            let output = ProcessCommand::new("git")
                .arg("-C")
                .arg(&repository)
                .args(["rev-parse", reference])
                .output()
                .expect("git rev-parse should spawn");
            assert!(output.status.success());
            String::from_utf8(output.stdout)
                .expect("oid UTF-8")
                .trim()
                .to_owned()
        };
        let worker_oid = oid("HEAD");
        let branch = "pce/gate-reproof/A/attempt-1";
        git(&repository, &["branch", branch, &worker_oid]);
        fs::write(repository.join("value"), "canonical-order").expect("repair value");
        fs::write(repository.join("guard"), "present").expect("repair guard");
        git(&repository, &["add", "value", "guard"]);
        git(&repository, &["commit", "-m", "gate repair"]);
        let repair_oid = oid("HEAD");

        let graph_path = directory.path().join("graph.json");
        fs::write(
            &graph_path,
            serde_json::to_vec(&json!({
                "vision":"gate-reproof", "plan_version":1,
                "authored_at_refs":{"repo":worker_oid},
                "packages":[{
                    "id":"A", "title":"A", "repositories":["repo"],
                    "criteria":[{
                        "name":"sealed baseline remains verifiable",
                        "input":"worker lineage", "observation":"original ordering verifies",
                        "command":r#"test "$(cat value)" = sealed-order"#
                    }],
                    "depends_on":[]
                }]
            }))
            .expect("graph JSON"),
        )
        .expect("graph write");
        let journal_path = directory.path().join("events.jsonl");
        let limits = pce_core::RecoveryLimits::new(
            pce_core::RetryLimit::new(1),
            pce_core::LocalPatchLimit::new(1),
        )
        .with_gate_failure_limit(pce_core::GateFailureLimit::new(3));
        for event in [
            pce_core::DriverEvent::RecoveryConfigured { limits },
            pce_core::DriverEvent::WorkerDispatched {
                package: "A".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::WorkerDone {
                package: "A".to_owned(),
                issuance: 1,
            },
            pce_core::DriverEvent::FindingReplayed {
                package: "A".to_owned(),
                gate: "gate-1".to_owned(),
                finding: 0,
                command: "test -f guard".to_owned(),
                repository_refs: vec![pce_core::AmendmentRepositoryRefs {
                    repository: "repo".to_owned(),
                    witness_ref: worker_oid.clone(),
                    repair_ref: repair_oid.clone(),
                }],
                witness: pce_core::CriterionExecution::new(
                    "test -f guard".to_owned(),
                    "repo".to_owned(),
                    pce_core::CommandExitStatus::Exited { code: 1 },
                    String::new(),
                    String::new(),
                ),
                repair: pce_core::CriterionExecution::new(
                    "test -f guard".to_owned(),
                    "repo".to_owned(),
                    pce_core::CommandExitStatus::Exited { code: 0 },
                    String::new(),
                    String::new(),
                ),
                decision: pce_core::FindingReplayDecision::Accepted,
            },
            pce_core::DriverEvent::GateFinished {
                package: "A".to_owned(),
                gate: "gate-1".to_owned(),
            },
        ] {
            crate::append_driver_event(&journal_path, &event).expect("journal event");
        }
        let command = crate::DriverRunCommand {
            graph_path: graph_path.clone(),
            journal_path: journal_path.clone(),
            repositories: vec![("repo".to_owned(), repository.clone())],
            preparations: std::collections::BTreeMap::new(),
            override_risk_ordering: false,
            recovery_limits: limits,
            worker_override: None,
            wait_timeout: None,
            worker_environment: std::collections::BTreeMap::new(),
            herdr_session: None,
        };
        let graph = crate::read_driver_graph(&graph_path).expect("graph");

        let proved = crate::finalize_gate_repairs(&graph, &command, "A", 1, "gate-1", Vec::new())
            .expect("reproof flow");

        assert_eq!(proved, crate::GateReproofOutcome::Rejected);
        assert_eq!(oid(branch), worker_oid);
        let events = crate::read_driver_journal(&journal_path).expect("journal");
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, pce_core::DriverEvent::GateReproofExecuted { .. }))
                .count(),
            2
        );
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::GateReproofExecuted { name, execution, .. }
                if name == "sealed baseline remains verifiable" && !execution.exit_status().is_success()
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::GateReproofExecuted {
                name,
                amendment_proof: Some(proof),
                ..
            } if name == "gate:gate-1:finding:0" && proof.proves_guard()
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::PackageRepairRolledBack { restored_oid, .. }
                if restored_oid == &worker_oid
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            pce_core::DriverEvent::GateFailed { reason, challenges, .. }
                if reason == crate::GATE_REPROOF_FAILED_REASON
                    && challenges.iter().any(|challenge| {
                        challenge.proposed_criterion_command()
                            == r#"test "$(cat value)" = sealed-order"#
                    })
        )));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, pce_core::DriverEvent::PackageCompleted { .. }))
        );
        let snapshot = pce_core::derive_driver_snapshot(&graph, &events, false).expect("snapshot");
        assert!(matches!(
            snapshot.packages()[0].1,
            pce_core::DriverPackageState::Judging { issuance: 1 }
        ));
        assert_eq!(pce_core::charged_failure_count(&events, "A"), 0);
        assert!(
            pce_core::effective_criteria(&graph, "A", &events)
                .expect("effective criteria")
                .iter()
                .all(|criterion| matches!(criterion.origin, pce_core::CriterionOrigin::Authored))
        );
        crate::harden_package_lineage_for_issuance(&graph, &command, "A", 1)
            .expect("rolled-back repair must leave no hardening credit");
        let events = crate::read_driver_journal(&journal_path).expect("post-rollback journal");
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, pce_core::DriverEvent::RepairCreditStale { .. }))
        );
    }

    #[test]
    fn installed_codex_schemas_conform_to_strict_dialect() {
        for relative in [
            "skills/pce/schemas/graph.schema.json",
            "skills/pce/schemas/verdict.schema.json",
        ] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
            crate::validate_codex_output_schema(&path)
                .unwrap_or_else(|error| panic!("{}: {error:#}", path.display()));
        }
    }
}
