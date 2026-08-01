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
    AbsoluteWorkingDirectory, EventBodyRef, KnownPayload, NESTED_SEATBELT_SKIP_MARKER,
    ObservedExitStatus, SeatbeltCapability, StdinBinding, classify_seatbelt_capability,
    parse_event_line, seatbelt_capability_probe,
};
use support::{
    CliHarness, WorkspaceFixtureDirectory, record_nested_seatbelt_skip,
    skip_without_nested_seatbelt as shared_seatbelt_skip,
};

static PROBE_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static SEATBELT_CAPABILITY: OnceLock<SeatbeltProbeObservation> = OnceLock::new();

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
    shared_seatbelt_skip()
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

struct LifecycleFixture {
    _workspace: WorkspaceFixtureDirectory,
    repository: PathBuf,
    events: PathBuf,
    invocation_log: PathBuf,
    probe_path: PathBuf,
    shim_directory: PathBuf,
}

fn lifecycle_fixture() -> LifecycleFixture {
    let workspace = WorkspaceFixtureDirectory::create("lifecycle")
        .expect("workspace lifecycle fixture should create");
    let repository = workspace.path().join("repository");
    let probe_path = workspace.path().join("external-probe");
    let shim_directory = repository.join("shims");
    fs::create_dir_all(&shim_directory).expect("shim directory should create");
    let output = Command::new("git")
        .args(["init", "-b", "main"])
        .arg(&repository)
        .output()
        .expect("git init should run");
    assert!(output.status.success());
    git_success(&repository, &["config", "user.name", "PCE Test"]);
    git_success(
        &repository,
        &["config", "user.email", "pce-test@example.invalid"],
    );
    fs::write(
        repository.join("Cargo.toml"),
        b"[workspace]\nmembers = []\n",
    )
    .expect("manifest should write");
    git_success(&repository, &["add", "."]);
    git_success(&repository, &["commit", "-m", "fixture"]);
    git_success(
        &repository,
        &[
            "symbolic-ref",
            "refs/remotes/origin/HEAD",
            "refs/remotes/origin/main",
        ],
    );
    workspace.assert_outside_temporary_roots(&repository);
    let canonical_temp = fs::canonicalize(std::env::temp_dir()).expect("temp should canonicalize");
    let canonical_tmp = fs::canonicalize("/tmp").expect("/tmp should canonicalize");
    assert!(!probe_path.starts_with(&repository));
    assert!(!probe_path.starts_with(canonical_temp));
    assert!(!probe_path.starts_with(canonical_tmp));
    fs::write(&probe_path, b"host-writable").expect("probe should be host writable");
    fs::remove_file(&probe_path).expect("probe should reset");
    let invocation_log = repository.join("uv-invocations.bin");
    let uv = shim_directory.join("uv");
    fs::write(
        &uv,
        format!(
            "#!/bin/sh\n{{ printf 'uv\\0%s\\0' \"$#\"; printf '%s\\0' \"$@\"; }} >> '{}' || exit 74\nif printf probe > '{}'; then exit 0; fi\nexit 73\n",
            invocation_log.display(),
            probe_path.display()
        ),
    )
    .expect("uv shim should write");
    let mut permissions = fs::metadata(&uv)
        .expect("uv metadata should read")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(uv, permissions).expect("uv shim should be executable");
    let events = workspace.path().join("events.jsonl");
    fs::write(&events, []).expect("event log should create");
    LifecycleFixture {
        _workspace: workspace,
        repository,
        events,
        invocation_log,
        probe_path,
        shim_directory,
    }
}

