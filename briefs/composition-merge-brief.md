# Brief: where parallel work joins, merging is work — and today nobody owns it

Status: GRILLED 2026-08-17 — ready to dispatch. Decisions and acceptance criteria are in the
"Grilled decisions" and "Acceptance criteria" sections below; doctrine is recorded in
`docs/adr/0015-a-conflicted-join-is-the-dependent-workers-first-task.md` and the
`Conflicted join` entry of `CONTEXT.md`. Written 2026-08-17 during the second
real-world driver run (vision `2026-08-10-incidence-core`), immediately after the run's
first parallel antichain (IC4 ∥ IC5) completed and their join (IC6) killed the run.

## The incident

IC4 and IC5 ran concurrently off IC3, both completed cleanly through criteria and gates.
The driver then composed IC6's base by merging both branches and hit a textbook add/add
conflict: IC4 added `pub mod disposition;` and IC5 added `pub mod dense_projection;` at the
same position in `crates/core/src/lib.rs`. Composition failed, the journal recorded
`package-composition-failed` for IC6, the run exited `blocked`, and re-running reproduces the
exit — `composition-failed` folds to a terminal package state with no continuation.

The human's resolution was two lines (keep both `pub mod` declarations); the merged base
passes the full 23-suite test run. It exists as tag `pce-compose/ic6-base` (60133df) in
`/Users/nicolaslazaro/Desktop/work/incidence`, installed by journal surgery: the
`package-composition-failed` line was replaced with a hand-written `package-base-composed`
event, which the driver accepts (pre-existing base-composed events are trusted and not
recomposed).

## Three defects, one incident

1. **No owner for the merge.** `compose_git_commits` (src/main.rs, called via
   `ensure_driver_package_bases`) runs `git merge` per dependency and treats any conflict as a
   terminal composition failure — before the dependent package's worker exists. Yet the most
   trivial conflict class (two siblings extending the same module list) is near-certain
   whenever two packages of one antichain touch one crate. As designed, parallelism in a
   single-repository graph regularly kills the run at exactly the point parallelism paid off.
2. **The evidence was swallowed.** The recorded reason is
   `failed to combine package IC5 commit 3deb441…: ` — empty after the colon. Git's actual
   conflict output (which names the file) was captured but not propagated. Every diagnosis in
   this incident came from re-running the merge by hand.
3. **Another dead end.** `composition-failed` is terminal in the fold; `ready` empties; the
   driver prints blocked and exits; re-running changes nothing. This is the second independent
   instance of the shape in `briefs/replan-deadend-brief.md` (a recorded fact the human can
   now refute, with no verb to say so). The fix dispatched for that brief covers park; this
   state needs the same doctrine applied or shared.

## The human's doctrine (to be grilled into shape)

Where parallel work merges, the graph must contain a merging step: a join is not plumbing, it
is work, with the same rights as any package — a worker to perform it, criteria to judge it,
recovery rungs when it fails. The driver's silent textual merge is acceptable only as the
degenerate case where the merge is clean.

## What a fix must answer (the grill questions)

- **Who resolves a conflicted join?** The dependent package's worker (IC6's worker receives a
  conflicted worktree plus the conflict evidence in its brief and resolving is its first
  task), or an explicit synthesized merge-package inserted between the antichain and its
  dependent? The first keeps the graph as authored; the second makes the merge visible,
  gateable, and separately recoverable.
- **Where does the clean/conflicted line sit?** Driver auto-merge for clean joins and worker
  escalation only on conflict — or does every join get a worker regardless, so behavior does
  not depend on textual luck?
- **Is a resolved merge judged?** After resolution, do the dependency packages' criteria
  re-run against the merged base before the dependent's worker starts (the conflict resolution
  could silently break a parent's guarantees — the assembly re-run would catch it only at the
  very end)?
- **Authoring-time duty:** should `/to-graph` be taught that every join point in the drafted
  graph implies a merge obligation, so the human sees it at plan time instead of at run time?
- **Recovery:** a failed composition should enter the normal ladder (retry / local patch with
  the conflict evidence inline / park) rather than a bespoke terminal state — does anything
  justify keeping `composition-failed` distinct from a criterion failure once a worker owns
  the merge?
- **Evidence:** whatever owns the merge, the journal must carry git's own words (conflicted
  paths, both sides' hunks or refs). The empty-reason bug is a one-line fix inside whichever
  design wins; it must not survive.

