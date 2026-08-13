//! meter : [EventRecord] -> [DispatchMeterRecord] (pure, deterministic)
//!
//! Correlates dispatch issuances with their terminal completions without acquiring any I/O
//! authority.

use serde::Serialize;
use thiserror::Error;
use tracing::instrument;

use crate::DispatchRef;
use crate::dispatch_ledger::{DispatchLedgerCompletion, DispatchLedgerError, fold_dispatch_ledger};
use crate::event_log::{
    ArtifactOutcome, ArtifactProduction, DispatchDuration, DispatchExitStatus, DispatchRole,
    DispatchTokenUsage, EventRecord, EventTimestamp, Evidence, NodeId, ReconciledDispatchOutcome,
    Sequence, SpawnDispatchOutcome,
};

/// The issuance half of one dispatch lifecycle report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchMeterIssuance {
    pub sequence: Sequence,
    pub timestamp: EventTimestamp,
    pub node: NodeId,
    pub role: DispatchRole,
    pub r#ref: DispatchRef,
    pub evidence: Evidence,
}

/// The optional completion half of one dispatch lifecycle report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum DispatchMeterCompletion {
    ObservedChild(ObservedDispatchMeterCompletion),
    SpawnFailed(SpawnFailedDispatchMeterCompletion),
    ReconciledDead(ReconciledDeadDispatchMeterCompletion),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObservedDispatchMeterCompletion {
    pub sequence: Sequence,
    pub timestamp: EventTimestamp,
    pub duration_ms: DispatchDuration,
    pub exit_status: DispatchExitStatus,
    pub artifact_outcome: ArtifactOutcome,
    pub usage: DispatchTokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpawnFailedDispatchMeterCompletion {
    pub sequence: Sequence,
    pub timestamp: EventTimestamp,
    pub outcome: SpawnDispatchOutcome,
    pub artifact_production: ArtifactProduction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReconciledDeadDispatchMeterCompletion {
    pub sequence: Sequence,
    pub timestamp: EventTimestamp,
    pub outcome: ReconciledDispatchOutcome,
    pub artifact_production: ArtifactProduction,
}

/// One dispatch issuance and its correlated completion, when present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DispatchMeterRecord {
    pub issuance: DispatchMeterIssuance,
    pub completion: Option<DispatchMeterCompletion>,
}

/// A malformed relationship between a dispatch completion and the event stream it names.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchMeterError {
    /// The shared ledger rejected sequence order before correlation.
    #[error(transparent)]
    Ledger { source: DispatchLedgerError },
    /// A completion names its own event sequence or a later event sequence.
    #[error(
        "dispatch completion {completion} points forward to issuance {issuance}",
        completion = .completion_sequence.get(),
        issuance = .issuance_sequence.get()
    )]
    ForwardReference {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    /// A completion names an earlier sequence that does not occur in the supplied records.
    #[error(
        "dispatch completion {completion} names missing issuance sequence {issuance}",
        completion = .completion_sequence.get(),
        issuance = .issuance_sequence.get()
    )]
    MissingTarget {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    /// A completion names an earlier event whose kind is not `dispatch`.
    #[error(
        "dispatch completion {completion} names non-dispatch sequence {issuance}",
        completion = .completion_sequence.get(),
        issuance = .issuance_sequence.get()
    )]
    NonDispatchTarget {
        completion_sequence: Sequence,
        issuance_sequence: Sequence,
    },
    /// More than one completion names the same dispatch issuance.
    #[error(
        "duplicate completion {duplicate} for issuance {issuance}; first completion was {first}",
        duplicate = .duplicate_completion_sequence.get(),
        issuance = .issuance_sequence.get(),
        first = .first_completion_sequence.get()
    )]
    DuplicateCompletion {
        issuance_sequence: Sequence,
        first_completion_sequence: Sequence,
        duplicate_completion_sequence: Sequence,
    },
    /// A completion's node differs from the node recorded by its issuance.
    #[error(
        "completion node {completion_node} does not match issuance {issuance} node {issuance_node}",
        completion_node = .completion_node.as_str(),
        issuance = .issuance_sequence.get(),
        issuance_node = .issuance_node.as_str()
    )]
    NodeMismatch {
        issuance_sequence: Sequence,
        issuance_node: NodeId,
        completion_node: NodeId,
    },
}

