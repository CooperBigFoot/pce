//! paired_execution_proof : BrokenCampaign × RepairedCampaign × ReplayClassifications → PairedProofDecision   (pure, deterministic)
//! Pure parsing, probe identity, and paired admission for the paired execution campaign.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{GateExecutionEvidence, GateExecutionRef, GateStimulus};

const SUBJECT_ROOT_MARKER: &str = "<EXECUTION_SUBJECT_ROOT>";

/// The campaign side whose independently recorded evidence is being considered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignSide {
    Broken,
    Repaired,
}

/// The only outer-verdict tokens admitted by the eight-field verdict boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum FalsificationVerdictToken {
    Approve,
    Revise,
    Block,
}

/// One parsed blocking issue, including both same-dispatch evidence references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedBlockingIssue {
    id: String,
    primary: Option<GateExecutionRef>,
    replacement: Option<GateExecutionRef>,
}

impl PairedBlockingIssue {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn primary(&self) -> Option<&GateExecutionRef> {
        self.primary.as_ref()
    }

    pub fn replacement(&self) -> Option<&GateExecutionRef> {
        self.replacement.as_ref()
    }
}

/// The report-relevant projection of one structurally parsed outer verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedFalsificationVerdict {
    token: FalsificationVerdictToken,
    blocking_issues: Vec<PairedBlockingIssue>,
}

impl PairedFalsificationVerdict {
    pub fn token(&self) -> FalsificationVerdictToken {
        self.token
    }

