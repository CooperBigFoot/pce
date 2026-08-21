# Brief: a gate's repair must travel with the package it hardened

Status: GRILLED 2026-08-17 — ready to dispatch. Decisions and acceptance criteria are in the
"Grilled decisions" and "Acceptance criteria" sections below; doctrine is recorded in
`docs/adr/0018-a-gates-repair-travels-with-the-package-it-hardened.md` and the `Amendment`,
`Hardened lineage`, and `Paired amendment execution` entries of `CONTEXT.md`. Written
2026-08-17 at the end of the second real-world driver run, whose final verdict this defect
decided: `assembly-failed` with every authored criterion green.

## The incident

The 2026-08-10-incidence-core run completed all eight packages across four plan versions. At
assembly, all fourteen authored criteria passed against the composed whole. The only failures
— and the reason the run ends `assembly-failed` instead of finished — were gate-amendment
criteria. **Four**, not three (the pre-grill draft of this brief missed IC3):

| Package | Amendment | Assembly result |
|---|---|---|
| IC1 | `gate:package-gate-1:finding:0` → `canonical_root_tags` | exit 101, `no test target` |
| IC2 | `gate:package-gate-2:finding:0` → `model_artifact_projection_bindings` | exit 101, `no test target` |
| IC3 | `gate:package-gate-3:finding:0` → `numerical_conservation` | exit 101, `no test target` |
| IC7 | `gate:package-gate-17:finding:0` → `linear_reservoir_fixture` | exit 101, `no test target` |
| IC4 | `gate:package-gate-4:finding:0` → `swallowed_allocation_rejection` | exit 0, 1 passed — by accident (below) |
| IC8 | `gate:package-gate-11:finding:0` → `execution_binding_cycles` | exit 0, 1 passed — by accident (below) |
| IC6 | `gate:package-gate-14:finding:0` → `prefix_continuation completion_seal_is_not_a_resumable_prefix` | exit 0, **0 passed; 1 filtered out** |

IC6 is the worst case in the table and it is green. Its command names a test *case*; the case
is absent from lineage, the filter matched nothing, cargo printed `running 0 tests … test
result: ok`, and the driver recorded a passing amendment. A criterion that executed nothing
reported success.

## The mechanism (established, not inferred)

A credited finding is exactly two commits on the gate's own driver-owned branch
`pce/<vision>-gate-N/<pkg>/attempt-M`: a **witness** commit that adds the failing check, then
a **repair** commit that fixes the source. Verified for all seven of this run's findings —
e.g. IC1: `b3da9c9` (the package tip, and the oid IC2 composed against) → `0766e34`
`test(core): witness duplicate canonical root tags` (adds
`crates/core/tests/canonical_root_tags.rs`) → `da39116` `fix(core): assign unique canonical
root tags` (touches `forcing.rs`, `interpolation_table.rs`, `forcing_and_tables.rs`).

Composition reads `pce/<vision>/<pkg>/attempt-N` (`package_branch`, src/main.rs:2425) and
never absorbs the gate branch. So every dependent join and the assembly is built from lineage
without the hardening, and the amendment — carried into `effective_criteria` from the
`finding-replayed` event — runs `cargo test --test <name>` against a tree that lacks the
target.

Three further facts, each verified:

- **Every gate branch is a fast-forward of its package tip.** All seven checked with
  `git merge-base --is-ancestor`. Absorbing the hardening is `git merge` succeeding trivially,
  not a merge to resolve.
- **The amendment never meets lineage before it is carried.** `package-completed` is appended
  immediately after `gate-finished`; the package-time criteria run
  (src/main.rs:4543) happened *before* the gate. So the first tree an amendment is ever
  executed against is a dependent's join or the assembly.
- **IC4 and IC8 are green by accident.** IC6's conflicted-join guard failed on their
  amendments, and IC6's local-patch worker recreated them by hand: lineage's
  `disposition.rs` and `swallowed_allocation_rejection.rs` are byte-identical to the gate's
  repair commit `028dd78`. A worker paid a recovery round to do by hand what `git merge` does.
- **None of the seven fixes is in `incidence` main.** The run proved seven real defects and
  delivered code containing all seven.

So the defect cost the run three times: IC6's local-patch round, the terminal assembly
verdict, and seven unshipped fixes.

## Grilled decisions

