//! review : SkillMarkdown × ReviewBindings → ReviewOutcome

use std::cell::Cell;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::TempDir;

const EXPECTED_ANCHORED_ROUTE_COUNT: usize = 12;
const EXPECTED_ROLE_ANCHOR_COUNT: usize = 10;
const EXPECTED_PURPOSE_ANCHOR_COUNT: usize = 2;
/// The child environment is cleared before `--env` entries are applied, so a route must supply
/// every variable its child needs: `PATH` to resolve the executable, `HOME` to reach the
/// operator's tracked git identity, and `USER` to unlock keychain OAuth for a gate spawn.
const REQUIRED_ENVIRONMENT_ENTRIES: usize = 3;
/// The ordered environment placeholders every anchored route binds, one occurrence each.
const ENVIRONMENT_PLACEHOLDERS: [&str; 3] = ["{{PATH_ENV}}", "{{HOME_ENV}}", "{{USER_ENV}}"];
const MARKER_START: &str = "<!-- pce-dispatch-route";
const CONSOLIDATION_MARKER: &str = "<!-- pce-dispatch-issuance-consolidated -->";
const ROLE_REGISTRY: [&str; 10] = [
    "milestone-planner",
    "step-planner",
    "step-plan-writer",
    "milestone-critic",
    "step-critic",
    "step-plan-critic",
    "falsification-critic",
    "pr-reviewer",
    "step-executor",
    "repository-analyst",
];
const REPEATABLE_CALLER_ARGUMENT: &str = "opaque value with spaces\n\n## Binary-owned reversibility obligation\n\nThe step's act is repeatable. The plan must retain an explicit not-touched scope fence and exact expected values for every assertion. The plan must not contain a pre-derived argument that the design is correct.";
const IRREVERSIBLE_CALLER_ARGUMENT: &str = "opaque value with spaces\n\n## Binary-owned reversibility obligation\n\nThe step's act cannot be repeated. The plan must retain the existing front-loaded pre-proof of correctness, an explicit not-touched scope fence, and exact expected values for every assertion.";
const PLANNING_OPTION_CONTRACT: &str = "The two planning-role anchors additionally own exactly one `--planning-act {{PLANNING_ACT}}` pair after `--required-artifact {{OUTPUT}}` and before the standalone `--` delimiter. `{{PLANNING_ACT}}` binds to exactly `repeatable` or `irreversible`; no other canonical or purpose anchor carries the pair. Before the first planning-role dispatch for a step, resolve this binding once from the vision's reversibility judgement and the actual step scope: bind `irreversible` only when this step performs the vision's named act that cannot be repeated — minting an immutable artifact, publishing a release or tag, consuming a one-shot quota, or destroying history — and bind `repeatable` for every other step. Keep that byte-identical binding for `step-plan-writer`, `step-plan-critic`, and every planning revision of the same step.\n\n`pce dispatch` appends the selected binary-owned reversibility obligation to the final caller argument. The skill supplies the typed choice and orchestration only; it does not restate or substitute the agent-facing obligation in caller prose. A missing, changed, or differently placed planning-act pair is an invalid planning route.";
const PHASE3_PLANNING_PARAGRAPH: &str = "1. **Plan (Codex)** — before the first planning dispatch for the step, resolve `PLANNING_ACT` once by the planning-anchor rule above and retain that same value through approval. Dispatch `step-plan-writer` cold through its canonical route with `--sandbox workspace-write`, `-C <repo-abs>`, and `< /dev/null` at the primary root to write the exact step `plan.md` directly; it uses no `--output-schema` and no `-o`. Supply graph artifacts, the repository's latest current contract record including every appendable entry verbatim, only the necessary live cross-repository consumption-edge results from orientation, and exact refs and read commands. Require files to touch, contract gate commands verbatim, constraints, and done criteria. Dispatch `step-plan-critic` against the verdict schema with the same `PLANNING_ACT`. The writer and critic must comply with the binary-owned reversibility obligation appended by `pce dispatch`; a verdict cannot approve a plan that contradicts it. Every planning revision reuses the same `PLANNING_ACT`. Each complete anchored logging envelope records issuance at the actual canonical step node.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteKind {
    CodexUnstructured,
    CodexStructured,
    GateStructured,
    CodexCommitCompletion,
    CodexDiagnostics,
}

impl RouteKind {
    fn parse(value: &str) -> Result<Self, ReviewError> {
        match value {
            "codex-unstructured" => Ok(Self::CodexUnstructured),
            "codex-structured" => Ok(Self::CodexStructured),
            "gate-structured" => Ok(Self::GateStructured),
            "codex-commit-completion" => Ok(Self::CodexCommitCompletion),
            "codex-diagnostics" => Ok(Self::CodexDiagnostics),
            _ => Err(ReviewError::UnknownKind),
        }
    }

    const fn target(self) -> &'static str {
        match self {
            Self::CodexUnstructured
            | Self::CodexStructured
            | Self::CodexCommitCompletion
            | Self::CodexDiagnostics => "codex",
            Self::GateStructured => "gate",
        }
    }

    const fn is_purpose(self) -> bool {
        matches!(self, Self::CodexCommitCompletion | Self::CodexDiagnostics)
    }

    const fn is_canonical_role(self) -> bool {
        !self.is_purpose()
    }

    const fn purpose_kind(self) -> Option<&'static str> {
        match self {
            Self::CodexCommitCompletion => Some("codex-commit-completion"),
            Self::CodexDiagnostics => Some("codex-diagnostics"),
            _ => None,
        }
    }

    const fn requires_structured(self) -> bool {
        matches!(self, Self::CodexStructured | Self::GateStructured)
    }

    const fn required_sandbox(self) -> Option<&'static str> {
        match self {
            Self::CodexUnstructured
            | Self::CodexStructured
            | Self::CodexCommitCompletion
            | Self::CodexDiagnostics => Some("workspace-write"),
            Self::GateStructured => None,
        }
    }

    const fn permits_plan_input(self) -> bool {
        !self.is_purpose()
    }

    const fn caller_tail_arity(self) -> Option<usize> {
        if self.is_purpose() { Some(1) } else { None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Placeholder {
    Cwd,
    Schema,
    Output,
    PlanFile,
    LogFile,
    PathEnv,
    HomeEnv,
    UserEnv,
    Node,
    MilestoneNode,
    StepNode,
    Role,
    Ref,
    Evidence,
    PlanningAct,
    AbsPath,
    CallerArg,
}

impl Placeholder {
    fn parse(value: &str) -> Result<Self, ReviewError> {
        match value {
            "{{CWD}}" => Ok(Self::Cwd),
            "{{SCHEMA}}" => Ok(Self::Schema),
            "{{OUTPUT}}" => Ok(Self::Output),
            "{{PLAN_FILE}}" => Ok(Self::PlanFile),
            "{{LOG_FILE}}" => Ok(Self::LogFile),
            "{{PATH_ENV}}" => Ok(Self::PathEnv),
            "{{HOME_ENV}}" => Ok(Self::HomeEnv),
            "{{USER_ENV}}" => Ok(Self::UserEnv),
            "{{NODE}}" => Ok(Self::Node),
            "{{MILESTONE_NODE}}" => Ok(Self::MilestoneNode),
            "{{STEP_NODE}}" => Ok(Self::StepNode),
            "{{ROLE}}" => Ok(Self::Role),
            "{{REF}}" => Ok(Self::Ref),
            "{{EVIDENCE}}" => Ok(Self::Evidence),
            "{{PLANNING_ACT}}" => Ok(Self::PlanningAct),
            "{{ABS_PATH}}" => Ok(Self::AbsPath),
            "{{CALLER_ARG}}" => Ok(Self::CallerArg),
            _ => Err(ReviewError::UnknownPlaceholder),
        }
    }
}

#[derive(Clone, Debug)]
struct AnchoredRoute {
    kind: RouteKind,
    source_line: usize,
    tokens: Vec<String>,
}

#[derive(Debug)]
struct ReviewBindings {
    cwd: PathBuf,
    schema: PathBuf,
    output: PathBuf,
    plan_file: PathBuf,
    log_file: PathBuf,
    shim_dir: PathBuf,
    child_home: PathBuf,
    child_user: OsString,
    graph_schema: PathBuf,
    verdict_schema: PathBuf,
    graph_outputs: BTreeMap<String, PathBuf>,
    verdict_root: PathBuf,
    complete_git_dir: PathBuf,
    context: ReviewContext,
    role: OsString,
    dispatch_ref: OsString,
    evidence: OsString,
    evidence_by_role: BTreeMap<String, OsString>,
    planning_act: OsString,
    caller_arg: OsString,
    pce_invocations: Cell<usize>,
}

#[derive(Debug)]
struct ReviewContext {
    accepted_events: Vec<serde_json::Value>,
    review_directories: BTreeMap<String, PathBuf>,
}

#[derive(Debug)]
struct Fixture {
    _root: TempDir,
    bindings: ReviewBindings,
}

#[derive(Clone, Debug)]
struct RouteObservation {
    source_line: usize,
    status: i32,
    child_argv: Vec<Vec<u8>>,
}

#[derive(Debug)]
struct ReviewReport {
    routes: Vec<(usize, RouteKind)>,
    observations: Vec<RouteObservation>,
}

#[derive(Debug)]
struct AttributedReviewError {
    source_line: usize,
    error: ReviewError,
    observations: Vec<RouteObservation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ReviewError {
    RawCodexExec,
    UnknownKind,
    MissingKind,
    ExtraMetadata,
    DuplicateMetadata,
    MissingFence,
    InterveningContent,
    WrongFenceLanguage,
    UnclosedFence,
    EmptyCommand,
    MultipleCommands,
    ShellConstruct,
    DanglingContinuation,
    MidlineBackslash,
    DoubleTerminalBackslash,
    WrongFirstToken,
    WrongSecondToken,
    TargetKindMismatch,
    UnknownPlaceholder,
    MalformedPlaceholder,
    EmbeddedPlaceholder,
    EmptyRole,
    EmptyRef,
    EmptyEvidence,
    ParentGrammar,
    ZeroEnvironmentEntries(usize),
    WrongEnvironmentCardinality(usize),
    MultipleDelimiters,
    CallerTailResuppliesBinaryArgument,
    IndentedMarker,
    IndentedOpeningFence,
    IndentedCommand,
    IndentedClosingFence,
    GateOutputPathMismatch,
    GateTailDesignator,
    GateTailValue,
    GateTailAbsolute,
    GateTailPrompt,
    GateTailCardinality,
    WrongVerdictIndex,
    WrongVerdictDirectory,
    RoleSemantics,
    OutsideAnchorDispatchFragment,
    CoLocatedStandaloneDispatchAppend,
    VerdictMissing,
    VerdictUnreadable,
    VerdictMalformed,
    VerdictSchemaInvalid,
    VerdictUnknown,
    PurposeTailCardinality(usize),
    Spawn(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerdictRoute {
    Forward,
    Revise,
    Escalate,
}

fn route_verdict(path: &Path, schema_path: &Path) -> Result<VerdictRoute, ReviewError> {
    let bytes = fs::read(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ReviewError::VerdictMissing
        } else {
            ReviewError::VerdictUnreadable
        }
    })?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| ReviewError::VerdictMalformed)?;
    let schema: serde_json::Value =
        serde_json::from_slice(&fs::read(schema_path).map_err(|_| ReviewError::VerdictUnreadable)?)
            .map_err(|_| ReviewError::VerdictSchemaInvalid)?;
    let validator =
        jsonschema::validator_for(&schema).map_err(|_| ReviewError::VerdictSchemaInvalid)?;
    if !validator.is_valid(&value) {
        return Err(ReviewError::VerdictSchemaInvalid);
    }
    match value["verdict"].as_str() {
        Some("APPROVE") => Ok(VerdictRoute::Forward),
        Some("REVISE") => Ok(VerdictRoute::Revise),
        Some("BLOCK") => Ok(VerdictRoute::Escalate),
        _ => Err(ReviewError::VerdictUnknown),
    }
}

fn prior_dispatch_count(events: &[serde_json::Value], node: &str, role: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            event["kind"] == "dispatch" && event["node"] == node && event["role"] == role
        })
        .count()
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir_in(std::env::temp_dir()).expect("platform temp fixture");
    let cwd = root.path().join("cwd");
    let shim_dir = root.path().join("shim");
    let child_home = root.path().join("home");
    fs::create_dir_all(cwd.join(".review")).expect("record directory");
    fs::create_dir_all(&shim_dir).expect("shim directory");
    fs::create_dir_all(&child_home).expect("child home directory");
    let schema = root.path().join("schema.json");
    let plan_file = root.path().join("plan.bin");
    fs::write(&schema, br#"{"type":"object"}"#).expect("schema");
    fs::write(&plan_file, b"plan\0bytes").expect("plan");
    let graph_schema = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("skills/pce/schemas/graph.schema.json")
        .canonicalize()
        .expect("graph schema");
    let verdict_schema = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("skills/pce/schemas/verdict.schema.json")
        .canonicalize()
        .expect("verdict schema");
    let graph_outputs = ["milestone-planner", "step-planner"]
        .into_iter()
        .map(|role| (role.to_owned(), root.path().join(format!("{role}.json"))))
        .collect();
    let verdict_root = root.path().join("reviews");
    fs::create_dir_all(&verdict_root).expect("review root");
    let mut review_directories = BTreeMap::new();
    for role in ROLE_REGISTRY {
        let directory = verdict_root.join(role);
        fs::create_dir_all(&directory).expect("role review directory");
        review_directories.insert(role.to_owned(), directory);
    }
    let mut accepted_events = Vec::new();
    let mut evidence_by_role = BTreeMap::new();
    for role in ROLE_REGISTRY {
        evidence_by_role.insert(
            role.to_owned(),
            OsString::from("measured evidence with spaces"),
        );
    }
    for role in [
        "repository-analyst",
        "milestone-critic",
        "step-critic",
        "step-plan-critic",
        "pr-reviewer",
    ] {
        let node = match role {
            "repository-analyst" | "milestone-critic" => "m1-s1",
            "step-critic" => "m7-s1",
            _ => "m7-s2",
        };
        for _ in 0..2 {
            accepted_events.push(serde_json::json!({"kind":"dispatch","node":node,"role":role}));
        }
    }
    accepted_events.push(
        serde_json::json!({"kind":"dispatch-completion","node":"m7-s2","role":"pr-reviewer"}),
    );
    accepted_events.push(serde_json::json!({"kind":"dispatch","node":"m7-s1","role":"other-role"}));
    let dot_git = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".git");
    let complete_git_dir = if dot_git.is_dir() {
        dot_git.canonicalize().expect("common git directory")
    } else {
        let metadata = fs::read_to_string(&dot_git).expect("gitdir file");
        let worktree_git = dot_git
            .parent()
            .expect("worktree root")
            .join(
                metadata
                    .trim()
                    .strip_prefix("gitdir: ")
                    .expect("gitdir prefix"),
            )
            .canonicalize()
            .expect("worktree git metadata");
        let worktrees = worktree_git.parent().expect("worktrees directory");
        assert_eq!(worktrees.file_name(), Some(OsStr::new("worktrees")));
        worktrees
            .parent()
            .expect("common git parent")
            .canonicalize()
            .expect("common git directory")
    };
    for executable in ["codex", "claude"] {
        let shim = shim_dir.join(executable);
        let result = if executable == "codex" {
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":4}}"#
        } else {
            r#"{"type":"result","subtype":"success","is_error":false,"result":"OK","usage":{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":3,"cache_read_input_tokens":4}}"#
        };
        let conformance = if executable == "codex" {
            format!(
                r#"structured=0
schema=
out_count=0
out=
previous=
for arg in "$@"; do
  if [ "$previous" = schema ]; then schema=$arg; previous=; continue; fi
  if [ "$previous" = out ]; then out=$arg; previous=; continue; fi
  if [ "$arg" = --output-schema ]; then structured=1; previous=schema; continue; fi
  if [ "$arg" = -o ]; then out_count=$((out_count + 1)); previous=out; continue; fi
done
if [ "$structured" = 1 ]; then
  if [ "$out_count" != 1 ] || [ -z "$out" ] || [ "${{out#/}}" = "$out" ]; then exit 73; fi
  printf '%s' "$out" > "$PWD/.review/recovered-path"
  if [ "$schema" = '{}' ]; then printf '{{"nodes":[]}}' > "$out"
  elif [ "$schema" = '{}' ]; then printf '{{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"fixture"}}' > "$out"
  else printf 'unrecognised schema: %s\n' "$schema" >&2; exit 74; fi
fi
"#,
                graph_schema.display(),
                verdict_schema.display()
            )
        } else {
            r#"count=0
out=
previous=
for arg in "$@"; do
  if [ "$previous" = out ]; then out=$arg; previous=; continue; fi
  if [ "$arg" = --append-system-prompt ]; then count=$((count + 1)); previous=out; fi
done
if [ "$count" != 1 ] || [ -z "$out" ]; then exit 75; fi
if [ "${out#/}" = "$out" ]; then
  out=${out#*<output-path>}
  out=${out%%</output-path>*}
fi
if [ -z "$out" ] || [ "${out#/}" = "$out" ]; then exit 75; fi
printf '%s' "$out" > "$PWD/.review/recovered-path"
printf '{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"fixture"}' > "$out"
"#
            .to_owned()
        };
        fs::write(
            &shim,
            format!(
                "#!/bin/sh\n: > \"$PWD/.review/argv\"\nfor arg in \"$@\"; do printf '%s\\0' \"$arg\" >> \"$PWD/.review/argv\"; done\n/bin/cat > \"$PWD/.review/stdin\"\n/usr/bin/env > \"$PWD/.review/env\"\nif [ \"${{ARTIFACT_MODE+x}}\" = x ]; then\nif [ \"$ARTIFACT_MODE\" = valid ]; then printf '{{}}' > \"$ARTIFACT\"; fi\nif [ \"$ARTIFACT_MODE\" = malformed ]; then printf '{{' > \"$ARTIFACT\"; fi\nelse\n{conformance}fi\nif [ \"${{SILENT_STDOUT:-}}\" != yes ]; then printf '%s\\n' '{result}'; fi\nprintf '%s' \"${{EXIT_CODE:-0}}\" > \"$PWD/.review/completion\"\nexit \"${{EXIT_CODE:-0}}\"\n"
            ),
        )
        .expect("shim");
        let mut permissions = fs::metadata(&shim).expect("shim metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim, permissions).expect("shim permissions");
    }
    Fixture {
        bindings: ReviewBindings {
            cwd,
            schema,
            output: root.path().join("output.json"),
            plan_file,
            log_file: root.path().join("events.jsonl"),
            shim_dir,
            child_home,
            child_user: OsString::from("review-operator"),
            graph_schema,
            verdict_schema,
            graph_outputs,
            verdict_root,
            complete_git_dir,
            context: ReviewContext {
                accepted_events,
                review_directories,
            },
            role: OsString::from("step-plan-writer"),
            dispatch_ref: OsString::from("07b85ccd"),
            evidence: OsString::from("measured evidence with spaces"),
            evidence_by_role,
            planning_act: OsString::from("repeatable"),
            caller_arg: OsString::from("opaque value with spaces"),
            pce_invocations: Cell::new(0),
        },
        _root: root,
    }
}

fn marker(kind: &str) -> String {
    format!("<!-- pce-dispatch-route kind=\"{kind}\" -->")
}

fn document(kind: &str, command: &str) -> String {
    format!("{}\n```sh\n{command}\n```\n", marker(kind))
}

fn base_command(kind: RouteKind) -> String {
    match kind {
        RouteKind::CodexUnstructured => concat!(
            "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write ",
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} -- {{CALLER_ARG}}"
        )
        .to_owned(),
        RouteKind::CodexStructured => concat!(
            "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write ",
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} -- {{CALLER_ARG}}"
        )
        .to_owned(),
        RouteKind::CodexCommitCompletion | RouteKind::CodexDiagnostics => concat!(
            "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write ",
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} ",
            "--log-file {{LOG_FILE}} --node {{NODE}} --role {{ROLE}} --ref {{REF}} ",
            "--evidence {{EVIDENCE}} --required-artifact {{OUTPUT}} -- {{CALLER_ARG}}"
        )
        .to_owned(),
        RouteKind::GateStructured => concat!(
            "pce dispatch gate --cwd {{CWD}} --env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} ",
            "--output-schema {{SCHEMA}} -o {{OUTPUT}} -- {{CALLER_ARG}}"
        )
        .to_owned(),
    }
}

fn parse_metadata(line: &str) -> Result<RouteKind, ReviewError> {
    if !line.ends_with(" -->") {
        return Err(ReviewError::ExtraMetadata);
    }
    let body = line
        .strip_prefix("<!-- pce-dispatch-route ")
        .and_then(|value| value.strip_suffix(" -->"))
        .ok_or(ReviewError::MissingKind)?;
    let count = body.matches("kind=").count();
    if count == 0 {
        return Err(ReviewError::MissingKind);
    }
    if count > 1 {
        return Err(ReviewError::DuplicateMetadata);
    }
    let value = body
        .strip_prefix("kind=\"")
        .and_then(|value| value.strip_suffix('"'))
        .ok_or(ReviewError::ExtraMetadata)?;
    if value.contains('"')
        || value
            .chars()
            .any(|character| character.is_ascii_whitespace())
    {
        return Err(ReviewError::ExtraMetadata);
    }
    RouteKind::parse(value)
}

fn tokenize(lines: &[&str]) -> Result<Vec<String>, ReviewError> {
    if lines.is_empty() || lines.iter().all(|line| line.trim().is_empty()) {
        return Err(ReviewError::EmptyCommand);
    }
    let mut logical = String::new();
    for (index, line) in lines.iter().enumerate() {
        let slash_count = line.chars().rev().take_while(|c| *c == '\\').count();
        if slash_count > 1 {
            return Err(ReviewError::DoubleTerminalBackslash);
        }
        if line[..line.len().saturating_sub(slash_count)].contains('\\') {
            return Err(ReviewError::MidlineBackslash);
        }
        if slash_count == 1 {
            if index + 1 == lines.len() {
                return Err(ReviewError::DanglingContinuation);
            }
            logical.push_str(&line[..line.len() - 1]);
            logical.push(' ');
        } else {
            logical.push_str(line);
            if index + 1 != lines.len() {
                return Err(ReviewError::MultipleCommands);
            }
        }
    }
    if logical.contains(['\'', '"', '$', '|', ';', '>', '<', '&', '`']) {
        return Err(ReviewError::ShellConstruct);
    }
    let tokens: Vec<String> = logical
        .split_ascii_whitespace()
        .map(ToOwned::to_owned)
        .collect();
    if tokens.is_empty() {
        return Err(ReviewError::EmptyCommand);
    }
    Ok(tokens)
}

fn extract_anchored_routes(markdown: &str) -> Result<Vec<AnchoredRoute>, ReviewError> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut routes = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim_start().starts_with(MARKER_START)
            && !lines[index].starts_with(MARKER_START)
        {
            return Err(ReviewError::IndentedMarker);
        }
        if !lines[index].starts_with(MARKER_START) {
            index += 1;
            continue;
        }
        let source_line = index + 1;
        let kind = parse_metadata(lines[index])?;
        index += 1;
        while index < lines.len() && lines[index].is_empty() {
            index += 1;
        }
        if index == lines.len() {
            return Err(ReviewError::MissingFence);
        }
        if lines[index].trim_start() == "```sh" && lines[index] != "```sh" {
            return Err(ReviewError::IndentedOpeningFence);
        }
        if !lines[index].starts_with("```") {
            return Err(ReviewError::InterveningContent);
        }
        if lines[index] != "```sh" {
            return Err(ReviewError::WrongFenceLanguage);
        }
        index += 1;
        let start = index;
        while index < lines.len() && lines[index] != "```" {
            if lines[index].trim_start() == "```" {
                return Err(ReviewError::IndentedClosingFence);
            }
            if lines[index].starts_with([' ', '\t']) {
                return Err(ReviewError::IndentedCommand);
            }
            index += 1;
        }
        if index == lines.len() {
            return Err(ReviewError::UnclosedFence);
        }
        let tokens = tokenize(&lines[start..index])?;
        routes.push(AnchoredRoute {
            kind,
            source_line,
            tokens,
        });
        index += 1;
    }
    Ok(routes)
}

fn route_role(route: &AnchoredRoute) -> Option<&str> {
    route
        .tokens
        .windows(2)
        .find(|pair| pair[0] == "--role")
        .map(|pair| pair[1].as_str())
        .filter(|role| ROLE_REGISTRY.contains(role))
}

fn environment_entry(name: &str, value: &OsStr) -> OsString {
    let mut entry = OsString::from(name);
    entry.push(value);
    entry
}

fn placeholder_value(placeholder: Placeholder, bindings: &ReviewBindings) -> &OsStr {
    match placeholder {
        Placeholder::Cwd => bindings.cwd.as_os_str(),
        Placeholder::Schema => bindings.schema.as_os_str(),
        Placeholder::Output => bindings.output.as_os_str(),
        Placeholder::PlanFile => bindings.plan_file.as_os_str(),
        Placeholder::LogFile => bindings.log_file.as_os_str(),
        Placeholder::PathEnv => bindings.shim_dir.as_os_str(),
        Placeholder::HomeEnv => bindings.child_home.as_os_str(),
        Placeholder::UserEnv => bindings.child_user.as_os_str(),
        Placeholder::Node => OsStr::new("m7-s1"),
        Placeholder::MilestoneNode => OsStr::new("m7-s1"),
        Placeholder::StepNode => OsStr::new("m7-s2"),
        Placeholder::Role => bindings.role.as_os_str(),
        Placeholder::Ref => bindings.dispatch_ref.as_os_str(),
        Placeholder::Evidence => bindings.evidence.as_os_str(),
        Placeholder::PlanningAct => bindings.planning_act.as_os_str(),
        Placeholder::AbsPath => bindings.cwd.as_os_str(),
        Placeholder::CallerArg => bindings.caller_arg.as_os_str(),
    }
}

fn substitute_route(
    route: &AnchoredRoute,
    bindings: &ReviewBindings,
) -> Result<Vec<OsString>, ReviewError> {
    route
        .tokens
        .iter()
        .map(|token| {
            let opens = token.matches("{{").count();
            let closes = token.matches("}}").count();
            if opens == 0 && closes == 0 {
                if token.contains('{') || token.contains('}') {
                    return Err(ReviewError::MalformedPlaceholder);
                }
                return Ok(OsString::from(token));
            }
            if opens != 1 || closes != 1 || !token.starts_with("{{") || !token.ends_with("}}") {
                return Err(ReviewError::EmbeddedPlaceholder);
            }
            let placeholder = Placeholder::parse(token)?;
            let role = route_role(route);
            let contextual;
            let value = match placeholder {
                Placeholder::Schema
                    if role.is_some_and(|role| {
                        matches!(role, "milestone-planner" | "step-planner")
                    }) =>
                {
                    bindings.graph_schema.as_os_str()
                }
                Placeholder::Schema if role.is_some() => bindings.verdict_schema.as_os_str(),
                Placeholder::Output
                    if role.is_some_and(|role| {
                        matches!(role, "milestone-planner" | "step-planner")
                    }) =>
                {
                    bindings
                        .graph_outputs
                        .get(role.expect("planner role"))
                        .expect("planner output")
                        .as_os_str()
                }
                Placeholder::Output if role.is_some_and(|role| role == "step-executor") => {
                    contextual = bindings.verdict_root.join("executor-result.json");
                    contextual.as_os_str()
                }
                Placeholder::Output if role.is_some() => {
                    let role = role.expect("canonical role");
                    let node = match role {
                        "repository-analyst" | "milestone-critic" => "m1-s1",
                        "step-critic" => "m7-s1",
                        _ => "m7-s2",
                    };
                    let prior = bindings
                        .context
                        .accepted_events
                        .iter()
                        .filter(|event| {
                            event["kind"] == "dispatch"
                                && event["node"] == node
                                && event["role"] == role
                        })
                        .count();
                    contextual = bindings.context.review_directories[role].join(format!(
                        "review-{}.json",
                        prior.checked_add(1).expect("review index")
                    ));
                    contextual.as_os_str()
                }
                Placeholder::AbsPath if role.is_some_and(|role| role == "step-executor") => {
                    bindings.complete_git_dir.as_os_str()
                }
                Placeholder::Evidence if role.is_some() => bindings
                    .evidence_by_role
                    .get(role.expect("canonical role"))
                    .expect("role evidence")
                    .as_os_str(),
                _ => placeholder_value(placeholder, bindings),
            };
            if value.is_empty() {
                return Err(match placeholder {
                    Placeholder::Role => ReviewError::EmptyRole,
                    Placeholder::Ref => ReviewError::EmptyRef,
                    Placeholder::Evidence => ReviewError::EmptyEvidence,
                    Placeholder::PlanningAct => ReviewError::RoleSemantics,
                    _ => ReviewError::ParentGrammar,
                });
            }
            match placeholder {
                Placeholder::PathEnv => Ok(environment_entry("PATH=", value)),
                Placeholder::HomeEnv => Ok(environment_entry("HOME=", value)),
                Placeholder::UserEnv => Ok(environment_entry("USER=", value)),
                _ => Ok(value.to_owned()),
            }
        })
        .collect()
}

