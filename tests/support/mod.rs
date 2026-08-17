use std::collections::{BTreeSet, HashSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use pce_core::{
    AbsoluteWorkingDirectory, NESTED_SEATBELT_SKIP_MARKER, ObservedExitStatus,
    RecordedProcessIdentity, SeatbeltCapability, StdinBinding, classify_seatbelt_capability,
    parse_dispatch_process_identity, seatbelt_capability_probe,
};
use tempfile::TempDir;

static SEATBELT_CAPABILITY: OnceLock<SeatbeltCapability> = OnceLock::new();
static WORKSPACE_FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub struct WorkspaceFixtureDirectory(PathBuf);

#[allow(dead_code)]
impl WorkspaceFixtureDirectory {
    pub fn create(label: &str) -> io::Result<Self> {
        let sequence = WORKSPACE_FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target/pce-seatbelt-fixtures")
            .join(format!("{label}-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn assert_outside_temporary_roots(&self, repository: &Path) {
        let repository = fs::canonicalize(repository).expect("repository should canonicalize");
        let canonical_tmp = fs::canonicalize("/tmp").expect("/tmp should canonicalize");
        let canonical_platform_temp =
            fs::canonicalize(std::env::temp_dir()).expect("platform temp should canonicalize");
        assert!(!repository.starts_with(canonical_tmp));
        assert!(!repository.starts_with(canonical_platform_temp));
    }
}

impl Drop for WorkspaceFixtureDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Probe whether this process may apply nested Seatbelt and record the exact skip marker on fd 2.
#[allow(dead_code)]
pub fn skip_without_nested_seatbelt() -> bool {
    let capability = *SEATBELT_CAPABILITY.get_or_init(|| {
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
        let status = ObservedExitStatus::from_code(
            output
                .status
                .code()
                .expect("Seatbelt capability probe should return an exit-status code"),
        );
        classify_seatbelt_capability(status)
    });
    if let SeatbeltCapability::Unavailable { .. } = capability {
        record_nested_seatbelt_skip();
        true
    } else {
        false
    }
}

#[allow(dead_code)]
pub fn record_nested_seatbelt_skip() {
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

const GIT_SHIM: &str = r#"#!/bin/sh
program=git
root=${PCE_SHIM_ROOT:?PCE_SHIM_ROOT is required}
request="$root/request.$$.bin"
trap 'rm -f "$request"' 0 1 2 3 15
{
    printf '%s\0' "$program" "$#"
    printf '%s\0' "$@"
} > "$request" || exit 126
cat "$request" >> "$root/invocations.bin" || exit 126
for entry in "$root"/responses/*; do
    if cmp -s "$request" "$entry/request.bin"; then
        if ! IFS= read -r exit_code < "$entry/exit_code"; then
            printf 'configured response has no exit code: %s\n' "$entry" >&2
            exit 126
        fi
        cat "$entry/stdout.bin" || exit 126
        cat "$entry/stderr.bin" >&2 || exit 126
        exit "$exit_code"
    fi
done
printf 'unconfigured git invocation' >&2
for argument in "$@"; do
    printf ' <%s>' "$argument" >&2
done
printf '\n' >&2
exit 127
"#;

const GH_SHIM: &str = r#"#!/bin/sh
program=gh
root=${PCE_SHIM_ROOT:?PCE_SHIM_ROOT is required}
request="$root/request.$$.bin"
trap 'rm -f "$request"' 0 1 2 3 15
{
    printf '%s\0' "$program" "$#"
    printf '%s\0' "$@"
} > "$request" || exit 126
cat "$request" >> "$root/invocations.bin" || exit 126
for entry in "$root"/responses/*; do
    if cmp -s "$request" "$entry/request.bin"; then
        if ! IFS= read -r exit_code < "$entry/exit_code"; then
            printf 'configured response has no exit code: %s\n' "$entry" >&2
            exit 126
        fi
        cat "$entry/stdout.bin" || exit 126
        cat "$entry/stderr.bin" >&2 || exit 126
        exit "$exit_code"
    fi
done
printf 'unconfigured gh invocation' >&2
for argument in "$@"; do
    printf ' <%s>' "$argument" >&2
done
printf '\n' >&2
exit 127
"#;

#[allow(dead_code)]
const CODEX_SHIM: &str = r#"#!/bin/sh
program=codex
root=${PCE_CODEX_RECORD_ROOT:?PCE_CODEX_RECORD_ROOT is required}
if [ "${PCE_CODEX_SKIP_RECORDING:-}" = 1 ]; then
    while IFS= read -r line; do printf '%s\n' "$line"; done < "$PCE_CODEX_STDOUT_FILE"
    exit "$PCE_CODEX_EXIT_CODE"
fi
mkdir "$root/invocation" || exit 126
{
    printf '%s\0' "$program" "$#"
    printf '%s\0' "$@"
} > "$root/invocation/request.bin" || exit 126
pwd -P > "$root/invocation/cwd.bin" || exit 126
/usr/bin/env -0 > "$root/invocation/environment.bin" || exit 126
[ -d "${TMPDIR:-}" ] || exit 126
: > "$TMPDIR/pce-child-probe" || exit 126
cat > "$root/invocation/stdin.bin" || exit 126
printf '%s\n' "$$" > "$root/invocation/pid" || exit 126
if [ -n "${PCE_CODEX_BLOCK_FILE:-}" ]; then
    polls=0
    while [ ! -e "$PCE_CODEX_BLOCK_FILE" ]; do
        polls=$((polls + 1))
        if [ "$polls" -ge "${PCE_SHIM_BLOCK_MAX_POLLS:-1200}" ]; then
            printf '%s\n' 'Codex shim block-file wait reached its iteration ceiling' >&2
            exit 124
        fi
        sleep "${PCE_SHIM_BLOCK_POLL_SECONDS:-0.25}"
    done
fi
if [ -n "${PCE_CODEX_SLEEP_SECONDS:-}" ]; then sleep "$PCE_CODEX_SLEEP_SECONDS"; fi
if [ -n "${PCE_CODEX_OUTPUT_BYTES_FILE:-}" ]; then
    output_path=
    previous=
    for argument in "$@"; do
        if [ "$previous" = -o ]; then output_path=$argument; break; fi
        previous=$argument
    done
    [ -n "$output_path" ] || exit 126
    cat "$PCE_CODEX_OUTPUT_BYTES_FILE" > "$output_path" || exit 126
fi
cat "$PCE_CODEX_STDOUT_FILE" || exit 126
cat "$PCE_CODEX_STDERR_FILE" >&2 || exit 126
if [ -n "${PCE_CODEX_SIGNAL:-}" ]; then kill -"$PCE_CODEX_SIGNAL" "$$"; fi
exit "$PCE_CODEX_EXIT_CODE"
"#;

#[allow(dead_code)]
const CLAUDE_SHIM: &str = r#"#!/bin/sh
program=claude
root=${PCE_CLAUDE_RECORD_ROOT:?PCE_CLAUDE_RECORD_ROOT is required}
mkdir "$root/invocation" || exit 126
{
    printf '%s\0' "$program" "$#"
    printf '%s\0' "$@"
} > "$root/invocation/request.bin" || exit 126
pwd -P > "$root/invocation/cwd.bin" || exit 126
/usr/bin/env -0 > "$root/invocation/environment.bin" || exit 126
[ -d "${TMPDIR:-}" ] || exit 126
: > "$TMPDIR/pce-child-probe" || exit 126
cat > "$root/invocation/stdin.bin" || exit 126
printf '%s\n' "$$" > "$root/invocation/pid" || exit 126
if [ -n "${PCE_CLAUDE_GATE_EXEC_REQUEST_DIR:-}" ] || [ -n "${PCE_CLAUDE_GATE_EXEC_RESPONSE_DIR:-}" ]; then
    if [ -z "${PCE_CLAUDE_GATE_EXEC_REQUEST_DIR:-}" ] || [ -z "${PCE_CLAUDE_GATE_EXEC_RESPONSE_DIR:-}" ]; then
        printf '%s\n' 'gate exec request and response directories must be supplied together' >&2
        exit 126
    fi
    for request in "$PCE_CLAUDE_GATE_EXEC_REQUEST_DIR"/*.json; do
        [ -f "$request" ] || continue
        basename=${request##*/}
        "$PCE_GATE_EXEC_CLIENT" gate exec < "$request" > "$PCE_CLAUDE_GATE_EXEC_RESPONSE_DIR/$basename" || exit 126
    done
fi
if [ "${PCE_CLAUDE_GATE_EXEC_RACE_EVIDENCE:-}" = 1 ]; then
    printf '%s' 'critic-authored' > "$PCE_CLAUDE_OUTPUT_PATH.executions.json" || exit 126
fi
if [ -n "${PCE_CLAUDE_BLOCK_FILE:-}" ]; then
    polls=0
    while [ ! -e "$PCE_CLAUDE_BLOCK_FILE" ]; do
        polls=$((polls + 1))
        if [ "$polls" -ge "${PCE_SHIM_BLOCK_MAX_POLLS:-1200}" ]; then
            printf '%s\n' 'Claude shim block-file wait reached its iteration ceiling' >&2
            exit 124
        fi
        sleep "${PCE_SHIM_BLOCK_POLL_SECONDS:-0.25}"
    done
fi
if [ -n "${PCE_CLAUDE_SLEEP_SECONDS:-}" ]; then sleep "$PCE_CLAUDE_SLEEP_SECONDS"; fi
if [ -n "${PCE_CLAUDE_OUTPUT_BYTES_FILE:-}" ]; then
    cat "$PCE_CLAUDE_OUTPUT_BYTES_FILE" > "$PCE_CLAUDE_OUTPUT_PATH" || exit 126
fi
cat "$PCE_CLAUDE_STDOUT_FILE" || exit 126
cat "$PCE_CLAUDE_STDERR_FILE" >&2 || exit 126
if [ -n "${PCE_CLAUDE_SIGNAL:-}" ]; then kill -"$PCE_CLAUDE_SIGNAL" "$$"; fi
exit "$PCE_CLAUDE_EXIT_CODE"
"#;

#[derive(Debug)]
pub struct ScriptedResponse {
    pub program: OsString,
    pub argv: Vec<OsString>,
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Invocation {
    pub program: OsString,
    pub argv: Vec<OsString>,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct CodexInvocation {
    pub program: OsString,
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub environment: BTreeSet<OsString>,
    pub stdin: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ClaudeInvocation {
    pub program: OsString,
    pub argv: Vec<OsString>,
    pub cwd: PathBuf,
    pub environment: BTreeSet<OsString>,
    pub stdin: Vec<u8>,
}

pub struct CliHarness {
    tempdir: TempDir,
    test_name: String,
    shim_dir: PathBuf,
    responses_dir: PathBuf,
    invocation_log: PathBuf,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecordedProcessObservation {
    Exact,
    AbsentOrReused,
}

#[cfg(target_os = "macos")]
fn observe_recorded_process(
    identity: RecordedProcessIdentity,
) -> Result<RecordedProcessObservation, String> {
    let process_number = identity.process_number().get();
    let pid = i32::try_from(process_number)
        .map_err(|_| format!("recorded process number {process_number} exceeds pid_t"))?;
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let expected_size = std::mem::size_of::<libc::proc_bsdinfo>();
    let expected_size_i32 =
        i32::try_from(expected_size).map_err(|_| "proc_bsdinfo size exceeds i32".to_owned())?;
    let observed_size = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            expected_size_i32,
        )
    };
    if observed_size == 0 {
        let error = io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(libc::ESRCH) => Ok(RecordedProcessObservation::AbsentOrReused),
            Some(libc::EPERM) => Err(format!(
                "cannot inspect recorded dispatch process {process_number}: {error}"
            )),
            _ => Err(format!(
                "failed to inspect recorded dispatch process {process_number}: {error}"
            )),
        };
    }
    if observed_size != expected_size_i32 || info.pbi_pid != process_number {
        return Err(format!(
            "proc_pidinfo returned inconsistent identity data for dispatch process {process_number}"
        ));
    }
    let expected = identity.process_start_identity();
    if info.pbi_start_tvsec == expected.seconds_since_unix_epoch()
        && u32::try_from(info.pbi_start_tvusec).ok() == Some(expected.microseconds())
    {
        Ok(RecordedProcessObservation::Exact)
    } else {
        Ok(RecordedProcessObservation::AbsentOrReused)
    }
}

#[cfg(target_os = "macos")]
fn collect_harness_process_records(
    root: &Path,
) -> Result<
    (
        Vec<(RecordedProcessIdentity, RecordedProcessIdentity)>,
        BTreeSet<u32>,
        usize,
    ),
    String,
> {
    fn visit(
        directory: &Path,
        identities: &mut Vec<(RecordedProcessIdentity, RecordedProcessIdentity)>,
        shim_processes: &mut BTreeSet<u32>,
        dispatch_issuances: &mut usize,
    ) -> Result<(), String> {
        let entries = fs::read_dir(directory)
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read an entry in {}: {error}",
                    directory.display()
                )
            })?;
            let file_type = entry.file_type().map_err(|error| {
                format!("failed to inspect {}: {error}", entry.path().display())
            })?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                visit(&path, identities, shim_processes, dispatch_issuances)?;
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let file_name = path.file_name().map(OsStr::as_bytes).unwrap_or_default();
            if file_name.ends_with(b".stderr")
                && file_name
                    .windows(b".dispatch-".len())
                    .any(|window| window == b".dispatch-")
            {
                *dispatch_issuances += 1;
            }
            let is_sidecar = path.extension() == Some(OsStr::new("json"))
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name.as_bytes().ends_with(b".dispatches"));
            if is_sidecar
                && let Ok(bytes) = fs::read(&path)
                && let Ok(identity) = parse_dispatch_process_identity(&bytes)
                && let Some(continuation) = identity.continuation_process_identity()
            {
                identities.push((continuation, identity.child_process_identity()));
            }
            let is_shim_pid = path.file_name() == Some(OsStr::new("pid"))
                && path.parent().and_then(Path::file_name) == Some(OsStr::new("invocation"));
            if is_shim_pid
                && let Ok(raw) = fs::read_to_string(&path)
                && let Ok(process_number) = raw.trim().parse::<u32>()
                && process_number > 0
            {
                shim_processes.insert(process_number);
            }
        }
        Ok(())
    }

    let mut identities = Vec::new();
    let mut shim_processes = BTreeSet::new();
    let mut dispatch_issuances = 0;
    visit(
        root,
        &mut identities,
        &mut shim_processes,
        &mut dispatch_issuances,
    )?;
    Ok((identities, shim_processes, dispatch_issuances))
}

