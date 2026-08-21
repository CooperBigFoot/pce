use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn failing_branch_climbs_recovery_ladder_while_sibling_completes() {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);

    let graph_path = temp.path().join("graph.json");
    let graph = json!({"vision":"recovery","plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"always fails","repositories":["repo"],"criteria":[{"name":"deterministic","input":"repo","observation":"fails with output","command":"printf actual-failure-output; exit 7"}],"depends_on":[]},
        {"id":"X","title":"independent","repositories":["repo"],"criteria":[{"name":"passes","input":"repo","observation":"zero","command":"true"}],"depends_on":[]}
    ]});
    fs::write(&graph_path, serde_json::to_vec(&graph).expect("graph")).expect("write graph");
    let journal = temp.path().join("driver.jsonl");
    let observations = temp.path().join("observations");
    let worker = temp.path().join("worker.sh");
    fs::write(
        &worker,
        format!(
            r#"#!/bin/sh
set -eu
printf '%s|%s\n' "${{PCE_RECOVERY_RUNG:-initial}}" "${{PCE_PACKAGE_BRIEF:-}}" >> '{}'
printf '%s' '{{"outcome":"done"}}' > "$PCE_PACKAGE_OUTCOME"
"#,
            observations.display()
        ),
    )
    .expect("worker");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph_path.to_str().expect("graph path"),
            "--journal",
            journal.to_str().expect("journal path"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker path"),
        ])
        .current_dir(temp.path())
        .output()
        .expect("pce");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    let packages = status["packages"].as_array().expect("packages");
    let state = |id: &str| {
        packages
            .iter()
            .find(|entry| entry[0] == id)
            .expect("package")[1]["state"]
            .as_str()
            .expect("state")
    };
    assert_eq!(state("A"), "parked");
    assert_eq!(state("X"), "complete");
    let recovery = status["recovery"].as_array().expect("recovery budgets");
    assert_eq!(recovery.len(), 2);
    let a_budget = recovery
        .iter()
        .find(|entry| entry[0] == "A")
        .expect("A budget");
    assert_eq!(a_budget[1]["dispatches_remaining"], 0);
    assert_eq!(a_budget[1]["next_rung"], "replan");
    let log = fs::read_to_string(&journal).expect("journal");
    assert!(log.contains("recovery-rung-attempted"));
    assert!(log.contains("\"rung\":\"retry\""));
    assert!(log.contains("\"rung\":\"local-patch\""));
    assert!(log.contains("recovery-parked"));
    assert!(log.contains("actual-failure-output"));
    let observed = fs::read_to_string(&observations).expect("observations");
    assert!(observed.contains("retry|"));
    assert!(observed.contains("local-patch|"));
    assert!(observed.contains("actual-failure-output"));
    assert!(observed.contains("printf actual-failure-output; exit 7"));
}

fn run_failure_case(retry: u32, local_patch: u32) -> usize {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    let graph_path = temp.path().join("graph.json");
    fs::write(&graph_path, serde_json::to_vec(&json!({"vision":"dispatch-proof","plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"red","input":"repo","observation":"red","command":"exit 19"}],"depends_on":[]}
    ]})).expect("graph")).expect("graph write");
    let count = temp.path().join("count");
    let worker = temp.path().join("worker.sh");
    fs::write(&worker, format!("#!/bin/sh\nprintf x >> '{}'\nprintf '%s' '{{\"outcome\":\"done\"}}' > \"$PCE_PACKAGE_OUTCOME\"\n", count.display())).expect("worker");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph_path.to_str().expect("graph"),
            "--journal",
            temp.path().join("journal.jsonl").to_str().expect("journal"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--retry-limit",
            &retry.to_string(),
            "--local-patch-limit",
            &local_patch.to_string(),
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker"),
        ])
        .current_dir(temp.path())
        .output()
        .expect("pce");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(count).expect("count").len()
}

#[test]
fn identical_failure_sequence_changes_actual_dispatches_when_ladder_is_removed() {
    assert_eq!(run_failure_case(1, 1), 3);
    assert_eq!(run_failure_case(0, 0), 1);
}

