//! meter_cli : JSONL(stdin) × argv → (status, JSONL(stdout), diagnostics(stderr)).

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde::Deserialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

#[allow(dead_code)]
mod support;

use support::{WorkspaceFixtureDirectory, skip_without_nested_seatbelt};

const M8_S2_SYNTHETIC_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.agents/m8-s2-campaign/ff96d11e6fb5054c9e6e0b037ca26767344e2602/synthetic/not-observable"
);

const M8_FILES: [&str; 5] = [
    "events.jsonl",
    "provenance-manifest.json",
    "meter-report.json",
    "dry-run-projection.json",
    "verification-output.txt",
];
const MANIFEST_KEYS: [&str; 18] = [
    "schema_version",
    "schema_mode",
    "base",
    "binary",
    "host",
    "quota_statement",
    "identity_registry",
    "attempts",
    "retries",
    "pid_observations",
    "auth_controls",
    "capability_controls",
    "snapshots",
    "projection_log",
    "aborts",
    "artifacts",
    "hashes",
    "epistemic_limits",
];
const ATTEMPT_PLAN_KEYS: [&str; 22] = [
    "attempt_id",
    "schedule_ordinal",
    "evidence",
    "identity_class",
    "route",
    "role",
    "node_ref",
    "issuance_sequence",
    "completion_sequence",
    "duration_ms",
    "child_exit",
    "parent_exit",
    "parent_stderr_class",
    "artifact_target",
    "artifact_present",
    "validation_classification",
    "usage",
    "disposition",
    "redacted_parent_argv",
    "projected_child_argv",
    "cwd",
    "log_transition",
];
const ATTEMPT_OBSERVABLE_KEYS: [&str; 23] = [
    "attempt_id",
    "schedule_ordinal",
    "evidence",
    "identity_class",
    "route",
    "role",
    "node_ref",
    "issuance_sequence",
    "completion_sequence",
    "duration_ms",
    "child_exit",
    "parent_exit",
    "parent_stderr_class",
    "artifact_target",
    "artifact_present",
    "validation_classification",
    "usage",
    "disposition",
    "redacted_parent_argv",
    "projected_child_argv",
    "cwd",
    "log_transition",
    "child_environment_observation",
];
const PID_KEYS: [&str; 7] = [
    "attempt_id",
    "parent_pid",
    "direct_child_pid",
    "observed_descendant_pids",
    "termination_targets",
    "wait_statuses",
    "survivors_after_wait",
];
const AUTH_KEYS_NOT_OBSERVABLE: [&str; 3] =
    ["attempt_id", "parent_prefix_present", "parent_env_names"];
const AUTH_KEYS_OBSERVABLE: [&str; 4] = [
    "attempt_id",
    "parent_prefix_present",
    "parent_env_names",
    "child_environment_observation_ref",
];
const CHILD_ENV_KEYS: [&str; 6] = [
    "capture_status",
    "child_pid",
    "ps_status",
    "raw_output_sha256",
    "observed_names",
    "observed_api_key_name",
];
const SNAPSHOT_KEYS: [&str; 3] = ["primary_tracked", "primary_untracked", "live_planning"];
const PROJECTION_LOG_KEYS: [&str; 4] = [
    "before_bytes",
    "before_sha256",
    "after_bytes",
    "after_sha256",
];
const PROJECTION_KEYS: [&str; 5] = ["evidence", "role", "raw", "normalized", "issuance"];
const PROJECTION_SURFACE_KEYS: [&str; 7] = [
    "parent_argv",
    "child_argv",
    "cwd",
    "environment",
    "stdin_binding",
    "schema_path",
    "output_path",
];

#[derive(Debug)]
struct FixtureVerificationError {
    site: String,
    observed: String,
}

impl std::fmt::Display for FixtureVerificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.site, self.observed)
    }
}

fn fixture_error(site: impl Into<String>, observed: impl Into<String>) -> FixtureVerificationError {
    FixtureVerificationError {
        site: site.into(),
        observed: observed.into(),
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sorted_keys(object: &Map<String, Value>) -> Vec<String> {
    let mut keys: Vec<_> = object.keys().cloned().collect();
    keys.sort();
    keys
}

fn expected_keys(keys: &[&str]) -> Vec<String> {
    let mut keys: Vec<_> = keys.iter().map(|key| (*key).to_owned()).collect();
    keys.sort();
    keys
}

fn exact_object<'a>(
    value: &'a Value,
    path: &str,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, FixtureVerificationError> {
    let object = value.as_object().ok_or_else(|| {
        fixture_error(
            "fixture/schema/object",
            format!("path={path}; value={value}"),
        )
    })?;
    let actual = sorted_keys(object);
    let expected = expected_keys(keys);
    if actual != expected {
        return Err(fixture_error(
            "fixture/schema/keys",
            format!("path={path}; expected={expected:?}; observed={actual:?}"),
        ));
    }
    Ok(object)
}

fn parse_json_file(path: &Path) -> Result<Value, FixtureVerificationError> {
    let bytes = fs::read(path).map_err(|error| {
        fixture_error(
            "fixture/files/read",
            format!("path={}; error={error}", path.display()),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        fixture_error(
            "fixture/schema/json",
            format!("path={}; error={error}", path.display()),
        )
    })
}

fn parse_manifest(path: &Path) -> Result<Value, FixtureVerificationError> {
    let manifest = parse_json_file(path)?;
    let top = exact_object(&manifest, "manifest", &MANIFEST_KEYS)?;
    let mode = top["schema_mode"].as_str().ok_or_else(|| {
        fixture_error(
            "fixture/schema/mode",
            format!("observed={}", top["schema_mode"]),
        )
    })?;
    if !matches!(
        mode,
        "child-environment-observable" | "child-environment-not-observable"
    ) {
        return Err(fixture_error(
            "fixture/schema/mode",
            format!("observed={mode}"),
        ));
    }
    let host = top["host"]
        .as_object()
        .ok_or_else(|| fixture_error("fixture/schema/host", "not-object"))?;
    let preflight = host
        .get("preflight")
        .ok_or_else(|| fixture_error("fixture/schema/host", "missing-preflight"))?;
    exact_object(
        preflight,
        "host.preflight",
        &[
            "node_path_sha256",
            "node_binary_sha256",
            "pid",
            "alive_before_ps",
            "ps_status",
            "raw_output_sha256",
            "sentinel_observed",
            "selected_mode",
            "permissions_default_mode",
        ],
    )?;
    let identity_registry = exact_object(
        &top["identity_registry"],
        "identity_registry",
        &["canonical", "retry_namespace"],
    )?;
    for (index, entry) in identity_registry["canonical"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/identity-registry", "canonical-not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(
            entry,
            &format!("identity_registry.canonical[{index}]"),
            &["evidence", "expected_role", "surface"],
        )?;
    }
    exact_object(
        &identity_registry["retry_namespace"],
        "identity_registry.retry_namespace",
        &["prefix", "ordinal_rule", "suffix_role_rule", "link_rule"],
    )?;
    let attempts = top["attempts"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/attempts", "not-array"))?;
    for (index, attempt) in attempts.iter().enumerate() {
        let keys = if mode == "child-environment-observable" && matches!(index, 2 | 3) {
            &ATTEMPT_OBSERVABLE_KEYS[..]
        } else {
            &ATTEMPT_PLAN_KEYS[..]
        };
        exact_object(attempt, &format!("attempts[{index}]"), keys)?;
        let has_observation = attempt.get("child_environment_observation").is_some();
        if mode == "child-environment-observable" && matches!(index, 2 | 3) {
            if !has_observation {
                return Err(fixture_error(
                    "fixture/schema/child-environment",
                    format!("attempt_index={index}; missing"),
                ));
            }
            exact_object(
                &attempt["child_environment_observation"],
                "child_environment_observation",
                &CHILD_ENV_KEYS,
            )?;
        } else if has_observation {
            return Err(fixture_error(
                "fixture/schema/child-environment",
                format!("attempt_index={index}; forbidden"),
            ));
        }
    }
    for (index, value) in top["pid_observations"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/pid", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(value, &format!("pid_observations[{index}]"), &PID_KEYS)?;
    }
    for (index, value) in top["retries"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/retries", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(
            value,
            &format!("retries[{index}]"),
            &[
                "retry_evidence",
                "retry_ordinal",
                "substitute_role",
                "replaces_attempt_id",
                "replaces_canonical_evidence",
            ],
        )?;
    }
    let auth_keys = if mode == "child-environment-observable" {
        &AUTH_KEYS_OBSERVABLE[..]
    } else {
        &AUTH_KEYS_NOT_OBSERVABLE[..]
    };
    for (index, value) in top["auth_controls"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/auth", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(value, &format!("auth_controls[{index}]"), auth_keys)?;
    }
    for (index, value) in top["capability_controls"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/capability", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(
            value,
            &format!("capability_controls[{index}]"),
            &[
                "attempt_id",
                "scheduled_disallowed_tools",
                "projected_disallowed_tools",
                "issued_disallowed_tools",
                "projection_sha256",
                "issued_parent_argv_sha256",
            ],
        )?;
    }
    exact_object(&top["snapshots"], "snapshots", &SNAPSHOT_KEYS)?;
    exact_object(
        &top["snapshots"]["primary_tracked"],
        "snapshots.primary_tracked",
        &["quiescence", "before_head", "after_head", "before", "after"],
    )?;
    exact_object(
        &top["snapshots"]["primary_untracked"],
        "snapshots.primary_untracked",
        &[
            "before",
            "after",
            "exclusion_prefixes",
            "excluded_before",
            "excluded_after",
            "post_exclusion_before",
            "post_exclusion_after",
        ],
    )?;
    exact_object(
        &top["snapshots"]["live_planning"],
        "snapshots.live_planning",
        &["names", "before", "after"],
    )?;
    exact_object(
        &top["projection_log"],
        "projection_log",
        &PROJECTION_LOG_KEYS,
    )?;
    for (index, abort) in top["aborts"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/aborts", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(
            abort,
            &format!("aborts[{index}]"),
            &[
                "attempt_id",
                "evidence",
                "route",
                "issuance_sequence",
                "stage",
                "observed_cause",
                "retry_disposition",
            ],
        )?;
    }
    let artifacts = exact_object(
        &top["artifacts"],
        "artifacts",
        &["targets", "retained_files"],
    )?;
    for (index, target) in artifacts["targets"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/artifact-targets", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(
            target,
            &format!("artifacts.targets[{index}]"),
            &["attempt_id", "normalized_path", "presence_observed"],
        )?;
    }
    for (index, retained) in artifacts["retained_files"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/retained-files", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(
            retained,
            &format!("artifacts.retained_files[{index}]"),
            &["name", "byte_count"],
        )?;
    }
    let hashes = exact_object(&top["hashes"], "hashes", &["verification_output"])?;
    exact_object(
        &hashes["verification_output"],
        "hashes.verification_output",
        &["byte_count", "sha256"],
    )?;
    Ok(manifest)
}

fn parse_projection(path: &Path) -> Result<Value, FixtureVerificationError> {
    let projection = parse_json_file(path)?;
    let top = exact_object(&projection, "projection", &["normalization", "projections"])?;
    for (index, value) in top["projections"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/projections", "not-array"))?
        .iter()
        .enumerate()
    {
        exact_object(value, &format!("projections[{index}]"), &PROJECTION_KEYS)?;
        exact_object(&value["raw"], "projection.raw", &PROJECTION_SURFACE_KEYS)?;
        exact_object(
            &value["normalized"],
            "projection.normalized",
            &PROJECTION_SURFACE_KEYS,
        )?;
        exact_object(
            &value["normalized"]["stdin_binding"],
            "projection.stdin_binding",
            &["kind", "bytes", "byte_count", "sha256"],
        )?;
    }
    Ok(projection)
}

fn event_values(bytes: &[u8]) -> Result<Vec<Value>, FixtureVerificationError> {
    std::str::from_utf8(bytes)
        .map_err(|error| fixture_error("fixture/events/utf8", error.to_string()))?
        .lines()
        .enumerate()
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|error| {
                fixture_error(
                    "fixture/events/json",
                    format!("line={}; error={error}", index + 1),
                )
            })
        })
        .collect()
}

