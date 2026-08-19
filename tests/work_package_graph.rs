use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use jsonschema::Validator;
use pce_core::{
    parse_criterion_revision_manifest, parse_work_package_graph, validate_criterion_revisions,
};
use serde_json::Value;
use tempfile::tempdir;

const FIXTURE: &[u8] =
    include_bytes!("../crates/core/tests/data/rivretrieve-work-package-graph.json");
const SCHEMA: &str = include_str!("../skills/pce/schemas/work-package-graph.schema.json");

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

#[test]
fn committed_fixture_conforms_to_committed_schema() {
    let schema: Value = serde_json::from_str(SCHEMA).expect("schema JSON");
    let validator = Validator::new(&schema).expect("schema compiles");
    let fixture: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    assert!(validator.validate(&fixture).is_ok());
}

#[test]
fn freezing_version_two_preserves_readable_version_one_bytes() {
    let directory = tempdir().expect("temporary directory");
    let repository = directory.path().join("repository");
    fs::create_dir(&repository).expect("repository directory");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("seed"), "seed").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    let authored_ref = git(&repository, &["rev-parse", "HEAD"]);
    let mapping = format!("RivRetrieve={}", repository.display());

    let vision_dir = directory
        .path()
        .join("2026-08-11-the-store-is-the-only-copy");
    fs::create_dir(&vision_dir).expect("vision directory");
    let source = vision_dir.join("graph.json");
    let mut v1: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    v1["authored_at_ref"] = Value::from(authored_ref);
    let v1_source = serde_json::to_vec_pretty(&v1).expect("serialize v1");
    fs::write(&source, &v1_source).expect("seed graph v1");

    let omitted = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .output()
        .expect("unverified freeze executes");
    assert!(!omitted.status.success());
    assert!(!vision_dir.join("graph.v1.json").exists());

    let first = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &mapping])
        .output()
        .expect("freeze v1 executes");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let frozen_v1 = vision_dir.join("graph.v1.json");
    let v1_bytes = fs::read(&frozen_v1).expect("frozen v1 readable");
    assert_eq!(v1_bytes, v1_source);

    let mut v2 = v1;
    v2["plan_version"] = Value::from(2);
    v2["packages"][0]["title"] = Value::from("shared store reader v2");
    fs::write(
        &source,
        serde_json::to_vec_pretty(&v2).expect("serialize v2"),
    )
    .expect("seed graph v2");
    let second = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &mapping])
        .output()
        .expect("freeze v2 executes");
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(fs::read(&frozen_v1).expect("v1 remains readable"), v1_bytes);
    let frozen_v2 = vision_dir.join("graph.v2.json");
    let v2_bytes = fs::read(&frozen_v2).expect("frozen v2 readable");
    let predecessor = v2["packages"][0]["criteria"][0].clone();
    let mut conflicting_successor = predecessor.clone();
    conflicting_successor["command"] = Value::from("conflicting command");
    v2["packages"][0]["criteria"][0] = conflicting_successor.clone();
    fs::write(
        &source,
        serde_json::to_vec_pretty(&v2).expect("conflict JSON"),
    )
    .expect("conflicting graph");
    let record = vision_dir.join("conflicting-human-record.json");
    fs::write(
        &record,
        serde_json::to_vec(&serde_json::json!({
            "schema_version":1,
            "ratified_by":"Nicolas",
            "revisions":[{"previous_package":"RR1","predecessor":predecessor,"successor":conflicting_successor,"rationale":"Conflicts with the already frozen version."}]
        }))
        .expect("record JSON"),
    )
    .expect("record");
    let conflicting = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args([
            "--criterion-revisions",
            record.to_str().expect("UTF-8 path"),
        ])
        .args(["--repository", &mapping])
        .output()
        .expect("conflicting freeze");
    assert!(!conflicting.status.success());
    assert!(
        !vision_dir
            .join("graph.v2.criterion-revisions.json")
            .exists()
    );

    v2 = serde_json::from_slice(&v2_bytes).expect("restore v2 value");
    v2["packages"][0]["title"] = Value::from("attempted rewrite");
    fs::write(
        &source,
        serde_json::to_vec_pretty(&v2).expect("serialize rewrite"),
    )
    .expect("seed rewrite");
    let rewrite = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &mapping])
        .output()
        .expect("rewrite executes");
    assert!(!rewrite.status.success());
    assert_eq!(fs::read(&frozen_v1).expect("v1 unchanged"), v1_bytes);
    assert_eq!(fs::read(&frozen_v2).expect("v2 unchanged"), v2_bytes);

    for path in [&frozen_v1, &frozen_v2] {
        let checked = pce()
            .args(["graph", "check", "--file"])
            .arg(path)
            .args(["--repository", &mapping])
            .output()
            .expect("check executes");
        assert!(
            checked.status.success(),
            "{}",
            String::from_utf8_lossy(&checked.stderr)
        );
    }
}