fn git_success(repository: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn lifecycle_path(fixture: &LifecycleFixture) -> std::ffi::OsString {
    let parent = std::env::var_os("PATH").expect("test PATH should exist");
    std::env::join_paths(
        std::iter::once(fixture.shim_directory.clone()).chain(std::env::split_paths(&parent)),
    )
    .expect("lifecycle PATH should join")
}

fn invoke_bootstrap(fixture: &LifecycleFixture) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["contract", "bootstrap", "--file"])
        .arg(&fixture.events)
        .arg("--repo-root")
        .arg(&fixture.repository)
        .args(["--repository", "fixture", "--node", "m5-s2"])
        .env("PATH", lifecycle_path(fixture))
        .output()
        .expect("bootstrap should run")
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
fn bootstrap_executes_uv_build_inside_seatbelt_boundary() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let fixture = lifecycle_fixture();
    fs::create_dir_all(fixture.repository.join(".pce")).expect("contract directory should create");
    fs::write(
        fixture.repository.join(".pce/repository-contract.json"),
        gate_contract("uv build"),
    )
    .expect("tracked contract should write");

    let output = invoke_bootstrap(&fixture);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(!output.status.success());
    assert!(
        stderr.contains("stated gate command `uv build` exited with status 73"),
        "stderr was: {stderr}"
    );
    assert_eq!(
        fs::read(&fixture.invocation_log).expect("invocation log should read"),
        b"uv\0\x31\0build\0"
    );
    assert!(!fixture.probe_path.exists());
}

fn prepare_refresh_fixture(absolute: bool) -> (LifecycleFixture, String) {
    let fixture = lifecycle_fixture();
    fs::create_dir_all(fixture.repository.join(".pce")).expect("contract directory should create");
    fs::write(
        fixture.repository.join(".pce/repository-contract.json"),
        gate_contract("true"),
    )
    .expect("initial tracked contract should write");
    let bootstrap = invoke_bootstrap(&fixture);
    assert!(
        bootstrap.status.success(),
        "{}",
        String::from_utf8_lossy(&bootstrap.stderr)
    );
    let command = if absolute {
        fixture.shim_directory.join("uv").display().to_string() + " build"
    } else {
        "uv build".to_owned()
    };
    fs::write(
        fixture.repository.join(".pce/repository-contract.json"),
        gate_contract(&command),
    )
    .expect("refreshed tracked contract should write");
    git_success(
        &fixture.repository,
        &["add", ".pce/repository-contract.json"],
    );
    git_success(&fixture.repository, &["commit", "-m", "tracked contract"]);
    fs::write(&fixture.invocation_log, []).expect("invocation log should reset");
    (fixture, command)
}

fn invoke_refresh_fixture(fixture: &LifecycleFixture) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["contract", "refresh", "--file"])
        .arg(&fixture.events)
        .arg("--repo-root")
        .arg(&fixture.repository)
        .args(["--node", "m5-s2"])
        .env("PATH", lifecycle_path(fixture))
        .output()
        .expect("refresh should run")
}

#[test]
fn refresh_executes_uv_build_inside_seatbelt_boundary() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let (fixture, command) = prepare_refresh_fixture(false);
    let output = invoke_refresh_fixture(&fixture);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(!output.status.success());
    assert!(
        stderr.contains(&format!(
            "stated gate command `{command}` exited with status 73"
        )),
        "stderr was: {stderr}"
    );
    assert_eq!(
        fs::read(&fixture.invocation_log).expect("invocation log should read"),
        b"uv\0\x31\0build\0"
    );
    assert!(!fixture.probe_path.exists());
}

#[test]
fn refresh_absolute_gate_stays_inside_seatbelt_boundary() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let (fixture, command) = prepare_refresh_fixture(true);
    let output = invoke_refresh_fixture(&fixture);
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(!output.status.success());
    assert!(
        stderr.contains(&format!(
            "stated gate command `{command}` exited with status 73"
        )),
        "stderr was: {stderr}"
    );
    assert_eq!(
        fs::read(&fixture.invocation_log).expect("invocation log should read"),
        b"uv\0\x31\0build\0"
    );
    assert!(!fixture.probe_path.exists());
}

#[test]
fn refresh_measurement_failure_leaves_contract_unchanged() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let (fixture, _command) = prepare_refresh_fixture(false);
    let contract_path = fixture.repository.join(".pce/repository-contract.json");
    let before = fs::read(&contract_path).expect("tracked baseline should read");
    let output = invoke_refresh_fixture(&fixture);
    assert!(!output.status.success());
    assert_eq!(
        fs::read(contract_path).expect("tracked contract should read"),
        before
    );
}

