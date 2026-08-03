---
name: work-ticket
description: Use `/work-ticket <n>` to claim one Effort ticket from the single active Program, reconstruct its ticket, Map, and domain context, Grill-with-docs toward one contained vision, reserve and record its vision directory, and hand off without automatically invoking `/to-vision` or `/pce`.
---

# Work Ticket

Claim one Effort ticket and discover one ambitious, contained vision. Run every command from the repository whose Program is being worked. Reconstruct durable state on every invocation; never rely on a previous conversation.

Follow this order exactly: parse the argument; discover exactly one Program; fetch and validate the Effort ticket and Map; claim the ticket; assemble context; conduct the deep Grill-with-docs session; derive the vision name once; reserve the directory; stamp and verify the linkage idempotently; then report and hand off.

## 1. Parse the Effort ticket number

Set `<n>` to `$ARGUMENTS`. Require exactly one usable GitHub issue number. If the argument is absent, malformed, or contains anything other than that one number, report a clear error and stop.

## 2. Discover exactly one active Program

Run exactly:

```bash
gh issue list --label pce:program --state open
```

Count the results and apply this rule prominently:

> ZERO open programs => clear error and stop; EXACTLY ONE open program => continue with that issue as the active Program Map; MORE THAN ONE open program => report every conflicting issue number and title clearly, then stop.

Never create or re-survey a Program. Require **EXACTLY ONE** open Program. On zero or more than one result, stop before validating or assigning the Effort ticket, grilling, reserving a directory, editing an issue, or invoking any downstream workflow.

## 3. Fetch and validate before mutation

Fetch the complete numbered issue. If the command fails, show the failure clearly and stop:

```bash
TICKET_JSON="$(gh issue view "<n>" --json number,title,body,labels,url)"
```

Fetch the complete active Map issue, substituting its discovered number for `<program-number>`:

```bash
MAP_JSON="$(gh issue view "<program-number>" --json number,title,body,url)"
```

Before any mutation, require all of these conditions:

1. Issue `#<n>` exists. Treat any failed ticket fetch as fatal.
2. Its labels include the exact required label name `pce:ticket`. Otherwise report that `#<n>` is not an Effort ticket and stop.
3. Its body contains exactly one root-level linkage line in the normative form `Program: #N`, and `N` equals the discovered Map number. Reject a missing, malformed, duplicate, or conflicting Program linkage clearly. Do not infer membership from links, title text, or user intent.

The ticket template's normative linkage is:

```markdown
Program: #N
```

Only after every validation succeeds, claim the Effort ticket by assigning it to the current authenticated GitHub user with exactly:

```bash
gh issue edit <n> --add-assignee @me
```

Assignment is the claim. Do not create a local claim file or use another state mechanism. If assignment fails, report the command failure and stop before the grill, directory reservation, or `Vision: ` mutation.

## 4. Assemble cold-session context

Treat the GitHub Map as the durable Program state. Fetch the complete ticket body and complete Map body even though they were included in the validation responses:

```bash
TICKET_BODY="$(gh issue view "<n>" --json body --jq '.body')"
```

```bash
MAP_BODY="$(gh issue view "<program-number>" --json body --jq '.body')"
```

Require exact `## Notes` and `## Decisions so far` headers in `MAP_BODY`, each ending at the next `## ` header or end of body. The Map template fixes them as:

```markdown
## Notes
<!-- Record durable program context. -->

## Decisions so far
<!-- Maintain the running one-line decision index that landed tickets append to. -->
```

After confirming both headers exist, extract only their content:

```bash
MAP_NOTES="$(printf '%s\n' "$MAP_BODY" | awk '/^## Notes[[:space:]]*$/{capture=1; next} /^## /{if(capture) exit} capture')"
MAP_DECISIONS="$(printf '%s\n' "$MAP_BODY" | awk '/^## Decisions so far[[:space:]]*$/{capture=1; next} /^## /{if(capture) exit} capture')"
```

Load the committed root glossary from the repository root:

```bash
ROOT_CONTEXT="$(cat CONTEXT.md)"
```

If either expected Map header or root `CONTEXT.md` is missing or unreadable, name the missing source clearly and stop. Do not grill with incomplete context. Present the full ticket body, extracted Map Notes, extracted Decisions-so-far index, and root context as the discovery starting material. Do not load the Map's Fog as Effort ticket scope or broaden the selected Effort ticket into a Program re-survey.

## 5. Conduct one deep Grill-with-docs session

Invoke and follow the sibling `grill-with-docs` skill completely, including every interview-loop, documentation-decision, glossary, ADR, and finish rule it defines. Do not duplicate or weaken that workflow. Specialize its subject to the selected Effort ticket, full ticket body, Map Notes, Decisions-so-far index, and root glossary.

Conduct a **deep** survey of this one Effort ticket toward one ambitious, contained, vision-sized result. This is not the breadth-first Program survey used by `chart-program`. Do not resolve other Effort tickets, re-survey the Program, or decompose the vision into implementation milestones, steps, or code tasks.

Ask exactly one prose question at a time, with a recommended answer and its reason. Inspect code and committed documentation instead of asking questions those sources answer. Resolve decision branches and dependencies, challenge language against `CONTEXT.md`, test boundaries with concrete edge cases, and capture crystallized terminology or qualifying decisions after each answer. Apply the sibling's domain-modeling formats and documentation rule during the interview: update glossary language immediately and offer an ADR only when all three of its durable-decision conditions hold. Finish only after every relevant branch is resolved or explicitly left open, then recap shared understanding, glossary entries updated, ADRs created, and unresolved questions without inventing answers.