fn value<'a>(
    argv: &'a [OsString],
    position: &mut usize,
    flag: &str,
) -> Result<&'a OsStr, ReviewError> {
    if argv.get(*position).is_none_or(|item| item != flag) {
        return Err(ReviewError::ParentGrammar);
    }
    *position += 1;
    let result = argv.get(*position).ok_or(ReviewError::ParentGrammar)?;
    if result.is_empty() {
        return Err(ReviewError::ParentGrammar);
    }
    *position += 1;
    Ok(result)
}

fn validate_route(route: &AnchoredRoute, argv: &[OsString]) -> Result<(), ReviewError> {
    if argv.first().is_none_or(|token| token != "pce") {
        return Err(ReviewError::WrongFirstToken);
    }
    if argv.get(1).is_none_or(|token| token != "dispatch") {
        return Err(ReviewError::WrongSecondToken);
    }
    if argv.get(2).is_none_or(|token| token != route.kind.target()) {
        return Err(ReviewError::TargetKindMismatch);
    }
    let delimiter_count = argv
        .iter()
        .filter(|token| token.as_os_str() == "--")
        .count();
    if delimiter_count != 1 {
        return Err(ReviewError::MultipleDelimiters);
    }
    let mut position = 3;
    let cwd = value(argv, &mut position, "--cwd")?;
    if !Path::new(cwd).is_absolute() {
        return Err(ReviewError::ParentGrammar);
    }
    if let Some(required) = route.kind.required_sandbox()
        && value(argv, &mut position, "--sandbox")? != required
    {
        return Err(ReviewError::ParentGrammar);
    }
    let mut env_count = 0;
    while argv.get(position).is_some_and(|token| token == "--env") {
        let entry = value(argv, &mut position, "--env")?;
        let text = entry.to_string_lossy();
        if !text.contains('=') || text.starts_with('=') {
            return Err(ReviewError::ParentGrammar);
        }
        if route.kind == RouteKind::GateStructured && text.starts_with("ANTHROPIC_API_KEY=") {
            return Err(ReviewError::ParentGrammar);
        }
        env_count += 1;
    }
    if env_count == 0 {
        return Err(ReviewError::ZeroEnvironmentEntries(env_count));
    }
    if env_count != REQUIRED_ENVIRONMENT_ENTRIES {
        return Err(ReviewError::WrongEnvironmentCardinality(env_count));
    }
    let has_structured = argv
        .get(position)
        .is_some_and(|token| token == "--output-schema");
    let mut parent_output = None;
    if has_structured {
        let schema = value(argv, &mut position, "--output-schema")?;
        let output = value(argv, &mut position, "-o")?;
        if !Path::new(schema).is_absolute() || !Path::new(output).is_absolute() {
            return Err(ReviewError::ParentGrammar);
        }
        parent_output = Some(output.to_owned());
    }
    if route.kind.requires_structured() != has_structured {
        return Err(ReviewError::ParentGrammar);
    }
    if argv
        .get(position)
        .is_some_and(|token| token == "--plan-file")
    {
        let plan = value(argv, &mut position, "--plan-file")?;
        if !route.kind.permits_plan_input() || !Path::new(plan).is_file() {
            return Err(ReviewError::ParentGrammar);
        }
    }
    let logging = argv
        .get(position)
        .is_some_and(|token| token == "--log-file");
    if logging {
        value(argv, &mut position, "--log-file")?;
        value(argv, &mut position, "--node")?;
        value(argv, &mut position, "--role")?;
        value(argv, &mut position, "--ref")?;
        value(argv, &mut position, "--evidence")?;
        let required_artifact = value(argv, &mut position, "--required-artifact")?;
        if !Path::new(required_artifact).is_absolute() {
            return Err(ReviewError::ParentGrammar);
        }
        if parent_output
            .as_deref()
            .is_some_and(|output| output != required_artifact)
        {
            return Err(ReviewError::ParentGrammar);
        }
        if argv
            .get(position)
            .is_some_and(|token| token == "--planning-act")
        {
            let planning_act = value(argv, &mut position, "--planning-act")?;
            if planning_act != "repeatable" && planning_act != "irreversible" {
                return Err(ReviewError::RoleSemantics);
            }
        }
        if argv.get(position).is_some_and(|token| token == "--dry-run") {
            position += 1;
        }
    }
    if argv.get(position).is_none_or(|token| token != "--") {
        return Err(ReviewError::ParentGrammar);
    }
    let tail = &argv[position + 1..];
    if tail.iter().any(|token| token == "--dry-run") {
        return Err(ReviewError::ParentGrammar);
    }
    if let Some(arity) = route.kind.caller_tail_arity()
        && tail.len() != arity
    {
        return Err(ReviewError::PurposeTailCardinality(tail.len()));
    }
    if route.kind != RouteKind::GateStructured {
        for (index, token) in tail.iter().enumerate() {
            let token = token.to_string_lossy();
            let forbidden = (index == 0 && token == "exec")
                || matches!(
                    token.as_ref(),
                    "--json" | "-C" | "--sandbox" | "--output-schema" | "-o"
                )
                || token.starts_with("--sandbox=")
                || token.starts_with("--output-schema=");
            if forbidden {
                return Err(ReviewError::CallerTailResuppliesBinaryArgument);
            }
        }
    } else if tail.iter().any(|token| {
        let token = token.to_string_lossy();
        matches!(token.as_ref(), "-p" | "--output-format") || token.starts_with("--output-format=")
    }) {
        return Err(ReviewError::CallerTailResuppliesBinaryArgument);
    } else if route_role(route) == Some("falsification-critic") {
        if tail.len() != 1 {
            return Err(ReviewError::GateTailCardinality);
        }
        if tail[0].is_empty() {
            return Err(ReviewError::GateTailPrompt);
        }
        if tail.iter().any(|token| {
            let token = token.to_string_lossy();
            token == "--append-system-prompt" || token.starts_with("--append-system-prompt=")
        }) {
            return Err(ReviewError::CallerTailResuppliesBinaryArgument);
        }
    } else if route_role(route).is_some() {
        if tail.len() != 3 {
            return Err(ReviewError::GateTailCardinality);
        }
        if tail[0] != "--append-system-prompt" {
            return Err(ReviewError::GateTailDesignator);
        }
        if tail[1].is_empty() {
            return Err(ReviewError::GateTailValue);
        }
        if !Path::new(&tail[1]).is_absolute() {
            return Err(ReviewError::GateTailAbsolute);
        }
        if tail[2].is_empty() {
            return Err(ReviewError::GateTailPrompt);
        }
        if parent_output.as_deref() != Some(tail[1].as_os_str()) {
            return Err(ReviewError::GateOutputPathMismatch);
        }
    }
    Ok(())
}

fn read_nul(path: &Path) -> Vec<Vec<u8>> {
    fs::read(path)
        .unwrap_or_default()
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(<[u8]>::to_vec)
        .collect()
}

fn execute_route(
    argv: &[OsString],
    bindings: &ReviewBindings,
) -> Result<RouteObservation, ReviewError> {
    let log_path = argv
        .windows(2)
        .find(|pair| pair[0] == "--log-file")
        .map(|pair| PathBuf::from(&pair[1]));
    let prior_records = log_path
        .as_ref()
        .and_then(|path| fs::read_to_string(path).ok())
        .map_or(0, |contents| contents.lines().count());
    bindings
        .pce_invocations
        .set(bindings.pce_invocations.get() + 1);
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(&argv[1..])
        .env("PCE_INHERITED_SENTINEL", "must-be-cleared")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| ReviewError::Spawn(error.to_string()))?;
    let status = output.status.code().unwrap_or(-1);
    if !output.status.success() {
        return Err(ReviewError::Spawn(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    if let Some(log_path) = log_path {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let settled = fs::read_to_string(&log_path)
                .ok()
                .is_some_and(|contents| contents.lines().count() == prior_records + 2);
            if settled {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "dispatch lifecycle did not settle for {}",
                log_path.display()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(RouteObservation {
        source_line: 0,
        status,
        child_argv: read_nul(&bindings.cwd.join(".review/argv")),
    })
}

fn expected_kind(role: &str) -> RouteKind {
    if matches!(
        role,
        "repository-analyst"
            | "milestone-critic"
            | "step-critic"
            | "step-plan-critic"
            | "falsification-critic"
            | "pr-reviewer"
    ) {
        RouteKind::GateStructured
    } else if role == "step-plan-writer" {
        RouteKind::CodexUnstructured
    } else {
        RouteKind::CodexStructured
    }
}

fn expected_node(role: &str) -> &str {
    match role {
        "repository-analyst" | "milestone-planner" | "milestone-critic" => "m1-s1",
        "step-planner" | "step-critic" => "{{MILESTONE_NODE}}",
        _ => "{{STEP_NODE}}",
    }
}

fn canonical_semantics(route: &AnchoredRoute) -> Result<(), ReviewError> {
    let planning_act_count = route
        .tokens
        .iter()
        .filter(|token| token.as_str() == "--planning-act")
        .count();
    if route.kind.is_purpose() && planning_act_count != 0 {
        return Err(ReviewError::RoleSemantics);
    }
    let Some(role) = route_role(route) else {
        return Ok(());
    };
    if route.kind != expected_kind(role) {
        return Err(ReviewError::RoleSemantics);
    }
    let tokens = &route.tokens;
    let count = |needle: &str| {
        tokens
            .iter()
            .filter(|token| token.as_str() == needle)
            .count()
    };
    if count("--env") != REQUIRED_ENVIRONMENT_ENTRIES
        || ENVIRONMENT_PLACEHOLDERS
            .iter()
            .any(|placeholder| count(placeholder) != 1)
    {
        return Err(ReviewError::RoleSemantics);
    }
    let planning_role = matches!(role, "step-plan-writer" | "step-plan-critic");
    if !planning_role && (planning_act_count != 0 || count("{{PLANNING_ACT}}") != 0) {
        return Err(ReviewError::RoleSemantics);
    }
    if !has_complete_logging_group(route) {
        return Err(ReviewError::ParentGrammar);
    }
    if count("--ref") != 1
        || count("{{REF}}") != 1
        || count("--evidence") != 1
        || count("{{EVIDENCE}}") != 1
    {
        return Err(ReviewError::RoleSemantics);
    }
    if planning_role {
        let logging = expected_logging_group(route).ok_or(ReviewError::RoleSemantics)?;
        let logging_start = tokens
            .windows(logging.len())
            .position(|window| window.iter().eq(logging.iter()))
            .ok_or(ReviewError::RoleSemantics)?;
        let planning_start = logging_start + logging.len();
        if planning_act_count != 1
            || count("{{PLANNING_ACT}}") != 1
            || tokens.get(planning_start).map(String::as_str) != Some("--planning-act")
            || tokens.get(planning_start + 1).map(String::as_str) != Some("{{PLANNING_ACT}}")
            || tokens.get(planning_start + 2).map(String::as_str) != Some("--")
        {
            return Err(ReviewError::RoleSemantics);
        }
    }
    let structured = count("--output-schema") == 1
        && count("{{SCHEMA}}") == 1
        && count("-o") == 1
        && count("{{OUTPUT}}") >= 1;
    if (role == "step-plan-writer") == structured {
        return Err(ReviewError::RoleSemantics);
    }
    if (role == "step-executor") != (count("--plan-file") == 1) {
        return Err(ReviewError::RoleSemantics);
    }
    if role == "step-executor" {
        let delimiter = tokens
            .iter()
            .position(|token| token == "--")
            .ok_or(ReviewError::RoleSemantics)?;
        if tokens.get(delimiter + 1..)
            != Some(
                &[
                    "--add-dir".to_owned(),
                    "{{ABS_PATH}}".to_owned(),
                    "{{CALLER_ARG}}".to_owned(),
                ][..],
            )
        {
            return Err(ReviewError::RoleSemantics);
        }
    }
    Ok(())
}

fn expected_logging_group(route: &AnchoredRoute) -> Option<Vec<String>> {
    let (node, role) = match (route_role(route), route.kind.is_purpose()) {
        (Some(role), false) => (expected_node(role).to_owned(), role.to_owned()),
        (None, true) => ("{{NODE}}".to_owned(), "{{ROLE}}".to_owned()),
        _ => return None,
    };
    Some(vec![
        "--log-file".to_owned(),
        "{{LOG_FILE}}".to_owned(),
        "--node".to_owned(),
        node,
        "--role".to_owned(),
        role,
        "--ref".to_owned(),
        "{{REF}}".to_owned(),
        "--evidence".to_owned(),
        "{{EVIDENCE}}".to_owned(),
        "--required-artifact".to_owned(),
        "{{OUTPUT}}".to_owned(),
    ])
}

fn measured_logging_group(route: &AnchoredRoute) -> Vec<String> {
    let start = route
        .tokens
        .iter()
        .position(|token| token == "--log-file")
        .unwrap_or(route.tokens.len());
    route.tokens[start..]
        .iter()
        .take_while(|token| *token != "--")
        .cloned()
        .collect()
}

fn missing_logging_member(route: &AnchoredRoute) -> Option<String> {
    let expected = expected_logging_group(route)?;
    let measured = measured_logging_group(route);
    expected
        .iter()
        .find(|member| !measured.contains(member))
        .cloned()
}

fn has_complete_logging_group(route: &AnchoredRoute) -> bool {
    let Some(logging) = expected_logging_group(route) else {
        return false;
    };
    route
        .tokens
        .windows(logging.len())
        .any(|window| window.iter().eq(logging.iter()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnchorClass {
    CanonicalRole,
    Purpose,
}

fn token_value<'a>(route: &'a AnchoredRoute, flag: &str) -> Option<&'a str> {
    route
        .tokens
        .windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].as_str())
}

fn classify_anchor(route: &AnchoredRoute) -> Option<AnchorClass> {
    if route.kind.is_purpose() {
        if route_role(route).is_none()
            && token_value(route, "--role") == Some("{{ROLE}}")
            && token_value(route, "--node") == Some("{{NODE}}")
        {
            return Some(AnchorClass::Purpose);
        }
        return None;
    }
    if route.kind.is_canonical_role() && route_role(route).is_some() {
        return Some(AnchorClass::CanonicalRole);
    }
    None
}

fn anchor_identity(route: &AnchoredRoute) -> Option<String> {
    match classify_anchor(route)? {
        AnchorClass::CanonicalRole => Some(format!("role/{}", route_role(route)?)),
        AnchorClass::Purpose => Some(format!("purpose/{}", route.kind.purpose_kind()?)),
    }
}

fn semantic_output_path(
    route: &AnchoredRoute,
    argv: &[OsString],
    bindings: &ReviewBindings,
) -> Result<(), ReviewError> {
    let Some(role) = route_role(route) else {
        return Ok(());
    };
    if route.kind != RouteKind::GateStructured {
        return Ok(());
    }
    let field = |flag: &str| {
        argv.windows(2)
            .find(|pair| pair[0] == flag)
            .map(|pair| pair[1].as_os_str())
    };
    let node = field("--node").ok_or(ReviewError::RoleSemantics)?;
    let output = Path::new(field("-o").ok_or(ReviewError::RoleSemantics)?);
    let directory = bindings
        .context
        .review_directories
        .get(role)
        .ok_or(ReviewError::WrongVerdictDirectory)?;
    if output.parent() != Some(directory.as_path()) {
        return Err(ReviewError::WrongVerdictDirectory);
    }
    let prior = bindings
        .context
        .accepted_events
        .iter()
        .filter(|event| {
            event["kind"] == "dispatch"
                && event["node"]
                    .as_str()
                    .is_some_and(|value| OsStr::new(value) == node)
                && event["role"] == role
        })
        .count();
    let expected = format!(
        "review-{}.json",
        prior.checked_add(1).ok_or(ReviewError::WrongVerdictIndex)?
    );
    if output.file_name() != Some(OsStr::new(&expected)) {
        return Err(ReviewError::WrongVerdictIndex);
    }
    Ok(())
}

const DISPATCH_OPTIONS: [&str; 14] = [
    "--cwd",
    "--sandbox",
    "--env",
    "--output-schema",
    "-o",
    "--plan-file",
    "--log-file",
    "--node",
    "--role",
    "--ref",
    "--evidence",
    "--required-artifact",
    "--planning-act",
    "--dry-run",
];

fn unit_has_prohibited_fragment(unit: &str) -> bool {
    let target = unit.contains("pce dispatch codex") || unit.contains("pce dispatch gate");
    let tokens: Vec<&str> = unit.split_ascii_whitespace().collect();
    let second = tokens.iter().enumerate().any(|(index, token)| {
        *token == "--"
            || DISPATCH_OPTIONS.iter().any(|option| {
                *token == *option
                    || token
                        .strip_prefix(option)
                        .is_some_and(|rest| rest.starts_with('='))
            })
            || tokens[..index].contains(&"--")
    });
    target && second
}

fn shell_like(line: &str) -> bool {
    let mut text = line.trim_start_matches([' ', '\t']);
    for prefix in ["> ", "- ", "* ", "+ "] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest;
            break;
        }
    }
    if let Some((marker, rest)) = text.split_once(' ')
        && marker.ends_with('.')
        && marker[..marker.len() - 1]
            .chars()
            .all(|c| c.is_ascii_digit())
    {
        text = rest;
    }
    let mut tokens = text.split_ascii_whitespace();
    let first = tokens.next().unwrap_or("");
    let first = if first == "$" {
        tokens.next().unwrap_or("")
    } else {
        first
    };
    first == "pce"
        || first == "--"
        || DISPATCH_OPTIONS.iter().any(|option| {
            first == *option
                || first
                    .strip_prefix(option)
                    .is_some_and(|rest| rest.starts_with('='))
        })
}

fn outside_fragment_units(markdown: &str) -> Vec<String> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut units = Vec::new();
    let mut in_anchor = false;
    let mut in_fence = false;
    let mut fence = String::new();
    for line in lines {
        if line.starts_with(MARKER_START) {
            in_anchor = true;
            continue;
        }
        if in_anchor {
            if line == "```" {
                in_anchor = false;
            }
            continue;
        }
        if line.starts_with("```") {
            if in_fence {
                units.push(fence.clone());
                fence.clear();
                in_fence = false;
            } else {
                in_fence = true;
            }
            continue;
        }
        if in_fence {
            fence.push_str(line);
            fence.push('\n');
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            rest = &rest[start + 1..];
            let Some(end) = rest.find('`') else { break };
            units.push(rest[..end].to_owned());
            rest = &rest[end + 1..];
        }
        if shell_like(line) {
            units.push(line.to_owned());
        }
    }
    units
}

fn reject_outside_fragments(markdown: &str) -> Result<(), ReviewError> {
    if outside_fragment_units(markdown)
        .iter()
        .any(|unit| unit_has_prohibited_fragment(unit))
    {
        return Err(ReviewError::OutsideAnchorDispatchFragment);
    }
    Ok(())
}

fn is_standalone_dispatch_append(line: &str) -> bool {
    if !shell_like(line) {
        return false;
    }
    let tokens: Vec<_> = line.split_ascii_whitespace().collect();
    let command = tokens.windows(2).position(|pair| pair == ["pce", "log"]);
    let kind = tokens
        .windows(2)
        .position(|pair| pair == ["--kind", "dispatch"]);
    command
        .zip(kind)
        .is_some_and(|(command, kind)| command < kind)
}

fn standalone_dispatch_append_lines(markdown: &str) -> Vec<usize> {
    let mut in_anchor = false;
    let mut matches = Vec::new();
    for (index, line) in markdown.lines().enumerate() {
        if line.starts_with(MARKER_START) {
            in_anchor = true;
            continue;
        }
        if in_anchor {
            if line == "```" {
                in_anchor = false;
            }
            continue;
        }
        if is_standalone_dispatch_append(line) {
            matches.push(index + 1);
        }
    }
    matches
}

fn atx_heading_lines(markdown: &str) -> Vec<bool> {
    let mut fenced = false;
    markdown
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                return false;
            }
            if fenced {
                return false;
            }
            line.split_once(' ').is_some_and(|(hashes, _)| {
                (1..=6).contains(&hashes.len()) && hashes.chars().all(|c| c == '#')
            })
        })
        .collect()
}

fn append_lines_by_venue(markdown: &str) -> BTreeMap<&'static str, Vec<usize>> {
    let lines: Vec<_> = markdown.lines().collect();
    let mut by_venue: BTreeMap<&'static str, Vec<usize>> = BTreeMap::new();
    for source_line in standalone_dispatch_append_lines(markdown) {
        let heading = lines[..source_line]
            .iter()
            .rev()
            .find(|line| line.starts_with("## "))
            .copied()
            .unwrap_or("");
        let venue = if heading == "## Event log contract" {
            "general-surface"
        } else if heading.starts_with("## Phase 0") {
            "phase-0"
        } else if heading.starts_with("## Phase 1") {
            "phase-1"
        } else if heading.starts_with("## Phase 2") {
            "phase-2"
        } else if heading.starts_with("## Phase 3") {
            "phase-3"
        } else {
            "other"
        };
        by_venue.entry(venue).or_default().push(source_line);
    }
    by_venue
}

fn reject_colocated_standalone_append(markdown: &str) -> Result<(), ReviewError> {
    if !markdown.contains(CONSOLIDATION_MARKER) {
        return Ok(());
    }
    let mut section = String::new();
    let inspect = |section: &str| {
        let anchor = section.contains(MARKER_START);
        let append = section.lines().any(is_standalone_dispatch_append);
        anchor && append
    };
    let headings = atx_heading_lines(markdown);
    for (index, line) in markdown.lines().enumerate() {
        let heading = headings[index];
        if heading && inspect(&section) {
            return Err(ReviewError::CoLocatedStandaloneDispatchAppend);
        }
        if heading {
            section.clear();
        }
        section.push_str(line);
        section.push('\n');
    }
    if inspect(&section) {
        return Err(ReviewError::CoLocatedStandaloneDispatchAppend);
    }
    Ok(())
}

fn review_document(markdown: &str, bindings: &ReviewBindings) -> Result<ReviewReport, ReviewError> {
    review_document_attributed(markdown, bindings).map_err(|failure| failure.error)
}

fn review_document_attributed(
    markdown: &str,
    bindings: &ReviewBindings,
) -> Result<ReviewReport, AttributedReviewError> {
    if markdown.contains("codex exec") {
        return Err(AttributedReviewError {
            source_line: 0,
            error: ReviewError::RawCodexExec,
            observations: Vec::new(),
        });
    }
    let global = |error| AttributedReviewError {
        source_line: 0,
        error,
        observations: Vec::new(),
    };
    let routes = extract_anchored_routes(markdown).map_err(global)?;
    reject_outside_fragments(markdown).map_err(global)?;
    reject_colocated_standalone_append(markdown).map_err(global)?;
    let mut observations = Vec::new();
    for route in &routes {
        let failure = |error| AttributedReviewError {
            source_line: route.source_line,
            error,
            observations: observations.clone(),
        };
        canonical_semantics(route).map_err(failure)?;
        let argv = substitute_route(route, bindings).map_err(failure)?;
        validate_route(route, &argv).map_err(failure)?;
        semantic_output_path(route, &argv, bindings).map_err(failure)?;
        let mut observation = execute_route(&argv, bindings).map_err(failure)?;
        observation.source_line = route.source_line;
        observations.push(observation);
    }
    Ok(ReviewReport {
        routes: routes
            .iter()
            .map(|route| (route.source_line, route.kind))
            .collect(),
        observations,
    })
}

fn error_for(markdown: &str) -> ReviewError {
    let fixture = fixture();
    match review_document(markdown, &fixture.bindings) {
        Ok(report) => {
            panic!("fixture must red, got {report:?}")
        }
        Err(error) => {
            assert_eq!(
                fixture.bindings.pce_invocations.get(),
                0,
                "review error occurred after pce invocation"
            );
            error
        }
    }
}

fn assert_document_error_before_pce(name: &str, markdown: &str, expected: ReviewError) {
    let fixture = fixture();
    let before = fixture.bindings.pce_invocations.get();
    let error = match review_document(markdown, &fixture.bindings) {
        Ok(report) => panic!("{name}: fixture must red, got {report:?}"),
        Err(error) => error,
    };
    assert_eq!(error, expected, "{name}");
    assert_eq!(
        before,
        fixture.bindings.pce_invocations.get(),
        "{name}: pce invocation changed"
    );
}

fn run_pce(args: &[OsString], stdin: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
    command
        .args(args)
        .env("PCE_INHERITED_SENTINEL", "must-be-cleared")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = command.spawn().expect("pce test binary");
    if let Some(bytes) = stdin {
        child
            .stdin
            .as_mut()
            .expect("piped stdin")
            .write_all(bytes)
            .expect("stdin write");
    }
    child.wait_with_output().expect("pce output")
}

fn run_direct_shim(fixture: &Fixture, executable: &str, args: &[OsString]) -> Output {
    Command::new(fixture.bindings.shim_dir.join(executable))
        .args(args)
        .current_dir(&fixture.bindings.cwd)
        .env_clear()
        .env("PATH", &fixture.bindings.shim_dir)
        .stdin(Stdio::null())
        .output()
        .expect("direct conformance shim")
}

fn repository_route(role: &str) -> AnchoredRoute {
    repository_routes()
        .into_iter()
        .find(|route| route_role(route) == Some(role))
        .unwrap_or_else(|| panic!("missing repository route {role}"))
}

fn route_document(route: &AnchoredRoute) -> String {
    format!(
        "{}\n```sh\n{}\n```\n",
        marker(match route.kind {
            RouteKind::CodexUnstructured => "codex-unstructured",
            RouteKind::CodexStructured => "codex-structured",
            RouteKind::GateStructured => "gate-structured",
            RouteKind::CodexCommitCompletion => "codex-commit-completion",
            RouteKind::CodexDiagnostics => "codex-diagnostics",
        }),
        route.tokens.join(" ")
    )
}

fn assert_parser_red(name: &str, args: Vec<OsString>, diagnostic: &str) {
    let output = run_pce(&args, None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{name}: unexpectedly accepted");
    assert!(
        stderr.contains(diagnostic),
        "{name}: expected {diagnostic:?}, got {stderr:?}"
    );
}

fn assert_parser_green(name: &str, args: Vec<OsString>) {
    let output = run_pce(&args, None);
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn complete_args(f: &Fixture, target: &str) -> Vec<OsString> {
    let mut args = vec![
        "dispatch".into(),
        target.into(),
        "--cwd".into(),
        f.bindings.cwd.as_os_str().to_owned(),
    ];
    if target == "codex" {
        args.extend(["--sandbox".into(), "workspace-write".into()]);
    }
    args.extend([
        "--env".into(),
        "A=1".into(),
        "--output-schema".into(),
        f.bindings.schema.as_os_str().to_owned(),
        "-o".into(),
        f.bindings.output.as_os_str().to_owned(),
        "--plan-file".into(),
        f.bindings.plan_file.as_os_str().to_owned(),
        "--log-file".into(),
        f.bindings.log_file.as_os_str().to_owned(),
        "--node".into(),
        "m7-s1".into(),
        "--role".into(),
        "step-plan-writer".into(),
        "--ref".into(),
        "07b85ccd".into(),
        "--evidence".into(),
        "fixture".into(),
        "--required-artifact".into(),
        f.bindings.output.as_os_str().to_owned(),
        "--dry-run".into(),
        "--".into(),
        "tail".into(),
    ]);
    args
}

#[test]
fn real_skill_has_exact_pinned_anchor_partition_twelve() {
    let fixture = fixture();
    let markdown = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("repository skill");
    let report = review_document(&markdown, &fixture.bindings).expect("real skill review");
    let routes = extract_anchored_routes(&markdown).expect("route extraction");
    assert_classification_total(&routes);
    let roles: std::collections::BTreeSet<_> = routes.iter().filter_map(route_role).collect();
    let expected: std::collections::BTreeSet<_> = ROLE_REGISTRY.into_iter().collect();
    assert_eq!(roles, expected);
    let role_measured = routes
        .iter()
        .filter(|route| classify_anchor(route) == Some(AnchorClass::CanonicalRole))
        .count();
    let purpose_measured = routes
        .iter()
        .filter(|route| classify_anchor(route) == Some(AnchorClass::Purpose))
        .count();
    assert!(
        report.routes.len() == EXPECTED_ANCHORED_ROUTE_COUNT
            && report.observations.len() == EXPECTED_ANCHORED_ROUTE_COUNT
            && role_measured == EXPECTED_ROLE_ANCHOR_COUNT
            && purpose_measured == EXPECTED_PURPOSE_ANCHOR_COUNT,
        "real_skill_has_exact_pinned_anchor_partition_twelve/partition: routes={} expected={}, observations={} expected={}, roles={} expected={}, purposes={} expected={}",
        report.routes.len(),
        EXPECTED_ANCHORED_ROUTE_COUNT,
        report.observations.len(),
        EXPECTED_ANCHORED_ROUTE_COUNT,
        role_measured,
        EXPECTED_ROLE_ANCHOR_COUNT,
        purpose_measured,
        EXPECTED_PURPOSE_ANCHOR_COUNT
    );
    let identities: std::collections::BTreeSet<String> =
        routes.iter().filter_map(anchor_identity).collect();
    let mut expected_identities: std::collections::BTreeSet<String> = ROLE_REGISTRY
        .into_iter()
        .map(|role| format!("role/{role}"))
        .collect();
    expected_identities.insert("purpose/codex-commit-completion".to_owned());
    expected_identities.insert("purpose/codex-diagnostics".to_owned());
    assert_eq!(identities, expected_identities);
    assert!(
        routes
            .iter()
            .all(|route| route.kind.is_purpose() != route.kind.is_canonical_role())
    );
}

fn assert_indentation_fixture(name: &str, mutation: fn(String) -> String, expected: ReviewError) {
    let document = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured),
    );
    assert_document_error_before_pce(name, &mutation(document), expected);
}

