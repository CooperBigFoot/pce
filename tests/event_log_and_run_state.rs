mod support;

#[cfg(target_os = "macos")]
use std::ffi::CString;
use std::ffi::OsString;
use std::fs;

#[cfg(target_os = "macos")]
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "macos")]
use std::os::unix::fs::FileTypeExt;
#[cfg(target_os = "macos")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::{Child, Command, Output, Stdio};
#[cfg(target_os = "macos")]
use std::sync::{Mutex, MutexGuard};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

use pce_core::{
    AcceptanceCriteria, ArtifactOutcome, CachedInputTokens, CriterionExecutionOutcome,
    DispatchExitStatus, DispatchTokenUsage, ExitCode, InputTokens, OutputTokens,
    ReasoningOutputTokens, RecoveryLogPath, RunSnapshot, VisionSlug, derive_run_state,
    parse_acceptance_criteria, parse_event_line,
};
#[cfg(target_os = "macos")]
use pce_core::{
    DispatchCompletionPayload, DispatchProcessIdentity, EventBodyRef, KnownPayload, ProcessNumber,
    ProcessStartIdentity, parse_dispatch_process_identity,
};
use serde_json::{Value, json};
use support::{CliHarness, Invocation, ScriptedResponse};

#[cfg(target_os = "macos")]
static CHECK_IN_PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(target_os = "macos")]
const CHECK_IN_SHIM: &str = r#"#!/bin/sh
printf '%s\n' "$$" > "${PCE_CHILD_PID_FILE:?PCE_CHILD_PID_FILE is required}" || exit 126
while [ ! -e "${PCE_RELEASE_FILE:?PCE_RELEASE_FILE is required}" ]; do /bin/sleep 0.01; done
if [ -n "${PCE_ARTIFACT_FILE:-}" ]; then
    printf '%s' '{"artifact":"real"}' > "$PCE_ARTIFACT_FILE" || exit 126
fi
printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":1,"cached_input_tokens":2,"output_tokens":3,"reasoning_output_tokens":4}}'
exit 0
"#;

#[cfg(target_os = "macos")]
const CHECK_IN_REPORT: &[u8] = b"{\"schema_id\":\"pce.dispatch-check-in\",\"schema_version\":1,\"dispatches\":[{\"issuance_sequence\":1,\"state\":\"finished\",\"completion\":\"recorded\",\"artifact_production\":\"produced\"},{\"issuance_sequence\":3,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":4,\"state\":\"running\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":5,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":6,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"}]}\n";

#[cfg(target_os = "macos")]
const CHECK_IN_EVENT: &[u8] = b"{\"sequence\":1,\"timestamp\":\"2026-08-09T12:00:00.000Z\",\"kind\":\"dispatch\",\"node\":\"m1-s2\",\"payload\":{\"role\":\"step-executor\",\"ref\":\"abc123\",\"evidence\":\"fixture\"}}\n";

#[cfg(target_os = "macos")]
fn check_in_test_guard() -> MutexGuard<'static, ()> {
    CHECK_IN_PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(target_os = "macos")]
struct CheckInPhase {
    pid_file: PathBuf,
    release_file: PathBuf,
    artifact_file: PathBuf,
}

#[cfg(target_os = "macos")]
fn spawn_check_in_dispatch(
    harness: &CliHarness,
    cwd: &Path,
    log_path: &Path,
    phase: &CheckInPhase,
) -> Child {
    let argv = [
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        cwd.display().to_string(),
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
        "--env".to_owned(),
        format!("PATH={}", harness.shim_path()),
        "--env".to_owned(),
        format!("PCE_CHILD_PID_FILE={}", phase.pid_file.display()),
        "--env".to_owned(),
        format!("PCE_RELEASE_FILE={}", phase.release_file.display()),
        "--env".to_owned(),
        format!("PCE_ARTIFACT_FILE={}", phase.artifact_file.display()),
        "--log-file".to_owned(),
        log_path.display().to_string(),
        "--node".to_owned(),
        "m1-s2".to_owned(),
        "--role".to_owned(),
        "step-executor".to_owned(),
        "--ref".to_owned(),
        "abc123".to_owned(),
        "--evidence".to_owned(),
        "fixture invocation".to_owned(),
        "--required-artifact".to_owned(),
        phase.artifact_file.display().to_string(),
        "--".to_owned(),
        "PROMPT".to_owned(),
    ];
    // This helper calls env_clear(), so every variable read by the shim is an explicit --env pair.
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(argv)
        .env_clear()
        .env("PATH", harness.shim_path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dispatch")
}

#[cfg(target_os = "macos")]
fn spawn_failing_check_in_dispatch(cwd: &Path, log_path: &Path, phase: &CheckInPhase) -> Child {
    let argv = [
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        cwd.display().to_string(),
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
        "--env".to_owned(),
        "PATH=/usr/bin:/bin".to_owned(),
        "--env".to_owned(),
        format!("PCE_CHILD_PID_FILE={}", phase.pid_file.display()),
        "--env".to_owned(),
        format!("PCE_RELEASE_FILE={}", phase.release_file.display()),
        "--env".to_owned(),
        format!("PCE_ARTIFACT_FILE={}", phase.artifact_file.display()),
        "--log-file".to_owned(),
        log_path.display().to_string(),
        "--node".to_owned(),
        "m1-s2".to_owned(),
        "--role".to_owned(),
        "step-executor".to_owned(),
        "--ref".to_owned(),
        "abc123".to_owned(),
        "--evidence".to_owned(),
        "fixture invocation".to_owned(),
        "--required-artifact".to_owned(),
        phase.artifact_file.display().to_string(),
        "--".to_owned(),
        "PROMPT".to_owned(),
    ];
    Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(argv)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn failing dispatch")
}

#[cfg(target_os = "macos")]
fn bounded_output(command: &mut Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bounded command");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if child.try_wait().expect("poll bounded command").is_some() {
            return child.wait_with_output().expect("collect bounded command");
        }
        if Instant::now() >= deadline {
            child.kill().expect("kill timed-out command");
            child.wait().expect("reap timed-out command");
            panic!("timed out waiting for process exit");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "macos")]
fn wait_for_check_in_path(path: &Path, failure: &str) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(Instant::now() < deadline, "{failure}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "macos")]
fn read_check_in_identity(path: &Path) -> (Vec<u8>, DispatchProcessIdentity) {
    let bytes = fs::read(path).expect("read dispatch sidecar");
    let identity = parse_dispatch_process_identity(&bytes).expect("parse dispatch sidecar");
    (bytes, identity)
}

#[cfg(target_os = "macos")]
fn observe_test_darwin_process(process_number: ProcessNumber) -> Option<ProcessStartIdentity> {
    let pid = i32::try_from(process_number.get()).expect("PID fits Darwin pid_t");
    let mut info = unsafe { std::mem::zeroed::<libc::proc_bsdinfo>() };
    let expected = std::mem::size_of::<libc::proc_bsdinfo>();
    let observed = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            i32::try_from(expected).expect("proc_bsdinfo size"),
        )
    };
    if observed == 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
        return None;
    }
    assert_eq!(observed, i32::try_from(expected).expect("size fits"));
    assert_eq!(info.pbi_pid, process_number.get());
    Some(
        ProcessStartIdentity::new(
            info.pbi_start_tvsec,
            u32::try_from(info.pbi_start_tvusec).expect("start microseconds"),
        )
        .expect("normalized start identity"),
    )
}

#[cfg(target_os = "macos")]
fn wait_for_test_process_exit(process_number: ProcessNumber) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while observe_test_darwin_process(process_number).is_some() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for process exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "macos")]
fn stop_child_and_kill_dispatch_parent(parent: &mut Child, identity: &DispatchProcessIdentity) {
    let parent_pid = i32::try_from(parent.id()).expect("parent PID");
    let child_pid = i32::try_from(identity.process_number().get()).expect("child PID");
    assert_eq!(unsafe { libc::kill(parent_pid, libc::SIGSTOP) }, 0);
    assert_eq!(unsafe { libc::kill(child_pid, libc::SIGKILL) }, 0);
    assert_eq!(unsafe { libc::kill(parent_pid, libc::SIGKILL) }, 0);
    parent.wait().expect("reap killed dispatch parent");
    wait_for_test_process_exit(identity.process_number());
}

