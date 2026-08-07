//! criterion_change : Ordered<BlockingCriterion> × AcceptanceCriteria → CriterionChangeVerification   (pure, deterministic)
//!
//! Compares a proposed criterion document with the complete effective blocking list.

use serde::Serialize;

use crate::{AcceptanceCriteria, AcceptanceCriterion, BlockingCriterion};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriterionChangeDecision {
    Accept,
    Refuse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CriterionChangeVerification {
    pub decision: CriterionChangeDecision,
    pub required_criteria: Vec<AcceptanceCriterion>,
    pub proposed_criteria: Vec<AcceptanceCriterion>,
}

pub fn verify_criterion_change(
    blocking_criteria: &[BlockingCriterion],
    proposed_criteria: &AcceptanceCriteria,
) -> CriterionChangeVerification {
    let required_criteria = blocking_criteria
        .iter()
        .map(|blocking| blocking.criterion().clone())
        .collect::<Vec<_>>();
    let proposed_criteria = proposed_criteria.as_slice().to_vec();
    let decision = if required_criteria == proposed_criteria {
        CriterionChangeDecision::Accept
    } else {
        CriterionChangeDecision::Refuse
    };
    CriterionChangeVerification {
        decision,
        required_criteria,
        proposed_criteria,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AcceptanceCriteria, RecoveryLogPath, VisionSlug, derive_run_state,
        parse_acceptance_criteria, parse_event_line,
    };

    use super::{CriterionChangeDecision, verify_criterion_change};

    const FIRST: &str = r#"{"name":"Ratified criterion","input":"Run the ratified probe.","observation":"The probe exits 0."}"#;
    const SECOND: &str = r#"{"name":"Second ratified criterion","input":"Run the second probe.","observation":"The second probe exits 0."}"#;
    const ADDED: &str = r#"{"name":"Added criterion","input":"Run the added probe.","observation":"The added probe exits 0."}"#;

    fn criteria(entries: &str) -> AcceptanceCriteria {
        parse_acceptance_criteria(&format!(
            "# Vision\n\n## Acceptance criteria (vision-level \"done\")\n\n```json\n{{\"criteria\":[{entries}]}}\n```\n"
        ))
        .expect("criterion fixture")
    }

    fn state(records: &[crate::EventRecord], floor: &AcceptanceCriteria) -> crate::DerivedRunState {
        derive_run_state(
            records,
            floor,
            &VisionSlug::parse("2026-08-03-criterion-change-fixture").expect("vision slug"),
            &RecoveryLogPath::new("events.jsonl"),
            &[],
            &[],
            &[],
        )
        .expect("run state")
    }

    fn added_event(sequence: u64, criterion: &str, change: &str) -> crate::EventRecord {
        parse_event_line(&format!(
            r#"{{"sequence":{sequence},"timestamp":"2026-08-03T12:00:00.000Z","kind":"criterion-added","node":"m4-s1","payload":{{"criterion":{criterion},"change_of_course":"{change}"}}}}"#
        ))
        .expect("criterion-added event")
    }

    #[test]
    fn unchanged_floor_is_accepted() {
        let floor = criteria(FIRST);
        let state = state(&[], &floor);
        let result = verify_criterion_change(state.blocking_criteria(), &criteria(FIRST));
        assert_eq!(result.decision, CriterionChangeDecision::Accept);
        assert_eq!(result.required_criteria.len(), 1);
        assert_eq!(result.proposed_criteria.len(), 1);
        assert_eq!(result.required_criteria, floor.as_slice());
        assert_eq!(result.proposed_criteria, floor.as_slice());
    }

    #[test]
    fn ratified_removal_reordering_and_each_field_change_are_refused() {
        let floor = criteria(&format!("{FIRST},{SECOND}"));
        let state = state(&[], &floor);
        let candidates = [
            SECOND.to_owned(),
            format!("{SECOND},{FIRST}"),
            r#"{"name":"Renamed criterion","input":"Run the ratified probe.","observation":"The probe exits 0."}"#.to_owned(),
            r#"{"name":"Ratified criterion","input":"Run a weaker probe.","observation":"The probe exits 0."}"#.to_owned(),
            r#"{"name":"Ratified criterion","input":"Run the ratified probe.","observation":"The probe may exit 0."}"#.to_owned(),
        ];
        for candidate in candidates {
            let proposed = criteria(&candidate);
            let result = verify_criterion_change(state.blocking_criteria(), &proposed);
            assert_eq!(result.decision, CriterionChangeDecision::Refuse);
            assert_eq!(result.required_criteria, floor.as_slice());
            assert_eq!(result.proposed_criteria, proposed.as_slice());
        }
    }

    #[test]
    fn unlogged_growth_is_refused_and_logged_growth_is_accepted() {
        let floor = criteria(FIRST);
        let proposed = criteria(&format!("{FIRST},{ADDED}"));
        let empty = state(&[], &floor);
        assert_eq!(
            verify_criterion_change(empty.blocking_criteria(), &proposed).decision,
            CriterionChangeDecision::Refuse
        );
        let records = [added_event(
            1,
            ADDED,
            "Reality exposed an uncovered failure.",
        )];
        let logged = state(&records, &floor);
        let accepted = verify_criterion_change(logged.blocking_criteria(), &proposed);
        assert_eq!(accepted.decision, CriterionChangeDecision::Accept);
        assert_eq!(accepted.required_criteria, proposed.as_slice());
        assert_eq!(accepted.proposed_criteria, proposed.as_slice());
        let dropped = verify_criterion_change(logged.blocking_criteria(), &floor);
        assert_eq!(dropped.decision, CriterionChangeDecision::Refuse);
        assert_eq!(dropped.required_criteria, proposed.as_slice());
    }

    #[test]
    fn duplicates_and_event_order_are_not_normalized() {
        let floor = criteria(FIRST);
        let records = [
            added_event(1, FIRST, "A duplicate became independently blocking."),
            added_event(2, ADDED, "Reality exposed an uncovered failure."),
        ];
        let state = state(&records, &floor);
        let exact = criteria(&format!("{FIRST},{FIRST},{ADDED}"));
        let accepted = verify_criterion_change(state.blocking_criteria(), &exact);
        assert_eq!(accepted.decision, CriterionChangeDecision::Accept);
        assert_eq!(accepted.required_criteria, exact.as_slice());
        assert_eq!(accepted.proposed_criteria, exact.as_slice());
        assert_eq!(
            verify_criterion_change(
                state.blocking_criteria(),
                &criteria(&format!("{FIRST},{ADDED}"))
            )
            .decision,
            CriterionChangeDecision::Refuse
        );
        assert_eq!(
            verify_criterion_change(
                state.blocking_criteria(),
                &criteria(&format!("{FIRST},{ADDED},{FIRST}"))
            )
            .decision,
            CriterionChangeDecision::Refuse
        );
    }
}
