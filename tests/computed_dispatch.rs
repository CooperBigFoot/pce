mod support;

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::{CliHarness, Invocation, ScriptedResponse};

const VISION_DOCUMENT: &str = r#"# Vision: fixture

## Acceptance criteria (vision-level "done")

```json
{
  "criteria": [
    {
      "name": "Ratified floor",
      "input": "Run the finished thing.",
      "observation": "It reports success."
    }
  ]
}
```

## Decomposition hints

None.
"#;

fn write_vision_document(vision_dir: &Path) {
    fs::write(vision_dir.join("vision.md"), VISION_DOCUMENT).expect("vision fixture");
}

#[derive(Clone, Copy)]
enum Authority {
    NotMerged,
    Merged,
    Unreachable,
}

struct ReadyFixture {
    harness: CliHarness,
    primary: PathBuf,
    docs: PathBuf,
    vision: PathBuf,
    log: PathBuf,
}

impl ReadyFixture {
    fn new(graph: &Value, dispatches: &[(&str, &str)]) -> Self {
        Self::new_with_contracts(graph, dispatches, |primary, docs| {
            vec![
                repository_contract("pce", primary),
                repository_contract("docs", docs),
            ]
        })
    }

    fn new_with_contracts(
        graph: &Value,
        dispatches: &[(&str, &str)],
        contracts: impl FnOnce(&Path, &Path) -> Vec<Value>,
    ) -> Self {
        let harness = CliHarness::new().expect("CLI harness");
        let primary = harness.path().join("pce");
        let docs = harness.path().join("docs");
        let vision = primary.join("planning/2026-07-28-computed");
        let log = primary.join("events.jsonl");
        fs::create_dir_all(&vision).expect("vision directory");
        write_vision_document(&vision);
        fs::create_dir_all(&docs).expect("docs repository");
        let graph_path = primary.join("graph.json");
        let bytes = serde_json::to_vec(graph).expect("graph JSON");
        fs::write(&graph_path, &bytes).expect("graph fixture");
        let digest = format!("{:x}", Sha256::digest(&bytes));

        let contract_payloads = contracts(&primary, &docs);
        let mut records = contract_payloads
            .into_iter()
            .enumerate()
            .map(|(offset, payload)| {
                record(1 + offset as u64, "repository-contract", "m1-s1", payload)
            })
            .collect::<Vec<_>>();
        let first_dispatch_sequence = 1 + records.len() as u64;
        for (offset, (node, role)) in dispatches.iter().enumerate() {
            records.push(record(
                first_dispatch_sequence + offset as u64,
                "dispatch",
                node,
                json!({
                    "role": role,
                    "ref": format!("{:040}", offset + 1),
                    "evidence": "dispatch fixture"
                }),
            ));
        }
        records.push(record(
            first_dispatch_sequence + dispatches.len() as u64,
            "planning-artifact-approved",
            "m1-s1",
            json!({
                "path": "graph.json",
                "sha256": digest,
                "evidence": "fixture digest"
            }),
        ));
        write_records(&log, &records);
        Self {
            harness,
            primary,
            docs,
            vision,
            log,
        }
    }

    fn root(&self, repository: &str) -> &Path {
        match repository {
            "pce" => &self.primary,
            "docs" => &self.docs,
            other => panic!("unknown fixture repository {other}"),
        }
    }

    fn run(&self) -> std::process::Output {
        self.harness
            .run(
                [
                    OsString::from("ready"),
                    OsString::from("--file"),
                    self.log.as_os_str().to_owned(),
                    OsString::from("--vision-dir"),
                    self.vision.as_os_str().to_owned(),
                ],
                b"",
            )
            .expect("run ready")
    }

    fn run_with_graph(&self, graph_path: &str) -> std::process::Output {
        self.harness
            .run(
                [
                    OsString::from("ready"),
                    OsString::from("--file"),
                    self.log.as_os_str().to_owned(),
                    OsString::from("--vision-dir"),
                    self.vision.as_os_str().to_owned(),
                    OsString::from("--graph"),
                    OsString::from(graph_path),
                ],
                b"",
            )
            .expect("run ready")
    }

    fn run_status(&self) -> std::process::Output {
        self.harness
            .run(
                [
                    OsString::from("status"),
                    OsString::from("--file"),
                    self.log.as_os_str().to_owned(),
                    OsString::from("--vision-dir"),
                    self.vision.as_os_str().to_owned(),
                ],
                b"",
            )
            .expect("run status")
    }
}

