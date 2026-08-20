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
fn live_herdr_package_dispatch_records_sleep_then_exit_three() {
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
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            directory.path().join("worktrees"),
        )
        .args(["dispatch", "package", "--file"])
        .arg(&log)
        .args(["--vision-dir"])
        .arg(directory.path())
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
        .args(["--env", "CAMPAIGN_PATH=direct-secret-value"])
        .args([
            "--",
            "/bin/sh",
            "-c",
            r#"test -n "$CAMPAIGN_PATH"; sleep 2; exit 3"#,
        ])
        .output()
        .expect("pce package dispatch");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Value = serde_json::from_slice(&output.stdout).expect("dispatch JSON");
    assert_eq!(response["issuance_sequence"], 1);
    assert!(
        response["herdr_version"]
            .as_str()
            .is_some_and(|version| version.starts_with("herdr 0.8."))
    );
    assert!(response["pane_run"]["result"].is_object());
    assert_eq!(
        response["dispatch_identity"]["pane_id"],
        response["worktrees"][0]["response"]["result"]["root_pane"]["pane_id"]
    );
    assert!(response["dispatch_identity"]["process"].is_object());
    let result_path =
        std::path::PathBuf::from(response["result_path"].as_str().expect("result path"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !result_path.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(
        result_path.exists(),
        "worker result did not appear before deadline"
    );
    let result: Value = serde_json::from_slice(&fs::read(&result_path).expect("result bytes"))
        .expect("result JSON");
    assert_eq!(result["exit_status"]["kind"], "exited");
    assert_eq!(result["exit_status"]["code"], 3);

    let collection = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["dispatch", "package-completions", "--file"])
        .arg(&log)
        .args(["--vision-dir"])
        .arg(directory.path())
        .output()
        .expect("collect completion");
    assert!(
        collection.status.success(),
        "{}",
        String::from_utf8_lossy(&collection.stderr)
    );
    let report: Value = serde_json::from_slice(&collection.stdout).expect("collection JSON");
    assert_eq!(report["appended"], serde_json::json!([1]));
    assert_eq!(report["packages"][0]["state"], "finished");
    assert_eq!(report["packages"][0]["exit_status"]["code"], 3);
    eprintln!(
        "PCE_LIVE_EVIDENCE pane_run={} result={} collection={}",
        response["pane_run"], result, report
    );
    let records = fs::read_to_string(&log).expect("event log");
    assert_eq!(records.lines().count(), 2);
    assert!(records.contains(r#""kind":"dispatch-completion""#));
    assert!(records.contains(r#""code":3"#));

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

#[test]
fn live_herdr_retry_uses_distinct_attempt_identity_and_composed_base() {
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
    fs::write(repository.join("base"), b"authored base").expect("base fixture");
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["add", "base"])
            .status()
            .expect("git add")
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["commit", "-qm", "base"])
            .status()
            .expect("git commit")
            .success()
    );
    let base = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("base oid")
            .stdout,
    )
    .expect("UTF-8 base")
    .trim()
    .to_owned();
    fs::write(repository.join("dependency"), b"completed dependency").expect("dependency fixture");
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["add", "dependency"])
            .status()
            .expect("git add dependency")
            .success()
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["commit", "-qm", "composed dependency base"])
            .status()
            .expect("git commit dependency")
            .success()
    );
    let composed_base = String::from_utf8(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("composed base oid")
            .stdout,
    )
    .expect("UTF-8 composed base")
    .trim()
    .to_owned();
    assert_eq!(
        String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&repository)
                .args(["rev-parse", "HEAD^"])
                .output()
                .expect("dependency parent")
                .stdout
        )
        .expect("UTF-8 dependency parent")
        .trim(),
        base
    );
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let graph_path = directory.path().join("graph.json");
    fs::write(&graph_path, serde_json::to_vec(&json!({
        "vision": format!("retry-{nonce}"), "plan_version": 1, "authored_at_ref": composed_base,
        "packages": [{"id":"WP11","title":"retry dispatch","repositories":["repo"],
            "criteria":[{"name":"true","input":"none","observation":"zero","command":"true"}],"depends_on":[]}]
    })).expect("graph JSON")).expect("graph");
    let log = directory.path().join("events.jsonl");
    let mut responses = Vec::new();
    let mut commits = Vec::new();
    for attempt in 1..=2 {
        let artifact = directory.path().join(format!("artifact-{attempt}"));
        let script = format!(
            "test -e dependency; test ! -e attempt-1; printf {attempt} > attempt-{attempt}; git add attempt-{attempt}; git commit -qm attempt-{attempt}; : > {}",
            artifact.display()
        );
        let output = Command::new(env!("CARGO_BIN_EXE_pce"))
            .env(
                "PCE_WORK_PACKAGE_WORKTREE_ROOT",
                directory.path().join("worktrees"),
            )
            .args(["dispatch", "package", "--file"])
            .arg(&log)
            .args(["--vision-dir"])
            .arg(directory.path())
            .args(["--graph"])
            .arg(&graph_path)
            .args(["--package", "WP11", "--required-artifact"])
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
            .args(["--", "/bin/sh", "-c", &script])
            .output()
            .expect("dispatch");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let response: Value = serde_json::from_slice(&output.stdout).expect("dispatch JSON");
        let result_path =
            std::path::PathBuf::from(response["result_path"].as_str().expect("result path"));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !result_path.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(result_path.exists(), "attempt {attempt} result missing");
        let worktree = std::path::PathBuf::from(
            response["worktrees"][0]["path"]
                .as_str()
                .expect("worktree path"),
        );
        let commit = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&worktree)
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("attempt oid")
                .stdout,
        )
        .expect("UTF-8 oid")
        .trim()
        .to_owned();
        commits.push(commit);
        responses.push(response);
    }
    assert_ne!(responses[0]["agent_name"], responses[1]["agent_name"]);
    assert_ne!(
        responses[0]["worktrees"][0]["path"],
        responses[1]["worktrees"][0]["path"]
    );
    let branch = |response: &Value| {
        let argv = response["worktrees"][0]["response"]["result"]["branch"].as_str();
        argv.map(str::to_owned).unwrap_or_else(|| {
            let path = response["worktrees"][0]["path"]
                .as_str()
                .expect("worktree path");
            String::from_utf8(
                Command::new("git")
                    .arg("-C")
                    .arg(path)
                    .args(["branch", "--show-current"])
                    .output()
                    .expect("branch")
                    .stdout,
            )
            .expect("UTF-8 branch")
            .trim()
            .to_owned()
        })
    };
    let branches = responses.iter().map(branch).collect::<Vec<_>>();
    assert_ne!(branches[0], branches[1]);
    assert_eq!(
        String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(
                    responses[1]["worktrees"][0]["path"]
                        .as_str()
                        .expect("second path")
                )
                .args(["rev-parse", "HEAD^"])
                .output()
                .expect("second parent")
                .stdout
        )
        .expect("UTF-8 parent")
        .trim(),
        composed_base
    );
    assert_eq!(
        String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&repository)
                .args(["rev-parse", &branches[0]])
                .output()
                .expect("first branch")
                .stdout
        )
        .expect("UTF-8 branch oid")
        .trim(),
        commits[0]
    );

    for response in &responses {
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
}