#[test]
fn column_zero_anchor_accepts() {
    review_document(
        &document(
            "codex-unstructured",
            &base_command(RouteKind::CodexUnstructured),
        ),
        &fixture().bindings,
    )
    .expect("column_zero_anchor_accepts");
}

#[test]
fn indented_anchor_marker_reds() {
    assert_indentation_fixture(
        "indented_anchor_marker_reds",
        |text| text.replacen(MARKER_START, &format!(" {MARKER_START}"), 1),
        ReviewError::IndentedMarker,
    );
}

#[test]
fn indented_anchor_opening_fence_reds() {
    assert_indentation_fixture(
        "indented_anchor_opening_fence_reds",
        |text| text.replacen("```sh", " ```sh", 1),
        ReviewError::IndentedOpeningFence,
    );
}

#[test]
fn indented_anchor_command_reds() {
    assert_indentation_fixture(
        "indented_anchor_command_reds",
        |text| text.replacen("pce dispatch", " pce dispatch", 1),
        ReviewError::IndentedCommand,
    );
}

#[test]
fn indented_anchor_closing_fence_reds() {
    assert_indentation_fixture(
        "indented_anchor_closing_fence_reds",
        |text| text.replacen("\n```\n", "\n ```\n", 1),
        ReviewError::IndentedClosingFence,
    );
}

#[test]
fn route_zero_env_reds() {
    let markdown = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured).replace(
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} ",
            "",
        ),
    );
    assert_document_error_before_pce(
        "route_zero_env_reds",
        &markdown,
        ReviewError::ZeroEnvironmentEntries(0),
    );
}

#[test]
fn route_exactly_three_env_accepts() {
    let markdown = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured),
    );
    review_document(&markdown, &fixture().bindings).expect("route_exactly_three_env_accepts");
}

#[test]
fn route_one_env_reds() {
    let markdown = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured).replace(
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}}",
            "--env {{PATH_ENV}}",
        ),
    );
    assert_document_error_before_pce(
        "route_one_env_reds",
        &markdown,
        ReviewError::WrongEnvironmentCardinality(1),
    );
}

#[test]
fn route_two_env_reds() {
    let markdown = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured).replace(
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}}",
            "--env {{PATH_ENV}} --env {{HOME_ENV}}",
        ),
    );
    assert_document_error_before_pce(
        "route_two_env_reds",
        &markdown,
        ReviewError::WrongEnvironmentCardinality(2),
    );
}

#[test]
fn route_four_env_reds() {
    let markdown = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured).replace(
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}}",
            "--env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} --env {{PATH_ENV}}",
        ),
    );
    assert_document_error_before_pce(
        "route_four_env_reds",
        &markdown,
        ReviewError::WrongEnvironmentCardinality(4),
    );
}

#[test]
fn two_anchor_error_attribution() {
    let first = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured),
    );
    let second = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured).replace("{{CALLER_ARG}}", "{{UNKNOWN}}"),
    );
    let markdown = format!("{first}{second}");
    let routes = extract_anchored_routes(&markdown).expect("two routes");
    let f = fixture();
    let failure = review_document_attributed(&markdown, &f.bindings).expect_err("second reds");
    assert_eq!(failure.error, ReviewError::UnknownPlaceholder);
    assert_eq!(
        failure.source_line, routes[1].source_line,
        "second source identity"
    );
    assert_eq!(failure.observations.len(), 1, "first observation only");
    assert_eq!(
        failure.observations[0].source_line, routes[0].source_line,
        "first attribution"
    );
}

#[test]
fn two_anchor_first_error_stops_before_second() {
    let first = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured).replace("{{CALLER_ARG}}", "{{UNKNOWN}}"),
    );
    let second = document(
        "codex-unstructured",
        &base_command(RouteKind::CodexUnstructured),
    );
    let markdown = format!("{first}{second}");
    let routes = extract_anchored_routes(&markdown).expect("two routes");
    let f = fixture();
    let failure = review_document_attributed(&markdown, &f.bindings).expect_err("first reds");
    assert_eq!(failure.error, ReviewError::UnknownPlaceholder);
    assert_eq!(
        failure.source_line, routes[0].source_line,
        "first source identity"
    );
    assert!(failure.observations.is_empty(), "second was not invoked");
    assert_eq!(
        f.bindings.pce_invocations.get(),
        0,
        "no invocation after first error"
    );
}

#[test]
fn canonical_role_semantics_by_role() {
    let markdown = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("repository skill");
    let all_routes = extract_anchored_routes(&markdown).expect("routes");
    assert_classification_total(&all_routes);
    let routes: Vec<AnchoredRoute> = all_routes
        .into_iter()
        .filter(|route| classify_anchor(route) == Some(AnchorClass::CanonicalRole))
        .collect();
    let mut counts = std::collections::BTreeMap::new();
    for route in &routes {
        let role = route_role(route).expect("canonical role");
        *counts.entry(role).or_insert(0usize) += 1;
        assert_eq!(route.kind, expected_kind(role), "route_kind_by_role/{role}");
        assert_eq!(
            route
                .tokens
                .windows(2)
                .find(|pair| pair[0] == "--node")
                .map(|pair| pair[1].as_str()),
            Some(expected_node(role)),
            "node_class_by_role/{role}"
        );
        assert_eq!(
            route
                .tokens
                .iter()
                .filter(|token| *token == "--env")
                .count(),
            REQUIRED_ENVIRONMENT_ENTRIES,
            "environment_cardinality_by_role/{role}"
        );
        assert!(
            canonical_semantics(route).is_ok(),
            "logging_group_by_role/{role}"
        );
        assert_eq!(
            route
                .tokens
                .iter()
                .filter(|token| *token == "{{REF}}")
                .count(),
            1,
            "ref_source_by_role/{role}"
        );
        assert_eq!(
            route
                .tokens
                .iter()
                .filter(|token| *token == "{{EVIDENCE}}")
                .count(),
            1,
            "evidence_source_by_role/{role}"
        );
        let structured = route.tokens.iter().any(|token| token == "--output-schema");
        assert_eq!(
            structured,
            role != "step-plan-writer",
            "schema_and_output_class_by_role/{role}"
        );
        assert_eq!(
            route.tokens.iter().any(|token| token == "--plan-file"),
            role == "step-executor",
            "stdin_class_by_role/{role}"
        );
    }
    let measured: std::collections::BTreeSet<_> = counts.keys().copied().collect();
    let expected: std::collections::BTreeSet<_> = ROLE_REGISTRY.into_iter().collect();
    assert_eq!(measured, expected, "role_registry_exact_set");
    assert!(
        counts.values().all(|count| *count == 1),
        "one_anchor_per_role: {counts:?}"
    );
    let executor = routes
        .iter()
        .find(|route| route_role(route) == Some("step-executor"))
        .expect("executor");
    assert!(
        canonical_semantics(executor).is_ok(),
        "executor_complete_git_parent_required"
    );
}

fn repository_routes() -> Vec<AnchoredRoute> {
    let markdown = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("repository skill");
    extract_anchored_routes(&markdown).expect("repository routes")
}

fn assert_classification_total(routes: &[AnchoredRoute]) {
    let unclassified: Vec<_> = routes
        .iter()
        .filter(|route| classify_anchor(route).is_none())
        .map(|route| {
            (
                route.source_line,
                route.kind,
                token_value(route, "--node").unwrap_or("<none>").to_owned(),
                token_value(route, "--role").unwrap_or("<none>").to_owned(),
            )
        })
        .collect();
    assert!(
        unclassified.is_empty(),
        "anchor_class_is_total: unclassified={unclassified:?}, \
expected node source {{{{NODE}}}} and role source {{{{ROLE}}}} for purpose anchors"
    );
}

fn canonical_role_routes() -> Vec<AnchoredRoute> {
    let routes = repository_routes();
    assert_classification_total(&routes);
    routes
        .into_iter()
        .filter(|route| classify_anchor(route) == Some(AnchorClass::CanonicalRole))
        .collect()
}

#[test]
fn planning_act_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        let expected =
            matches!(role, "step-plan-writer" | "step-plan-critic").then_some("{{PLANNING_ACT}}");
        assert_eq!(
            token_value(&route, "--planning-act"),
            expected,
            "planning_act_by_role/{role}"
        );
    }
}

#[test]
fn planning_act_route_membership_is_exact() {
    for route in repository_routes() {
        let role = route_role(&route);
        let expected = usize::from(matches!(
            role,
            Some("step-plan-writer" | "step-plan-critic")
        ));
        let count = route
            .tokens
            .iter()
            .filter(|token| token.as_str() == "--planning-act")
            .count();
        assert_eq!(
            count,
            expected,
            "planning_act_route_membership_is_exact/{}",
            anchor_identity(&route).expect("anchor identity")
        );
    }
}

#[test]
fn planning_routes_receive_binary_composed_caller_argument() {
    for (role, planning_act, expected) in [
        ("step-plan-writer", "repeatable", REPEATABLE_CALLER_ARGUMENT),
        (
            "step-plan-writer",
            "irreversible",
            IRREVERSIBLE_CALLER_ARGUMENT,
        ),
        ("step-plan-critic", "repeatable", REPEATABLE_CALLER_ARGUMENT),
        (
            "step-plan-critic",
            "irreversible",
            IRREVERSIBLE_CALLER_ARGUMENT,
        ),
    ] {
        let mut fixture = fixture();
        if planning_act == "irreversible" {
            fixture.bindings.planning_act = OsString::from("irreversible");
        }
        let route = repository_route(role);
        let report = review_document(&route_document(&route), &fixture.bindings)
            .unwrap_or_else(|error| panic!("{role}/{planning_act}: {error:?}"));
        assert_eq!(report.observations.len(), 1, "{role}/{planning_act}");
        let observation = &report.observations[0];
        assert_eq!(observation.status, 0, "{role}/{planning_act}");
        assert_eq!(
            observation.child_argv.last().map(Vec::as_slice),
            Some(expected.as_bytes()),
            "{role}/{planning_act}/caller-argument"
        );
        if role == "step-plan-critic" {
            assert_eq!(
                &observation.child_argv
                    [observation.child_argv.len() - 3..observation.child_argv.len() - 1],
                [
                    b"--append-system-prompt".as_slice(),
                    fixture
                        .bindings
                        .verdict_root
                        .join("step-plan-critic/review-3.json")
                        .as_os_str()
                        .as_encoded_bytes(),
                ],
                "{role}/{planning_act}/gate-tail"
            );
        }
        let environment =
            fs::read_to_string(fixture.bindings.cwd.join(".review/env")).expect("environment");
        assert!(
            !environment.contains("PCE_INHERITED_SENTINEL"),
            "{role}/{planning_act}/environment"
        );
        assert_eq!(
            fixture.bindings.pce_invocations.get(),
            1,
            "{role}/{planning_act}"
        );
    }
}

fn assert_planning_route_red(
    name: &str,
    route: AnchoredRoute,
    mutate_bindings: impl FnOnce(&mut ReviewBindings),
) {
    let mut fixture = fixture();
    mutate_bindings(&mut fixture.bindings);
    let error = review_document(&route_document(&route), &fixture.bindings)
        .expect_err("planning route mutation must red");
    assert_eq!(error, ReviewError::RoleSemantics, "{name}");
    assert_eq!(fixture.bindings.pce_invocations.get(), 0, "{name}");
}

#[test]
fn planning_route_mutations_red_before_invocation() {
    const PAIR: &str = "--planning-act {{PLANNING_ACT}}";

    let mut writer_missing = repository_route("step-plan-writer");
    writer_missing.tokens = writer_missing
        .tokens
        .into_iter()
        .filter(|token| !matches!(token.as_str(), "--planning-act" | "{{PLANNING_ACT}}"))
        .collect();
    assert_planning_route_red("writer-missing", writer_missing, |_| {});

    let mut critic_missing = repository_route("step-plan-critic");
    critic_missing.tokens = critic_missing
        .tokens
        .into_iter()
        .filter(|token| !matches!(token.as_str(), "--planning-act" | "{{PLANNING_ACT}}"))
        .collect();
    assert_planning_route_red("critic-missing", critic_missing, |_| {});

    let mut wrong_role = repository_route("step-executor");
    let evidence = wrong_role
        .tokens
        .windows(2)
        .position(|pair| pair == ["--evidence", "{{EVIDENCE}}"])
        .expect("evidence pair");
    wrong_role.tokens.splice(
        evidence + 2..evidence + 2,
        ["--planning-act".to_owned(), "{{PLANNING_ACT}}".to_owned()],
    );
    assert_planning_route_red("wrong-role", wrong_role, |_| {});

    let mut duplicate = repository_route("step-plan-writer");
    let delimiter = duplicate
        .tokens
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    duplicate.tokens.splice(
        delimiter..delimiter,
        ["--planning-act".to_owned(), "{{PLANNING_ACT}}".to_owned()],
    );
    assert_planning_route_red("duplicate", duplicate, |_| {});

    let mut moved = repository_route("step-plan-critic");
    let planning = moved
        .tokens
        .iter()
        .position(|token| token == "--planning-act")
        .expect(PAIR);
    moved.tokens.drain(planning..planning + 2);
    let delimiter = moved
        .tokens
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    moved.tokens.splice(
        delimiter + 1..delimiter + 1,
        ["--planning-act".to_owned(), "{{PLANNING_ACT}}".to_owned()],
    );
    assert_planning_route_red("moved-after-delimiter", moved, |_| {});

    assert_planning_route_red(
        "unsupported",
        repository_route("step-plan-writer"),
        |bindings| {
            bindings.planning_act = OsString::from("destructive");
        },
    );
    assert_planning_route_red("empty", repository_route("step-plan-critic"), |bindings| {
        bindings.planning_act = OsString::new();
    });

    let mut unknown = repository_route("step-plan-writer");
    let value = unknown
        .tokens
        .iter()
        .position(|token| token == "{{PLANNING_ACT}}")
        .expect("planning value");
    unknown.tokens[value] = "{{UNKNOWN}}".to_owned();
    assert_planning_route_red("canonical-unknown", unknown, |_| {});
}

#[test]
fn phase3_planning_act_lifecycle_is_explicit() {
    let markdown = real_skill_markdown();
    let start = markdown
        .find("## Phase 3 — Per step PCE-PR-C")
        .expect("Phase 3");
    let remainder = &markdown[start..];
    let end = remainder[3..]
        .find("\n## ")
        .map_or(markdown.len(), |offset| start + 3 + offset + 1);
    let phase3 = &markdown[start..end];
    assert_eq!(
        phase3.matches(PHASE3_PLANNING_PARAGRAPH).count(),
        1,
        "phase3 planning lifecycle paragraph"
    );
    assert_eq!(
        markdown.matches(PLANNING_OPTION_CONTRACT).count(),
        1,
        "planning option contract"
    );
}

#[test]
fn phase3_executes_falsification_before_pr_creation() {
    let markdown = real_skill_markdown();
    let phase3_start = markdown
        .find("## Phase 3 — Per step PCE-PR-C")
        .expect("Phase 3");
    let remainder = &markdown[phase3_start..];
    let phase3_end = remainder[3..]
        .find("\n## ")
        .map_or(markdown.len(), |offset| phase3_start + 3 + offset + 1);
    let phase3 = &markdown[phase3_start..phase3_end];
    let headings = [
        "1. **Plan (Codex)**",
        "2. **Isolate and contract check**",
        "3. **Execute (Codex)**",
        "4. **Falsify (Claude)**",
        "5. **PR (you)**",
        "6. **Review**",
        "7. **Merge (you)**",
    ];
    let mut previous = 0;
    for heading in headings {
        assert_eq!(phase3.matches(heading).count(), 1, "heading {heading}");
        let position = phase3.find(heading).expect("phase heading");
        assert!(position >= previous, "heading order for {heading}");
        previous = position;
    }

    let falsify_start = phase3.find(headings[3]).expect("falsify stage");
    let pr_start = phase3.find(headings[4]).expect("PR stage");
    let falsify = &phase3[falsify_start..pr_start];
    let clean = "git -C <worktree-abs> status --porcelain=v1 --untracked-files=no";
    assert_eq!(falsify.matches("`falsification-critic`").count(), 2);
    for expected in [
        "Only `APPROVE` together with `blocking_issues: []` advances to stage 5.",
        "Any non-empty `blocking_issues` is blocking regardless of the outer `verdict` token.",
        "Any non-empty validated `blocking_issues` list dispatches `step-executor` at the exact current committed step head after the tracked-clean observation.",
        "`REVISE` or `BLOCK` with `blocking_issues: []` enters the existing routing for that token and does not start a repair.",
        "Missing, unreadable, malformed, schema-invalid, unknown, or reference-invalid verdict data stops loudly.",
    ] {
        assert_eq!(
            falsify.matches(expected).count(),
            1,
            "classification {expected}"
        );
    }
    assert_eq!(phase3.matches(clean).count(), 1);
    for expected in [
        "Require exit status `0`",
        "stdout to be exactly empty (zero bytes)",
        "stderr to be exactly empty (zero bytes)",
        "before either stage 5 or any `REVISE` re-dispatch",
    ] {
        assert_eq!(
            falsify.matches(expected).count(),
            1,
            "clean result {expected}"
        );
    }
    let clean_position = phase3.find(clean).expect("tracked-clean command");
    assert!(clean_position < pr_start);
    let repair = "Repair the committed step artifact at exact head <current-step-head>";
    let repair_position = phase3.find(repair).expect("repair task");
    assert!(clean_position < repair_position);
    assert!(repair_position < pr_start);
    assert!(
        falsify_start
            < phase3
                .find("push the branch")
                .expect("first push instruction")
    );
    for binary_owned in [
        "A blocking issue is admissible only for a demonstrated break.",
        "do not block on style, naming, design preference, scope",
        "mutate the subject it claims to test and rerun the check",
        "delete the configuration entry that activates it",
    ] {
        assert!(
            !falsify.contains(binary_owned),
            "binary-owned mandate leaked into skill"
        );
    }
}

#[test]
fn phase3_falsification_repair_is_bounded_and_mechanical() {
    let markdown = real_skill_markdown();
    let phase3_start = markdown
        .find("## Phase 3 — Per step PCE-PR-C")
        .expect("Phase 3");
    let phase3_remainder = &markdown[phase3_start..];
    let phase3_end = phase3_remainder[3..]
        .find("\n## ")
        .map_or(markdown.len(), |offset| phase3_start + 3 + offset + 1);
    let phase3 = &markdown[phase3_start..phase3_end];
    let falsify_start = phase3
        .find("4. **Falsify (Claude)**")
        .expect("falsify stage");
    let pr_start = phase3.find("5. **PR (you)**").expect("PR stage");
    let falsify = &phase3[falsify_start..pr_start];
    let task = "Repair the committed step artifact at exact head <current-step-head> from the accepted pre-PR falsification verdict at <absolute-review-json-path>. Apply every supplied required_change and replacement_execution mechanically. Do not re-derive, reinterpret, broaden, or re-litigate any supplied finding. Read tracked inputs only with the supplied git show <current-step-head>:<path> commands. Run every acceptance gate from the approved plan. On success, amend the existing single conventional step commit, refresh pr-body.md and leave it untracked, create no tag, and do not push. Emit the normal structured executor result at <absolute-step-audit-dir>/executor-result-<n>.json outside the measured worktree. If the repair is infeasible as supplied, do not invent another repair; return the existing structured BLOCK result with the applicable root_cause.";
    assert_eq!(falsify.matches(task).count(), 1, "complete repair task");
    for expected in [
        "<absolute-step-audit-dir>/executor-result-<n>.json",
        "refresh pr-body.md and leave it untracked",
        "amend the existing single conventional step commit",
        "create no tag",
        "do not push",
    ] {
        assert_eq!(
            task.matches(expected).count(),
            1,
            "repair task policy {expected}"
        );
    }

    let policies = [
        "the validated-production spending limit for `(node, falsification-critic)` is `12` validated-production spends; a productless attempt charges no validated-production spend and remains governed by the `(node, role, required artifact)` consecutive-non-production hold.",
        "If two consecutive schema-valid verdict artifacts have substantially identical blocking issue sets under the existing comparison rule, append/open the existing escalation and stop before another executor dispatch, push, or PR.",
        "If validated-production spend `12` is blocking, escalate and stop before another executor dispatch, push, or PR.",
        "After a successful repair result, resolve the new exact committed step head and reissue `falsification-critic` through the same anchored binary route.",
        "Ground it on that new head, the same approved plan ref, and `git diff <milestone-integration-ref>...<new-step-head>`.",
        "Supply the prior exact verdict path as the finding to verify and ask the critic to verify the executed replacement on the repaired artifact rather than re-derive the finding.",
        "Successive gate verdicts remain `review-<n>.json`/`review-<n>.md`, indexed only by the exact `(node, falsification-critic)` issuance ordinal; add no counter or artifact family.",
    ];
    for expected in policies {
        assert_eq!(
            falsify.matches(expected).count(),
            1,
            "repair policy {expected}"
        );
        assert!(
            phase3.find(expected).expect("repair policy offset") < pr_start,
            "repair policy precedes PR: {expected}"
        );
    }
    assert_eq!(falsify.matches("stated.version_policy: NONE").count(), 1);
    assert!(falsify.contains("means no version edit and no tag"));

    let routing_start = markdown
        .find("## Routing, caps, and adaptation")
        .expect("routing section");
    let routing_remainder = &markdown[routing_start..];
    let routing_end = routing_remainder[3..]
        .find("\n## ")
        .map_or(markdown.len(), |offset| routing_start + 3 + offset + 1);
    let routing = &markdown[routing_start..routing_end];
    let general =
        "- `APPROVE` proceeds, `REVISE` loops with blocking issues, and `BLOCK` escalates.";
    let exception = "Phase 3 pre-PR falsification is the exception: a schema- and reference-valid verdict with non-empty `blocking_issues` follows stage 4's bounded repair path before generic `BLOCK` escalation while a repair remains available.";
    assert_eq!(routing.matches(general).count(), 1);
    assert_eq!(routing.matches(exception).count(), 1);
    assert!(
        routing.find(general).expect("general routing")
            < routing.find(exception).expect("exception")
    );
    for inherited in [
        "Plan/critic and PR/fix automatic spending limits are 12 validated-production spends per exact `(node, role)`.",
        "Derive validated-production spends only from validated production; productless attempts are exempt and are stopped separately by the `(node, role, required artifact)` consecutive-non-production hold.",
        "On two consecutive verdicts with substantially identical blocking issue sets, short-circuit the loop before the spending limit.",
        "Derive the comparison from review artifacts; do not update separate loop state.",
    ] {
        assert_eq!(routing.matches(inherited).count(), 1, "routing {inherited}");
    }
}

#[test]
fn role_registry_exact_set() {
    let measured: std::collections::BTreeSet<_> = repository_routes()
        .iter()
        .filter_map(route_role)
        .map(ToOwned::to_owned)
        .collect();
    let expected: std::collections::BTreeSet<_> =
        ROLE_REGISTRY.into_iter().map(ToOwned::to_owned).collect();
    assert_eq!(measured, expected, "role_registry_exact_set");
}

#[test]
fn one_anchor_per_role() {
    let mut counts = BTreeMap::new();
    for role in repository_routes().iter().filter_map(route_role) {
        *counts.entry(role.to_owned()).or_insert(0usize) += 1;
    }
    let missing: Vec<_> = ROLE_REGISTRY
        .iter()
        .filter(|role| !counts.contains_key(**role))
        .copied()
        .collect();
    let duplicated: Vec<_> = counts
        .iter()
        .filter(|(_, count)| **count != 1)
        .map(|(role, count)| (role.as_str(), *count))
        .collect();
    assert!(
        missing.is_empty() && duplicated.is_empty() && counts.len() == ROLE_REGISTRY.len(),
        "one_anchor_per_role: missing={missing:?}, duplicated={duplicated:?}, measured={counts:?}"
    );
}

#[test]
fn environment_cardinality_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        assert_eq!(
            route
                .tokens
                .iter()
                .filter(|token| *token == "--env")
                .count(),
            REQUIRED_ENVIRONMENT_ENTRIES,
            "environment_cardinality_by_role/{role}"
        );
        for placeholder in ENVIRONMENT_PLACEHOLDERS {
            assert_eq!(
                route
                    .tokens
                    .iter()
                    .filter(|token| token.as_str() == placeholder)
                    .count(),
                1,
                "environment_cardinality_by_role/{role}/{placeholder}"
            );
        }
    }
}

#[test]
fn logging_group_by_anchor() {
    let routes = repository_routes();
    assert_eq!(routes.len(), EXPECTED_ANCHORED_ROUTE_COUNT);
    for route in routes {
        let identity = anchor_identity(&route).unwrap_or_else(|| {
            format!(
                "purpose/{}",
                route.kind.purpose_kind().unwrap_or("unclassified")
            )
        });
        let current = usize::from(has_complete_logging_group(&route));
        let removed = missing_logging_member(&route);
        let measured = measured_logging_group(&route);
        assert_eq!(
            current, 1,
            "logging_group_by_anchor/{identity}: removed_member={removed:?}, \
measured={measured:?}, green=1, current={current}"
        );
    }
}

#[test]
fn required_artifact_logging_mutations_red_before_child_invocation() {
    let base = repository_route("step-executor");
    let pair = base
        .tokens
        .iter()
        .position(|token| token == "--required-artifact")
        .expect("required artifact flag");

    let mut missing_flag = base.clone();
    missing_flag.tokens.remove(pair);
    assert_document_error_before_pce(
        "required-artifact-missing-flag",
        &route_document(&missing_flag),
        ReviewError::ParentGrammar,
    );

    let mut missing_value = base.clone();
    missing_value.tokens.remove(pair + 1);
    assert_document_error_before_pce(
        "required-artifact-missing-value",
        &route_document(&missing_value),
        ReviewError::ParentGrammar,
    );

    let mut duplicate = base.clone();
    let delimiter = duplicate
        .tokens
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    duplicate.tokens.splice(
        delimiter..delimiter,
        ["--required-artifact".to_owned(), "{{OUTPUT}}".to_owned()],
    );
    assert_document_error_before_pce(
        "required-artifact-duplicate",
        &route_document(&duplicate),
        ReviewError::ParentGrammar,
    );

    let mut relative = base.clone();
    relative.tokens[pair + 1] = "relative/result.json".to_owned();
    assert_document_error_before_pce(
        "required-artifact-relative",
        &route_document(&relative),
        ReviewError::ParentGrammar,
    );

    let mut after_delimiter = base.clone();
    let moved = after_delimiter
        .tokens
        .drain(pair..pair + 2)
        .collect::<Vec<_>>();
    let delimiter = after_delimiter
        .tokens
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    after_delimiter
        .tokens
        .splice(delimiter + 1..delimiter + 1, moved);
    assert_document_error_before_pce(
        "required-artifact-after-delimiter",
        &route_document(&after_delimiter),
        ReviewError::ParentGrammar,
    );

    let mut mismatched = base;
    mismatched.tokens[pair + 1] = "/tmp/other.json".to_owned();
    assert_document_error_before_pce(
        "required-artifact-structured-mismatch",
        &route_document(&mismatched),
        ReviewError::ParentGrammar,
    );
}

#[test]
fn route_kind_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        assert_eq!(route.kind, expected_kind(role), "route_kind_by_role/{role}");
    }
}

#[test]
fn node_class_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        let node = route
            .tokens
            .windows(2)
            .find(|pair| pair[0] == "--node")
            .map(|pair| pair[1].as_str());
        assert_eq!(node, Some(expected_node(role)), "node_class_by_role/{role}");
    }
}

#[test]
fn ref_source_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        let source = route
            .tokens
            .windows(2)
            .filter(|pair| pair[0] == "--ref")
            .map(|pair| pair[1].as_str())
            .collect::<Vec<_>>();
        assert_eq!(source, ["{{REF}}"], "ref_source_by_role/{role}");
    }
}

#[test]
fn evidence_source_by_role() {
    let fixture = fixture();
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        let source = route
            .tokens
            .windows(2)
            .filter(|pair| pair[0] == "--evidence")
            .map(|pair| pair[1].as_str())
            .collect::<Vec<_>>();
        assert_eq!(source, ["{{EVIDENCE}}"], "evidence_source_by_role/{role}");
        let argv = substitute_route(&route, &fixture.bindings)
            .unwrap_or_else(|error| panic!("evidence_source_by_role/{role}: {error:?}"));
        let value = argv
            .windows(2)
            .find(|pair| pair[0] == "--evidence")
            .map(|pair| pair[1].as_os_str())
            .expect("evidence argv");
        assert!(
            !value.is_empty(),
            "evidence_source_by_role/{role}: empty binding"
        );
    }
}

