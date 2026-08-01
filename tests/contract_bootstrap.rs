#[allow(dead_code)]
mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use pce_core::{
    EventBodyRef, KnownPayload, LocalWorkflowStandIn, RepositoryContractPayload, parse_event_line,
    parse_tracked_repository_contract, serialize_tracked_repository_contract,
};
use serde_json::{Value, json};
use tempfile::{TempDir, tempdir};

use support::{WorkspaceFixtureDirectory, skip_without_nested_seatbelt};

const DEFAULT_BRANCH_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "cargo fmt --check",
      "lint": "cargo clippy --workspace --all-targets",
      "typecheck": "cargo check --workspace --all-targets",
      "test": "cargo test --workspace",
      "build": "cargo build --release"
    },
    "version_policy": "NONE",
    "branches": {
      "default": "main",
      "milestone": "pce/{vision}/milestone-{milestone}",
      "step": "pce/{vision}/m{milestone}-s{step}"
    },
    "pull_requests": {
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    },
    "workflows": []
  },
  "appendable": {
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }
}
"#;

const MANIFEST: &str = r#"[package]
name = "fixture"
version = "0.1.0"
edition = "2024"
"#;

const CI_WORKFLOW: &str = r#"name: CI
on: [push]
jobs:
  checks:
    runs-on: ubuntu-latest
    steps:
      - run: |-
          cargo fmt --check
          cargo clippy --all-targets
          cargo check --all-targets
          cargo test --all-targets
          cargo build --all-targets
"#;

const RELEASE_WORKFLOW: &str = r#"name: Release
on: [workflow_dispatch]
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
"#;

struct Fixture {
    _temporary: Option<TempDir>,
    _workspace: Option<WorkspaceFixtureDirectory>,
    repository: PathBuf,
    events: PathBuf,
}

impl Fixture {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let temporary = tempdir().expect("create fixture directory");
        let root = temporary.path().to_path_buf();
        Self::at_root(root, Some(temporary), None, files)
    }

    fn workspace(files: &[(&str, &[u8])]) -> Self {
        let workspace = WorkspaceFixtureDirectory::create("bootstrap")
            .expect("create workspace fixture directory");
        let root = workspace.path().to_path_buf();
        let fixture = Self::at_root(root, None, Some(workspace), files);
        fixture
            ._workspace
            .as_ref()
            .expect("workspace guard should exist")
            .assert_outside_temporary_roots(&fixture.repository);
        fixture
    }

    fn at_root(
        root: PathBuf,
        temporary: Option<TempDir>,
        workspace: Option<WorkspaceFixtureDirectory>,
        files: &[(&str, &[u8])],
    ) -> Self {
        let repository = root.join("repository");
        git(None, ["init", "-b", "main", path_text(&repository)]);
        git(Some(&repository), ["config", "user.name", "PCE Test"]);
        git(
            Some(&repository),
            ["config", "user.email", "pce-test@example.invalid"],
        );
        for (relative, bytes) in files {
            let path = repository.join(relative);
            fs::create_dir_all(path.parent().expect("fixture file parent"))
                .expect("create fixture file parent");
            fs::write(path, bytes).expect("write fixture file");
        }
        git(Some(&repository), ["add", "."]);
        git(Some(&repository), ["commit", "-m", "fixture"]);
        git(
            Some(&repository),
            [
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        let events = root.join("events.jsonl");
        fs::write(&events, b"").expect("create empty event log");
        Self {
            _temporary: temporary,
            _workspace: workspace,
            repository,
            events,
        }
    }

    fn invoke(&self, shim: Option<&Path>) -> Output {
        self.invoke_as(shim, "fixture")
    }

    fn invoke_as(&self, shim: Option<&Path>, repository_name: &str) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
        command.args([
            "contract",
            "bootstrap",
            "--file",
            path_text(&self.events),
            "--repo-root",
            path_text(&self.repository),
            "--repository",
            repository_name,
            "--node",
            "m3-s3",
        ]);
        if let Some(shim) = shim {
            let ordinary = std::env::var_os("PATH").expect("test PATH");
            command.env(
                "PATH",
                std::env::join_paths(
                    std::iter::once(shim.to_path_buf()).chain(std::env::split_paths(&ordinary)),
                )
                .expect("join shim PATH"),
            );
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run pce contract bootstrap")
    }

    fn invoke_with_exact_path(&self, path: &Path) -> Output {
        Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(["contract", "bootstrap", "--file"])
            .arg(&self.events)
            .arg("--repo-root")
            .arg(&self.repository)
            .args(["--repository", "fixture", "--node", "m3-s3"])
            .env_clear()
            .env("PATH", path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run pce contract bootstrap with exact PATH")
    }

    fn contract_path(&self) -> PathBuf {
        self.repository.join(".pce/repository-contract.json")
    }
}

fn probing_cargo_shim(fixture: &Fixture, exit_on_probe_denial: i32) -> (PathBuf, PathBuf, PathBuf) {
    let log = fixture.repository.join("probe-cargo.log");
    let probe = fixture
        ._workspace
        .as_ref()
        .expect("probing fixtures are workspace-backed")
        .path()
        .join("external-probe");
    assert!(!probe.starts_with(&fixture.repository));
    fs::write(&probe, b"host writable").expect("probe host write should work");
    fs::remove_file(&probe).expect("probe reset should work");
    let shim = create_shim(
        &fixture.repository,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}' || exit 74\nif printf probe > '{}'; then exit 0; fi\nexit {exit_on_probe_denial}\n",
            log.display(),
            probe.display()
        ),
    );
    (shim, log, probe)
}

