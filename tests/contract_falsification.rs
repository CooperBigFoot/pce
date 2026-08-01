#[allow(dead_code)]
mod support;

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use pce_core::{
    AbsoluteWorkingDirectory, ObservedExitStatus, SeatbeltCapability, StdinBinding,
    classify_seatbelt_capability, seatbelt_capability_probe,
};
use support::CliHarness;

static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static SEATBELT_CAPABILITY: OnceLock<SeatbeltProbeObservation> = OnceLock::new();

const NESTED_SEATBELT_SKIP_MARKER: &str =
    "PCE_TEST_SKIP: nested Seatbelt unavailable; permissive capability probe was denied";

fn record_nested_seatbelt_skip() {
    let status = Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s\\n' \"$1\" >&2",
            "pce-test-skip",
            NESTED_SEATBELT_SKIP_MARKER,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .expect("skip marker process should spawn");
    assert!(status.success(), "skip marker process should succeed");
}

#[derive(Clone, Copy)]
struct SeatbeltProbeObservation {
    status: ObservedExitStatus,
    capability: SeatbeltCapability,
}

fn observe_seatbelt_capability() -> SeatbeltProbeObservation {
    *SEATBELT_CAPABILITY.get_or_init(|| {
        let current_directory =
            fs::canonicalize(std::env::current_dir().expect("test current directory should read"))
                .expect("test current directory should canonicalize");
        let working_directory = AbsoluteWorkingDirectory::parse(current_directory)
            .expect("canonical test directory should be absolute");
        let envelope = seatbelt_capability_probe(working_directory)
            .expect("fixed Seatbelt capability probe should construct");
        assert_eq!(envelope.stdin(), &StdinBinding::Null);

        let output = Command::new(envelope.executable().as_str())
            .args(envelope.arguments().as_slice())
            .current_dir(envelope.working_directory().as_path())
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .expect("Seatbelt capability probe should spawn");
        let code = output
            .status
            .code()
            .expect("Seatbelt capability probe should return an exit-status code");
        let status = ObservedExitStatus::from_code(code);
        SeatbeltProbeObservation {
            status,
            capability: classify_seatbelt_capability(status),
        }
    })
}

fn skip_without_nested_seatbelt() -> bool {
    if let SeatbeltCapability::Unavailable { .. } = observe_seatbelt_capability().capability {
        record_nested_seatbelt_skip();
        true
    } else {
        false
    }
}

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

fn run_seatbelt_fixture_inside_seatbelt(fixture: &SeatbeltFixture) -> std::process::Output {
    Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "--"])
        .arg(env!("CARGO_BIN_EXE_pce"))
        .args(contract_check_args(
            &fixture.contract_path,
            &fixture.repository_root,
        ))
        .env_clear()
        .env("PATH", fixture.harness.shim_path())
        .output()
        .expect("outer Seatbelt process should run")
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
    if skip_without_nested_seatbelt() {
        return;
    }
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
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = seatbelt_fixture();
    let output = run_seatbelt_fixture(&fixture);
    assert_uv_status_and_invocation(&fixture, output);
}

#[test]
fn seatbelt_apply_failure_is_execution_failure_not_gate_status() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = seatbelt_fixture();
    let output = run_seatbelt_fixture_inside_seatbelt(&fixture);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert!(!output.status.success());
    assert!(
        stderr.contains("failed to verify Seatbelt execution capability"),
        "stderr was: {stderr}"
    );
    assert!(
        !stderr.contains("stated gate command `uv build` exited with status"),
        "Seatbelt failure must not be attributed to the tracked command; stderr was: {stderr}"
    );
    assert!(
        !fixture.invocation_log.exists(),
        "tracked gate must not run after capability failure"
    );
}

#[test]
fn seatbelt_probe_success_preserves_legitimate_gate_status_71() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let harness = CliHarness::new().expect("CLI harness should create");
    let repository_root = harness.path().join("repo");
    fs::create_dir(&repository_root).expect("repository fixture should create");
    let invocation_log = repository_root.join("status-71-invocations.bin");
    let shim = repository_root.join("status-71");
    let source = format!(
        "#!/bin/sh\nprintf invoked > '{}' || exit 74\nexit 71\n",
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
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");

    assert!(!output.status.success());
    assert!(
        stderr.contains(&format!(
            "stated gate command `{command}` exited with status 71"
        )),
        "stderr was: {stderr}"
    );
    assert_eq!(
        fs::read(invocation_log).expect("status-71 invocation log should read"),
        b"invoked"
    );
}

#[test]
fn uv_fixture_distinguishes_log_failure_from_probe_denial() {
    if skip_without_nested_seatbelt() {
        return;
    }
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
    if skip_without_nested_seatbelt() {
        return;
    }
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
    if skip_without_nested_seatbelt() {
        return;
    }
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

#[test]
fn host_permissive_probe_reports_nested_seatbelt_available() {
    let direct_status = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "--", "/usr/bin/true"])
        .env_clear()
        .stdin(Stdio::null())
        .status()
        .expect("direct permissive Seatbelt probe should spawn");
    if !direct_status.success() {
        record_nested_seatbelt_skip();
        return;
    }

    let observation = observe_seatbelt_capability();
    assert_eq!(
        observation.capability,
        SeatbeltCapability::Available,
        "the shared probe returned status {} after the independent direct probe succeeded",
        observation.status.code()
    );
    assert!(
        !skip_without_nested_seatbelt(),
        "the shared skip decision must remain false when the independent direct probe succeeds"
    );
}

#[test]
fn nested_seatbelt_skip_marker_survives_libtest_capture() {
    if skip_without_nested_seatbelt() {
        return;
    }

    let test_executable = std::env::current_exe().expect("test executable path should read");
    let output = Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "--"])
        .arg(test_executable)
        .args([
            "--exact",
            "contract_check_executes_gate_inside_seatbelt_boundary",
        ])
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .expect("captured nested-Seatbelt test should spawn");

    assert!(
        output.status.success(),
        "captured nested-Seatbelt test failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("test stderr should be UTF-8");
    assert!(
        stderr.contains(NESTED_SEATBELT_SKIP_MARKER),
        "uncaptured test stderr omitted the skip marker: {stderr}"
    );
}
