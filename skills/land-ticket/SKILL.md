---
name: land-ticket
description: Use `/land-ticket <n>` to land one validated Effort ticket in the single active Program, record its decision, graduate Fog through a reviewed Grill-with-docs proposal, and ask before closing a completed Program.
---

# Land Ticket

Land one delivered Effort ticket and expose the next Frontier. Run every command from the repository whose Program is being landed. Reconstruct durable state on every invocation from GitHub issues, committed repository source, machine proof, and human answers; never rely on a previous conversation or a local Program state file.

Follow this order exactly: parse the argument; discover exactly one active Program; fetch and validate the Effort ticket and Map without mutation; prove delivery; collect every required unpaid observation; confirm the destination; establish the landed decision and ADR links; load complete Program context; perform the full Fog-graduation interview; classify sharp tickets, retained Fog, and blockers; build and approve one complete proposal including any divergence; ensure the label if needed; apply only approved changes; re-fetch; detect Program completion; ask separately before Map closure; report verified results and partial failure.

## 1. Parse one Effort ticket number

Set `<n>` to `$ARGUMENTS`. Require exactly one usable GitHub issue number. If the argument is absent, malformed, or contains anything other than that one number, report a clear error and stop.

## 2. Discover exactly one active Program

Run exactly:

```bash
gh issue list --label pce:program --state open
```

Count the results and apply this rule prominently:

> ZERO open programs => clear error and stop; EXACTLY ONE open program => continue with that issue as the active Program Map; MORE THAN ONE open program => report every conflicting issue number and title clearly, then stop.

Never create or re-survey a Program during discovery. On zero or more than one result, stop before fetching or closing the selected ticket, grilling, creating labels or issues, editing the Map, or changing dependencies.

## 3. Fetch, validate, and prove delivery before any mutation

Fetch the complete selected issue, including state, and the complete active Map:

```bash
TICKET_JSON="$(gh issue view "<n>" --json number,title,body,labels,state,url)"
MAP_JSON="$(gh issue view "<program-number>" --json number,title,body,labels,state,url)"
```

Before any GitHub mutation, require all of these conditions:

1. Issue `#<n>` exists and is open. A failed fetch or closed issue is a clear fatal error.
2. Its labels include the exact name `pce:ticket`. Otherwise report that `#<n>` is not an Effort ticket and stop.
3. Its body contains exactly one root-level Program linkage in this complete normative form:

   ```markdown
   Program: #N
   ```

   Require `N` to equal the discovered active Map number. Reject a missing, malformed, duplicate, conflicting, or foreign linkage. Do not infer Program membership from a title, link, assignment, user intent, or proximity.
4. Its body contains exactly one root-level Vision linkage in this byte-consistent normative form:

   ```text
   Vision: planning/<YYYY-MM-DD>-<slug>
   ```

   Require the exact `Vision: ` prefix with one ASCII space and a root-relative `planning/<YYYY-MM-DD>-<slug>` value. Reject a missing, malformed, duplicate, non-root, absolute-path, or conflicting line. Do not normalize or rewrite it.

After all linkage validation succeeds, bind the exact Vision value as `VISION_DIR` and bind `LOG_PATH` to its literal child `events.jsonl`. Never ask the human for either value. Run exactly:

```text
pce log read --file "$VISION_DIR/events.jsonl" --kind criterion-execution
```

Require successful, validated, ordered output containing at least one criterion-execution record. Select the record with the greatest numeric `sequence` and parse its nonblank string `payload.finished_result` once as `FINISHED_RESULT`; never ask the human to select or confirm it. A missing log, nonzero read, malformed record, empty record set, or missing or blank identity stops before the next command, every human landing question, Grill-with-docs, and every GitHub mutation.

Capture stdout as `LANDING_JSON` without hiding the exit status when running exactly:

```text
pce landing check --file "$VISION_DIR/events.jsonl" --vision-dir "$VISION_DIR" --finished-result "$FINISHED_RESULT"
```

A nonzero exit, malformed JSON, top-level decision other than exact `ready`, any nonempty `problems` array, or any completion report with status `failed` or `missing` is a fatal pre-mutation stop. Report the machine failure without asking the human to interpret or override it. Do not reimplement the completion, merge, event-evidence, or three-valued authority fold in prose, and do not inspect git, GitHub pull requests, or raw delivery artifacts independently of the binary.

For a ready result, retain `completion.criteria` in its existing order. Select exactly the reports whose serialized status is `unpaid`, preserving that order; reports with status `passed` cause no human question. Use each selected report only to compose the plain-language check description in the next section. Never expose the raw status, reason, evidence, authority projection, index, sequence, finished-result identity, `LANDING_JSON`, or its mechanics to the human.

## 4. Confirm the destination and collect required observations

