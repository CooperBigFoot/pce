---
name: land-ticket
description: Use `/land-ticket <n>` to land one validated Effort ticket in the single active Program, record its decision, graduate Fog through a reviewed Grill-with-docs proposal, and ask before closing a completed Program.
---

# Land Ticket

Land one delivered Effort ticket and expose the next Frontier. Run every command from the repository whose Program is being landed. Reconstruct durable state on every invocation from GitHub issues, committed repository source, and human answers; never rely on a previous conversation or a local Program state file.

Follow this order exactly: parse the argument; discover exactly one active Program; fetch and validate the Effort ticket and Map without mutation; trust the human's delivery assertion; establish the landed decision and ADR links; load complete Program context; conduct the Fog-graduation Grill-with-docs session; classify sharp tickets, retained Fog, and blockers; obtain approval for one complete proposal; ensure the ticket label if needed; apply only the approved changes; re-fetch state; detect Program completion; ask separately before closing the Map; then report verified results and any partial failure.

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

## 3. Fetch and validate before any mutation

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

Validation proves linkage format and membership only. **TRUST THE HUMAN:** the human invoking `/land-ticket` is authoritative that the linked vision was delivered and merged. Ask for that assertion explicitly. Do not inspect the planning directory, `vision.md`, PCE state, commits, git history, pull requests, implementation output, acceptance gates, or delivery artifacts to independently verify delivery. If the human does not assert delivery or says it is not merged, stop without mutation.

## 4. Establish the landed decision and ADR links

Inspect the ticket body and comments, Map body and comments, and committed documentation for an unambiguous existing decision and any ADR produced by the ticket's Grill-with-docs session. Because prior chat is not durable, ask the human to supply or confirm the exact one-line decision when it cannot be reconstructed unambiguously. Ask whether that Grill-with-docs session produced an ADR when the fact is not durable. Never invent a decision or ADR association.

Use one physical Markdown line:

```markdown
- #<n> — <one-line landed decision>
```

Append every applicable ADR link on that same line:

```markdown
- #<n> — <one-line landed decision> ([ADR NNNN](<repository-url>/blob/<default-branch>/docs/adr/NNNN-short-slug.md))
```

Resolve the repository URL and default branch from the current repository. Accept an identified ADR only when its file is under `docs/adr/` and exists in committed source. Omit the parenthetical when no ADR was produced. If multiple ADRs genuinely apply, link each on the same decision line.

The approved Map edit must append this entry inside the existing `## Decisions so far` section immediately before the next `## ` header or end of body. Preserve all existing entries and every other section. Never append outside the section, replace history, or duplicate the selected ticket's entry. If `#<n>` already has an entry, reconcile it explicitly in the reviewed proposal.

## 5. Load complete durable Program context

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

## 6. Conduct the Fog-graduation Grill-with-docs session

Invoke and follow the sibling `grill-with-docs` skill completely, including its interview loop, domain-modeling formats, documentation decision rules, and finish rules. Specialize it narrowly to the current Map's `## Not yet specified` Fog after incorporating the selected ticket's landed decision, Map Notes, Decisions-so-far index, every existing Program ticket and blocker, and the root glossary. Do not re-survey or redesign the Program and do not decompose milestones, implementation steps, or code tasks.

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

## 7. Build and approve one complete proposal

Before closing the selected ticket, editing the Map, ensuring a label, creating an issue, or changing any dependency, present one complete proposed GitHub change set containing:

1. The selected Effort ticket `#<n>` and its open-to-closed transition, explicitly stating that delivery is trusted from the human and not independently verified.
2. The complete proposed Map title and full body: the decision entry or reconciliation in `## Decisions so far`; the complete retained `## Not yet specified` Fog; links or index entries for new tickets using mechanical placeholders; and every preserved section.
3. Every proposed newly sharp Effort ticket title and complete seeded-lean body.
4. Every blocking edge, naming blocker and blocked ticket, and the exact reviewed `Depends on: #M` fallback to write if native blocking is unavailable.
5. An explicit current-to-proposed reconciliation covering ticket closure, decision append or reconciliation, minted tickets, retained Fog, unchanged tickets and sections, Map link additions, dependency additions or removals, and every issue-number placeholder with its mechanical substitution explained.

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