    pub fn blocking_issues(&self) -> &[PairedBlockingIssue] {
        &self.blocking_issues
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VerdictWire {
    verdict: FalsificationVerdictToken,
    self_sufficiency: SelfSufficiencyWire,
    root_cause: RootCauseWire,
    blocking_issues: Vec<BlockingIssueWire>,
    non_blocking_notes: Vec<String>,
    summary: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SelfSufficiencyWire {
    Pass,
    Fail,
    NotApplicable,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RootCauseWire {
    Execution,
    StepPlan,
    MilestonePlan,
    Vision,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockingIssueWire {
    id: String,
    severity: SeverityWire,
    location: String,
    problem: String,
    input: String,
    observation: String,
    execution_ref: Option<String>,
    required_change: String,
    replacement_execution: ReplacementWire,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum SeverityWire {
    Critical,
    Major,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplacementWire {
    input: String,
    observation: String,
    execution_ref: Option<String>,
}

/// Parse the current eight-field falsification verdict without consulting schema I/O.
///
/// # Errors
///
/// Returns a typed error for malformed JSON, trailing input, invalid references, or empty fields
/// whose non-emptiness is part of the embedded schema.
pub fn parse_paired_falsification_verdict(
    bytes: &[u8],
) -> Result<PairedFalsificationVerdict, PairedExecutionProofError> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let wire = VerdictWire::deserialize(&mut deserializer)
        .map_err(|source| PairedExecutionProofError::InvalidVerdict { source })?;
    deserializer
        .end()
        .map_err(|source| PairedExecutionProofError::InvalidVerdict { source })?;
    let _unused = (
        &wire.self_sufficiency,
        &wire.root_cause,
        &wire.non_blocking_notes,
        &wire.summary,
    );
    let blocking_issues = wire
        .blocking_issues
        .into_iter()
        .map(|issue| {
            let _unused = (
                &issue.severity,
                &issue.location,
                &issue.problem,
                &issue.input,
                &issue.observation,
                &issue.required_change,
                &issue.replacement_execution.input,
                &issue.replacement_execution.observation,
            );
            if issue.id.is_empty()
                || issue.input.is_empty()
                || issue.observation.is_empty()
                || issue.required_change.is_empty()
                || issue.replacement_execution.input.is_empty()
                || issue.replacement_execution.observation.is_empty()
            {
                return Err(PairedExecutionProofError::EmptyVerdictField);
            }
            Ok(PairedBlockingIssue {
                id: issue.id,
                primary: issue
                    .execution_ref
                    .map(GateExecutionRef::parse)
                    .transpose()?,
                replacement: issue
                    .replacement_execution
                    .execution_ref
                    .map(GateExecutionRef::parse)
                    .transpose()?,
            })
        })
        .collect::<Result<Vec<_>, PairedExecutionProofError>>()?;
    Ok(PairedFalsificationVerdict {
        token: wire.verdict,
        blocking_issues,
    })
}

/// Whether the landed same-dispatch validator accepted every blocking-issue reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceValidation {
    Passed,
    Failed,
}

/// The replay classification retained for one exact evidence record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairedReplayClassification {
    RepairSensitive,
    NoRepairSignal,
    OppositeDirection,
    NonReproducible,
    CheckoutFailed,
    OracleFailed,
}

/// Replay classifications keyed by their canonical recorded execution reference.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReplayClassifications(BTreeMap<GateExecutionRef, PairedReplayClassification>);

impl ReplayClassifications {
    pub fn new(values: BTreeMap<GateExecutionRef, PairedReplayClassification>) -> Self {
        Self(values)
    }

    pub fn get(&self, reference: &GateExecutionRef) -> Option<PairedReplayClassification> {
        self.0.get(reference).copied()
    }
}

/// One side's complete immutable inputs to admission.
#[derive(Debug)]
pub struct PairedCampaign<'a> {
    pub side: CampaignSide,
    pub root: &'a Path,
    pub verdict: &'a PairedFalsificationVerdict,
    pub evidence: &'a GateExecutionEvidence,
    pub reference_validation: ReferenceValidation,
    pub replays: &'a ReplayClassifications,
}

/// Exact root-relocated identity of a recorded stimulus.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PairedStimulusIdentity(Vec<u8>);

fn relocate(value: &[u8], root: &[u8]) -> Vec<u8> {
    if root.is_empty() {
        return value.to_vec();
    }
    let mut output = Vec::new();
    let mut remaining = value;
    while let Some(index) = remaining.windows(root.len()).position(|part| part == root) {
        output.extend_from_slice(&remaining[..index]);
        output.extend_from_slice(SUBJECT_ROOT_MARKER.as_bytes());
        remaining = &remaining[index + root.len()..];
    }
    output.extend_from_slice(remaining);
    output
}

/// Produce the exact stimulus identity after replacing only the canonical recorded root.
///
/// # Errors
///
/// Returns an error when the root is not UTF-8 or the stimulus cwd is outside it.
pub fn paired_stimulus_identity(
    stimulus: &GateStimulus,
    root: &Path,
) -> Result<PairedStimulusIdentity, PairedExecutionProofError> {
    stimulus
        .working_directory()
        .strip_prefix(root)
        .map_err(|_| PairedExecutionProofError::StimulusOutsideCampaignRoot)?;
    let root_path = root;
    let root = root
        .to_str()
        .ok_or(PairedExecutionProofError::NonUtf8CampaignRoot)?
        .as_bytes();
    for process in stimulus
        .setup()
        .iter()
        .chain(std::iter::once(stimulus.command()))
    {
        let program = Path::new(process.program());
        if program.is_absolute() && program.strip_prefix(root_path).is_err() {
            return Err(PairedExecutionProofError::StimulusNamesFixedAbsolutePath);
        }
        if process.arguments().iter().any(|argument| {
            let path = Path::new(argument);
            path.is_absolute() && path.strip_prefix(root_path).is_err()
        }) {
            return Err(PairedExecutionProofError::StimulusNamesFixedAbsolutePath);
        }
    }
    let mut identity = Vec::new();
    append_part(
        &mut identity,
        &relocate(
            stimulus.working_directory().as_os_str().as_encoded_bytes(),
            root,
        ),
    );
    append_u64(&mut identity, stimulus.setup().len() as u64);
    for process in stimulus
        .setup()
        .iter()
        .chain(std::iter::once(stimulus.command()))
    {
        append_part(&mut identity, &relocate(process.program().as_bytes(), root));
        append_u64(&mut identity, process.arguments().len() as u64);
        for argument in process.arguments() {
            append_part(&mut identity, &relocate(argument.as_bytes(), root));
        }
        append_part(&mut identity, &relocate(process.input(), root));
        append_u64(&mut identity, process.environment().len() as u64);
        for (name, value) in process.environment() {
            append_part(&mut identity, name.as_bytes());
            append_part(&mut identity, &relocate(value.as_bytes(), root));
        }
    }
    Ok(PairedStimulusIdentity(identity))
}

fn append_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn append_part(output: &mut Vec<u8>, value: &[u8]) {
    append_u64(output, value.len() as u64);
    output.extend_from_slice(value);
}

/// Closed refusal reasons retained by the pure boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairedRefusalReason {
    BrokenReferenceValidationFailed,
    RepairedReferenceValidationFailed,
    BrokenVerdictIsNotBlock,
    RepairedVerdictIsNotApprove,
    BrokenHasNoBlockingIssue,
    RepairedHasBlockingIssue,
    BrokenHasNoQualifyingPrimary,
    RepairedHasNoQualifyingProbe,
    QualifyingProbesDoNotMatch,
}

/// Report-facing paired decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PairedProofDecision {
    Approve,
    Refuse,
}

/// Pure fold result with evidence-order qualifying references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedExecutionProofResult {
    pub decision: PairedProofDecision,
    pub broken_witnesses: Vec<GateExecutionRef>,
    pub repaired_probes: Vec<GateExecutionRef>,
    pub refusal_reasons: Vec<PairedRefusalReason>,
}

