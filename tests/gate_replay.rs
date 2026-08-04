//! Offline integration coverage for clean-ref gate replay.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

use pce_core::{
    AbsoluteGateExecClientPath, AbsoluteGateExecutionEvidencePath, AbsoluteGateExecutionSocketPath,
    AbsoluteOutputPath, AbsoluteWorkingDirectory, ArgumentVector, ChildEnvironment,
    DispatchEnvelope, DispatchRole, DispatchTarget, ExpectedVerdictOutcome, GateExecutionEvidence,
    GateExecutionRecord, GateExecutionRecorderConfig, GateExecutionRef, GateExecutionResponse,
    GateObservedResult, NamedReplayRef, StdinBinding, compose_gate_arguments, dispatch_invocation,
    parse_gate_stimulus, parse_replay_output_path, parse_replay_schema_path,
};

const VERDICT_SCHEMA: &[u8] = br#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["verdict", "self_sufficiency", "root_cause", "blocking_issues", "non_blocking_notes", "summary"],
  "properties": {
    "verdict": { "type": "string", "enum": ["APPROVE", "REVISE", "BLOCK"] },
    "self_sufficiency": { "type": "string", "enum": ["PASS", "FAIL", "NOT_APPLICABLE"] },
    "root_cause": { "type": "string", "enum": ["execution", "step_plan", "milestone_plan", "vision"] },
    "blocking_issues": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "severity", "location", "problem", "required_change"],
        "properties": {
          "id": { "type": "string" },
          "severity": { "type": "string", "enum": ["critical", "major"] },
          "location": { "type": "string" },
          "problem": { "type": "string" },
          "required_change": { "type": "string" }
        }
      }
    },
    "non_blocking_notes": { "type": "array", "items": { "type": "string" } },
    "summary": { "type": "string" }
  }
}
"#;
const CONFORMING: &[u8] = b"{\"verdict\":\"APPROVE\",\"self_sufficiency\":\"NOT_APPLICABLE\",\"root_cause\":\"execution\",\"blocking_issues\":[],\"non_blocking_notes\":[],\"summary\":\"repaired\"}\n";

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    evidence: PathBuf,
    broken: String,
    repaired: String,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary repository root");
        let root = temp.path().join("repo");
        std::fs::create_dir(&root).expect("repository directory");
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.email", "replay@example.invalid"]);
        git(&root, &["config", "user.name", "Replay Fixture"]);
        std::fs::write(root.join("schema.json"), VERDICT_SCHEMA).expect("schema fixture");
        std::fs::write(root.join("probe.sh"), probe_script(b"{}\n")).expect("broken probe");
        git(&root, &["add", "schema.json", "probe.sh"]);
        git(&root, &["commit", "-q", "-m", "broken"]);
        let broken = git_stdout(&root, &["rev-parse", "HEAD"]);
        git(&root, &["branch", "broken", &broken]);

        std::fs::write(root.join("probe.sh"), probe_script(CONFORMING)).expect("repaired probe");
        git(&root, &["add", "probe.sh"]);
        git(&root, &["commit", "-q", "-m", "repaired"]);
        let repaired = git_stdout(&root, &["rev-parse", "HEAD"]);
        git(&root, &["branch", "repaired", &repaired]);

        let canonical = std::fs::canonicalize(&root).expect("canonical fixture root");
        let evidence = root.join("evidence.json");
        write_evidence(
            &evidence,
            &canonical,
            "/bin/sh",
            vec![
                "./probe.sh".to_owned(),
                format!("{}/verdict.json", canonical.display()),
            ],
            vec![],
            None,
        );

        std::fs::write(root.join("probe.sh"), b"#!/bin/sh\nexit 99\n").expect("dirty probe");
        std::fs::write(root.join("verdict.json"), CONFORMING).expect("dirty source artifact");
        Self {
            _temp: temp,
            root: canonical,
            evidence,
            broken,
            repaired,
        }
    }

    fn args(&self, broken: &str, repaired: &str) -> Vec<String> {
        vec![
            "gate".into(),
            "replay".into(),
            "--repo-root".into(),
            self.root.display().to_string(),
            "--evidence".into(),
            self.evidence.display().to_string(),
            "--execution-ref".into(),
            "execution-000001".into(),
            "--broken-ref".into(),
            broken.into(),
            "--repaired-ref".into(),
            repaired.into(),
            "--schema".into(),
            "schema.json".into(),
            "--output".into(),
            "verdict.json".into(),
            "--expected".into(),
            "conforming-verdict".into(),
        ]
    }

    fn run(&self, broken: &str, repaired: &str) -> (Output, Value) {
        let output = run_cli(&self.args(broken, repaired), None);
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout.last(), Some(&b'\n'));
        let report = serde_json::from_slice(&output.stdout).expect("typed replay report");
        (output, report)
    }
}

fn probe_script(artifact: &[u8]) -> Vec<u8> {
    let escaped = String::from_utf8_lossy(artifact)
        .replace('\\', "\\\\")
        .replace('\'', "'\\''");
    format!(
        "#!/bin/sh\nIFS= read -r input\n[ \"$input\" = \"$PWD\" ] || exit 70\n[ \"$REPLAY_ROOT\" = \"$PWD\" ] || exit 71\n[ \"$1\" = \"$PWD/verdict.json\" ] || exit 72\nprintf '%s' '{escaped}' > \"$1\"\n"
    ).into_bytes()
}