## Environment facts a dispatched agent will need

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at a15dca5. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration` (branch `integration/work-package-harness`);
  a merge in the main checkout rewrites `skills/pce/` which live runs read through a symlink.
  Full suite there, fast-forward main, `cargo build --release`. Six other PCE runs share this
  machine and `~/.local/bin/pce`; a rebuild swaps the binary under them — coordinate with the
  in-flight replan-dead-end fix (dispatched 2026-08-17 from `briefs/replan-deadend-brief.md`,
  post-grill) so the two changes land through integration sequentially, not interleaved.
- Key code: `src/main.rs` — `compose_git_commits` (~2290), `ensure_driver_package_bases` and the
  base-composed reuse path (`driver_package_base_refs`, ~2261), composition-failure event
  emission; `crates/core/src/package_driver.rs` — `PackageCompositionFailed` /
  `AssemblyCompositionFailed` events and the `CompositionFailed` terminal state in the fold
  (assembly composition at src/main.rs:2760-2840 has the same conflict exposure and the same
  swallowed-reason bug — one antichain before assembly makes it reachable).
- Incident artifacts: vision dir
  `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core/` —
  `driver-journal.jsonl.bak-before-ic6-compose` (last line is the failure event with the empty
  reason), current journal (last line is the hand-written base-composed event), tag
  `pce-compose/ic6-base` in the incidence repo, conflicting parents 898eb9b (IC4) and
  3deb441 (IC5).
- Related doctrine: `briefs/replan-deadend-brief.md` "Grilled decisions" section — the
  adjudication/continuation design ratified there should be honored, not re-invented, when
  deciding what a human does with a conflicted or failed join.

## Grilled decisions (2026-08-17, ratified by the human)

Status: GRILLED — the questions above are answered. Glossary entry: `Conflicted join` in
`CONTEXT.md`; doctrine ADR: `docs/adr/0015`.

1. **The dependent package's worker owns the merge** (ratified). A conflicted join is a
   starting condition, never a failure: the dependent's worker receives the conflicted
   worktree plus git's conflict evidence in its brief, and resolving is its first task. No
   merge-package is ever synthesized at run time — an unauthored package in a frozen graph
   contradicts plan-version doctrine.
2. **Clean joins stay silent** (settled from existing doctrine). The `Concurrency admission`
   promise — "overlap dispatches and the merge absorbs the rebase" — already drew the line:
   the silent driver merge is the degenerate case where the absorbing work is empty. A clean
   join charges nothing and involves no worker.
3. **Parents are re-proved on conflicted joins** (ratified). After the worker resolves, every
   parent's criteria re-run against the merged base before the dependent's own work is
   judged. A resolution that breaks a finished package's guarantee is caught at the join,
   not at assembly. Clean joins skip this entirely.
4. **`composition-failed` survives only for infrastructure faults** (settled). With conflicts
   owned, the terminal state remains reachable only by genuine git/worktree errors (missing
   ref, worktree creation failure), and those route through the environment-failure path
   landed in a15dca5 — never a bespoke dead end requiring journal surgery.
5. **Assembly gets the same treatment** (settled, reversible). Assembly is the one join with
   no dependent package; a conflicted assembly composition dispatches a worker whose task is
   the resolution, judged by the assembly's own existing criteria. Same mechanism, no new
   doctrine.
6. **Evidence root cause** (settled): `compose_git_commits` reports `merge.stderr`, but git
   prints conflict details to stdout — hence the empty reason. The conflicted-join event
   must carry git's words: conflicted paths and both parents' refs. The empty-reason bug
   must not survive in either the package or the assembly path.
7. **`/to-graph` stays unchanged** (settled, reversible). With conflict a normal starting
   condition carrying automatic recovery, a plan-time merge-obligation warning buys little
   and risks re-inventing shared-file ordering edges, which doctrine forbids.

## Acceptance criteria

Each is name / input / observation. #4 is the designed-to-fail probe.

1. **The incident replays into a dispatch, not a death** — input: the preserved journal
   (`driver-journal.jsonl.bak-before-ic6-compose`) with frozen graph and conflicting parents
   898eb9b / 3deb441; run `driver-run`. Observation: no terminal composition-failed state;
   the journal records a conflicted join naming `crates/core/src/lib.rs` and both parent
   refs, and IC6's worker dispatches with the conflicted worktree.
2. **Git's own words reach the journal** — input: the same replay. Observation: the recorded
   event's reason is non-empty and names the conflicted path(s); nothing about the conflict
   requires re-running the merge by hand to diagnose.
3. **Parents are re-proved before the dependent is judged** — input: the same replay with the
   worker resolving as the human did (keep both `pub mod` lines). Observation: IC4's and
   IC5's criteria execute against the merged base, recorded in the journal, before IC6's
   outcome is judged.
4. **A resolution that breaks a parent is caught at the join** (designed to fail) — input: a
   resolution that deletes IC4's `pub mod disposition;` line, taking IC5's side wholesale.
   Observation: IC4's criteria fail against the merged base and the failure enters the normal
   recovery ladder naming the broken criterion — it does not surface first at assembly.
5. **A clean join charges nothing** — input: an antichain whose branches touch disjoint
   files. Observation: the base composes silently; no conflicted-join event, no parent
   criteria re-run, no worker involvement in the merge.
6. **Assembly conflict is owned, not terminal** — input: a graph whose final assembly merge
   conflicts. Observation: the conflict evidence is recorded with git's words, a resolution
   worker dispatches, and the assembly's criteria judge the result; re-running `driver-run`
   never reproduces a blocked exit with no continuation.
