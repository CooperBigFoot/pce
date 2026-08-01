#[allow(dead_code)]
mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use pce_core::{
    EventBodyRef, KnownPayload, LocalWorkflowStandIn, MilestonePullRequestBase,
    PullRequestMergeMethod, RepositoryContractPayload, StepPullRequestBase, VersionPolicy,
    parse_event_line, parse_tracked_repository_contract,
};
use serde_json::{Value, json};
use tempfile::tempdir;

use support::skip_without_nested_seatbelt;

const FINDING: &str = "Cargo.lock must be regenerated before cargo test";
const CURRENT_FINDING: &str = r#"{"sequence":2,"timestamp":"2026-07-29T12:00:01.000Z","kind":"key-finding","node":"m3-s5","payload":{"finding":"Cargo.lock must be regenerated before cargo test","evidence":"cargo test --workspace"}}"#;
const PRIOR_FINDING: &str = r#"{"sequence":2,"timestamp":"2026-07-28T12:00:01.000Z","kind":"key-finding","node":"m2-s2","payload":{"finding":"Cargo.lock must be regenerated before cargo test","evidence":"cargo test --workspace"}}"#;
const DIVERGENT_PRIOR_FINDING: &str = r#"{"sequence":2,"timestamp":"2026-07-28T12:00:01.000Z","kind":"key-finding","node":"m2-s2","payload":{"finding":"Cargo.lock can be regenerated after cargo test","evidence":"cargo test --workspace"}}"#;
const NON_SELECTED_CURRENT_FINDING: &str = r#"{"sequence":2,"timestamp":"2026-07-29T12:00:01.000Z","kind":"key-finding","node":"m3-s5","payload":{"finding":"Cargo.lock can be regenerated after cargo test","evidence":"cargo test --workspace"}}"#;
#[derive(Debug)]
struct GateFixtureCommands {
    format: String,
    lint: String,
    typecheck: String,
    test: String,
    build: String,
}

impl GateFixtureCommands {
    fn values(&self) -> [&str; 5] {
        [
            &self.format,
            &self.lint,
            &self.typecheck,
            &self.test,
            &self.build,
        ]
    }

    fn evidence(&self) -> String {
        self.values().join("\n")
    }
}

fn tracked_contract(commands: &GateFixtureCommands) -> Vec<u8> {
    format!(
        r#"{{
  "stated": {{
    "gates": {{
      "format": "{}",
      "lint": "{}",
      "typecheck": "{}",
      "test": "{}",
      "build": "{}"
    }},
    "version_policy": "SERIALIZE_DISPATCHES",
    "branches": {{
      "default": "contract-default-not-git-main",
      "milestone": "integration/{{vision}}/{{milestone}}",
      "step": "work/{{vision}}/{{milestone}}/{{step}}"
    }},
    "pull_requests": {{
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    }},
    "workflows": [
      {{
        "workflow": "ci.yml",
        "stand_in": {{
          "kind": "NONE"
        }}
      }}
    ]
  }},
  "appendable": {{
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }}
}}
"#,
        commands.format, commands.lint, commands.typecheck, commands.test, commands.build
    )
    .into_bytes()
}

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    current_log: PathBuf,
    prior_log: PathBuf,
    commands: GateFixtureCommands,
}