fn write_evidence(
    path: &Path,
    root: &Path,
    program: &str,
    arguments: Vec<String>,
    setup: Vec<Value>,
    environment_override: Option<Value>,
) {
    let input = format!("{}\n", root.display()).into_bytes();
    let environment =
        environment_override.unwrap_or_else(|| json!({"REPLAY_ROOT": root.display().to_string()}));
    let wire = json!({
        "schema_id": "pce.gate-execution-evidence",
        "schema_version": 1,
        "executions": [{
            "execution_ref": "execution-000001",
            "stimulus": {
                "working_directory": root,
                "setup": setup,
                "command": {"program": program, "arguments": arguments, "input": input, "environment": environment}
            },
            "observed_result": {"setup": [], "command": null}
        }]
    });
    std::fs::write(path, serde_json::to_vec(&wire).expect("evidence JSON"))
        .expect("evidence fixture");
}

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git fixture command");
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_stdout(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .expect("git fixture command");
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git UTF-8")
        .trim()
        .to_owned()
}

fn commit_paths(root: &Path, message: &str, paths: &[&str]) -> String {
    for path in paths {
        git(root, &["add", path]);
    }
    git(root, &["commit", "-q", "-m", message]);
    git_stdout(root, &["rev-parse", "HEAD"])
}

fn run_cli(args: &[String], environment: Option<(&str, &Path)>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some((name, value)) = environment {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("pce should spawn");
    let deadline = Instant::now() + Duration::from_secs(165);
    let status = loop {
        match child.try_wait().expect("pce wait") {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("gate replay integration process exceeded 165 seconds");
            }
        }
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .expect("pce stdout")
        .read_to_end(&mut stdout)
        .expect("read pce stdout");
    child
        .stderr
        .take()
        .expect("pce stderr")
        .read_to_end(&mut stderr)
        .expect("read pce stderr");
    Output {
        status,
        stdout,
        stderr,
    }
}

fn outcome_kind<'a>(report: &'a Value, side: &str) -> &'a str {
    report[side]["outcome"]["kind"]
        .as_str()
        .expect("outcome kind")
}

#[test]
fn classifications_are_closed_and_reproducible() {
    let fixture = Fixture::new();
    let (first_wire, first) = fixture.run("broken", "repaired");
    assert_eq!(first["classification"], "repair-sensitive", "{first:#}");
    assert_eq!(first["broken"]["outcome"]["matches_expected"], "no");
    assert_eq!(first["repaired"]["outcome"]["matches_expected"], "yes");
    assert_eq!(
        first["broken"]["outcome"]["observation"]["artifact"]["bytes"],
        json!(b"{}\n")
    );
    assert_eq!(
        first["repaired"]["outcome"]["observation"]["artifact"]["bytes"],
        json!(CONFORMING)
    );
    assert_eq!(outcome_kind(&first, "broken"), "reproducible");

    for (broken, repaired, classification) in [
        ("broken", "broken", "no-repair-signal"),
        ("repaired", "repaired", "no-repair-signal"),
        ("repaired", "broken", "opposite-direction"),
    ] {
        let (_, report) = fixture.run(broken, repaired);
        assert_eq!(report["classification"], classification);
    }
    let (second_wire, second) = fixture.run("broken", "repaired");
    assert_eq!(first, second);
    assert_eq!(first_wire.stdout, second_wire.stdout);
    let report_text = String::from_utf8(first_wire.stdout).expect("report UTF-8");
    for absent in [
        "schema.json",
        "verdict.json",
        "conforming-verdict",
        "PCE_REPLAY_PAUSE_AFTER_RESOLVE",
        "/tmp/pce-gate-replay-",
    ] {
        assert!(!report_text.contains(absent), "report leaked {absent}");
    }
    assert_eq!(
        std::fs::read(fixture.root.join("probe.sh")).expect("dirty source probe"),
        b"#!/bin/sh\nexit 99\n"
    );
    assert_eq!(
        std::fs::read(fixture.root.join("verdict.json")).expect("dirty source verdict"),
        CONFORMING
    );
    assert_eq!(
        git_stdout(&fixture.root, &["worktree", "list", "--porcelain"])
            .matches("worktree ")
            .count(),
        1
    );
}

#[test]
fn checkout_identity_cannot_forge_a_repair_and_paths_are_normalized() {
    let fixture = Fixture::new();
    let probe = br#"#!/bin/sh
printf '%s\n%s\n%s\n' "$PWD" "$(dirname "$PWD")" "$(basename "$PWD")"
case "$(basename "$PWD")" in
  broken-*) printf '{}\n' > verdict.json ;;
  *) printf '{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"%s"}\n' "$PWD" > verdict.json ;;
