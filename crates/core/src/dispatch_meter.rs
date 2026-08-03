//! meter : [EventRecord] -> [DispatchMeterRecord] (pure, deterministic)
//!
//! Correlates dispatch issuances with their terminal completions without acquiring any I/O
//! authority.

use std::collections::BTreeMap;

use serde::Serialize;
use thiserror::Error;
use tracing::instrument;

use crate::DispatchRef;
use crate::event_log::{
    ArtifactOutcome, DispatchDuration, DispatchExitStatus, DispatchRole, DispatchTokenUsage,
    EventBodyRef, EventRecord, EventTimestamp, Evidence, KnownPayload, NodeId, Sequence,
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
pub struct DispatchMeterCompletion {
    pub sequence: Sequence,
    pub timestamp: EventTimestamp,
    pub duration_ms: DispatchDuration,
    pub exit_status: DispatchExitStatus,
    pub artifact_outcome: ArtifactOutcome,
    pub usage: DispatchTokenUsage,
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

#[derive(Debug, Clone, Copy)]
enum SeenTarget {
    Dispatch { report_index: usize },
    Other,
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
    let mut seen = BTreeMap::<u64, SeenTarget>::new();
    let mut report = Vec::<DispatchMeterRecord>::new();

    for record in records {
        let sequence = record.sequence();
        match record.body_ref() {
            EventBodyRef::Known(KnownPayload::Dispatch(payload)) => {
                let report_index = report.len();
                report.push(DispatchMeterRecord {
                    issuance: DispatchMeterIssuance {
                        sequence,
                        timestamp: *record.timestamp(),
                        node: record.node().clone(),
                        role: payload.role.clone(),
                        r#ref: payload.r#ref.clone(),
                        evidence: payload.evidence.clone(),
                    },
                    completion: None,
                });
                seen.insert(sequence.get(), SeenTarget::Dispatch { report_index });
            }
            EventBodyRef::Known(KnownPayload::DispatchCompletion(payload)) => {
                let issuance_sequence = payload.issuance_sequence;
                if issuance_sequence.get() >= sequence.get() {
                    return Err(DispatchMeterError::ForwardReference {
                        completion_sequence: sequence,
                        issuance_sequence,
                    });
                }
                let target = seen.get(&issuance_sequence.get()).copied().ok_or(
                    DispatchMeterError::MissingTarget {
                        completion_sequence: sequence,
                        issuance_sequence,
                    },
                )?;
                let SeenTarget::Dispatch { report_index } = target else {
                    return Err(DispatchMeterError::NonDispatchTarget {
                        completion_sequence: sequence,
                        issuance_sequence,
                    });
                };
                let issuance = &report[report_index].issuance;
                if issuance.node != *record.node() {
                    return Err(DispatchMeterError::NodeMismatch {
                        issuance_sequence,
                        issuance_node: issuance.node.clone(),
                        completion_node: record.node().clone(),
                    });
                }
                if let Some(first) = &report[report_index].completion {
                    return Err(DispatchMeterError::DuplicateCompletion {
                        issuance_sequence,
                        first_completion_sequence: first.sequence,
                        duplicate_completion_sequence: sequence,
                    });
                }
                report[report_index].completion = Some(DispatchMeterCompletion {
                    sequence,
                    timestamp: *record.timestamp(),
                    duration_ms: payload.duration_ms,
                    exit_status: payload.exit_status,
                    artifact_outcome: payload.artifact_outcome,
                    usage: project_usage(&payload.usage),
                });
                seen.insert(sequence.get(), SeenTarget::Other);
            }
            EventBodyRef::Known(_) | EventBodyRef::Unknown { .. } => {
                seen.insert(sequence.get(), SeenTarget::Other);
            }
        }
    }

    Ok(report)
}
