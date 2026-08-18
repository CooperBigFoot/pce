//! dispatch_check_in : Ordered<EventRecord> × Ordered<DispatchIdentityObservation> → DispatchCheckInReport ∪ DispatchCheckInError   (pure, deterministic)
//! This module correlates binary-supplied process and artifact observations; it performs no file, process, clock, or environment I/O.

use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;

use crate::dispatch_ledger::{
    DispatchAccounting, DispatchLedgerCompletion, DispatchLedgerError, fold_dispatch_ledger,
};
use crate::dispatch_process_identity::ProcessStartIdentity;
use crate::event_log::{ArtifactProduction, EventRecord, Sequence};

const SCHEMA_ID: &str = "pce.dispatch-check-in";
const SCHEMA_VERSION: u32 = 1;

/// The operating system's observation at a recorded process number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessIdentityObservation {
    /// No process currently has the recorded process number.
    Absent,
    /// A live process holds the number, but its start identity is unreadable.
    ForeignPresent,
    /// A process exists and carries this observed Darwin start identity.
    Present(ProcessStartIdentity),
}

/// The binary-supplied observations for one dispatch issuance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchIdentityObservation {
    issuance_sequence: Sequence,
    recorded_start_identity: Option<ProcessStartIdentity>,
    process_identity: Option<ProcessIdentityObservation>,
    artifact_production: ArtifactProduction,
}

impl DispatchIdentityObservation {
    /// Construct one complete observation for a dispatch issuance.
    pub const fn new(
        issuance_sequence: Sequence,
        recorded_start_identity: ProcessStartIdentity,
        process_identity: ProcessIdentityObservation,
        artifact_production: ArtifactProduction,
    ) -> Self {
        Self {
            issuance_sequence,
            recorded_start_identity: Some(recorded_start_identity),
            process_identity: Some(process_identity),
            artifact_production,
        }
    }

    /// Construct the observation for an issuance with no persisted identity sidecar.
    pub const fn unrecorded(issuance_sequence: Sequence) -> Self {
        Self {
            issuance_sequence,
            recorded_start_identity: None,
            process_identity: None,
            artifact_production: ArtifactProduction::NotProduced,
        }
    }
}

/// The current lifecycle classification of a dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchLiveness {
    /// No process identity was recorded, so liveness cannot be inferred.
    Unknown,
    /// The exact recorded process identity is still present.
    Running,
    /// A valid completion has been recorded.
    Finished,
    /// The exact recorded process identity is absent.
    Dead,
}

/// Whether completion accounting exists in the event log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DispatchCompletionAccounting {
    /// The continuation observed and reaped its child.
    ObservedChild,
    /// Reconciliation durably observed the dispatch dead.
    ReconciledDead,
    /// No completion record exists.
    Unaccounted,
}

/// One canonical check-in result in dispatch issuance order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DispatchCheckInEntry {
    issuance_sequence: Sequence,
    #[serde(rename = "state")]
    liveness: DispatchLiveness,
    #[serde(rename = "completion")]
    accounting: DispatchCompletionAccounting,
    artifact_production: ArtifactProduction,
}

impl DispatchCheckInEntry {
    /// Return the dispatch issuance sequence.
    pub const fn issuance_sequence(&self) -> Sequence {
        self.issuance_sequence
    }

    /// Return the current liveness classification.
    pub const fn liveness(&self) -> DispatchLiveness {
        self.liveness
    }

    /// Return the completion-accounting classification.
    pub const fn accounting(&self) -> DispatchCompletionAccounting {
        self.accounting
    }

    /// Return the current required-artifact production classification.
    pub const fn artifact_production(&self) -> ArtifactProduction {
        self.artifact_production
    }
}