fn campaign_probe_identity(stimulus: &GateStimulus, root: &Path) -> Option<PairedStimulusIdentity> {
    const ARGUMENTS: [&str; 4] = [
        "gate",
        "execution-subject-probe",
        "--output",
        ".pce-execution-subject-verdict.json",
    ];
    let environment = stimulus.command().environment();
    if !stimulus.setup().is_empty()
        || stimulus.command().program() != "target/pce-execution-subject-probe/pce"
        || stimulus.command().arguments() != ARGUMENTS
        || !stimulus.command().input().is_empty()
        || environment.len() != 3
        || !environment.contains_key("PATH")
        || !environment.contains_key("HOME")
        || !environment.contains_key("USER")
    {
        return None;
    }
    paired_stimulus_identity(stimulus, root).ok()
}

/// Admit only a paired BLOCK/APPROVE proof carried by the same replayable behavioral probe.
pub fn fold_paired_execution_proof(
    broken: PairedCampaign<'_>,
    repaired: PairedCampaign<'_>,
) -> PairedExecutionProofResult {
    let mut refusal_reasons = Vec::new();
    if broken.side != CampaignSide::Broken
        || broken.reference_validation != ReferenceValidation::Passed
    {
        refusal_reasons.push(PairedRefusalReason::BrokenReferenceValidationFailed);
    }
    if repaired.side != CampaignSide::Repaired
        || repaired.reference_validation != ReferenceValidation::Passed
    {
        refusal_reasons.push(PairedRefusalReason::RepairedReferenceValidationFailed);
    }
    if broken.verdict.blocking_issues().is_empty() {
        refusal_reasons.push(PairedRefusalReason::BrokenHasNoBlockingIssue);
    }
    if broken.verdict.token() != FalsificationVerdictToken::Block {
        refusal_reasons.push(PairedRefusalReason::BrokenVerdictIsNotBlock);
    }
    if repaired.verdict.token() != FalsificationVerdictToken::Approve {
        refusal_reasons.push(PairedRefusalReason::RepairedVerdictIsNotApprove);
    }
    if !repaired.verdict.blocking_issues().is_empty() {
        refusal_reasons.push(PairedRefusalReason::RepairedHasBlockingIssue);
    }

    let cited_broken_primary = broken
        .verdict
        .blocking_issues()
        .iter()
        .filter_map(PairedBlockingIssue::primary)
        .collect::<Vec<_>>();
    let broken_primary = broken
        .evidence
        .executions()
        .iter()
        .filter(|record| cited_broken_primary.contains(&&record.execution_ref))
        .filter_map(|record| {
            broken
                .replays
                .get(&record.execution_ref)
                .eq(&Some(PairedReplayClassification::RepairSensitive))
                .then(|| campaign_probe_identity(&record.stimulus, broken.root))
                .flatten()
                .map(|identity| (record, identity))
        })
        .collect::<Vec<_>>();
    let repaired_records = repaired
        .evidence
        .executions()
        .iter()
        .filter_map(|record| {
            repaired
                .replays
                .get(&record.execution_ref)
                .eq(&Some(PairedReplayClassification::RepairSensitive))
                .then(|| campaign_probe_identity(&record.stimulus, repaired.root))
                .flatten()
                .map(|identity| (record, identity))
        })
        .collect::<Vec<_>>();
    if broken_primary.is_empty() {
        refusal_reasons.push(PairedRefusalReason::BrokenHasNoQualifyingPrimary);
    }
    if repaired_records.is_empty() {
        refusal_reasons.push(PairedRefusalReason::RepairedHasNoQualifyingProbe);
    }
    let broken_identities = broken_primary
        .iter()
        .map(|(_, identity)| identity.clone())
        .collect::<BTreeSet<_>>();
    let repaired_identities = repaired_records
        .iter()
        .map(|(_, identity)| identity.clone())
        .collect::<BTreeSet<_>>();
    let shared_identities = broken_identities
        .intersection(&repaired_identities)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !broken_primary.is_empty() && !repaired_records.is_empty() && shared_identities.is_empty() {
        refusal_reasons.push(PairedRefusalReason::QualifyingProbesDoNotMatch);
    }
    let matching_broken = broken_primary
        .iter()
        .filter(|(_, identity)| shared_identities.contains(identity))
        .map(|(record, _)| record.execution_ref.clone())
        .collect();
    let matching_repaired = repaired_records
        .iter()
        .filter(|(_, identity)| shared_identities.contains(identity))
        .map(|(record, _)| record.execution_ref.clone())
        .collect();
    PairedExecutionProofResult {
        decision: if refusal_reasons.is_empty() {
            PairedProofDecision::Approve
        } else {
            PairedProofDecision::Refuse
        },
        broken_witnesses: matching_broken,
        repaired_probes: matching_repaired,
        refusal_reasons,
    }
}

