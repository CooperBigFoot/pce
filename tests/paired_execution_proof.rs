//! Offline proof that retained paired campaign evidence admits exactly the paired decision.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use pce_core::{
    CampaignSide, GateExecutionEvidence, GateExecutionRef, PairedCampaign, PairedProofDecision,
    PairedReplayClassification, ReferenceValidation, ReplayClassifications,
    fold_paired_execution_proof, parse_gate_execution_evidence, parse_paired_falsification_verdict,
    validate_verdict_references,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const FIXTURE_ROOT: &str = "tests/fixtures/paired-execution-proof";
const PAIRED_BASE: &str = "6afc64eb9171350dde36d0def74bdb2ced0e5293";
const BROKEN_OID: &str = "a8a87cb44f84988fa61904bfba48614401482851";
const REPAIRED_OID: &str = "cbe499ef94864d223518ad328db2a396551fa85b";
const LIMITATION: &str = "Live authentication and free-running gate provenance cannot be reproduced from retained bytes alone; the immutable execution records and replay classifications are reproducible, while the claim that these bytes came from the recorded live campaign depends on the retained binary digest and invocation manifest.";

fn fixture(path: &str) -> Vec<u8> {
    let complete = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_ROOT)
        .join(path);
    std::fs::read(&complete).unwrap_or_else(|error| {
        panic!(
            "retained live paired campaign fixture {} is required: {error}",
            complete.display()
        )
    })
}

fn replay(path: &str) -> (GateExecutionRef, PairedReplayClassification, Value) {
    let value: Value = serde_json::from_slice(&fixture(path)).expect("retained replay report JSON");
    let reference = GateExecutionRef::parse(
        value["execution_ref"]
            .as_str()
            .expect("replay execution reference"),
    )
    .expect("canonical replay execution reference");
    let classification = match value["classification"]
        .as_str()
        .expect("replay classification")
    {
        "repair-sensitive" => PairedReplayClassification::RepairSensitive,
        "no-repair-signal" => PairedReplayClassification::NoRepairSignal,
        "opposite-direction" => PairedReplayClassification::OppositeDirection,
        "non-reproducible" => PairedReplayClassification::NonReproducible,
        "checkout-failed" => PairedReplayClassification::CheckoutFailed,
        "oracle-failed" => PairedReplayClassification::OracleFailed,
        other => panic!("unknown retained replay classification {other}"),
    };
    (reference, classification, value)
}

fn recorded_root(evidence: &GateExecutionEvidence) -> PathBuf {
    evidence
        .executions()
        .iter()
        .find(|record| {
            record.stimulus.command().program() == "target/pce-execution-subject-probe/pce"
        })
        .expect("campaign evidence contains the mandated probe")
        .stimulus
        .working_directory()
        .to_path_buf()
}

fn fold_fixture() -> pce_core::PairedExecutionProofResult {
    let broken_bytes = fixture("broken-verdict.json");
    let repaired_bytes = fixture("repaired-verdict.json");
    let broken_evidence_bytes = fixture("broken-verdict.json.executions.json");
    let repaired_evidence_bytes = fixture("repaired-verdict.json.executions.json");
    let broken_evidence =
        parse_gate_execution_evidence(&broken_evidence_bytes).expect("broken immutable evidence");
    let repaired_evidence = parse_gate_execution_evidence(&repaired_evidence_bytes)
        .expect("repaired immutable evidence");
    validate_verdict_references(&broken_bytes, broken_evidence.executions())
        .expect("every broken issue has same-dispatch references");
    validate_verdict_references(&repaired_bytes, repaired_evidence.executions())
        .expect("repaired verdict references are valid");
    let broken_verdict =
        parse_paired_falsification_verdict(&broken_bytes).expect("current broken verdict shape");
    let repaired_verdict = parse_paired_falsification_verdict(&repaired_bytes)
        .expect("current repaired verdict shape");
    let broken_root = recorded_root(&broken_evidence);
    let repaired_root = recorded_root(&repaired_evidence);
    let (broken_reference, broken_classification, _) =
        replay("replays/broken-execution-000001.json");
    let broken_replays =
        ReplayClassifications::new(BTreeMap::from([(broken_reference, broken_classification)]));
    let repaired_replays = ReplayClassifications::new(BTreeMap::from_iter(
        [
            "replays/repaired-execution-000001.json",
            "replays/repaired-execution-000002.json",
            "replays/repaired-execution-000003.json",
        ]
        .map(replay)
        .map(|(reference, classification, _)| (reference, classification)),
    ));
    fold_paired_execution_proof(
        PairedCampaign {
            side: CampaignSide::Broken,
            root: &broken_root,
            verdict: &broken_verdict,
            evidence: &broken_evidence,
            reference_validation: ReferenceValidation::Passed,
            replays: &broken_replays,
        },
        PairedCampaign {
            side: CampaignSide::Repaired,
            root: &repaired_root,
            verdict: &repaired_verdict,
            evidence: &repaired_evidence,
            reference_validation: ReferenceValidation::Passed,
            replays: &repaired_replays,
        },
    )
}

