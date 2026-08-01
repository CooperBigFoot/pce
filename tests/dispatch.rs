#[allow(dead_code)]
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pce_core::{
    AbsoluteOutputPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory, ArgumentVector,
    ArtifactOutcome, ChildEnvironment, CodexTokenUsage, Deferred, DispatchDuration,
    DispatchEnvelope, DispatchExitStatus, DispatchLogging, DispatchRef, DispatchRole, EventBodyRef,
    EventRecord, EventTimestamp, Evidence, Executable, KnownPayload, NodeId, Sandbox, Sequence,
    StdinBinding, UsageAbsenceReason, WriteKind, dispatch_invocation, dispatch_payload,
    parse_event_line, serialize_event_line,
};
use serde::Serialize;
use support::{CliHarness, CodexInvocation};

const INHERITED_MARKER: (&str, &str) = ("PCE_INHERITED_ONLY", "must-not-reach-codex");
const CHILD_MARKER: (&str, &str) = ("PCE_CHILD_MARKER", "explicit-child-value");
static DISPATCH_TEST_LOCK: Mutex<()> = Mutex::new(());

fn dispatch_test_guard() -> MutexGuard<'static, ()> {
    DISPATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Serialize)]
struct FixtureProjection {
    envelope: FixtureEnvelope,
    issuance: FixtureIssuance,
    completion: FixtureCompletion,
}

#[derive(Serialize)]
struct FixtureEnvelope {
    executable: &'static str,
    argv: Vec<String>,
    cwd: String,
    environment: BTreeMap<String, String>,
    stdin: FixtureStdin,
    schema_path: Option<String>,
    output_path: Option<String>,
}

#[derive(Serialize)]
struct FixtureStdin {
    binding: &'static str,
    bytes: Option<Vec<u8>>,
}

#[derive(Serialize)]
struct FixtureIssuance {
    sequence: FixtureDeferred,
    timestamp: FixtureDeferred,
    kind: &'static str,
    node: &'static str,
    payload: FixtureIssuancePayload,
}

#[derive(Serialize)]
struct FixtureIssuancePayload {
    role: &'static str,
    r#ref: &'static str,
    evidence: &'static str,
}

#[derive(Serialize)]
struct FixtureCompletion {
    sequence: FixtureDeferred,
    timestamp: FixtureDeferred,
    kind: &'static str,
    node: &'static str,
    payload: FixtureCompletionPayload,
}

#[derive(Serialize)]
struct FixtureCompletionPayload {
    issuance_sequence: FixtureDeferred,
    duration_ms: FixtureDeferred,
    usage: FixtureDeferred,
    exit_status: FixtureDeferred,
    artifact_outcome: FixtureArtifactOutcome,
}

#[derive(Clone, Copy, Serialize)]
struct FixtureDeferred {
    state: &'static str,
}