fn git(root: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git fixture command should spawn");
    assert!(
        output.status.success(),
        "git fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn repository_contract_line(
    root: &Path,
    repository: &str,
    commands: &GateFixtureCommands,
) -> String {
    json!({
        "sequence": 1,
        "timestamp": "2026-07-29T12:00:00.000Z",
        "kind": "repository-contract",
        "node": "m3-s4",
        "payload": {
            "repository": repository,
            "repo_root": root.to_str().expect("fixture root should be UTF-8"),
            "stated": {
                "format": commands.format,
                "lint": commands.lint,
                "typecheck": commands.typecheck,
                "test": commands.test,
                "build": commands.build,
                "version_policy": "SERIALIZE_DISPATCHES",
                "branch_convention": "default=contract-default-not-git-main; milestone=integration/{vision}/{milestone}; step=work/{vision}/{milestone}/{step}",
                "pull_request_convention": "step_base=MILESTONE; milestone_base=DEFAULT; merge_method=SQUASH"
            },
            "observations": {
                "format": 0,
                "lint": 0,
                "typecheck": 0,
                "test": 0,
                "build": 0
            },
            "workflow_map": {
                "ci.yml": null
            },
            "appendable": {
                "environment_hazards": [],
                "gate_orderings": [],
                "lockfile_rules": []
            },
            "evidence": commands.evidence()
        }
    })
    .to_string()
}

fn log_body(
    root: &Path,
    repository: &str,
    commands: &GateFixtureCommands,
    finding_line: Option<&str>,
) -> String {
    let mut body = repository_contract_line(root, repository, commands);
    body.push('\n');
    if let Some(line) = finding_line {
        body.push_str(line);
        body.push('\n');
    }
    body
}

fn gate_fixture_commands(root: &Path) -> GateFixtureCommands {
    let command = |role: &str| {
        let shim = root.join(format!("gate-{role}"));
        let log = root.join(format!("gate-{role}-invocations.bin"));
        fs::write(
            &shim,
            format!(
                "#!/bin/sh\n{{ printf '{role}\\0%s\\0' \"$#\"; printf '%s\\0' \"$@\"; }} >> '{}' || exit 74\n",
                log.display()
            ),
        )
        .expect("gate shim should write");
        let mut permissions = fs::metadata(&shim)
            .expect("gate shim metadata should read")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&shim, permissions).expect("gate shim should be executable");
        shim.display().to_string()
    };
    GateFixtureCommands {
        format: command("format"),
        lint: command("lint"),
        typecheck: command("typecheck"),
        test: command("test"),
        build: command("build"),
    }
}

fn fixture(prior_finding: &str) -> Fixture {
    let directory = tempdir().expect("temporary directory should create");
    let root = directory.path().join("repository");
    let init = Command::new("git")
        .args(["init", "-b", "main"])
        .arg(&root)
        .output()
        .expect("git init should spawn");
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    git(&root, &["config", "user.name", "PCE Test"]);
    git(&root, &["config", "user.email", "pce-test@example.invalid"]);
    let commands = gate_fixture_commands(&root);
    let workflow = root.join(".github/workflows/ci.yml");
    fs::create_dir_all(workflow.parent().expect("workflow should have parent"))
        .expect("workflow directory should create");
    fs::write(&workflow, "name: CI\non: [push]\njobs: {}\n").expect("workflow should write");
    let tracked = root.join(".pce/repository-contract.json");
    fs::create_dir_all(tracked.parent().expect("contract should have parent"))
        .expect("contract directory should create");
    fs::write(&tracked, tracked_contract(&commands)).expect("contract should write");
    git(&root, &["add", ".github/workflows/ci.yml"]);
    git(&root, &["add", ".pce/repository-contract.json"]);
    git(&root, &["commit", "-m", "initial authority"]);
    git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(
        &root,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );

    let current_log = directory.path().join("current.jsonl");
    let prior_log = directory.path().join("prior.jsonl");
    fs::write(
        &current_log,
        log_body(
            &root,
            "fixture-repository",
            &commands,
            Some(CURRENT_FINDING),
        ),
    )
    .expect("current log should write");
    fs::write(
        &prior_log,
        log_body(&root, "fixture-repository", &commands, Some(prior_finding)),
    )
    .expect("prior log should write");
    Fixture {
        _directory: directory,
        root,
        current_log,
        prior_log,
        commands,
    }
}

fn invocation_measurement(fixture: &Fixture) -> Vec<Vec<u8>> {
    ["format", "lint", "typecheck", "test", "build"]
        .into_iter()
        .map(|role| {
            fs::read(fixture.root.join(format!("gate-{role}-invocations.bin"))).unwrap_or_default()
        })
        .collect()
}

fn assert_no_gate_invocations(fixture: &Fixture) {
    assert_eq!(invocation_measurement(fixture), vec![Vec::<u8>::new(); 5]);
}

fn learn_args(current: &Path, prior: &Path, root: &Path) -> Vec<String> {
    vec![
        "contract".to_owned(),
        "learn".to_owned(),
        "--file".to_owned(),
        current.display().to_string(),
        "--prior-file".to_owned(),
        prior.display().to_string(),
        "--repo-root".to_owned(),
        root.display().to_string(),
        "--node".to_owned(),
        "m3-s5".to_owned(),
        "--category".to_owned(),
        "lockfile-rule".to_owned(),
        "--finding".to_owned(),
        FINDING.to_owned(),
    ]
}

fn run_pce(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(args)
        .output()
        .expect("pce binary should spawn")
}

