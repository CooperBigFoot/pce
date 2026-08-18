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
  printf '%s\t%s\n' "$path" "$branch" >> "$HOME/herdr-worktrees"
  count=1; [ ! -f "$HOME/herdr-worktree-count" ] || count=$(( $(cat "$HOME/herdr-worktree-count") + 1 ))
  printf '%s' "$count" > "$HOME/herdr-worktree-count"
  printf '{"result":{"workspace":{"workspace_id":"owned-workspace-%s"},"tab":{"tab_id":"owned-workspace-%s:t1"},"root_pane":{"pane_id":"owned-pane-%s","workspace_id":"owned-workspace-%s"}}}\n' "$count" "$count" "$count" "$count"
elif [ "$1 $2" = "workspace close" ]; then
  [ "$3" != "unrelated-workspace" ]
  printf '%s\n' "$3" >> "$HOME/herdr-closed-panes"
  if [ "$3" = "owned-workspace-2" ]; then echo 'simulated close refusal' >&2; exit 71; fi
  printf '%s\n' '{"result":{"closed":true}}'
else
  shift 2; agent_cwd=
  while [ "$1" != "--" ]; do if [ "$1" = "--cwd" ]; then agent_cwd=$2; shift 2; else shift; fi; done; shift
  (cd "$agent_cwd" && "$@") &
  printf '%s\n' '{}'
fi"#,
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
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
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
    let worktrees = fs::read_to_string(temp.path().join("herdr-worktrees")).expect("worktree log");
    let first = worktrees.lines().next().expect("implementation worktree");
    let (path, branch) = first.split_once('\t').expect("path and branch");
    assert!(
        !Path::new(path).exists(),
        "completed clean implementation worktree remains"
    );
    assert!(
        !Path::new(path).parent().expect("attempt root").exists(),
        "empty completed attempt root remains"
    );
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(&repository)
            .args(["rev-parse", "--verify", branch])
            .status()
            .expect("branch resolves")
            .success()
    );
    assert_eq!(
        fs::read_dir(temp.path().join("worktrees"))
            .expect("worktree root")
            .count(),
        0,
        "completed implementation and gate attempt roots remain"
    );
    let closed = fs::read_to_string(temp.path().join("herdr-closed-panes"))
        .unwrap_or_else(|error| panic!("closed pane log: {error}; journal: {journal}"));
    assert_eq!(
        closed.lines().collect::<Vec<_>>(),
        ["owned-workspace-1", "owned-workspace-2"]
    );
    assert!(!closed.contains("unrelated-workspace"));
    assert!(journal.contains("owned-pane-1"));
    assert!(journal.contains("owned-pane-2"));
    assert!(journal.contains("dispatch-pane-cleanup"));
    assert!(journal.contains("simulated close refusal"));
    assert!(journal.contains("\"outcome\":\"failed\""));
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
if [ -e "$HOME/herdr-unreachable" ]; then
  echo 'herdr socket unreachable' >&2
  exit 70
fi
if [ "$1 $2" = "worktree create" ]; then
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"},"root_pane":{"pane_id":"root-pane","workspace_id":"w1"}}}'
elif [ "$1 $2" = "agent start" ]; then
  name=$3; printf '%s' "$name" > "$HOME/agent-name"
  printf '{"result":{"type":"agent_started","agent":{"name":"%s","pane_id":"worker-pane","workspace_id":"w1"}}}\n' "$name"
elif [ "$1 $2" = "agent get" ]; then
  if [ -e "$HOME/herdr-unreachable" ]; then echo 'herdr socket unreachable' >&2; exit 70; fi
  name=$(cat "$HOME/agent-name")
  printf '{"result":{"type":"agent_info","agent":{"name":"%s","pane_id":"worker-pane","workspace_id":"w1"}}}\n' "$name"
elif [ "$1 $2" = "pane process-info" ]; then
  if [ -e "$HOME/herdr-unreachable" ] && [ ! -e "$HOME/first-inconclusive-read" ]; then
    : > "$HOME/first-inconclusive-read"; echo 'herdr socket unreachable' >&2; exit 70
  fi
  if [ -e "$HOME/herdr-unreachable" ]; then
    printf '%s\n' '{"result":{"process_info":{"shell_pid":41,"foreground_processes":[]}}}' 
  else
    printf '{"result":{"process_info":{"shell_pid":%s,"foreground_processes":[{"pid":%s,"name":"worker","argv":["worker"]}]}}}\n' "$$" "$$"
  fi
