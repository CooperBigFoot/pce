//! dispatch_ledger : Ordered<EventRecord> → Ordered<DispatchLedgerEntry> ∪ DispatchLedgerError   (pure, deterministic)

use std::collections::BTreeMap;

use thiserror::Error;

use crate::event_log::{
    DispatchCompletionOutcomeRef, DispatchRef, DispatchRole, EventBodyRef, EventRecord,
    EventTimestamp, Evidence, KnownPayload, NodeId, ObservedDispatchCompletionPayload,
    ReconciledDeadDispatchCompletionPayload, Sequence,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchLedger {
    entries: Vec<DispatchLedgerEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchLedgerEntry {
    issuance: DispatchLedgerIssuance,
    completion: Option<DispatchLedgerCompletion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchLedgerIssuance {
    sequence: Sequence,
    timestamp: EventTimestamp,
    node: NodeId,
    role: DispatchRole,
    dispatch_ref: DispatchRef,
    evidence: Evidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchLedgerCompletion {
    ObservedChild {
        sequence: Sequence,
        timestamp: EventTimestamp,
        payload: ObservedDispatchCompletionPayload,
    },
    ReconciledDead {
        sequence: Sequence,
        timestamp: EventTimestamp,
        payload: ReconciledDeadDispatchCompletionPayload,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnaccountedDispatchLedger {
    entries: Vec<DispatchLedgerIssuance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchAccounting {
    AllAccounted,
    Unaccounted(UnaccountedDispatchLedger),
}

impl DispatchLedger {
    pub fn entries(&self) -> &[DispatchLedgerEntry] {
        &self.entries
    }

    pub fn issuance(&self, sequence: Sequence) -> Option<&DispatchLedgerEntry> {
        self.entries
            .iter()
            .find(|entry| entry.issuance.sequence == sequence)
    }

    pub fn unaccounted(&self) -> UnaccountedDispatchLedger {
        UnaccountedDispatchLedger {
            entries: self
                .entries
                .iter()
                .filter(|entry| entry.completion.is_none())
                .map(|entry| entry.issuance.clone())
                .collect(),
        }
    }

    pub fn accounting(&self) -> DispatchAccounting {
        let ledger = self.unaccounted();
        if ledger.entries.is_empty() {
            DispatchAccounting::AllAccounted
        } else {
            DispatchAccounting::Unaccounted(ledger)
        }
    }
}

impl DispatchLedgerEntry {
    pub fn issuance(&self) -> &DispatchLedgerIssuance {
        &self.issuance
    }
    pub fn completion(&self) -> Option<&DispatchLedgerCompletion> {
        self.completion.as_ref()
    }
}

impl DispatchLedgerIssuance {
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }
    pub const fn timestamp(&self) -> EventTimestamp {
        self.timestamp
    }
    pub fn node(&self) -> &NodeId {
        &self.node
    }
    pub fn role(&self) -> &DispatchRole {
        &self.role
    }
    pub fn dispatch_ref(&self) -> &DispatchRef {
        &self.dispatch_ref
    }
    pub fn evidence(&self) -> &Evidence {
        &self.evidence
    }
}

impl DispatchLedgerCompletion {
    pub const fn sequence(&self) -> Sequence {
        match self {
            Self::ObservedChild { sequence, .. } | Self::ReconciledDead { sequence, .. } => {
                *sequence
            }
        }
    }
    pub const fn timestamp(&self) -> EventTimestamp {
        match self {
            Self::ObservedChild { timestamp, .. } | Self::ReconciledDead { timestamp, .. } => {
                *timestamp
            }
        }
    }
}

impl UnaccountedDispatchLedger {
    pub fn entries(&self) -> &[DispatchLedgerIssuance] {
        &self.entries
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DispatchLedgerError {
    #[error("dispatch completion {completion} points forward to issuance {issuance}", completion = .completion_sequence.get(), issuance = .issuance_sequence.get())]
    ForwardReference {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    #[error("dispatch completion {completion} names missing issuance sequence {issuance}", completion = .completion_sequence.get(), issuance = .issuance_sequence.get())]
    MissingTarget {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    #[error("dispatch completion {completion} names non-dispatch sequence {issuance}", completion = .completion_sequence.get(), issuance = .issuance_sequence.get())]
    NonDispatchTarget {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    #[error("duplicate completion {duplicate} for issuance {issuance}; first completion was {first}", duplicate = .duplicate_completion_sequence.get(), issuance = .issuance_sequence.get(), first = .first_completion_sequence.get())]
    DuplicateCompletion {
        issuance_sequence: Sequence,
        first_completion_sequence: Sequence,
        duplicate_completion_sequence: Sequence,
    },
    #[error("completion node {completion_node} does not match issuance {issuance} node {issuance_node}", completion_node = .completion_node.as_str(), issuance = .issuance_sequence.get(), issuance_node = .issuance_node.as_str())]
    NodeMismatch {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
        issuance_node: NodeId,
        completion_node: NodeId,
    },
    #[error("event sequence {sequence} is not greater than previous sequence {previous}", sequence = .sequence.get(), previous = .previous_sequence.get())]
    NonIncreasingSequence {
        previous_sequence: Sequence,
        sequence: Sequence,
    },
}

#[derive(Clone, Copy)]
enum Seen {
    Dispatch(usize),
    Other,
}

pub fn fold_dispatch_ledger(
    records: &[EventRecord],
) -> Result<DispatchLedger, DispatchLedgerError> {
    let mut entries = Vec::new();
    let mut seen = BTreeMap::<u64, Seen>::new();
    let mut previous: Option<Sequence> = None;
    for record in records {
        let sequence = record.sequence();
        if let Some(previous_sequence) = previous {
            if sequence.get() <= previous_sequence.get() {
                return Err(DispatchLedgerError::NonIncreasingSequence {
                    previous_sequence,
                    sequence,
                });
            }
        }
        previous = Some(sequence);
        match record.body_ref() {
            EventBodyRef::Known(KnownPayload::Dispatch(payload)) => {
                let index = entries.len();
                entries.push(DispatchLedgerEntry {
                    issuance: DispatchLedgerIssuance {
                        sequence,
                        timestamp: *record.timestamp(),
                        node: record.node().clone(),
                        role: payload.role.clone(),
                        dispatch_ref: payload.r#ref.clone(),
                        evidence: payload.evidence.clone(),
                    },
                    completion: None,
                });
                seen.insert(sequence.get(), Seen::Dispatch(index));
            }
            EventBodyRef::Known(KnownPayload::DispatchCompletion(payload)) => {
                let issuance_sequence = payload.issuance_sequence();
                if issuance_sequence.get() >= sequence.get() {
                    return Err(DispatchLedgerError::ForwardReference {
                        completion_sequence: sequence,
                        issuance_sequence,
                    });
                }
                let target = seen.get(&issuance_sequence.get()).copied().ok_or(
                    DispatchLedgerError::MissingTarget {
                        completion_sequence: sequence,
                        issuance_sequence,
                    },
                )?;
                let Seen::Dispatch(index) = target else {
                    return Err(DispatchLedgerError::NonDispatchTarget {
                        completion_sequence: sequence,
                        issuance_sequence,
                    });
                };
                let entry = &mut entries[index];
                if entry.issuance.node != *record.node() {
                    return Err(DispatchLedgerError::NodeMismatch {
                        completion_sequence: sequence,
                        issuance_sequence,
                        issuance_node: entry.issuance.node.clone(),
                        completion_node: record.node().clone(),
                    });
                }
                if let Some(first) = &entry.completion {
                    return Err(DispatchLedgerError::DuplicateCompletion {
                        issuance_sequence,
                        first_completion_sequence: first.sequence(),
                        duplicate_completion_sequence: sequence,
                    });
                }
                entry.completion = Some(match payload.outcome() {
                    DispatchCompletionOutcomeRef::ObservedChild(payload) => {
                        DispatchLedgerCompletion::ObservedChild {
                            sequence,
                            timestamp: *record.timestamp(),
                            payload: payload.clone(),
                        }
                    }
                    DispatchCompletionOutcomeRef::ReconciledDead(payload) => {
                        DispatchLedgerCompletion::ReconciledDead {
                            sequence,
                            timestamp: *record.timestamp(),
                            payload: payload.clone(),
                        }
                    }
                });
                seen.insert(sequence.get(), Seen::Other);
            }
            EventBodyRef::Known(_) | EventBodyRef::Unknown { .. } => {
                seen.insert(sequence.get(), Seen::Other);
            }
        }
    }
    Ok(DispatchLedger { entries })
}

#[cfg(test)]
mod tests {
    use super::{DispatchAccounting, DispatchLedgerCompletion, fold_dispatch_ledger};
    use crate::parse_event_line;

    fn event(sequence: u64, kind: &str, node: &str, payload: &str) -> crate::EventRecord {
        parse_event_line(&format!("{{\"sequence\":{sequence},\"timestamp\":\"2026-08-09T12:00:00.000Z\",\"kind\":\"{kind}\",\"node\":\"{node}\",\"payload\":{payload}}}")).expect("event")
    }
    fn issuance(sequence: u64) -> crate::EventRecord {
        event(
            sequence,
            "dispatch",
            "m1-s2",
            r#"{"role":"step-executor","ref":"abc123","evidence":"fixture"}"#,
        )
    }
    fn observed(sequence: u64, issuance: u64) -> crate::EventRecord {
        event(
            sequence,
            "dispatch-completion",
            "m1-s2",
            &format!(
                r#"{{"issuance_sequence":{issuance},"duration_ms":200,"usage":{{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5}},"exit_status":{{"kind":"exited","code":0}},"artifact_outcome":"not-validated"}}"#
            ),
        )
    }
    fn reconciled(sequence: u64, issuance: u64) -> crate::EventRecord {
        event(
            sequence,
            "dispatch-completion",
            "m1-s2",
            &format!(
                r#"{{"issuance_sequence":{issuance},"outcome":"reconciled-dead","artifact_production":"not-produced"}}"#
            ),
        )
    }

    #[test]
    fn ledger_preserves_every_issuance_and_only_completion_removes_it_from_unaccounted() {
        let records = [
            issuance(1),
            issuance(3),
            issuance(4),
            observed(5, 3),
            reconciled(6, 1),
        ];
        for _ in 0..2 {
            let ledger = fold_dispatch_ledger(&records).expect("ledger");
            assert_eq!(
                ledger
                    .entries()
                    .iter()
                    .map(|entry| entry.issuance().sequence().get())
                    .collect::<Vec<_>>(),
                [1, 3, 4]
            );
            assert!(matches!(
                ledger.entries()[0].completion(),
                Some(DispatchLedgerCompletion::ReconciledDead { .. })
            ));
            assert!(matches!(
                ledger.entries()[1].completion(),
                Some(DispatchLedgerCompletion::ObservedChild { .. })
            ));
            assert_eq!(
                ledger
                    .unaccounted()
                    .entries()
                    .iter()
                    .map(|entry| entry.sequence().get())
                    .collect::<Vec<_>>(),
                [4]
            );
        }
    }

    #[test]
    fn all_accounted_is_constructed_only_from_an_empty_ledger() {
        assert!(matches!(
            fold_dispatch_ledger(&[issuance(1)])
                .expect("ledger")
                .accounting(),
            DispatchAccounting::Unaccounted(_)
        ));
        assert!(matches!(
            fold_dispatch_ledger(&[issuance(1), observed(2, 1)])
                .expect("ledger")
                .accounting(),
            DispatchAccounting::AllAccounted
        ));
        assert!(matches!(
            fold_dispatch_ledger(&[issuance(1), reconciled(2, 1)])
                .expect("ledger")
                .accounting(),
            DispatchAccounting::AllAccounted
        ));
    }

    #[test]
    fn ledger_rejects_every_untrustworthy_correlation() {
        let cases = [
            (
                vec![issuance(2), observed(2, 2)],
                "event sequence 2 is not greater than previous sequence 2",
            ),
            (
                vec![observed(2, 1)],
                "dispatch completion 2 names missing issuance sequence 1",
            ),
            (
                vec![
                    event(1, "delta", "m1-s2", r#"{"message":"x"}"#),
                    observed(2, 1),
                ],
                "dispatch completion 2 names non-dispatch sequence 1",
            ),
            (
                vec![issuance(1), observed(2, 1), reconciled(3, 1)],
                "duplicate completion 3 for issuance 1; first completion was 2",
            ),
            (
                vec![
                    issuance(1),
                    event(
                        2,
                        "dispatch-completion",
                        "m8-s2",
                        r#"{"issuance_sequence":1,"outcome":"reconciled-dead","artifact_production":"not-produced"}"#,
                    ),
                ],
                "completion node m8-s2 does not match issuance 1 node m1-s2",
            ),
            (
                vec![issuance(2), observed(3, 3)],
                "dispatch completion 3 points forward to issuance 3",
            ),
        ];
        for (records, expected) in cases {
            assert_eq!(
                fold_dispatch_ledger(&records)
                    .expect_err("invalid")
                    .to_string(),
                expected
            );
        }
    }
}