#[test]
fn retained_campaign_reproduces_the_approval_offline() {
    let folded = fold_fixture();
    assert_eq!(folded.decision, PairedProofDecision::Approve);
    assert_eq!(
        folded.broken_witnesses,
        vec![GateExecutionRef::parse("execution-000001").expect("witness reference")]
    );
    assert_eq!(folded.repaired_probes, folded.broken_witnesses);
    assert!(folded.refusal_reasons.is_empty());

    let report: Value = serde_json::from_slice(&fixture("paired-execution-proof.json"))
        .expect("retained compact proof report");
    assert_eq!(report["schema_id"], "pce.paired-execution-proof");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["broken_ref"], BROKEN_OID);
    assert_eq!(report["repaired_ref"], REPAIRED_OID);
    assert!(matches!(
        report["broken_verdict"].as_str(),
        Some("BLOCK" | "REVISE" | "APPROVE")
    ));
    assert!(matches!(
        report["repaired_verdict"].as_str(),
        Some("BLOCK" | "REVISE" | "APPROVE")
    ));
    assert_eq!(report["broken_verdict"], "BLOCK");
    assert_eq!(report["broken_witnesses"], json!(["execution-000001"]));
    assert_eq!(report["repaired_verdict"], "APPROVE");
    assert_eq!(report["repaired_probes"], json!(["execution-000001"]));
    assert_eq!(report["decision"], "APPROVE");
}

#[test]
fn retained_manifest_authenticates_every_other_fixture() {
    let manifest: Value = serde_json::from_slice(&fixture("provenance-manifest.json"))
        .expect("retained provenance manifest JSON");
    assert_eq!(manifest["base_commit"], PAIRED_BASE);
    assert_eq!(manifest["limitation"], LIMITATION);

    let mut manifested = BTreeMap::new();
    for artifact in manifest["artifacts"]
        .as_array()
        .expect("manifest artifact array")
    {
        let path = artifact["path"].as_str().expect("manifest artifact path");
        let bytes = fixture(path);
        let digest = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            artifact["byte_count"].as_u64(),
            Some(u64::try_from(bytes.len()).expect("fixture byte count fits u64")),
            "byte count for {path}"
        );
        assert_eq!(
            artifact["sha256"].as_str(),
            Some(digest.as_str()),
            "SHA-256 for {path}"
        );
        assert!(manifested.insert(path.to_owned(), ()).is_none());
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_ROOT);
    let mut retained = BTreeMap::new();
    fn visit(root: &Path, directory: &Path, retained: &mut BTreeMap<String, ()>) {
        for entry in std::fs::read_dir(directory).expect("read retained fixture directory") {
            let entry = entry.expect("retained fixture entry");
            if entry.file_type().expect("retained fixture type").is_dir() {
                visit(root, &entry.path(), retained);
            } else {
                let relative = entry
                    .path()
                    .strip_prefix(root)
                    .expect("fixture beneath root")
                    .to_str()
                    .expect("UTF-8 fixture path")
                    .to_owned();
                if relative != "provenance-manifest.json" {
                    retained.insert(relative, ());
                }
            }
        }
    }
    visit(&root, &root, &mut retained);
    assert_eq!(manifested, retained);
}

