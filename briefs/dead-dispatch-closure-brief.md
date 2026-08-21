# Brief: a dead unaccounted dispatch has no closure path, and the driver waits on it forever

Status: GRILLED 2026-08-18 — ready to dispatch. Decisions and acceptance criteria are in the
"Grilled decisions" and "Acceptance criteria" sections below; doctrine is recorded in
`docs/adr/0020-the-driver-closes-the-deaths-it-causes.md` and the `Dispatch pane identity`,
`Environment closure`, `Unknown liveness` entries of `CONTEXT.md`. Written during the third
real-world driver run (vision `2026-08-09-gridded-statics-self-name-to-every-consumer`,
palaestra), which is stopped on exactly this.

## The incident that exposed this

WP1's dispatch sequence 7 (journal issuance 4) lost its entire process tree — worker,
package-agent, dispatch-worker — without writing a completion record or a result file. The
root cause of the death is separate and now known (corrected 2026-08-18 by the grill behind
`prime-agent-dispatch-durability-brief.md`; the idle-eviction theory recorded here earlier was
disproven — that sweep never ran): a shutdown signal reached the dispatch client, whose own
handler ordered the daemon to destroy the whole agent worker mid-prompt, and the waiting client
was never told, so it sat against a 24-hour request timeout. That is its own brief against its
own repository. This
brief is about the aftermath, which is that a recoverable death became permanent.

## The defect

1. `pce dispatch check-in` reports the dispatch as
   `{"issuance_sequence":7,"state":"dead","completion":"unaccounted"}` — but this is not
   evidence. `classify_dispatch_check_in` (`crates/core/src/dispatch_check_in.rs:203-215`)
   falls through to `Dead` whenever it holds no recorded process identity. The driver never
   records one, so **every dispatch the driver has ever issued reads as dead, alive or not**.
   It is right about seq 7 by coincidence. This is precisely the vacuity failure the existing
   `Dispatch liveness and vacuity` relationship in `CONTEXT.md` warns about, realized.
2. `pce dispatch reconcile --issuance 7 --node WP1` refuses with `dispatch process identity
   sidecar is absent` (`src/main.rs:8252`). This refusal is **correct and must stay correct**.
   The sidecar is written only on the directly-spawned-child path
   (`persist_dispatch_process_identity`, `src/main.rs:13505`, called from the spawn at
   `src/main.rs:13888`). The driver does not fork its workers: `issue_package_dispatch`
   (`src/main.rs:5387`) asks herdr to open a pane and `herdr agent start` runs the worker
   there. There is no child process number to record, so
   `.pce/package-dispatch.jsonl.dispatches/` does not exist for this run at all.
3. The driver, restarted, folds WP1 as Running with an unaccounted ledger entry
   (`src/main.rs:3957-3974`) and blocks in `wait_for_driver_results` (`src/main.rs:3143`) on a
   result file nothing will ever write. With `--wait-timeout-ms` it appends
   `driver-stopped-waiting` and exits; the next launch does the same. A loop of recorded
   waiting, never a closure.

So: no real proof of death, a closer that demands evidence this topology cannot produce, and a
scheduler waiting on testimony from a corpse. Three verbs, no path. The only exit today is
journal surgery by hand.

## What the driver already has and does not use

The driver journals the pane it opened for every dispatch: `{"event":"dispatch-pane-opened",
"package":"WP1","issuance":4,"pane_id":"wHA:p1","workspace_id":"wHA"}`. That pane is a live,
queryable handle. Asked about it on 2026-08-18, with seq 7 long dead:

```
$ herdr pane get wHA:p1
{"result":{"pane":{"agent_status":"unknown","cwd":"/private/tmp/pce-work-package-worktrees/
pce-8c778f6253d21e6f33a25b7329c2/00-orthographos","pane_id":"wHA:p1", ...}}}

$ herdr pane process-info --pane wHA:p1
{"result":{"process_info":{"foreground_processes":[{"argv":["-zsh"],"name":"zsh","pid":69469}],
"shell_pid":69469}}}
```

The pane still exists; nothing but a login shell is in it; no agent is registered against it
(`herdr agent list` does not include it). That is unambiguous death evidence of the kind this
topology can actually produce.

A second handle exists and is stronger: `derive_agent_name(vision, package_id, attempt)`
(`crates/core/src/herdr_dispatch.rs:529`) is deterministic and unique per attempt, and
`herdr agent get <name>` targets by unique agent name. The response of
`execute_herdr_agent_start` (`src/main.rs:5276`) is currently returned in the dispatch JSON as
`agent_start` and then discarded; it carries the identity of the pane/terminal the worker
actually runs in, which may or may not be the same pane as the worktree root pane recorded in
`dispatch-pane-opened`. **The implementer must determine which and record the identity of the
pane the worker actually occupies**, not merely the worktree root pane, and must not assume
they coincide because they happened to in this incident.