#[test]
fn mechanical_freeze_accepts_only_definition_preserving_successors() {
    let directory = tempdir().expect("temporary directory");
    let repository = directory.path().join("repository");
    fs::create_dir(&repository).expect("repository directory");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("seed"), "seed").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    let first_ref = git(&repository, &["rev-parse", "HEAD"]);
    fs::write(repository.join("second"), "second").expect("second");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "second"]);
    let second_ref = git(&repository, &["rev-parse", "HEAD"]);
    let mapping = format!("RivRetrieve={}", repository.display());

    let vision_dir = directory
        .path()
        .join("2026-08-11-the-store-is-the-only-copy");
    fs::create_dir(&vision_dir).expect("vision directory");
    let source = vision_dir.join("graph.json");
    let mut graph: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    graph["authored_at_ref"] = Value::from(first_ref);
    fs::write(&source, serde_json::to_vec_pretty(&graph).expect("v1 JSON")).expect("v1");
    let mechanical_v1 = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .arg("--mechanical")
        .args(["--repository", &mapping])
        .output()
        .expect("mechanical v1 freeze");
    assert!(!mechanical_v1.status.success());
    assert!(!vision_dir.join("graph.v1.json").exists());

    let initial = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &mapping])
        .output()
        .expect("initial freeze");
    assert!(
        initial.status.success(),
        "{}",
        String::from_utf8_lossy(&initial.stderr)
    );

    graph["plan_version"] = Value::from(2);
    graph
        .as_object_mut()
        .expect("graph object")
        .remove("authored_at_ref");
    graph["authored_at_refs"] = serde_json::json!({"RivRetrieve": second_ref});
    fs::write(&source, serde_json::to_vec_pretty(&graph).expect("v2 JSON")).expect("v2");
    let mechanical = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .arg("--mechanical")
        .args(["--repository", &mapping])
        .output()
        .expect("mechanical freeze");
    assert!(
        mechanical.status.success(),
        "{}",
        String::from_utf8_lossy(&mechanical.stderr)
    );
    let receipt: Value = serde_json::from_slice(&mechanical.stdout).expect("freeze receipt");
    assert_eq!(receipt["mechanical"], true);
    assert!(vision_dir.join("graph.v2.json").exists());

    graph["plan_version"] = Value::from(3);
    graph["packages"][1]["depends_on"][0]["reason"] = Value::from("typo fixed");
    fs::write(&source, serde_json::to_vec_pretty(&graph).expect("v3 JSON")).expect("v3");
    let reason_edit = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .arg("--mechanical")
        .args(["--repository", &mapping])
        .output()
        .expect("reason-edit freeze");
    assert!(!reason_edit.status.success());
    let stderr = String::from_utf8_lossy(&reason_edit.stderr);
    assert!(stderr.contains("package definitions differ"), "{stderr}");
    assert!(!vision_dir.join("graph.v3.json").exists());

    let combined = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .arg("--mechanical")
        .args(["--criterion-revisions", "unused-human-record.json"])
        .args(["--repository", &mapping])
        .output()
        .expect("combined mechanical and revision freeze");
    assert!(!combined.status.success());
    assert!(
        String::from_utf8_lossy(&combined.stderr)
            .contains("mechanical freeze cannot carry a human criterion revision record")
    );
    assert!(!vision_dir.join("graph.v3.json").exists());
    assert!(
        !vision_dir
            .join("graph.v3.criterion-revisions.json")
            .exists()
    );
}