fn canonical_registry() -> [(&'static str, &'static str); 11] {
    [
        ("m8-route/registry/repository-analyst", "repository-analyst"),
        ("m8-route/registry/milestone-planner", "milestone-planner"),
        ("m8-route/registry/milestone-critic", "milestone-critic"),
        ("m8-route/registry/step-planner", "step-planner"),
        ("m8-route/registry/step-critic", "step-critic"),
        ("m8-route/registry/step-plan-writer", "step-plan-writer"),
        ("m8-route/registry/step-plan-critic", "step-plan-critic"),
        ("m8-route/registry/step-executor", "step-executor"),
        ("m8-route/registry/pr-reviewer", "pr-reviewer"),
        ("m8-route/purpose/codex-diagnostics", "step-executor"),
        ("m8-route/purpose/codex-commit-completion", "step-executor"),
    ]
}

fn verify_m8_fixture(root: &Path, delivered: bool) -> Result<(), FixtureVerificationError> {
    let observed: Vec<_> = M8_FILES
        .iter()
        .filter(|name| root.join(name).is_file())
        .copied()
        .collect();
    if observed != M8_FILES {
        return Err(fixture_error(
            "fixture/files/complete",
            format!("expected={M8_FILES:?}; observed={observed:?}"),
        ));
    }
    let manifest_bytes = fs::read(root.join("provenance-manifest.json"))
        .map_err(|error| fixture_error("fixture/files/read", error.to_string()))?;
    let manifest_digest = sha256(&manifest_bytes);
    if delivered && synthetic_manifest_digests().contains(&manifest_digest.as_str()) {
        return Err(fixture_error(
            "fixture/provenance/synthetic-digest-rejected",
            format!(
                "delivered_digest={manifest_digest}; synthetic_digests={:?}",
                synthetic_manifest_digests()
            ),
        ));
    }
    let manifest = parse_manifest(&root.join("provenance-manifest.json"))?;
    let projection = parse_projection(&root.join("dry-run-projection.json"))?;
    let mode = manifest["schema_mode"].as_str().unwrap_or_default();
    if manifest["host"]["preflight"]["selected_mode"] != manifest["schema_mode"] {
        return Err(fixture_error(
            "fixture/schema/mode-consistency",
            format!(
                "schema_mode={}; selected_mode={}",
                manifest["schema_mode"], manifest["host"]["preflight"]["selected_mode"]
            ),
        ));
    }
    let events_bytes = fs::read(root.join("events.jsonl"))
        .map_err(|error| fixture_error("fixture/files/read", error.to_string()))?;
    let events = event_values(&events_bytes)?;
    let issuances: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "dispatch")
        .collect();
    let completions: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "dispatch-completion")
        .collect();
    let projections = projection["projections"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/projections", "not-array"))?;

    let expected: std::collections::BTreeMap<_, _> = canonical_registry().into_iter().collect();
    let registry_entries = manifest["identity_registry"]["canonical"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/registry", "not-array"))?;
    let registry: std::collections::BTreeMap<_, _> = registry_entries
        .iter()
        .filter_map(|entry| {
            Some((
                entry["evidence"].as_str()?,
                entry["expected_role"].as_str()?,
            ))
        })
        .collect();
    if registry != expected {
        return Err(fixture_error(
            "fixture/identities/registry",
            format!("expected={expected:?}; observed={registry:?}"),
        ));
    }
    let mut actual = Vec::new();
    for issuance in &issuances {
        actual.push((
            issuance["payload"]["evidence"].as_str().unwrap_or_default(),
            issuance["payload"]["role"].as_str().unwrap_or_default(),
        ));
    }
    for item in projections {
        actual.push((
            item["evidence"].as_str().unwrap_or_default(),
            item["issuance"]["payload"]["role"]
                .as_str()
                .unwrap_or_default(),
        ));
    }
    let actual_names: std::collections::BTreeSet<_> =
        actual.iter().map(|(evidence, _)| *evidence).collect();
    let expected_names: std::collections::BTreeSet<_> = expected.keys().copied().collect();
    let missing: Vec<_> = expected_names.difference(&actual_names).copied().collect();
    if !missing.is_empty() {
        return Err(fixture_error(
            "fixture/identities/missing",
            format!("expected={expected_names:?}; observed={actual_names:?}; missing={missing:?}"),
        ));
    }
    let extra: Vec<_> = actual_names
        .difference(&expected_names)
        .copied()
        .filter(|evidence| !evidence.starts_with("m8-route/retry/"))
        .collect();
    if !extra.is_empty() {
        return Err(fixture_error(
            "fixture/identities/extra",
            format!("expected={expected_names:?}; observed={actual_names:?}; extra={extra:?}"),
        ));
    }
    for evidence in &expected_names {
        let count = actual
            .iter()
            .filter(|(candidate, _)| candidate == evidence)
            .count();
        if count != 1 {
            return Err(fixture_error(
                "fixture/identities/canonical-unique",
                format!(
                    "evidence={evidence}; expected_count=1; observed_count={count}; actual={actual:?}"
                ),
            ));
        }
    }
    let retries = manifest["retries"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/retries", "not-array"))?;
    let retry_actual: Vec<_> = actual
        .iter()
        .filter(|(evidence, _)| evidence.starts_with("m8-route/retry/"))
        .collect();
    let mut retry_seen = std::collections::BTreeSet::new();
    for (evidence, role) in retry_actual {
        if !retry_seen.insert(*evidence) {
            return Err(fixture_error(
                "fixture/identities/retry-unique",
                format!("evidence={evidence}; actual={actual:?}"),
            ));
        }
        let links: Vec<_> = retries
            .iter()
            .filter(|retry| retry["retry_evidence"] == *evidence)
            .collect();
        if links.is_empty() {
            return Err(fixture_error(
                "fixture/identities/retry-unlinked",
                format!("evidence={evidence}; retries={retries:?}"),
            ));
        }
        let suffix = evidence.rsplit('/').next().unwrap_or_default();
        if links.len() != 1 || links[0]["substitute_role"] != *role || suffix != *role {
            return Err(fixture_error(
                "fixture/identities/retry-mismatch",
                format!("evidence={evidence}; role={role}; links={links:?}"),
            ));
        }
    }
    for (evidence, role) in &actual {
        if let Some(expected_role) = expected.get(evidence)
            && role != expected_role
        {
            return Err(fixture_error(
                format!("fixture/binding/{evidence}"),
                format!("registry_role={expected_role}; payload_role={role}"),
            ));
        }
    }

    let attempts = manifest["attempts"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/attempts", "not-array"))?;
    let qualifying: Vec<_> = attempts
        .iter()
        .filter(|attempt| {
            attempt["evidence"] == "m8-route/purpose/codex-diagnostics"
                && attempt["disposition"] == "qualifying-interruption"
                && attempt["completion_sequence"].is_null()
        })
        .collect();
    if qualifying.len() != 1 {
        return Err(fixture_error(
            "fixture/lifecycle/qualifying-interruption-missing",
            format!("qualifying={qualifying:?}"),
        ));
    }
    let aborts = manifest["aborts"]
        .as_array()
        .ok_or_else(|| fixture_error("fixture/schema/aborts", "not-array"))?;
    if aborts
        .iter()
        .any(|abort| abort["evidence"] == "m8-route/purpose/codex-diagnostics")
    {
        return Err(fixture_error(
            "fixture/aborts/qualifying-declared",
            format!("aborts={aborts:?}"),
        ));
    }
    let mut abort_keys = std::collections::BTreeSet::new();
    for abort in aborts {
        exact_object(
            abort,
            "abort",
            &[
                "attempt_id",
                "evidence",
                "route",
                "issuance_sequence",
                "stage",
                "observed_cause",
                "retry_disposition",
            ],
        )?;
        let key = (
            abort["attempt_id"].to_string(),
            abort["issuance_sequence"].to_string(),
        );
        if !abort_keys.insert(key) {
            return Err(fixture_error(
                "fixture/aborts/duplicate-declaration",
                format!("abort={abort}"),
            ));
        }
        if completions.iter().any(|completion| {
            completion["payload"]["issuance_sequence"] == abort["issuance_sequence"]
        }) {
            return Err(fixture_error(
                "fixture/aborts/completed-declaration",
                format!("abort={abort}"),
            ));
        }
        if !issuances
            .iter()
            .any(|issuance| issuance["sequence"] == abort["issuance_sequence"])
        {
            return Err(fixture_error(
                "fixture/aborts/unmatched-declaration",
                format!("abort={abort}"),
            ));
        }
        if abort["stage"] != "child-lost-before-completion" {
            return Err(fixture_error(
                "fixture/aborts/stage",
                format!("abort={abort}"),
            ));
        }
        if abort["observed_cause"].as_str().is_none_or(str::is_empty) {
            return Err(fixture_error(
                "fixture/aborts/nonempty-cause",
                format!("abort={abort}"),
            ));
        }
        if abort["retry_disposition"]
            .as_str()
            .is_none_or(str::is_empty)
        {
            return Err(fixture_error(
                "fixture/aborts/nonempty-disposition",
                format!("abort={abort}"),
            ));
        }
    }
    for issuance in issuances.iter().filter(|issuance| {
        let sequence = &issuance["sequence"];
        !completions
            .iter()
            .any(|completion| completion["payload"]["issuance_sequence"] == *sequence)
            && issuance["payload"]["evidence"] != "m8-route/purpose/codex-diagnostics"
    }) {
        let count = aborts
            .iter()
            .filter(|abort| abort["issuance_sequence"] == issuance["sequence"])
            .count();
        if count != 1 {
            return Err(fixture_error(
                "fixture/aborts/undeclared-issuance",
                format!("issuance={issuance}; declaration_count={count}"),
            ));
        }
    }
    for attempt in attempts {
        let completion = completions.iter().find(|completion| {
            completion["payload"]["issuance_sequence"] == attempt["issuance_sequence"]
        });
        if completion.is_some() && attempt["child_exit"].is_null() {
            return Err(fixture_error(
                "fixture/exits/completion-without-observation",
                format!("attempt={attempt}; completion={completion:?}"),
            ));
        }
        if completion.is_none() && !attempt["child_exit"].is_null() {
            return Err(fixture_error(
                "fixture/exits/observation-without-completion",
                format!("attempt={attempt}"),
            ));
        }
        if let Some(completion) = completion {
            if attempt["duration_ms"] != completion["payload"]["duration_ms"] {
                return Err(fixture_error(
                    "fixture/duration/event-vs-manifest",
                    format!(
                        "attempt_duration={}; event_duration={}",
                        attempt["duration_ms"], completion["payload"]["duration_ms"]
                    ),
                ));
            }
            if attempt["usage"] != completion["payload"]["usage"] {
                return Err(fixture_error(
                    "fixture/usage/event-vs-manifest",
                    format!(
                        "attempt_usage={}; event_usage={}",
                        attempt["usage"], completion["payload"]["usage"]
                    ),
                ));
            }
        }
    }
    let pr = attempts
        .iter()
        .find(|attempt| attempt["evidence"] == "m8-route/registry/pr-reviewer")
        .ok_or_else(|| fixture_error("fixture/validation/pr-reviewer", "missing"))?;
    if pr["disposition"] != "retained-unsuccessful" {
        return Err(fixture_error(
            "fixture/exits/unsuccessful-retained",
            format!("disposition={}", pr["disposition"]),
        ));
    }
    let codex: Vec<_> = attempts
        .iter()
        .filter(|attempt| attempt["route"] == "codex" && attempt["disposition"] == "successful")
        .collect();
    if codex.len() != 2
        || codex
            .iter()
            .any(|attempt| attempt["usage"]["availability"] != "measured")
    {
        return Err(fixture_error(
            "fixture/usage/codex-availability",
            format!("codex={codex:?}"),
        ));
    }
    let gate: Vec<_> = attempts
        .iter()
        .filter(|attempt| attempt["route"] == "gate" && attempt["disposition"] == "successful")
        .collect();
    if gate.len() != 2
        || gate
            .iter()
            .any(|attempt| attempt["usage"]["availability"] != "claude-measured")
    {
        return Err(fixture_error(
            "fixture/usage/gate-availability",
            format!("gate={gate:?}"),
        ));
    }
    if codex[0]["usage"] == codex[1]["usage"] {
        return Err(fixture_error(
            "fixture/usage/codex-responsive",
            format!("a={}; b={}", codex[0]["usage"], codex[1]["usage"]),
        ));
    }
    if gate[0]["usage"] == gate[1]["usage"] {
        return Err(fixture_error(
            "fixture/usage/gate-responsive",
            format!("a={}; b={}", gate[0]["usage"], gate[1]["usage"]),
        ));
    }
    let target = manifest["artifacts"]["targets"]
        .as_array()
        .and_then(|targets| {
            targets
                .iter()
                .find(|target| target["attempt_id"] == pr["attempt_id"])
        })
        .ok_or_else(|| fixture_error("fixture/validation/target", "missing"))?;
    if pr["artifact_present"] != target["presence_observed"] || pr["artifact_present"] != false {
        return Err(fixture_error(
            "fixture/validation/target-absent",
            format!(
                "attempt_present={}; inventory_present={}",
                pr["artifact_present"], target["presence_observed"]
            ),
        ));
    }
    if pr["parent_exit"].as_i64().is_none_or(|exit| exit == 0) {
        return Err(fixture_error(
            "fixture/validation/nonzero-parent",
            format!("parent_exit={}", pr["parent_exit"]),
        ));
    }
    if pr["validation_classification"] != "missing-artifact" {
        return Err(fixture_error(
            "fixture/validation/failure-class",
            format!("classification={}", pr["validation_classification"]),
        ));
    }

    for item in projections {
        let evidence = item["evidence"].as_str().unwrap_or_default();
        let raw = &item["raw"];
        let normalized = &item["normalized"];
        for (field, suffix) in [
            ("child_argv", "argv"),
            ("parent_argv", "parent-argv"),
            ("cwd", "cwd"),
            ("environment", "environment"),
            ("schema_path", "schema-path"),
            ("output_path", "output-path"),
        ] {
            if raw[field] != normalized[field] {
                let site = if suffix == "argv" {
                    format!("fixture/projection/{evidence}/argv")
                } else {
                    format!("fixture/projection/{suffix}")
                };
                return Err(fixture_error(
                    site,
                    format!("raw={}; normalized={}", raw[field], normalized[field]),
                ));
            }
        }
        let stdin = &normalized["stdin_binding"];
        let bytes = stdin["bytes"].as_str().unwrap_or_default().as_bytes();
        if stdin["byte_count"] != bytes.len() || stdin["sha256"] != sha256(bytes) {
            return Err(fixture_error(
                "fixture/projection/stdin-binding",
                format!(
                    "retained={stdin}; recomputed_byte_count={}; recomputed_sha256={}",
                    bytes.len(),
                    sha256(bytes)
                ),
            ));
        }
    }
    let projection_log = &manifest["projection_log"];
    if projection_log["before_bytes"] != projection_log["after_bytes"]
        || projection_log["before_sha256"] != projection_log["after_sha256"]
    {
        return Err(fixture_error(
            "fixture/projection/log-before-vs-after",
            format!(
                "before=({}, {}); after=({}, {})",
                projection_log["before_bytes"],
                projection_log["before_sha256"],
                projection_log["after_bytes"],
                projection_log["after_sha256"]
            ),
        ));
    }
    for control in manifest["auth_controls"].as_array().unwrap_or(&Vec::new()) {
        let attempt = attempts
            .iter()
            .find(|attempt| attempt["attempt_id"] == control["attempt_id"])
            .ok_or_else(|| {
                fixture_error("fixture/auth/control-attempt", format!("control={control}"))
            })?;
        let parsed_prefix = attempt["redacted_parent_argv"]
            .as_array()
            .is_some_and(|argv| {
                argv.iter().any(|token| {
                    token
                        .as_str()
                        .is_some_and(|token| token.starts_with("ANTHROPIC_API_KEY="))
                })
            });
        if control["parent_prefix_present"] != parsed_prefix {
            return Err(fixture_error(
                "fixture/auth/parent-command-shape",
                format!(
                    "recorded={}; parsed={parsed_prefix}; argv={}",
                    control["parent_prefix_present"], attempt["redacted_parent_argv"]
                ),
            ));
        }
    }
    if mode == "child-environment-observable" {
        for attempt in attempts
            .iter()
            .filter(|attempt| matches!(attempt["attempt_id"].as_str(), Some("a3" | "a4")))
        {
            let names = attempt["child_environment_observation"]["observed_names"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if !names.iter().any(|name| name == "PATH") {
                return Err(fixture_error(
                    "fixture/auth/child-path",
                    format!("names={names:?}"),
                ));
            }
            if !names.iter().any(|name| name == "HOME") {
                return Err(fixture_error(
                    "fixture/auth/child-home",
                    format!("names={names:?}"),
                ));
            }
            if names.iter().any(|name| name == "ANTHROPIC_API_KEY") {
                return Err(fixture_error(
                    "fixture/auth/child-sentinel-absent",
                    format!("names={names:?}"),
                ));
            }
        }
    }
    for control in manifest["capability_controls"]
        .as_array()
        .unwrap_or(&Vec::new())
    {
        if control["projected_disallowed_tools"] != control["issued_disallowed_tools"] {
            return Err(fixture_error(
                format!(
                    "fixture/capability/projected-vs-issued/{}",
                    control["attempt_id"].as_str().unwrap_or_default()
                ),
                format!(
                    "projected={}; issued={}",
                    control["projected_disallowed_tools"], control["issued_disallowed_tools"]
                ),
            ));
        }
    }
    let pid = &manifest["pid_observations"][0];
    if pid["direct_child_pid"].is_null()
        || pid["termination_targets"]
            .as_array()
            .is_none_or(Vec::is_empty)
        || !pid["survivors_after_wait"]
            .as_array()
            .is_some_and(Vec::is_empty)
    {
        return Err(fixture_error(
            "fixture/lifecycle/pid-observation",
            format!("pid_observation={pid}"),
        ));
    }
    let snapshots = &manifest["snapshots"];
    if snapshots["primary_tracked"]["before_head"] != snapshots["primary_tracked"]["after_head"]
        || snapshots["primary_tracked"]["before"] != snapshots["primary_tracked"]["after"]
    {
        return Err(fixture_error(
            "fixture/snapshot/primary-tracked-before-vs-after",
            format!(
                "before={}; after={}",
                snapshots["primary_tracked"]["before"], snapshots["primary_tracked"]["after"]
            ),
        ));
    }
    if snapshots["primary_untracked"]["exclusion_prefixes"]
        != serde_json::json!(["orchestrator-feedback/", ".agents/"])
        || snapshots["primary_untracked"]["post_exclusion_before"]
            != snapshots["primary_untracked"]["post_exclusion_after"]
    {
        return Err(fixture_error(
            "fixture/snapshot/primary-untracked-before-vs-after",
            format!(
                "before={}; after={}; exclusions={}",
                snapshots["primary_untracked"]["post_exclusion_before"],
                snapshots["primary_untracked"]["post_exclusion_after"],
                snapshots["primary_untracked"]["exclusion_prefixes"]
            ),
        ));
    }
    let names = snapshots["live_planning"]["names"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if names.len() != 10
        || snapshots["live_planning"]["before"] != snapshots["live_planning"]["after"]
    {
        return Err(fixture_error(
            "fixture/snapshot/live-planning-before-vs-after",
            format!(
                "names={names:?}; before={}; after={}",
                snapshots["live_planning"]["before"], snapshots["live_planning"]["after"]
            ),
        ));
    }
    for target in manifest["artifacts"]["targets"]
        .as_array()
        .unwrap_or(&Vec::new())
    {
        if !target["normalized_path"]
            .as_str()
            .is_some_and(|path| path.starts_with("<CAMPAIGN_REPO>/.campaign-output/"))
        {
            return Err(fixture_error(
                "fixture/projection/output-path",
                format!("target={target}"),
            ));
        }
    }
    let verification = fs::read(root.join("verification-output.txt"))
        .map_err(|error| fixture_error("fixture/files/read", error.to_string()))?;
    let hash = &manifest["hashes"]["verification_output"];
    if hash["byte_count"] != verification.len() {
        return Err(fixture_error(
            "fixture/inventory/verification-output-byte-count",
            format!(
                "recorded={}; recomputed={}",
                hash["byte_count"],
                verification.len()
            ),
        ));
    }
    if hash["sha256"] != sha256(&verification) {
        return Err(fixture_error(
            "fixture/inventory/verification-output-hash",
            format!(
                "recorded={}; recomputed={}",
                hash["sha256"],
                sha256(&verification)
            ),
        ));
    }
    let output = run_meter(&["log", "meter"], &events_bytes);
    if !output.status.success() {
        return Err(fixture_error(
            "fixture/meter/correlation",
            complete_measurement(&output),
        ));
    }
    let retained_report = fs::read(root.join("meter-report.json"))
        .map_err(|error| fixture_error("fixture/files/read", error.to_string()))?;
    if output.stdout != retained_report {
        return Err(fixture_error(
            "fixture/meter/report-bytes",
            format!(
                "recomputed=(bytes={}, sha256={}); retained=(bytes={}, sha256={})",
                output.stdout.len(),
                sha256(&output.stdout),
                retained_report.len(),
                sha256(&retained_report)
            ),
        ));
    }
    Ok(())
}

fn synthetic_manifest_digests() -> [&'static str; 2] {
    [
        "9a757086e676c52a15b9e8e58bb421ad6eef1158e5ee7a78f0b7ba9315acbfa4",
        "0aa4a49485da95f570b3a9b5a03217a5f2ca6605b063ee6cf52cedce0c70d866",
    ]
}

fn frozen_inventory(mode: &str) -> Value {
    serde_json::json!({
        "selected_mode": mode,
        "top_level": expected_keys(&MANIFEST_KEYS),
        "attempt": expected_keys(&ATTEMPT_PLAN_KEYS),
        "observable_attempt": expected_keys(&ATTEMPT_OBSERVABLE_KEYS),
        "pid_observation": expected_keys(&PID_KEYS),
        "auth_control": expected_keys(if mode == "child-environment-observable" { &AUTH_KEYS_OBSERVABLE } else { &AUTH_KEYS_NOT_OBSERVABLE }),
        "auth_control_not_observable": expected_keys(&AUTH_KEYS_NOT_OBSERVABLE),
        "auth_control_observable": expected_keys(&AUTH_KEYS_OBSERVABLE),
        "child_environment_observation": expected_keys(&CHILD_ENV_KEYS),
        "snapshot": expected_keys(&SNAPSHOT_KEYS),
        "projection_log": expected_keys(&PROJECTION_LOG_KEYS),
        "projection": expected_keys(&PROJECTION_KEYS),
        "projection_surface": expected_keys(&PROJECTION_SURFACE_KEYS),
    })
}

#[test]
fn m8_s2_synthetic_fixture_verifies() {
    let Some(root) = Path::new(M8_S2_SYNTHETIC_ROOT)
        .exists()
        .then_some(Path::new(M8_S2_SYNTHETIC_ROOT))
    else {
        return;
    };
    let result = verify_m8_fixture(root, false);
    assert!(
        result.is_ok(),
        "{}",
        result.err().map_or_else(
            || "unknown verification failure".to_owned(),
            |error| error.to_string()
        )
    );
}

#[test]
fn m8_s2_fixture_copy_verifies() {
    let Some(root) = std::env::var_os("M8_S2_FIXTURE_COPY_ROOT") else {
        return;
    };
    let result = verify_m8_fixture(Path::new(&root), false);
    assert!(
        result.is_ok(),
        "{}",
        result.err().map_or_else(
            || "unknown verification failure".to_owned(),
            |error| error.to_string()
        )
    );
}

#[test]
fn m8_s2_workspace_fixture_verifies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/m8-real-dispatch");
    let present = M8_FILES
        .iter()
        .filter(|name| root.join(name).is_file())
        .count();
    assert!(
        present == 0 || present == M8_FILES.len(),
        "fixture/files/partial-presence: present={present}; expected=0-or-{}",
        M8_FILES.len()
    );
    if present == M8_FILES.len() {
        let result = verify_m8_fixture(&root, true);
        assert!(
            result.is_ok(),
            "{}",
            result.err().map_or_else(
                || "unknown verification failure".to_owned(),
                |error| error.to_string()
            )
        );
    }
}

#[test]
fn m8_s2_manifest_inventory_is_parser_derived() {
    let mode = std::env::var("M8_S2_SCHEMA_MODE")
        .unwrap_or_else(|_| "child-environment-not-observable".to_owned());
    let root = if mode == "child-environment-observable" {
        Path::new(M8_S2_SYNTHETIC_ROOT)
            .parent()
            .expect("synthetic parent should exist")
            .join("observable")
    } else {
        Path::new(M8_S2_SYNTHETIC_ROOT).to_path_buf()
    };
    let Some(root) = root.exists().then_some(root) else {
        return;
    };
    let manifest = parse_manifest(&root.join("provenance-manifest.json"));
    assert!(
        manifest.is_ok(),
        "fixture/schema/inventory-source: {}",
        manifest
            .err()
            .map_or_else(|| "unknown".to_owned(), |error| error.to_string())
    );
    let inventory = frozen_inventory(&mode);
    assert_eq!(
        inventory["top_level"],
        serde_json::json!(expected_keys(&MANIFEST_KEYS)),
        "fixture/schema/inventory/top-level"
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&inventory).expect("inventory should serialize")
    );
    if let Some(path) = std::env::var_os("M8_S2_INVENTORY_OUTPUT") {
        fs::write(
            path,
            serde_json::to_vec(&inventory).expect("inventory should serialize"),
        )
        .expect("inventory receipt should write");
    }
}

#[test]
fn m8_s2_synthetic_digest_is_rejected_for_delivery() {
    let Some(root) = Path::new(M8_S2_SYNTHETIC_ROOT)
        .exists()
        .then_some(Path::new(M8_S2_SYNTHETIC_ROOT))
    else {
        return;
    };
    let result = verify_m8_fixture(root, true);
    let error = result.expect_err("synthetic manifest must not be deliverable");
    assert_eq!(
        error.site, "fixture/provenance/synthetic-digest-rejected",
        "fixture/provenance/synthetic-digest-falsifier: observed={error}"
    );
}

fn write_json(path: &Path, value: &Value) {
    fs::write(
        path,
        serde_json::to_vec_pretty(value).expect("mutation JSON should serialize"),
    )
    .expect("mutation JSON should write");
}

fn write_events(path: &Path, events: &[Value]) {
    let mut bytes = Vec::new();
    for event in events {
        serde_json::to_writer(&mut bytes, event).expect("mutation event should serialize");
        bytes.push(b'\n');
    }
    fs::write(path, bytes).expect("mutation events should write");
}

fn event_by_evidence_mut<'a>(events: &'a mut [Value], evidence: &str) -> &'a mut Value {
    events
        .iter_mut()
        .find(|event| event["kind"] == "dispatch" && event["payload"]["evidence"] == evidence)
        .expect("cycle issuance should exist")
}

fn completion_mut(events: &mut [Value], issuance_sequence: u64) -> &mut Value {
    events
        .iter_mut()
        .find(|event| {
            event["kind"] == "dispatch-completion"
                && event["payload"]["issuance_sequence"] == issuance_sequence
        })
        .expect("cycle completion should exist")
}

fn attempt_mut<'a>(manifest: &'a mut Value, evidence: &str) -> &'a mut Value {
    manifest["attempts"]
        .as_array_mut()
        .expect("attempts should be an array")
        .iter_mut()
        .find(|attempt| attempt["evidence"] == evidence)
        .expect("cycle attempt should exist")
}

