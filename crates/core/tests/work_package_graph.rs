#![allow(clippy::expect_used, clippy::unwrap_used)]

use pce_core::{
    MergeStatus, RiskOrdering, WorkPackageClassification, WorkPackageMergeObservation,
    parse_work_package_graph, ready_work_packages,
};

const FIXTURE: &[u8] = include_bytes!("data/rivretrieve-work-package-graph.json");

fn observations(
    graph: &pce_core::WorkPackageGraph,
    merged: &[&str],
) -> Vec<WorkPackageMergeObservation> {
    graph
        .packages()
        .iter()
        .flat_map(|package| {
            package.repositories().iter().map(move |repository| {
                WorkPackageMergeObservation::new(
                    package.id().clone(),
                    repository.clone(),
                    if merged.contains(&package.id().as_str()) {
                        MergeStatus::Merged
                    } else {
                        MergeStatus::NotMerged
                    },
                )
            })
        })
        .collect()
}

fn ids(merged: &[&str], risk: RiskOrdering) -> Vec<String> {
    let graph = parse_work_package_graph(FIXTURE).expect("fixture graph should validate");
    let observations = observations(&graph, merged);
    ready_work_packages(&graph, &observations, risk)
        .expect("ready set should compute")
        .ready()
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect()
}

#[test]
fn rivretrieve_ready_sets_follow_binding_and_soft_dependencies() {
    assert_eq!(ids(&[], RiskOrdering::Honour), ["RR1"]);
    assert_eq!(ids(&["RR1"], RiskOrdering::Honour), ["RR2", "RR6"]);
    assert_eq!(ids(&["RR1", "RR2", "RR6"], RiskOrdering::Honour), ["RR3"]);
    assert_eq!(
        ids(&["RR1", "RR2", "RR3", "RR6"], RiskOrdering::Honour),
        ["RR4", "RR5"]
    );
    assert_eq!(
        ids(
            &["RR1", "RR2", "RR3", "RR4", "RR5", "RR6"],
            RiskOrdering::Honour
        ),
        ["RR7"]
    );
}

#[test]
fn risk_override_is_explicit_and_exposes_rr4_after_rr2() {
    let graph = parse_work_package_graph(FIXTURE).expect("fixture graph should validate");
    let observations = observations(&graph, &["RR2"]);
    let report = ready_work_packages(&graph, &observations, RiskOrdering::Override)
        .expect("ready set should compute");
    assert!(report.ready().iter().any(|id| id.as_str() == "RR4"));
    assert!(report.override_applied());
}

#[test]
fn omitting_rr4_safety_edge_invalidates_soft_reduction() {
    let mut value: serde_json::Value = serde_json::from_slice(FIXTURE).expect("valid JSON");
    let packages = value["packages"].as_array_mut().expect("packages array");
    let rr4 = packages.iter_mut().find(|p| p["id"] == "RR4").expect("RR4");
    rr4["depends_on"]
        .as_array_mut()
        .expect("dependencies")
        .retain(|d| d["kind"] != "safety");
    let bytes = serde_json::to_vec(&value).expect("serialize mutation");
    let error = parse_work_package_graph(&bytes).expect_err("missing hard path must fail");
    assert!(error.to_string().contains("soft dependency"));
}

#[test]
fn missing_kind_specific_justification_is_rejected() {
    for kind in ["buildability", "safety", "risk-ordering"] {
        let mut value: serde_json::Value = serde_json::from_slice(FIXTURE).expect("valid JSON");
        let dependency = value["packages"]
            .as_array_mut()
            .expect("packages")
            .iter_mut()
            .flat_map(|p| {
                p["depends_on"]
                    .as_array_mut()
                    .expect("dependencies")
                    .iter_mut()
            })
            .find(|d| d["kind"] == kind)
            .expect("edge kind");
        dependency
            .as_object_mut()
            .expect("dependency object")
            .remove("reason");
        assert!(
            parse_work_package_graph(&serde_json::to_vec(&value).expect("serialize")).is_err(),
            "{kind}"
        );
    }
}

#[test]
fn cycle_empty_criteria_and_missing_command_are_rejected() {
    let mut cycle: serde_json::Value = serde_json::from_slice(FIXTURE).expect("valid JSON");
    cycle["packages"][0]["depends_on"] = serde_json::json!([{"id":"RR7","kind":"buildability","reason":"reader imports final surface"}]);
    assert!(parse_work_package_graph(&serde_json::to_vec(&cycle).expect("serialize")).is_err());

    let mut empty: serde_json::Value = serde_json::from_slice(FIXTURE).expect("valid JSON");
    empty["packages"][0]["criteria"] = serde_json::json!([]);
    assert!(parse_work_package_graph(&serde_json::to_vec(&empty).expect("serialize")).is_err());

    let mut command: serde_json::Value = serde_json::from_slice(FIXTURE).expect("valid JSON");
    command["packages"][0]["criteria"][0]
        .as_object_mut()
        .expect("criterion")
        .remove("command");
    assert!(parse_work_package_graph(&serde_json::to_vec(&command).expect("serialize")).is_err());
}

#[test]
fn unknown_dependency_fails_loudly() {
    let mut value: serde_json::Value = serde_json::from_slice(FIXTURE).expect("valid JSON");
    value["packages"][1]["depends_on"][0]["id"] = serde_json::Value::from("MISSING");
    let error = parse_work_package_graph(&serde_json::to_vec(&value).expect("serialize"))
        .expect_err("unknown dependency must fail");
    assert!(error.to_string().contains("unknown dependency MISSING"));
}

