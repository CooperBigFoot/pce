use std::fs;
use std::process::Command;

use pce_core::parse_package_gate_outcome;
use tempfile::tempdir;

const GRAPH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/crates/core/tests/data/rivretrieve-work-package-graph.json"
);
const RIVRETRIEVE_VISION: &[u8] = include_bytes!("../crates/core/tests/data/rivretrieve-vision.md");

fn pce() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pce"))
}

#[test]
fn rr2_gate_brief_marks_passed_criteria_as_floor_and_binds_repair_scope() {
    let directory = tempdir().expect("temporary directory");
    let vision = directory.path().join("vision.md");
    fs::write(&vision, RIVRETRIEVE_VISION).expect("vision fixture");

    let output = pce()
        .args(["package", "gate-brief", "--vision"])
        .arg(&vision)
        .args([
            "--graph",
            GRAPH,
            "--package",
            "RR2",
            "--artifact-ref",
            "refs/pce/built/rr2-abc123",
            "--worktree",
            "RivRetrieve=/worktrees/rivretrieve",
        ])
        .output()
        .expect("gate brief command");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let brief = String::from_utf8(output.stdout).expect("UTF-8 brief");

    assert!(brief.contains("Make compiled stores the only authoritative copy"));
    for (id, title) in [
        ("RR1", "shared store reader"),
        ("RR2", "certification harness"),
        ("RR3", "ca_eccc on the store"),
        ("RR4", "pl_imgw on the store"),
        ("RR5", "bulk surface + consent"),
        ("RR6", "raw -> receipts"),
        ("RR7", "retire legacy + document"),
    ] {
        assert!(brief.contains(&format!("{id}: {title}")), "missing {id}");
    }
    assert!(brief.contains("refs/pce/built/rr2-abc123"));
    assert!(brief.contains("uv run pytest tests/store/test_certification.py -k undeclared_column"));
    assert!(brief.contains("already passed"));
    assert!(brief.contains("floor, not targets"));
    assert!(brief.contains("Repair only the defect you named"));
    assert!(brief.contains("Do not refactor"));
    assert!(brief.contains("Do not expand scope"));
    assert!(brief.contains("Do not push, merge, or tag"));
    assert!(brief.contains("RivRetrieve: /worktrees/rivretrieve"));
}

#[test]
fn gate_outcome_is_strict_and_empty_findings_are_a_product() {
    let empty = parse_package_gate_outcome(br#"{"findings":[]}"#).expect("empty findings outcome");
    assert!(empty.findings().is_empty());
    assert!(parse_package_gate_outcome(b"").is_err());

    let complete = parse_package_gate_outcome(br#"{"findings":[{"description":"wrong contents","repair":"replaced bad with known","proposed_criterion_command":"test \"$(cat known.txt)\" = known","pre_repair_ref":"abc123","post_repair_ref":"def456"}]}"#)
        .expect("complete finding");
    let finding = &complete.findings()[0];
    assert_eq!(
        finding.proposed_criterion_command(),
        "test \"$(cat known.txt)\" = known"
    );
    assert_eq!(finding.pre_repair_ref(), "abc123");
    assert_eq!(finding.post_repair_ref(), "def456");

    for malformed in [
        br#"{"findings":[{"repair":"fixed","proposed_criterion_command":"true","pre_repair_ref":"a","post_repair_ref":"b"}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","proposed_criterion_command":"true","pre_repair_ref":"a","post_repair_ref":"b"}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","repair":"fixed","pre_repair_ref":"a","post_repair_ref":"b"}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"true","post_repair_ref":"b"}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"true","pre_repair_ref":"a"}]}"#.as_slice(),
        br#"{"findings":[{"description":" ","repair":"fixed","proposed_criterion_command":"true","pre_repair_ref":"a","post_repair_ref":"b"}]}"#.as_slice(),
        br#"{"findings":[],"extra":true}"#.as_slice(),
    ] {
        assert!(parse_package_gate_outcome(malformed).is_err(), "accepted {}", String::from_utf8_lossy(malformed));
    }
}

#[test]
fn gate_agent_pipes_brief_exposes_outcome_and_does_not_execute_proposed_criterion() {
    let directory = tempdir().expect("temporary directory");
    let vision = directory.path().join("vision.md");
    fs::write(
        &vision,
        r#"# Vision: trivial

## Goal / Why

Create one known file.

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Known file","input":"Inspect it","observation":"It is right"}]}
```
"#,
    )
    .expect("vision");
    let graph = directory.path().join("graph.json");
    let marker = directory.path().join("criterion-ran");
    fs::write(&graph, format!(r#"{{"vision":"trivial","plan_version":1,"authored_at_ref":"main","packages":[{{"id":"T1","title":"create known file","repositories":["repo"],"criteria":[{{"name":"file exists","input":"worktree","observation":"exists","command":"touch {}"}}],"depends_on":[]}}]}}"#, marker.display())).expect("graph");
    let outcome = directory.path().join("gate-outcome.json");
    let output = pce()
        .args(["package", "gate-agent", "--vision"]).arg(&vision)
        .args(["--graph"]).arg(&graph)
        .args(["--package", "T1", "--artifact-ref", "built-ref", "--outcome"]).arg(&outcome)
        .args(["--", "/bin/sh", "-c", "cat > \"$PCE_PACKAGE_GATE_OUTCOME.brief\"; printf %s \"$TMPDIR\" > \"$PCE_PACKAGE_GATE_OUTCOME.tmpdir\"; printf '{\"findings\":[]}' > \"$PCE_PACKAGE_GATE_OUTCOME\""])
        .env("PCE_WORKTREE_0", directory.path().join("repo-worktree"))
        .output().expect("gate agent");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed =
        parse_package_gate_outcome(&fs::read(&outcome).expect("outcome")).expect("valid outcome");
    assert!(parsed.findings().is_empty());
    assert_eq!(
        fs::read_to_string(format!("{}.tmpdir", outcome.display())).expect("child TMPDIR"),
        "/tmp/pce-tmp/e83b5998486a"
    );
    assert!(!marker.exists(), "criterion command was executed");
    let piped = fs::read_to_string(format!("{}.brief", outcome.display())).expect("piped brief");
    assert!(piped.contains("built-ref"));
    assert!(piped.contains("floor, not targets"));
}
