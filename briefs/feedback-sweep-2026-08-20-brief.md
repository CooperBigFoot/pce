# Brief: seven open defects from seventeen orchestrator reports

Status: READY TO DISPATCH — no grill. Every decision below is evidence-resolvable: examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-20 by the PCE supervisor after processing seventeen feedback reports
from five concurrent runs.

**Fan out, and take your own ground.** Two other agents are already working: one in
`/Users/nicolaslazaro/Desktop/work/pce-integration` (branch `integration/work-package-harness`) on
join attribution, one in `/Users/nicolaslazaro/Desktop/work/pce-recovery` (branch
`integration/recovery-counter`) on the recovery counter. **Do not touch either, or the main
checkout.** Create your own worktree(s) from `main` at `6f56851` —
`git worktree add -b integration/<workstream> ../pce-<workstream> 6f56851` — one per parallel
sub-agent so two of yours never share a tree. The supervisor merges every branch and runs the suite
once. The workstreams below are ordered by severity and are largely disjoint by file; A and E both
touch `graph check`, so give them to the same agent or sequence them.

## Already closed today — do not re-do

`$PCE_WORKTREE_N` unusable from a criterion (fixed, `6f56851`); the herdr `agent start` interface
divergence and the unversioned coupling (fixed, `de04614`, with a `>=0.8.2,<0.9.0` assert); named
herdr sessions and the socket-path bound (`2aa72f6`, `49b7cb3`); `/work-ticket`'s stale handoff
(`8e4e611`). Two more are in flight and out of scope for you: the false `conflicted join broke parent
criteria` attribution, and the plan-scope disagreement between the two recovery counters.

---

## A. Cross-repository dependencies order work but deliver nothing

- Severity: **high**. Three reports converge here from three different runs.
- Evidence: taqsim `2026-08-20-taqsim-modelling-layer-on-incidence.md`. `TQ1` (repository `taqsim`)
  declared `depends_on: [{id: IN1, kind: buildability, reason: "the basin's build path imports
  incidence.compile_model …"}]`. `IN1` completed with hardened oid `3eef8c8`. `TQ1` was dispatched
  into a worktree containing **only** `taqsim` and parked, having probed `../incidence/bindings/python`
  and `https://github.com/CooperBigFoot/incidence.git commit 3eef8c8…` and found neither reachable.
  The journal records `package-base-composed TQ1 / taqsim / dependencies: []` — the edge contributed
  nothing, correctly, because the dependency lives in another repository. Every `IN1` artifact was
  local-only: `git branch -r --contains 3eef8c8` was empty.
  **Cost:** five of six packages blocked on one edge, a human ruling, a push to a public remote, and
  `TQ1`'s one-shot overrule spent covering an orchestration gap rather than a mistaken worker.
- The same class, same-repository variant:
  `…-row-seam-7.md` — GD10 completed proving a repair to `verify_released_wheel_evidence.py`; GD16's
  criteria run that exact file; GD16 declared no edge on GD10, so `merge-base --is-ancestor 43c7a3d6
  bc540df0` was false and GD16 failed on the very `TypeError` GD10 had repaired. `graph check` passed;
  `--strict` produced 30 warnings and none was this one.
- And the ordering variant: `2026-08-19-incidence-python-binding-3.md` — a risk-ordering edge
  sequences a package without supplying what its criterion must read.

**What to change.** Two parts, and the first is the substantive one.

1. **Decide what a cross-repository dependency edge promises.** Today it promises ordering and
   delivers nothing, while a same-repository edge delivers content through base composition. That
   asymmetry is invisible in the graph and fatal at dispatch. Either deliver the dependency's proven
   oid into the dependent's worktree for that repository, or refuse such an edge at authoring time
   with a message that says delivery is not available and names what the author must do instead.
   Recommendation: deliver — a worker that is told "IN1 is your dependency" and handed a tree without
   it cannot succeed, and the honest alternative is to forbid the edge. Whichever you choose, the
   graph must stop implying a guarantee the driver does not provide.