#[test]
fn mechanical_freeze_normalizes_multi_repository_scalar_refs() {
    let directory = tempdir().expect("temporary directory");
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    for repository in [&first, &second] {
        fs::create_dir(repository).expect("repository directory");
        git(repository, &["init", "-q"]);
        git(repository, &["config", "user.email", "test@example.com"]);
        git(repository, &["config", "user.name", "Test"]);
        fs::write(repository.join("seed"), "seed").expect("seed");
        git(repository, &["add", "."]);
        git(repository, &["commit", "-qm", "seed"]);
    }
    let vision_dir = directory.path().join("multi-repository-mechanical");
    fs::create_dir(&vision_dir).expect("vision directory");
    let source = vision_dir.join("graph.json");
    let mut graph = serde_json::json!({
        "vision": "multi-repository-mechanical",
        "plan_version": 1,
        "authored_at_ref": "HEAD",
        "packages": [{
            "id": "A",
            "title": "A",
            "repositories": ["first", "second"],
            "criteria": [{"name": "a", "input": "run", "observation": "passes", "command": "true"}],
            "depends_on": []
        }]
    });
    fs::write(&source, serde_json::to_vec(&graph).expect("v1 JSON")).expect("v1");
    let first_mapping = format!("first={}", first.display());
    let second_mapping = format!("second={}", second.display());
    let frozen_v1 = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &first_mapping])
        .args(["--repository", &second_mapping])
        .output()
        .expect("v1 freeze");
    assert!(
        frozen_v1.status.success(),
        "{}",
        String::from_utf8_lossy(&frozen_v1.stderr)
    );

    graph["plan_version"] = Value::from(2);
    graph
        .as_object_mut()
        .expect("graph object")
        .remove("authored_at_ref");
    graph["authored_at_refs"] = serde_json::json!({
        "first": "HEAD",
        "second": "refs/heads/main"
    });
    fs::write(&source, serde_json::to_vec(&graph).expect("v2 JSON")).expect("v2");
    let frozen_v2 = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .arg("--mechanical")
        .args(["--repository", &first_mapping])
        .args(["--repository", &second_mapping])
        .output()
        .expect("v2 mechanical freeze");
    assert!(
        frozen_v2.status.success(),
        "{}",
        String::from_utf8_lossy(&frozen_v2.stderr)
    );
}

#[test]
fn mechanical_freeze_refuses_a_draft_behind_the_highest_frozen_version() {
    let directory = tempdir().expect("temporary directory");
    let repository = directory.path().join("repository");
    fs::create_dir(&repository).expect("repository directory");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("seed"), "seed").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    let mapping = format!("RivRetrieve={}", repository.display());
    let vision_dir = directory
        .path()
        .join("2026-08-11-the-store-is-the-only-copy");
    fs::create_dir(&vision_dir).expect("vision directory");
    let source = vision_dir.join("graph.json");
    let mut graph: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    graph["authored_at_ref"] = Value::from("HEAD");
    fs::write(&source, serde_json::to_vec(&graph).expect("v1 JSON")).expect("v1");
    let frozen_v1 = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &mapping])
        .output()
        .expect("v1 freeze");
    assert!(frozen_v1.status.success());

    let mut future = graph.clone();
    future["plan_version"] = Value::from(3);
    fs::write(
        vision_dir.join("graph.v3.json"),
        serde_json::to_vec(&future).expect("v3 JSON"),
    )
    .expect("future frozen graph");
    graph["plan_version"] = Value::from(2);
    fs::write(&source, serde_json::to_vec(&graph).expect("v2 JSON")).expect("v2");
    let refused = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .arg("--mechanical")
        .args(["--repository", &mapping])
        .output()
        .expect("stale mechanical freeze");
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("highest frozen version"));
    assert!(!vision_dir.join("graph.v2.json").exists());
}

