# Brief: a registered run is listed forever, and a run that stops without opening a hold is invisible

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-21 against `main` at `740d169`, after
`2026-08-21-what-reaches-the-human` promoted its assembly (PR #187).

This is additive work. No frozen criterion of plan version 1 covers run liveness, and none is edited
by this brief.

## Defect 1 — a run's registration never expires

`/work-graph` writes a registration record at launch (`skills/work-graph/SKILL.md:130`,
`pce hold register`). `RunRegistration` (`crates/core/src/overseer_registration.rs:43`) carries
repository, vision directory, frozen graph, journal path and Herdr session, and nothing about
whether the run is still alive. `QueueView` exposes it as `pub runs: Vec<RunRegistration>`
(`crates/core/src/overseer_view.rs:198`), a flat list of every run ever registered.

`OverseerLiveness` (`:165`) has exactly two variants, `Running` and `NotRunning`, and it describes
**the overseer**, not any run. So the queue distinguishes a dead overseer from a live one, and
cannot distinguish a dead run from a live one. A run that dies stays listed as present forever, and
the operator reading the view sees a fleet that is larger and healthier than it is.

The measured case is already in this corpus: taqsim's worker ran fourteen hours having produced
nothing, and the discriminating evidence — a `tool_execution_start` with no matching end — lay in
the prime daemon's recovery journal, outside every workflow artifact the run could see. The
concurrent palaestra vision is named `silence-means-the-run-has-stalled`. Silence is the failure
mode this fleet actually has.

Give a registration a liveness the view derives rather than asserts. ADR 0012 already ratifies how:
liveness is decided by testing a recorded process number **plus start identity**, never by reading a
child's output, so a reused process number cannot impersonate a dead run. Reuse that reasoning
rather than inventing a second liveness model. Whether the run refreshes a heartbeat, or the view
tests the recorded identity on read, or both, is your decision — record it. Two constraints hold:

- A run that has genuinely finished must be distinguishable from a run that died. Both stop
  refreshing; only one has a terminal journal state.
- Never terminate anything. A check-in reports and closes nothing
  (`CONTEXT.md`, canonical term *Check-in*), and that boundary is ratified.

## Defect 2 — nothing detects a run that stopped without opening a hold

`skills/work-graph/SKILL.md:390` instructs a stopped run to open a hold. It is an instruction in a
document, and this corpus has measured what that is worth: cold-orchestrator rule 3 was present,
correct, and worked around; five of five independent cold planners violated a `summary` constraint
that was written down; the durable fix in each case was to move the constraint to where the actor
actually looks, never to add another sentence. A run that dies before opening its hold, or that
reverts to notifying and stopping, produces exactly the silence this whole vision exists to remove —
and produces it invisibly, because the queue is empty and an empty queue reads as calm.

**The detection is derivable and needs no new instruction.** For every registered run the store
already names its frozen graph and journal. `pce package driver-status --graph <G> --journal <J>`
computes whether that run is blocked, parked, partially complete or finished. `pce hold list`
computes the open holds. A run whose driver-status is terminal-but-not-finished while no open hold
names it is a run that stopped without asking — a fact, computed from two things the store already
holds.

Surface that state in the queue view. It is the direct analogue of the criterion the operator
insisted on for the overseer itself: *silence is not calm*. The same argument applies one level
down, and the view currently makes the claim for the overseer and not for the fleet it watches.

Decide where the computation lives — a new derived field on the view, a distinct verb, or an
addition to `pce hold runs` — and record the choice. It must not require the operator to run
anything by hand, and it must not require the run to cooperate, because a run that has stopped
cooperating is the case being detected.

## What this does not change

- The four attributed doors, the hold lifecycle, routing refusals, or rule admission.
- The check-in boundary: nothing here may terminate a worker, a driver, or a session.
- Any frozen criterion. If satisfying this brief appears to require editing one, stop and report
  which criterion and why.
