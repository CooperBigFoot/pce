//! landing_readiness : CompletionGateResult × Ordered<StepMergeResult> × Ordered<CriterionExecutionObservation> → LandingReadinessResult   (pure, deterministic)

use crate::{
    AcceptanceCriterion, CompletionCriterionStatus, CompletionDecision, CompletionGateResult,
    CriterionExecutionObservation, CriterionExecutionOutcome, Evidence, FinishedResult,
    MergeStatus, NodeId, Sequence, StepMergeResult, StepSnapshot,
};
use serde::Serialize;
use tracing::instrument;

/// Whether the supplied completion, merge, and evidence authorities prove landing readiness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LandingReadinessDecision {
    /// Every landing-readiness condition is proven.
    Ready,
    /// At least one landing-readiness condition is not proven.
    Refuse,
}

/// Zero-based position of a report in the completion result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CompletionCriterionIndex(usize);

impl CompletionCriterionIndex {
    /// Return the zero-based completion-report position.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// Joined proof of completion, merge authorities, and criterion evidence.
#[derive(Debug, Serialize)]
pub struct LandingReadinessResult<'a> {
    /// The decision implied by the collected readiness problems.
    pub decision: LandingReadinessDecision,
    /// The supplied ordered completion result.
    pub completion: &'a CompletionGateResult,
    /// Exact step authority projections in caller order.
    pub steps: Vec<StepSnapshot<'a>>,
    /// Evidence backing each completion report in effective-criterion order.
    pub criterion_evidence: Vec<LandingCriterionEvidence<'a>>,
    /// Readiness problems in deterministic evaluation order.
    pub problems: Vec<LandingReadinessProblem<'a>>,
}

/// Event evidence backing one ordered completion report.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum LandingCriterionEvidence<'a> {
    /// The latest exact execution agrees with the completion report.
    Recorded {
        /// Position of the backed completion report.
        criterion_index: CompletionCriterionIndex,
        /// Source event sequence.
        sequence: Sequence,
        /// Source event node.
        node: &'a NodeId,
        /// Exact criterion from the source event.
        criterion: &'a AcceptanceCriterion,
        /// Exact finished result from the source event.
        finished_result: &'a FinishedResult,
        /// Exact outcome from the source event.
        outcome: &'a CriterionExecutionOutcome,
        /// Non-empty evidence from the source event.
        evidence: &'a Evidence,
    },
    /// No latest exact execution agrees with the completion report.
    Missing {
        /// Position of the unbacked completion report.
        criterion_index: CompletionCriterionIndex,
    },
}

/// One independently retained reason landing readiness is refused.
#[derive(Debug, Serialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum LandingReadinessProblem<'a> {
    /// The supplied completion gate refused the finished result.
    CompletionRefused,
    /// No log-visible step authority was supplied.
    NoStepAuthorities,
    /// A step is authoritatively not merged.
    StepNotMerged {
        /// Node whose exact authorities prove it is not merged.
        node: &'a NodeId,
    },
    /// A step's exact authorities cannot prove a merge state.
    StepInconclusive {
        /// Node whose exact authorities are inconclusive.
        node: &'a NodeId,
    },
    /// A completion report lacks matching latest event evidence.
    CriterionExecutionMissing {
        /// Position of the unbacked completion report.
        criterion_index: CompletionCriterionIndex,
    },
}

