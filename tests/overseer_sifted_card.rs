//! The operator reads a sifted card, not the orchestrator's raw report.
//!
//! The sifting criteria test what the skill produces. These test what the operator sees, which is
//! the half that shipped unwired: the store had no record kind for a card, so the page rendered the
//! reporting run's own text.

use std::time::Duration;

use pce_core::{
    EventTimestamp, HoldIdentity, HoldRoute, HoldStore, HoldStoreError, OverseerJournal,
    SiftedBlock, SiftedCard, SiftedOption, derive_queue_view, render_queue_html,
};
use serde_json::json;

fn at(second: u32) -> EventTimestamp {
    EventTimestamp::new(
        chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 8, 21, 12, 0, second)
            .single()
            .expect("fixture time"),
    )
}

fn store_with_hold(question_kind: &str, report: &str) -> (tempfile::TempDir, HoldStore, String) {
    let directory = tempfile::tempdir().expect("store root");
    let store = HoldStore::new(directory.path().join("holds"));
    let opened = store
        .open(
            HoldIdentity::parse("pourpoint", "19", "GD10", question_kind).expect("identity"),
            json!(report),
            at(0),
        )
        .expect("open hold");
    let key = opened.hold().key().as_str().to_owned();
    (directory, store, key)
}

fn card() -> SiftedCard {
    SiftedCard {
        by: "overseer".to_owned(),
        title: "How strong must the read proof be?".to_owned(),
        blocks: vec![SiftedBlock {
            heading: "What happened".to_owned(),
            body: "An earlier repair removed a check and put nothing in its place.".to_owned(),
        }],
        overseer_facts: vec![
            "This is the third time in this corpus that a repair removed a check.".to_owned(),
        ],
        options: vec![
            SiftedOption {
                option: "Require evidence that the finished worker read a map file.".to_owned(),
                note: Some("Strongest.".to_owned()),
                recommended_by_run: true,
            },
            SiftedOption {
                option: "Rely on the live run.".to_owned(),
                note: None,
                recommended_by_run: false,
            },
        ],
        consequence: Some(
            "Nothing in the finished product would prove the worker read the data".to_owned(),
        ),
    }
}

fn html_for(store: &HoldStore) -> String {
    let journal = OverseerJournal::new(store.root());
    let view = derive_queue_view(store, &journal, chrono::Utc::now(), Duration::from_secs(5))
        .expect("view");
    render_queue_html(&view)
}

/// The defect itself: before a card existed, the page could only render the run's own words.
#[test]
fn a_routed_hold_renders_its_card_and_keeps_the_report_reachable() {
    let report = "verify_case no longer requires retained evidence, so the read gate is inert.";
    let (_directory, store, key) = store_with_hold("criterion-revision", report);
    let hold_key = pce_core::HoldKey::parse(key).expect("key");

    store
        .sift(&hold_key, "overseer".to_owned(), card(), at(1))
        .expect("sift");

    let html = html_for(&store);
    assert!(
        html.contains("How strong must the read proof be?"),
        "the card's own question must be what the operator reads"
    );
    assert!(
        html.contains("An earlier repair removed a check"),
        "the translated prose must be rendered"
    );
    // The card is an additional record, never a replacement.
    assert!(
        html.contains("verify_case no longer requires retained evidence"),
        "the opening report must stay reachable"
    );
    assert!(
        html.contains("The run&#x27;s own report") || html.contains("The run's own report"),
        "the raw report must be reachable behind its own disclosure"
    );
    // Whose claim is whose.
    assert!(
        html.contains("Overseer fact"),
        "an overseer-supplied fact must be labelled"
    );
    assert!(
        html.contains("the run&#x27;s recommendation") || html.contains("the run's recommendation"),
        "the run's own recommendation must be attributed to the run"
    );
}

/// Modal force survives translation, or the card is refused.
#[test]
fn a_hedged_report_cannot_be_promoted_into_a_flat_assertion() {
    let (_directory, store, key) = store_with_hold(
        "environment-failure",
        "The environment may have failed during setup.",
    );
    let hold_key = pce_core::HoldKey::parse(key).expect("key");

    let mut promoted = card();
    promoted.consequence = None;
    promoted.title = "The environment failed during setup.".to_owned();
    promoted.blocks = vec![SiftedBlock {
        heading: "What happened".to_owned(),
        body: "Setup broke and the run stopped.".to_owned(),
    }];
    promoted.overseer_facts.clear();
    promoted.options = vec![SiftedOption {
        option: "Rerun it.".to_owned(),
        note: None,
        recommended_by_run: true,
    }];
    let refused = store.sift(&hold_key, "overseer".to_owned(), promoted.clone(), at(1));
    assert!(
        matches!(refused, Err(HoldStoreError::ModalForcePromoted { .. })),
        "a card that drops the report's hedge must be refused, got {refused:?}"
    );

    let mut preserved = promoted;
    preserved.title = "The environment may have failed during setup.".to_owned();
    store
        .sift(&hold_key, "overseer".to_owned(), preserved, at(2))
        .expect("a card that preserves the hedge is accepted");

    let html = html_for(&store);
    assert!(
        html.contains("may have failed"),
        "the hedge must reach the operator"
    );
}