esac
"#;
    std::fs::write(fixture.root.join("probe.sh"), probe).expect("path-observing probe");
    let commit = commit_paths(&fixture.root, "path-observing probe", &["probe.sh"]);
    git(&fixture.root, &["branch", "path-observer", &commit]);
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["./probe.sh".into()],
        vec![],
        Some(json!({})),
    );

    let (wire, report) = fixture.run("path-observer", "path-observer");
    assert_eq!(report["classification"], "no-repair-signal", "{report:#}");
    let expected_stdout = format!(
        "{}\n{}\ncheckout\n",
        fixture.root.display(),
        fixture.root.display()
    )
    .into_bytes();
    let expected_artifact = format!(
        "{{\"verdict\":\"APPROVE\",\"self_sufficiency\":\"NOT_APPLICABLE\",\"root_cause\":\"execution\",\"blocking_issues\":[],\"non_blocking_notes\":[],\"summary\":\"{}\"}}\n",
        fixture.root.display()
    )
    .into_bytes();
    for side in ["broken", "repaired"] {
        let observation = &report[side]["outcome"]["observation"];
        assert_eq!(report[side]["outcome"]["kind"], "reproducible");
        assert_eq!(
            observation["process"]["command"]["stdout"],
            json!(expected_stdout)
        );
        assert_eq!(observation["artifact"]["kind"], "conforming");
        assert_eq!(observation["artifact"]["bytes"], json!(expected_artifact));
    }
    let report_text = String::from_utf8(wire.stdout).expect("report UTF-8");
    for absent in [
        "pce-gate-replay-",
        "broken-1",
        "broken-2",
        "repaired-1",
        "repaired-2",
    ] {
        assert!(!report_text.contains(absent), "report leaked {absent}");
    }
}

#[test]
fn writable_git_state_is_isolated_across_distinct_commits() {
    let fixture = Fixture::new();
    let probe = br#"#!/bin/sh
common=$(git rev-parse --git-common-dir) || exit 80
n=$(cat "$common/attack-counter" 2>/dev/null || printf 0)
n=$((n + 1))
printf '%s' "$n" > "$common/attack-counter"
case "$n" in
  1|2) printf '{}\n' > verdict.json ;;
  *) printf '{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"repaired"}\n' > verdict.json ;;
esac
"#;
    std::fs::write(fixture.root.join("ordinal.sh"), probe).expect("ordinal probe");
    let commit = commit_paths(&fixture.root, "ordinal probe", &["ordinal.sh"]);
    git(&fixture.root, &["branch", "ordinal", &commit]);
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["./ordinal.sh".into()],
        vec![],
        Some(json!({})),
    );

    std::fs::write(fixture.root.join("irrelevant.txt"), b"other commit")
        .expect("irrelevant tracked change");
    let other = commit_paths(&fixture.root, "irrelevant change", &["irrelevant.txt"]);
    git(&fixture.root, &["branch", "ordinal-other", &other]);

    let (_, report) = fixture.run("ordinal", "ordinal-other");
    assert_eq!(
        report["classification"], "no-repair-signal",
        "state escaped one isolated Git directory: {report:#}"
    );
    assert!(!fixture.root.join(".git/attack-counter").exists());
    for side in ["broken", "repaired"] {
        assert_eq!(
            report[side]["outcome"]["observation"]["artifact"]["kind"],
            "missing"
        );
        assert_eq!(
            report[side]["outcome"]["observation"]["process"]["command"]["status"]["code"],
            80
        );
    }
}

#[test]
fn git_metadata_alone_cannot_forge_repair_sensitivity() {
    let fixture = Fixture::new();
    let repaired_identity = [
        git_stdout(&fixture.root, &["rev-list", "--count", &fixture.repaired]),
        git_stdout(&fixture.root, &["rev-parse", &fixture.repaired]),
        git_stdout(
            &fixture.root,
            &["show", "-s", "--format=%ct", &fixture.repaired],
        ),
        git_stdout(
            &fixture.root,
            &["show", "-s", "--format=%s", &fixture.repaired],
        ),
    ]
    .join("|");
    let probe = format!(
        r#"#!/bin/sh
identity="$(git rev-list --count HEAD 2>/dev/null)|$(git rev-parse HEAD 2>/dev/null)|$(git show -s --format=%ct HEAD 2>/dev/null)|$(git show -s --format=%s HEAD 2>/dev/null)"
metadata_path=$(find .. -maxdepth 2 \( -name .git -o -name repository.git \) -print -quit 2>/dev/null)
if [ "$identity" = '{repaired_identity}' ] || [ -n "$metadata_path" ]; then
  printf '%s' '{}' > verdict.json
else
  printf '{{}}\n' > verdict.json
fi
"#,
        String::from_utf8_lossy(CONFORMING)
    );
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["-c".into(), probe],
        vec![],
        Some(json!({})),
    );

    let (_, report) = fixture.run("broken", "repaired");
    assert_eq!(report["classification"], "no-repair-signal", "{report:#}");
    for side in ["broken", "repaired"] {
        assert_eq!(
            report[side]["outcome"]["observation"]["artifact"]["kind"],
            "schema-violating"
        );
    }
}

