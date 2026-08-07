//! completion_gate : FinishedResult × EffectiveCriteria × Ordered<CriterionExecutionObservation> → CompletionGateResult   (pure, deterministic)

use crate::{
    AcceptanceCriterion, BlockingCriterion, CriterionExecutionObservation,
    CriterionExecutionOutcome, FinishedResult, ObservedCriterionResult, UnpaidCriterionReason,
};
use serde::Serialize;

/// Whether the finished result has paid every effective blocking criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompletionDecision {
    Complete,
    Refuse,
}

/// The folded execution status for one effective criterion and finished result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum CompletionCriterionStatus {
    Passed {
        observed_result: ObservedCriterionResult,
    },
    Failed {
        observed_result: ObservedCriterionResult,
    },
    Unpaid {
        reason: UnpaidCriterionReason,
    },
    Missing,
}

/// One effective criterion paired with its folded execution status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompletionCriterionReport {
    pub criterion: AcceptanceCriterion,
    #[serde(flatten)]
    pub status: CompletionCriterionStatus,
}

/// The deterministic completion decision for one finished result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompletionGateResult {
    pub decision: CompletionDecision,
    pub finished_result: FinishedResult,
    pub criteria: Vec<CompletionCriterionReport>,
}

/// Evaluate the folded exact execution evidence for every effective criterion.
pub fn evaluate_completion(
    finished_result: &FinishedResult,
    effective_criteria: &[BlockingCriterion],
    executions: &[CriterionExecutionObservation],
) -> CompletionGateResult {
    let criteria = effective_criteria
        .iter()
        .map(|blocking| {
            let criterion = blocking.criterion();
            let status = executions
                .iter()
                .filter(|execution| {
                    execution.criterion() == criterion
                        && execution.finished_result() == finished_result
                })
                .fold(
                    CompletionCriterionStatus::Missing,
                    |current, execution| match execution.outcome() {
                        CriterionExecutionOutcome::Passed { observed_result } => {
                            CompletionCriterionStatus::Passed {
                                observed_result: observed_result.clone(),
                            }
                        }
                        CriterionExecutionOutcome::Failed { observed_result } => {
                            CompletionCriterionStatus::Failed {
                                observed_result: observed_result.clone(),
                            }
                        }
                        CriterionExecutionOutcome::Unpaid { reason } => match current {
                            failed @ CompletionCriterionStatus::Failed { .. } => failed,
                            _ => CompletionCriterionStatus::Unpaid {
                                reason: reason.clone(),
                            },
                        },
                    },
                );
            CompletionCriterionReport {
                criterion: criterion.clone(),
                status,
            }
        })
        .collect::<Vec<_>>();
    let decision = if criteria.iter().any(|report| {
        matches!(
            report.status,
            CompletionCriterionStatus::Failed { .. } | CompletionCriterionStatus::Missing
        )
    }) {
        CompletionDecision::Refuse
    } else {
        CompletionDecision::Complete
    };
    CompletionGateResult {
        decision,
        finished_result: finished_result.clone(),
        criteria,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompletionCriterionStatus, CompletionDecision, CompletionGateResult, evaluate_completion,
    };
    use crate::{
        AcceptanceCriteria, FinishedResult, RecoveryLogPath, VisionSlug, derive_run_state,
        parse_acceptance_criteria, parse_event_line,
    };
    use std::error::Error;

    const RUNNABLE: &str = r#"{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."}"#;
    const RESULT: &str = "main@0123456789abcdef";

    fn vision(criteria: &str) -> AcceptanceCriteria {
        parse_acceptance_criteria(&format!("# Vision: fixture\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{{\"criteria\":[{criteria}]}}\n```\n")).expect("criterion fixture")
    }

    fn execution(criterion: &str, result: &str, outcome: &str) -> String {
        format!(
            r#"{{"criterion":{criterion},"finished_result":"{result}","outcome":{outcome},"evidence":"run fixture"}}"#
        )
    }

    fn evaluate(criteria: &str, payloads: &[&str]) -> Result<CompletionGateResult, Box<dyn Error>> {
        let records = payloads.iter().enumerate().map(|(index, payload)| {
            let kind = if payload.contains("change_of_course") { "criterion-added" } else { "criterion-execution" };
            parse_event_line(&format!(r#"{{"sequence":{},"timestamp":"2026-08-03T12:00:{:02}.000Z","kind":"{kind}","node":"m3-s3","payload":{payload}}}"#, index + 1, index))
        }).collect::<Result<Vec<_>, _>>()?;
        let state = derive_run_state(
            &records,
            &vision(criteria),
            &VisionSlug::parse("2026-08-03-a-gate-runs-what-was-built")?,
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )?;
        Ok(evaluate_completion(
            &FinishedResult::parse(RESULT)?,
            state.blocking_criteria(),
            state.criterion_executions(),
        ))
    }

    #[test]
    fn all_latest_results_pass_completes() -> Result<(), Box<dyn Error>> {
        let probe = r#"{"name":"Probe criterion","input":"Run the finished probe.","observation":"It exits 0."}"#;
        let first = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"passed","observed_result":"The command exited 0."}"#,
        );
        let second = execution(
            probe,
            RESULT,
            r#"{"status":"passed","observed_result":"The probe exited 0."}"#,
        );
        let result = evaluate(&format!("{RUNNABLE},{probe}"), &[&first, &second])?;
        assert_eq!(result.decision, CompletionDecision::Complete);
        assert_eq!(result.finished_result.as_str(), RESULT);
        assert_eq!(result.criteria.len(), 2);
        assert!(
            matches!(&result.criteria[0].status, CompletionCriterionStatus::Passed { observed_result } if observed_result.as_str() == "The command exited 0.")
        );
        assert!(
            matches!(&result.criteria[1].status, CompletionCriterionStatus::Passed { observed_result } if observed_result.as_str() == "The probe exited 0.")
        );
        Ok(())
    }

    #[test]
    fn missing_failed_and_unpaid_refuse_in_effective_order() -> Result<(), Box<dyn Error>> {
        let missing = r#"{"name":"Missing criterion","input":"Run the missing command.","observation":"It exits 0."}"#;
        let failing = r#"{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."}"#;
        let unpaid = r#"{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."}"#;
        let failed = execution(
            failing,
            RESULT,
            r#"{"status":"failed","observed_result":"The command exited 7."}"#,
        );
        let unpaid_event = execution(
            unpaid,
            RESULT,
            r#"{"status":"unpaid","reason":"The run cannot activate the human-installed hook."}"#,
        );
        let result = evaluate(
            &format!("{missing},{failing},{unpaid}"),
            &[&failed, &unpaid_event],
        )?;
        assert_eq!(result.decision, CompletionDecision::Refuse);
        assert_eq!(
            result
                .criteria
                .iter()
                .map(|report| report.criterion.name().as_str())
                .collect::<Vec<_>>(),
            [
                "Missing criterion",
                "Failing criterion",
                "Install-only criterion"
            ]
        );
        assert!(matches!(
            result.criteria[0].status,
            CompletionCriterionStatus::Missing
        ));
        assert!(
            matches!(&result.criteria[1].status, CompletionCriterionStatus::Failed { observed_result } if observed_result.as_str() == "The command exited 7.")
        );
        assert!(
            matches!(&result.criteria[2].status, CompletionCriterionStatus::Unpaid { reason } if reason.as_str() == "The run cannot activate the human-installed hook.")
        );
        Ok(())
    }

    #[test]
    fn unpaid_is_named_but_does_not_refuse() -> Result<(), Box<dyn Error>> {
        let unpaid = r#"{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."}"#;
        let event = execution(
            unpaid,
            RESULT,
            r#"{"status":"unpaid","reason":"The run cannot activate the human-installed hook."}"#,
        );
        let result = evaluate(unpaid, &[&event])?;
        assert_eq!(result.decision, CompletionDecision::Complete);
        assert_eq!(
            result.criteria[0].criterion.input().as_str(),
            "Install the hook, then attempt the forbidden command."
        );
        assert_eq!(
            result.criteria[0].criterion.observation().as_str(),
            "The command is denied."
        );
        assert!(
            matches!(&result.criteria[0].status, CompletionCriterionStatus::Unpaid { reason } if reason.as_str() == "The run cannot activate the human-installed hook.")
        );
        Ok(())
    }

    #[test]
    fn later_unpaid_does_not_supersede_failed_execution() -> Result<(), Box<dyn Error>> {
        let failed = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"failed","observed_result":"The command exited 7."}"#,
        );
        let unpaid = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"unpaid","reason":"The run cannot execute the command now."}"#,
        );
        let result = evaluate(RUNNABLE, &[&failed, &unpaid])?;
        assert_eq!(result.decision, CompletionDecision::Refuse);
        assert_eq!(result.criteria.len(), 1);
        assert!(
            matches!(&result.criteria[0].status, CompletionCriterionStatus::Failed { observed_result } if observed_result.as_str() == "The command exited 7.")
        );
        Ok(())
    }

    #[test]
    fn later_passed_supersedes_failed_execution_after_repair() -> Result<(), Box<dyn Error>> {
        let failed = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"failed","observed_result":"The command exited 7."}"#,
        );
        let passed = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"passed","observed_result":"The command exited 0."}"#,
        );
        let result = evaluate(RUNNABLE, &[&failed, &passed])?;
        assert_eq!(result.decision, CompletionDecision::Complete);
        assert_eq!(result.criteria.len(), 1);
        assert!(
            matches!(&result.criteria[0].status, CompletionCriterionStatus::Passed { observed_result } if observed_result.as_str() == "The command exited 0.")
        );
        Ok(())
    }

    #[test]
    fn later_failed_supersedes_passed_execution() -> Result<(), Box<dyn Error>> {
        let passed = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"passed","observed_result":"The command exited 0."}"#,
        );
        let failed = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"failed","observed_result":"The command exited 7."}"#,
        );
        let result = evaluate(RUNNABLE, &[&passed, &failed])?;
        assert_eq!(result.decision, CompletionDecision::Refuse);
        assert_eq!(result.criteria.len(), 1);
        assert!(
            matches!(&result.criteria[0].status, CompletionCriterionStatus::Failed { observed_result } if observed_result.as_str() == "The command exited 7.")
        );
        Ok(())
    }

    #[test]
    fn different_result_and_non_effective_evidence_do_not_pay_criteria()
    -> Result<(), Box<dyn Error>> {
        let outside =
            r#"{"name":"Outside criterion","input":"Run outside.","observation":"It exits 0."}"#;
        let older = execution(
            RUNNABLE,
            "main@fedcba9876543210",
            r#"{"status":"passed","observed_result":"The command exited 0."}"#,
        );
        let extra = execution(
            outside,
            RESULT,
            r#"{"status":"passed","observed_result":"Outside exited 0."}"#,
        );
        let result = evaluate(RUNNABLE, &[&older, &extra])?;
        assert_eq!(result.decision, CompletionDecision::Refuse);
        assert_eq!(result.criteria.len(), 1);
        assert_eq!(
            result.criteria[0].criterion.name().as_str(),
            "Runnable criterion"
        );
        assert!(matches!(
            result.criteria[0].status,
            CompletionCriterionStatus::Missing
        ));
        Ok(())
    }

    #[test]
    fn duplicate_effective_criteria_remain_visible() -> Result<(), Box<dyn Error>> {
        let addition = format!(
            r#"{{"criterion":{RUNNABLE},"change_of_course":"Reality exposed an uncovered failure."}}"#
        );
        let passed = execution(
            RUNNABLE,
            RESULT,
            r#"{"status":"passed","observed_result":"The command exited 0."}"#,
        );
        let result = evaluate(RUNNABLE, &[&addition, &passed])?;
        assert_eq!(result.decision, CompletionDecision::Complete);
        assert_eq!(result.criteria.len(), 2);
        assert!(result.criteria.iter().all(|report| report.criterion.name().as_str() == "Runnable criterion" && matches!(&report.status, CompletionCriterionStatus::Passed { observed_result } if observed_result.as_str() == "The command exited 0.")));
        Ok(())
    }
}