Read the linked `vision.md` only after the machine result is ready. Require exactly one `## Goal / Why` and exactly one `## Scope — In`, in template order, with substantive content in both. Extract each section from its exact header through the next root-level `## ` header or end of document. A missing, duplicate, reordered, unreadable, or empty permitted section is a pre-mutation stop.

Restate the combined meaning of only those two section bodies in concise plain language. Remove mechanism and do not quote a forbidden token merely because the source contains it. Do not use the title, Scope — Out, Constraints, acceptance criteria, decomposition hints, open questions, PCE summary, Map, ticket body, commits, or delivery artifacts to compose the destination.

Put exactly this destination prompt to the human:

```text
Here is the world this work was meant to deliver:

<plain-language restatement>

Is this where you wanted to land? If not, state what should be different.
```

For each selected report, restate its criterion as a short plain-language check without exposing raw report keys or mechanics, and put this combined prompt to the human:

```text
For “<plain-language check>”, what command did you run, and what did you observe?
```

The preceding template is applied once per reported unpaid criterion at runtime, in result order. These prompts and the destination prompt are the only variable landing questions. Decision reconstruction, the Fog-graduation interview, proposal approval, and final Program closure remain separate lifecycle questions at intent altitude. Artifact bodies presented for review are proposed durable data, not question text.

For every response to a criterion prompt, require two separately identifiable, nonblank parts: the command the human ran and what they observed. An absent, skipped, blank, partial, one-part, or deferred reply is not an observation receipt. Do not re-prompt with a new question; state the failure and stop.

An unanswered, skipped, blank, partial, or deferred observation response stops landing before the landed decision, Fog graduation, proposal review, label assurance, selected-ticket closure, Map edit, issue creation, dependency write, or any other GitHub mutation.

Evaluate that stop rule for every selected report before interpreting the destination answer and before continuing. Once every required receipt exists, interpret the destination answer. An affirmative answer adds no divergence work. A negative answer never stops landing and never remains Fog. Preserve the stated desired difference. A bare negative creates the named fallback below without another question.

On a negative answer, create one mandatory divergence candidate before Fog graduation. Name its title concisely from the stated difference. If the answer contains no detail beyond its negative polarity, use exactly:

```text
Reconcile the delivered result with the intended destination
```

Seed the candidate with the normal Effort-ticket shape. With supplied details, replace the angle-bracket content with a faithful plain-language account. Without details, use exactly:

```markdown
## Question

What should change so the delivered result reaches the intended destination?

## Scope sketch

- Revisit the delivered world described during landing.
- Establish and deliver the intended difference.

Program: #N
```

The candidate has no `Vision:` line and no `Depends on:` line unless the later complete Fog-graduation dependency analysis finds a real blocker. Replace `N` with the active Map number in the reviewed proposal. It is mandatory named Effort work, not Fog and not an optional idea: the proposal may revise its wording but may not omit it while the answer remains negative. This is the durable record of the divergence.

The negative answer does not bypass existing safeguards. The complete Fog-graduation interview and whole-proposal approval still run, and no GitHub mutation occurs without that approval. A declined or unanswered proposal approval stops for lack of approval, not because the destination answer was negative.

## 5. Establish the landed decision and ADR links

Inspect the ticket body and comments, Map body and comments, and committed documentation for an unambiguous existing decision and any ADR produced by the ticket's Grill-with-docs session. Because prior chat is not durable, use this mechanic-free question when the one-line decision cannot be reconstructed unambiguously:

```text
What single sentence should this program remember about what the work settled?
```

When durable sources do not establish whether the session produced an ADR, ask:

```text
Did this work create a lasting architecture decision that should be linked?
```

Never invent a decision or ADR association. Use one physical Markdown line:

```markdown
- #<n> — <one-line landed decision>
```

Append every applicable ADR link on that same line:

```markdown
- #<n> — <one-line landed decision> ([ADR NNNN](<repository-url>/blob/<default-branch>/docs/adr/NNNN-short-slug.md))
```

Resolve the repository URL and default branch from the current repository. Accept an identified ADR only when its file is under `docs/adr/` and exists in committed source. Omit the parenthetical when no ADR was produced. If multiple ADRs genuinely apply, link each on the same decision line.

The approved Map edit must append this entry inside the existing `## Decisions so far` section immediately before the next `## ` header or end of body. Preserve all existing entries and every other section. Never append outside the section, replace history, or duplicate the selected ticket's entry. If `#<n>` already has an entry, reconcile it explicitly in the reviewed proposal.

## 6. Load complete durable Program context

Re-fetch complete bodies and comments as needed. Require these exact Map headers in this order and preserve their spelling:

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

Treat `## Not yet specified` as the sole Fog section, ending at the next `## ` header or body end. Whitespace and the untouched instructional HTML comment are empty Fog; all substantive text must be retained or graduated through the grill and review.