#[cfg(target_os = "macos")]
fn process_group_exists(process_group: u32) -> Result<bool, String> {
    let pgid = i32::try_from(process_group)
        .map_err(|_| format!("process group {process_group} exceeds pid_t"))?;
    let result = unsafe { libc::kill(-pgid, 0) };
    if result == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(format!(
            "failed to probe process group {process_group}: {error}"
        )),
    }
}

#[cfg(target_os = "macos")]
fn process_exists(process_number: u32) -> Result<bool, String> {
    let pid = i32::try_from(process_number)
        .map_err(|_| format!("process number {process_number} exceeds pid_t"))?;
    let result = unsafe { libc::kill(pid, 0) };
    if result == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(format!("failed to probe process {process_number}: {error}")),
    }
}

#[cfg(target_os = "macos")]
fn process_is_direct_test_child(process_number: u32) -> Result<bool, String> {
    let pid = i32::try_from(process_number)
        .map_err(|_| format!("process number {process_number} exceeds pid_t"))?;
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>())
        .map_err(|_| "proc_bsdinfo size exceeds i32".to_owned())?;
    let observed = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            size,
        )
    };
    if observed == 0 && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        return Ok(false);
    }
    if observed != size || info.pbi_pid != process_number {
        return Err(format!(
            "failed to inspect possible agent shim process {process_number}"
        ));
    }
    Ok(info.pbi_ppid == std::process::id())
}