#[test]
fn criterion_edit_requires_explicit_human_revision_record_at_freeze() {
    let directory = tempdir().expect("temporary directory");
    let repository = directory.path().join("repository");
    fs::create_dir(&repository).expect("repository directory");
    git(&repository, &["init", "-q"]);
    git(&repository, &["config", "user.email", "test@example.com"]);
    git(&repository, &["config", "user.name", "Test"]);
    fs::write(repository.join("seed"), "seed").expect("seed");
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-qm", "seed"]);
    let authored_ref = git(&repository, &["rev-parse", "HEAD"]);
    let mapping = format!("RivRetrieve={}", repository.display());
    let vision_dir = directory
        .path()
        .join("2026-08-11-the-store-is-the-only-copy");
    fs::create_dir(&vision_dir).expect("vision directory");
    let source = vision_dir.join("graph.json");
    let mut v1: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    v1["authored_at_ref"] = Value::from(authored_ref);
    fs::write(&source, serde_json::to_vec_pretty(&v1).expect("v1 JSON")).expect("v1");
    let first = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &mapping])
        .output()
        .expect("freeze v1");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );

    let predecessor = v1["packages"][0]["criteria"][0].clone();
    let mut successor = predecessor.clone();
    successor["command"] = Value::from("uv run pytest tests/store/test_value_states.py --strict");
    let mut v2 = v1;
    v2["plan_version"] = Value::from(2);
    v2["packages"][0]["criteria"][0] = successor.clone();
    fs::write(&source, serde_json::to_vec_pretty(&v2).expect("v2 JSON")).expect("v2");

    let refused = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &mapping])
        .output()
        .expect("unratified freeze");
    assert!(!refused.status.success());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("Four value states survive compile"),
        "{stderr}"
    );
    assert!(stderr.contains("--criterion-revisions"), "{stderr}");
    assert!(!vision_dir.join("graph.v2.json").exists());

    let mismatch = vision_dir.join("mismatched-revision.v2.json");
    let mut wrong_predecessor = predecessor.clone();
    wrong_predecessor["command"] = Value::from("not the frozen predecessor bytes");
    fs::write(
        &mismatch,
        serde_json::to_vec(&serde_json::json!({
            "schema_version": 1,
            "ratified_by": "Nicolas",
            "revisions": [{
                "previous_package": "RR1",
                "predecessor": wrong_predecessor,
                "successor": successor.clone(),
                "rationale": "Attempted stale record."
            }]
        }))
        .expect("mismatch JSON"),
    )
    .expect("mismatch record");
    let mismatched = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args([
            "--criterion-revisions",
            mismatch.to_str().expect("UTF-8 path"),
        ])
        .args(["--repository", &mapping])
        .output()
        .expect("mismatched freeze");
    assert!(!mismatched.status.success());
    assert!(!vision_dir.join("graph.v2.json").exists());
    assert!(
        !vision_dir
            .join("graph.v2.criterion-revisions.json")
            .exists()
    );

    let revisions = vision_dir.join("criterion-revisions.v2.json");
    fs::write(
        &revisions,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version": 1,
            "ratified_by": "Nicolas",
            "revisions": [{
                "previous_package": "RR1",
                "predecessor": predecessor,
                "successor": successor,
                "rationale": "The delivered command requires strict mode."
            }]
        }))
        .expect("revision JSON"),
    )
    .expect("revision record");
    let ratified = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args([
            "--criterion-revisions",
            revisions.to_str().expect("UTF-8 path"),
        ])
        .args(["--repository", &mapping])
        .output()
        .expect("ratified freeze");
    assert!(
        ratified.status.success(),
        "{}",
        String::from_utf8_lossy(&ratified.stderr)
    );
    let receipt: Value = serde_json::from_slice(&ratified.stdout).expect("freeze receipt");
    assert_eq!(
        receipt["criterion_revisions_sha256"].as_str().map(str::len),
        Some(64)
    );
    assert!(vision_dir.join("graph.v2.json").exists());
    assert!(
        vision_dir
            .join("graph.v2.criterion-revisions.json")
            .exists()
    );
}

