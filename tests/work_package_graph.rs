use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use jsonschema::Validator;
use pce_core::parse_work_package_graph;
use serde_json::Value;
use tempfile::tempdir;

const FIXTURE: &[u8] =
    include_bytes!("../crates/core/tests/data/rivretrieve-work-package-graph.json");
const SCHEMA: &str = include_str!("../skills/pce/schemas/work-package-graph.schema.json");

fn pce() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pce"))
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
    let vision_dir = directory
        .path()
        .join("2026-08-11-the-store-is-the-only-copy");
    fs::create_dir(&vision_dir).expect("vision directory");
    let source = vision_dir.join("graph.json");
    fs::write(&source, FIXTURE).expect("seed graph v1");
    let first = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .output()
        .expect("freeze v1 executes");
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let frozen_v1 = vision_dir.join("graph.v1.json");
    let v1_bytes = fs::read(&frozen_v1).expect("frozen v1 readable");
    assert_eq!(v1_bytes, FIXTURE);

    let mut v2: Value = serde_json::from_slice(FIXTURE).expect("fixture JSON");
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
    v2["packages"][0]["title"] = Value::from("attempted rewrite");
    fs::write(
        &source,
        serde_json::to_vec_pretty(&v2).expect("serialize rewrite"),
    )
    .expect("seed rewrite");
    let rewrite = pce()
        .args(["graph", "freeze", "--vision-dir"])
        .arg(&vision_dir)
        .output()
        .expect("rewrite executes");
    assert!(!rewrite.status.success());
    assert_eq!(fs::read(&frozen_v1).expect("v1 unchanged"), v1_bytes);
    assert_eq!(fs::read(&frozen_v2).expect("v2 unchanged"), v2_bytes);

    for path in [&frozen_v1, &frozen_v2] {
        let checked = pce()
            .args(["graph", "check", "--file"])
            .arg(path)
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
