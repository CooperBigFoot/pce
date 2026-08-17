use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::tempdir;

fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write executable");
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("permissions");
}

fn git(root: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

fn journal_events(path: &Path) -> Vec<Value> {
    fs::read_to_string(path)
        .expect("journal")
        .lines()
        .map(|line| serde_json::from_str(line).expect("journal event"))
        .collect()
}

#[test]
fn dependent_contains_two_dependencies_and_finished_assembly_is_gated() {
    let temp = tempdir().expect("tempdir");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("repository");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("seed"), "seed\n").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    fs::write(temp.path().join("vision.md"), "# Vision: composition\n\n## Goal / Why\n\nCompose.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"whole\",\"input\":\"repo\",\"observation\":\"green\"}]}\n```\n").expect("vision");
    let graph_path = temp.path().join("graph.json");
    let graph = json!({"vision":format!("composition-{}", std::process::id()),"plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"a","input":"repo","observation":"a","command":"test -f a.txt"}],"depends_on":[]},
        {"id":"B","title":"B","repositories":["repo"],"criteria":[{"name":"b","input":"repo","observation":"b","command":"test -f b.txt"}],"depends_on":[]},
        {"id":"C","title":"C","repositories":["repo"],"criteria":[{"name":"both","input":"repo","observation":"both","command":"test -f a.txt && test -f b.txt && test -f c.txt"}],"depends_on":[
            {"id":"A","kind":"buildability","reason":"needs A"},
            {"id":"B","kind":"safety","reason":"needs B"}
        ]}
    ]});
    fs::write(&graph_path, serde_json::to_vec(&graph).expect("graph")).expect("graph write");
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
  printf '%s\t%s\t%s\n' "$branch" "$base" "$path" >> "$HOME/herdr-worktrees"
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
if [ -n "${PCE_PACKAGE_OUTCOME-}" ]; then
  branch=$(git branch --show-current)
  case "$branch" in
    */A/*) printf a > a.txt;;
    */B/*) printf b > b.txt;;
    */C/*) test -f a.txt; test -f b.txt; printf c > c.txt;;
    *) exit 80;;
  esac
  git add .; git commit -m implementation >/dev/null
  printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
else
  printf '%s' '{"findings":[]}' > "$PCE_PACKAGE_GATE_OUTCOME"
fi
"#,
    );
    let journal = temp.path().join("driver.jsonl");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
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
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(&journal).unwrap_or_default()
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(status["outcome"], "finished");
    assert_eq!(status["assembly"]["state"], "complete");
    let events = fs::read_to_string(&journal).expect("journal");
    assert!(events.contains("package-base-composed"));
    assert!(!events.contains("package-join-conflicted"));
    assert!(!events.contains("join-criterion-executed"));
    assert_eq!(events.matches("assembly-criterion-executed").count(), 3);
    assert!(events.contains("assembly-completed"));
    let worktrees = fs::read_to_string(temp.path().join("herdr-worktrees")).expect("herdr log");
    let c = worktrees
        .lines()
        .find(|line| {
            line.starts_with(&format!(
                "pce/{}/C/",
                graph["vision"].as_str().expect("vision")
            ))
        })
        .expect("C dispatch");
    let mut fields = c.split('\t');
    let _branch = fields.next().expect("branch");
    let base = fields.next().expect("base");
    let base_tree = git(&repository, &["ls-tree", "--name-only", base]);
    assert!(base_tree.lines().any(|line| line == "a.txt"));
    assert!(base_tree.lines().any(|line| line == "b.txt"));
    let assembly = events
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|event| event["event"] == "assembly-repository-composed")
        .expect("assembly event");
    let assembly_oid = assembly["base_oid"].as_str().expect("assembly oid");
    let tree = git(&repository, &["ls-tree", "--name-only", assembly_oid]);
    for file in ["a.txt", "b.txt", "c.txt"] {
        assert!(tree.lines().any(|line| line == file), "missing {file}");
    }
}

#[test]
fn credited_gate_repair_is_hardened_before_dependent_composition_and_reproved() {
    let temp = tempdir().expect("tempdir");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("repository");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("seed"), "seed\n").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    fs::write(temp.path().join("vision.md"), "# Vision: hardened lineage\n\n## Goal / Why\n\nHarden.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"whole\",\"input\":\"repo\",\"observation\":\"green\"}]}\n```\n").expect("vision");
    let graph_path = temp.path().join("graph.json");
    let vision = format!("hardened-lineage-{}", std::process::id());
    let graph = json!({"vision":vision,"plan_version":1,"authored_at_ref":"HEAD","packages":[
        {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"a","input":"repo","observation":"exists","command":"test -f defect.txt"}],"depends_on":[]},
        {"id":"B","title":"B","repositories":["repo"],"criteria":[{"name":"b","input":"repo","observation":"hardened","command":"test -f b.txt"}],"depends_on":[{"id":"A","kind":"buildability","reason":"needs hardened A"}]}
    ]});
    fs::write(&graph_path, serde_json::to_vec(&graph).expect("graph")).expect("graph write");
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
  printf '%s\t%s\t%s\n' "$branch" "$base" "$path" >> "$HOME/herdr-worktrees"
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
branch=$(git branch --show-current)
if [ -n "${PCE_PACKAGE_OUTCOME-}" ]; then
  case "$branch" in
    */A/*) printf 'broken\n' > defect.txt;;
    */B/*)
      test -f amendment-guard.sh
      case "$branch" in
        */B/attempt-2)
          rm defect.txt
          printf '#!/bin/sh\nexit 0\n' > amendment-guard.sh
          chmod +x amendment-guard.sh;;
        *) test "$(cat defect.txt)" = hardened;;
      esac
      printf 'b\n' > b.txt;;
    *) exit 80;;
  esac
  git add .; git commit -m implementation >/dev/null
  printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