#[cfg(target_os = "macos")]
fn terminate_harness_dispatches(root: &Path) -> Result<(), String> {
    let publication_deadline = Instant::now() + Duration::from_secs(2);
    let (identities, shim_processes, _dispatch_issuances) = loop {
        let records = collect_harness_process_records(root)?;
        let recorded_groups = records
            .0
            .iter()
            .map(|(continuation, _child)| continuation.process_number().get())
            .collect::<BTreeSet<_>>();
        let unpublished_dispatch_exists = records.0.len() < records.2;
        let unmatched_live_shim_exists = records.1.iter().any(|process_number| {
            if !process_exists(*process_number).unwrap_or(true) {
                return false;
            }
            let Ok(pid) = i32::try_from(*process_number) else {
                return true;
            };
            let process_group = unsafe { libc::getpgid(pid) };
            process_group <= 0 || !recorded_groups.contains(&(process_group as u32))
        });
        if (!unpublished_dispatch_exists && !unmatched_live_shim_exists)
            || Instant::now() >= publication_deadline
        {
            break records;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let child_identities = identities
        .iter()
        .map(|(_continuation, child)| *child)
        .collect::<Vec<_>>();
    let mut recorded_groups = BTreeSet::new();
    let mut failures = Vec::new();
    for (identity, _child) in identities {
        let process_group = identity.process_number().get();
        match observe_recorded_process(identity) {
            Ok(RecordedProcessObservation::AbsentOrReused) => continue,
            Err(error) => {
                failures.push(error);
                continue;
            }
            Ok(RecordedProcessObservation::Exact) => {}
        }
        let pgid = match i32::try_from(process_group) {
            Ok(pgid) => pgid,
            Err(_) => {
                failures.push(format!("process group {process_group} exceeds pid_t"));
                continue;
            }
        };
        let observed_group = unsafe { libc::getpgid(pgid) };
        if observed_group != pgid {
            if observed_group == -1 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    failures.push(format!(
                        "failed to inspect process group leadership for {process_group}: {error}"
                    ));
                }
            }
            continue;
        }
        if !recorded_groups.insert(process_group) {
            continue;
        }
        if unsafe { libc::kill(-pgid, libc::SIGKILL) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                failures.push(format!(
                    "failed to kill recorded dispatch process group {process_group}: {error}"
                ));
            }
        }
    }

    let reap_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let any_group_alive = recorded_groups
            .iter()
            .any(|group| process_group_exists(*group).unwrap_or(true));
        if !any_group_alive || Instant::now() >= reap_deadline {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    for process_group in recorded_groups {
        match process_group_exists(process_group) {
            Ok(true) => failures.push(format!(
                "recorded dispatch process group {process_group} survived harness cleanup"
            )),
            Ok(false) => {}
            Err(error) => failures.push(error),
        }
    }
    for process_number in shim_processes {
        let recorded_child_is_live = child_identities
            .iter()
            .filter(|identity| identity.process_number().get() == process_number)
            .any(|identity| {
                observe_recorded_process(*identity) == Ok(RecordedProcessObservation::Exact)
            });
        match process_is_direct_test_child(process_number) {
            Ok(is_direct_child) if recorded_child_is_live || is_direct_child => failures.push(
                format!("agent shim process {process_number} is still owned by this test harness"),
            ),
            Ok(_) => {}
            Err(error) => failures.push(error),
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

impl Drop for CliHarness {
    fn drop(&mut self) {
        #[cfg(target_os = "macos")]
        if let Err(error) = terminate_harness_dispatches(self.tempdir.path()) {
            let diagnostic = format!(
                "test {} leaked a test agent process: {error}",
                self.test_name
            );
            if thread::panicking() {
                eprintln!("{diagnostic}");
                std::process::abort();
            }
            panic!("{diagnostic}");
        }
    }
}

impl CliHarness {
    pub fn new() -> io::Result<Self> {
        let tempdir = tempfile::tempdir()?;
        let shim_dir = tempdir.path().join("shims");
        let responses_dir = tempdir.path().join("responses");
        let invocation_log = tempdir.path().join("invocations.bin");
        fs::create_dir(&shim_dir)?;
        fs::create_dir(&responses_dir)?;
        fs::write(&invocation_log, [])?;
        write_shim(&shim_dir.join("git"), GIT_SHIM)?;
        write_shim(&shim_dir.join("gh"), GH_SHIM)?;
        write_shim(&shim_dir.join("codex"), CODEX_SHIM)?;
        write_shim(&shim_dir.join("claude"), CLAUDE_SHIM)?;

        let test_name = thread::current()
            .name()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("unnamed test process {}", std::process::id()));
        Ok(Self {
            tempdir,
            test_name,
            shim_dir,
            responses_dir,
            invocation_log,
        })
    }

    pub fn path(&self) -> &Path {
        self.tempdir.path()
    }

    #[allow(dead_code)]
    pub fn install_shim(&self, name: &str, source: &str) -> io::Result<PathBuf> {
        let path = self.shim_dir.join(name);
        write_shim(&path, source)?;
        Ok(path)
    }

    #[allow(dead_code)]
    pub fn shim_path(&self) -> String {
        format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", self.shim_dir.display())
    }

    pub fn materialize_responses(&self, responses: &[ScriptedResponse]) -> io::Result<()> {
        let mut requests = HashSet::new();
        for (index, response) in responses.iter().enumerate() {
            let request = encode_request(&response.program, &response.argv);
            if !requests.insert(request.clone()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "duplicate scripted response request",
                ));
            }

            let entry = self.responses_dir.join(format!("{index:04}"));
            fs::create_dir(&entry)?;
            fs::write(entry.join("request.bin"), request)?;
            fs::write(entry.join("exit_code"), format!("{}\n", response.exit_code))?;
            fs::write(entry.join("stdout.bin"), &response.stdout)?;
            fs::write(entry.join("stderr.bin"), &response.stderr)?;
        }
        Ok(())
    }

    pub fn run<I, S>(&self, argv: I, stdin: &[u8]) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", self.shim_dir.display());
        let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(argv)
            .env_clear()
            .env("PATH", path)
            .env("PCE_SHIM_ROOT", self.tempdir.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("pce child stdin was not piped"))?;
        child_stdin.write_all(stdin)?;
        drop(child_stdin);
        child.wait_with_output()
    }

    #[allow(dead_code)]
    pub fn run_with_parent_environment_and_stdin<I, S>(
        &self,
        argv: I,
        stdin: &[u8],
        name: &str,
        value: &str,
    ) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", self.shim_dir.display());
        let mut child = Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(argv)
            .env_clear()
            .env("PATH", path)
            .env("PCE_SHIM_ROOT", self.tempdir.path())
            .env(name, value)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut child_stdin = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("pce child stdin was not piped"))?;
        child_stdin.write_all(stdin)?;
        drop(child_stdin);
        child.wait_with_output()
    }

    #[allow(dead_code)]
    pub fn run_with_stdin_file<I, S>(
        &self,
        argv: I,
        stdin_path: &Path,
        inherited_marker: (&str, &str),
    ) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let stdin = fs::File::open(stdin_path)?;
        Command::new(env!("CARGO_BIN_EXE_pce"))
            .args(argv)
            .env_clear()
            .env("PATH", self.shim_path())
            .env("PCE_SHIM_ROOT", self.tempdir.path())
            .env(inherited_marker.0, inherited_marker.1)
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }

    #[allow(dead_code)]
    pub fn run_with_parent_environment<I, S>(
        &self,
        argv: I,
        parent_environment: &[(&str, &str)],
    ) -> io::Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pce"));
        command.args(argv).env_clear();
        for (name, value) in parent_environment {
            command.env(name, value);
        }
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    }

    #[allow(dead_code)]
    pub fn codex_invocations(&self, record_root: &Path) -> io::Result<Vec<CodexInvocation>> {
        let invocation = record_root.join("invocation");
        if !invocation.exists() {
            return Ok(Vec::new());
        }
        let request = fs::read(invocation.join("request.bin"))?;
        let parsed = parse_invocations(&request)?;
        let only = parsed.into_iter().next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing Codex invocation")
        })?;
        let cwd = fs::read(invocation.join("cwd.bin"))?;
        let cwd = PathBuf::from(OsString::from_vec(
            cwd.strip_suffix(b"\n").unwrap_or(&cwd).to_vec(),
        ));
        let environment = parse_nul_set(&fs::read(invocation.join("environment.bin"))?);
        let stdin = fs::read(invocation.join("stdin.bin"))?;
        Ok(vec![CodexInvocation {
            program: only.program,
            argv: only.argv,
            cwd,
            environment,
            stdin,
        }])
    }

    #[allow(dead_code)]
    pub fn claude_invocations(&self, record_root: &Path) -> io::Result<Vec<ClaudeInvocation>> {
        let invocation = record_root.join("invocation");
        if !invocation.exists() {
            return Ok(Vec::new());
        }
        let request = fs::read(invocation.join("request.bin"))?;
        let only = parse_invocations(&request)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "missing Claude invocation")
            })?;
        let cwd = fs::read(invocation.join("cwd.bin"))?;
        let cwd = PathBuf::from(OsString::from_vec(
            cwd.strip_suffix(b"\n").unwrap_or(&cwd).to_vec(),
        ));
        Ok(vec![ClaudeInvocation {
            program: only.program,
            argv: only.argv,
            cwd,
            environment: parse_nul_set(&fs::read(invocation.join("environment.bin"))?),
            stdin: fs::read(invocation.join("stdin.bin"))?,
        }])
    }

    pub fn invocations(&self) -> io::Result<Vec<Invocation>> {
        let bytes = fs::read(&self.invocation_log)?;
        parse_invocations(&bytes)
    }
}