fn invoke_learn(fixture: &Fixture) -> Output {
    run_pce(&learn_args(
        &fixture.current_log,
        &fixture.prior_log,
        &fixture.root,
    ))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn git_show_contract(root: &Path) -> Vec<u8> {
    git(root, &["show", "main:.pce/repository-contract.json"]).stdout
}

fn stated_range(bytes: &[u8]) -> Vec<u8> {
    const PREFIX: &[u8] = b"{\n  \"stated\": ";
    const DELIMITER: &[u8] = b",\n  \"appendable\": ";
    let prefix_positions = bytes
        .windows(PREFIX.len())
        .enumerate()
        .filter_map(|(index, candidate)| (candidate == PREFIX).then_some(index))
        .collect::<Vec<_>>();
    let delimiter_positions = bytes
        .windows(DELIMITER.len())
        .enumerate()
        .filter_map(|(index, candidate)| (candidate == DELIMITER).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(prefix_positions.len(), 1, "stated prefix must be unique");
    assert_eq!(
        delimiter_positions.len(),
        1,
        "appendable delimiter must be unique"
    );
    let start = prefix_positions[0] + PREFIX.len();
    let end = delimiter_positions[0];
    assert!(start < end, "stated range must be ordered");
    bytes[start..end].to_vec()
}

fn repository_payloads(path: &Path) -> Vec<RepositoryContractPayload> {
    fs::read_to_string(path)
        .expect("event log should read")
        .lines()
        .filter_map(|line| {
            let record = parse_event_line(line).expect("physical event line should parse");
            match record.body_ref() {
                EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) => {
                    Some(payload.clone())
                }
                _ => None,
            }
        })
        .collect()
}

fn assert_selected_appendable(path: &Path) {
    let payloads = repository_payloads(path);
    let latest = payloads.last().expect("current contract should exist");
    assert_eq!(latest.appendable.lockfile_rules, [FINDING]);
    assert!(latest.appendable.environment_hazards.is_empty());
    assert!(latest.appendable.gate_orderings.is_empty());
}

fn commit_learned_contract(root: &Path) {
    git(root, &["add", ".pce/repository-contract.json"]);
    git(root, &["commit", "-m", "record learned authority"]);
}

#[test]
fn first_occurrence_leaves_tracked_contract_and_current_log_unchanged() {
    let fixture = fixture(DIVERGENT_PRIOR_FINDING);
    let current: Value =
        serde_json::from_str(CURRENT_FINDING).expect("current finding should be JSON");
    let prior: Value =
        serde_json::from_str(DIVERGENT_PRIOR_FINDING).expect("prior finding should be JSON");
    assert_ne!(current["payload"]["finding"], prior["payload"]["finding"]);
    let tracked_path = fixture.root.join(".pce/repository-contract.json");
    let tracked_before = fs::read(&tracked_path).expect("tracked contract should read");
    let log_before = fs::read(&fixture.current_log).expect("current log should read");

    let output = invoke_learn(&fixture);

    assert!(output.status.success(), "learn failed: {}", stderr(&output));
    assert_eq!(
        fs::read(tracked_path).expect("tracked contract should read"),
        tracked_before
    );
    assert_eq!(
        fs::read(&fixture.current_log).expect("current log should read"),
        log_before
    );
    assert_no_gate_invocations(&fixture);
}

#[test]
fn recurrence_appends_once_and_preserves_stated_half_byte_for_byte() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = fixture(PRIOR_FINDING);
    let stated_before = stated_range(&git_show_contract(&fixture.root));
    let before = invocation_measurement(&fixture);

    let output = invoke_learn(&fixture);

    assert!(output.status.success(), "learn failed: {}", stderr(&output));
    let tracked_bytes = fs::read(fixture.root.join(".pce/repository-contract.json"))
        .expect("tracked contract should read");
    assert_eq!(stated_range(&tracked_bytes), stated_before);
    let tracked =
        parse_tracked_repository_contract(&tracked_bytes).expect("learned contract should parse");
    let stated = tracked.stated();
    assert_eq!(
        stated.gates().format().as_str(),
        fixture.commands.format.as_str()
    );
    assert_eq!(
        stated.gates().lint().as_str(),
        fixture.commands.lint.as_str()
    );
    assert_eq!(
        stated.gates().typecheck().as_str(),
        fixture.commands.typecheck.as_str()
    );
    assert_eq!(
        stated.gates().test().as_str(),
        fixture.commands.test.as_str()
    );
    assert_eq!(
        stated.gates().build().as_str(),
        fixture.commands.build.as_str()
    );
    assert_eq!(stated.version_policy(), &VersionPolicy::SerializeDispatches);
    assert_eq!(
        stated.branches().default().as_str(),
        "contract-default-not-git-main"
    );
    assert_eq!(
        stated.branches().milestone().as_str(),
        "integration/{vision}/{milestone}"
    );
    assert_eq!(
        stated.branches().step().as_str(),
        "work/{vision}/{milestone}/{step}"
    );
    assert_eq!(
        stated.pull_requests().step_base(),
        StepPullRequestBase::Milestone
    );
    assert_eq!(
        stated.pull_requests().milestone_base(),
        MilestonePullRequestBase::Default
    );
    assert_eq!(
        stated.pull_requests().merge_method(),
        PullRequestMergeMethod::Squash
    );
    let workflows = stated.workflows().as_slice();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].workflow().as_str(), "ci.yml");
    assert!(matches!(
        workflows[0].stand_in(),
        LocalWorkflowStandIn::None
    ));
    assert_eq!(
        tracked
            .appendable()
            .lockfile_rules()
            .iter()
            .map(|rule| rule.as_str())
            .collect::<Vec<_>>(),
        [FINDING]
    );
    assert!(tracked.appendable().environment_hazards().is_empty());
    assert!(tracked.appendable().gate_orderings().is_empty());
    assert_selected_appendable(&fixture.current_log);
    let after = invocation_measurement(&fixture);
    assert!(
        before
            .iter()
            .zip(&after)
            .all(|(first, second)| first.is_empty() && !second.is_empty()),
        "admitted learning must freshly invoke every role"
    );
}