else
  case "$branch" in
    */A/*)
      implementation=$(git for-each-ref --format='%(refname:short)' refs/heads/pce | grep '/A/attempt-1$' | grep -v -- '-gate-' | head -n 1)
      git reset --hard "$implementation" >/dev/null
      printf '#!/bin/sh\ngrep -q "^hardened" defect.txt\n' > amendment-guard.sh
      chmod +x amendment-guard.sh
      git add amendment-guard.sh; git commit -m witness >/dev/null
      witness=$(git rev-parse HEAD)
      printf 'hardened\n' > defect.txt
      git add defect.txt; git commit -m repair >/dev/null
      repair=$(git rev-parse HEAD)
      printf '%s\n' "$repair" > "$HOME/repair-oid"
      printf '{"findings":[{"description":"defect is unhardened","repair":"harden it","proposed_criterion_command":"./amendment-guard.sh","repository_refs":[{"repository":"repo","witness_ref":"%s","repair_ref":"%s"}]}]}' "$witness" "$repair" > "$PCE_PACKAGE_GATE_OUTCOME";;
    *) printf '%s' '{"findings":[]}' > "$PCE_PACKAGE_GATE_OUTCOME";;
  esac
fi
"#,
    );
    let journal = temp.path().join("driver.jsonl");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args(["--wait-timeout-ms", "10000"])
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
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(&journal).unwrap_or_default()
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(status["outcome"], "finished");
    let repair = fs::read_to_string(temp.path().join("repair-oid")).expect("repair");
    let branch = format!(
        "pce/{}/A/attempt-1",
        graph["vision"].as_str().expect("vision")
    );
    git(
        &repository,
        &["merge-base", "--is-ancestor", repair.trim(), &branch],
    );
    let events = fs::read_to_string(&journal).expect("journal");
    assert_eq!(events.matches("package-repair-merged").count(), 1);
    assert_eq!(events.matches("package-hardening-invalidated").count(), 1);
    assert!(events.contains("gate:package-gate-1:finding:0:restore-provability"));
    assert!(events.contains("amendment_proof"));
    assert!(events.contains("reverted"));
}

#[test]
fn conflicting_dependency_composition_dispatches_owner_and_reproves_parents() {
    let temp = tempdir().expect("tempdir");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("repository");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("shared.txt"), "base\n").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    fs::write(
        temp.path().join("vision.md"),
        "# Vision: composition test\n\n## Goal / Why\n\nCompose packages.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"whole\",\"input\":\"repo\",\"observation\":\"green\"}]}\n```\n",
    )
    .expect("vision");

    let graph_path = temp.path().join("graph.json");
    let graph = json!({
        "vision": format!("composition-conflict-{}", std::process::id()),
        "plan_version": 1,
        "authored_at_ref": "HEAD",
        "packages": [
            {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"a","input":"repo","observation":"A won its branch","command":"grep -qx A shared.txt"}],"depends_on":[]},
            {"id":"B","title":"B","repositories":["repo"],"criteria":[{"name":"b","input":"repo","observation":"B won its branch","command":"grep -qx B shared.txt"}],"depends_on":[]},
            {"id":"C","title":"C","repositories":["repo"],"criteria":[{"name":"composed","input":"repo","observation":"dependencies compose","command":"true"}],"depends_on":[
                {"id":"A","kind":"buildability","reason":"needs A"},
                {"id":"B","kind":"buildability","reason":"needs B"}
            ]}
        ]
    });
    fs::write(&graph_path, serde_json::to_vec(&graph).expect("graph")).expect("graph write");
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
  printf '%s\t%s\t%s\n' "$branch" "$base" "$path" >> "$HOME/herdr-worktrees"
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
cat > "$HOME/last-brief"
if [ -n "${PCE_PACKAGE_OUTCOME-}" ]; then
  branch=$(git branch --show-current)
  case "$branch" in
    */A/*) printf 'A\n' > shared.txt;;
    */B/*) printf 'B\n' > shared.txt;;
    */C/*)
      git diff --name-only --diff-filter=U | grep -qx shared.txt
      grep -q 'shared.txt' "$HOME/last-brief"
      printf 'A\nB\n' > shared.txt
      git add shared.txt
      ;;
    *) exit 80;;
  esac
  git add .; git commit -m implementation >/dev/null
  printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
else
  printf '%s' '{"findings":[]}' > "$PCE_PACKAGE_GATE_OUTCOME"
fi
"#,
    );
    let journal = temp.path().join("driver.jsonl");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args(["--wait-timeout-ms", "10000"])
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
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(&journal).unwrap_or_default()
    );

    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(
        status["outcome"],
        "finished",
        "{}",
        fs::read_to_string(&journal).expect("journal")
    );

    let events = journal_events(&journal);
    let conflict = events
        .iter()
        .find(|event| event["event"] == "package-join-conflicted")
        .expect("typed conflicted join event");
    assert_eq!(conflict["package"], "C");
    assert_eq!(conflict["repository"], "repo");
    assert!(
        conflict["reason"]
            .as_str()
            .expect("reason")
            .contains("shared.txt")
    );
    assert_eq!(conflict["conflicted_paths"], json!(["shared.txt"]));
    assert_eq!(
        conflict["dependencies"]
            .as_array()
            .expect("parent refs")
            .len(),
        2
    );
    assert!(
        events
            .iter()
            .any(|event| { event["event"] == "worker-dispatched" && event["package"] == "C" })
    );
    let parent_proofs = events
        .iter()
        .filter(|event| event["event"] == "join-criterion-executed")
        .collect::<Vec<_>>();
    assert_eq!(parent_proofs.len(), 2);
    assert!(
        parent_proofs
            .iter()
            .all(|event| { event["execution"]["exit_status"]["code"] == 0 })
    );
    let last_parent_proof = events
        .iter()
        .rposition(|event| event["event"] == "join-criterion-executed")
        .expect("parent proof");
    let dependent_criterion = events
        .iter()
        .position(|event| event["event"] == "criterion-executed" && event["package"] == "C")
        .expect("dependent criterion");
    assert!(last_parent_proof < dependent_criterion);
    assert!(!temp.path().join("unexpected-c-dispatch").exists());
}

#[test]
fn final_assembly_reexecutes_all_criteria_and_fails_without_failing_a_package() {
    let temp = tempdir().expect("tempdir");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("repository");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("seed"), "seed\n").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    fs::write(
        temp.path().join("vision.md"),
        "# Vision: composition test\n\n## Goal / Why\n\nCompose packages.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"whole\",\"input\":\"repo\",\"observation\":\"green\"}]}\n```\n",
    )
    .expect("vision");

    let graph_path = temp.path().join("graph.json");
    let graph = json!({
        "vision": format!("assembly-failure-{}", std::process::id()),
        "plan_version": 1,
        "authored_at_ref": "HEAD",
        "packages": [
            {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"b-absent","input":"repo","observation":"B has not added its file","command":"test ! -e b.txt"}],"depends_on":[]},
            {"id":"B","title":"B","repositories":["repo"],"criteria":[{"name":"b-present","input":"repo","observation":"B added its file","command":"test -f b.txt"}],"depends_on":[]}
        ]
    });
    fs::write(&graph_path, serde_json::to_vec(&graph).expect("graph")).expect("graph write");
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
if [ -n "${PCE_PACKAGE_OUTCOME-}" ]; then
  branch=$(git branch --show-current)
  case "$branch" in
    */A/*) printf 'a\n' > a.txt;;
    */B/*) printf 'b\n' > b.txt;;
    *) exit 80;;
  esac
  git add .; git commit -m implementation >/dev/null
  printf '%s' '{"outcome":"done"}' > "$PCE_PACKAGE_OUTCOME"
else
  printf '%s' '{"findings":[]}' > "$PCE_PACKAGE_GATE_OUTCOME"
fi
"#,
    );
    let journal = temp.path().join("driver.jsonl");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .args(["--wait-timeout-ms", "10000"])
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
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(&journal).unwrap_or_default()
    );

    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(status["outcome"], "blocked");
    assert_eq!(status["assembly"]["state"], "failed");
    assert!(
        status["packages"]
            .as_array()
            .expect("packages")
            .iter()
            .all(|entry| entry[1]["state"] == "complete")
    );

    let events = journal_events(&journal);
    let assembly_criteria = events
        .iter()
        .filter(|event| event["event"] == "assembly-criterion-executed")
        .collect::<Vec<_>>();
    assert_eq!(assembly_criteria.len(), 2);
    assert!(assembly_criteria.iter().any(|event| {
        event["package"] == "A"
            && event["name"] == "b-absent"
            && event["execution"]["exit_status"]["kind"] == "exited"
            && event["execution"]["exit_status"]["code"] != 0
    }));
    assert!(assembly_criteria.iter().any(|event| {
        event["package"] == "B"
            && event["name"] == "b-present"
            && event["execution"]["exit_status"]["code"] == 0
    }));
    assert!(
        events
            .iter()
            .any(|event| event["event"] == "assembly-failed")
    );
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == "assembly-completed")
    );
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == "package-failed")
    );
}