fn projection_mut<'a>(projection: &'a mut Value, evidence: &str) -> &'a mut Value {
    projection["projections"]
        .as_array_mut()
        .expect("projections should be an array")
        .iter_mut()
        .find(|item| item["evidence"] == evidence)
        .expect("cycle projection should exist")
}

fn add_green_retry(manifest: &mut Value, events: &mut Vec<Value>) {
    events.push(serde_json::json!({
        "sequence": 12,
        "timestamp": "2026-08-02T10:00:11Z",
        "kind": "dispatch",
        "node": "m8-s1",
        "payload": {
            "role": "step-critic",
            "ref": "ff96d11e6fb5054c9e6e0b037ca26767344e2602",
            "evidence": "m8-route/retry/1/step-critic"
        }
    }));
    manifest["retries"]
        .as_array_mut()
        .expect("retries should be an array")
        .push(serde_json::json!({
            "retry_evidence": "m8-route/retry/1/step-critic",
            "retry_ordinal": 1,
            "substitute_role": "step-critic",
            "replaces_attempt_id": "a4",
            "replaces_canonical_evidence": "m8-route/registry/milestone-critic"
        }));
    manifest["aborts"]
        .as_array_mut()
        .expect("aborts should be an array")
        .push(serde_json::json!({
            "attempt_id": "retry-1",
            "evidence": "m8-route/retry/1/step-critic",
            "route": "gate",
            "issuance_sequence": 12,
            "stage": "child-lost-before-completion",
            "observed_cause": "synthetic retry child lost",
            "retry_disposition": "retry exhausted"
        }));
}

