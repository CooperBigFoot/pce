#![allow(clippy::expect_used, clippy::unwrap_used)]

use pce_core::{
    AmendmentRepositoryRefs, CommandExitStatus, CriterionExecution, CriterionOrigin, DriverEvent,
    FindingRejectionReason, FindingReplayDecision, parse_work_package_graph, render_package_run,
};

fn execution(command: &str, code: i32, stdout: &str, stderr: &str) -> CriterionExecution {
    CriterionExecution::new(
        command.to_owned(),
        "/tmp/work".to_owned(),
        CommandExitStatus::Exited { code },
        stdout.to_owned(),
        stderr.to_owned(),
    )
}

fn graph() -> pce_core::WorkPackageGraph {
    parse_work_package_graph(br#"{
      "vision":"render-test","plan_version":1,"authored_at_ref":"abc123","packages":[
        {"id":"A","title":"Foundation","repositories":["repo"],"criteria":[{"name":"tests","input":"tree","observation":"passes","command":"cargo test"}],"depends_on":[]},
        {"id":"B","title":"Binding leaf","repositories":["repo"],"criteria":[{"name":"lint","input":"tree","observation":"clean","command":"cargo clippy"}],"depends_on":[{"id":"A","kind":"buildability","reason":"uses A"}]},
        {"id":"C","title":"Safe leaf","repositories":["repo"],"criteria":[{"name":"safe","input":"tree","observation":"safe","command":"./safe"}],"depends_on":[{"id":"B","kind":"safety","reason":"must validate B"}]},
        {"id":"D","title":"Choice leaf","repositories":["repo"],"criteria":[{"name":"risk","input":"tree","observation":"known","command":"./risk"}],"depends_on":[{"id":"A","kind":"risk-ordering","reason":"learn first"}]}
      ]
    }"#).expect("valid graph")
}

#[test]
fn unrun_graph_is_deterministic_structure_with_typed_edges_and_critical_path() {
    let graph = graph();
    let first = render_package_run(&graph, &[]).expect("render");
    let second = render_package_run(&graph, &[]).expect("render again");

    assert_eq!(first.as_bytes(), second.as_bytes());
    for package in ["A", "B", "C", "D"] {
        assert!(first.contains(&format!("data-package=\"{package}\"")));
    }
    for kind in ["buildability", "safety", "risk-ordering"] {
        assert!(first.contains(&format!("data-edge-kind=\"{kind}\"")));
    }
    assert!(first.contains("data-critical=\"true\" data-package=\"A\""));
    assert!(first.contains("data-critical=\"true\" data-package=\"B\""));
    assert!(first.contains("data-critical=\"true\" data-package=\"C\""));
    assert!(first.contains("data-edge-kind=\"buildability\" data-critical=\"true\""));
    assert!(first.contains("data-edge-kind=\"safety\" data-critical=\"true\""));
    assert!(first.contains("Overridable choice"));
    assert!(!first.contains("class=\"state-badge"));
    assert!(!first.contains("://"));
}

