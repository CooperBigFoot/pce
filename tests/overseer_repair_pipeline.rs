use std::cell::Cell;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use pce_core::{
    BuiltRepair, DispatchRepairDecision, EventTimestamp, FleetRunObservation, HoldIdentity,
    HoldStore, InstallRepairDecision, OverseerJournal, QueueRepairState, RepairDispatch,
    RepairFailureStage, RepairRecord, RunRegistration, VerifiedDefect, accept_repair,
    derive_queue_view, dispatch_repair, install_repair, render_queue_html,
};
use tempfile::tempdir;

fn at(second: u32) -> EventTimestamp {
    EventTimestamp::new(
        Utc.with_ymd_and_hms(2026, 8, 21, 12, 0, second)
            .single()
            .expect("fixture timestamp"),
    )
}

fn defect(root: &std::path::Path) -> VerifiedDefect {
    VerifiedDefect::parse(
        "a".repeat(64),
        "src/check.rs:17 accepts a successful no-op revert",
        "cargo test amendment_counterfactual_noop -- --exact",
        "Reject a no-op amendment counterfactual",
        root,
        "1".repeat(40),
        root.join("repair-worktree"),
        root.join("repair-target"),
        root.join("installed/pce"),
    )
    .expect("verified defect")
}

fn built(dispatch: &RepairDispatch) -> BuiltRepair {
    BuiltRepair::parse(
        "2".repeat(40),
        dispatch.target_directory().join("release/pce"),
        "b".repeat(64),
        vec![PathBuf::from("crates/core/src/check.rs")],
    )
    .expect("built repair")
}

#[test]
fn a_verified_defect_dispatches_once_and_failures_are_retained() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path().join("store"));
    let defect = defect(directory.path());
    let calls = Cell::new(0);

    let first = dispatch_repair(&store, &defect, at(1), |dispatch| {
        calls.set(calls.get() + 1);
        Ok(built(dispatch))
    })
    .expect("first dispatch");
    assert!(matches!(first, DispatchRepairDecision::Built { .. }));

    let second = dispatch_repair(&store, &defect, at(2), |_| {
        calls.set(calls.get() + 1);
        Err("must not run twice".to_owned())
    })
    .expect("idempotent dispatch");
    assert_eq!(second, DispatchRepairDecision::AlreadyDispatched);
    assert_eq!(calls.get(), 1);

    let overlapping = VerifiedDefect::parse(
        "e".repeat(64),
        "src/check.rs:17 reports a second symptom",
        "cargo test second_symptom -- --exact",
        "A second repair in the same file",
        directory.path(),
        "1".repeat(40),
        directory.path().join("overlap-worktree"),
        directory.path().join("overlap-target"),
        directory.path().join("installed/pce"),
    )
    .expect("overlapping defect");
    let overlap = dispatch_repair(&store, &overlapping, at(2), |_| {
        calls.set(calls.get() + 1);
        Err("overlapping worker must not start".to_owned())
    })
    .expect("path conflict");
    assert!(matches!(
        overlap,
        DispatchRepairDecision::PathConflict { .. }
    ));
    assert_eq!(calls.get(), 1);

    let records = store.repair_records().expect("repair records");
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record, RepairRecord::DefectBriefDispatched { .. }))
            .count(),
        1
    );

    let failed_defect = VerifiedDefect::parse(
        "c".repeat(64),
        "src/install.rs:9 cannot rename the staged binary",
        "cargo test atomic_install_failure -- --exact",
        "Record an atomic install failure",
        directory.path(),
        "1".repeat(40),
        directory.path().join("failed-worktree"),
        directory.path().join("failed-target"),
        directory.path().join("installed/pce"),
    )
    .expect("second defect");
    let failed_store = HoldStore::new(directory.path().join("failed-store"));
    let failed = dispatch_repair(&failed_store, &failed_defect, at(3), |_| {
        Err("worker exited 17".to_owned())
    })
    .expect("recorded dispatch failure");
    assert!(matches!(failed, DispatchRepairDecision::Failed { .. }));
    assert!(failed_store.repair_records().expect("records").iter().any(|record| {
        matches!(record, RepairRecord::Failed { stage: RepairFailureStage::Dispatch, detail, .. }
            if detail == "worker exited 17")
    }));
    let retried = dispatch_repair(&failed_store, &failed_defect, at(4), |dispatch| {
        Ok(built(dispatch))
    })
    .expect("retry failed worker");
    assert!(matches!(retried, DispatchRepairDecision::Built { .. }));
}