#[test]
fn ancestor_command_lines_cannot_disclose_the_parent_oracle() {
    let fixture = Fixture::new();
    let probe = format!(
        r#"#!/bin/sh
surface=$(/bin/ps -o args= -p "$PPID" 2>/dev/null || true)
surface="$surface $(/bin/ps eww -p "$PPID" 2>/dev/null || true)"
printf '%s\n' "$surface" >> "$LOG"
broken=$(printf '%s\n' "$surface" | sed -n 's/.*--broken-ref \([^ ]*\).*/\1/p')
repo=$(printf '%s\n' "$surface" | sed -n 's/.*--repo-root \([^ ]*\).*/\1/p')
expected=$(printf '%s\n' "$surface" | sed -n 's/.*--expected \([^ ]*\).*/\1/p')
if [ "$expected" = conforming-verdict ] &&
   [ -n "$broken" ] &&
   [ "$(git -C "$repo" rev-parse "$broken" 2>/dev/null || true)" = "$(git rev-parse HEAD)" ]; then
  printf '{{}}\n' > verdict.json
else
  printf '%s' '{}' > verdict.json
fi
"#,
        String::from_utf8_lossy(CONFORMING)
    );
    std::fs::write(fixture.root.join("ancestor.sh"), probe).expect("ancestor probe");
    std::fs::write(fixture.root.join("irrelevant.txt"), b"first").expect("irrelevant fixture");
    let first = commit_paths(
        &fixture.root,
        "ancestor probe",
        &["ancestor.sh", "irrelevant.txt"],
    );
    git(&fixture.root, &["branch", "argv-broken", &first]);
    std::fs::write(fixture.root.join("irrelevant.txt"), b"second").expect("irrelevant mutation");
    let second = commit_paths(&fixture.root, "irrelevant only", &["irrelevant.txt"]);
    git(&fixture.root, &["branch", "argv-repaired", &second]);
    let log = fixture._temp.path().join("ancestor-surface.log");
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["./ancestor.sh".into()],
        vec![],
        Some(json!({"LOG": log})),
    );

    let (_, report) = fixture.run("argv-broken", "argv-repaired");
    assert_eq!(report["classification"], "no-repair-signal", "{report:#}");
    for side in ["broken", "repaired"] {
        assert_eq!(
            report[side]["outcome"]["observation"]["artifact"]["kind"],
            "conforming"
        );
    }
    let surface = std::fs::read_to_string(log).expect("ancestor surface log");
    for secret in [
        "--broken-ref",
        "argv-broken",
        "argv-repaired",
        "conforming-verdict",
        fixture.root.to_str().expect("fixture root UTF-8"),
    ] {
        assert!(
            !surface.contains(secret),
            "ancestor surface leaked {secret}"
        );
    }
}

