# Follow-up: close two gaps on `repair/orchestration-dead-ends`

Continue on the same worktree and branch:
`/Users/nicolaslazaro/Desktop/work/pce-repair`, branch `repair/orchestration-dead-ends`,
last verified at `7ec3a05`.

All previous boundaries still apply: never build or write in the main checkout at
`/Users/nicolaslazaro/Desktop/work/pce`, never run `install.sh`, never push, merge, tag, or
open PRs, and keep `CARGO_TARGET_DIR` off the main checkout's `target/`. Six live runs still
read the symlinked binary and skill directory.

Your work was independently verified and holds up. Two gaps remain. Method is yours.

---

## First: one of your claims was wider than the evidence

You reported "All 17 live logs replayed successfully." The event logs do all parse — 17 of 17,
5,124 records, confirmed. But `pce status` fails on **8 of those 17** with
`failed to parse ratified acceptance criteria from vision.md`.

This is **not a regression** — the current `main` binary fails on the identical eight logs.
It is the pre-existing legacy-prose-vision boundary. No action needed on it here, and do not
try to fix it; it belongs to an open design question. It is stated only so your next report
distinguishes "the event log replays" from "`pce status` succeeds," because those diverge.

---

## Gap 1 — the two acceptance criteria have no tests

The two headline results were verified once, by hand, and are encoded nowhere. Nothing will
detect it when they stop being true. That is precisely the failure class this tool exists to
catch: a claim that passes with no falsifier attached.

**What must be true when you are done:**

1. A test proves that event logs written by the pre-repair binary still parse and still derive
   correct state — including that historical round counts keep their original meaning.
2. A test proves that the four previously-stranded `(node, role)` series are admissible, and
   that a series which reaches the spending limit parks and can be resumed exactly once by a
   recorded human decision. The four are `m1-s2` (from RivRetrieve
   `2026-08-11-the-store-is-the-only-copy`) and `m3-s4`, `m3-s9`, `m3-s11` (from taqsim
   `2026-08-10-incidence-core`).
3. Both tests are **self-contained**: they must not read paths outside the repository, and must
   not depend on logs that are still being appended to by live runs. Derive whatever fixture
   data you need and commit it.

Note for accuracy: those four nodes are currently admissible because the limit rose from 3 to
12 and their counts are 3 — the typed resume path is not what unstuck them. Make sure the
resume path itself is exercised against realistic state, not only a synthetic minimum.

## Gap 2 — the rename stopped at the code

You renamed the constant to `VALIDATED_PRODUCTION_SPENDING_LIMIT`, which is right. But the
surface a human actually reads did not change. In `pce status` output today:

- `spending` occurs **0** times
- `park` occurs **0** times
- `cap` occurs **39** times

So an orchestrator or a person reading status still sees the old vocabulary describing the new
behaviour, which is worse than either alone. The whole point of the rename was that the name
should state what the number measures.

**What must be true when you are done:** the machine-readable status surface names the spending
limit for what it is and makes a parked series visibly parked and visibly resumable.

**One constraint, because it touches live work.** The run-snapshot schema at
`skills/pce/schemas/run-snapshot.schema.json` is part of the installed surface, `SKILL.md`
requires it to be verified at startup, and orchestrators read the JSON it contracts. Renaming
output fields is therefore a breaking change to a live contract. Handle that coherently —
schema, emitted output, and any `SKILL.md` text that names these concepts must agree with each
other in the same commit. If a field must be renamed rather than added, say so explicitly in
your report so the migration is a decision rather than a surprise.

---

## Report back with

- What each new test asserts and how it would fail if the behaviour regressed.
- Whether any status field was renamed rather than added, and what an in-flight run reading the
  old field would experience.
- Anything you found while doing this that contradicts what either of us believed.
