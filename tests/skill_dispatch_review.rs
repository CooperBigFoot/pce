//! review : SkillMarkdown × ReviewBindings → ReviewOutcome

use std::cell::Cell;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

const EXPECTED_ANCHORED_ROUTE_COUNT: usize = 0;
const MARKER_START: &str = "<!-- pce-dispatch-route";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouteKind {
    CodexUnstructured,
    CodexStructured,
    GateStructured,
}

impl RouteKind {
    fn parse(value: &str) -> Result<Self, ReviewError> {
        match value {
            "codex-unstructured" => Ok(Self::CodexUnstructured),
            "codex-structured" => Ok(Self::CodexStructured),
            "gate-structured" => Ok(Self::GateStructured),
            _ => Err(ReviewError::UnknownKind),
        }
    }

    const fn target(self) -> &'static str {
        match self {
            Self::CodexUnstructured | Self::CodexStructured => "codex",
            Self::GateStructured => "gate",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Placeholder {
    Cwd,
    Schema,
    Output,
    PlanFile,
    LogFile,
    Env,
    Node,
    Role,
    Ref,
    Evidence,
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
            "{{ENV}}" => Ok(Self::Env),
            "{{NODE}}" => Ok(Self::Node),
            "{{ROLE}}" => Ok(Self::Role),
            "{{REF}}" => Ok(Self::Ref),
            "{{EVIDENCE}}" => Ok(Self::Evidence),
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
    role: OsString,
    dispatch_ref: OsString,
    evidence: OsString,
    caller_arg: OsString,
    pce_invocations: Cell<usize>,
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
    MultipleDelimiters,
    CallerTailResuppliesBinaryArgument,
    Spawn(String),
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir_in(std::env::temp_dir()).expect("platform temp fixture");
    let cwd = root.path().join("cwd");
    let shim_dir = root.path().join("shim");
    fs::create_dir_all(cwd.join(".review")).expect("record directory");
    fs::create_dir_all(&shim_dir).expect("shim directory");
    let schema = root.path().join("schema.json");
    let plan_file = root.path().join("plan.bin");
    fs::write(&schema, br#"{"type":"object"}"#).expect("schema");
    fs::write(&plan_file, b"plan\0bytes").expect("plan");
    for executable in ["codex", "claude"] {
        let shim = shim_dir.join(executable);
        let result = if executable == "codex" {
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":4}}"#
        } else {
            r#"{"type":"result","subtype":"success","is_error":false,"result":"OK","usage":{"input_tokens":1,"output_tokens":2,"cache_creation_input_tokens":3,"cache_read_input_tokens":4}}"#
        };
        fs::write(
            &shim,
            format!(
                "#!/bin/sh\n: > \"$PWD/.review/argv\"\nfor arg in \"$@\"; do printf '%s\\0' \"$arg\" >> \"$PWD/.review/argv\"; done\n/bin/cat > \"$PWD/.review/stdin\"\n/usr/bin/env > \"$PWD/.review/env\"\nif [ \"${{ARTIFACT_MODE:-}}\" = valid ]; then printf '{{}}' > \"$ARTIFACT\"; fi\nif [ \"${{ARTIFACT_MODE:-}}\" = malformed ]; then printf '{{' > \"$ARTIFACT\"; fi\nif [ \"${{SILENT_STDOUT:-}}\" != yes ]; then printf '%s\\n' '{result}'; fi\nprintf '%s' \"${{EXIT_CODE:-0}}\" > \"$PWD/.review/completion\"\nexit \"${{EXIT_CODE:-0}}\"\n"
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
            role: OsString::from("step-plan-writer"),
            dispatch_ref: OsString::from("07b85ccd"),
            evidence: OsString::from("measured evidence with spaces"),
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
            "--env {{ENV}} -- {{CALLER_ARG}}"
        )
        .to_owned(),
        RouteKind::CodexStructured => concat!(
            "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write ",
            "--env {{ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} -- {{CALLER_ARG}}"
        )
        .to_owned(),
        RouteKind::GateStructured => concat!(
            "pce dispatch gate --cwd {{CWD}} --env {{ENV}} ",
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
        if !lines[index].starts_with("```") {
            return Err(ReviewError::InterveningContent);
        }
        if lines[index] != "```sh" {
            return Err(ReviewError::WrongFenceLanguage);
        }
        index += 1;
        let start = index;
        while index < lines.len() && lines[index] != "```" {
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

fn placeholder_value<'a>(placeholder: Placeholder, bindings: &'a ReviewBindings) -> &'a OsStr {
    match placeholder {
        Placeholder::Cwd => bindings.cwd.as_os_str(),
        Placeholder::Schema => bindings.schema.as_os_str(),
        Placeholder::Output => bindings.output.as_os_str(),
        Placeholder::PlanFile => bindings.plan_file.as_os_str(),
        Placeholder::LogFile => bindings.log_file.as_os_str(),
        Placeholder::Env => bindings.shim_dir.as_os_str(),
        Placeholder::Node => OsStr::new("m7-s1"),
        Placeholder::Role => bindings.role.as_os_str(),
        Placeholder::Ref => bindings.dispatch_ref.as_os_str(),
        Placeholder::Evidence => bindings.evidence.as_os_str(),
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
            let value = placeholder_value(placeholder, bindings);
            if value.is_empty() {
                return Err(match placeholder {
                    Placeholder::Role => ReviewError::EmptyRole,
                    Placeholder::Ref => ReviewError::EmptyRef,
                    Placeholder::Evidence => ReviewError::EmptyEvidence,
                    _ => ReviewError::ParentGrammar,
                });
            }
            if placeholder == Placeholder::Env {
                let mut entry = OsString::from("PATH=");
                entry.push(value);
                Ok(entry)
            } else {
                Ok(value.to_owned())
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
    if route.kind != RouteKind::GateStructured {
        if value(argv, &mut position, "--sandbox")? != "workspace-write" {
            return Err(ReviewError::ParentGrammar);
        }
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
    if env_count != 1 {
        return Err(ReviewError::ParentGrammar);
    }
    let has_structured = argv
        .get(position)
        .is_some_and(|token| token == "--output-schema");
    if has_structured {
        if !Path::new(value(argv, &mut position, "--output-schema")?).is_absolute()
            || !Path::new(value(argv, &mut position, "-o")?).is_absolute()
        {
            return Err(ReviewError::ParentGrammar);
        }
    }
    if (route.kind == RouteKind::CodexUnstructured) == has_structured {
        return Err(ReviewError::ParentGrammar);
    }
    if argv
        .get(position)
        .is_some_and(|token| token == "--plan-file")
    {
        let plan = value(argv, &mut position, "--plan-file")?;
        if !Path::new(plan).is_file() {
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
    bindings
        .pce_invocations
        .set(bindings.pce_invocations.get() + 1);
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(&argv[1..])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| ReviewError::Spawn(error.to_string()))?;
    let status = output.status.code().unwrap_or(-1);
    if !output.status.success() {
        return Err(ReviewError::Spawn(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(RouteObservation {
        source_line: 0,
        status,
        child_argv: read_nul(&bindings.cwd.join(".review/argv")),
    })
}

fn review_document(markdown: &str, bindings: &ReviewBindings) -> Result<ReviewReport, ReviewError> {
    if markdown.contains("codex exec") {
        return Err(ReviewError::RawCodexExec);
    }
    let routes = extract_anchored_routes(markdown)?;
    let mut observations = Vec::new();
    for route in &routes {
        let argv = substitute_route(route, bindings)?;
        validate_route(route, &argv)?;
        let mut observation = execute_route(&argv, bindings)?;
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
            let name = std::env::var("PCE_REVIEW_CASE").unwrap_or_else(|_| "fixture".to_owned());
            panic!("{name}: fixture must red, got {report:?}")
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
    if !case_enabled(name) {
        return;
    }
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

fn case_enabled(name: &str) -> bool {
    std::env::var("PCE_REVIEW_CASE").map_or(true, |selected| selected == name)
}

fn target_enabled(target: &str) -> bool {
    std::env::var("PCE_REVIEW_CASE").map_or(true, |selected| {
        let targeted = selected.starts_with("codex_")
            || selected.starts_with("gate_")
            || selected.ends_with("/codex")
            || selected.ends_with("/gate");
        !targeted
            || selected.starts_with(&format!("{target}_"))
            || selected.ends_with(&format!("/{target}"))
    })
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

fn assert_parser_red(name: &str, args: Vec<OsString>, diagnostic: &str) {
    if !case_enabled(name) {
        return;
    }
    let record = args
        .windows(2)
        .find(|pair| pair[0] == "--cwd")
        .map(|pair| PathBuf::from(&pair[1]).join(".review/argv"));
    let before = record.as_deref().map(read_nul).unwrap_or_default().len();
    let output = run_pce(&args, None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{name}: unexpectedly accepted");
    assert!(
        stderr.contains(diagnostic),
        "{name}: expected {diagnostic:?}, got {stderr:?}"
    );
    let after = record.as_deref().map(read_nul).unwrap_or_default().len();
    assert_eq!(before, after, "{name}: child invocation changed");
}

fn assert_parser_green(name: &str, args: Vec<OsString>) {
    if !case_enabled(name) {
        return;
    }
    let record = args
        .windows(2)
        .find(|pair| pair[0] == "--cwd")
        .map(|pair| PathBuf::from(&pair[1]).join(".review/argv"));
    let before = record.as_deref().map(read_nul).unwrap_or_default().len();
    let output = run_pce(&args, None);
    assert!(
        output.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let after = record.as_deref().map(read_nul).unwrap_or_default().len();
    assert_eq!(before, after, "{name}: child invocation changed");
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
        "--dry-run".into(),
        "--".into(),
        "tail".into(),
    ]);
    args
}

#[test]
fn real_skill_has_exact_pinned_anchor_count_zero() {
    let fixture = fixture();
    let markdown = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/skills/pce/SKILL.md"))
        .expect("repository skill");
    let report = review_document(&markdown, &fixture.bindings).expect("real skill review");
    assert_eq!(report.routes.len(), EXPECTED_ANCHORED_ROUTE_COUNT);
    assert!(report.observations.is_empty());
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
    let command = base_command(RouteKind::CodexUnstructured).replace("{{CALLER_ARG}}", "exec");
    assert_eq!(
        error_for(&document("codex-unstructured", &command)),
        ReviewError::CallerTailResuppliesBinaryArgument
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
        if !case_enabled(name) {
            continue;
        }
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
        (
            "anchor_shell_operator",
            "{{CALLER_ARG}}",
            "|",
            ReviewError::ShellConstruct,
        ),
    ] {
        if !case_enabled(name) {
            continue;
        }
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
    let joined = good.replace("--sandbox", "\\\n--sandbox");
    let joined_fixture = fixture();
    let report = review_document(
        &document("codex-unstructured", &joined),
        &joined_fixture.bindings,
    )
    .expect("joined_line_continuation_accepts");
    assert_eq!(report.observations.len(), 1);
    let unbroken_fixture = fixture();
    let unbroken = review_document(
        &document("codex-unstructured", &good),
        &unbroken_fixture.bindings,
    )
    .expect("unbroken control");
    let joined_argv = &report.observations[0].child_argv;
    let unbroken_argv = &unbroken.observations[0].child_argv;
    assert_eq!(
        &joined_argv[..3],
        &unbroken_argv[..3],
        "joined_line_continuation_accepts"
    );
    assert_eq!(
        &joined_argv[4..],
        &unbroken_argv[4..],
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
        ("unknown_placeholder_env", "{{ENV}}"),
        ("unknown_placeholder_caller_tail", "{{CALLER_ARG}}"),
    ];
    for (name, position) in positions {
        assert_document_error_before_pce(
            name,
            &document("codex-structured", &base.replace(position, "{{UNKNOWN}}")),
            ReviewError::UnknownPlaceholder,
        );
    }
    let logging = "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write --env {{ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} --plan-file {{PLAN_FILE}} --log-file {{LOG_FILE}} --node {{NODE}} --role {{ROLE}} --ref {{REF}} --evidence {{EVIDENCE}} --dry-run -- {{CALLER_ARG}}";
    for (name, position) in [
        ("unknown_placeholder_plan_file", "{{PLAN_FILE}}"),
        ("unknown_placeholder_log_file", "{{LOG_FILE}}"),
        ("unknown_placeholder_node", "{{NODE}}"),
        ("unknown_placeholder_role", "{{ROLE}}"),
        ("unknown_placeholder_ref", "{{REF}}"),
        ("unknown_placeholder_evidence", "{{EVIDENCE}}"),
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
        if !case_enabled(name) {
            continue;
        }
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
        if !case_enabled(name) {
            continue;
        }
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
        if !case_enabled(name) {
            continue;
        }
        assert_eq!(
            error_for(&document(
                "gate-structured",
                &gate.replace("{{CALLER_ARG}}", tail)
            )),
            ReviewError::CallerTailResuppliesBinaryArgument,
            "{name}"
        );
    }
    if case_enabled("codex_multiple_delimiters_route") {
        assert_eq!(
            error_for(&document(
                "codex-unstructured",
                &codex.replace("{{CALLER_ARG}}", "x -- y")
            )),
            ReviewError::MultipleDelimiters,
            "codex_multiple_delimiters_route"
        );
    }
    if case_enabled("gate_multiple_delimiters_route") {
        assert_eq!(
            error_for(&document(
                "gate-structured",
                &gate.replace("{{CALLER_ARG}}", "x -- y")
            )),
            ReviewError::MultipleDelimiters,
            "gate_multiple_delimiters_route"
        );
    }
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

    let codex_logged = "pce dispatch codex --cwd {{CWD}} --sandbox workspace-write --env {{ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} --log-file {{LOG_FILE}} --node {{NODE}} --role {{ROLE}} --ref {{REF}} --evidence {{EVIDENCE}} --dry-run -- {{CALLER_ARG}}";
    let gate_logged = "pce dispatch gate --cwd {{CWD}} --env {{ENV}} --output-schema {{SCHEMA}} -o {{OUTPUT}} --log-file {{LOG_FILE}} --node {{NODE}} --role {{ROLE}} --ref {{REF}} --evidence {{EVIDENCE}} --dry-run -- {{CALLER_ARG}}";
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
        if !case_enabled(name) {
            continue;
        }
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
            ]
        };
        for flag in flags {
            let index = complete.iter().position(|arg| arg == flag).expect("flag");
            let args = complete[..=index].to_vec();
            let diagnostic = if matches!(
                *flag,
                "--log-file" | "--node" | "--role" | "--ref" | "--evidence"
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
                ("logging_before_dry_run", 14, 24),
            ]
        } else {
            vec![
                ("cwd_before_env", 2, 4),
                ("env_before_structured", 4, 6),
                ("structured_before_plan", 6, 10),
                ("plan_before_logging", 10, 12),
                ("logging_before_dry_run", 12, 22),
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
                "logging_before_dry_run" => 10,
                _ => unreachable!(),
            };
            let right_len = match suffix {
                "structured_before_plan" => 2,
                "plan_before_logging" => 10,
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
        let fields = ["--log-file", "--node", "--role", "--ref", "--evidence"];
        for supplied in 1..5 {
            let mut args = original[..base + supplied * 2].to_vec();
            args.extend(["--".into(), "tail".into()]);
            let suffix = [
                "log_file_only",
                "through_node",
                "through_role",
                "through_ref",
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
            "--log-file" | "--node" | "--role" | "--ref" | "--evidence"
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
        if !target_enabled(target) {
            continue;
        }
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
        if !target_enabled(target) {
            continue;
        }
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
            ],
        );
        let output = run_pce(&args, None);
        assert!(
            !output.status.success(),
            "child_nonzero_completion/{target}"
        );
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
        let stderr = String::from_utf8_lossy(&output.stderr);
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
    }
}

#[test]
fn repeated_placeholder_values_are_byte_identical() {
    let f = fixture();
    let command = base_command(RouteKind::CodexUnstructured)
        .replace("{{CALLER_ARG}}", "{{CALLER_ARG}} {{CALLER_ARG}}");
    let report = review_document(&document("codex-unstructured", &command), &f.bindings)
        .expect("repeated route");
    let argv = &report.observations[0].child_argv;
    let length = argv.len();
    assert!(length >= 2);
    assert_eq!(argv[length - 2], argv[length - 1]);
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