else
  printf '%s\n' '{"result":{"type":"ok"}}'
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
        .args(["--environment-failure-limit", "2"])
        .env("HERDR_ENV", "1")
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
        .env("PATH", &path)
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
    assert!(events.contains("dispatch-worker-identified"));
    assert!(events.contains("worker-pane"));
    assert!(events.contains("root-pane"));
    assert!(!events.contains("worker-failed"));
    assert!(!events.contains("worker-environment-failed"));
    let dispatch =
        fs::read_to_string(temp.path().join(".pce/package-dispatch.jsonl")).expect("dispatch");
    assert_eq!(dispatch.matches("\"kind\":\"dispatch\"").count(), 1);
    assert!(!dispatch.contains("dispatch-completion"));

    let live_restart = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args(["--wait-timeout-ms", "100"])
        .args(["--environment-failure-limit", "2"])
        .env("HERDR_ENV", "1")
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
        .env("PATH", &path)
        .env("HOME", temp.path())
        .env("USER", "tester")
        .output()
        .expect("restarted live driver");
    assert!(
        live_restart.status.success(),
        "{}",
        String::from_utf8_lossy(&live_restart.stderr)
    );
    let after_live_restart = fs::read_to_string(&journal).expect("journal after restart");
    assert_eq!(after_live_restart.matches("worker-dispatched").count(), 1);
    assert_eq!(
        after_live_restart.matches("driver-stopped-waiting").count(),
        2
    );
    assert!(!after_live_restart.contains("worker-environment-failed"));

    fs::write(temp.path().join("herdr-unreachable"), "unreachable").expect("marker");
    let inconclusive_restart = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args(["--wait-timeout-ms", "100"])
        .args(["--environment-failure-limit", "2"])
        .env("HERDR_ENV", "1")
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
        .env("PATH", &path)
        .env("HOME", temp.path())
        .env("USER", "tester")
        .output()
        .expect("inconclusive restart");
    assert!(
        inconclusive_restart.status.success(),
        "{}",
        String::from_utf8_lossy(&inconclusive_restart.stderr)
    );
    let repaired = fs::read_to_string(&journal).expect("repaired journal");
    assert!(repaired.contains("worker environment liveness evidence was inconclusive"));
    assert!(repaired.contains("dispatch-environment-observed"));
    assert_eq!(repaired.matches("worker-dispatched").count(), 2);
    let successor_brief = fs::read_to_string(temp.path().join(".pce/package-briefs/A/2.md"))
        .expect("successor brief");
    assert!(successor_brief.contains(
        "An earlier attempt was killed by the environment before finishing; nothing it produced was judged"
    ));
    assert!(!successor_brief.contains("attempt-1"));
    let repaired_dispatch =
        fs::read_to_string(temp.path().join(".pce/package-dispatch.jsonl")).expect("dispatch");
    assert!(repaired_dispatch.contains("reconciled-dead"));
}