#[test]
fn accepted_repair_waits_for_busy_run_then_installs_and_notifies_every_run() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path().join("store"));
    let defect = defect(directory.path());
    let dispatch = match dispatch_repair(&store, &defect, at(1), |dispatch| Ok(built(dispatch)))
        .expect("dispatch")
    {
        DispatchRepairDecision::Built { dispatch, .. } => dispatch,
        other => panic!("unexpected dispatch: {other:?}"),
    };
    accept_repair(&store, dispatch.id(), "human", at(2)).expect("accept once");
    accept_repair(&store, dispatch.id(), "human", at(3)).expect("accept twice is idempotent");

    let installs = Cell::new(0);
    let busy = vec![FleetRunObservation::mid_dispatch(
        "a".repeat(64),
        PathBuf::from("/tmp/run-a.jsonl"),
        "OQ6",
        7,
    )];
    let waiting = install_repair(&store, &busy, at(4), |_| {
        installs.set(installs.get() + 1);
        Ok(())
    })
    .expect("wait decision");
    assert!(matches!(
        waiting,
        InstallRepairDecision::WaitingForFleet { .. }
    ));
    assert_eq!(installs.get(), 0);

    let quiet = vec![
        FleetRunObservation::quiet("a".repeat(64), PathBuf::from("/tmp/run-a.jsonl")),
        FleetRunObservation::quiet("b".repeat(64), PathBuf::from("/tmp/run-b.jsonl")),
    ];
    let installed = install_repair(&store, &quiet, at(5), |_| {
        installs.set(installs.get() + 1);
        Ok(())
    })
    .expect("install decision");
    assert!(matches!(installed, InstallRepairDecision::Installed { .. }));
    assert_eq!(installs.get(), 1);

    let records = store.repair_records().expect("repair records");
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record, RepairRecord::InstallAccepted { .. }))
            .count(),
        1
    );
    assert_eq!(
        records
            .iter()
            .filter(|record| matches!(record, RepairRecord::RunNotified { .. }))
            .count(),
        2
    );
    assert!(records.iter().any(|record| {
        matches!(record, RepairRecord::Installed { digest, .. } if digest == &"b".repeat(64))
    }));
}

#[test]
fn a_forever_busy_install_remains_visible_with_age_and_blocker() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path().join("store"));
    let defect = defect(directory.path());
    let dispatch = match dispatch_repair(&store, &defect, at(1), |dispatch| Ok(built(dispatch)))
        .expect("dispatch")
    {
        DispatchRepairDecision::Built { dispatch, .. } => dispatch,
        other => panic!("unexpected dispatch: {other:?}"),
    };
    accept_repair(&store, dispatch.id(), "human", at(2)).expect("accept");
    let blocker = "d".repeat(64);
    let fleet = vec![FleetRunObservation::mid_dispatch(
        &blocker,
        PathBuf::from("/tmp/blocked.jsonl"),
        "OQ6",
        9,
    )];
    let _ = install_repair(&store, &fleet, at(3), |_| Ok(())).expect("defer install");

    let view = derive_queue_view(
        &store,
        &OverseerJournal::new(store.root()),
        Utc.with_ymd_and_hms(2026, 8, 21, 12, 2, 1)
            .single()
            .expect("view time"),
        Duration::from_secs(5),
    )
    .expect("queue view");
    assert_eq!(view.repairs.len(), 1);
    assert!(view.repairs[0].age_seconds >= 120);
    assert!(matches!(
        &view.repairs[0].state,
        QueueRepairState::WaitingForFleet { blocking_runs } if blocking_runs == &vec![blocker]
    ));
    let html = render_queue_html(&view);
    assert!(html.contains("1 defect briefs dispatched"));
    assert!(html.contains("1 pending installs"));
    assert!(html.contains("data-state=\"waiting-for-fleet\""));
}

#[test]
fn repair_list_cli_reads_the_durable_store() {
    let directory = tempdir().expect("temporary store");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["overseer", "repair-list", "--root"])
        .arg(directory.path())
        .output()
        .expect("run repair-list");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 output"),
        "[]\n"
    );
}

#[test]
fn fleet_membership_cannot_change_during_install_observation() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path().join("store"));
    let vision = directory.path().join("vision");
    std::fs::create_dir_all(&vision).expect("vision directory");
    let registration = RunRegistration::parse(
        "pce",
        &vision,
        vision.join("graph.json"),
        vision.join("journal.jsonl"),
        None,
    )
    .expect("registration");
    let lease = store.lease_run_registrations().expect("fleet lease");
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let worker_store = store.clone();
    let worker = thread::spawn(move || {
        started_tx.send(()).expect("started");
        let result = worker_store.register_run(registration, at(1));
        done_tx.send(result).expect("registration result");
    });
    started_rx.recv().expect("registration started");
    assert!(done_rx.recv_timeout(Duration::from_millis(100)).is_err());
    drop(lease);
    done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("registration released")
        .expect("register run");
    worker.join().expect("registration thread");
}

#[test]
fn defect_dispatch_refuses_a_claim_that_does_not_quote_the_source() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path().join("store"));
    let opened = store
        .open(
            HoldIdentity::parse("pce", "1", "OQ6", "binary-defect").expect("identity"),
            serde_json::json!({"finding":"fixture"}),
            at(1),
        )
        .expect("source hold");
    let output = Command::new(env!("CARGO_BIN_EXE_pce"))
        .args(["overseer", "defect-dispatch", "--root"])
        .arg(store.root())
        .args(["--source-hold", opened.hold().key().as_str()])
        .args(["--code-fact", "src/main.rs:1 invented claim"])
        .args(["--reproduction", "false"])
        .args(["--summary", "Reject unverified evidence"])
        .args(["--repository-root", env!("CARGO_MANIFEST_DIR")])
        .args(["--installed-binary"])
        .arg(directory.path().join("installed/pce"))
        .args(["--worker-program", "/usr/bin/false"])
        .output()
        .expect("run defect-dispatch");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exact trimmed source line"));
    assert!(store.repair_records().expect("repair records").is_empty());
}
