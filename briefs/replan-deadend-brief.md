# Brief: a parked package is a dead end the driver cannot walk back from

Status: GRILLED 2026-08-17 — ready to dispatch. Decisions and acceptance criteria are in the
"Grilled decisions" and "Acceptance criteria" sections below; doctrine is recorded in
`docs/adr/0014-adjudication-enters-the-journal.md` and the `Park adjudication` /
`Carried completion` entries of `CONTEXT.md`. Written during the second real-world driver
run (vision `2026-08-10-incidence-core`, seven-package Rust graph).

## The incident that exposed this

IC3's worker worked for 11 minutes, committed a real implementation, and wrote the outcome
`{"outcome":"mis-specified","fault":{"kind":"criterion","name":"Books close"}}`. The driver
parked IC3 immediately (designed: `docs/evidence/wp3-recovery.md` — "mis-specified is neither
charged nor retried; the graph, not the implementation attempt, must be re-authored"), the
ready antichain emptied, and the run exited `blocked`.

The human then re-ran the identical `driver-run` command. It printed the blocked status and
exited. There is no verb, flag, or event that continues from here. The evidence commit of the
parked attempt is preserved as tag `pce-evidence/ic3-attempt-3-mis-specified` in
`/Users/nicolaslazaro/Desktop/work/incidence`.

## The defect

Park is documented as terminal "for this plan version" (`crates/core/src/package_driver.rs:277`),
and the doctrine says the escalation target is a re-authored graph at plan version n+1. But the
resume side of that escalation does not exist:

1. Journal events carry no plan version. `derive_driver_snapshot` folds every event against
   whatever graph it is handed. Freeze a revised `graph.v2.json` and point `driver-run` at it
   with the same journal, and the old `package-parked` event still folds onto IC3 — still
   parked, still blocked. The designed escalation ("re-author for n+1") has no landing.
2. The only journal the driver accepts as unblocked is a fresh one, which forfeits every
   completed package's proof (here IC1 and IC2, ~40 minutes of dispatched, gated, replayed
   work) and re-executes them from scratch.
3. Un-parking by hand requires journal surgery plus four coordinated side-effect cleanups,
   because attempt identity is deterministic in (vision, package, issuance): the re-dispatched
   attempt reuses the same branch name `pce/<vision>/<pkg>/attempt-<n>`, the same
   `/tmp/pce-work-package-worktrees/<hash>` path (agent name is a hash of exactly those
   inputs), and the same `package-outcomes/<pkg>/<n>.json` outcome path — and nothing
   preclears a stale outcome file, so a leftover one is read as the new worker's testimony
   (the driver only checks presence after exit, `src/main.rs:4043`). The surgery that worked,
   in order: delete the attempt's three journal events (worker-dispatched,
   dispatch-pane-opened, package-parked); remove the attempt's git worktree, then its branch;
   move the stale outcome file aside; close the leaked herdr workspace. Each omitted step is a
   distinct failure mode on restart.

## Why this matters beyond this run

