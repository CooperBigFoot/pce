use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tempfile::tempdir;

fn herdr_available() -> bool {
    std::env::var("HERDR_ENV").as_deref() == Ok("1")
        && Command::new("herdr")
            .args(["status", "server"])
            .status()
            .is_ok_and(|status| status.success())
}

#[test]
fn live_herdr_package_dispatch_spawns_true_and_records_issuance() {
    if !herdr_available() {
        eprintln!("PCE_TEST_SKIP: live Herdr session unavailable");
        return;
    }
    let directory = tempdir().expect("temporary directory");
    let repository = directory.path().join("repo");
    fs::create_dir(&repository).expect("repository directory");
    for arguments in [
        vec!["init", "-q"],
        vec!["config", "user.email", "pce@example.invalid"],
        vec!["config", "user.name", "pce"],
    ] {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(&repository)
                .args(arguments)
                .status()
                .expect("git")
                .success()
        );
    }
    fs::write(repository.join("file"), b"fixture").expect("fixture");
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["add", "file"])
            .status()
            .expect("git add")
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["commit", "-qm", "fixture"])
            .status()
            .expect("git commit")
            .success()
    );
    let head = Command::new("git")
        .arg("-C")
        .arg(&repository)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("head");
    let head = String::from_utf8(head.stdout)
        .expect("UTF-8 head")
        .trim()
        .to_owned();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let graph_path = directory.path().join("graph.json");
    fs::write(&graph_path, serde_json::to_vec(&json!({
        "vision": format!("live-{nonce}"), "plan_version": 1, "authored_at_ref": head,
        "packages": [{"id":"WP4","title":"live spawn","repositories":["repo"],
            "criteria":[{"name":"true","input":"none","observation":"zero","command":"true"}],"depends_on":[]}]
    })).expect("graph JSON")).expect("graph");
    let log = directory.path().join("events.jsonl");
    let artifact = directory.path().join("result");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["dispatch", "package", "--file"])
        .arg(&log)
        .args(["--graph"])
        .arg(&graph_path)
        .args(["--package", "WP4", "--required-artifact"])
        .arg(&artifact)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args(["--env"])
        .arg(format!(
            "PATH={}",
            std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned())
        ))
        .args(["--env"])
        .arg(format!(
            "HOME={}",
            std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_owned())
        ))
        .args(["--env"])
        .arg(format!(
            "USER={}",
            std::env::var("USER").unwrap_or_else(|_| "worker".to_owned())
        ))
        .args(["--", "/usr/bin/true"])
        .output()
        .expect("pce package dispatch");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Value = serde_json::from_slice(&output.stdout).expect("dispatch JSON");
    assert_eq!(response["issuance_sequence"], 1);
    assert_eq!(response["agent_start"]["result"]["type"], "agent_started");
    let records = fs::read_to_string(&log).expect("event log");
    assert_eq!(records.lines().count(), 1);
    assert!(records.contains(r#""kind":"dispatch""#));

    if let Some(workspace) =
        response["worktrees"][0]["response"]["result"]["workspace"]["workspace_id"].as_str()
    {
        let _ = Command::new("herdr")
            .args([
                "worktree",
                "remove",
                "--workspace",
                workspace,
                "--force",
                "--json",
            ])
            .status();
    }
}