#[derive(Serialize)]
#[serde(untagged)]
enum FixtureArtifactOutcome {
    Observed(&'static str),
    Deferred(FixtureDeferred),
}

#[test]
fn dry_run_projects_all_envelope_shapes_exactly_and_matches_live_invocations() {
    let _guard = dispatch_test_guard();
    for (name, structured, plan) in [
        ("dry-unstructured-null", false, None),
        ("dry-structured-null", true, None),
        (
            "dry-unstructured-plan",
            false,
            Some(b"plan\0\xff".as_slice()),
        ),
        (
            "dry-structured-plan",
            true,
            Some(b"struct\0\x80".as_slice()),
        ),
    ] {
        assert_dry_projection_case(name, structured, plan);
    }
    assert_dry_parser_and_tail_failures();
}

fn assert_dry_projection_case(name: &str, structured: bool, plan: Option<&[u8]>) {
    const SEED: &[u8] = b"{\"sequence\":1,\"timestamp\":\"2026-07-27T12:34:56.000Z\",\"kind\":\"delta\",\"node\":\"m3-s1\",\"payload\":{\"message\":\"seed\"}}\n";
    let harness = CliHarness::new().expect("create CLI harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize child cwd");
    let log_path = harness.path().join(format!("{name}.jsonl"));
    fs::write(&log_path, SEED).expect("seed event log");
    let seeded_bytes = fs::read(&log_path).expect("read seed");
    let record_root = harness.path().join(format!("{name}-records"));
    fs::create_dir(&record_root).expect("create record root");
    let stdout_path = harness.path().join(format!("{name}.stdout"));
    let stderr_path = harness.path().join(format!("{name}.stderr"));
    let parent_stdin_path = harness.path().join(format!("{name}.parent-stdin"));
    fs::write(&stdout_path, b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":1,\"cached_input_tokens\":2,\"output_tokens\":3,\"reasoning_output_tokens\":4,\"future_total\":5}}\n").expect("write stdout fixture");
    fs::write(&stderr_path, []).expect("write stderr fixture");
    fs::write(&parent_stdin_path, format!("parent-{name}")).expect("write parent stdin");
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    environment.push(("ZZZ_EXPLICIT".to_owned(), "last".to_owned()));
    environment.push(("AAA_EXPLICIT".to_owned(), "first".to_owned()));
    let schema_path = cwd.join(format!("{name}-schema.json"));
    let output_path = cwd.join(format!("{name}-output.json"));
    let plan_path = plan.map(|bytes| {
        let path = harness.path().join(format!("{name}.plan"));
        fs::write(&path, bytes).expect("write plan");
        path
    });
    let caller_arguments = vec![format!("{name}-first"), format!("{name}-second")];
    let structured_paths = structured.then_some((&schema_path, &output_path));
    let mut dry_argv = dispatch_argv(
        &cwd,
        &environment,
        structured_paths,
        plan_path.as_deref(),
        caller_arguments.as_slice(),
    );
    let delimiter = dry_argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    dry_argv.splice(delimiter..delimiter, logging_arguments(&log_path));
    let delimiter = dry_argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    dry_argv.insert(delimiter, "--dry-run".to_owned());

    let dry_output = harness
        .run_with_stdin_file(&dry_argv, &parent_stdin_path, INHERITED_MARKER)
        .expect("run dry dispatch");
    assert!(
        dry_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&dry_output.stderr)
    );
    assert_eq!(
        dry_output.stdout,
        literal_projection(
            &cwd,
            &environment,
            structured_paths,
            plan,
            &caller_arguments
        ),
        "literal fixture for {name}"
    );
    let after_dry = fs::read(&log_path).expect("reread log");
    assert_eq!(after_dry, seeded_bytes);
    assert_eq!(
        seeded_bytes.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    assert_eq!(after_dry.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(
        harness
            .codex_invocations(&record_root)
            .expect("dry invocations"),
        Vec::new()
    );
    assert!(!record_root.join("invocation").exists());

    let projection: serde_json::Value =
        serde_json::from_slice(&dry_output.stdout).expect("parse projection");
    assert_shared_serializer_family(
        &projection,
        &cwd,
        &environment,
        structured_paths,
        plan,
        &caller_arguments,
    );

    let alternate_log = harness.path().join(format!("{name}-alternate.jsonl"));
    fs::write(&alternate_log, b"{\"sequence\":41,\"timestamp\":\"2026-07-27T12:34:56.000Z\",\"kind\":\"delta\",\"node\":\"m3-s1\",\"payload\":{\"message\":\"alternate\"}}\n").expect("write alternate tail");
    let mut alternate_argv = dry_argv.clone();
    let log_flag = alternate_argv
        .iter()
        .position(|value| value == "--log-file")
        .expect("log flag");
    alternate_argv[log_flag + 1] = alternate_log.display().to_string();
    let alternate_output = harness
        .run_with_stdin_file(&alternate_argv, &parent_stdin_path, INHERITED_MARKER)
        .expect("run alternate-tail projection");
    assert!(alternate_output.status.success());
    let alternate: serde_json::Value =
        serde_json::from_slice(&alternate_output.stdout).expect("parse alternate projection");
    for pointer in [
        "/issuance/sequence",
        "/issuance/timestamp",
        "/completion/sequence",
        "/completion/timestamp",
        "/completion/payload/issuance_sequence",
        "/completion/payload/duration_ms",
        "/completion/payload/usage",
        "/completion/payload/exit_status",
    ] {
        assert_eq!(
            projection.pointer(pointer),
            alternate.pointer(pointer),
            "tail-independent {pointer}"
        );
    }

    let mut live_argv = dry_argv.clone();
    let removed_position = live_argv
        .iter()
        .position(|value| value == "--dry-run")
        .expect("dry token");
    assert_eq!(live_argv.remove(removed_position), "--dry-run");
    assert_eq!(
        removed_position,
        live_argv
            .iter()
            .position(|value| value == "--")
            .expect("live delimiter")
    );
    let compared = dry_argv
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != removed_position)
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    assert_eq!(live_argv, compared);
    let live_output = harness
        .run_with_stdin_file(&live_argv, &parent_stdin_path, INHERITED_MARKER)
        .expect("run live dispatch");
    assert!(
        live_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&live_output.stderr)
    );
    let invocations = harness
        .codex_invocations(&record_root)
        .expect("live invocation");
    assert_eq!(invocations.len(), 1);
    assert_invocation(
        &invocations[0],
        &cwd,
        &environment,
        structured_paths,
        caller_arguments.as_slice(),
        plan.unwrap_or_default(),
    );
    assert_eq!(projection["envelope"]["executable"], "codex");
    let captured_argv = invocations[0]
        .argv
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        projection["envelope"]["argv"],
        serde_json::to_value(captured_argv).expect("serialize captured argv")
    );
    assert_eq!(projection["envelope"]["cwd"], cwd.display().to_string());
    assert_eq!(
        projection["envelope"]["stdin"]["bytes"],
        plan.map_or(serde_json::Value::Null, |bytes| serde_json::to_value(bytes)
            .expect("serialize bytes"))
    );
    let mut projected_environment = projection["envelope"]["environment"]
        .as_object()
        .expect("projected environment")
        .iter()
        .map(|(name, value)| {
            OsString::from(format!(
                "{name}={}",
                value.as_str().expect("environment value")
            ))
        })
        .collect::<BTreeSet<_>>();
    projected_environment.insert(OsString::from(format!("PWD={}", cwd.display())));
    projected_environment.insert(OsString::from("SHLVL=1"));
    projected_environment.insert(OsString::from("_=/usr/bin/env"));
    assert_eq!(invocations[0].environment, projected_environment);
    assert!(
        !invocations[0].environment.contains(&OsString::from(format!(
            "{}={}",
            INHERITED_MARKER.0, INHERITED_MARKER.1
        )))
    );
}

fn assert_dry_parser_and_tail_failures() {
    const DIAGNOSTIC: &str = "dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence";
    let harness = CliHarness::new().expect("create parser harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize parser cwd");
    let prefix = vec![
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        cwd.display().to_string(),
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
    ];
    let failures = [
        vec!["--dry-run", "--", "PROMPT"],
        vec![
            "--dry-run",
            "--log-file",
            "/tmp/events",
            "--node",
            "n",
            "--role",
            "r",
            "--ref",
            "x",
            "--evidence",
            "e",
            "--",
            "PROMPT",
        ],
        vec![
            "--log-file",
            "/tmp/events",
            "--node",
            "n",
            "--dry-run",
            "--role",
            "r",
            "--ref",
            "x",
            "--evidence",
            "e",
            "--",
            "PROMPT",
        ],
        vec![
            "--log-file",
            "/tmp/events",
            "--node",
            "n",
            "--dry-run",
            "--",
            "PROMPT",
        ],
    ];
    for suffix in failures {
        let argv = prefix
            .iter()
            .cloned()
            .chain(suffix.into_iter().map(str::to_owned))
            .collect::<Vec<_>>();
        let output = harness.run(&argv, b"").expect("run parser failure");
        assert!(!output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .last()
                .map(str::trim_start),
            Some(DIAGNOSTIC)
        );
        assert!(output.stdout.is_empty());
        assert!(harness.invocations().expect("shim invocations").is_empty());
    }

    let unreadable_plan = harness.path().join("absent.plan");
    let unreadable_log = harness.path().join("unreadable-plan-log.jsonl");
    fs::write(&unreadable_log, b"not-json\n").expect("write malformed unread log");
    let mut argv = prefix.clone();
    argv.extend([
        "--plan-file".to_owned(),
        unreadable_plan.display().to_string(),
    ]);
    argv.extend(logging_arguments(&unreadable_log));
    argv.extend(["--dry-run".to_owned(), "--".to_owned(), "PROMPT".to_owned()]);
    let output = harness
        .run(&argv, b"")
        .expect("run unreadable-plan projection");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to read plan file"));
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("supplied event-log tail is invalid")
    );
    assert!(output.stdout.is_empty());

    for (name, tail, expected) in [
        ("malformed", b"not-json\n".as_slice(), "supplied event-log tail is invalid"),
        ("overflow", b"{\"sequence\":18446744073709551615,\"timestamp\":\"2026-07-27T12:34:56.000Z\",\"kind\":\"delta\",\"node\":\"m3-s1\",\"payload\":{\"message\":\"max\"}}\n".as_slice(), "supplied event-log tail sequence 18446744073709551615 has no valid successor"),
    ] {
        let log_path = harness.path().join(format!("{name}.jsonl"));
        fs::write(&log_path, tail).expect("write invalid tail");
        let record_root = harness.path().join(format!("{name}-records"));
        let mut argv = prefix.clone();
        argv.extend(logging_arguments(&log_path));
        argv.extend(["--dry-run".to_owned(), "--".to_owned(), "PROMPT".to_owned()]);
        let output = harness.run(&argv, b"").expect("run invalid-tail projection");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        assert!(output.stdout.is_empty());
        assert_eq!(harness.codex_invocations(&record_root).expect("tail invocations"), Vec::new());
    }

    let missing_log = harness.path().join("missing.jsonl");
    let mut argv = prefix;
    argv.extend(logging_arguments(&missing_log));
    argv.extend(["--dry-run".to_owned(), "--".to_owned(), "PROMPT".to_owned()]);
    let output = harness.run(&argv, b"").expect("run missing-log projection");
    assert!(output.status.success());
    assert!(!missing_log.exists());
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
}