#[test]
fn retained_replays_name_classified_and_unreplayable_outcomes() {
    for (path, classification) in [
        (
            "replays/broken-execution-000001.json",
            PairedReplayClassification::RepairSensitive,
        ),
        (
            "replays/repaired-execution-000001.json",
            PairedReplayClassification::RepairSensitive,
        ),
        (
            "replays/repaired-execution-000002.json",
            PairedReplayClassification::NoRepairSignal,
        ),
        (
            "replays/repaired-execution-000003.json",
            PairedReplayClassification::NoRepairSignal,
        ),
    ] {
        let (_, actual, _) = replay(path);
        assert_eq!(actual, classification);
    }
    for path in [
        "replays/repaired-execution-000004.json",
        "replays/repaired-execution-000005.json",
    ] {
        let report: Value =
            serde_json::from_slice(&fixture(path)).expect("retained replayability report JSON");
        assert_eq!(report["replayability"], "unreplayable");
        assert_eq!(
            report["reason"],
            "fixed-absolute-path-outside-repository-root"
        );
        assert!(report.get("classification").is_none());
    }
}

#[test]
fn retained_broken_executions_are_repository_relative_and_inside_the_checkout() {
    let evidence = parse_gate_execution_evidence(&fixture("broken-verdict.json.executions.json"))
        .expect("broken immutable evidence");
    let root = recorded_root(&evidence);
    let programs: Vec<_> = evidence
        .executions()
        .iter()
        .map(|record| {
            assert_eq!(record.stimulus.working_directory(), root);
            record.stimulus.command().program()
        })
        .collect();
    assert_eq!(
        programs,
        [
            "target/pce-execution-subject-probe/pce",
            "target/pce-execution-subject-probe/pce",
            "./artifact",
            "./gate",
        ]
    );
}

fn synthetic_verdict(token: &str, with_issue: bool) -> Vec<u8> {
    let issues = if with_issue {
        json!([{
            "id": "F-1", "severity": "major", "location": "route",
            "problem": "behavior differs", "input": "probe", "observation": "failed",
            "execution_ref": "execution-000001", "required_change": "repair route",
            "replacement_execution": {
                "input": "probe after repair", "observation": "passed",
                "execution_ref": "execution-000002"
            }
        }])
    } else {
        json!([])
    };
    serde_json::to_vec(&json!({
        "verdict": token,
        "self_sufficiency": "NOT_APPLICABLE",
        "root_cause": "execution",
        "blocking_issues": issues,
        "non_blocking_notes": [],
        "summary": "synthetic fold boundary"
    }))
    .expect("synthetic verdict")
}

fn synthetic_evidence(root: &str, program: &str, altered: Option<(&str, Value)>) -> Vec<u8> {
    let mut stimulus = json!({
        "working_directory": root,
        "setup": [],
        "command": {
            "program": program,
            "arguments": ["gate", "execution-subject-probe", "--output", ".pce-execution-subject-verdict.json"],
            "input": [],
            "environment": {"PATH": "/bin", "HOME": "/home/operator", "USER": "operator"}
        }
    });
    if let Some((pointer, value)) = altered {
        *stimulus
            .pointer_mut(pointer)
            .expect("synthetic stimulus pointer") = value;
    }
    serde_json::to_vec(&json!({
        "schema_id": "pce.gate-execution-evidence",
        "schema_version": 1,
        "executions": [
            {"execution_ref": "execution-000001", "stimulus": stimulus, "observed_result": {"setup": [], "command": null}},
            {"execution_ref": "execution-000002", "stimulus": {"working_directory": root, "setup": [], "command": {"program": "/bin/true", "arguments": [], "input": [], "environment": {}}}, "observed_result": {"setup": [], "command": null}}
        ]
    }))
    .expect("synthetic evidence")
}

fn synthetic_fold(
    broken_token: &str,
    repaired_token: &str,
    repaired_issue: bool,
    classification: PairedReplayClassification,
    repaired_alteration: Option<(&str, Value)>,
) -> pce_core::PairedExecutionProofResult {
    let broken_root = Path::new("/campaign/broken");
    let repaired_root = Path::new("/campaign/repaired");
    let broken_verdict = parse_paired_falsification_verdict(&synthetic_verdict(broken_token, true))
        .expect("synthetic broken verdict");
    let repaired_verdict =
        parse_paired_falsification_verdict(&synthetic_verdict(repaired_token, repaired_issue))
            .expect("synthetic repaired verdict");
    let broken_evidence = parse_gate_execution_evidence(&synthetic_evidence(
        "/campaign/broken",
        "target/pce-execution-subject-probe/pce",
        None,
    ))
    .expect("synthetic broken evidence");
    let repaired_evidence = parse_gate_execution_evidence(&synthetic_evidence(
        "/campaign/repaired",
        "target/pce-execution-subject-probe/pce",
        repaired_alteration,
    ))
    .expect("synthetic repaired evidence");
    let reference = GateExecutionRef::parse("execution-000001").expect("synthetic reference");
    let replays = ReplayClassifications::new(BTreeMap::from([(reference, classification)]));
    fold_paired_execution_proof(
        PairedCampaign {
            side: CampaignSide::Broken,
            root: broken_root,
            verdict: &broken_verdict,
            evidence: &broken_evidence,
            reference_validation: ReferenceValidation::Passed,
            replays: &replays,
        },
        PairedCampaign {
            side: CampaignSide::Repaired,
            root: repaired_root,
            verdict: &repaired_verdict,
            evidence: &repaired_evidence,
            reference_validation: ReferenceValidation::Passed,
            replays: &replays,
        },
    )
}