#[test]
fn parent_replay_inputs_are_absent_from_every_gate_visible_carrier() {
    let fixture = Fixture::new();
    let expected = ExpectedVerdictOutcome::Conforming;
    let broken_ref = NamedReplayRef::parse("parent-only-broken-ref").expect("broken ref");
    let repaired_ref = NamedReplayRef::parse("parent-only-repaired-ref").expect("repaired ref");
    let schema_path =
        parse_replay_schema_path("parent-only/replay-schema.json").expect("replay schema path");
    let output_path =
        parse_replay_output_path("parent-only/replay-output.json").expect("replay output path");
    let seam = fixture._temp.path().join("parent-only-pause-seam");
    let replay_temp = "/private/tmp/pce-gate-replay-parent-only";
    let _parent_replay_request = (
        expected,
        &broken_ref,
        &repaired_ref,
        &schema_path,
        &output_path,
        &seam,
    );

    let gate_output = fixture.root.join("gate-verdict.json");
    let absolute_output = AbsoluteOutputPath::parse(gate_output.clone()).expect("gate output");
    let client = AbsoluteGateExecClientPath::parse("/usr/bin/pce").expect("gate exec client");
    let socket = AbsoluteGateExecutionSocketPath::parse(
        fixture._temp.path().join("gate-execution-boundary.sock"),
    )
    .expect("gate execution socket");
    let evidence_path = AbsoluteGateExecutionEvidencePath::from_verdict_path(&gate_output);
    let recorder = GateExecutionRecorderConfig::new(client.clone(), evidence_path, socket);
    let role = DispatchRole::new("falsification-critic");
    let arguments = compose_gate_arguments(
        Some(&role),
        &absolute_output,
        Some(&client),
        ArgumentVector::new(vec!["gate-subject.md".into()]),
    )
    .expect("falsification frame");
    let envelope = DispatchEnvelope::new(
        DispatchTarget::Gate,
        AbsoluteWorkingDirectory::parse(fixture.root.clone()).expect("gate cwd"),
        StdinBinding::Null,
    )
    .with_arguments(arguments)
    .with_environment(ChildEnvironment::new(BTreeMap::from([(
        "GATE_VISIBLE".to_owned(),
        "subject-only".to_owned(),
    )])))
    .with_output_path(absolute_output)
    .with_gate_execution_recorder(recorder)
    .expect("gate recorder");
    let invocation = dispatch_invocation(&envelope);
    let mandate = invocation
        .argv()
        .iter()
        .find(|argument| argument.starts_with("You are the falsification critic."))
        .expect("falsification mandate");

    let stimulus_bytes = serde_json::to_vec(&json!({
        "working_directory": fixture.root,
        "setup": [],
        "command": {
            "program": "/bin/true",
            "arguments": [],
            "input": [],
            "environment": {"SUBJECT_INPUT": "visible"}
        }
    }))
    .expect("request JSON");
    let stimulus = parse_gate_stimulus(&stimulus_bytes).expect("gate request");
    let observed_result = GateObservedResult {
        setup: vec![],
        command: None,
    };
    let execution_ref = GateExecutionRef::parse("execution-000001").expect("execution ref");
    let response_bytes = serde_json::to_vec(&GateExecutionResponse {
        execution_ref: execution_ref.clone(),
        observed_result: observed_result.clone(),
    })
    .expect("response JSON");
    let evidence_bytes =
        serde_json::to_vec(&GateExecutionEvidence::new(vec![GateExecutionRecord {
            execution_ref,
            stimulus,
            observed_result,
        }]))
        .expect("evidence JSON");
    let argv_bytes = serde_json::to_vec(invocation.argv()).expect("argv JSON");
    let environment_key_bytes =
        serde_json::to_vec(&invocation.environment().keys().collect::<Vec<_>>())
            .expect("environment keys JSON");
    let carriers = [
        ("argv", argv_bytes.as_slice()),
        ("environment keys", environment_key_bytes.as_slice()),
        ("mandate", mandate.as_bytes()),
        ("request", stimulus_bytes.as_slice()),
        ("response", response_bytes.as_slice()),
        ("evidence", evidence_bytes.as_slice()),
    ];
    let seam_spelling = seam.display().to_string();
    for (carrier_name, carrier) in carriers {
        let carrier = String::from_utf8_lossy(carrier);
        for absent in [
            "conforming-verdict",
            broken_ref.as_str(),
            repaired_ref.as_str(),
            "parent-only/replay-schema.json",
            "parent-only/replay-output.json",
            "repair-sensitive",
            "PCE_REPLAY_PAUSE_AFTER_RESOLVE",
            seam_spelling.as_str(),
            replay_temp,
        ] {
            assert!(
                !carrier.contains(absent),
                "{carrier_name} leaked parent replay input {absent}"
            );
        }
    }

    let recorded_evidence = std::fs::read(&fixture.evidence).expect("recorded evidence");
    std::fs::remove_file(&fixture.evidence).expect("withhold incomplete sidecar");
    let before_gate_exit = run_cli(&fixture.args("broken", "repaired"), None);
    assert!(!before_gate_exit.status.success());
    assert!(
        before_gate_exit.stdout.is_empty(),
        "report preceded gate exit"
    );
    let child_exit = fixture._temp.path().join("simulated-gate-child-exited");
    std::fs::write(&child_exit, b"").expect("simulate gate child exit");
    assert!(child_exit.exists());
    assert!(!fixture.evidence.exists());
    std::fs::write(&fixture.evidence, recorded_evidence).expect("complete evidence sidecar");
    let after_sidecar = run_cli(&fixture.args("broken", "repaired"), None);
    assert!(after_sidecar.status.success());
    assert!(!after_sidecar.stdout.is_empty());
}

#[test]
fn checkout_oracle_and_stimulus_failures_remain_distinct() {
    let fixture = Fixture::new();
    let (_, missing) = fixture.run("missing-ref", "repaired");
    assert_eq!(missing["classification"], "checkout-failed");
    assert_eq!(
        missing["broken"],
        json!({
            "requested_ref": "missing-ref",
            "checkout_failed": {"stage": "resolve-ref", "diagnostic": "failed to resolve replay ref `missing-ref` to a commit"}
        })
    );
    assert_eq!(missing["repaired"]["resolved_commit"], fixture.repaired);

    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/definitely/not/a/pce-replay-program",
        vec![],
        vec![],
        Some(json!({})),
    );
    let (_, spawn_failed) = fixture.run("broken", "repaired");
    assert_eq!(spawn_failed["classification"], "no-repair-signal");
    for side in ["broken", "repaired"] {
        let command = &spawn_failed[side]["outcome"]["observation"]["process"]["command"];
        assert_eq!(command["status"]["kind"], "spawn-failed");
        assert!(
            !command["status"]["detail"]
                .as_str()
                .expect("spawn detail")
                .is_empty()
        );
        assert_eq!(
            spawn_failed[side]["outcome"]["observation"]["artifact"]["kind"],
            "missing"
        );
    }

    git(&fixture.root, &["branch", "no-schema", &fixture.broken]);
    let schema_commit = fixture.root.join("schema.json");
    std::fs::remove_file(&schema_commit).expect("remove live schema");
    git(&fixture.root, &["add", "schema.json"]);
    git(&fixture.root, &["commit", "-q", "-m", "remove schema"]);
    let no_schema = git_stdout(&fixture.root, &["rev-parse", "HEAD"]);
    git(&fixture.root, &["branch", "-f", "no-schema", &no_schema]);
    let (_, mixed) = fixture.run("missing-ref", "no-schema");
    assert_eq!(mixed["classification"], "checkout-failed");
    assert_eq!(mixed["repaired"]["oracle_failed"]["stage"], "read-schema");
}