2. **Add the lineage-delivery lint to `pce graph check`**, taking an optional `--journal`: for each
   package `P` and each package `Q` ordered before `P` that the journal records complete, warn when
   `Q`'s proven commit range touches a file that `P`'s criterion commands name and `Q` is not in
   `P`'s transitive `depends_on`. Suppress when `Q` follows `P` in graph order. The reporting
   orchestrator implemented this ad hoc over its own graph: 15 packages, 4 raw flags, 3 discarded as
   backward-in-time, 1 true positive — the one already known. Extract the file set from criterion
   `command` strings, where the paths are already literal.

## B. Worktrees are never reclaimed

- Severity: **high**, and it has already endangered paid work.
- Evidence: `2026-08-19-close-the-seven-basin-coverage-gap-2.md`.
  `/private/tmp/pce-work-package-worktrees` held **200 directories totalling ~270 GB** across five
  visions. Free disk fell from 58 GB to 27 GB over roughly two hours **while a paid campaign was
  running**, and that campaign's third criterion requires every produced output to survive the
  machine that made it — preservation writes several GB locally. 154 directories had been untouched
  for six hours or more, accounting for 222 GB.
- Corroborating, operator-side: herdr workspaces accumulate the same way. The supervisor has closed
  **156 stale work-package workspaces by hand today** in four separate sweeps, and the count is back
  to 53. Cleanup fires only on the normal completion path, so every killed driver, wedged worker,
  spawn failure and park leaks one permanently.

**What to change.** Reclaim an attempt's worktree and its workspace when the attempt reaches a
terminal outcome, including the failure paths — park, environment failure, spawn failure, driver
abort — not only on completion. Add a sweep verb for what has already leaked.

**Decisions delegated:** whether reclamation is immediate or deferred to a retention window (a
supervisor sometimes needs a failed attempt's tree as evidence — `2026-08-20-silence-means-the-run-has-stalled.md`
records pane evidence surviving two parks as a *good* property, so do not destroy evidence to save
disk); how the sweep verb identifies a reclaimable tree without a label convention, given the herdr
label shape changed under `pane run`; and whether reclamation is the driver's job or a separate verb
the skill invokes. Recommendation: reclaim on terminal outcome with a retention window, and a verb
that refuses to touch anything belonging to a live dispatch.

## C. Nothing carries within a package across attempts

- Severity: **high**
- Evidence: `2026-08-20-signal-bearing-dudh-warm-window-8.md`, the per-attempt exit matrix:

  ```
  issuance 105   certify=0   resolve-window=0   bluesmith-tests=101   stopwatch-tests=0
  issuance 106   certify=0   resolve-window=0   bluesmith-tests=101   stopwatch-tests=0
  issuance 107   certify=1   resolve-window=2   bluesmith-tests=0     stopwatch-tests=0
  ```

  Attempt 107 repaired an inherited test-isolation defect — turning both suites green for the first
  time — and simultaneously **lost the two criteria that had passed twice**. The loss was total, not
  a regression: `iteration_benchmark.py: error: argument command: invalid choice: 'resolve-window'`.
  The verb was absent, because attempt 107 began from the composed base and never wrote it.
  **Cost:** three attempts, ~90 minutes, a paid instance idling throughout, to reach a park whose
  cause was the *shape of the package* rather than the difficulty of the work.

**What to change.** The driver has every fact needed to observe "different criteria passed on
different attempts, and no attempt passed all" — which is a strong, cheap signal that a package
should be split. Surface it: when a package parks, report the per-criterion outcome matrix across its
attempts in the park payload, so the diagnosis does not have to be reconstructed by hand from
`criterion-executed` records. Separately, state in the worker brief that attempts do not accumulate
and the composed base is the only inheritance — a worker rationing effort across four deliverables is
behaving rationally on false premises.

**Decisions delegated:** whether anything *should* carry between attempts (recommendation: no — that
is a much larger change and the composed-base model is deliberate); and whether the split signal is
advisory in the park payload or also surfaced by `driver-status`.

## D. A completed proof can be invalidated by state outside git