fn logging_arguments(log_path: &Path) -> Vec<String> {
    vec![
        "--log-file".to_owned(),
        log_path.display().to_string(),
        "--node".to_owned(),
        "m3-s2".to_owned(),
        "--role".to_owned(),
        "step-executor".to_owned(),
        "--ref".to_owned(),
        "fixture-ref".to_owned(),
        "--evidence".to_owned(),
        "fixture-evidence".to_owned(),
    ]
}

fn literal_projection(
    cwd: &Path,
    environment: &[(String, String)],
    structured: Option<(&PathBuf, &PathBuf)>,
    plan: Option<&[u8]>,
    caller_arguments: &[String],
) -> Vec<u8> {
    let mut argv = vec![
        "exec".to_owned(),
        "--json".to_owned(),
        "-C".to_owned(),
        cwd.display().to_string(),
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
    ];
    if let Some((schema, output)) = structured {
        argv.extend([
            "--output-schema".to_owned(),
            schema.display().to_string(),
            "-o".to_owned(),
            output.display().to_string(),
        ]);
    }
    argv.extend(caller_arguments.iter().cloned());
    let deferred = FixtureDeferred { state: "deferred" };
    let fixture = FixtureProjection {
        envelope: FixtureEnvelope {
            executable: "codex",
            argv,
            cwd: cwd.display().to_string(),
            environment: environment.iter().cloned().collect(),
            stdin: FixtureStdin {
                binding: if plan.is_some() { "plan-bytes" } else { "null" },
                bytes: plan.map(<[u8]>::to_vec),
            },
            schema_path: structured.map(|(schema, _)| schema.display().to_string()),
            output_path: structured.map(|(_, output)| output.display().to_string()),
        },
        issuance: FixtureIssuance {
            sequence: deferred,
            timestamp: deferred,
            kind: "dispatch",
            node: "m3-s2",
            payload: FixtureIssuancePayload {
                role: "step-executor",
                r#ref: "fixture-ref",
                evidence: "fixture-evidence",
            },
        },
        completion: FixtureCompletion {
            sequence: deferred,
            timestamp: deferred,
            kind: "dispatch-completion",
            node: "m3-s2",
            payload: FixtureCompletionPayload {
                issuance_sequence: deferred,
                duration_ms: deferred,
                usage: deferred,
                exit_status: deferred,
                artifact_outcome: structured
                    .map_or(FixtureArtifactOutcome::Observed("not-validated"), |_| {
                        FixtureArtifactOutcome::Deferred(deferred)
                    }),
            },
        },
    };
    let mut bytes = serde_json::to_vec(&fixture).expect("serialize independent fixture");
    bytes.push(b'\n');
    bytes
}

fn assert_shared_serializer_family(
    actual: &serde_json::Value,
    cwd: &Path,
    environment: &[(String, String)],
    structured: Option<(&PathBuf, &PathBuf)>,
    plan: Option<&[u8]>,
    caller_arguments: &[String],
) {
    let stdin = plan.map_or(StdinBinding::Null, |bytes| {
        StdinBinding::PlanBytes(bytes.to_vec())
    });
    let mut envelope = DispatchEnvelope::new(
        Executable::parse("codex").expect("executable"),
        AbsoluteWorkingDirectory::parse(cwd).expect("working directory"),
        stdin,
    )
    .with_arguments(ArgumentVector::new(caller_arguments.to_vec()))
    .with_environment(ChildEnvironment::new(environment.iter().cloned().collect()))
    .with_sandbox(Sandbox::WorkspaceWrite);
    if let Some((schema, output)) = structured {
        envelope = envelope
            .with_schema_path(AbsoluteSchemaPath::parse(schema).expect("schema path"))
            .with_output_path(AbsoluteOutputPath::parse(output).expect("output path"));
    }
    assert_eq!(
        actual["envelope"],
        serde_json::to_value(dispatch_invocation(&envelope)).expect("serialize shared invocation")
    );
    let logging = DispatchLogging {
        node: NodeId::parse("m3-s2").expect("node"),
        role: DispatchRole::new("step-executor"),
        dispatch_ref: DispatchRef::new("fixture-ref"),
        evidence: Evidence::parse("fixture-evidence").expect("evidence"),
    };
    let payload = dispatch_payload(&logging);
    assert_eq!(
        actual["issuance"]["payload"],
        serde_json::to_value(&payload).expect("serialize shared payload")
    );
    assert_eq!(actual["issuance"]["kind"], WriteKind::Dispatch.as_str());
    assert_eq!(actual["issuance"]["node"], logging.node.as_str());
    let record = EventRecord::known(
        Sequence::parse(71).expect("sequence"),
        EventTimestamp::parse("2026-08-01T12:34:56.789Z").expect("timestamp"),
        logging.node,
        KnownPayload::Dispatch(payload),
    );
    let real: serde_json::Value =
        serde_json::from_str(&serialize_event_line(&record).expect("serialize real event"))
            .expect("parse real event");
    let actual_keys = actual["issuance"]
        .as_object()
        .expect("issuance object")
        .keys()
        .collect::<BTreeSet<_>>();
    let real_keys = real
        .as_object()
        .expect("real event object")
        .keys()
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_keys, real_keys);
    let _: Deferred<Sequence> = serde_json::from_value(actual["issuance"]["sequence"].clone())
        .expect("typed deferred sequence");
    let _: Deferred<EventTimestamp> =
        serde_json::from_value(actual["issuance"]["timestamp"].clone())
            .expect("typed deferred timestamp");
    let _: Deferred<Sequence> =
        serde_json::from_value(actual["completion"]["payload"]["issuance_sequence"].clone())
            .expect("typed deferred issuance correlation");
    let _: Deferred<DispatchDuration> =
        serde_json::from_value(actual["completion"]["payload"]["duration_ms"].clone())
            .expect("typed deferred duration");
    let _: Deferred<CodexTokenUsage> =
        serde_json::from_value(actual["completion"]["payload"]["usage"].clone())
            .expect("typed deferred usage");
    let _: Deferred<DispatchExitStatus> =
        serde_json::from_value(actual["completion"]["payload"]["exit_status"].clone())
            .expect("typed deferred exit status");
}