#[test]
fn remaining_role_metadata_classes() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        let structured = route.tokens.iter().any(|token| token == "--output-schema");
        assert_eq!(
            structured,
            role != "step-plan-writer",
            "schema_and_output_class_by_role/{role}"
        );
        assert_eq!(
            route.tokens.iter().any(|token| token == "--plan-file"),
            role == "step-executor",
            "stdin_class_by_role/{role}"
        );
        assert!(
            route
                .tokens
                .windows(2)
                .any(|pair| pair == ["--ref", "{{REF}}"]),
            "ref_source_by_role/{role}"
        );
        assert!(
            route
                .tokens
                .windows(2)
                .any(|pair| pair == ["--evidence", "{{EVIDENCE}}"]),
            "evidence_source_by_role/{role}"
        );
        assert_eq!(
            route
                .tokens
                .iter()
                .filter(|token| token.as_str() == "--planning-act")
                .count(),
            usize::from(matches!(role, "step-plan-writer" | "step-plan-critic")),
            "planning_act_by_role/{role}"
        );
        if role == "step-executor" {
            assert!(
                canonical_semantics(&route).is_ok(),
                "executor_complete_git_parent_required"
            );
        }
        if route.kind == RouteKind::GateStructured {
            let delimiter = route
                .tokens
                .iter()
                .position(|token| token == "--")
                .expect("delimiter");
            let expected: &[&str] = if role == "falsification-critic" {
                &["{{CALLER_ARG}}"]
            } else {
                &["--append-system-prompt", "{{OUTPUT}}", "{{CALLER_ARG}}"]
            };
            assert!(
                route.tokens[delimiter + 1..]
                    .iter()
                    .map(String::as_str)
                    .eq(expected.iter().copied()),
                "caller_tail_class_by_role/{role}"
            );
        }
    }
}

#[test]
fn schema_and_output_class_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        let pair = route
            .tokens
            .iter()
            .filter(|token| *token == "--output-schema")
            .count()
            == 1
            && route
                .tokens
                .iter()
                .filter(|token| *token == "{{SCHEMA}}")
                .count()
                == 1
            && route.tokens.iter().filter(|token| *token == "-o").count() == 1
            && route
                .tokens
                .iter()
                .filter(|token| *token == "{{OUTPUT}}")
                .count()
                >= 1;
        assert_eq!(
            pair,
            role != "step-plan-writer",
            "schema_and_output_class_by_role/{role}"
        );
    }
}

#[test]
fn stdin_class_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        assert_eq!(
            route
                .tokens
                .iter()
                .filter(|token| *token == "--plan-file")
                .count(),
            usize::from(role == "step-executor"),
            "stdin_class_by_role/{role}"
        );
    }
}

#[test]
fn caller_tail_class_by_role() {
    for route in canonical_role_routes() {
        let role = route_role(&route).expect("canonical role");
        let delimiter = route
            .tokens
            .iter()
            .position(|token| token == "--")
            .expect("delimiter");
        let expected: &[&str] = match route.kind {
            RouteKind::GateStructured if role == "falsification-critic" => &["{{CALLER_ARG}}"],
            RouteKind::GateStructured => {
                &["--append-system-prompt", "{{OUTPUT}}", "{{CALLER_ARG}}"]
            }
            RouteKind::CodexStructured | RouteKind::CodexUnstructured
                if role == "step-executor" =>
            {
                &["--add-dir", "{{ABS_PATH}}", "{{CALLER_ARG}}"]
            }
            RouteKind::CodexStructured | RouteKind::CodexUnstructured => &["{{CALLER_ARG}}"],
            RouteKind::CodexCommitCompletion | RouteKind::CodexDiagnostics => &["{{CALLER_ARG}}"],
        };
        assert!(
            route.tokens[delimiter + 1..]
                .iter()
                .map(String::as_str)
                .eq(expected.iter().copied()),
            "caller_tail_class_by_role/{role}"
        );
    }
}

#[test]
fn executor_complete_git_parent_required() {
    let route = repository_route("step-executor");
    let delimiter = route
        .tokens
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    assert_eq!(
        &route.tokens[delimiter + 1..],
        ["--add-dir", "{{ABS_PATH}}", "{{CALLER_ARG}}"],
        "executor_complete_git_parent_required"
    );
    let f = fixture();
    let argv = substitute_route(&route, &f.bindings).expect("executor binding");
    let add_dir = argv
        .windows(2)
        .find(|pair| pair[0] == "--add-dir")
        .expect("add-dir");
    assert_eq!(
        add_dir[1], f.bindings.complete_git_dir,
        "executor complete parent git binding"
    );
}

fn assert_outside_red(name: &str, unit: &str) {
    let measured = outside_fragment_units(unit);
    assert!(
        measured
            .iter()
            .any(|item| unit_has_prohibited_fragment(item)),
        "{name}: both conjuncts"
    );
    assert_eq!(
        review_document(unit, &fixture().bindings).expect_err(name),
        ReviewError::OutsideAnchorDispatchFragment,
        "{name}"
    );
}

#[test]
fn outside_fragment_code_span_target_then_option_reds() {
    assert_outside_red(
        "outside_fragment_code_span_target_then_option_reds",
        "`pce dispatch codex --cwd`",
    );
}

#[test]
fn outside_fragment_code_span_option_then_target_reds() {
    assert_outside_red(
        "outside_fragment_code_span_option_then_target_reds",
        "`--cwd pce dispatch codex`",
    );
}

#[test]
fn outside_fragment_fence_target_then_delimiter_reds() {
    assert_outside_red(
        "outside_fragment_fence_target_then_delimiter_reds",
        "```text\npce dispatch gate --\n```",
    );
}

#[test]
fn outside_fragment_fence_delimiter_then_target_reds() {
    assert_outside_red(
        "outside_fragment_fence_delimiter_then_target_reds",
        "```text\n-- pce dispatch gate\n```",
    );
}

#[test]
fn outside_fragment_shell_line_target_then_caller_reds() {
    assert_outside_red(
        "outside_fragment_shell_line_target_then_caller_reds",
        "pce dispatch codex -- caller",
    );
}

#[test]
fn outside_fragment_shell_line_caller_then_target_reds() {
    assert_outside_red(
        "outside_fragment_shell_line_caller_then_target_reds",
        "-- caller pce dispatch codex",
    );
}

#[test]
fn outside_fragment_bare_route_accepts() {
    review_document("`pce dispatch codex`", &fixture().bindings).expect("bare route");
}

#[test]
fn outside_fragment_option_only_accepts() {
    review_document("`--cwd`", &fixture().bindings).expect("option only");
}

#[test]
fn outside_fragment_split_units_accepts() {
    review_document("`pce dispatch codex` and `--cwd`", &fixture().bindings).expect("split units");
}

#[test]
fn outside_fragment_valid_anchor_accepts() {
    review_document(
        &document(
            "codex-unstructured",
            &base_command(RouteKind::CodexUnstructured),
        ),
        &fixture().bindings,
    )
    .expect("valid anchor exemption");
}

#[test]
fn outside_fragment_protocol_definition_accepts() {
    let protocol = "Outside a valid anchored fence, a prohibited dispatch fragment is one inline code span, one complete fenced code block, or one shell-like physical line that contains both (a) the contiguous route name `pce dispatch codex` or `pce dispatch gate` and (b) at least one whitespace-delimited token that is a dispatch-only parent option, the standalone `--` delimiter, or any caller argument represented by a whitespace-delimited token after that delimiter.";
    review_document(protocol, &fixture().bindings).expect("protocol definition");
}

#[test]
fn outside_fragment_non_shell_line_with_both_conjuncts_accepts() {
    let control =
        "> Prose control: pce dispatch codex --cwd names tokens without presenting a command.";
    assert!(unit_has_prohibited_fragment(control));
    assert!(!shell_like(control));
    review_document(control, &fixture().bindings).expect("first-token boundary");
}

fn standalone_parts() -> (String, &'static str) {
    (
        document(
            "codex-unstructured",
            &base_command(RouteKind::CodexUnstructured),
        ),
        "pce log --file /tmp/events --kind dispatch --node m1-s1\n",
    )
}

#[test]
fn standalone_append_marker_absent_allows_legacy() {
    let (anchor, append) = standalone_parts();
    let markdown = format!("## One\n{anchor}{append}");
    assert_eq!(markdown.matches(CONSOLIDATION_MARKER).count(), 0);
    review_document(&markdown, &fixture().bindings).expect("marker absent");
}

#[test]
fn standalone_append_marker_present_non_colocated_accepts() {
    let (anchor, append) = standalone_parts();
    let markdown = format!("{CONSOLIDATION_MARKER}\n## One\n{anchor}## Two\n{append}");
    assert_eq!(markdown.matches(CONSOLIDATION_MARKER).count(), 1);
    assert_eq!(extract_anchored_routes(&markdown).expect("anchor").len(), 1);
    review_document(&markdown, &fixture().bindings).expect("non-colocated");
}

fn assert_colocated_append(name: &str, append_first: bool) {
    let (anchor, append) = standalone_parts();
    let body = if append_first {
        format!("{append}{anchor}")
    } else {
        format!("{anchor}{append}")
    };
    let markdown = format!("{CONSOLIDATION_MARKER}\n## One\n{body}");
    assert_eq!(
        markdown.matches(CONSOLIDATION_MARKER).count(),
        1,
        "{name}: marker"
    );
    let routes = extract_anchored_routes(&markdown).expect("anchor extraction");
    assert_eq!(routes.len(), 1, "{name}: anchor identity");
    assert!(
        markdown
            .lines()
            .any(|line| line.contains("pce log") && line.contains("--kind dispatch")),
        "{name}: append identity"
    );
    assert_eq!(
        review_document(&markdown, &fixture().bindings).expect_err(name),
        ReviewError::CoLocatedStandaloneDispatchAppend,
        "{name}"
    );
}

#[test]
fn standalone_append_after_anchor_reds() {
    assert_colocated_append("standalone_append_after_anchor_reds", false);
}

#[test]
fn standalone_append_before_anchor_reds() {
    assert_colocated_append("standalone_append_before_anchor_reds", true);
}

fn independently_tokenized_append_lines(markdown: &str) -> Vec<usize> {
    markdown
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let tokens: Vec<_> = line.split_ascii_whitespace().collect();
            let command = tokens.windows(2).position(|pair| pair == ["pce", "log"])?;
            let kind = tokens
                .windows(2)
                .position(|pair| pair == ["--kind", "dispatch"])?;
            (shell_like(line) && command < kind).then_some(index + 1)
        })
        .collect()
}

#[test]
fn standalone_append_document_wide_without_anchor_reds() {
    let markdown = "pce log --file /tmp/events --kind  dispatch --node m1-s1\n";
    let independently_tokenized = independently_tokenized_append_lines(markdown);
    let detector: Vec<_> = markdown
        .lines()
        .enumerate()
        .filter_map(|(index, line)| is_standalone_dispatch_append(line).then_some(index + 1))
        .collect();
    let walker = standalone_dispatch_append_lines(markdown);
    assert_eq!(
        detector, independently_tokenized,
        "standalone_append_document_wide_without_anchor_reds/detector-agreement: independent={independently_tokenized:?}, detector={detector:?}"
    );
    assert_eq!(
        walker, independently_tokenized,
        "standalone_append_document_wide_without_anchor_reds/walker-agreement: independent={independently_tokenized:?}, walker={walker:?}"
    );
    assert!(!independently_tokenized.is_empty());
}

fn assert_document_wide_across_heading(name: &str, append_first: bool) {
    let (anchor, append) = standalone_parts();
    let markdown = if append_first {
        format!("{CONSOLIDATION_MARKER}\n## Append\n{append}## Anchor\n{anchor}")
    } else {
        format!("{CONSOLIDATION_MARKER}\n## Anchor\n{anchor}## Append\n{append}")
    };
    reject_colocated_standalone_append(&markdown)
        .unwrap_or_else(|error| panic!("{name}/co-location-acceptance: {error:?}"));
    let expected = independently_tokenized_append_lines(&markdown);
    let measured = standalone_dispatch_append_lines(&markdown);
    assert_eq!(
        measured, expected,
        "{name}: expected={expected:?}, measured={measured:?}"
    );
    assert_eq!(measured.len(), 1, "{name}: one separated append");
}

#[test]
fn standalone_append_document_wide_after_separate_heading_reds() {
    assert_document_wide_across_heading(
        "standalone_append_document_wide_after_separate_heading_reds",
        false,
    );
}

#[test]
fn standalone_append_document_wide_before_separate_heading_reds() {
    assert_document_wide_across_heading(
        "standalone_append_document_wide_before_separate_heading_reds",
        true,
    );
}

fn independently_filtered_prior_count(events: &[serde_json::Value]) -> usize {
    events
        .iter()
        .filter(|event| event["kind"] == "dispatch")
        .filter(|event| event["node"] == "m7-s2")
        .filter(|event| event["role"] == "pr-reviewer")
        .count()
}

#[test]
fn verdict_index_derived_from_prior_records() {
    let f = fixture();
    let events = &f.bindings.context.accepted_events;
    let first = prior_dispatch_count(events, "m7-s2", "pr-reviewer");
    let independently_filtered = independently_filtered_prior_count(events);
    assert_eq!(
        first, independently_filtered,
        "verdict_index_derived_from_prior_records"
    );
    let next = first.checked_add(1).expect("checked successor");
    let directory = &f.bindings.context.review_directories["pr-reviewer"];
    let json = directory.join(format!("review-{next}.json"));
    let markdown = directory.join(format!("review-{next}.md"));
    let json_index = json
        .file_stem()
        .and_then(OsStr::to_str)
        .and_then(|name| name.strip_prefix("review-"))
        .expect("json index");
    let markdown_index = markdown
        .file_stem()
        .and_then(OsStr::to_str)
        .and_then(|name| name.strip_prefix("review-"))
        .expect("markdown index");
    assert_eq!(
        json_index, markdown_index,
        "verdict_index_derived_from_prior_records"
    );
}

fn assert_other_record_not_counted(name: &str, extra: serde_json::Value) {
    let f = fixture();
    let events = &f.bindings.context.accepted_events;
    let independently_filtered = independently_filtered_prior_count(events);
    let mut changed = events.clone();
    changed.push(extra);
    assert_eq!(
        prior_dispatch_count(&changed, "m7-s2", "pr-reviewer"),
        independently_filtered,
        "{name}"
    );
}

#[test]
fn verdict_other_kind_not_counted() {
    assert_other_record_not_counted(
        "verdict_other_kind_not_counted",
        serde_json::json!({"kind":"dispatch-completion","node":"m7-s2","role":"pr-reviewer"}),
    );
}

#[test]
fn verdict_other_node_not_counted() {
    assert_other_record_not_counted(
        "verdict_other_node_not_counted",
        serde_json::json!({"kind":"dispatch","node":"m8-s2","role":"pr-reviewer"}),
    );
}

#[test]
fn verdict_other_role_not_counted() {
    assert_other_record_not_counted(
        "verdict_other_role_not_counted",
        serde_json::json!({"kind":"dispatch","node":"m7-s2","role":"step-plan-critic"}),
    );
}

#[test]
fn verdict_matching_record_increments_index() {
    let f = fixture();
    let events = &f.bindings.context.accepted_events;
    let independently_filtered = independently_filtered_prior_count(events);
    let mut incremented = events.clone();
    incremented.push(serde_json::json!({"kind":"dispatch","node":"m7-s2","role":"pr-reviewer"}));
    assert_eq!(
        prior_dispatch_count(&incremented, "m7-s2", "pr-reviewer"),
        independently_filtered.checked_add(1).expect("successor"),
        "verdict_matching_record_increments_index"
    );
}

#[test]
fn prior_verdict_sentinel_survives_next_round() {
    let f = fixture();
    let events = &f.bindings.context.accepted_events;
    let first = prior_dispatch_count(events, "m7-s2", "pr-reviewer");
    let next = first.checked_add(1).expect("checked successor");
    let directory = &f.bindings.context.review_directories["pr-reviewer"];
    let json = directory.join(format!("review-{next}.json"));
    let prior = directory.join(format!("review-{first}.json"));
    fs::write(&prior, b"prior sentinel").expect("prior sentinel");
    let before_path = prior.clone();
    let before_bytes = fs::read(&prior).expect("prior bytes");
    let before_kind = fs::metadata(&prior).expect("prior metadata").file_type();
    let before_exists = prior.exists();
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let route = extract_anchored_routes(&skill)
        .expect("routes")
        .into_iter()
        .find(|route| route_role(route) == Some("pr-reviewer"))
        .expect("reviewer");
    let single = format!(
        "{}\n```sh\n{}\n```\n",
        marker("gate-structured"),
        route.tokens.join(" ")
    );
    review_document(&single, &f.bindings).expect("next gate");
    assert_eq!(
        prior, before_path,
        "prior_verdict_sentinel_survives_next_round/path"
    );
    assert_eq!(
        fs::read(&prior).expect("after prior"),
        before_bytes,
        "prior_verdict_sentinel_survives_next_round/bytes"
    );
    assert_eq!(
        fs::metadata(&prior).expect("after metadata").file_type(),
        before_kind,
        "prior_verdict_sentinel_survives_next_round/kind"
    );
    assert_eq!(
        prior.exists(),
        before_exists,
        "prior_verdict_sentinel_survives_next_round/existence"
    );
    assert!(json.exists(), "new verdict exists");
}

