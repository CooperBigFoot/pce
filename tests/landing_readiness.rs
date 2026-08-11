mod support;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::{Value, json};
use support::{CliHarness, Invocation, ScriptedResponse};

const FINISHED_RESULT: &str = "main@0123456789abcdef";
const READY_STDOUT: &str = concat!(
    r#"{"decision":"ready","completion":{"decision":"complete","finished_result":"main@0123456789abcdef","criteria":[{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"status":"passed","observed_result":"The command exited 0."},{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"status":"unpaid","reason":"The run cannot activate the human-installed hook."}]},"steps":[{"node":"m6-s1","subject":{"milestone":6,"step":1,"head_branch":"pce/landing-fixture/m6-s1","integration_branch":"pce/landing-fixture/milestone-6","pull_request_selector":{"head":"pce/landing-fixture/m6-s1","base":"pce/landing-fixture/milestone-6"}},"github":{"availability":"reachable","cardinality":"one-exact-match","pull_request":{"number":61,"selector":{"head":"pce/landing-fixture/m6-s1","base":"pce/landing-fixture/milestone-6"},"state":{"status":"merged","squash_commit_oid":"squash-landing-oid"}}},"git":{"availability":"reachable","state":"squash-commit-reachable","squash_commit_oid":"squash-landing-oid"},"merge_status":"merged"}],"criterion_evidence":[{"status":"recorded","criterion_index":0,"sequence":3,"node":"m6-s1","criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"run the finished command"},{"status":"recorded","criterion_index":1,"sequence":4,"node":"m6-s1","criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"inspect the human-install boundary"}],"problems":[]}"#,
    "\n"
);

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
    append_at(harness, log, kind, "m6-s1", payload);
}