#[test]
fn evidence_and_domain_failures_stop_without_a_report() {
    let fixture = Fixture::new();
    let mut args = fixture.args("broken", "repaired");
    let reference = args
        .iter()
        .position(|item| item == "execution-000001")
        .expect("reference argument");
    args[reference] = "execution-000002".into();
    let missing = run_cli(&args, None);
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
    assert_eq!(
        missing.stderr,
        b"Error: gate execution evidence does not contain reference `execution-000002`\n"
    );

    let outside = fixture.root.parent().expect("outside root").join("outside");
    std::fs::create_dir(&outside).expect("outside cwd");
    write_evidence(
        &fixture.evidence,
        &outside,
        "/bin/true",
        vec![],
        vec![],
        Some(json!({})),
    );
    let rejected = run_cli(&fixture.args("broken", "repaired"), None);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert_eq!(
        rejected.stderr,
        b"Error: recorded gate execution working directory is outside replay repository root\n"
    );
}

#[test]
fn setup_failure_stops_before_command_and_is_a_raw_observation() {
    let fixture = Fixture::new();
    let setup = vec![
        json!({"program":"/bin/sh","arguments":["-c","exit 23"],"input":[],"environment":{}}),
        json!({"program":"/bin/sh","arguments":["-c","touch second-setup"],"input":[],"environment":{}}),
    ];
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["-c".into(), "touch command-marker".into()],
        setup,
        Some(json!({})),
    );
    let (_, report) = fixture.run("broken", "repaired");
    for side in ["broken", "repaired"] {
        let process = &report[side]["outcome"]["observation"]["process"];
        assert_eq!(
            process,
            &json!({"setup":[{"status":{"kind":"exited","code":23},"stdout":[],"stderr":[]}],"command":null})
        );
        assert_eq!(
            report[side]["outcome"]["observation"]["artifact"]["kind"],
            "missing"
        );
    }
    assert_eq!(report["classification"], "no-repair-signal");
}

#[test]
fn refs_are_resolved_once_before_the_pause_seam() {
    let fixture = Fixture::new();
    git(&fixture.root, &["branch", "-f", "movable", &fixture.broken]);
    let seam = tempfile::tempdir().expect("pause seam");
    let args = fixture.args("movable", "movable");
    let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
    command
        .args(&args)
        .env("PCE_REPLAY_PAUSE_AFTER_RESOLVE", seam.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("paused replay");
    let marker_deadline = Instant::now() + Duration::from_secs(10);
    while !seam.path().join("resolved").exists() {
        assert!(
            Instant::now() < marker_deadline,
            "resolved marker was not created"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        std::fs::metadata(seam.path().join("resolved"))
            .expect("resolved metadata")
            .len(),
        0
    );
    git(
        &fixture.root,
        &["branch", "-f", "movable", &fixture.repaired],
    );
    std::fs::write(seam.path().join("continue"), b"").expect("continue marker");
    let deadline = Instant::now() + Duration::from_secs(165);
    let status = loop {
        match child.try_wait().expect("paused pce wait") {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("gate replay integration process exceeded 165 seconds");
            }
        }
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_end(&mut stdout)
        .expect("stdout bytes");
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_end(&mut stderr)
        .expect("stderr bytes");
    assert!(status.success(), "{}", String::from_utf8_lossy(&stderr));
    let report: Value = serde_json::from_slice(&stdout).expect("report");
    assert_eq!(report["broken"]["resolved_commit"], fixture.broken);
    assert_eq!(report["repaired"]["resolved_commit"], fixture.broken);
    assert_eq!(report["classification"], "no-repair-signal");
    assert!(!seam.path().join("resolved").exists());
    std::fs::remove_file(seam.path().join("continue")).expect("remove continue");
}

#[test]
fn strict_cli_rejects_wrong_order_and_expected_spelling() {
    let fixture = Fixture::new();
    let mut args = fixture.args("broken", "repaired");
    args.swap(2, 4);
    let wrong_order = run_cli(&args, None);
    assert_eq!(wrong_order.status.code(), Some(1));
    assert!(wrong_order.stdout.is_empty());
    assert!(String::from_utf8_lossy(&wrong_order.stderr).contains("usage: pce"));

    let mut args = fixture.args("broken", "repaired");
    let expected = args
        .iter()
        .position(|item| item == "conforming-verdict")
        .expect("expected argument");
    args[expected] = "maybe".into();
    let unsupported = run_cli(&args, None);
    assert_eq!(unsupported.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&unsupported.stderr)
            .contains("unsupported expected verdict outcome `maybe`")
    );
}

#[test]
fn artifact_partition_and_target_schema_are_revision_local() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.root.join("probe.sh"),
        b"#!/bin/sh\nIFS= read -r input\n[ \"$input\" = \"$PWD\" ] || exit 70\n[ \"$REPLAY_ROOT\" = \"$PWD\" ] || exit 71\nexit 0\n",
    )
    .expect("missing probe");
    let missing = commit_paths(&fixture.root, "missing", &["probe.sh"]);
    git(&fixture.root, &["branch", "artifact-missing", &missing]);

    std::fs::write(fixture.root.join("probe.sh"), probe_script(b"{")).expect("invalid probe");
    let invalid = commit_paths(&fixture.root, "invalid", &["probe.sh"]);
    git(&fixture.root, &["branch", "artifact-invalid", &invalid]);

    std::fs::write(fixture.root.join("probe.sh"), probe_script(b"{}\n")).expect("violating probe");
    let violating = commit_paths(&fixture.root, "violating", &["probe.sh"]);
    git(&fixture.root, &["branch", "artifact-violating", &violating]);

    for (reference, expected) in [
        ("artifact-missing", "missing"),
        ("artifact-invalid", "invalid-json"),
        ("artifact-violating", "schema-violating"),
        ("repaired", "conforming"),
    ] {
        let (_, report) = fixture.run(reference, reference);
        assert_eq!(
            report["broken"]["outcome"]["observation"]["artifact"]["kind"],
            expected
        );
    }

    std::fs::write(
        fixture.root.join("schema.json"),
        br#"{"type":"object","required":["impossible"]}"#,
    )
    .expect("rejecting schema");
    std::fs::write(fixture.root.join("probe.sh"), probe_script(CONFORMING))
        .expect("conforming probe");
    let rejecting = commit_paths(
        &fixture.root,
        "reject target artifact",
        &["schema.json", "probe.sh"],
    );
    git(&fixture.root, &["branch", "rejecting-schema", &rejecting]);
    let (_, target_owned) = fixture.run("repaired", "rejecting-schema");
    assert_eq!(target_owned["classification"], "opposite-direction");
    assert_eq!(
        target_owned["broken"]["outcome"]["observation"]["artifact"]["kind"],
        "conforming"
    );
    assert_eq!(
        target_owned["repaired"]["outcome"]["observation"]["artifact"]["kind"],
        "schema-violating"
    );
}

