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
fn relative_launch_composes_dependencies_without_dirtying_the_source() {
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
if [ "${1-}" = "--version" ]; then echo "herdr 0.8.2"; exit 0; fi
if [ "$1 $2" = "worktree create" ]; then
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\t%s\t%s\n' "$branch" "$base" "$path" >> "$HOME/herdr-worktrees"
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"},"root_pane":{"pane_id":"root-pane","workspace_id":"w1"}}}'
elif [ "$1 $2" = "pane run" ]; then
  (/bin/sh -c "$4") &
  printf '%s\n' '{}'
else
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
        .args(["package", "driver-run", "--graph", "graph.json"])
        .args(["--journal", "driver.jsonl"])
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .env("HERDR_ENV", "1")
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
        .env("PCE_COMPOSITION_ROOT", temp.path().join("compositions"))
        .env("PATH", path)
        .env("HOME", temp.path())
        .env("USER", "tester")
        .current_dir(temp.path())
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
    assert_eq!(git(&repository, &["status", "--short"]), "");
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
    let assembly_ref = format!(
        "refs/heads/pce/{}/assembly-v1",
        graph["vision"].as_str().expect("vision")
    );
    assert_eq!(
        git(&repository, &["rev-parse", &assembly_ref]),
        assembly_oid
    );
    assert!(events.contains(r#""event":"driver-ref-materialized""#));
    let tree = git(&repository, &["ls-tree", "--name-only", assembly_oid]);
    for file in ["a.txt", "b.txt", "c.txt"] {
        assert!(tree.lines().any(|line| line == file), "missing {file}");
    }

    // Reproduce the cold-resume shape: the proof journal survives, its driver refs do not,
    // and an out-of-band retention tag is the assembly commit's only explicit name.
    let mut cold_events = journal_events(&journal)
        .into_iter()
        .filter(|event| event["event"] != "driver-ref-materialized")
        .collect::<Vec<_>>();
    let composed_index = cold_events
        .iter()
        .position(|event| event["event"] == "assembly-repository-composed")
        .expect("assembly composition");
    cold_events.insert(
        composed_index,
        json!({
            "event": "assembly-resolution-done",
            "repository": "repo",
            "base_oid": assembly_oid,
        }),
    );
    cold_events.push(json!({"event":"driver-aborted","reason":"pre-fix abort"}));
    cold_events.push(json!({"event":"driver-resumed"}));
    let cold_journal = cold_events
        .iter()
        .map(|event| serde_json::to_string(event).expect("event json"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&journal, cold_journal).expect("cold journal");

    git(&repository, &["update-ref", "-d", &assembly_ref]);
    let vision = graph["vision"].as_str().expect("vision");
    for input in assembly["packages"].as_array().expect("assembly inputs") {
        let package = input["package"].as_str().expect("package");
        let issuance = cold_events
            .iter()
            .rev()
            .find(|event| event["event"] == "worker-done" && event["package"] == package)
            .and_then(|event| event["issuance"].as_u64())
            .expect("completed issuance");
        git(
            &repository,
            &[
                "update-ref",
                "-d",
                &format!("refs/heads/pce/{vision}/{package}/attempt-{issuance}"),
            ],
        );
    }
    git(
        &repository,
        &[
            "tag",
            "pce-retained/cold-resume/assembly-v1-resolution",
            assembly_oid,
        ],
    );

    let materialized = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "materialize-refs", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .output()
        .expect("materialize refs");
    assert!(
        materialized.status.success(),
        "{}",
        String::from_utf8_lossy(&materialized.stderr)
    );
    assert_eq!(
        git(&repository, &["rev-parse", &assembly_ref]),
        assembly_oid
    );
    let materialized_events = journal_events(&journal);
    let resolution_ref = materialized_events
        .iter()
        .find(|event| {
            event["event"] == "driver-ref-materialized"
                && event["product"]["kind"] == "assembly-resolution"
        })
        .and_then(|event| event["reference"].as_str())
        .expect("resolution materialization")
        .to_owned();
    assert_eq!(
        git(&repository, &["rev-parse", &resolution_ref]),
        assembly_oid
    );
    assert!(materialized_events.iter().any(|event| {
        event["event"] == "driver-ref-materialized"
            && event["reference"] == assembly_ref
            && event["product"]["kind"] == "assembly"
    }));
    assert!(materialized_events.iter().any(|event| {
        event["event"] == "driver-ref-materialized"
            && event["reference"] == resolution_ref
            && event["product"]["kind"] == "assembly-resolution"
    }));

    let idempotent = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "materialize-refs", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .output()
        .expect("idempotent materialization");
    assert!(idempotent.status.success());
    let report: Value = serde_json::from_slice(&idempotent.stdout).expect("report");
    assert_eq!(report["changed"], false);

    git(&repository, &["update-ref", "-d", &assembly_ref]);
    let wrong_oid = git(&repository, &["rev-parse", "HEAD"]);
    assert_ne!(wrong_oid, assembly_oid);
    git(&repository, &["update-ref", &assembly_ref, &wrong_oid]);
    let refused = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "materialize-refs", "--graph"])
        .arg(&graph_path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .output()
        .expect("wrong ref refusal");
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("refusing to move it"));
    assert_eq!(git(&repository, &["rev-parse", &assembly_ref]), wrong_oid);
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
if [ "${1-}" = "--version" ]; then echo "herdr 0.8.2"; exit 0; fi
if [ "$1 $2" = "worktree create" ]; then
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\t%s\t%s\n' "$branch" "$base" "$path" >> "$HOME/herdr-worktrees"
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"},"root_pane":{"pane_id":"root-pane","workspace_id":"w1"}}}'
elif [ "$1 $2" = "pane run" ]; then
  (/bin/sh -c "$4") &
  printf '%s\n' '{}'
else
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
      if [ ! -f "$HOME/a-gate-ran" ]; then
        touch "$HOME/a-gate-ran"
        printf '#!/bin/sh\ngrep -q "^hardened" defect.txt\n' > amendment-guard.sh
        chmod +x amendment-guard.sh
        git add amendment-guard.sh; git commit -m witness >/dev/null
        witness=$(git rev-parse HEAD)
        printf 'hardened\n' > defect.txt
        git add defect.txt; git commit -m repair >/dev/null
        repair=$(git rev-parse HEAD)
        printf '%s\n' "$repair" > "$HOME/repair-oid"
        printf '{"findings":[{"description":"defect is unhardened","repair":"harden it","proposed_criterion_command":"./amendment-guard.sh","repository_refs":[{"repository":"repo","witness_ref":"%s","repair_ref":"%s"}]},{"description":"unproved sibling","repair":"unknown","proposed_criterion_command":"false","repository_refs":[{"repository":"repo","witness_ref":"missing-witness","repair_ref":"HEAD"}]}]}' "$witness" "$repair" > "$PCE_PACKAGE_GATE_OUTCOME"
      else
        git rev-parse HEAD > "$HOME/second-gate-head"
        printf '%s' '{"findings":[]}' > "$PCE_PACKAGE_GATE_OUTCOME"
      fi;;
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
    let second_gate_head =
        fs::read_to_string(temp.path().join("second-gate-head")).expect("second gate head");
    assert_eq!(second_gate_head.trim(), repair.trim());
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
    assert_eq!(events.matches("gate-reproof-executed").count(), 4);
    let parsed = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("driver event"))
        .collect::<Vec<_>>();
    let event_index = |predicate: &dyn Fn(&Value) -> bool| {
        parsed
            .iter()
            .position(predicate)
            .expect("expected driver event")
    };
    let merged = event_index(&|event| event["event"] == "package-repair-merged");
    let authored_reproof = event_index(&|event| {
        event["event"] == "gate-reproof-executed"
            && event["gate"] == "package-gate-1-1"
            && event["name"] == "a"
    });
    let amendment_reproof = event_index(&|event| {
        event["event"] == "gate-reproof-executed"
            && event["gate"] == "package-gate-1-1"
            && event["name"] == "gate:package-gate-1-1:finding:0"
    });
    let incomplete = event_index(&|event| {
        event["event"] == "gate-failed" && event["gate"] == "package-gate-1-1"
    });
    let second_gate = event_index(&|event| {
        event["event"] == "gate-dispatched" && event["gate"] == "package-gate-1-2"
    });
    assert!(merged < authored_reproof);
    assert!(authored_reproof < amendment_reproof);
    assert!(amendment_reproof < incomplete);
    assert!(incomplete < second_gate);
    assert_eq!(
        parsed[amendment_reproof]["amendment_proof"]["outcome"],
        "reverted"
    );
    assert_ne!(
        parsed[amendment_reproof]["amendment_proof"]["execution"]["exit_status"]["code"],
        0
    );
    assert_eq!(events.matches("gate-dispatched").count(), 4);
    assert_eq!(events.matches("gate-failed").count(), 1);
    assert!(events.contains("package-gate-1-2"));
    assert_eq!(events.matches("package-hardening-invalidated").count(), 1);
    assert!(events.contains("gate:package-gate-1-1:finding:0:restore-provability"));
    assert!(events.contains("amendment_proof"));
    assert!(events.contains("reverted"));
}

