use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn numbered_section<'a>(document: &'a str, heading: &str, next_heading: Option<&str>) -> &'a str {
    let start_marker = format!("{heading}\n");
    assert_count(document, &start_marker, 1);
    let start = offset(document, &start_marker);
    let end = match next_heading {
        Some(next) => {
            let end_marker = format!("{next}\n");
            assert_count(document, &end_marker, 1);
            let end = offset(document, &end_marker);
            assert!(
                start < end,
                "section boundary {heading} must precede {next}"
            );
            end
        }
        None => document.len(),
    };
    &document[start..end]
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

const HEADINGS: [&str; 10] = [
    "## 1. Parse one Effort ticket number",
    "## 2. Discover exactly one active Program",
    "## 3. Fetch, validate, and prove delivery before any mutation",
    "## 4. Confirm the destination and collect required observations",
    "## 5. Establish the landed decision and ADR links",
    "## 6. Load complete durable Program context",
    "## 7. Conduct the Fog-graduation Grill-with-docs session",
    "## 8. Build and approve one complete proposal",
    "## 9. Apply only approved GitHub changes in auditable order",
    "## 10. Re-fetch, detect Program completion, and report",
];

const READ_COMMAND: &str =
    "pce log read --file \"$VISION_DIR/events.jsonl\" --kind criterion-execution";
const LANDING_COMMAND: &str = "pce landing check --file \"$VISION_DIR/events.jsonl\" --vision-dir \"$VISION_DIR\" --finished-result \"$FINISHED_RESULT\"";
const DESTINATION_LEAD: &str = "Here is the world this work was meant to deliver:";
const DESTINATION_QUESTION: &str =
    "Is this where you wanted to land? If not, state what should be different.";
const DESTINATION_BLOCK: &str = "Here is the world this work was meant to deliver:\n\n<plain-language restatement>\n\nIs this where you wanted to land? If not, state what should be different.";
const CHECK_QUESTION: &str =
    "For “<plain-language check>”, what command did you run, and what did you observe?";
const OBSERVATION_STOP: &str = "An unanswered, skipped, blank, partial, or deferred observation response stops landing before the landed decision, Fog graduation, proposal review, label assurance, selected-ticket closure, Map edit, issue creation, dependency write, or any other GitHub mutation.";

#[test]
fn machine_proof_precedes_every_landing_question_and_github_mutation() {
    let skill = read("skills/land-ticket/SKILL.md");

    let mut previous = 0;
    for (index, heading) in HEADINGS.iter().enumerate() {
        assert_count(&skill, &format!("{heading}\n"), 1);
        let current = offset(&skill, heading);
        if index > 0 {
            assert!(previous < current, "section headings must be ordered");
        }
        previous = current;
    }

    assert_count(&skill, READ_COMMAND, 1);
    assert_count(&skill, LANDING_COMMAND, 1);
    assert!(offset(&skill, READ_COMMAND) < offset(&skill, LANDING_COMMAND));
    assert!(offset(&skill, LANDING_COMMAND) < offset(&skill, DESTINATION_LEAD));
    assert!(offset(&skill, LANDING_COMMAND) < offset(&skill, CHECK_QUESTION));
    assert!(offset(&skill, DESTINATION_LEAD) < offset(&skill, OBSERVATION_STOP));
    assert!(offset(&skill, CHECK_QUESTION) < offset(&skill, OBSERVATION_STOP));
    for later in &HEADINGS[4..] {
        assert!(offset(&skill, OBSERVATION_STOP) < offset(&skill, later));
    }
    for mutation in [
        "gh label create pce:ticket --force",
        "gh issue close <n>",
        "gh issue close <program-number>",
    ] {
        assert!(offset(&skill, OBSERVATION_STOP) < offset(&skill, mutation));
    }

    for obsolete in [
        "trust the human's delivery assertion",
        "**TRUST THE HUMAN:**",
        "explicitly stating that delivery is trusted from the human and not independently verified",
        "trusted-human delivery stance",
    ] {
        assert_count(&skill, obsolete, 0);
    }
}