fn append_at(harness: &CliHarness, log: &Path, kind: &str, node: &str, payload: &str) {
    let output = harness
        .run(
            ["log", "--file", utf8(log), "--kind", kind, "--node", node],
            payload.as_bytes(),
        )
        .expect("append event");
    assert!(
        output.status.success(),
        "append {kind}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

const STEP_HEAD: &str = "pce/a-dispatch-outlives-the-call-that-started-it/m1-s3";
const INTEGRATION: &str = "pce/a-dispatch-outlives-the-call-that-started-it/milestone-1b";
const STEP_SQUASH: &str = "798ca422c9576983c8be362481169632952c578b";
const PROMOTION_SQUASH: &str = "e9a3eed73d6852ca9652b1af083127ba11126cc1";

struct ExceptionalFixture {
    harness: CliHarness,
    log: PathBuf,
    vision_dir: PathBuf,
}

impl ExceptionalFixture {
    fn new(branch: &str, step_pr: u64) -> Self {
        let harness = CliHarness::new().expect("CLI harness");
        let root = harness.path();
        let log = root.join("events.jsonl");
        let vision_dir =
            root.join("planning/2026-08-09-a-dispatch-outlives-the-call-that-started-it");
        fs::create_dir_all(&vision_dir).expect("vision directory");
        fs::write(vision_dir.join("vision.md"), "# Vision\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"Historical chain\",\"input\":\"Observe the chain.\",\"observation\":\"Both hops merged.\"}]}\n```\n").expect("vision");
        let contract = json!({
            "repository":"pce","repo_root":utf8(root),
            "stated":{"format":"cargo fmt --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --release","version_policy":"NONE","branch_convention":"fixture","pull_request_convention":"fixture"},
            "observations":{"format":0,"lint":0,"typecheck":0,"test":0,"build":0},
            "workflow_map":{},"appendable":{"environment_hazards":[],"gate_orderings":[],"lockfile_rules":[]},"evidence":"fixture"
        });
        append_at(
            &harness,
            &log,
            "repository-contract",
            "m1-s3",
            &contract.to_string(),
        );
        append_at(&harness, &log, "exceptional-merge-chain-declared", "m1-s3", &json!({
            "integration_branch":branch,
            "step_pull_request_number":step_pr,
            "promotion_pull_request_number":180,
            "evidence":"gh pr view 179 --json number,headRefName,baseRefName,state,mergeCommit && gh pr view 180 --json number,headRefName,baseRefName,state,mergeCommit"
        }).to_string());
        append_at(
            &harness,
            &log,
            "delta",
            "m1-s3",
            r#"{"message":"declaration is deliberately not the last m1-s3 record"}"#,
        );
        append_at(
            &harness,
            &log,
            "criterion-execution",
            "m1-s3",
            r#"{"criterion":{"name":"Historical chain","input":"Observe the chain.","observation":"Both hops merged."},"finished_result":"e9a3eed73d6852ca9652b1af083127ba11126cc1","outcome":{"status":"passed","observed_result":"Both hops merged."},"evidence":"fixture acceptance observation"}"#,
        );
        Self {
            harness,
            log,
            vision_dir,
        }
    }

    fn script(
        &self,
        branch: &str,
        step_json: &[u8],
        promotion_json: &[u8],
        step_merge_exit: Option<i32>,
        promotion_merge_exit: Option<i32>,
    ) {
        let root = self.harness.path();
        let mut responses = vec![
            response(
                "git",
                git_args(root, &["remote", "get-url", "origin"]),
                0,
                b"https://example.invalid/pce.git\n",
            ),
            response(
                "git",
                git_args(
                    root,
                    &[
                        "fetch",
                        "--no-tags",
                        "origin",
                        &format!("refs/heads/{branch}"),
                    ],
                ),
                0,
                b"",
            ),
            response(
                "git",
                git_args(root, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"]),
                0,
                b"fixture-milestone-1b-tip\n",
            ),
            response(
                "git",
                git_args(root, &["fetch", "--no-tags", "origin", "refs/heads/main"]),
                0,
                b"",
            ),
            response(
                "git",
                git_args(
                    root,
                    &[
                        "show-ref",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{branch}"),
                    ],
                ),
                0,
                b"",
            ),
            response(
                "git",
                git_args(root, &["worktree", "list", "--porcelain"]),
                0,
                format!(
                    "worktree {}/worktrees/m1-s3\nHEAD oid\nbranch refs/heads/{STEP_HEAD}\n",
                    root.display()
                )
                .as_bytes(),
            ),
            response(
                "git",
                git_args(
                    root,
                    &["rev-parse", "--verify", "--quiet", "refs/tags/v0.1.16^{}"],
                ),
                1,
                b"",
            ),
            response(
                "gh",
                os_args(&[
                    "pr",
                    "list",
                    "--head",
                    STEP_HEAD,
                    "--base",
                    branch,
                    "--state",
                    "all",
                    "--limit",
                    "1000",
                    "--json",
                    "number,headRefName,baseRefName,state,mergeCommit",
                ]),
                0,
                step_json,
            ),
            response(
                "gh",
                os_args(&[
                    "pr",
                    "list",
                    "--head",
                    branch,
                    "--base",
                    "main",
                    "--state",
                    "all",
                    "--limit",
                    "1000",
                    "--json",
                    "number,headRefName,baseRefName,state,mergeCommit",
                ]),
                0,
                promotion_json,
            ),
        ];
        if let Some(exit) = step_merge_exit {
            responses.push(response(
                "git",
                git_args(
                    root,
                    &[
                        "merge-base",
                        "--is-ancestor",
                        STEP_SQUASH,
                        "fixture-milestone-1b-tip",
                    ],
                ),
                exit,
                b"",
            ));
        }
        if let Some(exit) = promotion_merge_exit {
            responses.push(response(
                "git",
                git_args(
                    root,
                    &[
                        "merge-base",
                        "--is-ancestor",
                        PROMOTION_SQUASH,
                        "fixture-main-tip",
                    ],
                ),
                exit,
                b"",
            ));
        }
        self.harness
            .materialize_responses(&responses)
            .expect("script exceptional authorities");
        let shim_dir = PathBuf::from(
            self.harness
                .shim_path()
                .split(':')
                .next()
                .expect("shim directory"),
        );
        let original = shim_dir.join("git-base");
        fs::copy(shim_dir.join("git"), &original).expect("copy git shim");
        let counter = self.harness.path().join("rev-parse-count");
        let wrapper = format!(
            r#"#!/bin/sh
output="$({original} "$@")"
status=$?
if [ "$1" = "-C" ] && [ "$3" = "rev-parse" ] && [ "$4" = "--verify" ] && [ "$5" = "FETCH_HEAD^{{commit}}" ]; then
  count=0
  if [ -f "{counter}" ]; then count=$(sed -n '1p' "{counter}"); fi
  count=$((count + 1))
  printf '%s\n' "$count" > "{counter}"
  if [ "$count" -eq 2 ]; then output="fixture-main-tip"; fi
fi
printf '%s\n' "$output"
exit "$status"
"#,
            original = original.display(),
            counter = counter.display()
        );
        self.harness
            .install_shim("git", &wrapper)
            .expect("stateful git shim");
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
                    PROMOTION_SQUASH,
                ],
                b"",
            )
            .expect("landing command")
    }
}

fn merged_pr(number: u64, head: &str, base: &str, oid: &str) -> Vec<u8> {
    serde_json::to_vec(&json!([{"number":number,"headRefName":head,"baseRefName":base,"state":"MERGED","mergeCommit":{"oid":oid}}])).expect("PR JSON")
}

fn not_merged_pr(number: u64, head: &str, base: &str, state: &str) -> Vec<u8> {
    serde_json::to_vec(&json!([{"number":number,"headRefName":head,"baseRefName":base,"state":state,"mergeCommit":null}])).expect("PR JSON")
}

fn assert_exceptional_refusal(output: &Output, status: &str, reason: &str) -> Value {
    assert_eq!(output.status.code(), Some(1));
    let value: Value = serde_json::from_slice(&output.stdout).expect("landing JSON");
    assert_eq!(value["steps"][0]["merge_status"], status);
    assert_eq!(value["problems"], json!([{"reason":reason,"node":"m1-s3"}]));
    value
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
    assert_eq!(output.stdout, READY_STDOUT.as_bytes());
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

fn exceptional_invocations(
    root: &Path,
    branch: &str,
    step_oid: Option<&str>,
    promotion_oid: Option<&str>,
) -> Vec<Invocation> {
    let mut calls = vec![
        invocation("git", git_args(root, &["remote", "get-url", "origin"])),
        invocation(
            "git",
            git_args(
                root,
                &[
                    "fetch",
                    "--no-tags",
                    "origin",
                    &format!("refs/heads/{branch}"),
                ],
            ),
        ),
        invocation(
            "git",
            git_args(root, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"]),
        ),
        invocation(
            "git",
            git_args(root, &["fetch", "--no-tags", "origin", "refs/heads/main"]),
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
                    &format!("refs/heads/{branch}"),
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
                STEP_HEAD,
                "--base",
                branch,
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
    ];
    if let Some(oid) = step_oid {
        calls.push(invocation(
            "git",
            git_args(
                root,
                &[
                    "merge-base",
                    "--is-ancestor",
                    oid,
                    "fixture-milestone-1b-tip",
                ],
            ),
        ));
    }
    calls.push(invocation(
        "gh",
        os_args(&[
            "pr",
            "list",
            "--head",
            branch,
            "--base",
            "main",
            "--state",
            "all",
            "--limit",
            "1000",
            "--json",
            "number,headRefName,baseRefName,state,mergeCommit",
        ]),
    ));
    if let Some(oid) = promotion_oid {
        calls.push(invocation(
            "git",
            git_args(
                root,
                &["merge-base", "--is-ancestor", oid, "fixture-main-tip"],
            ),
        ));
    }
    calls
}

#[test]
fn real_m1_s3_declared_chain_is_ready_only_after_both_hops_verify() {
    let fixture = ExceptionalFixture::new(INTEGRATION, 179);
    fixture.script(
        INTEGRATION,
        &merged_pr(179, STEP_HEAD, INTEGRATION, STEP_SQUASH),
        &merged_pr(180, INTEGRATION, "main", PROMOTION_SQUASH),
        Some(0),
        Some(0),
    );
    let output = fixture.check();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("landing JSON");
    assert_eq!(value["decision"], "ready");
    assert_eq!(value["problems"], json!([]));
    let step = &value["steps"][0];
    assert_eq!(step["merge_status"], "merged");
    assert_eq!(
        step["exceptional_merge_chain"]["step_pull_request_number"],
        179
    );
    assert_eq!(
        step["exceptional_merge_chain"]["promotion_pull_request_number"],
        180
    );
    assert_eq!(
        step["exceptional_merge_chain"]["step_to_integration"]["merge_status"],
        "merged"
    );
    assert_eq!(
        step["exceptional_merge_chain"]["integration_to_default"]["merge_status"],
        "merged"
    );
    assert_eq!(
        step["exceptional_merge_chain"]["step_to_integration"]["github"]["pull_request"]["state"]["squash_commit_oid"],
        STEP_SQUASH
    );
    assert_eq!(
        step["exceptional_merge_chain"]["integration_to_default"]["github"]["pull_request"]["state"]
            ["squash_commit_oid"],
        PROMOTION_SQUASH
    );
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        exceptional_invocations(
            fixture.harness.path(),
            INTEGRATION,
            Some(STEP_SQUASH),
            Some(PROMOTION_SQUASH)
        )
    );
}

#[test]
fn declared_existing_branch_that_is_not_the_step_pr_base_refuses() {
    let branch = "pce/a-dispatch-outlives-the-call-that-started-it/milestone-1";
    let fixture = ExceptionalFixture::new(branch, 179);
    fixture.script(
        branch,
        &merged_pr(179, STEP_HEAD, INTEGRATION, STEP_SQUASH),
        &merged_pr(180, branch, "main", PROMOTION_SQUASH),
        None,
        Some(0),
    );
    let output = fixture.check();
    assert_exceptional_refusal(&output, "not-merged", "step-not-merged");
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        exceptional_invocations(fixture.harness.path(), branch, None, Some(PROMOTION_SQUASH))
    );
}

#[test]
fn declared_pull_request_that_is_not_merged_refuses() {
    let fixture = ExceptionalFixture::new(INTEGRATION, 179);
    fixture.script(
        INTEGRATION,
        &not_merged_pr(179, STEP_HEAD, INTEGRATION, "OPEN"),
        &merged_pr(180, INTEGRATION, "main", PROMOTION_SQUASH),
        None,
        Some(0),
    );
    let output = fixture.check();
    let value = assert_exceptional_refusal(&output, "not-merged", "step-not-merged");
    assert_eq!(
        value["steps"][0]["exceptional_merge_chain"]["step_to_integration"]["merge_status"],
        "not-merged"
    );
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        exceptional_invocations(
            fixture.harness.path(),
            INTEGRATION,
            None,
            Some(PROMOTION_SQUASH)
        )
    );
}

#[test]
fn github_squash_unreachable_from_declared_integration_branch_refuses() {
    let fixture = ExceptionalFixture::new(INTEGRATION, 179);
    fixture.script(
        INTEGRATION,
        &merged_pr(179, STEP_HEAD, INTEGRATION, STEP_SQUASH),
        &merged_pr(180, INTEGRATION, "main", PROMOTION_SQUASH),
        Some(1),
        Some(0),
    );
    let output = fixture.check();
    let value = assert_exceptional_refusal(&output, "inconclusive", "step-inconclusive");
    assert_eq!(
        value["steps"][0]["exceptional_merge_chain"]["step_to_integration"]["git"]["state"],
        "not-merged"
    );
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        exceptional_invocations(
            fixture.harness.path(),
            INTEGRATION,
            Some(STEP_SQUASH),
            Some(PROMOTION_SQUASH)
        )
    );
}

