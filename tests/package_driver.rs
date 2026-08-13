use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

fn run(root: &Path, args: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("pce should start")
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git should start");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git output should be UTF-8")
        .trim()
        .to_owned()
}

fn repository(root: &Path, name: &str, value: &str) -> std::path::PathBuf {
    let repo = root.join(name);
    fs::create_dir(&repo).expect("repo directory");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), value).expect("write value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    repo
}

fn graph(path: &Path, repositories: &[&str], criteria: Value) {
    let value = json!({"vision":"driver-test","plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"A","repositories":repositories,"criteria":criteria,"depends_on":[]},
        {"id":"B","title":"B","repositories":[repositories[0]],"criteria":[{"name":"dependent","input":"repo","observation":"zero","command":"true"}],"depends_on":[{"id":"A","kind":"buildability","reason":"A builds B"}]}
    ]});
    fs::write(
        path,
        serde_json::to_vec(&value).expect("graph serialization"),
    )
    .expect("graph write");
}

fn append(path: &Path, value: Value) {
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("journal open");
    writeln!(file, "{}", value).expect("journal append");
}

#[test]
fn criteria_record_command_status_and_output_and_block_dependents() {
    let temp = TempDir::new().expect("tempdir");
    let repo = repository(temp.path(), "repo", "base");
    let graph_path = temp.path().join("graph.json");
    let journal = temp.path().join("driver.jsonl");
    graph(
        &graph_path,
        &["repo"],
        json!([
            {"name":"passes","input":"repo","observation":"zero","command":"printf pass-out; printf pass-err >&2"},
            {"name":"fails","input":"repo","observation":"zero","command":"printf fail-out; printf fail-err >&2; exit 7"}
        ]),
    );
    append(
        &journal,
        json!({"event":"worker-dispatched","package":"A","issuance":1}),
    );
    append(
        &journal,
        json!({"event":"worker-done","package":"A","issuance":1}),
    );
    let before = git(
        &repo,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let output = run(
        temp.path(),
        &[
            "package".into(),
            "criteria-run".into(),
            "--graph".into(),
            graph_path.display().to_string(),
            "--journal".into(),
            journal.display().to_string(),
            "--package".into(),
            "A".into(),
            "--repository".into(),
            format!("repo={}", repo.display()),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        before,
        git(
            &repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
    );
    let events = fs::read_to_string(&journal).expect("journal");
    assert!(events.contains("pass-out"));
    assert!(events.contains("pass-err"));
    assert!(events.contains("fail-out"));
    assert!(events.contains("fail-err"));
    assert!(events.contains("\"code\":7"));
    assert!(events.contains("criteria failed: fails"));
    let status: Value = serde_json::from_slice(&output.stdout).expect("status JSON");
    assert_eq!(status["outcome"], "blocked");
    assert_eq!(status["ready"], json!([]));
}

#[test]
fn coordinated_replay_accepts_amendment_and_rejects_false_findings() {
    let temp = TempDir::new().expect("tempdir");
    let a = repository(temp.path(), "a", "witness");
    let b = repository(temp.path(), "b", "witness");
    let witness_a = git(&a, &["rev-parse", "HEAD"]);
    let witness_b = git(&b, &["rev-parse", "HEAD"]);
    fs::write(a.join("value"), "repair").expect("repair A");
    git(&a, &["commit", "-qam", "repair"]);
    let repair_a = git(&a, &["rev-parse", "HEAD"]);
    fs::write(b.join("value"), "repair").expect("repair B");
    git(&b, &["commit", "-qam", "repair"]);
    let repair_b = git(&b, &["rev-parse", "HEAD"]);
    let graph_path = temp.path().join("graph.json");
    let journal = temp.path().join("driver.jsonl");
    graph(
        &graph_path,
        &["a", "b"],
        json!([{"name":"floor","input":"repos","observation":"zero","command":"true"}]),
    );
    append(
        &journal,
        json!({"event":"worker-dispatched","package":"A","issuance":1}),
    );
    append(
        &journal,
        json!({"event":"worker-done","package":"A","issuance":1}),
    );
    let command =
        "test \"$(cat value)\" = repair && test \"$(cat \"$PCE_WORKTREE_1/value\")\" = repair";
    let outcome = temp.path().join("gate.json");
    fs::write(&outcome, serde_json::to_vec(&json!({"findings":[{"description":"wrong coordinated values","repair":"repair both","proposed_criterion_command":command,"repository_refs":[
        {"repository":"a","witness_ref":witness_a,"repair_ref":repair_a},{"repository":"b","witness_ref":witness_b,"repair_ref":repair_b}
    ]}]})).expect("outcome serialization")).expect("outcome write");
    let base = vec![
        "package".into(),
        "replay-finding".into(),
        "--graph".into(),
        graph_path.display().to_string(),
        "--journal".into(),
        journal.display().to_string(),
        "--package".into(),
        "A".into(),
        "--gate".into(),
        "gate-1".into(),
        "--finding".into(),
        "0".into(),
        "--outcome".into(),
        outcome.display().to_string(),
        "--repository".into(),
        format!("a={}", a.display()),
        "--repository".into(),
        format!("b={}", b.display()),
    ];
    let output = run(temp.path(), &base);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).expect("decision JSON");
    assert_eq!(decision["decision"]["decision"], "accepted");
    let status = run(
        temp.path(),
        &[
            "package".into(),
            "driver-status".into(),
            "--graph".into(),
            graph_path.display().to_string(),
            "--journal".into(),
            journal.display().to_string(),
        ],
    );
    let snapshot: Value = serde_json::from_slice(&status.stdout).expect("snapshot JSON");
    assert_eq!(snapshot["amendments"][0][1]["command"], command);
    assert_eq!(
        git(&a, &["status", "--porcelain=v1", "--untracked-files=all"]),
        ""
    );
    assert_eq!(
        git(&b, &["status", "--porcelain=v1", "--untracked-files=all"]),
        ""
    );

    let false_outcome = temp.path().join("false.json");
    fs::write(&false_outcome, serde_json::to_vec(&json!({"findings":[{"description":"invented","repair":"none","proposed_criterion_command":"true","repository_refs":[{"repository":"a","witness_ref":witness_a,"repair_ref":repair_a}]}]})).expect("false serialization")).expect("false write");
    let mut false_args = base;
    let position = false_args
        .iter()
        .position(|value| value == &outcome.display().to_string())
        .expect("outcome arg");
    false_args[position] = false_outcome.display().to_string();
    let rejected = run(temp.path(), &false_args);
    assert!(
        rejected.status.success(),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    let rejection: Value = serde_json::from_slice(&rejected.stdout).expect("rejection JSON");
    assert_eq!(rejection["decision"]["decision"], "rejected");
    assert_eq!(rejection["decision"]["reason"], "witness-passed");

    let broken_repair = temp.path().join("broken-repair.json");
    fs::write(&broken_repair, serde_json::to_vec(&json!({"findings":[{"description":"unrepaired","repair":"none","proposed_criterion_command":"false","repository_refs":[{"repository":"a","witness_ref":witness_a,"repair_ref":repair_a}]}]})).expect("broken repair serialization")).expect("broken repair write");
    false_args[position] = broken_repair.display().to_string();
    let rejected = run(temp.path(), &false_args);
    assert!(
        rejected.status.success(),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    let rejection: Value = serde_json::from_slice(&rejected.stdout).expect("repair rejection JSON");
    assert_eq!(rejection["decision"]["reason"], "repair-failed");
}

#[test]
fn driver_dispatches_antichain_concurrently_and_parks_only_its_branch() {
    let temp = TempDir::new().expect("tempdir");
    let repo = repository(temp.path(), "repo", "base");
    let graph_path = temp.path().join("graph.json");
    let journal = temp.path().join("driver.jsonl");
    let graph_value = json!({"vision":"concurrency","plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"a","input":"repo","observation":"zero","command":"true"}],"depends_on":[]},
        {"id":"X","title":"X","repositories":["repo"],"criteria":[{"name":"x","input":"repo","observation":"zero","command":"true"}],"depends_on":[]},
        {"id":"B","title":"B","repositories":["repo"],"criteria":[{"name":"b","input":"repo","observation":"zero","command":"true"}],"depends_on":[{"id":"A","kind":"buildability","reason":"A"}]},
        {"id":"Y","title":"Y","repositories":["repo"],"criteria":[{"name":"y","input":"repo","observation":"zero","command":"true"}],"depends_on":[{"id":"X","kind":"buildability","reason":"X"}]}
    ]});
    fs::write(
        &graph_path,
        serde_json::to_vec(&graph_value).expect("graph serialization"),
    )
    .expect("graph write");
    let worker = temp.path().join("worker.sh");
    fs::write(&worker, r#"#!/bin/sh
set -eu
if [ "$PCE_PACKAGE" = X ]; then printf '%s' '{"outcome":"mis-specified","fault":{"kind":"criterion","name":"wrong X"}}' > "$PCE_PACKAGE_OUTCOME"; exit 0; fi
if [ "$PCE_PACKAGE" = A ]; then touch "$PCE_BARRIER/A"; i=0; while [ ! -e "$PCE_BARRIER/X" ]; do i=$((i+1)); [ "$i" -lt 200 ] || exit 9; sleep .01; done; fi
if [ "$PCE_PACKAGE" = X ]; then touch "$PCE_BARRIER/X"; fi
printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
"#).expect("worker write");
    // Use a second script because X must publish its barrier before reporting the graph fault.
    fs::write(&worker, format!(r#"#!/bin/sh
set -eu
mkdir -p '{barrier}'
touch '{barrier}/'$PCE_PACKAGE
if [ "$PCE_PACKAGE" = A ]; then i=0; while [ ! -e '{barrier}/X' ]; do i=$((i+1)); [ "$i" -lt 200 ] || exit 9; sleep .01; done; fi
if [ "$PCE_PACKAGE" = X ]; then printf '%s' '{{"outcome":"mis-specified","fault":{{"kind":"criterion","name":"wrong X"}}}}' > "$PCE_PACKAGE_OUTCOME"; else printf '%s' '{{"outcome":"done"}}' > "$PCE_PACKAGE_OUTCOME"; fi
"#, barrier=temp.path().join("barrier").display())).expect("worker write");
    git(&repo, &["add", "."]); // does not include external script; maintains clean repository
    let output = run(
        temp.path(),
        &[
            "package".into(),
            "driver-run".into(),
            "--graph".into(),
            graph_path.display().to_string(),
            "--journal".into(),
            journal.display().to_string(),
            "--repository".into(),
            format!("repo={}", repo.display()),
            "--".into(),
            "/bin/sh".into(),
            worker.display().to_string(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("snapshot");
    assert_eq!(snapshot["outcome"], "blocked");
    let states = snapshot["packages"].as_array().expect("packages");
    let state = |name: &str| {
        states
            .iter()
            .find(|entry| entry[0] == name)
            .expect("package state")[1]["state"]
            .as_str()
            .expect("state")
    };
    assert_eq!(state("A"), "complete");
    assert_eq!(state("B"), "complete");
    assert_eq!(state("X"), "parked");
    assert_eq!(state("Y"), "pending");
    let log = fs::read_to_string(&journal).expect("journal");
    assert_eq!(
        log.matches("\"package\":\"X\",\"issuance\"").count(),
        2,
        "one dispatch plus one park event"
    );
}

#[test]
fn restarted_driver_collects_existing_outcome_without_redispatch() {
    let temp = TempDir::new().expect("tempdir");
    let repo = repository(temp.path(), "repo", "base");
    let graph_path = temp.path().join("graph.json");
    let journal = temp.path().join("driver.jsonl");
    graph(
        &graph_path,
        &["repo"],
        json!([{"name":"floor","input":"repo","observation":"zero","command":"true"}]),
    );
    append(
        &journal,
        json!({"event":"worker-dispatched","package":"A","issuance":1}),
    );
    let outcome = temp.path().join("package-outcomes/A/1.json");
    fs::create_dir_all(outcome.parent().expect("outcome parent")).expect("outcome dir");
    fs::write(&outcome, r#"{"outcome":"done"}"#).expect("outcome");
    let count = temp.path().join("count");
    let worker = temp.path().join("worker.sh");
    fs::write(&worker, format!("#!/bin/sh\nprintf x >> '{}'\nprintf '%s' '{{\"outcome\":\"done\"}}' > \"$PCE_PACKAGE_OUTCOME\"\n", count.display())).expect("worker");
    let output = run(
        temp.path(),
        &[
            "package".into(),
            "driver-run".into(),
            "--graph".into(),
            graph_path.display().to_string(),
            "--journal".into(),
            journal.display().to_string(),
            "--repository".into(),
            format!("repo={}", repo.display()),
            "--".into(),
            "/bin/sh".into(),
            worker.display().to_string(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("snapshot");
    assert_eq!(snapshot["outcome"], "finished");
    assert_eq!(fs::read_to_string(&count).expect("only B dispatched"), "x");
    let journal_text = fs::read_to_string(&journal).expect("journal");
    assert_eq!(
        journal_text
            .lines()
            .filter(|line| line.contains("\"event\":\"worker-dispatched\"")
                && line.contains("\"package\":\"A\""))
            .count(),
        1
    );
}

#[test]
fn preparation_runs_once_per_repository_and_failure_is_not_a_criterion_failure() {
    let temp = TempDir::new().expect("tempdir");
    let repo = repository(temp.path(), "repo", "base");
    let graph_path = temp.path().join("graph.json");
    let journal = temp.path().join("driver.jsonl");
    graph(
        &graph_path,
        &["repo"],
        json!([{"name":"must-not-run","input":"repo","observation":"zero","command":"touch criterion-ran; exit 8"}]),
    );
    append(
        &journal,
        json!({"event":"worker-dispatched","package":"A","issuance":1}),
    );
    append(
        &journal,
        json!({"event":"worker-done","package":"A","issuance":1}),
    );
    let source_before = git(
        &repo,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let output = run(
        temp.path(),
        &[
            "package".into(),
            "criteria-run".into(),
            "--graph".into(),
            graph_path.display().to_string(),
            "--journal".into(),
            journal.display().to_string(),
            "--package".into(),
            "A".into(),
            "--repository".into(),
            format!("repo={}", repo.display()),
            "--prepare".into(),
            "repo=printf prepared > prepared-marker; printf prep-out; printf prep-err >&2; exit 6"
                .into(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        source_before,
        git(
            &repo,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
    );
    let events = fs::read_to_string(&journal).expect("journal");
    assert!(events.contains("environment-preparation-executed"));
    assert!(events.contains("repo=printf prepared") || events.contains("printf prepared"));
    assert!(events.contains("prep-out"));
    assert!(events.contains("prep-err"));
    assert!(events.contains("\"code\":6"));
    assert!(!events.contains("criterion-executed"));
    assert!(!events.contains("criteria failed"));
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("snapshot");
    let state = &snapshot["packages"][0][1];
    assert_eq!(state["state"], "environment-preparation-failed");
    assert_eq!(state["repository"], "repo");
}

#[test]
fn coordinated_replay_prepares_every_repository_in_both_states_before_replay() {
    let temp = TempDir::new().expect("tempdir");
    let a = repository(temp.path(), "a", "witness");
    let b = repository(temp.path(), "b", "witness");
    let wa = git(&a, &["rev-parse", "HEAD"]);
    let wb = git(&b, &["rev-parse", "HEAD"]);
    fs::write(a.join("value"), "repair").expect("repair a");
    git(&a, &["commit", "-qam", "repair"]);
    let ra = git(&a, &["rev-parse", "HEAD"]);
    fs::write(b.join("value"), "repair").expect("repair b");
    git(&b, &["commit", "-qam", "repair"]);
    let rb = git(&b, &["rev-parse", "HEAD"]);
    let graph_path = temp.path().join("graph.json");
    let journal = temp.path().join("driver.jsonl");
    graph(
        &graph_path,
        &["a", "b"],
        json!([{"name":"floor","input":"repo","observation":"zero","command":"true"}]),
    );
    append(
        &journal,
        json!({"event":"worker-dispatched","package":"A","issuance":1}),
    );
    append(
        &journal,
        json!({"event":"worker-done","package":"A","issuance":1}),
    );
    let outcome = temp.path().join("gate.json");
    fs::write(&outcome,serde_json::to_vec(&json!({"findings":[{"description":"values","repair":"both","proposed_criterion_command":"test -e prepared-a && test -e \"$PCE_WORKTREE_1/prepared-b\" && test \"$(cat value)\" = repair","repository_refs":[{"repository":"a","witness_ref":wa,"repair_ref":ra},{"repository":"b","witness_ref":wb,"repair_ref":rb}]}]})).expect("json")).expect("outcome");
    let output = run(
        temp.path(),
        &[
            "package".into(),
            "replay-finding".into(),
            "--graph".into(),
            graph_path.display().to_string(),
            "--journal".into(),
            journal.display().to_string(),
            "--package".into(),
            "A".into(),
            "--gate".into(),
            "g".into(),
            "--finding".into(),
            "0".into(),
            "--outcome".into(),
            outcome.display().to_string(),
            "--repository".into(),
            format!("a={}", a.display()),
            "--prepare".into(),
            "a=touch prepared-a".into(),
            "--repository".into(),
            format!("b={}", b.display()),
            "--prepare".into(),
            "b=touch prepared-b".into(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let decision: Value = serde_json::from_slice(&output.stdout).expect("decision");
    assert_eq!(decision["decision"]["decision"], "accepted");
    let events = fs::read_to_string(&journal).expect("journal");
    assert_eq!(
        events.matches("environment-preparation-executed").count(),
        4
    );
    assert!(events.contains("\"materialization\":\"witness\""));
    assert!(events.contains("\"materialization\":\"repair\""));
}

#[test]
fn replay_preparation_failure_records_not_judged_and_executes_no_proposed_command() {
    let temp = TempDir::new().expect("tempdir");
    let repo = repository(temp.path(), "repo", "witness");
    let witness = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(repo.join("value"), "repair").expect("repair");
    git(&repo, &["commit", "-qam", "repair"]);
    let repair = git(&repo, &["rev-parse", "HEAD"]);
    let graph_path = temp.path().join("graph.json");
    let journal = temp.path().join("driver.jsonl");
    graph(
        &graph_path,
        &["repo"],
        json!([{"name":"floor","input":"repo","observation":"zero","command":"true"}]),
    );
    append(
        &journal,
        json!({"event":"worker-dispatched","package":"A","issuance":1}),
    );
    append(
        &journal,
        json!({"event":"worker-done","package":"A","issuance":1}),
    );
    let sentinel = temp.path().join("proposed-ran");
    let outcome = temp.path().join("gate.json");
    fs::write(&outcome,serde_json::to_vec(&json!({"findings":[{"description":"d","repair":"r","proposed_criterion_command":format!("touch {}",sentinel.display()),"repository_refs":[{"repository":"repo","witness_ref":witness,"repair_ref":repair}]}]})).expect("json")).expect("outcome");
    let output = run(
        temp.path(),
        &[
            "package".into(),
            "replay-finding".into(),
            "--graph".into(),
            graph_path.display().to_string(),
            "--journal".into(),
            journal.display().to_string(),
            "--package".into(),
            "A".into(),
            "--gate".into(),
            "g".into(),
            "--finding".into(),
            "0".into(),
            "--outcome".into(),
            outcome.display().to_string(),
            "--repository".into(),
            format!("repo={}", repo.display()),
            "--prepare".into(),
            "repo=test \"$(cat value)\" = witness".into(),
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!sentinel.exists());
    let result: Value = serde_json::from_slice(&output.stdout).expect("result");
    assert_eq!(result["decision"], "not-judged");
    let events = fs::read_to_string(&journal).expect("journal");
    assert!(events.contains("environment-preparation-executed"));
    assert!(events.contains("\"materialization\":\"repair\""));
    assert!(!events.contains("finding-replayed"));
}
