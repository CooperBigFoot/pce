use std::fs;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::tempdir;

fn pce() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pce"))
}

fn git(root: &std::path::Path, args: &[&str]) -> String {
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
        .expect("UTF-8")
        .trim()
        .to_owned()
}

fn repository(root: &std::path::Path, name: &str, files: &[&str]) -> (std::path::PathBuf, String) {
    let path = root.join(name);
    fs::create_dir(&path).expect("repository");
    git(&path, &["init", "-q"]);
    git(&path, &["config", "user.email", "test@example.com"]);
    git(&path, &["config", "user.name", "Test"]);
    for file in files {
        let target = path.join(file);
        fs::create_dir_all(target.parent().expect("parent")).expect("parents");
        fs::write(target, "x").expect("file");
    }
    fs::write(path.join("seed"), "seed").expect("seed");
    git(&path, &["add", "."]);
    git(&path, &["commit", "-qm", "seed"]);
    let oid = git(&path, &["rev-parse", "HEAD"]);
    (path, oid)
}

fn criterion(command: &str) -> Value {
    json!({"name":"proof","input":"run","observation":"passes","command":command})
}
fn package(
    id: &str,
    title: &str,
    repositories: &[&str],
    command: &str,
    dependencies: &[&str],
) -> Value {
    json!({"id":id,"title":title,"repositories":repositories,"criteria":[criterion(command)],"depends_on":dependencies.iter().map(|id| json!({"id":id,"kind":"buildability","reason":"required"})).collect::<Vec<_>>()})
}
fn check(path: &std::path::Path, mappings: &[String], strict: bool) -> std::process::Output {
    let mut command = pce();
    command.args(["graph", "check", "--file"]).arg(path);
    if strict {
        command.arg("--strict");
    }
    for mapping in mappings {
        command.args(["--repository", mapping]);
    }
    command.output().expect("check")
}

#[test]
fn artifact_lints_warn_by_default_fail_in_strict_and_ignore_ambiguous_tokens() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &[]);
    let graph = json!({"vision":"lint","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[package("A","Act", &["repo"], "test -f missing/artifact.json /absolute/path ../escape.json *.json https://example/a.json '$VALUE/file' path/{one,two}.json", &[])]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let mapping = format!("repo={}", repo.display());
    let default = check(&path, std::slice::from_ref(&mapping), false);
    assert!(
        default.status.success(),
        "{}",
        String::from_utf8_lossy(&default.stderr)
    );
    let receipt: Value = serde_json::from_slice(&default.stdout).expect("receipt");
    assert_eq!(receipt["valid"], true);
    assert_eq!(receipt["refs_verified"], true);
    assert_eq!(receipt["warnings"].as_array().expect("warnings").len(), 1);
    assert_eq!(receipt["warnings"][0]["path"], "missing/artifact.json");
    let strict = check(&path, &[mapping], true);
    assert!(!strict.status.success());
    assert!(String::from_utf8_lossy(&strict.stderr).contains("authoring warning"));
}

#[test]
fn exact_worktree_paths_use_the_declared_multi_repository_mapping() {
    let directory = tempdir().expect("temp");
    let (one, oid1) = repository(directory.path(), "one", &[]);
    let (two, oid2) = repository(directory.path(), "two", &["present/data.json"]);
    let graph = json!({"vision":"multi","plan_version":1,"authored_at_refs":{"one":oid1,"two":oid2},"packages":[package("A","Act", &["one","two"], "cat $PCE_WORKTREE_2/present/data.json $PCE_WORKTREE_1/missing/data.json $PCE_WORKTREE_X/no.json", &[])]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let output = check(
        &path,
        &[
            format!("one={}", one.display()),
            format!("two={}", two.display()),
        ],
        false,
    );
    assert!(output.status.success());
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    let warnings = receipt["warnings"].as_array().expect("warnings");
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0]["repository"], "one");
    assert_eq!(warnings[0]["path"], "missing/data.json");
}

#[test]
fn provenance_distinguishes_strict_upstream_from_downstream_references() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &[]);
    let graph = json!({"vision":"edges","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[
      package("A","Producer", &["repo"], "cat shared/upstream.json shared/downstream.json", &[]),
      package("B","Consumer", &["repo"], "cat shared/upstream.json", &["A"]),
      package("C","Earlier", &["repo"], "cat shared/downstream.json", &[])
    ]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let output = check(&path, &[format!("repo={}", repo.display())], false);
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    let b = receipt["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .find(|w| w["package"] == "B")
        .expect("B warning");
    assert_eq!(b["strict_upstream_references"], json!(["A"]));
    assert!(
        b["message"]
            .as_str()
            .expect("message")
            .contains("graph has no output declarations")
    );
    let a_down = receipt["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .find(|w| w["package"] == "A" && w["path"] == "shared/downstream.json")
        .expect("A warning");
    assert_eq!(a_down["strict_upstream_references"], json!([]));
}

#[test]
fn ownership_applies_only_to_new_packages_and_missing_predecessor_is_visible() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &[]);
    let vision = directory.path().join("ownership");
    fs::create_dir(&vision).expect("vision");
    let predecessor = json!({"vision":"ownership","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[package("OLD","Create shared manifest", &["repo"], "cat shared/manifest.json", &[])]});
    fs::write(
        vision.join("graph.v1.json"),
        serde_json::to_vec(&predecessor).expect("JSON"),
    )
    .expect("predecessor");
    let successor = json!({"vision":"ownership","plan_version":2,"authored_at_refs":{"repo":oid},"packages":[package("OLD","Create shared manifest", &["repo"], "cat shared/manifest.json", &[]),package("NEW","create-shared manifest!", &["repo"], "cat shared/manifest.json", &[])]});
    let path = vision.join("graph.json");
    fs::write(&path, serde_json::to_vec(&successor).expect("JSON")).expect("graph");
    let mapping = format!("repo={}", repo.display());
    let output = check(&path, std::slice::from_ref(&mapping), false);
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    let ownership = receipt["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .filter(|w| {
            w["kind"]
                .as_str()
                .expect("kind")
                .starts_with("act-ownership")
        })
        .collect::<Vec<_>>();
    assert_eq!(ownership.len(), 2);
    assert!(ownership.iter().all(|w| w["package"] == "NEW"));
    fs::remove_file(vision.join("graph.v1.json")).expect("remove predecessor");
    let missing = check(&path, &[mapping], false);
    assert!(missing.status.success());
    let receipt: Value = serde_json::from_slice(&missing.stdout).expect("receipt");
    assert!(
        receipt["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|w| w["kind"] == "predecessor-unavailable")
    );
}