#[test]
fn verdict_routing_and_markdown_preservation() {
    let f = fixture();
    let schema = &f.bindings.verdict_schema;
    let verdict = f.bindings.verdict_root.join("routing.json");
    let markdown = f.bindings.verdict_root.join("routing.md");
    fs::write(&markdown, b"explanatory sentinel").expect("markdown");
    let markdown_before = fs::read(&markdown).expect("markdown before");
    for (name, value, expected) in [
        (
            "verdict_approve_routes_forward",
            "APPROVE",
            VerdictRoute::Forward,
        ),
        (
            "verdict_revise_routes_to_phase_codex_anchor",
            "REVISE",
            VerdictRoute::Revise,
        ),
        (
            "verdict_block_routes_to_escalation",
            "BLOCK",
            VerdictRoute::Escalate,
        ),
    ] {
        fs::write(&verdict, format!(r#"{{"verdict":"{value}","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"fixture"}}"#)).expect("verdict");
        assert_eq!(route_verdict(&verdict, schema), Ok(expected), "{name}");
    }
    assert_eq!(
        fs::read(&markdown).expect("markdown after"),
        markdown_before,
        "review_markdown_retained_separately"
    );
    let missing = f.bindings.verdict_root.join("missing.json");
    assert_eq!(
        route_verdict(&missing, schema),
        Err(ReviewError::VerdictMissing),
        "verdict_missing_reds"
    );
    fs::write(&verdict, b"{").expect("malformed");
    assert_eq!(
        route_verdict(&verdict, schema),
        Err(ReviewError::VerdictMalformed),
        "verdict_malformed_json_reds"
    );
    fs::write(&verdict, br#"{"verdict":"MAYBE"}"#).expect("invalid");
    assert_eq!(
        route_verdict(&verdict, schema),
        Err(ReviewError::VerdictSchemaInvalid),
        "verdict_schema_invalid_reds"
    );
}

fn write_verdict(path: &Path, verdict: &str) {
    fs::write(
        path,
        format!(
            r#"{{"verdict":"{verdict}","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"fixture"}}"#
        ),
    )
    .expect("verdict fixture");
}

#[test]
fn verdict_approve_routes_forward() {
    let f = fixture();
    let path = f.bindings.verdict_root.join("approve.json");
    write_verdict(&path, "APPROVE");
    assert_eq!(
        route_verdict(&path, &f.bindings.verdict_schema),
        Ok(VerdictRoute::Forward)
    );
}

#[test]
fn verdict_revise_routes_to_phase_codex_anchor() {
    let f = fixture();
    let path = f.bindings.verdict_root.join("revise.json");
    write_verdict(&path, "REVISE");
    assert_eq!(
        route_verdict(&path, &f.bindings.verdict_schema),
        Ok(VerdictRoute::Revise)
    );
    assert_eq!(
        expected_kind("milestone-planner"),
        RouteKind::CodexStructured
    );
    assert_eq!(expected_kind("step-planner"), RouteKind::CodexStructured);
}

#[test]
fn verdict_block_routes_to_escalation() {
    let f = fixture();
    let path = f.bindings.verdict_root.join("block.json");
    write_verdict(&path, "BLOCK");
    assert_eq!(
        route_verdict(&path, &f.bindings.verdict_schema),
        Ok(VerdictRoute::Escalate)
    );
}

#[test]
fn verdict_missing_reds() {
    let f = fixture();
    assert_eq!(
        route_verdict(
            &f.bindings.verdict_root.join("absent.json"),
            &f.bindings.verdict_schema
        ),
        Err(ReviewError::VerdictMissing)
    );
}

#[test]
fn verdict_unreadable_reds() {
    let f = fixture();
    assert_eq!(
        route_verdict(&f.bindings.verdict_root, &f.bindings.verdict_schema),
        Err(ReviewError::VerdictUnreadable)
    );
}

#[test]
fn verdict_malformed_json_reds() {
    let f = fixture();
    let path = f.bindings.verdict_root.join("malformed.json");
    fs::write(&path, b"{").expect("malformed");
    assert_eq!(
        route_verdict(&path, &f.bindings.verdict_schema),
        Err(ReviewError::VerdictMalformed)
    );
}

#[test]
fn verdict_schema_invalid_reds() {
    let f = fixture();
    let path = f.bindings.verdict_root.join("invalid.json");
    write_verdict(&path, "MAYBE");
    assert_eq!(
        route_verdict(&path, &f.bindings.verdict_schema),
        Err(ReviewError::VerdictSchemaInvalid)
    );
}

#[test]
fn verdict_schema_requires_executed_break_and_replacement_evidence() {
    const BLOCKING: &str = r#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"dispatch route","problem":"the executed route cannot authenticate","input":"pce dispatch gate through the candidate route","observation":"exit 1; Not logged in; verdict artifact absent","execution_ref":"execution-000001","required_change":"supply PATH, HOME, and USER to the cleared child environment","replacement_execution":{"input":"pce dispatch gate through the replacement route with PATH, HOME, and USER","observation":"exit 0; authenticated; conforming verdict artifact written","execution_ref":"execution-000002"}}],"non_blocking_notes":[],"summary":"one demonstrated break"}"#;
    const LEGACY_BLOCKING: &str = r#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"dispatch route","problem":"the executed route cannot authenticate","input":"pce dispatch gate through the candidate route","observation":"exit 1; Not logged in; verdict artifact absent","required_change":"supply PATH, HOME, and USER to the cleared child environment","replacement_execution":{"input":"pce dispatch gate through the replacement route with PATH, HOME, and USER","observation":"exit 0; authenticated; conforming verdict artifact written"}}],"non_blocking_notes":[],"summary":"one demonstrated break"}"#;
    const APPROVE: &str = r#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"no demonstrated break"}"#;
    const OPINION: &str = r#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"dispatch route","problem":"looks wrong","required_change":"change it"}],"non_blocking_notes":[],"summary":"opinion only"}"#;

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../skills/pce/schemas/verdict.schema.json"))
            .expect("schema JSON");
    let validator = jsonschema::validator_for(&schema).expect("verdict validator");
    let blocking: serde_json::Value = serde_json::from_str(BLOCKING).expect("blocking fixture");
    let legacy_blocking: serde_json::Value =
        serde_json::from_str(LEGACY_BLOCKING).expect("legacy blocking fixture");
    let approve: serde_json::Value = serde_json::from_str(APPROVE).expect("approval fixture");
    assert_eq!(validator.iter_errors(&blocking).count(), 0);
    assert_eq!(validator.iter_errors(&legacy_blocking).count(), 2);
    assert_eq!(validator.iter_errors(&approve).count(), 0);

    let opinion: serde_json::Value = serde_json::from_str(OPINION).expect("opinion fixture");
    let opinion_errors: Vec<_> = validator
        .iter_errors(&opinion)
        .map(|error| (error.instance_path().to_string(), error.to_string()))
        .collect();
    for property in ["input", "observation", "replacement_execution"] {
        assert!(
            opinion_errors.iter().any(|(path, message)| {
                path == "/blocking_issues/0" && message.contains(property)
            })
        );
    }

    for (pointer, fragment, mutate) in [
        (
            "/blocking_issues/0/input",
            "shorter than 1 character",
            "input",
        ),
        (
            "/blocking_issues/0/observation",
            "shorter than 1 character",
            "observation",
        ),
        (
            "/blocking_issues/0/required_change",
            "shorter than 1 character",
            "required_change",
        ),
    ] {
        let mut instance: serde_json::Value = serde_json::from_str(BLOCKING).expect("fixture");
        instance["blocking_issues"][0][mutate] = serde_json::json!("");
        assert!(validator.iter_errors(&instance).any(|error| {
            error.instance_path().to_string() == pointer && error.to_string().contains(fragment)
        }));
    }
    for field in ["input", "observation"] {
        let mut instance: serde_json::Value = serde_json::from_str(BLOCKING).expect("fixture");
        instance["blocking_issues"][0]["replacement_execution"][field] = serde_json::json!("");
        let pointer = format!("/blocking_issues/0/replacement_execution/{field}");
        assert!(validator.iter_errors(&instance).any(|error| {
            error.instance_path().to_string() == pointer
                && error.to_string().contains("shorter than 1 character")
        }));
    }
    for pointer in [
        "/blocking_issues/0/execution_ref",
        "/blocking_issues/0/replacement_execution/execution_ref",
    ] {
        for malformed in ["", "execution-1", "execution-000001x"] {
            let mut instance: serde_json::Value = serde_json::from_str(BLOCKING).expect("fixture");
            *instance.pointer_mut(pointer).expect("reference pointer") =
                serde_json::json!(malformed);
            assert!(
                validator
                    .iter_errors(&instance)
                    .any(|error| { error.instance_path().to_string() == pointer })
            );
        }
    }
    let mut extra: serde_json::Value = serde_json::from_str(BLOCKING).expect("fixture");
    extra["blocking_issues"][0]["replacement_execution"]["command"] =
        serde_json::json!("pce dispatch gate");
    assert!(validator.iter_errors(&extra).any(|error| {
        error.instance_path().to_string() == "/blocking_issues/0/replacement_execution"
            && error.to_string().contains("command")
    }));
}

#[test]
fn verdict_unknown_value_reds() {
    let f = fixture();
    let path = f.bindings.verdict_root.join("unknown.json");
    let permissive = f.bindings.verdict_root.join("permissive.schema.json");
    write_verdict(&path, "MAYBE");
    fs::write(&permissive, br#"{"type":"object","required":["verdict"]}"#).expect("schema");
    assert_eq!(
        route_verdict(&path, &permissive),
        Err(ReviewError::VerdictUnknown)
    );
}

#[test]
fn wrong_verdict_index_reds_before_spawn() {
    let (f, route, mut argv) = gate_argv();
    let role = route_role(&route).expect("role");
    let node = argv
        .windows(2)
        .find(|pair| pair[0] == "--node")
        .expect("node")[1]
        .clone();
    let prior = prior_dispatch_count(
        &f.bindings.context.accepted_events,
        node.to_str().expect("node text"),
        role,
    );
    let wrong = f.bindings.context.review_directories[role].join(format!("review-{prior}.json"));
    for flag in ["-o", "--append-system-prompt"] {
        let index = argv
            .iter()
            .position(|arg| arg == flag)
            .expect("output flag");
        argv[index + 1] = wrong.as_os_str().to_owned();
    }
    assert_eq!(
        argv.windows(2)
            .find(|pair| pair[0] == "-o")
            .expect("parent")[1],
        argv.windows(2)
            .find(|pair| pair[0] == "--append-system-prompt")
            .expect("tail")[1],
        "path equality remains true"
    );
    assert_eq!(
        semantic_output_path(&route, &argv, &f.bindings),
        Err(ReviewError::WrongVerdictIndex)
    );
    assert_eq!(f.bindings.pce_invocations.get(), 0);
}

#[test]
fn wrong_verdict_directory_reds_before_spawn() {
    let (f, route, mut argv) = gate_argv();
    let correct = argv
        .windows(2)
        .find(|pair| pair[0] == "-o")
        .expect("parent")[1]
        .clone();
    let wrong = f
        .bindings
        .verdict_root
        .join("wrong-directory")
        .join(Path::new(&correct).file_name().expect("filename"));
    for flag in ["-o", "--append-system-prompt"] {
        let index = argv
            .iter()
            .position(|arg| arg == flag)
            .expect("output flag");
        argv[index + 1] = wrong.as_os_str().to_owned();
    }
    assert_eq!(
        semantic_output_path(&route, &argv, &f.bindings),
        Err(ReviewError::WrongVerdictDirectory)
    );
    assert_eq!(f.bindings.pce_invocations.get(), 0);
    for route in repository_routes()
        .into_iter()
        .filter(|route| route.kind == RouteKind::GateStructured)
    {
        let f = fixture();
        let argv = substitute_route(&route, &f.bindings).expect("canonical gate binding");
        assert_eq!(
            semantic_output_path(&route, &argv, &f.bindings),
            Ok(()),
            "canonical gate directory/{}",
            route_role(&route).expect("gate role")
        );
    }
}

#[test]
fn review_markdown_retained_separately() {
    let f = fixture();
    let json = f.bindings.verdict_root.join("separate.json");
    let markdown = f.bindings.verdict_root.join("separate.md");
    write_verdict(&json, "APPROVE");
    fs::write(&markdown, b"markdown sentinel").expect("markdown");
    let before = (
        markdown.clone(),
        fs::read(&markdown).expect("bytes"),
        fs::metadata(&markdown).expect("metadata").file_type(),
        markdown.exists(),
    );
    assert_eq!(
        route_verdict(&json, &f.bindings.verdict_schema),
        Ok(VerdictRoute::Forward)
    );
    let after = (
        markdown.clone(),
        fs::read(&markdown).expect("bytes"),
        fs::metadata(&markdown).expect("metadata").file_type(),
        markdown.exists(),
    );
    assert_eq!(before, after);
    assert_ne!(json, markdown);
}

#[test]
fn conformance_paths_and_projection_limitation() {
    let f = fixture();
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let report = review_document(&skill, &f.bindings).expect("canonical conformance");
    assert_eq!(
        report.observations.len(),
        EXPECTED_ANCHORED_ROUTE_COUNT,
        "codex_conformance_shim_terminal_envelope_accepts"
    );
    for role in ["milestone-planner", "step-planner"] {
        let graph_output = &f.bindings.graph_outputs[role];
        assert!(
            graph_output.exists(),
            "codex_conformance_shim_selects_payload_by_schema/{role}"
        );
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(graph_output).expect("graph artifact"))
                .expect("graph json");
        let schema: serde_json::Value =
            serde_json::from_slice(&fs::read(&f.bindings.graph_schema).expect("graph schema"))
                .expect("graph schema json");
        assert!(
            jsonschema::validator_for(&schema)
                .expect("graph validator")
                .is_valid(&value),
            "codex_conformance_shim_selects_payload_by_schema/{role}"
        );
    }
    let recovered = PathBuf::from(
        fs::read_to_string(f.bindings.cwd.join(".review/recovered-path")).expect("recorded path"),
    );
    let reviewer = repository_route("pr-reviewer");
    let reviewer_argv = substitute_route(&reviewer, &f.bindings).expect("reviewer binding");
    semantic_output_path(&reviewer, &reviewer_argv, &f.bindings).expect("reviewer semantic path");
    let parent = PathBuf::from(
        reviewer_argv
            .windows(2)
            .find(|pair| pair[0] == "-o")
            .expect("measured reviewer output")[1]
            .clone(),
    );
    assert_eq!(
        recovered, parent,
        "gate_conformance_shim_recovers_path_from_argv/recorded-parent"
    );
    assert!(
        parent.exists(),
        "gate_conformance_shim_recovers_path_from_argv/artifact"
    );
    let env = fs::read_to_string(f.bindings.cwd.join(".review/env")).expect("environment");
    assert!(
        !env.contains("PCE_GATE_OUTPUT_PATH")
            && !env.contains("PCE_CLAUDE_OUTPUT_PATH")
            && !env.contains("ARTIFACT="),
        "gate_conformance_shim_recovers_path_from_argv/environment"
    );
    let sentence = "`run_dispatch_projection` receives an already-parsed output path and performs no child artifact write, so it cannot inspect or falsify `-o` naming, verdict indexing, directory selection, gate-tail visibility, or sentinel preservation; a dry run is not evidence for those properties.";
    assert_eq!(
        skill.matches(sentence).count(),
        1,
        "projection_limitation_sentence_present"
    );
}

#[test]
fn falsification_route_has_one_binary_owned_frame_and_recoverable_path() {
    let f = fixture();
    let route = repository_route("falsification-critic");
    let delimiter = route
        .tokens
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    assert_eq!(&route.tokens[delimiter + 1..], ["{{CALLER_ARG}}"]);
    assert_eq!(
        route.tokens[delimiter + 1..]
            .iter()
            .filter(|token| {
                token.as_str() == "--append-system-prompt"
                    || token.starts_with("--append-system-prompt=")
            })
            .count(),
        0
    );

    let argv = substitute_route(&route, &f.bindings).expect("falsification binding");
    validate_route(&route, &argv).expect("falsification route");
    let parent = PathBuf::from(
        argv.windows(2)
            .find(|pair| pair[0] == "-o")
            .expect("parent output")[1]
            .clone(),
    );
    let observation = execute_route(&argv, &f.bindings).expect("falsification spawn");
    let child: Vec<String> = observation
        .child_argv
        .iter()
        .map(|argument| String::from_utf8(argument.clone()).expect("UTF-8 argv"))
        .collect();
    let pce = env!("CARGO_BIN_EXE_pce");
    let permission = format!("Bash({pce} gate exec:*)");
    assert_eq!(
        &child[..10],
        [
            "-p",
            "--output-format",
            "json",
            "--allowedTools",
            permission.as_str(),
            "Read",
            "Glob",
            "Grep",
            "Write",
            "--append-system-prompt",
        ]
    );
    assert_eq!(
        child
            .iter()
            .filter(|arg| *arg == "--append-system-prompt")
            .count(),
        1
    );
    let mandate = &child[10];
    assert_eq!(child[11], f.bindings.caller_arg.to_string_lossy());
    assert_eq!(mandate.matches("<output-path>").count(), 1);
    assert_eq!(mandate.matches("</output-path>").count(), 1);
    assert_eq!(mandate.matches("<gate-exec-command>").count(), 1);
    assert_eq!(mandate.matches("</gate-exec-command>").count(), 1);
    let recovered_command = mandate
        .split_once("<gate-exec-command>")
        .and_then(|(_, tail)| tail.split_once("</gate-exec-command>"))
        .map(|(command, _)| command)
        .expect("gate exec command");
    assert_eq!(recovered_command, format!("{pce} gate exec"));
    assert!(!recovered_command.contains("target/debug/deps/"));
    let recovered_from_mandate = mandate
        .split_once("<output-path>")
        .and_then(|(_, tail)| tail.split_once("</output-path>"))
        .map(|(path, _)| PathBuf::from(path))
        .expect("mandate path");
    assert_eq!(recovered_from_mandate, parent);
    assert!(!mandate.contains(&f.bindings.verdict_schema.display().to_string()));
    assert!(!mandate.contains(&f.bindings.caller_arg.to_string_lossy().to_string()));
    let recovered = PathBuf::from(
        fs::read_to_string(f.bindings.cwd.join(".review/recovered-path")).expect("shim path"),
    );
    assert_eq!(recovered, parent);
    assert!(parent.exists());
    assert_schema_valid(
        &parent,
        &f.bindings.verdict_schema,
        "falsification artifact",
    );
    let evidence = PathBuf::from(format!("{}.executions.json", parent.display()));
    assert_eq!(
        fs::read(&evidence).expect("empty gate execution evidence"),
        b"{\"schema_id\":\"pce.gate-execution-evidence\",\"schema_version\":1,\"executions\":[]}\n"
    );
    assert_eq!(
        fs::metadata(&evidence)
            .expect("evidence metadata")
            .permissions()
            .mode()
            & 0o777,
        0o444
    );
    let env = fs::read_to_string(f.bindings.cwd.join(".review/env")).expect("environment");
    let socket = env
        .lines()
        .find_map(|line| line.strip_prefix("PCE_GATE_EXEC_SOCKET="))
        .map(PathBuf::from)
        .expect("gate execution socket environment");
    assert!(socket.is_absolute());
    assert!(socket.as_os_str().as_encoded_bytes().len() <= 103);
    assert!(!socket.exists());
}

#[test]
fn falsification_gate_tail_deviations_red() {
    let f = fixture();
    let route = repository_route("falsification-critic");
    let base = substitute_route(&route, &f.bindings).expect("falsification binding");
    let delimiter = base
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    let mut extra = base.clone();
    extra.push(OsString::from("second"));
    assert_eq!(
        validate_route(&route, &extra),
        Err(ReviewError::GateTailCardinality)
    );
    let mut empty = base.clone();
    empty[delimiter + 1] = OsString::new();
    assert_eq!(
        validate_route(&route, &empty),
        Err(ReviewError::GateTailPrompt)
    );
    for token in ["--append-system-prompt", "--append-system-prompt=caller"] {
        let mut resupplied = base.clone();
        resupplied[delimiter + 1] = OsString::from(token);
        assert_eq!(
            validate_route(&route, &resupplied),
            Err(ReviewError::CallerTailResuppliesBinaryArgument)
        );
    }
}

fn gate_argv() -> (Fixture, AnchoredRoute, Vec<OsString>) {
    let f = fixture();
    let route = repository_route("pr-reviewer");
    let argv = substitute_route(&route, &f.bindings).expect("gate argv");
    (f, route, argv)
}

fn assert_gate_tail_error(
    name: &str,
    mutate: impl FnOnce(&mut Vec<OsString>),
    expected: ReviewError,
) {
    let (_f, route, mut argv) = gate_argv();
    mutate(&mut argv);
    assert_eq!(validate_route(&route, &argv), Err(expected), "{name}");
}

#[test]
fn gate_tail_output_path_equal_accepts() {
    let (_f, route, argv) = gate_argv();
    validate_route(&route, &argv).expect("gate_tail_output_path_equal_accepts");
    let parent = argv
        .windows(2)
        .find(|pair| pair[0] == "-o")
        .expect("parent output");
    let tail = argv
        .windows(2)
        .find(|pair| pair[0] == "--append-system-prompt")
        .expect("tail path");
    assert_eq!(parent[1], tail[1]);
    let gate_routes: Vec<_> = repository_routes()
        .into_iter()
        .filter(|route| route.kind == RouteKind::GateStructured)
        .collect();
    let excluded: Vec<String> = gate_routes
        .iter()
        .filter_map(|route| route_role(route))
        .filter(|role| *role == "falsification-critic")
        .map(str::to_owned)
        .collect();
    assert_eq!(excluded, ["falsification-critic"]);
    let caller_authored: Vec<_> = gate_routes
        .into_iter()
        .filter(|route| route_role(route) != Some("falsification-critic"))
        .collect();
    assert_eq!(caller_authored.len(), 5);
    for route in caller_authored {
        let f = fixture();
        let argv = substitute_route(&route, &f.bindings).expect("canonical gate binding");
        validate_route(&route, &argv)
            .unwrap_or_else(|error| panic!("{}: {error:?}", route_role(&route).expect("role")));
        let parent = argv
            .windows(2)
            .find(|pair| pair[0] == "-o")
            .expect("parent");
        let tail = argv
            .windows(2)
            .find(|pair| pair[0] == "--append-system-prompt")
            .expect("tail");
        assert_eq!(parent[1], tail[1], "{}", route_role(&route).expect("role"));
    }
}

#[test]
fn gate_tail_output_path_mismatch_reds() {
    assert_gate_tail_error(
        "gate_tail_output_path_mismatch_reds",
        |argv| {
            let index = argv
                .iter()
                .position(|arg| arg == "--append-system-prompt")
                .expect("designator");
            argv[index + 1] = "/tmp/same-name/review-3.json".into();
        },
        ReviewError::GateOutputPathMismatch,
    );
}

#[test]
fn gate_tail_missing_designator_reds() {
    assert_gate_tail_error(
        "gate_tail_missing_designator_reds",
        |argv| {
            let index = argv
                .iter()
                .position(|arg| arg == "--append-system-prompt")
                .expect("designator");
            argv[index] = "--missing-designator".into();
        },
        ReviewError::GateTailDesignator,
    );
}

#[test]
fn gate_tail_missing_path_reds() {
    assert_gate_tail_error(
        "gate_tail_missing_path_reds",
        |argv| {
            let index = argv
                .iter()
                .position(|arg| arg == "--append-system-prompt")
                .expect("designator");
            argv[index + 1] = "".into();
        },
        ReviewError::GateTailValue,
    );
}

#[test]
fn gate_tail_relative_path_reds() {
    assert_gate_tail_error(
        "gate_tail_relative_path_reds",
        |argv| {
            let index = argv
                .iter()
                .position(|arg| arg == "--append-system-prompt")
                .expect("designator");
            argv[index + 1] = "review.json".into();
        },
        ReviewError::GateTailAbsolute,
    );
}

#[test]
fn gate_tail_missing_prompt_reds() {
    assert_gate_tail_error(
        "gate_tail_missing_prompt_reds",
        |argv| {
            let prompt = argv.last_mut().expect("prompt");
            *prompt = "".into();
        },
        ReviewError::GateTailPrompt,
    );
}

#[test]
fn gate_tail_extra_argument_reds() {
    assert_gate_tail_error(
        "gate_tail_extra_argument_reds",
        |argv| argv.push("extra".into()),
        ReviewError::GateTailCardinality,
    );
}

fn codex_shim_args(f: &Fixture, schema: Option<&Path>, outputs: &[&Path]) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "exec".into(),
        "--json".into(),
        "-C".into(),
        f.bindings.cwd.as_os_str().to_owned(),
        "--sandbox".into(),
        "workspace-write".into(),
    ];
    if let Some(schema) = schema {
        args.extend(["--output-schema".into(), schema.as_os_str().to_owned()]);
    }
    for output in outputs {
        args.extend(["-o".into(), output.as_os_str().to_owned()]);
    }
    args.push("opaque prompt".into());
    args
}

fn assert_schema_valid(path: &Path, schema: &Path, name: &str) {
    let instance: serde_json::Value =
        serde_json::from_slice(&fs::read(path).expect("artifact bytes")).expect("artifact JSON");
    let schema: serde_json::Value =
        serde_json::from_slice(&fs::read(schema).expect("schema bytes")).expect("schema JSON");
    assert!(
        jsonschema::validator_for(&schema)
            .expect("validator")
            .is_valid(&instance),
        "{name}: schema validation"
    );
}

#[test]
fn codex_conformance_shim_recovers_output_from_argv() {
    let f = fixture();
    let parent = f.bindings.verdict_root.join("codex-recovered.json");
    let output = run_direct_shim(
        &f,
        "codex",
        &codex_shim_args(&f, Some(&f.bindings.verdict_schema), &[&parent]),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let recovered = PathBuf::from(
        fs::read_to_string(f.bindings.cwd.join(".review/recovered-path")).expect("recovered path"),
    );
    let artifact = fs::read_dir(parent.parent().expect("artifact directory"))
        .expect("artifact directory")
        .map(|entry| entry.expect("artifact entry").path())
        .find(|path| path.file_name() == parent.file_name())
        .expect("artifact path");
    assert_eq!(recovered, parent, "parent/shim path");
    assert_eq!(artifact, parent, "parent/artifact path");
    assert_eq!(recovered, artifact, "shim/artifact path");
    let environment = fs::read_to_string(f.bindings.cwd.join(".review/env")).expect("environment");
    assert!(!environment.contains("ARTIFACT=") && !environment.contains("OUTPUT_PATH"));
    assert_schema_valid(&parent, &f.bindings.verdict_schema, "codex recovery");
}

#[test]
fn codex_conformance_shim_no_output_reds() {
    let f = fixture();
    let output = run_direct_shim(
        &f,
        "codex",
        &codex_shim_args(&f, Some(&f.bindings.verdict_schema), &[]),
    );
    assert!(!output.status.success(), "zero -o must fail");
    assert!(!f.bindings.output.exists(), "zero -o wrote no artifact");
}

#[test]
fn codex_conformance_shim_duplicate_output_reds() {
    let f = fixture();
    let first = f.bindings.verdict_root.join("duplicate-first.json");
    let second = f.bindings.verdict_root.join("duplicate-second.json");
    let output = run_direct_shim(
        &f,
        "codex",
        &codex_shim_args(&f, Some(&f.bindings.verdict_schema), &[&first, &second]),
    );
    assert!(!output.status.success(), "duplicate -o must fail");
    assert!(
        !first.exists() && !second.exists(),
        "duplicate -o wrote no artifact"
    );
}

#[test]
fn codex_conformance_shim_selects_payload_by_schema() {
    let f = fixture();
    for (role, schema, expected_schema) in [
        (
            "milestone-planner",
            &f.bindings.graph_schema,
            &f.bindings.graph_schema,
        ),
        (
            "step-planner",
            &f.bindings.graph_schema,
            &f.bindings.graph_schema,
        ),
        (
            "step-executor",
            &f.bindings.verdict_schema,
            &f.bindings.verdict_schema,
        ),
    ] {
        let path = f.bindings.verdict_root.join(format!("{role}.json"));
        let output = run_direct_shim(&f, "codex", &codex_shim_args(&f, Some(schema), &[&path]));
        assert!(
            output.status.success(),
            "{role}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_schema_valid(&path, expected_schema, role);
    }
}

#[test]
fn codex_conformance_shim_terminal_envelope_accepts() {
    const ENVELOPE: &[u8] = b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":1,\"cached_input_tokens\":2,\"output_tokens\":3,\"reasoning_output_tokens\":4}}\n";
    let f = fixture();
    let structured = f.bindings.verdict_root.join("terminal.json");
    let output = run_direct_shim(
        &f,
        "codex",
        &codex_shim_args(&f, Some(&f.bindings.verdict_schema), &[&structured]),
    );
    assert_eq!(output.stdout, ENVELOPE, "structured direct envelope");
    let output = run_direct_shim(&f, "codex", &codex_shim_args(&f, None, &[]));
    assert_eq!(output.stdout, ENVELOPE, "unstructured direct envelope");
    for role in [
        "milestone-planner",
        "step-planner",
        "step-plan-writer",
        "step-executor",
    ] {
        let f = fixture();
        review_document(&route_document(&repository_route(role)), &f.bindings)
            .unwrap_or_else(|error| panic!("{role} built-binary envelope: {error:?}"));
    }
}

fn gate_shim_args(paths: &[&Path]) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-p".into(), "--output-format".into(), "json".into()];
    for path in paths {
        args.extend(["--append-system-prompt".into(), path.as_os_str().to_owned()]);
    }
    args.push("opaque prompt".into());
    args
}

#[test]
fn gate_conformance_shim_recovers_path_from_argv() {
    let f = fixture();
    let parent = f.bindings.verdict_root.join("gate-recovered.json");
    let output = run_direct_shim(&f, "claude", &gate_shim_args(&[&parent]));
    assert!(output.status.success());
    let recovered = PathBuf::from(
        fs::read_to_string(f.bindings.cwd.join(".review/recovered-path")).expect("recovered path"),
    );
    let artifact = fs::read_dir(parent.parent().expect("artifact directory"))
        .expect("artifact directory")
        .map(|entry| entry.expect("artifact entry").path())
        .find(|path| path.file_name() == parent.file_name())
        .expect("artifact path");
    assert_eq!(recovered, parent);
    assert_eq!(artifact, parent);
    assert_eq!(recovered, artifact);
    assert_schema_valid(&parent, &f.bindings.verdict_schema, "gate recovery");
    let environment = fs::read_to_string(f.bindings.cwd.join(".review/env")).expect("environment");
    assert!(!environment.contains("ARTIFACT=") && !environment.contains("OUTPUT_PATH"));
}

#[test]
fn gate_conformance_shim_no_designated_path_reds() {
    let f = fixture();
    let output = run_direct_shim(&f, "claude", &gate_shim_args(&[]));
    assert!(!output.status.success());
    assert!(!f.bindings.output.exists());
}

#[test]
fn gate_conformance_shim_duplicate_designated_path_reds() {
    let f = fixture();
    let first = f.bindings.verdict_root.join("gate-first.json");
    let second = f.bindings.verdict_root.join("gate-second.json");
    let output = run_direct_shim(&f, "claude", &gate_shim_args(&[&first, &second]));
    assert!(!output.status.success());
    assert!(!first.exists() && !second.exists());
}

const CONSOLIDATION_EXPLANATION: &str = "The dispatch-issuance consolidation marker immediately above activates only `reject_colocated_standalone_append`, which rejects a standalone `pce log ... --kind dispatch` append in the same ATX section as an anchored dispatch route when the marker is present. That marker-gated co-location predicate is strictly weaker than, and is subsumed by, the separate document-wide `real_skill_has_no_standalone_dispatch_append_anywhere` backstop, which rejects such an append in every venue; keep exactly one marker while binary-owned issuance is the operating contract. The marker is protocol state, not a decorative comment.";

const CONSOLIDATION_SCOPE_CLAUSES: [(&str, &str); 3] = [
    (
        "co-location-only",
        "activates only `reject_colocated_standalone_append`",
    ),
    (
        "strictly-weaker",
        "strictly weaker than, and is subsumed by",
    ),
    (
        "document-wide-backstop",
        "separate document-wide `real_skill_has_no_standalone_dispatch_append_anywhere` backstop",
    ),
];

const METER_DOCUMENTATION_CLAIMS: [(&str, &str); 2] = [
    (
        "surface",
        "pce log meter                                                     JSONL read from STDIN to EOF",
    ),
    (
        "stdin-only-rationale",
        "The meter accepts JSONL only on standard input so it has no path or transcript-read capability, while `pce log` and `pce log read` retain path arguments for append and raw retrieval.",
    ),
];

const BINARY_OWNED_SENTENCES: [&str; 4] = [
    "`pce dispatch` appends dispatch issuance from its complete ordered logging envelope; the orchestrator invokes the following manual append, read, status, readiness, and contract surfaces in their exact argument order:",
    "The `dispatch` payload is binary-owned: each anchored route supplies `node`, an exact registry `role`, the exact repository `ref`, and non-empty exact invocation `evidence` through the complete ordered logging envelope, and `pce dispatch` appends issuance before it spawns the child.",
    "For binary-owned `dispatch`, `evidence` is the non-empty exact invocation supplied through the ordered dispatch envelope.",
    "`pce dispatch` appends one `dispatch` record from the complete ordered logging envelope before spawning every Claude or Codex child.",
];

const BINARY_OWNED_CLAUSES: [(&str, &str); 6] = [
    (
        "binary-owner",
        "`pce dispatch` appends dispatch issuance from its complete ordered logging envelope",
    ),
    ("payload-role", "an exact registry `role`"),
    ("payload-ref", "the exact repository `ref`"),
    ("payload-evidence", "non-empty exact invocation `evidence`"),
    (
        "pre-spawn",
        "`pce dispatch` appends issuance before it spawns the child",
    ),
    (
        "ref-and-ordinal-source",
        "sole source for dispatch refs and for the exact `(node, role)` issuance ordinal",
    ),
];

fn forbidden_symbol_matches(source: &str) -> Vec<String> {
    [
        ["anchor_shell_", "operator"].concat(),
        ["repeated_placeholder_values_are_", "byte_identical"].concat(),
        ["PCE_REVIEW_", "CASE"].concat(),
    ]
    .into_iter()
    .filter(|symbol| source.contains(symbol))
    .collect()
}

#[test]
fn real_skill_has_no_prohibited_outside_anchor_fragment() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    reject_outside_fragments(&skill).expect("real_skill_has_no_prohibited_outside_anchor_fragment");
}

#[test]
fn real_skill_has_exactly_one_consolidation_marker() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let offsets: Vec<_> = skill
        .match_indices(CONSOLIDATION_MARKER)
        .map(|(offset, _)| offset)
        .collect();
    assert_eq!(
        offsets.len(),
        1,
        "real_skill_has_exactly_one_consolidation_marker: measured_count={}, offsets={offsets:?}",
        offsets.len()
    );
    reject_colocated_standalone_append(&skill).expect("marker-activated co-location review");
}

#[test]
fn real_skill_explains_consolidation_marker() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let explanation_count = skill.matches(CONSOLIDATION_EXPLANATION).count();
    let adjacent = format!("{CONSOLIDATION_MARKER}\n\n{CONSOLIDATION_EXPLANATION}");
    let adjacent_count = skill.matches(&adjacent).count();
    assert_eq!(
        (explanation_count, adjacent_count),
        (1, 1),
        "real_skill_explains_consolidation_marker: explanation_count={explanation_count}, adjacent_count={adjacent_count}"
    );
}

#[test]
fn consolidation_marker_scope_claim_is_exact() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let marker_count = skill.matches(CONSOLIDATION_MARKER).count();
    let clause_counts: Vec<_> = CONSOLIDATION_SCOPE_CLAUSES
        .iter()
        .map(|(label, clause)| (*label, skill.matches(clause).count()))
        .collect();
    for (label, current) in &clause_counts {
        assert_eq!(
            *current, marker_count,
            "consolidation_marker_scope_claim_is_exact/{label}: marker_count={marker_count}, clause_counts={clause_counts:?}"
        );
    }
}

#[test]
fn meter_stdin_only_surface_is_documented() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let claim_counts: Vec<_> = METER_DOCUMENTATION_CLAIMS
        .iter()
        .map(|(label, claim)| (*label, skill.matches(claim).count()))
        .collect();
    for (label, current) in &claim_counts {
        assert_eq!(
            *current, 1,
            "meter_stdin_only_surface_is_documented/{label}: claim_counts={claim_counts:?}"
        );
    }
}

#[test]
fn binary_owned_issuance_semantics_are_exact() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let clause_counts: Vec<_> = BINARY_OWNED_CLAUSES
        .iter()
        .map(|(label, clause)| (*label, skill.matches(clause).count()))
        .collect();
    let sentence_counts: Vec<_> = BINARY_OWNED_SENTENCES
        .iter()
        .map(|sentence| skill.matches(sentence).count())
        .collect();
    for (label, current) in clause_counts {
        assert_eq!(
            current, 1,
            "binary_owned_issuance_semantics_are_exact/{label}: green=1, current={current}"
        );
    }
    for (index, current) in sentence_counts.into_iter().enumerate() {
        assert_eq!(
            current, 1,
            "binary_owned_issuance_semantics_are_exact/sentence-{index}: green=1, current={current}"
        );
    }
}

#[test]
fn completion_manual_appends_and_orchestration_are_exact() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let execution = "pce log --file <LOG_PATH> --kind criterion-execution --node <NODE>";
    let addition = "pce log --file <LOG_PATH> --kind criterion-added --node <NODE>";
    assert_eq!(skill.lines().filter(|line| *line == execution).count(), 3);
    assert_eq!(skill.lines().filter(|line| *line == addition).count(), 1);

    for payload in [
        r#"{"criterion":{"name":"<CRITERION_NAME>","input":"<EXACT_INPUT>","observation":"<EXPECTED_OBSERVATION>"},"finished_result":"<FINISHED_RESULT>","outcome":{"status":"passed","observed_result":"<OBSERVED_RESULT>"},"evidence":"<NON_EMPTY_EXACT_INVOCATION>"}"#,
        r#"{"criterion":{"name":"<CRITERION_NAME>","input":"<EXACT_INPUT>","observation":"<EXPECTED_OBSERVATION>"},"finished_result":"<FINISHED_RESULT>","outcome":{"status":"failed","observed_result":"<OBSERVED_RESULT>"},"evidence":"<NON_EMPTY_EXACT_INVOCATION>"}"#,
        r#"{"criterion":{"name":"<CRITERION_NAME>","input":"<EXACT_INPUT>","observation":"<EXPECTED_OBSERVATION>"},"finished_result":"<FINISHED_RESULT>","outcome":{"status":"unpaid","reason":"<WHY_THIS_RUN_CANNOT_EXECUTE_IT>"},"evidence":"<NON_EMPTY_EXACT_INVOCATION>"}"#,
        r#"{"criterion":{"name":"<CRITERION_NAME>","input":"<EXACT_INPUT>","observation":"<EXPECTED_OBSERVATION>"},"change_of_course":"<NON_EMPTY_CHANGE_OF_COURSE>"}"#,
    ] {
        assert_eq!(skill.matches(payload).count(), 1, "payload: {payload}");
    }

    let command = "pce completion check --file <LOG_PATH> --vision-dir <VISION_DIR> --finished-result <FINISHED_RESULT>";
    let merge = skill
        .find("After the final milestone merge")
        .expect("merge language");
    let command_offset = skill.find(command).expect("completion command");
    let done = skill.find("\n## Done\n").expect("done section");
    assert_eq!(skill.matches(command).count(), 1);
    assert!(merge < command_offset && command_offset < done);

    for sentence in [
        "On refusal, do not enter `## Done` and do not post the final summary.",
        "On completion, name every unpaid criterion with its exact input and unpaid reason; never report it as passed or green.",
    ] {
        assert_eq!(skill.matches(sentence).count(), 1);
        let offset = skill.find(sentence).expect("completion sentence");
        assert!(command_offset < offset && offset < done);
    }
}

#[test]
fn real_skill_has_no_standalone_dispatch_append_anywhere() {
    let skill = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    let matches = standalone_dispatch_append_lines(&skill);
    let by_venue = append_lines_by_venue(&skill);
    for venue in [
        "general-surface",
        "phase-0",
        "phase-1",
        "phase-2",
        "phase-3",
        "other",
    ] {
        let current = by_venue.get(venue).cloned().unwrap_or_default();
        assert!(
            current.is_empty(),
            "real_skill_has_no_standalone_dispatch_append_anywhere/{venue}: green=[], current={current:?}"
        );
    }
    assert!(matches.is_empty());
}

#[test]
fn degenerate_control_audit_known_match_reds() {
    let known = ["anchor_shell_", "operator"].concat();
    assert_eq!(
        forbidden_symbol_matches(&known),
        vec![known],
        "degenerate_control_audit_known_match_reds: green=1, current=0"
    );
}

#[test]
fn review_has_no_degenerate_controls() {
    let source = fs::read_to_string(file!()).expect("review source");
    let matches = forbidden_symbol_matches(&source);
    assert!(
        matches.is_empty(),
        "review_has_no_degenerate_controls: {matches:?}"
    );
}

