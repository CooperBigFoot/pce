# Brief: an exhausted recovery ladder can never be recovered, because the remedy it prescribes is filtered out by the counter that enforces it

Status: READY TO DISPATCH — no grill. **A live run is stopped behind this and the documented escape
is inert.** The decisions below are evidence-resolvable; examine the evidence, decide, implement, and
record what you decided and why in your completion report and `CONTEXT.md`. Boundary: if a decision
would change ratified doctrine (an ADR, a frozen criterion), stop and report. Written 2026-08-20.

## The defect

Two counters in the driver disagree about which events count, and the enforcing one ignores plan
versions.

- `active_events` begins at the last `PlanVersionAdvanced`
  (`crates/core/src/package_driver.rs:1067-1069`).
- **Rung selection** counts inside that window: `charged_failure_count(&active_events[..=event_index],
  package)` at `:1543` and `:1747`.
- **The snapshot that reports and gates dispatch** counts over the whole journal:
  `charged_failure_count(events, package.id().as_str())` at `:1809`, feeding
  `recovery_budget(...)`, where `charged > dispatch_budget` yields `Replan`
  (`crates/core/src/package_recovery.rs:155-177`).

So when a package exhausts its ladder, the driver parks it with:

```
recovery-parked  "recovery spending exhausted after 3 attributable failures;
                  re-author as plan version n+1"
```

and performing exactly that remedy changes nothing the enforcing counter can see. **The prescribed
escape is unreachable by construction.**

## The evidence

pourpoint `planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`.
GD4 exhausted its ladder on three identical failures caused by a separate harness defect (since
fixed, `6f56851`). The remedy was performed: plan version 19 was frozen and the driver relaunched.
The journal's last four events are the whole experiment:

```
plan-version-advanced   18 -> 19   carried 14 completions
package-base-composed   GD4 / hfx       5603645f…
package-base-composed   GD4 / pourpoint cb3b8595…
recovery-parked         GD4  "recovery spending exhausted after 3 attributable failures;
                              re-author as plan version n+1"
```

GD4 reached `package-base-composed`, so the advance did move it out of its parked state — then it was
re-parked **before any worker was dispatched**. Counted directly from the journal:

```
GD4 charged failures — full journal: 3 | active window since the v19 advance: 0
driver-status: GD4 {dispatches_remaining: 0, local_patch_remaining: 0, retry_remaining: 0, next_rung: replan}
               GD5 {dispatches_remaining: 2, local_patch_remaining: 1, retry_remaining: 1, next_rung: retry}
```

Zero charged failures in the active window, and still no dispatches remaining. Five packages sit
behind GD4 and the run is stopped.

## What to change

Make the two counters agree, so that a plan-version advance actually resets what it is documented to
reset.

**Decisions delegated to you, and the first one is the whole brief:**

1. **Which scope is correct.** The two candidates are not equivalent and the choice is a doctrine
   statement about what a plan version means.
   - *Active-window scoping* (recommended): a new plan version is a new specification, so recovery
     spending against the old one does not carry. This is what the park message already promises,
     what rung selection already implements, and what every orchestrator has been told. It also
     matches the existing reset of `disputed_parks` and `consumed_overrules` on advance
     (`package_driver.rs:1000-1002`).
   - *Full-journal scoping*: spending is a run-level fact and a plan advance must not launder it.
     Defensible — but then the park message is false and must be rewritten, and there must be some
     other door out of an exhausted ladder, or a package can be permanently undispatchable by design.
   Pick one and make **both** call sites use it. Do not leave two.
2. **If you choose active-window scoping, what stops a bump loop.** A supervisor could mint plan
   versions to refresh a ladder indefinitely. Note that this is already true of every other
   plan-scoped budget, and that a freeze is a human ruling requiring a real change — so the existing
   friction may be sufficient. Say what you concluded rather than adding a mechanism by reflex.
3. **Whether the park message should name the counter's scope.** Whatever you choose, a reader of
   `recovery-parked` should be able to tell what will and will not reset. One clause.
4. **What `driver-status` reports.** It currently renders the full-journal count. Under active-window
   scoping it must render the active-window count, or a supervisor reading status will predict the
   wrong thing — which is exactly how this went undiagnosed through a freeze and a relaunch.

## Tests

The decisive test is the experiment the orchestrator ran by hand: a package charged to exhaustion,
then a `PlanVersionAdvanced`, then a dispatch attempt — the package **must** be dispatchable, and
`driver-status` must report a refreshed ladder. Pair it with the negative: within a single plan
version, exhaustion must still park exactly as today. Also assert directly that rung selection and
the snapshot return the same charged count for the same package and journal — the property whose
absence is this defect.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `6f56851`, pushed and clean.
- **Work in `/Users/nicolaslazaro/Desktop/work/pce-recovery`, branch
  `integration/recovery-counter`** — a dedicated worktree already created for you at `6f56851`.
  Another agent is editing `pce-integration` concurrently on a different defect; do not touch that
  checkout or the main one. The supervisor merges both branches. Full suite in your worktree: `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.
  Known parallel-load flakes in `tests/dispatch.rs`
  (`gate_execution_echoes_large_input_without_deadlock`,
  `gate_execution_drains_three_pipes_concurrently`) — verify in isolation before blaming a change.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh`; that symlink
  is a fleet install across five runs. The supervisor owns installation.
- Installed digest `d4b8a13b…`.
- Key code: `crates/core/src/package_driver.rs:1067-1069` (`active_events`), `:1543` and `:1747`
  (rung selection), `:1809` (snapshot recovery map), `:1000-1002` (what a plan advance already
  resets); `crates/core/src/package_recovery.rs:155-177` (`recovery_budget`).
- Full evidence: `orchestrator-feedback/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam-11.md`.

## The waiting consumer

pourpoint, plan version 19, fourteen packages complete, GD4 undispatchable and GD5, GD6, GD12 and the
assembly behind it. The harness defect that originally exhausted GD4's ladder is already fixed and
installed, so the moment this lands GD4 should dispatch and its three frozen criteria should pass
untouched.

A healthy first pass: relaunch pourpoint on `graph.v19.json` with no new plan version, and GD4
dispatches.
