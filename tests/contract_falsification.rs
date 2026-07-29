#[allow(dead_code)]
mod support;

use std::ffi::OsString;
use std::fs;

use support::CliHarness;

const NON_ZERO_GATE_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "exit 17",
      "lint": "true",
      "typecheck": "true",
      "test": "true",
      "build": "true"
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
}"#;

const OMITTED_WORKFLOW_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "true",
      "lint": "true",
      "typecheck": "true",
      "test": "true",
      "build": "true"
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
}"#;

const EXPLICIT_NONE_CONTRACT: &[u8] = br#"{
  "stated": {
    "gates": {
      "format": "true",
      "lint": "true",
      "typecheck": "true",
      "test": "true",
      "build": "true"
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
          "kind": "NONE"
        }
      }
    ]
  },
  "appendable": {
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }
}"#;

fn contract_check_args(
    contract_path: &std::path::Path,
    repository_root: &std::path::Path,
) -> [OsString; 6] {
    [
        OsString::from("contract"),
        OsString::from("check"),
        OsString::from("--file"),
        contract_path.as_os_str().to_owned(),
        OsString::from("--repo-root"),
        repository_root.as_os_str().to_owned(),
    ]
}

#[test]
fn contract_check_rejects_non_zero_base_gate_with_command_and_status() {
    let harness = CliHarness::new().expect("CLI harness should create");
    let repository_root = harness.path().join("repo");
    fs::create_dir(&repository_root).expect("repository fixture should create");
    let contract_path = harness.path().join("contract.json");
    fs::write(&contract_path, NON_ZERO_GATE_CONTRACT).expect("contract fixture should write");

    let output = harness
        .run(contract_check_args(&contract_path, &repository_root), b"")
        .expect("CLI should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("stated gate command `exit 17` exited with status 17"),
        "stderr was: {stderr}"
    );
}

#[test]
fn contract_check_rejects_omitted_workflow_and_accepts_explicit_none() {
    let harness = CliHarness::new().expect("CLI harness should create");
    let repository_root = harness.path().join("repo");
    let workflows_directory = repository_root.join(".github/workflows");
    fs::create_dir_all(&workflows_directory).expect("workflow directory fixture should create");
    fs::write(
        workflows_directory.join("ci.yml"),
        b"name: CI\non: [push]\njobs: {}\n",
    )
    .expect("workflow fixture should write");
    let contract_path = harness.path().join("contract.json");
    fs::write(&contract_path, OMITTED_WORKFLOW_CONTRACT).expect("contract fixture should write");
    let args = contract_check_args(&contract_path, &repository_root);

    let omitted_output = harness
        .run(args.clone(), b"")
        .expect("omitted-workflow CLI should run");
    assert!(!omitted_output.status.success());
    let stderr = String::from_utf8(omitted_output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains(
            "tracked repository contract omits workflow `ci.yml` present in .github/workflows"
        ),
        "stderr was: {stderr}"
    );

    fs::write(&contract_path, EXPLICIT_NONE_CONTRACT)
        .expect("explicit NONE contract fixture should write");
    let covered_output = harness
        .run(args, b"")
        .expect("covered-workflow CLI should run");
    assert!(
        covered_output.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&covered_output.stderr)
    );
}