#[test]
fn bootstrap_rejects_contract_present_at_default_branch_head_without_writing() {
    let fixture = Fixture::new(&[(".pce/repository-contract.json", DEFAULT_BRANCH_CONTRACT)]);
    fs::remove_file(fixture.contract_path()).expect("delete worktree contract");
    assert!(!fixture.contract_path().exists());
    let shown = git_output(
        Some(&fixture.repository),
        ["show", "main:.pce/repository-contract.json"],
    );
    assert!(shown.status.success());
    assert_eq!(shown.stdout, DEFAULT_BRANCH_CONTRACT);

    let output = fixture.invoke(None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains(
            "tracked repository contract .pce/repository-contract.json is present at default-branch HEAD; use `pce contract refresh`"
        ),
        "stderr: {stderr}"
    );
    assert_eq!(fs::read(&fixture.events).expect("read event log"), b"");
    assert!(!fixture.contract_path().exists());
}

#[test]
fn ci_derived_bootstrap_creates_contract_and_persists_every_workflow_unattended() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
        (".github/workflows/release.yml", RELEASE_WORKFLOW.as_bytes()),
    ]);
    assert_default_contract_absent(&fixture);
    let invocation_log = fixture.repository.join("ci-cargo.log");
    let shim = create_shim(
        &fixture.repository,
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> '{}' || exit 74
case "$*" in
  "fmt --check" | \
  "clippy --all-targets" | \
  "check --all-targets" | \
  "test --all-targets" | \
  "build --all-targets")
    exit 0
    ;;
  *)
    printf 'unexpected cargo command: %s\n' "$*" >&2
    exit 97
    ;;
