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
    assert!(brief.contains("witness commit that introduces the falsifier and nothing else"));
    assert!(brief.contains("repair commit, descending from the witness"));
    assert!(brief.contains("Do not push, merge, tag, or create refs yourself"));
    assert!(brief.contains("the binary retains submitted witness and repair commits"));
    assert!(brief.contains("RivRetrieve: /worktrees/rivretrieve"));
}

#[test]
fn gate_outcome_is_strict_and_empty_findings_are_a_product() {
    let empty = parse_package_gate_outcome(br#"{"findings":[]}"#).expect("empty findings outcome");
    assert!(empty.findings().is_empty());
    assert!(parse_package_gate_outcome(b"").is_err());

    let complete = parse_package_gate_outcome(br#"{"findings":[{"description":"wrong contents","repair":"replaced bad with known","proposed_criterion_command":"test \"$(cat known.txt)\" = known","repository_refs":[{"repository":"RivRetrieve","witness_ref":"abc123","repair_ref":"def456"}]}]}"#)
        .expect("complete finding");
    let finding = &complete.findings()[0];
    assert_eq!(
        finding.proposed_criterion_command(),
        "test \"$(cat known.txt)\" = known"
    );
    let refs = &finding.repository_refs()[0];
    assert_eq!(refs.repository(), "RivRetrieve");
    assert_eq!(refs.witness_ref(), "abc123");
    assert_eq!(refs.repair_ref(), "def456");

    for malformed in [
        br#"{"findings":[{"repair":"fixed","proposed_criterion_command":"true","repository_refs":[{"repository":"RivRetrieve","witness_ref":"a","repair_ref":"b"}]}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","proposed_criterion_command":"true","repository_refs":[{"repository":"RivRetrieve","witness_ref":"a","repair_ref":"b"}]}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","repair":"fixed","repository_refs":[{"repository":"RivRetrieve","witness_ref":"a","repair_ref":"b"}]}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"true","repository_refs":[]}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"true","repository_refs":[{"repository":"RivRetrieve","repair_ref":"b"}]}]}"#.as_slice(),
        br#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"true","repository_refs":[{"repository":"RivRetrieve","witness_ref":"a"}]}]}"#.as_slice(),
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
    let challenges = directory.path().join("challenges.json");
    fs::write(&challenges, r#"[{"description":"ancestor lookup is broad","repair":"restrict the canonical path","proposed_criterion_command":"test canonical"}]"#).expect("challenges");
    let output = pce()
        .args(["package", "gate-agent", "--vision"]).arg(&vision)
        .args(["--graph"]).arg(&graph)
        .args(["--package", "T1", "--artifact-ref", "built-ref", "--outcome"]).arg(&outcome)
        .args(["--challenges"]).arg(&challenges)
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
    assert!(piped.contains("ancestor lookup is broad"));
    assert!(piped.contains("test canonical"));
    assert!(
        piped.contains(
            "Independently confirm it with a valid witness and repair pair, or refute it"
        )
    );
    assert!(piped.contains("Do not treat any claim as established"));
}

fn initialize_gate_validation_fixture(
    directory: &std::path::Path,
) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let repository = directory.join("repo");
    fs::create_dir(&repository).expect("repository");
    for arguments in [
        vec!["init", "-q"],
        vec!["config", "user.email", "gate@example.invalid"],
        vec!["config", "user.name", "Gate Test"],
    ] {
        let status = Command::new("git")
            .args(arguments)
            .current_dir(&repository)
            .status()
            .expect("git setup");
        assert!(status.success());
    }
    fs::write(repository.join("seed"), "seed").expect("seed");
    assert!(
        Command::new("git")
            .args(["add", "seed"])
            .current_dir(&repository)
            .status()
            .expect("add")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["commit", "-qm", "seed"])
            .current_dir(&repository)
            .status()
            .expect("commit")
            .success()
    );

    let vision = directory.join("vision.md");
    fs::write(
        &vision,
        r#"# Vision: refs

## Goal / Why

Prove repairs with witnesses.

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Proof","input":"commits","observation":"ordered"}]}
```
"#,
    )
    .expect("vision");
    let graph = directory.join("graph.json");
    fs::write(&graph, r#"{"vision":"refs","plan_version":1,"authored_at_ref":"HEAD","packages":[{"id":"T1","title":"prove repair","repositories":["repo"],"criteria":[{"name":"existing floor","input":"repo","observation":"seed exists","command":"test -f seed"}],"depends_on":[]}]}"#).expect("graph");
    (repository, vision, graph)
}

fn run_gate_with_outcome(
    repository: &std::path::Path,
    vision: &std::path::Path,
    graph: &std::path::Path,
    outcome_document: &str,
) -> std::process::Output {
    let outcome = repository
        .parent()
        .expect("fixture root")
        .join("validated-outcome.json");
    pce()
        .args(["package", "gate-agent", "--vision"])
        .arg(vision)
        .args(["--graph"])
        .arg(graph)
        .args(["--package", "T1", "--artifact-ref", "HEAD", "--outcome"])
        .arg(&outcome)
        .args([
            "--",
            "/bin/sh",
            "-c",
            "cat >/dev/null; printf %s \"$OUTCOME_DOCUMENT\" > \"$PCE_PACKAGE_GATE_OUTCOME\"",
        ])
        .env("PCE_WORKTREE_0", repository)
        .env("OUTCOME_DOCUMENT", outcome_document)
        .output()
        .expect("gate agent")
}

#[test]
fn gate_agent_rejects_untouched_unresolved_and_nonancestor_repository_refs() {
    let directory = tempdir().expect("temporary directory");
    let (repository, vision, graph) = initialize_gate_validation_fixture(directory.path());

    let untouched = run_gate_with_outcome(
        &repository,
        &vision,
        &graph,
        r#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"false","repository_refs":[{"repository":"other","witness_ref":"HEAD","repair_ref":"HEAD"}]}]}"#,
    );
    assert!(!untouched.status.success());
    assert!(String::from_utf8_lossy(&untouched.stderr).contains("untouched repository `other`"));

    let unresolved = run_gate_with_outcome(
        &repository,
        &vision,
        &graph,
        r#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"false","repository_refs":[{"repository":"repo","witness_ref":"missing-witness","repair_ref":"HEAD"}]}]}"#,
    );
    assert!(!unresolved.status.success());
    assert!(String::from_utf8_lossy(&unresolved.stderr).contains("does not resolve to a commit"));

    let identical = run_gate_with_outcome(
        &repository,
        &vision,
        &graph,
        r#"{"findings":[{"description":"bad","repair":"fixed","proposed_criterion_command":"false","repository_refs":[{"repository":"repo","witness_ref":"HEAD","repair_ref":"HEAD"}]}]}"#,
    );
    assert!(!identical.status.success());
    assert!(String::from_utf8_lossy(&identical.stderr).contains("identify the same commit"));

    let mut dangling = Command::new("git")
        .args(["commit-tree", "HEAD^{tree}", "-p", "HEAD"])
        .current_dir(&repository)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("dangling commit");
    use std::io::Write as _;
    dangling
        .stdin
        .as_mut()
        .expect("commit stdin")
        .write_all(b"dangling repair\n")
        .expect("commit message");
    let dangling = dangling.wait_with_output().expect("dangling result");
    assert!(dangling.status.success());
    let dangling = String::from_utf8(dangling.stdout)
        .expect("dangling UTF-8")
        .trim()
        .to_owned();
    let unreachable_document = format!(
        r#"{{"findings":[{{"description":"bad","repair":"fixed","proposed_criterion_command":"false","repository_refs":[{{"repository":"repo","witness_ref":"HEAD","repair_ref":"{dangling}"}}]}}]}}"#
    );
    let unreachable = run_gate_with_outcome(&repository, &vision, &graph, &unreachable_document);
    assert!(
        unreachable.status.success(),
        "{}",
        String::from_utf8_lossy(&unreachable.stderr)
    );
    let anchored = Command::new("git")
        .args([
            "for-each-ref",
            "--contains",
            &dangling,
            "--format=%(refname)",
            "refs/pce-gate/T1/1/1",
        ])
        .current_dir(&repository)
        .output()
        .expect("anchored refs");
    assert!(anchored.status.success());
    assert!(String::from_utf8_lossy(&anchored.stdout).contains("/repair"));

    let seed = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository)
        .output()
        .expect("seed ref");
    let seed = String::from_utf8(seed.stdout)
        .expect("seed UTF-8")
        .trim()
        .to_owned();
    fs::write(repository.join("witness"), "falsifier").expect("witness");
    assert!(
        Command::new("git")
            .args(["add", "witness"])
            .current_dir(&repository)
            .status()
            .expect("add witness")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["commit", "-qm", "witness"])
            .current_dir(&repository)
            .status()
            .expect("witness commit")
            .success()
    );
    let witness = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository)
        .output()
        .expect("witness ref");
    let witness = String::from_utf8(witness.stdout)
        .expect("witness UTF-8")
        .trim()
        .to_owned();
    assert!(
        Command::new("git")
            .args(["checkout", "-qb", "diverged", &seed])
            .current_dir(&repository)
            .status()
            .expect("diverge")
            .success()
    );
    fs::write(repository.join("repair"), "unrelated").expect("repair");
    assert!(
        Command::new("git")
            .args(["add", "repair"])
            .current_dir(&repository)
            .status()
            .expect("add repair")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["commit", "-qm", "unrelated repair"])
            .current_dir(&repository)
            .status()
            .expect("repair commit")
            .success()
    );
    let repair = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository)
        .output()
        .expect("repair ref");
    let repair = String::from_utf8(repair.stdout)
        .expect("repair UTF-8")
        .trim()
        .to_owned();
    let nonancestor_document = format!(
        r#"{{"findings":[{{"description":"bad","repair":"fixed","proposed_criterion_command":"false","repository_refs":[{{"repository":"repo","witness_ref":"{witness}","repair_ref":"{repair}"}}]}}]}}"#
    );
    let nonancestor = run_gate_with_outcome(&repository, &vision, &graph, &nonancestor_document);
    assert!(!nonancestor.status.success());
    assert!(String::from_utf8_lossy(&nonancestor.stderr).contains("must have witness ref"));
}

#[test]
fn gate_agent_accepts_distinct_direct_witness_and_repair_commits() {
    let directory = tempdir().expect("temporary directory");
    let (repository, vision, graph) = initialize_gate_validation_fixture(directory.path());
    fs::write(
        repository.join("proposed-criterion.sh"),
        "#!/bin/sh\nfalse\n",
    )
    .expect("witness");
    assert!(
        Command::new("git")
            .args(["add", "proposed-criterion.sh"])
            .current_dir(&repository)
            .status()
            .expect("add witness")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["commit", "-qm", "witness"])
            .current_dir(&repository)
            .status()
            .expect("witness")
            .success()
    );
    let witness = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository)
        .output()
        .expect("witness ref");
    let witness = String::from_utf8(witness.stdout)
        .expect("witness UTF-8")
        .trim()
        .to_owned();
    fs::write(repository.join("seed"), "repaired").expect("repair");
    assert!(
        Command::new("git")
            .args(["add", "seed"])
            .current_dir(&repository)
            .status()
            .expect("add repair")
            .success()
    );
    assert!(
        Command::new("git")
            .args(["commit", "-qm", "repair"])
            .current_dir(&repository)
            .status()
            .expect("repair")
            .success()
    );
    let repair = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository)
        .output()
        .expect("repair ref");
    let repair = String::from_utf8(repair.stdout)
        .expect("repair UTF-8")
        .trim()
        .to_owned();
    let document = format!(
        r#"{{"findings":[{{"description":"bad","repair":"fixed","proposed_criterion_command":"./proposed-criterion.sh","repository_refs":[{{"repository":"repo","witness_ref":"{witness}","repair_ref":"{repair}"}}]}}]}}"#
    );
    let accepted = run_gate_with_outcome(&repository, &vision, &graph, &document);
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
}
