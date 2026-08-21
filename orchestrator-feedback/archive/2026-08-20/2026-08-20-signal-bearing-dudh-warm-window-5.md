# PCE workflow feedback: composition worktrees are placed by a relative vision path, re-rooted per repository

- Date: `2026-08-20`
- Orchestrator: `Claude Code / work-graph skill, session b2d7df0c`
- Run: `bluesmith planning/2026-07-29-signal-bearing-dudh-warm-window`, plan version 10
- Outcome: `recovered by the supervisor; defect reproduces deterministically`

## Executive summary

`pce package driver-run` invoked with a **relative** `--graph` / `--journal` path creates each
repository's composition worktree at `<that-repo-root>/<relative-vision-dir>/.pce/compositions/<tmp>`.
For the repository that owns the vision directory this resolves correctly. For every **other**
repository in a multi-repository graph it creates the path inside that repository's working tree,
where the vision directory does not exist. A follow-up `git` invocation then consumes the same
relative string from a different working directory, cannot find it, and the composition fails.

Three consequences, all observed: composition of the second repository fails permanently; the real
merge outcome is never captured because the failure-inspection step dies on the same missing path; and
the failure is not counted against any budget, so the driver loops without bound. A fourth is silent:
the second repository's **source working tree is left dirty** by directories `pce` created inside it.

Relaunching with absolute `--graph` and `--journal` paths fixes all of it. That is the workaround and
also the evidence for the cause.

## Evidence reviewed

- `planning/2026-07-29-signal-bearing-dudh-warm-window/driver-journal.jsonl`, records 666-825
- `git worktree list` in `/Users/nicolaslazaro/Desktop/work/stopwatch`
- `git status --short` in the same repository
- `git merge-tree --write-tree` and a hand-reproduced sequential merge of the same three dependency oids

## Friction and failures

### Composition worktree path is re-rooted per repository

- Severity: `high`
- Phase: `execution`
- Observation: on the first advance to plan version 10 the bluesmith base composed successfully, then
  every stopwatch composition failed with:

  ```
  package composition infrastructure failed in stopwatch: failed to inspect unsuccessful merge of
  package W3 commit ae8a8a5feeaf8930e1c99dbeb076bf41ca6390ff: fatal: cannot change to
  'planning/2026-07-29-signal-bearing-dudh-warm-window/.pce/compositions/26983-759701599de03701-1787215125247504000':
  No such file or directory
  ```

- Evidence: `git worktree list` in the **stopwatch** repository listed a live composition worktree at

  ```
  /Users/nicolaslazaro/Desktop/work/stopwatch/planning/2026-07-29-signal-bearing-dudh-warm-window/.pce/compositions/26983-759701599de03701-1787215154420124000   2803c13 (detached HEAD)
  ```

  The vision directory lives in **bluesmith**; stopwatch has no `planning/` directory of its own. The
  worktree was therefore created, under the stopwatch root, and then looked for elsewhere. The driver
  was launched with `--graph planning/2026-07-29-.../graph.v10.json` — relative.
- Inference: the vision-directory path is stored as given and joined against each repository's root at
  composition time, while the consuming `git -C` runs from the driver's own working directory. Creation
  and consumption disagree about what the relative path is relative to.
- Impact: plan version 10 could not compose at all. 159 journal records and 53 issuances (43 -> 96)
  were burned in roughly ninety seconds before the supervisor killed the driver.

### The merge was not actually unsuccessful

- Severity: `medium`
- Phase: `execution`
- Observation: the reason text asserts an "unsuccessful merge of package W3". No merge conflict exists.
- Evidence: against the authored stopwatch base `2803c132`, `git merge-tree --write-tree` returns a
  clean tree for each of the three dependency oids (`ae8a8a5`, `381a437`, `7c676b7`), and a sequential
  hand merge of all three in a scratch worktree produced no conflicted paths.
- Inference: `git worktree add` failed (or produced a path the caller could not reach), the caller
  classified the absent worktree as a failed merge, and the "inspect the failure" branch then died on
  the same missing path — so the genuine error was overwritten by a misleading one.
- Impact: the reported reason sends the reader to a merge conflict that does not exist. A supervisor
  who trusted it would have gone looking for a graph or dependency problem.

### The failure is not budget-counted

- Severity: `high`
- Phase: `recovery`
- Observation: `worker-environment-failed` for this cause repeated indefinitely at several issuances
  per second and did not consume the environment-failure budget.
- Evidence: issuance advanced 43 -> 96 with no `package-parked` and no budget exhaustion. The
  documented `environment_failures: 6` limit did not engage.
- Inference: composition-infrastructure failures are raised on a path that does not decrement the
  environment budget.
- Impact: an unattended driver would spin until disk or the journal filled. The doctrine that a
  recurring identical blocker parks was not applied here.

### `pce` writes into a source working tree and leaves it dirty

- Severity: `high`
- Phase: `execution`
- Observation: after the failed run, `git status --short` in stopwatch reported `?? planning/`.
- Evidence: `planning/` is gitignored in bluesmith but **not** in stopwatch, so the directories `pce`
  created there are untracked and visible.
- Inference: same root cause; the debris is the created-but-unreachable worktree plus its parents.
- Impact: worse than cosmetic for this repository. Three of stopwatch's own m3 publisher tests refuse
  to run against a dirty tree, so this defect can make a later criterion fail for a reason that has
  nothing to do with the work under test. The source worktree is supposed to be read-only to a run;
  here `pce` mutated it.

## Recommendations

### Resolve the vision directory to an absolute path once, at startup

- Addresses: all four findings
- Change: canonicalise the `--graph` and `--journal` paths when the driver starts and carry the
  absolute vision directory thereafter, so composition scratch space is placed once and never
  re-rooted per repository.
- Location: driver startup / composition workspace allocation
- Trade-off: none apparent; the relative form is only an input convenience.
- Confidence: `high` — relaunching with absolute paths made stopwatch compose on the first attempt and
  left the source tree clean.

### Never place composition scratch space inside a source repository

- Addresses: findings 1 and 4
- Change: allocate composition worktrees under a scratch root that no graph repository owns, the way
  worker worktrees already are (`/tmp/pce-work-package-worktrees/...`). Composition is the one place
  that still writes into the repositories it is composing from.
- Location: composition workspace allocation
- Trade-off: none apparent; workers already prove the pattern works.
- Confidence: `high`

### Do not let the failure-inspection path overwrite the failure

- Addresses: finding 2
- Change: if inspecting a failed merge itself fails, report both — the original error and the
  inspection error — rather than replacing the first with the second.
- Location: composition failure reporting
- Trade-off: slightly longer reason strings.
- Confidence: `high`

### Count composition-infrastructure failures against a budget

- Addresses: finding 3
- Change: charge these to the environment-failure budget, or park on the second identical composition
  failure as the recovery ladder already does for identical blockers.
- Location: recovery ladder / environment budget accounting
- Trade-off: a genuinely transient composition failure would park sooner; parking is the safe
  direction.
- Confidence: `high`

## No-change decisions

The stale `git worktree` registrations accumulated across this vision (51 in stopwatch, 47 in
bluesmith, many pointing at removed composition directories) were an early suspect and are **not** the
cause; `git worktree prune --dry-run` reported zero prunable entries at the moment of failure. They are
untidy but harmless, and a periodic prune is a housekeeping matter rather than a workflow change.

## Suggested follow-up

Confirm whether single-repository visions are affected. The relative path resolves correctly for the
repository that owns the vision directory, so this may have gone unnoticed precisely because
one-repository runs never exercise the re-rooting.