#[test]
fn second_identical_environment_failure_stops_without_spending_recovery_budget() {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    let graph_path = temp.path().join("graph.json");
    fs::write(&graph_path, serde_json::to_vec(&json!({"vision":"environment","plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"green","input":"repo","observation":"zero","command":"true"}],"depends_on":[]}
    ]})).expect("graph")).expect("graph write");
    let attempts = temp.path().join("attempts");
    let worker = temp.path().join("worker.sh");
    fs::write(
        &worker,
        format!(
            r#"#!/bin/sh
set -eu
printf x >> '{}'
count=$(wc -c < '{}')
if [ "$count" -le 5 ]; then exit 70; fi
printf '%s' '{{"outcome":"done"}}' > "$PCE_PACKAGE_OUTCOME"
"#,
            attempts.display(),
            attempts.display()
        ),
    )
    .expect("worker");
    let journal = temp.path().join("journal.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph_path.to_str().expect("graph"),
            "--journal",
            journal.to_str().expect("journal"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker"),
        ])
        .current_dir(temp.path())
        .output()
        .expect("pce");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(status["packages"][0][1]["state"], "environment-blocked");
    assert_eq!(status["recovery"][0][1]["retry_remaining"], 1);
    assert_eq!(status["recovery"][0][1]["local_patch_remaining"], 1);
    assert_eq!(fs::read_to_string(&attempts).expect("attempts").len(), 2);
    let log = fs::read_to_string(journal).expect("journal");
    assert_eq!(log.matches("worker-environment-failed").count(), 1);
    assert_eq!(log.matches("package-environment-blocked").count(), 1);
    assert!(!log.contains("recovery-rung-attempted"));
}

#[test]
fn crashing_gate_retries_judgment_without_redispatching_or_charging_worker() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "green").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    let base = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repo)
        .output()
        .expect("base");
    let base = String::from_utf8(base.stdout)
        .expect("base utf8")
        .trim()
        .to_owned();

    fs::write(temp.path().join("vision.md"), "# Vision: gate failure\n\n## Goal / Why\n\nJudge green work.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"green\",\"input\":\"repo\",\"observation\":\"zero\"}]}\n```\n").expect("vision");
    let graph_path = temp.path().join("graph.json");
    fs::write(&graph_path, serde_json::to_vec(&json!({"vision":"gate-failure","plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"green","input":"repo","observation":"zero","command":"true"}],"depends_on":[]}
    ]})).expect("graph")).expect("graph write");

    let bin = temp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    let herdr = bin.join("herdr");
    fs::write(&herdr, r#"#!/bin/sh
set -eu
if [ "${1-}" = "--version" ]; then echo "herdr 0.8.2"; exit 0; fi
if [ "$1 $2" = "worktree create" ]; then
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"},"root_pane":{"pane_id":"root-pane","workspace_id":"w1"}}}'
elif [ "$1 $2" = "pane run" ]; then
  (/bin/sh -c "$4") &
  printf '%s\n' '{}'
else
  printf '%s\n' '{}'
fi
"#).expect("herdr");
    let gate = bin.join("prime-agent");
    fs::write(
        &gate,
        r#"#!/bin/sh
cat >/dev/null
if [ -n "${PCE_PACKAGE_OUTCOME-}" ]; then
  printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
  exit 0
fi
exit 1
"#,
    )
    .expect("gate");
    for executable in [&herdr, &gate] {
        let mut permissions = fs::metadata(executable).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(executable, permissions).expect("permissions");
    }
    let journal = temp.path().join("journal.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repo.display()))
        .args(["--gate-failure-limit", "3"])
        .env("HERDR_ENV", "1")
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
        .env(
            "PATH",
            format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH")),
        )
        .env("HOME", temp.path())
        .env("USER", "tester")
        .current_dir(temp.path())
        .output()
        .expect("driver");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    let events = fs::read_to_string(&journal).expect("journal");
    assert_eq!(
        status["packages"][0][1]["state"], "gate-blocked",
        "{events}"
    );
    assert_eq!(status["packages"][0][1]["identical_failures"], 3);
    assert_eq!(status["recovery"][0][1]["retry_remaining"], 1);
    assert_eq!(status["recovery"][0][1]["local_patch_remaining"], 1);

    assert_eq!(events.matches("worker-dispatched").count(), 1);
    assert_eq!(events.matches("gate-dispatched").count(), 3);
    assert_eq!(events.matches("gate-failed").count(), 2);
    assert_eq!(events.matches("package-gate-blocked").count(), 1);
    assert_eq!(
        events
            .matches("gate dispatch stopped without an outcome")
            .count(),
        3
    );
    assert!(!events.contains("package-failed"));
    assert!(!events.contains("package-completed"));
    assert!(!events.contains("recovery-parked"));
    assert!(events.contains("package-gate-1-1"));
    assert!(events.contains("package-gate-1-2"));
    assert!(events.contains("package-gate-1-3"));
    let implementation = Command::new("git")
        .args(["rev-parse", "pce/gate-failure/A/attempt-1"])
        .current_dir(&repo)
        .output()
        .expect("implementation ref");
    assert!(implementation.status.success());
    assert_eq!(
        String::from_utf8(implementation.stdout)
            .expect("head utf8")
            .trim(),
        base
    );
}

#[test]
fn second_identical_worker_blocker_parks_without_exhausting_recovery_budget() {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);

    let graph_path = temp.path().join("graph.json");
    fs::write(
        &graph_path,
        serde_json::to_vec(&json!({
            "vision":"repeated-blocker",
            "plan_version":1,
            "authored_at_ref":"HEAD",
            "packages":[{
                "id":"A",
                "title":"A",
                "repositories":["repo"],
                "criteria":[{"name":"green","input":"repo","observation":"zero","command":"true"}],
                "depends_on":[]
            }]
        }))
        .expect("graph"),
    )
    .expect("graph write");
    let attempts = temp.path().join("attempts");
    let worker = temp.path().join("worker.sh");
    let blocked_by = "HFX_CAMPAIGN_EVIDENCE and HFX_S3_ENV_FILE are unset";
    fs::write(
        &worker,
        format!(
            "#!/bin/sh\nprintf x >> '{}'\nprintf '%s' '{{\"outcome\":\"failed\",\"blocked_by\":\"{blocked_by}\"}}' > \"$PCE_PACKAGE_OUTCOME\"\n",
            attempts.display()
        ),
    )
    .expect("worker");
    let journal = temp.path().join("journal.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph_path.to_str().expect("graph"),
            "--journal",
            journal.to_str().expect("journal"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker"),
        ])
        .current_dir(temp.path())
        .output()
        .expect("pce");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(fs::read_to_string(&attempts).expect("attempts").len(), 2);
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(status["packages"][0][1]["state"], "parked");
    assert_eq!(status["packages"][0][1]["blocked_by"], blocked_by);
    assert_eq!(
        status["packages"][0][1]["reason"],
        "repeated identical worker blocker; package work cannot resolve it"
    );
    assert_eq!(status["recovery"][0][1]["dispatches_remaining"], 1);
    assert_eq!(status["recovery"][0][1]["next_rung"], "local-patch");

    let events = fs::read_to_string(&journal).expect("journal");
    let parked = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event"))
        .find(|event| event["event"] == "recovery-parked")
        .expect("recovery park");
    assert_eq!(parked["blocked_by"], blocked_by);
    assert_eq!(
        parked["reason"],
        "repeated identical worker blocker; package work cannot resolve it"
    );
}

