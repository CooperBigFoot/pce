//! meter_cli : JSONL(stdin) × argv → (status, JSONL(stdout), diagnostics(stderr)).

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde::Deserialize;

#[allow(dead_code)]
mod support;

use support::{WorkspaceFixtureDirectory, skip_without_nested_seatbelt};

const DISPATCH: &str = r#"{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert codex shim measurement"}}"#;
const EXPECTED_REPORT: &str = r#"{"issuance":{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","node":"m8-s1","role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert codex shim measurement"},"completion":null}
"#;
const DISPATCH_TWO: &str = r#"{"sequence":2,"timestamp":"2026-08-02T12:42:43.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"second issuance"}}"#;
const COMPLETION_ONE: &str = r#"{"sequence":3,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":128,"usage":{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const COMPLETION_TWO: &str = r#"{"sequence":4,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":2,"duration_ms":64,"usage":{"availability":"measured","input_tokens":211,"cached_input_tokens":43,"output_tokens":31,"reasoning_output_tokens":11},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const CLAUDE_DISPATCH_ONE: &str = r#"{"sequence":5,"timestamp":"2026-08-02T12:42:46.273Z","kind":"dispatch","node":"m8-s3","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"first claude usage fixture"}}"#;
const CLAUDE_COMPLETION_ONE: &str = r#"{"sequence":6,"timestamp":"2026-08-02T12:42:47.273Z","kind":"dispatch-completion","node":"m8-s3","payload":{"issuance_sequence":5,"duration_ms":9,"usage":{"availability":"claude-measured","input_tokens":41,"output_tokens":47,"cache_creation_input_tokens":53,"cache_read_input_tokens":59},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;
const CLAUDE_DISPATCH_TWO: &str = r#"{"sequence":7,"timestamp":"2026-08-02T12:42:48.273Z","kind":"dispatch","node":"m8-s3","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"second claude usage fixture"}}"#;
const CLAUDE_COMPLETION_TWO: &str = r#"{"sequence":8,"timestamp":"2026-08-02T12:42:49.273Z","kind":"dispatch-completion","node":"m8-s3","payload":{"issuance_sequence":7,"duration_ms":10,"usage":{"availability":"claude-measured","input_tokens":61,"output_tokens":67,"cache_creation_input_tokens":71,"cache_read_input_tokens":73},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;
const DISTINCT_DISPATCH: &str = r#"{"sequence":11,"timestamp":"2026-08-02T13:42:42.273Z","kind":"dispatch","node":"m8-s2","payload":{"role":"milestone-executor","ref":"0123456789abcdef0123456789abcdef01234567","evidence":"independent issuance measurement"}}"#;
const DISTINCT_COMPLETION: &str = r#"{"sequence":12,"timestamp":"2026-08-02T13:42:43.273Z","kind":"dispatch-completion","node":"m8-s2","payload":{"issuance_sequence":11,"duration_ms":7,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"signaled","signal":15},"artifact_outcome":"missing"}}"#;
const MEASURED_CODEX_A: &str = r#"{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert codex shim measurement"}}
{"sequence":2,"timestamp":"2026-08-02T12:42:42.411Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":128,"usage":{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const MEASURED_CLAUDE_A: &str = r#"{"sequence":3,"timestamp":"2026-08-02T12:42:43.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert claude shim measurement"}}
{"sequence":4,"timestamp":"2026-08-02T12:42:43.399Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":3,"duration_ms":126,"usage":{"availability":"claude-measured","input_tokens":11,"output_tokens":13,"cache_creation_input_tokens":17,"cache_read_input_tokens":19},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;
const MEASURED_CODEX_B: &str = r#"{"sequence":5,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i second inert codex shim measurement"}}
{"sequence":6,"timestamp":"2026-08-02T12:42:44.278Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":5,"duration_ms":5,"usage":{"availability":"measured","input_tokens":127,"cached_input_tokens":29,"output_tokens":19,"reasoning_output_tokens":7},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const MEASURED_CLAUDE_B: &str = r#"{"sequence":7,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i second inert claude shim measurement"}}
{"sequence":8,"timestamp":"2026-08-02T12:42:45.279Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":7,"duration_ms":6,"usage":{"availability":"claude-measured","input_tokens":23,"output_tokens":29,"cache_creation_input_tokens":31,"cache_read_input_tokens":37},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;

#[derive(Deserialize)]
struct MeasuredEvidenceReport {
    issuance: MeasuredEvidenceIssuance,
    completion: Option<MeasuredEvidenceCompletion>,
}

#[derive(Deserialize)]
struct MeasuredEvidenceIssuance {
    sequence: u64,
}

#[derive(Deserialize)]
struct MeasuredEvidenceCompletion {
    usage: MeasuredEvidenceUsage,
}

#[derive(Deserialize)]
#[serde(tag = "availability", rename_all = "kebab-case")]
enum MeasuredEvidenceUsage {
    Measured {
        input_tokens: u64,
        cached_input_tokens: u64,
        output_tokens: u64,
        reasoning_output_tokens: u64,
    },
    ClaudeMeasured {
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_input_tokens: u64,
        cache_read_input_tokens: u64,
    },
    Absent {
        reason: String,
    },
}

impl MeasuredEvidenceUsage {
    fn observation(&self, issuance_sequence: u64) -> (u64, &'static str, Option<&str>) {
        match self {
            Self::Measured { .. } => (issuance_sequence, "measured", None),
            Self::ClaudeMeasured { .. } => (issuance_sequence, "claude-measured", None),
            Self::Absent { reason } => (issuance_sequence, "absent", Some(reason)),
        }
    }
}

fn run_meter(arguments: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pce meter process should spawn");
    child
        .stdin
        .take()
        .expect("pce meter stdin should be piped")
        .write_all(stdin)
        .expect("meter fixture should write");
    child.wait_with_output().expect("pce meter should exit")
}

fn run_command(mut command: Command, stdin: &[u8]) -> Output {
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("measured process should spawn");
    child
        .stdin
        .take()
        .expect("measured process stdin should be piped")
        .write_all(stdin)
        .expect("measured process fixture should write");
    child
        .wait_with_output()
        .expect("measured process should exit")
}

fn strict_seatbelt_profile(pce_executable: &Path) -> String {
    let executable_literal = pce_executable
        .to_str()
        .expect("canonical pce executable path should be UTF-8")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        "(version 1)\n\
(deny default)\n\
(import \"system.sb\")\n\
(allow process*)\n\
(allow file-read*\n\
  (literal \"{executable_literal}\")\n\
  (subpath \"/usr\")\n\
  (subpath \"/System\")\n\
  (subpath \"/bin\")\n\
  (subpath \"/private/var/db/dyld\")\n\
  (subpath \"/Library/Apple\"))\n"
    )
}

fn run_cat(path: &Path) -> Output {
    let mut command = Command::new("/bin/cat");
    command.arg(path);
    run_command(command, b"")
}

fn run_strict_cat(profile: &str, path: &Path) -> Output {
    let mut command = Command::new("/usr/bin/sandbox-exec");
    command.args(["-p", profile, "/bin/cat"]).arg(path);
    run_command(command, b"")
}

fn run_pce(executable: &Path, profile: Option<&str>, stdin: &[u8]) -> Output {
    let command = match profile {
        Some(profile) => {
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command
                .args(["-p", profile])
                .arg(executable)
                .args(["log", "meter"]);
            command
        }
        None => {
            let mut command = Command::new(executable);
            command.args(["log", "meter"]);
            command
        }
    };
    run_command(command, stdin)
}

fn complete_measurement(output: &Output) -> String {
    format!(
        "status={:?}; stdout={:?}; stderr={:?}",
        output.status.code(),
        output.stdout,
        output.stderr
    )
}

fn jsonl(lines: &[&str]) -> Vec<u8> {
    lines.join("\n").into_bytes()
}

fn report_values(output: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("meter report line should be JSON"))
        .collect()
}

fn assert_issuance_field(
    field_label: &str,
    source_records: &[serde_json::Value],
    reports: &[serde_json::Value],
    envelope_field: Option<&str>,
    report_field: &str,
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_records
        .iter()
        .map(|record| match envelope_field {
            Some(field) => &record[field],
            None => &record["payload"][report_field],
        })
        .collect();
    let observed: Vec<&serde_json::Value> = reports
        .iter()
        .map(|report| &report["issuance"][report_field])
        .collect();
    let expected_present = expected.iter().filter(|value| !value.is_null()).count();
    let observed_present = reports
        .iter()
        .filter(|report| {
            report["issuance"]
                .as_object()
                .is_some_and(|issuance| issuance.contains_key(report_field))
        })
        .count();
    assert_eq!(
        observed_present,
        expected_present,
        "meter_report_complete/{field_label}-omitted: expected measured presence count {expected_present} with values {expected:?}; observed presence count {observed_present} with complete values {observed:?}; {}",
        complete_measurement(output)
    );
    assert_eq!(
        observed,
        expected,
        "meter_report_complete/{field_label}-altered: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn assert_completion_field(
    field_label: &str,
    alteration_label: &str,
    source_completions: &[&serde_json::Value],
    report_completions: &[&serde_json::Value],
    envelope_field: Option<&str>,
    report_field: &str,
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_completions
        .iter()
        .map(|record| match envelope_field {
            Some(field) => &record[field],
            None => &record["payload"][report_field],
        })
        .collect();
    let observed: Vec<&serde_json::Value> = report_completions
        .iter()
        .map(|completion| &completion[report_field])
        .collect();
    let expected_present = expected.iter().filter(|value| !value.is_null()).count();
    let observed_present = report_completions
        .iter()
        .filter(|completion| {
            completion
                .as_object()
                .is_some_and(|completion| completion.contains_key(report_field))
        })
        .count();
    assert_eq!(
        observed_present,
        expected_present,
        "meter_report_complete/{field_label}-omitted: expected measured presence count {expected_present} with values {expected:?}; observed presence count {observed_present} with complete values {observed:?}; {}",
        complete_measurement(output)
    );
    assert_eq!(
        observed,
        expected,
        "meter_report_complete/{alteration_label}: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn assert_completion_value(
    label: &str,
    source_completions: &[&serde_json::Value],
    report_completions: &[&serde_json::Value],
    report_field: &str,
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_completions
        .iter()
        .map(|record| &record["payload"][report_field])
        .collect();
    let observed: Vec<&serde_json::Value> = report_completions
        .iter()
        .map(|completion| &completion[report_field])
        .collect();
    assert_eq!(
        observed,
        expected,
        "meter_report_complete/{label}: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn assert_usage_field(
    label: &str,
    availability: &str,
    field: &str,
    source_usage: &[&serde_json::Value],
    report_usage: &[&serde_json::Value],
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_usage
        .iter()
        .filter(|usage| usage["availability"] == availability)
        .map(|usage| &usage[field])
        .collect();
    let observed: Vec<&serde_json::Value> = report_usage
        .iter()
        .filter(|usage| usage["availability"] == availability)
        .map(|usage| &usage[field])
        .collect();
    assert_eq!(
        observed,
        expected,
        "meter_usage_complete/{label}: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn usage_case(usage: serde_json::Value) -> Vec<u8> {
    let issuance = serde_json::json!({
        "sequence": 1,
        "timestamp": "2026-08-02T12:42:42.273Z",
        "kind": "dispatch",
        "node": "m8-s1",
        "payload": {
            "role": "step-executor",
            "ref": "cf94db2ba021ea176b9b00099fa34855976dd563",
            "evidence": "strict usage fixture"
        }
    });
    let completion = serde_json::json!({
        "sequence": 2,
        "timestamp": "2026-08-02T12:42:43.273Z",
        "kind": "dispatch-completion",
        "node": "m8-s1",
        "payload": {
            "issuance_sequence": 1,
            "duration_ms": 1,
            "usage": usage,
            "exit_status": {"kind": "exited", "code": 0},
            "artifact_outcome": "not-validated"
        }
    });
    format!("{issuance}\n{completion}").into_bytes()
}

fn assert_unknown_usage_rejected(label: &str, unknown_key: &str, mut usage: serde_json::Value) {
    usage
        .as_object_mut()
        .expect("usage fixture should be an object")
        .insert(unknown_key.to_owned(), serde_json::json!("unexpected"));
    let output = run_meter(&["log", "meter"], &usage_case(usage));
    assert!(
        !output.status.success(),
        "meter_usage_denies_unknown/{label}: expected unknown key {unknown_key:?} to be rejected; {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        b"",
        "meter_usage_denies_unknown/{label}: expected zero stdout for unknown key {unknown_key:?}; {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_cli_accepts_exact_stdin_only_surface() {
    let output = run_meter(&["log", "meter"], b"");
    assert!(
        output.status.success(),
        "meter_cli_accepts_exact_stdin_only_surface/status: {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        b"",
        "meter_cli_accepts_exact_stdin_only_surface/stdout: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_cli_rejects_all_arguments() {
    let cases: &[(&str, &[&str])] = &[
        ("--file", &["log", "meter", "--file", "poison"]),
        ("--kind", &["log", "meter", "--kind", "dispatch"]),
        ("--node", &["log", "meter", "--node", "m8-s1"]),
        ("path", &["log", "meter", "events.jsonl"]),
        ("dash", &["log", "meter", "-"]),
    ];
    for (label, arguments) in cases {
        let output = run_meter(arguments, b"");
        assert!(
            !output.status.success(),
            "meter_cli_rejects_all_arguments/{label}/status: {}",
            complete_measurement(&output)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("usage: pce"),
            "meter_cli_rejects_all_arguments/{label}/stderr: {}",
            complete_measurement(&output)
        );
        assert_eq!(
            output.stdout,
            b"",
            "meter_cli_rejects_all_arguments/{label}/stdout: {}",
            complete_measurement(&output)
        );
    }
}

#[test]
fn meter_cli_consumes_stdin() {
    let output = run_meter(&["log", "meter"], DISPATCH.as_bytes());
    assert!(
        output.status.success(),
        "meter_cli_consumes_stdin/status: {}",
        complete_measurement(&output)
    );
    let report_count = output.stdout.split(|byte| *byte == b'\n').count() - 1;
    assert_eq!(
        report_count,
        1,
        "meter_cli_consumes_stdin/report-count: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_output_is_exact_jsonl() {
    let output = run_meter(&["log", "meter"], DISPATCH.as_bytes());
    assert!(
        output.status.success(),
        "meter_output_is_exact_jsonl/status: {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        EXPECTED_REPORT.as_bytes(),
        "meter_output_is_exact_jsonl/bytes: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_retains_uncompleted_issuance() {
    let output = run_meter(&["log", "meter"], DISPATCH.as_bytes());
    assert!(
        output.status.success(),
        "meter_retains_uncompleted_issuance/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    assert_eq!(
        reports.len(),
        1,
        "meter_retains_uncompleted_issuance: {}",
        complete_measurement(&output)
    );
    assert_eq!(
        reports[0]["completion"],
        serde_json::Value::Null,
        "meter_retains_uncompleted_issuance: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_preserves_issuance_order() {
    let input = jsonl(&[DISPATCH, DISPATCH_TWO]);
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_preserves_issuance_order/status: {}",
        complete_measurement(&output)
    );
    let observed: Vec<u64> = report_values(&output)
        .iter()
        .map(|report| report["issuance"]["sequence"].as_u64().expect("sequence"))
        .collect();
    assert_eq!(
        observed,
        vec![1, 2],
        "meter_preserves_issuance_order: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_correlates_by_issuance_sequence() {
    let input = jsonl(&[DISPATCH, DISPATCH_TWO, COMPLETION_ONE]);
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_correlates_by_issuance_sequence/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    let observed: Vec<(u64, Option<u64>)> = reports
        .iter()
        .map(|report| {
            (
                report["issuance"]["sequence"].as_u64().expect("sequence"),
                report["completion"]["sequence"].as_u64(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        vec![(1, Some(3)), (2, None)],
        "meter_correlates_by_issuance_sequence: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_report_complete() {
    let input = jsonl(&[
        DISPATCH,
        COMPLETION_ONE,
        DISTINCT_DISPATCH,
        DISTINCT_COMPLETION,
    ]);
    let source_records: Vec<serde_json::Value> = input
        .split(|byte| *byte == b'\n')
        .map(|line| serde_json::from_slice(line).expect("source event should be JSON"))
        .collect();
    let source_issuances: Vec<serde_json::Value> = source_records
        .iter()
        .filter(|record| record["kind"] == "dispatch")
        .cloned()
        .collect();
    let source_completions: Vec<&serde_json::Value> = source_records
        .iter()
        .filter(|record| record["kind"] == "dispatch-completion")
        .collect();
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_report_complete/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    assert_eq!(
        reports.len(),
        source_issuances.len(),
        "meter_report_complete/report-count: source measurements {source_issuances:?}; observed reports {reports:?}; {}",
        complete_measurement(&output)
    );
    let report_completions: Vec<&serde_json::Value> = reports
        .iter()
        .filter_map(|report| {
            report["completion"]
                .as_object()
                .map(|_| &report["completion"])
        })
        .collect();
    assert_eq!(
        report_completions.len(),
        source_completions.len(),
        "meter_report_complete/completion-count: source measurements {source_completions:?}; observed completions {report_completions:?}; {}",
        complete_measurement(&output)
    );

    assert_issuance_field(
        "issuance-sequence",
        &source_issuances,
        &reports,
        Some("sequence"),
        "sequence",
        &output,
    );
    assert_issuance_field(
        "issuance-timestamp",
        &source_issuances,
        &reports,
        Some("timestamp"),
        "timestamp",
        &output,
    );
    assert_issuance_field(
        "issuance-node",
        &source_issuances,
        &reports,
        Some("node"),
        "node",
        &output,
    );
    assert_issuance_field(
        "issuance-role",
        &source_issuances,
        &reports,
        None,
        "role",
        &output,
    );
    assert_issuance_field(
        "issuance-ref",
        &source_issuances,
        &reports,
        None,
        "ref",
        &output,
    );
    assert_issuance_field(
        "issuance-evidence",
        &source_issuances,
        &reports,
        None,
        "evidence",
        &output,
    );

    assert_completion_field(
        "completion-sequence",
        "completion-sequence-altered",
        &source_completions,
        &report_completions,
        Some("sequence"),
        "sequence",
        &output,
    );
    assert_completion_field(
        "completion-timestamp",
        "completion-timestamp-altered",
        &source_completions,
        &report_completions,
        Some("timestamp"),
        "timestamp",
        &output,
    );
    assert_completion_field(
        "duration-ms",
        "duration-ms-zeroed",
        &source_completions,
        &report_completions,
        None,
        "duration_ms",
        &output,
    );
    assert_completion_value(
        "exit-status",
        &source_completions,
        &report_completions,
        "exit_status",
        &output,
    );
    assert_completion_value(
        "artifact-outcome",
        &source_completions,
        &report_completions,
        "artifact_outcome",
        &output,
    );
}

#[test]
fn meter_usage_complete() {
    let input = jsonl(&[
        DISPATCH,
        DISPATCH_TWO,
        COMPLETION_ONE,
        COMPLETION_TWO,
        CLAUDE_DISPATCH_ONE,
        CLAUDE_COMPLETION_ONE,
        CLAUDE_DISPATCH_TWO,
        CLAUDE_COMPLETION_TWO,
        DISTINCT_DISPATCH,
        DISTINCT_COMPLETION,
    ]);
    let source_records: Vec<serde_json::Value> = input
        .split(|byte| *byte == b'\n')
        .map(|line| serde_json::from_slice(line).expect("source event should be JSON"))
        .collect();
    let source_usage: Vec<&serde_json::Value> = source_records
        .iter()
        .filter(|record| record["kind"] == "dispatch-completion")
        .map(|record| &record["payload"]["usage"])
        .collect();
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_usage_complete/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    let report_usage: Vec<&serde_json::Value> = reports
        .iter()
        .filter_map(|report| {
            report["completion"]
                .as_object()
                .map(|_| &report["completion"]["usage"])
        })
        .collect();

    let expected_availability: Vec<&serde_json::Value> = source_usage
        .iter()
        .map(|usage| &usage["availability"])
        .collect();
    let observed_availability: Vec<&serde_json::Value> = report_usage
        .iter()
        .map(|usage| &usage["availability"])
        .collect();
    assert_eq!(
        observed_availability,
        expected_availability,
        "meter_usage_complete/absent-not-zero-filled: expected measured availability values {expected_availability:?}; observed complete availability values {observed_availability:?}; {}",
        complete_measurement(&output)
    );

    assert_usage_field(
        "codex-input",
        "measured",
        "input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "codex-cached-input",
        "measured",
        "cached_input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "codex-output",
        "measured",
        "output_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "codex-reasoning-output",
        "measured",
        "reasoning_output_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-input",
        "claude-measured",
        "input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-output",
        "claude-measured",
        "output_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-cache-creation",
        "claude-measured",
        "cache_creation_input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-cache-read",
        "claude-measured",
        "cache_read_input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );

    let expected_absent_reasons: Vec<&serde_json::Value> = source_usage
        .iter()
        .filter(|usage| usage["availability"] == "absent")
        .map(|usage| &usage["reason"])
        .collect();
    let observed_absent_reasons: Vec<&serde_json::Value> = report_usage
        .iter()
        .filter(|usage| usage["availability"] == "absent")
        .map(|usage| &usage["reason"])
        .collect();
    let expected_reason_presence = expected_absent_reasons
        .iter()
        .filter(|reason| !reason.is_null())
        .count();
    let observed_reason_presence = report_usage
        .iter()
        .filter(|usage| {
            usage["availability"] == "absent"
                && usage
                    .as_object()
                    .is_some_and(|usage| usage.contains_key("reason"))
        })
        .count();
    assert_eq!(
        observed_reason_presence,
        expected_reason_presence,
        "meter_usage_complete/absent-reason-preserved: expected measured presence count {expected_reason_presence} with values {expected_absent_reasons:?}; observed presence count {observed_reason_presence} with complete values {observed_absent_reasons:?}; {}",
        complete_measurement(&output)
    );
    assert_eq!(
        observed_absent_reasons,
        expected_absent_reasons,
        "meter_usage_complete/absent-reason-preserved: expected measured values {expected_absent_reasons:?}; observed complete values {observed_absent_reasons:?}; {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_usage_denies_unknown() {
    assert_unknown_usage_rejected(
        "measured",
        "measured_extra",
        serde_json::json!({
            "availability": "measured",
            "input_tokens": 101,
            "cached_input_tokens": 23,
            "output_tokens": 17,
            "reasoning_output_tokens": 5
        }),
    );
    assert_unknown_usage_rejected(
        "claude-measured",
        "claude_measured_extra",
        serde_json::json!({
            "availability": "claude-measured",
            "input_tokens": 11,
            "output_tokens": 13,
            "cache_creation_input_tokens": 17,
            "cache_read_input_tokens": 19
        }),
    );
    assert_unknown_usage_rejected(
        "absent",
        "absent_extra",
        serde_json::json!({
            "availability": "absent",
            "reason": "no-terminal-turn"
        }),
    );
}

#[test]
fn measured_evidence_is_recoverable_from_event_log_alone() {
    let input = jsonl(&[
        MEASURED_CODEX_A,
        MEASURED_CLAUDE_A,
        MEASURED_CODEX_B,
        MEASURED_CLAUDE_B,
    ]);
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "measured_evidence/status: {}",
        complete_measurement(&output)
    );
    let reports: Vec<MeasuredEvidenceReport> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            serde_json::from_str(line).expect("measured evidence report should deserialize")
        })
        .collect();
    let controlled: Vec<(u64, &MeasuredEvidenceUsage)> = reports
        .iter()
        .filter(|report| matches!(report.issuance.sequence, 1 | 3 | 5 | 7))
        .filter_map(|report| {
            report
                .completion
                .as_ref()
                .map(|completion| (report.issuance.sequence, &completion.usage))
        })
        .collect();
    let codex: Vec<(u64, &MeasuredEvidenceUsage)> = controlled
        .iter()
        .filter(|(sequence, _)| matches!(sequence, 1 | 5))
        .copied()
        .collect();
    let claude: Vec<(u64, &MeasuredEvidenceUsage)> = controlled
        .iter()
        .filter(|(sequence, _)| matches!(sequence, 3 | 7))
        .copied()
        .collect();

    let codex_availability: Vec<(u64, &str, Option<&str>)> = codex
        .iter()
        .map(|(sequence, usage)| usage.observation(*sequence))
        .collect();
    let measured_count = controlled
        .iter()
        .filter(|(_, usage)| matches!(usage, MeasuredEvidenceUsage::Measured { .. }))
        .count();
    assert_eq!(
        (codex_availability.as_slice(), measured_count),
        ([(1, "measured", None), (5, "measured", None)].as_slice(), 2),
        "measured_evidence/codex-availability: expected controlled availability [(1, measured, None), (5, measured, None)] and exactly two measured completions; observed complete availability {codex_availability:?} and measured count {measured_count}; {}",
        complete_measurement(&output)
    );

    let claude_availability: Vec<(u64, &str, Option<&str>)> = claude
        .iter()
        .map(|(sequence, usage)| usage.observation(*sequence))
        .collect();
    let claude_measured_count = controlled
        .iter()
        .filter(|(_, usage)| matches!(usage, MeasuredEvidenceUsage::ClaudeMeasured { .. }))
        .count();
    assert_eq!(
        (claude_availability.as_slice(), claude_measured_count),
        (
            [(3, "claude-measured", None), (7, "claude-measured", None),].as_slice(),
            2
        ),
        "measured_evidence/claude-availability: expected controlled availability [(3, claude-measured, None), (7, claude-measured, None)] and exactly two claude-measured completions; observed complete availability {claude_availability:?} and claude-measured count {claude_measured_count}; {}",
        complete_measurement(&output)
    );

    let codex_vectors: Vec<(u64, u64, u64, u64)> = codex
        .iter()
        .filter_map(|(_, usage)| match usage {
            MeasuredEvidenceUsage::Measured {
                input_tokens,
                cached_input_tokens,
                output_tokens,
                reasoning_output_tokens,
            } => Some((
                *input_tokens,
                *cached_input_tokens,
                *output_tokens,
                *reasoning_output_tokens,
            )),
            MeasuredEvidenceUsage::ClaudeMeasured { .. } | MeasuredEvidenceUsage::Absent { .. } => {
                None
            }
        })
        .collect();
    assert_ne!(
        codex_vectors[0],
        codex_vectors[1],
        "measured_evidence/codex-responsive: expected two different complete measured token vectors; observed Codex A {:?} and Codex B {:?}; {}",
        codex_vectors[0],
        codex_vectors[1],
        complete_measurement(&output)
    );

    let claude_vectors: Vec<(u64, u64, u64, u64)> = claude
        .iter()
        .filter_map(|(_, usage)| match usage {
            MeasuredEvidenceUsage::ClaudeMeasured {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens,
                cache_read_input_tokens,
            } => Some((
                *input_tokens,
                *output_tokens,
                *cache_creation_input_tokens,
                *cache_read_input_tokens,
            )),
            MeasuredEvidenceUsage::Measured { .. } | MeasuredEvidenceUsage::Absent { .. } => None,
        })
        .collect();
    assert_ne!(
        claude_vectors[0],
        claude_vectors[1],
        "measured_evidence/claude-responsive: expected two different complete claude-measured token vectors; observed Claude A {:?} and Claude B {:?}; {}",
        claude_vectors[0],
        claude_vectors[1],
        complete_measurement(&output)
    );
}

fn assert_invalid_correlation(label: &str, input: &[u8], expected_diagnostic: &str) {
    let output = run_meter(&["log", "meter"], input);
    assert!(
        !output.status.success(),
        "meter_rejects_invalid_correlation/{label}: expected diagnostic {expected_diagnostic:?}; {}",
        complete_measurement(&output)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_diagnostic),
        "meter_rejects_invalid_correlation/{label}: expected diagnostic {expected_diagnostic:?}; {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        b"",
        "meter_rejects_invalid_correlation/{label}: expected zero stdout; {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_rejects_invalid_correlation() {
    let missing = jsonl(&[
        DISPATCH,
        r#"{"sequence":3,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":2,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation("missing", &missing, "missing issuance sequence 2");

    let non_dispatch = jsonl(&[
        r#"{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","kind":"delta","node":"m8-s1","payload":{"message":"not an issuance"}}"#,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation("non-dispatch", &non_dispatch, "non-dispatch sequence 1");

    let forward_own = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":2,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation(
        "forward-own",
        &forward_own,
        "completion 2 points forward to issuance 2",
    );

    let forward_later = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":3,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
        r#"{"sequence":3,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"later issuance"}}"#,
    ]);
    assert_invalid_correlation(
        "forward-later",
        &forward_later,
        "completion 2 points forward to issuance 3",
    );

    let duplicate = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
        r#"{"sequence":3,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":2,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation(
        "duplicate",
        &duplicate,
        "duplicate completion 3 for issuance 1",
    );

    let node_mismatch = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s2","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation(
        "node-mismatch",
        &node_mismatch,
        "node m8-s2 does not match issuance 1 node m8-s1",
    );
}

#[test]
fn strict_seatbelt_meter_excludes_transcripts() {
    if skip_without_nested_seatbelt() {
        return;
    }

    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = WorkspaceFixtureDirectory::create("strict-meter-transcripts")
        .expect("workspace poison fixture directory should be created");
    fixture.assert_outside_temporary_roots(repository);
    let poison_a = fixture.path().join("transcript-a.txt");
    let poison_b = fixture.path().join("transcript-b.txt");
    let sentinel_a = b"PCE_POISON_TRANSCRIPT_A_COMPLETE_SENTINEL\n";
    let sentinel_b = b"PCE_POISON_TRANSCRIPT_B_COMPLETE_SENTINEL\n";
    fs::write(&poison_a, sentinel_a).expect("poison transcript A should write");
    fs::write(&poison_b, sentinel_b).expect("poison transcript B should write");

    let pce_executable = fs::canonicalize(env!("CARGO_BIN_EXE_pce"))
        .expect("integration-test pce executable should canonicalize");
    let strict_profile = strict_seatbelt_profile(&pce_executable);
    let uncompleted_issuance = r#"{"sequence":9,"timestamp":"2026-08-02T12:42:46.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"strict harness uncompleted issuance"}}"#;
    let event_log = jsonl(&[
        MEASURED_CODEX_A,
        MEASURED_CLAUDE_A,
        MEASURED_CODEX_B,
        MEASURED_CLAUDE_B,
        uncompleted_issuance,
    ]);
    let unsandboxed_meter = run_pce(&pce_executable, None, &event_log);
    assert!(
        unsandboxed_meter.status.success(),
        "strict_seatbelt_meter_excludes_transcripts/unsandboxed-meter-control: expected successful unconfined meter run; {}",
        complete_measurement(&unsandboxed_meter)
    );

    let outside_probe_a = run_cat(&poison_a);
    let outside_probe_b = run_cat(&poison_b);
    assert!(
        outside_probe_a.status.success()
            && outside_probe_b.status.success()
            && outside_probe_a.stdout == sentinel_a
            && outside_probe_b.stdout == sentinel_b
            && outside_probe_a.stdout != outside_probe_b.stdout,
        "strict_seatbelt_meter_excludes_transcripts/transcript_probe_distinguishes_sentinels: expected successful transcript-preferring reads with distinct complete sentinel bytes; sentinel_a={sentinel_a:?}; sentinel_b={sentinel_b:?}; outside_a={}; outside_b={}",
        complete_measurement(&outside_probe_a),
        complete_measurement(&outside_probe_b)
    );

    let strict_preference_probe = run_strict_cat(&strict_profile, &poison_a);
    assert!(
        !strict_preference_probe
            .stdout
            .windows(outside_probe_a.stdout.len())
            .any(|window| window == outside_probe_a.stdout),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_transcript_preference/sentinel-absent: expected complete outside sentinel to be absent under strict profile; outside={}; strict={}",
        complete_measurement(&outside_probe_a),
        complete_measurement(&strict_preference_probe)
    );
    assert!(
        String::from_utf8_lossy(&strict_preference_probe.stderr)
            .contains("Operation not permitted"),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_transcript_preference/denial-visible: expected stderr substring {:?}; outside={}; strict={}",
        "Operation not permitted",
        complete_measurement(&outside_probe_a),
        complete_measurement(&strict_preference_probe)
    );

    let explicit_poison_read = run_strict_cat(&strict_profile, &poison_a);
    assert!(
        !explicit_poison_read
            .stdout
            .windows(outside_probe_a.stdout.len())
            .any(|window| window == outside_probe_a.stdout),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_explicit_poison_read/sentinel-absent: expected complete outside sentinel to be absent under strict profile; outside={}; strict={}",
        complete_measurement(&outside_probe_a),
        complete_measurement(&explicit_poison_read)
    );
    assert!(
        String::from_utf8_lossy(&explicit_poison_read.stderr).contains("Operation not permitted"),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_explicit_poison_read/denial-visible: expected stderr substring {:?}; outside={}; strict={}",
        "Operation not permitted",
        complete_measurement(&outside_probe_a),
        complete_measurement(&explicit_poison_read)
    );

    let outside_executable_read = run_cat(&pce_executable);
    assert!(
        outside_executable_read.status.success() && !outside_executable_read.stdout.is_empty(),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_permits_executable_literal/outside-readable: expected status_success=true and non-empty stdout; status_success={}; stdout_len={}; stderr={:?}",
        outside_executable_read.status.success(),
        outside_executable_read.stdout.len(),
        outside_executable_read.stderr
    );
    let strict_executable_read = run_strict_cat(&strict_profile, &pce_executable);
    assert_eq!(
        (
            strict_executable_read.status.success(),
            strict_executable_read.stdout.as_slice()
        ),
        (
            outside_executable_read.status.success(),
            outside_executable_read.stdout.as_slice()
        ),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_permits_executable_literal: expected identical (status_success, stdout) pairs; outside_status_success={}; outside_stdout_len={}; outside_stderr={:?}; strict_status_success={}; strict_stdout_len={}; strict_stderr={:?}",
        outside_executable_read.status.success(),
        outside_executable_read.stdout.len(),
        outside_executable_read.stderr,
        strict_executable_read.status.success(),
        strict_executable_read.stdout.len(),
        strict_executable_read.stderr
    );

    let strict_meter = run_pce(&pce_executable, Some(&strict_profile), &event_log);
    assert_eq!(
        (
            strict_meter.status.success(),
            strict_meter.stdout.as_slice()
        ),
        (true, unsandboxed_meter.stdout.as_slice()),
        "strict_seatbelt_meter_excludes_transcripts/strict_meter_matches_unsandboxed_bytes: expected strict success and byte-identical report; strict_status_success={}; strict_stderr={:?}; unsandboxed_stdout={:?}; strict_stdout={:?}",
        strict_meter.status.success(),
        strict_meter.stderr,
        unsandboxed_meter.stdout,
        strict_meter.stdout
    );
}