fn add_second_retry(manifest: &mut Value, events: &mut Vec<Value>, with_link: bool) {
    events.push(serde_json::json!({
        "sequence": 13,
        "timestamp": "2026-08-02T10:00:12Z",
        "kind": "dispatch",
        "node": "m8-s2",
        "payload": {
            "role": "step-plan-critic",
            "ref": "ff96d11e6fb5054c9e6e0b037ca26767344e2602",
            "evidence": "m8-route/retry/2/step-plan-critic"
        }
    }));
    if with_link {
        manifest["retries"]
            .as_array_mut()
            .expect("retries should be an array")
            .push(serde_json::json!({
                "retry_evidence": "m8-route/retry/2/step-plan-critic",
                "retry_ordinal": 2,
                "substitute_role": "step-plan-critic",
                "replaces_attempt_id": "a4",
                "replaces_canonical_evidence": "m8-route/registry/milestone-critic"
            }));
    }
}

fn cycle_expected_site(cycle: &str) -> String {
    match cycle {
        "I01" => "fixture/identities/missing".to_owned(),
        "I02" => "fixture/identities/extra".to_owned(),
        "I03" => "fixture/identities/canonical-unique".to_owned(),
        "I04" => "fixture/identities/retry-unique".to_owned(),
        "I05" => "fixture/identities/retry-unlinked".to_owned(),
        "I06" => "fixture/identities/retry-mismatch".to_owned(),
        value if value.starts_with('B') => {
            let evidence = match value {
                "B01" => "m8-route/registry/repository-analyst",
                "B02" => "m8-route/registry/milestone-planner",
                "B03" => "m8-route/registry/milestone-critic",
                "B04" => "m8-route/registry/step-planner",
                "B05" => "m8-route/registry/step-critic",
                "B06" => "m8-route/registry/step-plan-writer",
                "B07" => "m8-route/registry/step-plan-critic",
                "B08" => "m8-route/registry/step-executor",
                "B09" => "m8-route/registry/pr-reviewer",
                "B10" => "m8-route/purpose/codex-diagnostics",
                "B11" => "m8-route/purpose/codex-commit-completion",
                _ => unreachable!(),
            };
            format!("fixture/binding/{evidence}")
        }
        "L01" => "fixture/meter/correlation".to_owned(),
        "L02" => "fixture/lifecycle/qualifying-interruption-missing".to_owned(),
        "L03" | "L04" => "fixture/aborts/undeclared-issuance".to_owned(),
        "L05" => "fixture/aborts/nonempty-cause".to_owned(),
        "L06" => "fixture/aborts/unmatched-declaration".to_owned(),
        "L07" => "fixture/aborts/completed-declaration".to_owned(),
        "L08" => "fixture/aborts/duplicate-declaration".to_owned(),
        "L09" => "fixture/exits/completion-without-observation".to_owned(),
        "L10" => "fixture/exits/observation-without-completion".to_owned(),
        "L11" => "fixture/exits/unsuccessful-retained".to_owned(),
        "L12" => "fixture/aborts/nonempty-disposition".to_owned(),
        "L13" => "fixture/aborts/qualifying-declared".to_owned(),
        "L14" => "fixture/aborts/stage".to_owned(),
        "M01A" | "M01B" => "fixture/usage/codex-availability".to_owned(),
        "M02A" | "M02B" => "fixture/usage/gate-availability".to_owned(),
        "M03" => "fixture/usage/codex-responsive".to_owned(),
        "M04" => "fixture/usage/gate-responsive".to_owned(),
        "M05" => "fixture/duration/event-vs-manifest".to_owned(),
        "M06" => "fixture/validation/target-absent".to_owned(),
        "M07" => "fixture/validation/nonzero-parent".to_owned(),
        "M08" => "fixture/validation/failure-class".to_owned(),
        "M09" => "fixture/meter/report-bytes".to_owned(),
        "M10" => "fixture/inventory/verification-output-hash".to_owned(),
        value if matches!(value, "P01" | "P02" | "P03" | "P04" | "P05") => {
            let evidence = match value {
                "P01" => "m8-route/registry/step-plan-writer",
                "P02" => "m8-route/registry/step-executor",
                "P03" => "m8-route/purpose/codex-commit-completion",
                "P04" => "m8-route/registry/step-critic",
                "P05" => "m8-route/registry/step-plan-critic",
                _ => unreachable!(),
            };
            format!("fixture/projection/{evidence}/argv")
        }
        "P06" => "fixture/projection/log-before-vs-after".to_owned(),
        "P07" => "fixture/snapshot/primary-tracked-before-vs-after".to_owned(),
        "P08" => "fixture/snapshot/live-planning-before-vs-after".to_owned(),
        "P09" => "fixture/auth/child-path".to_owned(),
        "P10" => "fixture/auth/child-home".to_owned(),
        "P11" => "fixture/auth/child-sentinel-absent".to_owned(),
        "P12" => "fixture/auth/parent-command-shape".to_owned(),
        "P13" => "fixture/snapshot/primary-untracked-before-vs-after".to_owned(),
        "P14" => "fixture/capability/projected-vs-issued/a3".to_owned(),
        "P15" => "fixture/projection/parent-argv".to_owned(),
        "P16" => "fixture/projection/cwd".to_owned(),
        "P17" => "fixture/projection/environment".to_owned(),
        "P18" => "fixture/projection/stdin-binding".to_owned(),
        "P19" => "fixture/projection/schema-path".to_owned(),
        "P20" => "fixture/projection/output-path".to_owned(),
        "P21" => "fixture/lifecycle/pid-observation".to_owned(),
        _ => panic!("unknown cycle {cycle}"),
    }
}