#[test]
fn fold_requires_the_exact_paired_verdicts_and_shared_probe() {
    assert_eq!(
        synthetic_fold(
            "BLOCK",
            "APPROVE",
            false,
            PairedReplayClassification::RepairSensitive,
            None,
        )
        .decision,
        PairedProofDecision::Approve
    );
    for classification in [
        PairedReplayClassification::NoRepairSignal,
        PairedReplayClassification::OppositeDirection,
        PairedReplayClassification::NonReproducible,
        PairedReplayClassification::CheckoutFailed,
        PairedReplayClassification::OracleFailed,
    ] {
        assert_eq!(
            synthetic_fold("BLOCK", "APPROVE", false, classification, None).decision,
            PairedProofDecision::Refuse
        );
    }
    for (broken, repaired, repaired_issue) in [
        ("REVISE", "APPROVE", false),
        ("APPROVE", "APPROVE", false),
        ("BLOCK", "REVISE", false),
        ("BLOCK", "BLOCK", true),
        ("BLOCK", "APPROVE", true),
    ] {
        assert_eq!(
            synthetic_fold(
                broken,
                repaired,
                repaired_issue,
                PairedReplayClassification::RepairSensitive,
                None,
            )
            .decision,
            PairedProofDecision::Refuse
        );
    }
    assert_eq!(
        synthetic_fold(
            "BLOCK",
            "APPROVE",
            false,
            PairedReplayClassification::RepairSensitive,
            Some(("/command/input", json!([1]))),
        )
        .decision,
        PairedProofDecision::Refuse
    );
}

#[test]
fn forged_repair_sensitive_program_is_excluded_at_probe_identity() {
    let broken_root = Path::new("/campaign/broken");
    let repaired_root = Path::new("/campaign/repaired");
    let broken_verdict = parse_paired_falsification_verdict(&synthetic_verdict("BLOCK", true))
        .expect("synthetic broken verdict");
    let repaired_verdict = parse_paired_falsification_verdict(&synthetic_verdict("APPROVE", false))
        .expect("synthetic repaired verdict");
    let broken_evidence =
        parse_gate_execution_evidence(&synthetic_evidence("/campaign/broken", "artifact", None))
            .expect("forged broken evidence");
    let repaired_evidence =
        parse_gate_execution_evidence(&synthetic_evidence("/campaign/repaired", "artifact", None))
            .expect("forged repaired evidence");
    let reference = GateExecutionRef::parse("execution-000001").expect("synthetic reference");
    let replays = ReplayClassifications::new(BTreeMap::from([(
        reference,
        PairedReplayClassification::RepairSensitive,
    )]));
    let result = fold_paired_execution_proof(
        PairedCampaign {
            side: CampaignSide::Broken,
            root: broken_root,
            verdict: &broken_verdict,
            evidence: &broken_evidence,
            reference_validation: ReferenceValidation::Passed,
            replays: &replays,
        },
        PairedCampaign {
            side: CampaignSide::Repaired,
            root: repaired_root,
            verdict: &repaired_verdict,
            evidence: &repaired_evidence,
            reference_validation: ReferenceValidation::Passed,
            replays: &replays,
        },
    );
    assert!(result.broken_witnesses.is_empty());
    assert!(result.repaired_probes.is_empty());
    assert!(
        result
            .refusal_reasons
            .contains(&pce_core::PairedRefusalReason::BrokenHasNoQualifyingPrimary)
    );
    assert!(
        result
            .refusal_reasons
            .contains(&pce_core::PairedRefusalReason::RepairedHasNoQualifyingProbe)
    );
}

