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
    _temporary: TempDir,
    root: PathBuf,
    repository: PathBuf,
    events: PathBuf,
}

impl Fixture {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let temporary = tempdir().expect("create fixture directory");
        let root = temporary.path().to_path_buf();
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
            root,
            repository,
            events,
        }
    }

    fn invoke(&self, shim: Option<&Path>, cargo_log: Option<&Path>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
        command.args([
            "contract",
            "bootstrap",
            "--file",
            path_text(&self.events),
            "--repo-root",
            path_text(&self.repository),
            "--repository",
            "fixture",
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
        if let Some(cargo_log) = cargo_log {
            command.env("PCE_CARGO_LOG", cargo_log);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("run pce contract bootstrap")
    }

    fn contract_path(&self) -> PathBuf {
        self.repository.join(".pce/repository-contract.json")
    }
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

    let output = fixture.invoke(None, None);
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
    let fixture = Fixture::new(&[
        ("Cargo.toml", MANIFEST.as_bytes()),
        (".github/workflows/ci.yml", CI_WORKFLOW.as_bytes()),
        (".github/workflows/release.yml", RELEASE_WORKFLOW.as_bytes()),
    ]);
    assert_default_contract_absent(&fixture);
    let shim = create_shim(
        &fixture.root,
        r#"#!/bin/sh
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
    );
    let output = fixture.invoke(Some(&shim), None);
    assert_success(&output);

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
    let fixture = Fixture::new(&[("Cargo.toml", MANIFEST.as_bytes())]);
    assert_default_contract_absent(&fixture);
    let cargo_log = fixture.root.join("cargo.log");
    let shim = create_shim(
        &fixture.root,
        r#"#!/bin/sh
printf '%s\n' "$*" >> "${PCE_CARGO_LOG:?PCE_CARGO_LOG is required}"
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
"#,
    );
    let first = fixture.invoke(Some(&shim), Some(&cargo_log));
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
        b"fmt --all --check\nfmt --check\nclippy --workspace --all-targets\ncheck --workspace --all-targets\ntest --workspace\nbuild --workspace\n"
    );
    let stated = first_json["stated"].clone();
    let cargo_log_bytes = fs::read(&cargo_log).expect("save cargo log");

    let second = fixture.invoke(Some(&shim), Some(&cargo_log));
    assert_success(&second);
    let second_json: Value =
        serde_json::from_slice(&fs::read(fixture.contract_path()).expect("read second contract"))
            .expect("parse second contract JSON");
    assert_eq!(second_json["stated"], stated);
    assert_eq!(
        fs::read(&cargo_log).expect("reread cargo log"),
        cargo_log_bytes
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