#[cfg(target_os = "macos")]
fn wait_for_dispatch_parent(parent: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = parent.try_wait().expect("poll dispatch parent") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for process exit"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(target_os = "macos")]
struct ReusedProcessHolder {
    process_number: ProcessNumber,
    start_identity: ProcessStartIdentity,
    created_by_test: bool,
}

#[cfg(target_os = "macos")]
fn force_darwin_pid_reuse(
    target: ProcessNumber,
    recorded: ProcessStartIdentity,
) -> ReusedProcessHolder {
    let started = Instant::now();
    for _ in 0..200_000 {
        assert!(
            started.elapsed() < Duration::from_secs(15 * 60),
            "timed out forcing Darwin PID reuse"
        );
        if let Some(start_identity) = observe_test_darwin_process(target) {
            assert_ne!(start_identity, recorded);
            return ReusedProcessHolder {
                process_number: target,
                start_identity,
                created_by_test: false,
            };
        }
        let forked = unsafe { libc::fork() };
        assert!(forked >= 0, "fork failed while forcing Darwin PID reuse");
        if forked == 0 {
            if unsafe { libc::getpid() } == i32::try_from(target.get()).expect("target PID") {
                loop {
                    unsafe { libc::pause() };
                }
            }
            unsafe { libc::_exit(0) };
        }
        if u32::try_from(forked).expect("forked PID") == target.get() {
            let start_identity = loop {
                if let Some(identity) = observe_test_darwin_process(target) {
                    break identity;
                }
                std::thread::yield_now();
            };
            assert_ne!(start_identity, recorded);
            return ReusedProcessHolder {
                process_number: target,
                start_identity,
                created_by_test: true,
            };
        }
        let mut status = 0;
        assert_eq!(unsafe { libc::waitpid(forked, &mut status, 0) }, forked);
    }
    panic!("exceeded 200000 forks forcing Darwin PID reuse");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "slow: forces real Darwin PID wraparound to prove process-number reuse"]
fn production_check_in_pairs_dead_running_and_reused_process_number() {
    let _guard = check_in_test_guard();
    let harness = CliHarness::new().expect("create check-in harness");
    harness
        .install_shim("codex", CHECK_IN_SHIM)
        .expect("install exact check-in shim");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize harness path");
    let log_path = cwd.join("events.jsonl");
    let phase = |sequence| CheckInPhase {
        pid_file: cwd.join(format!("pid-{sequence}")),
        release_file: cwd.join(format!("release-{sequence}")),
        artifact_file: cwd.join(format!("artifact-{sequence}.json")),
    };
    let phase_1 = phase(1);
    let phase_3 = phase(3);
    let phase_4 = phase(4);
    let phase_5 = phase(5);
    let phase_6 = phase(6);

    fs::write(&phase_1.release_file, []).expect("create release-1");
    let phase_1_output = spawn_check_in_dispatch(&harness, &cwd, &log_path, &phase_1)
        .wait_with_output()
        .expect("complete issuance-1 dispatch");
    assert!(phase_1_output.status.success());
    assert_eq!(
        phase_1_output.stdout,
        b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":1,\"cached_input_tokens\":2,\"output_tokens\":3,\"reasoning_output_tokens\":4}}\n"
    );
    assert_eq!(phase_1_output.stderr, b"");
    assert_eq!(
        fs::read(&phase_1.artifact_file).expect("read artifact-1"),
        b"{\"artifact\":\"real\"}"
    );

    let sidecar_directory = PathBuf::from(format!("{}.dispatches", log_path.display()));
    let sidecar_1_path = sidecar_directory.join("1.json");
    wait_for_check_in_path(&phase_1.pid_file, "timed out waiting for dispatch child");
    wait_for_check_in_path(&sidecar_1_path, "timed out waiting for dispatch sidecar");
    let (sidecar_1_bytes, identity_1) = read_check_in_identity(&sidecar_1_path);
    assert_eq!(identity_1.issuance_sequence().get(), 1);

    let mut phase_3_parent = spawn_check_in_dispatch(&harness, &cwd, &log_path, &phase_3);
    let sidecar_3_path = sidecar_directory.join("3.json");
    wait_for_check_in_path(&phase_3.pid_file, "timed out waiting for dispatch child");
    wait_for_check_in_path(&sidecar_3_path, "timed out waiting for dispatch sidecar");
    let (sidecar_3_bytes, identity_3) = read_check_in_identity(&sidecar_3_path);
    assert_eq!(
        fs::read_to_string(&phase_3.pid_file)
            .expect("read pid-3")
            .trim()
            .parse::<u32>()
            .expect("parse pid-3"),
        identity_3.process_number().get()
    );
    stop_child_and_kill_dispatch_parent(&mut phase_3_parent, &identity_3);
    assert!(!phase_3.artifact_file.exists());

    let mut phase_4_parent = spawn_check_in_dispatch(&harness, &cwd, &log_path, &phase_4);
    let sidecar_4_path = sidecar_directory.join("4.json");
    wait_for_check_in_path(&phase_4.pid_file, "timed out waiting for dispatch child");
    wait_for_check_in_path(&sidecar_4_path, "timed out waiting for dispatch sidecar");
    let (sidecar_4_bytes, identity_4) = read_check_in_identity(&sidecar_4_path);
    assert_eq!(
        fs::read_to_string(&phase_4.pid_file)
            .expect("read pid-4")
            .trim()
            .parse::<u32>()
            .expect("parse pid-4"),
        identity_4.process_number().get()
    );
    assert_eq!(
        observe_test_darwin_process(identity_4.process_number()),
        Some(identity_4.process_start_identity())
    );

    let mut phase_5_parent = spawn_check_in_dispatch(&harness, &cwd, &log_path, &phase_5);
    let sidecar_5_path = sidecar_directory.join("5.json");
    wait_for_check_in_path(&phase_5.pid_file, "timed out waiting for dispatch child");
    wait_for_check_in_path(&sidecar_5_path, "timed out waiting for dispatch sidecar");
    let (sidecar_5_bytes, identity_5) = read_check_in_identity(&sidecar_5_path);
    assert_eq!(
        fs::read_to_string(&phase_5.pid_file)
            .expect("read pid-5")
            .trim()
            .parse::<u32>()
            .expect("parse pid-5"),
        identity_5.process_number().get()
    );
    stop_child_and_kill_dispatch_parent(&mut phase_5_parent, &identity_5);
    assert!(!phase_5.artifact_file.exists());

    let phase_6_output = spawn_failing_check_in_dispatch(&cwd, &log_path, &phase_6)
        .wait_with_output()
        .expect("complete issuance-6 failed dispatch");
    assert!(!phase_6_output.status.success());
    assert!(String::from_utf8_lossy(&phase_6_output.stderr).contains("failed to spawn `codex`"));
    let records_after_spawn_failure = fs::read_to_string(&log_path)
        .expect("read event log after spawn failure")
        .lines()
        .map(|line| parse_event_line(line).expect("parse event after spawn failure"))
        .collect::<Vec<_>>();
    assert_eq!(records_after_spawn_failure.len(), 6);
    assert_eq!(records_after_spawn_failure[5].sequence().get(), 6);
    assert!(matches!(
        records_after_spawn_failure[5].body_ref(),
        EventBodyRef::Known(KnownPayload::Dispatch(_))
    ));
    assert!(!sidecar_directory.join("6.json").exists());
    assert!(!phase_6.pid_file.exists());
    assert!(!phase_6.release_file.exists());
    assert!(!phase_6.artifact_file.exists());

    let holder = force_darwin_pid_reuse(
        identity_5.process_number(),
        identity_5.process_start_identity(),
    );
    assert_eq!(holder.process_number, identity_5.process_number());
    assert_ne!(holder.start_identity, identity_5.process_start_identity());

    for required in [
        &phase_1.pid_file,
        &phase_3.pid_file,
        &phase_4.pid_file,
        &phase_5.pid_file,
        &phase_1.release_file,
        &phase_1.artifact_file,
    ] {
        assert!(
            required.exists(),
            "required check-in fixture missing: {}",
            required.display()
        );
    }
    for absent in [
        &phase_3.release_file,
        &phase_4.release_file,
        &phase_5.release_file,
        &phase_6.pid_file,
        &phase_6.release_file,
        &phase_3.artifact_file,
        &phase_4.artifact_file,
        &phase_5.artifact_file,
        &phase_6.artifact_file,
    ] {
        assert!(
            !absent.exists(),
            "unexpected check-in fixture: {}",
            absent.display()
        );
    }

    let event_log_before = fs::read(&log_path).expect("read event log before check-in");
    assert_eq!(
        std::str::from_utf8(&event_log_before)
            .expect("UTF-8 event log")
            .lines()
            .count(),
        6
    );
    let check_in = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "dispatch",
            "check-in",
            "--file",
            log_path.to_str().expect("absolute UTF-8 log path"),
        ])
        .env_clear()
        .env("PATH", harness.shim_path())
        .stdin(Stdio::null())
        .output()
        .expect("run real dispatch check-in");
    assert!(
        check_in.status.success(),
        "{}",
        String::from_utf8_lossy(&check_in.stderr)
    );
    assert_eq!(check_in.stdout, CHECK_IN_REPORT);
    assert_eq!(check_in.stderr, b"");

    assert_eq!(
        fs::read(&log_path).expect("read event log after check-in"),
        event_log_before
    );
    for (path, bytes) in [
        (&sidecar_1_path, &sidecar_1_bytes),
        (&sidecar_3_path, &sidecar_3_bytes),
        (&sidecar_4_path, &sidecar_4_bytes),
        (&sidecar_5_path, &sidecar_5_bytes),
    ] {
        assert_eq!(fs::read(path).expect("reread dispatch sidecar"), *bytes);
    }
    assert_eq!(
        observe_test_darwin_process(identity_4.process_number()),
        Some(identity_4.process_start_identity())
    );
    assert_eq!(
        observe_test_darwin_process(holder.process_number),
        Some(holder.start_identity)
    );
    assert_eq!(
        std::str::from_utf8(&fs::read(&log_path).expect("reread event log"))
            .expect("UTF-8 event log")
            .lines()
            .count(),
        6
    );

    fs::write(&phase_4.release_file, []).expect("release working issuance-4 child");
    let phase_4_status = wait_for_dispatch_parent(&mut phase_4_parent);
    assert!(phase_4_status.success());
    assert_eq!(
        fs::read(&phase_4.artifact_file).expect("read artifact-4"),
        b"{\"artifact\":\"real\"}"
    );
    assert!(!phase_3.artifact_file.exists());
    assert!(!phase_5.artifact_file.exists());

    let final_log = fs::read_to_string(&log_path).expect("read completed event log");
    let final_records = final_log
        .lines()
        .map(|line| parse_event_line(line).expect("parse final event record"))
        .collect::<Vec<_>>();
    assert_eq!(final_records.len(), 7);
    assert_eq!(final_records[6].sequence().get(), 7);
    let EventBodyRef::Known(KnownPayload::DispatchCompletion(DispatchCompletionPayload {
        issuance_sequence,
        usage,
        exit_status,
        artifact_outcome,
        ..
    })) = final_records[6].body_ref()
    else {
        panic!("sequence 7 must be the real issuance-4 completion")
    };
    assert_eq!(issuance_sequence.get(), 4);
    assert_eq!(
        usage,
        &DispatchTokenUsage::Measured {
            input_tokens: pce_core::InputTokens::new(1),
            cached_input_tokens: pce_core::CachedInputTokens::new(2),
            output_tokens: pce_core::OutputTokens::new(3),
            reasoning_output_tokens: pce_core::ReasoningOutputTokens::new(4),
        }
    );
    assert_eq!(
        exit_status,
        &DispatchExitStatus::Exited {
            code: pce_core::ExitCode::new(0),
        }
    );
    assert_eq!(artifact_outcome, &ArtifactOutcome::NotValidated);
    assert!(final_records.iter().all(|record| {
        !matches!(
            record.body_ref(),
            EventBodyRef::Known(KnownPayload::DispatchCompletion(completion))
                if completion.issuance_sequence.get() == 3
                    || completion.issuance_sequence.get() == 5
                    || completion.issuance_sequence.get() == 6
        )
    }));

    if holder.created_by_test {
        assert_eq!(
            unsafe {
                libc::kill(
                    i32::try_from(holder.process_number.get()).expect("holder PID"),
                    libc::SIGKILL,
                )
            },
            0
        );
        let mut status = 0;
        assert_eq!(
            unsafe {
                libc::waitpid(
                    i32::try_from(holder.process_number.get()).expect("holder PID"),
                    &mut status,
                    0,
                )
            },
            i32::try_from(holder.process_number.get()).expect("holder PID")
        );
    }

    let fifo_directory = cwd.join("fifo-sidecar");
    fs::create_dir(&fifo_directory).expect("create FIFO scenario directory");
    let fifo_log = fifo_directory.join("events.jsonl");
    fs::write(&fifo_log, CHECK_IN_EVENT).expect("write FIFO event log");
    let fifo_sidecar_directory = PathBuf::from(format!("{}.dispatches", fifo_log.display()));
    fs::create_dir(&fifo_sidecar_directory).expect("create FIFO sidecar directory");
    let fifo_sidecar = fifo_sidecar_directory.join("1.json");
    let fifo_sidecar_c =
        CString::new(fifo_sidecar.as_os_str().as_bytes()).expect("FIFO path has no NUL");
    assert_eq!(unsafe { libc::mkfifo(fifo_sidecar_c.as_ptr(), 0o600) }, 0);
    let fifo_log_before = fs::read(&fifo_log).expect("read FIFO log before check-in");
    let fifo_output = bounded_output(
        Command::new(env!("CARGO_BIN_EXE_pce"))
            .args([
                "dispatch",
                "check-in",
                "--file",
                fifo_log.to_str().expect("UTF-8 FIFO log path"),
            ])
            .env_clear(),
    );
    assert!(!fifo_output.status.success());
    assert_eq!(fifo_output.stdout, b"");
    assert_eq!(
        fifo_output.stderr,
        b"Error: dispatch process identity sidecar is not a regular file\n"
    );
    assert_eq!(
        fs::read(&fifo_log).expect("reread FIFO log"),
        fifo_log_before
    );
    assert!(
        fs::symlink_metadata(&fifo_sidecar)
            .expect("inspect FIFO sidecar")
            .file_type()
            .is_fifo()
    );

    let non_regular_directory = cwd.join("non-regular-artifact");
    fs::create_dir(&non_regular_directory).expect("create artifact scenario directory");
    let non_regular_log = non_regular_directory.join("events.jsonl");
    fs::write(&non_regular_log, CHECK_IN_EVENT).expect("write artifact event log");
    let non_regular_sidecar_directory =
        PathBuf::from(format!("{}.dispatches", non_regular_log.display()));
    fs::create_dir(&non_regular_sidecar_directory)
        .expect("create artifact scenario sidecar directory");
    let non_regular_artifact = non_regular_directory.join("artifact-1.json");
    fs::create_dir(&non_regular_artifact).expect("create non-regular artifact");
    let sidecar_bytes = format!(
        "{{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":1,\"process_number\":999999,\"process_start_identity\":{{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456}},\"required_artifact_path\":\"{}\"}}\n",
        non_regular_artifact.display()
    );
    fs::write(non_regular_sidecar_directory.join("1.json"), sidecar_bytes)
        .expect("write artifact scenario sidecar");
    let non_regular_output = bounded_output(
        Command::new(env!("CARGO_BIN_EXE_pce"))
            .args([
                "dispatch",
                "check-in",
                "--file",
                non_regular_log
                    .to_str()
                    .expect("UTF-8 artifact scenario log path"),
            ])
            .env_clear(),
    );
    assert!(
        non_regular_output.status.success(),
        "{}",
        String::from_utf8_lossy(&non_regular_output.stderr)
    );
    assert_eq!(
        non_regular_output.stdout,
        b"{\"schema_id\":\"pce.dispatch-check-in\",\"schema_version\":1,\"dispatches\":[{\"issuance_sequence\":1,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"}]}\n"
    );
    assert_eq!(non_regular_output.stderr, b"");
}

