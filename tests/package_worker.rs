use std::fs;
use std::process::Command;

use pce_core::{PackageOutcome, parse_package_outcome};
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
fn rr2_brief_contains_global_context_target_detail_and_scope_boundary() {
    let directory = tempdir().expect("temporary directory");
    let vision = directory.path().join("vision.md");
    fs::write(&vision, RIVRETRIEVE_VISION).expect("vision fixture");

    let run = || {
        pce()
            .args(["package", "brief", "--vision"])
            .arg(&vision)
            .args([
                "--graph",
                GRAPH,
                "--package",
                "RR2",
                "--worktree",
                "RivRetrieve=/worktrees/rivretrieve",
            ])
            .output()
            .expect("brief command")
    };
    let first = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = run();
    assert!(second.status.success());
    assert_eq!(first.stdout, second.stdout);
    let brief = String::from_utf8(first.stdout).expect("UTF-8 brief");

    assert!(brief.contains("Make compiled stores the only authoritative copy"));
    assert!(brief.contains("Certified stores replace artifacts"));
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
    assert!(brief.contains("uv run pytest tests/store/test_certification.py -k undeclared_column"));
    assert!(brief.contains("RR1 [buildability]: read-back calls the shared reader's query path"));
    assert!(brief.contains("RivRetrieve: /worktrees/rivretrieve"));
    assert!(brief.contains("RR1, RR3, RR4, RR5, RR6, and RR7 are out of bounds"));
    assert!(brief.contains("not the judgement"));
    assert!(brief.contains("executed independently"));
    assert!(brief.contains("Commit all completed work before reporting done"));
    assert!(brief.contains("uncommitted work will not be judged"));

    // Other-package summary criteria are visible, but their commands, repositories, and edges are not.
    assert!(brief.contains("Four value states survive compile"));
    assert!(!brief.contains("tests/store/test_value_states.py"));
    assert!(!brief.contains("the compile deletes the publisher artifact"));
}

#[test]
fn package_agent_pipes_composed_brief_and_exposes_outcome_path() {
    let directory = tempdir().expect("temporary directory");
    let vision = directory.path().join("vision.md");
    fs::write(&vision, r#"# Vision: trivial

## Goal / Why

Create one known file.

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Known file","input":"Inspect the file","observation":"It has known contents"}]}
```
"#).expect("vision");
    let graph = directory.path().join("graph.json");
    fs::write(&graph, r#"{"vision":"trivial","plan_version":1,"authored_at_ref":"main","packages":[{"id":"T1","title":"create known file","repositories":["repo"],"criteria":[{"name":"file exists","input":"worktree","observation":"known contents","command":"test \"$(cat known.txt)\" = known"}],"depends_on":[]}] }"#).expect("graph");
    let outcome = directory.path().join("outcome.json");
    let binary_owned_tmpdir = "/tmp/pce-tmp/e83b5998486a";
    let output = pce()
        .args(["package", "agent", "--vision"]).arg(&vision)
        .args(["--graph"]).arg(&graph)
        .args(["--package", "T1", "--outcome"]).arg(&outcome)
        .args(["--", "/bin/sh", "-c", "cat > \"$PCE_PACKAGE_OUTCOME.brief\"; printf %s \"${TMPDIR-unset}\" > \"$PCE_PACKAGE_OUTCOME.tmpdir\"; printf '{\"outcome\":\"done\"}' > \"$PCE_PACKAGE_OUTCOME\""])
        .env("PCE_WORKTREE_0", directory.path().join("repo-worktree"))
        .env("TMPDIR", "/tmp/operator-controlled-and-intentionally-long")
        .output().expect("package agent");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(matches!(
        parse_package_outcome(&fs::read(&outcome).expect("outcome")),
        Ok(PackageOutcome::Done)
    ));
    assert_eq!(
        fs::read_to_string(format!("{}.tmpdir", outcome.display()))
            .expect("child TMPDIR observation"),
        binary_owned_tmpdir
    );
    let piped = fs::read_to_string(format!("{}.brief", outcome.display())).expect("piped brief");
    assert!(piped.contains("Create one known file."));
    assert!(piped.contains(&format!(
        "repo: {}",
        directory.path().join("repo-worktree").display()
    )));
}

#[test]
fn two_repository_package_carries_both_worktree_paths() {
    let directory = tempdir().expect("temporary directory");
    let vision = directory.path().join("vision.md");
    fs::write(
        &vision,
        r#"# Vision: two

## Goal / Why

Change two repositories coherently.

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Coherent","input":"Inspect both","observation":"Both changed"}]}
```
"#,
    )
    .expect("vision");
    let graph = directory.path().join("graph.json");
    fs::write(&graph, r#"{"vision":"two","plan_version":1,"authored_at_ref":"main","packages":[{"id":"T2","title":"two repositories","repositories":["alpha","beta"],"criteria":[{"name":"both","input":"trees","observation":"changed","command":"true"}],"depends_on":[]}] }"#).expect("graph");
    let first = directory.path().join("alpha-tree");
    let second = directory.path().join("beta-tree");
    let output = pce()
        .args(["package", "brief", "--vision"])
        .arg(&vision)
        .args(["--graph"])
        .arg(&graph)
        .args(["--package", "T2", "--worktree"])
        .arg(format!("alpha={}", first.display()))
        .args(["--worktree"])
        .arg(format!("beta={}", second.display()))
        .output()
        .expect("brief");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let brief = String::from_utf8(output.stdout).expect("brief UTF-8");
    assert!(brief.contains(&format!("alpha: {}", first.display())));
    assert!(brief.contains(&format!("beta: {}", second.display())));
}