#[test]
fn soft_then_binding_path_also_requires_a_binding_alternative() {
    let graph = serde_json::json!({
        "vision":"2026-08-11-mixed-path", "plan_version":1, "authored_at_ref":"ref",
        "packages":[
            {"id":"A","title":"A","repositories":["repo"],"criteria":[{"name":"a","input":"a","observation":"a","command":"a"}],"depends_on":[]},
            {"id":"B","title":"B","repositories":["repo"],"criteria":[{"name":"b","input":"b","observation":"b","command":"b"}],"depends_on":[{"id":"A","kind":"risk-ordering","reason":"B cheaply tests A"}]},
            {"id":"C","title":"C","repositories":["repo"],"criteria":[{"name":"c","input":"c","observation":"c","command":"c"}],"depends_on":[{"id":"B","kind":"buildability","reason":"C imports B"}]}
        ]
    });
    let error = parse_work_package_graph(&serde_json::to_vec(&graph).expect("serialize"))
        .expect_err("mixed soft path must not imply a hard edge");
    assert!(
        error
            .to_string()
            .contains("soft dependency path from A to C")
    );
}

fn aggregation_graph() -> pce_core::WorkPackageGraph {
    let graph = serde_json::json!({
        "vision":"2026-08-12-merge-observations", "plan_version":1, "authored_at_ref":"ref",
        "packages":[
            {"id":"A","title":"A","repositories":["one","two"],"criteria":[{"name":"a","input":"a","observation":"a","command":"a"}],"depends_on":[]},
            {"id":"B","title":"B","repositories":["one"],"criteria":[{"name":"b","input":"b","observation":"b","command":"b"}],"depends_on":[{"id":"A","kind":"buildability","reason":"B imports A"}]}
        ]
    });
    parse_work_package_graph(&serde_json::to_vec(&graph).expect("serialize graph"))
        .expect("aggregation graph")
}

fn observation_for(
    graph: &pce_core::WorkPackageGraph,
    package_id: &str,
    repository: &str,
    status: MergeStatus,
) -> WorkPackageMergeObservation {
    let package = graph
        .packages()
        .iter()
        .find(|package| package.id().as_str() == package_id)
        .expect("package fixture");
    WorkPackageMergeObservation::new(package.id().clone(), repository.to_owned(), status)
}

#[test]
fn inconclusive_repository_makes_package_and_dependency_inconclusive() {
    let graph = aggregation_graph();
    let observations = [
        observation_for(&graph, "A", "one", MergeStatus::Merged),
        observation_for(&graph, "A", "two", MergeStatus::Inconclusive),
        observation_for(&graph, "B", "one", MergeStatus::NotMerged),
    ];
    let report = ready_work_packages(&graph, &observations, RiskOrdering::Honour)
        .expect("readiness computes");
    let a = &report.packages()[0];
    let b = &report.packages()[1];
    assert_eq!(a.merge_status(), MergeStatus::Inconclusive);
    assert_eq!(
        a.classification(),
        WorkPackageClassification::DependencyInconclusive
    );
    assert_eq!(b.merge_status(), MergeStatus::NotMerged);
    assert_eq!(
        b.classification(),
        WorkPackageClassification::DependencyInconclusive
    );
}

#[test]
fn two_repository_aggregation_requires_every_repository_to_merge() {
    let graph = aggregation_graph();
    for (left, right, aggregate, dependent) in [
        (
            MergeStatus::Merged,
            MergeStatus::Merged,
            MergeStatus::Merged,
            WorkPackageClassification::Ready,
        ),
        (
            MergeStatus::Merged,
            MergeStatus::NotMerged,
            MergeStatus::NotMerged,
            WorkPackageClassification::Waiting,
        ),
        (
            MergeStatus::NotMerged,
            MergeStatus::Inconclusive,
            MergeStatus::Inconclusive,
            WorkPackageClassification::DependencyInconclusive,
        ),
    ] {
        let observations = [
            observation_for(&graph, "A", "one", left),
            observation_for(&graph, "A", "two", right),
            observation_for(&graph, "B", "one", MergeStatus::NotMerged),
        ];
        let report = ready_work_packages(&graph, &observations, RiskOrdering::Honour)
            .expect("readiness computes");
        assert_eq!(report.packages()[0].merge_status(), aggregate);
        assert_eq!(report.packages()[1].classification(), dependent);
    }
}

#[test]
fn merge_observations_must_cover_each_exact_package_repository_pair() {
    let graph = aggregation_graph();
    let missing = [
        observation_for(&graph, "A", "one", MergeStatus::Merged),
        observation_for(&graph, "B", "one", MergeStatus::NotMerged),
    ];
    let error = ready_work_packages(&graph, &missing, RiskOrdering::Honour)
        .expect_err("missing repository observation must fail");
    assert!(error.to_string().contains("missing merge observation"));

    let duplicate = [
        observation_for(&graph, "A", "one", MergeStatus::Merged),
        observation_for(&graph, "A", "one", MergeStatus::Merged),
        observation_for(&graph, "A", "two", MergeStatus::Merged),
        observation_for(&graph, "B", "one", MergeStatus::NotMerged),
    ];
    let error = ready_work_packages(&graph, &duplicate, RiskOrdering::Honour)
        .expect_err("duplicate repository observation must fail");
    assert!(error.to_string().contains("duplicate merge observation"));
}