struct DispatchCase<'a> {
    name: &'a str,
    structured: bool,
    plan: Option<&'a [u8]>,
    caller_tail: &'a str,
}

#[test]
fn dispatches_unstructured_null_with_explicit_process_configuration() {
    assert_dispatch(DispatchCase {
        name: "unstructured-null",
        structured: false,
        plan: None,
        caller_tail: "NULL_PROMPT_MARKER",
    });
}

#[test]
fn dispatches_structured_null_with_explicit_process_configuration() {
    assert_dispatch(DispatchCase {
        name: "structured-null",
        structured: true,
        plan: None,
        caller_tail: "STRUCTURED_NULL_PROMPT_MARKER",
    });
}

#[test]
fn dispatches_unstructured_plan_with_exact_plan_stdin() {
    assert_dispatch(DispatchCase {
        name: "unstructured-plan",
        structured: false,
        plan: Some(b"plan\0bytes\xffwithout-newline"),
        caller_tail: "-",
    });
}

#[test]
fn dispatches_structured_plan_with_exact_plan_stdin() {
    assert_dispatch(DispatchCase {
        name: "structured-plan",
        structured: true,
        plan: Some(b"structured\x80plan\0bytes"),
        caller_tail: "-",
    });
}

fn assert_dispatch(case: DispatchCase<'_>) {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create CLI harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize child cwd");
    let record_root = harness.path().join(format!("{}-records", case.name));
    fs::create_dir(&record_root).expect("create record root");
    let stdout_path = harness.path().join(format!("{}.stdout", case.name));
    let stderr_path = harness.path().join(format!("{}.stderr", case.name));
    let expected_stdout = b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":1,\"cached_input_tokens\":2,\"output_tokens\":3,\"reasoning_output_tokens\":4}}\n".to_vec();
    let expected_stderr = format!("{} stderr\0bytes", case.name).into_bytes();
    fs::write(&stdout_path, &expected_stdout).expect("write fixture stdout");
    fs::write(&stderr_path, &expected_stderr).expect("write fixture stderr");
    let sentinel_path = harness.path().join(format!("{}.parent-stdin", case.name));
    let sentinel = format!("unique-parent-sentinel-{}", case.name).into_bytes();
    fs::write(&sentinel_path, &sentinel).expect("write parent stdin sentinel");

    let child_environment =
        child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let schema_path = cwd.join("schema.json");
    let output_path = cwd.join("output.json");
    let plan_path = case.plan.map(|bytes| {
        let path = harness.path().join(format!("{}.plan", case.name));
        fs::write(&path, bytes).expect("write plan fixture");
        path
    });
    let argv = dispatch_argv(
        &cwd,
        &child_environment,
        case.structured.then_some((&schema_path, &output_path)),
        plan_path.as_deref(),
        case.caller_tail,
    );
    let output = harness
        .run_with_stdin_file(&argv, &sentinel_path, INHERITED_MARKER)
        .expect("run dispatch");

    let invocations = harness
        .codex_invocations(&record_root)
        .expect("read Codex record");
    assert_eq!(invocations.len(), 1);
    let invocation = &invocations[0];
    assert_invocation(
        invocation,
        &cwd,
        &child_environment,
        case.structured.then_some((&schema_path, &output_path)),
        case.caller_tail,
        case.plan.unwrap_or_default(),
    );
    assert_ne!(invocation.stdin, sentinel);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected_stdout);
    assert!(output.stderr.ends_with(&expected_stderr));
    assert_no_jsonl_files(harness.path());
}

fn assert_no_jsonl_files(root: &Path) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read harness directory") {
            let entry = entry.expect("read harness entry");
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                assert_ne!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("jsonl"),
                    "logging-absent dispatch created {}",
                    path.display()
                );
            }
        }
    }
}

fn child_environment(
    harness: &CliHarness,
    record_root: &Path,
    stdout_path: &Path,
    stderr_path: &Path,
    exit_code: i32,
) -> Vec<(String, String)> {
    vec![
        ("PATH".to_owned(), harness.shim_path()),
        (
            "PCE_CODEX_RECORD_ROOT".to_owned(),
            record_root.display().to_string(),
        ),
        (
            "PCE_CODEX_STDOUT_FILE".to_owned(),
            stdout_path.display().to_string(),
        ),
        (
            "PCE_CODEX_STDERR_FILE".to_owned(),
            stderr_path.display().to_string(),
        ),
        ("PCE_CODEX_EXIT_CODE".to_owned(), exit_code.to_string()),
        (CHILD_MARKER.0.to_owned(), CHILD_MARKER.1.to_owned()),
    ]
}

trait CallerArguments {
    fn append_strings(&self, target: &mut Vec<String>);
    fn append_os_strings(&self, target: &mut Vec<OsString>);
}

impl CallerArguments for str {
    fn append_strings(&self, target: &mut Vec<String>) {
        target.push(self.to_owned());
    }