## Grilled decisions (2026-08-18, ratified by the human)

1. **The driver closes its own dead dispatches; reconcile gains no relaxed mode.** On restart,
   and on each wait timeout, the driver re-observes the recorded identity for every unaccounted
   dispatch of a Running package. Where no live worker remains it appends an uncharged
   `worker-environment-failed` and redispatches, making death-observation part of the ordinary
   restart fold. Teaching `reconcile` to accept check-in's verdict was rejected: it would build
   the closer on an oracle that reports dead unconditionally. Journal surgery is never a door.
2. **The driver records a dispatch identity appropriate to its topology.** Pane, workspace, the
   deterministic agent name, and the process observed in that pane at spawn — the driver-path
   counterpart of the process-identity sidecar. It is not a PID sidecar and must not be forced
   into that shape.
3. **Missing evidence is `unknown`, never `dead`.** `classify_dispatch_check_in` gains an
   unknown liveness classification for an observation it holds no record for. Nothing may close
   on `unknown` alone; only the driver, holding its own pane identity, may close.
4. **Inconclusive reads close and say so** (ratified). Where the driver cannot get a clear read
   — herdr unreachable, a pane holding something unidentifiable, herdr restarted and no longer
   knowing the agent — it closes and redispatches, and the recorded reason names the evidence
   as inconclusive. Rationale: the cost of being wrong is bounded to one abandoned worker's
   forfeited work plus a stray process, while the cost of holding is a run parked until a human
   notices, which is the failure the machine exists to remove. Recorded as an open ambiguity
   (`Inconclusive liveness` in `CONTEXT.md`) to revisit once a real run measures how often an
   inconclusive read was a live worker.
5. **The closure's reason text is a fixed phrase.** The existing ceiling that stops endless
   redispatch counts *identical consecutive* environment failures for a package
   (`crates/core/src/package_driver.rs:1403-1441`). A reason carrying the issuance, a
   timestamp, or a pane id resets the count every time and silently converts that ceiling into
   an unbounded loop. Varying detail goes in the event's own fields, never in the counted text.
   Two fixed phrases are needed and must be distinguishable, one for an observed death and one
   for an inconclusive read; each is separately counted.
