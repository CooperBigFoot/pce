mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

use serde_json::json;
use support::CliHarness;

const RATIFIED: &str = r#"{"name":"Ratified criterion","input":"Run the ratified probe.","observation":"The probe exits 0."}"#;
const SECOND: &str = r#"{"name":"Second ratified criterion","input":"Run the second probe.","observation":"The second probe exits 0."}"#;
const ADDED: &str = r#"{"name":"Added criterion","input":"Run the added probe.","observation":"The added probe exits 0."}"#;
const REFUSAL: &str = "criterion change refused: proposed acceptance criteria must exactly match the ratified floor plus logged criterion-added events";

struct Fixture {
    harness: CliHarness,
    log: PathBuf,
    vision_dir: PathBuf,
}

impl Fixture {
    fn new(ratified: &str) -> Self {
        let harness = CliHarness::new().expect("CLI harness");
        let root = harness.path();
        let log = root.join("events.jsonl");
        let vision_dir = root.join("planning/2026-08-03-criterion-change-fixture");
        fs::create_dir_all(&vision_dir).expect("vision directory");
        fs::write(
            vision_dir.join("vision.md"),
            document("ratified fixture", ratified),
        )
        .expect("ratified vision");
        fs::write(root.join("approved.json"), b"{}").expect("approved artifact");
        let contract = json!({
            "repository":"pce","repo_root":root.to_str().expect("UTF-8 root"),
            "stated":{"format":"cargo fmt --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --release","version_policy":"NONE","branch_convention":"pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>","pull_request_convention":"step head targets the matching milestone integration branch"},
            "observations":{"format":0,"lint":0,"typecheck":0,"test":0,"build":0},
            "workflow_map":{"ci.yml":"cargo test --workspace","docs.yml":null},
            "appendable":{"environment_hazards":["stdin is reserved for event payload input"],"gate_orderings":["format before lint before typecheck before test before build"],"lockfile_rules":["Cargo.lock must remain synchronized with Cargo.toml"]},
            "evidence":"cargo fmt --check\ncargo clippy --workspace --all-targets\ncargo check --workspace --all-targets\ncargo test --workspace\ncargo build --release"
        });
        append(&harness, &log, "repository-contract", &contract.to_string());
        append(
            &harness,
            &log,
            "planning-artifact-approved",
            r#"{"path":"approved.json","sha256":"44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a","evidence":"sha256 of exact approved.json bytes"}"#,
        );
        Self {
            harness,
            log,
            vision_dir,
        }
    }

    fn check(&self, proposed: &str) -> Output {
        self.harness
            .run(
                [
                    "criteria",
                    "check",
                    "--file",
                    utf8(&self.log),
                    "--vision-dir",
                    utf8(&self.vision_dir),
                ],
                document("proposed fixture", proposed).as_bytes(),
            )
            .expect("criteria check")
    }

    fn append_added(&self) -> Output {
        self.harness
            .run(
                [
                    "log",
                    "--file",
                    utf8(&self.log),
                    "--kind",
                    "criterion-added",
                    "--node",
                    "m4-s1",
                ],
                br#"{"criterion":{"name":"Added criterion","input":"Run the added probe.","observation":"The added probe exits 0."},"change_of_course":"Reality exposed an uncovered failure."}"#,
            )
            .expect("criterion-added append")
    }
}

fn document(title: &str, entries: &str) -> String {
    format!(
        "# Vision: {title}\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{{\"criteria\":[{entries}]}}\n```\n"
    )
}

fn utf8(path: &Path) -> &str {
    path.to_str().expect("UTF-8 fixture path")
}

fn append(harness: &CliHarness, log: &Path, kind: &str, payload: &str) {
    let output = harness
        .run(
            [
                "log",
                "--file",
                utf8(log),
                "--kind",
                kind,
                "--node",
                "m4-s1",
            ],
            payload.as_bytes(),
        )
        .expect("append event");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
}

fn report(decision: &str, required: &str, proposed: &str) -> String {
    format!(
        "{{\"decision\":\"{decision}\",\"required_criteria\":[{required}],\"proposed_criteria\":[{proposed}]}}\n"
    )
}

fn assert_refused(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, expected.as_bytes());
    assert!(String::from_utf8_lossy(&output.stderr).contains(REFUSAL));
}

#[test]
fn unchanged_ratified_document_exits_zero_with_exact_json() {
    let fixture = Fixture::new(RATIFIED);
    let output = fixture.check(RATIFIED);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        output.stdout,
        report("accept", RATIFIED, RATIFIED).as_bytes()
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn removal_reordering_and_all_field_modifications_exit_one() {
    let two = Fixture::new(&format!("{RATIFIED},{SECOND}"));
    assert_refused(
        &two.check(SECOND),
        &report("refuse", &format!("{RATIFIED},{SECOND}"), SECOND),
    );
    assert_refused(
        &two.check(&format!("{SECOND},{RATIFIED}")),
        &report(
            "refuse",
            &format!("{RATIFIED},{SECOND}"),
            &format!("{SECOND},{RATIFIED}"),
        ),
    );
    let one = Fixture::new(RATIFIED);
    let modifications = [
        r#"{"name":"Renamed criterion","input":"Run the ratified probe.","observation":"The probe exits 0."}"#,
        r#"{"name":"Ratified criterion","input":"Run a weaker probe.","observation":"The probe exits 0."}"#,
        r#"{"name":"Ratified criterion","input":"Run the ratified probe.","observation":"The probe may exit 0."}"#,
    ];
    for modified in modifications {
        assert_refused(&one.check(modified), &report("refuse", RATIFIED, modified));
    }
}

#[test]
fn growth_requires_the_existing_criterion_added_surface() {
    let fixture = Fixture::new(RATIFIED);
    let grown = format!("{RATIFIED},{ADDED}");
    assert_refused(&fixture.check(&grown), &report("refuse", RATIFIED, &grown));
    let append = fixture.append_added();
    assert_eq!(append.status.code(), Some(0));
    assert!(append.stderr.is_empty());
    let accepted = fixture.check(&grown);
    assert_eq!(accepted.status.code(), Some(0));
    assert_eq!(accepted.stdout, report("accept", &grown, &grown).as_bytes());
    assert!(accepted.stderr.is_empty());
    assert_refused(
        &fixture.check(RATIFIED),
        &report("refuse", &grown, RATIFIED),
    );
}

#[test]
fn malformed_proposed_document_fails_before_a_report() {
    let fixture = Fixture::new(RATIFIED);
    let output = fixture
        .harness
        .run(
            [
                "criteria",
                "check",
                "--file",
                utf8(&fixture.log),
                "--vision-dir",
                utf8(&fixture.vision_dir),
            ],
            b"# Vision: missing acceptance criteria\n",
        )
        .expect("criteria check");
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to parse proposed acceptance criteria"));
    assert!(stderr.contains(
        "vision document must contain exactly one `## Acceptance criteria (vision-level \"done\")` section"
    ));
}