/// A report naming no options belongs back with its run, never in front of the human with options
/// the sifter invented.
#[test]
fn a_report_naming_no_options_produces_no_human_card() {
    let (_directory, store, key) = store_with_hold("which-way", "Which way should I go?");
    let hold_key = pce_core::HoldKey::parse(key).expect("key");

    let mut optionless = card();
    optionless.consequence = None;
    optionless.options.clear();
    let refused = store.sift(&hold_key, "overseer".to_owned(), optionless, at(1));
    assert!(
        matches!(refused, Err(HoldStoreError::CardNamesNoOption)),
        "got {refused:?}"
    );

    // The existing path for such a report: back to its own run.
    let hold = store
        .route(&hold_key, HoldRoute::ReportingRun, at(2))
        .expect("route back");
    assert!(
        hold.card().is_none(),
        "no card may exist for an optionless report"
    );
}

/// A door card carries exactly one consequence sentence; a non-door card carries none.
#[test]
fn only_a_door_card_carries_one_consequence_sentence() {
    // Non-door kind with a consequence: refused.
    let (_a, non_door_store, non_door_key) = store_with_hold("which-way", "Pick one of two.");
    let non_door = pce_core::HoldKey::parse(non_door_key).expect("key");
    let refused = non_door_store.sift(&non_door, "overseer".to_owned(), card(), at(1));
    assert!(
        matches!(
            refused,
            Err(HoldStoreError::NonDoorCardCarriesConsequence { .. })
        ),
        "got {refused:?}"
    );

    // Door kind without one: refused.
    let (_b, door_store, door_key) = store_with_hold("park-overrule", "Two ways to go.");
    let door = pce_core::HoldKey::parse(door_key).expect("key");
    let mut missing = card();
    missing.consequence = None;
    let refused = door_store.sift(&door, "overseer".to_owned(), missing, at(1));
    assert!(
        matches!(
            refused,
            Err(HoldStoreError::DoorCardNeedsConsequence { .. })
        ),
        "got {refused:?}"
    );

    // Door kind with two sentences: refused.
    let mut two = card();
    two.consequence = Some("The proof disappears. Nothing replaces it.".to_owned());
    let refused = door_store.sift(&door, "overseer".to_owned(), two, at(1));
    assert!(
        matches!(
            refused,
            Err(HoldStoreError::ConsequenceIsNotOneSentence { .. })
        ),
        "got {refused:?}"
    );

    // Door kind with exactly one: accepted and rendered.
    door_store
        .sift(&door, "overseer".to_owned(), card(), at(2))
        .expect("one sentence is accepted");
    assert!(html_for(&door_store).contains("class=\"consequence\""));
}

/// A spend authorisation carries a consequence sentence.
///
/// Spend has no attributed-door mechanism, so the first authoring of `DOOR_QUESTION_KINDS` left it
/// out and the store refused every spend card that said what the money buys. That is the one card
/// where the consequence is the whole decision: ruling 12 of the #186 record authorised a live
/// reference host, and the choice turned on whether the run inherits a host whose properties were
/// proved. This fails against that list and passes against the corrected one.
#[test]
fn a_spend_card_carries_its_consequence() {
    let (_directory, store, key) =
        store_with_hold("spend", "Authorise the host, or park the work.");
    let hold_key = pce_core::HoldKey::parse(key).expect("key");

    store
        .sift(&hold_key, "overseer".to_owned(), card(), at(1))
        .expect("a spend card carries what the money buys");

    assert!(
        html_for(&store).contains("class=\"consequence\""),
        "the operator must see what the spend commits them to"
    );
}

/// Sifting routes the hold to the human, and answering hands it to the overseer rather than closing
/// it. The page must say so rather than implying send-and-dismiss.
#[test]
fn answering_does_not_close_a_hold_and_the_page_says_so() {
    let (_directory, store, key) = store_with_hold("park-overrule", "Two ways to go.");
    let hold_key = pce_core::HoldKey::parse(key).expect("key");
    let hold = store
        .sift(&hold_key, "overseer".to_owned(), card(), at(1))
        .expect("sift");
    assert_eq!(
        hold.state(),
        pce_core::HoldState::Open {
            route: HoldRoute::Human
        },
        "sifting routes the hold to the human"
    );

    let html = html_for(&store);
    assert!(
        html.contains("does not close the hold"),
        "the page must not imply that answering dismisses the question"
    );

    let answered = store
        .answer(&hold_key, "human".to_owned(), "Option 1.".to_owned(), at(2))
        .expect("answer");
    assert!(answered.is_open(), "an answered hold stays open");
    assert!(
        answered
            .thread()
            .iter()
            .any(|(who, what)| who == "human" && what == "Option 1."),
        "the answer joins the visible thread"
    );
}

