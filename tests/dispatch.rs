#[allow(dead_code)]
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pce_core::{
    AbsoluteOutputPath, AbsoluteRequiredArtifactPath, AbsoluteSchemaPath, AbsoluteWorkingDirectory,
    ArgumentVector, ArtifactOutcome, ChildEnvironment, Deferred, DispatchDuration,
    DispatchEnvelope, DispatchExitStatus, DispatchLogging, DispatchProcessIdentity, DispatchRef,
    DispatchRole, DispatchTarget, DispatchTokenUsage, EventBodyRef, EventRecord, EventTimestamp,
    Evidence, KnownPayload, NodeId, Sandbox, Sequence, StdinBinding, UsageAbsenceReason, WriteKind,
    dispatch_invocation, dispatch_payload, parse_dispatch_process_identity, parse_event_line,
    serialize_event_line,
};
use serde::Serialize;
use serde_json::{Value, json};
use support::{ClaudeInvocation, CliHarness, CodexInvocation};

const INHERITED_MARKER: (&str, &str) = ("PCE_INHERITED_ONLY", "must-not-reach-codex");
const CHILD_MARKER: (&str, &str) = ("PCE_CHILD_MARKER", "explicit-child-value");
const VALID_ARTIFACT_SCHEMA: &[u8] = br#"{
  "type": "object",
  "required": ["verdict", "summary"],
  "properties": {
    "verdict": { "type": "string" },
    "summary": { "type": "string" },
    "nested": {
      "type": "object",
      "properties": { "count": { "type": "integer" } }
    }
  }
}"#;
const CONFORMING_ARTIFACT: &[u8] = br#"{"verdict":"pass","summary":"ok"}"#;
const SUCCESSFUL_STRUCTURED_TRANSCRIPT: &[u8] = b"{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"{\\\"verdict\\\":\\\"SUCCESS\\\",\\\"summary\\\":\\\"transcript says success\\\"}\"}}\n{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n";
const DETACHED_SUCCESS_TRANSCRIPT: &[u8] = b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n";
static DISPATCH_TEST_LOCK: Mutex<()> = Mutex::new(());
const REPEATABLE_PLANNING_FRAME: &str = "Plan the step.\n\n## Binary-owned reversibility obligation\n\nThe step's act is repeatable. The plan must retain an explicit not-touched scope fence and exact expected values for every assertion. The plan must not contain a pre-derived argument that the design is correct.";
const IRREVERSIBLE_PLANNING_FRAME: &str = "Critique the plan.\n\n## Binary-owned reversibility obligation\n\nThe step's act cannot be repeated. The plan must retain the existing front-loaded pre-proof of correctness, an explicit not-touched scope fence, and exact expected values for every assertion.";

fn dispatch_test_guard() -> MutexGuard<'static, ()> {
    DISPATCH_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn dispatch_sidecar(log_path: &Path, sequence: u64, extension: &str) -> PathBuf {
    PathBuf::from(format!(
        "{}.dispatch-{sequence:06}.{extension}",
        log_path.display()
    ))
}

fn wait_for_lifecycle_records(
    log_path: &Path,
    expected_count: usize,
    deadline: Instant,
) -> Vec<EventRecord> {
    loop {
        if let Ok(contents) = fs::read_to_string(log_path) {
            let records = contents
                .lines()
                .map(parse_event_line)
                .collect::<Result<Vec<_>, _>>();
            if let Ok(records) = records
                && records.len() == expected_count
            {
                return records;
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {expected_count} lifecycle records in {}",
            log_path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn settle_lifecycle(log_path: &Path) -> Vec<EventRecord> {
    wait_for_lifecycle_records(log_path, 2, Instant::now() + Duration::from_secs(15))
}

fn wait_for_file_bytes(path: &Path, predicate: impl Fn(&[u8]) -> bool) -> Vec<u8> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(bytes) = fs::read(path)
            && predicate(&bytes)
        {
            return bytes;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for complete bytes in {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn codex_planning_frame_is_exact_in_dry_run_and_live_child() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create Codex planning harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join("planning-records");
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join("planning.stdout");
    let stderr_path = harness.path().join("planning.stderr");
    fs::write(&stdout_path, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let log_path = harness.path().join("planning.jsonl");
    let required_artifact = harness.path().join("plan.md");
    let caller = vec!["--caller-option".to_owned(), "Plan the step.".to_owned()];
    let mut dry = dispatch_argv(&cwd, &environment, None, None, caller.as_slice());
    let delimiter = dry
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    dry.splice(
        delimiter..delimiter,
        [
            "--log-file",
            log_path.to_str().expect("log path"),
            "--node",
            "m5-s1",
            "--role",
            "step-plan-writer",
            "--ref",
            "fixture-ref",
            "--evidence",
            "fixture-evidence",
            "--required-artifact",
            required_artifact.to_str().expect("required artifact"),
            "--planning-act",
            "repeatable",
            "--dry-run",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    let projected = harness.run(&dry, b"").expect("project planning dispatch");
    assert!(
        projected.status.success(),
        "{}",
        String::from_utf8_lossy(&projected.stderr)
    );
    let projection: Value = serde_json::from_slice(&projected.stdout).expect("projection JSON");
    assert_eq!(
        projection["envelope"]["argv"]
            .as_array()
            .expect("argv")
            .last(),
        Some(&json!(REPEATABLE_PLANNING_FRAME))
    );
    assert_eq!(
        projection["envelope"]["stdin"],
        json!({"binding":"null","bytes":null})
    );
    assert_eq!(
        projection["issuance"]["payload"],
        json!({"role":"step-plan-writer","ref":"fixture-ref","evidence":"fixture-evidence"})
    );
    assert!(!log_path.exists());
    assert!(
        harness
            .codex_invocations(&record_root)
            .expect("dry invocations")
            .is_empty()
    );

    let mut live = dry.clone();
    live.retain(|value| value != "--dry-run");
    let output = harness.run(&live, b"").expect("run planning dispatch");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log_path);
    wait_for_path(&record_root.join("invocation/pid"));
    let invocation = harness
        .codex_invocations(&record_root)
        .expect("live invocation")
        .remove(0);
    let captured = invocation
        .argv
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(projection["envelope"]["argv"], json!(captured));
    assert_eq!(
        captured.last().map(String::as_str),
        Some(REPEATABLE_PLANNING_FRAME)
    );
    assert_eq!(invocation.stdin, b"");
}

#[test]
fn planning_roles_without_planning_act_preserve_caller_arguments_exactly() {
    let _guard = dispatch_test_guard();
    for role in ["step-plan-writer", "step-plan-critic"] {
        let harness = CliHarness::new().expect("create preservation harness");
        let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
        let record_root = harness.path().join(format!("{role}-records"));
        fs::create_dir(&record_root).expect("create records");
        let stdout_path = harness.path().join(format!("{role}.stdout"));
        let stderr_path = harness.path().join(format!("{role}.stderr"));
        fs::write(&stdout_path, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("write stdout");
        fs::write(&stderr_path, []).expect("write stderr");
        let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
        let log_path = harness.path().join(format!("{role}.jsonl"));
        let required_artifact = harness.path().join(format!("{role}-plan.md"));
        let caller = vec!["--caller-option".to_owned(), "Plan the step.".to_owned()];
        let mut argv = dispatch_argv(&cwd, &environment, None, None, caller.as_slice());
        let delimiter = argv
            .iter()
            .position(|value| value == "--")
            .expect("delimiter");
        argv.splice(
            delimiter..delimiter,
            [
                "--log-file",
                log_path.to_str().expect("log path"),
                "--node",
                "m5-s1",
                "--role",
                role,
                "--ref",
                "fixture-ref",
                "--evidence",
                "fixture-evidence",
                "--required-artifact",
                required_artifact.to_str().expect("required artifact"),
                "--dry-run",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        let projected = harness.run(&argv, b"").expect("project unframed dispatch");
        assert!(projected.status.success(), "{role}");
        let projection: Value = serde_json::from_slice(&projected.stdout).expect("projection JSON");
        let projected_argv = projection["envelope"]["argv"]
            .as_array()
            .expect("argv")
            .clone();
        assert_eq!(
            &projected_argv[projected_argv.len() - 2..],
            [json!("--caller-option"), json!("Plan the step.")],
            "{role}"
        );
    }
}

#[test]
fn planning_frame_rejections_have_no_dispatch_side_effects() {
    let _guard = dispatch_test_guard();
    for (name, role, act, caller, with_logging, expected) in [
        (
            "act",
            "step-plan-writer",
            "destructive",
            Some("Plan the step."),
            true,
            "unsupported planning act `destructive`; expected `repeatable` or `irreversible`",
        ),
        (
            "logging",
            "step-plan-writer",
            "repeatable",
            Some("Plan the step."),
            false,
            "`--planning-act` requires complete dispatch logging metadata",
        ),
        (
            "role",
            "step-executor",
            "repeatable",
            Some("Plan the step."),
            true,
            "planning act is supported only for roles `step-plan-writer` and `step-plan-critic`; rejected role `step-executor`",
        ),
        (
            "missing",
            "step-plan-writer",
            "repeatable",
            None,
            true,
            "planning role `step-plan-writer` requires a non-empty final caller argument to carry its binary-owned frame",
        ),
        (
            "empty",
            "step-plan-writer",
            "repeatable",
            Some(""),
            true,
            "planning role `step-plan-writer` requires a non-empty final caller argument to carry its binary-owned frame",
        ),
    ] {
        let harness = CliHarness::new().expect("create rejection harness");
        let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
        let record_root = harness.path().join(format!("{name}-records"));
        fs::create_dir(&record_root).expect("create records");
        let stdout_path = harness.path().join(format!("{name}.stdout"));
        let stderr_path = harness.path().join(format!("{name}.stderr"));
        fs::write(&stdout_path, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("write stdout");
        fs::write(&stderr_path, []).expect("write stderr");
        let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
        let log_path = harness.path().join(format!("{name}.jsonl"));
        let required_artifact = harness.path().join(format!("{name}-plan.md"));
        let mut argv = dispatch_argv(&cwd, &environment, None, None, &[] as &[String]);
        let delimiter = argv
            .iter()
            .position(|value| value == "--")
            .expect("delimiter");
        let mut options = Vec::new();
        if with_logging {
            options.extend([
                "--log-file".to_owned(),
                log_path.display().to_string(),
                "--node".to_owned(),
                "m5-s1".to_owned(),
                "--role".to_owned(),
                role.to_owned(),
                "--ref".to_owned(),
                "fixture-ref".to_owned(),
                "--evidence".to_owned(),
                "fixture-evidence".to_owned(),
                "--required-artifact".to_owned(),
                required_artifact.display().to_string(),
            ]);
        }
        options.extend(["--planning-act".to_owned(), act.to_owned()]);
        argv.splice(delimiter..delimiter, options);
        if let Some(caller) = caller {
            argv.push(caller.to_owned());
        }
        let output = harness.run(&argv, b"").expect("run rejection");
        assert!(!output.status.success(), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(&log_path).unwrap_or_default(), b"", "{name}");
        assert!(
            harness
                .codex_invocations(&record_root)
                .expect("invocations")
                .is_empty(),
            "{name}"
        );
    }
}

#[derive(Serialize)]
struct FixtureProjection {
    envelope: FixtureEnvelope,
    issuance: FixtureIssuance,
    completion: FixtureCompletion,
}

#[derive(Serialize)]
struct FixtureEnvelope {
    target: &'static str,
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
    required_artifact_presence: FixtureDeferred,
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
    if structured {
        fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write structured schema");
        fs::write(&output_path, CONFORMING_ARTIFACT).expect("write structured artifact");
    }
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
    dry_argv.splice(
        delimiter..delimiter,
        logging_arguments(&log_path, &output_path),
    );
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
    assert!(
        !harness
            .path()
            .join(format!("{name}.jsonl.dispatches"))
            .exists()
    );
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
    settle_lifecycle(&log_path);
    wait_for_path(&record_root.join("invocation/pid"));
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
    assert_environment_with_synthesized_tmpdir(&invocations[0].environment, &projected_environment);
    assert!(
        !invocations[0].environment.contains(&OsString::from(format!(
            "{}={}",
            INHERITED_MARKER.0, INHERITED_MARKER.1
        )))
    );
}

fn assert_dry_parser_and_tail_failures() {
    const DIAGNOSTIC: &str = "dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence, --required-artifact";
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
    let required_artifact = harness.path().join("parser-result.json");
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
    argv.extend(logging_arguments(&unreadable_log, &required_artifact));
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
        argv.extend(logging_arguments(&log_path, &required_artifact));
        argv.extend(["--dry-run".to_owned(), "--".to_owned(), "PROMPT".to_owned()]);
        let output = harness.run(&argv, b"").expect("run invalid-tail projection");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
        assert!(output.stdout.is_empty());
        assert_eq!(harness.codex_invocations(&record_root).expect("tail invocations"), Vec::new());
    }

    let missing_log = harness.path().join("missing.jsonl");
    let mut argv = prefix;
    argv.extend(logging_arguments(&missing_log, &required_artifact));
    argv.extend(["--dry-run".to_owned(), "--".to_owned(), "PROMPT".to_owned()]);
    let output = harness.run(&argv, b"").expect("run missing-log projection");
    assert!(output.status.success());
    assert!(!missing_log.exists());
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
}

fn logging_arguments(log_path: &Path, required_artifact: &Path) -> Vec<String> {
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
        "--required-artifact".to_owned(),
        required_artifact.display().to_string(),
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
            target: "codex",
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
                required_artifact_presence: deferred,
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
        DispatchTarget::Codex,
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
        required_artifact_path: AbsoluteRequiredArtifactPath::parse("/workspace/result.json")
            .expect("required artifact path"),
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
    let _: Deferred<DispatchTokenUsage> =
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
    if case.structured {
        fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write structured schema");
        fs::write(&output_path, CONFORMING_ARTIFACT).expect("write structured artifact");
    }
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

#[derive(Clone, Copy)]
enum StructuredRejectionFixture {
    Missing,
    Truncated,
    SchemaInvalid,
    SchemaViolating,
    UnreadableSchema,
}

#[test]
fn rejects_logged_structured_artifacts_from_successful_children() {
    let _guard = dispatch_test_guard();
    for fixture in [
        StructuredRejectionFixture::Missing,
        StructuredRejectionFixture::Truncated,
        StructuredRejectionFixture::SchemaInvalid,
        StructuredRejectionFixture::SchemaViolating,
        StructuredRejectionFixture::UnreadableSchema,
    ] {
        assert_logged_structured_rejection(fixture);
    }
}

#[test]
fn rejects_logged_missing_artifact() {
    assert_logged_structured_rejection(StructuredRejectionFixture::Missing);
}

#[test]
fn rejects_logged_truncated_artifact() {
    assert_logged_structured_rejection(StructuredRejectionFixture::Truncated);
}

#[test]
fn rejects_logged_invalid_schema() {
    assert_logged_structured_rejection(StructuredRejectionFixture::SchemaInvalid);
}

#[test]
fn rejects_logged_schema_violations() {
    assert_logged_structured_rejection(StructuredRejectionFixture::SchemaViolating);
}

#[test]
fn rejects_logged_unreadable_schema() {
    assert_logged_structured_rejection(StructuredRejectionFixture::UnreadableSchema);
}

fn assert_logged_structured_rejection(fixture: StructuredRejectionFixture) {
    let name = match fixture {
        StructuredRejectionFixture::Missing => "missing",
        StructuredRejectionFixture::Truncated => "truncated",
        StructuredRejectionFixture::SchemaInvalid => "schema-invalid",
        StructuredRejectionFixture::SchemaViolating => "schema-violating",
        StructuredRejectionFixture::UnreadableSchema => "unreadable-schema",
    };
    let harness = CliHarness::new().expect("create rejection harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize rejection cwd");
    let record_root = harness.path().join(format!("{name}-records"));
    fs::create_dir(&record_root).expect("create rejection record root");
    let stdout_path = harness.path().join(format!("{name}.stdout"));
    let stderr_path = harness.path().join(format!("{name}.stderr"));
    let log_path = harness.path().join(format!("{name}.jsonl"));
    let schema_path = cwd.join(format!("{name}-schema.json"));
    let output_path = cwd.join(format!("{name}-output.json"));
    let bytes_path = harness.path().join(format!("{name}-bytes"));
    fs::write(&stdout_path, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("write transcript");
    fs::write(&stderr_path, []).expect("write empty child stderr");
    let (expected_outcome, spelling, diagnostic, artifact): (_, _, _, Option<&[u8]>) = match fixture
    {
        StructuredRejectionFixture::Missing => {
            fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write valid schema");
            (
                ArtifactOutcome::Missing,
                "missing",
                format!("artifact output `{}` is missing", output_path.display()),
                None,
            )
        }
        StructuredRejectionFixture::Truncated => {
            fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write valid schema");
            (
                ArtifactOutcome::Truncated,
                "truncated",
                format!(
                    "artifact output `{}` is not complete valid JSON: EOF while parsing an object at line 1 column 1",
                    output_path.display()
                ),
                Some(b"{"),
            )
        }
        StructuredRejectionFixture::SchemaInvalid => {
            fs::write(&schema_path, br#"{"type":5}"#).expect("write invalid schema");
            (
                ArtifactOutcome::SchemaInvalid,
                "schema-invalid",
                format!(
                    "artifact schema `{}` cannot be compiled: 5 is not valid under any of the schemas listed in the 'anyOf' keyword",
                    schema_path.display()
                ),
                Some(CONFORMING_ARTIFACT),
            )
        }
        StructuredRejectionFixture::SchemaViolating => {
            fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write valid schema");
            let first = format!(
                "artifact output `{}` violates schema keyword/location `required` at instance `<root>`: \"verdict\" is a required property",
                output_path.display()
            );
            let second = format!(
                "artifact output `{}` violates schema keyword/location `type` at instance `/nested/count`: \"not-an-int\" is not of type \"integer\"",
                output_path.display()
            );
            (
                ArtifactOutcome::SchemaViolating,
                "schema-violating",
                format!("{first}; {second}"),
                Some(br#"{"summary":"ok","nested":{"count":"not-an-int"}}"#),
            )
        }
        StructuredRejectionFixture::UnreadableSchema => {
            fs::create_dir(&schema_path).expect("create schema directory");
            let detail = fs::read(&schema_path)
                .expect_err("directory read must fail")
                .to_string();
            (
                ArtifactOutcome::SchemaInvalid,
                "schema-invalid",
                format!(
                    "artifact schema `{}` is unreadable: {detail}",
                    schema_path.display()
                ),
                Some(CONFORMING_ARTIFACT),
            )
        }
    };
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    if let Some(bytes) = artifact {
        fs::write(&bytes_path, bytes).expect("write artifact byte fixture");
        environment.push((
            "PCE_CODEX_OUTPUT_BYTES_FILE".to_owned(),
            bytes_path.display().to_string(),
        ));
    }
    let mut argv = dispatch_argv(
        &cwd,
        &environment,
        Some((&schema_path, &output_path)),
        None,
        "STRUCTURED_REJECTION",
    );
    let delimiter = argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    argv.splice(
        delimiter..delimiter,
        logging_arguments(&log_path, &output_path),
    );
    let output = harness.run(&argv, b"").expect("run rejection");
    assert!(output.status.success(), "{name} starter failed");
    settle_lifecycle(&log_path);
    assert_eq!(
        wait_for_file_bytes(&dispatch_sidecar(&log_path, 1, "stderr"), |bytes| {
            bytes == format!("Error: {diagnostic}\n").as_bytes()
        }),
        format!("Error: {diagnostic}\n").as_bytes(),
        "{name} exact diagnostic"
    );
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    assert_eq!(
        fs::read(dispatch_sidecar(&log_path, 1, "stdout")).expect("read transcript sidecar"),
        SUCCESSFUL_STRUCTURED_TRANSCRIPT
    );
    let lines = fs::read_to_string(&log_path).expect("read rejection lifecycle");
    let records = lines
        .lines()
        .map(|line| parse_event_line(line).expect("parse lifecycle record"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2, "{name} record count");
    assert!(matches!(
        records[0].body_ref(),
        EventBodyRef::Known(KnownPayload::Dispatch(_))
    ));
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("{name} completion expected");
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
    };
    assert_eq!(
        completion.issuance_sequence,
        records[0].sequence(),
        "{name} correlation"
    );
    assert_eq!(completion.usage, measured_usage(), "{name} measured usage");
    assert_eq!(
        completion.exit_status,
        DispatchExitStatus::Exited {
            code: pce_core::ExitCode::new(0)
        },
        "{name} zero child exit"
    );
    assert_eq!(
        completion.artifact_outcome, expected_outcome,
        "{name} typed outcome"
    );
    let completion_json: serde_json::Value =
        serde_json::from_str(lines.lines().nth(1).expect("completion line"))
            .expect("parse completion JSON");
    assert_eq!(
        completion_json["payload"]["artifact_outcome"], spelling,
        "{name} outcome spelling"
    );
    if let Some(bytes) = artifact {
        assert_eq!(
            fs::read(&output_path).expect("shim-created artifact"),
            bytes
        );
    }
}

fn measured_usage() -> DispatchTokenUsage {
    DispatchTokenUsage::Measured {
        input_tokens: pce_core::InputTokens::new(101),
        cached_input_tokens: pce_core::CachedInputTokens::new(23),
        output_tokens: pce_core::OutputTokens::new(17),
        reasoning_output_tokens: pce_core::ReasoningOutputTokens::new(5),
    }
}

#[test]
fn accepts_valid_structured_artifact_with_transcript_verdict() {
    assert_valid_structured_artifact(SUCCESSFUL_STRUCTURED_TRANSCRIPT);
}

#[test]
fn accepts_valid_structured_artifact_without_transcript_verdict() {
    assert_valid_structured_artifact(b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n");
}

fn assert_valid_structured_artifact(transcript: &[u8]) {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create positive harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize positive cwd");
    let record_root = harness.path().join("positive-records");
    fs::create_dir(&record_root).expect("create positive record root");
    let stdout_path = harness.path().join("positive.stdout");
    let stderr_path = harness.path().join("positive.stderr");
    let bytes_path = harness.path().join("positive-bytes");
    let schema_path = cwd.join("positive-schema.json");
    let output_path = cwd.join("positive-output.json");
    let log_path = harness.path().join("positive.jsonl");
    fs::write(&stdout_path, transcript).expect("write positive transcript");
    fs::write(&stderr_path, []).expect("write empty child stderr");
    fs::write(&bytes_path, CONFORMING_ARTIFACT).expect("write artifact fixture");
    fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write schema");
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    environment.push((
        "PCE_CODEX_OUTPUT_BYTES_FILE".to_owned(),
        bytes_path.display().to_string(),
    ));
    let mut argv = dispatch_argv(
        &cwd,
        &environment,
        Some((&schema_path, &output_path)),
        None,
        "STRUCTURED_POSITIVE",
    );
    let delimiter = argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    argv.splice(
        delimiter..delimiter,
        logging_arguments(&log_path, &output_path),
    );
    let output = harness.run(&argv, b"").expect("run positive dispatch");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"");
    assert_eq!(output.stdout, b"");
    settle_lifecycle(&log_path);
    assert_eq!(
        fs::read(&output_path).expect("shim artifact"),
        CONFORMING_ARTIFACT
    );
    let lines = fs::read_to_string(&log_path).expect("read positive lifecycle");
    let records = lines
        .lines()
        .map(|line| parse_event_line(line).expect("parse positive record"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("positive completion expected");
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
    };
    assert_eq!(completion.issuance_sequence, records[0].sequence());
    assert_eq!(completion.artifact_outcome, ArtifactOutcome::Validated);
    assert_eq!(completion.usage, measured_usage());
    assert_eq!(
        completion.exit_status,
        DispatchExitStatus::Exited {
            code: pce_core::ExitCode::new(0)
        }
    );
    let completion_json: serde_json::Value =
        serde_json::from_str(lines.lines().nth(1).expect("completion line"))
            .expect("parse completion JSON");
    assert_eq!(completion_json["payload"]["artifact_outcome"], "validated");
}

#[test]
fn rejects_missing_structured_artifact_without_logging() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create no-log harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize no-log cwd");
    let record_root = harness.path().join("no-log-records");
    fs::create_dir(&record_root).expect("create no-log record root");
    let stdout_path = harness.path().join("no-log.stdout");
    let stderr_path = harness.path().join("no-log.stderr");
    let schema_path = cwd.join("no-log-schema.json");
    let output_path = cwd.join("no-log-output.json");
    fs::write(&stdout_path, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("write no-log transcript");
    fs::write(&stderr_path, []).expect("write empty child stderr");
    fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write no-log schema");
    let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let argv = dispatch_argv(
        &cwd,
        &environment,
        Some((&schema_path, &output_path)),
        None,
        "NO_LOG_MISSING",
    );
    let output = harness.run(&argv, b"").expect("run no-log rejection");
    assert!(!output.status.success());
    assert_eq!(
        output.stderr,
        format!(
            "Error: artifact output `{}` is missing\n",
            output_path.display()
        )
        .as_bytes()
    );
    assert_no_jsonl_files(harness.path());
}

fn assert_no_jsonl_files(root: &Path) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read harness directory") {
            let entry = entry.expect("read harness entry");
            let path = entry.path();
            if path.is_dir() {
                assert!(
                    !path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.ends_with(".dispatches")),
                    "logging-absent dispatch created {}",
                    path.display()
                );
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

fn assert_environment_with_synthesized_tmpdir(
    actual: &BTreeSet<OsString>,
    expected: &BTreeSet<OsString>,
) {
    let tmp_entries = actual
        .iter()
        .filter(|entry| entry.to_string_lossy().starts_with("TMPDIR="))
        .collect::<Vec<_>>();
    assert_eq!(tmp_entries.len(), 1, "one binary-owned TMPDIR is required");
    let mut forwarded = actual.clone();
    forwarded.remove(tmp_entries[0]);
    assert_eq!(&forwarded, expected);
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
    assert_environment_with_synthesized_tmpdir(&invocation.environment, &expected_environment);
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
            "--required-artifact".to_owned(),
            plan_path.display().to_string(),
        ],
    );
    let wall_started = SystemTime::now();
    let started = Instant::now();
    let output = harness.run(&argv, b"").expect("run lifecycle");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log_path);
    let elapsed = started.elapsed();
    let wall_finished = SystemTime::now();
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    assert_eq!(
        fs::read(dispatch_sidecar(&log_path, 1, "stdout")).expect("read lifecycle stdout"),
        fixture
    );
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
    assert_eq!(lines.as_bytes().last(), Some(&b'\n'));
    assert_eq!(
        lines
            .as_bytes()
            .iter()
            .filter(|byte| **byte == b'\n')
            .count(),
        2
    );
    for forbidden in [
        "process_number",
        "process_start_identity",
        "required_artifact_path",
        "pce.dispatch-process-identity",
    ] {
        assert!(!lines.contains(forbidden), "event log leaked {forbidden}");
    }
    let mut normalized = lines
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event JSON"))
        .collect::<Vec<_>>();
    for event in &mut normalized {
        let object = event.as_object_mut().expect("event object");
        object.remove("sequence");
        object.remove("timestamp");
    }
    normalized[1]["payload"]
        .as_object_mut()
        .expect("completion payload")
        .remove("duration_ms");
    assert_eq!(
        normalized,
        vec![
            json!({"kind":"dispatch","node":"m3-s1","payload":{"role":"step-executor","ref":"abc123","evidence":"fixture invocation"}}),
            json!({"kind":"dispatch-completion","node":"m3-s1","payload":{"issuance_sequence":1,"usage":{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated","required_artifact_presence":"present"}}),
        ]
    );
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
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
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
        DispatchTokenUsage::Measured {
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
            "--required-artifact".to_owned(),
            plan_path.display().to_string(),
        ],
    );
    let fast_started = Instant::now();
    let fast_output = harness.run(&fast_argv, b"").expect("run fast lifecycle");
    assert!(
        fast_output.status.success(),
        "{}",
        String::from_utf8_lossy(&fast_output.stderr)
    );
    let fast_records = settle_lifecycle(&fast_log_path);
    let fast_elapsed = fast_started.elapsed();
    assert_eq!(fast_records.len(), 2);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(fast_completion)) =
        fast_records[1].body_ref()
    else {
        panic!("second fast record is not completion")
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(fast_completion) =
        fast_completion
    else {
        panic!("reconciled completion")
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
        let required_artifact = harness.path().join(format!("{name}-result.json"));
        argv.splice(delimiter..delimiter, ["--log-file", log_path.to_str().expect("path"), "--node", "m3-s1", "--role", "step-executor", "--ref", "abc", "--evidence", "fixture", "--required-artifact", required_artifact.to_str().expect("required artifact")].map(str::to_owned));
        let started = Instant::now();
        let output = harness.run(&argv, b"").expect("run failed lifecycle");
        assert!(output.status.success());
        settle_lifecycle(&log_path);
        let elapsed = started.elapsed();
        let records = fs::read_to_string(&log_path).expect("read log").lines().map(|line| parse_event_line(line).expect("parse")).collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref() else { panic!("missing completion") };
        let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) = completion else { panic!("reconciled completion") };
        assert_eq!(completion.usage, DispatchTokenUsage::Absent { reason });
        assert_eq!(completion.exit_status, DispatchExitStatus::Exited { code: pce_core::ExitCode::new(code as u64) });
        assert!(Duration::from_millis(completion.duration_ms.get()) <= elapsed);
        if matches!(reason, UsageAbsenceReason::MalformedTerminalData | UsageAbsenceReason::DuplicateTerminalData | UsageAbsenceReason::ContradictoryTerminalData) {
            let diagnostic = wait_for_file_bytes(
                &dispatch_sidecar(&log_path, 1, "stderr"),
                |bytes| String::from_utf8_lossy(bytes).contains(usage_reason_name(reason)),
            );
            assert!(
                String::from_utf8_lossy(&diagnostic).contains(usage_reason_name(reason))
            );
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
    let required_artifact = harness.path().join("additive-usage-result.json");
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
            "--required-artifact",
            required_artifact.to_str().expect("required artifact"),
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
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    settle_lifecycle(&log_path);
    assert_eq!(
        fs::read(dispatch_sidecar(&log_path, 1, "stdout")).expect("read stdout sidecar"),
        fixture
    );
    let records = settle_lifecycle(&log_path);
    assert_eq!(records.len(), 2);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("missing completion")
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
    };
    assert_eq!(
        completion.usage,
        DispatchTokenUsage::Measured {
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
        UsageAbsenceReason::ClaudeMalformedResult => "claude-malformed-result",
        UsageAbsenceReason::ClaudeMissingUsage => "claude-missing-usage",
        UsageAbsenceReason::ClaudeErrorEnvelope => "claude-error-envelope",
        UsageAbsenceReason::ClaudeExitEnvelopeContradiction => "claude-exit-envelope-contradiction",
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
    let required_artifact = harness.path().join("signal-result.json");
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
            "--required-artifact",
            required_artifact.to_str().expect("required artifact"),
        ]
        .map(str::to_owned),
    );
    let output = harness.run(&argv, b"").expect("run signal lifecycle");
    assert!(output.status.success());
    settle_lifecycle(&log_path);
    let records = settle_lifecycle(&log_path);
    assert_eq!(records.len(), 2);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("missing completion")
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
    };
    assert_eq!(
        completion.exit_status,
        DispatchExitStatus::Signaled {
            signal: pce_core::SignalNumber::new(15)
        }
    );
    assert_eq!(
        completion.usage,
        DispatchTokenUsage::Absent {
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
    let required_artifact = harness.path().join("interleaving-result.json");
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
            "--required-artifact",
            required_artifact.to_str().expect("required artifact"),
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
    let records = wait_for_lifecycle_records(&log_path, 3, Instant::now() + Duration::from_secs(5));
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
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
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

fn wait_for_non_empty_trimmed_file(path: &Path, failure: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(contents) = fs::read_to_string(path) {
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                return trimmed.to_owned();
            }
        }
        assert!(Instant::now() < deadline, "{failure}");
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "macos")]
fn independently_observe_darwin_start(pid: u32) -> (u64, u32) {
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let observed = unsafe {
        libc::proc_pidinfo(
            i32::try_from(pid).expect("PID fits pid_t"),
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            size,
        )
    };
    assert_eq!(observed, size, "independent proc_pidinfo observation");
    assert_eq!(info.pbi_pid, pid);
    (
        info.pbi_start_tvsec,
        u32::try_from(info.pbi_start_tvusec).expect("microseconds fit u32"),
    )
}

#[cfg(target_os = "macos")]
fn assert_live_distinct_continuation_identity(
    identity: &DispatchProcessIdentity,
    child_pid: u32,
    starter_pid: u32,
) {
    let continuation = identity
        .continuation_process_identity()
        .expect("production sidecar continuation identity");
    let continuation_pid = continuation.process_number().get();
    assert_ne!(continuation_pid, child_pid);
    assert_ne!(continuation_pid, starter_pid);
    let observed = independently_observe_darwin_start(continuation_pid);
    assert_eq!(
        continuation
            .process_start_identity()
            .seconds_since_unix_epoch(),
        observed.0
    );
    assert_eq!(
        continuation.process_start_identity().microseconds(),
        observed.1
    );
}

#[cfg(target_os = "macos")]
fn assert_blocked_identity(target: DispatchTarget) {
    const SEED: &[u8] = b"{\"sequence\":41,\"timestamp\":\"2026-08-09T12:34:56.000Z\",\"kind\":\"delta\",\"node\":\"m1-s1\",\"payload\":{\"message\":\"seed\"}}\n";
    match target {
        DispatchTarget::Codex => {
            let harness = CliHarness::new().expect("create identity harness");
            let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
            let record_root = harness.path().join("identity-codex-records");
            fs::create_dir(&record_root).expect("create record root");
            let stdout_path = harness.path().join("identity-codex.stdout");
            let stderr_path = harness.path().join("identity-codex.stderr");
            let schema_path = harness.path().join("identity-codex.schema.json");
            let output_path = harness.path().join("identity-codex.output.json");
            let artifact_bytes = harness.path().join("identity-codex.artifact");
            let release = harness.path().join("identity-codex.release");
            let log = harness.path().join("events.jsonl");
            fs::write(&stdout_path, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("stdout");
            fs::write(&stderr_path, []).expect("stderr");
            fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("schema");
            fs::write(&artifact_bytes, CONFORMING_ARTIFACT).expect("artifact bytes");
            fs::write(&log, SEED).expect("seed log");
            let mut environment =
                child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
            environment.extend([
                (
                    "PCE_CODEX_BLOCK_FILE".to_owned(),
                    release.display().to_string(),
                ),
                (
                    "PCE_CODEX_OUTPUT_BYTES_FILE".to_owned(),
                    artifact_bytes.display().to_string(),
                ),
            ]);
            let mut argv = dispatch_argv(
                &cwd,
                &environment,
                Some((&schema_path, &output_path)),
                None,
                "IDENTITY",
            );
            let delimiter = argv
                .iter()
                .position(|item| item == "--")
                .expect("delimiter");
            argv.splice(delimiter..delimiter, logging_arguments(&log, &output_path));
            let parent = Command::new(env!("CARGO_BIN_EXE_pce"))
                .args(&argv)
                .env_clear()
                .env("PATH", harness.shim_path())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn pce");
            let starter_pid = parent.id();
            let pid_path = record_root.join("invocation/pid");
            let sidecar = harness.path().join("events.jsonl.dispatches/42.json");
            let pid = wait_for_non_empty_trimmed_file(
                &pid_path,
                "shim PID was not readable within five seconds",
            )
            .parse::<u32>()
            .expect("parse shim PID");
            wait_for_path(&sidecar);
            let cat = Command::new("/bin/cat")
                .arg(&sidecar)
                .output()
                .expect("cross-process sidecar read");
            assert!(cat.status.success());
            let identity = parse_dispatch_process_identity(&cat.stdout).expect("parse sidecar");
            let observed = independently_observe_darwin_start(pid);
            assert_live_distinct_continuation_identity(&identity, pid, starter_pid);
            assert_eq!(identity.issuance_sequence().get(), 42);
            assert_eq!(identity.process_number().get(), pid);
            assert_eq!(
                identity.process_start_identity().seconds_since_unix_epoch(),
                observed.0
            );
            assert_eq!(identity.process_start_identity().microseconds(), observed.1);
            assert_eq!(identity.required_artifact_path().as_path(), output_path);
            assert_eq!(
                fs::metadata(&sidecar)
                    .expect("sidecar metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o444
            );
            assert_eq!(
                fs::metadata(sidecar.parent().expect("sidecar directory"))
                    .expect("directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            let blocked_log = fs::read_to_string(&log).expect("blocked log");
            assert_eq!(blocked_log.lines().count(), 2);
            fs::write(&release, []).expect("release child");
            let output = parent.wait_with_output().expect("reap pce");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            wait_for_lifecycle_records(&log, 3, Instant::now() + Duration::from_secs(5));
            assert_eq!(
                fs::read(&output_path).expect("real artifact"),
                CONFORMING_ARTIFACT
            );
            assert_eq!(
                fs::read_to_string(&log).expect("final log").lines().count(),
                3
            );
        }
        DispatchTarget::Gate => {
            let fixture = GateFixture::new("identity-gate", CLAUDE_SUCCESS);
            let log = fixture.harness.path().join("events.jsonl");
            let release = fixture.harness.path().join("identity-gate.release");
            fs::write(&log, SEED).expect("seed log");
            let mut environment = fixture.environment(0);
            environment.push((
                "PCE_CLAUDE_BLOCK_FILE".to_owned(),
                release.display().to_string(),
            ));
            let mut argv = fixture.argv(&environment, &[]);
            insert_gate_logging(&mut argv, &log, false);
            let parent = Command::new(env!("CARGO_BIN_EXE_pce"))
                .args(&argv)
                .env_clear()
                .env("PATH", fixture.harness.shim_path())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn gate pce");
            let starter_pid = parent.id();
            let pid_path = fixture.record_root.join("invocation/pid");
            let sidecar = fixture
                .harness
                .path()
                .join("events.jsonl.dispatches/42.json");
            let pid = wait_for_non_empty_trimmed_file(
                &pid_path,
                "gate shim PID was not readable within five seconds",
            )
            .parse::<u32>()
            .expect("parse PID");
            wait_for_path(&sidecar);
            let cat = Command::new("/bin/cat")
                .arg(&sidecar)
                .output()
                .expect("cat sidecar");
            assert!(cat.status.success());
            let identity = parse_dispatch_process_identity(&cat.stdout).expect("parse sidecar");
            let observed = independently_observe_darwin_start(pid);
            assert_live_distinct_continuation_identity(&identity, pid, starter_pid);
            assert_eq!(identity.issuance_sequence().get(), 42);
            assert_eq!(identity.process_number().get(), pid);
            assert_eq!(
                identity.process_start_identity().seconds_since_unix_epoch(),
                observed.0
            );
            assert_eq!(identity.process_start_identity().microseconds(), observed.1);
            assert_eq!(
                identity.required_artifact_path().as_path(),
                fixture.output_path
            );
            assert_eq!(
                fs::metadata(&sidecar)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o444
            );
            assert_eq!(
                fs::read_to_string(&log)
                    .expect("blocked log")
                    .lines()
                    .count(),
                2
            );
            fs::write(&release, []).expect("release gate");
            let output = parent.wait_with_output().expect("reap gate");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            wait_for_lifecycle_records(&log, 3, Instant::now() + Duration::from_secs(5));
            assert_eq!(
                fs::read_to_string(&log).expect("final log").lines().count(),
                3
            );
        }
        DispatchTarget::Seatbelt => panic!("not a dispatch identity route"),
    }
}

#[cfg(target_os = "macos")]
#[test]
fn logged_codex_publishes_real_darwin_identity_while_child_is_blocked() {
    let _guard = dispatch_test_guard();
    assert_blocked_identity(DispatchTarget::Codex);
}

#[cfg(target_os = "macos")]
#[test]
fn logged_gate_publishes_real_darwin_identity_while_child_is_blocked() {
    let _guard = dispatch_test_guard();
    assert_blocked_identity(DispatchTarget::Gate);
}

#[cfg(target_os = "macos")]
#[test]
fn concurrent_reader_observes_dispatch_identity_absent_or_complete() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create concurrent-reader harness");
    let cwd = fs::canonicalize(harness.path()).expect("cwd");
    let records = harness.path().join("reader-records");
    fs::create_dir(&records).expect("records");
    let stdout = harness.path().join("reader.stdout");
    let stderr = harness.path().join("reader.stderr");
    let release = harness.path().join("reader.release");
    let log = harness.path().join("events.jsonl");
    let artifact = harness.path().join("reader-result.json");
    fs::write(&stdout, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("stdout");
    fs::write(&stderr, []).expect("stderr");
    let mut environment = child_environment(&harness, &records, &stdout, &stderr, 0);
    environment.push((
        "PCE_CODEX_BLOCK_FILE".to_owned(),
        release.display().to_string(),
    ));
    let mut argv = dispatch_argv(&cwd, &environment, None, None, "READER");
    let delimiter = argv
        .iter()
        .position(|item| item == "--")
        .expect("delimiter");
    argv.splice(delimiter..delimiter, logging_arguments(&log, &artifact));
    let sidecar = harness.path().join("events.jsonl.dispatches/1.json");
    let reader = Command::new("/bin/sh")
        .args([
            "-c",
            "while :; do if [ -e \"$1\" ]; then /bin/cat \"$1\"; exit; fi; done",
            "reader",
        ])
        .arg(&sidecar)
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn independent reader");
    let mut parent = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(&argv)
        .env_clear()
        .env("PATH", harness.shim_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn pce");
    wait_for_path(&records.join("invocation/pid"));
    let read = reader.wait_with_output().expect("reap reader");
    assert!(read.status.success());
    assert!(read.stdout.ends_with(b"\n"));
    parse_dispatch_process_identity(&read.stdout).expect("reader observed complete document");
    fs::write(&release, []).expect("release");
    assert!(parent.wait().expect("reap parent").success());
}

#[cfg(target_os = "macos")]
#[test]
fn existing_dispatch_identity_is_never_overwritten() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create overwrite harness");
    let cwd = fs::canonicalize(harness.path()).expect("cwd");
    let records = harness.path().join("overwrite-records");
    fs::create_dir(&records).expect("records");
    let stdout = harness.path().join("overwrite.stdout");
    let stderr = harness.path().join("overwrite.stderr");
    let release = harness.path().join("never-release");
    let log = harness.path().join("events.jsonl");
    let artifact = harness.path().join("overwrite-result.json");
    fs::write(&stdout, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("stdout");
    fs::write(&stderr, []).expect("stderr");
    fs::write(&log, b"{\"sequence\":41,\"timestamp\":\"2026-08-09T12:34:56.000Z\",\"kind\":\"delta\",\"node\":\"m1-s1\",\"payload\":{\"message\":\"seed\"}}\n").expect("seed");
    let directory = harness.path().join("events.jsonl.dispatches");
    fs::create_dir(&directory).expect("sidecar directory");
    let sidecar = directory.join("42.json");
    fs::write(&sidecar, b"attacker-owned\n").expect("attacker sidecar");
    let mut environment = child_environment(&harness, &records, &stdout, &stderr, 0);
    environment.push((
        "PCE_CODEX_BLOCK_FILE".to_owned(),
        release.display().to_string(),
    ));
    let mut argv = dispatch_argv(&cwd, &environment, None, None, "OVERWRITE");
    let delimiter = argv
        .iter()
        .position(|item| item == "--")
        .expect("delimiter");
    argv.splice(delimiter..delimiter, logging_arguments(&log, &artifact));
    let output = harness.run(&argv, b"").expect("run overwrite dispatch");
    assert!(output.status.success());
    let continuation_stderr = harness.path().join("events.jsonl.dispatch-000042.stderr");
    wait_for_path(&continuation_stderr);
    let deadline = Instant::now() + Duration::from_secs(5);
    let diagnostic = loop {
        let diagnostic = fs::read_to_string(&continuation_stderr).expect("continuation diagnostic");
        if diagnostic.contains("dispatch process identity sidecar already exists") {
            break diagnostic;
        }
        assert!(Instant::now() < deadline, "missing continuation diagnostic");
        thread::sleep(Duration::from_millis(10));
    };
    assert!(diagnostic.contains("dispatch process identity sidecar already exists"));
    let shim_pid = diagnostic
        .split("child PID ")
        .nth(1)
        .and_then(|suffix| suffix.split_whitespace().next())
        .and_then(|pid| pid.parse::<u32>().ok())
        .unwrap_or_else(|| panic!("real spawned child PID in setup diagnostic: {diagnostic}"));
    assert_eq!(fs::read(&sidecar).expect("sidecar"), b"attacker-owned\n");
    wait_for_lifecycle_records(&log, 3, deadline);
    assert_eq!(fs::read_to_string(&log).expect("log").lines().count(), 3);
    let status = Command::new("/bin/kill")
        .args(["-0", &shim_pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .expect("probe PID");
    assert!(!status.success(), "setup cleanup left child alive");
}

struct DetachedCodexFixture {
    harness: CliHarness,
    record_root: PathBuf,
    release_path: PathBuf,
    log_path: PathBuf,
    argv: Vec<String>,
}

fn detached_codex_fixture(
    name: &str,
    block_name: Option<&str>,
    sleep_seconds: Option<&str>,
) -> DetachedCodexFixture {
    let harness = CliHarness::new().expect("create detached Codex harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize detached cwd");
    let record_root = harness.path().join(format!("{name}-records"));
    fs::create_dir(&record_root).expect("create detached record root");
    let stdout_path = harness.path().join(format!("{name}.stdout"));
    let stderr_path = harness.path().join(format!("{name}.stderr"));
    let release_path = harness.path().join(block_name.unwrap_or("unused-release"));
    let log_path = harness.path().join("events.jsonl");
    fs::write(&stdout_path, DETACHED_SUCCESS_TRANSCRIPT).expect("write detached transcript");
    fs::write(&stderr_path, []).expect("write detached stderr");
    let mut environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    if block_name.is_some() {
        environment.push((
            "PCE_CODEX_BLOCK_FILE".to_owned(),
            release_path.display().to_string(),
        ));
    }
    if let Some(seconds) = sleep_seconds {
        environment.push(("PCE_CODEX_SLEEP_SECONDS".to_owned(), seconds.to_owned()));
    }
    let mut argv = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let delimiter = argv.iter().position(|arg| arg == "--").expect("delimiter");
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
            "--required-artifact".to_owned(),
            harness
                .path()
                .join(format!("{name}-artifact.json"))
                .display()
                .to_string(),
        ],
    );
    DetachedCodexFixture {
        harness,
        record_root,
        release_path,
        log_path,
        argv,
    }
}

#[cfg(target_os = "macos")]
#[test]
fn harness_drop_cleans_blocked_dispatch_when_test_panics_helper() {
    let Some(snapshot_path) = std::env::var_os("PCE_HARNESS_CLEANUP_SNAPSHOT") else {
        return;
    };
    let fixture = detached_codex_fixture("panic-cleanup", Some("never-release"), None);
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(&fixture.argv)
        .env_clear()
        .env("PATH", fixture.harness.shim_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("start blocked dispatch");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    wait_for_path(&fixture.record_root.join("invocation/pid"));
    let sidecar = fixture
        .harness
        .path()
        .join("events.jsonl.dispatches/1.json");
    wait_for_path(&sidecar);
    fs::copy(sidecar, snapshot_path).expect("copy dispatch identity outside harness tempdir");
    panic!("intentional panic after blocked dispatch");
}

#[cfg(target_os = "macos")]
#[test]
fn crashed_test_process_leaves_no_recorded_dispatch_group_alive() {
    let snapshot_dir = tempfile::tempdir().expect("create cleanup snapshot directory");
    let snapshot_path = snapshot_dir.path().join("dispatch-identity.json");
    let output = Command::new(std::env::current_exe().expect("resolve test executable"))
        .args([
            "--exact",
            "harness_drop_cleans_blocked_dispatch_when_test_panics_helper",
            "--nocapture",
        ])
        .env("PCE_HARNESS_CLEANUP_SNAPSHOT", &snapshot_path)
        .output()
        .expect("run intentionally panicking test subprocess");
    assert!(
        !output.status.success(),
        "helper must exercise panic unwinding"
    );
    let identity = parse_dispatch_process_identity(
        &fs::read(&snapshot_path).expect("read copied dispatch identity"),
    )
    .expect("parse copied dispatch identity");
    let continuation = identity
        .continuation_process_identity()
        .expect("current sidecars record continuation identity");
    let process_number = continuation.process_number().get();
    let identity_is_exact = |identity: pce_core::RecordedProcessIdentity| {
        let pid = identity.process_number().get();
        let start = identity.process_start_identity();
        let expected = format!(
            "{} {}",
            start.seconds_since_unix_epoch(),
            start.microseconds()
        )
        .into_bytes();
        process_start_identity(&pid.to_string()).as_deref() == Some(expected.as_slice())
    };
    let child = identity.child_process_identity();
    let continuation_proves_group = identity_is_exact(continuation)
        && unsafe { libc::getpgid(process_number as i32) } == process_number as i32;
    let child_proves_group = identity_is_exact(child)
        && unsafe { libc::getpgid(child.process_number().get() as i32) } == process_number as i32;
    let group_is_alive = unsafe { libc::kill(-(process_number as i32), 0) } == 0;
    if group_is_alive && (continuation_proves_group || child_proves_group) {
        unsafe {
            libc::kill(-(process_number as i32), libc::SIGKILL);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while unsafe { libc::kill(-(process_number as i32), 0) } == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
    }
    assert!(
        !group_is_alive,
        "panicking test left recorded dispatch process group {process_number} alive"
    );
}

#[cfg(target_os = "macos")]
#[test]
#[allow(clippy::zombie_processes)]
fn harness_drop_reports_unrecorded_live_shim_helper() {
    let Some(snapshot_path) = std::env::var_os("PCE_HARNESS_UNRECORDED_PID") else {
        return;
    };
    let harness = CliHarness::new().expect("create unrecorded-shim harness");
    let record_root = harness.path().join("unrecorded-records");
    fs::create_dir(&record_root).expect("create unrecorded record root");
    let stdout_path = harness.path().join("unrecorded.stdout");
    let stderr_path = harness.path().join("unrecorded.stderr");
    fs::write(&stdout_path, []).expect("write shim stdout");
    fs::write(&stderr_path, []).expect("write shim stderr");
    let mut command = Command::new("codex");
    command
        .env_clear()
        .env("PATH", harness.shim_path())
        .env("TMPDIR", harness.path())
        .env("PCE_CODEX_RECORD_ROOT", &record_root)
        .env("PCE_CODEX_STDOUT_FILE", &stdout_path)
        .env("PCE_CODEX_STDERR_FILE", &stderr_path)
        .env("PCE_CODEX_EXIT_CODE", "0")
        .env("PCE_CODEX_BLOCK_FILE", harness.path().join("never-created"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let _shim = command.spawn().expect("spawn unrecorded shim group");
    let pid_path = record_root.join("invocation/pid");
    let process_number = wait_for_non_empty_trimmed_file(
        &pid_path,
        "unrecorded shim PID was not readable within five seconds",
    );
    let start_identity = process_start_identity(&process_number)
        .expect("read unrecorded shim start identity before dropping harness");
    let snapshot = format!(
        "{} {}",
        process_number,
        String::from_utf8(start_identity).expect("Darwin start identity is ASCII")
    );
    fs::write(snapshot_path, snapshot).expect("copy unrecorded shim identity");
    drop(harness);
}

#[cfg(target_os = "macos")]
#[test]
fn unrecorded_live_shim_fails_and_names_the_test() {
    let snapshot_dir = tempfile::tempdir().expect("create unrecorded PID directory");
    let snapshot_path = snapshot_dir.path().join("shim-pid");
    let output = Command::new(std::env::current_exe().expect("resolve test executable"))
        .args([
            "--exact",
            "harness_drop_reports_unrecorded_live_shim_helper",
            "--nocapture",
        ])
        .env("PCE_HARNESS_UNRECORDED_PID", &snapshot_path)
        .output()
        .expect("run unrecorded-shim helper subprocess");
    let snapshot = fs::read_to_string(&snapshot_path).expect("read copied shim identity");
    let mut fields = snapshot.split_whitespace();
    let process_number = fields
        .next()
        .expect("copied shim PID")
        .parse::<i32>()
        .expect("parse copied shim PID");
    let expected_start_identity = fields.collect::<Vec<_>>().join(" ").into_bytes();
    let exact_group_leader = process_start_identity(&process_number.to_string()).as_deref()
        == Some(expected_start_identity.as_slice())
        && unsafe { libc::getpgid(process_number) } == process_number;
    if exact_group_leader {
        unsafe {
            libc::kill(-process_number, libc::SIGKILL);
        }
    }
    assert!(
        !output.status.success(),
        "live unrecorded shim must fail its test"
    );
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    assert!(
        diagnostic.contains(
            "test harness_drop_reports_unrecorded_live_shim_helper leaked a test agent process"
        ),
        "leak diagnostic did not name the test: {diagnostic}"
    );
}

#[test]
fn agent_shims_exit_at_their_block_iteration_ceiling() {
    for program in ["codex", "claude"] {
        let harness = CliHarness::new().expect("create self-limiting shim harness");
        let record_root = harness.path().join(format!("{program}-ceiling-records"));
        fs::create_dir(&record_root).expect("create ceiling record root");
        let stdout_path = harness.path().join(format!("{program}.stdout"));
        let stderr_path = harness.path().join(format!("{program}.stderr"));
        let output_path = harness.path().join(format!("{program}.output"));
        fs::write(&stdout_path, []).expect("write shim stdout");
        fs::write(&stderr_path, []).expect("write shim stderr");
        let prefix = program.to_ascii_uppercase();
        let started = Instant::now();
        let output = Command::new(program)
            .env_clear()
            .env("PATH", harness.shim_path())
            .env("TMPDIR", harness.path())
            .env(format!("PCE_{prefix}_RECORD_ROOT"), &record_root)
            .env(format!("PCE_{prefix}_STDOUT_FILE"), &stdout_path)
            .env(format!("PCE_{prefix}_STDERR_FILE"), &stderr_path)
            .env(format!("PCE_{prefix}_EXIT_CODE"), "0")
            .env(format!("PCE_{prefix}_OUTPUT_PATH"), &output_path)
            .env(
                format!("PCE_{prefix}_BLOCK_FILE"),
                harness.path().join("never-created"),
            )
            .env("PCE_SHIM_BLOCK_MAX_POLLS", "2")
            .env("PCE_SHIM_BLOCK_POLL_SECONDS", "0.01")
            .stdin(Stdio::null())
            .output()
            .expect("run self-limiting shim");
        assert_eq!(output.status.code(), Some(124), "{program} ceiling status");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{program} did not honor its block iteration ceiling"
        );
    }
}

#[cfg(target_os = "macos")]
fn process_start_identity(pid: &str) -> Option<Vec<u8>> {
    let pid = pid.parse::<u32>().ok()?;
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
    let observed = unsafe {
        libc::proc_pidinfo(
            i32::try_from(pid).ok()?,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            size,
        )
    };
    (observed == size && info.pbi_pid == pid)
        .then(|| format!("{} {}", info.pbi_start_tvsec, info.pbi_start_tvusec).into_bytes())
}

#[cfg(not(target_os = "macos"))]
fn process_start_identity(pid: &str) -> Option<Vec<u8>> {
    let output = Command::new("/bin/ps")
        .args(["-o", "lstart=", "-p", pid])
        .output()
        .expect("observe process identity with ps");
    let identity = output.stdout.trim_ascii().to_vec();
    (output.status.success() && !identity.is_empty()).then_some(identity)
}

fn run_captured_starter_with_deadline(
    argv: &[String],
    harness: &CliHarness,
    shim_pid_path: &Path,
) -> std::process::Output {
    let child = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(argv)
        .env_clear()
        .env("PATH", harness.shim_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn captured starter");
    let starter_pid = child.id();
    let (sender, receiver) = mpsc::sync_channel(1);
    let waiter = thread::spawn(move || {
        let _sent = sender.send(child.wait_with_output().expect("wait for captured starter"));
    });
    match receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(output) => {
            waiter.join().expect("join starter waiter");
            output
        }
        Err(error) => {
            let _status = Command::new("/bin/kill")
                .args(["-TERM", &starter_pid.to_string()])
                .status();
            if let Ok(shim_pid) = fs::read_to_string(shim_pid_path) {
                let _status = Command::new("/bin/kill")
                    .args(["-TERM", shim_pid.trim()])
                    .status();
            }
            let _joined = waiter.join();
            panic!("starter capture did not reach EOF within five seconds: {error}");
        }
    }
}

fn assert_exact_issuance(record: &EventRecord) {
    assert_eq!(record.sequence(), Sequence::first());
    assert_eq!(record.node().as_str(), "m3-s1");
    let EventBodyRef::Known(KnownPayload::Dispatch(payload)) = record.body_ref() else {
        panic!("first record is not dispatch issuance")
    };
    assert_eq!(payload.role.as_str(), "step-executor");
    assert_eq!(payload.r#ref.as_str(), "abc123");
    assert_eq!(payload.evidence.as_str(), "fixture invocation");
}

fn assert_exact_detached_lifecycle(records: &[EventRecord], slow: bool) {
    assert_eq!(records.len(), 2);
    assert_exact_issuance(&records[0]);
    assert_eq!(records[1].sequence().get(), 2);
    assert_eq!(records[1].node().as_str(), "m3-s1");
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("second record is not dispatch completion")
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
    };
    assert_eq!(completion.issuance_sequence, Sequence::first());
    assert_eq!(
        completion.exit_status,
        DispatchExitStatus::Exited {
            code: pce_core::ExitCode::new(0),
        }
    );
    assert_eq!(completion.usage, measured_usage());
    assert_eq!(completion.artifact_outcome, ArtifactOutcome::NotValidated);
    if slow {
        assert!(completion.duration_ms.get() > 600_000);
    } else {
        assert!(completion.duration_ms.get() > 0);
    }
}

#[test]
fn logged_dispatch_starter_returns_while_blocked_child_remains_alive_and_later_appends_one_correlated_completion()
 {
    let _guard = dispatch_test_guard();
    let fixture = detached_codex_fixture("blocked", Some("block"), None);
    let starter_stdout = fixture.harness.path().join("starter.stdout");
    let starter_stderr = fixture.harness.path().join("starter.stderr");
    let mut starter = Command::new(env!("CARGO_BIN_EXE_pce"));
    starter
        .args(&fixture.argv)
        .env_clear()
        .env("PATH", fixture.harness.shim_path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            fs::File::create(&starter_stdout).expect("create starter stdout"),
        ))
        .stderr(Stdio::from(
            fs::File::create(&starter_stderr).expect("create starter stderr"),
        ));
    let mut starter = starter.spawn().expect("spawn logged starter");
    let shim_pid = wait_for_non_empty_trimmed_file(
        &fixture.record_root.join("invocation/pid"),
        "shim PID was not readable within five seconds",
    );
    let shim_pid = shim_pid.as_str();
    let identity_deadline = Instant::now() + Duration::from_secs(5);
    let identity = loop {
        if let Some(identity) = process_start_identity(shim_pid) {
            break identity;
        }
        assert!(
            Instant::now() < identity_deadline,
            "child start identity for PID {shim_pid} was not readable within five seconds"
        );
        thread::sleep(Duration::from_millis(10));
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = starter.try_wait().expect("poll starter") {
            assert!(status.success());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "starter did not return within five seconds"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!fixture.release_path.exists());
    assert_eq!(
        process_start_identity(shim_pid).as_deref(),
        Some(identity.as_slice())
    );
    let issuance = wait_for_lifecycle_records(
        &fixture.log_path,
        1,
        Instant::now() + Duration::from_secs(5),
    );
    assert_exact_issuance(&issuance[0]);
    assert_eq!(fs::read(&starter_stdout).expect("starter stdout"), b"");
    assert_eq!(fs::read(&starter_stderr).expect("starter stderr"), b"");
    fs::write(&fixture.release_path, []).expect("release blocked child");
    let deadline = Instant::now() + Duration::from_secs(5);
    while process_start_identity(shim_pid).as_deref() == Some(identity.as_slice()) {
        assert!(
            Instant::now() < deadline,
            "child identity remained live after release"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let records = wait_for_lifecycle_records(&fixture.log_path, 2, deadline);
    assert_exact_detached_lifecycle(&records, false);
    assert_eq!(
        fs::read(dispatch_sidecar(&fixture.log_path, 1, "stdout")).expect("stdout sidecar"),
        DETACHED_SUCCESS_TRANSCRIPT
    );
    assert_eq!(
        fs::read(dispatch_sidecar(&fixture.log_path, 1, "stderr")).expect("stderr sidecar"),
        b""
    );
}

#[test]
fn detached_continuation_tees_stdout_without_contaminating_event_log() {
    let _guard = dispatch_test_guard();
    let fixture = detached_codex_fixture("tee", None, None);
    let output = run_captured_starter_with_deadline(
        &fixture.argv,
        &fixture.harness,
        &fixture.record_root.join("invocation/pid"),
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    let records = settle_lifecycle(&fixture.log_path);
    assert_exact_detached_lifecycle(&records, false);
    assert_eq!(
        fs::read(dispatch_sidecar(&fixture.log_path, 1, "stdout")).expect("stdout sidecar"),
        DETACHED_SUCCESS_TRANSCRIPT
    );
    assert_eq!(
        fs::read(dispatch_sidecar(&fixture.log_path, 1, "stderr")).expect("stderr sidecar"),
        b""
    );
    let log = fs::read_to_string(&fixture.log_path).expect("event log");
    assert!(
        log.lines()
            .all(|line| !line.contains("\"type\":\"turn.completed\""))
    );
}

#[test]
#[ignore = "runs a real dispatch child for at least 601 seconds"]
fn dispatch_child_runtime_has_no_ceiling() {
    let _guard = dispatch_test_guard();
    let fixture = detached_codex_fixture("slow", None, Some("601"));
    let started = Instant::now();
    let output = run_captured_starter_with_deadline(
        &fixture.argv,
        &fixture.harness,
        &fixture.record_root.join("invocation/pid"),
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    let records = wait_for_lifecycle_records(
        &fixture.log_path,
        2,
        Instant::now() + Duration::from_secs(660),
    );
    assert!(started.elapsed() >= Duration::from_secs(601));
    assert_exact_detached_lifecycle(&records, true);
    assert_eq!(
        fs::read(dispatch_sidecar(&fixture.log_path, 1, "stdout")).expect("slow stdout"),
        DETACHED_SUCCESS_TRANSCRIPT
    );
    assert_eq!(
        fs::read(dispatch_sidecar(&fixture.log_path, 1, "stderr")).expect("slow stderr"),
        b""
    );
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

    let log = harness.path().join("spawn-events.jsonl");
    let artifact = harness.path().join("spawn-result.json");
    let mut logged = dispatch_argv(&cwd, &environment, None, None, "PROMPT");
    let delimiter = logged
        .iter()
        .position(|item| item == "--")
        .expect("delimiter");
    logged.splice(delimiter..delimiter, logging_arguments(&log, &artifact));
    let output = harness.run(&logged, b"").expect("run logged spawn failure");
    assert!(output.status.success());
    let continuation_stderr = harness
        .path()
        .join("spawn-events.jsonl.dispatch-000001.stderr");
    wait_for_path(&continuation_stderr);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let diagnostic = fs::read_to_string(&continuation_stderr).expect("continuation stderr");
        if diagnostic.contains("failed to spawn `codex`") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "missing spawn failure diagnostic"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read_to_string(&log)
            .expect("issuance log")
            .lines()
            .count(),
        1
    );
    assert!(
        !harness
            .path()
            .join("spawn-events.jsonl.dispatches")
            .exists()
    );
}

#[test]
fn logged_dispatch_rejects_missing_relative_or_mismatched_required_artifact_before_spawn() {
    let _guard = dispatch_test_guard();
    for (name, structured, logging, expected) in [
        (
            "missing",
            false,
            vec![
                "--log-file",
                "/tmp/events.jsonl",
                "--node",
                "m1-s1",
                "--role",
                "step-executor",
                "--ref",
                "ref",
                "--evidence",
                "evidence",
            ],
            "dispatch logging options must be supplied together in this order: --log-file, --node, --role, --ref, --evidence, --required-artifact",
        ),
        (
            "relative",
            false,
            vec![
                "--log-file",
                "/tmp/events.jsonl",
                "--node",
                "m1-s1",
                "--role",
                "step-executor",
                "--ref",
                "ref",
                "--evidence",
                "evidence",
                "--required-artifact",
                "relative/result.json",
            ],
            "required artifact path must be absolute: relative/result.json",
        ),
        (
            "mismatch",
            true,
            vec![
                "--log-file",
                "/tmp/events.jsonl",
                "--node",
                "m1-s1",
                "--role",
                "step-executor",
                "--ref",
                "ref",
                "--evidence",
                "evidence",
                "--required-artifact",
                "/tmp/pce/required.json",
            ],
            "dispatch required artifact path `/tmp/pce/required.json` does not match output path `/tmp/pce/output.json`",
        ),
    ] {
        let (harness, cwd, record_root, _, environment) = failure_fixture(name, 0);
        let schema = PathBuf::from("/tmp/pce/schema.json");
        let output = PathBuf::from("/tmp/pce/output.json");
        let mut argv = dispatch_argv(
            &cwd,
            &environment,
            structured.then_some((&schema, &output)),
            None,
            "PROMPT",
        );
        let delimiter = argv
            .iter()
            .position(|item| item == "--")
            .expect("delimiter");
        argv.splice(delimiter..delimiter, logging.into_iter().map(str::to_owned));
        let result = harness.run(&argv, b"").expect("run parser boundary");
        assert!(!result.status.success(), "{name}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!record_root.join("invocation").exists(), "{name}");
        assert!(
            !harness.path().join("events.jsonl.dispatches").exists(),
            "{name}"
        );
    }
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

const CLAUDE_SUCCESS: &[u8] = br#"{"is_error":false,"duration_ms":7,"result":"ok","usage":{"input_tokens":11,"output_tokens":13,"cache_creation_input_tokens":17,"cache_read_input_tokens":19}}"#;

struct GateFixture {
    harness: CliHarness,
    cwd: PathBuf,
    record_root: PathBuf,
    schema_path: PathBuf,
    output_path: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl GateFixture {
    fn new(name: &str, stdout: &[u8]) -> Self {
        let harness = CliHarness::new().expect("create gate harness");
        let cwd = fs::canonicalize(harness.path()).expect("canonicalize gate cwd");
        let record_root = harness.path().join(format!("{name}-records"));
        fs::create_dir(&record_root).expect("create gate record root");
        let schema_path = harness.path().join(format!("{name}-schema.json"));
        let output_path = harness.path().join(format!("{name}-output.json"));
        let stdout_path = harness.path().join(format!("{name}-stdout.bin"));
        let stderr_path = harness.path().join(format!("{name}-stderr.bin"));
        fs::write(&schema_path, VALID_ARTIFACT_SCHEMA).expect("write gate schema");
        fs::write(&output_path, CONFORMING_ARTIFACT).expect("write gate output");
        fs::write(&stdout_path, stdout).expect("write gate stdout");
        fs::write(&stderr_path, []).expect("write gate stderr");
        Self {
            harness,
            cwd,
            record_root,
            schema_path,
            output_path,
            stdout_path,
            stderr_path,
        }
    }

    fn environment(&self, exit_code: i32) -> Vec<(String, String)> {
        vec![
            ("PATH".to_owned(), self.harness.shim_path()),
            (
                "PCE_CLAUDE_RECORD_ROOT".to_owned(),
                self.record_root.display().to_string(),
            ),
            (
                "PCE_CLAUDE_STDOUT_FILE".to_owned(),
                self.stdout_path.display().to_string(),
            ),
            (
                "PCE_CLAUDE_STDERR_FILE".to_owned(),
                self.stderr_path.display().to_string(),
            ),
            ("PCE_CLAUDE_EXIT_CODE".to_owned(), exit_code.to_string()),
        ]
    }

    fn argv(&self, environment: &[(String, String)], tail: &[&str]) -> Vec<String> {
        let mut argv = vec![
            "dispatch".to_owned(),
            "gate".to_owned(),
            "--cwd".to_owned(),
            self.cwd.display().to_string(),
        ];
        for (name, value) in environment {
            argv.extend(["--env".to_owned(), format!("{name}={value}")]);
        }
        argv.extend([
            "--output-schema".to_owned(),
            self.schema_path.display().to_string(),
            "-o".to_owned(),
            self.output_path.display().to_string(),
            "--".to_owned(),
        ]);
        argv.extend(tail.iter().map(|value| (*value).to_owned()));
        argv
    }

    fn invocation(&self) -> ClaudeInvocation {
        wait_for_path(&self.record_root.join("invocation/pid"));
        let mut invocations = self
            .harness
            .claude_invocations(&self.record_root)
            .expect("read Claude invocation");
        assert_eq!(invocations.len(), 1);
        invocations.remove(0)
    }
}

#[test]
fn gate_planning_frame_is_exact_in_dry_run_and_live_child() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("planning-frame", CLAUDE_SUCCESS);
    let environment = fixture.environment(0);
    let log_path = fixture.harness.path().join("planning-frame.jsonl");
    let mut dry = fixture.argv(
        &environment,
        &[
            "--append-system-prompt",
            "/tmp/review.json",
            "Critique the plan.",
        ],
    );
    let delimiter = dry
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    dry.splice(
        delimiter..delimiter,
        [
            "--log-file",
            log_path.to_str().expect("log path"),
            "--node",
            "m5-s1",
            "--role",
            "step-plan-critic",
            "--ref",
            "fixture-ref",
            "--evidence",
            "fixture-evidence",
            "--required-artifact",
            fixture.output_path.to_str().expect("required artifact"),
            "--planning-act",
            "irreversible",
            "--dry-run",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    let projected = fixture
        .harness
        .run(&dry, b"")
        .expect("project gate planning dispatch");
    assert!(
        projected.status.success(),
        "{}",
        String::from_utf8_lossy(&projected.stderr)
    );
    let projection: Value = serde_json::from_slice(&projected.stdout).expect("projection JSON");
    let projected_argv = projection["envelope"]["argv"].as_array().expect("argv");
    assert_eq!(
        &projected_argv[projected_argv.len() - 3..],
        [
            json!("--append-system-prompt"),
            json!("/tmp/review.json"),
            json!(IRREVERSIBLE_PLANNING_FRAME)
        ]
    );
    assert_eq!(
        projection["envelope"]["stdin"],
        json!({"binding":"null","bytes":null})
    );
    assert_eq!(
        projection["envelope"]["schema_path"],
        fixture.schema_path.display().to_string()
    );
    assert_eq!(
        projection["envelope"]["output_path"],
        fixture.output_path.display().to_string()
    );
    assert_eq!(
        projection["issuance"]["payload"],
        json!({"role":"step-plan-critic","ref":"fixture-ref","evidence":"fixture-evidence"})
    );
    assert!(!log_path.exists());
    assert!(
        fixture
            .harness
            .claude_invocations(&fixture.record_root)
            .expect("dry invocations")
            .is_empty()
    );

    let mut live = dry.clone();
    live.retain(|value| value != "--dry-run");
    let output = fixture
        .harness
        .run(&live, b"")
        .expect("run gate planning dispatch");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log_path);
    let invocation = fixture.invocation();
    let captured = invocation
        .argv
        .iter()
        .map(|value| value.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(projection["envelope"]["argv"], json!(captured));
    assert_eq!(
        &captured[captured.len() - 3..],
        [
            "--append-system-prompt",
            "/tmp/review.json",
            IRREVERSIBLE_PLANNING_FRAME
        ]
    );
    assert_eq!(invocation.stdin, b"");
}

fn insert_gate_logging(argv: &mut Vec<String>, log_path: &Path, dry_run: bool) {
    let delimiter = argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    let output_flag = argv
        .iter()
        .position(|value| value == "-o")
        .expect("output flag");
    let required_artifact = argv[output_flag + 1].clone();
    let mut logging = vec![
        "--log-file".to_owned(),
        log_path.display().to_string(),
        "--node".to_owned(),
        "m6-s2".to_owned(),
        "--role".to_owned(),
        "critic".to_owned(),
        "--ref".to_owned(),
        "gate-ref".to_owned(),
        "--evidence".to_owned(),
        "gate-evidence".to_owned(),
        "--required-artifact".to_owned(),
        required_artifact,
    ];
    if dry_run {
        logging.push("--dry-run".to_owned());
    }
    argv.splice(delimiter..delimiter, logging);
}

fn select_falsification_critic(argv: &mut [String]) {
    let role = argv
        .iter()
        .position(|value| value == "--role")
        .expect("logging role");
    argv[role + 1] = "falsification-critic".to_owned();
}

fn falsification_mandate(output: &Path, client: &Path) -> String {
    format!(
        "You are the falsification critic. Judge the built artifact by executing probes, never by reviewing prose alone. Submit every stimulus and all of its setup through the harness command between the markers by writing exactly one request JSON object to its standard input: <gate-exec-command>{} gate exec</gate-exec-command>. The harness alone executes the command and setup, observes the result, and returns its execution_ref and observed_result; an execution the harness did not perform is not admissible evidence. A blocking issue is admissible only for a demonstrated break. For every blocking_issues entry, summarize the exact input in input and the returned observed_result in observation, and put that issue's returned harness reference in execution_ref; do not block on style, naming, design preference, scope, or any other reading-based opinion. For every rejection probe, execute an acceptance probe on the same built artifact whose input differs only in the property under test, and identify that acceptance execution's exact input, returned observed_result, and returned harness reference in the same issue's input and observation. Put the exact replacement you executed in required_change, summarize its input and returned observed_result in replacement_execution, and put that replacement run's returned harness reference in replacement_execution.execution_ref. If you cannot demonstrate a break, emit no blocking issue. Treat every check as a claim: mutate the subject it claims to test and rerun the check; if it stays green, that demonstrated vacuity is blocking, including when the check belongs to this gate rather than to the subject. For every rule the delivered work adds, delete the configuration entry that activates it and rerun the rule's checks; if they stay green, block. Run each mutation, configuration deletion, check, and replacement through the harness, and give each blocking issue its own execution rather than reusing one issue's evidence for another. You may reference harness records but cannot author or edit them. Write exactly one conforming verdict JSON object to the absolute path between the markers below: <output-path>{}</output-path>",
        client.display(),
        output.display()
    )
}

fn gate_completion(
    log_path: &Path,
) -> pce_core::ObservedDispatchCompletionWithArtifactPresencePayload {
    let records = settle_lifecycle(log_path);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[1].body_ref()
    else {
        panic!("second gate record is not a completion")
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("gate completion was reconciled")
    };
    completion.clone()
}

fn gate_request_directories(
    fixture: &GateFixture,
    requests: &[Value],
) -> (PathBuf, Vec<(String, String)>) {
    let request_directory = fixture.harness.path().join("gate-exec-requests");
    let response_directory = fixture.harness.path().join("gate-exec-responses");
    fs::create_dir(&request_directory).expect("create gate request directory");
    fs::create_dir(&response_directory).expect("create gate response directory");
    for (index, request) in requests.iter().enumerate() {
        fs::write(
            request_directory.join(format!("{:04}.json", index + 1)),
            serde_json::to_vec_pretty(request).expect("gate request JSON"),
        )
        .expect("write gate request");
    }
    let mut environment = fixture.environment(0);
    environment.extend([
        (
            "PCE_CLAUDE_GATE_EXEC_REQUEST_DIR".to_owned(),
            request_directory.display().to_string(),
        ),
        (
            "PCE_CLAUDE_GATE_EXEC_RESPONSE_DIR".to_owned(),
            response_directory.display().to_string(),
        ),
    ]);
    (response_directory, environment)
}

fn spawn_blocked_falsification_recorder(
    fixture: &GateFixture,
    name: &str,
) -> (std::process::Child, PathBuf, PathBuf, PathBuf) {
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"none"}"#,
    )
    .expect("write approval verdict");
    let block = fixture.harness.path().join(format!("release-{name}"));
    let mut environment = fixture.environment(0);
    environment.push((
        "PCE_CLAUDE_BLOCK_FILE".to_owned(),
        block.display().to_string(),
    ));
    let log = fixture.harness.path().join(format!("{name}.jsonl"));
    let mut argv = fixture.argv(&environment, &["probe"]);
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);
    let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
    command.args(&argv);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let child = command.spawn().expect("spawn parent dispatch");
    let environment_path = fixture.record_root.join("invocation/environment.bin");
    let ready_path = fixture.record_root.join("invocation/pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready_path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !ready_path.exists() {
        let output = child
            .wait_with_output()
            .expect("collect dispatch that did not spawn the gate child");
        panic!(
            "dispatch did not spawn the gate child: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let environment_bytes = fs::read(&environment_path).expect("recorded Claude environment");
    let socket = environment_bytes
        .split(|byte| *byte == 0)
        .find_map(|entry| {
            entry
                .strip_prefix(b"PCE_GATE_EXEC_SOCKET=")
                .map(|value| PathBuf::from(OsString::from_vec(value.to_vec())))
        })
        .expect("socket environment entry");
    (child, socket, block, log)
}

fn wait_for_dispatch_deadline(
    mut child: std::process::Child,
    panic_message: &str,
) -> std::process::Output {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait().expect("poll parent dispatch") {
            Some(_status) => return child.wait_with_output().expect("collect parent dispatch"),
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                child.kill().expect("kill overdue parent dispatch");
                let _status = child.wait().expect("reap overdue parent dispatch");
                panic!("{panic_message}");
            }
        }
    }
}

#[test]
fn falsification_dispatch_starts_while_a_stale_socket_file_exists() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("stale socket issuance", CLAUDE_SUCCESS);
    let stale_socket = fixture.harness.path().join("pce-gate-exec-stale-1.sock");
    let stale_listener = std::os::unix::net::UnixListener::bind(&stale_socket)
        .expect("stale socket fixture should bind");
    drop(stale_listener);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "stale-socket");
    assert!(stale_socket.exists());
    assert_ne!(socket, stale_socket);
    let response = raw_gate_response(
        &socket,
        &json!({
            "working_directory": fixture.cwd,
            "setup": [],
            "command": {"program":"/usr/bin/true","arguments":[],"input":[],"environment":{}}
        }),
    );
    assert_eq!(response["execution_ref"], "execution-000001");
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(child, "stale-socket dispatch exceeded 15 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_evidence(&fixture)["executions"]
            .as_array()
            .expect("executions")
            .len(),
        1
    );
}

fn raw_gate_response(socket: &Path, request: &Value) -> Value {
    let wire = serde_json::to_vec(request).expect("raw gate request JSON");
    let mut stream = UnixStream::connect(socket).expect("connect raw gate client");
    stream.write_all(&wire).expect("write raw gate request");
    stream
        .shutdown(Shutdown::Write)
        .expect("finish raw gate request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read raw gate response");
    serde_json::from_slice(&response).expect("raw gate response JSON")
}

fn gate_evidence(fixture: &GateFixture) -> Value {
    let path = PathBuf::from(format!("{}.executions.json", fixture.output_path.display()));
    let bytes = wait_for_file_bytes(&path, |bytes| {
        serde_json::from_slice::<Value>(bytes).is_ok()
    });
    serde_json::from_slice(&bytes).expect("gate execution evidence JSON")
}

#[test]
fn gate_dry_run_projects_null_and_plan_routes() {
    let _guard = dispatch_test_guard();
    for (name, plan) in [("null", None), ("plan", Some(b"plan\0\xff".as_slice()))] {
        let fixture = GateFixture::new(name, CLAUDE_SUCCESS);
        let environment = fixture.environment(0);
        let mut argv = fixture.argv(&environment, &["alpha", "beta"]);
        if let Some(bytes) = plan {
            let path = fixture.harness.path().join(format!("{name}.plan"));
            fs::write(&path, bytes).expect("write gate plan");
            let delimiter = argv
                .iter()
                .position(|value| value == "--")
                .expect("delimiter");
            argv.splice(
                delimiter..delimiter,
                ["--plan-file".to_owned(), path.display().to_string()],
            );
        }
        let log = fixture.harness.path().join(format!("{name}.jsonl"));
        insert_gate_logging(&mut argv, &log, true);
        let output = fixture.harness.run(&argv, b"").expect("run gate dry-run");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("projection");
        assert_eq!(value["envelope"]["target"], "gate");
        assert_eq!(value["envelope"]["executable"], "claude");
        assert_eq!(
            value["envelope"]["argv"],
            json!(["-p", "--output-format", "json", "alpha", "beta"])
        );
        assert_eq!(
            value["envelope"]["schema_path"],
            fixture.schema_path.display().to_string()
        );
        assert_eq!(
            value["envelope"]["output_path"],
            fixture.output_path.display().to_string()
        );
        assert_eq!(
            value["envelope"]["stdin"]["bytes"],
            plan.map_or(Value::Null, |bytes| json!(bytes))
        );
        assert!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("dry records")
                .is_empty()
        );
    }
}

#[test]
fn gate_invocation_is_pinned_and_environment_is_exact() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("pinned", CLAUDE_SUCCESS);
    let mut environment = fixture.environment(0);
    environment.push(("EXPLICIT".to_owned(), "value".to_owned()));
    let argv = fixture.argv(&environment, &["one", "two words", "three"]);
    let output = fixture
        .harness
        .run_with_parent_environment(&argv, &[("PARENT_ONLY", "secret")])
        .expect("run pinned gate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let invocation = fixture.invocation();
    assert_eq!(invocation.program, "claude");
    assert_eq!(
        invocation.argv,
        ["-p", "--output-format", "json", "one", "two words", "three"].map(OsString::from)
    );
    assert_eq!(invocation.cwd, fixture.cwd);
    assert_eq!(invocation.stdin, b"");
    let mut expected = environment
        .iter()
        .map(|(name, value)| OsString::from(format!("{name}={value}")))
        .collect::<BTreeSet<_>>();
    expected.extend([
        OsString::from(format!("PWD={}", fixture.cwd.display())),
        OsString::from("SHLVL=1"),
        OsString::from("_=/usr/bin/env"),
    ]);
    assert_environment_with_synthesized_tmpdir(&invocation.environment, &expected);
    assert!(
        !invocation
            .environment
            .contains(&OsString::from("PARENT_ONLY=secret"))
    );
}

#[test]
fn gate_null_and_plan_stdin_are_distinct() {
    let _guard = dispatch_test_guard();
    let null = GateFixture::new("stdin-null", CLAUDE_SUCCESS);
    let output = null
        .harness
        .run(null.argv(&null.environment(0), &[]), b"parent stdin")
        .expect("run null gate");
    assert!(output.status.success());
    assert_eq!(null.invocation().stdin, b"");
    let plan = GateFixture::new("stdin-plan", CLAUDE_SUCCESS);
    let bytes = b"\0plan\xff\n";
    let plan_path = plan.harness.path().join("exact.plan");
    fs::write(&plan_path, bytes).expect("write exact plan");
    let environment = plan.environment(0);
    let mut argv = plan.argv(&environment, &[]);
    let delimiter = argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    argv.splice(
        delimiter..delimiter,
        ["--plan-file".to_owned(), plan_path.display().to_string()],
    );
    let output = plan.harness.run(&argv, b"ignored").expect("run plan gate");
    assert!(output.status.success());
    let invocation = plan.invocation();
    assert_eq!(invocation.stdin, bytes);
    assert_eq!(
        invocation.argv,
        ["-p", "--output-format", "json"].map(OsString::from)
    );
}

#[test]
fn gate_stdout_is_teed_byte_for_byte() {
    let _guard = dispatch_test_guard();
    let bytes = b" \n{\"is_error\":false,\"usage\":{\"input_tokens\":1,\"output_tokens\":2,\"cache_creation_input_tokens\":3,\"cache_read_input_tokens\":4}} \t";
    let fixture = GateFixture::new("tee", bytes);
    let log = fixture.harness.path().join("tee.jsonl");
    let mut argv = fixture.argv(&fixture.environment(0), &[]);
    insert_gate_logging(&mut argv, &log, false);
    let output = fixture.harness.run(&argv, b"").expect("run tee gate");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, b"");
    settle_lifecycle(&log);
    assert_eq!(
        fs::read(dispatch_sidecar(&log, 1, "stdout")).expect("read tee stdout"),
        bytes
    );
    assert_eq!(
        fs::read(dispatch_sidecar(&log, 1, "stderr")).expect("read tee stderr"),
        b""
    );
    assert!(matches!(
        gate_completion(&log).usage,
        DispatchTokenUsage::ClaudeMeasured { .. }
    ));
}

#[test]
fn gate_rejects_explicit_anthropic_api_key() {
    let _guard = dispatch_test_guard();
    for value in ["", "secret"] {
        let fixture = GateFixture::new(&format!("key-{value}"), CLAUDE_SUCCESS);
        let mut environment = fixture.environment(0);
        environment.push(("ANTHROPIC_API_KEY".to_owned(), value.to_owned()));
        let output = fixture
            .harness
            .run(fixture.argv(&environment, &[]), b"")
            .expect("reject key");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("gate dispatch environment must not contain `ANTHROPIC_API_KEY`")
        );
        assert!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("key records")
                .is_empty()
        );
    }
}

#[test]
fn gate_rejects_caller_output_format_spellings() {
    let _guard = dispatch_test_guard();
    for token in ["--output-format", "--output-format=stream-json"] {
        let fixture = GateFixture::new(token, CLAUDE_SUCCESS);
        let output = fixture
            .harness
            .run(fixture.argv(&fixture.environment(0), &[token]), b"")
            .expect("reject format");
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("gate caller arguments must not contain `--output-format`")
        );
        assert!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("format records")
                .is_empty()
        );
    }
    let fixture = GateFixture::new("near-miss", CLAUDE_SUCCESS);
    let mut argv = fixture.argv(
        &fixture.environment(0),
        &[
            "--output-formatting",
            "--no-output-format",
            "prompt mentions --output-format in prose",
        ],
    );
    let log = fixture.harness.path().join("near-miss.jsonl");
    insert_gate_logging(&mut argv, &log, false);
    let evidence = argv
        .iter()
        .position(|value| value == "--evidence")
        .expect("evidence");
    argv[evidence + 1] = "--output-format".to_owned();
    let output = fixture.harness.run(&argv, b"").expect("accept near misses");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fixture.invocation().argv,
        [
            "-p",
            "--output-format",
            "json",
            "--output-formatting",
            "--no-output-format",
            "prompt mentions --output-format in prose"
        ]
        .map(OsString::from)
    );
}

#[test]
fn falsification_critic_dry_run_and_live_share_binary_owned_frame() {
    let _guard = dispatch_test_guard();
    let mut projected = None;
    let fixture = GateFixture::new("falsification path with spaces", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"no demonstrated break"}"#,
    )
    .expect("write approval verdict");
    let client = PathBuf::from(env!("CARGO_BIN_EXE_pce"));
    for dry_run in [true, false] {
        let log = fixture.harness.path().join("falsification.jsonl");
        let mut argv = fixture.argv(&fixture.environment(0), &["probe the built artifact"]);
        insert_gate_logging(&mut argv, &log, dry_run);
        select_falsification_critic(&mut argv);
        let output = fixture
            .harness
            .run(&argv, b"")
            .expect("run falsification critic");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mandate = falsification_mandate(&fixture.output_path, &client);
        let expected = [
            "-p".to_owned(),
            "--output-format".to_owned(),
            "json".to_owned(),
            "--allowedTools".to_owned(),
            format!("Bash({} gate exec:*)", client.display()),
            "Read".to_owned(),
            "Glob".to_owned(),
            "Grep".to_owned(),
            "Write".to_owned(),
            "--append-system-prompt".to_owned(),
            mandate.clone(),
            "probe the built artifact".to_owned(),
        ];
        let actual = if dry_run {
            let value: serde_json::Value =
                serde_json::from_slice(&output.stdout).expect("projection");
            value["envelope"]["argv"]
                .as_array()
                .expect("argv")
                .iter()
                .map(|value| value.as_str().expect("argument").to_owned())
                .collect::<Vec<_>>()
        } else {
            let invocation = fixture.invocation();
            invocation
                .argv
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(actual, expected);
        let acceptance_twin = "For every rejection probe, execute an acceptance probe on the same built artifact whose input differs only in the property under test, and identify that acceptance execution's exact input, returned observed_result, and returned harness reference in the same issue's input and observation.";
        assert_eq!(mandate.matches(acceptance_twin).count(), 1);
        assert_eq!(
            actual
                .iter()
                .filter(|arg| *arg == "--append-system-prompt")
                .count(),
            1
        );
        let mandate = &actual[10];
        for clause in [
            "mutate the subject it claims to test",
            "including when the check belongs to this gate rather than to the subject",
            "delete the configuration entry that activates it",
        ] {
            assert_eq!(mandate.matches(clause).count(), 1, "{clause}");
        }
        assert_eq!(mandate.matches("<output-path>").count(), 1);
        assert_eq!(mandate.matches("</output-path>").count(), 1);
        assert!(!mandate.contains(&fixture.schema_path.display().to_string()));
        if dry_run {
            projected = Some(actual);
            assert!(!log.exists());
            assert!(
                !PathBuf::from(format!("{}.executions.json", fixture.output_path.display()))
                    .exists()
            );
        } else {
            assert_eq!(projected.as_ref().expect("dry projection"), &actual);
            settle_lifecycle(&log);
            let records = fs::read_to_string(&log).expect("event log");
            assert_eq!(records.lines().count(), 2);
            assert!(
                records
                    .lines()
                    .next()
                    .expect("issuance")
                    .contains("\"role\":\"falsification-critic\"")
            );
            let evidence =
                PathBuf::from(format!("{}.executions.json", fixture.output_path.display()));
            assert_eq!(
                wait_for_file_bytes(&evidence, |bytes| {
                    bytes == b"{\"schema_id\":\"pce.gate-execution-evidence\",\"schema_version\":1,\"executions\":[]}\n"
                }),
                b"{\"schema_id\":\"pce.gate-execution-evidence\",\"schema_version\":1,\"executions\":[]}\n"
            );
            assert_eq!(
                fs::metadata(evidence)
                    .expect("evidence metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o444
            );
        }
    }
}

#[test]
fn falsification_recorder_executes_two_requests_and_persists_parent_observations() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("falsification executions", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"dispatch route","problem":"demonstrated break","input":"primary","observation":"exit 7","execution_ref":"execution-000001","required_change":"replacement","replacement_execution":{"input":"replacement","observation":"exit 0","execution_ref":"execution-000002"}}],"non_blocking_notes":[],"summary":"one demonstrated break"}"#,
    )
    .expect("write referenced verdict");

    let working_directory = fixture.cwd.join("worktree");
    let bin = fixture.harness.path().join("fixture-bin");
    let requests = fixture.harness.path().join("gate-exec-requests");
    let responses = fixture.harness.path().join("gate-exec-responses");
    for directory in [&working_directory, &bin, &requests, &responses] {
        fs::create_dir_all(directory).expect("create recorder fixture directory");
    }
    let setup = bin.join("setup");
    let primary = bin.join("probe");
    let replacement = bin.join("replacement");
    let common_checks = format!(
        "[ \"$PWD\" = '{}' ] || exit 91\n[ \"$LANG\" = C ] || exit 92\n",
        working_directory.display()
    );
    fs::write(
        &setup,
        format!(
            "#!/bin/sh\n{common_checks}[ \"$TOKEN\" = setup ] || exit 93\n[ \"$1\" = --prepare ] || exit 94\n[ \"$2\" = 'value with spaces' ] || exit 95\n/bin/cat\nprintf '%s' '-out'\nprintf '%s' 'setup-err' >&2\n"
        ),
    )
    .expect("write setup executable");
    fs::write(
        &primary,
        format!(
            "#!/bin/sh\n{common_checks}[ \"$TOKEN\" = probe ] || exit 93\n[ \"$1\" = --check ] || exit 94\n[ \"$2\" = 'value with spaces' ] || exit 95\n/bin/cat\nprintf '%s' 'probe-err' >&2\nexit 7\n"
        ),
    )
    .expect("write primary executable");
    fs::write(
        &replacement,
        format!(
            "#!/bin/sh\n{common_checks}[ \"$TOKEN\" = replacement ] || exit 93\n[ \"$1\" = --verify ] || exit 94\n[ \"$2\" = 'value with spaces' ] || exit 95\n/bin/cat\nprintf '%s' 'ment-out'\n"
        ),
    )
    .expect("write replacement executable");
    for executable in [&setup, &primary, &replacement] {
        fs::set_permissions(executable, fs::Permissions::from_mode(0o755))
            .expect("make fixture executable");
    }
    let primary_request = json!({
        "working_directory": working_directory,
        "setup": [{
            "program": setup,
            "arguments": ["--prepare", "value with spaces"],
            "input": [115, 101, 116, 117, 112],
            "environment": {"LANG": "C", "TOKEN": "setup"}
        }],
        "command": {
            "program": primary,
            "arguments": ["--check", "value with spaces"],
            "input": [0, 255, 10],
            "environment": {"LANG": "C", "TOKEN": "probe"}
        }
    });
    let replacement_request = json!({
        "working_directory": fixture.cwd.join("worktree"),
        "setup": [],
        "command": {
            "program": replacement,
            "arguments": ["--verify", "value with spaces"],
            "input": [114, 101, 112, 108, 97, 99, 101],
            "environment": {"LANG": "C", "TOKEN": "replacement"}
        }
    });
    fs::write(
        requests.join("0001.json"),
        serde_json::to_vec_pretty(&primary_request).expect("primary request JSON"),
    )
    .expect("write primary request");
    fs::write(
        requests.join("0002.json"),
        serde_json::to_vec_pretty(&replacement_request).expect("replacement request JSON"),
    )
    .expect("write replacement request");

    let mut environment = fixture.environment(0);
    environment.extend([
        (
            "PCE_CLAUDE_GATE_EXEC_REQUEST_DIR".to_owned(),
            requests.display().to_string(),
        ),
        (
            "PCE_CLAUDE_GATE_EXEC_RESPONSE_DIR".to_owned(),
            responses.display().to_string(),
        ),
    ]);
    let mut argv = fixture.argv(&environment, &["probe the built artifact"]);
    let log = fixture
        .harness
        .path()
        .join("falsification-executions.jsonl");
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);
    let output = fixture.harness.run(&argv, b"").expect("run recorder");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log);
    let primary_response: Value =
        serde_json::from_slice(&fs::read(responses.join("0001.json")).expect("primary response"))
            .expect("primary response JSON");
    assert_eq!(primary_response["execution_ref"], "execution-000001");
    assert_eq!(
        primary_response["observed_result"]["setup"][0]["stdout"],
        json!(b"setup-out")
    );
    assert_eq!(
        primary_response["observed_result"]["setup"][0]["stderr"],
        json!(b"setup-err")
    );
    assert_eq!(
        primary_response["observed_result"]["command"]["status"],
        json!({"kind":"exited","code":7})
    );
    assert_eq!(
        primary_response["observed_result"]["command"]["stdout"],
        json!([0, 255, 10])
    );
    assert_eq!(
        primary_response["observed_result"]["command"]["stderr"],
        json!(b"probe-err")
    );
    let replacement_response: Value = serde_json::from_slice(
        &fs::read(responses.join("0002.json")).expect("replacement response"),
    )
    .expect("replacement response JSON");
    assert_eq!(replacement_response["execution_ref"], "execution-000002");
    assert_eq!(
        replacement_response["observed_result"]["command"]["stdout"],
        json!(b"replacement-out")
    );
    assert_eq!(
        replacement_response["observed_result"]["command"]["stderr"],
        json!([])
    );
    let evidence_path = PathBuf::from(format!("{}.executions.json", fixture.output_path.display()));
    let evidence: Value = serde_json::from_slice(&fs::read(&evidence_path).expect("evidence"))
        .expect("evidence JSON");
    assert_eq!(
        evidence["executions"].as_array().expect("executions").len(),
        2
    );
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
    assert_eq!(
        evidence["executions"][1]["execution_ref"],
        "execution-000002"
    );
    assert_eq!(
        fs::metadata(evidence_path)
            .expect("evidence metadata")
            .permissions()
            .mode()
            & 0o777,
        0o444
    );
}

#[test]
fn falsification_recorder_preserves_large_request_through_real_client() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("large falsification execution", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"large request recorded"}"#,
    )
    .expect("write approval verdict");
    let input = (0..200_000)
        .map(|index| u8::try_from(index % 251).expect("bounded input byte"))
        .collect::<Vec<_>>();
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {
            "program": "/bin/sh",
            "arguments": ["-c", "/bin/cat >/dev/null"],
            "input": input,
            "environment": {}
        }
    });
    let (responses, environment) = gate_request_directories(&fixture, &[request]);
    let mut argv = fixture.argv(&environment, &["probe a large input"]);
    let log = fixture.harness.path().join("large-falsification.jsonl");
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);

    let output = fixture
        .harness
        .run(&argv, b"")
        .expect("run large recorder request");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log);
    let response: Value = serde_json::from_slice(
        &fs::read(responses.join("0001.json")).expect("large request response"),
    )
    .expect("large response JSON");
    assert_eq!(response["execution_ref"], "execution-000001");
    assert_eq!(response["observed_result"]["command"]["stdout"], json!([]));

    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
    assert_eq!(
        evidence["executions"][0]["stimulus"]["command"]["input"],
        json!(input)
    );
}

#[test]
fn gate_execution_echoes_large_input_without_deadlock() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("large input echo", CLAUDE_SUCCESS);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "large-input-echo");
    let input = vec![65_u8; 200_000];
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/bin/cat","arguments":[],"input":input,"environment":{}}
    });
    let response = raw_gate_response(&socket, &request);
    let finalization_started = Instant::now();
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(
        child,
        "echoing large-input gate execution exceeded 15 seconds",
    );
    assert!(
        finalization_started.elapsed() < Duration::from_millis(500),
        "large-input evidence finalization exceeded 0.5 seconds"
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(response["execution_ref"], "execution-000001");
    assert_eq!(response["observed_result"]["setup"], json!([]));
    assert_eq!(
        response["observed_result"]["command"]["status"],
        json!({"kind":"exited","code":0})
    );
    assert_eq!(
        response["observed_result"]["command"]["stdout"],
        json!(input)
    );
    assert_eq!(response["observed_result"]["command"]["stderr"], json!([]));
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"][0]["stimulus"]["command"]["input"],
        json!(input)
    );
    assert_eq!(
        evidence["executions"][0]["observed_result"]["command"]["stdout"],
        json!(input)
    );
}

#[test]
fn gate_execution_records_process_that_ignores_large_stdin() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("ignore large stdin", CLAUDE_SUCCESS);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "ignore-large-stdin");
    let input = vec![65_u8; 200_000];
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/usr/bin/true","arguments":[],"input":input,"environment":{}}
    });
    let response = raw_gate_response(&socket, &request);
    assert_eq!(
        response["execution_ref"], "execution-000001",
        "ignoring-stdin stimulus did not return execution-000001"
    );
    assert_eq!(
        response["observed_result"]["command"],
        json!({"status":{"kind":"exited","code":0},"stdout":[],"stderr":[]})
    );
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(
        child,
        "ignoring-stdin stimulus did not return execution-000001",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_evidence(&fixture)["executions"][0]["stimulus"]["command"]["input"],
        json!(input)
    );
}