#[test]
fn raw_codex_exec_positive_control_reds() {
    let fixture = fixture();
    assert_eq!(
        review_document("codex exec", &fixture.bindings).expect_err("raw must red"),
        ReviewError::RawCodexExec
    );
    assert!(!fixture.bindings.cwd.join(".review/argv").exists());
    assert_eq!(fixture.bindings.pce_invocations.get(), 0);
}

#[test]
fn automatic_coverage_added_anchor_green() {
    let fixture = fixture();
    let base = "foundation prose\n";
    let before = review_document(base, &fixture.bindings).expect("base");
    let augmented = format!(
        "{base}{}",
        document(
            "codex-unstructured",
            &base_command(RouteKind::CodexUnstructured)
        )
    );
    let after = review_document(&augmented, &fixture.bindings).expect("added route");
    assert_eq!(after.routes.len(), before.routes.len() + 1);
    assert_eq!(after.observations.len(), 1);
    assert_eq!(after.observations[0].status, 0);
    assert_eq!(after.observations[0].source_line, 2);
}

#[test]
fn automatic_coverage_added_malformed_anchor_red() {
    let prefix = "foundation prose\n";
    let command = base_command(RouteKind::CodexUnstructured).replace("{{CALLER_ARG}}", "exec");
    let malformed = format!("{prefix}{}", document("codex-unstructured", &command));
    let green = format!(
        "{prefix}{}",
        document(
            "codex-unstructured",
            &base_command(RouteKind::CodexUnstructured)
        )
    );
    assert_eq!(
        &malformed.as_bytes()[..prefix.len()],
        &green.as_bytes()[..prefix.len()]
    );
    assert_eq!(
        error_for(&malformed),
        ReviewError::CallerTailResuppliesBinaryArgument
    );
    let outside_fence = "```text\npce dispatch gate --\n```\n";
    assert!(unit_has_prohibited_fragment("pce dispatch gate --"));
    assert_eq!(
        review_document(outside_fence, &fixture().bindings)
            .expect_err("outside fenced known-match must red"),
        ReviewError::OutsideAnchorDispatchFragment
    );
}

#[test]
fn anchor_metadata_and_fence_cases() {
    let good = base_command(RouteKind::CodexUnstructured);
    let cases = [
        (
            "anchor_unknown_kind",
            document("future", &good),
            ReviewError::UnknownKind,
        ),
        (
            "anchor_missing_kind",
            format!("<!-- pce-dispatch-route -->\n```sh\n{good}\n```"),
            ReviewError::MissingKind,
        ),
        (
            "anchor_extra_metadata",
            format!(
                "<!-- pce-dispatch-route kind=\"codex-unstructured\" extra=\"x\" -->\n```sh\n{good}\n```"
            ),
            ReviewError::ExtraMetadata,
        ),
        (
            "anchor_duplicate_metadata",
            format!(
                "<!-- pce-dispatch-route kind=\"codex-unstructured\" kind=\"codex-unstructured\" -->\n```sh\n{good}\n```"
            ),
            ReviewError::DuplicateMetadata,
        ),
        (
            "anchor_target_kind_mismatch",
            document("gate-structured", &good),
            ReviewError::TargetKindMismatch,
        ),
        (
            "anchor_missing_fence",
            marker("codex-unstructured"),
            ReviewError::MissingFence,
        ),
        (
            "anchor_intervening_nonblank",
            format!(
                "{}\nprose\n```sh\n{good}\n```",
                marker("codex-unstructured")
            ),
            ReviewError::InterveningContent,
        ),
        (
            "anchor_wrong_fence_language",
            format!("{}\n```bash\n{good}\n```", marker("codex-unstructured")),
            ReviewError::WrongFenceLanguage,
        ),
        (
            "anchor_unclosed_fence",
            format!("{}\n```sh\n{good}", marker("codex-unstructured")),
            ReviewError::UnclosedFence,
        ),
        (
            "anchor_empty_command",
            format!("{}\n```sh\n```", marker("codex-unstructured")),
            ReviewError::EmptyCommand,
        ),
        (
            "anchor_multiple_commands",
            format!(
                "{}\n```sh\n{good}\n{good}\n```",
                marker("codex-unstructured")
            ),
            ReviewError::MultipleCommands,
        ),
    ];
    for (name, markdown, expected) in cases {
        assert_eq!(error_for(&markdown), expected, "{name}");
    }
}

#[test]
fn anchor_prefix_and_shell_syntax_cases() {
    let good = base_command(RouteKind::CodexUnstructured);
    for (name, needle, replacement, expected) in [
        (
            "anchor_wrong_first_token",
            "pce dispatch",
            "pcx dispatch",
            ReviewError::WrongFirstToken,
        ),
        (
            "anchor_wrong_second_token",
            "pce dispatch",
            "pce run",
            ReviewError::WrongSecondToken,
        ),
        (
            "anchor_quote",
            "{{CALLER_ARG}}",
            "'x'",
            ReviewError::ShellConstruct,
        ),
        (
            "anchor_command_substitution",
            "{{CALLER_ARG}}",
            "$(x)",
            ReviewError::ShellConstruct,
        ),
        (
            "anchor_variable_expansion",
            "{{CALLER_ARG}}",
            "$X",
            ReviewError::ShellConstruct,
        ),
        (
            "anchor_pipe",
            "{{CALLER_ARG}}",
            "|",
            ReviewError::ShellConstruct,
        ),
        (
            "anchor_list",
            "{{CALLER_ARG}}",
            ";",
            ReviewError::ShellConstruct,
        ),
        (
            "anchor_redirection",
            "{{CALLER_ARG}}",
            ">x",
            ReviewError::ShellConstruct,
        ),
        (
            "anchor_backgrounding",
            "{{CALLER_ARG}}",
            "&",
            ReviewError::ShellConstruct,
        ),
    ] {
        assert_eq!(
            error_for(&document(
                "codex-unstructured",
                &good.replace(needle, replacement)
            )),
            expected,
            "{name}"
        );
    }
}

#[test]
fn line_continuation_cases() {
    let good = base_command(RouteKind::CodexUnstructured);
    let joined = good.replace("{{CALLER_ARG}}", "\\\n{{CALLER_ARG}}");
    assert!(
        joined.contains("-- \\\n{{CALLER_ARG}}"),
        "joined_line_continuation_accepts: caller-tail split precondition"
    );
    let joined_fixture = fixture();
    let report = review_document(
        &document("codex-unstructured", &joined),
        &joined_fixture.bindings,
    )
    .expect("joined_line_continuation_accepts");
    assert_eq!(report.observations.len(), 1);
    let unbroken = review_document(
        &document("codex-unstructured", &good),
        &joined_fixture.bindings,
    )
    .expect("unbroken control");
    let joined_argv = &report.observations[0].child_argv;
    let unbroken_argv = &unbroken.observations[0].child_argv;
    assert_eq!(
        joined_argv, unbroken_argv,
        "joined_line_continuation_accepts"
    );
    assert_eq!(
        error_for(&format!(
            "{}\n```sh\n{good}\\\n```",
            marker("codex-unstructured")
        )),
        ReviewError::DanglingContinuation,
        "dangling_line_continuation_reds"
    );
    assert_eq!(
        error_for(&document(
            "codex-unstructured",
            &good.replace("dispatch", "dis\\patch")
        )),
        ReviewError::MidlineBackslash,
        "midline_backslash_reds"
    );
    assert_eq!(
        error_for(&document(
            "codex-unstructured",
            &good.replace("--sandbox", "\\\\\n--sandbox")
        )),
        ReviewError::DoubleTerminalBackslash,
        "double_backslash_line_end_reds"
    );
}

#[test]
fn placeholder_cases() {
    let base = base_command(RouteKind::CodexStructured);
    let positions = [
        ("unknown_placeholder_cwd", "{{CWD}}"),
        ("unknown_placeholder_schema", "{{SCHEMA}}"),
        ("unknown_placeholder_output", "{{OUTPUT}}"),
        ("unknown_placeholder_env", "{{PATH_ENV}}"),
        ("unknown_placeholder_caller_tail", "{{CALLER_ARG}}"),
    ];
    for (name, position) in positions {
        assert_document_error_before_pce(
            name,
            &document("codex-structured", &base.replace(position, "{{UNKNOWN}}")),
            ReviewError::UnknownPlaceholder,
        );
    }
    let logging = "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write --env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} --plan-file {{PLAN_FILE}} --log-file {{LOG_FILE}} --node {{NODE}} --role {{ROLE}} --ref {{REF}} --evidence {{EVIDENCE}} --required-artifact {{OUTPUT}} --planning-act {{PLANNING_ACT}} --dry-run -- {{CALLER_ARG}}";
    for (name, position) in [
        ("unknown_placeholder_plan_file", "{{PLAN_FILE}}"),
        ("unknown_placeholder_log_file", "{{LOG_FILE}}"),
        ("unknown_placeholder_node", "{{NODE}}"),
        ("unknown_placeholder_role", "{{ROLE}}"),
        ("unknown_placeholder_ref", "{{REF}}"),
        ("unknown_placeholder_evidence", "{{EVIDENCE}}"),
        ("unknown_placeholder_planning_act", "{{PLANNING_ACT}}"),
    ] {
        assert_document_error_before_pce(
            name,
            &document(
                "codex-structured",
                &logging.replace(position, "{{UNKNOWN}}"),
            ),
            ReviewError::UnknownPlaceholder,
        );
    }
    assert_eq!(
        error_for(&document(
            "codex-unstructured",
            &base_command(RouteKind::CodexUnstructured).replace("{{CALLER_ARG}}", "{CALLER_ARG}")
        )),
        ReviewError::MalformedPlaceholder,
        "malformed_placeholder_reds"
    );
    assert_eq!(
        error_for(&document(
            "codex-unstructured",
            &base_command(RouteKind::CodexUnstructured)
                .replace("{{CALLER_ARG}}", "x{{CALLER_ARG}}")
        )),
        ReviewError::EmbeddedPlaceholder,
        "embedded_placeholder_reds"
    );

    for (name, field) in [
        ("empty_placeholder_role_reds", "role"),
        ("empty_placeholder_ref_reds", "dispatch_ref"),
        ("empty_placeholder_evidence_reds", "evidence"),
    ] {
        let mut fixture = fixture();
        match field {
            "role" => fixture.bindings.role = OsString::new(),
            "dispatch_ref" => fixture.bindings.dispatch_ref = OsString::new(),
            "evidence" => fixture.bindings.evidence = OsString::new(),
            _ => unreachable!(),
        }
        let expected = match field {
            "role" => ReviewError::EmptyRole,
            "dispatch_ref" => ReviewError::EmptyRef,
            _ => ReviewError::EmptyEvidence,
        };
        assert_eq!(
            review_document(&document("codex-structured", logging), &fixture.bindings)
                .expect_err("empty binding must red"),
            expected,
            "{name}"
        );
        assert!(
            !fixture.bindings.cwd.join(".review/argv").exists(),
            "{name}"
        );
    }
}

#[test]
fn route_shape_tail_and_delimiter_cases() {
    let codex = base_command(RouteKind::CodexUnstructured);
    for (name, tail) in [
        ("codex_tail_exec_prefix_rejected", "exec"),
        ("codex_tail_json_rejected", "--json"),
        ("codex_tail_c_rejected", "-C"),
        ("codex_tail_sandbox_rejected", "--sandbox"),
        ("codex_tail_sandbox_equals_rejected", "--sandbox=x"),
        ("codex_tail_output_schema_rejected", "--output-schema"),
        (
            "codex_tail_output_schema_equals_rejected",
            "--output-schema=x",
        ),
        ("codex_tail_output_rejected", "-o"),
    ] {
        assert_eq!(
            error_for(&document(
                "codex-unstructured",
                &codex.replace("{{CALLER_ARG}}", tail)
            )),
            ReviewError::CallerTailResuppliesBinaryArgument,
            "{name}"
        );
    }
    let later_fixture = fixture();
    let later = codex.replace("{{CALLER_ARG}}", "later exec");
    review_document(
        &document("codex-unstructured", &later),
        &later_fixture.bindings,
    )
    .expect("codex_tail_later_exec_accepts");
    let gate = base_command(RouteKind::GateStructured);
    for (name, tail) in [
        ("gate_tail_p_rejected", "-p"),
        (
            "gate_output_format_separate_rejected_route",
            "--output-format",
        ),
        (
            "gate_output_format_equals_rejected_route",
            "--output-format=json",
        ),
    ] {
        assert_eq!(
            error_for(&document(
                "gate-structured",
                &gate.replace("{{CALLER_ARG}}", tail)
            )),
            ReviewError::CallerTailResuppliesBinaryArgument,
            "{name}"
        );
    }
    assert_eq!(
        error_for(&document(
            "codex-unstructured",
            &codex.replace("{{CALLER_ARG}}", "x -- y")
        )),
        ReviewError::MultipleDelimiters,
        "codex_multiple_delimiters_route"
    );
    assert_eq!(
        error_for(&document(
            "gate-structured",
            &gate.replace("{{CALLER_ARG}}", "x -- y")
        )),
        ReviewError::MultipleDelimiters,
        "gate_multiple_delimiters_route"
    );
    let pair = "--output-schema {{SCHEMA}} -o {{OUTPUT}} ";
    assert_eq!(
        error_for(&document(
            "codex-unstructured",
            &codex.replace(
                "-- {{CALLER_ARG}}",
                "--output-schema {{SCHEMA}} -o {{OUTPUT}} -- {{CALLER_ARG}}"
            )
        )),
        ReviewError::ParentGrammar,
        "codex_unstructured_with_structured_pair_rejected"
    );
    let structured = base_command(RouteKind::CodexStructured);
    let structured_fixture = fixture();
    let mut routes = extract_anchored_routes(&document("codex-structured", &structured))
        .expect("structured extraction");
    let structured_route = routes.remove(0);
    let structured_argv = substitute_route(&structured_route, &structured_fixture.bindings)
        .expect("structured substitution");
    validate_route(&structured_route, &structured_argv).expect("codex_structured_pair_accepts");
    let unstructured_fixture = fixture();
    review_document(
        &document("codex-unstructured", &codex),
        &unstructured_fixture.bindings,
    )
    .expect("codex_unstructured_shape_accepts");
    assert_eq!(
        error_for(&document(
            "codex-structured",
            &structured.replace("-o {{OUTPUT}}", "")
        )),
        ReviewError::ParentGrammar,
        "codex_output_schema_half_pair_rejected"
    );
    assert_eq!(
        error_for(&document("codex-structured", &structured.replace(pair, ""))),
        ReviewError::ParentGrammar,
        "codex_structured_without_pair_rejected"
    );

    let codex_logged = "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write --env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} --log-file {{LOG_FILE}} --node {{NODE}} --role {{ROLE}} --ref {{REF}} --evidence {{EVIDENCE}} --required-artifact {{OUTPUT}} --dry-run -- {{CALLER_ARG}}";
    let gate_logged = "pce dispatch gate --cwd {{CWD}} --env {{PATH_ENV}} --env {{HOME_ENV}} --env {{USER_ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} --log-file {{LOG_FILE}} --node {{NODE}} --role {{ROLE}} --ref {{REF}} --evidence {{EVIDENCE}} --required-artifact {{OUTPUT}} --dry-run -- {{CALLER_ARG}}";
    for (name, kind, command) in [
        (
            "codex_order_dry_run_before_delimiter",
            "codex-structured",
            codex_logged.replace("--dry-run --", "-- --dry-run"),
        ),
        (
            "gate_order_dry_run_before_delimiter",
            "gate-structured",
            gate_logged.replace("--dry-run --", "-- --dry-run"),
        ),
    ] {
        assert_eq!(
            error_for(&document(kind, &command)),
            ReviewError::ParentGrammar,
            "{name}"
        );
    }
}

#[test]
fn parser_visible_required_and_environment_cases() {
    let f = fixture();
    let cwd = f.bindings.cwd.as_os_str().to_owned();
    let schema = f.bindings.schema.as_os_str().to_owned();
    let output = f.bindings.output.as_os_str().to_owned();
    let d = "dispatch arguments require the `--` delimiter";
    assert_parser_red(
        "target_unknown_rejects",
        vec!["dispatch".into(), "other".into()],
        "unsupported dispatch target",
    );
    assert_parser_red(
        "codex_missing_cwd",
        vec!["dispatch".into(), "codex".into()],
        "expected `--cwd`",
    );
    assert_parser_red(
        "gate_missing_cwd",
        vec!["dispatch".into(), "gate".into()],
        "expected `--cwd`",
    );
    assert_parser_red(
        "codex_missing_sandbox",
        vec![
            "dispatch".into(),
            "codex".into(),
            "--cwd".into(),
            cwd.clone(),
        ],
        "expected `--sandbox`",
    );
    assert_parser_red(
        "gate_missing_output_schema",
        vec![
            "dispatch".into(),
            "gate".into(),
            "--cwd".into(),
            cwd.clone(),
        ],
        "expected `--output-schema`",
    );
    assert_parser_red(
        "gate_missing_output",
        vec![
            "dispatch".into(),
            "gate".into(),
            "--cwd".into(),
            cwd.clone(),
            "--output-schema".into(),
            schema.clone(),
        ],
        "expected `-o`",
    );
    assert_parser_red(
        "codex_unsupported_sandbox",
        vec![
            "dispatch".into(),
            "codex".into(),
            "--cwd".into(),
            cwd.clone(),
            "--sandbox".into(),
            "read-only".into(),
        ],
        "unsupported sandbox",
    );
    for (name, target, extra, diagnostic) in [
        (
            "codex_env_without_equals",
            "codex",
            vec!["--sandbox", "workspace-write", "--env", "BAD"],
            "environment entry must contain",
        ),
        (
            "gate_env_without_equals",
            "gate",
            vec!["--env", "BAD"],
            "environment entry must contain",
        ),
        (
            "codex_env_empty_name",
            "codex",
            vec!["--sandbox", "workspace-write", "--env", "=x"],
            "environment name must not be empty",
        ),
        (
            "gate_env_empty_name",
            "gate",
            vec!["--env", "=x"],
            "environment name must not be empty",
        ),
        (
            "codex_env_duplicate_name",
            "codex",
            vec![
                "--sandbox",
                "workspace-write",
                "--env",
                "A=1",
                "--env",
                "A=2",
            ],
            "duplicate environment name",
        ),
        (
            "gate_env_duplicate_name",
            "gate",
            vec!["--env", "A=1", "--env", "A=2"],
            "duplicate environment name",
        ),
        (
            "gate_env_anthropic_api_key",
            "gate",
            vec!["--env", "ANTHROPIC_API_KEY=x"],
            "must not contain `ANTHROPIC_API_KEY`",
        ),
    ] {
        let mut args: Vec<OsString> = vec![
            "dispatch".into(),
            target.into(),
            "--cwd".into(),
            cwd.clone(),
        ];
        args.extend(extra.into_iter().map(OsString::from));
        assert_parser_red(name, args, diagnostic);
    }
    assert_parser_red(
        "codex_missing_delimiter",
        vec![
            "dispatch".into(),
            "codex".into(),
            "--cwd".into(),
            cwd.clone(),
            "--sandbox".into(),
            "workspace-write".into(),
        ],
        d,
    );
    assert_parser_red(
        "gate_missing_delimiter",
        vec![
            "dispatch".into(),
            "gate".into(),
            "--cwd".into(),
            cwd,
            "--output-schema".into(),
            schema,
            "-o".into(),
            output,
        ],
        d,
    );

    assert_parser_red(
        "codex_structured_missing_output",
        vec![
            "dispatch".into(),
            "codex".into(),
            "--cwd".into(),
            f.bindings.cwd.as_os_str().to_owned(),
            "--sandbox".into(),
            "workspace-write".into(),
            "--output-schema".into(),
            f.bindings.schema.as_os_str().to_owned(),
        ],
        "expected `-o`",
    );
    assert_parser_red(
        "codex_structured_missing_output_schema",
        vec![
            "dispatch".into(),
            "codex".into(),
            "--cwd".into(),
            f.bindings.cwd.as_os_str().to_owned(),
            "--sandbox".into(),
            "workspace-write".into(),
            "-o".into(),
            f.bindings.output.as_os_str().to_owned(),
            "--".into(),
        ],
        d,
    );
}

#[test]
fn parser_trailing_value_matrix() {
    let f = fixture();
    for target in ["codex", "gate"] {
        let complete = complete_args(&f, target);
        let flags: &[&str] = if target == "codex" {
            &[
                "--cwd",
                "--sandbox",
                "--env",
                "--output-schema",
                "-o",
                "--plan-file",
                "--log-file",
                "--node",
                "--role",
                "--ref",
                "--evidence",
                "--required-artifact",
            ]
        } else {
            &[
                "--cwd",
                "--env",
                "--output-schema",
                "-o",
                "--plan-file",
                "--log-file",
                "--node",
                "--role",
                "--ref",
                "--evidence",
                "--required-artifact",
            ]
        };
        for flag in flags {
            let index = complete.iter().position(|arg| arg == flag).expect("flag");
            let args = complete[..=index].to_vec();
            let diagnostic = if matches!(
                *flag,
                "--log-file" | "--node" | "--role" | "--ref" | "--evidence" | "--required-artifact"
            ) {
                "dispatch logging options must be supplied together"
            } else {
                "missing value for"
            };
            let suffix = if *flag == "-o" {
                "output".to_owned()
            } else {
                flag.trim_start_matches("--").replace('-', "_")
            };
            assert_parser_red(&format!("{target}_trailing_{suffix}"), args, diagnostic);
        }
    }
}

#[test]
fn parser_order_logging_paths_and_positive_matrix() {
    let f = fixture();
    for target in ["codex", "gate"] {
        assert_parser_green(
            if target == "codex" {
                "target_codex_accepts"
            } else {
                "target_gate_accepts"
            },
            complete_args(&f, target),
        );
        assert_parser_green(
            if target == "codex" {
                "codex_dry_run_after_logging_accepts"
            } else {
                "gate_dry_run_after_logging_accepts"
            },
            complete_args(&f, target),
        );
        assert_parser_green(
            if target == "codex" {
                "codex_logging_complete"
            } else {
                "gate_logging_complete"
            },
            complete_args(&f, target),
        );
    }

    let delimiter = "dispatch arguments require the `--` delimiter";
    let logging = "dispatch logging options must be supplied together";
    for target in ["codex", "gate"] {
        let prefix = if target == "codex" { "codex" } else { "gate" };
        let original = complete_args(&f, target);
        let groups: Vec<(&str, usize, usize)> = if target == "codex" {
            vec![
                ("cwd_before_sandbox", 2, 4),
                ("sandbox_before_env", 4, 6),
                ("env_before_structured", 6, 8),
                ("structured_before_plan", 8, 12),
                ("plan_before_logging", 12, 14),
                ("logging_before_dry_run", 14, 26),
            ]
        } else {
            vec![
                ("cwd_before_env", 2, 4),
                ("env_before_structured", 4, 6),
                ("structured_before_plan", 6, 10),
                ("plan_before_logging", 10, 12),
                ("logging_before_dry_run", 12, 24),
            ]
        };
        for (suffix, left, right) in groups {
            let left_len = match suffix {
                "cwd_before_sandbox"
                | "cwd_before_env"
                | "sandbox_before_env"
                | "env_before_structured" => 2,
                "structured_before_plan" => 4,
                "plan_before_logging" => 2,
                "logging_before_dry_run" => 12,
                _ => unreachable!(),
            };
            let right_len = match suffix {
                "structured_before_plan" => 2,
                "plan_before_logging" => 12,
                "logging_before_dry_run" => 1,
                _ => {
                    if suffix.contains("env_before_structured") {
                        4
                    } else {
                        2
                    }
                }
            };
            let mut swapped = original.clone();
            let left_group = swapped[left..left + left_len].to_vec();
            let right_group = swapped[right..right + right_len].to_vec();
            swapped.splice(
                left..right + right_len,
                right_group.into_iter().chain(left_group),
            );
            let diagnostic = match (target, suffix) {
                ("gate", "structured_before_plan") => "expected `--output-schema`",
                (_, "logging_before_dry_run") => logging,
                (_, "cwd_before_sandbox") | (_, "cwd_before_env") => "expected `--cwd`",
                (_, "sandbox_before_env") => "expected `--sandbox`",
                _ => delimiter,
            };
            assert_parser_red(&format!("{prefix}_order_{suffix}"), swapped, diagnostic);
        }

        let base = if target == "codex" { 14 } else { 12 };
        let fields = [
            "--log-file",
            "--node",
            "--role",
            "--ref",
            "--evidence",
            "--required-artifact",
        ];
        for supplied in 1..6 {
            let mut args = original[..base + supplied * 2].to_vec();
            args.extend(["--".into(), "tail".into()]);
            let suffix = [
                "log_file_only",
                "through_node",
                "through_role",
                "through_ref",
                "through_evidence",
            ][supplied - 1];
            assert_parser_red(&format!("{prefix}_logging_prefix_{suffix}"), args, logging);
        }
        for field in &fields[1..] {
            let offset = fields
                .iter()
                .position(|candidate| candidate == field)
                .expect("field");
            let mut args = original[..base].to_vec();
            args.extend(
                original[base + offset * 2..base + offset * 2 + 2]
                    .iter()
                    .cloned(),
            );
            args.extend(["--".into(), "tail".into()]);
            assert_parser_red(
                &format!(
                    "{prefix}_logging_starts_{}",
                    field.trim_start_matches("--").replace('-', "_")
                ),
                args,
                logging,
            );
        }
        let mut without_logging = original[..base].to_vec();
        without_logging.extend(["--dry-run".into(), "--".into(), "tail".into()]);
        assert_parser_red(
            &format!("{prefix}_dry_run_without_logging"),
            without_logging,
            logging,
        );
    }

    for (name, target, value, diagnostic) in [
        ("codex_relative_cwd", "codex", "cwd", "absolute"),
        ("gate_relative_cwd", "gate", "cwd", "absolute"),
        (
            "codex_relative_output_schema",
            "codex",
            "schema",
            "absolute",
        ),
        ("gate_relative_output_schema", "gate", "schema", "absolute"),
        ("codex_relative_output", "codex", "output", "absolute"),
        ("gate_relative_output", "gate", "output", "absolute"),
    ] {
        let mut args = complete_args(&f, target);
        let flag = match value {
            "cwd" => "--cwd",
            "schema" => "--output-schema",
            _ => "-o",
        };
        let index = args.iter().position(|arg| arg == flag).expect("path flag");
        args[index + 1] = "relative".into();
        assert_parser_red(name, args, diagnostic);
    }
    for (name, target) in [
        ("codex_unreadable_plan_file", "codex"),
        ("gate_unreadable_plan_file", "gate"),
    ] {
        let mut args = complete_args(&f, target);
        let index = args
            .iter()
            .position(|arg| arg == "--plan-file")
            .expect("plan");
        args[index + 1] = f.bindings.cwd.join("missing-plan").into_os_string();
        assert_parser_red(name, args, "failed to read plan file");
    }

    let mut codex_key = complete_args(&f, "codex");
    let env = codex_key
        .iter()
        .position(|arg| arg == "--env")
        .expect("env");
    codex_key.splice(env..env + 2, ["--env".into(), "ANTHROPIC_API_KEY=x".into()]);
    assert_parser_green("codex_env_anthropic_api_key_accepts", codex_key);

    for (name, form) in [
        (
            "gate_output_format_separate_rejected",
            vec!["--output-format", "json"],
        ),
        (
            "gate_output_format_equals_rejected",
            vec!["--output-format=json"],
        ),
    ] {
        let mut args = complete_args(&f, "gate");
        let delimiter_index = args.iter().position(|arg| arg == "--").expect("delimiter");
        args.splice(delimiter_index + 1.., form.into_iter().map(OsString::from));
        assert_parser_red(name, args, "must not contain `--output-format`");
    }
}

#[test]
fn parser_empty_value_matrix() {
    let f = fixture();
    for (name, target, flag) in [
        ("codex_empty_cwd", "codex", "--cwd"),
        ("codex_empty_sandbox", "codex", "--sandbox"),
        ("codex_empty_env", "codex", "--env"),
        ("codex_empty_output_schema", "codex", "--output-schema"),
        ("codex_empty_output", "codex", "-o"),
        ("codex_empty_plan_file", "codex", "--plan-file"),
        ("codex_empty_log_file", "codex", "--log-file"),
        ("codex_empty_node", "codex", "--node"),
        ("codex_empty_role", "codex", "--role"),
        ("codex_empty_ref", "codex", "--ref"),
        ("codex_empty_evidence", "codex", "--evidence"),
        (
            "codex_empty_required_artifact",
            "codex",
            "--required-artifact",
        ),
        ("gate_empty_cwd", "gate", "--cwd"),
        ("gate_empty_env", "gate", "--env"),
        ("gate_empty_output_schema", "gate", "--output-schema"),
        ("gate_empty_output", "gate", "-o"),
        ("gate_empty_plan_file", "gate", "--plan-file"),
        ("gate_empty_log_file", "gate", "--log-file"),
        ("gate_empty_node", "gate", "--node"),
        ("gate_empty_role", "gate", "--role"),
        ("gate_empty_ref", "gate", "--ref"),
        ("gate_empty_evidence", "gate", "--evidence"),
        (
            "gate_empty_required_artifact",
            "gate",
            "--required-artifact",
        ),
    ] {
        let mut args = complete_args(&f, target);
        let index = args
            .iter()
            .position(|item| item == flag)
            .expect("empty flag");
        args[index + 1] = OsString::new();
        let diagnostic = if flag == "--sandbox" {
            "empty value for `--sandbox`"
        } else if matches!(
            flag,
            "--log-file" | "--node" | "--role" | "--ref" | "--evidence" | "--required-artifact"
        ) {
            "dispatch logging options must be supplied together"
        } else {
            "empty value for"
        };
        assert_parser_red(name, args, diagnostic);
    }
}