#[test]
fn human_question_templates_are_bounded_and_mechanic_free() {
    let skill = read("skills/land-ticket/SKILL.md");
    let decision = "What single sentence should this program remember about what the work settled?";
    let architecture =
        "Did this work create a lasting architecture decision that should be linked?";
    let approval = "Do you approve this whole change set?";
    let closure = "Should I close this program now?";

    assert_count(&skill, DESTINATION_LEAD, 1);
    assert_count(&skill, DESTINATION_QUESTION, 1);
    for question in [CHECK_QUESTION, decision, architecture, approval, closure] {
        assert_count(&skill, question, 1);
    }

    for question in [
        DESTINATION_BLOCK,
        CHECK_QUESTION,
        decision,
        architecture,
        approval,
        closure,
    ] {
        let lowered = question.to_ascii_lowercase();
        for forbidden in [
            "planning",
            "events.jsonl",
            "finished_result",
            "criterion_index",
            "sequence",
            "payload",
            "evidence",
            "sha256",
            "digest",
            "ready",
            "refuse",
            "complete",
            "passed",
            "failed",
            "unpaid",
            "missing",
            "milestone-planner",
            "milestone-critic",
            "step-planner",
            "step-critic",
            "step-plan-writer",
            "step-plan-critic",
            "step-executor",
            "pr-reviewer",
            "falsification-critic",
            "repository-analyst",
        ] {
            assert!(
                !lowered.contains(forbidden),
                "forbidden question text: {forbidden}"
            );
        }
        assert!(!question.contains('/'));
        assert!(!question.contains('\\'));
        assert!(!lowered.contains(".md"));
    }

    let section = numbered_section(&skill, HEADINGS[3], Some(HEADINGS[4]));
    let allowlist_start = offset(section, "Read the linked");
    let allowlist_end_marker = "root-level `## ` header or end of document.";
    let allowlist_end = offset(section, allowlist_end_marker) + allowlist_end_marker.len();
    let allowlist = &section[allowlist_start..allowlist_end];
    assert!(allowlist.contains("## Goal / Why"));
    assert!(allowlist.contains("## Scope — In"));
    for other in [
        "## Scope — Out",
        "## Constraints",
        "## Acceptance Criteria",
        "## Decomposition Hints",
        "## Open Questions",
    ] {
        assert!(!allowlist.contains(other));
    }
    assert!(section.contains("only those two section bodies"));
}

#[test]
fn every_reported_unpaid_criterion_requires_one_complete_response() {
    let skill = read("skills/land-ticket/SKILL.md");
    let proof = numbered_section(&skill, HEADINGS[2], Some(HEADINGS[3]));
    let questions = numbered_section(&skill, HEADINGS[3], Some(HEADINGS[4]));

    assert!(proof.contains("completion.criteria"));
    assert!(proof.contains("serialized status is `unpaid`"));
    assert!(proof.contains("preserving that order"));
    assert_count(&skill, CHECK_QUESTION, 1);
    assert!(questions.contains("applied once per reported unpaid criterion at runtime"));
    assert_count(&skill, OBSERVATION_STOP, 1);
    assert!(offset(&skill, OBSERVATION_STOP) < offset(&skill, HEADINGS[4]));
    for mutation in [
        "gh label create pce:ticket --force",
        "gh issue close <n>",
        "gh issue close <program-number>",
    ] {
        assert!(offset(&skill, OBSERVATION_STOP) < offset(&skill, mutation));
    }
    assert!(questions.contains("two separately identifiable, nonblank parts"));
    assert!(questions.contains("the command the human ran and what they observed"));
    assert!(questions.contains("Do not re-prompt with a new question"));
}