#[test]
fn gate_execution_drains_three_pipes_concurrently() {
    let _guard = dispatch_test_guard();
    for executable in ["/usr/bin/yes", "/usr/bin/tr", "/usr/bin/head"] {
        assert!(
            Path::new(executable).exists(),
            "required fixture executable {executable} is unavailable"
        );
    }
    let fixture = GateFixture::new("three pipe pressure", CLAUDE_SUCCESS);
    let script = fixture.harness.path().join("three-pipe-fixture.sh");
    fs::write(
        &script,
        b"#!/bin/sh\n/usr/bin/yes O | /usr/bin/tr -d '\\n' | /usr/bin/head -c 200000\n/usr/bin/yes E | /usr/bin/tr -d '\\n' | /usr/bin/head -c 200000 >&2\n/bin/cat\n",
    )
    .expect("write three-pipe fixture");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755))
        .expect("make three-pipe fixture executable");
    let (child, socket, block, _log) = spawn_blocked_falsification_recorder(&fixture, "three-pipe");
    let input = vec![65_u8; 200_000];
    let mut expected_stdout = vec![79_u8; 200_000];
    expected_stdout.extend_from_slice(&input);
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":script,"arguments":[],"input":input,"environment":{}}
    });
    let response = raw_gate_response(&socket, &request);
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(child, "three-pipe gate execution exceeded 15 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(response["execution_ref"], "execution-000001");
    assert_eq!(
        response["observed_result"]["command"]["status"],
        json!({"kind":"exited","code":0})
    );
    assert_eq!(
        response["observed_result"]["command"]["stdout"],
        json!(expected_stdout)
    );
    assert_eq!(
        response["observed_result"]["command"]["stderr"],
        json!(vec![69_u8; 200_000])
    );
}