fn run_conflicting_dependency_composition(
    fail_join_preparation: bool,
    attribution_case: Option<&str>,
) -> (tempfile::TempDir, std::path::PathBuf, std::process::Output) {
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
    let parent_a_command = match attribution_case {
        Some("already-red") => r#"test ! -e "$HOME/parent-red""#,
        Some("join-broke") => r#"test "$(cat shared.txt)" = A"#,
        Some(other) => panic!("unknown attribution case {other}"),
        None => "test -x ./prepared-tool && ./prepared-tool A",
    };
    let parent_a_observation = if attribution_case == Some("already-red") {
        "A won its branch; parent-red marker controls current evidence"
    } else {
        "A won its branch"
    };
    let graph = json!({
        "vision": format!("composition-conflict-{}", std::process::id()),
        "plan_version": 1,
        "authored_at_ref": "HEAD",
        "packages": [
            {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"a","input":"repo","observation":parent_a_observation,"command":parent_a_command}],"depends_on":[]},
            {"id":"B","title":"B","repositories":["repo"],"criteria":[{"name":"b","input":"repo","observation":"B won its branch","command":"test -x ./prepared-tool && ./prepared-tool B"}],"depends_on":[]},
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
if [ "${1-}" = "--version" ]; then echo "herdr 0.8.2"; exit 0; fi
if [ "$1 $2" = "worktree create" ]; then
  shift 2; cwd= path= branch= base=
  while [ $# -gt 0 ]; do case "$1" in --cwd) cwd=$2; shift 2;; --path) path=$2; shift 2;; --branch) branch=$2; shift 2;; --base) base=$2; shift 2;; *) shift;; esac; done
  git -C "$cwd" worktree add -b "$branch" "$path" "$base" >/dev/null
  printf '%s\t%s\t%s\n' "$branch" "$base" "$path" >> "$HOME/herdr-worktrees"
  printf '%s\n' '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"},"root_pane":{"pane_id":"root-pane","workspace_id":"w1"}}}'
elif [ "$1 $2" = "pane run" ]; then
  (/bin/sh -c "$4") &
  printf '%s\n' '{}'
else
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
      grep -q "compare it with the parent's own ref before attributing" "$HOME/last-brief"
      if grep -q "retain every parent's guarantee" "$HOME/last-brief"; then exit 81; fi
      printf 'A\nB\n' > shared.txt
      if grep -q 'parent-red' "$HOME/last-brief"; then touch "$HOME/parent-red"; fi
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
        .args([
            "--prepare",
            "repo=case \"${PCE_FAIL_JOIN_PREPARATION-}:$PWD\" in 1:*join-C-*) exit 42;; esac; printf '#!/bin/sh\ngrep -qx \"$1\" shared.txt\n' > prepared-tool && chmod +x prepared-tool",
        ])
        .args(["--wait-timeout-ms", "10000"])
        .env("HERDR_ENV", "1")
        .env(
            "PCE_WORK_PACKAGE_WORKTREE_ROOT",
            temp.path().join("worktrees"),
        )
        .env("PATH", path)
        .env("HOME", temp.path())
        .env("USER", "tester")
        .env(
            "PCE_FAIL_JOIN_PREPARATION",
            if fail_join_preparation { "1" } else { "" },
        )
        .output()
        .expect("driver");
    (temp, journal, output)
}

#[test]
fn conflicting_dependency_composition_dispatches_owner_and_reproves_parents() {
    let (temp, journal, output) = run_conflicting_dependency_composition(false, None);
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
    let join_preparations = events
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            event["event"] == "environment-preparation-executed"
                && event["package"] == "C"
                && event["materialization"]
                    .as_str()
                    .is_some_and(|label| label.starts_with("join-C-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(join_preparations.len(), 2);
    assert_eq!(
        join_preparations
            .iter()
            .map(|(_, event)| event["materialization"].as_str().expect("label"))
            .collect::<Vec<_>>(),
        vec!["join-C-A-3", "join-C-B-3"]
    );
    assert!(
        join_preparations
            .iter()
            .all(|(_, event)| { event["repository"] == "repo" && event["outcome"] == "succeeded" })
    );
    let parent_proofs = events
        .iter()
        .filter(|event| event["event"] == "join-criterion-executed")
        .collect::<Vec<_>>();
    assert_eq!(parent_proofs.len(), 2);
    for proof in &parent_proofs {
        let parent = proof["parent"].as_str().expect("parent");
        let preparation_index = join_preparations
            .iter()
            .find(|(_, event)| {
                event["materialization"]
                    .as_str()
                    .is_some_and(|label| label.starts_with(&format!("join-C-{parent}-")))
            })
            .map(|(index, _)| *index)
            .expect("parent join preparation");
        let proof_index = events
            .iter()
            .position(|event| std::ptr::eq(event, *proof))
            .expect("parent proof index");
        assert!(preparation_index < proof_index);
    }
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
fn parent_criterion_already_failing_parks_without_charging_joining_package() {
    let (temp, journal, output) =
        run_conflicting_dependency_composition(false, Some("already-red"));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(&journal).unwrap_or_default()
    );
    assert!(
        temp.path().join("parent-red").is_file(),
        "parent marker missing"
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    let c_state = status["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|entry| entry[0] == "C")
        .map(|entry| &entry[1])
        .expect("C state");
    assert_eq!(c_state["state"], "parked");

    let events = journal_events(&journal);
    let already_failing = events
        .iter()
        .find(|event| event["event"] == "parent-criteria-already-failing")
        .expect("typed pre-existing parent failure");
    assert_eq!(already_failing["package"], "C");
    assert_eq!(already_failing["failures"][0]["parent"], "A");
    assert_eq!(already_failing["failures"][0]["name"], "a");
    assert!(
        !events
            .iter()
            .any(|event| { event["event"] == "package-failed" && event["package"] == "C" })
    );
    assert!(
        !events.iter().any(|event| {
            event["event"] == "recovery-rung-attempted" && event["package"] == "C"
        })
    );
    let typed_events = fs::read_to_string(&journal)
        .expect("journal")
        .lines()
        .map(|line| serde_json::from_str::<pce_core::DriverEvent>(line).expect("typed event"))
        .collect::<Vec<_>>();
    assert_eq!(pce_core::charged_failure_count(&typed_events, "C"), 0);
}

#[test]
fn parent_criterion_broken_only_by_join_remains_attributed_to_joining_package() {
    let (_temp, journal, output) =
        run_conflicting_dependency_composition(false, Some("join-broke"));
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(&journal).unwrap_or_default()
    );
    let events = journal_events(&journal);
    assert!(events.iter().any(|event| {
        event["event"] == "environment-preparation-executed"
            && event["package"] == "C"
            && event["materialization"] == "parent-A-before-join-C-3"
    }));
    assert!(events.iter().any(|event| {
        event["event"] == "package-failed"
            && event["package"] == "C"
            && event["reason"] == "conflicted join broke parent criteria: A:a"
    }));
    assert!(
        !events
            .iter()
            .any(|event| event["event"] == "parent-criteria-already-failing")
    );
    let typed_events = fs::read_to_string(&journal)
        .expect("journal")
        .lines()
        .map(|line| serde_json::from_str::<pce_core::DriverEvent>(line).expect("typed event"))
        .collect::<Vec<_>>();
    assert!(pce_core::charged_failure_count(&typed_events, "C") > 0);
}

#[test]
fn failed_join_preparation_is_environmental_and_does_not_charge_the_package() {
    let (_temp, journal, output) = run_conflicting_dependency_composition(true, None);
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(&journal).unwrap_or_default()
    );
    let status: Value = serde_json::from_slice(&output.stdout).expect("status");
    assert_eq!(status["outcome"], "blocked");

    let events = journal_events(&journal);
    let worker_done = events
        .iter()
        .position(|event| event["event"] == "worker-done" && event["package"] == "C")
        .expect("dependent worker done");
    let failed_preparations = events
        .iter()
        .enumerate()
        .filter(|(_, event)| {
            event["event"] == "environment-preparation-executed"
                && event["package"] == "C"
                && event["outcome"] == "failed"
        })
        .collect::<Vec<_>>();
    assert_eq!(failed_preparations.len(), 1);
    let (failed_preparation_index, failed_preparation) = failed_preparations[0];
    assert!(worker_done < failed_preparation_index);
    assert_eq!(failed_preparation["materialization"], "join-C-A-3");
    assert_eq!(failed_preparation["repository"], "repo");
    assert_eq!(failed_preparation["execution"]["exit_status"]["code"], 42);
    assert!(
        !events.iter().any(|event| {
            event["event"] == "join-criterion-executed" && event["package"] == "C"
        })
    );
    for forbidden in [
        "criterion-executed",
        "gate-dispatched",
        "gate-finished",
        "package-completed",
        "worker-failed",
        "package-failed",
        "recovery-rung-attempted",
        "package-parked",
    ] {
        assert!(
            !events
                .iter()
                .any(|event| { event["event"] == forbidden && event["package"] == "C" }),
            "unexpected {forbidden} for dependent package"
        );
    }
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

#[test]
fn driver_refuses_an_already_dirty_source_before_composition() {
    let temp = tempdir().expect("tempdir");
    let repository = temp.path().join("repo");
    fs::create_dir(&repository).expect("repository");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("tracked"), "base\n").expect("tracked file");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "base"]);
    fs::write(repository.join("unrelated-local-change"), "dirty\n").expect("dirty file");

    let vision = temp.path().join("vision");
    fs::create_dir(&vision).expect("vision directory");
    fs::write(vision.join("graph.json"), b"{}").expect("graph fixture");
    let bin = temp.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    executable(
        &bin.join("herdr"),
        "#!/bin/sh\nif [ \"${1-}\" = --version ]; then echo 'herdr 0.8.2'; exit 0; fi\nexit 1\n",
    );
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").expect("PATH"));
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["package", "driver-run", "--graph", "graph.json"])
        .args(["--journal", "driver.jsonl"])
        .args(["--repository"])
        .arg(format!("repo={}", repository.display()))
        .env("PATH", path)
        .current_dir(&vision)
        .output()
        .expect("driver");

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(
        "source repository `repo` is dirty; commit, stash, or remove its changes before driver-run"
    ));
    assert_eq!(
        git(&repository, &["status", "--short"]),
        "?? unrelated-local-change"
    );
}
