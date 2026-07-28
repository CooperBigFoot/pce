mod support;

use std::ffi::OsString;
use std::fs;

use serde_json::{Value, json};
use support::{CliHarness, Invocation, ScriptedResponse};

#[test]
fn missing_evidence_rejection_appends_zero_bytes() {
    let harness = CliHarness::new().expect("create CLI harness");
    let log = harness.path().join("events.jsonl");

    let seed = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("delta"),
                OsString::from("--node"),
                OsString::from("m3-s1"),
            ],
            br#"{"message":"seed before rejected fact"}"#,
        )
        .expect("run seed log command");
    assert!(seed.status.success(), "seed stderr: {}", stderr(&seed));
    let length_before = fs::metadata(&log).expect("read seeded log metadata").len();
    assert!(length_before > 0);

    let rejected = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("key-finding"),
                OsString::from("--node"),
                OsString::from("m3-s1"),
            ],
            br#"{"finding":"missing evidence must not append"}"#,
        )
        .expect("run rejected log command");
    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("failed to validate submitted event payload"),
        "rejection stderr: {}",
        stderr(&rejected)
    );
    assert!(
        stderr(&rejected).contains(
            "submitted event payload is invalid: event kind key-finding requires an evidence field"
        ),
        "rejection stderr: {}",
        stderr(&rejected)
    );
    let length_after = fs::metadata(&log)
        .expect("read rejected log metadata")
        .len();
    assert_eq!(length_after, length_before);
}