/// Pure verdict parsing and stimulus-identity error.
#[derive(Debug, Error)]
pub enum PairedExecutionProofError {
    /// The outer verdict is malformed or does not have the current exact shape.
    #[error("failed to parse paired falsification verdict")]
    InvalidVerdict { source: serde_json::Error },
    /// A schema-required non-empty verdict field was empty.
    #[error("paired falsification verdict contains an empty required field")]
    EmptyVerdictField,
    /// An execution reference has a noncanonical spelling.
    #[error(transparent)]
    GateExecution(#[from] crate::GateExecutionError),
    /// The recorded stimulus cwd is outside its originating campaign root.
    #[error("paired stimulus working directory is outside its campaign root")]
    StimulusOutsideCampaignRoot,
    /// The originating campaign root cannot be represented exactly in JSON evidence.
    #[error("paired campaign root must be UTF-8")]
    NonUtf8CampaignRoot,
    /// The stimulus names a fixed absolute program or input outside its repository.
    #[error("paired stimulus names a fixed absolute path outside its campaign root")]
    StimulusNamesFixedAbsolutePath,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;

    use crate::{GateExecutionRef, parse_gate_execution_evidence};

    use super::{
        CampaignSide, PairedCampaign, PairedProofDecision, PairedReplayClassification,
        ReferenceValidation, ReplayClassifications, fold_paired_execution_proof,
        paired_stimulus_identity, parse_paired_falsification_verdict,
    };

    fn verdict(token: &str, issue: bool) -> Vec<u8> {
        let issues = if issue {
            r#"[{"id":"F-1","severity":"major","location":"x","problem":"x","input":"x","observation":"x","execution_ref":"execution-000001","required_change":"x","replacement_execution":{"input":"x","observation":"x","execution_ref":"execution-000002"}}]"#
        } else {
            "[]"
        };
        format!(
            r#"{{"verdict":"{token}","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":{issues},"non_blocking_notes":[],"summary":"x"}}"#
        )
        .into_bytes()
    }

    fn evidence(root: &str) -> Vec<u8> {
        format!(
            r#"{{"schema_id":"pce.gate-execution-evidence","schema_version":1,"executions":[{{"execution_ref":"execution-000001","stimulus":{{"working_directory":"{root}","setup":[],"command":{{"program":"target/pce-execution-subject-probe/pce","arguments":["gate","execution-subject-probe","--output",".pce-execution-subject-verdict.json"],"input":[],"environment":{{"HOME":"h","PATH":"p","USER":"u"}}}}}},"observed_result":{{"setup":[],"command":null}}}},{{"execution_ref":"execution-000002","stimulus":{{"working_directory":"{root}","setup":[],"command":{{"program":"/bin/true","arguments":[],"input":[],"environment":{{}}}}}},"observed_result":{{"setup":[],"command":null}}}}]}}"#
        )
        .into_bytes()
    }

    #[test]
    fn replayability_rejects_outside_cwd_program_and_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        for stimulus in [
            r#"{"working_directory":"/outside","setup":[],"command":{"program":"/repo/pce","arguments":[],"input":[],"environment":{}}}"#,
            r#"{"working_directory":"/repo","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{}}}"#,
            r#"{"working_directory":"/repo","setup":[],"command":{"program":"/repo/pce","arguments":["/outside/input"],"input":[],"environment":{}}}"#,
        ] {
            let stimulus = crate::parse_gate_stimulus(stimulus.as_bytes())?;
            assert!(paired_stimulus_identity(&stimulus, Path::new("/repo")).is_err());
        }
        Ok(())
    }

