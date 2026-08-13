use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("permissions");
}

#[test]
fn default_driver_composes_dispatch_and_waits_for_durable_result() {
    let temp = tempdir().expect("tempdir");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("repository");
    for args in [
        ["init"].as_slice(),
        ["config", "user.email", "test@example.com"].as_slice(),
        ["config", "user.name", "Test"].as_slice(),
    ] {
        assert!(
            Command::new("git")
                .current_dir(&repository)
                .args(args)
                .status()
                .expect("git")
                .success()
        );
    }
    fs::write(repository.join("seed"), "seed\n").expect("seed");
    assert!(
        Command::new("git")
            .current_dir(&repository)
            .args(["add", "."])
            .status()
            .expect("add")
            .success()
    );
    assert!(
        Command::new("git")
            .current_dir(&repository)
            .args(["commit", "-m", "seed"])
            .status()
            .expect("commit")
            .success()
    );
    let vision_name = format!("dispatch-{}", std::process::id());
    let vision = temp.path().join("vision.md");
    fs::write(&vision, "# Vision: dispatch\n\n## Goal / Why\n\nCreate known.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"known\",\"input\":\"file\",\"observation\":\"known\"}]}\n```\n").expect("vision");
    let graph = temp.path().join("graph.json");
    fs::write(&graph, format!(r#"{{"vision":"{vision_name}","plan_version":1,"authored_at_ref":"HEAD","packages":[{{"id":"A","title":"A","repositories":["repo"],"criteria":[{{"name":"known","input":"repo","observation":"known","command":"test \"$(cat known.txt)\" = known"}}],"depends_on":[]}}]}}"#)).expect("graph");
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    executable(
        &bin.join("herdr"),
        r#"#!/bin/sh
set -eu
if [ "$1 $2" = "worktree create" ]; then
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"}}}'
else
  shift 2; agent_cwd=
  while [ "$1" != "--" ]; do if [ "$1" = "--cwd" ]; then agent_cwd=$2; shift 2; else shift; fi; done; shift
  (cd "$agent_cwd" && "$@") &
  printf '%s\n' '{}'
fi
"#,
    );
    executable(
        &bin.join("prime-agent"),
        r#"#!/bin/sh
set -eu
cat >/dev/null
sleep 0.35
if [ -n "${PCE_PACKAGE_OUTCOME-}" ]; then
  printf 'known\n' > known.txt
  git add known.txt; git commit -m implementation >/dev/null
  printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
else
  printf '%s' '{"findings":[]}' > "$PCE_PACKAGE_GATE_OUTCOME"
fi
"#,
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph)
        .args(["--journal"])
        .arg(temp.path().join("driver.jsonl"))
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .env("HERDR_ENV", "1")
        .env("PATH", path)
        .env("HOME", temp.path())
        .env("USER", "tester")
        .output()
        .expect("driver");
    assert!(
        output.status.success(),
        "{}
{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(temp.path().join("driver.jsonl")).unwrap_or_default()
    );
    let stdout: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout JSON");
    assert_eq!(stdout["outcome"], "finished");
    let journal = fs::read_to_string(temp.path().join("driver.jsonl")).expect("journal");
    assert_eq!(journal.matches("worker-dispatched").count(), 1);
    assert!(!journal.contains("driver-no-findings"));
    assert!(temp.path().join(".pce/package-results/A/1.json").is_file());
    assert!(temp.path().join(".pce/package-results/A/3.json").is_file());
}

#[test]
fn deliberate_wait_bound_leaves_issuance_running_and_unaccounted() {
    let temp = tempdir().expect("tempdir");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("repository");
    for args in [
        ["init"].as_slice(),
        ["config", "user.email", "test@example.com"].as_slice(),
        ["config", "user.name", "Test"].as_slice(),
    ] {
        assert!(
            Command::new("git")
                .current_dir(&repository)
                .args(args)
                .status()
                .expect("git")
                .success()
        );
    }
    fs::write(repository.join("seed"), "seed\n").expect("seed");
    assert!(
        Command::new("git")
            .current_dir(&repository)
            .args(["add", "."])
            .status()
            .expect("add")
            .success()
    );
    assert!(
        Command::new("git")
            .current_dir(&repository)
            .args(["commit", "-m", "seed"])
            .status()
            .expect("commit")
            .success()
    );
    fs::write(temp.path().join("vision.md"), "# Vision: timeout\n\n## Goal / Why\n\nWait honestly.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"floor\",\"input\":\"repo\",\"observation\":\"true\"}]}\n```\n").expect("vision");
    let graph = temp.path().join("graph.json");
    fs::write(&graph, format!(r#"{{"vision":"timeout-{}","plan_version":1,"authored_at_ref":"HEAD","packages":[{{"id":"A","title":"A","repositories":["repo"],"criteria":[{{"name":"floor","input":"repo","observation":"true","command":"true"}}],"depends_on":[]}}]}}"#, std::process::id())).expect("graph");
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    executable(
        &bin.join("herdr"),
        r#"#!/bin/sh
set -eu
if [ "$1 $2" = "worktree create" ]; then
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"}}}'
else
  printf '%s\n' '{}'
fi
"#,
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let journal = temp.path().join("driver.jsonl");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args(["--wait-timeout-ms", "100"])
        .env("HERDR_ENV", "1")
        .env("PATH", path)
        .env("HOME", temp.path())
        .env("USER", "tester")
        .output()
        .expect("driver");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&output.stdout).expect("snapshot");
    assert_eq!(snapshot["outcome"], "running");
    let events = fs::read_to_string(&journal).expect("journal");
    assert!(events.contains("driver-stopped-waiting"));
    assert!(!events.contains("worker-failed"));
    assert!(!events.contains("worker-environment-failed"));
    let dispatch =
        fs::read_to_string(temp.path().join(".pce/package-dispatch.jsonl")).expect("dispatch");
    assert_eq!(dispatch.matches("\"kind\":\"dispatch\"").count(), 1);
    assert!(!dispatch.contains("dispatch-completion"));
}