Load all of the following:

- the complete active Map body and comments;
- the selected ticket body and comments;
- existing Map Notes and Decisions-so-far content;
- the complete Fog section;
- root `CONTEXT.md`;
- every issue labeled `pce:ticket`, in all states, whose body has exactly one `Program: #N` linkage to this Map; and
- the available native or fallback dependency state for those member tickets.

Use complete Program membership to prevent duplicates, preserve blockers, and later detect completion. Exclude foreign `pce:ticket` issues. If a required Map header is missing or `CONTEXT.md` is unreadable, name the missing durable source and stop before grilling or mutation. GitHub issues and the Map are the sole durable Program state; do not introduce a local state file.

## 7. Conduct the Fog-graduation Grill-with-docs session

Invoke and follow the sibling `grill-with-docs` skill completely, including its interview loop, domain-modeling formats, documentation decision rules, and finish rules. Specialize it narrowly to the current Map's `## Not yet specified` Fog after incorporating the selected ticket's landed decision, the mandatory divergence candidate when present, Map Notes, Decisions-so-far index, every existing Program ticket and blocker, and the root glossary. Do not re-survey or redesign the Program and do not decompose milestones, implementation steps, or code tasks. The divergence candidate is mandatory input: use the interview to find duplicates, real dependencies, and refined wording, but never reclassify or silently discard it as Fog.

During the interview:

- Ask exactly one prose question at a time, with a recommended answer and its reason. Resolve every relevant decision-tree branch and dependency one by one.
- Keep questions at Program or vision altitude: goals, boundaries, domain concepts, durable constraints, alternatives, risks, and success conditions.
- Inspect code and committed documentation rather than asking questions they answer. Surface contradictions with the human's model as questions.
- Challenge terms against `CONTEXT.md`, use canonical Program, Map, Effort ticket, Fog, Frontier, and Grill-with-docs language, test boundaries with concrete edge cases, and capture crystallized terminology or qualifying decisions after every answer.
- Apply `domain-modeling/CONTEXT-FORMAT.md` and `domain-modeling/ADR-FORMAT.md` during the interview, not in a final batch. Immediately update `CONTEXT.md` for resolved project-specific terms, aliases to avoid, relationships, or ambiguities, while keeping it a glossary. Preserve existing entries and use `CONTEXT-MAP.md` when present.
- Offer an ADR only when the decision is hard to reverse, surprising without context, and a real trade-off among meaningful alternatives. If accepted, create it immediately under `docs/adr/` with the next unused `NNNN-short-title.md` sequence and the normative format. A durable ADR is consequential, durable, and supported by meaningful alternatives; routine, reversible, glossary, status, or implementation choices do not qualify. Never rewrite accepted history. Supersede it with a new linked ADR, and update `CONTEXT.md` too when domain language changes.
- Finish only when every relevant branch is resolved or explicitly open. Recap shared understanding, `CONTEXT.md` entries changed, ADRs created, and unresolved questions without inventing answers.

Finish Fog graduation only when every Fog branch is classified as newly sharp or explicitly retained. A newly sharp Effort ticket states one ambitious, contained, vision-sized question and a lean scope sketch of 2–4 bullets. Fog that cannot meet that bar remains only in `## Not yet specified`; never mint a speculative ticket or silently discard Fog.

Identify real ordering dependencies between sharp Program tickets. Do not impose a total order. Add a blocker only for a real dependency. The unblocked actionable Effort tickets form the Frontier. Runtime glossary edits and accepted ADR creation follow the composed skill's documentation rules, but authorize no GitHub mutation and do not bypass review.

## 8. Build and approve one complete proposal

Before closing the selected ticket, editing the Map, ensuring a label, creating an issue, or changing any dependency, present one complete proposed GitHub change set containing:

1. The selected Effort ticket `#<n>` and its open-to-closed transition; that the binary proved delivery; that every required command-and-observation answer was supplied; and the destination answer's affirmative or negative polarity.
2. The complete proposed Map title and full body: the decision entry or reconciliation in `## Decisions so far`; the complete retained `## Not yet specified` Fog; links or index entries for new tickets using mechanical placeholders; and every preserved section.
3. Every proposed newly sharp Effort ticket title and complete seeded-lean body.
4. Every blocking edge, naming blocker and blocked ticket, and the exact reviewed `Depends on: #M` fallback to write if native blocking is unavailable.
5. An explicit current-to-proposed reconciliation covering ticket closure, decision append or reconciliation, minted tickets, retained Fog, unchanged tickets and sections, Map link additions, dependency additions or removals, and every issue-number placeholder with its mechanical substitution explained.

On a negative answer, include the mandatory divergence ticket's title and complete seeded body among the new Effort tickets, its Map link or index placeholder, and every real dependency. The reconciliation must contain `negative destination answer -> mandatory named Effort ticket`, never retained Fog. Approval may revise the candidate but cannot omit it unless the destination answer itself is corrected and the complete proposal is presented again.

