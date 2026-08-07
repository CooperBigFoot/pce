mod support;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::{Value, json};
use support::{CliHarness, Invocation, ScriptedResponse};

const FINISHED_RESULT: &str = "main@0123456789abcdef";

struct Fixture {
    harness: CliHarness,
    log: PathBuf,
    vision_dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let harness = CliHarness::new().expect("CLI harness");
        let root = harness.path();
        let log = root.join("events.jsonl");
        let vision_dir = root.join("planning/2026-08-03-landing-fixture");
        fs::create_dir_all(&vision_dir).expect("vision directory");
        fs::write(
            vision_dir.join("vision.md"),
            concat!(
                "# Vision: landing fixture\n\n",
                "## Acceptance criteria (vision-level \"done\")\n\n",
                "```json\n",
                "{\"criteria\":[{\"name\":\"Runnable criterion\",\"input\":\"Run the finished command.\",\"observation\":\"It exits 0.\"},{\"name\":\"Install-only criterion\",\"input\":\"Install the hook, then attempt the forbidden command.\",\"observation\":\"The command is denied.\"}]}\n",
                "```\n"
            ),
        )
        .expect("vision document");
        fs::write(root.join("approved.json"), b"{}").expect("approved artifact");
        let contract = json!({
            "repository":"pce","repo_root":utf8(root),
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
        append(
            &harness,
            &log,
            "criterion-execution",
            r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"run the finished command"}"#,
        );
        append(
            &harness,
            &log,
            "criterion-execution",
            r#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"inspect the human-install boundary"}"#,
        );
        Self {
            harness,
            log,
            vision_dir,
        }
    }

