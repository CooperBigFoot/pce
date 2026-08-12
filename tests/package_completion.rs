use std::fs;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde_json::Value;
use tempfile::tempdir;

fn pce() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pce"))
}

fn result_path(vision: &Path) -> PathBuf {
    vision.join(".pce/package-results/WP4/1.json")
}

fn run_wrapper(vision: &Path, code: i32) -> std::process::Output {
    let artifact = vision.join("artifact");
    pce()
        .args(["dispatch", "package-worker", "--result"])
        .arg(result_path(vision))
        .args(["--required-artifact"])
        .arg(artifact)
        .args(["--", "/bin/sh", "-c", &format!("exit {code}")])
        .output()
        .expect("wrapper executes")
}

#[test]
fn wrapper_records_zero_and_three_as_results() {
    for code in [0, 3] {
        let directory = tempdir().expect("temporary directory");
        let output = run_wrapper(directory.path(), code);
        assert_eq!(output.status.code(), Some(code));
        let value: Value =
            serde_json::from_slice(&fs::read(result_path(directory.path())).expect("result file"))
                .expect("result JSON");
        assert_eq!(value["exit_status"]["kind"], "exited");
        assert_eq!(value["exit_status"]["code"], code);
        assert!(value["stopped_at"].as_str().is_some());
    }
}

#[test]
fn signaled_child_is_recorded_but_killed_wrapper_leaves_no_result() {
    let signaled = tempdir().expect("temporary directory");
    let output = pce()
        .args(["dispatch", "package-worker", "--result"])
        .arg(result_path(signaled.path()))
        .args(["--required-artifact"])
        .arg(signaled.path().join("artifact"))
        .args(["--", "/bin/sh", "-c", "kill -KILL $$"])
        .output()
        .expect("wrapper executes");
    assert_eq!(output.status.code(), Some(137));
    let value: Value =
        serde_json::from_slice(&fs::read(result_path(signaled.path())).expect("signal result"))
            .expect("result JSON");
    assert_eq!(value["exit_status"]["kind"], "signaled");
    assert_eq!(value["exit_status"]["signal"], 9);

    let killed = tempdir().expect("temporary directory");
    let mut wrapper = pce()
        .args(["dispatch", "package-worker", "--result"])
        .arg(result_path(killed.path()))
        .args(["--required-artifact"])
        .arg(killed.path().join("artifact"))
        .args(["--", "/bin/sleep", "1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("wrapper starts");
    thread::sleep(Duration::from_millis(100));
    assert!(
        Command::new("/bin/kill")
            .args(["-KILL", &wrapper.id().to_string()])
            .status()
            .expect("kill")
            .success()
    );
    let status = wrapper.wait().expect("wrapper wait");
    assert_eq!(status.signal(), Some(9));
    thread::sleep(Duration::from_millis(1100));
    assert!(!result_path(killed.path()).exists());
}

fn seed_issuance(log: &Path) {
    fs::write(log, r#"{"sequence":1,"timestamp":"2026-08-12T12:00:00.000Z","kind":"dispatch","node":"WP4","payload":{"role":"work-package-worker","ref":"abc","evidence":"fixture"}}
"#).expect("issuance log");
}

#[test]
fn collection_rederives_from_disk_and_never_consults_herdr() {
    let directory = tempdir().expect("temporary directory");
    let vision = directory.path().join("vision");
    fs::create_dir(&vision).expect("vision");
    let log = directory.path().join("events.jsonl");
    seed_issuance(&log);
    let output = run_wrapper(&vision, 3);
    assert_eq!(output.status.code(), Some(3));

    let bin = directory.path().join("bin");
    fs::create_dir(&bin).expect("bin");
    let marker = directory.path().join("herdr-called");
    fs::write(
        bin.join("herdr"),
        format!("#!/bin/sh\ntouch '{}'\nexit 97\n", marker.display()),
    )
    .expect("sentinel");
    let mut permissions = fs::metadata(bin.join("herdr"))
        .expect("metadata")
        .permissions();
    use std::os::unix::fs::PermissionsExt;
    permissions.set_mode(0o755);
    fs::set_permissions(bin.join("herdr"), permissions).expect("permissions");

    let collect = || {
        pce()
            .args(["dispatch", "package-completions", "--file"])
            .arg(&log)
            .args(["--vision-dir"])
            .arg(&vision)
            .env("PATH", &bin)
            .output()
            .expect("collect")
    };
    let first = collect();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first_json: Value = serde_json::from_slice(&first.stdout).expect("first JSON");
    assert_eq!(first_json["appended"], serde_json::json!([1]));
    assert_eq!(first_json["packages"][0]["state"], "finished");
    assert_eq!(first_json["packages"][0]["exit_status"]["code"], 3);
    let second = collect();
    assert!(second.status.success());
    let second_json: Value = serde_json::from_slice(&second.stdout).expect("second JSON");
    assert_eq!(second_json["appended"], serde_json::json!([]));
    assert_eq!(second_json["packages"], first_json["packages"]);
    assert_eq!(fs::read_to_string(&log).expect("log").lines().count(), 2);
    assert!(!marker.exists(), "completion derivation consulted herdr");
}

#[test]
fn absent_result_remains_unaccounted_without_completion() {
    let directory = tempdir().expect("temporary directory");
    let vision = directory.path().join("vision");
    fs::create_dir(&vision).expect("vision");
    let log = directory.path().join("events.jsonl");
    seed_issuance(&log);
    let output = pce()
        .args(["dispatch", "package-completions", "--file"])
        .arg(&log)
        .args(["--vision-dir"])
        .arg(&vision)
        .output()
        .expect("collect");
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["appended"], serde_json::json!([]));
    assert_eq!(value["packages"][0]["state"], "unaccounted");
    assert_eq!(fs::read_to_string(log).expect("log").lines().count(), 1);
}