#[test]
fn retained_schema_cannot_be_weakened_by_the_stimulus() {
    let fixture = Fixture::new();
    std::fs::write(fixture.root.join("artifact.txt"), CONFORMING).expect("artifact fixture");
    std::fs::write(
        fixture.root.join("rewrite.sh"),
        b"#!/bin/sh\nprintf '{' > schema.json\ncp artifact.txt verdict.json\n",
    )
    .expect("schema rewrite probe");
    let commit = commit_paths(
        &fixture.root,
        "schema rewrite probe",
        &["artifact.txt", "rewrite.sh"],
    );
    git(&fixture.root, &["branch", "rewrite-schema", &commit]);
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["./rewrite.sh".into()],
        vec![],
        Some(json!({})),
    );
    let (_, report) = fixture.run("rewrite-schema", "rewrite-schema");
    assert_eq!(report["classification"], "no-repair-signal");
    assert_eq!(
        report["broken"]["outcome"]["observation"]["artifact"]["kind"],
        "conforming"
    );
}

#[test]
fn preexisting_output_and_unusable_schema_are_oracle_failures() {
    let fixture = Fixture::new();
    git(&fixture.root, &["add", "verdict.json"]);
    git(&fixture.root, &["commit", "-q", "-m", "tracked output"]);
    let tracked_output = git_stdout(&fixture.root, &["rev-parse", "HEAD"]);
    git(
        &fixture.root,
        &["branch", "tracked-output", &tracked_output],
    );
    let (_, output_report) = fixture.run("tracked-output", "tracked-output");
    assert_eq!(output_report["classification"], "oracle-failed");
    for side in ["broken", "repaired"] {
        assert_eq!(
            output_report[side]["oracle_failed"]["stage"],
            "prepare-output"
        );
    }

    let malformed = Fixture::new();
    std::fs::write(malformed.root.join("schema.json"), b"{").expect("malformed schema");
    let malformed_commit = commit_paths(&malformed.root, "malformed schema", &["schema.json"]);
    git(
        &malformed.root,
        &["branch", "malformed-schema", &malformed_commit],
    );
    let (_, schema_report) = malformed.run("malformed-schema", "malformed-schema");
    assert_eq!(schema_report["classification"], "oracle-failed");
    for side in ["broken", "repaired"] {
        assert_eq!(schema_report[side]["oracle_failed"]["stage"], "read-schema");
    }
}

