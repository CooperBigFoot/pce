# ADR-0022: A package never pays for its judge

## Status

Accepted

## Context

A package enters judging once its own criteria pass, and a dispatched gate decides whether it
completes. Two gate malfunctions were being recorded as `package-failed`: a gate dispatch that
stopped without writing its outcome, and an outcome whose findings the driver could not
structurally use. Both charged the package's two-rung recovery ladder and redispatched the
worker, replacing a criteria-green implementation to cure a defect the implementation did not
have. The same failure on the worker side — a dispatch that stops without its required artifact —
is an uncharged environment failure with its own ceiling. Observed three times; the third parked
gridded-statics WP1 after its implementation had passed all three criteria twice, and the finding
discarded on the way there was substantively real.

The alternatives were to keep charging the package (a gate that cannot speak is treated as
evidence against the work it was judging), or to let a package whose gate never spoke complete on
the strength of its own criteria (shipping work no judge accepted).

## Decision

Gate misbehaviour is a fault of the judge and is never charged to the package's recovery ladder.
Its remedy is a new judgment: the gate re-runs at the same issuance, carrying any structurally
rejected finding forward as a claim to prove or refute, and the worker is not redispatched. Any
rejected finding makes the judgment incomplete, including one whose siblings were usable. Gate
misbehaviour spends its own ceiling, counted by identical reason text; exhausting it puts the
package in a `gate-blocked` terminal state that names the judge and is not completion — a package
whose gate never accepted it does not ship. Because every worktree is detached and the driver
validates findings in the worker's worktree, the binary anchors each finding's witness and repair
commits in the shared ref store before accepting a gate outcome; the witness/repair contract is
unchanged.

## Consequences

A permanently broken gate now stops a run instead of parking an innocent package, which is the
honest failure and requires a human to look at the gate. Gate rounds cost minutes each, and a
re-run after a partially usable outcome judges the already-hardened tree rather than the one the
first gate saw, so a re-run may legitimately report something new. Carrying a rejected finding
forward risks priming the next gate, which is why it is stated as a challenge requiring its own
witness and repair. The reason text counted toward the gate ceiling must stay byte-identical
across occurrences; varying detail belongs in the event's fields. Parks recorded under the old
accounting, including gridded-statics WP1 at plan version 1, stand as correct history.
