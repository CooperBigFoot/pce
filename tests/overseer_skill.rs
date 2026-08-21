use std::fs;

use chrono::{TimeZone, Utc};
use pce_core::EventTimestamp;
use pce_core::hold_store::{HoldIdentity, HoldRoute, HoldState, HoldStore};
use serde_json::json;
use tempfile::tempdir;

fn timestamp(second: u32) -> EventTimestamp {
    EventTimestamp::new(
        Utc.with_ymd_and_hms(2026, 8, 21, 12, 0, second)
            .single()
            .unwrap(),
    )
}

#[test]
fn a_respawned_session_reads_the_pending_thread() {
    let directory = tempdir().unwrap();
    let store = HoldStore::new(directory.path());
    let opened = store
        .open(
            HoldIdentity::parse("pce", "v1", "OQ6", "fleet-fact").unwrap(),
            json!({
                "question": "Should this run retry?",
                "options": ["retry", "wait"],
                "fact": "the daemon journal is outside the reporting run"
            }),
            timestamp(0),
        )
        .unwrap();
    let key = opened.hold().key().clone();
    store.route(&key, HoldRoute::Human, timestamp(1)).unwrap();
    let clarification = "Which run wrote the last daemon journal record?";
    store
        .answer(
            &key,
            "human".to_owned(),
            clarification.to_owned(),
            timestamp(2),
        )
        .unwrap();
    store
        .route(&key, HoldRoute::Overseer, timestamp(3))
        .unwrap();

    // A new session has no input except the store. Reconstruction lists identities, then reads
    // each hold again so the complete append-only thread is its authority.
    let reconstructed = store
        .list()
        .unwrap()
        .into_iter()
        .map(|hold| store.read(hold.key()).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(reconstructed.len(), 1);
    assert_eq!(
        reconstructed[0].state(),
        HoldState::Open {
            route: HoldRoute::Overseer
        }
    );
    assert_eq!(reconstructed[0].records().len(), 4);
    let encoded = serde_json::to_string(&reconstructed[0]).unwrap();
    assert!(encoded.contains(clarification));
    assert!(encoded.contains("daemon journal is outside the reporting run"));
    assert!(!encoded.contains("repeat"));
}

#[test]
fn the_skill_reconstructs_on_every_invocation() {
    let skill = fs::read_to_string("skills/overseer/SKILL.md").unwrap();
    let lower = skill.to_lowercase();
    assert!(skill.contains("Cold reconstruction is mandatory on every invocation."));
    assert!(skill.contains("pce hold runs --root"));
    assert!(skill.contains("pce hold list --root"));
    assert!(skill.contains("pce hold read --root"));
    assert!(skill.contains("Read the complete"));
    assert!(skill.contains("Never ask the human to repeat"));
    assert!(!lower.contains("compact"));
    assert!(!lower.contains("carry state between turns"));

    let runs = skill.find("pce hold runs --root").unwrap();
    let list = skill.find("pce hold list --root").unwrap();
    let read = skill.find("pce hold read --root").unwrap();
    assert!(runs < list && list < read);
}

#[test]
fn the_skill_uses_the_store_root_exported_by_the_server() {
    let server = fs::read_to_string("src/main.rs").unwrap();
    let skill = fs::read_to_string("skills/overseer/SKILL.md").unwrap();

    assert!(server.contains(".env(\"PCE_HOLD_STORE_ROOT\", store_root)"));
    assert!(skill.contains("`PCE_HOLD_STORE_ROOT` names the store root."));
    assert!(skill.matches("$PCE_HOLD_STORE_ROOT").count() >= 5);
    assert!(!skill.contains("$PCE_HOLD_STORE\""));
}