#[test]
fn stopped_run_keeps_failure_output_amendment_and_rejected_finding() {
    let graph = graph();
    let refs = vec![AmendmentRepositoryRefs {
        repository: "repo".to_owned(),
        witness_ref: "bad123".to_owned(),
        repair_ref: "fix456".to_owned(),
    }];
    let events = vec![
        DriverEvent::WorkerDispatched {
            package: "A".to_owned(),
            issuance: 1,
        },
        DriverEvent::WorkerDone {
            package: "A".to_owned(),
            issuance: 1,
        },
        DriverEvent::CriterionExecuted {
            package: "A".to_owned(),
            name: "tests".to_owned(),
            origin: CriterionOrigin::Authored,
            execution: execution(
                "cargo test",
                7,
                "one test ran\n",
                "assertion exploded <here>\n",
            ),
        },
        DriverEvent::FindingReplayed {
            package: "A".to_owned(),
            gate: "falsifier-1".to_owned(),
            finding: 0,
            command: "./regression".to_owned(),
            repository_refs: refs.clone(),
            witness: execution("./regression", 1, "", "old behavior"),
            repair: execution("./regression", 0, "fixed", ""),
            decision: FindingReplayDecision::Accepted,
        },
        DriverEvent::FindingReplayed {
            package: "A".to_owned(),
            gate: "falsifier-1".to_owned(),
            finding: 1,
            command: "./false-positive".to_owned(),
            repository_refs: refs,
            witness: execution("./false-positive", 0, "already passed", ""),
            repair: execution("./false-positive", 0, "passed", ""),
            decision: FindingReplayDecision::Rejected {
                reason: FindingRejectionReason::WitnessPassed,
            },
        },
        DriverEvent::FindingRejected {
            package: "A".to_owned(),
            gate: "falsifier-1".to_owned(),
            finding: 2,
            command: "./unusable".to_owned(),
            reason: FindingRejectionReason::StructurallyMalformed,
            detail: "missing & unreachable <ref>".to_owned(),
        },
        DriverEvent::GateFinished {
            package: "A".to_owned(),
            gate: "empty-gate".to_owned(),
        },
        DriverEvent::PackageFailed {
            package: "A".to_owned(),
            reason: "criterion tests failed".to_owned(),
        },
    ];

    let html = render_package_run(&graph, &events).expect("render stopped run");
    assert!(html.contains("data-state=\"failed\""));
    assert!(html.contains("assertion exploded &lt;here&gt;"));
    assert!(html.contains("exit 7"));
    assert!(html.contains("Amendment from gate falsifier-1, finding 0"));
    assert!(html.contains("data-finding-decision=\"accepted\""));
    assert!(html.contains("data-finding-decision=\"rejected\""));
    assert!(html.contains("witness-passed"));
    assert!(html.contains("structurally-malformed"));
    assert!(html.contains("missing &amp; unreachable &lt;ref&gt;"));
    assert!(html.contains("empty-gate"));
    assert!(html.contains("No findings reported"));
    assert!(html.contains("bad123"));
}

#[test]
fn single_package_graph_has_a_nonzero_legible_canvas() {
    let graph = parse_work_package_graph(br#"{"vision":"one","plan_version":1,"authored_at_ref":"HEAD","packages":[{"id":"only","title":"Only package","repositories":["repo"],"criteria":[{"name":"proof","input":"input","observation":"works","command":"true"}],"depends_on":[]}] }"#).expect("single graph");
    let html = render_package_run(&graph, &[]).expect("single render");
    assert!(html.contains("viewBox=\"0 0 300 150\""));
    assert!(html.contains("Only package"));
}

#[test]
fn rivretrieve_fixture_draws_seven_packages_and_all_nine_dependencies() {
    let bytes = include_bytes!("data/rivretrieve-work-package-graph.json");
    let graph = parse_work_package_graph(bytes).expect("fixture graph");
    let html = render_package_run(&graph, &[]).expect("fixture render");
    let svg = html
        .split_once("<svg")
        .expect("svg start")
        .1
        .split_once("</svg>")
        .expect("svg end")
        .0;
    assert_eq!(svg.matches("data-package=\"").count(), 7);
    assert_eq!(svg.matches("data-edge-kind=\"buildability\"").count(), 6);
    assert_eq!(svg.matches("data-edge-kind=\"safety\"").count(), 2);
    assert_eq!(svg.matches("data-edge-kind=\"risk-ordering\"").count(), 1);
    for package in ["RR1", "RR2", "RR3", "RR5", "RR7"] {
        assert!(svg.contains(&format!(
            "data-critical=\"true\" data-package=\"{package}\""
        )));
    }
}

#[test]
fn layout_uses_topology_even_when_author_order_is_reversed() {
    let graph = parse_work_package_graph(br#"{"vision":"reverse","plan_version":1,"authored_at_ref":"HEAD","packages":[{"id":"B","title":"B","repositories":["r"],"criteria":[{"name":"b","input":"i","observation":"o","command":"true"}],"depends_on":[{"id":"A","kind":"buildability","reason":"A first"}]},{"id":"A","title":"A","repositories":["r"],"criteria":[{"name":"a","input":"i","observation":"o","command":"true"}],"depends_on":[]}]}"#).expect("reversed graph");
    let html = render_package_run(&graph, &[]).expect("render");
    assert!(html.contains("data-package=\"A\"><title>A</title><rect x=\"40\""));
    assert!(html.contains("data-package=\"B\"><title>B</title><rect x=\"330\""));
}
