# PCE workflow feedback: `dispatch-worker-identified` never carries `session_path`

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan versions 1-19
- Outcome: `blocked` — reported as a standing observation, not a blocker for this run

## Executive summary

The `session_path` field was introduced so a worker's Prime transcript would be addressable from the
journal, replacing `grep` over `~/.prime`, which misattributes sessions across visions. **It has never
been populated in this run: 41 of 41 `dispatch-worker-identified` records lack the field**, across two
different dispatch implementations and four `pce` binaries.

The emitter is not at fault. The field is `Option<String>` and the driver writes whatever the identity
supplies; the identity supplies `None` because `observe_prime_session_path` never finds a matching
Prime descriptor within its 500 ms budget.

The practical consequence is that every park in this run — fifteen for GD2 alone — was reconstructed
from the package outcome file and the worktree, because no transcript was addressable. Reports 1 and 8
both record evidence lost this way.

## Evidence reviewed

- the run's journal, all 41 `dispatch-worker-identified` records:
  ```
  dispatch-worker-identified records: 41 | missing session_path: 41
  union of keys: [agent_name, dispatch_sequence, event, issuance, package, pane_id, process, workspace_id]
  ```
  This spans issuances 1-41, the `herdr agent start` dispatch path and its `herdr pane run`
  replacement, and the `pce` upgrade whose notes introduced the field.
- `crates/core/src/package_driver.rs:444` — `session_path: Option<String>`, and `:2336` constructing
  one with `session_path: None`
- `src/main.rs:3839-3853` — the driver reads `session_path` out of the identity and stores it
- `src/main.rs:7499-7536` — `matching_prime_sessions`, which selects descriptors under
  `~/.prime/agent/daemon-workers/*/*.json` whose `/createCommand/config/cwd` canonicalizes to the
  worktree path, and maps them to `/createCommand/sessionPath`
- `src/main.rs:7538-7549` — `observe_prime_session_path`, which polls that for up to 500 ms and
  returns `Some` only on exactly one match
- the live descriptor store on this machine

## What worked

### The pointers in the source are correct

- Evidence: a descriptor read directly from
  `~/.prime/agent/daemon-workers/<daemon>/<worker>.json` has
  `createCommand.config.cwd = /private/tmp/pce-work-package-worktrees/…/00-bluesmith` and
  `createCommand.sessionPath = /Users/nicolaslazaro/.prime/agent/sessions/<uuid>.jsonl`, both
  absolute, exactly the shape `matching_prime_sessions` expects.
- Effect: this rules out a wrong JSON pointer, which was my first hypothesis. Correcting my own
  reading: I initially checked `createCommand.cwd`, found it absent, and briefly believed the
  descriptor shape had drifted. It had not — the value lives at `createCommand.config.cwd`, which is
  precisely where the code looks.

### The mechanism works for other concurrent runs

- Evidence: of four descriptors present on this machine, two point at *other* visions' package
  worktrees (`…/00-taqsim`, `…/00-bluesmith`), both written today at 21:39 with `lifecycle: ready`.
- Effect: descriptors are genuinely written for PCE package workers, so the feature is not inert. It
  also shows descriptors are transient — none survive for this run's completed workers.

## Friction and failures

### 1. The identity observation loses a race it cannot win

- Severity: `medium`
- Phase: `driver execution`
- Observation: 41 of 41 records lack the field, with no successes to compare against.
- Evidence: `observe_prime_session_path` (`src/main.rs:7538-7549`) polls for 500 ms in 25 ms steps and
  returns `None` the moment the descriptor set is empty at expiry. Zero descriptors exist for this
  run's worktrees now, consistent with descriptors being removed when a worker exits.
- Inference, clearly separated from the above: the leading candidate is that the descriptor is not yet
  written when the 500 ms budget expires. The other `None` branches are `[None]` (a descriptor whose
  `sessionPath` is missing or relative — contradicted by the descriptors inspected, all absolute) and
  `[_, _, ..]` (two or more matches — implausible, since each attempt gets a unique worktree path).
  **I could not distinguish these by observation**: doing so requires watching a descriptor appear
  relative to a dispatch on a live driver, and this run's driver is deliberately stopped awaiting a
  fixed binary. I am not relaunching to test it.
- Impact: the stated benefit — stop grepping `~/.prime`, which misattributes sessions across visions —
  is unavailable, and silently so. There is no warning, no `session_path: null` in the record, nothing
  to distinguish "not supplied" from "field not emitted". I spent two exchanges with the human
  establishing which it was.

### 2. A 500 ms budget is spent at the worst moment

- Severity: `low`
- Phase: `driver execution`
- Observation: the poll runs inline at dispatch, immediately after spawn.
- Inference: that is the point at which the descriptor is least likely to exist yet, and the driver
  has nothing else to do with the wait.
- Impact: up to half a second of dead time per dispatch, 41 times in this run, buying nothing.

## Recommendations

### Record the observation's outcome instead of only its success

- Addresses: finding 1
- Change: emit `session_path: null` explicitly, or a sibling field naming why it is absent — no
  descriptor found, multiple matches, or non-absolute path. A consumer can then tell "not observable"
  from "not emitted" without reading the source.
- Location: `src/main.rs:3839-3853` and the `DispatchWorkerIdentified` serializer.
- Trade-off: one more field on a frequent event.
- Confidence: `high`

### Resolve the session lazily rather than at dispatch

- Addresses: findings 1 and 2
- Change: attach the session path when the worker's *first* outcome is recorded — `worker-done` or
  `package-parked` — rather than at spawn. By then the descriptor has certainly been written, and the
  transcript is only ever wanted after something has happened.
- Location: the dispatch identity path, moving the `observe_prime_session_path` call site.
- Trade-off: the field arrives on a later event than the dispatch record, so a consumer correlates by
  issuance rather than reading it off the identity. Every consumer already has the issuance.
- Confidence: `medium` — the timing argument is inference, so this should be treated as the experiment
  that tests it: if lazy resolution populates the field, finding 1's leading candidate is confirmed.

### Match on the worker's own identifier, not its working directory

- Addresses: finding 1
- Change: `matching_prime_sessions` correlates by canonicalized `cwd`. The dispatch already knows the
  agent name (`pce-<hex>`) that appears in the journal. If the descriptor carries that, matching on it
  removes both the canonicalization dependency and the multiple-match branch.
- Location: `src/main.rs:7499-7536`.
- Trade-off: requires the descriptor to carry an identifier PCE controls; may need a Prime-side change.
- Confidence: `experimental` — I have not verified that such an identifier exists in the descriptor.

## No-change decisions

- **`session_path` being `Option<String>`.** Correct. The observation genuinely can fail, and there is
  a regression test named `old_dispatch_worker_identity_without_session_path_deserializes` that keeps
  old journals readable. Neither should change.
- **Polling rather than blocking on a Prime callback.** Reasonable for a best-effort field; the
  problem is when it polls, not that it polls.
- **The descriptor store layout.** `~/.prime/agent/daemon-workers/<daemon>/<worker>.json` is fine and
  the pointers into it are right.

## Suggested follow-up

- **Instrument one dispatch on a live driver.** Record whether the descriptor exists at spawn + 500 ms.
  That single measurement decides between the three `None` branches and confirms or kills the lazy
  resolution recommendation. It needs a running driver, which this run does not currently have.
- **Re-check after the `PCE_WORKTREE_N` fix lands** (report 9). The relaunch that verifies GD4 is a
  free opportunity to check whether `session_path` appears, at no extra cost.
