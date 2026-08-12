//! fixture_replay : CommittedLegacyEvents × CommittedSidecars → HistoricalSpendingState.

use std::fs;
use std::path::{Path, PathBuf};

use pce_core::{
    AbsoluteRequiredArtifactPath, DispatchAdmission, DispatchRequiredArtifactObservation,
    DispatchRole, EventBodyRef, KnownPayload, NodeId, NonProductionKey, Sequence,
    classify_dispatch_admission, derive_dispatch_outcome_state, parse_event_line,
    serialize_event_line,
};
use serde::Deserialize;

const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/pre-repair-orchestration"
);

#[derive(Debug, Deserialize)]
struct ArtifactObservationFixture {
    issuance_sequence: u64,
    required_artifact_path: PathBuf,
}

fn fixture_records(name: &str) -> (String, Vec<pce_core::EventRecord>) {
    let bytes = fs::read_to_string(Path::new(FIXTURE_ROOT).join(name)).expect("read committed log");
    let records = bytes
        .lines()
        .map(|line| parse_event_line(line).expect("parse committed legacy record"))
        .collect();
    (bytes, records)
}

fn fixture_observations(name: &str) -> Vec<DispatchRequiredArtifactObservation> {
    let bytes = fs::read(Path::new(FIXTURE_ROOT).join(name)).expect("read committed sidecars");
    serde_json::from_slice::<Vec<ArtifactObservationFixture>>(&bytes)
        .expect("parse committed sidecars")
        .into_iter()
        .map(|observation| {
            DispatchRequiredArtifactObservation::new(
                Sequence::parse(observation.issuance_sequence).expect("positive issuance sequence"),
                AbsoluteRequiredArtifactPath::parse(observation.required_artifact_path)
                    .expect("absolute historical identity"),
            )
        })
        .collect()
}

fn assert_canonical_legacy_bytes(bytes: &str, records: &[pce_core::EventRecord]) {
    let physical_lines = bytes.lines().collect::<Vec<_>>();
    assert_eq!(physical_lines.len(), records.len());
    assert!(bytes.ends_with('\n'));
    for (line, record) in physical_lines.iter().zip(records) {
        assert_eq!(
            serialize_event_line(record).expect("serialize legacy record"),
            *line,
            "legacy wire bytes changed at sequence {}",
            record.sequence().get()
        );
    }
}

fn key(node: &str, role: &str, artifact: &str) -> NonProductionKey {
    NonProductionKey {
        node: NodeId::parse(node).expect("fixture node"),
        role: DispatchRole::new(role),
        required_artifact_path: AbsoluteRequiredArtifactPath::parse(artifact)
            .expect("absolute fixture artifact identity"),
    }
}

#[test]
fn pre_repair_legacy_records_round_trip_and_recover_original_round_meaning() {
    let (bytes, records) = fixture_records("pre-repair-events.jsonl");
    let observations = fixture_observations("pre-repair-artifact-observations.json");
    assert_canonical_legacy_bytes(&bytes, &records);
    assert_eq!(records.len(), 24);
    assert_eq!(observations.len(), 12);

    let completions = records
        .iter()
        .filter(|record| {
            matches!(
                record.body_ref(),
                EventBodyRef::Known(KnownPayload::DispatchCompletion(_))
            )
        })
        .count();
    assert_eq!(completions, 12);
    assert_eq!(bytes.matches("root_cause").count(), 0);

    let state = derive_dispatch_outcome_state(&records, &observations)
        .expect("derive historical state from committed fixture");
    let expected = [
        (
            "m1-s2",
            "step-executor",
            "/historical/pce/m1-s2/execution-result.json",
        ),
        (
            "m3-s4",
            "step-executor",
            "/historical/pce/m3-s4/execution-result.json",
        ),
        (
            "m3-s9",
            "step-plan-critic",
            "/historical/pce/m3-s9/review.json",
        ),
        (
            "m3-s11",
            "step-executor",
            "/historical/pce/m3-s11/execution-result.json",
        ),
    ];
    assert_eq!(state.validated_production_counts().len(), expected.len());
    for ((node, role, artifact), series) in expected.iter().zip(state.validated_production_counts())
    {
        assert_eq!(series.node().as_str(), *node);
        assert_eq!(series.role().as_str(), *role);
        assert_eq!(
            series.count().get(),
            3,
            "legacy completion must charge its reporting role"
        );
        assert_eq!(
            classify_dispatch_admission(&state, &key(node, role, artifact)),
            DispatchAdmission::Admit,
            "count 3 remains below the automatic limit 12"
        );
    }
}

#[test]
fn spending_limit_parks_and_one_human_recorded_resume_is_consumed_exactly_once() {
    let (bytes, records) = fixture_records("spending-limit-resume-events.jsonl");
    let observations = fixture_observations("spending-limit-resume-artifact-observations.json");
    assert_canonical_legacy_bytes(&bytes, &records);
    assert_eq!(records.len(), 27);
    assert_eq!(observations.len(), 13);

    let route = key(
        "m1-s2",
        "step-executor",
        "/historical/pce/m1-s2/execution-result.json",
    );
    let parked = derive_dispatch_outcome_state(&records[..24], &observations[..12])
        .expect("derive state at automatic spending limit");
    assert_eq!(parked.validated_production_counts()[0].count().get(), 12);
    assert!(matches!(
        classify_dispatch_admission(&parked, &route),
        DispatchAdmission::OpenNonProductionHold { consecutive } if consecutive.get() == 0
    ));

    let authorization = derive_dispatch_outcome_state(&records[..26], &observations[..12])
        .expect("derive recorded human authorization");
    assert_eq!(
        classify_dispatch_admission(&authorization, &route),
        DispatchAdmission::Admit
    );

    let resumed = derive_dispatch_outcome_state(&records, &observations)
        .expect("derive state after the authorized issuance");
    assert_eq!(resumed.validated_production_counts()[0].count().get(), 12);
    assert!(matches!(
        classify_dispatch_admission(&resumed, &route),
        DispatchAdmission::OpenNonProductionHold { consecutive } if consecutive.get() == 0
    ));

    let (opens, closes) =
        records
            .iter()
            .fold((0, 0), |(opens, closes), record| match record.body_ref() {
                EventBodyRef::Known(KnownPayload::EscalationOpen(_)) => (opens + 1, closes),
                EventBodyRef::Known(KnownPayload::EscalationClose(_)) => (opens, closes + 1),
                _ => (opens, closes),
            });
    assert_eq!((opens, closes), (1, 1));
    assert_eq!(resumed.issuance_ordinals()[0].ordinal().get(), 13);
}
