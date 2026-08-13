//! The landing skill is doctrine under test.
//!
//! These assertions exist because the skill's cost to the human is its question count, and a
//! question is the cheapest thing in the world to add back. The previous version of this file
//! pinned a landing that asked for a destination confirmation, one observation per unpaid
//! criterion, a whole-change-set approval, and a full Fog-graduation interview. That design was
//! replaced deliberately: delivery is now proved by reading what the driver recorded, so the only
//! question left is the one no computation supplies.

use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn offset(document: &str, needle: &str) -> usize {
    document
        .find(needle)
        .unwrap_or_else(|| panic!("missing expected text: {needle}"))
}

fn assert_count(document: &str, needle: &str, expected: usize) {
    assert_eq!(
        document.matches(needle).count(),
        expected,
        "unexpected occurrence count for: {needle}"
    );
}

/// Collapse every run of whitespace to one space.
///
/// Prose assertions run against this rather than the raw bytes, so reflowing a paragraph to a
/// different column width cannot fail a test about what the doctrine says. Headings and commands
/// are still matched exactly, because their layout is part of their meaning.
fn flatten(document: &str) -> String {
    document.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn assert_says(document: &str, sentence: &str, expected: usize) {
    let flat = flatten(document);
    let needle = flatten(sentence);
    assert_eq!(
        flat.matches(&needle).count(),
        expected,
        "unexpected occurrence count for sentence: {needle}"
    );
}

const HEADINGS: [&str; 8] = [
    "## The question budget",
    "## 1. Establish the ticket, the Program, and the vision",
    "## 2. Prove delivery from the driver's journal",
    "## 3. Ask the one question",
    "## 4. Write the decision, derived not asked",
    "## 5. Apply, in an order that makes partial failure auditable",
    "## 6. Program completion — the second question, rarely",
    "## 7. Report",
];

const THE_QUESTION: &str = "> Does the world now look like the one this vision described?";
const DRIVER_STATUS: &str = "pce package driver-status";
const RENDER: &str = "pce package render";

#[test]
fn machine_proof_precedes_the_question_and_every_mutation() {
    let skill = read("skills/land-ticket/SKILL.md");

    let mut previous = 0;
    for (index, heading) in HEADINGS.iter().enumerate() {
        assert_count(&skill, &format!("{heading}\n"), 1);
        let current = offset(&skill, heading);
        if index > 0 {
            assert!(
                previous < current,
                "section headings must be ordered: {heading}"
            );
        }
        previous = current;
    }

    // Delivery is read from the driver before anything is asked of the human.
    assert!(offset(&skill, DRIVER_STATUS) < offset(&skill, THE_QUESTION));
    // The evidence is rendered and handed over, because the human cannot override the proof.
    assert!(offset(&skill, RENDER) < offset(&skill, THE_QUESTION));
    // Nothing is written to GitHub before the question is answered.
    assert!(offset(&skill, THE_QUESTION) < offset(&skill, "## 5. Apply"));
}

#[test]
fn exactly_one_question_is_asked_by_default() {
    let skill = read("skills/land-ticket/SKILL.md");

    assert_says(&skill, THE_QUESTION, 1);

    // The budget is stated as a constraint on the skill, not left to emerge from its prose.
    for rule in [
        "**One question by default.**",
        "Two only when the Program is genuinely finished.",
        "Three only when a decision meets every ADR condition.",
    ] {
        assert_says(&skill, rule, 1);
    }
}

#[test]
fn the_retired_question_templates_stay_retired() {
    let skill = read("skills/land-ticket/SKILL.md");

    // Each of these asked the human to do work the binary already does, or to ratify something
    // they could not evaluate. Reintroducing any of them is the regression this test exists for.
    for retired in [
        "Here is the world this work was meant to deliver:",
        "Is this where you wanted to land? If not, state what should be different.",
        "what command did you run, and what did you observe?",
        "Do you approve this whole change set?",
        "What single sentence should this program remember about what the work settled?",
        "## 4. Confirm the destination and collect required observations",
        "## 7. Conduct the Fog-graduation Grill-with-docs session",
        "## 8. Build and approve one complete proposal",
    ] {
        assert_says(&skill, retired, 0);
    }

    // The delivery-trust stance the corpus falsified must not return either.
    for obsolete in [
        "trust the human's delivery assertion",
        "**TRUST THE HUMAN:**",
        "trusted-human delivery stance",
    ] {
        assert_says(&skill, obsolete, 0);
    }
}

#[test]
fn delivery_cannot_be_overridden_by_the_human() {
    let skill = read("skills/land-ticket/SKILL.md");

    // A landing that can be argued into completing is not a proof of delivery.
    assert_says(
        &skill,
        "**Do not ask the human to interpret it, override it, or accept it anyway.**",
        1,
    );
    assert_says(
        &skill,
        "A landing that can be talked into completing is not a proof of delivery.",
        1,
    );

    // Every leg of the proof is named, so none can be quietly dropped.
    for leg in [
        "Every package in the frozen graph is complete.",
        "has a recorded execution that exited zero",
        "Every gate finding was replayed and decided.",
        "The work merged.",
    ] {
        assert_says(&skill, leg, 1);
    }
}

#[test]
fn fog_graduation_belongs_to_charting_not_landing() {
    let skill = read("skills/land-ticket/SKILL.md");

    // Landing must not bury a Program survey inside a question about one ticket.
    assert_says(&skill, "grill-with-docs", 0);
    assert_says(
        &skill,
        "Fog graduation and ticket minting belong to `/chart-program`",
        1,
    );
}

#[test]
fn program_closure_is_a_separate_and_conditional_question() {
    let skill = read("skills/land-ticket/SKILL.md");

    // Both conditions must hold before closure is even raised.
    assert_says(&skill, "no open `pce:ticket` issue references this Map", 1);
    assert_says(&skill, "`Not yet specified` is empty", 1);
    assert_says(&skill, "**Do not ask about closing.**", 1);
    assert_says(&skill, "Nothing earlier authorizes that closure.", 1);
}

#[test]
fn the_report_leads_at_intent_altitude() {
    let skill = read("skills/land-ticket/SKILL.md");

    assert_says(&skill, "in plain language, no issue numbers", 1);
    assert_says(
        &skill,
        "the report has failed regardless of its accuracy",
        1,
    );
}