1. **The repair enters lineage; gate-authored code ships.** Rejected: re-materializing each
   amendment from its witness/repair refs at every join and assembly — that certifies the tree
   the gate saw rather than the composed whole, making a green amendment vacuous by
   construction. Also rejected: treating a credited finding as new work for the package's
   worker — one worker round per finding, replacing a fix the driver has already watched flip
   red to green with an unproven one. (The human's ruling, 2026-08-17: "merge the gate's
   commits".)
2. **Hardened lineage is an invariant checked before composition, not an action taken at
   credit time.** Before any composition that includes package P, P's branch must contain the
   repair commit of every finding credited to P; the driver merges and records a journal event
   if not. This is idempotent and repairs journals written by the current binary with no
   journal surgery.
3. **An amendment executes as a pair wherever it executes.** Once against the composed tree
   (must pass) and once against that tree with the repair commit's diff reverted (must fail).
   Exit status alone is not a result — IC6 proves it. Applies at joins and at assembly, not
   only at assembly, because a join resolution can delete a guard.
4. **When the revert cannot be constructed, the package that rewrote the hardened source is
   charged — never the human, never a dependent.** That package fails with the finding named,
   and its worker is dispatched to restore the finding's provability, exactly as a conflicted
   join hands resolution to the dependent's worker. Escalation is only by exhausting the
   ordinary recovery ladder, whose question (continue or repartition) is one the human can
   answer; a replay-provability conflict is not. This also disposes of the misattribution in
   the pre-grill draft: the join charged IC6's recovery ladder for a defect in how the driver
   stores amendments.

## Retroactive repair of the evidencing run — pre-verified

The v5 route works and its outcome is already measured. In a scratch worktree from the
assembly base `b28ed9e6`, merging all seven gate tips (`3a278f9 da39116 560c282 d0006fe
028dd78 31dad34 6d9a665`) succeeded with **zero conflicts**; all seven amendment commands then
passed (IC6's now reports `1 passed; 1 filtered out`), and `cargo test --workspace` is fully
green. Evidence tree:
`/private/tmp/claude-501/-Users-nicolaslazaro-Desktop-work-pce/b4026cd1-42b5-44f6-a27e-5b1e6a2c6c23/scratchpad/retro`
(registered as a worktree of `/Users/nicolaslazaro/Desktop/work/incidence`; remove with
`git worktree remove` when no longer needed).

So a v5 freeze with byte-identical criteria carries all eight completions, the fixed binary
repairs lineage on the way into composition, and assembly re-runs green. Journal surgery is
not required and must not be used.

## Acceptance criteria

Each is name / input / observation. #6 and #8 are the designed-to-fail probes. #5 is only
checkable outside the delivering run — it needs the promoted binary against the real journal,
and the human times the install.

1. **The repair reaches lineage** — input: run a package whose gate credits one finding, then
   inspect its attempt branch. Observation: the repair commit is an ancestor of the branch tip
   and the amendment's test file is present in a checkout of that tip.
2. **Composition refuses unhardened input** — input: a package with a credited finding whose
   repair is not an ancestor of its branch, offered to a join. Observation: the driver merges
   the repair and records a merge event before composing; no composition is built from the
   unhardened tip.
3. **Repair is idempotent** — input: run the composition step twice for a package whose repair
   is already an ancestor. Observation: branch tip oid unchanged, no second merge event.
4. **An old journal is repaired without surgery** — input: the unmodified
   `2026-08-10-incidence-core` journal advanced to plan version 5. Observation: each of the
   eight branches contains its credited repair before any composition, and the journal's
   existing 199 events are unaltered.
5. **The dead run completes** — input: run assembly for that v5 freeze. Observation: seven
   amendment criteria execute with exit 0, fourteen authored criteria pass, the journal ends
   `assembly-completed`.
6. **A deleted guard is caught, not passed** (designed to fail) — input: a composed tree with
   the case `completion_seal_is_not_a_resumable_prefix` deleted from
   `crates/core/tests/prefix_continuation.rs`, then run IC6's amendment. Observation: the
   amendment is recorded failed, even though the raw command exits 0 reporting
   `0 passed … ok` — the exact execution this run recorded as a pass.
7. **A guard that guards nothing is caught** — input: an amendment whose test still passes
   when the repair commit's diff is reverted. Observation: recorded failed, with both
   executions in the journal.
8. **An unprovable guard is charged to its author** (designed to fail) — input: a composed
   tree in which a later package rewrote the hardened source so the revert cannot be
   constructed. Observation: that package fails, the failure names the finding to restore, its
   worker is dispatched with that as its task, and no human ruling is requested.

## Open

How often #8 fires, and what the doubled execution costs when a criterion is slow. It would
have fired zero times in this run, so the design is reasoned rather than measured; recorded as
the `Paired amendment cost` ambiguity in `CONTEXT.md`, to settle on the next real run's
numbers.

## Environment facts a dispatched agent will need

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at c355ec0. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration` (integration/work-package-harness), full
  suite there, fast-forward main; `./install.sh` from the MAIN checkout only; the install swaps
  `~/.local/bin/pce` under live runs — the human times it.
- Key code: `package_branch` and `ensure_driver_package_bases` / `compose_git_commits`
  (src/main.rs around 2425–2760) for composition; finding replay and crediting
  (src/main.rs around 4620–4740, `pce gate replay`); package-time criteria execution
  (src/main.rs:4543); the join guard `JoinCriterionExecuted` (src/main.rs:2238, commit
  79a13a7); assembly criterion execution (src/main.rs around 3400–3510); amendment derivation
  in `crates/core/src/package_driver.rs` (`EffectiveCriterion`, `AmendmentRepositoryRefs`,
  `derive_driver_snapshot`, `effective_criteria`).
- The evidencing run: journal at
  `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core/driver-journal.jsonl`
  (199 events; assembly events at the tail), frozen graphs v1–v4 alongside. Repository
  `/Users/nicolaslazaro/Desktop/work/incidence`. Package branches
  `pce/2026-08-10-incidence-core/<pkg>/attempt-N`; gate branches
  `pce/2026-08-10-incidence-core-gate-N/<pkg>/attempt-M`. The seven credited findings, with
  witness → repair: IC1 `0766e34`→`da39116`, IC2 `113c7a4`→`560c282`,
  IC3 `6e08809`→`d0006fe`, IC4 `ce89751`→`028dd78`, IC6 `4693209`→`31dad34`,
  IC7 `7a5e6d0`→`3a278f9`, IC8 `44090d7`→`6d9a665`.