#[test]
fn duplicate_recurrence_is_idempotent() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = fixture(PRIOR_FINDING);
    let baseline = invocation_measurement(&fixture);
    let first = invoke_learn(&fixture);
    assert!(
        first.status.success(),
        "first learn failed: {}",
        stderr(&first)
    );
    commit_learned_contract(&fixture.root);
    let committed = git_show_contract(&fixture.root);
    let committed_text = String::from_utf8(committed.clone()).expect("contract should be UTF-8");
    assert_eq!(committed_text.matches(FINDING).count(), 1);
    let stated_before = stated_range(&committed);
    let count_before = repository_payloads(&fixture.current_log).len();
    let first_measurement = invocation_measurement(&fixture);

    let second = invoke_learn(&fixture);

    assert!(
        second.status.success(),
        "second learn failed: {}",
        stderr(&second)
    );
    let tracked_bytes = fs::read(fixture.root.join(".pce/repository-contract.json"))
        .expect("tracked contract should read");
    let tracked =
        parse_tracked_repository_contract(&tracked_bytes).expect("tracked contract should parse");
    assert_eq!(
        tracked
            .appendable()
            .lockfile_rules()
            .iter()
            .map(|rule| rule.as_str())
            .collect::<Vec<_>>(),
        [FINDING]
    );
    let second_measurement = invocation_measurement(&fixture);
    for ((baseline, first), second) in baseline
        .iter()
        .zip(&first_measurement)
        .zip(&second_measurement)
    {
        assert!(baseline.is_empty());
        assert_eq!(second.len(), first.len() * 2);
    }
    assert!(tracked.appendable().environment_hazards().is_empty());
    assert!(tracked.appendable().gate_orderings().is_empty());
    assert_eq!(stated_range(&tracked_bytes), stated_before);
    let payloads = repository_payloads(&fixture.current_log);
    assert_eq!(payloads.len(), count_before + 1);
    assert_eq!(
        payloads
            .last()
            .expect("latest payload should exist")
            .appendable
            .lockfile_rules,
        [FINDING]
    );
}

#[test]
fn same_log_path_is_rejected() {
    let fixture = fixture(PRIOR_FINDING);
    let tracked_path = fixture.root.join(".pce/repository-contract.json");
    let tracked_before = fs::read(&tracked_path).expect("tracked contract should read");
    let log_before = fs::read(&fixture.current_log).expect("current log should read");
    let output = run_pce(&learn_args(
        &fixture.current_log,
        &fixture.current_log,
        &fixture.root,
    ));

    assert!(!output.status.success());
    assert!(stderr(&output).contains("current and prior run logs must be distinct paths"));
    assert_eq!(
        fs::read(tracked_path).expect("tracked contract should read"),
        tracked_before
    );
    assert_eq!(
        fs::read(&fixture.current_log).expect("current log should read"),
        log_before
    );
    assert_no_gate_invocations(&fixture);
}