#[test]
fn gate_execution_records_a_never_reading_process_that_signals_itself() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("never reading process", CLAUDE_SUCCESS);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "never-reading-process");
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/bin/sh","arguments":["-c", "kill -KILL $$"],"input":vec![65_u8; 200_000],"environment":{}}
    });
    let started = Instant::now();
    let response = raw_gate_response(&socket, &request);
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(3),
        "unexpected execution duration {elapsed:?}"
    );
    assert_eq!(response["execution_ref"], "execution-000001");
    assert_eq!(
        response["observed_result"]["command"],
        json!({"status":{"kind":"signaled","signal":9},"stdout":[],"stderr":[]})
    );
    fs::write(&block, []).expect("release Claude child");
    let output =
        wait_for_dispatch_deadline(child, "never-reading gate execution exceeded 15 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_evidence(&fixture)["executions"][0]["observed_result"]["command"]["status"],
        json!({"kind":"signaled","signal":9})
    );
}

#[test]
fn gate_execution_rejects_partial_and_silent_requests_without_reference_skew() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("bounded raw requests", CLAUDE_SUCCESS);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "bounded-raw-requests");
    let exact_rejection = br#"{"error":"gate execution request read timed out after 2 seconds"}"#;

    let mut partial = UnixStream::connect(&socket).expect("connect partial gate client");
    partial
        .set_read_timeout(Some(Duration::from_secs(6)))
        .expect("bound partial response read");
    partial
        .write_all(br#"{"working_directory":"#)
        .expect("write partial gate request");
    let mut rejection = Vec::new();
    partial.read_to_end(&mut rejection).unwrap_or_else(|error| {
        panic!("partial gate request produced no rejection within 6 seconds: {error}")
    });
    assert_eq!(rejection, exact_rejection);

    let mut silent = UnixStream::connect(&socket).expect("connect silent gate client");
    silent
        .set_read_timeout(Some(Duration::from_secs(6)))
        .expect("bound silent response read");
    rejection.clear();
    silent.read_to_end(&mut rejection).unwrap_or_else(|error| {
        panic!("silent gate connection produced no rejection within 6 seconds: {error}")
    });
    assert_eq!(rejection, exact_rejection);

    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/usr/bin/true","arguments":[],"input":[],"environment":{}}
    });
    let response = raw_gate_response(&socket, &request);
    assert_eq!(response["execution_ref"], "execution-000001");
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(child, "bounded raw gate requests exceeded 15 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_evidence(&fixture)["executions"]
            .as_array()
            .expect("executions")
            .len(),
        1
    );
}