    fn append_os_strings(&self, target: &mut Vec<OsString>) {
        target.push(OsString::from(self));
    }
}

impl CallerArguments for [String] {
    fn append_strings(&self, target: &mut Vec<String>) {
        target.extend(self.iter().cloned());
    }

    fn append_os_strings(&self, target: &mut Vec<OsString>) {
        target.extend(self.iter().map(OsString::from));
    }
}

fn dispatch_argv<A: CallerArguments + ?Sized>(
    cwd: &Path,
    environment: &[(String, String)],
    structured: Option<(&PathBuf, &PathBuf)>,
    plan_path: Option<&Path>,
    caller_arguments: &A,
) -> Vec<String> {
    let mut argv = vec![
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        cwd.display().to_string(),
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
    ];
    for (name, value) in environment {
        argv.extend(["--env".to_owned(), format!("{name}={value}")]);
    }
    if let Some((schema, output)) = structured {
        argv.extend([
            "--output-schema".to_owned(),
            schema.display().to_string(),
            "-o".to_owned(),
            output.display().to_string(),
        ]);
    }
    if let Some(path) = plan_path {
        argv.extend(["--plan-file".to_owned(), path.display().to_string()]);
    }
    argv.push("--".to_owned());
    caller_arguments.append_strings(&mut argv);
    argv
}

fn assert_invocation<A: CallerArguments + ?Sized>(
    invocation: &CodexInvocation,
    cwd: &Path,
    environment: &[(String, String)],
    structured: Option<(&PathBuf, &PathBuf)>,
    caller_arguments: &A,
    expected_stdin: &[u8],
) {
    let mut expected_argv = vec![
        OsString::from("exec"),
        OsString::from("--json"),
        OsString::from("-C"),
        cwd.as_os_str().to_owned(),
        OsString::from("--sandbox"),
        OsString::from("workspace-write"),
    ];
    if let Some((schema, output)) = structured {
        expected_argv.extend([
            OsString::from("--output-schema"),
            schema.as_os_str().to_owned(),
            OsString::from("-o"),
            output.as_os_str().to_owned(),
        ]);
    }
    caller_arguments.append_os_strings(&mut expected_argv);
    assert_eq!(invocation.argv, expected_argv);
    assert_eq!(invocation.cwd, cwd);
    let mut expected_environment: BTreeSet<OsString> = environment
        .iter()
        .map(|(name, value)| OsString::from(format!("{name}={value}")))
        .collect();
    expected_environment.insert(OsString::from(format!("PWD={}", cwd.display())));
    expected_environment.insert(OsString::from("SHLVL=1"));
    expected_environment.insert(OsString::from("_=/usr/bin/env"));
    assert_eq!(invocation.environment, expected_environment);
    assert!(!invocation.environment.contains(&OsString::from(format!(
        "{}={}",
        INHERITED_MARKER.0, INHERITED_MARKER.1
    ))));
    assert_eq!(invocation.stdin, expected_stdin);
}

#[test]
fn records_measured_dispatch_lifecycle_with_exact_correlation() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create lifecycle harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join("lifecycle-records");
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join("lifecycle.stdout");
    let stderr_path = harness.path().join("lifecycle.stderr");
    let log_path = harness.path().join("events.jsonl");
    let plan_path = harness.path().join("approved.plan");
    let plan = b"approved plan bytes\0with exact stdin";
    let fixture = b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n";
    fs::write(&stdout_path, fixture).expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    fs::write(&plan_path, plan).expect("write plan");
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    environment.push(("PCE_CODEX_SLEEP_SECONDS".to_owned(), "0.2".to_owned()));
    let mut argv = dispatch_argv(&cwd, &environment, None, Some(&plan_path), "-");
    let delimiter = argv.len() - 2;
    argv.splice(
        delimiter..delimiter,
        [
            "--log-file".to_owned(),
            log_path.display().to_string(),
            "--node".to_owned(),
            "m3-s1".to_owned(),
            "--role".to_owned(),
            "step-executor".to_owned(),
            "--ref".to_owned(),
            "abc123".to_owned(),
            "--evidence".to_owned(),
            "fixture invocation".to_owned(),
        ],
    );
    let wall_started = SystemTime::now();
    let started = Instant::now();
    let output = harness.run(&argv, b"").expect("run lifecycle");
    let elapsed = started.elapsed();
    let wall_finished = SystemTime::now();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, fixture);
    let invocations = harness
        .codex_invocations(&record_root)
        .expect("read Codex invocation");
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].stdin, plan);
    let lines = fs::read_to_string(&log_path).expect("read lifecycle log");
    let records = lines
        .lines()
        .map(|line| parse_event_line(line).expect("parse record"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2);
    let lower = wall_started
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_millis() as i64;
    let upper = wall_finished
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_millis() as i64;
    let issued_at = records[0].timestamp().as_datetime().timestamp_millis();
    let completed_at = records[1].timestamp().as_datetime().timestamp_millis();
    assert!(issued_at >= lower && issued_at <= upper);
    assert!(completed_at >= lower && completed_at <= upper);
    assert!(completed_at >= issued_at);
    let EventBodyRef::Known(KnownPayload::Dispatch(issuance)) = records[0].body_ref() else {
        panic!("first record is not issuance")
    };
    assert_eq!(records[0].node().as_str(), "m3-s1");
    assert_eq!(issuance.role.as_str(), "step-executor");
    assert_eq!(issuance.r#ref.as_str(), "abc123");
    assert_eq!(issuance.evidence.as_str(), "fixture invocation");
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("second record is not completion")
    };
    let sleeping_duration_ms = completion.duration_ms.get();
    assert_eq!(completion.issuance_sequence, records[0].sequence());
    assert_eq!(
        completion.exit_status,
        DispatchExitStatus::Exited {
            code: pce_core::ExitCode::new(0)
        }
    );
    assert_eq!(completion.artifact_outcome, ArtifactOutcome::NotValidated);
    assert_eq!(
        completion.usage,
        CodexTokenUsage::Measured {
            input_tokens: pce_core::InputTokens::new(101),
            cached_input_tokens: pce_core::CachedInputTokens::new(23),
            output_tokens: pce_core::OutputTokens::new(17),
            reasoning_output_tokens: pce_core::ReasoningOutputTokens::new(5),
        }
    );
    let filtered = harness
        .run(
            [
                "log",
                "read",
                "--file",
                log_path.to_str().expect("log path"),
                "--kind",
                "dispatch-completion",
            ],
            b"",
        )
        .expect("read completion kind");
    assert!(filtered.status.success());
    assert_eq!(
        String::from_utf8(filtered.stdout).expect("UTF-8 read output"),
        format!("{}\n", lines.lines().nth(1).expect("completion line"))
    );

    let fast_record_root = harness.path().join("fast-lifecycle-records");
    fs::create_dir(&fast_record_root).expect("create fast records");
    let fast_log_path = harness.path().join("fast-events.jsonl");
    let fast_environment =
        child_environment(&harness, &fast_record_root, &stdout_path, &stderr_path, 0);
    let mut fast_argv = dispatch_argv(&cwd, &fast_environment, None, Some(&plan_path), "-");
    let delimiter = fast_argv.len() - 2;
    fast_argv.splice(
        delimiter..delimiter,
        [
            "--log-file".to_owned(),
            fast_log_path.display().to_string(),
            "--node".to_owned(),
            "m3-s1".to_owned(),
            "--role".to_owned(),
            "step-executor".to_owned(),
            "--ref".to_owned(),
            "abc123".to_owned(),
            "--evidence".to_owned(),
            "fixture invocation without sleep".to_owned(),
        ],
    );
    let fast_started = Instant::now();
    let fast_output = harness.run(&fast_argv, b"").expect("run fast lifecycle");
    let fast_elapsed = fast_started.elapsed();
    assert!(
        fast_output.status.success(),
        "{}",
        String::from_utf8_lossy(&fast_output.stderr)
    );
    let fast_records = fs::read_to_string(&fast_log_path)
        .expect("read fast lifecycle log")
        .lines()
        .map(|line| parse_event_line(line).expect("parse fast record"))
        .collect::<Vec<_>>();
    assert_eq!(fast_records.len(), 2);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(fast_completion)) =
        fast_records[1].body_ref()
    else {
        panic!("second fast record is not completion")
    };
    let fast_duration_ms = fast_completion.duration_ms.get();
    assert!(
        sleeping_duration_ms
            .checked_sub(fast_duration_ms)
            .is_some_and(|difference| difference >= 150),
        "sleeping duration was {sleeping_duration_ms} ms, fast duration was {fast_duration_ms} ms"
    );
    assert!(sleeping_duration_ms >= 100);
    assert!(Duration::from_millis(sleeping_duration_ms) <= elapsed);
    assert!(Duration::from_millis(fast_duration_ms) <= fast_elapsed);
}