#[test]
fn prior_repository_identity_mismatch_is_rejected() {
    let fixture = fixture(PRIOR_FINDING);
    fs::write(
        &fixture.prior_log,
        log_body(
            &fixture.root,
            "other-repository",
            &fixture.commands,
            Some(PRIOR_FINDING),
        ),
    )
    .expect("prior log should write");
    let tracked_path = fixture.root.join(".pce/repository-contract.json");
    let tracked_before = fs::read(&tracked_path).expect("tracked contract should read");
    let log_before = fs::read(&fixture.current_log).expect("current log should read");

    let output = invoke_learn(&fixture);

    assert!(!output.status.success());
    assert!(stderr(&output).contains(
        "prior run log repository identity does not match current run repository fixture-repository"
    ));
    assert_eq!(
        fs::read(tracked_path).expect("tracked contract should read"),
        tracked_before
    );
    assert_eq!(
        fs::read(&fixture.current_log).expect("current log should read"),
        log_before
    );
    assert_no_gate_invocations(&fixture);
}

#[test]
fn missing_current_key_finding_is_rejected() {
    let fixture = fixture(PRIOR_FINDING);
    fs::write(
        &fixture.current_log,
        log_body(
            &fixture.root,
            "fixture-repository",
            &fixture.commands,
            Some(NON_SELECTED_CURRENT_FINDING),
        ),
    )
    .expect("current log should write");
    let tracked_path = fixture.root.join(".pce/repository-contract.json");
    let tracked_before = fs::read(&tracked_path).expect("tracked contract should read");
    let log_before = fs::read(&fixture.current_log).expect("current log should read");

    let output = invoke_learn(&fixture);

    assert!(!output.status.success());
    assert!(stderr(&output).contains(
        "current run log contains no byte-exact key-finding \"Cargo.lock must be regenerated before cargo test\""
    ));
    assert_eq!(
        fs::read(tracked_path).expect("tracked contract should read"),
        tracked_before
    );
    assert_eq!(
        fs::read(&fixture.current_log).expect("current log should read"),
        log_before
    );
    assert_no_gate_invocations(&fixture);
}

#[test]
fn later_lifecycle_invocation_recovers_exact_entry_without_key_finding() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = fixture(PRIOR_FINDING);
    let baseline = invocation_measurement(&fixture);
    let learned = invoke_learn(&fixture);
    assert!(
        learned.status.success(),
        "learn failed: {}",
        stderr(&learned)
    );
    commit_learned_contract(&fixture.root);
    let learned_measurement = invocation_measurement(&fixture);
    let later_log = fixture._directory.path().join("later.jsonl");
    fs::write(
        &later_log,
        log_body(&fixture.root, "fixture-repository", &fixture.commands, None),
    )
    .expect("later log should write");
    let refresh_args = vec![
        "contract".to_owned(),
        "refresh".to_owned(),
        "--file".to_owned(),
        later_log.display().to_string(),
        "--repo-root".to_owned(),
        fixture.root.display().to_string(),
        "--node".to_owned(),
        "m3-s6".to_owned(),
    ];

    let refreshed = run_pce(&refresh_args);

    assert!(
        refreshed.status.success(),
        "refresh failed: {}",
        stderr(&refreshed)
    );
    assert_selected_appendable(&later_log);
    let has_key_finding = fs::read_to_string(&later_log)
        .expect("later log should read")
        .lines()
        .any(|line| {
            let record = parse_event_line(line).expect("event line should parse");
            matches!(
                record.body_ref(),
                EventBodyRef::Known(KnownPayload::KeyFinding(_))
            )
        });
    assert!(!has_key_finding);
    let refreshed_measurement = invocation_measurement(&fixture);
    for ((baseline, learned), refreshed) in baseline
        .iter()
        .zip(&learned_measurement)
        .zip(&refreshed_measurement)
    {
        assert!(baseline.is_empty());
        let learn_delta = &learned[baseline.len()..];
        let refresh_delta = &refreshed[learned.len()..];
        assert!(!learn_delta.is_empty());
        assert_eq!(refresh_delta, learn_delta);
    }
}