#[test]
fn gate_execution_request_read_uses_total_deadline_and_hard_size_cap() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("dribbling raw request", CLAUDE_SUCCESS);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "dribbling-raw-request");
    let mut stream = UnixStream::connect(&socket).expect("connect dribbling gate client");
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .expect("bound dribbling response read");
    let started = Instant::now();
    stream.write_all(b"{").expect("write first dripped byte");
    thread::sleep(Duration::from_millis(1_500));
    stream.write_all(b"\"").expect("write second dripped byte");
    let mut response_bytes = Vec::new();
    stream
        .read_to_end(&mut response_bytes)
        .unwrap_or_else(|error| {
            panic!("dribbling gate request produced no response within 4 seconds: {error}")
        });
    assert_eq!(
        response_bytes,
        br#"{"error":"gate execution request read timed out after 2 seconds"}"#
    );
    assert!(
        started.elapsed() <= Duration::from_millis(2_250),
        "dribbling gate request exceeded the total read deadline: {:?}",
        started.elapsed()
    );

    let mut oversized = UnixStream::connect(&socket).expect("connect oversized gate client");
    oversized
        .set_read_timeout(Some(Duration::from_secs(6)))
        .expect("bound oversized response read");
    oversized
        .write_all(&vec![b' '; 16 * 1024 * 1024 + 1])
        .expect("write oversized gate request");
    if let Err(error) = oversized.shutdown(Shutdown::Write)
        && !matches!(
            error.kind(),
            std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::NotConnected
        )
    {
        panic!("finish oversized gate request: {error}");
    }
    let mut rejection = Vec::new();
    oversized
        .read_to_end(&mut rejection)
        .unwrap_or_else(|error| {
            panic!("oversized gate request produced no rejection within 6 seconds: {error}")
        });
    assert_eq!(
        rejection,
        br#"{"error":"gate execution request exceeds 16777216 bytes"}"#
    );

    let response = raw_gate_response(
        &socket,
        &json!({
            "working_directory": fixture.cwd,
            "setup": [],
            "command": {"program":"/usr/bin/true","arguments":[],"input":[],"environment":{}}
        }),
    );
    assert_eq!(response["execution_ref"], "execution-000001");
    fs::write(&block, []).expect("release Claude child");
    let output =
        wait_for_dispatch_deadline(child, "dribbling raw gate request exceeded 15 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_evidence(&fixture)["executions"]
            .as_array()
            .expect("executions")
            .len(),
        1
    );
}