6. **The successor is told, and not seeded.** The redispatch starts from the package's clean
   composed base (`driver_package_base_refs`, `src/main.rs:2536`) exactly as today; the dead
   attempt's branch is never handed over. The composed brief gains one sentence stating that an
   earlier attempt was killed by the environment before finishing, that nothing it produced was
   judged, and that durable side effects it may have left outside the repository are to be
   verified against this package's own criteria rather than read as corruption or redone.
   Rationale: work inside the worktree dies with the attempt and starting clean is free, while
   work outside it outlives the worker and the successor will meet it regardless. WP1's own
   second criterion ("an attempt that finds a baseline already sealed by an earlier attempt has
   found its own work done — verify it, treat that as success, never re-seal") is the case, and
   it is protected today only because that author happened to think of it.
7. **Every per-attempt path varies by attempt.** `package_temporary_directory`
   (`src/main.rs:5373`) digests only the vision and the package id, so every attempt of a
   package shares one `/tmp/pce-tmp/<hash>` scratch directory. The worktree root already varies
   by attempt through the derived agent name; the temporary directory must too. This is in
   scope because automatic redispatch is what makes the collision reachable: a replacement
   would write into the scratch directory its abandoned predecessor still holds.
8. **`driver-stopped-waiting` acquires a meaning.** Today it is recorded and never read. Under
   this fix a wait timeout triggers a liveness observation: dead or inconclusive closes and
   redispatches without appending it; alive appends it and exits as today. It becomes the record
   of "verified alive, gave up waiting this launch" rather than of "gave up".

## Acceptance criteria

Each is name / input / observation. #2 is the designed-to-fail probe. #4 is the only one not
checkable inside the run that delivers it.

1. **No evidence is not death** — input: run `pce dispatch check-in` against a log whose
   issuance has no recorded identity, while that dispatch's worker is demonstrably still
   working. Observation: the entry reports unknown liveness, not `dead`, and the dispatch is
   absent from anything the driver would close.
2. **A live worker survives the restart fold** (designed to fail the happy path) — input:
   dispatch a package, let the worker reach working state, kill the driver process only, then
   relaunch the driver. Observation: the worker is still running afterwards, no
   `worker-environment-failed` was appended for it, and the driver returns to waiting.
3. **Inconclusive still closes, and admits it** — input: point the driver at an unreachable
   herdr socket with one unaccounted dispatch outstanding. Observation: it appends an
   environment failure whose fixed reason text names the evidence as inconclusive, and issues a
   fresh dispatch.
4. **The stopped run repairs itself** — input: the gridded-statics vision directory exactly as
   it stands, driver relaunched after `./install.sh`. Observation: issuance 7 gains a completion
   record and WP1 is dispatched afresh from its composed base, with no file edited by hand.
   (Checkable only against the live palaestra directory, outside the delivering run; the human
   times the install.)
5. **The death ceiling still stops** — input: a package whose worker dies immediately on every
   dispatch, left to run. Observation: after the configured number of identical failures the
   package is environment-blocked instead of dispatched again.
6. **Attempts never share scratch** — input: two attempts of the same package in the same
   vision. Observation: their dispatch temporary directories are different paths.
7. **Told, not seeded** — input: the brief composed for a redispatch that follows an environment
   closure. Observation: it carries the killed-predecessor sentence, and the predecessor's
   branch OID appears nowhere in the brief or in the dispatch's base refs.

## Adjacent findings from the same night — NOT in this brief

- **prime-agent, two fixes, own brief against `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`:**
  a session with an in-flight prompt must never read as idle to the eviction sweep; and worker
  death must fail the waiting client immediately instead of leaving it against a 24-hour
  timeout. That is the actual cause; this brief is what makes the cause survivable.
- Killing the driver while a dispatch is in flight orphans the dispatch's recording chain. A
  supervisor lesson, and an argument for decision 1: the driver must tolerate recording-chain
  loss it can itself cause.
- ~90 prime-agent processes orphaned to PID 1, up to 5 days old, idle — machine-wide agent leak,
  same family as the fixed test-shim orphans. Needs reaping; out of scope here.

## Environment facts a dispatched agent will need

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at `d929729` or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration` (branch `integration/work-package-harness`),
  full suite there, then fast-forward main. `./install.sh` from the MAIN checkout only. The
  install swaps `~/.local/bin/pce` under live runs; the human times it. The stopped
  gridded-statics run must NOT be redriven until the fix is installed.
- Workspace layout and doctrine: `AGENTS.md`. `src/` is the composition root only; all domain
  logic lives in `crates/*`. Clippy denies `.unwrap()`, `.expect()`, and `println!`/`eprintln!`
  in library crates. `cargo fmt`, `cargo clippy --workspace --all-targets`,
  `cargo test --workspace`.
- Key code, verified at `d929729`: restart fold and the wait, `src/main.rs:3957-4030` and
  `wait_for_driver_results` at `src/main.rs:3143`; the driver's herdr dispatch,
  `issue_package_dispatch` at `src/main.rs:5387` with `execute_herdr_agent_start` at
  `src/main.rs:5276`; check-in at `src/main.rs:10233` classifying via
  `crates/core/src/dispatch_check_in.rs:150-225`; reconcile's refusal at `src/main.rs:8238-8252`;
  the sidecar writer `persist_dispatch_process_identity` at `src/main.rs:13505`, called only
  from the direct-spawn path at `src/main.rs:13888`; the environment-failure ceiling at
  `crates/core/src/package_driver.rs:1396-1441`; base refs at `src/main.rs:2536`; the temporary
  directory digest at `src/main.rs:5373`; the agent name derivation at
  `crates/core/src/herdr_dispatch.rs:529`.
- herdr verbs available for liveness: `herdr pane get <pane_id>`,
  `herdr pane process-info --pane <pane_id>`, `herdr agent get <target>`, `herdr agent list`.
  All return JSON on stdout. `issue_package_dispatch` refuses unless `HERDR_ENV=1`.
- The stopped run's artifacts: vision dir
  `/Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-09-gridded-statics-self-name-to-every-consumer/`
  — `driver-journal.jsonl` (last event `driver-stopped-waiting`, issuance 4), dispatch ledger
  `.pce/package-dispatch.jsonl` (seq 7 dispatch with no completion; seqs 1, 3, 5 closed),
  `.pce/package-results/WP1/` holding `1.json`, `3.json`, `5.json` and no `7.json`,
  `supervision.md` with the full intervention narrative. Committed WP1 work sits at branch
  `pce/<vision>/WP1/attempt-4` (`2770e36`); under decision 6 it is **not** a recovery seed and
  must not be handed to the successor. This stopped run is the regression fixture for
  criterion 4.
