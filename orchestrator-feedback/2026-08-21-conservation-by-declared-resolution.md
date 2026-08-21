# PCE workflow feedback: 2026-08-21-conservation-by-declared-resolution

- Date: `2026-08-21`
- Orchestrator: `work-graph skill, Claude Code (Opus 5)`
- Run: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-21-conservation-by-declared-resolution`
- Outcome: `aborted; driver journal left underivable`

## Correction notice

An earlier revision of this file inferred that the driver ran `--prepare` before consulting a
terminal-state guard, and recommended enforcing that guard at append time and making derivation
tolerant of post-terminal events. **Both recommendations were wrong and have been replaced.** Records
57 and 58 are true and correctly written; the defect is a missing arm in the replay fold. The corrected
analysis below follows the repository owner's bisection, and the authoritative treatment is
`briefs/environment-preparation-recovery-brief.md`.

## Executive summary

Two findings, one of them severe.

1. **A driver-owned environment-preparation failure is an unrecoverable terminal package state.**
   `environment-preparation-failed` is reached from a `--prepare` command, which is *launch
   configuration supplied by the supervisor*, not package work. Once reached there is no exit:
   `driver-run` re-enters nothing, `criteria-run` refuses, and a plan advance refuses. Recovery
   counters still advertise `next_rung "retry"`, `retry_remaining 1`, `dispatches_remaining 2`,
   which is misleading — none of them is reachable.

2. **The replay fold rejects a legal history.** `EnvironmentPreparationFailed` is set in exactly one
   place and cleared in none, so it is terminal *only by omission*. When the same `uv sync` later
   **succeeded**, the fold arm for `EnvironmentPreparationExecuted` admitted only `Judging` and
   returned `EventAfterTerminal`. `driver-status` now fails against **both** frozen graph versions.
   The records are true and correctly written; the event that records the recovery is
   unrepresentable in the state machine that replays it.

## Evidence reviewed

- `planning/2026-08-21-conservation-by-declared-resolution/driver-journal.jsonl` (59 records)
- `planning/2026-08-21-conservation-by-declared-resolution/supervision.md`
- `planning/2026-08-21-conservation-by-declared-resolution/graph.v1.json`, `graph.v2.json`
- `pce/docs/evidence/wp7-driver.md` lines 60-75
- `pce/src/main.rs` `ensure_recovery_spending_reset_imported` (~5948), `RecoveryResetRecord` (~12155)

## What worked

### Gate finding replay with paired witness/repair proof

- Evidence: journal records 17 and 49. IQ1's finding replayed at incidence witness
  `e235136ffddce4ef33f4630cb97c7dbe8b1aeae4` (exit 101) and repair
  `36a268df4b2c61457d700fa4ca04bae3acd6e6d4` (exit 0); IQ2's at witness
  `6d0f542bf1bd8449bfc9366668ed92beab28f2e8` and repair `0e4283abc3acc1b08d7968b32aaff58568dcf1d4`.
  Record 23 carries `amendment_proof.outcome "reverted"` with the paired execution at exit 101.
- Effect: both amendments are demonstrably non-vacuous. IQ1's gate caught a real hole the authored
  criterion missed — stocks split across compartments whose sum crosses the quantum ceiling were not
  refused, so the extra quantum vanished during totalling. This is the mechanism working exactly as
  intended and it should not be disturbed by any fix below.

### The dirty-source WARN is correctly a warning

- Evidence: driver stderr at 13:06:51Z; taqsim had `M CONTEXT.md` and an untracked ADR outside the
  vision directory.
- Effect: proceeded on committed refs rather than refusing on irrelevant dirt.

## Friction and failures

### `environment-preparation-failed` is terminal with no exit, and is reachable from supervisor configuration

- Severity: `high`
- Phase: `execution / recovery`
- Observation: TQ1's `uv sync` prepare exited 1. The package folded to
  `environment-preparation-failed` and `driver-status` reported `outcome "blocked"`, `ready []`.
  The underlying cause was then genuinely remedied (the missing git object was published, and the
  driver's own record 57 shows the identical `uv sync` subsequently `succeeded`). No mechanism
  allowed the run to continue.
- Evidence:
  - Relaunch with identical argv at 13:51:44Z: pane dead in under 8s, status 0, **zero** new journal
    records, byte-identical blocked status.
  - `pce package criteria-run --package TQ1 …` →
    `Error: failed to derive effective criteria / Caused by: driver event occurs after package TQ1 reached a terminal state`.
  - Relaunch against the frozen successor plan → record 59
    `driver-aborted: failed to derive predecessor plan before advancing: driver event occurs after package TQ1 reached a terminal state`.
  - `docs/evidence/wp7-driver.md:71`: "A nonzero status folds to the distinct terminal state
    `environment-preparation-failed`". Terminal is the documented intent; the missing piece is the exit.
- Inference: the state is modelled as terminal for the *package*, but its cause lives in the
  *driver's* materialization and in supervisor-supplied `--prepare` configuration. Those are exactly
  the failures most likely to be transient or externally remediable, so they are the worst candidates
  for an irreversible terminal state. `--recovery-reset` appears aimed at exhausted recovery spending
  (`src/main.rs:2552`), not at this state, and its own validation runs `derive_driver_snapshot` over
  a candidate that would still contain the post-terminal events, so it looks likely to hit the same
  refusal. Not tested, because the record requires an attributed human `reset_by` that the supervisor
  must not author.
- Impact: a run with two fully proven packages, both carrying gate-earned amendments, cannot advance
  and cannot be read. Four packages never started.

### The state machine cannot represent recovery from a failed preparation

- Severity: `high`
- Phase: `execution / replay`
- Observation: relaunching against `graph.v2.json` re-ran TQ1's preparation. It **succeeded** and was
  journalled as records 57 and 58. The driver then aborted deriving the predecessor plan, and
  `driver-status` has failed against every graph version since.
- Evidence:
  - Bisected by the repository owner: `head -56` derives, `head -57` does not. Record 39 is `uv sync`
    for TQ1 with `outcome=failed`, exit 1, which set
    `EnvironmentPreparationFailed` at `crates/core/src/package_driver.rs:1875`. Record 57 is that same
    command succeeding after the missing git object was published.
  - `crates/core/src/package_driver.rs:1860`, the `EnvironmentPreparationExecuted` arm, opens
    `if !matches!(state, DriverPackageState::Judging { .. }) { return Err(EventAfterTerminal) }`.
  - `grep EnvironmentPreparationFailed` over the crate and binary: variant declaration (`:842`), one
    classification arm (`:1343`), the single assignment (`:1875`), a render arm, and a test. Nothing
    transitions out of it.
- Inference: the write path accepted a transition the read path denies. `EnvironmentPreparationFailed`
  is terminal by omission rather than by decision, and the fold's transition table is simply
  incomplete: a *succeeded* preparation while in that state has no arm.
- Impact: the append-only journal is the admissible run proof, and the supervisor may not edit it. A
  run whose blocking cause has been genuinely remedied is left permanently unreadable — strictly worse
  than remaining blocked.

## Recommendations

### Add the missing fold arm: a succeeded preparation returns the package to `Judging`

- Addresses: finding 2.
- Change: in the `EnvironmentPreparationExecuted` arm, before the `Judging`-only guard, admit the case
  where the outcome succeeded and the package is in `EnvironmentPreparationFailed`, and return it to
  `Judging { issuance }` — the state it left. One arm.
- Location: `crates/core/src/package_driver.rs:1860`.
- Trade-off: none identified; it makes representable a transition that already occurs in reality.
- Confidence: `high`

### Make the error name the offending record and state

- Addresses: finding 2, diagnosis cost.
- Change: `EventAfterTerminal` reports only the package. Finding record 57 required hand-bisecting the
  journal with `head -n`. Carry the record index, the event kind, and the state that rejected it.
- Location: `crates/core/src/package_driver.rs`, `PackageDriverError::EventAfterTerminal`.
- Trade-off: a wider error type.
- Confidence: `high`

### Give `environment-preparation-failed` a first-class exit

- Addresses: finding 1.
- Change: decide deliberately whether this state should be terminal at all, rather than leaving it
  terminal by omission. With the fold arm above in place a successful re-preparation recovers on its
  own, which may be the whole fix. Separately, stop advertising `next_rung "retry"` /
  `retry_remaining 1` / `dispatches_remaining 2` for a package that cannot reach any of them.
- Location: `crates/core/src/package_driver.rs`, package state model; status rendering.
- Trade-off: none for the status correction.
- Confidence: `high` that the advertised recovery counters are misleading; the terminality question is
  delegated in `briefs/environment-preparation-recovery-brief.md`.
- Correction: an earlier revision of this file proposed `--recovery-reset` as the exit. That is wrong.
  TQ1 is not recovery-parked, recovery spending is not exhausted, and the record's validation folds
  these same records.

### Reconsider whether a prepare failure should be package-scoped at all

- Addresses: finding 1.
- Change: `--prepare` is launch configuration for a driver-owned materialization. A failure in it is
  arguably a *driver* fault, not a package fault, and could block the run without staining the
  package's state at all.
- Location: `src/main.rs`, preparation outcome handling.
- Trade-off: a genuinely package-specific preparation need would no longer be attributed to it.
- Confidence: `experimental`

## No-change decisions

- The dirty-source WARN behaves correctly and needs no change.
- Multi-repository composition of local oids is not itself a defect. That taqsim resolves incidence
  over GitHub while PCE composes incidence locally is a property of the *repositories*, not of pce,
  and belongs in the taqsim repository contract rather than here.

## Suggested follow-up

- A separate vision for the repository-contract question: how a taqsim package proves against an
  incidence commit that the same run produces and has not published. Candidate directions include a
  path/workspace uv source, or publishing composed dependency oids as part of dispatch. Note that
  the driver's criteria materialization names checkouts by index (`00-repository`, `01-repository`)
  while worker worktrees name them by repository (`00-taqsim`, `01-incidence`), so no single
  committed relative path is correct in both layouts; any path-source direction must fix that first.

---

## Addendum 2026-08-21T21:07Z — the hold store lost every registration and open hold

### Finding: an open hold and its run registration disappeared from the default store

- Severity: `high`
- Phase: `recovery` / hold-store durability
- Observation: at `20:20Z` this run was registered and a terminal-stop hold was opened in the default
  store, returning key `fec9050d174916fd09ef4f690fc645e2978d6592910fa170e0c49a372610ba23`, state
  `open`, route `overseer`. At `21:02–21:07Z`, with no `--root` flag and from the same cwd,
  `pce hold runs` → `[]`, `pce hold list` → `[]`, and
  `pce hold read --key fec9050d...` → `Error: failed to read hold / hold ... does not exist`.
- Evidence:
  - `~/.pce` and `~/.pce/holds` both created/mtime `2026-08-21T21:02:22Z`.
  - `~/.pce/holds/overseer/events.jsonl` starts at `21:02:22.400Z` with `heartbeat`, then
    `{"kind":"wake-started","reason":"startup"}`, then `session-spawned`; its first
    `session-exited` detail reads `Registered runs: 0 · Open holds: 0 · Routed to human: 0`.
  - `pgrep -fl "overseer serve"` → `57005 pce overseer serve`, live and heartbeating.
  - `env | grep -iE 'pce|hold'` → empty at both invocations, so no root was redirected by environment.
  - After this run re-registered, `pce hold runs` returned **four** registrations (`taqsim`, `hfx`,
    `palaestra`, `pourpoint`) — other supervisors were concurrently re-registering into the same fresh
    store, so the emptiness was global, not a filter on this run.
- Inference (stated as inference, not observation): `pce overseer serve` recreates or clears its store
  root at startup, discarding registrations and holds written before it started. A competing reading —
  that the `20:20Z` writes landed in a different root — is not excluded by these observations, but no
  root-selecting flag or environment variable differed between the two invocations. This supervisor
  did not read the binary's source and did not deliberately reproduce the wipe.
- Impact: the durable record that section 7 exists to create is not durable. A supervisor that had not
  re-checked the store would have reported a stop as recorded when it was not, and the overseer's
  first pass reported a truthful-looking `Open holds: 0` for a fleet that had open holds. Every
  affected run silently drops off the overseer's queue.

### Recommendation: never destructively initialise the store root

- Addresses: the finding above.
- Change: make store-root initialisation strictly create-if-absent. If `overseer serve` needs a clean
  session area, scope that to `holds/sessions/` and `holds/overseer/`, never to the registration and
  hold records. If a schema migration genuinely requires a reset, refuse and print the incompatibility
  rather than silently emptying the store.
- Confidence: `high` for the invariant; `medium` for the located cause, which is inference.

### Recommendation: make store emptiness distinguishable from store loss

- Addresses: the same finding.
- Change: persist a store creation stamp and a monotonic record counter, and have `hold runs` /
  `hold list` report it. A supervisor could then observe "store created after my last write" and say
  so, instead of inferring it from directory mtimes as this one did.
- Confidence: `medium`

### What worked: the stop identity was self-healing

- Evidence: `<STOP_ID>` was recomputed from first principles (length-delimited SHA-256 over vision
  directory, journal path, and establishing sequence `132`) and reproduced
  `2ff4fff1...c6a5` byte-identically; re-opening returned the identical hold key `fec9050d...ba23`.
- Effect: the store loss cost a re-derivation, not a duplicate question to the human. The section-7
  rule that the identity must hash the immutable establishing event and never the report is what made
  recovery mechanical. This is worth keeping exactly as specified.
