# Brief: /work-ticket hands off to a stale skill and buries the handoff in ceremony

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-20. Ruled by the human the same day.

Target: `skills/work-ticket/SKILL.md` in the pce repo (`./install.sh` syncs it to
`~/.claude/skills/`; the supervisor runs it, you do not). Documentation only — no Rust changes.

## Two defects, both in the last third of the skill

### 1. The handoff stops one stage short of the pipeline

Section 8 tells the user to run `/to-vision "<name>"`, and section 6 says the derived name is
retained "for both `pce vision new` and the final `/to-vision` handoff".

A claimed ticket now needs to end at a **frozen work-package graph**, not a vision: `/work-graph`
supervises a frozen graph and cannot run on a vision alone. `/to-graph` is the skill that gets there,
and it already subsumes the vision step — its section 1 says, verbatim, "Invoke and follow the
sibling `to-vision` skill to create the directory and write `vision.md` against its fixed template."

**Consequence for the rest of the skill: less than it looks.** Because `/to-graph` delegates to
`/to-vision`, which runs the same `pce vision new "<name>"`, the reservation contract in section 6
and the linkage stamping in section 7 stay correct exactly as written — same verb, same
byte-identical name, same same-day idempotent reuse of the reserved directory. Do not rewrite them.

**Change:** the handoff command becomes

```text
/to-graph "<name>"
```

and every sentence that names `/to-vision` as the thing the user runs is reworded to name
`/to-graph`, while remaining accurate that the vision materialization happens *inside* it. Section 6's
"for both `pce vision new` and the final `/to-vision` handoff" and section 8's "`/to-vision` runs its
own `pce vision new`" both describe a delegation that is now one level deeper; say so plainly rather
than deleting the caution. The midnight caveat still applies and stays — it is a property of
`pce vision new`, not of which skill calls it.

Keep the existing prohibitions: do not automatically invoke `/to-graph`, do not automatically invoke
`/pce` or `/work-graph`, do not write `vision.md`. Presenting the command and stopping is the point.

### 2. Section 8 buries what the user needs under what you verified

Section 8 currently mandates, in order: an end-state picture, the acceptance criteria one line each
with all three fields, open items, a reversibility judgement, then four numbered bookkeeping items,
then the same-day warning, then a list of things not to do. The reader is the person who just spent
a long grill converging this vision; most of that is telling them what they already know, or telling
them what you checked.

**Change: the report ends with three things.**

1. **The end state in two or three sentences** — where we start, where we end, what disappears, what
   someone does next. Plain words, no ticket number, no path, no validation result.
2. **Anything genuinely unresolved**, without guessing at answers. Omit the heading entirely when
   nothing is open.
3. **The exact handoff command**, with the byte-identical retained name.

Everything else — selected ticket and Program Map, glossary entries and ADRs, reserved directory,
verified `Vision:` linkage — is verification you performed, not information they need. State in one
short line that it is available on request, and do not enumerate it. Add the reversibility judgement
only when reversibility is not obvious from the end state; a sentence that always says "this is
reversible" carries no information.

**Deliberately dropped: the acceptance-criteria enumeration.** The human ruled on this: `/to-graph`
re-derives the criteria and settles them with the human as its own step, so listing name, input and
observation at the ticket boundary is a duplicate review of the same material minutes before the
real one. If you conclude while implementing that this loses something the grill cannot recover,
stop and report rather than reinstating it silently.

## Decisions delegated to you

1. Whether the "available on request" line names the categories (ticket, Map, glossary, ADRs,
   directory, linkage) or stays generic. Recommendation: name them in one comma-separated line, so
   the user knows what they can ask for without reading a list of what happened.
2. Whether section 8's style prohibitions ("never open a report with an issue number, a directory
   path, a linkage line, or a validation result") survive as written. They encode the same judgement
   as change 2 and may now be redundant with it; keep whichever statement is shorter, not both.
3. How the same-day / midnight caution is phrased once the delegation is two levels deep. It must
   stay true: `/to-graph` invokes `/to-vision`, which runs `pce vision new "<name>"`; the same name on
   the same day reuses the reserved directory untouched, and crossing midnight creates a new
   date-stamped directory that diverges from the stamped `Vision:` line.

## Out of scope

- **The directory reservation itself stays in `/work-ticket`.** `/land-ticket` reads the
  `Vision: planning/<YYYY-MM-DD>-<slug>` line from the Effort ticket body, so the linkage must exist
  at claim time. Two skills knowing about `pce vision new` is the cost of that contract and is not
  being consolidated here.
- Sections 1 through 5 and 7. No changes to ticket discovery, validation, context assembly, the
  grill, or linkage stamping beyond the `/to-vision` → `/to-graph` rewording named above.

## Verification

There is no test suite for skill text. Verify by reading: after the change, a reader of section 8
must be able to reach the handoff command within the first screen, and no sentence anywhere in the
skill may tell the user to run `/to-vision`. Confirm `grep -n "to-vision" skills/work-ticket/SKILL.md`
returns only occurrences that describe `/to-graph`'s internal delegation.

## The waiting consumer

The human, who claims Effort tickets with `/work-ticket <n>` and then has to scroll past four blocks
of bookkeeping to find the one command they need — and who, on running it, would today be sent to a
skill that stops before the artifact `/work-graph` requires.