fn direct_args(
    f: &Fixture,
    target: &str,
    plan: Option<&Path>,
    artifact_mode: Option<&str>,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "dispatch".into(),
        target.into(),
        "--cwd".into(),
        f.bindings.cwd.as_os_str().to_owned(),
    ];
    if target == "codex" {
        args.extend(["--sandbox".into(), "workspace-write".into()]);
    }
    args.extend([
        "--env".into(),
        format!("PATH={}", f.bindings.shim_dir.display()).into(),
    ]);
    if let Some(mode) = artifact_mode {
        args.extend([
            "--env".into(),
            format!("ARTIFACT_MODE={mode}").into(),
            "--env".into(),
            format!("ARTIFACT={}", f.bindings.output.display()).into(),
        ]);
    }
    if target == "gate" || artifact_mode.is_some() {
        args.extend([
            "--output-schema".into(),
            f.bindings.schema.as_os_str().to_owned(),
            "-o".into(),
            f.bindings.output.as_os_str().to_owned(),
        ]);
    }
    if let Some(path) = plan {
        args.extend(["--plan-file".into(), path.as_os_str().to_owned()]);
    }
    args.push("--".into());
    args.push("fixture-tail".into());
    args
}

#[test]
fn spawn_path_stdin_environment_shape_and_artifact_cases() {
    for target in ["codex", "gate"] {
        let f = fixture();
        let output = run_pce(
            &direct_args(
                &f,
                target,
                None,
                if target == "gate" {
                    Some("valid")
                } else {
                    None
                },
            ),
            Some(b"parent sentinel"),
        );
        assert!(
            output.status.success(),
            "{target}_null_stdin_spawn: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read(f.bindings.cwd.join(".review/stdin")).expect("recorded stdin"),
            b"",
            "{target}_null_stdin_spawn"
        );
        let argv = read_nul(&f.bindings.cwd.join(".review/argv"));
        if target == "codex" {
            assert_eq!(
                &argv[..6],
                [
                    b"exec".to_vec(),
                    b"--json".to_vec(),
                    b"-C".to_vec(),
                    f.bindings.cwd.as_os_str().as_encoded_bytes().to_vec(),
                    b"--sandbox".to_vec(),
                    b"workspace-write".to_vec()
                ],
                "codex_unstructured_spawn_shape"
            );
            assert!(
                !argv
                    .iter()
                    .any(|arg| arg == b"--output-schema" || arg == b"-o"),
                "codex_unstructured_spawn_shape"
            );
        } else {
            assert_eq!(
                &argv[..3],
                [
                    b"-p".to_vec(),
                    b"--output-format".to_vec(),
                    b"json".to_vec()
                ],
                "gate_spawn_shape"
            );
            assert_eq!(
                argv.iter().filter(|arg| arg.as_slice() == b"-p").count(),
                1,
                "gate_spawn_shape"
            );
            assert_eq!(
                argv.iter()
                    .filter(|arg| arg.as_slice() == b"--output-format")
                    .count(),
                1,
                "gate_spawn_shape"
            );
        }
        let env =
            fs::read_to_string(f.bindings.cwd.join(".review/env")).expect("environment record");
        assert!(
            env.contains(&format!("PATH={}", f.bindings.shim_dir.display())),
            "{target}_constructed_environment_spawn"
        );
        assert!(
            !env.contains("PCE_INHERITED_SENTINEL"),
            "{target}_constructed_environment_spawn"
        );

        let f = fixture();
        let expected = fs::read(&f.bindings.plan_file).expect("plan measurement");
        let output = run_pce(
            &direct_args(
                &f,
                target,
                Some(&f.bindings.plan_file),
                if target == "gate" {
                    Some("valid")
                } else {
                    None
                },
            ),
            None,
        );
        assert!(
            output.status.success(),
            "{target}_plan_file_exact_stdin_spawn: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read(f.bindings.cwd.join(".review/stdin")).expect("stdin record"),
            expected,
            "{target}_plan_file_exact_stdin_spawn"
        );
    }

    for target in ["codex", "gate"] {
        for (name, mode, success, diagnostic) in [
            (
                "structured_missing_artifact_completion",
                None,
                false,
                "is missing",
            ),
            (
                "structured_malformed_artifact_completion",
                Some("malformed"),
                false,
                "not complete valid JSON",
            ),
            (
                "structured_valid_artifact_completion",
                Some("valid"),
                true,
                "",
            ),
        ] {
            let f = fixture();
            let output = run_pce(&direct_args(&f, target, None, mode.or(Some(""))), None);
            assert_eq!(
                output.status.success(),
                success,
                "{name}/{target}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if !success {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains(diagnostic),
                    "{name}/{target}"
                );
            }
            if name == "structured_valid_artifact_completion" && target == "codex" {
                let argv = read_nul(&f.bindings.cwd.join(".review/argv"));
                let schema = argv
                    .iter()
                    .position(|arg| arg == b"--output-schema")
                    .expect("codex_structured_spawn_shape");
                assert_eq!(argv[schema + 2], b"-o", "codex_structured_spawn_shape");
                assert_eq!(
                    argv.iter()
                        .filter(|arg| arg.as_slice() == b"--output-schema")
                        .count(),
                    1,
                    "codex_structured_spawn_shape"
                );
                assert_eq!(
                    argv.iter().filter(|arg| arg.as_slice() == b"-o").count(),
                    1,
                    "codex_structured_spawn_shape"
                );
            }
        }
    }
}

