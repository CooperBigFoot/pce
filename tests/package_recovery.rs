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
fn repeated_environment_failures_do_not_spend_recovery_budget() {
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
    assert_eq!(status["packages"][0][1]["state"], "complete");
    assert_eq!(status["recovery"][0][1]["retry_remaining"], 1);
    assert_eq!(status["recovery"][0][1]["local_patch_remaining"], 1);
    assert_eq!(fs::read_to_string(&attempts).expect("attempts").len(), 6);
    let log = fs::read_to_string(journal).expect("journal");
    assert_eq!(log.matches("worker-environment-failed").count(), 5);
    assert!(!log.contains("recovery-rung-attempted"));
}