fn ratified_floor() -> AcceptanceCriteria {
    parse_acceptance_criteria(
        r#"# Vision: fixture

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Ratified floor","input":"Run the finished thing.","observation":"It reports success."}]}
```

## Decomposition hints

None.
"#,
    )
    .expect("ratified floor fixture")
}

const VISION_DOCUMENT: &str = r#"# Vision: fixture

## Acceptance criteria (vision-level "done")

```json
{
  "criteria": [
    {
      "name": "Ratified floor",
      "input": "Run the finished thing.",
      "observation": "It reports success."
    }
  ]
}
```

## Decomposition hints

None.
"#;

fn write_vision_document(vision_dir: &std::path::Path) {
    fs::create_dir_all(vision_dir).expect("vision directory");
    fs::write(vision_dir.join("vision.md"), VISION_DOCUMENT).expect("vision fixture");
}

#[test]
fn binary_appends_reads_and_refuses_criterion_mutations() {
    let harness = CliHarness::new().expect("CLI harness");
    let log = harness.path().join("criterion-events.jsonl");
    let payload = br#"{"criterion":{"name":"New blocking criterion","input":"Run the new probe.","observation":"The probe exits 0."},"change_of_course":"Reality exposed an uncovered failure."}"#;
    let append = harness
        .run(
            [
                "log",
                "--file",
                log.to_str().expect("UTF-8 path"),
                "--kind",
                "criterion-added",
                "--node",
                "m2-s1",
            ],
            payload,
        )
        .expect("append criterion");
    assert!(append.status.success());
    assert_eq!(append.stderr, b"");

    let read = harness
        .run(
            [
                "log",
                "read",
                "--file",
                log.to_str().expect("UTF-8 path"),
                "--kind",
                "criterion-added",
            ],
            b"",
        )
        .expect("read criterion");
    assert!(read.status.success());
    assert_eq!(read.stderr, b"");
    assert!(read.stdout.ends_with(b"\n"));
    assert_eq!(read.stdout.iter().filter(|byte| **byte == b'\n').count(), 1);
    let line = std::str::from_utf8(&read.stdout)
        .expect("UTF-8 output")
        .trim_end();
    let record = parse_event_line(line).expect("criterion record");
    assert_eq!(record.sequence().get(), 1);
    assert_eq!(record.node().as_str(), "m2-s1");
    let pce_core::ReadPayload::Known(pce_core::KnownPayload::CriterionAdded(payload)) =
        record.payload()
    else {
        panic!("criterion-added payload expected");
    };
    assert_eq!(payload.criterion.name().as_str(), "New blocking criterion");
    assert_eq!(payload.criterion.input().as_str(), "Run the new probe.");
    assert_eq!(
        payload.criterion.observation().as_str(),
        "The probe exits 0."
    );
    assert_eq!(
        payload.change_of_course.as_str(),
        "Reality exposed an uncovered failure."
    );

    let before = fs::metadata(&log).expect("log metadata").len();
    for kind in [
        "criterion-removed",
        "criterion-updated",
        "criterion-weakened",
    ] {
        let refused = harness
            .run(
                [
                    "log",
                    "--file",
                    log.to_str().expect("UTF-8 path"),
                    "--kind",
                    kind,
                    "--node",
                    "m2-s1",
                ],
                b"{}",
            )
            .expect("refused mutation");
        assert_eq!(refused.status.code(), Some(1));
        assert_eq!(refused.stdout, b"");
        assert!(String::from_utf8_lossy(&refused.stderr).contains("failed to parse event kind"));
        assert_eq!(fs::metadata(&log).expect("log metadata").len(), before);
    }
}

fn current_repository_contract(root: &std::path::Path, evidence: &str) -> Value {
    json!({
        "repository": "pce",
        "repo_root": root,
        "stated": {
            "format": "cargo fmt --check",
            "lint": "cargo clippy --workspace --all-targets",
            "typecheck": "cargo check --workspace --all-targets",
            "test": "cargo test --workspace",
            "build": "cargo build --release",
            "version_policy": "NONE",
            "branch_convention": "pce/<vision-slug>/m<m>-s<s> from pce/<vision-slug>/milestone-<m>",
            "pull_request_convention": "step head targets the matching milestone integration branch"
        },
        "observations": {
            "format": 0,
            "lint": 0,
            "typecheck": 0,
            "test": 0,
            "build": 0
        },
        "workflow_map": {},
        "appendable": {
            "environment_hazards": [],
            "gate_orderings": [],
            "lockfile_rules": []
        },
        "evidence": evidence
    })
}

#[test]
fn binary_log_read_folds_the_complete_measured_lifecycle() {
    let harness = CliHarness::new().expect("create lifecycle recovery harness");
    let cwd = fs::canonicalize(harness.path()).expect("canonicalize cwd");
    let record_root = harness.path().join("lifecycle-records");
    fs::create_dir(&record_root).expect("create invocation record root");
    let stdout_path = harness.path().join("codex.stdout");
    let stderr_path = harness.path().join("codex.stderr");
    let log_path = harness.path().join("events.jsonl");
    fs::write(
        &stdout_path,
        b"{\"type\":\"turn.completed\",\"usage\":{\"input_tokens\":101,\"cached_input_tokens\":23,\"output_tokens\":17,\"reasoning_output_tokens\":5}}\n",
    )
    .expect("write Codex stdout fixture");
    fs::write(&stderr_path, []).expect("write Codex stderr fixture");
    let child_path = harness.shim_path();
    let args = vec![
        "dispatch".to_owned(),
        "codex".to_owned(),
        "--cwd".to_owned(),
        cwd.display().to_string(),
        "--sandbox".to_owned(),
        "workspace-write".to_owned(),
        "--env".to_owned(),
        format!("PATH={child_path}"),
        "--env".to_owned(),
        format!("PCE_CODEX_RECORD_ROOT={}", record_root.display()),
        "--env".to_owned(),
        format!("PCE_CODEX_STDOUT_FILE={}", stdout_path.display()),
        "--env".to_owned(),
        format!("PCE_CODEX_STDERR_FILE={}", stderr_path.display()),
        "--env".to_owned(),
        "PCE_CODEX_EXIT_CODE=0".to_owned(),
        "--env".to_owned(),
        "PCE_CODEX_SLEEP_SECONDS=0.2".to_owned(),
        "--log-file".to_owned(),
        log_path.display().to_string(),
        "--node".to_owned(),
        "m3-s1".to_owned(),
        "--role".to_owned(),
        "step-executor".to_owned(),
        "--ref".to_owned(),
        "abc123".to_owned(),
        "--evidence".to_owned(),
        "binary lifecycle fixture".to_owned(),
        "--required-artifact".to_owned(),
        harness
            .path()
            .join("lifecycle-artifact.json")
            .display()
            .to_string(),
        "--".to_owned(),
        "PROMPT".to_owned(),
    ];
    let dispatched = harness.run(&args, b"").expect("run logged dispatch");
    assert!(
        dispatched.status.success(),
        "dispatch stderr: {}",
        stderr(&dispatched)
    );

    let read = harness
        .run(
            [
                "log",
                "read",
                "--file",
                log_path.to_str().expect("UTF-8 log path"),
            ],
            b"",
        )
        .expect("read complete lifecycle log");
    assert!(read.status.success(), "log read stderr: {}", stderr(&read));
    let records = String::from_utf8(read.stdout)
        .expect("log read emits UTF-8")
        .lines()
        .map(|line| parse_event_line(line).expect("parse binary-written record"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 2);
    let raw_log = fs::read_to_string(&log_path).expect("read raw lifecycle log");
    assert_eq!(raw_log.lines().count(), 2);
    for forbidden in [
        "process_number",
        "process_start_identity",
        "required_artifact_path",
        "pce.dispatch-process-identity",
    ] {
        assert!(!raw_log.contains(forbidden), "event log leaked {forbidden}");
    }
    let vision =
        VisionSlug::parse("2026-07-31-the-binary-owns-every-dispatch").expect("parse vision");
    let state = derive_run_state(
        &records,
        &ratified_floor(),
        &vision,
        &RecoveryLogPath::new(log_path.display().to_string()),
        &[],
        &[],
        &[],
    )
    .expect("fold binary-written lifecycle");
    assert_eq!(state.dispatch_lifecycles().len(), 1);
    let lifecycle = &state.dispatch_lifecycles()[0];
    assert_eq!(lifecycle.issuance().sequence().get(), 1);
    assert_eq!(lifecycle.issuance().node().as_str(), "m3-s1");
    assert_eq!(lifecycle.issuance().role().as_str(), "step-executor");
    assert_eq!(lifecycle.issuance().dispatch_ref().as_str(), "abc123");
    assert_eq!(lifecycle.completion_sequence().get(), 2);
    assert_eq!(lifecycle.completion_timestamp(), *records[1].timestamp());
    assert!(lifecycle.duration().get() >= 100);
    assert_eq!(
        lifecycle.usage(),
        &DispatchTokenUsage::Measured {
            input_tokens: InputTokens::new(101),
            cached_input_tokens: CachedInputTokens::new(23),
            output_tokens: OutputTokens::new(17),
            reasoning_output_tokens: ReasoningOutputTokens::new(5),
        }
    );
    assert_eq!(
        lifecycle.exit_status(),
        DispatchExitStatus::Exited {
            code: ExitCode::new(0)
        }
    );
    assert_eq!(lifecycle.artifact_outcome(), ArtifactOutcome::NotValidated);
    assert_eq!(state.rounds().len(), 1);
    assert_eq!(state.rounds()[0].count().get(), 1);
    let snapshot = serde_json::to_value(RunSnapshot::from(&state)).expect("serialize snapshot");
    assert_eq!(snapshot["schema_id"], "pce.run-snapshot");
    assert_eq!(snapshot["schema_version"], 1);
    assert!(snapshot.get("dispatch_lifecycles").is_none());
}

#[test]
fn binary_records_reads_and_folds_distinct_criterion_outcomes() {
    let harness = CliHarness::new().expect("create criterion execution harness");
    let log = harness.path().join("events.jsonl");
    let payloads = [
        r#"{"criterion":{"name":"Runnable criterion","input":"Run the finished command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"passed","observed_result":"The command exited 0."},"evidence":"git rev-parse HEAD\n./finished-command"}"#,
        r#"{"criterion":{"name":"Failing criterion","input":"Run the broken command.","observation":"It exits 0."},"finished_result":"main@0123456789abcdef","outcome":{"status":"failed","observed_result":"The command exited 7."},"evidence":"git rev-parse HEAD\n./broken-command"}"#,
        r#"{"criterion":{"name":"Install-only criterion","input":"Install the hook, then attempt the forbidden command.","observation":"The command is denied."},"finished_result":"main@0123456789abcdef","outcome":{"status":"unpaid","reason":"The run cannot activate the human-installed hook."},"evidence":"test -L ~/.claude/hooks/pre-tool-use.sh"}"#,
    ];
    for payload in payloads {
        let appended = harness
            .run(
                [
                    OsString::from("log"),
                    OsString::from("--file"),
                    log.as_os_str().to_owned(),
                    OsString::from("--kind"),
                    OsString::from("criterion-execution"),
                    OsString::from("--node"),
                    OsString::from("m3-s2"),
                ],
                payload.as_bytes(),
            )
            .expect("append criterion execution");
        assert_eq!(appended.status.code(), Some(0));
        assert!(appended.stdout.is_empty());
        assert!(appended.stderr.is_empty());
    }

    let before_rejection = fs::metadata(&log).expect("log metadata").len();
    let rejected = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("criterion-execution"),
                OsString::from("--node"),
                OsString::from("m3-s2"),
            ],
            br#"{"criterion":{"name":"Bad","input":"Run","observation":"Pass"},"finished_result":"main@0123456789abcdef","outcome":{"status":"green","observed_result":"green"},"evidence":"command"}"#,
        )
        .expect("reject unknown outcome");
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    assert!(stderr(&rejected).contains("submitted event payload is invalid"));
    assert_eq!(
        fs::metadata(&log).expect("log metadata").len(),
        before_rejection
    );

    let read = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("read"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("criterion-execution"),
            ],
            b"",
        )
        .expect("read criterion executions");
    assert_eq!(read.status.code(), Some(0));
    assert!(read.stderr.is_empty());
    let stdout = String::from_utf8(read.stdout).expect("UTF-8 event output");
    assert!(stdout.ends_with('\n'));
    let records = stdout
        .lines()
        .map(|line| parse_event_line(line).expect("parse criterion record"))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 3);
    assert_eq!(
        records
            .iter()
            .map(|record| record.sequence().get())
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    let vision = VisionSlug::parse("2026-08-03-a-gate-runs-what-was-built").expect("vision");
    let state = derive_run_state(
        &records,
        &ratified_floor(),
        &vision,
        &RecoveryLogPath::new(log.display().to_string()),
        &[],
        &[],
        &[],
    )
    .expect("fold criterion executions");
    let executions = state.criterion_executions();
    assert_eq!(executions.len(), 3);
    assert_eq!(
        executions
            .iter()
            .map(|entry| entry.criterion().name().as_str())
            .collect::<Vec<_>>(),
        [
            "Runnable criterion",
            "Failing criterion",
            "Install-only criterion"
        ]
    );
    assert!(
        matches!(executions[0].outcome(), CriterionExecutionOutcome::Passed { observed_result } if observed_result.as_str() == "The command exited 0.")
    );
    assert!(
        matches!(executions[1].outcome(), CriterionExecutionOutcome::Failed { observed_result } if observed_result.as_str() == "The command exited 7.")
    );
    assert!(
        matches!(executions[2].outcome(), CriterionExecutionOutcome::Unpaid { reason } if reason.as_str() == "The run cannot activate the human-installed hook.")
    );
    assert!(
        executions
            .iter()
            .skip(1)
            .all(|entry| !matches!(entry.outcome(), CriterionExecutionOutcome::Passed { .. }))
    );
}

#[test]
fn missing_evidence_rejection_appends_zero_bytes() {
    let harness = CliHarness::new().expect("create CLI harness");
    let log = harness.path().join("events.jsonl");

    let seed = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("delta"),
                OsString::from("--node"),
                OsString::from("m3-s1"),
            ],
            br#"{"message":"seed before rejected fact"}"#,
        )
        .expect("run seed log command");
    assert!(seed.status.success(), "seed stderr: {}", stderr(&seed));
    let length_before = fs::metadata(&log).expect("read seeded log metadata").len();
    assert!(length_before > 0);

    let rejected = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("key-finding"),
                OsString::from("--node"),
                OsString::from("m3-s1"),
            ],
            br#"{"finding":"missing evidence must not append"}"#,
        )
        .expect("run rejected log command");
    assert!(!rejected.status.success());
    assert!(
        stderr(&rejected).contains("failed to validate submitted event payload"),
        "rejection stderr: {}",
        stderr(&rejected)
    );
    assert!(
        stderr(&rejected).contains(
            "submitted event payload is invalid: event kind key-finding requires an evidence field"
        ),
        "rejection stderr: {}",
        stderr(&rejected)
    );
    let length_after = fs::metadata(&log)
        .expect("read rejected log metadata")
        .len();
    assert_eq!(length_after, length_before);
}

#[test]
fn legacy_repository_contract_append_is_rejected_without_writing_bytes() {
    let harness = CliHarness::new().expect("create CLI harness");
    let log = harness.path().join("events.jsonl");

    let seed = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("delta"),
                OsString::from("--node"),
                OsString::from("m2-s1"),
            ],
            br#"{"message":"seed before rejected legacy contract"}"#,
        )
        .expect("run seed log command");
    assert!(seed.status.success(), "seed stderr: {}", stderr(&seed));
    let length_before = fs::metadata(&log).expect("read seeded log metadata").len();

    let rejected = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("repository-contract"),
                OsString::from("--node"),
                OsString::from("m2-s1"),
            ],
            br#"{"repository":"pce","repo_root":"/workspace/pce","stack":"Rust 2024-edition Cargo workspace (rustc/cargo 1.93.1)","format":"cargo fmt --all --check","lint":"cargo clippy --workspace --all-targets","typecheck":"cargo check --workspace --all-targets","test":"cargo test --workspace","build":"cargo build --workspace","preflight":"cargo check --workspace --all-targets","gates_rule":"From the repo root, all four gates must exit zero before committing.","install":"None required for gates.","evidence":"rustc --version\ncargo --version\ngit rev-parse --show-toplevel"}"#,
        )
        .expect("run rejected legacy repository-contract command");

    assert!(!rejected.status.success());
    let rejected_stderr = stderr(&rejected);
    assert!(
        rejected_stderr.contains("failed to validate submitted event payload"),
        "rejection stderr: {rejected_stderr}"
    );
    assert!(
        rejected_stderr.contains(
            "submitted event payload is invalid: invalid payload for known event kind repository-contract"
        ),
        "rejection stderr: {rejected_stderr}"
    );
    assert_eq!(
        fs::metadata(&log)
            .expect("read rejected log metadata")
            .len(),
        length_before
    );
}

#[test]
fn status_smoke_uses_every_isolated_adapter_path() {
    let harness = CliHarness::new().expect("create CLI harness");
    let root = harness.path().join("repo");
    let vision_dir = root.join("planning/2026-07-27-shim-smoke");
    write_vision_document(&vision_dir);
    let log = vision_dir.join("events.jsonl");
    fs::create_dir_all(&vision_dir).expect("create scratch vision directory");

    let contract = current_repository_contract(&root, "fixture repository contract").to_string();
    let seed = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("repository-contract"),
                OsString::from("--node"),
                OsString::from("m3-s1"),
            ],
            contract.as_bytes(),
        )
        .expect("run repository-contract log command");
    assert!(seed.status.success(), "seed stderr: {}", stderr(&seed));

    let root_arg = root.as_os_str().to_owned();
    let responses = vec![
        response(
            "git",
            argv(["-C", root.to_str().expect("UTF-8 root"), "remote", "get-url", "origin"]),
            0,
            b"https://example.invalid/pce.git\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "fetch",
                "--no-tags",
                "origin",
                "refs/heads/pce/shim-smoke/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "rev-parse",
                "--verify",
                "FETCH_HEAD^{commit}",
            ]),
            0,
            b"fetched-oid\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "show-ref",
                "--verify",
                "--quiet",
                "refs/heads/pce/shim-smoke/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "worktree",
                "list",
                "--porcelain",
            ]),
            0,
            format!(
                "worktree {}/worktrees/m3-s1\nHEAD worktree-head-oid\nbranch refs/heads/pce/shim-smoke/m3-s1\n",
                root.display()
            )
            .as_bytes(),
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/tags/v0.1.16^{}",
            ]),
            0,
            b"tag-oid\n",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/shim-smoke/m3-s1",
                "--base",
                "pce/shim-smoke/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            b"[{\"number\":17,\"headRefName\":\"pce/shim-smoke/m3-s1\",\"baseRefName\":\"pce/shim-smoke/milestone-3\",\"state\":\"MERGED\",\"mergeCommit\":{\"oid\":\"squash-oid\"}}]\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root.to_str().expect("UTF-8 root"),
                "merge-base",
                "--is-ancestor",
                "squash-oid",
                "fetched-oid",
            ]),
            0,
            b"",
        ),
    ];
    harness
        .materialize_responses(&responses)
        .expect("materialize scripted responses");

    let output = harness
        .run(
            [
                OsString::from("status"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--vision-dir"),
                vision_dir.as_os_str().to_owned(),
            ],
            b"",
        )
        .expect("run status command");
    assert!(
        output.status.success(),
        "status stderr: {}",
        stderr(&output)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("parse status snapshot");

    assert_eq!(at(&snapshot, "/schema_id"), &json!("pce.run-snapshot"));
    assert_eq!(at(&snapshot, "/schema_version"), &json!(1));
    assert_eq!(at(&snapshot, "/repositories/0/repository"), &json!("pce"));
    assert_eq!(
        at(&snapshot, "/repositories/0/fetch/state"),
        &json!("observed")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/fetch/observation_ref"),
        &json!("fetched-oid")
    );
    let fetched_at = at(&snapshot, "/repositories/0/fetch/fetched_at")
        .as_str()
        .expect("fetched_at string");
    assert!(
        is_millisecond_z_timestamp(fetched_at),
        "fetched_at: {fetched_at}"
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/branch/name"),
        &json!("pce/shim-smoke/milestone-3")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/branch/state"),
        &json!("present")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/worktree/identity"),
        &json!("pce/shim-smoke/m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/worktree/state"),
        &json!("present")
    );
    assert_eq!(at(&snapshot, "/repositories/0/tag/name"), &json!("v0.1.16"));
    assert_eq!(
        at(&snapshot, "/repositories/0/tag/state"),
        &json!("points-to")
    );
    assert_eq!(
        at(&snapshot, "/repositories/0/tag/target"),
        &json!("tag-oid")
    );
    assert_eq!(at(&snapshot, "/steps/0/node"), &json!("m3-s1"));
    assert_eq!(at(&snapshot, "/steps/0/subject/milestone"), &json!(3));
    assert_eq!(at(&snapshot, "/steps/0/subject/step"), &json!(1));
    assert_eq!(
        at(&snapshot, "/steps/0/subject/head_branch"),
        &json!("pce/shim-smoke/m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/subject/integration_branch"),
        &json!("pce/shim-smoke/milestone-3")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/subject/pull_request_selector/head"),
        &json!("pce/shim-smoke/m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/subject/pull_request_selector/base"),
        &json!("pce/shim-smoke/milestone-3")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/availability"),
        &json!("reachable")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/cardinality"),
        &json!("one-exact-match")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/pull_request/number"),
        &json!(17)
    );
    assert_eq!(
        at(&snapshot, "/steps/0/github/pull_request/state/status"),
        &json!("merged")
    );
    assert_eq!(
        at(
            &snapshot,
            "/steps/0/github/pull_request/state/squash_commit_oid"
        ),
        &json!("squash-oid")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/git/availability"),
        &json!("reachable")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/git/state"),
        &json!("squash-commit-reachable")
    );
    assert_eq!(
        at(&snapshot, "/steps/0/git/squash_commit_oid"),
        &json!("squash-oid")
    );
    assert_eq!(at(&snapshot, "/steps/0/merge_status"), &json!("merged"));
    assert_eq!(
        at(&snapshot, "/resume/state"),
        &json!("no-log-visible-candidate")
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/sequence"),
        &json!(1)
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/node"),
        &json!("m3-s1")
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/kind"),
        &json!("repository-contract")
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries/0/evidence"),
        &json!("fixture repository contract")
    );

    let expected = vec![
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "remote".into(),
                "get-url".into(),
                "origin".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                "refs/heads/pce/shim-smoke/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/heads/pce/shim-smoke/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/tags/v0.1.16^{}".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/shim-smoke/m3-s1",
                "--base",
                "pce/shim-smoke/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg,
                "merge-base".into(),
                "--is-ancestor".into(),
                "squash-oid".into(),
                "fetched-oid".into(),
            ],
        ),
    ];
    assert_eq!(harness.invocations().expect("parse invocations"), expected);
}

#[test]
fn cold_resume_skips_newer_merged_node_across_milestones() {
    let harness = CliHarness::new().expect("create CLI harness");
    let root = harness.path().join("repo");
    let vision_dir = root.join("planning/2026-07-27-cold-resume");
    let log = vision_dir.join("events.jsonl");
    let milestone_1_artifact = vision_dir.join("milestone-1/steps.json");
    let milestone_2_artifact = vision_dir.join("milestone-2/steps.json");
    fs::create_dir(&root).expect("create scratch repository root");
    write_vision_document(&vision_dir);
    fs::create_dir_all(
        milestone_1_artifact
            .parent()
            .expect("milestone-1 artifact parent"),
    )
    .expect("create milestone-1 artifact directory");
    fs::create_dir_all(
        milestone_2_artifact
            .parent()
            .expect("milestone-2 artifact parent"),
    )
    .expect("create milestone-2 artifact directory");
    fs::write(
        &milestone_1_artifact,
        b"{\"milestone\":1,\"status\":\"approved\"}\n",
    )
    .expect("write milestone-1 artifact");
    fs::write(
        &milestone_2_artifact,
        b"{\"milestone\":2,\"status\":\"approved\"}\n",
    )
    .expect("write milestone-2 artifact");

    let contract =
        current_repository_contract(&root, "fixture repository contract observation").to_string();
    let repository_contract = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("repository-contract"),
                OsString::from("--node"),
                OsString::from("m1-s1"),
            ],
            contract.as_bytes(),
        )
        .expect("run repository-contract log command");
    assert!(
        repository_contract.status.success(),
        "repository-contract stderr: {}",
        stderr(&repository_contract)
    );

    let milestone_1_dispatch = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("dispatch"),
                OsString::from("--node"),
                OsString::from("m1-s1"),
            ],
            br#"{"role":"step-executor","ref":"1111111111111111111111111111111111111111","evidence":"fixture m1 execution dispatch"}"#,
        )
        .expect("run milestone-1 dispatch log command");
    assert!(
        milestone_1_dispatch.status.success(),
        "milestone-1 dispatch stderr: {}",
        stderr(&milestone_1_dispatch)
    );

    let milestone_1_approval = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("planning-artifact-approved"),
                OsString::from("--node"),
                OsString::from("m1-s1"),
            ],
            br#"{"path":"planning/2026-07-27-cold-resume/milestone-1/steps.json","sha256":"5e43325921df733344f143cf035d4efb95f47a56872ee0223dc38527d8751a9f","evidence":"fixture m1 approved artifact digest"}"#,
        )
        .expect("run milestone-1 approval log command");
    assert!(
        milestone_1_approval.status.success(),
        "milestone-1 approval stderr: {}",
        stderr(&milestone_1_approval)
    );

    let milestone_2_dispatch = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("dispatch"),
                OsString::from("--node"),
                OsString::from("m2-s1"),
            ],
            br#"{"role":"step-plan-writer","ref":"2222222222222222222222222222222222222222","evidence":"fixture m2 plan dispatch"}"#,
        )
        .expect("run milestone-2 dispatch log command");
    assert!(
        milestone_2_dispatch.status.success(),
        "milestone-2 dispatch stderr: {}",
        stderr(&milestone_2_dispatch)
    );

    let milestone_2_delta = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("delta"),
                OsString::from("--node"),
                OsString::from("m2-s1"),
            ],
            br#"{"message":"m2 implementation remains pending"}"#,
        )
        .expect("run milestone-2 delta log command");
    assert!(
        milestone_2_delta.status.success(),
        "milestone-2 delta stderr: {}",
        stderr(&milestone_2_delta)
    );

    let milestone_2_approval = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("planning-artifact-approved"),
                OsString::from("--node"),
                OsString::from("m2-s1"),
            ],
            br#"{"path":"planning/2026-07-27-cold-resume/milestone-2/steps.json","sha256":"37218a4a0ff191f387a0ec4d75a67f4f7778abac5265ad222277c56864b5232a","evidence":"fixture m2 approved artifact digest"}"#,
        )
        .expect("run milestone-2 approval log command");
    assert!(
        milestone_2_approval.status.success(),
        "milestone-2 approval stderr: {}",
        stderr(&milestone_2_approval)
    );

    // Keep the merged node newer than the intended resume node. If merged
    // exclusion is removed, sequence 7 wins over sequence 6 and this test
    // incorrectly resumes m1-s1 instead of m2-s1.
    let post_merge_delta = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("delta"),
                OsString::from("--node"),
                OsString::from("m1-s1"),
            ],
            br#"{"message":"post-merge audit keeps merged node newest"}"#,
        )
        .expect("run post-merge delta log command");
    assert!(
        post_merge_delta.status.success(),
        "post-merge delta stderr: {}",
        stderr(&post_merge_delta)
    );

    let root_text = root.to_str().expect("UTF-8 root");
    let responses = vec![
        response(
            "git",
            argv(["-C", root_text, "remote", "get-url", "origin"]),
            0,
            b"https://example.invalid/cold-resume.git\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "fetch",
                "--no-tags",
                "origin",
                "refs/heads/pce/cold-resume/milestone-2",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "fetch",
                "--no-tags",
                "origin",
                "refs/heads/pce/cold-resume/milestone-1",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "FETCH_HEAD^{commit}",
            ]),
            0,
            b"fetched-integration-oid\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "show-ref",
                "--verify",
                "--quiet",
                "refs/heads/pce/cold-resume/milestone-1",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "worktree",
                "list",
                "--porcelain",
            ]),
            0,
            format!(
                "worktree {}/worktrees/m1-s1\nHEAD selected-worktree-oid\nbranch refs/heads/pce/cold-resume/m1-s1\n",
                root.display()
            )
            .as_bytes(),
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/tags/v0.1.16^{}",
            ]),
            1,
            b"",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/cold-resume/m1-s1",
                "--base",
                "pce/cold-resume/milestone-1",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            b"[{\"number\":101,\"headRefName\":\"pce/cold-resume/m1-s1\",\"baseRefName\":\"pce/cold-resume/milestone-1\",\"state\":\"MERGED\",\"mergeCommit\":{\"oid\":\"squash-m1-oid\"}}]\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "merge-base",
                "--is-ancestor",
                "squash-m1-oid",
                "fetched-integration-oid",
            ]),
            0,
            b"",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/cold-resume/m2-s1",
                "--base",
                "pce/cold-resume/milestone-2",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            b"[{\"number\":202,\"headRefName\":\"pce/cold-resume/m2-s1\",\"baseRefName\":\"pce/cold-resume/milestone-2\",\"state\":\"OPEN\",\"mergeCommit\":null}]\n",
        ),
    ];
    harness
        .materialize_responses(&responses)
        .expect("materialize scripted responses");

    let output = harness
        .run(
            [
                OsString::from("status"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--vision-dir"),
                vision_dir.as_os_str().to_owned(),
            ],
            b"",
        )
        .expect("run cold-resume status command");
    assert!(
        output.status.success(),
        "status stderr: {}",
        stderr(&output)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("parse status snapshot");

    let exact_fields = [
        ("/schema_id", json!("pce.run-snapshot")),
        ("/schema_version", json!(1)),
        ("/repositories/0/repository", json!("pce")),
        ("/repositories/0/fetch/state", json!("observed")),
        (
            "/repositories/0/fetch/observation_ref",
            json!("fetched-integration-oid"),
        ),
        (
            "/repositories/0/branch/name",
            json!("pce/cold-resume/milestone-1"),
        ),
        ("/repositories/0/branch/state", json!("present")),
        (
            "/repositories/0/worktree/identity",
            json!("pce/cold-resume/m1-s1"),
        ),
        ("/repositories/0/worktree/state", json!("present")),
        ("/repositories/0/tag/name", json!("v0.1.16")),
        ("/repositories/0/tag/state", json!("absent")),
        ("/steps/0/node", json!("m1-s1")),
        ("/steps/0/subject/milestone", json!(1)),
        ("/steps/0/subject/step", json!(1)),
        (
            "/steps/0/subject/head_branch",
            json!("pce/cold-resume/m1-s1"),
        ),
        (
            "/steps/0/subject/integration_branch",
            json!("pce/cold-resume/milestone-1"),
        ),
        (
            "/steps/0/subject/pull_request_selector/head",
            json!("pce/cold-resume/m1-s1"),
        ),
        (
            "/steps/0/subject/pull_request_selector/base",
            json!("pce/cold-resume/milestone-1"),
        ),
        ("/steps/0/github/availability", json!("reachable")),
        ("/steps/0/github/cardinality", json!("one-exact-match")),
        ("/steps/0/github/pull_request/number", json!(101)),
        (
            "/steps/0/github/pull_request/selector/head",
            json!("pce/cold-resume/m1-s1"),
        ),
        (
            "/steps/0/github/pull_request/selector/base",
            json!("pce/cold-resume/milestone-1"),
        ),
        ("/steps/0/github/pull_request/state/status", json!("merged")),
        (
            "/steps/0/github/pull_request/state/squash_commit_oid",
            json!("squash-m1-oid"),
        ),
        ("/steps/0/git/availability", json!("reachable")),
        ("/steps/0/git/state", json!("squash-commit-reachable")),
        ("/steps/0/git/squash_commit_oid", json!("squash-m1-oid")),
        ("/steps/0/merge_status", json!("merged")),
        ("/steps/1/node", json!("m2-s1")),
        ("/steps/1/subject/milestone", json!(2)),
        ("/steps/1/subject/step", json!(1)),
        (
            "/steps/1/subject/head_branch",
            json!("pce/cold-resume/m2-s1"),
        ),
        (
            "/steps/1/subject/integration_branch",
            json!("pce/cold-resume/milestone-2"),
        ),
        (
            "/steps/1/subject/pull_request_selector/head",
            json!("pce/cold-resume/m2-s1"),
        ),
        (
            "/steps/1/subject/pull_request_selector/base",
            json!("pce/cold-resume/milestone-2"),
        ),
        ("/steps/1/github/availability", json!("reachable")),
        ("/steps/1/github/cardinality", json!("one-exact-match")),
        ("/steps/1/github/pull_request/number", json!(202)),
        (
            "/steps/1/github/pull_request/selector/head",
            json!("pce/cold-resume/m2-s1"),
        ),
        (
            "/steps/1/github/pull_request/selector/base",
            json!("pce/cold-resume/milestone-2"),
        ),
        (
            "/steps/1/github/pull_request/state/status",
            json!("not-merged"),
        ),
        ("/steps/1/git/availability", json!("reachable")),
        ("/steps/1/git/state", json!("not-merged")),
        ("/steps/1/merge_status", json!("not-merged")),
        ("/resume/state", json!("candidate")),
        ("/resume/node", json!("m2-s1")),
        ("/resume/latest_sequence", json!(6)),
        ("/resume/cycle_position/state", json!("plan-dispatched")),
        ("/resume/cycle_position/sequence", json!(4)),
        ("/dispatches/0/sequence", json!(2)),
        ("/dispatches/0/node", json!("m1-s1")),
        ("/dispatches/0/role", json!("step-executor")),
        (
            "/dispatches/0/ref",
            json!("1111111111111111111111111111111111111111"),
        ),
        ("/dispatches/1/sequence", json!(4)),
        ("/dispatches/1/node", json!("m2-s1")),
        ("/dispatches/1/role", json!("step-plan-writer")),
        (
            "/dispatches/1/ref",
            json!("2222222222222222222222222222222222222222"),
        ),
        ("/rounds/0/node", json!("m1-s1")),
        ("/rounds/0/role", json!("step-executor")),
        ("/rounds/0/classification", json!("execution")),
        ("/rounds/0/count", json!(1)),
        ("/rounds/1/node", json!("m2-s1")),
        ("/rounds/1/role", json!("step-plan-writer")),
        ("/rounds/1/classification", json!("plan-producing")),
        ("/rounds/1/count", json!(1)),
        (
            "/provenance/0/path",
            json!("planning/2026-07-27-cold-resume/milestone-1/steps.json"),
        ),
        (
            "/provenance/0/approved_sha256",
            json!("5e43325921df733344f143cf035d4efb95f47a56872ee0223dc38527d8751a9f"),
        ),
        ("/provenance/0/approval_node", json!("m1-s1")),
        ("/provenance/0/approval_sequence", json!(3)),
        ("/provenance/0/condition/state", json!("digest-matches")),
        (
            "/provenance/1/path",
            json!("planning/2026-07-27-cold-resume/milestone-2/steps.json"),
        ),
        (
            "/provenance/1/approved_sha256",
            json!("37218a4a0ff191f387a0ec4d75a67f4f7778abac5265ad222277c56864b5232a"),
        ),
        ("/provenance/1/approval_node", json!("m2-s1")),
        ("/provenance/1/approval_sequence", json!(6)),
        ("/provenance/1/condition/state", json!("digest-matches")),
        ("/recovery_digest/rounds/entries/0/sequence", json!(2)),
        ("/recovery_digest/rounds/entries/0/node", json!("m1-s1")),
        (
            "/recovery_digest/rounds/entries/0/role",
            json!("step-executor"),
        ),
        ("/recovery_digest/rounds/entries/0/round_number", json!(1)),
        ("/recovery_digest/rounds/entries/1/sequence", json!(4)),
        ("/recovery_digest/rounds/entries/1/node", json!("m2-s1")),
        (
            "/recovery_digest/rounds/entries/1/role",
            json!("step-plan-writer"),
        ),
        ("/recovery_digest/rounds/entries/1/round_number", json!(1)),
        ("/recovery_digest/deltas/entries/0/sequence", json!(5)),
        ("/recovery_digest/deltas/entries/0/node", json!("m2-s1")),
        (
            "/recovery_digest/deltas/entries/0/message",
            json!("m2 implementation remains pending"),
        ),
        ("/recovery_digest/deltas/entries/1/sequence", json!(7)),
        ("/recovery_digest/deltas/entries/1/node", json!("m1-s1")),
        (
            "/recovery_digest/deltas/entries/1/message",
            json!("post-merge audit keeps merged node newest"),
        ),
        ("/recovery_digest/facts/entries/0/sequence", json!(1)),
        ("/recovery_digest/facts/entries/0/node", json!("m1-s1")),
        (
            "/recovery_digest/facts/entries/0/kind",
            json!("repository-contract"),
        ),
        (
            "/recovery_digest/facts/entries/0/evidence",
            json!("fixture repository contract observation"),
        ),
        ("/recovery_digest/facts/entries/1/sequence", json!(2)),
        ("/recovery_digest/facts/entries/1/node", json!("m1-s1")),
        ("/recovery_digest/facts/entries/1/kind", json!("dispatch")),
        (
            "/recovery_digest/facts/entries/1/evidence",
            json!("fixture m1 execution dispatch"),
        ),
        ("/recovery_digest/facts/entries/2/sequence", json!(3)),
        ("/recovery_digest/facts/entries/2/node", json!("m1-s1")),
        (
            "/recovery_digest/facts/entries/2/kind",
            json!("planning-artifact-approved"),
        ),
        (
            "/recovery_digest/facts/entries/2/evidence",
            json!("fixture m1 approved artifact digest"),
        ),
        ("/recovery_digest/facts/entries/3/sequence", json!(4)),
        ("/recovery_digest/facts/entries/3/node", json!("m2-s1")),
        ("/recovery_digest/facts/entries/3/kind", json!("dispatch")),
        (
            "/recovery_digest/facts/entries/3/evidence",
            json!("fixture m2 plan dispatch"),
        ),
        ("/recovery_digest/facts/entries/4/sequence", json!(6)),
        ("/recovery_digest/facts/entries/4/node", json!("m2-s1")),
        (
            "/recovery_digest/facts/entries/4/kind",
            json!("planning-artifact-approved"),
        ),
        (
            "/recovery_digest/facts/entries/4/evidence",
            json!("fixture m2 approved artifact digest"),
        ),
    ];
    for (pointer, expected) in exact_fields {
        assert_eq!(at(&snapshot, pointer), &expected, "JSON pointer {pointer}");
    }

    assert_eq!(
        at(&snapshot, "/repositories").as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(at(&snapshot, "/steps").as_array().map(Vec::len), Some(2));
    assert_eq!(
        at(&snapshot, "/dispatches").as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(at(&snapshot, "/rounds").as_array().map(Vec::len), Some(2));
    assert_eq!(at(&snapshot, "/holds"), &json!([]));
    assert_eq!(
        at(&snapshot, "/provenance").as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/rounds/entries")
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/rounds/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/entries"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/deltas/entries")
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/deltas/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries")
            .as_array()
            .map(Vec::len),
        Some(5)
    );
    assert_eq!(at(&snapshot, "/recovery_digest/facts/elisions"), &json!([]));
    let fetched_at = at(&snapshot, "/repositories/0/fetch/fetched_at")
        .as_str()
        .expect("fetched_at string");
    assert!(
        is_millisecond_z_timestamp(fetched_at),
        "fetched_at: {fetched_at}"
    );

    let root_arg = root.as_os_str().to_owned();
    let expected = vec![
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "remote".into(),
                "get-url".into(),
                "origin".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                "refs/heads/pce/cold-resume/milestone-2".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                "refs/heads/pce/cold-resume/milestone-1".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/heads/pce/cold-resume/milestone-1".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/tags/v0.1.16^{}".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/cold-resume/m1-s1",
                "--base",
                "pce/cold-resume/milestone-1",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "merge-base".into(),
                "--is-ancestor".into(),
                "squash-m1-oid".into(),
                "fetched-integration-oid".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/cold-resume/m2-s1",
                "--base",
                "pce/cold-resume/milestone-2",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
    ];
    assert_eq!(harness.invocations().expect("parse invocations"), expected);
}

#[test]
fn mutated_approved_artifact_reports_mismatch_without_changing_not_merged_status() {
    let harness = CliHarness::new().expect("create CLI harness");
    let root = harness.path().join("repo");
    let vision_dir = root.join("planning/2026-07-27-provenance-mismatch");
    let log = vision_dir.join("events.jsonl");
    let artifact = vision_dir.join("milestone-3/step-m3-s3/plan.md");
    fs::create_dir(&root).expect("create scratch repository root");
    write_vision_document(&vision_dir);
    fs::create_dir_all(artifact.parent().expect("artifact parent"))
        .expect("create scratch artifact parent");
    fs::write(&artifact, b"{\"node\":\"m3-s3\",\"status\":\"approved\"}\n")
        .expect("write approved artifact");

    let contract =
        current_repository_contract(&root, "fixture repository contract observation").to_string();
    let contract_log = harness
        .run(
            vec![
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("repository-contract"),
                OsString::from("--node"),
                OsString::from("m3-s3"),
            ],
            contract.as_bytes(),
        )
        .expect("run repository-contract log command");
    assert!(
        contract_log.status.success(),
        "repository-contract stderr: {}",
        stderr(&contract_log)
    );

    let approval_log = harness
        .run(
            vec![
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("planning-artifact-approved"),
                OsString::from("--node"),
                OsString::from("m3-s3"),
            ],
            br#"{"path":"planning/2026-07-27-provenance-mismatch/milestone-3/step-m3-s3/plan.md","sha256":"624073aa3bbe6d74fc77297baada273ec51cf5a191c1c12fba0d5f3a8a8f6f4c","evidence":"fixture approved artifact sha256"}"#,
        )
        .expect("run planning-artifact-approved log command");
    assert!(
        approval_log.status.success(),
        "planning-artifact-approved stderr: {}",
        stderr(&approval_log)
    );

    fs::write(
        &artifact,
        b"{\"node\":\"m3-s3\",\"status\":\"mutated-after-approval\"}\n",
    )
    .expect("mutate approved artifact");

    let root_text = root.to_str().expect("UTF-8 root");
    // In observe_git, when GitHub reports ZeroExactMatches or OneExactMatch with NotMerged, git is NEVER consulted for merge status - the adapter returns a reachable not-merged observation DIRECTLY without issuing any merge-base call. So if you want a not-merged step, configure NO merge-base response for it, and an unconfigured merge-base call would exit 127 loudly, which you may additionally assert as absent from the recorded invocations. Crucially, that not-merged outcome is CONTINGENT on the selected integration branch's fetch AND its branch-free rev-parse --verify FETCH_HEAD^{commit} BOTH succeeding in the shim: if either fails, git becomes Unreachable and the merge status derives to inconclusive, silently defeating a not-merged assertion.
    let responses = vec![
        response(
            "git",
            argv(["-C", root_text, "remote", "get-url", "origin"]),
            0,
            b"https://example.invalid/provenance-mismatch.git\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "fetch",
                "--no-tags",
                "origin",
                "refs/heads/pce/provenance-mismatch/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "FETCH_HEAD^{commit}",
            ]),
            0,
            b"fetched-provenance-oid\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "show-ref",
                "--verify",
                "--quiet",
                "refs/heads/pce/provenance-mismatch/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "worktree",
                "list",
                "--porcelain",
            ]),
            0,
            format!(
                "worktree {}/worktrees/m3-s3\nHEAD selected-provenance-worktree-oid\nbranch refs/heads/pce/provenance-mismatch/m3-s3\n",
                root.display()
            )
            .as_bytes(),
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/tags/v0.1.16^{}",
            ]),
            1,
            b"",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/provenance-mismatch/m3-s3",
                "--base",
                "pce/provenance-mismatch/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            b"[{\"number\":303,\"headRefName\":\"pce/provenance-mismatch/m3-s3\",\"baseRefName\":\"pce/provenance-mismatch/milestone-3\",\"state\":\"OPEN\",\"mergeCommit\":null}]\n",
        ),
    ];
    harness
        .materialize_responses(&responses)
        .expect("materialize scripted responses");

    let output = harness
        .run(
            vec![
                OsString::from("status"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--vision-dir"),
                vision_dir.as_os_str().to_owned(),
            ],
            b"",
        )
        .expect("run provenance mismatch status command");
    assert!(
        output.status.success(),
        "status stderr: {}",
        stderr(&output)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("parse status snapshot");

    let exact_fields = [
        ("/schema_id", json!("pce.run-snapshot")),
        ("/schema_version", json!(1)),
        ("/repositories/0/repository", json!("pce")),
        ("/repositories/0/fetch/state", json!("observed")),
        (
            "/repositories/0/fetch/observation_ref",
            json!("fetched-provenance-oid"),
        ),
        (
            "/repositories/0/branch/name",
            json!("pce/provenance-mismatch/milestone-3"),
        ),
        ("/repositories/0/branch/state", json!("present")),
        (
            "/repositories/0/worktree/identity",
            json!("pce/provenance-mismatch/m3-s3"),
        ),
        ("/repositories/0/worktree/state", json!("present")),
        ("/repositories/0/tag/name", json!("v0.1.16")),
        ("/repositories/0/tag/state", json!("absent")),
        ("/steps/0/node", json!("m3-s3")),
        ("/steps/0/subject/milestone", json!(3)),
        ("/steps/0/subject/step", json!(3)),
        (
            "/steps/0/subject/head_branch",
            json!("pce/provenance-mismatch/m3-s3"),
        ),
        (
            "/steps/0/subject/integration_branch",
            json!("pce/provenance-mismatch/milestone-3"),
        ),
        (
            "/steps/0/subject/pull_request_selector/head",
            json!("pce/provenance-mismatch/m3-s3"),
        ),
        (
            "/steps/0/subject/pull_request_selector/base",
            json!("pce/provenance-mismatch/milestone-3"),
        ),
        ("/steps/0/github/availability", json!("reachable")),
        ("/steps/0/github/cardinality", json!("one-exact-match")),
        ("/steps/0/github/pull_request/number", json!(303)),
        (
            "/steps/0/github/pull_request/selector/head",
            json!("pce/provenance-mismatch/m3-s3"),
        ),
        (
            "/steps/0/github/pull_request/selector/base",
            json!("pce/provenance-mismatch/milestone-3"),
        ),
        (
            "/steps/0/github/pull_request/state/status",
            json!("not-merged"),
        ),
        ("/steps/0/git/availability", json!("reachable")),
        ("/steps/0/git/state", json!("not-merged")),
        ("/steps/0/merge_status", json!("not-merged")),
        (
            "/provenance/0/path",
            json!("planning/2026-07-27-provenance-mismatch/milestone-3/step-m3-s3/plan.md"),
        ),
        (
            "/provenance/0/approved_sha256",
            json!("624073aa3bbe6d74fc77297baada273ec51cf5a191c1c12fba0d5f3a8a8f6f4c"),
        ),
        ("/provenance/0/approval_node", json!("m3-s3")),
        ("/provenance/0/approval_sequence", json!(2)),
        ("/provenance/0/condition/state", json!("digest-mismatch")),
        (
            "/provenance/0/condition/approved_sha256",
            json!("624073aa3bbe6d74fc77297baada273ec51cf5a191c1c12fba0d5f3a8a8f6f4c"),
        ),
        (
            "/provenance/0/condition/current_sha256",
            json!("5b7a6e39eb5ddac0d9a2bc2fae5a31b563a023f8b5431969d3b1c44cd5b3b72f"),
        ),
        ("/resume/state", json!("candidate")),
        ("/resume/node", json!("m3-s3")),
        ("/resume/latest_sequence", json!(2)),
        ("/resume/cycle_position/state", json!("no-round-dispatch")),
        ("/recovery_digest/facts/entries/0/sequence", json!(1)),
        ("/recovery_digest/facts/entries/0/node", json!("m3-s3")),
        (
            "/recovery_digest/facts/entries/0/kind",
            json!("repository-contract"),
        ),
        (
            "/recovery_digest/facts/entries/0/evidence",
            json!("fixture repository contract observation"),
        ),
        ("/recovery_digest/facts/entries/1/sequence", json!(2)),
        ("/recovery_digest/facts/entries/1/node", json!("m3-s3")),
        (
            "/recovery_digest/facts/entries/1/kind",
            json!("planning-artifact-approved"),
        ),
        (
            "/recovery_digest/facts/entries/1/evidence",
            json!("fixture approved artifact sha256"),
        ),
    ];
    for (pointer, expected) in exact_fields {
        assert_eq!(at(&snapshot, pointer), &expected, "JSON pointer {pointer}");
    }

    assert_eq!(
        at(&snapshot, "/repositories").as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(at(&snapshot, "/steps").as_array().map(Vec::len), Some(1));
    assert_eq!(at(&snapshot, "/dispatches"), &json!([]));
    assert_eq!(at(&snapshot, "/rounds"), &json!([]));
    assert_eq!(at(&snapshot, "/holds"), &json!([]));
    assert_eq!(
        at(&snapshot, "/provenance").as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(at(&snapshot, "/recovery_digest/rounds/entries"), &json!([]));
    assert_eq!(
        at(&snapshot, "/recovery_digest/rounds/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/entries"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/elisions"),
        &json!([])
    );
    assert_eq!(at(&snapshot, "/recovery_digest/deltas/entries"), &json!([]));
    assert_eq!(
        at(&snapshot, "/recovery_digest/deltas/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries")
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(at(&snapshot, "/recovery_digest/facts/elisions"), &json!([]));
    let fetched_at = at(&snapshot, "/repositories/0/fetch/fetched_at")
        .as_str()
        .expect("fetched_at string");
    assert!(
        is_millisecond_z_timestamp(fetched_at),
        "fetched_at: {fetched_at}"
    );

    let root_arg = root.as_os_str().to_owned();
    let expected = vec![
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "remote".into(),
                "get-url".into(),
                "origin".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                "refs/heads/pce/provenance-mismatch/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/heads/pce/provenance-mismatch/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg,
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/tags/v0.1.16^{}".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/provenance-mismatch/m3-s3",
                "--base",
                "pce/provenance-mismatch/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
    ];
    let invocations = harness.invocations().expect("parse invocations");
    assert_eq!(invocations, expected);
    assert!(
        !invocations.iter().any(|item| {
            item.program == "git" && item.argv.iter().any(|argument| argument == "merge-base")
        }),
        "merge-base must not be invoked for an OPEN pull request"
    );
}

#[test]
fn authority_disagreement_reports_inconclusive() {
    let harness = CliHarness::new().expect("create CLI harness");
    let root = harness.path().join("repo");
    let vision_dir = root.join("planning/2026-07-27-authority-disagreement");
    write_vision_document(&vision_dir);
    let log = vision_dir.join("events.jsonl");
    fs::create_dir_all(&vision_dir).expect("create scratch vision directory");

    let contract =
        current_repository_contract(&root, "fixture disagreement repository contract").to_string();
    let seed = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("repository-contract"),
                OsString::from("--node"),
                OsString::from("m3-s4"),
            ],
            contract.as_bytes(),
        )
        .expect("run repository-contract log command");
    assert!(seed.status.success(), "seed stderr: {}", stderr(&seed));

    let root_text = root.to_str().expect("UTF-8 root");
    // observe_git first resolves the selected branch fetch result. A failed fetch or
    // branch-free FETCH_HEAD rev-parse makes git unreachable for every node.
    // Git is not consulted when gh reports zero exact matches or one exact not-merged
    // match. The exact MERGED match below is therefore required to issue merge-base.
    // Only merge-base exit 1 means not-an-ancestor; every other nonzero exit is
    // deliberately unreachable so a missing object cannot become a false negative.
    let responses = vec![
        response(
            "git",
            argv(["-C", root_text, "remote", "get-url", "origin"]),
            0,
            b"https://example.invalid/authority-disagreement.git\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "fetch",
                "--no-tags",
                "origin",
                "refs/heads/pce/authority-disagreement/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "FETCH_HEAD^{commit}",
            ]),
            0,
            b"fetched-authority-oid\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "show-ref",
                "--verify",
                "--quiet",
                "refs/heads/pce/authority-disagreement/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv(["-C", root_text, "worktree", "list", "--porcelain"]),
            0,
            format!(
                "worktree {}/worktrees/m3-s4\nHEAD disagreement-worktree-oid\nbranch refs/heads/pce/authority-disagreement/m3-s4\n",
                root.display()
            )
            .as_bytes(),
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/tags/v0.1.16^{}",
            ]),
            0,
            b"tag-authority-oid\n",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/authority-disagreement/m3-s4",
                "--base",
                "pce/authority-disagreement/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            b"[{\"number\":404,\"headRefName\":\"pce/authority-disagreement/m3-s4\",\"baseRefName\":\"pce/authority-disagreement/milestone-3\",\"state\":\"MERGED\",\"mergeCommit\":{\"oid\":\"squash-disagreement-oid\"}}]\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "merge-base",
                "--is-ancestor",
                "squash-disagreement-oid",
                "fetched-authority-oid",
            ]),
            1,
            b"",
        ),
    ];
    harness
        .materialize_responses(&responses)
        .expect("materialize scripted responses");

    let output = harness
        .run(
            [
                OsString::from("status"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--vision-dir"),
                vision_dir.as_os_str().to_owned(),
            ],
            b"",
        )
        .expect("run authority disagreement status command");
    assert!(
        output.status.success(),
        "status stderr: {}",
        stderr(&output)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("parse status snapshot");

    let exact_fields = [
        ("/schema_id", json!("pce.run-snapshot")),
        ("/schema_version", json!(1)),
        ("/repositories/0/repository", json!("pce")),
        ("/repositories/0/fetch/state", json!("observed")),
        (
            "/repositories/0/fetch/observation_ref",
            json!("fetched-authority-oid"),
        ),
        (
            "/repositories/0/branch/name",
            json!("pce/authority-disagreement/milestone-3"),
        ),
        ("/repositories/0/branch/state", json!("present")),
        (
            "/repositories/0/worktree/identity",
            json!("pce/authority-disagreement/m3-s4"),
        ),
        ("/repositories/0/worktree/state", json!("present")),
        ("/repositories/0/tag/name", json!("v0.1.16")),
        ("/repositories/0/tag/state", json!("points-to")),
        ("/repositories/0/tag/target", json!("tag-authority-oid")),
        ("/steps/0/node", json!("m3-s4")),
        ("/steps/0/subject/milestone", json!(3)),
        ("/steps/0/subject/step", json!(4)),
        (
            "/steps/0/subject/head_branch",
            json!("pce/authority-disagreement/m3-s4"),
        ),
        (
            "/steps/0/subject/integration_branch",
            json!("pce/authority-disagreement/milestone-3"),
        ),
        (
            "/steps/0/subject/pull_request_selector/head",
            json!("pce/authority-disagreement/m3-s4"),
        ),
        (
            "/steps/0/subject/pull_request_selector/base",
            json!("pce/authority-disagreement/milestone-3"),
        ),
        ("/steps/0/github/availability", json!("reachable")),
        ("/steps/0/github/cardinality", json!("one-exact-match")),
        ("/steps/0/github/pull_request/number", json!(404)),
        (
            "/steps/0/github/pull_request/selector/head",
            json!("pce/authority-disagreement/m3-s4"),
        ),
        (
            "/steps/0/github/pull_request/selector/base",
            json!("pce/authority-disagreement/milestone-3"),
        ),
        ("/steps/0/github/pull_request/state/status", json!("merged")),
        (
            "/steps/0/github/pull_request/state/squash_commit_oid",
            json!("squash-disagreement-oid"),
        ),
        ("/steps/0/git/availability", json!("reachable")),
        ("/steps/0/git/state", json!("not-merged")),
        ("/steps/0/merge_status", json!("inconclusive")),
        ("/resume/state", json!("candidate")),
        ("/resume/node", json!("m3-s4")),
        ("/resume/latest_sequence", json!(1)),
        ("/resume/cycle_position/state", json!("no-round-dispatch")),
        ("/recovery_digest/facts/entries/0/sequence", json!(1)),
        ("/recovery_digest/facts/entries/0/node", json!("m3-s4")),
        (
            "/recovery_digest/facts/entries/0/kind",
            json!("repository-contract"),
        ),
        (
            "/recovery_digest/facts/entries/0/evidence",
            json!("fixture disagreement repository contract"),
        ),
    ];
    for (pointer, expected) in exact_fields {
        assert_eq!(at(&snapshot, pointer), &expected, "JSON pointer {pointer}");
    }

    assert_eq!(
        at(&snapshot, "/repositories").as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(at(&snapshot, "/steps").as_array().map(Vec::len), Some(1));
    assert_eq!(at(&snapshot, "/dispatches"), &json!([]));
    assert_eq!(at(&snapshot, "/rounds"), &json!([]));
    assert_eq!(at(&snapshot, "/holds"), &json!([]));
    assert_eq!(at(&snapshot, "/provenance"), &json!([]));
    assert_eq!(at(&snapshot, "/recovery_digest/rounds/entries"), &json!([]));
    assert_eq!(
        at(&snapshot, "/recovery_digest/rounds/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/entries"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/elisions"),
        &json!([])
    );
    assert_eq!(at(&snapshot, "/recovery_digest/deltas/entries"), &json!([]));
    assert_eq!(
        at(&snapshot, "/recovery_digest/deltas/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries")
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(at(&snapshot, "/recovery_digest/facts/elisions"), &json!([]));
    let fetched_at = at(&snapshot, "/repositories/0/fetch/fetched_at")
        .as_str()
        .expect("fetched_at string");
    assert!(
        is_millisecond_z_timestamp(fetched_at),
        "fetched_at: {fetched_at}"
    );

    let root_arg = root.as_os_str().to_owned();
    let expected = vec![
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "remote".into(),
                "get-url".into(),
                "origin".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                "refs/heads/pce/authority-disagreement/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/heads/pce/authority-disagreement/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/tags/v0.1.16^{}".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/authority-disagreement/m3-s4",
                "--base",
                "pce/authority-disagreement/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg,
                "merge-base".into(),
                "--is-ancestor".into(),
                "squash-disagreement-oid".into(),
                "fetched-authority-oid".into(),
            ],
        ),
    ];
    assert_eq!(harness.invocations().expect("parse invocations"), expected);
}

#[test]
fn merge_base_failure_reports_inconclusive() {
    let harness = CliHarness::new().expect("create CLI harness");
    let root = harness.path().join("repo");
    let vision_dir = root.join("planning/2026-07-27-authority-unreachable");
    write_vision_document(&vision_dir);
    let log = vision_dir.join("events.jsonl");
    fs::create_dir_all(&vision_dir).expect("create scratch vision directory");

    let contract =
        current_repository_contract(&root, "fixture unreachable repository contract").to_string();
    let seed = harness
        .run(
            [
                OsString::from("log"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--kind"),
                OsString::from("repository-contract"),
                OsString::from("--node"),
                OsString::from("m3-s4"),
            ],
            contract.as_bytes(),
        )
        .expect("run repository-contract log command");
    assert!(seed.status.success(), "seed stderr: {}", stderr(&seed));

    let root_text = root.to_str().expect("UTF-8 root");
    // observe_git first resolves the selected branch fetch result. A failed fetch or
    // branch-free FETCH_HEAD rev-parse makes git unreachable for every node.
    // Git is not consulted when gh reports zero exact matches or one exact not-merged
    // match. The exact MERGED match below is therefore required to issue merge-base.
    // Only merge-base exit 1 means not-an-ancestor; every other nonzero exit is
    // deliberately unreachable so a missing object cannot become a false negative.
    let responses = vec![
        response(
            "git",
            argv(["-C", root_text, "remote", "get-url", "origin"]),
            0,
            b"https://example.invalid/authority-unreachable.git\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "fetch",
                "--no-tags",
                "origin",
                "refs/heads/pce/authority-unreachable/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "FETCH_HEAD^{commit}",
            ]),
            0,
            b"fetched-authority-oid\n",
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "show-ref",
                "--verify",
                "--quiet",
                "refs/heads/pce/authority-unreachable/milestone-3",
            ]),
            0,
            b"",
        ),
        response(
            "git",
            argv(["-C", root_text, "worktree", "list", "--porcelain"]),
            0,
            format!(
                "worktree {}/worktrees/m3-s4\nHEAD unreachable-worktree-oid\nbranch refs/heads/pce/authority-unreachable/m3-s4\n",
                root.display()
            )
            .as_bytes(),
        ),
        response(
            "git",
            argv([
                "-C",
                root_text,
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/tags/v0.1.16^{}",
            ]),
            0,
            b"tag-authority-oid\n",
        ),
        response(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/authority-unreachable/m3-s4",
                "--base",
                "pce/authority-unreachable/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
            0,
            b"[{\"number\":405,\"headRefName\":\"pce/authority-unreachable/m3-s4\",\"baseRefName\":\"pce/authority-unreachable/milestone-3\",\"state\":\"MERGED\",\"mergeCommit\":{\"oid\":\"squash-unreachable-oid\"}}]\n",
        ),
        ScriptedResponse {
            program: "git".into(),
            argv: argv([
                "-C",
                root_text,
                "merge-base",
                "--is-ancestor",
                "squash-unreachable-oid",
                "fetched-authority-oid",
            ]),
            exit_code: 42,
            stdout: Vec::new(),
            stderr: b"fatal: simulated missing squash object\n".to_vec(),
        },
    ];
    harness
        .materialize_responses(&responses)
        .expect("materialize scripted responses");

    let output = harness
        .run(
            [
                OsString::from("status"),
                OsString::from("--file"),
                log.as_os_str().to_owned(),
                OsString::from("--vision-dir"),
                vision_dir.as_os_str().to_owned(),
            ],
            b"",
        )
        .expect("run authority unreachability status command");
    assert!(
        output.status.success(),
        "status stderr: {}",
        stderr(&output)
    );
    let snapshot: Value = serde_json::from_slice(&output.stdout).expect("parse status snapshot");
    let expected_failure = format!(
        "command `git -C {} merge-base --is-ancestor squash-unreachable-oid fetched-authority-oid` exited exit status: 42; stdout: ; stderr: fatal: simulated missing squash object\n",
        root.display()
    );

    let exact_fields = [
        ("/schema_id", json!("pce.run-snapshot")),
        ("/schema_version", json!(1)),
        ("/repositories/0/repository", json!("pce")),
        ("/repositories/0/fetch/state", json!("observed")),
        (
            "/repositories/0/fetch/observation_ref",
            json!("fetched-authority-oid"),
        ),
        (
            "/repositories/0/branch/name",
            json!("pce/authority-unreachable/milestone-3"),
        ),
        ("/repositories/0/branch/state", json!("present")),
        (
            "/repositories/0/worktree/identity",
            json!("pce/authority-unreachable/m3-s4"),
        ),
        ("/repositories/0/worktree/state", json!("present")),
        ("/repositories/0/tag/name", json!("v0.1.16")),
        ("/repositories/0/tag/state", json!("points-to")),
        ("/repositories/0/tag/target", json!("tag-authority-oid")),
        ("/steps/0/node", json!("m3-s4")),
        ("/steps/0/subject/milestone", json!(3)),
        ("/steps/0/subject/step", json!(4)),
        (
            "/steps/0/subject/head_branch",
            json!("pce/authority-unreachable/m3-s4"),
        ),
        (
            "/steps/0/subject/integration_branch",
            json!("pce/authority-unreachable/milestone-3"),
        ),
        (
            "/steps/0/subject/pull_request_selector/head",
            json!("pce/authority-unreachable/m3-s4"),
        ),
        (
            "/steps/0/subject/pull_request_selector/base",
            json!("pce/authority-unreachable/milestone-3"),
        ),
        ("/steps/0/github/availability", json!("reachable")),
        ("/steps/0/github/cardinality", json!("one-exact-match")),
        ("/steps/0/github/pull_request/number", json!(405)),
        (
            "/steps/0/github/pull_request/selector/head",
            json!("pce/authority-unreachable/m3-s4"),
        ),
        (
            "/steps/0/github/pull_request/selector/base",
            json!("pce/authority-unreachable/milestone-3"),
        ),
        ("/steps/0/github/pull_request/state/status", json!("merged")),
        (
            "/steps/0/github/pull_request/state/squash_commit_oid",
            json!("squash-unreachable-oid"),
        ),
        ("/steps/0/git/availability", json!("unreachable")),
        ("/steps/0/git/failure", json!(expected_failure)),
        ("/steps/0/merge_status", json!("inconclusive")),
        ("/resume/state", json!("candidate")),
        ("/resume/node", json!("m3-s4")),
        ("/resume/latest_sequence", json!(1)),
        ("/resume/cycle_position/state", json!("no-round-dispatch")),
        ("/recovery_digest/facts/entries/0/sequence", json!(1)),
        ("/recovery_digest/facts/entries/0/node", json!("m3-s4")),
        (
            "/recovery_digest/facts/entries/0/kind",
            json!("repository-contract"),
        ),
        (
            "/recovery_digest/facts/entries/0/evidence",
            json!("fixture unreachable repository contract"),
        ),
    ];
    for (pointer, expected) in exact_fields {
        assert_eq!(at(&snapshot, pointer), &expected, "JSON pointer {pointer}");
    }

    assert_eq!(
        at(&snapshot, "/repositories").as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(at(&snapshot, "/steps").as_array().map(Vec::len), Some(1));
    assert_eq!(at(&snapshot, "/dispatches"), &json!([]));
    assert_eq!(at(&snapshot, "/rounds"), &json!([]));
    assert_eq!(at(&snapshot, "/holds"), &json!([]));
    assert_eq!(at(&snapshot, "/provenance"), &json!([]));
    assert_eq!(at(&snapshot, "/recovery_digest/rounds/entries"), &json!([]));
    assert_eq!(
        at(&snapshot, "/recovery_digest/rounds/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/entries"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/open_holds/elisions"),
        &json!([])
    );
    assert_eq!(at(&snapshot, "/recovery_digest/deltas/entries"), &json!([]));
    assert_eq!(
        at(&snapshot, "/recovery_digest/deltas/elisions"),
        &json!([])
    );
    assert_eq!(
        at(&snapshot, "/recovery_digest/facts/entries")
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(at(&snapshot, "/recovery_digest/facts/elisions"), &json!([]));
    let fetched_at = at(&snapshot, "/repositories/0/fetch/fetched_at")
        .as_str()
        .expect("fetched_at string");
    assert!(
        is_millisecond_z_timestamp(fetched_at),
        "fetched_at: {fetched_at}"
    );

    let root_arg = root.as_os_str().to_owned();
    let expected = vec![
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "remote".into(),
                "get-url".into(),
                "origin".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "fetch".into(),
                "--no-tags".into(),
                "origin".into(),
                "refs/heads/pce/authority-unreachable/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "FETCH_HEAD^{commit}".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "show-ref".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/heads/pce/authority-unreachable/milestone-3".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "worktree".into(),
                "list".into(),
                "--porcelain".into(),
            ],
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg.clone(),
                "rev-parse".into(),
                "--verify".into(),
                "--quiet".into(),
                "refs/tags/v0.1.16^{}".into(),
            ],
        ),
        invocation(
            "gh",
            argv([
                "pr",
                "list",
                "--head",
                "pce/authority-unreachable/m3-s4",
                "--base",
                "pce/authority-unreachable/milestone-3",
                "--state",
                "all",
                "--limit",
                "1000",
                "--json",
                "number,headRefName,baseRefName,state,mergeCommit",
            ]),
        ),
        invocation(
            "git",
            vec![
                "-C".into(),
                root_arg,
                "merge-base".into(),
                "--is-ancestor".into(),
                "squash-unreachable-oid".into(),
                "fetched-authority-oid".into(),
            ],
        ),
    ];
    assert_eq!(harness.invocations().expect("parse invocations"), expected);
}

fn response(program: &str, argv: Vec<OsString>, exit_code: i32, stdout: &[u8]) -> ScriptedResponse {
    ScriptedResponse {
        program: program.into(),
        argv,
        exit_code,
        stdout: stdout.to_vec(),
        stderr: Vec::new(),
    }
}

fn invocation(program: &str, argv: Vec<OsString>) -> Invocation {
    Invocation {
        program: program.into(),
        argv,
    }
}

fn argv<const N: usize>(arguments: [&str; N]) -> Vec<OsString> {
    arguments.into_iter().map(OsString::from).collect()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn at<'a>(document: &'a Value, pointer: &str) -> &'a Value {
    document
        .pointer(pointer)
        .unwrap_or_else(|| panic!("missing JSON pointer {pointer}"))
}

fn is_millisecond_z_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 24
        && [4, 7].into_iter().all(|index| bytes[index] == b'-')
        && bytes[10] == b'T'
        && [13, 16].into_iter().all(|index| bytes[index] == b':')
        && bytes[19] == b'.'
        && bytes[23] == b'Z'
        && bytes
            .iter()
            .enumerate()
            .filter(|(index, _)| ![4, 7, 10, 13, 16, 19, 23].contains(index))
            .all(|(_, byte)| byte.is_ascii_digit())
}

#[cfg(target_os = "macos")]
#[test]
fn check_in_classifies_a_foreign_owned_process_number_without_aborting_the_report() {
    let root = std::env::temp_dir().join("pce-check-in-foreign-owned");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create scenario root");
    let log_path = root.join("events.jsonl");
    let mut log = Vec::new();
    for sequence in [1_u64, 2] {
        log.extend_from_slice(
            format!(
                "{{\"sequence\":{sequence},\"timestamp\":\"2026-08-09T12:00:00.000Z\",\"kind\":\"dispatch\",\"node\":\"m1-s2\",\"payload\":{{\"role\":\"step-executor\",\"ref\":\"abc123\",\"evidence\":\"fixture\"}}}}\n"
            )
            .as_bytes(),
        );
    }
    fs::write(&log_path, &log).expect("write event log");
    let sidecars = PathBuf::from(format!("{}.dispatches", log_path.display()));
    fs::create_dir(&sidecars).expect("create sidecar directory");
    // PID 1 is launchd: it is always alive and always owned by root, so proc_pidinfo answers
    // EPERM for this unprivileged process. Issuance 2 names an absent process number.
    for (sequence, process_number) in [(1_u64, 1_u32), (2, 999_999)] {
        let artifact = root.join(format!("artifact-{sequence}.json"));
        fs::write(
            sidecars.join(format!("{sequence}.json")),
            format!(
                "{{\"schema_id\":\"pce.dispatch-process-identity\",\"schema_version\":1,\"issuance_sequence\":{sequence},\"process_number\":{process_number},\"process_start_identity\":{{\"seconds_since_unix_epoch\":1723200000,\"microseconds\":123456}},\"required_artifact_path\":\"{}\"}}\n",
                artifact.display()
            ),
        )
        .expect("write sidecar");
    }
    let output = bounded_output(
        Command::new(env!("CARGO_BIN_EXE_pce"))
            .args([
                "dispatch",
                "check-in",
                "--file",
                log_path.to_str().expect("UTF-8 log path"),
            ])
            .env_clear(),
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stderr, b"");
    assert_eq!(
        output.stdout,
        b"{\"schema_id\":\"pce.dispatch-check-in\",\"schema_version\":1,\"dispatches\":[{\"issuance_sequence\":1,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"},{\"issuance_sequence\":2,\"state\":\"dead\",\"completion\":\"unaccounted\",\"artifact_production\":\"not-produced\"}]}\n"
    );
}