#[test]
fn records_failed_and_absent_terminal_reasons_before_reporting_exit() {
    let _guard = dispatch_test_guard();
    for (name, fixture, code, reason) in [
        ("failed", b"{\"type\":\"turn.failed\"}\n".as_slice(), 41, UsageAbsenceReason::TurnFailed),
        ("absent", b"{}\n".as_slice(), 42, UsageAbsenceReason::NoTerminalTurn),
        ("malformed", b"not-json\n".as_slice(), 43, UsageAbsenceReason::MalformedTerminalData),
        ("duplicate", b"{\"type\":\"turn.failed\"}\n{\"type\":\"turn.failed\"}\n".as_slice(), 41, UsageAbsenceReason::DuplicateTerminalData),
        ("contradictory", b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n{\"type\":\"turn.failed\"}\n".as_slice(), 44, UsageAbsenceReason::ContradictoryTerminalData),
    ] {
        let harness = CliHarness::new().expect("create failure harness");
        let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
        let record_root = harness.path().join(format!("{name}-records"));
        fs::create_dir(&record_root).expect("create records");
        let stdout_path = harness.path().join(format!("{name}.stdout"));
        let stderr_path = harness.path().join(format!("{name}.stderr"));
        let log_path = harness.path().join(format!("{name}.jsonl"));
        fs::write(&stdout_path, fixture).expect("write stdout");
        fs::write(&stderr_path, []).expect("write stderr");
        let mut environment =
            child_environment(&harness, &record_root, &stdout_path, &stderr_path, code);
        environment.push(("PCE_CODEX_SKIP_RECORDING".to_owned(), "1".to_owned()));
        let mut argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
        let delimiter = argv.len() - 2;
        argv.splice(delimiter..delimiter, ["--log-file", log_path.to_str().expect("path"), "--node", "m3-s1", "--role", "step-executor", "--ref", "abc", "--evidence", "fixture"].map(str::to_owned));
        let started = Instant::now();
        let output = harness.run(&argv, b"").expect("run failed lifecycle");
        let elapsed = started.elapsed();
        assert!(!output.status.success());
        let records = fs::read_to_string(&log_path).expect("read log").lines().map(|line| parse_event_line(line).expect("parse")).collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref() else { panic!("missing completion") };
        assert_eq!(completion.usage, CodexTokenUsage::Absent { reason });
        assert_eq!(completion.exit_status, DispatchExitStatus::Exited { code: pce_core::ExitCode::new(code as u64) });
        assert!(Duration::from_millis(completion.duration_ms.get()) <= elapsed);
        if matches!(reason, UsageAbsenceReason::MalformedTerminalData | UsageAbsenceReason::DuplicateTerminalData | UsageAbsenceReason::ContradictoryTerminalData) {
            assert!(String::from_utf8_lossy(&output.stderr).contains(usage_reason_name(reason)));
        }
    }
}

#[test]
fn accepts_additive_usage_fields_and_blank_jsonl_lines() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create additive usage harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join("additive-usage-records");
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join("additive-usage.stdout");
    let stderr_path = harness.path().join("additive-usage.stderr");
    let log_path = harness.path().join("additive-usage.jsonl");
    let fixture = b"{\"type\":\"thread.started\"}\n \t\n{\"type\":\"turn.completed\",\"usage\":{\"total_tokens\":146,\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n";
    fs::write(&stdout_path, fixture).expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let mut argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let delimiter = argv.len() - 2;
    argv.splice(
        delimiter..delimiter,
        [
            "--log-file",
            log_path.to_str().expect("path"),
            "--node",
            "m3-s1",
            "--role",
            "step-executor",
            "--ref",
            "abc",
            "--evidence",
            "fixture",
        ]
        .map(str::to_owned),
    );

    let output = harness
        .run(&argv, b"")
        .expect("run additive usage lifecycle");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, fixture);
    let records = fs::read_to_string(&log_path)
        .expect("read log")
        .lines()
        .map(|line| parse_event_line(line).expect("parse"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("missing completion")
    };
    assert_eq!(
        completion.usage,
        CodexTokenUsage::Measured {
            input_tokens: pce_core::InputTokens::new(101),
            cached_input_tokens: pce_core::CachedInputTokens::new(23),
            output_tokens: pce_core::OutputTokens::new(17),
            reasoning_output_tokens: pce_core::ReasoningOutputTokens::new(5),
        }
    );
}