#[test]
fn synchronous_herdr_refusal_is_recorded_and_restart_redispatches() {
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
    fs::write(
        repository.join("seed"),
        "seed
",
    )
    .expect("seed");
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
    fs::write(
        temp.path().join("vision.md"),
        "# Vision: spawn refusal

## Goal / Why

Prove restart.

## Acceptance criteria (vision-level \"done\")

```json
{\"criteria\":[{\"name\":\"known\",\"input\":\"file\",\"observation\":\"known\"}]}
```
",
    )
    .expect("vision");
    let graph = temp.path().join("graph.json");
    fs::write(&graph, format!(r#"{{"vision":"spawn-refusal-{}","plan_version":1,"authored_at_ref":"HEAD","packages":[{{"id":"A","title":"A","repositories":["repo"],"criteria":[{{"name":"known","input":"repo","observation":"known","command":"test \"$(cat known.txt)\" = known"}}],"depends_on":[]}}]}}"#, std::process::id())).expect("graph");
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    executable(
        &bin.join("herdr"),
        r#"#!/bin/sh
set -eu
marker="$HOME/herdr-refused-once"
if [ "$1 $2" = "worktree create" ]; then
  if [ ! -e "$marker" ]; then : > "$marker"; echo refused >&2; exit 73; fi
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\t%s\n' "$path" "$branch" >> "$HOME/herdr-worktrees"
  printf '%s
' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"}}}'
else
  shift 2; agent_cwd=
  while [ "$1" != "--" ]; do if [ "$1" = "--cwd" ]; then agent_cwd=$2; shift 2; else shift; fi; done; shift
  (cd "$agent_cwd" && "$@") &
  printf '%s
' '{}'
fi
"#,
    );
    executable(
        &bin.join("prime-agent"),
        r#"#!/bin/sh
set -eu
cat >/dev/null
if [ -n "${PCE_PACKAGE_OUTCOME-}" ]; then
  printf 'known
' > known.txt
  git add known.txt; git commit -m implementation >/dev/null
  printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
else
  printf '%s' '{"findings":[]}' > "$PCE_PACKAGE_GATE_OUTCOME"
fi
"#,
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let journal = temp.path().join("driver.jsonl");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(["package", "driver-run", "--graph"])
            .arg(&graph)
            .args(["--journal"])
            .arg(&journal)
            .args(["--repository"])
            .arg(format!("repo={}", repository.display()))
            .env("HERDR_ENV", "1")
            .env(
                "PCE_WORK_PACKAGE_WORKTREE_ROOT",
                temp.path().join("worktrees"),
            )
            .env("PATH", &path)
            .env("HOME", temp.path())
            .env("USER", "tester")
            .output()
            .expect("driver")
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_snapshot: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("snapshot");
    assert_eq!(first_snapshot["packages"][0][1]["state"], "pending");
    let first_journal = fs::read_to_string(&journal).expect("journal");
    assert!(first_journal.contains("worker-dispatched"));
    assert!(first_journal.contains("worker-spawn-failed"));
    assert!(!first_journal.contains("driver-stopped-waiting"));
    let dispatch =
        fs::read_to_string(temp.path().join(".pce/package-dispatch.jsonl")).expect("dispatch");
    assert!(dispatch.contains("spawn-failed"));
    assert!(dispatch.contains("not-produced"));

    let second = run();
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let snapshot: serde_json::Value = serde_json::from_slice(&second.stdout).expect("snapshot");
    assert_eq!(snapshot["outcome"], "finished");
    let journal_text = fs::read_to_string(&journal).expect("journal");
    assert_eq!(journal_text.matches("worker-spawn-failed").count(), 1);
    assert_eq!(journal_text.matches("worker-dispatched").count(), 2);
}

#[test]
fn repeated_identical_worker_environment_failure_terminates_without_spending_recovery_budget() {
    use std::thread;
    use std::time::{Duration, Instant};

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
    fs::write(
        temp.path().join("vision.md"),
        "# Vision: environment backstop\n",
    )
    .expect("vision");
    let graph = temp.path().join("graph.json");
    fs::write(&graph, format!(r#"{{"vision":"environment-backstop-{}","plan_version":1,"authored_at_ref":"HEAD","packages":[{{"id":"A","title":"A","repositories":["repo"],"criteria":[{{"name":"floor","input":"repo","observation":"true","command":"true"}}],"depends_on":[]}},{{"id":"B","title":"B","repositories":["repo"],"criteria":[{{"name":"floor","input":"repo","observation":"true","command":"true"}}],"depends_on":[{{"id":"A","kind":"buildability","reason":"A enables B"}}]}},{{"id":"X","title":"X","repositories":["repo"],"criteria":[{{"name":"floor","input":"repo","observation":"true","command":"true"}}],"depends_on":[]}}]}}"#, std::process::id())).expect("graph");
    let journal = temp.path().join("driver.jsonl");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph)
        .args(["--journal"])
        .arg(&journal)
        .args(["--environment-failure-limit", "2"])
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args([
            "--worker-override",
            "--",
            "sh",
            "-c",
            r#"if [ "$PCE_PACKAGE" = A ]; then exit 23; else printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"; fi"#,
        ])
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("driver");
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll driver") {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill looping driver");
            let _ = child.wait();
            break None;
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert!(
        status.is_some_and(|status| status.success()),
        "driver did not terminate successfully; journal: {}",
        fs::read_to_string(&journal).unwrap_or_default()
    );
    use std::io::Read as _;
    let mut stdout = String::new();
    child
        .stdout
        .take()
        .expect("driver stdout")
        .read_to_string(&mut stdout)
        .expect("read driver stdout");
    let snapshot: serde_json::Value = serde_json::from_str(&stdout).expect("snapshot");
    let state = |package: &str| {
        snapshot["packages"]
            .as_array()
            .expect("packages")
            .iter()
            .find(|entry| entry[0] == package)
            .expect("package")[1]["state"]
            .as_str()
            .expect("state")
    };
    assert_eq!(state("A"), "environment-blocked");
    assert_eq!(state("B"), "pending");
    assert_eq!(state("X"), "complete");
    assert_eq!(snapshot["outcome"], "blocked");
    assert_eq!(snapshot["recovery"][0][1]["retry_remaining"], 1);
    assert_eq!(snapshot["recovery"][0][1]["local_patch_remaining"], 1);
    let journal = fs::read_to_string(&journal).expect("journal");
    assert_eq!(journal.matches("worker-environment-failed").count(), 1);
    assert!(journal.contains("package-environment-blocked"));
    for distinct_outcome in [
        "worker-failed",
        "package-failed",
        "package-parked",
        "recovery-parked",
        "environment-preparation-executed",
    ] {
        assert!(!journal.contains(distinct_outcome));
    }
    assert!(!journal.contains("recovery-rung-attempted"));
}