#[test]
fn child_nonzero_completion() {
    for target in ["codex", "gate"] {
        let f = fixture();
        let mut args = direct_args(
            &f,
            target,
            None,
            if target == "gate" {
                Some("valid")
            } else {
                None
            },
        );
        let structured = args
            .iter()
            .position(|arg| arg == "--output-schema")
            .unwrap_or_else(|| args.iter().position(|arg| arg == "--").expect("delimiter"));
        let mut controls = vec!["--env".into(), "EXIT_CODE=42".into()];
        if target == "codex" {
            controls.extend(["--env".into(), "SILENT_STDOUT=yes".into()]);
        }
        args.splice(structured..structured, controls);
        let delimiter = args.iter().position(|arg| arg == "--").expect("delimiter");
        args.splice(
            delimiter..delimiter,
            [
                "--log-file".into(),
                f.bindings.log_file.as_os_str().to_owned(),
                "--node".into(),
                "m7-s1".into(),
                "--role".into(),
                "step-plan-writer".into(),
                "--ref".into(),
                "07b85ccd".into(),
                "--evidence".into(),
                "child-nonzero".into(),
                "--required-artifact".into(),
                f.bindings.output.as_os_str().to_owned(),
            ],
        );
        let output = run_pce(&args, None);
        assert!(output.status.success(), "child_nonzero_completion/{target}");
        assert_eq!(output.stdout, b"", "child_nonzero_completion/{target}");
        assert_eq!(output.stderr, b"", "child_nonzero_completion/{target}");
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let settled = fs::read_to_string(&f.bindings.log_file)
                .ok()
                .is_some_and(|contents| {
                    contents.lines().count() == 2
                        && contents
                            .lines()
                            .all(|line| serde_json::from_str::<serde_json::Value>(line).is_ok())
                });
            if settled {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "lifecycle did not settle for {target}"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let completion =
            fs::read_to_string(f.bindings.cwd.join(".review/completion")).expect("shim completion");
        assert_eq!(completion, "42", "child_nonzero_completion/{target}");
        let records = fs::read_to_string(&f.bindings.log_file).expect("event log");
        let completion_record: serde_json::Value =
            serde_json::from_str(records.lines().last().expect("completion event"))
                .expect("completion JSON");
        assert_eq!(
            completion_record["kind"], "dispatch-completion",
            "child_nonzero_completion/{target}"
        );
        assert_eq!(
            completion_record["payload"]["exit_status"]["code"], 42,
            "child_nonzero_completion/{target}"
        );
        let stderr_path = PathBuf::from(format!(
            "{}.dispatch-000001.stderr",
            f.bindings.log_file.display()
        ));
        let expected = if target == "codex" {
            "`codex` child exited with status exit status: 42"
        } else {
            "claude-exit-envelope-contradiction"
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        let stderr = loop {
            let bytes = fs::read(&stderr_path).expect("continuation stderr sidecar");
            if String::from_utf8_lossy(&bytes).contains(expected) {
                break bytes;
            }
            assert!(
                Instant::now() < deadline,
                "continuation diagnostic did not settle"
            );
            thread::sleep(Duration::from_millis(10));
        };
        let stderr = String::from_utf8_lossy(&stderr);
        if target == "codex" {
            assert!(
                stderr.contains("`codex` child exited with status exit status: 42"),
                "child_nonzero_completion/codex: {stderr}"
            );
        } else {
            assert!(
                stderr.contains("claude-exit-envelope-contradiction"),
                "child_nonzero_completion/gate: {stderr}"
            );
        }
        if target == "codex" {
            assert_eq!(
                fs::read(PathBuf::from(format!(
                    "{}.dispatch-000001.stdout",
                    f.bindings.log_file.display()
                )))
                .expect("Codex stdout sidecar"),
                b""
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ManifestEntry {
    path: PathBuf,
    kind: &'static str,
    mode: u32,
    bytes: Vec<u8>,
    target: Option<PathBuf>,
}

fn manifest(root: &Path) -> Vec<ManifestEntry> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<ManifestEntry>) {
        let mut children: Vec<_> = fs::read_dir(path)
            .expect("manifest directory")
            .map(|entry| entry.expect("entry").path())
            .collect();
        children.sort();
        for child in children {
            let metadata = fs::symlink_metadata(&child).expect("manifest metadata");
            let kind = if metadata.file_type().is_symlink() {
                "symlink"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "file"
            };
            entries.push(ManifestEntry {
                path: child.strip_prefix(root).expect("relative").to_owned(),
                kind,
                mode: metadata.permissions().mode(),
                bytes: if metadata.is_file() {
                    fs::read(&child).expect("manifest file")
                } else {
                    Vec::new()
                },
                target: if metadata.file_type().is_symlink() {
                    Some(fs::read_link(&child).expect("link"))
                } else {
                    None
                },
            });
            if metadata.is_dir() {
                visit(root, &child, entries);
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}

#[test]
fn review_child_with_stand_in_home() {
    let Some(home) = std::env::var_os("PCE_REVIEW_CHILD_HOME") else {
        return;
    };
    let expected = std::env::var_os("PCE_REVIEW_CHILD_TARGET").expect("child target");
    let link = PathBuf::from(home).join(".claude/skills/pce");
    assert_eq!(
        fs::read_link(link).expect("installed link"),
        PathBuf::from(expected)
    );
    let f = fixture();
    let markdown = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("skill");
    review_document(&markdown, &f.bindings).expect("child real review");
    std::io::stdout()
        .write_all(b"PCE_REVIEW_CHILD_RAN\n")
        .expect("child sentinel");
}

#[test]
fn review_preserves_installed_skill_symlink_target() {
    if std::env::var_os("PCE_REVIEW_CHILD_HOME").is_some() {
        return;
    }
    let home = tempfile::tempdir_in(std::env::temp_dir()).expect("stand-in home");
    let sentinel = tempfile::tempdir_in(std::env::temp_dir()).expect("sentinel root");
    fs::create_dir_all(sentinel.path().join("schemas")).expect("schemas");
    fs::create_dir_all(sentinel.path().join("nested/empty")).expect("empty witness");
    fs::write(sentinel.path().join("SKILL.md"), b"sentinel skill").expect("skill witness");
    for name in [
        "verdict.schema.json",
        "graph.schema.json",
        "run-snapshot.schema.json",
    ] {
        fs::write(sentinel.path().join("schemas").join(name), b"{}").expect("schema witness");
    }
    fs::write(sentinel.path().join("nested/file"), b"nested").expect("nested witness");
    symlink("missing-target", sentinel.path().join("dangling")).expect("dangling witness");
    let installed_parent = home.path().join(".claude/skills");
    fs::create_dir_all(&installed_parent).expect("installed parent");
    let installed = installed_parent.join("pce");
    symlink(sentinel.path(), &installed).expect("installed link");
    assert!(
        sentinel.path().join("SKILL.md").is_file(),
        "known-witness precondition"
    );
    let before_manifest = manifest(sentinel.path());
    let before_target = fs::read_link(&installed).expect("before link");
    let output = Command::new(std::env::current_exe().expect("current test executable"))
        .arg("review_child_with_stand_in_home")
        .arg("--exact")
        .arg("--nocapture")
        .env("HOME", home.path())
        .env("PCE_REVIEW_CHILD_HOME", home.path())
        .env("PCE_REVIEW_CHILD_TARGET", sentinel.path())
        .output()
        .expect("child review");
    assert!(
        output.status.success(),
        "child stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let child_stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        child_stdout.matches("PCE_REVIEW_CHILD_RAN").count(),
        1,
        "child-run sentinel: {child_stdout}"
    );
    assert!(
        child_stdout.contains("1 passed"),
        "child test measurement: {child_stdout}"
    );
    let after_manifest = manifest(sentinel.path());
    let after_target = fs::read_link(&installed).expect("after link");
    assert_eq!(before_manifest, after_manifest);
    assert_eq!(before_target, after_target);
    assert!(
        installed
            .symlink_metadata()
            .expect("installed state")
            .file_type()
            .is_symlink()
    );
}

const BINARY_OWNED_ORCHESTRATION_PARAGRAPH: &str = "`pce dispatch` appends issuance and returns while the binary-owned continuation retains the child without a runtime or spend ceiling and appends exactly one observed completion when the child exits. The orchestrator observes every issuance with bounded `pce dispatch check-in`, which reports unknown, running, finished, or dead state and required-artifact production without signaling or killing a child, appending a completion, reconciling, or closing anything. Only `pce dispatch reconcile` durably closes a dead incomplete issuance, and it refuses wherever a real completion could still land. A missing product alone neither closes a dispatch nor licenses a completion or accounting claim; all-accounted requires the issuance-ordered unaccounted ledger to be empty. Never append a `delta` or invent a result to close a dispatch.";

const COLD_RULES_HEADING: &str = "## Cold-orchestrator falsification rules";

const COLD_RULES: [&str; 11] = [
    "1. State every remedy as an addition: preserve and never remove the constraint being refined.",
    "2. Falsify a measured quantity by comparing the before and after measurements; an expected literal is not a measurement.",
    "3. After every step merge or milestone merge, refresh the local integration ref from the remote before branching from it.",
    "4. After every post-PR commit, republish `pr-body.md`; measure the repository setting with `gh api repos/{owner}/{repo} --jq .squash_merge_commit_message` and never infer it from `merge_method`.",
    "5. For every issued dispatch, use bounded `pce dispatch check-in` for read-only observation; if an incomplete issuance is dead, close it only with `pce dispatch reconcile`. A missing required product alone neither closes nor accounts for the dispatch; never append a `delta` or invent a result.",
    "6. Enumerate fixtures for every negation, alternative, exception, and ordering branch stated in a specification.",
    "7. Treat any harness option, fixture switch, shim behavior, or injected capability not driven through its production path as a receipt for a missing falsifier: add that falsifier or remove the unused control.",
    "8. Pair every `compile_fail` doctest with a positive twin whose imports and bindings are byte-identical.",
    "9. Prove grep and every other inspection audit against a known match so that zero matches cannot read as clean without a demonstrated detector.",
    "10. Describe a prior fix only from landed code verified at the cited ref. A figure supplied as context is never a measurement; when a prompt states an expected value, report it alongside the independently measured value and compare them.",
    "11. Sweep every remedy as a new claim under the same falsification standard as the repaired text.",
];

const PURPOSE_IDENTITIES: [&str; 2] = [
    "purpose/codex-commit-completion",
    "purpose/codex-diagnostics",
];

const BOUNDED_EVIDENCE_CLAUSE: &str = "names one bounded evidence question and its permitted reads";
const CONFINED_SCOPE_CLAUSE: &str =
    "confines work to that evidence request and the single caller-tail token";
const NO_MUTATION_CLAUSE: &str = "neither implements a remedy nor mutates unrelated work";
const BOUNDEDNESS_DISCLOSURE_CLAUSES: [(&str, &str); 3] = [
    (
        "documentary-caller-tail",
        "documentary and caller-tail validated",
    ),
    ("not-enforced", "not environmentally enforced"),
    ("cause", "child retains workspace-write capability"),
];

fn real_skill_markdown() -> String {
    fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("repository skill")
}

const PURPOSE_ROUTE_LIFETIME: &str = "After `pce dispatch` appends issuance, the invocation returns; the binary-owned continuation retains the purpose child with no runtime or spend ceiling, and the orchestrator observes the issuance with `pce dispatch check-in` like any other issuance until the continuation appends the existing observed completion or `pce dispatch reconcile` durably closes a dead incomplete issuance.";
const COMMON_ROLE_ROUTE_LIFETIME: &str = "For every logged purpose or role route, `pce dispatch` returns after issuance while the binary-owned continuation retains the child without a runtime or spend ceiling and appends exactly one observed completion when the child exits.";
const MANUAL_DISPATCH_LIFECYCLE_BLOCK: &str = "pce dispatch check-in --file <ABSOLUTE_LOG_PATH>\npce dispatch reconcile --file <ABSOLUTE_LOG_PATH> --issuance <ISSUANCE_SEQUENCE> --node <NODE>";

const AUTHORED_REPLACEMENTS: [(&str, &str); 14] = [
    ("Both children are awaited.", PURPOSE_ROUTE_LIFETIME),
    (
        "These binary-owned issuance records are the sole source for round counts and dispatch refs.",
        "These binary-owned issuance records are the sole source for dispatch refs and for the exact `(node, role)` issuance ordinal; validated-production completion facts alone derive validated-production spends, while completion and required-artifact facts derive consecutive non-production.",
    ),
    (
        "Round series are keyed by `(node, role)`, so Phase 1 `(m1-s1, milestone-planner)` cannot collide with Phase 3 `(m1-s1, step-plan-writer)`.",
        "Issuance-ordinal and validated-production spending series are keyed by `(node, role)`, while consecutive non-production is keyed by `(node, role, required artifact)`, so Phase 1 `(m1-s1, milestone-planner)` cannot collide with Phase 3 `(m1-s1, step-plan-writer)`.",
    ),
    (
        "3. Iterate cold planner and critic to `APPROVE`, cap 3 with stuck detection. Derive rounds from dispatch records and blocker history from review artifacts. Every revision is a fresh invocation naming the artifact and `review-<n>.md`; never resume a prior Codex session. On approval, digest the approved `milestones.json` bytes and append:",
        "3. Iterate cold planner and critic to `APPROVE`, subject to an automatic limit of 12 validated-production spends per exact `(node, role)`, with stuck detection. Productless attempts charge no validated-production spend and are governed by the separate consecutive-non-production hold. Derive blocker history from review artifacts, whose `review-<n>.md` index is the exact `(node, role)` issuance ordinal and advances for every issuance. Every revision is a fresh invocation; never resume a prior Codex session. On approval, digest the approved `milestones.json` bytes and append:",
    ),
    (
        "Iterate cold invocations to approval, cap 3 with stuck detection; review artifacts supply blocker history. On approval, digest that milestone's `steps.json` bytes and append:",
        "Iterate cold invocations to approval, subject to an automatic limit of 12 validated-production spends per exact `(node, role)`, with stuck detection; productless attempts charge no validated-production spend and are governed by the separate consecutive-non-production hold, while review artifacts indexed by the issuance ordinal supply blocker history. On approval, digest that milestone's `steps.json` bytes and append:",
    ),
    (
        "   Iterate fresh invocations to `APPROVE`, cap 3 with review-artifact stuck detection. At approval, digest the approved bytes and append:",
        "   Iterate fresh invocations to `APPROVE`, subject to an automatic limit of 12 validated-production spends per exact `(node, role)`, with review-artifact stuck detection; productless attempts charge no validated-production spend and are governed by the separate consecutive-non-production hold. At approval, digest the approved bytes and append:",
    ),
    (
        "Use the next exact `(node, falsification-critic)` dispatch count and the existing review-artifact naming rule for the verdict; add no counter or artifact family.",
        "Use the next exact `(node, falsification-critic)` issuance ordinal only for the existing review-artifact naming rule; validated-production spending accounting and consecutive-non-production admission remain separate derivations, and no new counter or artifact family is added.",
    ),
    (
        "   Derive the falsification round solely from prior `(node, falsification-critic)` dispatch records. The first falsification is round `1`; at most `3` falsification dispatches may occur for the step. Continue only while the next checked dispatch count is at most `3`. If two consecutive schema-valid verdict artifacts have substantially identical blocking issue sets under the existing comparison rule, append/open the existing escalation and stop before another executor dispatch, push, or PR. If dispatch count `3` is blocking, escalate and stop before another executor dispatch, push, or PR. Successive gate verdicts remain `review-<n>.json`/`review-<n>.md`, indexed by prior `(node, falsification-critic)` dispatch records; add no counter or artifact family.",
        "   Derive the falsification review-artifact issuance ordinal from every prior exact `(node, falsification-critic)` issuance. Ordinal `1` names the first `review-1.json`/`review-1.md`, and every issuance advances that ordinal even when it produces nothing. Separately, the validated-production spending limit for `(node, falsification-critic)` is `12` validated-production spends; a productless attempt charges no validated-production spend and remains governed by the `(node, role, required artifact)` consecutive-non-production hold. Continue only while the derived validated-production spend count is below `12` and no non-production hold prevents admission. If two consecutive schema-valid verdict artifacts have substantially identical blocking issue sets under the existing comparison rule, append/open the existing escalation and stop before another executor dispatch, push, or PR. If validated-production spend `12` is blocking, escalate and stop before another executor dispatch, push, or PR. Successive gate verdicts remain `review-<n>.json`/`review-<n>.md`, indexed only by the exact `(node, falsification-critic)` issuance ordinal; add no counter or artifact family.",
    ),
    (
        "Cap 3 with review-artifact stuck detection.",
        "Limit PR/fix work to 12 validated-production spends per exact `(node, role)`, with review-artifact stuck detection; productless attempts charge no validated-production spend and remain subject to the separate consecutive-non-production hold.",
    ),
    (
        "Immediately before each verdict-producing gate issuance, read the accepted source event log and count only prior `dispatch` records whose node and role byte-match the route. Checked successor `n = prior_count + 1` names both distinct review artifacts in the already designated review directory: `review-<n>.json` is the absolute parent `-o` and gate-tail path, while `review-<n>.md` retains explanatory history. After the gate returns, read the exact JSON path and validate it again against the installed verdict schema. Missing, unreadable, malformed, schema-invalid, or unknown verdict data stops loudly. `APPROVE` proceeds, `REVISE` uses the phase-appropriate planning anchor for plan causes or `step-executor` for execution fixes at the exact current ref, and `BLOCK` escalates. The same prior `(node, role)` dispatch count determines the artifact index and round cap; no separate counter exists.",
        "Immediately before each verdict-producing gate issuance, read the accepted source event log and derive the exact `(node, role)` issuance ordinal by counting every prior `dispatch` record whose node and role byte-match the route. Checked successor `n = prior_issuance_ordinal + 1` names both distinct artifacts in the already designated review directory. For `pr-reviewer`, ordinal `n` names `pr-review-<n>.json` and `pr-review-<n>.md`; for every other verdict-producing gate, it names `review-<n>.json` and `review-<n>.md`. The JSON artifact is the absolute parent `-o` and gate-tail path, while the Markdown artifact retains explanatory history. The issuance ordinal advances for every issuance and determines artifact naming only. After the gate finishes, read the exact JSON path and validate it again against the installed verdict schema. Missing, unreadable, malformed, schema-invalid, or unknown verdict data stops loudly, but a missing product alone neither closes a dispatch nor licenses a completion or accounting claim. `APPROVE` proceeds, `REVISE` uses the phase-appropriate planning anchor for plan causes or `step-executor` for execution fixes at the exact current ref, and `BLOCK` escalates. Separately, validated-production spending limits count only validated-production spends keyed by `(node, role)`, so productless attempts are exempt. Consecutive non-production is keyed by `(node, role, required artifact)`; after two consecutive non-producing completions its typed hold is the separate stop, and the hold closes only through `retry`, `re-plan`, or `abandon`.",
    ),
    (
        "- Plan/critic and PR/fix caps are 3. Derive rounds from dispatch records. On two consecutive verdicts with substantially identical blocking issue sets, short-circuit the loop before the cap. Derive the comparison from review artifacts; do not update separate loop state.",
        "- Plan/critic and PR/fix automatic spending limits are 12 validated-production spends per exact `(node, role)`. Derive validated-production spends only from validated production; productless attempts are exempt and are stopped separately by the `(node, role, required artifact)` consecutive-non-production hold. On two consecutive verdicts with substantially identical blocking issue sets, short-circuit the loop before the spending limit. Derive the comparison from review artifacts; do not update separate loop state.",
    ),
    (
        "A detached dispatch uses the anchored `pce dispatch codex` route and must be paired with a registered wait. `pce dispatch` has no detached option and awaits its child. If a recorded dispatch has no required product, append a `delta` at the canonical node to reconcile that fact; never invent a result.",
        BINARY_OWNED_ORCHESTRATION_PARAGRAPH,
    ),
    (
        "5. For detached work, use the anchored `pce dispatch codex` route, pair the dispatch with a registered wait, and reconcile a missing required product with a `delta` at the canonical node instead of inventing a result.",
        COLD_RULES[4],
    ),
    (
        "- Unexpected merge conflict dispatches Codex to rebase and resolve from the named base and head refs, supplying `git diff <base>...<head>`; classify the dispatch as REVISE-class for cap accounting, and escalate if unresolved.",
        "- Unexpected merge conflict dispatches Codex to rebase and resolve from the named base and head refs, supplying `git diff <base>...<head>`; a validated produced REVISE result charges the applicable validated-production spending limit, while a productless attempt charges no validated-production spend and remains subject to the separate consecutive-non-production hold; escalate if unresolved.",
    ),
];

fn cold_rules_section(markdown: &str) -> String {
    let start = markdown
        .find(COLD_RULES_HEADING)
        .expect("cold_orchestrator_rules_are_exact/heading");
    let rest = &markdown[start + COLD_RULES_HEADING.len()..];
    let end = rest.find("\n## ").map_or(markdown.len(), |offset| {
        start + COLD_RULES_HEADING.len() + offset
    });
    markdown[start..end].to_owned()
}

fn purpose_route(kind: RouteKind) -> AnchoredRoute {
    repository_routes()
        .into_iter()
        .find(|route| route.kind == kind)
        .unwrap_or_else(|| panic!("missing purpose anchor {kind:?}"))
}

#[test]
fn pr_review_repair_route_and_artifacts_are_unambiguous() {
    let markdown = real_skill_markdown();
    for clause in [
        "Stage 6 always repairs the committed artifact through `step-executor` at the exact current PR head, regardless of `root_cause`.",
        "Stage 6 does not re-dispatch `step-plan-writer`; there, `root_cause` is diagnostic and cannot override the code-repair route.",
        "For `pr-reviewer`, ordinal `n` names `pr-review-<n>.json` and `pr-review-<n>.md`",
    ] {
        assert_eq!(markdown.matches(clause).count(), 1, "{clause}");
    }
    assert!(!markdown.contains("`step_plan` re-dispatches `step-plan-writer` cold"));
}

#[test]
fn integration_branches_are_remote_and_retained_run_authority() {
    let markdown = real_skill_markdown();
    for clause in [
        "push it to `origin` immediately when it is created and before the first step-readiness query",
        "Retain both the local and remote integration branch while any active or resumable PCE run refers to it",
        "Never remove either ref merely because one milestone has merged",
    ] {
        assert_eq!(markdown.matches(clause).count(), 1, "{clause}");
    }
}

#[test]
fn validated_production_spending_parks_and_resumes() {
    let markdown = real_skill_markdown();
    for clause in [
        "The automatic validated-production spending limit is 12 per exact `(node, role)`.",
        "Reaching it parks the series by opening the exact typed non-production hold",
        "A human-recorded typed `retry` close records the current issuance ordinal and grants exactly the next dispatch",
        "advancing the issuance ordinal consumes that one-use authority",
        "Generic free-text escalation closure grants nothing",
        "does not charge the reporting role's spending series",
    ] {
        assert_eq!(markdown.matches(clause).count(), 1, "{clause}");
    }
    assert!(!markdown.contains("validated-production defect round"));
}

#[test]
fn child_tmpdir_and_mutation_gate_authority_are_explicit() {
    let markdown = real_skill_markdown();
    for clause in [
        "fresh mode-`0700` directory under its fixed `/tmp` root",
        "overrides `TMPDIR` for that child after applying the caller's forwarded environment",
        "The operator's shell `TMPDIR` is ignored",
        "is not a fourth forwarded `--env` entry",
        "Contract-check Seatbelt execution permits and receives the same fresh `TMPDIR` lifecycle",
        "Mutation testing has no universal or Rust-specific workflow command",
        "runs only when the repository's tracked contract declares `stated.gates.mutation`",
    ] {
        assert_eq!(markdown.matches(clause).count(), 1, "{clause}");
    }
}

#[test]
fn codex_non_interactive_documents_detached_derived_lifecycle() {
    let markdown = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/codex-non-interactive.md"
    ))
    .expect("codex non-interactive documentation");
    for clause in [
        "returns after issuance while a binary-owned continuation retains the child",
        "No lifecycle tally or round counter is stored",
        "Validated-production completions alone charge the validated-production spending limit",
        "two consecutive non-producing completions for the same `(node, role, required artifact)` open the separate typed hold",
        "`pce dispatch check-in` observes without closing",
        "`pce dispatch reconcile` is the only way to close a dead incomplete issuance",
    ] {
        assert_eq!(markdown.matches(clause).count(), 1, "{clause}");
    }
    assert!(!markdown.contains("stdout is instead the byte-preserved live JSONL event stream"));
}

#[test]
fn anchor_class_is_total() {
    let routes = repository_routes();
    assert_classification_total(&routes);
    assert_eq!(routes.len(), EXPECTED_ANCHORED_ROUTE_COUNT);
}

#[test]
fn purpose_anchor_exact_set() {
    let routes = repository_routes();
    let total = routes.len();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for route in &routes {
        if let Some(kind) = route.kind.purpose_kind() {
            *counts.entry(format!("purpose/{kind}")).or_insert(0usize) += 1;
        }
    }
    let measured: std::collections::BTreeSet<String> = counts.keys().cloned().collect();
    let expected: std::collections::BTreeSet<String> = PURPOSE_IDENTITIES
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        measured, expected,
        "purpose_anchor_exact_set: total routes {total}, measured {counts:?}"
    );
    for identity in PURPOSE_IDENTITIES {
        let current = counts.get(identity).copied().unwrap_or(0);
        assert_eq!(
            current, 1,
            "purpose_anchor_exact_set/{identity}: expected 1, current {current}, total routes {total}"
        );
    }
    assert_eq!(
        counts.values().sum::<usize>(),
        EXPECTED_PURPOSE_ANCHOR_COUNT,
        "purpose_anchor_exact_set/cardinality: total routes {total}"
    );
}

#[test]
fn purpose_anchor_generic_coverage() {
    let f = fixture();
    let markdown = real_skill_markdown();
    let marker_kinds: Vec<String> = markdown
        .lines()
        .filter(|line| line.starts_with(MARKER_START))
        .map(ToOwned::to_owned)
        .collect();
    let routes = match extract_anchored_routes(&markdown) {
        Ok(routes) => routes,
        Err(error) => panic!(
            "purpose_anchor_generic_coverage/extraction: variant={error:?}, \
observations=0, measured markers={marker_kinds:?}"
        ),
    };
    let identities: std::collections::BTreeSet<String> = routes
        .iter()
        .filter(|route| route.kind.is_purpose())
        .filter_map(anchor_identity)
        .collect();
    let expected: std::collections::BTreeSet<String> = PURPOSE_IDENTITIES
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        identities, expected,
        "purpose_anchor_generic_coverage/identities"
    );
    let report = match review_document_attributed(&markdown, &f.bindings) {
        Ok(report) => report,
        Err(failure) => {
            let unknown_placeholders: Vec<_> = routes
                .iter()
                .find(|route| route.source_line == failure.source_line)
                .into_iter()
                .flat_map(|route| route.tokens.iter())
                .filter(|token| {
                    token.starts_with("{{")
                        && token.ends_with("}}")
                        && Placeholder::parse(token).is_err()
                })
                .map(|token| token.trim_start_matches("{{").trim_end_matches("}}"))
                .collect();
            panic!(
                "purpose_anchor_generic_coverage/review: variant={:?}, source_line={}, unknown_placeholders={unknown_placeholders:?}, observations={}, invocations={}",
                failure.error,
                failure.source_line,
                failure.observations.len(),
                f.bindings.pce_invocations.get()
            )
        }
    };
    for route in routes.iter().filter(|route| route.kind.is_purpose()) {
        let identity = anchor_identity(route).expect("purpose identity");
        let observed = report
            .observations
            .iter()
            .find(|observation| observation.source_line == route.source_line);
        assert!(
            observed.is_some_and(|observation| observation.status == 0),
            "purpose_anchor_generic_coverage/{identity}: no successful observation at source line {}, observed={observed:?}",
            route.source_line
        );
    }
}

fn assert_purpose_route_grammar(name: &str, kind: RouteKind) {
    let route = purpose_route(kind);
    let tokens = &route.tokens;
    let count = |needle: &str| {
        tokens
            .iter()
            .filter(|token| token.as_str() == needle)
            .count()
    };
    assert_eq!(route.kind.target(), "codex", "{name}/target");
    assert_eq!(
        token_value(&route, "--sandbox"),
        route.kind.required_sandbox(),
        "{name}/sandbox: expected {:?}, current {:?}",
        route.kind.required_sandbox(),
        token_value(&route, "--sandbox")
    );
    assert_eq!(
        count("--env"),
        REQUIRED_ENVIRONMENT_ENTRIES,
        "{name}/environment"
    );
    for placeholder in ENVIRONMENT_PLACEHOLDERS {
        assert_eq!(count(placeholder), 1, "{name}/environment/{placeholder}");
    }
    let artifact = count("--output-schema") + count("-o");
    assert_eq!(
        artifact, 0,
        "{name}/no-output-artifact: left {artifact}, right 0"
    );
    let plan_input = count("--plan-file");
    assert_eq!(
        plan_input, 0,
        "{name}/null-stdin: left {plan_input}, right 0"
    );
    assert_eq!(
        token_value(&route, "--node"),
        Some("{{NODE}}"),
        "{name}/dynamic-node: left {:?}, right {:?}",
        token_value(&route, "--node"),
        Some("{{NODE}}")
    );
    assert_eq!(
        token_value(&route, "--role"),
        Some("{{ROLE}}"),
        "{name}/dynamic-role: left {:?}, right {:?}",
        token_value(&route, "--role"),
        Some("{{ROLE}}")
    );
    assert_eq!(
        token_value(&route, "--ref"),
        Some("{{REF}}"),
        "{name}/ref-source: left {:?}, right {:?}",
        token_value(&route, "--ref"),
        Some("{{REF}}")
    );
    assert_eq!(
        token_value(&route, "--evidence"),
        Some("{{EVIDENCE}}"),
        "{name}/evidence-source: left {:?}, right {:?}",
        token_value(&route, "--evidence"),
        Some("{{EVIDENCE}}")
    );
    assert!(
        has_complete_logging_group(&route),
        "{name}/logging: removed_member={:?}, measured={:?}",
        missing_logging_member(&route),
        measured_logging_group(&route)
    );
    assert_eq!(count("--"), 1, "{name}/delimiter");
    let delimiter = tokens
        .iter()
        .position(|token| token == "--")
        .expect("delimiter");
    let tail: Vec<&str> = tokens[delimiter + 1..].iter().map(String::as_str).collect();
    assert_eq!(tail, ["{{CALLER_ARG}}"], "{name}/caller-tail");
    let f = fixture();
    let argv = substitute_route(&route, &f.bindings)
        .unwrap_or_else(|error| panic!("{name}/substitution: {error:?}"));
    let position = argv
        .iter()
        .position(|token| token == "--")
        .expect("substituted delimiter");
    let substituted_tail = &argv[position + 1..];
    assert_eq!(
        substituted_tail.len(),
        1,
        "{name}/post-substitution-tail: left {}, right 1",
        substituted_tail.len()
    );
    validate_route(&route, &argv)
        .unwrap_or_else(|error| panic!("{name}/parent-grammar: {error:?}"));
    assert_eq!(
        f.bindings.pce_invocations.get(),
        0,
        "{name}/before-execution"
    );
}

#[test]
fn commit_completion_route_grammar() {
    assert_purpose_route_grammar(
        "commit_completion_route_grammar",
        RouteKind::CodexCommitCompletion,
    );
}

#[test]
fn diagnostics_route_grammar() {
    assert_purpose_route_grammar("diagnostics_route_grammar", RouteKind::CodexDiagnostics);
}

#[test]
fn purpose_routes_use_complete_logging_envelope() {
    for kind in [
        RouteKind::CodexCommitCompletion,
        RouteKind::CodexDiagnostics,
    ] {
        let route = purpose_route(kind);
        let identity = format!("purpose/{}", kind.purpose_kind().expect("purpose kind"));
        let expected = expected_logging_group(&route).expect("purpose logging group");
        let measured = measured_logging_group(&route);
        let removed = missing_logging_member(&route);
        assert_eq!(
            removed, None,
            "purpose_routes_use_complete_logging_envelope/{identity}: removed_member={removed:?}, measured={measured:?}, expected={expected:?}"
        );
        assert!(
            has_complete_logging_group(&route),
            "purpose_routes_use_complete_logging_envelope/{identity}: ordered group measured={measured:?}, expected={expected:?}"
        );
    }
}

#[test]
fn purpose_routes_return_to_binary_owned_continuation() {
    let markdown = real_skill_markdown();
    assert_eq!(markdown.matches(PURPOSE_ROUTE_LIFETIME).count(), 1);
    assert_eq!(markdown.matches("Both children are awaited.").count(), 0);
    let reverted = markdown.replacen(PURPOSE_ROUTE_LIFETIME, "Both children are awaited.", 1);
    assert_eq!(reverted.matches(PURPOSE_ROUTE_LIFETIME).count(), 0);
    assert_eq!(reverted.matches("Both children are awaited.").count(), 1);
}

#[test]
fn manual_dispatch_lifecycle_surface_is_exact() {
    let markdown = real_skill_markdown();
    let heading = "the following manual append, read, status, readiness, and contract surfaces in their exact argument order:";
    let start = markdown.find(heading).expect("manual surface heading");
    let fenced = &markdown[start..];
    let block_start = fenced.find("```text\n").expect("manual text fence") + 8;
    let block_remainder = &fenced[block_start..];
    let block_end = block_remainder.find("\n```").expect("manual fence end");
    let block = &block_remainder[..block_end];
    for command in MANUAL_DISPATCH_LIFECYCLE_BLOCK.lines() {
        assert_eq!(block.lines().filter(|line| *line == command).count(), 1);
        assert!(!command.starts_with("<!-- pce-dispatch-route"));
    }
    assert_eq!(block.matches(MANUAL_DISPATCH_LIFECYCLE_BLOCK).count(), 1);
    assert_eq!(EXPECTED_ANCHORED_ROUTE_COUNT, 12);
}

#[test]
fn three_dispatch_derivations_are_exact() {
    let markdown = real_skill_markdown();
    let sentences = [
        "These binary-owned issuance records are the sole source for dispatch refs and for the exact `(node, role)` issuance ordinal; validated-production completion facts alone derive validated-production spends, while completion and required-artifact facts derive consecutive non-production.",
        "Issuance-ordinal and validated-production spending series are keyed by `(node, role)`, while consecutive non-production is keyed by `(node, role, required artifact)`, so Phase 1 `(m1-s1, milestone-planner)` cannot collide with Phase 3 `(m1-s1, step-plan-writer)`.",
        COMMON_ROLE_ROUTE_LIFETIME,
        "The issuance ordinal advances for every issuance and determines artifact naming only.",
        "Separately, validated-production spending limits count only validated-production spends keyed by `(node, role)`, so productless attempts are exempt.",
        "Consecutive non-production is keyed by `(node, role, required artifact)`; after two consecutive non-producing completions its typed hold is the separate stop, and the hold closes only through `retry`, `re-plan`, or `abandon`.",
        "Use the next exact `(node, falsification-critic)` issuance ordinal only for the existing review-artifact naming rule; validated-production spending accounting and consecutive-non-production admission remain separate derivations, and no new counter or artifact family is added.",
        "- Unexpected merge conflict dispatches Codex to rebase and resolve from the named base and head refs, supplying `git diff <base>...<head>`; a validated produced REVISE result charges the applicable validated-production spending limit, while a productless attempt charges no validated-production spend and remains subject to the separate consecutive-non-production hold; escalate if unresolved.",
        "Immediately before each verdict-producing gate issuance, read the accepted source event log and derive the exact `(node, role)` issuance ordinal by counting every prior `dispatch` record whose node and role byte-match the route.",
        "After the gate finishes, read the exact JSON path and validate it again against the installed verdict schema.",
    ];
    for sentence in sentences {
        assert_eq!(markdown.matches(sentence).count(), 1, "{sentence}");
    }
    for (_, cap_line) in &AUTHORED_REPLACEMENTS[3..6] {
        assert_eq!(markdown.matches(cap_line).count(), 1, "{cap_line}");
    }
    let review_cap = AUTHORED_REPLACEMENTS[8].1;
    assert_eq!(markdown.matches(review_cap).count(), 1, "{review_cap}");

    for (old, new) in [AUTHORED_REPLACEMENTS[6], AUTHORED_REPLACEMENTS[13]] {
        let reverted = markdown.replacen(new, old, 1);
        assert_eq!(reverted.matches(new).count(), 0);
        assert_eq!(reverted.matches(old).count(), 1);
    }
}

#[test]
fn binary_owned_check_in_reconcile_contract_is_exact() {
    let markdown = real_skill_markdown();
    assert_eq!(
        markdown
            .matches(BINARY_OWNED_ORCHESTRATION_PARAGRAPH)
            .count(),
        1
    );
    for clause in [
        "reports unknown, running, finished, or dead state and required-artifact production",
        "without signaling or killing a child, appending a completion, reconciling, or closing anything",
        "Only `pce dispatch reconcile` durably closes a dead incomplete issuance",
        "it refuses wherever a real completion could still land",
        "A missing product alone neither closes a dispatch nor licenses a completion or accounting claim",
        "all-accounted requires the issuance-ordered unaccounted ledger to be empty",
    ] {
        assert_eq!(
            BINARY_OWNED_ORCHESTRATION_PARAGRAPH.matches(clause).count(),
            1
        );
        assert_eq!(markdown.matches(clause).count(), 1);
        let mutated = markdown.replacen(clause, "inequivalent contract", 1);
        assert_eq!(mutated.matches(clause).count(), 0);
    }
}

#[test]
fn binary_owned_orchestration_clause_cardinalities_are_exact() {
    let markdown = real_skill_markdown();
    let section = cold_rules_section(&markdown);
    for (arm, clause) in [
        ("issuance-return", "appends issuance and returns"),
        (
            "continuation-retention",
            "appends issuance and returns while the binary-owned continuation retains the child",
        ),
        (
            "bounded-check-in",
            "observes every issuance with bounded `pce dispatch check-in`",
        ),
        (
            "read-only-check-in",
            "without signaling or killing a child, appending a completion, reconciling, or closing anything",
        ),
        (
            "reconcile-only",
            "Only `pce dispatch reconcile` durably closes a dead incomplete issuance",
        ),
        (
            "reconcile-refusal",
            "it refuses wherever a real completion could still land",
        ),
        (
            "accounting-ledger",
            "all-accounted requires the issuance-ordered unaccounted ledger to be empty",
        ),
        (
            "no-delta-result",
            "never append a `delta` or invent a result",
        ),
        (
            "rule5-check-in",
            "use bounded `pce dispatch check-in` for read-only observation",
        ),
        (
            "rule5-reconcile",
            "if an incomplete issuance is dead, close it only with `pce dispatch reconcile`",
        ),
        (
            "rule5-missing-product",
            "A missing required product alone neither closes nor accounts for the dispatch",
        ),
    ] {
        assert_eq!(section.matches(clause).count(), 1, "section/{arm}");
        assert_eq!(markdown.matches(clause).count(), 1, "document/{arm}");
        let mutated = markdown.replacen(clause, "inequivalent contract", 1);
        assert_eq!(mutated.matches(clause).count(), 0, "mutation/{arm}");
    }
}

#[test]
fn authored_replacement_manifest_is_exact() {
    let markdown = real_skill_markdown();
    for (old, new) in AUTHORED_REPLACEMENTS {
        assert_eq!(markdown.matches(old).count(), 0, "old payload: {old}");
        assert_eq!(markdown.matches(new).count(), 1, "new payload: {new}");
        let reverted = markdown.replacen(new, old, 1);
        assert_eq!(reverted.matches(new).count(), 0, "reverted new: {new}");
        assert_eq!(reverted.matches(old).count(), 1, "restored old: {old}");
    }
    assert_eq!(markdown.matches(MANUAL_DISPATCH_LIFECYCLE_BLOCK).count(), 1);
    assert_eq!(markdown.matches(COMMON_ROLE_ROUTE_LIFETIME).count(), 1);
    for removed in [MANUAL_DISPATCH_LIFECYCLE_BLOCK, COMMON_ROLE_ROUTE_LIFETIME] {
        let mutated = markdown.replacen(removed, "", 1);
        assert_eq!(mutated.matches(removed).count(), 0);
    }
}

#[test]
fn stale_dispatch_orchestration_claims_are_absent() {
    let markdown = real_skill_markdown();
    for stale in [
        "Both children are awaited.",
        "must be paired with a registered wait",
        "awaits its child",
        "append a `delta` at the canonical node to reconcile that fact",
        "reconcile a missing required product with a `delta` at the canonical node instead of inventing a result",
        "The same prior `(node, role)` dispatch count determines the artifact index and round cap; no separate counter exists.",
        "Derive rounds from dispatch records.",
        "Use the next exact `(node, falsification-critic)` dispatch count and the existing review-artifact naming rule for the verdict; add no counter or artifact family.",
        "classify the dispatch as REVISE-class for cap accounting",
        "After the gate returns,",
    ] {
        assert_eq!(markdown.matches(stale).count(), 0, "{stale}");
    }
}

#[test]
fn diagnostics_boundedness_contract_is_exact() {
    let markdown = real_skill_markdown();
    for (arm, clause) in [
        ("bounded-evidence", BOUNDED_EVIDENCE_CLAUSE),
        ("confined-scope", CONFINED_SCOPE_CLAUSE),
        ("no-mutation", NO_MUTATION_CLAUSE),
    ] {
        let current = markdown.matches(clause).count();
        assert_eq!(
            current, 1,
            "diagnostics_boundedness_contract_is_exact/{arm}: expected 1, current {current}"
        );
    }
    let current = markdown
        .lines()
        .find(|line| line.contains("Codex routes own their working-directory and "))
        .and_then(|line| line.split_once("working-directory and "))
        .and_then(|(_, rest)| rest.split_once(" sandbox options."))
        .map(|(sandbox, _)| sandbox)
        .unwrap_or("<missing>");
    assert_eq!(
        current, "workspace-write",
        "diagnostics_boundedness_contract_is_exact/sandbox-ownership: expected workspace-write, current {current}"
    );
}

#[test]
fn diagnostics_boundedness_environmental_limit_is_disclosed() {
    let markdown = real_skill_markdown();
    let bounded_evidence_count = markdown.matches(BOUNDED_EVIDENCE_CLAUSE).count();
    let clause_counts: Vec<_> = BOUNDEDNESS_DISCLOSURE_CLAUSES
        .iter()
        .map(|(label, clause)| (*label, markdown.matches(clause).count()))
        .collect();
    for (label, current) in &clause_counts {
        assert_eq!(
            *current, bounded_evidence_count,
            "diagnostics_boundedness_environmental_limit_is_disclosed/{label}: bounded_evidence_count={bounded_evidence_count}, clause_counts={clause_counts:?}"
        );
    }
}

#[test]
fn detached_dispatch_rule_is_exact() {
    let markdown = real_skill_markdown();
    let paragraphs = markdown
        .matches(BINARY_OWNED_ORCHESTRATION_PARAGRAPH)
        .count();
    let invented = markdown.matches("--detached").count();
    assert_eq!(
        invented, 0,
        "detached_dispatch_rule_is_exact/no-invented-option: expected 0, current {invented}"
    );
    let section = cold_rules_section(&markdown);
    for (arm, clause) in [
        ("issuance-return", "appends issuance and returns"),
        (
            "continuation-retains-child",
            "appends issuance and returns while the binary-owned continuation retains the child",
        ),
        (
            "bounded-check-in",
            "observes every issuance with bounded `pce dispatch check-in`",
        ),
        (
            "read-only-observation",
            "without signaling or killing a child, appending a completion, reconciling, or closing anything",
        ),
        (
            "reconciliation",
            "Only `pce dispatch reconcile` durably closes a dead incomplete issuance",
        ),
        (
            "reconcile-refusal",
            "it refuses wherever a real completion could still land",
        ),
        (
            "all-accounted",
            "all-accounted requires the issuance-ordered unaccounted ledger to be empty",
        ),
    ] {
        let current = section.matches(clause).count();
        assert_eq!(
            current, 1,
            "detached_dispatch_rule_is_exact/{arm}/section: expected 1, current {current}"
        );
        let document_current = markdown.matches(clause).count();
        assert_eq!(
            document_current, 1,
            "detached_dispatch_rule_is_exact/{arm}/document: expected 1, current {document_current}"
        );
    }
    assert_eq!(
        paragraphs, 1,
        "detached_dispatch_rule_is_exact/paragraph: expected 1, current {paragraphs}"
    );
}

#[test]
fn cold_orchestrator_rules_are_exact() {
    let markdown = real_skill_markdown();
    let headings = markdown.matches(COLD_RULES_HEADING).count();
    assert_eq!(
        headings, 1,
        "cold_orchestrator_rules_are_exact/heading: expected 1, current {headings}"
    );
    let section = cold_rules_section(&markdown);
    let detached = section
        .matches(BINARY_OWNED_ORCHESTRATION_PARAGRAPH)
        .count();
    assert_eq!(
        detached, 1,
        "cold_orchestrator_rules_are_exact/detached-paragraph: expected 1, current {detached}"
    );
    for (arm, clause) in [
        (
            "remedy-addition",
            "preserve and never remove the constraint being refined",
        ),
        (
            "measurement-comparison",
            "comparing the before and after measurements",
        ),
        ("remote-refresh", "from the remote"),
        ("pr-body-republication", "republish `pr-body.md`; "),
        (
            "setting-measurement",
            "`gh api repos/{owner}/{repo} --jq .squash_merge_commit_message`",
        ),
        (
            "rule5-check-in",
            "use bounded `pce dispatch check-in` for read-only observation",
        ),
        (
            "rule5-dead-reconciliation",
            "if an incomplete issuance is dead, close it only with `pce dispatch reconcile`",
        ),
        ("negation", "negation, "),
        ("alternative", "alternative, "),
        ("exception", "exception, "),
        ("ordering", "and ordering "),
    ] {
        let current = section.matches(clause).count();
        assert_eq!(
            current, 1,
            "cold_orchestrator_rules_are_exact/{arm}: expected 1, current {current}"
        );
    }
    let mut offsets = Vec::new();
    for (index, rule) in COLD_RULES.iter().enumerate() {
        let current = section.matches(rule).count();
        let arm = COLD_RULE_ARMS[index];
        assert_eq!(
            current, 1,
            "cold_orchestrator_rules_are_exact/{arm}: expected 1, current {current}"
        );
        offsets.push(section.find(rule).expect("rule offset"));
    }
    let mut ascending = offsets.clone();
    ascending.sort_unstable();
    assert_eq!(
        offsets, ascending,
        "cold_orchestrator_rules_are_exact/order: measured offsets {offsets:?}"
    );
    let detached_offset = section
        .find(BINARY_OWNED_ORCHESTRATION_PARAGRAPH)
        .expect("detached offset");
    assert!(
        detached_offset < offsets[0],
        "cold_orchestrator_rules_are_exact/order: detached {detached_offset}, first rule {}",
        offsets[0]
    );
}

const COLD_RULE_ARMS: [&str; 11] = [
    "remedy-addition",
    "measurement-comparison",
    "remote-refresh",
    "pr-body-republication",
    "detached-route",
    "branch-enumeration",
    "unused-control-rule",
    "compile-fail-twin",
    "known-match-audit",
    "prior-fix-measurement",
    "remedy-sweep",
];

#[test]
fn cold_rule_branch_controls() {
    let section = cold_rules_section(&real_skill_markdown());
    let controls: [(&str, &str, &str); 16] = [
        (
            "remedy-addition",
            "preserve and never remove the constraint being refined",
            "restate the constraint being refined",
        ),
        (
            "measurement-comparison",
            "comparing the before and after measurements",
            "comparing it with the expected literal",
        ),
        ("remote-refresh", "from the remote", ""),
        ("pr-body-republication", "republish `pr-body.md`; ", ""),
        (
            "setting-measurement",
            "`gh api repos/{owner}/{repo} --jq .squash_merge_commit_message`",
            "`merge_method`",
        ),
        (
            "rule5-check-in",
            "use bounded `pce dispatch check-in` for read-only observation",
            "",
        ),
        (
            "rule5-dead-reconciliation",
            "if an incomplete issuance is dead, close it only with `pce dispatch reconcile`",
            "proceed",
        ),
        ("negation", "negation, ", ""),
        ("alternative", "alternative, ", ""),
        ("exception", "exception, ", ""),
        ("ordering", "and ordering ", ""),
        (
            "unused-control",
            ": add that falsifier or remove the unused control",
            "",
        ),
        ("compile-fail-imports", "imports", "sources"),
        ("compile-fail-bindings", "bindings", "locals"),
        (
            "supplied-figure",
            "A figure supplied as context is never a measurement",
            "A figure supplied as context may be reported as observed",
        ),
        (
            "expected-comparison",
            "report it alongside the independently measured value and compare them",
            "report it",
        ),
    ];
    for (arm, clause, replacement) in controls {
        let green = section.matches(clause).count();
        assert_eq!(
            green, 1,
            "cold_rule_branch_controls/{arm}: green expected 1, current {green}"
        );
        let mutated = section.replacen(clause, replacement, 1);
        let current = mutated.matches(clause).count();
        assert_eq!(
            current, 0,
            "cold_rule_branch_controls/{arm}: mutated expected 0, current {current}"
        );
    }
    let remedy_sweep = "under the same falsification standard as the repaired text";
    let green = section.matches(remedy_sweep).count();
    assert_eq!(
        green, 1,
        "cold_rule_branch_controls/remedy-sweep: green expected 1, current {green}"
    );
    let exempted = section.replacen(remedy_sweep, "except where the claim is a remedy", 1);
    assert_eq!(
        exempted.matches(remedy_sweep).count(),
        0,
        "cold_rule_branch_controls/remedy-sweep: mutated expected 0"
    );
}

#[test]
fn fenced_hash_is_not_atx_heading() {
    let document = format!(
        "{CONSOLIDATION_MARKER}\n\n## Real section\n\n{}\n```sh\npce dispatch codex --cwd {{{{CWD}}}} --sandbox workspace-write --env {{{{PATH_ENV}}}} --env {{{{HOME_ENV}}}} --env {{{{USER_ENV}}}} --log-file {{{{LOG_FILE}}}} --node {{{{NODE}}}} --role {{{{ROLE}}}} --ref {{{{REF}}}} --evidence {{{{EVIDENCE}}}} -- {{{{CALLER_ARG}}}}\n```\n\n```sh\n# Only for the tracked-file-present, no-current-record initial case:\npce status\n```\n\npce log --file /tmp/events --kind dispatch --node m7-s4\n\n## Next real section\n",
        marker("codex-commit-completion")
    );
    let headings = atx_heading_lines(&document);
    let fenced_hash = document
        .lines()
        .position(|line| line.starts_with("# Only for the tracked-file-present"))
        .expect("fenced hash line");
    assert!(
        !headings[fenced_hash],
        "fenced_hash_is_not_atx_heading/detector: fenced # classified as heading"
    );
    assert_eq!(
        reject_colocated_standalone_append(&document),
        Err(ReviewError::CoLocatedStandaloneDispatchAppend),
        "fenced_hash_is_not_atx_heading: detector sets headings={:?}",
        headings
            .iter()
            .enumerate()
            .filter(|(_, heading)| **heading)
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
    );
}

#[test]
fn standalone_append_venue_known_match_control() {
    let markdown = real_skill_markdown();
    let control = markdown.replace(
        COLD_RULES_HEADING,
        &format!(
            "{COLD_RULES_HEADING}\n\npce log --file /tmp/events --kind dispatch --node m7-s4\n"
        ),
    );
    let by_venue = append_lines_by_venue(&control);
    let current = by_venue.get("other").cloned().unwrap_or_default();
    assert_eq!(
        current.len(),
        1,
        "standalone_append_venue_known_match_control/other: known match must measure venue other, current={current:?}"
    );
    assert!(
        append_lines_by_venue(&markdown).is_empty(),
        "standalone_append_venue_known_match_control/green: real document must measure zero"
    );
}

#[test]
fn to_graph_requires_race_safe_resume_for_irreversible_acts() {
    let skill =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/to-graph/SKILL.md"))
            .expect("to-graph skill");

    assert!(skill.contains("predecessor process is still writing"));
    assert!(skill.contains("WAIT, not permission to remove or repeat the act"));
    assert!(skill.contains("completion report and status file"));
}