fn mutate_cycle(
    cycle: &str,
    manifest: &mut Value,
    projection: &mut Value,
    events: &mut Vec<Value>,
) {
    match cycle {
        "I01" => projection["projections"]
            .as_array_mut()
            .expect("projections array")
            .retain(|item| item["evidence"] != "m8-route/registry/step-plan-writer"),
        "I02" | "I03" => {
            let (evidence, role) = if cycle == "I02" {
                ("m8-route/outside/unknown", "step-executor")
            } else {
                ("m8-route/registry/milestone-planner", "milestone-planner")
            };
            events.push(serde_json::json!({"sequence":12,"timestamp":"2026-08-02T10:00:11Z","kind":"dispatch","node":"m8-s2","payload":{"role":role,"ref":"ff96d11e6fb5054c9e6e0b037ca26767344e2602","evidence":evidence}}));
            events.push(serde_json::json!({"sequence":13,"timestamp":"2026-08-02T10:00:12Z","kind":"dispatch-completion","node":"m8-s2","payload":{"issuance_sequence":12,"duration_ms":1,"usage":{"availability":"absent","reason":"synthetic"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}));
        }
        "I04" => events.push(serde_json::json!({"sequence":13,"timestamp":"2026-08-02T10:00:12Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-critic","ref":"ff96d11e6fb5054c9e6e0b037ca26767344e2602","evidence":"m8-route/retry/1/step-critic"}})),
        "I05" => add_second_retry(manifest, events, false),
        "I06" => manifest["retries"][0]["substitute_role"] = Value::String("step-plan-critic".to_owned()),
        value if value.starts_with('B') => {
            let evidence = cycle_expected_site(value).trim_start_matches("fixture/binding/").to_owned();
            if matches!(value, "B01" | "B02" | "B03" | "B04" | "B09" | "B10") {
                event_by_evidence_mut(events, &evidence)["payload"]["role"] = Value::String("wrong-role".to_owned());
            } else {
                projection_mut(projection, &evidence)["issuance"]["payload"]["role"] = Value::String("wrong-role".to_owned());
            }
        }
        "L01" => events.push(serde_json::json!({"sequence":12,"timestamp":"2026-08-02T10:00:11Z","kind":"dispatch-completion","node":"m1-s1","payload":{"issuance_sequence":1,"duration_ms":101,"usage":{"availability":"measured","input_tokens":100,"cached_input_tokens":10,"output_tokens":20,"reasoning_output_tokens":2},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"validated"}})),
        "L02" => {
            events.push(serde_json::json!({"sequence":12,"timestamp":"2026-08-02T10:00:11Z","kind":"dispatch-completion","node":"m8-s2","payload":{"issuance_sequence":11,"duration_ms":1,"usage":{"availability":"absent","reason":"synthetic"},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}));
            let attempt = attempt_mut(manifest, "m8-route/purpose/codex-diagnostics");
            attempt["completion_sequence"] = Value::from(12);
            attempt["duration_ms"] = Value::from(1);
            attempt["child_exit"] = serde_json::json!({"kind":"exited","code":0});
            attempt["usage"] = serde_json::json!({"availability":"absent","reason":"synthetic"});
        }
        "L03" => add_second_retry(manifest, events, true),
        "L04" => manifest["aborts"].as_array_mut().expect("aborts array").clear(),
        "L05" => manifest["aborts"][0]["observed_cause"] = Value::String(String::new()),
        "L06" => manifest["aborts"].as_array_mut().expect("aborts array").push(serde_json::json!({"attempt_id":"unknown","evidence":"m8-route/retry/9/step-critic","route":"gate","issuance_sequence":99,"stage":"child-lost-before-completion","observed_cause":"synthetic","retry_disposition":"none"})),
        "L07" => {
            let abort = &mut manifest["aborts"][0];
            abort["attempt_id"] = Value::String("a1".to_owned());
            abort["evidence"] = Value::String("m8-route/registry/milestone-planner".to_owned());
            abort["issuance_sequence"] = Value::from(1);
        }
        "L08" => {
            let duplicate = manifest["aborts"][0].clone();
            manifest["aborts"].as_array_mut().expect("aborts array").push(duplicate);
        }
        "L09" => attempt_mut(manifest, "m8-route/registry/milestone-planner")["child_exit"] = Value::Null,
        "L10" => attempt_mut(manifest, "m8-route/purpose/codex-diagnostics")["child_exit"] = serde_json::json!({"kind":"signaled","signal":9}),
        "L11" => attempt_mut(manifest, "m8-route/registry/pr-reviewer")["disposition"] = Value::String("successful".to_owned()),
        "L12" => manifest["aborts"][0]["retry_disposition"] = Value::String(String::new()),
        "L13" => manifest["aborts"].as_array_mut().expect("aborts array").push(serde_json::json!({"attempt_id":"a6","evidence":"m8-route/purpose/codex-diagnostics","route":"codex","issuance_sequence":11,"stage":"child-lost-before-completion","observed_cause":"synthetic","retry_disposition":"none"})),
        "L14" => manifest["aborts"][0]["stage"] = Value::String("unknown-stage".to_owned()),
        "M01A" | "M01B" | "M02A" | "M02B" => {
            let (evidence, sequence) = match cycle {
                "M01A" => ("m8-route/registry/milestone-planner", 1),
                "M01B" => ("m8-route/registry/step-planner", 3),
                "M02A" => ("m8-route/registry/repository-analyst", 5),
                "M02B" => ("m8-route/registry/milestone-critic", 7),
                _ => unreachable!(),
            };
            let absent = serde_json::json!({"availability":"absent","reason":"no-terminal-turn"});
            attempt_mut(manifest, evidence)["usage"] = absent.clone();
            completion_mut(events, sequence)["payload"]["usage"] = absent;
        }
        "M03" | "M04" => {
            let (a, b, a_sequence, b_sequence) = if cycle == "M03" {
                ("m8-route/registry/milestone-planner", "m8-route/registry/step-planner", 1, 3)
            } else {
                ("m8-route/registry/repository-analyst", "m8-route/registry/milestone-critic", 5, 7)
            };
            let usage = attempt_mut(manifest, a)["usage"].clone();
            attempt_mut(manifest, b)["usage"] = usage.clone();
            let event_usage = completion_mut(events, a_sequence)["payload"]["usage"].clone();
            completion_mut(events, b_sequence)["payload"]["usage"] = event_usage;
        }
        "M05" => attempt_mut(manifest, "m8-route/registry/milestone-planner")["duration_ms"] = Value::from(999),
        "M06" => attempt_mut(manifest, "m8-route/registry/pr-reviewer")["artifact_present"] = Value::Bool(true),
        "M07" => attempt_mut(manifest, "m8-route/registry/pr-reviewer")["parent_exit"] = Value::from(0),
        "M08" => attempt_mut(manifest, "m8-route/registry/pr-reviewer")["validation_classification"] = Value::String("valid".to_owned()),
        "M09" => {}
        "M10" => manifest["hashes"]["verification_output"]["sha256"] = Value::String("0".repeat(64)),
        value if matches!(value, "P01" | "P02" | "P03" | "P04" | "P05") => {
            let evidence = match value {
                "P01" => "m8-route/registry/step-plan-writer",
                "P02" => "m8-route/registry/step-executor",
                "P03" => "m8-route/purpose/codex-commit-completion",
                "P04" => "m8-route/registry/step-critic",
                "P05" => "m8-route/registry/step-plan-critic",
                _ => unreachable!(),
            };
            projection_mut(projection, evidence)["normalized"]["child_argv"][0] = Value::String("altered".to_owned());
        }
        "P06" => manifest["projection_log"]["after_sha256"] = Value::String("altered".to_owned()),
        "P07" => manifest["snapshots"]["primary_tracked"]["after"]["src/main.rs"] = Value::String("altered".to_owned()),
        "P08" => manifest["snapshots"]["live_planning"]["after"]["milestone-8/steps.json"] = Value::String("altered".to_owned()),
        "P09" => attempt_mut(manifest, "m8-route/registry/repository-analyst")["child_environment_observation"]["observed_names"] = serde_json::json!(["HOME"]),
        "P10" => attempt_mut(manifest, "m8-route/registry/repository-analyst")["child_environment_observation"]["observed_names"] = serde_json::json!(["PATH"]),
        "P11" => attempt_mut(manifest, "m8-route/registry/repository-analyst")["child_environment_observation"]["observed_names"] = serde_json::json!(["PATH","HOME","ANTHROPIC_API_KEY"]),
        "P12" => manifest["auth_controls"][0]["parent_prefix_present"] = Value::Bool(false),
        "P13" => manifest["snapshots"]["primary_untracked"]["post_exclusion_after"] = serde_json::json!(["escape.txt"]),
        "P14" => manifest["capability_controls"][0]["issued_disallowed_tools"] = serde_json::json!(["Bash","WebFetch"]),
        "P15" => projection["projections"][0]["normalized"]["parent_argv"][0] = Value::String("altered".to_owned()),
        "P16" => projection["projections"][0]["normalized"]["cwd"] = Value::String("<OTHER>".to_owned()),
        "P17" => projection["projections"][0]["normalized"]["environment"]["PATH"] = Value::String("<OTHER>".to_owned()),
        "P18" => projection["projections"][0]["normalized"]["stdin_binding"]["byte_count"] = Value::from(99),
        "P19" => projection["projections"][1]["normalized"]["schema_path"] = Value::String("<OTHER>".to_owned()),
        "P20" => projection["projections"][1]["normalized"]["output_path"] = Value::String("<OTHER>".to_owned()),
        "P21" => manifest["pid_observations"][0]["direct_child_pid"] = Value::Null,
        _ => panic!("unknown mutation {cycle}"),
    }
}

#[test]
fn m8_s2_copy_falsification_cycle() {
    let Some(root) = std::env::var_os("M8_S2_FIXTURE_COPY_ROOT") else {
        return;
    };
    let cycle = std::env::var("M8_S2_CYCLE").expect("M8_S2_CYCLE must name one cycle");
    let root = Path::new(&root);
    let manifest_path = root.join("provenance-manifest.json");
    let projection_path = root.join("dry-run-projection.json");
    let events_path = root.join("events.jsonl");
    let meter_path = root.join("meter-report.json");
    let mut manifest = parse_json_file(&manifest_path).expect("baseline manifest should parse");
    let mut projection =
        parse_json_file(&projection_path).expect("baseline projection should parse");
    let mut events = event_values(&fs::read(&events_path).expect("baseline events should read"))
        .expect("baseline events should parse");
    let green_retry = matches!(
        cycle.as_str(),
        "I04" | "I05" | "I06" | "L03" | "L04" | "L05" | "L07" | "L08" | "L12" | "L14"
    );
    if green_retry {
        add_green_retry(&mut manifest, &mut events);
        write_json(&manifest_path, &manifest);
        write_events(&events_path, &events);
        let output = run_meter(
            &["log", "meter"],
            &fs::read(&events_path).expect("green events should read"),
        );
        assert!(
            output.status.success(),
            "green retry meter failed: {}",
            complete_measurement(&output)
        );
        fs::write(&meter_path, output.stdout).expect("green retry report should write");
    }
    let green = verify_m8_fixture(root, false);
    assert!(
        green.is_ok(),
        "cycle/{cycle}/green-precondition: {}",
        green.expect_err("green result should have an error")
    );
    let baseline: Vec<_> = M8_FILES
        .iter()
        .map(|name| {
            (
                (*name).to_owned(),
                fs::read(root.join(name)).expect("baseline file should read"),
            )
        })
        .collect();
    mutate_cycle(&cycle, &mut manifest, &mut projection, &mut events);
    write_json(&manifest_path, &manifest);
    write_json(&projection_path, &projection);
    write_events(&events_path, &events);
    if cycle == "M09" {
        let mut bytes = fs::read(&meter_path).expect("meter report should read");
        bytes[0] ^= 1;
        fs::write(&meter_path, bytes).expect("meter report mutation should write");
    }
    let observed = verify_m8_fixture(root, false).expect_err("mutated fixture must be rejected");
    let expected = cycle_expected_site(&cycle);
    for (name, bytes) in &baseline {
        fs::write(root.join(name), bytes).expect("baseline restoration should write");
    }
    let restored = verify_m8_fixture(root, false);
    assert!(
        restored.is_ok(),
        "cycle/{cycle}/restored-green: {}",
        restored.expect_err("restored result should have an error")
    );
    let restored_identity = baseline
        .iter()
        .all(|(name, bytes)| fs::read(root.join(name)).is_ok_and(|restored| restored == *bytes));
    assert!(restored_identity, "cycle/{cycle}/copy-byte-identity");
    let ledger = serde_json::json!({
        "identity": cycle,
        "venue": "Harness/fixture self-test, no production falsification weight",
        "expected_site": expected,
        "actual_site": observed.site,
        "observed": observed.observed,
        "green_before": true,
        "restored_green": true,
        "copy_byte_identity": true,
        "meter_report_last": matches!(cycle.as_str(), "I01"|"I02"|"I03"|"B01"|"B02"|"B03"|"B04"|"B09"|"B10"|"L02"|"L03"|"M01A"|"M01B"|"M02A"|"M02B"|"M03"|"M04")
    });
    let ledger_path = root.join("ledger.jsonl");
    let mut ledger_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ledger_path)
        .expect("ledger should open");
    writeln!(
        ledger_file,
        "{}",
        serde_json::to_string(&ledger).expect("ledger should serialize")
    )
    .expect("ledger should append");
    assert_eq!(
        observed.site, expected,
        "cycle/{cycle}/named-assertion-pre-emption: observed={observed}"
    );
    panic!("falsification-red/{cycle}: {observed}");
}

const DISPATCH: &str = r#"{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert codex shim measurement"}}"#;
const EXPECTED_REPORT: &str = r#"{"issuance":{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","node":"m8-s1","role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert codex shim measurement"},"completion":null}
"#;
const DISPATCH_TWO: &str = r#"{"sequence":2,"timestamp":"2026-08-02T12:42:43.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"second issuance"}}"#;
const COMPLETION_ONE: &str = r#"{"sequence":3,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":128,"usage":{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const COMPLETION_TWO: &str = r#"{"sequence":4,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":2,"duration_ms":64,"usage":{"availability":"measured","input_tokens":211,"cached_input_tokens":43,"output_tokens":31,"reasoning_output_tokens":11},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const CLAUDE_DISPATCH_ONE: &str = r#"{"sequence":5,"timestamp":"2026-08-02T12:42:46.273Z","kind":"dispatch","node":"m8-s3","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"first claude usage fixture"}}"#;
const CLAUDE_COMPLETION_ONE: &str = r#"{"sequence":6,"timestamp":"2026-08-02T12:42:47.273Z","kind":"dispatch-completion","node":"m8-s3","payload":{"issuance_sequence":5,"duration_ms":9,"usage":{"availability":"claude-measured","input_tokens":41,"output_tokens":47,"cache_creation_input_tokens":53,"cache_read_input_tokens":59},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;
const CLAUDE_DISPATCH_TWO: &str = r#"{"sequence":7,"timestamp":"2026-08-02T12:42:48.273Z","kind":"dispatch","node":"m8-s3","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"second claude usage fixture"}}"#;
const CLAUDE_COMPLETION_TWO: &str = r#"{"sequence":8,"timestamp":"2026-08-02T12:42:49.273Z","kind":"dispatch-completion","node":"m8-s3","payload":{"issuance_sequence":7,"duration_ms":10,"usage":{"availability":"claude-measured","input_tokens":61,"output_tokens":67,"cache_creation_input_tokens":71,"cache_read_input_tokens":73},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;
const DISTINCT_DISPATCH: &str = r#"{"sequence":11,"timestamp":"2026-08-02T13:42:42.273Z","kind":"dispatch","node":"m8-s2","payload":{"role":"milestone-executor","ref":"0123456789abcdef0123456789abcdef01234567","evidence":"independent issuance measurement"}}"#;
const DISTINCT_COMPLETION: &str = r#"{"sequence":12,"timestamp":"2026-08-02T13:42:43.273Z","kind":"dispatch-completion","node":"m8-s2","payload":{"issuance_sequence":11,"duration_ms":7,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"signaled","signal":15},"artifact_outcome":"missing"}}"#;
const MEASURED_CODEX_A: &str = r#"{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert codex shim measurement"}}
{"sequence":2,"timestamp":"2026-08-02T12:42:42.411Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":128,"usage":{"availability":"measured","input_tokens":101,"cached_input_tokens":23,"output_tokens":17,"reasoning_output_tokens":5},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const MEASURED_CLAUDE_A: &str = r#"{"sequence":3,"timestamp":"2026-08-02T12:42:43.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i inert claude shim measurement"}}
{"sequence":4,"timestamp":"2026-08-02T12:42:43.399Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":3,"duration_ms":126,"usage":{"availability":"claude-measured","input_tokens":11,"output_tokens":13,"cache_creation_input_tokens":17,"cache_read_input_tokens":19},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;
const MEASURED_CODEX_B: &str = r#"{"sequence":5,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i second inert codex shim measurement"}}
{"sequence":6,"timestamp":"2026-08-02T12:42:44.278Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":5,"duration_ms":5,"usage":{"availability":"measured","input_tokens":127,"cached_input_tokens":29,"output_tokens":19,"reasoning_output_tokens":7},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"not-validated"}}"#;
const MEASURED_CLAUDE_B: &str = r#"{"sequence":7,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-critic","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"env -i second inert claude shim measurement"}}
{"sequence":8,"timestamp":"2026-08-02T12:42:45.279Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":7,"duration_ms":6,"usage":{"availability":"claude-measured","input_tokens":23,"output_tokens":29,"cache_creation_input_tokens":31,"cache_read_input_tokens":37},"exit_status":{"kind":"exited","code":0},"artifact_outcome":"missing"}}"#;

#[derive(Deserialize)]
struct MeasuredEvidenceReport {
    issuance: MeasuredEvidenceIssuance,
    completion: Option<MeasuredEvidenceCompletion>,
}

#[derive(Deserialize)]
struct MeasuredEvidenceIssuance {
    sequence: u64,
}

#[derive(Deserialize)]
struct MeasuredEvidenceCompletion {
    usage: MeasuredEvidenceUsage,
}

#[derive(Deserialize)]
#[serde(tag = "availability", rename_all = "kebab-case")]
enum MeasuredEvidenceUsage {
    Measured {
        input_tokens: u64,
        cached_input_tokens: u64,
        output_tokens: u64,
        reasoning_output_tokens: u64,
    },
    ClaudeMeasured {
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_input_tokens: u64,
        cache_read_input_tokens: u64,
    },
    Absent {
        reason: String,
    },
}

impl MeasuredEvidenceUsage {
    fn observation(&self, issuance_sequence: u64) -> (u64, &'static str, Option<&str>) {
        match self {
            Self::Measured { .. } => (issuance_sequence, "measured", None),
            Self::ClaudeMeasured { .. } => (issuance_sequence, "claude-measured", None),
            Self::Absent { reason } => (issuance_sequence, "absent", Some(reason)),
        }
    }
}

fn run_meter(arguments: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("pce meter process should spawn");
    child
        .stdin
        .take()
        .expect("pce meter stdin should be piped")
        .write_all(stdin)
        .expect("meter fixture should write");
    child.wait_with_output().expect("pce meter should exit")
}

fn run_command(mut command: Command, stdin: &[u8]) -> Output {
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("measured process should spawn");
    child
        .stdin
        .take()
        .expect("measured process stdin should be piped")
        .write_all(stdin)
        .expect("measured process fixture should write");
    child
        .wait_with_output()
        .expect("measured process should exit")
}

fn strict_seatbelt_profile(pce_executable: &Path) -> String {
    let executable_literal = pce_executable
        .to_str()
        .expect("canonical pce executable path should be UTF-8")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        "(version 1)\n\
(deny default)\n\
(import \"system.sb\")\n\
(allow process*)\n\
(allow file-read*\n\
  (literal \"{executable_literal}\")\n\
  (subpath \"/usr\")\n\
  (subpath \"/System\")\n\
  (subpath \"/bin\")\n\
  (subpath \"/private/var/db/dyld\")\n\
  (subpath \"/Library/Apple\"))\n"
    )
}