#[test]
fn gate_execution_allocates_concurrent_partial_requests_in_parse_order() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("concurrent partial requests", CLAUDE_SUCCESS);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "concurrent-partial-requests");
    let requests = [1_u8, 2, 3].map(|byte| {
        serde_json::to_vec(&json!({
            "working_directory": fixture.cwd,
            "setup": [],
            "command": {"program":"/usr/bin/true","arguments":[],"input":[byte],"environment":{}}
        }))
        .expect("partial request JSON")
    });
    let mut streams = [
        UnixStream::connect(&socket).expect("connect partial client one"),
        UnixStream::connect(&socket).expect("connect partial client two"),
        UnixStream::connect(&socket).expect("connect partial client three"),
    ];
    for (stream, request) in streams.iter_mut().zip(&requests) {
        stream
            .write_all(&request[..24])
            .expect("write request prefix");
    }
    thread::sleep(Duration::from_millis(100));
    let mut finish = |index: usize| -> Value {
        streams[index]
            .write_all(&requests[index][24..])
            .expect("finish partial request");
        streams[index]
            .shutdown(Shutdown::Write)
            .expect("finish partial request write half");
        let mut bytes = Vec::new();
        streams[index]
            .read_to_end(&mut bytes)
            .expect("read partial request response");
        serde_json::from_slice(&bytes).expect("partial request response JSON")
    };
    let second = finish(1);
    let first = finish(0);
    let third = finish(2);
    assert_eq!(second["execution_ref"], "execution-000001");
    assert_eq!(first["execution_ref"], "execution-000002");
    assert_eq!(third["execution_ref"], "execution-000003");
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(
        child,
        "concurrent partial gate requests exceeded 15 seconds",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
    assert_eq!(
        evidence["executions"][1]["execution_ref"],
        "execution-000002"
    );
    assert_eq!(
        evidence["executions"][2]["execution_ref"],
        "execution-000003"
    );
}

#[test]
fn gate_execution_allocates_unique_refs_for_simultaneous_requests() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("simultaneous complete requests", CLAUDE_SUCCESS);
    let (child, socket, block, _log) =
        spawn_blocked_falsification_recorder(&fixture, "simultaneous-complete-requests");
    let request = serde_json::to_vec(&json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/bin/sh","arguments":["-c","/bin/sleep 0.5"],"input":[],"environment":{}}
    }))
    .expect("simultaneous request JSON");
    let mut first = UnixStream::connect(&socket).expect("connect first simultaneous client");
    let mut second = UnixStream::connect(&socket).expect("connect second simultaneous client");
    first
        .write_all(&request)
        .expect("write first simultaneous request");
    first
        .shutdown(Shutdown::Write)
        .expect("finish first simultaneous request");
    second
        .write_all(&request)
        .expect("write second simultaneous request");
    second
        .shutdown(Shutdown::Write)
        .expect("finish second simultaneous request");
    let mut first_bytes = Vec::new();
    let mut second_bytes = Vec::new();
    first
        .read_to_end(&mut first_bytes)
        .expect("read first simultaneous response");
    second
        .read_to_end(&mut second_bytes)
        .expect("read second simultaneous response");
    let first: Value =
        serde_json::from_slice(&first_bytes).expect("first simultaneous response JSON");
    let second: Value =
        serde_json::from_slice(&second_bytes).expect("second simultaneous response JSON");
    assert_ne!(
        first["execution_ref"], second["execution_ref"],
        "concurrent gate requests received duplicate execution references"
    );
    let mut references = vec![
        first["execution_ref"].as_str().expect("first reference"),
        second["execution_ref"].as_str().expect("second reference"),
    ];
    references.sort_unstable();
    assert_eq!(references, ["execution-000001", "execution-000002"]);
    fs::write(&block, []).expect("release Claude child");
    let output =
        wait_for_dispatch_deadline(child, "simultaneous gate requests exceeded 15 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
    assert_eq!(
        evidence["executions"][1]["execution_ref"],
        "execution-000002"
    );
}

#[test]
fn unread_large_gate_response_cannot_suppress_record() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("unread large response", CLAUDE_SUCCESS);
    let (child, socket, block, log) =
        spawn_blocked_falsification_recorder(&fixture, "unread-large-response");
    let input = vec![65_u8; 200_000];
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/bin/cat","arguments":[],"input":input,"environment":{}}
    });
    let wire = serde_json::to_vec(&request).expect("unread request JSON");
    let mut stream = UnixStream::connect(&socket).expect("connect unread-response client");
    stream
        .write_all(&wire)
        .expect("write unread-response request");
    stream
        .shutdown(Shutdown::Write)
        .expect("finish unread-response request");
    thread::sleep(Duration::from_secs(2));
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(
        child,
        "unread gate response suppressed the record within 15 seconds",
    );
    drop(stream);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_completion(&log).artifact_outcome,
        ArtifactOutcome::Validated
    );
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
    assert_eq!(
        evidence["executions"][0]["stimulus"]["command"]["input"],
        json!(input)
    );
    assert_eq!(
        evidence["executions"][0]["observed_result"]["command"]["stdout"],
        json!(input)
    );
}

#[test]
fn gate_execution_connection_limit_rejects_without_allocating() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("connection limit", CLAUDE_SUCCESS);
    let (child, socket, block, log) =
        spawn_blocked_falsification_recorder(&fixture, "connection-limit");
    let mut peers = (0..33)
        .map(|_| UnixStream::connect(&socket).expect("connect held gate peer"))
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(4);
    let exact = br#"{"error":"gate execution recorder connection limit reached"}"#;
    let mut delivered = false;
    while !delivered && Instant::now() < deadline {
        for peer in &mut peers {
            peer.set_nonblocking(true)
                .expect("make held peer nonblocking");
            let mut bytes = Vec::new();
            match peer.read_to_end(&mut bytes) {
                Ok(_) if bytes == exact => {
                    delivered = true;
                    break;
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("read connection-limit rejection: {error}"),
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(
        delivered,
        "connection-limit rejection was not delivered within 4 seconds"
    );
    drop(peers);
    thread::sleep(Duration::from_millis(100));
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/usr/bin/true","arguments":[],"input":[],"environment":{}}
    });
    let response = raw_gate_response(&socket, &request);
    assert_eq!(response["execution_ref"], "execution-000001");
    fs::write(&block, []).expect("release Claude child");
    let output = wait_for_dispatch_deadline(child, "connection-limit dispatch exceeded 15 seconds");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_completion(&log).artifact_outcome,
        ArtifactOutcome::Validated
    );
    assert_eq!(
        gate_evidence(&fixture)["executions"]
            .as_array()
            .expect("executions")
            .len(),
        1
    );
}

#[test]
fn falsification_recorder_preserves_setup_spawn_and_signal_observations() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("terminal observations", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"observed"}"#,
    )
    .expect("write approval verdict");
    let missing_program = fixture.harness.path().join("does-not-exist");
    let requests = [
        json!({
            "working_directory": fixture.cwd,
            "setup": [{
                "program": "/bin/sh",
                "arguments": ["-c", "exit 23"],
                "input": [],
                "environment": {}
            }],
            "command": {
                "program": "/bin/true",
                "arguments": [],
                "input": [],
                "environment": {}
            }
        }),
        json!({
            "working_directory": fixture.cwd,
            "setup": [],
            "command": {
                "program": missing_program,
                "arguments": [],
                "input": [],
                "environment": {}
            }
        }),
        json!({
            "working_directory": fixture.cwd,
            "setup": [],
            "command": {
                "program": "/bin/sh",
                "arguments": ["-c", "kill -9 \"$$\""],
                "input": [],
                "environment": {}
            }
        }),
    ];
    let (responses, environment) = gate_request_directories(&fixture, &requests);
    let log = fixture.harness.path().join("terminal-observations.jsonl");
    let mut argv = fixture.argv(&environment, &["probe"]);
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);
    let output = fixture.harness.run(&argv, b"").expect("run exact role");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log);
    let response = |name: &str| -> Value {
        serde_json::from_slice(&fs::read(responses.join(name)).expect("gate response"))
            .expect("gate response JSON")
    };
    let setup_failed = response("0001.json");
    assert_eq!(setup_failed["execution_ref"], "execution-000001");
    assert_eq!(
        setup_failed["observed_result"]["setup"][0]["status"],
        json!({"kind":"exited","code":23})
    );
    assert!(setup_failed["observed_result"]["command"].is_null());
    let spawn_failed = response("0002.json");
    assert_eq!(spawn_failed["execution_ref"], "execution-000002");
    assert_eq!(
        spawn_failed["observed_result"]["command"]["status"],
        json!({"kind":"spawn-failed","detail":"No such file or directory (os error 2)"})
    );
    assert_eq!(
        spawn_failed["observed_result"]["command"]["stdout"],
        json!([])
    );
    assert_eq!(
        spawn_failed["observed_result"]["command"]["stderr"],
        json!([])
    );
    let signaled = response("0003.json");
    assert_eq!(signaled["execution_ref"], "execution-000003");
    assert_eq!(
        signaled["observed_result"]["command"]["status"],
        json!({"kind":"signaled","signal":9})
    );
    assert!(
        signaled["observed_result"]["command"]["status"]
            .get("code")
            .is_none()
    );
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"].as_array().expect("executions").len(),
        3
    );
    assert_eq!(
        evidence["executions"][0]["observed_result"]["setup"][0]["status"],
        json!({"kind":"exited","code":23})
    );
    assert!(evidence["executions"][0]["observed_result"]["command"].is_null());
    assert_eq!(
        evidence["executions"][1]["observed_result"]["command"]["status"],
        json!({"kind":"spawn-failed","detail":"No such file or directory (os error 2)"})
    );
    assert_eq!(
        evidence["executions"][1]["observed_result"]["command"]["stdout"],
        json!([])
    );
    assert_eq!(
        evidence["executions"][1]["observed_result"]["command"]["stderr"],
        json!([])
    );
    assert_eq!(
        evidence["executions"][2]["observed_result"]["command"]["status"],
        json!({"kind":"signaled","signal":9})
    );
}