#[test]
fn absent_or_unmerged_promotion_hop_refuses() {
    for promotion in [
        b"[]".to_vec(),
        not_merged_pr(180, INTEGRATION, "main", "CLOSED"),
    ] {
        let fixture = ExceptionalFixture::new(INTEGRATION, 179);
        fixture.script(
            INTEGRATION,
            &merged_pr(179, STEP_HEAD, INTEGRATION, STEP_SQUASH),
            &promotion,
            Some(0),
            None,
        );
        let output = fixture.check();
        let value = assert_exceptional_refusal(&output, "not-merged", "step-not-merged");
        assert_eq!(
            value["steps"][0]["exceptional_merge_chain"]["integration_to_default"]["merge_status"],
            "not-merged"
        );
        assert_eq!(
            fixture.harness.invocations().expect("invocations"),
            exceptional_invocations(fixture.harness.path(), INTEGRATION, Some(STEP_SQUASH), None)
        );
    }
}

#[test]
fn declared_pr_number_must_equal_the_unique_exact_match() {
    let fixture = ExceptionalFixture::new(INTEGRATION, 178);
    fixture.script(
        INTEGRATION,
        &merged_pr(179, STEP_HEAD, INTEGRATION, STEP_SQUASH),
        &merged_pr(180, INTEGRATION, "main", PROMOTION_SQUASH),
        Some(0),
        Some(0),
    );
    let output = fixture.check();
    assert_exceptional_refusal(&output, "inconclusive", "step-inconclusive");
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        exceptional_invocations(
            fixture.harness.path(),
            INTEGRATION,
            Some(STEP_SQUASH),
            Some(PROMOTION_SQUASH)
        )
    );
}

