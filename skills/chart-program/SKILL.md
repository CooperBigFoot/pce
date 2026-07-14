---
name: chart-program
description: Chart or fully re-survey a multi-vision Program through GitHub issues. Use for `/chart-program "idea"` when a breadth-first survey must create or refresh the Program Map, sharp Effort tickets, dependency edges, Frontier, and Fog only after review of the complete proposed change set.
---

# Chart a Program

Treat a **Program** as the durable container for one multi-vision idea and its single GitHub issue as the **Map**. Treat each vision-sized question as an **Effort ticket**, unresolved territory as **Fog**, and actionable unblocked Effort tickets as the **Frontier**. **Chart** means survey breadth-first, create or refresh the Map, mint only sharp Effort tickets, retain Fog, and resolve no ticket. The Map records program-level discovery; never decompose it into implementation milestones or steps.

Run every command from the repository whose Program is being charted.

## 1. Establish the subject and discover Program state

Use `"<idea>"` as the charting subject. If it is missing or unusable, reconstruct it only when discovery finds exactly one existing Program whose complete Map supplies a single unambiguous subject. Otherwise ask the user for the idea before proceeding. Never invent it.

Run exactly:

```bash
gh issue list --label pce:program --state open
```

Count the results and apply this trichotomy exactly:

> ZERO open programs => create a new map; EXACTLY ONE => full re-survey of that existing map; MORE THAN ONE => clear error and stop.

- **Zero:** Enter new-Program mode. Do not create the Map yet. First complete the grill, prepare the complete proposed Map, Effort tickets, and dependency edges, and pass the review gate.
- **Exactly one:** Enter re-survey mode. Read the entire existing Map, including comments, and every linked or Program-referencing Effort ticket, including open or closed state and native or body-recorded dependency information. Use `gh issue view` for complete issue bodies and comments, not list summaries. List all `pce:ticket` issues in all states and inspect every issue linked by the Map or containing `Program: #N`, where `N` is the Map number. Re-survey the whole Program rather than appending only the supplied idea. Preserve and reconcile durable context, especially `Notes` and `Decisions so far`. Propose explicit edits, additions, retained Fog, Effort-ticket changes, and dependency changes. Make no mutation before review approval.
- **More than one:** Report every conflicting issue number and title clearly, then stop. Do not grill, edit files, create labels, or mutate issues. One repository cannot have concurrent open Programs.

The zero-result behavior intentionally overrides only the generic create-case comment in `.github/ISSUE_TEMPLATE/pce-program.md`. Other Program-layer consumers may continue to require exactly one result.

## 2. Compose a breadth-first `grill-with-docs` session

Invoke and follow the sibling `grill-with-docs` skill. Do not replace or weaken its interview and documentation discipline. Survey at Program and vision altitude across the entire Program: destination, boundaries, distinct vision-sized questions, relationships, ordering constraints, risks, success conditions, and Fog. Do not deeply design one Effort, decompose implementation milestones or steps, or turn every uncertainty into an Effort ticket.

Interview relentlessly until shared understanding is reached. Walk each relevant decision branch and resolve dependencies between decisions one by one.

- Ask exactly one prose question at a time.
- Keep questions at Program/vision altitude: goals, boundaries, domain concepts, durable constraints, alternatives, risks, and success conditions. Do not decompose work into milestones, implementation steps, or code tasks.
- With every question, provide a recommended answer and the reason for it.
- Inspect the codebase and committed documentation instead of asking when they can answer the question. Present contradictions between those sources and the user's model as the next question.
- Challenge language that conflicts with `CONTEXT.md`. Sharpen vague or overloaded language into canonical terms, and test boundaries and relationships with concrete edge cases.
- After each answer, capture crystallized terminology or a qualifying decision before asking the next question.

Use the sibling `domain-modeling` skill's `CONTEXT-FORMAT.md` and `ADR-FORMAT.md` as normative. Apply their rules during the interview, not as a batch afterward.

Update `CONTEXT.md` immediately when the conversation resolves a project-specific canonical term, an alias to avoid, a relationship, or a material ambiguity. Preserve existing entries and use the existing domain-modeling format. Keep it a concise glossary, never a specification, scratchpad, work list, or implementation log.

Offer an ADR only when all three conditions hold:

1. The decision is hard to reverse, so changing it later has meaningful cost.
2. The decision is surprising without context, so a future reader would reasonably ask why it was made.
3. The decision results from a real trade-off between genuine alternatives chosen for specific reasons.

Create an ADR only after the user accepts the offer. Use the next `docs/adr/NNNN-short-slug.md` sequence and the domain-modeling format. Do not create an ADR for easy-to-reverse choices, unsurprising decisions, unresolved hypotheses, glossary wording, or implementation notes.

Finish the grill only when every relevant breadth-first branch is resolved or explicitly retained as Fog/open. Recap the shared understanding, glossary edits, accepted ADRs, and unresolved territory before classification.

## 3. Classify Effort tickets, Fog, and Frontier

Classify all visible territory after the grill:

- A sharp Effort ticket states one ambitious, contained, vision-sized question. Seed it with a lean scope sketch of 2–4 bullets. It is not an implementation task, milestone, or step.
- Fog is acknowledged future territory that cannot yet be phrased as one contained vision-sized question. Keep it only in the Map's `Not yet specified` section. Do not mint speculative Effort tickets for it.
- Add a blocker only for a real ordering dependency between sharp Effort tickets. Currently unblocked actionable Effort tickets form the Frontier. Do not impose a total order to make the Map appear complete.