The grill is not finished until three things exist, and `/to-vision` will need all three:

1. **An end-state picture** the user has reacted to — what is true today, what is true after, **what disappears**, and how the follow-on work happens once this lands. Plain language, no jargon, concrete enough that the user could repeat it to someone else. Show what goes away every time; that is where silent assumptions live.
2. **Acceptance criteria**, one per claim the vision makes. Each has exactly the three fields handed to `/to-vision`: a non-blank name, input, and observation, never a test that must exist. At least one names an input designed to make the thing fail. The user supplies the fear, you supply the probe, and they ratify in plain words that the probe would catch the fear — never ask them to ratify something they cannot evaluate. Flag any acceptance criterion checkable only outside the run that delivers it.
3. **A reversibility judgement.** State whether this vision contains an act that cannot be repeated — minting an immutable artifact, publishing a release or tag, consuming a one-shot quota, destroying history. Where it does, name that act; it is the only place that earns front-loaded proof. Where it does not, say so, and expect a light brief with heavy falsification of what was built.

Require the result to be sufficiently settled to name one contained vision. If discovery cannot converge on one contained vision, report the mismatch and stop without reserving a directory or stamping a speculative linkage.

## 6. Derive the name once and reserve the directory

After convergence, derive one human-readable `<name>` from the contained vision exactly once. Retain that exact byte sequence, including case, spacing, and punctuation. Use the retained value for both `pce vision new` and the final `/to-vision` handoff; never reconstruct it from the slug, issue title, or recap.

Reserve the directory with the existing CLI verb:

```bash
VISION_DIR="$(pce vision new "<name>")"
```

Apply this existing `pce vision new` contract exactly:

- Its **sole stdout output** is the relative path of the vision directory (`planning/<YYYY-MM-DD>-<slug>`). All diagnostics and warnings go to **stderr**, so the command substitution above captures exactly the directory path and nothing else. After a zero exit, trust `$VISION_DIR` unconditionally.
- **Different-date duplicate — warn and proceed**: if the same slug already exists under another date (e.g. `planning/2020-01-01-auth-refactor`), the command warns on stderr and proceeds, creating a fresh directory under today's date. Do not treat the warning as an error; do not stop.
- **Same-date rerun — idempotent no-op**: if today's directory already exists, the command leaves its contents — including any existing `vision.md` — **untouched**, and prints the existing directory path. Rerunning `/to-vision` on the same day never destroys prior work.
- **Non-zero exit** (empty or unsluggable name, `pce` not on PATH): show the user the stderr output and stop. If the binary is missing, tell the user to run `install.sh` from the pce repo first.

Do not implement or request a Rust CLI change. If reservation fails, stop and report that the Effort ticket was claimed but the directory was not reserved or linked.

## 7. Stamp and verify the exact Vision linkage

On successful reservation, update the Effort ticket body to contain exactly one root-level line in this exact machine-readable format:

```text
Vision: planning/<YYYY-MM-DD>-<slug>
```

Use the exact prefix `Vision: `, with one ASCII space, and the root-relative `$VISION_DIR` value. This is a cross-milestone contract read by `/land-ticket` in milestone 5.

Re-fetch the current complete ticket body immediately before editing. Create a temporary body file that preserves every non-`Vision: ` line byte-for-byte, removes every root-level line beginning exactly `Vision: `, and appends exactly `Vision: $VISION_DIR`. Preserve the body's final-newline state as far as the GitHub full-body edit permits. Use a cleanup trap for the temporary file and edit safely with:

```bash
gh issue edit <n> --body-file <file>
```

This replacement makes reruns idempotent and normalizes duplicate existing `Vision: ` lines to one. Do not append a second line and do not alter other ticket-body content.

After the edit, re-fetch the complete issue body. Verify that exactly one root-level line begins exactly `Vision: ` and that it equals exactly `Vision: $VISION_DIR`. If stamping or verification fails, stop and report separately that assignment and reservation completed but verified linkage did not. Never claim a linkage that was not verified.

## 8. Report and hand off

Lead with what the user can act on. Bookkeeping goes underneath, or waits until they ask.

Open with the **end-state picture** in plain words — where we start, where we end, what disappears, what someone does next. Then the **acceptance criteria**, one line each with the exact three fields handed to `/to-vision`: name, input, and observation. Then anything still open, without guessing at answers. Then the reversibility judgement in one sentence.

Only after that, and clearly marked as detail they may skip:

1. The selected Effort ticket and active Program Map.
2. Glossary entries updated and ADRs created.
3. The reserved directory and verified `Vision: $VISION_DIR` linkage line.
4. This exact invocation, substituting the byte-identical retained `<name>`:

   ```text
   /to-vision "<name>"
   ```

Never open a report with an issue number, a directory path, a linkage line, or a validation result. Those are yours to have verified, not theirs to read first.

Tell the user to complete the `/to-vision` handoff in the same session and on the same calendar day. `/to-vision` runs its own `pce vision new "<name>"`; the same byte-identical name on the same day reuses the reserved directory untouched. Warn that if the handoff crosses midnight, the date-stamped directory can diverge because `pce vision new` creates a new current-date directory.

Do **not** automatically invoke `/to-vision`. Do **not** automatically invoke `/pce`. Present only the exact handoff command. Do not write `vision.md`; `/to-vision` owns it. Do not begin Work, close the Effort ticket, edit the Map's Decisions-so-far index, graduate Fog, or run `/land-ticket`.