fn usage_reason_name(reason: UsageAbsenceReason) -> &'static str {
    match reason {
        UsageAbsenceReason::TurnFailed => "turn-failed",
        UsageAbsenceReason::NoTerminalTurn => "no-terminal-turn",
        UsageAbsenceReason::MalformedTerminalData => "malformed-terminal-data",
        UsageAbsenceReason::DuplicateTerminalData => "duplicate-terminal-data",
        UsageAbsenceReason::ContradictoryTerminalData => "contradictory-terminal-data",
    }
}

#[test]
fn records_signal_and_no_terminal_usage_without_fabricating_exit_zero() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create signal harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join("signal-records");
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join("signal.stdout");
    let stderr_path = harness.path().join("signal.stderr");
    let log_path = harness.path().join("signal.jsonl");
    fs::write(&stdout_path, b"{}\n").expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    environment.push(("PCE_CODEX_SIGNAL".to_owned(), "15".to_owned()));
    let mut argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let delimiter = argv.len() - 2;
    argv.splice(
        delimiter..delimiter,
        [
            "--log-file",
            log_path.to_str().expect("path"),
            "--node",
            "m3-s1",
            "--role",
            "step-executor",
            "--ref",
            "abc",
            "--evidence",
            "fixture",
        ]
        .map(str::to_owned),
    );
    let output = harness.run(&argv, b"").expect("run signal lifecycle");
    assert!(!output.status.success());
    let records = fs::read_to_string(&log_path)
        .expect("read log")
        .lines()
        .map(|line| parse_event_line(line).expect("parse"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("missing completion")
    };
    assert_eq!(
        completion.exit_status,
        DispatchExitStatus::Signaled {
            signal: pce_core::SignalNumber::new(15)
        }
    );
    assert_eq!(
        completion.usage,
        CodexTokenUsage::Absent {
            reason: UsageAbsenceReason::NoTerminalTurn
        }
    );
}