#[allow(dead_code)]
fn parse_nul_set(bytes: &[u8]) -> BTreeSet<OsString> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| OsString::from_vec(field.to_vec()))
        .collect()
}

fn write_shim(path: &Path, source: &str) -> io::Result<()> {
    fs::write(path, source)?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

fn encode_request(program: &OsStr, argv: &[OsString]) -> Vec<u8> {
    let mut encoded = Vec::new();
    push_field(&mut encoded, program.as_bytes());
    push_field(&mut encoded, argv.len().to_string().as_bytes());
    for argument in argv {
        push_field(&mut encoded, argument.as_bytes());
    }
    encoded
}

fn push_field(encoded: &mut Vec<u8>, field: &[u8]) {
    encoded.extend_from_slice(field);
    encoded.push(0);
}

fn parse_invocations(bytes: &[u8]) -> io::Result<Vec<Invocation>> {
    let mut fields = bytes.split(|byte| *byte == 0);
    let mut invocations = Vec::new();
    loop {
        let Some(program) = fields.next() else {
            break;
        };
        if program.is_empty() {
            if fields.next().is_none() {
                break;
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invocation log contains an empty program",
            ));
        }
        let argc = fields
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing argc"))?;
        let argc = std::str::from_utf8(argc)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .parse::<usize>()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let argv = (0..argc)
            .map(|_| {
                fields
                    .next()
                    .map(|field| OsString::from_vec(field.to_vec()))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "missing invocation argument")
                    })
            })
            .collect::<io::Result<Vec<_>>>()?;
        invocations.push(Invocation {
            program: OsString::from_vec(program.to_vec()),
            argv,
        });
    }
    Ok(invocations)
}

// Every current adapter call has at least one argument. With zero arguments, the
// shim's `printf '%s\0' "$@"` would emit one stray NUL unlike this Rust encoder.