In re-survey mode, classify the entire existing Map again. Keep still-valid Effort tickets; propose edits when a question or lean seed changed; promote newly sharp Fog into proposed Effort tickets; return no-longer-sharp territory to Fog when appropriate; retain unresolved Fog; and reconcile dependency edges. Never silently discard existing Map state or settled decisions. Do not change issue state as part of reclassification.

## 4. Present the complete proposal and obtain approval

Before creating or updating any label, issue, Effort ticket, assignment, closure, or blocking edge, present one complete proposed change set containing:

1. The complete proposed Program Map title and full body, with every template section populated or intentionally empty.
2. Every proposed Effort ticket title and complete seeded-lean body.
3. Every proposed native blocking edge, naming the blocker and blocked Effort ticket, and the exact `Depends on: #M` fallback that will be written if native blocking is unavailable.
4. In re-survey mode, an explicit reconciliation of creates, edits, unchanged items, Fog changes, and dependency additions and removals against current GitHub state.
5. Every issue-number placeholder required before GitHub assigns numbers, with its mechanical substitution explained.

Ask for explicit human approval of the entire proposal. Questions or revisions return to the proposal/grill cycle. Any change to the proposed Map, Effort tickets, or edges invalidates earlier approval: present the revised complete proposal and obtain approval again. Approval of a partial draft never satisfies this gate.

Do not mutate GitHub before that approval. Runtime glossary edits and user-accepted ADRs occur under the composed grill's documentation rules; they do not authorize issue mutations.

## 5. Ensure labels exist after approval

Immediately before approved issue mutations, idempotently ensure these labels exist:

```text
pce:program
pce:ticket
```

Use these commands, or check each label and create only the missing label:

```bash
gh label create pce:program --force
gh label create pce:ticket --force
```

Descriptions and colors may be supplied only if applied consistently. Do not depend on manual provisioning. If either command fails, report the command and its failure, then stop before attempting any issue write that requires the missing label.

## 6. Apply only the approved change set

Use `.github/ISSUE_TEMPLATE/pce-program.md` as the normative Map template. Preserve these headers byte-for-byte and in this order:

```markdown
## Destination
<!-- Describe the big idea's end state. -->

## Notes
<!-- Record durable program context. -->

## Decisions so far
<!-- Maintain the running one-line decision index that landed tickets append to. -->

## Not yet specified
<!-- Keep the fog of un-sharpened future tickets here. -->

## Out of scope
<!-- List explicit non-goals. -->
```

Replace the comments with approved content, or leave a section intentionally empty as reviewed. `Destination` states the end state. `Notes` preserves durable Program context and appropriate ticket links or indexes. `Decisions so far` preserves the running one-line landed-decision index. `Not yet specified` contains Fog. `Out of scope` lists explicit non-goals. The exact final header is `## Out of scope`, never “Out of scope for the map.”

In new-Program mode, create the approved Map with label `pce:program`; in re-survey mode, update the exactly-one Map with the approved title and body. After GitHub assigns the Map number, mechanically replace its approved `#N` placeholders in Effort tickets.

Use `.github/ISSUE_TEMPLATE/pce-ticket.md` as the normative seeded-lean Effort-ticket template:

```markdown
## Question

What single question does this ticket resolve?

## Scope sketch

- Add 2–4 bullets describing the likely scope.

Depends on: #M

Program: #N
```

Replace the instructional question and bullet with the approved question and 2–4 approved lean scope bullets. Replace `Program: #N` with the actual Map number. Use `Depends on: #M` only for an actual fallback dependency, replacing `#M` with the blocker's issue number. Omit that line entirely for an unblocked Effort ticket; never leave a fake dependency placeholder in a final body. Keep each body a seed for its later deep grill, not a miniature vision or implementation plan.

Create or update only the approved currently sharp Effort tickets with label `pce:ticket`. Once GitHub assigns their numbers, perform only the approved mechanical placeholder substitutions and ensure the Map links its Effort tickets and every Effort ticket points back with `Program: #N`. Any substantive change requires a new complete review and approval.

Prefer native GitHub issue blocking relationships when reachable through the supported `gh` CLI or API surface. For each approved edge, make the named blocker block the named blocked Effort ticket. If native blocking cannot be created, write the already-reviewed exact `Depends on: #M` line into the blocked ticket body. Report which representation each edge uses, and never claim a native edge exists when only the fallback line was written. Do not retain a fallback line for an edge successfully represented natively unless the approved proposal explicitly showed both representations.

Apply mutations in a sequence that makes partial state auditable: labels, Map create/update, Effort-ticket create/update, mechanical number substitution and Map links, then dependency edges or fallbacks. After each mutation, retain its issue number and result. If a write fails partway through, stop immediately; report exactly what was created or updated, what failed, and what remains unapplied. Do not conceal or automatically compensate for partial state.

## 7. Resolve nothing, report, and support reruns

Charting resolves nothing. Never assign or claim an Effort ticket, close or reopen an Effort ticket, close or reopen the Map, or treat the Chart as delivery. Leave every newly created Effort ticket open and unassigned. Mutate only approved Map and Effort-ticket titles/bodies and approved dependency relationships.

Conclude with a concise report listing the Map issue, Effort-ticket issues, dependency representation, retained Fog, and any `CONTEXT.md` or ADR updates.

On every rerun, repeat discovery, load all existing Program state, run the full breadth-first survey, classify all territory, and enforce a new complete review gate. Exactly one open Map triggers a full re-survey, never an error or narrow incremental append.