#[test]
fn routes_both_altitudes_through_exact_repository_local_selectors() {
    let graph = graph(vec![
        node("m2", "pce", vec![]),
        node("m2-s3", "docs", vec![]),
    ]);
    let fixture = ReadyFixture::new(&graph, &[]);
    fixture
        .harness
        .materialize_responses(&responses(
            &fixture,
            &[
                (
                    "m2",
                    "pce",
                    "pce/computed/milestone-2",
                    "main",
                    Authority::NotMerged,
                ),
                (
                    "m2-s3",
                    "docs",
                    "pce/computed/m2-s3",
                    "pce/computed/milestone-2",
                    Authority::NotMerged,
                ),
            ],
        ))
        .expect("responses");

    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m2","repository":"pce"},
            {"classification":"ready","node":"m2-s3","repository":"docs"}
        ]})
    );
    let invocations = fixture.harness.invocations().expect("invocations");
    let mut expected =
        expected_not_merged_invocations(&fixture.primary, "pce/computed/milestone-2", "main");
    expected.extend(expected_not_merged_invocations(
        &fixture.docs,
        "pce/computed/m2-s3",
        "pce/computed/milestone-2",
    ));
    assert_eq!(invocations, expected);
}

#[test]
fn reason_edge_changes_waiting_to_ready_when_dependency_merges() {
    for (authority, expected) in [
        (Authority::NotMerged, "waiting"),
        (Authority::Merged, "ready"),
    ] {
        let graph = graph(vec![
            node("m1-s1", "pce", vec![]),
            node(
                "m1-s2",
                "pce",
                vec![json!({"id":"m1-s1","reason":"required predecessor API"})],
            ),
        ]);
        let fixture = ReadyFixture::new(&graph, &[]);
        fixture
            .harness
            .materialize_responses(&responses(
                &fixture,
                &[
                    (
                        "m1-s1",
                        "pce",
                        "pce/computed/m1-s1",
                        "pce/computed/milestone-1",
                        authority,
                    ),
                    (
                        "m1-s2",
                        "pce",
                        "pce/computed/m1-s2",
                        "pce/computed/milestone-1",
                        Authority::NotMerged,
                    ),
                ],
            ))
            .expect("responses");
        let output = fixture.run();
        assert_success(&output);
        let value = stdout_json(&output);
        assert_eq!(value["results"][1]["classification"], expected);
    }
}

#[test]
fn contract_policies_narrow_only_the_selected_repository() {
    let graph = graph(vec![
        node("m1-s1", "pce", vec![]),
        node("m1-s2", "pce", vec![]),
        node("m1-s3", "docs", vec![]),
        node("m1-s4", "docs", vec![]),
    ]);
    for (pce_policy, docs_policy, expected) in [
        (
            "SERIALIZE_DISPATCHES",
            "NONE",
            vec!["ready", "waiting", "ready", "ready"],
        ),
        (
            "NONE",
            "SERIALIZE_DISPATCHES",
            vec!["ready", "ready", "ready", "waiting"],
        ),
    ] {
        let fixture = ReadyFixture::new_with_contracts(&graph, &[], |primary, docs| {
            vec![
                repository_contract_with_policy("pce", primary, pce_policy),
                repository_contract_with_policy("docs", docs, docs_policy),
            ]
        });
        fixture
            .harness
            .materialize_responses(&all_not_merged(&fixture, &graph))
            .expect("responses");
        let output = fixture.run();
        assert_success(&output);
        let value = stdout_json(&output);
        let actual = value["results"]
            .as_array()
            .expect("results")
            .iter()
            .map(|item| item["classification"].as_str().expect("classification"))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
    }

    let fixture = ReadyFixture::new(&graph, &[]);
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &graph))
        .expect("responses");
    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output)["results"]
            .as_array()
            .expect("results")
            .len(),
        4
    );
}

#[test]
fn latest_current_contract_policy_controls_readiness() {
    let graph = two_pce_node_graph();
    let fixture = ReadyFixture::new_with_contracts(&graph, &[], |primary, _| {
        vec![
            repository_contract_with_policy("pce", primary, "NONE"),
            repository_contract_with_policy("pce", primary, "SERIALIZE_DISPATCHES"),
        ]
    });
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &graph))
        .expect("responses");

    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m1-s1","repository":"pce"},
            {"classification":"waiting","node":"m1-s2","repository":"pce"}
        ]})
    );
}

#[test]
fn legacy_only_history_reads_none_policy_at_default_branch_head() {
    let graph = two_pce_node_graph();
    let fixture = ReadyFixture::new_with_contracts(&graph, &[], |primary, _| {
        vec![legacy_repository_contract("pce", primary)]
    });
    let mut scripted = tracked_contract_responses(&fixture.primary, 0, tracked_contract_none());
    scripted.extend(all_not_merged(&fixture, &graph));
    fixture
        .harness
        .materialize_responses(&scripted)
        .expect("responses");

    let output = fixture.run();
    assert_success(&output);
    let invocations = fixture.harness.invocations().expect("invocations");
    assert_eq!(
        &invocations[..2],
        &[
            invocation(
                "git",
                git_args(
                    &fixture.primary,
                    &["symbolic-ref", "refs/remotes/origin/HEAD"]
                )
            ),
            invocation(
                "git",
                git_args(
                    &fixture.primary,
                    &["show", "main:.pce/repository-contract.json"]
                )
            )
        ]
    );
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m1-s1","repository":"pce"},
            {"classification":"ready","node":"m1-s2","repository":"pce"}
        ]})
    );
}

