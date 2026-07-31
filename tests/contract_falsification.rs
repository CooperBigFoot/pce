#[allow(dead_code)]
mod support;

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use support::CliHarness;

static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct ProbeGuard(PathBuf);

impl Drop for ProbeGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

struct DirectoryGuard(PathBuf);

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct PermissionGuard {
    path: PathBuf,
    mode: u32,
}

impl Drop for PermissionGuard {
    fn drop(&mut self) {
        if let Ok(metadata) = fs::metadata(&self.path) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(self.mode);
            let _ = fs::set_permissions(&self.path, permissions);
        }
    }
}

struct SeatbeltFixture {
    harness: CliHarness,
    repository_root: PathBuf,
    contract_path: PathBuf,
    invocation_log: PathBuf,
    probe_path: PathBuf,
    _probe_guard: ProbeGuard,
    _repository_guard: DirectoryGuard,
}

fn gate_contract(format_command: &str) -> Vec<u8> {
    format!(
        r#"{{
  "stated": {{
    "gates": {{
      "format": "{format_command}",
      "lint": "true",
      "typecheck": "true",
      "test": "true",
      "build": "true"
    }},
    "version_policy": "NONE",
    "branches": {{
      "default": "main",
      "milestone": "pce/{{vision}}/milestone-{{milestone}}",
      "step": "pce/{{vision}}/m{{milestone}}-s{{step}}"
    }},
    "pull_requests": {{
      "step_base": "MILESTONE",
      "milestone_base": "DEFAULT",
      "merge_method": "SQUASH"
    }},
    "workflows": []
  }},
  "appendable": {{
    "environment_hazards": [],
    "gate_orderings": [],
    "lockfile_rules": []
  }}
}}"#
    )
    .into_bytes()
}

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