/// The three open states are visible as text, not only as attributes, and a hold with the overseer
/// offers no answer box.
#[test]
fn the_states_are_visible_and_only_the_human_queue_can_be_answered() {
    let (_directory, store, key) = store_with_hold("park-overrule", "Two ways to go.");
    let hold_key = pce_core::HoldKey::parse(key).expect("key");
    store
        .sift(&hold_key, "overseer".to_owned(), card(), at(1))
        .expect("sift");

    let waiting = html_for(&store);
    assert!(waiting.contains("waiting for you"));
    assert!(
        waiting.contains("Your answer"),
        "the human queue is answerable"
    );

    store
        .answer(
            &hold_key,
            "human".to_owned(),
            "What does it cost?".to_owned(),
            at(2),
        )
        .expect("answer");
    store
        .route(&hold_key, HoldRoute::Overseer, at(3))
        .expect("hand to overseer");

    let with_overseer = html_for(&store);
    assert!(with_overseer.contains("with the overseer"));
    assert!(
        !with_overseer.contains("Your answer"),
        "a hold sitting with the overseer must not offer an answer box"
    );
    assert!(
        with_overseer.contains("What does it cost?"),
        "the card shows what the overseer is working on"
    );

    store
        .close(&hold_key, "answered".to_owned(), at(4))
        .expect("close");
    let closed = html_for(&store);
    assert!(
        closed.contains("Settled without you"),
        "settled holds move into their own drawer"
    );
}

/// Liveness is visible, and idle does not look like cannot-start.
#[test]
fn the_four_liveness_states_are_distinguishable() {
    use pce_core::{OverseerEvent, OverseerWakeReason};
    let directory = tempfile::tempdir().expect("root");
    let store = HoldStore::new(directory.path().join("holds"));
    let journal = OverseerJournal::new(store.root());
    let now = chrono::Utc::now();
    let recent = EventTimestamp::new(now);

    let render = |journal: &OverseerJournal| {
        render_queue_html(
            &derive_queue_view(&store, journal, now, Duration::from_secs(60)).expect("view"),
        )
    };

    journal
        .append(&OverseerEvent::Heartbeat { timestamp: recent })
        .expect("heartbeat");
    assert!(render(&journal).contains("overseer is idle"));

    journal
        .append(&OverseerEvent::WakeStarted {
            timestamp: recent,
            reason: OverseerWakeReason::Startup,
        })
        .expect("wake");
    assert!(render(&journal).contains("overseer is working"));

    journal
        .append(&OverseerEvent::SessionExited {
            timestamp: recent,
            code: Some(9),
            detail: None,
        })
        .expect("exit");
    let cannot = render(&journal);
    assert!(cannot.contains("overseer cannot start"));
    assert!(
        !cannot.contains("overseer is idle"),
        "cannot-start must not read as idle"
    );
}

/// Filed feedback accumulates. It is not work, so it must not wake the overseer and must not turn
/// itself into a brief: the operator sits down with it deliberately, in a session he runs.
#[test]
fn filed_feedback_accumulates_and_wakes_nothing() {
    let directory = tempfile::tempdir().expect("root");
    let store = HoldStore::new(directory.path().join("holds"));

    for (index, text) in ["first thought", "second thought", "third thought"]
        .into_iter()
        .enumerate()
    {
        store
            .file_feedback(
                "operator".to_owned(),
                text.to_owned(),
                at(u32::try_from(index).expect("index")),
            )
            .expect("file feedback");
    }

    // It piles up in order, nothing consuming or clearing it.
    let filed = store.feedback().expect("read feedback");
    assert_eq!(filed.len(), 3, "every entry is retained");
    assert_eq!(filed[0].text, "first thought");
    assert_eq!(filed[2].text, "third thought");

    // It is not a hold, so it never enters the queue the overseer works.
    assert!(
        store.list().expect("list holds").is_empty(),
        "feedback must not become a hold"
    );

    // The page says so rather than implying the overseer picks it up.
    let html = html_for(&store);
    assert!(
        html.contains("Collected, not acted on"),
        "the page must not imply that filing feedback dispatches anything"
    );
}

/// The page offers a feedback box, and what it files lands in the store, never in a repository.
#[test]
fn feedback_is_filed_into_the_store_and_never_into_a_repository() {
    let directory = tempfile::tempdir().expect("root");
    let store = HoldStore::new(directory.path().join("holds"));
    assert!(html_for(&store).contains("Feedback"));

    store
        .file_feedback(
            "operator".to_owned(),
            "the queue page buries the run registry".to_owned(),
            at(0),
        )
        .expect("file feedback");
    let filed = store.feedback().expect("read feedback");
    assert_eq!(filed.len(), 1);
    assert_eq!(filed[0].text, "the queue page buries the run registry");
    assert!(
        store
            .root()
            .join("feedback")
            .join("entries.jsonl")
            .is_file(),
        "feedback lands in the store, which is the only place the server may write"
    );
}
