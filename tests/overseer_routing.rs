use std::cell::Cell;

use chrono::{TimeZone, Utc};
use pce_core::{
    DoorKind, EventTimestamp, HoldIdentity, HoldRecord, HoldRoute, HoldState, HoldStore,
    InstallDecision, InstallRequest, OverseerRecord, ProposedRoutingRule, RouteDecision,
    RoutingRuleAction, RoutingRuleClass, RuleAdmission, RuleRefusal, admit_routing_rule,
    fold_dispatch_ledger, overseer_records, parse_event_line, request_install, route_hold,
};
use serde_json::json;
use tempfile::tempdir;

fn at(second: u32) -> EventTimestamp {
    EventTimestamp::new(
        Utc.with_ymd_and_hms(2026, 8, 21, 12, 0, second)
            .single()
            .expect("fixture timestamp"),
    )
}

fn open(
    store: &HoldStore,
    package: &str,
    kind: &str,
    report: serde_json::Value,
) -> pce_core::HoldKey {
    store
        .open(
            HoldIdentity::parse("pce", "v1", package, kind).expect("identity"),
            report,
            at(0),
        )
        .expect("open hold")
        .hold()
        .key()
        .clone()
}

#[test]
fn a_door_hold_refuses_a_machine_route() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path());
    let rules = store.routing_rules().expect("starting rules");
    assert!(rules.iter().any(|rule| {
        rule.discriminating_fact()
            .contains("frozen acceptance criterion")
    }));
    let key = open(
        &store,
        "OQ5",
        "criterion-revision",
        json!({"report":"The proposed change alters a frozen acceptance criterion"}),
    );

    let decision = route_hold(&store, &key, HoldRoute::Overseer, at(1)).expect("route decision");
    assert!(matches!(
        decision,
        RouteDecision::RefusedDoor {
            door: DoorKind::CriterionRevision,
            ..
        }
    ));
    assert_eq!(
        decision.hold().state(),
        HoldState::Open {
            route: HoldRoute::Human
        }
    );
    assert!(
        decision
            .hold()
            .records()
            .iter()
            .all(|record| { !matches!(record, HoldRecord::Answered { by, .. } if by != "human") })
    );
    assert!(decision.hold().records().iter().any(|record| {
        matches!(
            record,
            HoldRecord::RouteRefused {
                requested: HoldRoute::Overseer,
                enforced: HoldRoute::Human,
                ..
            }
        )
    }));
}

#[test]
fn rule_admission_refuses_an_ambiguous_replay() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path());
    let first = open(
        &store,
        "OQ5-a",
        "routing-fact",
        json!({
            "symptom":"base currency unavailable",
            "cause":"origin HEAD is missing"
        }),
    );
    let second = open(
        &store,
        "OQ5-b",
        "routing-fact",
        json!({
            "symptom":"base currency unavailable",
            "cause":"intentional historical offline ref"
        }),
    );
    store
        .answer(&first, "human".to_owned(), "reject".to_owned(), at(1))
        .expect("first answer");
    store
        .answer(
            &second,
            "human".to_owned(),
            "accept the risk".to_owned(),
            at(1),
        )
        .expect("second answer");
    let before = store.routing_rules().expect("rules before").len();
    let proposal = ProposedRoutingRule::parse(
        "learned-base-currency",
        RoutingRuleClass::RunLocalDecision,
        RoutingRuleAction::ReturnToRun,
        "base currency unavailable",
        "reuse the first answer",
        "reject",
    )
    .expect("proposal");

    let admission = admit_routing_rule(&store, &proposal, at(2)).expect("admission");
    assert!(matches!(
        admission,
        RuleAdmission::Refused(RuleRefusal::AmbiguousReplay { ref matching_holds })
            if matching_holds.len() == 2
    ));
    assert_eq!(store.routing_rules().expect("rules after").len(), before);
}