Use this seeded-lean ticket body shape:

```markdown
## Question

<one ambitious, contained, vision-sized question>

## Scope sketch

- <reviewed lean scope bullet 1>
- <reviewed lean scope bullet 2>

Depends on: #M

Program: #N
```

Use 2–4 reviewed scope bullets. Replace `Program: #N` with the active Map number. Include `Depends on: #M` only for a real fallback dependency and replace `M` with the blocker number or a clearly explained pre-creation placeholder; omit it for an unblocked ticket. Do not stamp `Vision:` because `/work-ticket` owns that transition. Keep each body a seed for a later deep grill, not a miniature vision or implementation plan.

Ask exactly:

```text
Do you approve this whole change set?
```

Questions or revisions return to the grill and proposal cycle. Any substantive change to the Map body, ticket bodies, selected closure, or edges invalidates approval and requires presenting the complete revised proposal. Partial approval is insufficient and must be refused. No GitHub mutation may precede whole-proposal approval; Grill-with-docs documentation changes do not authorize issue writes.

## 9. Apply only approved GitHub changes in auditable order

If and only if new Effort tickets are approved, ensure the established shared label immediately before creating them:

```bash
gh label create pce:ticket --force
```

Reuse exact label `pce:ticket`; never create a duplicate or renamed variant. If assurance fails, report the command and failure and stop before any issue write that needs the label. Do not require label creation when no ticket will be minted.

Apply only the approved changes in these six numbered stages and this exact order:

1. Ensure `pce:ticket` when new tickets will be created.
2. Close the selected ticket with exactly `gh issue close <n>`. If it fails, report it and stop; do not claim the ticket landed. A negative destination answer reaches this same step: never branch around selected-ticket closure or create the divergence first.
3. Update the Map with the approved full body, including the reconciled decision entry and retained Fog while preserving every other approved section.
4. Create all and only approved newly sharp tickets, including the mandatory divergence ticket when applicable, with label `pce:ticket`, retaining every assigned issue number.
5. Mechanically substitute reviewed number placeholders and update approved Map ticket links or indexes.
6. Create approved native blocking relationships whenever reachable through the supported `gh` CLI or API. Make each named blocker block the named blocked ticket. If native blocking cannot be created, write the already-reviewed exact `Depends on: #M` line into the blocked ticket body. Do not claim a native edge when only fallback text exists, and do not retain fallback text for a successful native edge unless the proposal approved both.

Retain the issue number and result after every mutation. If any write fails partway, including a Map edit, divergence mint, placeholder substitution, or dependency write, stop immediately and report exactly what closed, what was created or edited, what failed, and what remains unapplied. Do not conceal, roll back, automatically compensate, or claim an unapplied mutation. Only mechanical number substitution and the reviewed native-to-fallback choice are pre-authorized; any substantive deviation requires a new complete review.

## 10. Re-fetch, detect Program completion, and report

After all approved landing and Fog-graduation writes succeed, re-fetch the active Map, every open issue labeled `pce:ticket` with complete bodies, and dependency state. Program-done is true only when both conditions hold simultaneously:

1. No open `pce:ticket` issue has exactly one `Program: #N` linkage to the active Map. Count Program members, not foreign tickets.
2. The Map's `## Not yet specified` section has no substantive Fog after ignoring whitespace and the untouched template comment.

A newly minted divergence ticket is open, so it keeps the Program open and enters the reported Frontier unless a real blocker excludes it. Any other newly minted Effort ticket has the same open-member effect. If either an open member ticket or retained Fog remains, keep the Map open, report the remaining Frontier and Fog, and do not ask to close it.

Only when both conditions are true, compose a final Program summary from the Map Destination, Notes, complete Decisions-so-far index, completed member-ticket set, ADR links, and Out-of-scope section. Present it, then ask separately:

```text
Should I close this program now?
```

Earlier proposal approval never authorizes Map closure. On an affirmative answer only, run:

```bash
gh issue close <program-number>
```

If the human declines or does not answer, leave the Map open. If closing fails, report the failure and do not claim completion. After success, re-fetch the Map and verify its closed state.

Conclude with the selected ticket and active Map; the verified `Program:` and `Vision:` linkages; machine-proved delivery; whether all required external observations were supplied; destination-answer polarity; the exact one-line decision and applicable ADR links; the divergence ticket when applicable; every other minted ticket and native or fallback dependency; retained Fog and current Frontier; `CONTEXT.md` and ADR changes from the grill; whether Program-done was detected; whether Map closure was offered, declined, or verified; and every partial-state warning. Never report an issue, edge, edit, or closure that was not re-fetched or otherwise verified.
