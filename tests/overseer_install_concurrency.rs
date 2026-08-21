use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use pce_core::{
    EventTimestamp, HoldStore, InstallDecision, InstallRequest, fold_dispatch_ledger,
    overseer_records, request_install,
};
use tempfile::tempdir;

fn at(second: u32) -> EventTimestamp {
    EventTimestamp::new(
        Utc.with_ymd_and_hms(2026, 8, 21, 12, 0, second)
            .single()
            .expect("fixture timestamp"),
    )
}

#[test]
fn concurrent_identical_requests_run_the_installer_once() {
    let directory = tempdir().expect("temporary store");
    let store = HoldStore::new(directory.path());
    let request = InstallRequest::parse("install-0.1.17", "0.1.17").expect("request");
    let ledger = fold_dispatch_ledger(&[]).expect("quiet ledger");
    let (first_entered_tx, first_entered_rx) = mpsc::channel();
    let (release_first_tx, release_first_rx) = mpsc::channel();

    let first_store = store.clone();
    let first_request = request.clone();
    let first_ledger = ledger.clone();
    let first = thread::spawn(move || {
        request_install(&first_store, &first_request, &first_ledger, at(1), || {
            first_entered_tx
                .send(())
                .map_err(|error| error.to_string())?;
            release_first_rx.recv().map_err(|error| error.to_string())
        })
    });
    first_entered_rx
        .recv()
        .expect("first installer should enter");

    let (second_entered_tx, second_entered_rx) = mpsc::channel();
    let second_store = store.clone();
    let second_request = request.clone();
    let second_ledger = ledger.clone();
    let second = thread::spawn(move || {
        request_install(
            &second_store,
            &second_request,
            &second_ledger,
            at(2),
            || {
                second_entered_tx
                    .send(())
                    .map_err(|error| error.to_string())
            },
        )
    });

    let second_entered_while_first_was_running = second_entered_rx
        .recv_timeout(Duration::from_secs(2))
        .is_ok();
    release_first_tx.send(()).expect("release first installer");
    let first_decision = first.join().expect("first thread").expect("first request");
    let second_decision = second
        .join()
        .expect("second thread")
        .expect("second request");

    assert!(
        !second_entered_while_first_was_running,
        "a duplicate installer entered before the first installation completed"
    );
    assert_eq!(
        [first_decision, second_decision]
            .into_iter()
            .filter(|decision| *decision == InstallDecision::Installed)
            .count(),
        1
    );
    assert_eq!(overseer_records(&store).expect("policy records").len(), 2);
}
