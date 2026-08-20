use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) {
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
}

fn write_graph(path: &Path, version: u64) {
    fs::write(
        path,
        serde_json::to_vec(&json!({
            "vision":"external-evidence", "plan_version":version, "authored_at_ref":"HEAD",
            "packages":[{"id":"A","title":"A","repositories":["repo"],
                "external_evidence_root":"CAMPAIGN_EVIDENCE",
                "criteria":[{"name":"green","input":"repo","observation":"zero","command":"true"}],
                "depends_on":[]}]
        }))
        .expect("graph"),
    )
    .expect("graph write");
}

fn run_driver(
    root: &Path,
    graph: &Path,
    journal: &Path,
    repo: &Path,
    evidence: &Path,
    worker: &Path,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "package",
            "driver-run",
            "--graph",
            graph.to_str().expect("graph"),
            "--journal",
            journal.to_str().expect("journal"),
            "--repository",
            &format!("repo={}", repo.display()),
            "--worker-env",
            "CAMPAIGN_EVIDENCE",
            "--worker-override",
            "--",
            "/bin/sh",
            worker.to_str().expect("worker"),
        ])
        .env("CAMPAIGN_EVIDENCE", evidence)
        .current_dir(root)
        .output()
        .expect("pce")
}

#[test]
fn external_evidence_root_is_valid_in_the_published_graph_schema() {
    let schema: Value = serde_json::from_str(include_str!(
        "../skills/pce/schemas/work-package-graph.schema.json"
    ))
    .expect("schema JSON");
    let validator = jsonschema::Validator::new(&schema).expect("schema compiles");
    let graph = json!({
        "vision":"external-evidence", "plan_version":1, "authored_at_ref":"HEAD",
        "packages":[{"id":"A","title":"A","repositories":["repo"],
            "external_evidence_root":"CAMPAIGN_EVIDENCE",
            "criteria":[{"name":"green","input":"repo","observation":"zero","command":"true"}],
            "depends_on":[]}]
    });
    assert!(validator.validate(&graph).is_ok());
}

#[test]
fn carried_completion_reports_changed_external_evidence_root() {
    let temp = TempDir::new().expect("tempdir");
    let repo = temp.path().join("repo");
    fs::create_dir(&repo).expect("repo");
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "test@example.com"]);
    git(&repo, &["config", "user.name", "Test"]);
    fs::write(repo.join("value"), "base").expect("value");
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "base"]);
    fs::write(temp.path().join("vision.md"), "# Vision\n\n## Goal / Why\n\nProve external evidence.\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{\"criteria\":[{\"name\":\"done\",\"input\":\"root\",\"observation\":\"recorded\"}]}\n```\n").expect("vision");
    let evidence = temp.path().join("evidence");
    fs::create_dir(&evidence).expect("evidence");
    fs::write(evidence.join("campaign-record.json"), "campaign-one").expect("evidence one");
    let worker = temp.path().join("worker.sh");
    fs::write(
        &worker,
        "#!/bin/sh\nprintf '%s' '{\"outcome\":\"done\"}' > \"$PCE_PACKAGE_OUTCOME\"\n",
    )
    .expect("worker");
    let graph1 = temp.path().join("graph.v1.json");
    write_graph(&graph1, 1);
    let journal = temp.path().join("journal.jsonl");
    let first = run_driver(temp.path(), &graph1, &journal, &repo, &evidence, &worker);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_events = fs::read_to_string(&journal).expect("journal");
    assert!(first_events.contains("external-evidence-recorded"));

    let graph2 = temp.path().join("graph.v2.json");
    write_graph(&graph2, 2);
    let carried = run_driver(temp.path(), &graph2, &journal, &repo, &evidence, &worker);
    assert!(
        carried.status.success(),
        "{}",
        String::from_utf8_lossy(&carried.stderr)
    );
    assert!(
        !fs::read_to_string(&journal)
            .expect("journal")
            .contains("carried-completion-external-evidence-mismatch")
    );

    fs::write(evidence.join("campaign-record.json"), "campaign-two").expect("evidence two");
    let second = run_driver(temp.path(), &graph2, &journal, &repo, &evidence, &worker);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let events = fs::read_to_string(&journal).expect("journal");
    let mismatch = events
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("event"))
        .find(|event| event["event"] == "carried-completion-external-evidence-mismatch")
        .expect("mismatch");
    assert_eq!(mismatch["package"], "A");
    assert_eq!(mismatch["environment"], "CAMPAIGN_EVIDENCE");
    assert_eq!(mismatch["current"]["state"], "identified");
    assert_ne!(
        mismatch["completed_identity"],
        mismatch["current"]["identity"]
    );
    let status: Value = serde_json::from_slice(&second.stdout).expect("status");
    assert_eq!(status["external_evidence_mismatches"][0]["package"], "A");

    let graph3 = temp.path().join("graph.v3.json");
    write_graph(&graph3, 3);
    fs::remove_dir_all(&evidence).expect("remove external evidence root");
    let missing = run_driver(temp.path(), &graph3, &journal, &repo, &evidence, &worker);
    assert!(
        missing.status.success(),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    let missing_status: Value = serde_json::from_slice(&missing.stdout).expect("missing status");
    assert_eq!(
        missing_status["external_evidence_mismatches"][0]["current"]["state"],
        "root-missing"
    );
}