#[test]
fn legacy_only_history_fails_when_tracked_contract_is_unreachable() {
    let graph = two_pce_node_graph();
    let fixture = ReadyFixture::new_with_contracts(&graph, &[], |primary, _| {
        vec![legacy_repository_contract("pce", primary)]
    });
    let mut scripted = tracked_contract_responses(
        &fixture.primary,
        128,
        b"fatal: path '.pce/repository-contract.json' does not exist in 'main'\n",
    );
    scripted.extend(all_not_merged(&fixture, &graph));
    fixture
        .harness
        .materialize_responses(&scripted)
        .expect("responses");

    let output = fixture.run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        stderr(&output),
        "Error: repository pce has neither a current repository-contract record nor .pce/repository-contract.json at default-branch HEAD\n"
    );
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        vec![
            invocation(
                "git",
                git_args(
                    &fixture.primary,
                    &["symbolic-ref", "refs/remotes/origin/HEAD"]
                )
            ),
            invocation(
                "git",
                git_args(
                    &fixture.primary,
                    &["show", "main:.pce/repository-contract.json"]
                )
            )
        ]
    );
}

/// Preservation guard for replacement projection and live command shapes.
#[test]
fn legacy_then_current_history_keeps_status_and_ready_operational() {
    let graph = graph(vec![node("m1-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new_with_contracts(&graph, &[], |primary, _| {
        vec![
            legacy_repository_contract("pce", primary),
            repository_contract_with_policy("pce", primary, "NONE"),
        ]
    });
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &graph))
        .expect("responses");

    let status = fixture.run_status();
    assert!(
        status.status.success(),
        "status failed with stderr: {}",
        stderr(&status)
    );
    assert_eq!(
        stdout_json(&status).pointer("/repositories/0/repository"),
        Some(&json!("pce"))
    );

    let ready = fixture.run_with_graph("graph.json");
    assert_success(&ready);
    assert_eq!(
        stdout_json(&ready),
        json!({"results":[
            {"classification":"ready","node":"m1-s1","repository":"pce"}
        ]})
    );
    let invocations = fixture.harness.invocations().expect("invocations");
    assert!(!invocations.iter().any(|invocation| {
        invocation.program == OsStr::new("git")
            && (invocation.argv
                == git_args(
                    &fixture.primary,
                    &["symbolic-ref", "refs/remotes/origin/HEAD"],
                )
                || invocation.argv
                    == git_args(
                        &fixture.primary,
                        &["show", "main:.pce/repository-contract.json"],
                    ))
    }));
}

