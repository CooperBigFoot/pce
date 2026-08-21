# Brief: the queue shows the orchestrator's raw report, because there is nowhere to put a sifted card

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-21 against `main` at `740d169`.

This is new work against merged code. No criterion of
`planning/2026-08-21-what-reaches-the-human/graph.v1.json` is edited by it.

## The defect

The whole point of this vision is that an orchestrator's dense technical report reaches the operator
as plain language he can rule on in one sentence. Measured against the merged binary with a seeded
store, the page renders the orchestrator's report **verbatim**:

```
$ pce hold open --root ./holds --repository pourpoint --plan-version 19 --package GD10 \
    --question-kind criterion-revision --report "<the orchestrator's own text>"
$ pce overseer view --root ./holds
```

produces, for each hold, an `<h2>repository · package</h2>`, the question kind, an age, and:

```html
<pre class="report">&quot;An earlier repair deleted GD10&#39;s read gate and put nothing in its place. verify_case no longer requires retained evidence …&quot;</pre>
```

That is the exact register this Program exists to remove.

**It is not a rendering bug.** `HoldRecord` (`crates/core/src/hold_store.rs:156`) has six kinds —
`Opened`, `Answered`, `Routed`, `RouteRefused`, `ReportingRunRequest`, `Closed`. None of them carries
a sifted card. The overseer skill's sifting discipline (`skills/overseer/SKILL.md` §2) produces a
card *internally* and has nowhere durable to put it, so the view has nothing to render but the
opening report.

The sifting criteria pass because they test what the skill produces, never what the operator sees.
The human-facing half of the promise is unwired.

## Part 1 — the store must carry the sifted card

Add a record kind the overseer writes when it routes a hold to the human. Field shape is your
decision; these constraints come from ratified decisions in `CONTEXT.md` under this Program and are
not negotiable:

- **The card carries the reporting orchestrator's own options and its own recommendation.** The
  sifter translates register; it never authors options and never advocates. A hold routed to the
  human whose report named no options must not acquire invented ones — the existing
  `ReportingRunRequest` path already covers returning such a report to its run.
- **Modal force is preserved.** `may`, `might`, `could`, `appears`, `likely`, `reported`,
  `according to` and explicit negation survive into the card. `the environment may have failed`
  never becomes `the environment failed`.
- **The raw report stays reachable.** The card is an additional record, never a replacement. The
  hold file remains the complete thread, and the operator must be able to reach the original text.
- **A door card carries exactly one consequence sentence** — what is true in the world after the
  option lands. Door kinds are criterion revision, worker-environment extension, base-currency
  acceptance, park overrule, and publication. Non-door cards carry none.
- **An overseer-supplied fact is distinguishable from the run's own report.** The overseer's job
  includes supplying facts a run cannot see; the operator must be able to tell whose claim he is
  reading.

## Part 2 — the view must render the card, and read like a product

`crates/core/src/overseer_view.rs:348` emits an unstyled document: no stylesheet, states expressed
only as invisible `data-state` attributes, every hold expanded at once with a bare answer form
underneath, and the run registry printed as absolute paths.

**A design reference is committed at `briefs/assets/overseer-queue-reference.html`.** Open it. It is
a static mockup with fabricated data — do not copy its markup or its hardcoded content. Reproduce
its *decisions*:

- **A register split carried by typography.** Monospace for anything the machine owns — repository,
  package, keys, refs, tags. Serif for the sifted prose the operator reads. The page's subject is
  translation between two registers, and the type encodes it.
- **A queue rail and one open card**, not every hold expanded at once. The operator drains a
  worklist; he does not scroll a feed.
- **The three states visible at a glance**, not as attributes: waiting for the human, with the
  overseer, closed. A card that is with the overseer shows what it is waiting on and has no answer
  box.
- **Liveness as a visible indicator** carrying all four states the lifecycle work added — working,
  idle, cannot-start, not-running. Idle and cannot-start must not look alike.
- **Holds settled without the human in a collapsed drawer at the bottom**, showing a count. It is
  the evidence the overseer is doing its job, and it must not compete with the queue.
- **A thread**, so a clarifying question and its answer are visible in the card.
- **A feedback box** that files raw operator text for the overseer to turn into a brief.
- Both light and dark, responsive, and no horizontal page scroll.

Answering must not close a hold. Only the overseer writes the closing record; the page's copy must
say so rather than implying a send-and-dismiss.

## What this does not change

- The four attributed doors. Nothing here lets any session sign a record.
- The hold lifecycle, routing refusals, rule admission, or the wake-and-exit session model.
- The hold store's append-only discipline: the card is a new record, never a rewrite.
- Any frozen criterion. If satisfying this brief appears to require editing one, stop and report
  which criterion and why.

## Falsifiers worth pairing

The defect this brief fixes shipped behind passing criteria, so prefer checks that fail on the
current code and pass after:

- A hold whose report contains `may have failed` produces a card routed to the human that contains
  `may have` and does not contain a bare assertion that it failed.
- A hold routed to the human renders its card rather than its opening report, and the opening report
  is still reachable.
- A report naming no options produces no human card.
- A non-door card carries no consequence sentence; a door card carries exactly one.