/// Join completion, merge authorities, and latest exact criterion evidence into a landing proof.
#[instrument(skip(completion, steps, executions))]
pub fn evaluate_landing_readiness<'a>(
    completion: &'a CompletionGateResult,
    steps: &'a [StepMergeResult],
    executions: &'a [CriterionExecutionObservation],
) -> LandingReadinessResult<'a> {
    let step_snapshots = steps.iter().map(StepSnapshot::from).collect();
    let mut problems = Vec::new();
    if completion.decision == CompletionDecision::Refuse {
        problems.push(LandingReadinessProblem::CompletionRefused);
    }
    if steps.is_empty() {
        problems.push(LandingReadinessProblem::NoStepAuthorities);
    } else {
        for step in steps {
            match step.status() {
                MergeStatus::Merged => {}
                MergeStatus::NotMerged => {
                    problems.push(LandingReadinessProblem::StepNotMerged { node: step.node() })
                }
                MergeStatus::Inconclusive => {
                    problems.push(LandingReadinessProblem::StepInconclusive { node: step.node() });
                }
            }
        }
    }

    let mut criterion_evidence = Vec::with_capacity(completion.criteria.len());
    for (index, report) in completion.criteria.iter().enumerate() {
        let criterion_index = CompletionCriterionIndex(index);
        let latest = executions.iter().rev().find(|execution| {
            execution.criterion() == &report.criterion
                && execution.finished_result() == &completion.finished_result
        });
        let matching = latest.filter(|execution| match (&report.status, execution.outcome()) {
            (
                CompletionCriterionStatus::Passed {
                    observed_result: expected,
                },
                CriterionExecutionOutcome::Passed {
                    observed_result: actual,
                },
            )
            | (
                CompletionCriterionStatus::Failed {
                    observed_result: expected,
                },
                CriterionExecutionOutcome::Failed {
                    observed_result: actual,
                },
            ) => expected == actual,
            (
                CompletionCriterionStatus::Unpaid { reason: expected },
                CriterionExecutionOutcome::Unpaid { reason: actual },
            ) => expected == actual,
            (CompletionCriterionStatus::Missing, _)
            | (CompletionCriterionStatus::Passed { .. }, _)
            | (CompletionCriterionStatus::Failed { .. }, _)
            | (CompletionCriterionStatus::Unpaid { .. }, _) => false,
        });
        if let Some(execution) = matching {
            criterion_evidence.push(LandingCriterionEvidence::Recorded {
                criterion_index,
                sequence: execution.sequence(),
                node: execution.node(),
                criterion: execution.criterion(),
                finished_result: execution.finished_result(),
                outcome: execution.outcome(),
                evidence: execution.evidence(),
            });
        } else {
            criterion_evidence.push(LandingCriterionEvidence::Missing { criterion_index });
            problems.push(LandingReadinessProblem::CriterionExecutionMissing { criterion_index });
        }
    }

    let decision = if problems.is_empty() {
        LandingReadinessDecision::Ready
    } else {
        LandingReadinessDecision::Refuse
    };
    LandingReadinessResult {
        decision,
        completion,
        steps: step_snapshots,
        criterion_evidence,
        problems,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use crate::{
        CompletionCriterionReport, CompletionCriterionStatus, CompletionDecision,
        CompletionGateResult, ExactPullRequestIdentity, ExactPullRequestState,
        GitAuthorityObservation, GitHubAuthorityObservation, GitHubPullRequestObservation,
        GitMergeObservation, MergeSubject, NodeId, ObservedCriterionResult, PullRequestNumber,
        RecoveryLogPath, SquashCommitOid, StepAuthorityObservation, StepNode,
        UnpaidCriterionReason, VisionSlug, derive_run_state, evaluate_completion,
        parse_acceptance_criteria, parse_event_line,
    };
    use serde_json::{Value, json};

    use super::{
        LandingCriterionEvidence, LandingReadinessDecision, LandingReadinessProblem,
        evaluate_landing_readiness,
    };

    const RESULT: &str = "main@0123456789abcdef";

    fn criteria(names: &[&str]) -> crate::AcceptanceCriteria {
        let values = names
            .iter()
            .map(|name| {
                json!({"name":name,"input":format!("Run {name}."),"observation":"It exits 0."})
            })
            .collect::<Vec<_>>();
        parse_acceptance_criteria(&format!(
            "# Vision\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{}\n```\n",
            json!({"criteria": values})
        ))
        .expect("criteria fixture")
    }

    fn execution(
        sequence: u64,
        criterion: &crate::AcceptanceCriterion,
        finished_result: &str,
        outcome: Value,
        evidence: &str,
    ) -> crate::EventRecord {
        execution_for_node(
            sequence,
            "m6-s1",
            criterion,
            finished_result,
            outcome,
            evidence,
        )
    }

    fn execution_for_node(
        sequence: u64,
        node: &str,
        criterion: &crate::AcceptanceCriterion,
        finished_result: &str,
        outcome: Value,
        evidence: &str,
    ) -> crate::EventRecord {
        parse_event_line(
            &json!({
                "sequence":sequence,
                "timestamp":format!("2026-08-03T00:00:{sequence:02}.000Z"),
                "kind":"criterion-execution",
                "node":node,
                "payload":{"criterion":criterion,"finished_result":finished_result,"outcome":outcome,"evidence":evidence}
            })
            .to_string(),
        )
        .expect("execution fixture")
    }

    fn merged_authority(vision: &VisionSlug, node: &str) -> StepAuthorityObservation {
        let node_id = NodeId::parse(node).expect("node fixture");
        let subject =
            MergeSubject::derive(vision, StepNode::parse(&node_id).expect("step fixture"));
        let squash = SquashCommitOid::parse("squash-landing-oid").expect("squash fixture");
        StepAuthorityObservation::new(
            node_id,
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::OneExactMatch {
                    identity: ExactPullRequestIdentity::from_selector(
                        PullRequestNumber::parse(61).expect("PR fixture"),
                        subject.selector(),
                    ),
                    state: ExactPullRequestState::Merged {
                        squash_commit: squash.clone(),
                    },
                },
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::SquashCommitReachable {
                    squash_commit: squash,
                },
            },
        )
    }

    fn not_merged_authority(node: &str) -> StepAuthorityObservation {
        StepAuthorityObservation::new(
            NodeId::parse(node).expect("node fixture"),
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::ZeroExactMatches,
            },
            GitAuthorityObservation::Reachable {
                observation: GitMergeObservation::NotMerged,
            },
        )
    }

    fn inconclusive_authority(node: &str) -> StepAuthorityObservation {
        StepAuthorityObservation::new(
            NodeId::parse(node).expect("node fixture"),
            GitHubAuthorityObservation::Reachable {
                observation: GitHubPullRequestObservation::MultipleExactMatches,
            },
            GitAuthorityObservation::Unreachable {
                failure: crate::AuthorityFailure::parse("ambiguous authority")
                    .expect("failure fixture"),
            },
        )
    }

    fn state(
        criteria: &crate::AcceptanceCriteria,
        records: &[crate::EventRecord],
        authorities: &[StepAuthorityObservation],
    ) -> Result<crate::DerivedRunState, Box<dyn Error>> {
        Ok(derive_run_state(
            records,
            criteria,
            &VisionSlug::parse("2026-08-03-landing-fixture")?,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            authorities,
        )?)
    }

    #[test]
    fn complete_merged_and_evidenced_is_ready_in_effective_order() -> Result<(), Box<dyn Error>> {
        let criteria = criteria(&["Runnable criterion", "Install-only criterion"]);
        let records = vec![
            execution(
                3,
                &criteria.as_slice()[0],
                RESULT,
                json!({"status":"passed","observed_result":"The command exited 0."}),
                "run the finished command",
            ),
            execution(
                4,
                &criteria.as_slice()[1],
                RESULT,
                json!({"status":"unpaid","reason":"The run cannot activate the human-installed hook."}),
                "inspect the human-install boundary",
            ),
        ];
        let vision = VisionSlug::parse("2026-08-03-landing-fixture")?;
        let state = state(&criteria, &records, &[merged_authority(&vision, "m6-s1")])?;
        let completion = evaluate_completion(
            &crate::FinishedResult::parse(RESULT)?,
            state.blocking_criteria(),
            state.criterion_executions(),
        );
        let result =
            evaluate_landing_readiness(&completion, state.steps(), state.criterion_executions());
        assert_eq!(result.decision, LandingReadinessDecision::Ready);
        assert!(result.problems.is_empty());
        assert_eq!(result.criterion_evidence.len(), 2);
        assert_eq!(
            serde_json::to_value(&result)?["steps"][0]["merge_status"],
            "merged"
        );
        Ok(())
    }

    #[test]
    fn failed_and_missing_completion_refuses_without_hiding_unpaid() -> Result<(), Box<dyn Error>> {
        let criteria = criteria(&[
            "Missing criterion",
            "Failing criterion",
            "Install-only criterion",
        ]);
        let records = vec![
            execution(
                3,
                &criteria.as_slice()[1],
                RESULT,
                json!({"status":"failed","observed_result":"The command exited 7."}),
                "run the broken command",
            ),
            execution(
                4,
                &criteria.as_slice()[2],
                RESULT,
                json!({"status":"unpaid","reason":"The run cannot activate the human-installed hook."}),
                "inspect the human-install boundary",
            ),
        ];
        let vision = VisionSlug::parse("2026-08-03-landing-fixture")?;
        let state = state(&criteria, &records, &[merged_authority(&vision, "m6-s1")])?;
        let completion = evaluate_completion(
            &crate::FinishedResult::parse(RESULT)?,
            state.blocking_criteria(),
            state.criterion_executions(),
        );
        let result =
            evaluate_landing_readiness(&completion, state.steps(), state.criterion_executions());
        assert_eq!(result.decision, LandingReadinessDecision::Refuse);
        assert!(matches!(
            result.criterion_evidence[0],
            LandingCriterionEvidence::Missing { .. }
        ));
        assert!(matches!(
            result.criterion_evidence[2],
            LandingCriterionEvidence::Recorded { .. }
        ));
        assert_eq!(
            serde_json::to_value(&result)?["problems"],
            json!([{"reason":"completion-refused"},{"reason":"criterion-execution-missing","criterion_index":0}])
        );
        Ok(())
    }

    #[test]
    fn not_merged_inconclusive_and_empty_authorities_each_refuse() -> Result<(), Box<dyn Error>> {
        let criteria = criteria(&["Runnable criterion"]);
        let records = vec![
            execution(
                1,
                &criteria.as_slice()[0],
                RESULT,
                json!({"status":"passed","observed_result":"ok"}),
                "run",
            ),
            execution_for_node(
                2,
                "m6-s2",
                &criteria.as_slice()[0],
                RESULT,
                json!({"status":"passed","observed_result":"ok"}),
                "rerun",
            ),
        ];
        let state = state(
            &criteria,
            &records,
            &[
                not_merged_authority("m6-s1"),
                inconclusive_authority("m6-s2"),
            ],
        )?;
        let completion = evaluate_completion(
            &crate::FinishedResult::parse(RESULT)?,
            state.blocking_criteria(),
            state.criterion_executions(),
        );
        let result =
            evaluate_landing_readiness(&completion, state.steps(), state.criterion_executions());
        assert!(matches!(
            result.problems[0],
            LandingReadinessProblem::StepNotMerged { .. }
        ));
        assert!(matches!(
            result.problems[1],
            LandingReadinessProblem::StepInconclusive { .. }
        ));
        let empty = evaluate_landing_readiness(&completion, &[], state.criterion_executions());
        assert!(matches!(
            empty.problems.as_slice(),
            [LandingReadinessProblem::NoStepAuthorities]
        ));
        Ok(())
    }

    #[test]
    fn latest_exact_execution_must_match_the_completion_report() -> Result<(), Box<dyn Error>> {
        let criteria = criteria(&["Runnable criterion"]);
        let records = vec![
            execution(
                1,
                &criteria.as_slice()[0],
                RESULT,
                json!({"status":"passed","observed_result":"The command exited 0."}),
                "older",
            ),
            execution(
                2,
                &criteria.as_slice()[0],
                RESULT,
                json!({"status":"unpaid","reason":"The run cannot rerun the promoted binary."}),
                "newer",
            ),
        ];
        let vision = VisionSlug::parse("2026-08-03-landing-fixture")?;
        let state = state(&criteria, &records, &[merged_authority(&vision, "m6-s1")])?;
        let constructed = CompletionGateResult {
            decision: CompletionDecision::Complete,
            finished_result: crate::FinishedResult::parse(RESULT)?,
            criteria: vec![CompletionCriterionReport {
                criterion: criteria.as_slice()[0].clone(),
                status: CompletionCriterionStatus::Passed {
                    observed_result: ObservedCriterionResult::parse("The command exited 0.")?,
                },
            }],
        };
        let refused =
            evaluate_landing_readiness(&constructed, state.steps(), state.criterion_executions());
        assert!(matches!(
            refused.criterion_evidence[0],
            LandingCriterionEvidence::Missing { .. }
        ));
        let actual = evaluate_completion(
            &constructed.finished_result,
            state.blocking_criteria(),
            state.criterion_executions(),
        );
        let ready =
            evaluate_landing_readiness(&actual, state.steps(), state.criterion_executions());
        assert_eq!(ready.decision, LandingReadinessDecision::Ready);
        Ok(())
    }

    #[test]
    fn different_finished_result_and_non_effective_execution_are_unevidenced()
    -> Result<(), Box<dyn Error>> {
        let all_criteria = criteria(&["Runnable criterion", "Outside criterion"]);
        let records = vec![
            execution(
                1,
                &all_criteria.as_slice()[0],
                "main@fedcba9876543210",
                json!({"status":"passed","observed_result":"ok"}),
                "other result",
            ),
            execution(
                2,
                &all_criteria.as_slice()[1],
                RESULT,
                json!({"status":"passed","observed_result":"ok"}),
                "outside",
            ),
        ];
        let only = criteria(&["Runnable criterion"]);
        let state = state(&only, &records, &[])?;
        let completion = evaluate_completion(
            &crate::FinishedResult::parse(RESULT)?,
            state.blocking_criteria(),
            state.criterion_executions(),
        );
        let result = evaluate_landing_readiness(&completion, &[], state.criterion_executions());
        assert!(matches!(
            result.criterion_evidence.as_slice(),
            [LandingCriterionEvidence::Missing { .. }]
        ));
        Ok(())
    }

    #[test]
    fn criterion_status_and_outcome_values_must_match_exactly() -> Result<(), Box<dyn Error>> {
        let criteria = criteria(&["Runnable criterion"]);
        let record = execution(
            1,
            &criteria.as_slice()[0],
            RESULT,
            json!({"status":"passed","observed_result":"actual"}),
            "run",
        );
        let state = state(&criteria, &[record], &[])?;
        let completion = CompletionGateResult {
            decision: CompletionDecision::Complete,
            finished_result: crate::FinishedResult::parse(RESULT)?,
            criteria: vec![CompletionCriterionReport {
                criterion: criteria.as_slice()[0].clone(),
                status: CompletionCriterionStatus::Passed {
                    observed_result: ObservedCriterionResult::parse("expected")?,
                },
            }],
        };
        let mismatch = evaluate_landing_readiness(&completion, &[], state.criterion_executions());
        assert!(matches!(
            mismatch.criterion_evidence[0],
            LandingCriterionEvidence::Missing { .. }
        ));
        let exact = CompletionGateResult {
            criteria: vec![CompletionCriterionReport {
                criterion: criteria.as_slice()[0].clone(),
                status: CompletionCriterionStatus::Passed {
                    observed_result: ObservedCriterionResult::parse("actual")?,
                },
            }],
            ..completion
        };
        let control = evaluate_landing_readiness(&exact, &[], state.criterion_executions());
        assert!(matches!(
            control.criterion_evidence[0],
            LandingCriterionEvidence::Recorded { .. }
        ));
        let _ = UnpaidCriterionReason::parse("reason")?;
        Ok(())
    }
}
