use std::fs;
use std::process::Command;

use chrono::{TimeZone, Utc};
use pce_core::{EventTimestamp, HoldIdentity, HoldStore, HoldStoreError, OpenDisposition};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn asked_once() {
    let directory = tempdir().expect("temporary hold root should create");
    let store = HoldStore::new(directory.path());
    let identity = HoldIdentity::parse("pce", "plan-v1", "OQ1", "route-question")
        .expect("hold identity should parse");
    let timestamp = Utc
        .with_ymd_and_hms(2026, 8, 21, 12, 0, 0)
        .single()
        .expect("fixture timestamp should exist");

    let first = store
        .open(
            identity.clone(),
            json!({"question":"Which route?"}),
            EventTimestamp::new(timestamp),
        )
        .expect("first open should create a hold");
    assert_eq!(first.disposition(), OpenDisposition::Created);
    let key = first.hold().key().clone();
    let first_bytes = fs::read(directory.path().join(format!("{}.jsonl", key.as_str())))
        .expect("created hold should read");

    for _ in 0..2 {
        let repeated = store
            .open(
                identity.clone(),
                json!({"question":"A restarted run may phrase this differently"}),
                EventTimestamp::new(timestamp),
            )
            .expect("restarted open should find the existing hold");
        assert_eq!(repeated.disposition(), OpenDisposition::AlreadyExists);
        assert_eq!(repeated.hold().key(), &key);
        assert_eq!(store.list().expect("holds should list").len(), 1);
    }

    let hold_files: Vec<_> = fs::read_dir(directory.path())
        .expect("hold root should list")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert_eq!(hold_files.len(), 1);
    assert_eq!(
        fs::read(&hold_files[0]).expect("hold file should read"),
        first_bytes,
        "idempotent open must not append or rewrite the hold"
    );
}

#[test]
fn legacy_verb_surface_unchanged() {
    let directory = tempdir().expect("temporary directory should create");
    let impossible_store = directory.path().join("hold-store-must-remain-absent");
    let binary = env!("CARGO_BIN_EXE_pce");
    let missing_log = directory.path().join("legacy-events.jsonl");
    let missing_graph = directory.path().join("legacy-graph.json");
    let missing_vision = directory.path().join("legacy-vision");

    let cases = [
        (
            vec![
                "dispatch".to_owned(),
                "check-in".to_owned(),
                "--file".to_owned(),
                missing_log.display().to_string(),
            ],
            "failed to open event log",
        ),
        (
            vec![
                "criteria".to_owned(),
                "check".to_owned(),
                "--file".to_owned(),
                missing_log.display().to_string(),
                "--vision-dir".to_owned(),
                missing_vision.display().to_string(),
            ],
            "failed to parse proposed acceptance criteria",
        ),
        (
            vec![
                "package".to_owned(),
                "driver-status".to_owned(),
                "--graph".to_owned(),
                missing_graph.display().to_string(),
                "--journal".to_owned(),
                missing_log.display().to_string(),
            ],
            "failed to read graph",
        ),
        (
            vec![
                "status".to_owned(),
                "--file".to_owned(),
                missing_log.display().to_string(),
                "--vision-dir".to_owned(),
                missing_vision.display().to_string(),
            ],
            "failed to open event log",
        ),
    ];

    for (arguments, recorded_diagnostic) in cases {
        let output = Command::new(binary)
            .args(arguments)
            .env("PCE_HOLD_STORE_ROOT", &impossible_store)
            .output()
            .expect("legacy command should execute");
        assert!(!output.status.success());
        assert_eq!(output.stdout, b"");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(recorded_diagnostic),
            "legacy diagnostic changed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !impossible_store.exists(),
            "an existing verb must not initialize or inspect the hold store"
        );
    }
}

#[test]
fn incomplete_tail_is_not_authoritative() {
    let directory = tempdir().expect("temporary hold root should create");
    let store = HoldStore::new(directory.path());
    let identity = HoldIdentity::parse("pce", "plan-v1", "OQ1", "torn-question")
        .expect("hold identity should parse");
    let timestamp = Utc
        .with_ymd_and_hms(2026, 8, 21, 12, 0, 0)
        .single()
        .expect("fixture timestamp should exist");
    let opened = store
        .open(
            identity,
            json!({"question":"Was this record fully appended?"}),
            EventTimestamp::new(timestamp),
        )
        .expect("hold should open");
    let key = opened.hold().key();
    let path = directory.path().join(format!("{}.jsonl", key.as_str()));
    let mut bytes = fs::read(&path).expect("hold file should read");
    assert_eq!(bytes.pop(), Some(b'\n'));
    fs::write(&path, bytes).expect("torn fixture should write");

    assert!(matches!(
        store.read(key),
        Err(HoldStoreError::IncompleteTail { .. })
    ));
}