Ask for explicit human approval of the entire proposal. Questions or revisions return to the grill and proposal cycle. Any substantive change to the Map body, ticket bodies, selected closure, or edges invalidates approval and requires presenting the complete revised proposal. Partial approval is insufficient. No GitHub mutation may precede approval; Grill-with-docs documentation changes do not authorize issue writes.

## 8. Apply only approved GitHub changes in auditable order

If and only if new Effort tickets are approved, ensure the established shared label immediately before creating them:

```bash
gh label create pce:ticket --force
```

Reuse exact label `pce:ticket`; never create a duplicate or renamed variant. If assurance fails, report the command and failure and stop before any issue write that needs the label. Do not require label creation when no ticket will be minted.

Apply only the approved changes in this order:

1. Ensure `pce:ticket` when new tickets will be created.
2. Close the selected ticket with exactly `gh issue close <n>`. If it fails, report it and stop; do not claim the ticket landed.
3. Update the Map with the approved full body, including the reconciled decision entry and retained Fog while preserving every other approved section.
4. Create only the approved newly sharp tickets with label `pce:ticket`, retaining every assigned issue number.
5. Mechanically substitute reviewed number placeholders and update approved Map ticket links or indexes.
6. Create approved native blocking relationships whenever reachable through the supported `gh` CLI or API. Make each named blocker block the named blocked ticket. If native blocking cannot be created, write the already-reviewed exact `Depends on: #M` line into the blocked ticket body. Do not claim a native edge when only fallback text exists, and do not retain fallback text for a successful native edge unless the proposal approved both.

Retain the issue number and result after every mutation. If any write fails partway, stop immediately and report exactly what closed, what was created or edited, what failed, and what remains unapplied. Do not conceal, roll back, or automatically compensate for partial state. Only mechanical number substitution and the reviewed native-to-fallback choice are pre-authorized; any substantive deviation requires a new complete review.

## 9. Re-fetch, detect Program completion, and report

After all approved landing and Fog-graduation writes succeed, re-fetch the active Map, every open issue labeled `pce:ticket` with complete bodies, and dependency state. Program-done is true only when both conditions hold simultaneously:

1. No open `pce:ticket` issue has exactly one `Program: #N` linkage to the active Map. Count Program members, not foreign tickets.
2. The Map's `## Not yet specified` section has no substantive Fog after ignoring whitespace and the untouched template comment.

Newly minted Effort tickets are open, so they make the Program not done. If either an open member ticket or retained Fog remains, keep the Map open, report the remaining Frontier and Fog, and do not ask to close it.

Only when both conditions are true, compose a final Program summary from the Map Destination, Notes, complete Decisions-so-far index, completed member-ticket set, ADR links, and Out-of-scope section. Present it and ask the human separately and explicitly whether to close the Map. Earlier proposal approval does not authorize Map closure. On an affirmative answer only, run:

```bash
gh issue close <program-number>
```

If the human declines or does not answer, leave the Map open. If closing fails, report the failure and do not claim completion. After success, re-fetch the Map and verify its closed state.

Conclude with the selected ticket and active Map; the verified `Program:` and `Vision:` lines; the trusted-human delivery stance; the exact one-line decision and applicable ADR links; newly minted tickets; the native or fallback representation of every dependency; retained Fog and current Frontier; `CONTEXT.md` and ADR changes from the grill; whether Program-done was detected; whether Map closure was offered, declined, or verified; and any partial-state warning. Never report an issue, edge, edit, or closure that was not re-fetched or otherwise verified.
