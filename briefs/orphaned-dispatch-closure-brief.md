# Brief: a dead unaccounted dispatch has no closure path, and the driver waits on it forever

Status: DRAFT — to be grilled before dispatch. Written 2026-08-18 during the third real-world
driver run (vision 2026-08-09-gridded-statics-self-name-to-every-consumer, palaestra), which
is stopped on exactly this.

## The incident

WP1's dispatch sequence 7 (journal issuance 4) lost its entire process tree — worker,
package-agent, dispatch-worker — without writing a completion record or a result file. (Root
cause of the death is separate: a supervisor killed the driver while the dispatch ran, and
the recording chain died with it; see "adjacent findings".) The aftermath is the defect:

1. `pce dispatch check-in` correctly reports the dispatch:
   `{"issuance_sequence":7,"state":"dead","completion":"unaccounted"}`. The machine can prove
   the death.
2. `pce dispatch reconcile --issuance 7 --node WP1` refuses:
   `dispatch process identity sidecar is absent`. Driver-composed dispatches never write the
   identity sidecar that reconcile requires — `.pce/package-dispatch.jsonl.dispatches/` does
   not exist for this run at all. The closure verb is unusable for every dispatch the driver
   has ever issued.
3. The driver, restarted, folds WP1 as Running with an unaccounted ledger entry and blocks in
   `wait_for_driver_results` on a result file nothing will ever write. With
   `--wait-timeout-ms` it appends `driver-stopped-waiting` and exits — and the next launch
   does exactly the same. A loop of recorded waiting, never a closure.

So: proof of death exists (check-in), the closer demands evidence the driver never produced
(sidecar), and the scheduler waits forever on testimony from a corpse. Three verbs, no path.

This is the same shape as the pending Program #37 finding on /land-ticket (journal-only proof
excludes true facts) and the replan dead-end fixed on 2026-08-17: a fact the machine can
verify has no doorway into the record that schedules work.

## What a fix must answer (the grill questions)

- **Who closes a dead unaccounted dispatch?** Candidates: (a) the driver itself — on restart,
  when a Running package's unaccounted dispatch is check-in-dead, append the
  worker-environment-failed event (uncharged) and redispatch, making death-observation part
  of the ordinary restart fold; (b) reconcile learns a sidecar-less mode that accepts
  check-in's death evidence; (c) driver dispatches start writing the identity sidecar so
  reconcile works as designed. (a) and (c) are compatible and may both be right: (c) restores
  reconcile's integrity, (a) removes the human from a machinery loop entirely.
- **What stops a live-but-slow worker from being falsely closed?** check-in's death evidence
  must be strong enough that PID reuse or a paused process never reads as dead. Whatever
  check-in uses today to say "dead", state it and test it adversarially — a false
  "dead" here kills a working dispatch and forfeits its work.
- **Is `driver-stopped-waiting` ever consumed?** Today it is recorded and never read. If the
  driver gains the closure path, does stopped-waiting become the trigger ("waited once, then
  verify liveness"), or is it purely evidentiary?
- **Retroactive repair of this run:** with the fix installed, a driver restart should close
  seq 7 and redispatch WP1 with no journal surgery. State that explicitly as the acceptance
  path — this stopped run is the regression fixture.

## Adjacent findings from the same night — NOT in this brief, ledger only

- Worker agents go inert: three WP1 attempts stalled (one kernel spinning at 100% CPU with no
  open files; two agents at 0% CPU, no children, no connections), the latter two at ~56
  minutes of lifetime. Suspected ~1-hour limit or defect in the prime-agent path. Separate
  investigation; this brief only ensures such deaths are closable.
- Killing the driver while a dispatch is in flight can orphan the dispatch's recording chain
  (that is how seq 7 lost its record). A supervisor lesson, and an argument for (a): the
  driver must tolerate recording-chain loss it can itself cause.
- ~90 prime-agent processes orphaned to PID 1, up to 5 days old, idle — machine-wide agent
  leak, same family as the fixed test-shim orphans.

## Environment facts a dispatched agent will need

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at d929729 (or later). Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration` (integration/work-package-harness), full
  suite there, fast-forward main; `./install.sh` from the MAIN checkout only. The install
  swaps `~/.local/bin/pce` under live runs; the human times it. The stopped gridded-statics
  run must NOT be redriven until the fix is installed.
- Key code: restart handling `src/main.rs:2984-3098` (`running_results`, `wait_for_driver_results`,
  DriverStoppedWaiting); dispatch ledger fold and `unaccounted()`; `pce dispatch check-in`
  implementation (its "dead" determination is the load-bearing evidence); `pce dispatch
  reconcile` refusal at the sidecar check; sidecar write path `persist_dispatch_process_identity`
  (src/main.rs ~13522, per CONTEXT.md "Dispatch process identity") — apparently not invoked on
  the driver's composed-dispatch path.
- The stopped run's artifacts: vision dir
  `/Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-09-gridded-statics-self-name-to-every-consumer/`
  — driver-journal.jsonl (last event `driver-stopped-waiting` issuance 4), dispatch ledger
  `.pce/package-dispatch.jsonl` (seq 7 dead-unaccounted per check-in), supervision.md with the
  full intervention narrative, committed WP1 work at branch `pce/<vision>/WP1/attempt-4`
  (2770e36) reusable as a recovery seed.