esac
"#,
            invocation_log.display()
        ),
    );
    let output = fixture.invoke(Some(&shim));
    assert_success(&output);
    assert_eq!(
        fs::read_to_string(invocation_log).expect("CI invocation log should read"),
        "fmt --check\nclippy --all-targets\ncheck --all-targets\ntest --all-targets\nbuild --all-targets\n"
    );

    let expected = ci_contract_value();
    let bytes = fs::read(fixture.contract_path()).expect("read persisted contract");
    let tracked =
        parse_tracked_repository_contract(&bytes).expect("parse persisted tracked contract");
    let canonical =
        serialize_tracked_repository_contract(&tracked).expect("serialize persisted contract");
    assert_eq!(bytes, canonical);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).expect("parse contract JSON"),
        expected
    );
    let mappings = tracked.stated().workflows().as_slice();
    assert_eq!(mappings[0].workflow().as_str(), "ci.yml");
    let LocalWorkflowStandIn::Command(command) = mappings[0].stand_in() else {
        panic!("ci.yml should have a command stand-in");
    };
    assert_eq!(
        command.as_str(),
        "cargo fmt --check\ncargo clippy --all-targets\ncargo check --all-targets\ncargo test --all-targets\ncargo build --all-targets"
    );
    assert_eq!(mappings[1].workflow().as_str(), "release.yml");
    assert!(matches!(mappings[1].stand_in(), LocalWorkflowStandIn::None));

    let payloads = current_payloads(&fixture.events);
    assert_eq!(payloads.len(), 1);
    assert_payload_matches_contract(&payloads[0], &expected);
    assert_no_escalations(&fixture.events);
}

#[test]
fn ci_less_bootstrap_uses_declared_precedence_stops_early_and_preserves_stated_half() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[("Cargo.toml", MANIFEST.as_bytes())]);
    assert_default_contract_absent(&fixture);
    let cargo_log = fixture.repository.join("cargo.log");
    let cargo_log_text = path_text(&cargo_log);
    let shim = create_shim(
        &fixture.repository,
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> "{cargo_log_text}"
case "$*" in
  "fmt --all --check")
    exit 31
    ;;
  "fmt --check" | \
  "clippy --workspace --all-targets" | \
  "check --workspace --all-targets" | \
  "test --workspace" | \
  "build --workspace")
    exit 0
    ;;
  *)
    printf 'unexpected cargo command: %s\n' "$*" >&2
    exit 97
    ;;
esac
"#
        ),
    );
    let first = fixture.invoke(Some(&shim));
    assert_success(&first);
    let first_json: Value =
        serde_json::from_slice(&fs::read(fixture.contract_path()).expect("read first contract"))
            .expect("parse first contract JSON");
    assert_eq!(
        first_json.pointer("/stated/gates"),
        Some(&json!({
            "format": "cargo fmt --check",
            "lint": "cargo clippy --workspace --all-targets",
            "typecheck": "cargo check --workspace --all-targets",
            "test": "cargo test --workspace",
            "build": "cargo build --workspace"
        }))
    );
    assert_eq!(
        fs::read(&cargo_log).expect("read cargo invocation log"),
        b"fmt --all --check\nfmt --check\nclippy --workspace --all-targets\ncheck --workspace --all-targets\ntest --workspace\nbuild --workspace\nfmt --check\nclippy --workspace --all-targets\ncheck --workspace --all-targets\ntest --workspace\nbuild --workspace\n"
    );
    let stated = first_json["stated"].clone();
    let cargo_log_bytes = fs::read(&cargo_log).expect("save cargo log");

    let second = fixture.invoke(Some(&shim));
    assert_success(&second);
    let second_json: Value =
        serde_json::from_slice(&fs::read(fixture.contract_path()).expect("read second contract"))
            .expect("parse second contract JSON");
    assert_eq!(second_json["stated"], stated);
    let second_log = fs::read(&cargo_log).expect("reread cargo log");
    assert_eq!(
        &second_log[..cargo_log_bytes.len()],
        cargo_log_bytes.as_slice()
    );
    assert_eq!(
        &second_log[cargo_log_bytes.len()..],
        b"fmt --check\nclippy --workspace --all-targets\ncheck --workspace --all-targets\ntest --workspace\nbuild --workspace\n"
    );
    let payloads = current_payloads(&fixture.events);
    assert_eq!(payloads.len(), 2);
    assert_eq!(payloads[0].stated, payloads[1].stated);
    for payload in payloads {
        assert_eq!(payload.repository.as_str(), "fixture");
        assert_eq!(payload.repo_root.as_str(), path_text(&fixture.repository));
        assert_zero_observations(&payload);
    }
    assert_no_escalations(&fixture.events);
}