#[test]
fn falsification_reference_admission_uses_binary_owned_verdict_schema() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("installed verdict schema", CLAUDE_SUCCESS);
    let installed_schema = br#"{
      "type":"object",
      "additionalProperties":false,
      "required":["verdict","self_sufficiency","root_cause","blocking_issues","non_blocking_notes","summary"],
      "properties":{
        "verdict":{"type":"string","enum":["APPROVE","REVISE","BLOCK"]},
        "self_sufficiency":{"type":"string","enum":["PASS","FAIL","NOT_APPLICABLE"]},
        "root_cause":{"type":"string","enum":["execution","step_plan","milestone_plan","vision"]},
        "blocking_issues":{"type":"array","items":{
          "type":"object",
          "additionalProperties":false,
          "required":["id","severity","location","problem","required_change"],
          "properties":{
            "id":{"type":"string"},
            "severity":{"type":"string","enum":["critical","major"]},
            "location":{"type":"string"},
            "problem":{"type":"string"},
            "required_change":{"type":"string"}
          }
        }},
        "non_blocking_notes":{"type":"array","items":{"type":"string"}},
        "summary":{"type":"string"}
      }
    }"#;
    fs::write(&fixture.schema_path, installed_schema).expect("write five-field installed schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"REVISE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"dispatch route","problem":"demonstrated break","input":"primary","observation":"exit 1","execution_ref":"execution-000001","required_change":"replacement","replacement_execution":{"input":"replacement","observation":"exit 0","execution_ref":"execution-000001"}}],"non_blocking_notes":[],"summary":"demonstrated and repaired"}"#,
    )
    .expect("write binary-schema verdict");
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {"program":"/usr/bin/true","arguments":[],"input":[],"environment":{}}
    });
    let (_responses, environment) = gate_request_directories(&fixture, &[request]);
    let log = fixture
        .harness
        .path()
        .join("installed-schema-reference-admission.jsonl");
    let mut argv = fixture.argv(&environment, &["probe"]);
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);

    let output = fixture
        .harness
        .run(&argv, b"")
        .expect("run against five-field installed schema");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_completion(&log).artifact_outcome,
        ArtifactOutcome::Validated
    );
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"].as_array().expect("executions").len(),
        1
    );
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
}

#[test]
fn falsification_reference_admission_does_not_read_the_installed_verdict_schema() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("missing installed verdict schema", CLAUDE_SUCCESS);
    fs::remove_file(&fixture.schema_path).expect("remove installed verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"binary-owned schema"}"#,
    )
    .expect("write binary-schema verdict");
    let log = fixture
        .harness
        .path()
        .join("missing-schema-reference-admission.jsonl");
    let environment = fixture.environment(0);
    let mut argv = fixture.argv(&environment, &["probe"]);
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);

    let output = fixture
        .harness
        .run(&argv, b"")
        .expect("run without installed verdict schema");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        gate_completion(&log).artifact_outcome,
        ArtifactOutcome::Validated
    );
}

#[test]
fn falsification_reference_admission_uses_only_retained_same_dispatch_records() {
    let _guard = dispatch_test_guard();
    let issue = |id: &str, primary: Option<&str>, replacement: Option<&str>| {
        let mut issue = json!({
            "id": id,
            "severity": "major",
            "location": "dispatch route",
            "problem": "claimed break",
            "input": "primary",
            "observation": "claimed",
            "required_change": "replacement",
            "replacement_execution": {
                "input": "replacement",
                "observation": "claimed"
            }
        });
        if let Some(primary) = primary {
            issue["execution_ref"] = json!(primary);
        }
        if let Some(replacement) = replacement {
            issue["replacement_execution"]["execution_ref"] = json!(replacement);
        }
        issue
    };
    let cases = [
        (
            "unknown replacement",
            vec![issue(
                "F-1",
                Some("execution-000001"),
                Some("execution-000003"),
            )],
            "blocking issue `F-1` replacement references unknown gate execution `execution-000003`",
        ),
        (
            "unknown neighboring issue",
            vec![
                issue("F-1", Some("execution-000001"), Some("execution-000002")),
                issue("F-2", Some("execution-000003"), Some("execution-000002")),
            ],
            "blocking issue `F-2` references unknown gate execution `execution-000003`",
        ),
        (
            "missing primary",
            vec![issue("F-1", None, Some("execution-000002"))],
            "violates schema keyword/location `required` at instance `/blocking_issues/0`: \"execution_ref\" is a required property",
        ),
        (
            "missing replacement",
            vec![issue("F-1", Some("execution-000001"), None)],
            "violates schema keyword/location `required` at instance `/blocking_issues/0/replacement_execution`: \"execution_ref\" is a required property",
        ),
        (
            "zero reference",
            vec![issue(
                "F-1",
                Some("execution-000000"),
                Some("execution-000002"),
            )],
            "invalid gate execution reference `execution-000000`",
        ),
    ];
    for (name, blocking_issues, diagnostic) in cases {
        let fixture = GateFixture::new(name, CLAUDE_SUCCESS);
        fs::write(
            &fixture.schema_path,
            include_bytes!("../skills/pce/schemas/verdict.schema.json"),
        )
        .expect("write tracked verdict schema");
        fs::write(
            &fixture.output_path,
            serde_json::to_vec(&json!({
                "verdict": "REVISE",
                "self_sufficiency": "NOT_APPLICABLE",
                "root_cause": "execution",
                "blocking_issues": blocking_issues,
                "non_blocking_notes": [],
                "summary": "claimed break"
            }))
            .expect("verdict JSON"),
        )
        .expect("write reference verdict");
        let requests = [
            json!({
                "working_directory": fixture.cwd,
                "setup": [],
                "command": {"program":"/bin/true","arguments":[],"input":[],"environment":{}}
            }),
            json!({
                "working_directory": fixture.cwd,
                "setup": [],
                "command": {"program":"/bin/true","arguments":[],"input":[],"environment":{}}
            }),
        ];
        let (_responses, environment) = gate_request_directories(&fixture, &requests);
        let log = fixture.harness.path().join("reference-admission.jsonl");
        let mut argv = fixture.argv(&environment, &["probe"]);
        insert_gate_logging(&mut argv, &log, false);
        select_falsification_critic(&mut argv);
        let output = fixture.harness.run(&argv, b"").expect("run exact role");
        assert!(output.status.success(), "{name} starter failed");
        settle_lifecycle(&log);
        let continuation_stderr =
            wait_for_file_bytes(&dispatch_sidecar(&log, 1, "stderr"), |bytes| {
                String::from_utf8_lossy(bytes).contains(diagnostic)
            });
        assert!(
            String::from_utf8_lossy(&continuation_stderr).contains(diagnostic),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            gate_completion(&log).artifact_outcome,
            ArtifactOutcome::SchemaViolating,
            "{name}"
        );
        let evidence: Value = serde_json::from_slice(
            &fs::read(format!("{}.executions.json", fixture.output_path.display()))
                .expect("durable evidence"),
        )
        .expect("evidence JSON");
        assert_eq!(
            evidence["executions"].as_array().expect("executions").len(),
            2,
            "{name}"
        );
    }
}

#[test]
fn falsification_evidence_destination_is_never_overwritten() {
    let _guard = dispatch_test_guard();
    for raced in [false, true] {
        let fixture = GateFixture::new(
            if raced {
                "raced evidence"
            } else {
                "preexisting evidence"
            },
            CLAUDE_SUCCESS,
        );
        fs::write(
            &fixture.schema_path,
            include_bytes!("../skills/pce/schemas/verdict.schema.json"),
        )
        .expect("write tracked verdict schema");
        fs::write(
            &fixture.output_path,
            br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"none"}"#,
        )
        .expect("write approval");
        let evidence = PathBuf::from(format!("{}.executions.json", fixture.output_path.display()));
        let mut environment = fixture.environment(0);
        if raced {
            environment.extend([
                (
                    "PCE_CLAUDE_GATE_EXEC_RACE_EVIDENCE".to_owned(),
                    "1".to_owned(),
                ),
                (
                    "PCE_CLAUDE_OUTPUT_PATH".to_owned(),
                    fixture.output_path.display().to_string(),
                ),
            ]);
        } else {
            fs::write(&evidence, b"preexisting").expect("write preexisting evidence");
        }
        let log = fixture.harness.path().join("immutable.jsonl");
        let mut argv = fixture.argv(&environment, &["probe"]);
        insert_gate_logging(&mut argv, &log, false);
        select_falsification_critic(&mut argv);
        let output = fixture.harness.run(&argv, b"").expect("run immutable case");
        assert!(output.status.success());
        if raced {
            settle_lifecycle(&log);
        }
        let diagnostic = wait_for_file_bytes(&dispatch_sidecar(&log, 1, "stderr"), |bytes| {
            String::from_utf8_lossy(bytes).contains("gate execution evidence path already exists")
        });
        assert!(
            String::from_utf8_lossy(&diagnostic)
                .contains("gate execution evidence path already exists")
        );
        assert_eq!(
            fs::read(&evidence).expect("preserved evidence bytes"),
            if raced {
                b"critic-authored".as_slice()
            } else {
                b"preexisting".as_slice()
            }
        );
        let invocation_count = fixture
            .harness
            .claude_invocations(&fixture.record_root)
            .expect("invocations")
            .len();
        assert_eq!(invocation_count, usize::from(raced));
    }
}

#[test]
fn gate_exec_client_rejects_typed_input_before_connecting() {
    let valid = br#"{"working_directory":"/tmp","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{}}}"#;
    let cases: [(&[u8], Option<&str>, &str); 4] = [
        (
            valid,
            None,
            "PCE_GATE_EXEC_SOCKET is required for `pce gate exec`",
        ),
        (
            br#"{"working_directory":"relative","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{}}}"#,
            Some("/tmp/does-not-exist.sock"),
            "gate execution working directory must be absolute",
        ),
        (
            br#"{"working_directory":"/tmp","setup":[],"command":{"program":"","arguments":[],"input":[],"environment":{}}}"#,
            Some("/tmp/does-not-exist.sock"),
            "gate execution program must not be empty",
        ),
        (
            br#"{"working_directory":"/tmp","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{"":"value"}}}"#,
            Some("/tmp/does-not-exist.sock"),
            "gate execution environment name must not be empty",
        ),
    ];
    for (request, socket, diagnostic) in cases {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
        command.args(["gate", "exec"]);
        command.env_clear();
        if let Some(socket) = socket {
            command.env("PCE_GATE_EXEC_SOCKET", socket);
        }
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let mut child = command.spawn().expect("spawn gate exec client");
        child
            .stdin
            .take()
            .expect("client stdin")
            .write_all(request)
            .expect("write request");
        let output = child.wait_with_output().expect("client output");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, format!("{diagnostic}\n").as_bytes());
    }
}

#[test]
fn raw_socket_request_repeats_parent_validation_without_allocating_a_reference() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("raw parent rejection", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"none"}"#,
    )
    .expect("write approval");
    let block = fixture.harness.path().join("release-claude");
    let mut environment = fixture.environment(0);
    environment.push((
        "PCE_CLAUDE_BLOCK_FILE".to_owned(),
        block.display().to_string(),
    ));
    let log = fixture.harness.path().join("raw-parent.jsonl");
    let mut argv = fixture.argv(&environment, &["probe"]);
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);
    let mut parent = Command::new(env!("CARGO_BIN_EXE_pce"));
    parent.args(&argv);
    parent.stdin(Stdio::null());
    parent.stdout(Stdio::piped());
    parent.stderr(Stdio::piped());
    let child = parent.spawn().expect("spawn parent dispatch");
    let environment_path = fixture.record_root.join("invocation/environment.bin");
    let ready_path = fixture.record_root.join("invocation/pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready_path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let environment_bytes = fs::read(&environment_path).expect("recorded Claude environment");
    let socket = environment_bytes
        .split(|byte| *byte == 0)
        .find_map(|entry| {
            entry
                .strip_prefix(b"PCE_GATE_EXEC_SOCKET=")
                .map(|value| PathBuf::from(OsString::from_vec(value.to_vec())))
        })
        .expect("socket environment entry");
    let mut stream = UnixStream::connect(&socket).expect("connect raw socket client");
    stream
        .write_all(br#"{"working_directory":"relative","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{}}}"#)
        .expect("write raw request");
    stream
        .shutdown(Shutdown::Write)
        .expect("finish raw request");
    let mut rejection = Vec::new();
    stream.read_to_end(&mut rejection).expect("read rejection");
    fs::write(&block, []).expect("release Claude child");
    let output = child.wait_with_output().expect("parent output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log);
    assert_eq!(
        rejection,
        br#"{"error":"gate execution working directory must be absolute"}"#
    );
    assert_eq!(
        wait_for_file_bytes(
            &PathBuf::from(format!("{}.executions.json", fixture.output_path.display())),
            |bytes| {
                bytes == b"{\"schema_id\":\"pce.gate-execution-evidence\",\"schema_version\":1,\"executions\":[]}\n"
            }
        ),
        b"{\"schema_id\":\"pce.gate-execution-evidence\",\"schema_version\":1,\"executions\":[]}\n"
    );
    assert!(!socket.exists());
}

#[test]
fn hung_up_gate_client_cannot_suppress_completion_or_evidence() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("hung-up client", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"none"}"#,
    )
    .expect("write approval");
    let block = fixture.harness.path().join("release-hung-up-claude");
    let mut environment = fixture.environment(0);
    environment.push((
        "PCE_CLAUDE_BLOCK_FILE".to_owned(),
        block.display().to_string(),
    ));
    let log = fixture.harness.path().join("hung-up-client.jsonl");
    let mut argv = fixture.argv(&environment, &["probe"]);
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);
    let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
    command.args(&argv);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let child = command.spawn().expect("spawn parent dispatch");
    let environment_path = fixture.record_root.join("invocation/environment.bin");
    let ready_path = fixture.record_root.join("invocation/pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready_path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let environment_bytes = fs::read(&environment_path).expect("recorded Claude environment");
    let socket = environment_bytes
        .split(|byte| *byte == 0)
        .find_map(|entry| {
            entry
                .strip_prefix(b"PCE_GATE_EXEC_SOCKET=")
                .map(|value| PathBuf::from(OsString::from_vec(value.to_vec())))
        })
        .expect("socket environment entry");
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {
            "program": "/bin/sh",
            "arguments": ["-c", "/bin/sleep 0.1"],
            "input": [],
            "environment": {}
        }
    });
    let mut stream = UnixStream::connect(&socket).expect("connect raw socket client");
    stream
        .write_all(&serde_json::to_vec(&request).expect("raw request JSON"))
        .expect("write raw request");
    stream
        .shutdown(Shutdown::Both)
        .expect("hang up raw request");
    drop(stream);
    thread::sleep(Duration::from_millis(250));
    fs::write(&block, []).expect("release Claude child");
    let output = child.wait_with_output().expect("parent output");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    settle_lifecycle(&log);
    assert_eq!(
        gate_completion(&log).artifact_outcome,
        ArtifactOutcome::Validated
    );
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"].as_array().expect("executions").len(),
        1,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
    assert!(!socket.exists());
}

#[test]
fn long_running_gate_stimulus_has_bounded_shutdown_and_durable_evidence() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("long-running stimulus", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write tracked verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"none"}"#,
    )
    .expect("write approval");
    let block = fixture.harness.path().join("release-long-running-claude");
    let stimulus_release = fixture.harness.path().join("release-long-running-stimulus");
    let mut environment = fixture.environment(0);
    environment.push((
        "PCE_CLAUDE_BLOCK_FILE".to_owned(),
        block.display().to_string(),
    ));
    let log = fixture.harness.path().join("long-running-stimulus.jsonl");
    let mut argv = fixture.argv(&environment, &["probe"]);
    insert_gate_logging(&mut argv, &log, false);
    select_falsification_critic(&mut argv);
    let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
    command.args(&argv);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let child = command.spawn().expect("spawn parent dispatch");
    let environment_path = fixture.record_root.join("invocation/environment.bin");
    let ready_path = fixture.record_root.join("invocation/pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ready_path.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    let environment_bytes = fs::read(&environment_path).expect("recorded Claude environment");
    let socket = environment_bytes
        .split(|byte| *byte == 0)
        .find_map(|entry| {
            entry
                .strip_prefix(b"PCE_GATE_EXEC_SOCKET=")
                .map(|value| PathBuf::from(OsString::from_vec(value.to_vec())))
        })
        .expect("socket environment entry");
    let request = json!({
        "working_directory": fixture.cwd,
        "setup": [],
        "command": {
            "program": "/bin/sh",
            "arguments": [
                "-c",
                "while [ ! -e \"$1\" ]; do /bin/sleep 0.01; done",
                "gate-long-running",
                stimulus_release
            ],
            "input": [],
            "environment": {}
        }
    });
    let mut stream = UnixStream::connect(&socket).expect("connect raw socket client");
    stream
        .write_all(&serde_json::to_vec(&request).expect("raw request JSON"))
        .expect("write raw request");
    stream
        .shutdown(Shutdown::Write)
        .expect("finish long-running raw request");
    drop(stream);
    thread::sleep(Duration::from_millis(100));
    let started_shutdown = Instant::now();
    fs::write(&block, []).expect("release Claude child");
    let output =
        wait_for_dispatch_deadline(child, "bounded-shutdown gate execution exceeded 15 seconds");
    wait_for_lifecycle_records(&log, 2, Instant::now() + Duration::from_secs(15));
    let shutdown_duration = started_shutdown.elapsed();
    fs::write(&stimulus_release, []).expect("release detached stimulus");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        shutdown_duration < Duration::from_secs(8),
        "shutdown took {shutdown_duration:?}"
    );
    assert_eq!(
        gate_completion(&log).artifact_outcome,
        ArtifactOutcome::Validated
    );
    let evidence = gate_evidence(&fixture);
    assert_eq!(
        evidence["executions"].as_array().expect("executions").len(),
        1
    );
    assert_eq!(
        evidence["executions"][0]["execution_ref"],
        "execution-000001"
    );
    assert_eq!(
        evidence["executions"][0]["observed_result"]["command"]["status"],
        json!({"kind":"signaled","signal":9})
    );
    assert_eq!(
        evidence["executions"][0]["observed_result"]["command"]["stdout"],
        json!([])
    );
    assert_eq!(
        evidence["executions"][0]["observed_result"]["command"]["stderr"],
        json!([])
    );
    assert!(!socket.exists());
}

#[test]
fn falsification_critic_rejects_caller_owned_system_prompt_before_issuance() {
    let _guard = dispatch_test_guard();
    for (token, diagnostic) in [
        (
            "--append-system-prompt",
            "falsification-critic caller arguments must not contain `--append-system-prompt`",
        ),
        (
            "--append-system-prompt=caller-value",
            "falsification-critic caller arguments must not contain `--append-system-prompt`",
        ),
        (
            "--allowedTools",
            "falsification-critic caller arguments must not contain `--allowedTools`",
        ),
        (
            "--allowedTools=Bash",
            "falsification-critic caller arguments must not contain `--allowedTools`",
        ),
    ] {
        let fixture = GateFixture::new("falsification reject", CLAUDE_SUCCESS);
        let log = fixture.harness.path().join("rejected.jsonl");
        let mut argv = fixture.argv(&fixture.environment(0), &[token]);
        insert_gate_logging(&mut argv, &log, false);
        select_falsification_critic(&mut argv);
        let output = fixture
            .harness
            .run(&argv, b"")
            .expect("reject caller frame");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        assert!(!log.exists() || fs::read(&log).expect("log bytes").is_empty());
        assert!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("invocations")
                .is_empty()
        );
    }
}

#[test]
fn falsification_critic_rejects_reserved_environment_before_issuance() {
    let _guard = dispatch_test_guard();
    for (name, diagnostic) in [
        (
            "PCE_GATE_EXEC_CLIENT",
            "falsification-critic environment must not supply binary-owned `PCE_GATE_EXEC_CLIENT`",
        ),
        (
            "PCE_GATE_EXEC_SOCKET",
            "falsification-critic environment must not supply binary-owned `PCE_GATE_EXEC_SOCKET`",
        ),
    ] {
        let fixture = GateFixture::new(name, CLAUDE_SUCCESS);
        let mut environment = fixture.environment(0);
        environment.push((name.to_owned(), "caller-owned".to_owned()));
        let log = fixture.harness.path().join("reserved.jsonl");
        let mut argv = fixture.argv(&environment, &["probe"]);
        insert_gate_logging(&mut argv, &log, false);
        select_falsification_critic(&mut argv);
        let output = fixture
            .harness
            .run(&argv, b"")
            .expect("reject reserved env");
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        assert!(!log.exists());
        assert!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("invocations")
                .is_empty()
        );
    }
}

#[test]
fn gate_parent_anthropic_key_is_absent_from_exact_child_environment() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("parent-key", CLAUDE_SUCCESS);
    let environment = fixture.environment(0);
    let argv = fixture.argv(&environment, &[]);
    let output = fixture
        .harness
        .run_with_parent_environment(&argv, &[("ANTHROPIC_API_KEY", "parent-secret")])
        .expect("run parent-key gate");
    assert!(output.status.success());
    let invocation = fixture.invocation();
    let mut expected = environment
        .iter()
        .map(|(name, value)| OsString::from(format!("{name}={value}")))
        .collect::<BTreeSet<_>>();
    expected.extend([
        OsString::from(format!("PWD={}", fixture.cwd.display())),
        OsString::from("SHLVL=1"),
        OsString::from("_=/usr/bin/env"),
    ]);
    assert_environment_with_synthesized_tmpdir(&invocation.environment, &expected);
    assert!(
        !invocation
            .environment
            .contains(&OsString::from("ANTHROPIC_API_KEY=parent-secret"))
    );
}