#[test]
fn valid_campaign_probes_with_different_environment_refuse_as_nonmatching() {
    let result = synthetic_fold(
        "BLOCK",
        "APPROVE",
        false,
        PairedReplayClassification::RepairSensitive,
        Some(("/command/environment/USER", json!("different-operator"))),
    );
    assert_eq!(result.decision, PairedProofDecision::Refuse);
    assert_eq!(
        result.refusal_reasons,
        vec![pce_core::PairedRefusalReason::QualifyingProbesDoNotMatch]
    );
}

#[test]
fn no_repair_signal_never_enters_witness_or_probe_arrays() {
    let result = synthetic_fold(
        "BLOCK",
        "APPROVE",
        false,
        PairedReplayClassification::NoRepairSignal,
        None,
    );
    assert!(result.broken_witnesses.is_empty());
    assert!(result.repaired_probes.is_empty());
    assert!(
        result
            .refusal_reasons
            .contains(&pce_core::PairedRefusalReason::BrokenHasNoQualifyingPrimary)
    );
    assert!(
        result
            .refusal_reasons
            .contains(&pce_core::PairedRefusalReason::RepairedHasNoQualifyingProbe)
    );
}

#[test]
fn deterministic_shim_campaign_uses_the_real_recorder_and_ordinary_gate() {
    let temp = tempfile::tempdir().expect("paired shim directory");
    let socket_probe = temp.path().join("socket-probe");
    match std::os::unix::net::UnixListener::bind(&socket_probe) {
        Ok(listener) => drop(listener),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
        Err(error) => panic!("Unix socket capability probe failed: {error}"),
    }
    std::fs::remove_file(&socket_probe).expect("socket capability probe cleanup");
    let bin = temp.path().join("bin");
    let artifacts = temp.path().join("artifacts");
    std::fs::create_dir(&bin).expect("shim bin directory");
    std::fs::create_dir(&artifacts).expect("empty campaign artifact directory");
    let claude = bin.join("claude");
    std::fs::write(
        &claude,
        r##"#!/bin/sh
set -eu
artifact=
previous=
for argument in "$@"; do
    if [ "$previous" = "--append-system-prompt" ]; then artifact=$argument; fi
    previous=$argument
done
[ -n "${PCE_GATE_EXEC_CLIENT:-}" ] || exit 1
client_parent=$(cd "$(dirname "$PCE_GATE_EXEC_CLIENT")" && pwd -P)
[ "$client_parent" = "$PWD" ] || exit 26
case "$PCE_GATE_EXEC_CLIENT" in
    *execution-subject*|*paired-execution*|*broken*|*repaired*) exit 27 ;;
esac
if strings "$PCE_GATE_EXEC_CLIENT" | grep -E 'a8a87cb|cbe499e|tests/fixtures/execution-subject|repair-sensitive|conforming-verdict|paired-execution-proof' >/dev/null; then
    exit 28
fi
artifact_matches=$(find "$client_parent" -type f -exec /usr/bin/shasum -a 256 {} \; | /usr/bin/awk '$1 == "ced92902e86ca5b37ecb373942983564bb4101e8580b87f1163e1158dd48e155" || $1 == "1e5c286c9d256514d0032067d209ec637959c65c1bd4af6135b3efa17cdb9717" { count += 1 } END { print count + 0 }')
[ "$artifact_matches" -eq 1 ] || exit 29
# There is intentionally no process-table isolation control here. The recorder executes arbitrary
# critic-supplied programs without controlling the host, so an ancestor argv can expose the
# campaign's --repo-root and labelled fixtures. ADR-0011 accepts that channel as open.
[ "$(git rev-list --all --count)" -eq 1 ] || exit 20
[ "$(find .. -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" -eq 1 ] || exit 21
if find ../.. -mindepth 1 -maxdepth 1 -type d -name 'pce-paired-execution-*' | grep . >/dev/null; then
    exit 23
fi
if find ../.. -type f -name 'critic-*.json' -readable 2>/dev/null | grep . >/dev/null; then
    exit 24
fi
if ps -o command= -p "$PPID" | grep -E 'critic-(one|two)|critic-(broken|repaired)' >/dev/null; then
    exit 25
fi
if strings target/pce-execution-subject-probe/pce | grep -E 'a8a87cb|cbe499e|repair-sensitive|conforming-verdict|paired-execution-proof' >/dev/null; then
    exit 22
fi
mandate=
previous=
for argument in "$@"; do
    if [ "$previous" = "--append-system-prompt" ] && [ -z "$mandate" ]; then mandate=$argument; fi
    previous=$argument
done
output=$(printf '%s' "$mandate" | /usr/bin/sed -n 's|.*<output-path>\([^<]*\)</output-path>.*|\1|p')
[ -n "$output" ] || exit 3
request="$PWD/request-$$.json"
response="$PWD/response-$$.json"
printf '{"working_directory":"/tmp","setup":[],"command":{"program":"/bin/echo","arguments":["hello"],"input":[],"environment":{}}}\n' > "$request"
"$PCE_GATE_EXEC_CLIENT" gate exec < "$request" > "$response"
printf '{"working_directory":"%s","setup":[],"command":{"program":"target/pce-execution-subject-probe/pce","arguments":["gate","execution-subject-probe","--output",".pce-execution-subject-verdict.json"],"input":[],"environment":{"PATH":"%s","HOME":"%s","USER":"%s"}}}\n' "$PWD" "$PATH" "$HOME" "$USER" > "$request"
"$PCE_GATE_EXEC_CLIENT" gate exec < "$request" > "$response"
if ! /usr/bin/grep -q '"code":0' "$response"; then
    printf '{"working_directory":"%s","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{}}}\n' "$PWD" > "$request"
    "$PCE_GATE_EXEC_CLIENT" gate exec < "$request" > "$response"
    if [ "$USER" = "refuse-user" ]; then
        printf '%s\n' '{"verdict":"BLOCK","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"artifact and gate","problem":"the ordinary repository gate fails on its plain input","input":"control execution","observation":"the recorded control completed","execution_ref":"execution-000003","required_change":"make artifact produce the value required by the ordinary gate","replacement_execution":{"input":"outside-root control","observation":"the recorded control completed","execution_ref":"execution-000001"}}],"non_blocking_notes":[],"summary":"recorded a non-admitted reading-only block"}' > "$output"
    else
        printf '%s\n' '{"verdict":"BLOCK","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[{"id":"F-1","severity":"major","location":"artifact and gate","problem":"the ordinary repository gate fails on its plain input","input":"required campaign probe","observation":"the recorded command exited non-zero","execution_ref":"execution-000002","required_change":"make artifact produce the value required by the ordinary gate","replacement_execution":{"input":"control execution","observation":"the recorded control completed","execution_ref":"execution-000003"}}],"non_blocking_notes":[],"summary":"demonstrated executable subject break"}' > "$output"
    fi
else
    printf '{"working_directory":"%s","setup":[],"command":{"program":"/bin/true","arguments":[],"input":[],"environment":{}}}\n' "$PWD" > "$request"
    "$PCE_GATE_EXEC_CLIENT" gate exec < "$request" > "$response"
    printf '%s\n' '{"verdict":"APPROVE","self_sufficiency":"NOT_APPLICABLE","root_cause":"execution","blocking_issues":[],"non_blocking_notes":[],"summary":"ordinary execution subject probe passed"}' > "$output"
fi
/bin/rm -f "$request" "$response"
printf '%s\n' '{"type":"result","subtype":"success","is_error":false,"result":"OK","usage":{"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}'
"##,
    )
    .expect("write deterministic Claude shim");
    std::fs::set_permissions(&claude, std::fs::Permissions::from_mode(0o755))
        .expect("make Claude shim executable");
    let executable = env!("CARGO_BIN_EXE_pce");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .canonicalize()
        .expect("canonical repository root");
    let shim_path = format!("{}:/usr/bin:/bin", bin.display());
    let output = Command::new(executable)
        .args(["gate", "paired-execution-proof", "--repo-root"])
        .arg(&repository)
        .args(["--artifacts"])
        .arg(&artifacts)
        .args([
            "--env",
            &format!("PATH={shim_path}"),
            "--env",
            "HOME=/tmp/pce-paired-home",
            "--env",
            "USER=pce-paired-user",
        ])
        .output()
        .expect("run deterministic paired campaign");
    assert!(
        output.status.success(),
        "deterministic campaign stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        std::fs::read(artifacts.join("paired-execution-proof.json"))
            .expect("deterministic proof report")
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("deterministic proof report JSON");
    assert_eq!(report["decision"], "APPROVE");
    assert_eq!(report["broken_verdict"], "BLOCK");
    assert_eq!(report["repaired_verdict"], "APPROVE");
    for side in ["broken", "repaired"] {
        let evidence = parse_gate_execution_evidence(
            &std::fs::read(artifacts.join(format!("{side}-verdict.json.executions.json")))
                .expect("campaign evidence sidecar"),
        )
        .expect("campaign evidence parses");
        let probe = evidence
            .record(&GateExecutionRef::parse("execution-000002").expect("probe reference"))
            .expect("campaign probe record");
        assert_eq!(
            probe.stimulus.command().program(),
            "target/pce-execution-subject-probe/pce"
        );
    }
    assert!(artifacts.join("replays/broken-witness.json").is_file());
    assert!(artifacts.join("replays/repaired-probe.json").is_file());
    assert!(
        artifacts
            .join("replays/broken-execution-000002.json")
            .is_file()
    );
    assert!(
        !artifacts
            .join("replays/broken-execution-000001.json")
            .exists()
    );
    let unreplayable: Value = serde_json::from_slice(
        &std::fs::read(artifacts.join("replays/repaired-execution-000001.json"))
            .expect("unreplayable execution report"),
    )
    .expect("unreplayable report JSON");
    assert_eq!(unreplayable["replayability"], "unreplayable");
    assert_eq!(
        unreplayable["reason"],
        "working-directory-outside-repository-root"
    );
    let fixed_absolute: Value = serde_json::from_slice(
        &std::fs::read(artifacts.join("replays/repaired-execution-000003.json"))
            .expect("fixed absolute execution report"),
    )
    .expect("fixed absolute report JSON");
    assert_eq!(fixed_absolute["replayability"], "unreplayable");
    assert_eq!(
        fixed_absolute["reason"],
        "fixed-absolute-path-outside-repository-root"
    );
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(artifacts.join("provenance-manifest.json"))
            .expect("deterministic provenance manifest"),
    )
    .expect("deterministic provenance manifest JSON");
    assert_eq!(manifest["base_commit"], PAIRED_BASE);
    let expected_head = Command::new("git")
        .args(["-C"])
        .arg(&repository)
        .args(["rev-parse", "HEAD^{commit}"])
        .output()
        .expect("resolve repository HEAD for provenance assertion");
    assert!(expected_head.status.success());
    assert_eq!(
        manifest["head_commit"],
        String::from_utf8(expected_head.stdout)
            .expect("repository HEAD is UTF-8")
            .trim()
    );
    assert_eq!(manifest["limitation"], LIMITATION);
    let manifest_text = manifest.to_string();
    for secret in [
        shim_path.as_str(),
        "/tmp/pce-paired-home",
        "pce-paired-user",
    ] {
        assert!(!manifest_text.contains(secret));
    }

    let refused_artifacts = temp.path().join("refused-artifacts");
    std::fs::create_dir(&refused_artifacts).expect("empty refused artifact directory");
    let refused = Command::new(executable)
        .args(["gate", "paired-execution-proof", "--repo-root"])
        .arg(&repository)
        .args(["--artifacts"])
        .arg(&refused_artifacts)
        .args([
            "--env",
            &format!("PATH={shim_path}"),
            "--env",
            "HOME=/tmp/pce-paired-home",
            "--env",
            "USER=refuse-user",
        ])
        .output()
        .expect("run deterministic refused paired campaign");
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(refused.stderr, b"Error: paired execution proof refused\n");
    let refused_manifest: Value = serde_json::from_slice(
        &std::fs::read(refused_artifacts.join("provenance-manifest.json"))
            .expect("refused campaign provenance manifest"),
    )
    .expect("refused campaign provenance manifest JSON");
    assert_eq!(refused_manifest["limitation"], LIMITATION);
    assert!(
        refused_manifest["artifacts"]
            .as_array()
            .expect("refused manifest artifact array")
            .iter()
            .any(|artifact| artifact["path"] == "paired-execution-proof.json")
    );
    assert_eq!(
        refused.stdout,
        std::fs::read(refused_artifacts.join("paired-execution-proof.json"))
            .expect("refused proof report")
    );
    let refused_report: Value =
        serde_json::from_slice(&refused.stdout).expect("refused proof report JSON");
    assert_eq!(refused_report["decision"], "REFUSE");
    assert_eq!(refused_report["broken_witnesses"], json!([]));
    assert_eq!(refused_report["repaired_probes"], json!([]));
}
