mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::json;
use support::CliHarness;

const FINISHED_RESULT: &str = "main@0123456789abcdef";

struct Fixture {
    harness: CliHarness,
    log: PathBuf,
    vision_dir: PathBuf,
}

impl Fixture {
    fn new(criteria: &str) -> Self {
        let harness = CliHarness::new().expect("CLI harness");
        let root = harness.path();
        let log = root.join("events.jsonl");
        let vision_dir = root.join("planning/2026-08-03-completion-fixture");
        fs::create_dir_all(&vision_dir).expect("vision directory");
        fs::write(
            vision_dir.join("vision.md"),
            format!("# Vision: completion fixture\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{{\"criteria\":[{criteria}]}}\n```\n"),
        )
        .expect("vision document");
        fs::write(root.join("approved.json"), b"{}").expect("approved artifact");

        let contract = json!({
            "repository":"pce","repo_root":root.to_str().expect("UTF-8 root"),
            "stated":{"format":"cargo fmt --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --release","version_policy":"NONE","branch_convention":"pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>","pull_request_convention":"step head targets the matching milestone integration branch"},
            "observations":{"format":0,"lint":0,"typecheck":0,"test":0,"build":0},
            "workflow_map":{"ci.yml":"cargo test --workspace","docs.yml":null},
            "appendable":{"environment_hazards":["stdin is reserved for event payload input"],"gate_orderings":["format before lint before typecheck before test before build"],"lockfile_rules":["Cargo.lock must remain synchronized with Cargo.toml"]},
            "evidence":"cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"
        });
        append(&harness, &log, "repository-contract", &contract.to_string());
        append(
            &harness,
            &log,
            "planning-artifact-approved",
            r#"{"path":"approved.json","sha256":"44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a","evidence":"sha256 of exact approved.json bytes"}"#,
        );
        Self {
            harness,
            log,
            vision_dir,
        }
    }

    fn append(&self, kind: &str, payload: &str) {
        append(&self.harness, &self.log, kind, payload);
    }

    fn check(&self, result: &str) -> Output {
        self.harness
            .run(
                [
                    "completion",
                    "check",
                    "--file",
                    utf8(&self.log),
                    "--vision-dir",
                    utf8(&self.vision_dir),
                    "--finished-result",
                    result,
                ],
                b"",
            )
            .expect("completion command")
    }
}

fn utf8(path: &Path) -> &str {
    path.to_str().expect("UTF-8 fixture path")
}

fn append(harness: &CliHarness, log: &Path, kind: &str, payload: &str) {
    let output = harness
        .run(
            [
                "log",
                "--file",
                utf8(log),
                "--kind",
                kind,
                "--node",
                "m3-s3",
            ],
            payload.as_bytes(),
        )
        .expect("append event");
    assert!(
        output.status.success(),
        "append {kind}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn refusal_then_later_completion_emit_exact_json() {
    let criteria = r#"{"name":"Missing criterion","input":"Run the missing command.","observation":"It exits 0."},{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."}"#;
    let fixture = Fixture::new(criteria);
    fixture.append("criterion-execution", r#"{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed","observed_result":"The command exited 7."},"evidence":"run the broken command"}"#);
    fixture.append("criterion-execution", r#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"inspect the human-install boundary"}"#);

    let refused = fixture.check(FINISHED_RESULT);
    assert_eq!(refused.status.code(), Some(1));
    let expected_refusal = concat!(
        r#"{"decision":"refuse","finished_result":"main@0123456789abcdef","criteria":[{"criterion":{"name":"Missing criterion","input":"Run the missing command.","observation":"It exits 0."},"status":"missing"},{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"status":"failed","observed_result":"The command exited 7."},{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"status":"unpaid","reason":"The run cannot activate the human-installed hook."}]}"#,
        "\n"
    );
    assert_eq!(refused.stdout, expected_refusal.as_bytes());
    assert!(
        String::from_utf8_lossy(&refused.stderr)
            .contains("completion refused: 2 blocking criteria (1 missing, 1 failed)")
    );

    fixture.append("criterion-execution", r#"{"criterion":{"name":"Missing criterion","input":"Run the missing command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"run the missing command"}"#);
    fixture.append("criterion-execution", r#"{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"rerun the repaired command"}"#);
    let complete = fixture.check(FINISHED_RESULT);
    assert!(complete.status.success());
    let expected_complete = concat!(
        r#"{"decision":"complete","finished_result":"main@0123456789abcdef","criteria":[{"criterion":{"name":"Missing criterion","input":"Run the missing command.","observation":"It exits 0."},"status":"passed","observed_result":"The command exited 0."},{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"status":"passed","observed_result":"The command exited 0."},{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"status":"unpaid","reason":"The run cannot activate the human-installed hook."}]}"#,
        "\n"
    );
    assert_eq!(complete.stdout, expected_complete.as_bytes());
}

#[test]
fn unpaid_after_failure_keeps_exact_failure_blocking() {
    let fixture = Fixture::new(
        r#"{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."}"#,
    );
    fixture.append("criterion-execution", r#"{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed","observed_result":"The command exited 7."},"evidence":"run the broken command"}"#);
    fixture.append("criterion-execution", r#"{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot execute the broken command now."},"evidence":"inspect the execution boundary"}"#);

    let output = fixture.check(FINISHED_RESULT);
    assert_eq!(output.status.code(), Some(1));
    let expected = concat!(
        r#"{"decision":"refuse","finished_result":"main@0123456789abcdef","criteria":[{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"status":"failed","observed_result":"The command exited 7."}]}"#,
        "\n"
    );
    assert_eq!(output.stdout, expected.as_bytes());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("completion refused: 1 blocking criteria (0 missing, 1 failed)")
    );
}

#[test]
fn evidence_for_a_different_finished_result_does_not_pay() {
    let fixture = Fixture::new(
        r#"{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."}"#,
    );
    fixture.append("criterion-execution", r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@fedcba9876543210","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"run the finished command"}"#);
    let output = fixture.check(FINISHED_RESULT);
    assert_eq!(output.status.code(), Some(1));
    let expected = concat!(
        r#"{"decision":"refuse","finished_result":"main@0123456789abcdef","criteria":[{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"status":"missing"}]}"#,
        "\n"
    );
    assert_eq!(output.stdout, expected.as_bytes());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("completion refused: 1 blocking criteria (1 missing, 0 failed)")
    );
}

#[test]
fn blank_finished_result_fails_before_emitting_a_decision() {
    let harness = CliHarness::new().expect("CLI harness");
    let output = harness
        .run(
            [
                "completion",
                "check",
                "--file",
                "events.jsonl",
                "--vision-dir",
                "planning/2026-08-03-completion-fixture",
                "--finished-result",
                "   ",
            ],
            b"",
        )
        .expect("completion command");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse finished result"));
    assert!(stderr.contains("finished result cannot be blank"));
}