This is the same shape as the pending `/land-ticket` finding on Program #37: the journal is
the only admissible proof, and the journal cannot say the thing that is now true. There, a
crashed driver leaves delivered work unlandable; here, a human judgement ("this criterion is
fine" or "here is the revised graph") has no way to enter the record. Park doctrine routes a
spec dispute to the human, but the human's answer has nowhere to go.

## What a fix must answer (the grill questions)

- What is the durable form of the human's adjudication? A journal event appended by a human
  verb? A new plan version whose freeze carries forward completed-package proofs? Something
  else? (The append-only journal doctrine should survive whatever is chosen.)
- When plan version n+1 revises only the parked package's criteria, what carries IC1/IC2-style
  completions across: replaying their journal events against the new graph after checking the
  packages are unchanged, or re-deriving proof from repository state (branch + criteria
  re-execution)?
- Does an adjudication of "the criterion was fine, re-attempt as-is" exist, or is the only
  path a graph revision? (This run's human chose re-attempt-as-is; today that is expressible
  only as surgery.)
- Attempt identity: should a resurrected or re-planned attempt ever reuse an issuance number,
  or must issuance be strictly monotonic in the journal even across surgery/replan so branch,
  worktree, and outcome paths can never collide?

## Adjacent defects seen in the same incident — explicitly NOT in this brief

- A `mis-specified` outcome carries no rationale (schema is kind+name only,
  `crates/core/src/package_worker.rs:266-330`), so the human adjudicates blind. The worker's
  own worktree passed all five criteria commands, and nothing recorded why it disputed the spec.
- `package-parked` does not clean up the dispatch pane; completion does. Workspace `wBD`
  ("IC3:incidence:attempt-3") was left open.
- `/tmp/pce-work-package-worktrees/` accumulates worktree directories across runs (~96 found).

## Environment facts a dispatched agent will need

- pce repo: `/Users/nicolaslazaro/Desktop/work/pce`, main at a15dca5. Merges happen only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration` (branch `integration/work-package-harness`);
  a merge in the main checkout rewrites `skills/pce/` which live runs read through a symlink.
  After merging: run the full suite there, fast-forward main, `cargo build --release`.
- Six other PCE runs share this machine and the installed binary at `~/.local/bin/pce`;
  a rebuild swaps it under them.
- Live incident artifacts: vision dir
  `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core/`
  (journal, `driver-journal.jsonl.bak-before-unpark` showing the parked history,
  `package-outcomes/IC3/3.json.bak-mis-specified`, frozen `graph.v1.json` at plan_version 1).
- Key code: `crates/core/src/package_driver.rs` (events, fold, `DriverPackageState::Parked`),
  `src/main.rs` driver loop ~2960-3300 (restart handling, issuance allocation
  `max(WorkerDispatched)+1`, dispatch issue), `crates/core/src/herdr_dispatch.rs:529-561`
  (deterministic agent name, branch, worktree path), `src/main.rs:6770` (`run_graph_freeze`,
  which already supports freezing v2 when `graph.json`'s plan_version is bumped and v1 exists).
- Recovery doctrine: `docs/evidence/wp3-recovery.md`; driver evidence: `docs/evidence/wp7-driver.md`.

## Grilled decisions (2026-08-17, ratified by the human)

Status: GRILLED — the questions above are answered. Glossary entries: `Park adjudication`,
`Carried completion` in `CONTEXT.md`.

1. **Two adjudication doors, no third.** An **overrule** is an appended journal event written
   by a new human verb ("the criterion stands"); it un-parks the package for a fresh attempt
   against the same frozen graph. A **sustain** has no verb: freezing graph v(n+1) is itself
   the ruling, and the driver reads the new freeze as dissolving the park. There is no
   in-place criterion edit — changing one word of one criterion is a new plan version.
   Journal surgery is never a door; no event is ever deleted or rewritten.
2. **Overrule charges nothing** (ratified). Mis-specified stays neither charged nor retried;
   a ruling that the spec was fine cannot retroactively convert the dispute into a charged
   failure. Empirical support: on this incident's re-drive, the *identical* brief succeeded
   on redraw — re-attempt-as-is is a sometimes-correct verdict, not an escape hatch.
3. **Overrule is not repeatable on the same package.** A second `mis-specified` after an
   overrule parks with graph revision as the only exit: two workers independently refusing
   the same spec outranks one human ruling.
4. **Carry-forward is journal replay, gated on unchanged definition.** A completed package's
   proof replays against v(n+1) only when its brief, criteria, and dependencies are
   byte-identical between versions; any revision makes it new work. No re-deriving proof
   from repository state.
5. **Issuance is strictly monotonic** in the journal across overrule, replan, and any future
   surgery — an attempt number is never reused, so branch, worktree, and outcome paths can
   never collide. Independently, the driver refuses (or preclears with a recorded event) a
   pre-existing outcome file at dispatch instead of reading it as the new worker's testimony.
6. The adjudication event carries a free-text rationale field so a later reader knows why
   the human overruled. (The worker-side missing-rationale defect stays out of scope.)

## Acceptance criteria

Each is name / input / observation. #5 is the designed-to-fail probe.

1. **Overrule un-parks without surgery** — input: the incident's preserved journal
   (`driver-journal.jsonl.bak-before-unpark`) with frozen `graph.v1.json`; run the overrule
   verb on IC3, then `driver-run`. Observation: IC3 dispatches a fresh attempt with an
   issuance strictly greater than every issuance in the journal; IC1/IC2 remain complete;
   the pre-overrule journal is a byte-identical prefix of the post-run journal.
2. **Stale outcome is not testimony** — input: pre-plant an outcome file at the exact path
   the next attempt will use, then dispatch. Observation: the driver refuses or preclears
   with a recorded event; the planted file's contents never appear as the worker's outcome.
3. **Sustain carries finished work** — input: freeze v2 revising only IC3's criteria; run
   `driver-run` with the same journal. Observation: IC1/IC2 stay complete with zero
   re-dispatch; IC3 leaves Parked and becomes ready.
4. **A revised package cannot smuggle its completion** — input: freeze v2 that also edits
   one criterion of completed IC1; run `driver-run`. Observation: IC1's completion does not
   carry — it is reported as new work, never silently treated as done.
5. **Second dispute outranks the human** (designed to fail the happy path) — input: overrule
   IC3, then have the next worker write `mis-specified` again; attempt a second overrule.
   Observation: the package parks and the overrule verb refuses, naming graph revision as
   the only exit.
6. **Adjudication survives replay** — input: after an overrule and a completed re-attempt,
   re-run `driver-run` from the journal alone. Observation: the fold reaches the same
   states with no re-dispatch and no residual park.