#[test]
fn gate_parser_rejections_precede_invocation() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("parser", CLAUDE_SUCCESS);
    let cwd = fixture.cwd.display().to_string();
    let schema = fixture.schema_path.display().to_string();
    let output = fixture.output_path.display().to_string();
    let base = || {
        vec![
            "dispatch".to_owned(),
            "gate".to_owned(),
            "--cwd".to_owned(),
            cwd.clone(),
        ]
    };
    let cases: Vec<(&str, Vec<String>, &str)> = vec![
        (
            "relative-cwd",
            vec![
                "dispatch",
                "gate",
                "--cwd",
                "relative",
                "--output-schema",
                &schema,
                "-o",
                &output,
                "--",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            "working directory must be absolute",
        ),
        (
            "malformed-env",
            [
                base(),
                vec![
                    "--env".to_owned(),
                    "broken".to_owned(),
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "environment entry must contain `=`",
        ),
        (
            "empty-env",
            [
                base(),
                vec![
                    "--env".to_owned(),
                    "=x".to_owned(),
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "environment name must not be empty",
        ),
        (
            "duplicate-env",
            [
                base(),
                vec![
                    "--env".to_owned(),
                    "A=1".to_owned(),
                    "--env".to_owned(),
                    "A=2".to_owned(),
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "duplicate environment name `A`",
        ),
        (
            "key",
            [
                base(),
                vec![
                    "--env".to_owned(),
                    "ANTHROPIC_API_KEY=".to_owned(),
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "gate dispatch environment must not contain `ANTHROPIC_API_KEY`",
        ),
        (
            "absent-schema",
            [base(), vec!["--".to_owned()]].concat(),
            "expected `--output-schema`",
        ),
        (
            "schema-order",
            [
                base(),
                vec![
                    "-o".to_owned(),
                    output.clone(),
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "expected `--output-schema`",
        ),
        (
            "relative-schema",
            [
                base(),
                vec![
                    "--output-schema".to_owned(),
                    "relative".to_owned(),
                    "-o".to_owned(),
                    output.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "schema path must be absolute",
        ),
        (
            "unpaired-output",
            [
                base(),
                vec![
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "expected `-o`",
        ),
        (
            "relative-output",
            [
                base(),
                vec![
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    "relative".to_owned(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "output path must be absolute",
        ),
        (
            "sandbox",
            [
                base(),
                vec![
                    "--sandbox".to_owned(),
                    "workspace-write".to_owned(),
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "expected `--output-schema`",
        ),
        (
            "stray-dry",
            [
                base(),
                vec![
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                    "--dry-run".to_owned(),
                    "--".to_owned(),
                ],
            ]
            .concat(),
            "dispatch logging options must be supplied together",
        ),
        (
            "missing-delimiter",
            [
                base(),
                vec![
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                ],
            ]
            .concat(),
            "dispatch arguments require the `--` delimiter",
        ),
        (
            "displaced-delimiter",
            [
                base(),
                vec![
                    "--".to_owned(),
                    "--output-schema".to_owned(),
                    schema.clone(),
                    "-o".to_owned(),
                    output.clone(),
                ],
            ]
            .concat(),
            "expected `--output-schema`",
        ),
    ];
    for (name, argv, diagnostic) in cases {
        let result = fixture
            .harness
            .run(&argv, b"")
            .expect("run gate parser rejection");
        assert!(!result.status.success(), "{name} unexpectedly succeeded");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(diagnostic),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("parser records")
                .is_empty(),
            "{name} invoked Claude"
        );
    }
    for target in ["claude", "Gate"] {
        let result = fixture
            .harness
            .run(["dispatch", target], b"")
            .expect("unsupported target");
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains(&format!("unsupported dispatch target `{target}`"))
        );
        assert!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("Claude records")
                .is_empty()
        );
        assert!(
            fixture
                .harness
                .invocations()
                .expect("generic records")
                .is_empty()
        );
    }
    let missing_plan = fixture.harness.path().join("missing-plan");
    let mut argv = fixture.argv(&fixture.environment(0), &[]);
    let delimiter = argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    argv.splice(
        delimiter..delimiter,
        ["--plan-file".to_owned(), missing_plan.display().to_string()],
    );
    let result = fixture.harness.run(&argv, b"").expect("missing gate plan");
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains(&format!(
        "failed to read plan file `{}`",
        missing_plan.display()
    )));

    let swaps = [(0, 2), (2, 4), (4, 6), (6, 8), (8, 10)];
    for (case, (left, right)) in swaps.into_iter().enumerate() {
        let log = fixture.harness.path().join(format!("swap-{case}.jsonl"));
        let mut logging = vec![
            "--log-file".to_owned(),
            log.display().to_string(),
            "--node".to_owned(),
            "m6-s2".to_owned(),
            "--role".to_owned(),
            "critic".to_owned(),
            "--ref".to_owned(),
            "ref".to_owned(),
            "--evidence".to_owned(),
            "evidence".to_owned(),
            "--required-artifact".to_owned(),
            fixture.output_path.display().to_string(),
        ];
        logging.swap(left, right);
        logging.swap(left + 1, right + 1);
        let mut argv = fixture.argv(&fixture.environment(0), &[]);
        let delimiter = argv
            .iter()
            .position(|value| value == "--")
            .expect("delimiter");
        argv.splice(delimiter..delimiter, logging);
        let result = fixture.harness.run(&argv, b"").expect("logging swap");
        assert!(
            !result.status.success(),
            "adjacent logging swap {case} succeeded"
        );
        assert!(
            String::from_utf8_lossy(&result.stderr)
                .contains("dispatch logging options must be supplied together"),
            "swap {case}"
        );
    }
}

#[test]
fn codex_accepts_explicit_anthropic_api_key_and_reaches_shim() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create Codex key harness");
    let cwd = fs::canonicalize(harness.path()).expect("cwd");
    let record_root = harness.path().join("codex-key-records");
    fs::create_dir(&record_root).expect("records");
    let stdout = harness.path().join("codex-key.stdout");
    let stderr = harness.path().join("codex-key.stderr");
    fs::write(&stdout, b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":1,\"cached_input_tokens\":2,\"output_tokens\":3,\"reasoning_output_tokens\":4}}\n").expect("stdout");
    fs::write(&stderr, []).expect("stderr");
    let mut environment = child_environment(&harness, &record_root, &stdout, &stderr, 0);
    environment.push((
        "ANTHROPIC_API_KEY".to_owned(),
        "allowed-for-codex".to_owned(),
    ));
    let result = harness
        .run(dispatch_argv(&cwd, &environment, None, None, "prompt"), b"")
        .expect("run Codex with key");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        harness
            .codex_invocations(&record_root)
            .expect("Codex key record")
            .len(),
        1
    );
}

#[test]
fn gate_logging_group_is_optional_exact_and_ordered() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("logging", CLAUDE_SUCCESS);
    let result = fixture
        .harness
        .run(fixture.argv(&fixture.environment(0), &[]), b"")
        .expect("no logging");
    assert!(result.status.success());
    for dry_run in [false, true] {
        let fixture = GateFixture::new(
            if dry_run {
                "logging-dry"
            } else {
                "logging-live"
            },
            CLAUDE_SUCCESS,
        );
        let log = fixture.harness.path().join("events.jsonl");
        let mut argv = fixture.argv(&fixture.environment(0), &[]);
        insert_gate_logging(&mut argv, &log, dry_run);
        let result = fixture.harness.run(&argv, b"").expect("complete logging");
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        if !dry_run {
            settle_lifecycle(&log);
            wait_for_path(&fixture.record_root.join("invocation/request.bin"));
        }
        assert_eq!(
            fixture
                .harness
                .claude_invocations(&fixture.record_root)
                .expect("logging records")
                .len(),
            usize::from(!dry_run)
        );
    }
}

fn assert_gate_classification(
    name: &str,
    stdout: &[u8],
    exit_code: i32,
    reason: UsageAbsenceReason,
) {
    let fixture = GateFixture::new(name, stdout);
    let log = fixture.harness.path().join("events.jsonl");
    let mut argv = fixture.argv(&fixture.environment(exit_code), &[]);
    insert_gate_logging(&mut argv, &log, false);
    let result = fixture
        .harness
        .run(&argv, b"")
        .expect("run classified gate");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"");
    assert_eq!(result.stderr, b"");
    let completion = gate_completion(&log);
    let continuation_stderr = wait_for_file_bytes(&dispatch_sidecar(&log, 1, "stderr"), |bytes| {
        String::from_utf8_lossy(bytes).contains(usage_reason_name(reason))
    });
    assert!(
        String::from_utf8_lossy(&continuation_stderr).contains(&format!(
            "invalid Claude result data: {}",
            usage_reason_name(reason)
        ))
    );
    assert_eq!(completion.usage, DispatchTokenUsage::Absent { reason });
    assert_eq!(
        completion.exit_status,
        DispatchExitStatus::Exited {
            code: pce_core::ExitCode::new(
                u64::try_from(exit_code).expect("nonnegative fixture exit")
            )
        }
    );
    if name == "malformed" {
        assert_eq!(
            fs::read(dispatch_sidecar(&log, 1, "stdout")).expect("read malformed stdout"),
            stdout
        );
    }
}

#[test]
fn gate_success_records_claude_usage() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("usage-success", CLAUDE_SUCCESS);
    fs::write(
        &fixture.schema_path,
        include_bytes!("../skills/pce/schemas/verdict.schema.json"),
    )
    .expect("write verdict schema");
    fs::write(
        &fixture.output_path,
        br#"{"verdict":"BLOCK","self_sufficiency":"NOT_APPLICABLE","root_cause":"step_plan","blocking_issues":[],"non_blocking_notes":[],"summary":"PLAN_INFEASIBLE: usage fixture"}"#,
    )
    .expect("write attributed verdict");
    let log = fixture.harness.path().join("events.jsonl");
    let mut argv = fixture.argv(&fixture.environment(0), &[]);
    insert_gate_logging(&mut argv, &log, false);
    let result = fixture.harness.run(&argv, b"").expect("run usage gate");
    assert!(result.status.success());
    let completion = gate_completion(&log);
    assert_eq!(
        completion.usage,
        DispatchTokenUsage::ClaudeMeasured {
            input_tokens: pce_core::InputTokens::new(11),
            output_tokens: pce_core::OutputTokens::new(13),
            cache_creation_input_tokens: pce_core::CacheCreationInputTokens::new(17),
            cache_read_input_tokens: pce_core::CacheReadInputTokens::new(19)
        }
    );
    assert_eq!(
        completion.root_cause,
        Some(pce_core::DispatchRootCause::StepPlan),
        "validated verdict attribution must be durable in the completion event"
    );
}

#[test]
fn gate_missing_usage_reports_route_specific_reason() {
    let _guard = dispatch_test_guard();
    assert_gate_classification(
        "missing-usage",
        br#"{"is_error":false}"#,
        0,
        UsageAbsenceReason::ClaudeMissingUsage,
    );
}

#[test]
fn gate_malformed_result_reports_route_specific_reason() {
    let _guard = dispatch_test_guard();
    assert_gate_classification(
        "malformed",
        b"not json",
        0,
        UsageAbsenceReason::ClaudeMalformedResult,
    );
}

#[test]
fn gate_error_result_records_error_envelope() {
    let _guard = dispatch_test_guard();
    assert_gate_classification(
        "error",
        br#"{"is_error":true}"#,
        7,
        UsageAbsenceReason::ClaudeErrorEnvelope,
    );
}

#[test]
fn gate_error_result_with_zero_exit_is_contradiction() {
    let _guard = dispatch_test_guard();
    assert_gate_classification(
        "error-zero",
        br#"{"is_error":true}"#,
        0,
        UsageAbsenceReason::ClaudeExitEnvelopeContradiction,
    );
}

#[test]
fn gate_success_result_with_nonzero_exit_is_contradiction() {
    let _guard = dispatch_test_guard();
    assert_gate_classification(
        "success-nonzero",
        CLAUDE_SUCCESS,
        9,
        UsageAbsenceReason::ClaudeExitEnvelopeContradiction,
    );
}

#[test]
fn gate_signal_exit_is_preserved() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("signal", br#"{"is_error":true}"#);
    let log = fixture.harness.path().join("events.jsonl");
    let mut environment = fixture.environment(0);
    environment.push(("PCE_CLAUDE_SIGNAL".to_owned(), "15".to_owned()));
    let mut argv = fixture.argv(&environment, &[]);
    insert_gate_logging(&mut argv, &log, false);
    let result = fixture.harness.run(&argv, b"").expect("run signaled gate");
    assert!(result.status.success());
    assert_eq!(
        gate_completion(&log).exit_status,
        DispatchExitStatus::Signaled {
            signal: pce_core::SignalNumber::new(15)
        }
    );
    assert_eq!(
        gate_completion(&log).usage,
        DispatchTokenUsage::Absent {
            reason: UsageAbsenceReason::ClaudeErrorEnvelope
        }
    );
}

#[test]
fn gate_artifact_outcomes_cover_all_categories() {
    let _guard = dispatch_test_guard();
    for (name, schema, artifact, expected) in [
        (
            "validated",
            VALID_ARTIFACT_SCHEMA as &[u8],
            Some(CONFORMING_ARTIFACT as &[u8]),
            ArtifactOutcome::Validated,
        ),
        (
            "missing",
            VALID_ARTIFACT_SCHEMA,
            None,
            ArtifactOutcome::Missing,
        ),
        (
            "truncated",
            VALID_ARTIFACT_SCHEMA,
            Some(b"{".as_slice()),
            ArtifactOutcome::Truncated,
        ),
        (
            "schema-invalid",
            b"{".as_slice(),
            Some(CONFORMING_ARTIFACT),
            ArtifactOutcome::SchemaInvalid,
        ),
        (
            "schema-violating",
            VALID_ARTIFACT_SCHEMA,
            Some(b"{}".as_slice()),
            ArtifactOutcome::SchemaViolating,
        ),
    ] {
        let stdout = if name == "missing" {
            br#"{"is_error":false,"result":"{\"verdict\":\"pass\",\"summary\":\"looks valid\"}","usage":{"input_tokens":11,"output_tokens":13,"cache_creation_input_tokens":17,"cache_read_input_tokens":19}}"#
        } else {
            CLAUDE_SUCCESS
        };
        let fixture = GateFixture::new(name, stdout);
        fs::write(&fixture.schema_path, schema).expect("replace schema fixture");
        let mut environment = fixture.environment(0);
        match (name, artifact) {
            ("validated" | "truncated" | "schema-violating", Some(bytes)) => {
                let bytes_path = fixture.harness.path().join(format!("{name}-artifact.bin"));
                fs::write(&bytes_path, bytes).expect("write child artifact bytes");
                fs::remove_file(&fixture.output_path).expect("remove pre-created artifact fixture");
                environment.extend([
                    (
                        "PCE_CLAUDE_OUTPUT_BYTES_FILE".to_owned(),
                        bytes_path.display().to_string(),
                    ),
                    (
                        "PCE_CLAUDE_OUTPUT_PATH".to_owned(),
                        fixture.output_path.display().to_string(),
                    ),
                ]);
            }
            (_, Some(bytes)) => {
                fs::write(&fixture.output_path, bytes).expect("replace artifact fixture")
            }
            (_, None) => fs::remove_file(&fixture.output_path).expect("remove artifact fixture"),
        }
        let log = fixture.harness.path().join("events.jsonl");
        let mut argv = fixture.argv(&environment, &[]);
        insert_gate_logging(&mut argv, &log, false);
        let result = fixture.harness.run(&argv, b"").expect("run artifact gate");
        assert!(result.status.success(), "{name} starter failed");
        assert_eq!(gate_completion(&log).artifact_outcome, expected, "{name}");
    }
}

#[test]
fn two_productless_attempts_atomically_open_hold_and_production_resets_streak() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("non-production-hold", CLAUDE_SUCCESS);
    let required_artifact = fs::canonicalize(&fixture.output_path).expect("canonical artifact");
    fs::remove_file(&fixture.output_path).expect("remove required artifact");
    let log = fixture.harness.path().join("events.jsonl");
    let environment = fixture.environment(0);
    let mut argv = fixture.argv(&environment, &[]);
    insert_gate_logging(&mut argv, &log, false);
    configure_gate_key(&mut argv, "m4-s1", "step-plan-writer", &required_artifact);
    for expected in [2, 4] {
        remove_gate_invocation(&fixture);
        let output = fixture
            .harness
            .run(&argv, b"")
            .expect("run productless dispatch");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        wait_for_lifecycle_records(&log, expected, Instant::now() + Duration::from_secs(15));
    }
    remove_gate_invocation(&fixture);
    let locked_log = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&log)
        .expect("open log for concurrency barrier");
    locked_log.lock().expect("lock concurrency barrier");
    let first = spawn_gate_dispatch(&fixture, &argv);
    let second = spawn_gate_dispatch(&fixture, &argv);
    thread::sleep(Duration::from_millis(500));
    locked_log.unlock().expect("release concurrency barrier");
    let outputs = [
        first.wait_with_output().expect("first concurrent dispatch"),
        second
            .wait_with_output()
            .expect("second concurrent dispatch"),
    ];
    let opened = format!(
        "Error: dispatch admission opened non-production hold for node m4-s1, role step-plan-writer, required artifact {} after 2 consecutive non-production completions; resolve with retry, re-plan, or abandon\n",
        required_artifact.display()
    );
    let already_open = format!(
        "Error: dispatch admission refused: non-production hold is open for node m4-s1, role step-plan-writer, required artifact {}; resolve with retry, re-plan, or abandon\n",
        required_artifact.display()
    );
    assert!(outputs.iter().all(|output| !output.status.success()));
    assert!(
        outputs
            .iter()
            .any(|output| output.stderr == opened.as_bytes())
    );
    assert!(
        outputs
            .iter()
            .any(|output| output.stderr == already_open.as_bytes())
    );
    let records = wait_for_lifecycle_records(&log, 5, Instant::now() + Duration::from_secs(15));
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(
                record.body_ref(),
                EventBodyRef::Known(KnownPayload::Dispatch(_))
            ))
            .count(),
        2
    );
    assert!(matches!(
        records[4].body_ref(),
        EventBodyRef::Known(KnownPayload::NonProductionHoldOpen(_))
    ));
    assert!(!fixture.record_root.join("invocation").exists());

    append_non_production_hold_close(
        &fixture,
        &log,
        "m4-s1",
        "step-plan-writer",
        &required_artifact,
        "retry",
    );
    let retried = fixture
        .harness
        .run(&argv, b"")
        .expect("run first dispatch in retry window");
    assert!(
        retried.status.success(),
        "{}",
        String::from_utf8_lossy(&retried.stderr)
    );
    wait_for_lifecycle_records(&log, 8, Instant::now() + Duration::from_secs(15));
    let retry_state = derive_logged_dispatch_state(&log, &required_artifact);
    assert_eq!(
        retry_state.non_production_streaks()[0].consecutive().get(),
        1
    );
    let retry_key = non_production_key("m4-s1", "step-plan-writer", &required_artifact);
    assert_eq!(
        pce_core::classify_dispatch_admission(&retry_state, &retry_key),
        pce_core::DispatchAdmission::Admit
    );
    remove_gate_invocation(&fixture);
    let second_retry = fixture
        .harness
        .run(&argv, b"")
        .expect("run second dispatch in retry window");
    assert!(
        second_retry.status.success(),
        "{}",
        String::from_utf8_lossy(&second_retry.stderr)
    );
    wait_for_lifecycle_records(&log, 10, Instant::now() + Duration::from_secs(15));

    let reset_fixture = GateFixture::new("production-resets-streak", CLAUDE_SUCCESS);
    let reset_artifact =
        fs::canonicalize(&reset_fixture.output_path).expect("canonical reset artifact");
    let reset_log = reset_fixture.harness.path().join("events.jsonl");
    let reset_environment = reset_fixture.environment(0);
    let mut reset_argv = reset_fixture.argv(&reset_environment, &[]);
    insert_gate_logging(&mut reset_argv, &reset_log, false);
    configure_gate_key(
        &mut reset_argv,
        "m4-s2",
        "step-plan-writer",
        &reset_artifact,
    );
    fs::remove_file(&reset_fixture.output_path).expect("remove reset artifact");
    run_successful_gate(&reset_fixture, &reset_argv, &reset_log, 2);
    fs::write(&reset_fixture.output_path, CONFORMING_ARTIFACT).expect("write valid production");
    run_successful_gate(&reset_fixture, &reset_argv, &reset_log, 4);
    fs::remove_file(&reset_fixture.output_path).expect("remove reset artifact again");
    run_successful_gate(&reset_fixture, &reset_argv, &reset_log, 6);
    let reset_state = derive_logged_dispatch_state(&reset_log, &reset_artifact);
    assert_eq!(
        reset_state.non_production_streaks()[0].consecutive().get(),
        1
    );
    assert!(reset_state.non_production_holds().is_empty());
    let reset_key = non_production_key("m4-s2", "step-plan-writer", &reset_artifact);
    assert_eq!(
        pce_core::classify_dispatch_admission(&reset_state, &reset_key),
        pce_core::DispatchAdmission::Admit
    );
    run_successful_gate(&reset_fixture, &reset_argv, &reset_log, 8);
}

fn configure_gate_key(argv: &mut [String], node: &str, role: &str, artifact: &Path) {
    for (flag, value) in [
        ("--node", node),
        ("--role", role),
        (
            "--required-artifact",
            artifact.to_str().expect("artifact path"),
        ),
        ("-o", artifact.to_str().expect("artifact path")),
    ] {
        let index = argv
            .iter()
            .position(|argument| argument == flag)
            .unwrap_or_else(|| panic!("missing {flag}"));
        argv[index + 1] = value.to_owned();
    }
}

fn remove_gate_invocation(fixture: &GateFixture) {
    let invocation = fixture.record_root.join("invocation");
    if invocation.exists() {
        fs::remove_dir_all(invocation).expect("remove prior gate invocation");
    }
}

fn spawn_gate_dispatch(fixture: &GateFixture, argv: &[String]) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(argv)
        .env_clear()
        .env("PATH", fixture.harness.shim_path())
        .env("PCE_SHIM_ROOT", fixture.harness.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn concurrent dispatch")
}

fn run_successful_gate(
    fixture: &GateFixture,
    argv: &[String],
    log: &Path,
    expected_records: usize,
) {
    remove_gate_invocation(fixture);
    let output = fixture.harness.run(argv, b"").expect("run gate dispatch");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    wait_for_lifecycle_records(
        log,
        expected_records,
        Instant::now() + Duration::from_secs(15),
    );
}

fn append_non_production_hold_close(
    fixture: &GateFixture,
    log: &Path,
    node: &str,
    role: &str,
    artifact: &Path,
    resolution: &str,
) {
    let payload = serde_json::to_vec(&json!({
        "key": {
            "node": node,
            "role": role,
            "required_artifact_path": artifact,
        },
        "resolution": resolution,
    }))
    .expect("serialize hold close");
    let output = fixture
        .harness
        .run(
            [
                "log",
                "--file",
                log.to_str().expect("log path"),
                "--kind",
                "non-production-hold-close",
                "--node",
                node,
            ],
            &payload,
        )
        .expect("append hold close");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn non_production_key(node: &str, role: &str, artifact: &Path) -> pce_core::NonProductionKey {
    pce_core::NonProductionKey {
        node: NodeId::parse(node).expect("node"),
        role: DispatchRole::new(role),
        required_artifact_path: AbsoluteRequiredArtifactPath::parse(artifact).expect("artifact"),
    }
}

fn derive_logged_dispatch_state(log: &Path, artifact: &Path) -> pce_core::DispatchOutcomeState {
    let records = fs::read_to_string(log)
        .expect("event log")
        .lines()
        .map(|line| parse_event_line(line).expect("event record"))
        .collect::<Vec<_>>();
    let observations = records
        .iter()
        .filter(|record| {
            matches!(
                record.body_ref(),
                EventBodyRef::Known(KnownPayload::Dispatch(_))
            )
        })
        .map(|record| {
            pce_core::DispatchRequiredArtifactObservation::new(
                record.sequence(),
                AbsoluteRequiredArtifactPath::parse(artifact).expect("artifact"),
            )
        })
        .collect::<Vec<_>>();
    pce_core::derive_dispatch_outcome_state(&records, &observations).expect("outcome state")
}

fn derived_fixture(records: &[&str], observed: &[u64]) -> pce_core::DispatchOutcomeState {
    let records = records
        .iter()
        .map(|line| parse_event_line(line).expect("fixture record"))
        .collect::<Vec<_>>();
    let observations = observed
        .iter()
        .map(|sequence| {
            pce_core::DispatchRequiredArtifactObservation::new(
                Sequence::parse(*sequence).expect("sequence"),
                AbsoluteRequiredArtifactPath::parse("/workspace/plan.md").expect("path"),
            )
        })
        .collect::<Vec<_>>();
    pce_core::derive_dispatch_outcome_state(&records, &observations).expect("outcome state")
}

const PRODUCTLESS_DISPATCH: &str = r#"{"sequence":1,"timestamp":"2026-08-09T12:00:00.000Z","kind":"dispatch","node":"m4-s1","payload":{"role":"step-plan-writer","ref":"abc","evidence":"fixture"}}"#;
const PRODUCTLESS_COMPLETION: &str = r#"{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated","required_artifact_presence":"absent"}}"#;

#[test]
fn recorded_generic_human_escalation_resumes_an_exhausted_exact_dispatch_once() {
    let mut lines = Vec::new();
    let mut observed = Vec::new();
    for round in 0..12_u64 {
        let issuance = round * 2 + 1;
        let completion = issuance + 1;
        lines.push(format!(r#"{{"sequence":{issuance},"timestamp":"2026-08-09T12:00:00.000Z","kind":"dispatch","node":"m1-s2","payload":{{"role":"step-plan-critic","ref":"abc","evidence":"fixture"}}}}"#));
        lines.push(format!(r#"{{"sequence":{completion},"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m1-s2","payload":{{"issuance_sequence":{issuance},"duration_ms":1,"usage":{{"availability":"absent","reason":"no-terminal-turn"}},"exit_status":{{"kind":"exited","code":0}},"artifact_outcome":"validated","required_artifact_presence":"present"}}}}"#));
        observed.push(issuance);
    }
    lines.push(r#"{"sequence":25,"timestamp":"2026-08-09T12:00:02.000Z","kind":"escalation-open","node":"m1-s2","payload":{"key":"live-cap-recovery","question":"May this exact dispatch resume?"}}"#.to_owned());
    lines.push(r#"{"sequence":26,"timestamp":"2026-08-09T12:00:03.000Z","kind":"escalation-close","node":"m1-s2","payload":{"key":"live-cap-recovery","resolution":"approved"}}"#.to_owned());
    let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
    let state = derived_fixture(&refs, &observed);
    let key = non_production_key("m1-s2", "step-plan-critic", Path::new("/workspace/plan.md"));
    assert_eq!(
        pce_core::classify_dispatch_admission(&state, &key),
        pce_core::DispatchAdmission::Admit
    );

    lines.push(r#"{"sequence":27,"timestamp":"2026-08-09T12:00:04.000Z","kind":"dispatch","node":"m1-s2","payload":{"role":"step-plan-critic","ref":"abc","evidence":"fixture"}}"#.to_owned());
    observed.push(27);
    let refs = lines.iter().map(String::as_str).collect::<Vec<_>>();
    let spent = derived_fixture(&refs, &observed);
    assert!(matches!(
        pce_core::classify_dispatch_admission(&spent, &key),
        pce_core::DispatchAdmission::OpenNonProductionHold { consecutive } if consecutive.get() == 0
    ));
}

#[test]
fn validated_upstream_root_cause_is_durable_and_does_not_charge_reporter() {
    let upstream = r#"{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"validated","required_artifact_presence":"present","root_cause":"step_plan"}}"#;
    let reporting_dispatch = PRODUCTLESS_DISPATCH.replace("step-plan-writer", "step-plan-critic");
    let state = derived_fixture(&[&reporting_dispatch, upstream], &[1]);
    assert!(state.validated_production_counts().is_empty());
    let key = non_production_key("m4-s1", "step-plan-critic", Path::new("/workspace/plan.md"));
    assert_eq!(
        pce_core::classify_dispatch_admission(&state, &key),
        pce_core::DispatchAdmission::Admit
    );

    let parsed = parse_event_line(upstream).expect("parse attributed completion");
    let serialized = serialize_event_line(&parsed).expect("serialize attributed completion");
    assert!(serialized.contains(r#""root_cause":"step_plan""#));
}

#[test]
fn legacy_validated_completion_keeps_reporting_role_charge() {
    let legacy = r#"{"sequence":2,"timestamp":"2026-08-09T12:00:01.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"validated","required_artifact_presence":"present"}}"#;
    let state = derived_fixture(&[PRODUCTLESS_DISPATCH, legacy], &[1]);
    assert_eq!(state.validated_production_counts()[0].count().get(), 1);
    let parsed = parse_event_line(legacy).expect("parse legacy completion");
    let serialized = serialize_event_line(&parsed).expect("serialize legacy completion");
    assert!(!serialized.contains("root_cause"));
}

#[test]
fn valid_production_resets_the_non_production_streak() {
    const SECOND_DISPATCH: &str = r#"{"sequence":3,"timestamp":"2026-08-09T12:00:02.000Z","kind":"dispatch","node":"m4-s1","payload":{"role":"step-plan-writer","ref":"abc","evidence":"fixture"}}"#;
    const VALIDATED_COMPLETION: &str = r#"{"sequence":4,"timestamp":"2026-08-09T12:00:03.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{"issuance_sequence":3,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"validated","required_artifact_presence":"present"}}"#;
    let state = derived_fixture(
        &[
            PRODUCTLESS_DISPATCH,
            PRODUCTLESS_COMPLETION,
            SECOND_DISPATCH,
            VALIDATED_COMPLETION,
        ],
        &[1, 3],
    );
    assert_eq!(state.rounds()[0].count().get(), 1);
    assert_eq!(state.non_production_streaks().len(), 1);
    assert_eq!(state.non_production_streaks()[0].consecutive().get(), 0);
}

#[test]
fn validated_production_spending_is_environment_independent_and_generous() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("validated-production-spending", CLAUDE_SUCCESS);
    let required_artifact = fs::canonicalize(&fixture.output_path).expect("canonical artifact");
    let log = fixture.harness.path().join("events.jsonl");
    let environment = fixture.environment(0);
    let mut argv = fixture.argv(&environment, &[]);
    insert_gate_logging(&mut argv, &log, false);
    configure_gate_key(&mut argv, "m4-s1", "step-plan-critic", &required_artifact);

    for round in 1..=4 {
        remove_gate_invocation(&fixture);
        let output = Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(&argv)
            .env_clear()
            .env("PATH", fixture.harness.shim_path())
            .env("PCE_SHIM_ROOT", fixture.harness.path())
            .env("PCE_DEFECT_ROUND_CAP", "1")
            .stdin(Stdio::null())
            .output()
            .expect("run environment-independent dispatch");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        wait_for_lifecycle_records(&log, round * 2, Instant::now() + Duration::from_secs(15));
    }
    let state = derive_logged_dispatch_state(&log, &required_artifact);
    assert_eq!(state.validated_production_counts()[0].count().get(), 4);
    let key = non_production_key("m4-s1", "step-plan-critic", &required_artifact);
    assert_eq!(
        pce_core::classify_dispatch_admission(&state, &key),
        pce_core::DispatchAdmission::Admit
    );
}

#[test]
fn spending_limit_exhaustion_parks_and_recorded_retry_resumes_once() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("validated-production-parking", CLAUDE_SUCCESS);
    let required_artifact = fs::canonicalize(&fixture.output_path).expect("canonical artifact");
    let log = fixture.harness.path().join("events.jsonl");
    let environment = fixture.environment(0);
    let mut argv = fixture.argv(&environment, &[]);
    insert_gate_logging(&mut argv, &log, false);
    configure_gate_key(&mut argv, "m4-s1", "step-plan-critic", &required_artifact);
    for round in 1..=12 {
        run_successful_gate(&fixture, &argv, &log, round * 2);
    }

    remove_gate_invocation(&fixture);
    let parked = fixture
        .harness
        .run(&argv, b"")
        .expect("park exhausted series");
    assert!(!parked.status.success());
    let records = wait_for_lifecycle_records(&log, 25, Instant::now() + Duration::from_secs(15));
    assert!(matches!(
        records[24].body_ref(),
        EventBodyRef::Known(KnownPayload::NonProductionHoldOpen(_))
    ));

    append_non_production_hold_close(
        &fixture,
        &log,
        "m4-s1",
        "step-plan-critic",
        &required_artifact,
        "retry",
    );
    run_successful_gate(&fixture, &argv, &log, 28);

    remove_gate_invocation(&fixture);
    let parked_again = fixture
        .harness
        .run(&argv, b"")
        .expect("re-park spent retry");
    assert!(!parked_again.status.success());
    wait_for_lifecycle_records(&log, 29, Instant::now() + Duration::from_secs(15));
    assert!(!fixture.record_root.join("invocation").exists());
}

#[test]
fn admission_resolution_diagnostics_are_byte_exact_through_production_cli() {
    let _guard = dispatch_test_guard();
    for resolution in ["re-plan", "abandon"] {
        let fixture = GateFixture::new(resolution, CLAUDE_SUCCESS);
        let required_artifact = fs::canonicalize(&fixture.output_path).expect("canonical artifact");
        fs::remove_file(&fixture.output_path).expect("remove required artifact");
        let log = fixture.harness.path().join("events.jsonl");
        let environment = fixture.environment(0);
        let mut argv = fixture.argv(&environment, &[]);
        insert_gate_logging(&mut argv, &log, false);
        configure_gate_key(&mut argv, "m4-s1", "step-plan-writer", &required_artifact);
        run_successful_gate(&fixture, &argv, &log, 2);
        run_successful_gate(&fixture, &argv, &log, 4);
        remove_gate_invocation(&fixture);
        let opened = fixture
            .harness
            .run(&argv, b"")
            .expect("open non-production hold");
        assert!(!opened.status.success());
        assert_eq!(
            opened.stderr,
            format!(
                "Error: dispatch admission opened non-production hold for node m4-s1, role step-plan-writer, required artifact {} after 2 consecutive non-production completions; resolve with retry, re-plan, or abandon\n",
                required_artifact.display()
            )
            .as_bytes()
        );
        wait_for_lifecycle_records(&log, 5, Instant::now() + Duration::from_secs(15));
        append_non_production_hold_close(
            &fixture,
            &log,
            "m4-s1",
            "step-plan-writer",
            &required_artifact,
            resolution,
        );
        let before = fs::read(&log).expect("log before closed-hold refusal");
        let refused = fixture
            .harness
            .run(&argv, b"")
            .expect("attempt dispatch after terminal hold resolution");
        assert!(!refused.status.success());
        assert_eq!(
            refused.stderr,
            format!(
                "Error: dispatch admission refused: non-production hold for node m4-s1, role step-plan-writer, required artifact {} was resolved with {resolution}\n",
                required_artifact.display()
            )
            .as_bytes()
        );
        assert_eq!(
            fs::read(&log).expect("log after closed-hold refusal"),
            before
        );
        assert!(!fixture.record_root.join("invocation").exists());
    }
}

#[test]
fn free_text_escalation_close_cannot_resolve_typed_non_production_hold() {
    let free_text = r#"{"sequence":3,"timestamp":"2026-08-09T12:00:02.000Z","kind":"escalation-close","node":"m4-s1","payload":{"key":"review","resolution":"retry"}}"#;
    let state = derived_fixture(
        &[PRODUCTLESS_DISPATCH, PRODUCTLESS_COMPLETION, free_text],
        &[1],
    );
    assert_eq!(state.non_production_streaks()[0].consecutive().get(), 1);
}

#[test]
fn unstructured_required_artifact_is_production_without_defect_round() {
    const SECOND_DISPATCH: &str = r#"{"sequence":3,"timestamp":"2026-08-09T12:00:02.000Z","kind":"dispatch","node":"m4-s1","payload":{"role":"step-plan-writer","ref":"abc","evidence":"fixture"}}"#;
    let present = PRODUCTLESS_COMPLETION
        .replace("\"sequence\":2", "\"sequence\":4")
        .replace("\"issuance_sequence\":1", "\"issuance_sequence\":3")
        .replace(
            "\"required_artifact_presence\":\"absent\"",
            "\"required_artifact_presence\":\"present\"",
        );
    let before = derived_fixture(
        &[
            PRODUCTLESS_DISPATCH,
            PRODUCTLESS_COMPLETION,
            SECOND_DISPATCH,
        ],
        &[1, 3],
    );
    assert_eq!(before.non_production_streaks()[0].consecutive().get(), 1);
    assert_eq!(before.issuance_ordinals()[0].ordinal().get(), 2);
    let state = derived_fixture(
        &[
            PRODUCTLESS_DISPATCH,
            PRODUCTLESS_COMPLETION,
            SECOND_DISPATCH,
            &present,
        ],
        &[1, 3],
    );
    assert!(state.rounds().is_empty());
    assert_eq!(state.non_production_streaks()[0].consecutive().get(), 0);
    assert_eq!(state.issuance_ordinals()[0].ordinal().get(), 2);
}

#[test]
fn reconciled_produced_resets_productless_streak() {
    const SECOND_DISPATCH: &str = r#"{"sequence":3,"timestamp":"2026-08-09T12:00:02.000Z","kind":"dispatch","node":"m4-s1","payload":{"role":"step-plan-writer","ref":"abc","evidence":"fixture"}}"#;
    const PRODUCED_RECONCILIATION: &str = r#"{"sequence":4,"timestamp":"2026-08-09T12:00:03.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{"issuance_sequence":3,"outcome":"reconciled-dead","artifact_production":"produced"}}"#;
    const THIRD_DISPATCH: &str = r#"{"sequence":5,"timestamp":"2026-08-09T12:00:04.000Z","kind":"dispatch","node":"m4-s1","payload":{"role":"step-plan-writer","ref":"abc","evidence":"fixture"}}"#;
    const THIRD_PRODUCTLESS: &str = r#"{"sequence":6,"timestamp":"2026-08-09T12:00:05.000Z","kind":"dispatch-completion","node":"m4-s1","payload":{"issuance_sequence":5,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated","required_artifact_presence":"absent"}}"#;
    let before_third = derived_fixture(
        &[
            PRODUCTLESS_DISPATCH,
            PRODUCTLESS_COMPLETION,
            SECOND_DISPATCH,
            PRODUCED_RECONCILIATION,
        ],
        &[1, 3],
    );
    let key = non_production_key("m4-s1", "step-plan-writer", Path::new("/workspace/plan.md"));
    assert_eq!(
        pce_core::classify_dispatch_admission(&before_third, &key),
        pce_core::DispatchAdmission::Admit
    );
    let state = derived_fixture(
        &[
            PRODUCTLESS_DISPATCH,
            PRODUCTLESS_COMPLETION,
            SECOND_DISPATCH,
            PRODUCED_RECONCILIATION,
            THIRD_DISPATCH,
            THIRD_PRODUCTLESS,
        ],
        &[1, 3, 5],
    );
    assert!(state.rounds().is_empty());
    assert_eq!(state.non_production_streaks()[0].consecutive().get(), 1);
    assert!(state.non_production_holds().is_empty());
    assert_eq!(
        pce_core::classify_dispatch_admission(&state, &key),
        pce_core::DispatchAdmission::Admit
    );
}

#[test]
fn unresolved_issuance_is_neutral_even_when_artifact_exists() {
    let state = derived_fixture(&[PRODUCTLESS_DISPATCH], &[1]);
    assert!(state.rounds().is_empty());
    assert!(state.non_production_streaks().is_empty());
}

#[test]
fn pre_m4_observed_completions_are_neutral_not_absent() {
    let legacy = PRODUCTLESS_COMPLETION.replace(",\"required_artifact_presence\":\"absent\"", "");
    let state = derived_fixture(&[PRODUCTLESS_DISPATCH, &legacy], &[1]);
    assert!(state.rounds().is_empty());
    assert!(state.non_production_streaks().is_empty());
}

fn run_measured_gate(
    name: &str,
    stdout: &[u8],
    sleep: Option<&str>,
) -> pce_core::ObservedDispatchCompletionWithArtifactPresencePayload {
    let fixture = GateFixture::new(name, stdout);
    let log = fixture.harness.path().join("events.jsonl");
    let mut environment = fixture.environment(0);
    if let Some(seconds) = sleep {
        environment.push(("PCE_CLAUDE_SLEEP_SECONDS".to_owned(), seconds.to_owned()));
    }
    let mut argv = fixture.argv(&environment, &[]);
    insert_gate_logging(&mut argv, &log, false);
    let result = fixture.harness.run(&argv, b"").expect("run measured gate");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    gate_completion(&log)
}

#[test]
fn gate_duration_reports_child_runtime() {
    let _guard = dispatch_test_guard();
    let slow = run_measured_gate("duration-slow", CLAUDE_SUCCESS, Some("0.5"));
    assert!(
        slow.duration_ms.get() >= 500,
        "recorded duration was {} ms for a child that slept 500 ms",
        slow.duration_ms.get()
    );
}

#[test]
fn gate_usage_compares_two_measurements() {
    let _guard = dispatch_test_guard();
    let first = run_measured_gate("usage-first", CLAUDE_SUCCESS, None);
    let second_bytes = br#"{"is_error":false,"duration_ms":7,"usage":{"input_tokens":22,"output_tokens":30,"cache_creation_input_tokens":51,"cache_read_input_tokens":74}}"#;
    let second = run_measured_gate("usage-second", second_bytes, None);
    let (
        DispatchTokenUsage::ClaudeMeasured {
            input_tokens: first_input,
            output_tokens: first_output,
            cache_creation_input_tokens: first_creation,
            cache_read_input_tokens: first_read,
        },
        DispatchTokenUsage::ClaudeMeasured {
            input_tokens: second_input,
            output_tokens: second_output,
            cache_creation_input_tokens: second_creation,
            cache_read_input_tokens: second_read,
        },
    ) = (first.usage, second.usage)
    else {
        panic!("both gate usages must be measured")
    };
    assert_eq!(second_input.get() - first_input.get(), 11);
    assert_eq!(second_output.get() - first_output.get(), 17);
    assert_eq!(second_creation.get() - first_creation.get(), 34);
    assert_eq!(second_read.get() - first_read.get(), 55);
}

#[test]
fn gate_valid_artifact_succeeds_without_logging() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("no-log", CLAUDE_SUCCESS);
    let log = fixture.harness.path().join("must-not-exist.jsonl");
    let result = fixture
        .harness
        .run(fixture.argv(&fixture.environment(0), &["prompt"]), b"")
        .expect("run unlogged gate");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, CLAUDE_SUCCESS);
    assert!(!log.exists());
    assert_no_jsonl_files(fixture.harness.path());
}

#[test]
fn gate_schema_violation_exits_nonzero_without_logging() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("no-log-schema-violation", CLAUDE_SUCCESS);
    fs::write(&fixture.output_path, b"{}").expect("write schema-violating gate artifact");
    let result = fixture
        .harness
        .run(fixture.argv(&fixture.environment(0), &["prompt"]), b"")
        .expect("run unlogged schema-violating gate");
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("violates schema"),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn step_executor_output_inside_measured_worktree_is_rejected_before_spawn() {
    let _guard = dispatch_test_guard();
    let harness = CliHarness::new().expect("create measured worktree harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize measured worktree");
    let initialized = Command::new("git")
        .arg("-C")
        .arg(&cwd)
        .args(["init", "--quiet"])
        .status()
        .expect("initialize measured worktree");
    assert!(initialized.success());
    let record_root = harness.path().join("records");
    fs::create_dir(&record_root).expect("create records");
    let stdout_path = harness.path().join("child.stdout");
    let stderr_path = harness.path().join("child.stderr");
    fs::write(&stdout_path, SUCCESSFUL_STRUCTURED_TRANSCRIPT).expect("write stdout");
    fs::write(&stderr_path, []).expect("write stderr");
    let environment = child_environment(&harness, &record_root, &stdout_path, &stderr_path, 0);
    let schema = cwd.join("schema.json");
    let output_path = cwd.join("executor-result.json");
    fs::write(&schema, VALID_ARTIFACT_SCHEMA).expect("write schema");
    let log_path = harness.path().join("events.jsonl");
    let mut argv = dispatch_argv(
        &cwd,
        &environment,
        Some((&schema, &output_path)),
        None,
        "execute",
    );
    let delimiter = argv
        .iter()
        .position(|value| value == "--")
        .expect("delimiter");
    argv.splice(
        delimiter..delimiter,
        logging_arguments(&log_path, &output_path),
    );

    let result = harness
        .run(&argv, b"")
        .expect("run rejected executor dispatch");

    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains(&format!(
            "step-executor output path must be outside its measured worktree: {}",
            output_path.display()
        )),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output_path.exists());
    assert!(
        harness
            .codex_invocations(&record_root)
            .expect("read invocations")
            .is_empty(),
        "rejected dispatch must not spawn Codex"
    );
}

#[test]
fn gate_rejects_missing_structured_artifact_without_logging() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("no-log-missing", CLAUDE_SUCCESS);
    fs::remove_file(&fixture.output_path).expect("remove no-log gate artifact");
    let result = fixture
        .harness
        .run(fixture.argv(&fixture.environment(0), &["prompt"]), b"")
        .expect("run unlogged gate with missing artifact");
    assert!(!result.status.success());
    assert_eq!(
        result.stderr,
        format!(
            "Error: artifact output `{}` is missing\n",
            fixture.output_path.display()
        )
        .as_bytes()
    );
    assert_no_jsonl_files(fixture.harness.path());
}

#[test]
fn gate_records_exact_issuance_correlation() {
    let _guard = dispatch_test_guard();
    let fixture = GateFixture::new("correlation", CLAUDE_SUCCESS);
    let log = fixture.harness.path().join("events.jsonl");
    let block = fixture.harness.path().join("release");
    let mut environment = fixture.environment(0);
    environment.push((
        "PCE_CLAUDE_BLOCK_FILE".to_owned(),
        block.display().to_string(),
    ));
    let mut argv = fixture.argv(&environment, &[]);
    insert_gate_logging(&mut argv, &log, false);
    let binary = env!("CARGO_BIN_EXE_pce");
    let mut child = Command::new(binary)
        .args(&argv)
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn blocked gate");
    wait_for_path(&fixture.record_root.join("invocation/pid"));
    let interleaved = fixture
        .harness
        .run(
            [
                "log",
                "--file",
                log.to_str().expect("log"),
                "--kind",
                "delta",
                "--node",
                "m6-s2",
            ],
            br#"{"message":"interleaved"}"#,
        )
        .expect("append interleaved record");
    assert!(interleaved.status.success(), "lock was held while gate ran");
    fs::write(&block, []).expect("release gate");
    let status = child.wait().expect("wait blocked gate");
    assert!(status.success());
    let records = wait_for_lifecycle_records(&log, 3, Instant::now() + Duration::from_secs(5));
    assert_eq!(records.len(), 3);
    let EventBodyRef::Known(KnownPayload::Dispatch(issuance)) = records[0].body_ref() else {
        panic!("issuance")
    };
    assert_eq!(issuance.role.as_str(), "critic");
    assert_eq!(issuance.r#ref.as_str(), "gate-ref");
    assert_eq!(issuance.evidence.as_str(), "gate-evidence");
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(completion)) = records[2].body_ref()
    else {
        panic!("completion")
    };
    let pce_core::DispatchCompletionPayload::ObservedChildWithArtifactPresence(completion) =
        completion
    else {
        panic!("reconciled completion")
    };
    assert_eq!(completion.issuance_sequence, records[0].sequence());
    assert_ne!(
        completion.issuance_sequence.get(),
        records[2].sequence().get() - 1
    );
}
