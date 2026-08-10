# ADR-0012: Dispatch liveness is observed, never enforced by termination

## Status

Accepted

## Context

ADR-0007 made a dispatch an object the binary spawns and measures: issuance is appended before the
spawn, exactly one completion after it, and the child's transcript is never read as a fallback. The
binary waits for its child, so the dispatch's life is bounded by the life of the command the
orchestrator used to start it. That command is a foreground harness call with a ten-minute ceiling.

A `pr-reviewer` child exceeded that ceiling and was killed after issuance had been appended. The
round was spent with nothing to show, the step reached its three-round cap on the following
dispatch, and had that dispatch returned `REVISE` the step would have escalated for a reason
unrelated to its code. Long gate children are normal rather than exceptional, and a falsification
gate that executes stimuli is expected to be slow.

While a dispatch runs, the run cannot distinguish a working child from a wedged or dead one.
Earlier evidence measures what happens when the run tries: ninety-eight detached dispatches with no
completion signal and a hand-written poller per batch, two of three workers finished and idle when
the human asked how the orchestrator knew, a zero-byte log unnoticed for the remainder of a run,
four dispatches writing their terminal marker having produced no artifact, and a liveness check
that pattern-matched a process name and matched the prompt text instead of the process. Every one
of these reads a child's output and infers a state from it. ADR-0002's placement rule already
correlates a completion to its issuance, so an unpaired issuance is derivable — but only after the
fact and only to a reader.

The credible alternatives were: a hand-written poller per dispatch, which is the measured failure
and puts the obligation back on orchestrator memory; a kill-on-deadline bound, which is what the
harness ceiling already does and which destroyed the round above; and a supervising daemon
outliving every run, which owns child lifecycle at the cost of a second long-lived authority beside
the three ADR-0002 fixes.

## Decision

A dispatch's liveness is decided by testing a process identity the binary records at spawn — the
child's process number together with its start identity, so a reused number cannot impersonate a
dead child — and never by reading the child's output. The orchestrator's invocation of `pce
dispatch` does not hold a foreground command, so no harness ceiling can kill the binary mid-wait;
the binary still waits, and still writes exactly one completion record.

The bound on an unfinished dispatch reports and never terminates. A check-in states which liveness
state the child is in and whether it has produced its artifact, and closes nothing: a dispatch is
closed by its completion record or by a durable reconciliation, never by an observer deciding what
a child probably did. An unaccounted dispatch stays in the ledger until reconciled and is never
dropped from it.

Non-production is counted separately from defect rounds. An attempt ended by infrastructure
advances the liveness escalation threshold and charges no round against the plan or review cap;
that exemption is derived from the binary's own record of why the child ended, never from the
orchestrator's account of itself. Only the threshold's hold reaches the human; every check-in and
completion reaches the orchestrator alone.

## Consequences

The ten-minute ceiling on how long any dispatched agent may think is removed, which is what makes a
falsification gate that executes stimuli affordable and stops a slow gate from being a hazard to the
step it gates. Killed-by-our-own-patience becomes unreachable rather than avoided, in the same way
ADR-0007 made a missing standard-input binding unspellable.

The cost is that repeated infrastructure failure can no longer stop a node through the round cap;
the liveness escalation threshold is now the only thing that stops it, and a defect in that
threshold leaves a node retrying without bound. A bound that only reports also cannot bound spend,
which is consistent with this Program's standing non-goal of a cost ceiling but means a wedged child
consumes wall-clock until the threshold's hold fires.

A liveness check is a claim that passes for the wrong reason more easily than most: one reporting
everything alive is perfect in every run where nothing dies, and one reporting nothing alive passes
any test that only kills. The killed child and the still-working child are therefore one paired
obligation, and process-number reuse — where "alive" is true about a process and false about the
dispatch — is a third required case. No implementation of this ADR is established by a test that
exercises only one side.

Reconciliation does not add a lifecycle record: it conditionally occupies ADR-0007's one existing completion slot with a reconciled-dead outcome that carries current artifact production and no measured duration, exit status, or usage. Newly written schema-version-1 identity sidecars also record the owning continuation identity so reconciliation is refused while that process can still append an observed-child completion. The continuation identity is optional only when reading pre-upgrade schema-version-1 sidecars; those sidecars remain readable and reconcilable under the child-identity guard alone.