#[test]
fn an_optionless_report_is_returned() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path());
    let key = open(
        &store,
        "OQ5",
        "operator-choice",
        json!({"question":"Which way should this run go?"}),
    );

    let decision = route_hold(&store, &key, HoldRoute::Human, at(1)).expect("route decision");
    assert!(matches!(decision, RouteDecision::ReturnedForOptions { .. }));
    assert_eq!(
        decision.hold().state(),
        HoldState::Open {
            route: HoldRoute::ReportingRun
        }
    );
    assert!(decision.hold().records().iter().any(|record| {
        matches!(record, HoldRecord::ReportingRunRequest { request, .. }
            if request.contains("Name each option"))
    }));
}

fn event(sequence: u64, kind: &str, payload: &str) -> pce_core::EventRecord {
    parse_event_line(&format!(
        "{{\"sequence\":{sequence},\"timestamp\":\"2026-08-21T12:00:00.000Z\",\"kind\":\"{kind}\",\"node\":\"OQ5\",\"payload\":{payload}}}"
    ))
    .expect("ledger event")
}

#[test]
fn an_install_waits_for_a_quiet_fleet() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path());
    let request = InstallRequest::parse("install-0.1.17", "0.1.17").expect("request");
    let issuance = event(
        1,
        "dispatch",
        r#"{"role":"step-executor","ref":"abc123","evidence":"fixture"}"#,
    );
    let live = fold_dispatch_ledger(std::slice::from_ref(&issuance)).expect("live ledger");
    let installs = Cell::new(0);
    let waiting = request_install(&store, &request, &live, at(1), || {
        installs.set(installs.get() + 1);
        Ok(())
    })
    .expect("waiting decision");
    assert_eq!(waiting, InstallDecision::WaitingForQuietFleet);
    assert_eq!(installs.get(), 0);
    assert!(matches!(
        overseer_records(&store).expect("request record").as_slice(),
        [OverseerRecord::InstallRequested { .. }]
    ));

    let completion = event(
        2,
        "dispatch-completion",
        r#"{"issuance_sequence":1,"duration_ms":20,"usage":{"availability":"measured","input_tokens":1,"cached_input_tokens":0,"output_tokens":1,"reasoning_output_tokens":0},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}"#,
    );
    let quiet = fold_dispatch_ledger(&[issuance, completion]).expect("quiet ledger");
    let installed = request_install(&store, &request, &quiet, at(2), || {
        installs.set(installs.get() + 1);
        Ok(())
    })
    .expect("install decision");
    assert_eq!(installed, InstallDecision::Installed);
    assert_eq!(installs.get(), 1);
    assert!(matches!(
        overseer_records(&store)
            .expect("install records")
            .as_slice(),
        [
            OverseerRecord::InstallRequested { .. },
            OverseerRecord::InstallCompleted { .. }
        ]
    ));
}

#[test]
fn a_rule_may_not_be_learned_for_a_ruling() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path());
    for (package, second) in [("OQ5-a", 1), ("OQ5-b", 2)] {
        let key = open(
            &store,
            package,
            "criterion-revision",
            json!({"fact":"frozen criterion wording must change"}),
        );
        store
            .answer(&key, "Nicolas".to_owned(), "approve".to_owned(), at(second))
            .expect("human answer");
    }
    let before = store.routing_rules().expect("rules before").len();
    let proposal = ProposedRoutingRule::parse(
        "learned-criterion-approval",
        RoutingRuleClass::RunLocalDecision,
        RoutingRuleAction::ReturnToRun,
        "frozen criterion wording",
        "approve automatically",
        "approve",
    )
    .expect("proposal");

    let admission = admit_routing_rule(&store, &proposal, at(3)).expect("admission");
    assert_eq!(
        admission,
        RuleAdmission::Refused(RuleRefusal::SubjectIsRuling {
            door: DoorKind::CriterionRevision
        })
    );
    assert_eq!(store.routing_rules().expect("rules after").len(), before);
    assert!(
        overseer_records(&store)
            .expect("records")
            .iter()
            .any(|record| {
                matches!(record, OverseerRecord::RuleAdmissionRefused { reason, .. }
            if reason.contains("criterion-revision"))
            })
    );
}