fn project_usage(usage: &DispatchTokenUsage) -> DispatchTokenUsage {
    match usage {
        DispatchTokenUsage::Measured {
            input_tokens,
            cached_input_tokens,
            output_tokens,
            reasoning_output_tokens,
        } => DispatchTokenUsage::Measured {
            input_tokens: *input_tokens,
            cached_input_tokens: *cached_input_tokens,
            output_tokens: *output_tokens,
            reasoning_output_tokens: *reasoning_output_tokens,
        },
        DispatchTokenUsage::ClaudeMeasured {
            input_tokens,
            output_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
        } => DispatchTokenUsage::ClaudeMeasured {
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
            cache_creation_input_tokens: *cache_creation_input_tokens,
            cache_read_input_tokens: *cache_read_input_tokens,
        },
        DispatchTokenUsage::Absent { reason } => DispatchTokenUsage::Absent { reason: *reason },
    }
}

/// Correlate dispatch lifecycle records in issuance order.
///
/// The caller must supply records with strictly increasing event sequences.
///
/// # Errors
///
/// Returns [`DispatchMeterError`] when a completion points forward, names a missing or
/// non-dispatch event, repeats an already completed issuance, or has a different node from its
/// issuance.
#[instrument(skip(records))]
pub fn meter_dispatches(
    records: &[EventRecord],
) -> Result<Vec<DispatchMeterRecord>, DispatchMeterError> {
    let ledger = fold_dispatch_ledger(records).map_err(map_ledger_error)?;
    Ok(ledger
        .entries()
        .iter()
        .map(|entry| {
            let issuance = entry.issuance();
            DispatchMeterRecord {
                issuance: DispatchMeterIssuance {
                    sequence: issuance.sequence(),
                    timestamp: issuance.timestamp(),
                    node: issuance.node().clone(),
                    role: issuance.role().clone(),
                    r#ref: issuance.dispatch_ref().clone(),
                    evidence: issuance.evidence().clone(),
                },
                completion: entry.completion().map(|completion| match completion {
                    DispatchLedgerCompletion::ObservedChild {
                        sequence,
                        timestamp,
                        payload,
                    } => DispatchMeterCompletion::ObservedChild(ObservedDispatchMeterCompletion {
                        sequence: *sequence,
                        timestamp: *timestamp,
                        duration_ms: payload.duration_ms(),
                        exit_status: payload.exit_status(),
                        artifact_outcome: payload.artifact_outcome(),
                        usage: project_usage(payload.usage()),
                    }),
                    DispatchLedgerCompletion::SpawnFailed {
                        sequence,
                        timestamp,
                        payload,
                    } => DispatchMeterCompletion::SpawnFailed(SpawnFailedDispatchMeterCompletion {
                        sequence: *sequence,
                        timestamp: *timestamp,
                        outcome: payload.outcome,
                        artifact_production: payload.artifact_production,
                    }),
                    DispatchLedgerCompletion::ReconciledDead {
                        sequence,
                        timestamp,
                        payload,
                    } => DispatchMeterCompletion::ReconciledDead(
                        ReconciledDeadDispatchMeterCompletion {
                            sequence: *sequence,
                            timestamp: *timestamp,
                            outcome: payload.outcome,
                            artifact_production: payload.artifact_production,
                        },
                    ),
                }),
            }
        })
        .collect())
}

fn map_ledger_error(error: DispatchLedgerError) -> DispatchMeterError {
    match error {
        DispatchLedgerError::ForwardReference {
            completion_sequence,
            issuance_sequence,
        } => DispatchMeterError::ForwardReference {
            completion_sequence,
            issuance_sequence,
        },
        DispatchLedgerError::MissingTarget {
            completion_sequence,
            issuance_sequence,
        } => DispatchMeterError::MissingTarget {
            completion_sequence,
            issuance_sequence,
        },
        DispatchLedgerError::NonDispatchTarget {
            completion_sequence,
            issuance_sequence,
        } => DispatchMeterError::NonDispatchTarget {
            completion_sequence,
            issuance_sequence,
        },
        DispatchLedgerError::DuplicateCompletion {
            issuance_sequence,
            first_completion_sequence,
            duplicate_completion_sequence,
        } => DispatchMeterError::DuplicateCompletion {
            issuance_sequence,
            first_completion_sequence,
            duplicate_completion_sequence,
        },
        DispatchLedgerError::NodeMismatch {
            completion_sequence: _,
            issuance_sequence,
            issuance_node,
            completion_node,
        } => DispatchMeterError::NodeMismatch {
            issuance_sequence,
            issuance_node,
            completion_node,
        },
        error @ DispatchLedgerError::NonIncreasingSequence { .. } => {
            DispatchMeterError::Ledger { source: error }
        }
    }
}