#[test]
fn releases_log_lock_while_child_runs_and_keeps_exact_issuance_identity() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create interleaving harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join("interleaving-records");
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join("interleaving.stdout");
    let stderr_path = harness.path().join("interleaving.stderr");
    let release_path = harness.path().join("release");
    let log_path = harness.path().join("interleaving.jsonl");
    fs::write(&stdout_path, b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n").expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    environment.push((
        "PCE_CODEX_BLOCK_FILE".to_owned(),
        release_path.display().to_string(),
    ));
    let mut argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let delimiter = argv.len() - 2;
    argv.splice(
        delimiter..delimiter,
        [
            "--log-file",
            log_path.to_str().expect("path"),
            "--node",
            "m3-s1",
            "--role",
            "step-executor",
            "--ref",
            "abc",
            "--evidence",
            "fixture",
        ]
        .map(str::to_owned),
    );
    let mut parent = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(&argv)
        .env_clear()
        .env("PATH", harness.shim_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn parent");
    wait_for_path(&record_root.join("invocation/request.bin"));
    let mut append = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "log",
            "--file",
            log_path.to_str().expect("log path"),
            "--kind",
            "delta",
            "--node",
            "m3-s1",
        ])
        .env_clear()
        .env("PATH", harness.shim_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn intervening append");
    append
        .stdin
        .take()
        .expect("append stdin")
        .write_all(br#"{"message":"intervening"}"#)
        .expect("write delta");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if append.try_wait().expect("poll append").is_some() {
            break;
        }
        if Instant::now() >= deadline {
            append.kill().expect("kill blocked append");
            fs::write(&release_path, []).expect("release child during timeout cleanup");
            parent.kill().expect("kill parent during timeout cleanup");
            parent.wait().expect("reap parent during timeout cleanup");
            panic!("intervening append remained blocked, indicating a dispatch lock leak");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let append = append.wait_with_output().expect("collect append");
    assert!(
        append.status.success(),
        "intervening append failed: {}",
        String::from_utf8_lossy(&append.stderr)
    );
    fs::write(&release_path, []).expect("release child");
    let output = parent.wait_with_output().expect("collect parent");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let records = fs::read_to_string(&log_path)
        .expect("read log")
        .lines()
        .map(|line| parse_event_line(line).expect("parse"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 3);
    assert!(matches!(
        records[0].body_ref(),
        EventBodyRef::Known(KnownPayload::Dispatch(_))
    ));
    assert!(matches!(
        records[1].body_ref(),
        EventBodyRef::Known(KnownPayload::Delta(_))
    ));
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[2].body_ref()
    else {
        panic!("completion not last")
    };
    assert_eq!(completion.issuance_sequence, records[0].sequence());
}

fn wait_for_path(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn interruption_leaves_only_durable_issuance() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create interruption harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join("interruption-records");
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join("interruption.stdout");
    let stderr_path = harness.path().join("interruption.stderr");
    let release_path = harness.path().join("never-release");
    let log_path = harness.path().join("interruption.jsonl");
    fs::write(&stdout_path, b"{}\n").expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 42);
    environment.push((
        "PCE_CODEX_BLOCK_FILE".to_owned(),
        release_path.display().to_string(),
    ));
    let mut argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let delimiter = argv.len() - 2;
    argv.splice(
        delimiter..delimiter,
        [
            "--log-file",
            log_path.to_str().expect("path"),
            "--node",
            "m3-s1",
            "--role",
            "step-executor",
            "--ref",
            "abc",
            "--evidence",
            "fixture",
        ]
        .map(str::to_owned),
    );
    let mut parent = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(&argv)
        .env_clear()
        .env("PATH", harness.shim_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn parent");
    wait_for_path(&record_root.join("invocation/request.bin"));
    wait_for_path(&record_root.join("invocation/pid"));
    let before = fs::read_to_string(&log_path).expect("read issuance");
    assert_eq!(before.lines().count(), 1);
    assert!(matches!(
        parse_event_line(before.trim_end())
            .expect("parse issuance")
            .body_ref(),
        EventBodyRef::Known(KnownPayload::Dispatch(_))
    ));
    parent.kill().expect("kill parent");
    parent.wait().expect("reap parent");
    let shim_pid = fs::read_to_string(record_root.join("invocation/pid")).expect("read shim pid");
    let killed = Command::new("/bin/kill")
        .args(["-TERM", shim_pid.trim()])
        .status()
        .expect("signal shim");
    assert!(killed.success());
    let final_log = fs::read_to_string(&log_path).expect("read final log");
    assert_eq!(final_log.lines().count(), 1);
}

#[test]
fn propagates_nonzero_codex_status() {
    let _guard = dispatch_test_guard();
    let (harness, cwd, record_root, stdin_path, environment) = failure_fixture("nonzero", 37);
    let argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let output = harness
        .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
        .expect("run nonzero dispatch");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exited with status"));
    assert_eq!(
        harness
            .codex_invocations(&record_root)
            .expect("read record")
            .len(),
        1
    );
}

#[test]
fn reports_codex_spawn_failure() {
    let _guard = dispatch_test_guard();
    let (harness, cwd, record_root, stdin_path, mut environment) = failure_fixture("spawn", 0);
    environment
        .iter_mut()
        .find(|entry| entry.0 == "PATH")
        .expect("PATH entry")
        .1 = "/usr/bin:/bin".to_owned();
    let argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let output = harness
        .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
        .expect("run missing Codex dispatch");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to spawn `codex`"));
    assert!(
        harness
            .codex_invocations(&record_root)
            .expect("read records")
            .is_empty()
    );
}

fn failure_fixture(
    name: &str,
    exit_code: i32,
) -> (CliHarness, PathBuf, PathBuf, PathBuf, Vec<(String, String)>) {
    let harness = CliHarness::new().expect("create harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join(format!("{name}-records"));
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join(format!("{name}.stdout"));
    let stderr_path = harness.path().join(format!("{name}.stderr"));
    let stdin_path = harness.path().join(format!("{name}.stdin"));
    fs::write(&stdout_path, []).expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    fs::write(&stdin_path, b"parent sentinel").expect("write stdin");
    let environment = child_environment(
        &harness,
        &record_root,
        &stdout_path,
        &stderr_path,
        exit_code,
    );
    (harness, cwd, record_root, stdin_path, environment)
}

#[test]
fn rejects_invalid_dispatch_inputs_before_invocation() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let stdin_path = harness.path().join("parser.stdin");
    fs::write(&stdin_path, b"sentinel").expect("write stdin");
    let absolute = cwd.display().to_string();
    let cases = [
        (
            "relative-cwd",
            "relative",
            "workspace-write",
            vec![],
            "working directory must be absolute",
        ),
        (
            "relative-schema",
            &absolute,
            "workspace-write",
            vec!["--output-schema", "relative", "-o", &absolute],
            "schema path must be absolute",
        ),
        (
            "relative-output",
            &absolute,
            "workspace-write",
            vec!["--output-schema", &absolute, "-o", "relative"],
            "output path must be absolute",
        ),
        (
            "unpaired-schema",
            &absolute,
            "workspace-write",
            vec!["--output-schema", &absolute],
            "expected `-o`",
        ),
        (
            "malformed-env",
            &absolute,
            "workspace-write",
            vec!["--env", "malformed"],
            "environment entry must contain `=`",
        ),
        (
            "duplicate-env",
            &absolute,
            "workspace-write",
            vec!["--env", "A=1", "--env", "A=2"],
            "duplicate environment name `A`",
        ),
        (
            "unknown-sandbox",
            &absolute,
            "unknown",
            vec![],
            "unsupported sandbox `unknown`",
        ),
        (
            "empty-env-name",
            &absolute,
            "workspace-write",
            vec!["--env", "=value"],
            "environment name must not be empty",
        ),
    ];
    for (name, raw_cwd, sandbox, extra_options, expected_stderr) in cases {
        let record_root = harness.path().join(format!("{name}-records"));
        fs::create_dir(&record_root).expect("create parser record root");
        let stdout_path = harness.path().join(format!("{name}.stdout"));
        let stderr_path = harness.path().join(format!("{name}.stderr"));
        fs::write(&stdout_path, []).expect("write parser fixture stdout");
        fs::write(&stderr_path, []).expect("write parser fixture stderr");
        let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
        let mut argv = vec![
            "dispatch".to_owned(),
            "codex".to_owned(),
            "--cwd".to_owned(),
            raw_cwd.to_owned(),
            "--sandbox".to_owned(),
            sandbox.to_owned(),
        ];
        for (name, value) in &environment {
            argv.extend(["--env".to_owned(), format!("{name}={value}")]);
        }
        argv.extend(extra_options.into_iter().map(str::to_owned));
        argv.push("--".to_owned());
        let output = harness
            .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
            .expect("run parser case");
        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_stderr),
            "{name} stderr did not contain {expected_stderr:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            harness
                .codex_invocations(&record_root)
                .expect("read parser records")
                .is_empty(),
            "{name} invoked Codex"
        );
    }
    let missing_plan = harness.path().join("missing.plan");
    let record_root = harness.path().join("missing-plan-records");
    fs::create_dir(&record_root).expect("create missing-plan record root");
    let stdout_path = harness.path().join("missing-plan.stdout");
    let stderr_path = harness.path().join("missing-plan.stderr");
    fs::write(&stdout_path, []).expect("write missing-plan fixture stdout");
    fs::write(&stderr_path, []).expect("write missing-plan fixture stderr");
    let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let argv = vec![
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        absolute,
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
    ]
    .into_iter()
    .chain(
        environment
            .iter()
            .flat_map(|(name, value)| ["--env".to_owned(), format!("{name}={value}")]),
    )
    .chain([
        "--plan-file".to_owned(),
        missing_plan.display().to_string(),
        "--".to_owned(),
        "-".to_owned(),
    ])
    .collect::<Vec<_>>();
    let output = harness
        .run_with_stdin_file(argv, &stdin_path, INHERITED_MARKER)
        .expect("run unreadable plan");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(missing_plan.to_string_lossy().as_ref())
    );
    assert!(
        harness
            .codex_invocations(&record_root)
            .expect("read missing-plan records")
            .is_empty()
    );
}