#[test]
fn negative_destination_is_mandatory_named_effort_and_still_closes_selected_ticket() {
    let skill = read("skills/land-ticket/SKILL.md");
    let questions = numbered_section(&skill, HEADINGS[3], Some(HEADINGS[4]));
    let proposal = numbered_section(&skill, HEADINGS[7], Some(HEADINGS[8]));
    let mutation = numbered_section(&skill, HEADINGS[8], Some(HEADINGS[9]));
    let finish = numbered_section(&skill, HEADINGS[9], None);
    let fallback = "## Question\n\nWhat should change so the delivered result reaches the intended destination?\n\n## Scope sketch\n\n- Revisit the delivered world described during landing.\n- Establish and deliver the intended difference.\n\nProgram: #N";

    assert_count(
        questions,
        "Reconcile the delivered result with the intended destination",
        1,
    );
    assert_count(questions, fallback, 1);
    let mandatory = "mandatory named Effort work, not Fog and not an optional idea";
    assert_count(&skill, mandatory, 1);
    assert!(offset(&skill, mandatory) < offset(&skill, HEADINGS[7]));
    assert_count(
        proposal,
        "negative destination answer -> mandatory named Effort ticket",
        1,
    );
    for required in [
        "divergence ticket's title and complete seeded body",
        "Map link or index placeholder",
        "every real dependency",
    ] {
        assert!(proposal.contains(required));
    }
    assert!(
        offset(mutation, "gh issue close <n>")
            < offset(mutation, "mandatory divergence ticket when applicable")
    );
    assert!(mutation.contains("never branch around selected-ticket closure"));
    assert!(finish.contains(
        "newly minted divergence ticket is open, so it keeps the Program open and enters the reported Frontier unless a real blocker excludes it"
    ));
}

#[test]
fn decision_fog_review_mutation_and_map_closure_contracts_remain_intact() {
    let skill = read("skills/land-ticket/SKILL.md");
    let decision = numbered_section(&skill, HEADINGS[4], Some(HEADINGS[5]));
    let context = numbered_section(&skill, HEADINGS[5], Some(HEADINGS[6]));
    let grill = numbered_section(&skill, HEADINGS[6], Some(HEADINGS[7]));
    let proposal = numbered_section(&skill, HEADINGS[7], Some(HEADINGS[8]));
    let mutation = numbered_section(&skill, HEADINGS[8], Some(HEADINGS[9]));
    let finish = numbered_section(&skill, HEADINGS[9], None);

    assert_count(
        decision,
        "```markdown\n- #<n> — <one-line landed decision>\n```",
        1,
    );
    assert_count(
        decision,
        "```markdown\n- #<n> — <one-line landed decision> ([ADR NNNN](<repository-url>/blob/<default-branch>/docs/adr/NNNN-short-slug.md))\n```",
        1,
    );
    for rule in ["under `docs/adr/`", "same decision line", "Never invent"] {
        assert!(decision.contains(rule));
    }

    let map_block = "## Destination\n<!-- Describe the big idea's end state. -->\n\n## Notes\n<!-- Record durable program context. -->\n\n## Decisions so far\n<!-- Maintain the running one-line decision index that landed tickets append to. -->\n\n## Not yet specified\n<!-- Keep the fog of un-sharpened future tickets here. -->\n\n## Out of scope\n<!-- List explicit non-goals. -->";
    assert_count(context, map_block, 1);
    for rule in [
        "sole Fog section",
        "complete Program membership",
        "Exclude foreign",
        "do not introduce a local state file",
    ] {
        assert!(context.contains(rule));
    }

    for rule in [
        "sibling `grill-with-docs` skill completely",
        "exactly one prose question at a time",
        "recommended answer and its reason",
        "Immediately update `CONTEXT.md`",
        "hard to reverse, surprising without context, and a real trade-off",
        "every Fog branch is classified",
        "2–4 bullets",
        "real ordering dependencies",
        "unblocked actionable Effort tickets form the Frontier",
    ] {
        assert!(grill.contains(rule));
    }

    assert!(proposal.contains("seeded-lean ticket body shape"));
    assert_count(proposal, "Do you approve this whole change set?", 1);
    assert!(proposal.contains("complete revised proposal"));
    assert!(proposal.contains("Partial approval is insufficient and must be refused"));

    assert_count(mutation, "gh label create pce:ticket --force", 1);
    assert_count(mutation, "gh issue close <n>", 1);
    for stage in 1..=6 {
        assert!(mutation.contains(&format!("{stage}. ")));
    }

    assert!(finish.contains("both conditions hold simultaneously"));
    assert!(finish.contains("No open `pce:ticket` issue"));
    assert!(finish.contains("has no substantive Fog"));
    assert_count(finish, "Should I close this program now?", 1);
    assert_count(finish, "gh issue close <program-number>", 1);
    assert!(finish.contains("re-fetch the Map and verify its closed state"));
    assert!(finish.contains("partial-state warning"));
}