#[test]
fn refresh_measurement_failure_leaves_event_unchanged() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let (fixture, _command) = prepare_refresh_fixture(false);
    let before = fs::read(&fixture.events).expect("event baseline should read");
    let output = invoke_refresh_fixture(&fixture);
    assert!(!output.status.success());
    assert_eq!(
        fs::read(&fixture.events).expect("event log should read"),
        before
    );
}

#[test]
fn successful_sandboxed_refresh_preserves_stated_and_appends_event() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let (fixture, _old_command) = prepare_refresh_fixture(false);
    let success_log = fixture.repository.join("successful-refresh.log");
    let success_gate = fixture.repository.join("shims/success-gate");
    fs::write(
        &success_gate,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}' || exit 74\nexit 0\n",
            success_log.display()
        ),
    )
    .expect("success gate should write");
    let mut permissions = fs::metadata(&success_gate)
        .expect("success gate metadata should read")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&success_gate, permissions).expect("success gate should be executable");
    let command = format!("{} test-argument", success_gate.display());
    let stated = gate_contract(&command);
    fs::write(
        fixture.repository.join(".pce/repository-contract.json"),
        &stated,
    )
    .expect("successful refreshed contract should write");
    git_success(
        &fixture.repository,
        &["add", ".pce/repository-contract.json", "shims/success-gate"],
    );
    git_success(&fixture.repository, &["commit", "-m", "successful refresh"]);
    let events_before = fs::read_to_string(&fixture.events)
        .expect("event baseline should read")
        .lines()
        .count();

    let output = invoke_refresh_fixture(&fixture);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(success_log).expect("success invocation should read"),
        "test-argument\n"
    );
    let event_text = fs::read_to_string(&fixture.events).expect("event log should read");
    assert_eq!(event_text.lines().count(), events_before + 1);
    let last = parse_event_line(
        event_text
            .lines()
            .last()
            .expect("latest event should exist"),
    )
    .expect("latest event should parse");
    let EventBodyRef::Known(KnownPayload::RepositoryContract(payload)) = last.body_ref() else {
        panic!("latest event should be a repository contract");
    };
    assert_eq!(payload.stated.format, command);
    assert_eq!(payload.stated.lint, "true");
    assert_eq!(payload.stated.typecheck, "true");
    assert_eq!(payload.stated.test, "true");
    assert_eq!(payload.stated.build, "true");
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
fn contract_check_gate_child_environment_forwards_only_runtime_allowlist() {
    if skip_without_nested_seatbelt() {
        return;
    }
    let harness = CliHarness::new().expect("CLI harness should create");
    let repository_root = harness.path().join("repo");
    fs::create_dir(&repository_root).expect("repository fixture should create");
    let observed = repository_root.join("child-environment");
    let shim = repository_root.join("environment-gate");
    fs::write(
        &shim,
        format!(
            "#!/bin/sh\nenv | sed 's/=.*//' | sort -u | grep -v -E '^(PWD|SHLVL|_)$' > '{}' || exit 74\nwhile IFS= read -r name; do\n  case \"$name\" in PATH|HOME|CARGO_HOME|RUSTUP_HOME) ;; *) exit 75 ;; esac\ndone < '{}'\n",
            observed.display(),
            observed.display()
        ),
    )
    .expect("environment shim should write");
    let mut permissions = fs::metadata(&shim)
        .expect("environment shim metadata should read")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&shim, permissions).expect("environment shim should be executable");
    let contract_path = harness.path().join("contract.json");
    fs::write(&contract_path, gate_contract(&shim.display().to_string()))
        .expect("contract fixture should write");

    let output = harness
        .run(contract_check_args(&contract_path, &repository_root), b"")
        .expect("CLI should run");
    assert!(
        output.status.success(),
        "stderr was: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let names = fs::read_to_string(observed).expect("observed environment should read");
    assert!(
        names
            .lines()
            .all(|name| matches!(name, "PATH" | "HOME" | "CARGO_HOME" | "RUSTUP_HOME")),
        "unexpected child environment: {names:?}"
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
    if direct_status.code() == Some(71) {
        record_nested_seatbelt_skip();
        return;
    }
    assert!(
        direct_status.success(),
        "direct permissive probe failed with unexpected status {:?}",
        direct_status.code()
    );

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