- Severity: **high**
- Evidence, in-place mutation: `2026-08-20-close-the-seven-basin-coverage-gap.md`. SB7 completed
  against campaign 1's evidence. Campaign 2 wrote to the same `$HFX_CAMPAIGN_EVIDENCE` path,
  overwriting `campaign-record.json` **in place**. SB9's replay of SB7's criteria then failed, and
  issuance 24 parked, with the two `campaign-record.json` files differing at the named reach.
- Evidence, ephemeral resource: `2026-08-20-signal-bearing-dudh-warm-window-7.md`. W7 completed and
  proved nine properties *of a specific EC2 instance*. The instance was later torn down. W7 stayed
  `complete` and carried, while W5 — which needs the host live — could not run. A completed
  provisioning package cannot restore what it delivered.

**What to change.** A package's completion currently asserts something about a tree; both cases show
completions that also depend on state git does not hold. Let a package declare that its completion is
contingent on a named external precondition, and record the identity of external evidence roots
(digest or manifest) at completion so a later divergence is detectable rather than surprising.

**Decisions delegated, and this one deserves care:** the full form — a declared precondition the
driver re-checks before dependents dispatch — is a graph-schema change and may be more than is
warranted. The cheap form is to record identity at completion and report a mismatch when a carried
completion's external root has changed. Recommendation: implement the cheap form, and write up the
full form as a proposal rather than building it. Say which you did.

## E. `graph check --strict` cannot pass a graph with a shared harness

- Severity: **medium**
- Evidence: `…-row-seam-6.md`, corroborated independently by hfx. On one draft, `--strict` refused
  with 30 warnings — 11 `artifact-provenance`, 19 `act-ownership-artifact` — and **none identified a
  defect**. 21 of 24 provenance warnings in hfx's census sat on packages that were *already complete*,
  having passed every criterion on the first attempt while carrying identical warnings. Every
  provenance warning reads "a producer cannot be proven because the graph has no output
  declarations"; the message names its own cause. Ownership fires on every package that references a
  shared harness file, which every repair package must.
- Both runs reached the same verdict independently, and the pourpoint orchestrator then **skimmed a
  true positive** buried among the noise — exactly what its own report predicted would happen.

**What to change.** Add output declarations to the graph schema (`"produces": [...]`), key provenance
off them — warn only when an artifact is neither present at the authored ref nor declared by the
package itself or a strictly upstream one — and make ownership warn when two packages declare they
**produce** the same artifact rather than when they reference it. Deduplicate warnings to one per
distinct (kind, package, predecessor, path); the 30 above represent 2 distinct findings.

**Also in this workstream, and explicitly not mechanical:** `2026-08-20-close-the-seven-basin-coverage-gap.md`
records that nothing verifies a criterion's *command* enforces its *prose* — SB7's check asserted
recorded pairs equal a hardcoded transcript of campaign 1's numbers, which its criterion never asks
for. No lint can close that. Add it to `skills/to-graph/SKILL.md` as authoring guidance with this
worked example, alongside the existing numeric-standard rule: a fixture is a recording, and a check
that pins a recording has quietly replaced the criterion.

## F. Dispatch observability

- Severity: **medium**
- `session_path` is always null: `…-row-seam-10.md`. 41 of 41 records lack the field.
  `observe_prime_session_path` (`src/main.rs:7538-7549`) polls for 500 ms in 25 ms steps and returns
  `None` the moment the descriptor set is empty at expiry, and records only success — so a failure is
  indistinguishable from a run that never looked. This is the field landed specifically so a worker's
  transcript is addressable from the journal; it has never once been populated.
- A completed worker is not harvested while a sibling worker is still in flight:
  `2026-08-19-incidence-python-binding-2.md`. This is the mechanism behind taqsim's fourteen-hour
  stall — IPB8 finished, wrote `{"outcome":"done"}`, exited 0 with its artifact present, and the
  driver never emitted `worker-done` because IPB6 was wedged. One stuck worker holds every finished
  sibling hostage.
- A spawn failure is not distinguishable from a package fault in the journal
  (`2026-08-19-incidence-python-binding-3.md`).