#[test]
fn status_smoke_uses_every_isolated_adapter_path() {
    let harness = CliHarness::new().expect("create CLI harness");
    let root = harness.path().join("repo");
    let vision_dir = root.join("planning/2026-07-27-shim-smoke");
    let log = vision_dir.join("events.jsonl");
    fs::create_dir_all(&vision_dir).expect("create scratch vision directory");

    let contract = json!({
        "repository": "pce",
        "repo_root": root,
        "stack": "Rust test fixture",
        "format": "cargo fmt --all --check",
        "lint": "cargo clippy --workspace --all-targets",
        "typecheck": "cargo check --workspace --all-targets",
        "test": "cargo test --workspace",
        "build": "cargo build --workspace",
        "preflight": "cargo check --workspace --all-targets",
        "gates_rule": "all four fixture gates must pass",
        "install": "none",
        "evidence": "fixture repository contract"
    })
    .to_string();
    let seed = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("repository-contract"),
                OsString::from("--node"),
                OsString::from("m3-s1"),
            ],
            contract.as_bytes(),
        )
        .expect("run repository-contract log command");
    assert!(seed.status.success(), "seed stderr: {}", stderr(&seed));

    let root_arg = root.as_os_str().to_owned();
    let responses = vec![
        response(
            "git",
            argv(["-C", root.to_str().expect("UTF-8 root"), "remote", "get-url", "origin"]),
            0,
            b"https://example.invalid/pce.git\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "fetch",
                "--no-tags",
                "origin",
                "refs/heads/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "rev-parse",
                "--verify",
                "FETCH_HEAD^{commit}",
            ]),
            0,
            b"fetched-oid\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "show-ref",
                "--verify",
                "--quiet",
                "refs/heads/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "worktree",
                "list",
                "--porcelain",
            ]),
            0,
            format!(
                "worktree {}/worktrees/m3-s1\nHEAD worktree-head-oid\nbranch refs/heads/pce/shim-smoke/m3-s1\n",
                root.display()
            )
            .as_bytes(),
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/tags/v0.1.16^{}",
            ]),
            0,
            b"tag-oid\n",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/shim-smoke/m3-s1",
                "--base",
                "milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            b"[{\"number\":17,\"headRefName\":\"pce/shim-smoke/m3-s1\",\"baseRefName\":\"milestone-3\",\"state\":\"MERGED\",\"mergeCommit\":{\"oid\":\"squash-oid\"}}]\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "merge-base",
                "--is-ancestor",
                "squash-oid",
                "fetched-oid",
            ]),
            0,
            b"",
        ),
    ];
    harness
        .materialize_responses(&responses)
        .expect("materialize scripted responses");

    let output = harness
        .run(
            [
                OsString::from("status"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--vision-dir"),
                vision_dir.as_os_str().to_owned(),
            ],
            b"",
        )
        .expect("run status command");
    assert!(
        output.status.success(),
        "status stderr: {}",
        stderr(&output)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("parse status snapshot");

    assert_eq!(at(&snapshot, "/schema_id"), &json!("pce.run-snapshot"));
    assert_eq!(at(&snapshot, "/schema_version"), &json!(1));
    assert_eq!(at(&snapshot, "/repositories/0/repository"), &json!("pce"));
    assert_eq!(
        at(&snapshot, "/repositories/0/fetch/state"),
        &json!("observed")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/fetch/observation_ref"),
        &json!("fetched-oid")
    );
    let fetched_at = at(&snapshot, "/repositories/0/fetch/fetched_at")
        .as_str()
        .expect("fetched_at string");
    assert!(
        is_millisecond_z_timestamp(fetched_at),
        "fetched_at: {fetched_at}"
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/branch/name"),
        &json!("milestone-3")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/branch/state"),
        &json!("present")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/worktree/identity"),
        &json!("pce/shim-smoke/m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/worktree/state"),
        &json!("present")
    );
    assert_eq!(at(&snapshot, "/repositories/0/tag/name"), &json!("v0.1.16"));
    assert_eq!(
        at(&snapshot, "/repositories/0/tag/state"),
        &json!("points-to")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/tag/target"),
        &json!("tag-oid")
    );
    assert_eq!(at(&snapshot, "/steps/0/node"), &json!("m3-s1"));
    assert_eq!(at(&snapshot, "/steps/0/subject/milestone"), &json!(3));
    assert_eq!(at(&snapshot, "/steps/0/subject/step"), &json!(1));
    assert_eq!(
        at(&snapshot, "/steps/0/subject/head_branch"),
        &json!("pce/shim-smoke/m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/subject/integration_branch"),
        &json!("milestone-3")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/subject/pull_request_selector/head"),
        &json!("pce/shim-smoke/m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/subject/pull_request_selector/base"),
        &json!("milestone-3")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/availability"),
        &json!("reachable")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/cardinality"),
        &json!("one-exact-match")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/pull_request/number"),
        &json!(17)
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/pull_request/state/status"),
        &json!("merged")
    );
    assert_eq!(
        at(
            &snapshot,
            "/steps/0/github/pull_request/state/squash_commit_oid"
        ),
        &json!("squash-oid")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/git/availability"),
        &json!("reachable")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/git/state"),
        &json!("squash-commit-reachable")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/git/squash_commit_oid"),
        &json!("squash-oid")
    );
    assert_eq!(at(&snapshot, "/steps/0/merge_status"), &json!("merged"));
    assert_eq!(
        at(&snapshot, "/resume/state"),
        &json!("no-log-visible-candidate")
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/sequence"),
        &json!(1)
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/node"),
        &json!("m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/kind"),
        &json!("repository-contract")
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/evidence"),
        &json!("fixture repository contract")
    );

    let expected = vec![
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "remote".into(),
                "get-url".into(),
                "origin".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                "refs/heads/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/heads/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/tags/v0.1.16^{}".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/shim-smoke/m3-s1",
                "--base",
                "milestone-3",
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
            vec![
                "-C".into(),
                root_arg,
                "merge-base".into(),
                "--is-ancestor".into(),
                "squash-oid".into(),
                "fetched-oid".into(),
            ],
        ),
    ];
    assert_eq!(harness.invocations().expect("parse invocations"), expected);
}

fn response(program: &str, argv: Vec<OsString>, exit_code: i32, stdout: &[u8]) -> ScriptedResponse {
    ScriptedResponse {
        program: program.into(),
        argv,
        exit_code,
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

fn invocation(program: &str, argv: Vec<OsString>) -> Invocation {
    Invocation {
        program: program.into(),
        argv,
    }
}

fn argv<const N: usize>(arguments: [&str; N]) -> Vec<OsString> {
    arguments.into_iter().map(OsString::from).collect()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn at<'a>(document: &'a Value, pointer: &str) -> &'a Value {
    document
        .pointer(pointer)
        .unwrap_or_else(|| panic!("missing JSON pointer {pointer}"))
}

fn is_millisecond_z_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 24
        && [4, 7].into_iter().all(|index| bytes[index] == b'-')
        && bytes[10] == b'T'
        && [13, 16].into_iter().all(|index| bytes[index] == b':')
        && bytes[19] == b'.'
        && bytes[23] == b'Z'
        && bytes
            .iter()
            .enumerate()
            .filter(|(index, _)| ![4, 7, 10, 13, 16, 19, 23].contains(index))
            .all(|(_, byte)| byte.is_ascii_digit())
}