    #[test]
    fn admits_a_repair_sensitive_broken_finding() -> Result<(), Box<dyn std::error::Error>> {
        let broken_verdict = parse_paired_falsification_verdict(&verdict("BLOCK", true))?;
        let repaired_verdict = parse_paired_falsification_verdict(&verdict("APPROVE", false))?;
        let broken_evidence = parse_gate_execution_evidence(&evidence("/campaign/broken"))?;
        let repaired_evidence = parse_gate_execution_evidence(&evidence("/campaign/repaired"))?;
        let reference = GateExecutionRef::parse("execution-000001")?;
        let classifications = ReplayClassifications::new(BTreeMap::from([(
            reference,
            PairedReplayClassification::RepairSensitive,
        )]));
        let result = fold_paired_execution_proof(
            PairedCampaign {
                side: CampaignSide::Broken,
                root: Path::new("/campaign/broken"),
                verdict: &broken_verdict,
                evidence: &broken_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &classifications,
            },
            PairedCampaign {
                side: CampaignSide::Repaired,
                root: Path::new("/campaign/repaired"),
                verdict: &repaired_verdict,
                evidence: &repaired_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &classifications,
            },
        );
        assert_eq!(result.decision, PairedProofDecision::Approve);
        assert_eq!(result.broken_witnesses[0].as_str(), "execution-000001");
        assert_eq!(result.repaired_probes[0].as_str(), "execution-000001");
        Ok(())
    }

    #[test]
    fn refuses_when_no_broken_finding_has_a_repair_sensitive_primary()
    -> Result<(), Box<dyn std::error::Error>> {
        let broken_verdict = parse_paired_falsification_verdict(&verdict("REVISE", false))?;
        let repaired_verdict = parse_paired_falsification_verdict(&verdict("APPROVE", false))?;
        let broken_evidence = parse_gate_execution_evidence(&evidence("/campaign/broken"))?;
        let repaired_evidence = parse_gate_execution_evidence(
            br#"{"schema_id":"pce.gate-execution-evidence","schema_version":1,"executions":[]}"#,
        )?;
        let classifications = ReplayClassifications::default();
        let result = fold_paired_execution_proof(
            PairedCampaign {
                side: CampaignSide::Broken,
                root: Path::new("/campaign/broken"),
                verdict: &broken_verdict,
                evidence: &broken_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &classifications,
            },
            PairedCampaign {
                side: CampaignSide::Repaired,
                root: Path::new("/campaign/repaired"),
                verdict: &repaired_verdict,
                evidence: &repaired_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &classifications,
            },
        );
        assert_eq!(result.decision, PairedProofDecision::Refuse);
        assert!(result.broken_witnesses.is_empty());
        assert!(result.repaired_probes.is_empty());
        Ok(())
    }

    #[test]
    fn repaired_blocking_verdict_refuses_the_paired_proof() -> Result<(), Box<dyn std::error::Error>>
    {
        let broken_verdict = parse_paired_falsification_verdict(&verdict("BLOCK", true))?;
        let repaired_verdict = parse_paired_falsification_verdict(&verdict("BLOCK", true))?;
        let broken_evidence = parse_gate_execution_evidence(&evidence("/campaign/broken"))?;
        let repaired_evidence = parse_gate_execution_evidence(&evidence("/campaign/repaired"))?;
        let reference = GateExecutionRef::parse("execution-000001")?;
        let classifications = ReplayClassifications::new(BTreeMap::from([(
            reference,
            PairedReplayClassification::RepairSensitive,
        )]));
        let result = fold_paired_execution_proof(
            PairedCampaign {
                side: CampaignSide::Broken,
                root: Path::new("/campaign/broken"),
                verdict: &broken_verdict,
                evidence: &broken_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &classifications,
            },
            PairedCampaign {
                side: CampaignSide::Repaired,
                root: Path::new("/campaign/repaired"),
                verdict: &repaired_verdict,
                evidence: &repaired_evidence,
                reference_validation: ReferenceValidation::Passed,
                replays: &classifications,
            },
        );
        assert_eq!(result.decision, PairedProofDecision::Refuse);
        Ok(())
    }
}