**What to change.** Record the observation's *outcome* rather than only its success, so a null field
carries a reason. Fix or widen the descriptor race — the reporting orchestrator's leading hypothesis
is that the descriptor is not yet written when the poll expires. Harvest a completed worker
independently of its siblings' liveness. Give a spawn failure a distinguishable typed shape.

**Decisions delegated:** whether harvesting becomes per-worker or the wait loop is restructured;
whether the 500 ms budget is extended, retried later, or the observation moved to a point where the
descriptor is guaranteed present. The last is best if it exists.

## G. Composition path hygiene

- Severity: **medium**
- Evidence: `2026-08-20-signal-bearing-dudh-warm-window-5.md` — composition worktrees are placed by a
  relative vision path and **re-rooted per repository**, so a second repository's composition lands
  somewhere unintended; and `pce` writes into a source working tree and leaves it dirty.
- Related and already fixed for a different path: `6f56851` absolutized the *driver materialization*
  root. The composition root (`src/main.rs:3325-3345`, `<vision-dir>/.pce/compositions/…`) was not
  part of that change.

**What to change.** Resolve the vision directory to an absolute path once, at startup, and derive
every path from it — composition roots included. Stop writing into a source working tree; if
composition needs a scratch tree, it belongs under a binary-owned root.

**Decisions delegated:** whether the absolutization belongs at argument parsing (one place, covers
everything) or per-derivation; and what the correct behaviour is when a source working tree is
already dirty — refuse, or proceed and record. The `2026-08-20-silence-means-the-run-has-stalled.md`
report notes cleanliness checks failing "for an unrelated reason" when pce itself dirtied the tree,
so this has already cost a run once.

---

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `6f56851`, pushed and clean.
- **Do not edit the main checkout, `pce-integration`, or `pce-recovery`.** Create worktrees from
  `main` as described at the top. The supervisor merges every branch and runs the suite once.
- Full suite in your worktree: `cargo fmt --check`, `cargo clippy --workspace --all-targets`,
  `cargo test --workspace`. Known parallel-load flakes in `tests/dispatch.rs`
  (`gate_execution_echoes_large_input_without_deadlock`,
  `gate_execution_drains_three_pipes_concurrently`) — verify in isolation before blaming a change.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh`; that symlink
  is a fleet install across five live runs. The supervisor owns installation.
- Installed digest `d4b8a13b…`; installed herdr is 0.8.2 and pce enforces `>=0.8.2,<0.9.0`.
- Every report cited is in `orchestrator-feedback/`. Read the one behind a workstream before starting
  it; the summaries here are compressed and the reports carry the exact oids, paths and counts.

## Tests

Every workstream ships tests, and prefer asserting emitted journal events or CLI output over unit
tests of internal helpers. Two lessons from today's failures are worth honouring: the herdr argv
tests passed while production was completely broken because they asserted only what pce *sends*; and
the worktree-absolutization defect survived because every test launched with absolute paths. Where a
workstream has a "passes for the wrong reason" shape, write the pair — the positive and the case
that would have caught the defect.

## Waiting consumers

Five runs, all live or one ruling from live: **pourpoint** (v19, 14 packages complete, blocked),
**hfx** (v10, blocked pending v11), **bluesmith** (v11, W5 working), **taqsim**
(`2026-08-20-taqsim-modelling-layer-on-incidence`, five of six packages blocked behind the
cross-repository edge in workstream A), and **palaestra**
(`2026-08-20-silence-means-the-run-has-stalled`, six packages, launching).

A healthy first pass, per workstream: A — a cross-repository dependency's proven work is present in
the dependent's tree, or the edge is refused at authoring with a message naming the remedy. B — a
parked attempt's worktree and workspace are gone, and `/private/tmp/pce-work-package-worktrees` stops
growing across a run. C — a park payload carries the per-criterion outcome matrix. D — a carried
completion whose external evidence root changed is reported, not silently replayed. E — a graph with
a shared harness passes `--strict`, and the one true positive is visible. F — `session_path` is
populated, or its absence carries a reason. G — every composed path is absolute and no source working
tree is left dirty.
