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
fn zero_based_worktree_paths_use_the_declared_multi_repository_mapping() {
    let directory = tempdir().expect("temp");
    let (one, oid1) = repository(directory.path(), "one", &[]);
    let (two, oid2) = repository(directory.path(), "two", &["present/data.json"]);
    let graph = json!({"vision":"multi","plan_version":1,"authored_at_refs":{"one":oid1,"two":oid2},"packages":[package("A","Act", &["one","two"], "cat $PCE_WORKTREE_1/present/data.json $PCE_WORKTREE_0/missing/data.json $PCE_WORKTREE_2/out-of-range.json $PCE_WORKTREE_X/no.json", &[])]});
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
      {"id":"A","title":"Producer","repositories":["repo"],"produces":["shared/upstream.json","shared/downstream.json"],"criteria":[criterion("cat shared/upstream.json shared/downstream.json")],"depends_on":[]},
      package("B","Consumer", &["repo"], "cat shared/upstream.json", &["A"]),
      package("C","Earlier", &["repo"], "cat shared/downstream.json", &[])
    ]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let output = check(&path, &[format!("repo={}", repo.display())], false);
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    assert!(
        receipt["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .all(|warning| warning["package"] != "B"),
        "an exact path referenced by strict upstream A suppresses B's missing-producer warning"
    );
    assert!(
        receipt["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["package"] == "C" && warning["path"] == "shared/downstream.json"),
        "a downstream or unrelated reference must not suppress a missing-producer warning"
    );
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
    assert_eq!(ownership.len(), 1);
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

#[test]
fn produces_make_shared_harness_provenance_and_ownership_precise() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &["scripts/shared_harness.py"]);
    let vision = directory.path().join("outputs");
    fs::create_dir(&vision).expect("vision");
    let old = json!({"vision":"outputs","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[
      {"id":"OLD","title":"Build fixture","repositories":["repo"],"produces":["generated/fixture.json"],"criteria":[criterion("python scripts/shared_harness.py generated/fixture.json")],"depends_on":[]}
    ]});
    fs::write(
        vision.join("graph.v1.json"),
        serde_json::to_vec(&old).expect("JSON"),
    )
    .expect("old");
    let current = json!({"vision":"outputs","plan_version":2,"authored_at_refs":{"repo":oid},"packages":[
      {"id":"OLD","title":"Build fixture","repositories":["repo"],"produces":["generated/fixture.json"],"criteria":[criterion("python scripts/shared_harness.py generated/fixture.json")],"depends_on":[]},
      {"id":"NEW","title":"Repair shared harness","repositories":["repo"],"produces":["scripts/shared_harness.py"],"criteria":[criterion("python scripts/shared_harness.py generated/fixture.json")],"depends_on":[{"id":"OLD","kind":"buildability","reason":"fixture input"}]}
    ]});
    let path = vision.join("graph.json");
    fs::write(&path, serde_json::to_vec(&current).expect("JSON")).expect("current");
    let output = check(&path, &[format!("repo={}", repo.display())], true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ownership_warns_once_for_each_duplicate_declared_output() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &[]);
    let vision = directory.path().join("duplicate-output");
    fs::create_dir(&vision).expect("vision");
    let old = json!({"vision":"duplicate-output","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[
      {"id":"OLD","title":"Old","repositories":["repo"],"produces":["shared/result.json"],"criteria":[criterion("cat shared/result.json shared/result.json")],"depends_on":[]}
    ]});
    fs::write(
        vision.join("graph.v1.json"),
        serde_json::to_vec(&old).expect("JSON"),
    )
    .expect("old");
    let current = json!({"vision":"duplicate-output","plan_version":2,"authored_at_refs":{"repo":oid},"packages":[
      {"id":"OLD","title":"Old","repositories":["repo"],"produces":["shared/result.json"],"criteria":[criterion("cat shared/result.json")],"depends_on":[]},
      {"id":"NEW","title":"New","repositories":["repo"],"produces":["shared/result.json"],"criteria":[criterion("cat shared/result.json shared/result.json")],"depends_on":[]}
    ]});
    let path = vision.join("graph.json");
    fs::write(&path, serde_json::to_vec(&current).expect("JSON")).expect("current");
    let output = check(&path, &[format!("repo={}", repo.display())], false);
    assert!(output.status.success());
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    let warnings = receipt["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .filter(|warning| warning["kind"] == "act-ownership-artifact")
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["path"], "shared/result.json");
}