fn seatbelt_fixture() -> SeatbeltFixture {
    let harness = CliHarness::new().expect("CLI harness should create");
    let workspace_root =
        fs::canonicalize(std::env::current_dir().expect("test current directory should read"))
            .expect("test workspace should canonicalize");
    let sequence = PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let repository_root = workspace_root.join(format!(
        "target/pce-seatbelt-repositories/repo-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&repository_root).expect("repository fixture should create");
    let repository_root =
        fs::canonicalize(repository_root).expect("repository should canonicalize");
    let repository_guard = DirectoryGuard(repository_root.clone());

    let probe_parent = workspace_root.join("target/pce-seatbelt-probes");
    fs::create_dir_all(&probe_parent).expect("probe parent should create");
    let probe_parent = fs::canonicalize(probe_parent).expect("probe parent should canonicalize");
    let probe_path = probe_parent.join(format!("probe-{}-{}", std::process::id(), sequence));
    let canonical_temp = fs::canonicalize("/tmp").expect("pce env-clear temp should canonicalize");
    let host_temp = fs::canonicalize(std::env::temp_dir()).expect("host temp should canonicalize");
    assert!(!probe_path.starts_with(&repository_root));
    assert!(!probe_path.starts_with(&canonical_temp));
    assert!(!probe_path.starts_with(&host_temp));
    assert!(!repository_root.starts_with(&canonical_temp));
    assert!(!repository_root.starts_with(&host_temp));
    assert!(!repository_root.starts_with(&probe_parent));
    fs::write(&probe_path, b"host-writable").expect("host probe write should succeed");
    fs::remove_file(&probe_path).expect("host probe cleanup should succeed");
    let probe_guard = ProbeGuard(probe_path.clone());

    let invocation_log = repository_root.join("uv-invocations.bin");
    let source = format!(
        r#"#!/bin/sh
{{ printf 'uv\0%s\0' "$#"; printf '%s\0' "$@"; }} >> '{}' || exit 74
if [ "${{ANTHROPIC_API_KEY+x}}" = x ]; then
    exit 75
fi
if printf probe > '{}'; then
    exit 0
fi
exit 73
"#,
        invocation_log.display(),
        probe_path.display()
    );
    harness
        .install_shim("uv", &source)
        .expect("uv shim should install");
    let contract_path = harness.path().join("seatbelt-contract.json");
    fs::write(&contract_path, gate_contract("uv build")).expect("contract fixture should write");

    SeatbeltFixture {
        harness,
        repository_root,
        contract_path,
        invocation_log,
        probe_path,
        _probe_guard: probe_guard,
        _repository_guard: repository_guard,
    }
}

fn run_seatbelt_fixture(fixture: &SeatbeltFixture) -> std::process::Output {
    fixture
        .harness
        .run_with_parent_environment(
            contract_check_args(&fixture.contract_path, &fixture.repository_root),
            b"",
            "ANTHROPIC_API_KEY",
            "must-not-reach-gate",
        )
        .expect("contract check should run")
}

fn assert_uv_status_and_invocation(fixture: &SeatbeltFixture, output: std::process::Output) {
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("stated gate command `uv build` exited with status 73"),
        "stderr was: {stderr}"
    );
    assert!(!output.status.success());
    assert_eq!(
        fs::read(&fixture.invocation_log).expect("uv invocation log should read"),
        b"uv\0\x31\0build\0"
    );
}

#[test]
fn contract_check_executes_gate_inside_seatbelt_boundary() {
    let fixture = seatbelt_fixture();
    let output = run_seatbelt_fixture(&fixture);
    assert!(
        !fixture.probe_path.exists(),
        "out-of-boundary probe must be absent immediately after contract check"
    );
    assert_uv_status_and_invocation(&fixture, output);
}

#[test]
fn seatbelt_profile_permits_repository_write_and_denies_external_probe() {
    let fixture = seatbelt_fixture();
    let output = run_seatbelt_fixture(&fixture);
    assert_uv_status_and_invocation(&fixture, output);
}

#[test]
fn uv_fixture_distinguishes_log_failure_from_probe_denial() {
    let fixture = seatbelt_fixture();
    fs::write(&fixture.invocation_log, []).expect("uv log should pre-create");
    let mut permissions = fs::metadata(&fixture.invocation_log)
        .expect("uv log metadata should read")
        .permissions();
    let original_mode = permissions.mode();
    let _permission_guard = PermissionGuard {
        path: fixture.invocation_log.clone(),
        mode: original_mode,
    };
    permissions.set_mode(0o444);
    fs::set_permissions(&fixture.invocation_log, permissions)
        .expect("uv log should become readonly");

    let output = run_seatbelt_fixture(&fixture);

    assert!(
        !fixture.probe_path.exists(),
        "probe must remain absent when logging fails"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains("stated gate command `uv build` exited with status 74"),
        "logging failure must retain dedicated status; stderr was: {stderr}"
    );
    assert!(!stderr.contains("exited with status 73"));
}

#[test]
fn contract_check_rejects_non_zero_base_gate_with_command_and_status() {
    let harness = CliHarness::new().expect("CLI harness should create");
    let repository_root = harness.path().join("repo");
    fs::create_dir(&repository_root).expect("repository fixture should create");
    let invocation_log = repository_root.join("status-17-invocations.bin");
    let shim = repository_root.join("status-17");
    let source = format!(
        "#!/bin/sh\n{{ printf 'status-17\\0%s\\0' \"$#\"; printf '%s\\0' \"$@\"; }} >> '{}' || exit 74\nexit 17\n",
        invocation_log.display()
    );
    fs::write(&shim, source).expect("status shim should write");
    let mut permissions = fs::metadata(&shim)
        .expect("status shim metadata should read")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&shim, permissions).expect("status shim should be executable");
    let command = shim.display().to_string();
    let contract_path = harness.path().join("contract.json");
    fs::write(&contract_path, gate_contract(&command)).expect("contract fixture should write");

    let output = harness
        .run(contract_check_args(&contract_path, &repository_root), b"")
        .expect("CLI should run");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(
        stderr.contains(&format!(
            "stated gate command `{command}` exited with status 17"
        )),
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