#[test]
fn digest_mismatch_fails_before_any_adapter_invocation() {
    let original_graph = graph(vec![node("m1-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new(&original_graph, &[]);
    let changed_graph =
        serde_json::to_vec(&graph(vec![node("m1-s2", "pce", vec![])])).expect("changed graph JSON");
    fs::write(fixture.primary.join("graph.json"), changed_graph).expect("mutate graph");
    fixture
        .harness
        .materialize_responses(&[])
        .expect("empty responses");

    let output = fixture.run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("graph.json"));
    assert!(error.contains("digest"));
    assert!(
        fixture
            .harness
            .invocations()
            .expect("invocations")
            .is_empty()
    );
}

#[test]
fn default_selector_walk_skips_newer_non_graph() {
    let older_graph = graph(vec![node("m4-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new(&older_graph, &[]);
    let graph_bytes = fs::read(fixture.primary.join("graph.json")).expect("graph bytes");
    let graph_digest = format!("{:x}", Sha256::digest(&graph_bytes));
    let plan_bytes = b"# selected implementation plan\n";
    fs::write(fixture.primary.join("plan.md"), plan_bytes).expect("plan fixture");
    let plan_digest = format!("{:x}", Sha256::digest(plan_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m4-s1",
                json!({"path":"graph.json","sha256":graph_digest,"evidence":"older graph"}),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m4-s1",
                json!({"path":"plan.md","sha256":plan_digest,"evidence":"newer plan"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &older_graph))
        .expect("responses");

    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m4-s1","repository":"pce"}
        ]})
    );
    let error = stderr(&output);
    assert!(error.contains("plan.md"));
    assert!(error.contains("graph is not valid JSON"));
}

#[test]
fn selector_targets_an_older_approved_graph() {
    let newer_graph = graph(vec![node("m4-s2", "pce", vec![])]);
    let fixture = ReadyFixture::new(&newer_graph, &[]);
    let older_graph = graph(vec![node("m4-s1", "pce", vec![])]);
    let older_bytes = serde_json::to_vec(&older_graph).expect("older graph JSON");
    fs::write(fixture.primary.join("older.json"), &older_bytes).expect("older graph fixture");
    let older_digest = format!("{:x}", Sha256::digest(&older_bytes));
    let newer_bytes = fs::read(fixture.primary.join("graph.json")).expect("newer graph bytes");
    let newer_digest = format!("{:x}", Sha256::digest(&newer_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m4-s1",
                json!({"path":"older.json","sha256":older_digest,"evidence":"older graph"}),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m4-s2",
                json!({"path":"graph.json","sha256":newer_digest,"evidence":"newer graph"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &older_graph))
        .expect("responses");

    let output = fixture.run_with_graph("older.json");
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m4-s1","repository":"pce"}
        ]})
    );
    assert!(!stdout_json(&output).to_string().contains("m4-s2"));
}

#[test]
fn selector_digest_is_bound_to_the_named_record() {
    let other_graph = graph(vec![node("m5-s2", "pce", vec![])]);
    let fixture = ReadyFixture::new(&other_graph, &[]);
    let named_graph = graph(vec![node("m5-s1", "pce", vec![])]);
    let named_bytes = serde_json::to_vec(&named_graph).expect("named graph JSON");
    fs::write(fixture.primary.join("named.json"), &named_bytes).expect("named graph fixture");
    let other_bytes = fs::read(fixture.primary.join("graph.json")).expect("other graph bytes");
    let other_digest = format!("{:x}", Sha256::digest(&other_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m5-s1",
                json!({
                    "path":"named.json",
                    "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "evidence":"stale named graph"
                }),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m5-s2",
                json!({"path":"graph.json","sha256":other_digest,"evidence":"matching other graph"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&[])
        .expect("empty responses");

    let output = fixture.run_with_graph("named.json");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("selected planning artifact failed preliminary provenance check"));
    assert!(error.contains("digest mismatch"));
    assert!(error.contains("named.json"));
    assert!(!error.contains("planning artifact ArtifactPath(\"graph.json\") digest mismatch"));
}

#[test]
fn unknown_graph_selector_fails_without_fallback() {
    let graph = graph(vec![node("m6-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new(&graph, &[]);
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &graph))
        .expect("responses");

    let output = fixture.run_with_graph("unknown.json");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        stderr(&output)
            .contains("no planning-artifact-approved record for graph path unknown.json")
    );
    assert_ne!(output.stdout, br#"{"results":[]}"#);
    assert!(
        fixture
            .harness
            .invocations()
            .expect("invocations")
            .is_empty()
    );
}

#[test]
fn named_non_graph_fails_without_fallback() {
    let older_graph = graph(vec![node("m7-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new(&older_graph, &[]);
    let graph_bytes = fs::read(fixture.primary.join("graph.json")).expect("graph bytes");
    let graph_digest = format!("{:x}", Sha256::digest(&graph_bytes));
    let plan_bytes = b"# implementation plan\n";
    fs::write(fixture.primary.join("plan.md"), plan_bytes).expect("plan fixture");
    let plan_digest = format!("{:x}", Sha256::digest(plan_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m7-s1",
                json!({"path":"graph.json","sha256":graph_digest,"evidence":"older graph"}),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m7-s1",
                json!({"path":"plan.md","sha256":plan_digest,"evidence":"selected plan"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&[])
        .expect("empty responses");

    let output = fixture.run_with_graph("plan.md");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("approved graph path plan.md is not a conforming graph:"));
    assert!(error.contains("graph is not valid JSON"));
    assert!(
        fixture
            .harness
            .invocations()
            .expect("invocations")
            .is_empty()
    );
}

#[test]
fn graph_selector_digest_mismatch_precedes_all_adapters() {
    let original_graph = graph(vec![node("m8-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new(&original_graph, &[]);
    let changed_graph =
        serde_json::to_vec(&graph(vec![node("m8-s2", "pce", vec![])])).expect("changed graph JSON");
    fs::write(fixture.primary.join("graph.json"), changed_graph).expect("mutate graph");
    fixture
        .harness
        .materialize_responses(&[])
        .expect("empty responses");

    let output = fixture.run_with_graph("graph.json");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("graph.json"));
    assert!(error.contains("digest mismatch"));
    assert_eq!(
        fixture.harness.invocations().expect("invocations"),
        Vec::<Invocation>::new()
    );
}

#[test]
fn older_graph_survives_newer_missing_and_non_graph_approvals() {
    let older_graph = graph(vec![node("m2-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new(&older_graph, &[]);
    let graph_bytes = fs::read(fixture.primary.join("graph.json")).expect("graph bytes");
    let graph_digest = format!("{:x}", Sha256::digest(&graph_bytes));
    let plan_bytes = b"# implementation plan\n";
    fs::write(fixture.primary.join("plan.md"), plan_bytes).expect("plan fixture");
    let plan_digest = format!("{:x}", Sha256::digest(plan_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m2-s1",
                json!({"path":"graph.json","sha256":graph_digest,"evidence":"older graph"}),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m2-s1",
                json!({
                    "path":"missing.json",
                    "sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "evidence":"missing artifact"
                }),
            ),
            record(
                5,
                "planning-artifact-approved",
                "m2-s1",
                json!({"path":"plan.md","sha256":plan_digest,"evidence":"newer plan"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &older_graph))
        .expect("responses");

    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m2-s1","repository":"pce"}
        ]})
    );
    let error = stderr(&output);
    assert!(error.contains("plan.md"));
    assert!(error.contains("graph is not valid JSON"));
    assert!(error.contains("missing.json"));
    assert!(error.contains("artifact missing"));
}

#[test]
fn digest_verdict_belongs_to_selected_older_graph() {
    let older_graph = graph(vec![node("m2-s2", "pce", vec![])]);
    let fixture = ReadyFixture::new(&older_graph, &[]);
    let plan_bytes = b"# matching plan\n";
    fs::write(fixture.primary.join("plan.md"), plan_bytes).expect("plan fixture");
    let plan_digest = format!("{:x}", Sha256::digest(plan_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m2-s2",
                json!({
                    "path":"graph.json",
                    "sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "evidence":"stale graph digest"
                }),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m2-s2",
                json!({"path":"plan.md","sha256":plan_digest,"evidence":"matching plan"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&[])
        .expect("empty responses");

    let output = fixture.run();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = stderr(&output);
    assert!(error.contains("digest mismatch"));
    assert!(error.contains("graph.json"));
    assert!(!error.contains("planning artifact ArtifactPath(\"plan.md\") digest mismatch"));
    assert!(
        fixture
            .harness
            .invocations()
            .expect("invocations")
            .is_empty()
    );
}

#[test]
fn newest_conforming_graph_wins() {
    let newer_graph = graph(vec![node("m3-s2", "pce", vec![])]);
    let fixture = ReadyFixture::new(&newer_graph, &[]);
    let older_graph = graph(vec![node("m3-s1", "pce", vec![])]);
    let older_bytes = serde_json::to_vec(&older_graph).expect("older graph JSON");
    fs::write(fixture.primary.join("older.json"), &older_bytes).expect("older graph fixture");
    let older_digest = format!("{:x}", Sha256::digest(&older_bytes));
    let newer_bytes = fs::read(fixture.primary.join("graph.json")).expect("newer graph bytes");
    let newer_digest = format!("{:x}", Sha256::digest(&newer_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m3-s1",
                json!({"path":"older.json","sha256":older_digest,"evidence":"older graph"}),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m3-s2",
                json!({"path":"graph.json","sha256":newer_digest,"evidence":"newer graph"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &newer_graph))
        .expect("responses");

    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m3-s2","repository":"pce"}
        ]})
    );
    assert!(!stdout_json(&output).to_string().contains("m3-s1"));
}

#[test]
fn emits_every_ready_node_in_graph_order() {
    let graph = graph(vec![
        node("m3-s3", "pce", vec![]),
        node("m1-s1", "pce", vec![]),
        node("m2-s2", "pce", vec![]),
    ]);
    let fixture = ReadyFixture::new(&graph, &[]);
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &graph))
        .expect("responses");
    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m3-s3","repository":"pce"},
            {"classification":"ready","node":"m1-s1","repository":"pce"},
            {"classification":"ready","node":"m2-s2","repository":"pce"}
        ]})
    );
}

#[test]
fn dependency_inconclusive_is_not_collapsed() {
    let graph = graph(vec![
        node("m1-s1", "pce", vec![]),
        node(
            "m1-s2",
            "pce",
            vec![json!({"id":"m1-s1","reason":"authority required"})],
        ),
    ]);
    let fixture = ReadyFixture::new(&graph, &[]);
    fixture
        .harness
        .materialize_responses(&responses(
            &fixture,
            &[
                (
                    "m1-s1",
                    "pce",
                    "pce/computed/m1-s1",
                    "pce/computed/milestone-1",
                    Authority::Unreachable,
                ),
                (
                    "m1-s2",
                    "pce",
                    "pce/computed/m1-s2",
                    "pce/computed/milestone-1",
                    Authority::NotMerged,
                ),
            ],
        ))
        .expect("responses");
    let output = fixture.run();
    assert_success(&output);
    let classification = stdout_json(&output)["results"][1]["classification"]
        .as_str()
        .expect("classification")
        .to_owned();
    assert_eq!(classification, "dependency-inconclusive");
    assert_ne!(classification, "ready");
    assert_ne!(classification, "waiting");
}

#[test]
fn malformed_matching_graph_fails_before_adapters() {
    for graph in [
        json!({"nodes":[{"id":"m1-s1","title":"x","repo":"pce","depends_on":[{"id":"m1-s1","reason":""}],"summary":"x"}]}),
        json!({"nodes":[{"id":"m1-s1","title":"x","repo":"pce","depends_on":[{"id":"m1-s2","reason":"missing"}],"summary":"x"}]}),
        json!({"nodes":[{"id":"m1-s1","title":"x","repo":"pce","depends_on":[],"summary":"x","extra":true}]}),
    ] {
        let fixture = ReadyFixture::new(&graph, &[]);
        let graph_bytes = fs::read(fixture.primary.join("graph.json")).expect("graph bytes");
        let graph_digest = format!("{:x}", Sha256::digest(&graph_bytes));
        let plan_bytes = b"# not a graph\n";
        fs::write(fixture.primary.join("plan.md"), plan_bytes).expect("plan fixture");
        let plan_digest = format!("{:x}", Sha256::digest(plan_bytes));
        write_records(
            &fixture.log,
            &[
                record(
                    1,
                    "repository-contract",
                    "m1-s1",
                    repository_contract("pce", &fixture.primary),
                ),
                record(
                    2,
                    "repository-contract",
                    "m1-s1",
                    repository_contract("docs", &fixture.docs),
                ),
                record(
                    3,
                    "planning-artifact-approved",
                    "m1-s1",
                    json!({"path":"graph.json","sha256":graph_digest,"evidence":"malformed graph"}),
                ),
                record(
                    4,
                    "planning-artifact-approved",
                    "m1-s1",
                    json!({"path":"plan.md","sha256":plan_digest,"evidence":"non-graph"}),
                ),
            ],
        );
        fixture
            .harness
            .materialize_responses(&[])
            .expect("empty responses");
        let output = fixture.run();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("conforming graph"));
        assert!(stderr(&output).contains("no approved artifact is a conforming graph"));
        assert_ne!(output.stdout, br#"{"results":[]}"#);
        assert!(
            fixture
                .harness
                .invocations()
                .expect("invocations")
                .is_empty()
        );
    }
}

#[test]
fn in_flight_execution_is_filtered_but_still_observed() {
    let graph = graph(vec![
        node("m2-s1", "pce", vec![]),
        node("m2-s2", "pce", vec![]),
    ]);
    let fixture = ReadyFixture::new(
        &graph,
        &[
            ("m2-s1", "step-executor"),
            ("m2-s2", "step-plan-writer"),
            ("m2-s2", "step-critic"),
        ],
    );
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &graph))
        .expect("responses");
    let output = fixture.run();
    assert_success(&output);
    assert_eq!(
        stdout_json(&output),
        json!({"results":[
            {"classification":"ready","node":"m2-s2","repository":"pce"}
        ]})
    );
    let invocations = fixture.harness.invocations().expect("invocations");
    assert!(invocations.iter().any(|call| {
        call.program == OsStr::new("gh") && call.argv.iter().any(|arg| arg == "pce/computed/m2-s1")
    }));
}

#[test]
fn every_approved_artifact_path_is_observed() {
    let graph = graph(vec![node("m1-s1", "pce", vec![])]);
    let fixture = ReadyFixture::new(&graph, &[]);
    let earlier = fixture.primary.join("earlier.json");
    fs::write(&earlier, b"earlier artifact").expect("earlier artifact");
    let earlier_digest = format!("{:x}", Sha256::digest(b"earlier artifact"));
    let graph_bytes = fs::read(fixture.primary.join("graph.json")).expect("graph bytes");
    let graph_digest = format!("{:x}", Sha256::digest(&graph_bytes));
    write_records(
        &fixture.log,
        &[
            record(
                1,
                "repository-contract",
                "m1-s1",
                repository_contract("pce", &fixture.primary),
            ),
            record(
                2,
                "repository-contract",
                "m1-s1",
                repository_contract("docs", &fixture.docs),
            ),
            record(
                3,
                "planning-artifact-approved",
                "m1-s1",
                json!({"path":"earlier.json","sha256":earlier_digest,"evidence":"earlier"}),
            ),
            record(
                4,
                "planning-artifact-approved",
                "m1-s1",
                json!({"path":"graph.json","sha256":graph_digest,"evidence":"selected"}),
            ),
        ],
    );
    fixture
        .harness
        .materialize_responses(&all_not_merged(&fixture, &graph))
        .expect("responses");
    let output = fixture.run();
    assert_success(&output);
    assert_eq!(stdout_json(&output)["results"][0]["node"], "m1-s1");
}

fn graph(nodes: Vec<Value>) -> Value {
    json!({ "nodes": nodes })
}

fn two_pce_node_graph() -> Value {
    graph(vec![
        node("m1-s1", "pce", vec![]),
        node("m1-s2", "pce", vec![]),
    ])
}

fn node(id: &str, repository: &str, depends_on: Vec<Value>) -> Value {
    json!({
        "id": id,
        "title": id,
        "repo": repository,
        "depends_on": depends_on,
        "summary": id
    })
}

fn repository_contract(repository: &str, root: &Path) -> Value {
    repository_contract_with_policy(repository, root, "NONE")
}

fn repository_contract_with_policy(repository: &str, root: &Path, policy: &str) -> Value {
    json!({
        "repository": repository,
        "repo_root": root.to_str().expect("UTF-8 root"),
        "stated": {
            "format": "cargo fmt --check",
            "lint": "cargo clippy --workspace --all-targets",
            "typecheck": "cargo check --workspace --all-targets",
            "test": "cargo test --workspace",
            "build": "cargo build --release",
            "version_policy": policy,
            "branch_convention": "pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>",
            "pull_request_convention": "step head targets the matching milestone integration branch"
        },
        "observations": {
            "format": 0,
            "lint": 0,
            "typecheck": 0,
            "test": 0,
            "build": 0
        },
        "workflow_map": {},
        "appendable": {
            "environment_hazards": [],
            "gate_orderings": [],
            "lockfile_rules": []
        },
        "evidence": "fixture"
    })
}

fn legacy_repository_contract(repository: &str, root: &Path) -> Value {
    json!({
        "repository": repository,
        "repo_root": root.to_str().expect("UTF-8 root"),
        "stack": "Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)",
        "format": "cargo fmt --all --check",
        "lint": "cargo clippy --workspace --all-targets",
        "typecheck": "cargo check --workspace --all-targets",
        "test": "cargo test --workspace",
        "build": "cargo build --workspace",
        "preflight": "cargo check --workspace --all-targets",
        "gates_rule": "From the repo root, all four gates must exit zero before committing.",
        "install": "None required for gates.",
        "evidence": "rustc --version\ncargo --version\ngit rev-parse --show-toplevel"
    })
}

fn tracked_contract_none() -> &'static [u8] {
    br#"{
  "stated": {
    "gates": {
      "format": "cargo fmt --check",
      "lint": "cargo clippy --workspace --all-targets",
      "typecheck": "cargo check --workspace --all-targets",
      "test": "cargo test --workspace",
      "build": "cargo build --release"
    },
    "version_policy": "NONE",
    "branches": {
      "default": "main",
      "milestone": "pce/{vision}/milestone-{milestone}",
      "step": "pce/{vision}/m{milestone}-s{step}"
    },
    "pull_requests": {
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    },
    "workflows": []
  },
  "appendable": {
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }
}"#
}

fn tracked_contract_responses(
    root: &Path,
    show_exit_code: i32,
    show_output: &[u8],
) -> Vec<ScriptedResponse> {
    vec![
        response(
            "git",
            git_args(root, &["symbolic-ref", "refs/remotes/origin/HEAD"]),
            0,
            b"refs/remotes/origin/main\n",
        ),
        if show_exit_code == 0 {
            response(
                "git",
                git_args(root, &["show", "main:.pce/repository-contract.json"]),
                0,
                show_output,
            )
        } else {
            response_with_stderr(
                "git",
                git_args(root, &["show", "main:.pce/repository-contract.json"]),
                show_exit_code,
                b"",
                show_output,
            )
        },
    ]
}

fn record(sequence: u64, kind: &str, node: &str, payload: Value) -> Value {
    json!({
        "sequence": sequence,
        "timestamp": format!("2026-07-28T12:00:{sequence:02}.000Z"),
        "kind": kind,
        "node": node,
        "payload": payload
    })
}

fn write_records(path: &Path, records: &[Value]) {
    let mut bytes = Vec::new();
    for record in records {
        bytes.extend(serde_json::to_vec(record).expect("record JSON"));
        bytes.push(b'\n');
    }
    fs::write(path, bytes).expect("event log");
}

fn all_not_merged(fixture: &ReadyFixture, graph: &Value) -> Vec<ScriptedResponse> {
    let nodes = graph["nodes"].as_array().expect("nodes");
    let observations = nodes
        .iter()
        .map(|node| {
            let id = node["id"].as_str().expect("id");
            let repository = node["repo"].as_str().expect("repository");
            if id.contains("-s") {
                let milestone = id.split_once("-s").expect("step").0.trim_start_matches('m');
                (
                    id.to_owned(),
                    repository.to_owned(),
                    format!("pce/computed/{id}"),
                    format!("pce/computed/milestone-{milestone}"),
                    Authority::NotMerged,
                )
            } else {
                (
                    id.to_owned(),
                    repository.to_owned(),
                    format!("pce/computed/milestone-{}", id.trim_start_matches('m')),
                    "main".to_owned(),
                    Authority::NotMerged,
                )
            }
        })
        .collect::<Vec<_>>();
    let borrowed = observations
        .iter()
        .map(|(id, repository, head, base, authority)| {
            (
                id.as_str(),
                repository.as_str(),
                head.as_str(),
                base.as_str(),
                *authority,
            )
        })
        .collect::<Vec<_>>();
    responses(fixture, &borrowed)
}

fn responses(
    fixture: &ReadyFixture,
    observations: &[(&str, &str, &str, &str, Authority)],
) -> Vec<ScriptedResponse> {
    let mut responses = Vec::new();
    let mut seen = BTreeSet::<Vec<OsString>>::new();
    for (_, repository, head, base, authority) in observations {
        let root = fixture.root(repository);
        add_unique(
            &mut responses,
            &mut seen,
            response(
                "git",
                git_args(root, &["remote", "get-url", "origin"]),
                0,
                b"origin\n",
            ),
        );
        add_unique(
            &mut responses,
            &mut seen,
            response(
                "git",
                git_args(
                    root,
                    &[
                        "fetch",
                        "--no-tags",
                        "origin",
                        &format!("refs/heads/{base}"),
                    ],
                ),
                0,
                b"",
            ),
        );
        add_unique(
            &mut responses,
            &mut seen,
            response(
                "git",
                git_args(root, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"]),
                0,
                b"fetched-oid\n",
            ),
        );
        add_unique(
            &mut responses,
            &mut seen,
            response(
                "git",
                git_args(
                    root,
                    &[
                        "show-ref",
                        "--verify",
                        "--quiet",
                        &format!("refs/heads/{base}"),
                    ],
                ),
                1,
                b"",
            ),
        );
        add_unique(
            &mut responses,
            &mut seen,
            response(
                "git",
                git_args(root, &["worktree", "list", "--porcelain"]),
                0,
                format!(
                    "worktree {}\nHEAD fixture\nbranch refs/heads/main\n",
                    root.display()
                )
                .as_bytes(),
            ),
        );
        add_unique(
            &mut responses,
            &mut seen,
            response(
                "git",
                git_args(
                    root,
                    &["rev-parse", "--verify", "--quiet", "refs/tags/v0.1.16^{}"],
                ),
                1,
                b"",
            ),
        );

        let gh_argv = os_args(&[
            "pr",
            "list",
            "--head",
            head,
            "--base",
            base,
            "--state",
            "all",
            "--limit",
            "1000",
            "--json",
            "number,headRefName,baseRefName,state,mergeCommit",
        ]);
        match authority {
            Authority::NotMerged => add_unique(
                &mut responses,
                &mut seen,
                response("gh", gh_argv, 0, b"[]\n"),
            ),
            Authority::Unreachable => add_unique(
                &mut responses,
                &mut seen,
                response_with_stderr("gh", gh_argv, 42, b"", b"unreachable\n"),
            ),
            Authority::Merged => {
                let oid = format!("merged-{}", head.replace('/', "-"));
                let stdout = serde_json::to_vec(&json!([{
                    "number": 1,
                    "headRefName": head,
                    "baseRefName": base,
                    "state": "MERGED",
                    "mergeCommit": {"oid": oid}
                }]))
                .expect("gh JSON");
                add_unique(
                    &mut responses,
                    &mut seen,
                    response("gh", gh_argv, 0, &stdout),
                );
                add_unique(
                    &mut responses,
                    &mut seen,
                    response(
                        "git",
                        git_args(root, &["merge-base", "--is-ancestor", &oid, "fetched-oid"]),
                        0,
                        b"",
                    ),
                );
            }
        }
    }
    responses
}

fn add_unique(
    responses: &mut Vec<ScriptedResponse>,
    seen: &mut BTreeSet<Vec<OsString>>,
    response: ScriptedResponse,
) {
    let mut key = vec![response.program.clone()];
    key.extend(response.argv.clone());
    if seen.insert(key) {
        responses.push(response);
    }
}

fn response(program: &str, argv: Vec<OsString>, exit_code: i32, stdout: &[u8]) -> ScriptedResponse {
    response_with_stderr(program, argv, exit_code, stdout, b"")
}

fn response_with_stderr(
    program: &str,
    argv: Vec<OsString>,
    exit_code: i32,
    stdout: &[u8],
    stderr: &[u8],
) -> ScriptedResponse {
    ScriptedResponse {
        program: program.into(),
        argv,
        exit_code,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    }
}

fn expected_not_merged_invocations(root: &Path, head: &str, base: &str) -> Vec<Invocation> {
    vec![
        invocation("git", git_args(root, &["remote", "get-url", "origin"])),
        invocation(
            "git",
            git_args(
                root,
                &[
                    "fetch",
                    "--no-tags",
                    "origin",
                    &format!("refs/heads/{base}"),
                ],
            ),
        ),
        invocation(
            "git",
            git_args(root, &["rev-parse", "--verify", "FETCH_HEAD^{commit}"]),
        ),
        invocation(
            "git",
            git_args(
                root,
                &[
                    "show-ref",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{base}"),
                ],
            ),
        ),
        invocation("git", git_args(root, &["worktree", "list", "--porcelain"])),
        invocation(
            "git",
            git_args(
                root,
                &["rev-parse", "--verify", "--quiet", "refs/tags/v0.1.16^{}"],
            ),
        ),
        invocation(
            "gh",
            os_args(&[
                "pr",
                "list",
                "--head",
                head,
                "--base",
                base,
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
    ]
}

fn invocation(program: &str, argv: Vec<OsString>) -> Invocation {
    Invocation {
        program: program.into(),
        argv,
    }
}

fn git_args(root: &Path, args: &[&str]) -> Vec<OsString> {
    let mut result = vec![OsString::from("-C"), root.as_os_str().to_owned()];
    result.extend(args.iter().map(OsString::from));
    result
}

fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(OsString::from).collect()
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "ready failed with stderr: {}",
        stderr(output)
    );
}

fn stdout_json(output: &std::process::Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout JSON")
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