#[test]
fn journal_lineage_warns_only_for_earlier_completed_undeclared_delivery() {
    let directory = tempdir().expect("temp");
    let (repo, base) = repository(directory.path(), "repo", &[]);
    fs::create_dir_all(repo.join("scripts")).expect("scripts");
    fs::write(repo.join("scripts/check.py"), "fixed").expect("repair");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "repair"]);
    let repaired = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["branch", "pce/lineage/Q/attempt-1", &repaired]);
    let graph = json!({"vision":"lineage","plan_version":1,"authored_at_refs":{"repo":base},"packages":[
      {"id":"Q","title":"Repair","repositories":["repo"],"produces":["scripts/check.py"],"criteria":[criterion("true")],"depends_on":[]},
      {"id":"P","title":"Consume","repositories":["repo"],"produces":[],"criteria":[criterion("python scripts/check.py")],"depends_on":[]},
      {"id":"LATER","title":"Later","repositories":["repo"],"produces":["scripts/check.py"],"criteria":[criterion("true")],"depends_on":[]}
    ]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let journal = directory.path().join("journal.jsonl");
    fs::write(&journal, "{\"event\":\"worker-dispatched\",\"package\":\"Q\",\"issuance\":1}\n{\"event\":\"worker-done\",\"package\":\"Q\",\"issuance\":1}\n{\"event\":\"package-completed\",\"package\":\"Q\"}\n").expect("journal");
    let output = pce()
        .args(["graph", "check", "--file"])
        .arg(&path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository", &format!("repo={}", repo.display())])
        .output()
        .expect("check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    let warnings = receipt["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .filter(|warning| warning["kind"] == "lineage-delivery")
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["package"], "P");
    assert_eq!(warnings[0]["predecessor_package"], "Q");
    assert_eq!(warnings[0]["path"], "scripts/check.py");
}

#[test]
fn ownership_compares_outputs_within_the_current_graph_once() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &[]);
    let graph = json!({"vision":"current-ownership","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[
      {"id":"A","title":"First","repositories":["repo"],"produces":["shared/result.json"],"criteria":[criterion("cat shared/result.json shared/result.json")],"depends_on":[]},
      {"id":"B","title":"Second","repositories":["repo"],"produces":["shared/result.json"],"criteria":[criterion("cat shared/result.json")],"depends_on":[]}
    ]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let output = check(&path, &[format!("repo={}", repo.display())], false);
    assert!(output.status.success());
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    let warnings = receipt["warnings"]
        .as_array()
        .expect("warnings")
        .iter()
        .filter(|warning| warning["kind"] == "act-ownership-artifact")
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0]["package"], "B");
    assert_eq!(warnings[0]["predecessor_package"], "A");
    assert_eq!(warnings[0]["path"], "shared/result.json");
}

#[test]
fn shared_harness_reference_is_not_duplicate_output_ownership() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &["scripts/shared_harness.py"]);
    let graph = json!({"vision":"shared-harness","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[
      {"id":"A","title":"Fixture","repositories":["repo"],"produces":["generated/a.json"],"criteria":[criterion("python scripts/shared_harness.py generated/a.json")],"depends_on":[]},
      {"id":"B","title":"Report","repositories":["repo"],"produces":["generated/b.json"],"criteria":[criterion("python scripts/shared_harness.py generated/b.json")],"depends_on":[{"id":"A","kind":"buildability","reason":"uses fixture"}]}
    ]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let output = check(&path, &[format!("repo={}", repo.display())], true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn lineage_check_refuses_completed_package_without_resolvable_proof() {
    let directory = tempdir().expect("temp");
    let (repo, oid) = repository(directory.path(), "repo", &[]);
    let graph = json!({"vision":"missing-proof","plan_version":1,"authored_at_refs":{"repo":oid},"packages":[
      {"id":"Q","title":"Repair","repositories":["repo"],"produces":["scripts/check.py"],"criteria":[criterion("true")],"depends_on":[]},
      {"id":"P","title":"Consumer","repositories":["repo"],"produces":[],"criteria":[criterion("python scripts/check.py")],"depends_on":[]}
    ]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let journal = directory.path().join("journal.jsonl");
    fs::write(&journal, "{\"event\":\"worker-dispatched\",\"package\":\"Q\",\"issuance\":1}\n{\"event\":\"worker-done\",\"package\":\"Q\",\"issuance\":1}\n{\"event\":\"package-completed\",\"package\":\"Q\"}\n").expect("journal");
    let output = pce()
        .args(["graph", "check", "--file"])
        .arg(&path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository", &format!("repo={}", repo.display())])
        .output()
        .expect("check");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("package Q"), "{stderr}");
    assert!(stderr.contains("repository repo"), "{stderr}");
    assert!(stderr.contains("pce/missing-proof/Q/attempt-1"), "{stderr}");
    assert!(stderr.contains("driver-ref-materialized"), "{stderr}");
}

#[test]
fn materialized_package_ref_is_compatible_lineage_fallback() {
    let directory = tempdir().expect("temp");
    let (repo, base) = repository(directory.path(), "repo", &[]);
    fs::create_dir_all(repo.join("scripts")).expect("scripts");
    fs::write(repo.join("scripts/check.py"), "fixed").expect("repair");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "repair"]);
    let repaired = git(&repo, &["rev-parse", "HEAD"]);
    let graph = json!({"vision":"fallback","plan_version":1,"authored_at_refs":{"repo":base},"packages":[
      {"id":"Q","title":"Repair","repositories":["repo"],"produces":["scripts/check.py"],"criteria":[criterion("true")],"depends_on":[]},
      {"id":"P","title":"Consumer","repositories":["repo"],"produces":[],"criteria":[criterion("python scripts/check.py")],"depends_on":[]}
    ]});
    let path = directory.path().join("graph.json");
    fs::write(&path, serde_json::to_vec(&graph).expect("JSON")).expect("graph");
    let journal = directory.path().join("journal.jsonl");
    fs::write(&journal, format!("{{\"event\":\"package-completed\",\"package\":\"Q\"}}\n{{\"event\":\"driver-ref-materialized\",\"repository\":\"repo\",\"reference\":\"refs/pce/q\",\"oid\":\"{repaired}\",\"product\":{{\"kind\":\"package-attempt\",\"package\":\"Q\",\"issuance\":1}}}}\n")).expect("journal");
    let output = pce()
        .args(["graph", "check", "--file"])
        .arg(&path)
        .args(["--journal"])
        .arg(&journal)
        .args(["--repository", &format!("repo={}", repo.display())])
        .output()
        .expect("check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt: Value = serde_json::from_slice(&output.stdout).expect("receipt");
    assert!(
        receipt["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["kind"] == "lineage-delivery")
    );
}