#[test]
fn declaration_free_node_preserves_derived_route_byte_for_byte() {
    assert!(matches!(
        pce_core::StepMergeRoute::Derived,
        pce_core::StepMergeRoute::Derived
    ));
    let fixture = Fixture::new();
    fixture.script(br#"[{"number":61,"headRefName":"pce/landing-fixture/m6-s1","baseRefName":"pce/landing-fixture/milestone-6","state":"MERGED","mergeCommit":{"oid":"squash-landing-oid"}}]"#, true);
    let output = fixture.check();
    assert_eq!(output.stdout, READY_STDOUT.as_bytes());
    let calls = fixture.harness.invocations().expect("invocations");
    assert_eq!(calls.len(), 8);
    assert!(
        !calls
            .iter()
            .any(|call| call.argv.iter().any(|arg| arg == "main"))
    );
}

#[test]
fn declaration_bearing_status_snapshot_is_schema_valid_and_exact() {
    let fixture = ExceptionalFixture::new(INTEGRATION, 179);
    let declaration = fs::read_to_string(&fixture.log)
        .expect("event log")
        .lines()
        .map(|line| pce_core::parse_event_line(line).expect("event record"))
        .find(|record| {
            matches!(
                record.body_ref(),
                pce_core::EventBodyRef::Known(
                    pce_core::KnownPayload::ExceptionalMergeChainDeclared(_)
                )
            )
        })
        .expect("typed exceptional declaration");
    let pce_core::EventBodyRef::Known(pce_core::KnownPayload::ExceptionalMergeChainDeclared(
        payload,
    )) = declaration.body_ref()
    else {
        unreachable!()
    };
    assert_eq!(declaration.node().as_str(), "m1-s3");
    assert_eq!(payload.integration_branch.as_str(), INTEGRATION);
    assert_eq!(payload.step_pull_request_number.get(), 179);
    assert_eq!(payload.promotion_pull_request_number.get(), 180);
    assert_eq!(
        payload.evidence.as_str(),
        "gh pr view 179 --json number,headRefName,baseRefName,state,mergeCommit && gh pr view 180 --json number,headRefName,baseRefName,state,mergeCommit"
    );
    let root = fixture.harness.path();
    let derived_base = "pce/a-dispatch-outlives-the-call-that-started-it/milestone-1";
    fixture
        .harness
        .materialize_responses(&[
            response(
                "git",
                git_args(root, &["remote", "get-url", "origin"]),
                0,
                b"https://example.invalid/pce.git\n",
            ),
            response(
                "git",
                git_args(
                    root,
                    &[
                        "fetch",
                        "--no-tags",
                        "origin",
                        &format!("refs/heads/{derived_base}"),
                    ],
                ),
                0,
                b"",
            ),
            response(
                "git",
                git_args(root, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"]),
                0,
                b"derived-tip\n",
            ),
            response(
                "git",
                git_args(
                    root,
                    &[
                        "show-ref",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{derived_base}"),
                    ],
                ),
                0,
                b"",
            ),
            response(
                "git",
                git_args(root, &["worktree", "list", "--porcelain"]),
                0,
                format!(
                    "worktree {}/worktrees/m1-s3\nHEAD oid\nbranch refs/heads/{STEP_HEAD}\n",
                    root.display()
                )
                .as_bytes(),
            ),
            response(
                "git",
                git_args(
                    root,
                    &["rev-parse", "--verify", "--quiet", "refs/tags/v0.1.16^{}"],
                ),
                1,
                b"",
            ),
            response(
                "gh",
                os_args(&[
                    "pr",
                    "list",
                    "--head",
                    STEP_HEAD,
                    "--base",
                    derived_base,
                    "--state",
                    "all",
                    "--limit",
                    "1000",
                    "--json",
                    "number,headRefName,baseRefName,state,mergeCommit",
                ]),
                0,
                b"[]",
            ),
        ])
        .expect("status authorities");
    let output = fixture
        .harness
        .run(
            [
                "status",
                "--file",
                utf8(&fixture.log),
                "--vision-dir",
                utf8(&fixture.vision_dir),
            ],
            b"",
        )
        .expect("status command");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("snapshot JSON");
    let steps = snapshot["steps"].as_array().expect("steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0],
        json!({
            "node":"m1-s3",
            "subject":{"milestone":1,"step":3,"head_branch":STEP_HEAD,"integration_branch":derived_base,"pull_request_selector":{"head":STEP_HEAD,"base":derived_base}},
            "github":{"availability":"reachable","cardinality":"zero-exact-matches"},
            "git":{"availability":"reachable","state":"not-merged"},
            "merge_status":"not-merged"
        })
    );
    let calls = fixture.harness.invocations().expect("invocations");
    assert_eq!(calls.len(), 7);
    assert!(!calls.iter().any(|call| {
        call.argv
            .iter()
            .any(|arg| arg == "main" || arg == INTEGRATION)
    }));
}