fn run_cat(path: &Path) -> Output {
    let mut command = Command::new("/bin/cat");
    command.arg(path);
    run_command(command, b"")
}

fn run_strict_cat(profile: &str, path: &Path) -> Output {
    let mut command = Command::new("/usr/bin/sandbox-exec");
    command.args(["-p", profile, "/bin/cat"]).arg(path);
    run_command(command, b"")
}

fn run_pce(executable: &Path, profile: Option<&str>, stdin: &[u8]) -> Output {
    let command = match profile {
        Some(profile) => {
            let mut command = Command::new("/usr/bin/sandbox-exec");
            command
                .args(["-p", profile])
                .arg(executable)
                .args(["log", "meter"]);
            command
        }
        None => {
            let mut command = Command::new(executable);
            command.args(["log", "meter"]);
            command
        }
    };
    run_command(command, stdin)
}

fn complete_measurement(output: &Output) -> String {
    format!(
        "status={:?}; stdout={:?}; stderr={:?}",
        output.status.code(),
        output.stdout,
        output.stderr
    )
}

fn jsonl(lines: &[&str]) -> Vec<u8> {
    lines.join("\n").into_bytes()
}

fn report_values(output: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("meter report line should be JSON"))
        .collect()
}

fn assert_issuance_field(
    field_label: &str,
    source_records: &[serde_json::Value],
    reports: &[serde_json::Value],
    envelope_field: Option<&str>,
    report_field: &str,
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_records
        .iter()
        .map(|record| match envelope_field {
            Some(field) => &record[field],
            None => &record["payload"][report_field],
        })
        .collect();
    let observed: Vec<&serde_json::Value> = reports
        .iter()
        .map(|report| &report["issuance"][report_field])
        .collect();
    let expected_present = expected.iter().filter(|value| !value.is_null()).count();
    let observed_present = reports
        .iter()
        .filter(|report| {
            report["issuance"]
                .as_object()
                .is_some_and(|issuance| issuance.contains_key(report_field))
        })
        .count();
    assert_eq!(
        observed_present,
        expected_present,
        "meter_report_complete/{field_label}-omitted: expected measured presence count {expected_present} with values {expected:?}; observed presence count {observed_present} with complete values {observed:?}; {}",
        complete_measurement(output)
    );
    assert_eq!(
        observed,
        expected,
        "meter_report_complete/{field_label}-altered: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn assert_completion_field(
    field_label: &str,
    alteration_label: &str,
    source_completions: &[&serde_json::Value],
    report_completions: &[&serde_json::Value],
    envelope_field: Option<&str>,
    report_field: &str,
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_completions
        .iter()
        .map(|record| match envelope_field {
            Some(field) => &record[field],
            None => &record["payload"][report_field],
        })
        .collect();
    let observed: Vec<&serde_json::Value> = report_completions
        .iter()
        .map(|completion| &completion[report_field])
        .collect();
    let expected_present = expected.iter().filter(|value| !value.is_null()).count();
    let observed_present = report_completions
        .iter()
        .filter(|completion| {
            completion
                .as_object()
                .is_some_and(|completion| completion.contains_key(report_field))
        })
        .count();
    assert_eq!(
        observed_present,
        expected_present,
        "meter_report_complete/{field_label}-omitted: expected measured presence count {expected_present} with values {expected:?}; observed presence count {observed_present} with complete values {observed:?}; {}",
        complete_measurement(output)
    );
    assert_eq!(
        observed,
        expected,
        "meter_report_complete/{alteration_label}: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn assert_completion_value(
    label: &str,
    source_completions: &[&serde_json::Value],
    report_completions: &[&serde_json::Value],
    report_field: &str,
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_completions
        .iter()
        .map(|record| &record["payload"][report_field])
        .collect();
    let observed: Vec<&serde_json::Value> = report_completions
        .iter()
        .map(|completion| &completion[report_field])
        .collect();
    assert_eq!(
        observed,
        expected,
        "meter_report_complete/{label}: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn assert_usage_field(
    label: &str,
    availability: &str,
    field: &str,
    source_usage: &[&serde_json::Value],
    report_usage: &[&serde_json::Value],
    output: &Output,
) {
    let expected: Vec<&serde_json::Value> = source_usage
        .iter()
        .filter(|usage| usage["availability"] == availability)
        .map(|usage| &usage[field])
        .collect();
    let observed: Vec<&serde_json::Value> = report_usage
        .iter()
        .filter(|usage| usage["availability"] == availability)
        .map(|usage| &usage[field])
        .collect();
    assert_eq!(
        observed,
        expected,
        "meter_usage_complete/{label}: expected measured values {expected:?}; observed complete values {observed:?}; {}",
        complete_measurement(output)
    );
}

fn usage_case(usage: serde_json::Value) -> Vec<u8> {
    let issuance = serde_json::json!({
        "sequence": 1,
        "timestamp": "2026-08-02T12:42:42.273Z",
        "kind": "dispatch",
        "node": "m8-s1",
        "payload": {
            "role": "step-executor",
            "ref": "cf94db2ba021ea176b9b00099fa34855976dd563",
            "evidence": "strict usage fixture"
        }
    });
    let completion = serde_json::json!({
        "sequence": 2,
        "timestamp": "2026-08-02T12:42:43.273Z",
        "kind": "dispatch-completion",
        "node": "m8-s1",
        "payload": {
            "issuance_sequence": 1,
            "duration_ms": 1,
            "usage": usage,
            "exit_status": {"kind": "exited", "code": 0},
            "artifact_outcome": "not-validated"
        }
    });
    format!("{issuance}\n{completion}").into_bytes()
}

fn assert_unknown_usage_rejected(label: &str, unknown_key: &str, mut usage: serde_json::Value) {
    usage
        .as_object_mut()
        .expect("usage fixture should be an object")
        .insert(unknown_key.to_owned(), serde_json::json!("unexpected"));
    let output = run_meter(&["log", "meter"], &usage_case(usage));
    assert!(
        !output.status.success(),
        "meter_usage_denies_unknown/{label}: expected unknown key {unknown_key:?} to be rejected; {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        b"",
        "meter_usage_denies_unknown/{label}: expected zero stdout for unknown key {unknown_key:?}; {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_cli_accepts_exact_stdin_only_surface() {
    let output = run_meter(&["log", "meter"], b"");
    assert!(
        output.status.success(),
        "meter_cli_accepts_exact_stdin_only_surface/status: {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        b"",
        "meter_cli_accepts_exact_stdin_only_surface/stdout: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_distinguishes_reconciled_dead_without_fabricated_measurements() {
    let input = b"{\"sequence\":1,\"timestamp\":\"2026-08-09T12:00:00.000Z\",\"kind\":\"dispatch\",\"node\":\"m1-s2\",\"payload\":{\"role\":\"step-executor\",\"ref\":\"abc123\",\"evidence\":\"fixture\"}}\n{\"sequence\":2,\"timestamp\":\"2026-08-09T12:00:01.000Z\",\"kind\":\"dispatch-completion\",\"node\":\"m1-s2\",\"payload\":{\"issuance_sequence\":1,\"outcome\":\"reconciled-dead\",\"artifact_production\":\"not-produced\"}}\n";
    let output = run_meter(&["log", "meter"], input);
    assert!(output.status.success(), "{}", complete_measurement(&output));
    assert_eq!(output.stderr, b"");
    assert_eq!(output.stdout, b"{\"issuance\":{\"sequence\":1,\"timestamp\":\"2026-08-09T12:00:00.000Z\",\"node\":\"m1-s2\",\"role\":\"step-executor\",\"ref\":\"abc123\",\"evidence\":\"fixture\"},\"completion\":{\"sequence\":2,\"timestamp\":\"2026-08-09T12:00:01.000Z\",\"outcome\":\"reconciled-dead\",\"artifact_production\":\"not-produced\"}}\n");
}

#[test]
fn meter_cli_rejects_all_arguments() {
    let cases: &[(&str, &[&str])] = &[
        ("--file", &["log", "meter", "--file", "poison"]),
        ("--kind", &["log", "meter", "--kind", "dispatch"]),
        ("--node", &["log", "meter", "--node", "m8-s1"]),
        ("path", &["log", "meter", "events.jsonl"]),
        ("dash", &["log", "meter", "-"]),
    ];
    for (label, arguments) in cases {
        let output = run_meter(arguments, b"");
        assert!(
            !output.status.success(),
            "meter_cli_rejects_all_arguments/{label}/status: {}",
            complete_measurement(&output)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("usage: pce"),
            "meter_cli_rejects_all_arguments/{label}/stderr: {}",
            complete_measurement(&output)
        );
        assert_eq!(
            output.stdout,
            b"",
            "meter_cli_rejects_all_arguments/{label}/stdout: {}",
            complete_measurement(&output)
        );
    }
}

#[test]
fn meter_cli_consumes_stdin() {
    let output = run_meter(&["log", "meter"], DISPATCH.as_bytes());
    assert!(
        output.status.success(),
        "meter_cli_consumes_stdin/status: {}",
        complete_measurement(&output)
    );
    let report_count = output.stdout.split(|byte| *byte == b'\n').count() - 1;
    assert_eq!(
        report_count,
        1,
        "meter_cli_consumes_stdin/report-count: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_output_is_exact_jsonl() {
    let output = run_meter(&["log", "meter"], DISPATCH.as_bytes());
    assert!(
        output.status.success(),
        "meter_output_is_exact_jsonl/status: {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        EXPECTED_REPORT.as_bytes(),
        "meter_output_is_exact_jsonl/bytes: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_retains_uncompleted_issuance() {
    let output = run_meter(&["log", "meter"], DISPATCH.as_bytes());
    assert!(
        output.status.success(),
        "meter_retains_uncompleted_issuance/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    assert_eq!(
        reports.len(),
        1,
        "meter_retains_uncompleted_issuance: {}",
        complete_measurement(&output)
    );
    assert_eq!(
        reports[0]["completion"],
        serde_json::Value::Null,
        "meter_retains_uncompleted_issuance: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_preserves_issuance_order() {
    let input = jsonl(&[DISPATCH, DISPATCH_TWO]);
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_preserves_issuance_order/status: {}",
        complete_measurement(&output)
    );
    let observed: Vec<u64> = report_values(&output)
        .iter()
        .map(|report| report["issuance"]["sequence"].as_u64().expect("sequence"))
        .collect();
    assert_eq!(
        observed,
        vec![1, 2],
        "meter_preserves_issuance_order: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_correlates_by_issuance_sequence() {
    let input = jsonl(&[DISPATCH, DISPATCH_TWO, COMPLETION_ONE]);
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_correlates_by_issuance_sequence/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    let observed: Vec<(u64, Option<u64>)> = reports
        .iter()
        .map(|report| {
            (
                report["issuance"]["sequence"].as_u64().expect("sequence"),
                report["completion"]["sequence"].as_u64(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        vec![(1, Some(3)), (2, None)],
        "meter_correlates_by_issuance_sequence: {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_report_complete() {
    let input = jsonl(&[
        DISPATCH,
        COMPLETION_ONE,
        DISTINCT_DISPATCH,
        DISTINCT_COMPLETION,
    ]);
    let source_records: Vec<serde_json::Value> = input
        .split(|byte| *byte == b'\n')
        .map(|line| serde_json::from_slice(line).expect("source event should be JSON"))
        .collect();
    let source_issuances: Vec<serde_json::Value> = source_records
        .iter()
        .filter(|record| record["kind"] == "dispatch")
        .cloned()
        .collect();
    let source_completions: Vec<&serde_json::Value> = source_records
        .iter()
        .filter(|record| record["kind"] == "dispatch-completion")
        .collect();
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_report_complete/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    assert_eq!(
        reports.len(),
        source_issuances.len(),
        "meter_report_complete/report-count: source measurements {source_issuances:?}; observed reports {reports:?}; {}",
        complete_measurement(&output)
    );
    let report_completions: Vec<&serde_json::Value> = reports
        .iter()
        .filter_map(|report| {
            report["completion"]
                .as_object()
                .map(|_| &report["completion"])
        })
        .collect();
    assert_eq!(
        report_completions.len(),
        source_completions.len(),
        "meter_report_complete/completion-count: source measurements {source_completions:?}; observed completions {report_completions:?}; {}",
        complete_measurement(&output)
    );

    assert_issuance_field(
        "issuance-sequence",
        &source_issuances,
        &reports,
        Some("sequence"),
        "sequence",
        &output,
    );
    assert_issuance_field(
        "issuance-timestamp",
        &source_issuances,
        &reports,
        Some("timestamp"),
        "timestamp",
        &output,
    );
    assert_issuance_field(
        "issuance-node",
        &source_issuances,
        &reports,
        Some("node"),
        "node",
        &output,
    );
    assert_issuance_field(
        "issuance-role",
        &source_issuances,
        &reports,
        None,
        "role",
        &output,
    );
    assert_issuance_field(
        "issuance-ref",
        &source_issuances,
        &reports,
        None,
        "ref",
        &output,
    );
    assert_issuance_field(
        "issuance-evidence",
        &source_issuances,
        &reports,
        None,
        "evidence",
        &output,
    );

    assert_completion_field(
        "completion-sequence",
        "completion-sequence-altered",
        &source_completions,
        &report_completions,
        Some("sequence"),
        "sequence",
        &output,
    );
    assert_completion_field(
        "completion-timestamp",
        "completion-timestamp-altered",
        &source_completions,
        &report_completions,
        Some("timestamp"),
        "timestamp",
        &output,
    );
    assert_completion_field(
        "duration-ms",
        "duration-ms-zeroed",
        &source_completions,
        &report_completions,
        None,
        "duration_ms",
        &output,
    );
    assert_completion_value(
        "exit-status",
        &source_completions,
        &report_completions,
        "exit_status",
        &output,
    );
    assert_completion_value(
        "artifact-outcome",
        &source_completions,
        &report_completions,
        "artifact_outcome",
        &output,
    );
}

#[test]
fn meter_usage_complete() {
    let input = jsonl(&[
        DISPATCH,
        DISPATCH_TWO,
        COMPLETION_ONE,
        COMPLETION_TWO,
        CLAUDE_DISPATCH_ONE,
        CLAUDE_COMPLETION_ONE,
        CLAUDE_DISPATCH_TWO,
        CLAUDE_COMPLETION_TWO,
        DISTINCT_DISPATCH,
        DISTINCT_COMPLETION,
    ]);
    let source_records: Vec<serde_json::Value> = input
        .split(|byte| *byte == b'\n')
        .map(|line| serde_json::from_slice(line).expect("source event should be JSON"))
        .collect();
    let source_usage: Vec<&serde_json::Value> = source_records
        .iter()
        .filter(|record| record["kind"] == "dispatch-completion")
        .map(|record| &record["payload"]["usage"])
        .collect();
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "meter_usage_complete/status: {}",
        complete_measurement(&output)
    );
    let reports = report_values(&output);
    let report_usage: Vec<&serde_json::Value> = reports
        .iter()
        .filter_map(|report| {
            report["completion"]
                .as_object()
                .map(|_| &report["completion"]["usage"])
        })
        .collect();

    let expected_availability: Vec<&serde_json::Value> = source_usage
        .iter()
        .map(|usage| &usage["availability"])
        .collect();
    let observed_availability: Vec<&serde_json::Value> = report_usage
        .iter()
        .map(|usage| &usage["availability"])
        .collect();
    assert_eq!(
        observed_availability,
        expected_availability,
        "meter_usage_complete/absent-not-zero-filled: expected measured availability values {expected_availability:?}; observed complete availability values {observed_availability:?}; {}",
        complete_measurement(&output)
    );

    assert_usage_field(
        "codex-input",
        "measured",
        "input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "codex-cached-input",
        "measured",
        "cached_input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "codex-output",
        "measured",
        "output_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "codex-reasoning-output",
        "measured",
        "reasoning_output_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-input",
        "claude-measured",
        "input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-output",
        "claude-measured",
        "output_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-cache-creation",
        "claude-measured",
        "cache_creation_input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );
    assert_usage_field(
        "claude-cache-read",
        "claude-measured",
        "cache_read_input_tokens",
        &source_usage,
        &report_usage,
        &output,
    );

    let expected_absent_reasons: Vec<&serde_json::Value> = source_usage
        .iter()
        .filter(|usage| usage["availability"] == "absent")
        .map(|usage| &usage["reason"])
        .collect();
    let observed_absent_reasons: Vec<&serde_json::Value> = report_usage
        .iter()
        .filter(|usage| usage["availability"] == "absent")
        .map(|usage| &usage["reason"])
        .collect();
    let expected_reason_presence = expected_absent_reasons
        .iter()
        .filter(|reason| !reason.is_null())
        .count();
    let observed_reason_presence = report_usage
        .iter()
        .filter(|usage| {
            usage["availability"] == "absent"
                && usage
                    .as_object()
                    .is_some_and(|usage| usage.contains_key("reason"))
        })
        .count();
    assert_eq!(
        observed_reason_presence,
        expected_reason_presence,
        "meter_usage_complete/absent-reason-preserved: expected measured presence count {expected_reason_presence} with values {expected_absent_reasons:?}; observed presence count {observed_reason_presence} with complete values {observed_absent_reasons:?}; {}",
        complete_measurement(&output)
    );
    assert_eq!(
        observed_absent_reasons,
        expected_absent_reasons,
        "meter_usage_complete/absent-reason-preserved: expected measured values {expected_absent_reasons:?}; observed complete values {observed_absent_reasons:?}; {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_usage_denies_unknown() {
    assert_unknown_usage_rejected(
        "measured",
        "measured_extra",
        serde_json::json!({
            "availability": "measured",
            "input_tokens": 101,
            "cached_input_tokens": 23,
            "output_tokens": 17,
            "reasoning_output_tokens": 5
        }),
    );
    assert_unknown_usage_rejected(
        "claude-measured",
        "claude_measured_extra",
        serde_json::json!({
            "availability": "claude-measured",
            "input_tokens": 11,
            "output_tokens": 13,
            "cache_creation_input_tokens": 17,
            "cache_read_input_tokens": 19
        }),
    );
    assert_unknown_usage_rejected(
        "absent",
        "absent_extra",
        serde_json::json!({
            "availability": "absent",
            "reason": "no-terminal-turn"
        }),
    );
}

#[test]
fn measured_evidence_is_recoverable_from_event_log_alone() {
    let input = jsonl(&[
        MEASURED_CODEX_A,
        MEASURED_CLAUDE_A,
        MEASURED_CODEX_B,
        MEASURED_CLAUDE_B,
    ]);
    let output = run_meter(&["log", "meter"], &input);
    assert!(
        output.status.success(),
        "measured_evidence/status: {}",
        complete_measurement(&output)
    );
    let reports: Vec<MeasuredEvidenceReport> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            serde_json::from_str(line).expect("measured evidence report should deserialize")
        })
        .collect();
    let controlled: Vec<(u64, &MeasuredEvidenceUsage)> = reports
        .iter()
        .filter(|report| matches!(report.issuance.sequence, 1 | 3 | 5 | 7))
        .filter_map(|report| {
            report
                .completion
                .as_ref()
                .map(|completion| (report.issuance.sequence, &completion.usage))
        })
        .collect();
    let codex: Vec<(u64, &MeasuredEvidenceUsage)> = controlled
        .iter()
        .filter(|(sequence, _)| matches!(sequence, 1 | 5))
        .copied()
        .collect();
    let claude: Vec<(u64, &MeasuredEvidenceUsage)> = controlled
        .iter()
        .filter(|(sequence, _)| matches!(sequence, 3 | 7))
        .copied()
        .collect();

    let codex_availability: Vec<(u64, &str, Option<&str>)> = codex
        .iter()
        .map(|(sequence, usage)| usage.observation(*sequence))
        .collect();
    let measured_count = controlled
        .iter()
        .filter(|(_, usage)| matches!(usage, MeasuredEvidenceUsage::Measured { .. }))
        .count();
    assert_eq!(
        (codex_availability.as_slice(), measured_count),
        ([(1, "measured", None), (5, "measured", None)].as_slice(), 2),
        "measured_evidence/codex-availability: expected controlled availability [(1, measured, None), (5, measured, None)] and exactly two measured completions; observed complete availability {codex_availability:?} and measured count {measured_count}; {}",
        complete_measurement(&output)
    );

    let claude_availability: Vec<(u64, &str, Option<&str>)> = claude
        .iter()
        .map(|(sequence, usage)| usage.observation(*sequence))
        .collect();
    let claude_measured_count = controlled
        .iter()
        .filter(|(_, usage)| matches!(usage, MeasuredEvidenceUsage::ClaudeMeasured { .. }))
        .count();
    assert_eq!(
        (claude_availability.as_slice(), claude_measured_count),
        (
            [(3, "claude-measured", None), (7, "claude-measured", None),].as_slice(),
            2
        ),
        "measured_evidence/claude-availability: expected controlled availability [(3, claude-measured, None), (7, claude-measured, None)] and exactly two claude-measured completions; observed complete availability {claude_availability:?} and claude-measured count {claude_measured_count}; {}",
        complete_measurement(&output)
    );

    let codex_vectors: Vec<(u64, u64, u64, u64)> = codex
        .iter()
        .filter_map(|(_, usage)| match usage {
            MeasuredEvidenceUsage::Measured {
                input_tokens,
                cached_input_tokens,
                output_tokens,
                reasoning_output_tokens,
            } => Some((
                *input_tokens,
                *cached_input_tokens,
                *output_tokens,
                *reasoning_output_tokens,
            )),
            MeasuredEvidenceUsage::ClaudeMeasured { .. } | MeasuredEvidenceUsage::Absent { .. } => {
                None
            }
        })
        .collect();
    assert_ne!(
        codex_vectors[0],
        codex_vectors[1],
        "measured_evidence/codex-responsive: expected two different complete measured token vectors; observed Codex A {:?} and Codex B {:?}; {}",
        codex_vectors[0],
        codex_vectors[1],
        complete_measurement(&output)
    );

    let claude_vectors: Vec<(u64, u64, u64, u64)> = claude
        .iter()
        .filter_map(|(_, usage)| match usage {
            MeasuredEvidenceUsage::ClaudeMeasured {
                input_tokens,
                output_tokens,
                cache_creation_input_tokens,
                cache_read_input_tokens,
            } => Some((
                *input_tokens,
                *output_tokens,
                *cache_creation_input_tokens,
                *cache_read_input_tokens,
            )),
            MeasuredEvidenceUsage::Measured { .. } | MeasuredEvidenceUsage::Absent { .. } => None,
        })
        .collect();
    assert_ne!(
        claude_vectors[0],
        claude_vectors[1],
        "measured_evidence/claude-responsive: expected two different complete claude-measured token vectors; observed Claude A {:?} and Claude B {:?}; {}",
        claude_vectors[0],
        claude_vectors[1],
        complete_measurement(&output)
    );
}

fn assert_invalid_correlation(label: &str, input: &[u8], expected_diagnostic: &str) {
    let output = run_meter(&["log", "meter"], input);
    assert!(
        !output.status.success(),
        "meter_rejects_invalid_correlation/{label}: expected diagnostic {expected_diagnostic:?}; {}",
        complete_measurement(&output)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected_diagnostic),
        "meter_rejects_invalid_correlation/{label}: expected diagnostic {expected_diagnostic:?}; {}",
        complete_measurement(&output)
    );
    assert_eq!(
        output.stdout,
        b"",
        "meter_rejects_invalid_correlation/{label}: expected zero stdout; {}",
        complete_measurement(&output)
    );
}

#[test]
fn meter_rejects_invalid_correlation() {
    let missing = jsonl(&[
        DISPATCH,
        r#"{"sequence":3,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":2,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation("missing", &missing, "missing issuance sequence 2");

    let non_dispatch = jsonl(&[
        r#"{"sequence":1,"timestamp":"2026-08-02T12:42:42.273Z","kind":"delta","node":"m8-s1","payload":{"message":"not an issuance"}}"#,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation("non-dispatch", &non_dispatch, "non-dispatch sequence 1");

    let forward_own = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":2,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation(
        "forward-own",
        &forward_own,
        "completion 2 points forward to issuance 2",
    );

    let forward_later = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":3,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
        r#"{"sequence":3,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"later issuance"}}"#,
    ]);
    assert_invalid_correlation(
        "forward-later",
        &forward_later,
        "completion 2 points forward to issuance 3",
    );

    let duplicate = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
        r#"{"sequence":3,"timestamp":"2026-08-02T12:42:45.273Z","kind":"dispatch-completion","node":"m8-s1","payload":{"issuance_sequence":1,"duration_ms":2,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation(
        "duplicate",
        &duplicate,
        "duplicate completion 3 for issuance 1",
    );

    let node_mismatch = jsonl(&[
        DISPATCH,
        r#"{"sequence":2,"timestamp":"2026-08-02T12:42:44.273Z","kind":"dispatch-completion","node":"m8-s2","payload":{"issuance_sequence":1,"duration_ms":1,"usage":{"availability":"absent","reason":"no-terminal-turn"},"exit_status":{"kind":"exited","code":1},"artifact_outcome":"missing"}}"#,
    ]);
    assert_invalid_correlation(
        "node-mismatch",
        &node_mismatch,
        "node m8-s2 does not match issuance 1 node m8-s1",
    );
}

#[test]
fn strict_seatbelt_meter_excludes_transcripts() {
    if skip_without_nested_seatbelt() {
        return;
    }

    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = WorkspaceFixtureDirectory::create("strict-meter-transcripts")
        .expect("workspace poison fixture directory should be created");
    fixture.assert_outside_temporary_roots(repository);
    let poison_a = fixture.path().join("transcript-a.txt");
    let poison_b = fixture.path().join("transcript-b.txt");
    let sentinel_a = b"PCE_POISON_TRANSCRIPT_A_COMPLETE_SENTINEL\n";
    let sentinel_b = b"PCE_POISON_TRANSCRIPT_B_COMPLETE_SENTINEL\n";
    fs::write(&poison_a, sentinel_a).expect("poison transcript A should write");
    fs::write(&poison_b, sentinel_b).expect("poison transcript B should write");

    let pce_executable = fs::canonicalize(env!("CARGO_BIN_EXE_pce"))
        .expect("integration-test pce executable should canonicalize");
    let strict_profile = strict_seatbelt_profile(&pce_executable);
    let uncompleted_issuance = r#"{"sequence":9,"timestamp":"2026-08-02T12:42:46.273Z","kind":"dispatch","node":"m8-s1","payload":{"role":"step-executor","ref":"cf94db2ba021ea176b9b00099fa34855976dd563","evidence":"strict harness uncompleted issuance"}}"#;
    let event_log = jsonl(&[
        MEASURED_CODEX_A,
        MEASURED_CLAUDE_A,
        MEASURED_CODEX_B,
        MEASURED_CLAUDE_B,
        uncompleted_issuance,
    ]);
    let unsandboxed_meter = run_pce(&pce_executable, None, &event_log);
    assert!(
        unsandboxed_meter.status.success(),
        "strict_seatbelt_meter_excludes_transcripts/unsandboxed-meter-control: expected successful unconfined meter run; {}",
        complete_measurement(&unsandboxed_meter)
    );

    let outside_probe_a = run_cat(&poison_a);
    let outside_probe_b = run_cat(&poison_b);
    assert!(
        outside_probe_a.status.success()
            && outside_probe_b.status.success()
            && outside_probe_a.stdout == sentinel_a
            && outside_probe_b.stdout == sentinel_b
            && outside_probe_a.stdout != outside_probe_b.stdout,
        "strict_seatbelt_meter_excludes_transcripts/transcript_probe_distinguishes_sentinels: expected successful transcript-preferring reads with distinct complete sentinel bytes; sentinel_a={sentinel_a:?}; sentinel_b={sentinel_b:?}; outside_a={}; outside_b={}",
        complete_measurement(&outside_probe_a),
        complete_measurement(&outside_probe_b)
    );

    let strict_preference_probe = run_strict_cat(&strict_profile, &poison_a);
    assert!(
        !strict_preference_probe
            .stdout
            .windows(outside_probe_a.stdout.len())
            .any(|window| window == outside_probe_a.stdout),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_transcript_preference/sentinel-absent: expected complete outside sentinel to be absent under strict profile; outside={}; strict={}",
        complete_measurement(&outside_probe_a),
        complete_measurement(&strict_preference_probe)
    );
    assert!(
        String::from_utf8_lossy(&strict_preference_probe.stderr)
            .contains("Operation not permitted"),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_transcript_preference/denial-visible: expected stderr substring {:?}; outside={}; strict={}",
        "Operation not permitted",
        complete_measurement(&outside_probe_a),
        complete_measurement(&strict_preference_probe)
    );

    let explicit_poison_read = run_strict_cat(&strict_profile, &poison_a);
    assert!(
        !explicit_poison_read
            .stdout
            .windows(outside_probe_a.stdout.len())
            .any(|window| window == outside_probe_a.stdout),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_explicit_poison_read/sentinel-absent: expected complete outside sentinel to be absent under strict profile; outside={}; strict={}",
        complete_measurement(&outside_probe_a),
        complete_measurement(&explicit_poison_read)
    );
    assert!(
        String::from_utf8_lossy(&explicit_poison_read.stderr).contains("Operation not permitted"),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_rejects_explicit_poison_read/denial-visible: expected stderr substring {:?}; outside={}; strict={}",
        "Operation not permitted",
        complete_measurement(&outside_probe_a),
        complete_measurement(&explicit_poison_read)
    );

    let outside_executable_read = run_cat(&pce_executable);
    assert!(
        outside_executable_read.status.success() && !outside_executable_read.stdout.is_empty(),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_permits_executable_literal/outside-readable: expected status_success=true and non-empty stdout; status_success={}; stdout_len={}; stderr={:?}",
        outside_executable_read.status.success(),
        outside_executable_read.stdout.len(),
        outside_executable_read.stderr
    );
    let strict_executable_read = run_strict_cat(&strict_profile, &pce_executable);
    assert_eq!(
        (
            strict_executable_read.status.success(),
            strict_executable_read.stdout.as_slice()
        ),
        (
            outside_executable_read.status.success(),
            outside_executable_read.stdout.as_slice()
        ),
        "strict_seatbelt_meter_excludes_transcripts/strict_profile_permits_executable_literal: expected identical (status_success, stdout) pairs; outside_status_success={}; outside_stdout_len={}; outside_stderr={:?}; strict_status_success={}; strict_stdout_len={}; strict_stderr={:?}",
        outside_executable_read.status.success(),
        outside_executable_read.stdout.len(),
        outside_executable_read.stderr,
        strict_executable_read.status.success(),
        strict_executable_read.stdout.len(),
        strict_executable_read.stderr
    );

    let strict_meter = run_pce(&pce_executable, Some(&strict_profile), &event_log);
    assert_eq!(
        (
            strict_meter.status.success(),
            strict_meter.stdout.as_slice()
        ),
        (true, unsandboxed_meter.stdout.as_slice()),
        "strict_seatbelt_meter_excludes_transcripts/strict_meter_matches_unsandboxed_bytes: expected strict success and byte-identical report; strict_status_success={}; strict_stderr={:?}; unsandboxed_stdout={:?}; strict_stdout={:?}",
        strict_meter.status.success(),
        strict_meter.stderr,
        unsandboxed_meter.stdout,
        strict_meter.stdout
    );
}
