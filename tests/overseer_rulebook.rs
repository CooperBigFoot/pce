use std::collections::HashSet;
use std::fs;

use pce_core::{HoldStore, RoutingRuleClass, RoutingRuleOrigin, initial_routing_rules};
use tempfile::tempdir;

#[test]
fn rulebook_arrives_loaded() {
    let directory = tempdir().expect("temporary store should create");
    let store = HoldStore::new(directory.path());

    let rules = store
        .routing_rules()
        .expect("first query should initialize and read the rulebook");
    let recorded: Vec<_> = rules
        .iter()
        .filter(|rule| rule.origin() == RoutingRuleOrigin::RecordedRuling)
        .collect();
    let defect_classes: Vec<_> = rules
        .iter()
        .filter(|rule| rule.origin() == RoutingRuleOrigin::NamedDefectClass)
        .collect();

    assert_eq!(recorded.len(), 19, "all classified rulings must ship");
    for (classification, expected) in [
        (RoutingRuleClass::HumanRuling, 4),
        (RoutingRuleClass::RunLocalDecision, 9),
        (RoutingRuleClass::FleetFact, 4),
        (RoutingRuleClass::BinaryDefect, 2),
    ] {
        assert_eq!(
            recorded
                .iter()
                .filter(|rule| rule.classification() == classification)
                .count(),
            expected,
            "recorded ruling classification changed"
        );
    }
    assert!(
        !defect_classes.is_empty(),
        "the corpus's named recurring defect classes must ship"
    );
    assert!(
        rules
            .iter()
            .all(|rule| !rule.discriminating_fact().trim().is_empty()),
        "every rule must retain its discriminating fact"
    );
    let ids: HashSet<_> = rules.iter().map(|rule| rule.id()).collect();
    assert_eq!(ids.len(), rules.len(), "routing rule ids must be unique");
    for required in [
        "fact-another-run-push",
        "fact-another-run-launch-spelling",
        "fact-installed-binary-surface",
        "fact-external-daemon-journal",
        "defect-absent-binary-verb",
        "defect-class-rubber-stamp",
        "defect-class-inert-rule",
        "defect-class-unreachable-success",
        "defect-class-wrong-baseline",
    ] {
        assert!(
            ids.contains(required),
            "starting rule `{required}` is absent"
        );
    }

    let path = directory.path().join("rulebook/routing-rules.jsonl");
    let first_bytes = fs::read(&path).expect("installed rulebook should be retained");
    let queried_again = store
        .routing_rules()
        .expect("installed rulebook should query");
    assert_eq!(queried_again, rules);
    assert_eq!(
        fs::read(path).expect("retained rulebook should read"),
        first_bytes,
        "querying an initialized store must not rewrite or duplicate records"
    );
}

#[test]
fn interrupted_rulebook_install_recovers_incomplete_tail() {
    let directory = tempdir().expect("temporary store should create");
    let rulebook_directory = directory.path().join("rulebook");
    fs::create_dir(&rulebook_directory).expect("rulebook directory should create");
    let path = rulebook_directory.join("routing-rules.jsonl");
    let expected = initial_routing_rules();
    let mut interrupted = serde_json::to_vec(&expected[0]).expect("first rule should encode");
    interrupted.push(b'\n');
    let second = serde_json::to_vec(&expected[1]).expect("second rule should encode");
    interrupted.extend_from_slice(&second[..second.len() / 2]);
    fs::write(&path, interrupted).expect("interrupted install should seed");

    let loaded = HoldStore::new(directory.path())
        .routing_rules()
        .expect("a valid prefix with an incomplete tail should resume installation");

    assert_eq!(loaded, expected);
    let installed = fs::read(path).expect("recovered rulebook should read");
    assert!(installed.ends_with(b"\n"));
    assert_eq!(
        installed.iter().filter(|byte| **byte == b'\n').count(),
        loaded.len()
    );
}