#[test]
fn ci_less_bootstrap_rejects_host_green_sandbox_red_candidates_without_persistence() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[("Cargo.toml", MANIFEST.as_bytes())]);
    let (shim, log, probe) = probing_cargo_shim(&fixture, 73);
    let output = fixture.invoke(Some(&shim));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(stderr.contains("no passing bootstrap candidate for format"));
    assert_eq!(
        fs::read_to_string(log).expect("candidate log should read"),
        "fmt --all --check\nfmt --check\n"
    );
    assert!(!probe.exists());
    assert!(!fixture.contract_path().exists());
    assert_eq!(
        fs::read(&fixture.events).expect("event log should read"),
        b""
    );
}

#[test]
fn ci_less_bootstrap_profile_denies_external_probe_and_allows_repository_log() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[("Cargo.toml", MANIFEST.as_bytes())]);
    let (shim, log, probe) = probing_cargo_shim(&fixture, 73);
    let output = fixture.invoke(Some(&shim));
    assert!(!output.status.success());
    assert!(
        log.exists(),
        "repository-local invocation log must be writable"
    );
    assert!(!probe.exists(), "external probe must remain denied");
}

#[test]
fn ci_less_bootstrap_distinguishes_unavailable_seatbelt_from_all_candidates_red() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[("Cargo.toml", MANIFEST.as_bytes())]);
    let (shim, log, _probe) = probing_cargo_shim(&fixture, 73);
    let ordinary = std::env::var_os("PATH").expect("test PATH should exist");
    let composed_path =
        std::env::join_paths(std::iter::once(shim.clone()).chain(std::env::split_paths(&ordinary)))
            .expect("outer PATH should compose");
    let unavailable = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "--"])
        .arg(env!("CARGO_BIN_EXE_pce"))
        .args(["contract", "bootstrap", "--file"])
        .arg(&fixture.events)
        .arg("--repo-root")
        .arg(&fixture.repository)
        .args(["--repository", "fixture", "--node", "m3-s3"])
        .env_clear()
        .env("PATH", composed_path)
        .output()
        .expect("outer Seatbelt bootstrap should run");
    let unavailable_stderr = String::from_utf8_lossy(&unavailable.stderr);
    assert!(!unavailable.status.success());
    assert!(
        unavailable_stderr.contains("failed to verify Seatbelt execution capability"),
        "stderr: {unavailable_stderr}"
    );
    assert!(!unavailable_stderr.contains("no passing bootstrap candidate"));
    assert!(!log.exists(), "capability failure must precede candidates");

    let all_red = fixture.invoke(Some(&shim));
    let all_red_stderr = String::from_utf8_lossy(&all_red.stderr);
    assert!(all_red_stderr.contains("no passing bootstrap candidate for format"));
    assert!(
        log.exists(),
        "repository-owned exhaustion must invoke candidates"
    );
}

#[test]
fn bootstrap_named_gate_distinguishes_path_resolution_from_gate_status() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[("Cargo.toml", MANIFEST.as_bytes())]);
    let (shim, log, probe) = probing_cargo_shim(&fixture, 73);
    let output = fixture.invoke(Some(&shim));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no passing bootstrap candidate for format"));
    assert!(!stderr.contains("failed to resolve executable"));
    assert!(
        fs::read_to_string(log)
            .expect("PATH-resolved invocation should record")
            .starts_with("fmt --all --check\n")
    );
    assert!(!probe.exists());
}

#[test]
fn bootstrap_missing_gate_executable_is_execution_failure_without_invocation() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
    ]);
    let path = fixture.repository.join("missing-cargo-path");
    fs::create_dir(&path).expect("exact PATH directory should create");
    std::os::unix::fs::symlink("/usr/bin/git", path.join("git"))
        .expect("git symlink should create");
    let output = fixture.invoke_with_exact_path(&path);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains(
            "gate executable `cargo` was not found as an executable file on forwarded PATH"
        ),
        "stderr: {stderr}"
    );
    assert!(!fixture.repository.join("missing-invocation.log").exists());
    assert!(!fixture.contract_path().exists());
}