/// The complete version-one dispatch check-in report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchCheckInReport {
    schema_id: String,
    schema_version: u32,
    accounting: DispatchAccountingSummary,
    dispatches: Vec<DispatchCheckInEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchAccountingSummary {
    state: DispatchAccountingState,
    issuance_sequences: Vec<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DispatchAccountingState {
    AllAccounted,
    Unaccounted,
}

/// Correlate event records and binary-supplied observations into a read-only report.
///
/// # Errors
///
/// Returns a typed error for invalid completion correlation or a missing, duplicate, or extraneous
/// dispatch observation.
pub fn classify_dispatch_check_in(
    records: &[EventRecord],
    observations: &[DispatchIdentityObservation],
) -> Result<DispatchCheckInReport, DispatchCheckInError> {
    let ledger = fold_dispatch_ledger(records).map_err(map_ledger_error)?;
    let dispatch_sequences = ledger
        .entries()
        .iter()
        .map(|entry| entry.issuance().sequence().get())
        .collect::<Vec<_>>();

    let mut observations_by_sequence = BTreeMap::<u64, &DispatchIdentityObservation>::new();
    for observation in observations {
        let issuance_sequence = observation.issuance_sequence.get();
        if observations_by_sequence
            .insert(issuance_sequence, observation)
            .is_some()
        {
            return Err(DispatchCheckInError::ObservationRepeatsIssuance {
                issuance_sequence: observation.issuance_sequence.get(),
            });
        }
        if !dispatch_sequences.contains(&issuance_sequence) {
            return Err(DispatchCheckInError::ObservationNamesNonDispatchIssuance {
                issuance_sequence: observation.issuance_sequence.get(),
            });
        }
    }

    let dispatches = ledger
        .entries()
        .iter()
        .map(|entry| {
            let issuance_sequence = entry.issuance().sequence();
            let observation = observations_by_sequence
                .get(&issuance_sequence.get())
                .ok_or(DispatchCheckInError::ObservationMissingForIssuance {
                    issuance_sequence: issuance_sequence.get(),
                })?;
            let (liveness, accounting) = if matches!(
                entry.completion(),
                Some(DispatchLedgerCompletion::ObservedChild { .. })
            ) {
                (
                    DispatchLiveness::Finished,
                    DispatchCompletionAccounting::ObservedChild,
                )
            } else if matches!(
                entry.completion(),
                Some(DispatchLedgerCompletion::ReconciledDead { .. })
            ) {
                (
                    DispatchLiveness::Dead,
                    DispatchCompletionAccounting::ReconciledDead,
                )
            } else if matches!(
                (observation.process_identity, observation.recorded_start_identity),
                (Some(ProcessIdentityObservation::Present(observed)), Some(recorded))
                    if observed == recorded
            ) {
                (
                    DispatchLiveness::Running,
                    DispatchCompletionAccounting::Unaccounted,
                )
            } else if observation.recorded_start_identity.is_none() {
                (
                    DispatchLiveness::Unknown,
                    DispatchCompletionAccounting::Unaccounted,
                )
            } else {
                (
                    DispatchLiveness::Dead,
                    DispatchCompletionAccounting::Unaccounted,
                )
            };
            Ok(DispatchCheckInEntry {
                issuance_sequence,
                liveness,
                accounting,
                artifact_production: observation.artifact_production,
            })
        })
        .collect::<Result<Vec<_>, DispatchCheckInError>>()?;

    Ok(DispatchCheckInReport {
        schema_id: SCHEMA_ID.to_owned(),
        schema_version: SCHEMA_VERSION,
        accounting: match ledger.accounting() {
            DispatchAccounting::AllAccounted => DispatchAccountingSummary {
                state: DispatchAccountingState::AllAccounted,
                issuance_sequences: Vec::new(),
            },
            DispatchAccounting::Unaccounted(unaccounted) => DispatchAccountingSummary {
                state: DispatchAccountingState::Unaccounted,
                issuance_sequences: unaccounted
                    .entries()
                    .iter()
                    .map(|issuance| issuance.sequence().get())
                    .collect(),
            },
        },
        dispatches,
    })
}

fn map_ledger_error(error: DispatchLedgerError) -> DispatchCheckInError {
    DispatchCheckInError::Ledger { source: error }
}

/// Serialize canonical compact report JSON followed by exactly one LF.
///
/// # Errors
///
/// Returns [`DispatchCheckInError::Serialization`] when JSON serialization fails.
pub fn serialize_dispatch_check_in(
    report: &DispatchCheckInReport,
) -> Result<Vec<u8>, DispatchCheckInError> {
    let mut bytes = serde_json::to_vec(report)
        .map_err(|source| DispatchCheckInError::Serialization { source })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// A dispatch check-in could not be correlated or serialized.
#[derive(Debug, Error)]
pub enum DispatchCheckInError {
    /// The shared dispatch ledger rejected an untrustworthy event relationship.
    #[error(transparent)]
    Ledger { source: DispatchLedgerError },
    /// A completion names an issuance absent from the event records.
    #[error(
        "dispatch check-in completion {completion_sequence} points to missing issuance {issuance_sequence}"
    )]
    CompletionIssuanceMissing {
        completion_sequence: u64,
        issuance_sequence: u64,
    },
    /// A completion names an event that is not a dispatch.
    #[error(
        "dispatch check-in completion {completion_sequence} points to non-dispatch issuance {issuance_sequence}"
    )]
    CompletionPointsToNonDispatch {
        completion_sequence: u64,
        issuance_sequence: u64,
    },
    /// A completion points to itself or a later issuance.
    #[error(
        "dispatch check-in completion {completion_sequence} points forward to issuance {issuance_sequence}"
    )]
    CompletionPointsForward {
        completion_sequence: u64,
        issuance_sequence: u64,
    },
    /// More than one completion names the same dispatch issuance.
    #[error(
        "dispatch check-in completion {completion_sequence} repeats completed issuance {issuance_sequence}"
    )]
    CompletionRepeatsIssuance {
        completion_sequence: u64,
        issuance_sequence: u64,
    },
    /// Completion and issuance nodes differ.
    #[error(
        "dispatch check-in completion {completion_sequence} node {completion_node} differs from issuance {issuance_sequence} node {issuance_node}"
    )]
    CompletionNodeMismatch {
        completion_sequence: u64,
        issuance_sequence: u64,
        completion_node: String,
        issuance_node: String,
    },
    /// More than one process observation names the same issuance.
    #[error("dispatch check-in observation repeats issuance {issuance_sequence}")]
    ObservationRepeatsIssuance { issuance_sequence: u64 },
    /// A process observation names an event that is not a dispatch.
    #[error("dispatch check-in observation names non-dispatch issuance {issuance_sequence}")]
    ObservationNamesNonDispatchIssuance { issuance_sequence: u64 },
    /// A dispatch has no process observation.
    #[error("dispatch check-in observation missing for issuance {issuance_sequence}")]
    ObservationMissingForIssuance { issuance_sequence: u64 },
    /// Canonical report serialization failed.
    #[error("failed to serialize dispatch check-in report")]
    Serialization { source: serde_json::Error },
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactProduction, DispatchCheckInError, DispatchCompletionAccounting,
        DispatchIdentityObservation, DispatchLiveness, ProcessIdentityObservation,
        classify_dispatch_check_in, serialize_dispatch_check_in,
    };
    use crate::{ProcessStartIdentity, Sequence, parse_event_line};

    const CANONICAL: &[u8] = b"{\"schema_id\":\"pce.dispatch-check-in\",\"schema_version\":1,\"accounting\":{\"state\":\"unaccounted\",\"issuance_sequences\":[3,4,5,6,7]},\"dispatches\":[{\"issuance_sequence\":1,\"state\":\"finished\",\"completion\":\"observed-child\",\"artifact_production\":\"produced\"},{\"issuance_sequence\":3,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":4,\"state\":\"running\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":5,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":6,\"state\":\"unknown\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":7,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":8,\"state\":\"finished\",\"completion\":\"observed-child\",\"artifact_production\":\"not-produced\"}]}\n";

    fn record(sequence: u64, kind: &str, node: &str, payload: &str) -> crate::EventRecord {
        parse_event_line(&format!(
            "{{\"sequence\":{sequence},\"timestamp\":\"2026-08-09T12:00:00.000Z\",\"kind\":\"{kind}\",\"node\":\"{node}\",\"payload\":{payload}}}"
        ))
        .expect("event fixture")
    }

    fn dispatch(sequence: u64, node: &str) -> crate::EventRecord {
        record(
            sequence,
            "dispatch",
            node,
            r#"{"role":"step-executor","ref":"abc123","evidence":"fixture"}"#,
        )
    }

    fn completion(sequence: u64, issuance: u64, node: &str) -> crate::EventRecord {
        record(
            sequence,
            "dispatch-completion",
            node,
            &format!(
                r#"{{"issuance_sequence":{issuance},"duration_ms":1,"usage":{{"availability":"absent","reason":"no-terminal-turn"}},"exit_status":{{"kind":"exited","code":0}},"artifact_outcome":"not-validated"}}"#
            ),
        )
    }

    fn start(seconds: u64) -> ProcessStartIdentity {
        ProcessStartIdentity::new(seconds, 123).expect("start identity")
    }

    fn observation(
        sequence: u64,
        recorded: ProcessStartIdentity,
        process: ProcessIdentityObservation,
        artifact: ArtifactProduction,
    ) -> DispatchIdentityObservation {
        DispatchIdentityObservation::new(
            Sequence::parse(sequence).expect("sequence"),
            recorded,
            process,
            artifact,
        )
    }

    #[test]
    fn canonical_report_bytes_distinguish_observed_reconciled_running_dead_and_unrecorded() {
        let records = [
            dispatch(1, "m1-s2"),
            completion(2, 1, "m1-s2"),
            dispatch(3, "m1-s2"),
            dispatch(4, "m1-s2"),
            dispatch(5, "m1-s2"),
            dispatch(6, "m1-s2"),
            dispatch(7, "m1-s2"),
            dispatch(8, "m1-s2"),
            completion(9, 8, "m1-s2"),
        ];
        let observations = [
            observation(
                1,
                start(10),
                ProcessIdentityObservation::Absent,
                ArtifactProduction::Produced,
            ),
            observation(
                3,
                start(30),
                ProcessIdentityObservation::Absent,
                ArtifactProduction::NotProduced,
            ),
            observation(
                4,
                start(40),
                ProcessIdentityObservation::Present(start(40)),
                ArtifactProduction::NotProduced,
            ),
            observation(
                5,
                start(50),
                ProcessIdentityObservation::Present(start(51)),
                ArtifactProduction::NotProduced,
            ),
            DispatchIdentityObservation::unrecorded(Sequence::parse(6).expect("sequence")),
            observation(
                7,
                start(70),
                ProcessIdentityObservation::ForeignPresent,
                ArtifactProduction::NotProduced,
            ),
            DispatchIdentityObservation::unrecorded(Sequence::parse(8).expect("sequence")),
        ];
        let report = classify_dispatch_check_in(&records, &observations).expect("classify");
        assert_eq!(report.dispatches[0].liveness(), DispatchLiveness::Finished);
        assert_eq!(
            report.dispatches[0].accounting(),
            DispatchCompletionAccounting::ObservedChild
        );
        assert_eq!(
            report.dispatches[0].artifact_production(),
            ArtifactProduction::Produced
        );
        assert_eq!(report.dispatches[0].issuance_sequence(), Sequence::first());
        assert_eq!(report.dispatches[1].liveness(), DispatchLiveness::Dead);
        assert_eq!(
            report.dispatches[1].accounting(),
            DispatchCompletionAccounting::Unaccounted
        );
        assert_eq!(
            report.dispatches[1].artifact_production(),
            ArtifactProduction::NotProduced
        );
        assert_eq!(report.dispatches[2].liveness(), DispatchLiveness::Running);
        assert_eq!(
            report.dispatches[2].accounting(),
            DispatchCompletionAccounting::Unaccounted
        );
        assert_eq!(
            report.dispatches[2].artifact_production(),
            ArtifactProduction::NotProduced
        );
        assert_eq!(report.dispatches[3].liveness(), DispatchLiveness::Dead);
        assert_eq!(
            report.dispatches[3].accounting(),
            DispatchCompletionAccounting::Unaccounted
        );
        assert_eq!(
            report.dispatches[3].artifact_production(),
            ArtifactProduction::NotProduced
        );
        assert_eq!(report.dispatches[4].liveness(), DispatchLiveness::Unknown);
        assert_eq!(report.dispatches[5].liveness(), DispatchLiveness::Dead);
        assert_eq!(report.dispatches[6].liveness(), DispatchLiveness::Finished);
        assert_eq!(
            report.dispatches[6].accounting(),
            DispatchCompletionAccounting::ObservedChild
        );
        assert_eq!(
            report
                .dispatches
                .iter()
                .map(|entry| (
                    entry.issuance_sequence().get(),
                    entry.liveness(),
                    entry.accounting(),
                    entry.artifact_production(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    1,
                    DispatchLiveness::Finished,
                    DispatchCompletionAccounting::ObservedChild,
                    ArtifactProduction::Produced,
                ),
                (
                    3,
                    DispatchLiveness::Dead,
                    DispatchCompletionAccounting::Unaccounted,
                    ArtifactProduction::NotProduced,
                ),
                (
                    4,
                    DispatchLiveness::Running,
                    DispatchCompletionAccounting::Unaccounted,
                    ArtifactProduction::NotProduced,
                ),
                (
                    5,
                    DispatchLiveness::Dead,
                    DispatchCompletionAccounting::Unaccounted,
                    ArtifactProduction::NotProduced,
                ),
                (
                    6,
                    DispatchLiveness::Unknown,
                    DispatchCompletionAccounting::Unaccounted,
                    ArtifactProduction::NotProduced,
                ),
                (
                    7,
                    DispatchLiveness::Dead,
                    DispatchCompletionAccounting::Unaccounted,
                    ArtifactProduction::NotProduced,
                ),
                (
                    8,
                    DispatchLiveness::Finished,
                    DispatchCompletionAccounting::ObservedChild,
                    ArtifactProduction::NotProduced,
                ),
            ]
        );
        assert_eq!(
            serialize_dispatch_check_in(&report).expect("serialize"),
            CANONICAL
        );
        let reconciled_records = [
            dispatch(1, "m1-s2"),
            record(
                2,
                "dispatch-completion",
                "m1-s2",
                r#"{"issuance_sequence":1,"outcome":"reconciled-dead","artifact_production":"not-produced"}"#,
            ),
        ];
        let reconciled = classify_dispatch_check_in(
            &reconciled_records,
            &[observation(
                1,
                start(1),
                ProcessIdentityObservation::Absent,
                ArtifactProduction::NotProduced,
            )],
        )
        .expect("reconciled report");
        assert_eq!(serialize_dispatch_check_in(&reconciled).expect("serialize"), b"{\"schema_id\":\"pce.dispatch-check-in\",\"schema_version\":1,\"accounting\":{\"state\":\"all-accounted\",\"issuance_sequences\":[]},\"dispatches\":[{\"issuance_sequence\":1,\"state\":\"dead\",\"completion\":\"reconciled-dead\",\"artifact_production\":\"not-produced\"}]}\n");
    }

    #[test]
    fn classification_requires_exactly_one_observation_per_dispatch() {
        let records = [dispatch(1, "m1-s2")];
        let one = observation(
            1,
            start(1),
            ProcessIdentityObservation::Absent,
            ArtifactProduction::NotProduced,
        );
        assert_eq!(
            classify_dispatch_check_in(&records, &[])
                .expect_err("missing")
                .to_string(),
            "dispatch check-in observation missing for issuance 1"
        );
        assert_eq!(
            classify_dispatch_check_in(&records, &[one, one])
                .expect_err("duplicate")
                .to_string(),
            "dispatch check-in observation repeats issuance 1"
        );
        let non_dispatch = observation(
            2,
            start(2),
            ProcessIdentityObservation::Absent,
            ArtifactProduction::NotProduced,
        );
        assert_eq!(
            classify_dispatch_check_in(&records, &[one, non_dispatch])
                .expect_err("non-dispatch")
                .to_string(),
            "dispatch check-in observation names non-dispatch issuance 2"
        );
    }

    #[test]
    fn classification_rejects_invalid_completion_correlation() {
        let cases = [
            (
                vec![dispatch(1, "m1-s2"), completion(3, 2, "m1-s2")],
                "dispatch completion 3 names missing issuance sequence 2",
            ),
            (
                vec![
                    record(1, "delta", "m1-s2", r#"{"message":"x"}"#),
                    completion(2, 1, "m1-s2"),
                ],
                "dispatch completion 2 names non-dispatch sequence 1",
            ),
            (
                vec![dispatch(1, "m1-s2"), completion(2, 2, "m1-s2")],
                "dispatch completion 2 points forward to issuance 2",
            ),
            (
                vec![
                    dispatch(1, "m1-s2"),
                    completion(2, 1, "m1-s2"),
                    completion(3, 1, "m1-s2"),
                ],
                "duplicate completion 3 for issuance 1; first completion was 2",
            ),
            (
                vec![dispatch(1, "m1-s2"), completion(2, 1, "m1-other")],
                "completion node m1-other does not match issuance 1 node m1-s2",
            ),
        ];
        for (records, expected) in cases {
            let error = classify_dispatch_check_in(&records, &[]).expect_err("invalid completion");
            assert_eq!(error.to_string(), expected);
            assert!(matches!(error, DispatchCheckInError::Ledger { .. }));
        }
    }
}