    fn script(&self, github: &[u8], merged: bool) {
        let root = self.harness.path();
        let mut responses = vec![
            response("git", git_args(root, &["remote", "get-url", "origin"]), 0, b"https://example.invalid/pce.git\n"),
            response("git", git_args(root, &["fetch", "--no-tags", "origin", "refs/heads/pce/landing-fixture/milestone-6"]), 0, b""),
            response("git", git_args(root, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"]), 0, b"fetched-landing-oid\n"),
            response("git", git_args(root, &["show-ref", "--verify", "--quiet", "refs/heads/pce/landing-fixture/milestone-6"]), 0, b""),
            response("git", git_args(root, &["worktree", "list", "--porcelain"]), 0, format!("worktree {}/worktrees/m6-s1\nHEAD worktree-head-oid\nbranch refs/heads/pce/landing-fixture/m6-s1\n", root.display()).as_bytes()),
            response("git", git_args(root, &["rev-parse", "--verify", "--quiet", "refs/tags/v0.1.16^{}"]), 1, b""),
            response("gh", os_args(&["pr", "list", "--head", "pce/landing-fixture/m6-s1", "--base", "pce/landing-fixture/milestone-6", "--state", "all", "--limit", "1000", "--json", "number,headRefName,baseRefName,state,mergeCommit"]), 0, github),
        ];
        if merged {
            responses.push(response(
                "git",
                git_args(
                    root,
                    &[
                        "merge-base",
                        "--is-ancestor",
                        "squash-landing-oid",
                        "fetched-landing-oid",
                    ],
                ),
                0,
                b"",
            ));
        }
        self.harness
            .materialize_responses(&responses)
            .expect("script authorities");
    }

    fn check(&self) -> Output {
        self.harness
            .run(
                [
                    "landing",
                    "check",
                    "--file",
                    utf8(&self.log),
                    "--vision-dir",
                    utf8(&self.vision_dir),
                    "--finished-result",
                    FINISHED_RESULT,
                ],
                b"",
            )
            .expect("landing command")
    }
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
                "m6-s1",
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

fn utf8(path: &Path) -> &str {
    path.to_str().expect("UTF-8 fixture path")
}
fn os_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
fn git_args(root: &Path, tail: &[&str]) -> Vec<OsString> {
    let mut args = vec![OsString::from("-C"), root.as_os_str().to_owned()];
    args.extend(tail.iter().map(OsString::from));
    args
}
fn response(program: &str, argv: Vec<OsString>, exit_code: i32, stdout: &[u8]) -> ScriptedResponse {
    ScriptedResponse {
        program: OsString::from(program),
        argv,
        exit_code,
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

#[test]
fn binary_emits_exact_ready_proof_and_reuses_status_authorities() {
    let fixture = Fixture::new();
    fixture.script(br#"[{"number":61,"headRefName":"pce/landing-fixture/m6-s1","baseRefName":"pce/landing-fixture/milestone-6","state":"MERGED","mergeCommit":{"oid":"squash-landing-oid"}}]"#, true);
    let output = fixture.check();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let proof: Value = serde_json::from_slice(&output.stdout).expect("landing JSON");
    assert_eq!(proof["decision"], "ready");
    assert_eq!(proof["completion"]["decision"], "complete");
    assert_eq!(proof["steps"][0]["merge_status"], "merged");
    assert_eq!(proof["criterion_evidence"][0]["sequence"], 3);
    assert_eq!(proof["criterion_evidence"][1]["sequence"], 4);
    assert_eq!(proof["problems"], json!([]));
    let root = fixture.harness.path();
    let expected = vec![
        invocation("git", git_args(root, &["remote", "get-url", "origin"])),
        invocation(
            "git",
            git_args(
                root,
                &[
                    "fetch",
                    "--no-tags",
                    "origin",
                    "refs/heads/pce/landing-fixture/milestone-6",
                ],
            ),
        ),
        invocation(
            "git",
            git_args(root, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"]),
        ),
        invocation(
            "git",
            git_args(
                root,
                &[
                    "show-ref",
                    "--verify",
                    "--quiet",
                    "refs/heads/pce/landing-fixture/milestone-6",
                ],
            ),
        ),
        invocation("git", git_args(root, &["worktree", "list", "--porcelain"])),
        invocation(
            "git",
            git_args(
                root,
                &["rev-parse", "--verify", "--quiet", "refs/tags/v0.1.16^{}"],
            ),
        ),
        invocation(
            "gh",
            os_args(&[
                "pr",
                "list",
                "--head",
                "pce/landing-fixture/m6-s1",
                "--base",
                "pce/landing-fixture/milestone-6",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
        invocation(
            "git",
            git_args(
                root,
                &[
                    "merge-base",
                    "--is-ancestor",
                    "squash-landing-oid",
                    "fetched-landing-oid",
                ],
            ),
        ),
    ];
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        expected
    );
}

fn invocation(program: &str, argv: Vec<OsString>) -> Invocation {
    Invocation {
        program: OsString::from(program),
        argv,
    }
}

#[test]
fn binary_emits_refusal_before_nonzero_exit() {
    let fixture = Fixture::new();
    fixture.script(br#"[{"number":61,"headRefName":"pce/landing-fixture/m6-s1","baseRefName":"pce/landing-fixture/milestone-6","state":"OPEN","mergeCommit":null}]"#, false);
    let output = fixture.check();
    assert_eq!(output.status.code(), Some(1));
    let proof: Value = serde_json::from_slice(&output.stdout).expect("refusal JSON");
    assert_eq!(
        proof["steps"][0]["github"]["pull_request"]["state"]["status"],
        "not-merged"
    );
    assert_eq!(proof["steps"][0]["git"]["state"], "not-merged");
    assert_eq!(
        proof["problems"],
        json!([{"reason":"step-not-merged","node":"m6-s1"}])
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("landing refused: 1 readiness problems")
    );
}

#[test]
fn binary_refuses_ambiguous_authority_without_collapsing_it() {
    let fixture = Fixture::new();
    fixture.script(br#"[{"number":61,"headRefName":"pce/landing-fixture/m6-s1","baseRefName":"pce/landing-fixture/milestone-6","state":"MERGED","mergeCommit":{"oid":"one"}},{"number":62,"headRefName":"pce/landing-fixture/m6-s1","baseRefName":"pce/landing-fixture/milestone-6","state":"MERGED","mergeCommit":{"oid":"two"}}]"#, false);
    let output = fixture.check();
    assert_eq!(output.status.code(), Some(1));
    let proof: Value = serde_json::from_slice(&output.stdout).expect("refusal JSON");
    assert_eq!(proof["steps"][0]["merge_status"], "inconclusive");
    assert_eq!(
        proof["steps"][0]["github"]["cardinality"],
        "multiple-exact-matches"
    );
    assert_eq!(proof["steps"][0]["git"]["availability"], "unreachable");
    assert_eq!(
        proof["steps"][0]["git"]["failure"],
        "multiple exact GitHub pull requests prevent squash OID selection"
    );
    assert_eq!(
        proof["problems"],
        json!([{"reason":"step-inconclusive","node":"m6-s1"}])
    );
}

#[test]
fn unevidenced_event_is_rejected_before_a_landing_result() {
    for (evidence, expected) in [
        (
            "",
            "event kind criterion-execution requires an evidence field",
        ),
        (",\"evidence\":\"\"", "event evidence cannot be empty: \"\""),
    ] {
        let harness = CliHarness::new().expect("CLI harness");
        let log = harness.path().join("events.jsonl");
        let mut line = format!(
            r#"{{"sequence":1,"timestamp":"2026-08-03T00:00:00Z","kind":"criterion-execution","node":"m6-s1","payload":{{"criterion":{{"name":"Runnable criterion","input":"Run it.","observation":"It exits 0."}},"finished_result":"main@0123456789abcdef","outcome":{{"status":"passed","observed_result":"ok"}}{evidence}}}}}"#
        );
        line.push('\n');
        fs::write(&log, line).expect("malformed event fixture");
        let output = harness
            .run(
                [
                    "landing",
                    "check",
                    "--file",
                    utf8(&log),
                    "--vision-dir",
                    "vision",
                    "--finished-result",
                    FINISHED_RESULT,
                ],
                b"",
            )
            .expect("landing command");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected),
            "expected {expected:?} in {stderr:?}"
        );
    }
}