#[test]
fn bootstrap_measurement_failure_leaves_contract_unchanged() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
    ]);
    let shim = create_shim(&fixture.repository, "#!/bin/sh\nexit 29\n");
    let output = fixture.invoke(Some(&shim));
    assert!(!output.status.success());
    assert!(!fixture.contract_path().exists());
}

#[test]
fn bootstrap_measurement_failure_leaves_event_unchanged() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
    ]);
    let before = fs::read(&fixture.events).expect("event baseline should read");
    let shim = create_shim(&fixture.repository, "#!/bin/sh\nexit 29\n");
    let output = fixture.invoke(Some(&shim));
    assert!(!output.status.success());
    assert_eq!(
        fs::read(&fixture.events).expect("event log should read"),
        before
    );
}

#[test]
fn successful_sandboxed_bootstrap_preserves_stated_and_appends_event() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
        (".github/workflows/release.yml", RELEASE_WORKFLOW.as_bytes()),
    ]);
    let log = fixture.repository.join("successful-bootstrap.log");
    let shim = create_shim(
        &fixture.repository,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}' || exit 74\nexit 0\n",
            log.display()
        ),
    );
    let output = fixture.invoke(Some(&shim));
    assert_success(&output);
    let tracked: Value = serde_json::from_slice(
        &fs::read(fixture.contract_path()).expect("tracked contract should read"),
    )
    .expect("tracked contract should parse");
    assert_eq!(tracked["stated"], ci_contract_value()["stated"]);
    assert_eq!(current_payloads(&fixture.events).len(), 1);
    assert_eq!(
        current_payloads(&fixture.events)[0].stated.test,
        "cargo test --all-targets"
    );
}

#[test]
fn bootstrap_rejects_ambiguous_duplicate_contract_identity() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = Fixture::workspace(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
    ]);
    let log = fixture.repository.join("identity.log");
    let shim = create_shim(
        &fixture.repository,
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexit 0\n",
            log.display()
        ),
    );
    assert_success(&fixture.invoke(Some(&shim)));
    let accepted_baseline = fs::read(&log).expect("first measurement should read");
    assert_success(&fixture.invoke(Some(&shim)));
    let accepted = fs::read(&log).expect("second measurement should read");
    assert!(
        accepted.len() > accepted_baseline.len(),
        "exact identity must remeasure"
    );

    let event_before = fs::read(&fixture.events).expect("events should read");
    let log_before = fs::read(&log).expect("log should read");
    let different_name = fixture.invoke_as(Some(&shim), "other");
    assert!(!different_name.status.success());
    assert!(String::from_utf8_lossy(&different_name.stderr).contains("ambiguous duplicate"));
    assert_eq!(
        fs::read(&fixture.events).expect("events should read"),
        event_before
    );
    assert_eq!(fs::read(&log).expect("log should read"), log_before);

    let other = Fixture::workspace(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
    ]);
    fs::write(&other.events, &event_before).expect("shared event history should write");
    let other_output = other.invoke(Some(&shim));
    assert!(!other_output.status.success());
    assert!(String::from_utf8_lossy(&other_output.stderr).contains("ambiguous duplicate"));
    assert!(!other.contract_path().exists());
}

fn assert_default_contract_absent(fixture: &Fixture) {
    let shown = git_output(
        Some(&fixture.repository),
        ["show", "main:.pce/repository-contract.json"],
    );
    assert!(!shown.status.success());
    assert!(!fixture.contract_path().exists());
}