#[test]
fn different_conforming_bytes_are_non_reproducible_before_the_oracle() {
    let fixture = Fixture::new();
    let counter = fixture._temp.path().join("counter");
    let script = b"#!/bin/sh\nn=$(cat \"$COUNTER\" 2>/dev/null || printf 0)\nn=$((n + 1))\nprintf '%s' \"$n\" > \"$COUNTER\"\nif [ $((n % 2)) -eq 1 ]; then summary=first; else summary=second; fi\nprintf '{\"verdict\":\"APPROVE\",\"self_sufficiency\":\"NOT_APPLICABLE\",\"root_cause\":\"execution\",\"blocking_issues\":[],\"non_blocking_notes\":[],\"summary\":\"%s\"}\\n' \"$summary\" > verdict.json\n";
    std::fs::write(fixture.root.join("nondeterministic.sh"), script)
        .expect("nondeterministic probe");
    let commit = commit_paths(&fixture.root, "nondeterministic", &["nondeterministic.sh"]);
    git(&fixture.root, &["branch", "nondeterministic", &commit]);
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["./nondeterministic.sh".into()],
        vec![],
        Some(json!({"COUNTER": counter})),
    );
    let (_, report) = fixture.run("nondeterministic", "nondeterministic");
    assert_eq!(report["classification"], "non-reproducible");
    for side in ["broken", "repaired"] {
        assert_eq!(outcome_kind(&report, side), "non-reproducible");
        assert_ne!(
            report[side]["outcome"]["first"]["artifact"]["bytes"],
            report[side]["outcome"]["second"]["artifact"]["bytes"]
        );
        assert_eq!(
            report[side]["outcome"]["first"]["artifact"]["kind"],
            "conforming"
        );
        assert_eq!(
            report[side]["outcome"]["second"]["artifact"]["kind"],
            "conforming"
        );
    }
}

#[test]
fn setup_mutations_are_isolated_in_four_distinct_clean_worktrees() {
    let fixture = Fixture::new();
    let roots = fixture._temp.path().join("roots.log");
    std::fs::write(fixture.root.join("tracked.txt"), b"clean").expect("tracked fixture");
    std::fs::write(
        fixture.root.join("isolation.sh"),
        format!(
            "#!/bin/sh\n[ \"$(cat tracked.txt)\" = changed ] || exit 80\n[ -f untracked ] || exit 81\nprintf '%s' '{}' > verdict.json\nprintf '%s\\n' \"$PWD\" >> \"$LOG\"\n",
            String::from_utf8_lossy(CONFORMING)
        ),
    )
    .expect("isolation probe");
    let commit = commit_paths(&fixture.root, "isolation", &["tracked.txt", "isolation.sh"]);
    git(&fixture.root, &["branch", "isolated", &commit]);
    let setup = vec![json!({
        "program": "/bin/sh",
        "arguments": ["-c", "printf changed > tracked.txt; touch untracked"],
        "input": [],
        "environment": {}
    })];
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["./isolation.sh".into()],
        setup,
        Some(json!({"LOG": roots})),
    );
    let (_, report) = fixture.run("isolated", "isolated");
    assert_eq!(report["classification"], "no-repair-signal");
    let root_lines = std::fs::read_to_string(&roots).expect("recorded checkout roots");
    let checkout_roots = root_lines
        .lines()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(checkout_roots.len(), 4);
    for checkout in checkout_roots {
        assert!(
            !Path::new(checkout).exists(),
            "checkout remained at {checkout}"
        );
    }
}

#[test]
fn pause_seam_timeout_is_exact_and_precedes_worktree_creation() {
    let fixture = Fixture::new();
    let seam = tempfile::tempdir().expect("pause seam");
    let started = Instant::now();
    let output = run_cli(
        &fixture.args("broken", "repaired"),
        Some(("PCE_REPLAY_PAUSE_AFTER_RESOLVE", seam.path())),
    );
    assert!(started.elapsed() >= Duration::from_secs(10));
    assert!(started.elapsed() < Duration::from_secs(30));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"Error: replay pause-after-resolve seam timed out after 10 seconds\n"
    );
    assert!(!seam.path().join("resolved").exists());
    assert_eq!(
        git_stdout(&fixture.root, &["worktree", "list", "--porcelain"])
            .matches("worktree ")
            .count(),
        1
    );
}

#[test]
fn slow_drip_commands_are_bounded_and_cleaned() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.root.join("slow.sh"),
        b"#!/bin/sh\ni=0\nwhile [ \"$i\" -lt 100 ]; do printf x; sleep 0.1; i=$((i + 1)); done\n",
    )
    .expect("slow drip probe");
    let commit = commit_paths(&fixture.root, "slow drip", &["slow.sh"]);
    git(&fixture.root, &["branch", "slow-drip", &commit]);
    write_evidence(
        &fixture.evidence,
        &fixture.root,
        "/bin/sh",
        vec!["./slow.sh".into()],
        vec![],
        Some(json!({})),
    );
    let started = Instant::now();
    let (_, report) = fixture.run("slow-drip", "slow-drip");
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_secs(20), "elapsed {elapsed:?}");
    assert!(elapsed < Duration::from_secs(60), "elapsed {elapsed:?}");
    for side in ["broken", "repaired"] {
        let outcome = &report[side]["outcome"];
        match outcome["kind"].as_str().expect("slow outcome kind") {
            "reproducible" => assert_eq!(
                outcome["observation"]["process"]["command"]["status"]["kind"],
                "signaled"
            ),
            "non-reproducible" => {
                assert_eq!(
                    outcome["first"]["process"]["command"]["status"]["kind"],
                    "signaled"
                );
                assert_eq!(
                    outcome["second"]["process"]["command"]["status"]["kind"],
                    "signaled"
                );
            }
            other => panic!("unexpected slow outcome {other}"),
        }
    }
    assert_eq!(
        git_stdout(&fixture.root, &["worktree", "list", "--porcelain"])
            .matches("worktree ")
            .count(),
        1
    );
}
