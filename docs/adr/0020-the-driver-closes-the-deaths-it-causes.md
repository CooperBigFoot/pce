# ADR-0020: The driver closes the deaths it causes

## Status

Accepted

## Context

A work-package dispatch whose whole process tree dies without writing a result file leaves the driver waiting on a file nothing will write. Restarting reproduces the wait; a wait timeout records `driver-stopped-waiting` and the next launch waits again. The two verbs that should have supplied the exit do not: `pce dispatch reconcile` refuses without a recorded process identity, and the driver never records one, because it does not fork its workers — it asks herdr to run them in a pane, so there is no child process number to record. `pce dispatch check-in` appeared to prove the death, but it falls through to `dead` whenever it holds no recorded identity, which is every dispatch the driver has ever issued; it was right about this one by coincidence.

Three directions were credible. Reconcile could learn a mode that accepts check-in's verdict, which would build the closer on an oracle that reports dead unconditionally. The human could remain the closer, which puts a person inside a machinery loop the machine can observe unaided. Or the driver could observe the death itself, using the pane identity it already journals.

## Decision

The driver closes its own dead dispatches. On restart, and on each wait timeout, it re-observes the pane it recorded for every unaccounted dispatch of a running package; where no live worker remains it appends an uncharged environment failure and redispatches. Reconcile keeps its strict process-identity requirement and gains no relaxed mode. A liveness check that holds no evidence reports unknown, never dead. Where the observation is inconclusive the driver closes and redispatches, recording the inconclusiveness. A successor is told that its predecessor was killed and is never handed its branch.

## Consequences

A dispatch death becomes an ordinary restart fold rather than an intervention, and the stopped run resumes without journal surgery. The driver may abandon a live worker it failed to recognise, forfeiting that attempt's work and leaving a stray agent until something reaps it; this is accepted against the alternative of a run parked until a human notices. Because closure now redispatches automatically, every per-attempt path must vary by attempt, and the ceiling on repeated environment failures becomes load-bearing — it counts identical reason texts, so the closure's reason must be a fixed phrase. Correcting check-in from `dead` to `unknown` changes output that other readers may already depend on.
