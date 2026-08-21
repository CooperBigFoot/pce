use std::fs;
use std::process::Command;

use serde_json::Value;
use tempfile::tempdir;

#[test]
fn run_registers_without_a_human_step() {
    let directory = tempdir().expect("temporary root should create");
    let store = directory.path().join("holds");
    let vision = directory.path().join("vision");
    fs::create_dir_all(&vision).expect("vision directory should create");
    let graph = vision.join("graph.v7.json");
    let journal = vision.join("driver-journal.jsonl");
    fs::write(&graph, b"{}\n").expect("graph fixture should write");
    let binary = env!("CARGO_BIN_EXE_pce");
    let arguments = [
        "hold",
        "register",
        "--root",
        store.to_str().expect("store path should be UTF-8"),
        "--repository",
        "pce",
        "--vision-dir",
        vision.to_str().expect("vision path should be UTF-8"),
        "--frozen-graph",
        graph.to_str().expect("graph path should be UTF-8"),
        "--journal",
        journal.to_str().expect("journal path should be UTF-8"),
        "--herdr-session",
        "pce-test",
    ];

    for expected_disposition in ["created", "already-current"] {
        let output = Command::new(binary)
            .args(arguments)
            .output()
            .expect("registration command should execute");
        assert!(
            output.status.success(),
            "registration failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value =
            serde_json::from_slice(&output.stdout).expect("registration output should be JSON");
        assert_eq!(result["disposition"], expected_disposition);
    }

    let registration_files: Vec<_> = fs::read_dir(store.join("runs"))
        .expect("registration directory should list")
        .map(|entry| entry.expect("registration entry should read").path())
        .collect();
    assert_eq!(registration_files.len(), 1);
    let registration_bytes =
        fs::read(&registration_files[0]).expect("registration stream should read");
    assert_eq!(
        registration_bytes
            .iter()
            .filter(|byte| **byte == b'\n')
            .count(),
        1,
        "an identical relaunch must not append a duplicate registration"
    );

    let output = Command::new(binary)
        .args([
            "hold",
            "runs",
            "--root",
            store.to_str().expect("store path should be UTF-8"),
        ])
        .output()
        .expect("run list should execute");
    assert!(output.status.success());
    let runs: Value = serde_json::from_slice(&output.stdout).expect("run list should be JSON");
    let runs = runs.as_array().expect("run list should be an array");
    assert_eq!(runs.len(), 1, "relaunch must retain one logical run");
    assert_eq!(runs[0]["repository"], "pce");
    assert_eq!(runs[0]["vision_directory"], vision.display().to_string());
    assert_eq!(runs[0]["frozen_graph"], graph.display().to_string());
    assert_eq!(runs[0]["journal"], journal.display().to_string());
    assert_eq!(runs[0]["herdr_session"], "pce-test");
}

#[test]
fn blocked_run_opens_a_hold() {
    let skill = fs::read_to_string("skills/work-graph/SKILL.md")
        .expect("work-graph skill should be readable");
    let terminal = skill
        .split("## 7. Hold or promote at the terminal boundary")
        .nth(1)
        .expect("skill should define the terminal hold boundary");
    assert!(terminal.contains("pce hold open --repository"));
    assert!(terminal.contains("--question-kind work-graph-terminal-stop"));
    assert!(terminal.contains("exact terminal state and"));
    assert!(terminal.contains("read that key from the store"));
    assert!(terminal.contains("wait: do not notify the human separately"));
    assert!(
        !terminal.contains(r#"herdr notification show "pce graph stopped""#),
        "a blocked run must not use the old bare-notification stop path"
    );
}

#[test]
fn registration_refuses_an_impossible_herdr_session() {
    let directory = tempdir().expect("temporary root should create");
    let vision = directory.path().join("vision");
    let store = directory.path().join("holds");
    let graph = vision.join("graph.v1.json");
    let journal = vision.join("driver-journal.jsonl");
    fs::create_dir_all(&vision).expect("vision directory should create");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args([
            "hold",
            "register",
            "--root",
            store.to_str().expect("store path should be UTF-8"),
            "--repository",
            "pce",
            "--vision-dir",
            vision.to_str().expect("vision path should be UTF-8"),
            "--frozen-graph",
            graph.to_str().expect("graph path should be UTF-8"),
            "--journal",
            journal.to_str().expect("journal path should be UTF-8"),
            "--herdr-session",
            "../bad",
        ])
        .output()
        .expect("registration command should execute");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid Herdr session name"),
        "diagnostic should name the invalid domain value: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