#[test]
fn planning_approval_accepts_graph_scoped_node() {
    let directory = tempdir().expect("temporary directory");
    let log = directory.path().join("events.jsonl");
    let mut child = pce()
        .args(["log", "--file"])
        .arg(&log)
        .args(["--kind", "planning-artifact-approved", "--node", "graph"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("log command starts");
    child.stdin.as_mut().expect("stdin").write_all(br#"{"path":"graph.v1.json","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","evidence":"sha256 exact frozen bytes"}"#).expect("payload writes");
    let status = child.wait().expect("log command exits");
    assert!(status.success());
    let record: Value = serde_json::from_str(fs::read_to_string(log).expect("log readable").trim())
        .expect("record JSON");
    assert_eq!(record["node"], "graph");
}

#[test]
fn schema_and_runtime_reject_shared_boundary_mutations() {
    let schema: Value = serde_json::from_str(SCHEMA).expect("schema JSON");
    let validator = Validator::new(&schema).expect("schema compiles");
    let fixture: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
    let mut mutations = Vec::new();
    let mut whitespace_schema = fixture.clone();
    whitespace_schema["$schema"] = Value::from("   ");
    mutations.push(whitespace_schema);
    let mut duplicate_dependency = fixture;
    let edge = duplicate_dependency["packages"][1]["depends_on"][0].clone();
    duplicate_dependency["packages"][1]["depends_on"]
        .as_array_mut()
        .expect("dependencies")
        .push(edge);
    mutations.push(duplicate_dependency);
    for mutation in mutations {
        assert!(validator.validate(&mutation).is_err());
        assert!(
            parse_work_package_graph(&serde_json::to_vec(&mutation).expect("mutation bytes"))
                .is_err()
        );
    }
}

#[test]
fn graph_check_verifies_each_repository_authored_ref() {
    let directory = tempdir().expect("temporary directory");
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    for repository in [&first, &second] {
        fs::create_dir(repository).expect("repository directory");
        git(repository, &["init", "-q"]);
        git(repository, &["config", "user.email", "test@example.com"]);
        git(repository, &["config", "user.name", "Test"]);
        fs::write(repository.join("seed"), repository.display().to_string()).expect("seed");
        git(repository, &["add", "."]);
        git(repository, &["commit", "-qm", "seed"]);
    }
    let first_oid = git(&first, &["rev-parse", "HEAD"]);
    let graph = serde_json::json!({
        "vision":"per-repository-refs", "plan_version":1,
        "authored_at_refs":{"first":first_oid,"second":"missing-ref"},
        "packages":[{"id":"A","title":"A","repositories":["first","second"],
          "criteria":[{"name":"a","input":"repos","observation":"works","command":"true"}],"depends_on":[]}]
    });
    let vision_dir = directory.path().join("per-repository-refs");
    fs::create_dir(&vision_dir).expect("vision directory");
    let graph_path = vision_dir.join("graph.json");
    fs::write(&graph_path, serde_json::to_vec(&graph).expect("graph JSON")).expect("graph write");
    let output = pce()
        .args(["graph", "check", "--file"])
        .arg(&graph_path)
        .args(["--repository", &format!("first={}", first.display())])
        .args(["--repository", &format!("second={}", second.display())])
        .output()
        .expect("check executes");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("second"), "{stderr}");
    assert!(stderr.contains("missing-ref"), "{stderr}");

    let frozen = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .args(["--repository", &format!("first={}", first.display())])
        .args(["--repository", &format!("second={}", second.display())])
        .output()
        .expect("freeze executes");
    assert!(!frozen.status.success());
    let stderr = String::from_utf8_lossy(&frozen.stderr);
    assert!(stderr.contains("second"), "{stderr}");
    assert!(stderr.contains("missing-ref"), "{stderr}");
    assert!(!vision_dir.join("graph.v1.json").exists());
}

#[test]
fn graph_skills_preserve_human_only_criterion_revision_authority() {
    let work_graph = include_str!("../skills/work-graph/SKILL.md");
    assert!(work_graph.contains("The skill never writes the revision record"));
    assert!(work_graph.contains("--criterion-revisions <human-authored-record-path>"));
    assert!(work_graph.contains("quoting the predecessor criterion bytes"));
    assert!(work_graph.contains("cannot be correlated exactly"));
    let to_graph = include_str!("../skills/to-graph/SKILL.md");
    assert!(to_graph.contains("commands you have executed successfully"));
    assert!(to_graph.contains("Only an explicit human-ratified revision"));
    assert!(to_graph.contains("ruling forfeits the affected package's carried"));
}

#[test]
fn revision_matching_preserves_duplicates_and_allows_ratified_package_removal() {
    let criterion = |command: &str| serde_json::json!({"name":"same","input":"repo","observation":"zero","command":command});
    let graph = |version: u64, packages: Value| {
        parse_work_package_graph(
            &serde_json::to_vec(&serde_json::json!({
                "vision":"revision-edges",
                "plan_version":version,
                "authored_at_ref":"HEAD",
                "packages":packages
            }))
            .expect("graph JSON"),
        )
        .expect("valid graph")
    };
    let previous = graph(
        1,
        serde_json::json!([
            {"id":"A","title":"A","repositories":["repo"],"criteria":[criterion("old"),criterion("old")],"depends_on":[]},
            {"id":"B","title":"B","repositories":["repo"],"criteria":[criterion("stable")],"depends_on":[]}
        ]),
    );
    let successor = graph(
        2,
        serde_json::json!([
            {"id":"A","title":"A","repositories":["repo"],"criteria":[criterion("old"),criterion("new")],"depends_on":[]},
            {"id":"B","title":"B","repositories":["repo"],"criteria":[criterion("stable")],"depends_on":[]}
        ]),
    );
    let duplicate_manifest = parse_criterion_revision_manifest(
        &serde_json::to_vec(&serde_json::json!({
            "schema_version":1,
            "ratified_by":"Nicolas",
            "revisions":[{"previous_package":"A","predecessor":criterion("old"),"successor":criterion("new"),"rationale":"Revise exactly one duplicate occurrence."}]
        }))
        .expect("manifest JSON"),
    )
    .expect("manifest");
    validate_criterion_revisions(&previous, &successor, duplicate_manifest.revisions())
        .expect("one duplicate revision is exact");

    let removed = graph(
        2,
        serde_json::json!([
            {"id":"B","title":"B","repositories":["repo"],"criteria":[criterion("stable")],"depends_on":[]}
        ]),
    );
    let removal_manifest = parse_criterion_revision_manifest(
        &serde_json::to_vec(&serde_json::json!({
            "schema_version":1,
            "ratified_by":"Nicolas",
            "revisions":[
                {"previous_package":"A","predecessor":criterion("old"),"successor":null,"rationale":"Remove first occurrence."},
                {"previous_package":"A","predecessor":criterion("old"),"successor":null,"rationale":"Remove second occurrence."}
            ]
        }))
        .expect("manifest JSON"),
    )
    .expect("manifest");
    validate_criterion_revisions(&previous, &removed, removal_manifest.revisions())
        .expect("removing the package ratifies every removed occurrence");
}