#[test]
fn normative_land_vocabulary_matches_the_skill_boundary() {
    let context = read("CONTEXT.md");
    let program = read("docs/program-layer.md");
    let land_row = "| Land | Prove from the binary-owned landing result that the linked vision was delivered and merged, collect the human's command and observation for every unpaid criterion, confirm the destination, close the effort ticket, add its one-line decision to the map, record and mint any negative destination divergence as named Effort work, graduate newly visible fog, and detect completion when the active map has no open linked effort tickets and no substantive fog. An unanswered unpaid observation blocks every GitHub mutation; a negative destination answer does not block effort-ticket closure. Map closure still requires separate confirmation. |";
    let relationship_row = "| Landing confirmation and the end-state picture | Landing confirmation is composed only from the linked vision's `Goal / Why` and `Scope — In`, restated together in plain language. It asks whether that delivered world is the intended one after binary-owned delivery proof and required unpaid observations, so the human judges the destination rather than mechanics. A negative answer becomes named Effort work and does not block the selected ticket from closing. |";

    assert_count(&context, &format!("{land_row}\n"), 1);
    assert_count(&context, &format!("{relationship_row}\n"), 1);
    assert_count(&context, "| Land | Trust the human assertion", 0);
    assert_count(&context, "They are the same text read twice.", 0);
    for row in [
        "| Landing confirmation |",
        "| Unpaid criterion |",
        "| Decisions-so-far index |",
        "| Fog-graduation |",
        "| Frontier |",
    ] {
        assert_eq!(
            context.lines().filter(|line| line.starts_with(row)).count(),
            1,
            "canonical term row must remain unique: {row}"
        );
    }

    let lifecycle = "3. **Land.** After that vision's independent `/to-vision \"<name>\"` then `/pce`\n   delivery cycle finishes, `/land-ticket <n>` validates the ticket's `Program:`\n   and `Vision:` linkages and consumes the binary-owned landing result, which\n   proves executed criteria, merge state, and event evidence without asking the\n   human to assert delivery. It restates only the linked vision's `Goal / Why`\n   and `Scope — In` as one plain-language destination question and asks one\n   combined command-and-observation question for each unpaid criterion. Every\n   such observation is required before any GitHub mutation. It closes the\n   Effort ticket on either destination answer; a negative answer is recorded and\n   minted through the reviewed proposal as named Effort work. Land appends or\n   reconciles the ticket's one-line landed decision in the Map's `Decisions so\n   far`, links applicable committed ADRs, and runs the complete reviewed\n   Fog-graduation Grill-with-docs session. Newly sharp Fog becomes lean Effort\n   tickets with real blocking relationships; unresolved territory stays Fog.\n   After re-fetching state, Land considers the Program done only when no open\n   member Effort tickets remain and the Map's Fog is empty. It then presents a\n   final summary and asks separately before closing the Map.";
    assert_count(&program, lifecycle, 1);
    assert_count(&program, "explicitly trusts the human's", 0);
    assert_count(&program, "3. **Land.**", 1);
    assert_count(
        &program,
        "final summary and asks separately before closing the Map.",
        1,
    );
    assert_count(&program, "Each Effort ticket", 1);
}