#[test]
fn budget_exhaustion_parks_with_final_worker_blocker_in_status_and_journal() {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);

    let graph_path = temp.path().join("graph.json");
    fs::write(
        &graph_path,
        serde_json::to_vec(&json!({
            "vision":"budget-blocker", "plan_version":1, "authored_at_ref":"HEAD",
            "packages":[{"id":"A","title":"A","repositories":["repo"],
                "criteria":[{"name":"green","input":"repo","observation":"zero","command":"true"}],
                "depends_on":[]}]
        }))
        .expect("graph"),
    )
    .expect("graph write");
    let count = temp.path().join("count");
    let worker = temp.path().join("worker.sh");
    fs::write(&worker, format!(r#"#!/bin/sh
if test -f '{}'; then blocker='final SDK probe: /opt/acme/sdk missing'; else blocker='initial registry timeout'; touch '{}'; fi
printf '{{"outcome":"failed","blocked_by":"%s"}}' "$blocker" > "$PCE_PACKAGE_OUTCOME"
"#, count.display(), count.display())).expect("worker");
    let journal = temp.path().join("journal.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph_path.to_str().expect("graph"),
            "--journal",
            journal.to_str().expect("journal"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--retry-limit",
            "1",
            "--local-patch-limit",
            "0",
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker"),
        ])
        .current_dir(temp.path())
        .output()
        .expect("pce");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    let blocker = "final SDK probe: /opt/acme/sdk missing";
    assert_eq!(status["packages"][0][1]["state"], "parked");
    assert_eq!(status["packages"][0][1]["blocked_by"], blocker);
    assert!(
        status["packages"][0][1]["reason"]
            .as_str()
            .expect("reason")
            .contains("spending exhausted")
    );
    let parked = fs::read_to_string(&journal)
        .expect("journal")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event"))
        .find(|event| event["event"] == "recovery-parked")
        .expect("recovery park");
    assert_eq!(parked["blocked_by"], blocker);
}

#[test]
fn recovery_park_reports_per_criterion_outcomes_across_attempts() {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);

    let counter = temp.path().join("counter");
    let graph_path = temp.path().join("graph.json");
    fs::write(&graph_path, serde_json::to_vec(&json!({
        "vision":"criterion-matrix", "plan_version":1, "authored_at_ref":"HEAD",
        "packages":[{"id":"A","title":"A","repositories":["repo"],"criteria":[
            {"name":"first","input":"attempt","observation":"first attempt","command":format!(r#"test "$(cat {})" = 1"#, counter.display())},
            {"name":"second","input":"attempt","observation":"second attempt","command":format!(r#"test "$(cat {})" = 2"#, counter.display())}
        ],"depends_on":[]}]
    })).expect("graph")).expect("graph write");
    let worker = temp.path().join("worker.sh");
    fs::write(
        &worker,
        format!(
            r#"#!/bin/sh
value=0; test ! -f '{0}' || value=$(cat '{0}')
value=$((value + 1)); printf '%s' "$value" > '{0}'
printf '%s' '{{"outcome":"done"}}' > "$PCE_PACKAGE_OUTCOME"
"#,
            counter.display()
        ),
    )
    .expect("worker");
    let journal = temp.path().join("journal.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph_path.to_str().expect("graph"),
            "--journal",
            journal.to_str().expect("journal"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--retry-limit",
            "1",
            "--local-patch-limit",
            "0",
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker"),
        ])
        .current_dir(temp.path())
        .output()
        .expect("pce");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parked = fs::read_to_string(&journal)
        .expect("journal")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event"))
        .find(|event| event["event"] == "recovery-parked")
        .expect("recovery park");
    assert_eq!(
        parked["criterion_outcomes"],
        json!([
            {"issuance":1,"criteria":[{"name":"first","outcome":"passed"},{"name":"second","outcome":"failed"}]},
            {"issuance":2,"criteria":[{"name":"first","outcome":"failed"},{"name":"second","outcome":"passed"}]}
        ])
    );
}

#[test]
fn attributed_recovery_reset_reopens_exhausted_package_and_refreshes_status() {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);

    let graph_path = temp.path().join("graph.json");
    fs::write(
        &graph_path,
        serde_json::to_vec(&json!({
            "vision":"recovery-reset",
            "plan_version":1,
            "authored_at_ref":"HEAD",
            "packages":[{
                "id":"A",
                "title":"A",
                "repositories":["repo"],
                "criteria":[{"name":"green","input":"repo","observation":"zero","command":"true"}],
                "depends_on":[]
            }]
        }))
        .expect("graph"),
    )
    .expect("graph write");
    let journal = temp.path().join("journal.jsonl");
    let events = [
        json!({"event":"recovery-configured","limits":{"retry_attempts":1,"local_patch_attempts":1,"environment_failures":6,"gate_failures":3}}),
        json!({"event":"worker-dispatched","package":"A","issuance":1}),
        json!({"event":"worker-failed","package":"A","issuance":1,"reason":"first"}),
        json!({"event":"worker-dispatched","package":"A","issuance":2}),
        json!({"event":"worker-failed","package":"A","issuance":2,"reason":"second"}),
        json!({"event":"worker-dispatched","package":"A","issuance":3}),
        json!({"event":"worker-failed","package":"A","issuance":3,"reason":"third"}),
        json!({"event":"recovery-parked","package":"A","reason":"recovery spending exhausted after 3 attributable failures; supply an attributed recovery reset record","attempts":[]}),
    ];
    let journal_text = events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&journal, format!("{journal_text}\n")).expect("journal");
    let reset = temp.path().join("recovery-reset.json");
    fs::write(
        &reset,
        serde_json::to_vec(&json!({
            "schema_version":1,
            "package":"A",
            "reset_by":"operator@example.com",
            "rationale":"the attributed harness defect was repaired"
        }))
        .expect("reset"),
    )
    .expect("reset write");
    let worker = temp.path().join("worker.sh");
    fs::write(
        &worker,
        "#!/bin/sh\nprintf '%s' '{\"outcome\":\"done\"}' > \"$PCE_PACKAGE_OUTCOME\"\n",
    )
    .expect("worker");

    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph_path.to_str().expect("graph path"),
            "--journal",
            journal.to_str().expect("journal path"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--recovery-reset",
            reset.to_str().expect("reset path"),
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker path"),
        ])
        .current_dir(temp.path())
        .output()
        .expect("pce");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(status["packages"][0][1]["state"], "complete");
    assert_eq!(status["recovery"][0][1]["dispatches_remaining"], 2);
    assert_eq!(status["recovery"][0][1]["next_rung"], "retry");
    assert_eq!(status["recovery_spending_resets"][0]["package"], "A");
    assert_eq!(
        status["recovery_spending_resets"][0]["reset_by"],
        "operator@example.com"
    );
    let log = fs::read_to_string(&journal).expect("journal after reset");
    assert!(log.contains("\"event\":\"recovery-spending-reset\""));
    assert!(log.contains("\"package\":\"A\",\"issuance\":4"));
}