fn ci_contract_value() -> Value {
    json!({
        "stated": {
            "gates": {
                "format": "cargo fmt --check",
                "lint": "cargo clippy --all-targets",
                "typecheck": "cargo check --all-targets",
                "test": "cargo test --all-targets",
                "build": "cargo build --all-targets"
            },
            "version_policy": "NONE",
            "branches": {
                "default": "main",
                "milestone": "pce/{vision}/milestone-{milestone}",
                "step": "pce/{vision}/m{milestone}-s{step}"
            },
            "pull_requests": {
                "step_base": "MILESTONE",
                "milestone_base": "DEFAULT",
                "merge_method": "SQUASH"
            },
            "workflows": [
                {
                    "workflow": "ci.yml",
                    "stand_in": {
                        "kind": "COMMAND",
                        "command": "cargo fmt --check\ncargo clippy --all-targets\ncargo check --all-targets\ncargo test --all-targets\ncargo build --all-targets"
                    }
                },
                {"workflow": "release.yml", "stand_in": {"kind": "NONE"}}
            ]
        },
        "appendable": {
            "environment_hazards": [],
            "gate_orderings": [],
            "lockfile_rules": []
        }
    })
}

fn current_payloads(path: &Path) -> Vec<RepositoryContractPayload> {
    let text = fs::read_to_string(path).expect("read event log");
    text.lines()
        .map(|line| parse_event_line(line).expect("parse event line"))
        .filter_map(|record| match record.body_ref() {
            EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) => Some(payload.clone()),
            _ => None,
        })
        .collect()
}

fn assert_payload_matches_contract(payload: &RepositoryContractPayload, contract: &Value) {
    assert_eq!(payload.repository.as_str(), "fixture");
    assert_zero_observations(payload);
    assert_eq!(
        payload.workflow_map.as_map().get("ci.yml"),
        Some(&Some(
            "cargo fmt --check\ncargo clippy --all-targets\ncargo check --all-targets\ncargo test --all-targets\ncargo build --all-targets".to_owned()
        ))
    );
    assert_eq!(
        payload.workflow_map.as_map().get("release.yml"),
        Some(&None)
    );
    assert_eq!(payload.stated.format, contract["stated"]["gates"]["format"]);
    assert_eq!(payload.stated.lint, contract["stated"]["gates"]["lint"]);
    assert_eq!(
        payload.evidence.as_str(),
        "cargo fmt --check\ncargo clippy --all-targets\ncargo check --all-targets\ncargo test --all-targets\ncargo build --all-targets"
    );
}

fn assert_zero_observations(payload: &RepositoryContractPayload) {
    assert_eq!(payload.observations.format.code(), 0);
    assert_eq!(payload.observations.lint.code(), 0);
    assert_eq!(payload.observations.typecheck.code(), 0);
    assert_eq!(payload.observations.test.code(), 0);
    assert_eq!(payload.observations.build.code(), 0);
}

fn assert_no_escalations(path: &Path) {
    let text = fs::read_to_string(path).expect("read event log");
    for line in text.lines() {
        let record = parse_event_line(line).expect("parse event line");
        assert!(!matches!(
            record.body_ref(),
            EventBodyRef::Known(KnownPayload::EscalationOpen(_) | KnownPayload::EscalationClose(_))
        ));
    }
}

fn create_shim(root: &Path, body: &str) -> PathBuf {
    let directory = root.join("shim");
    fs::create_dir(&directory).expect("create shim directory");
    let cargo = directory.join("cargo");
    fs::write(&cargo, body).expect("write cargo shim");
    let mut permissions = fs::metadata(&cargo)
        .expect("cargo shim metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cargo, permissions).expect("make cargo shim executable");
    directory
}

fn assert_success(output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
}

fn git<const N: usize>(repository: Option<&Path>, args: [&str; N]) {
    let output = git_output(repository, args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "git failed: {stderr}");
}

fn git_output<const N: usize>(repository: Option<&Path>, args: [&str; N]) -> Output {
    let mut command = Command::new("git");
    if let Some(repository) = repository {
        command.arg("-C").arg(repository);
    }
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run git")
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("fixture path should be UTF-8")
}
