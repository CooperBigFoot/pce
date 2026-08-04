# ADR-0010: The replay oracle stays with the parent harness

## Status

Accepted

## Context

The gate chooses a falsification stimulus and can see the recorder's observation and immutable
execution reference. Repair relevance, however, depends on an expected outcome, isolated target
revisions, each revision's own schema, and reproducibility across repeated raw observations. Putting
those authorities in the gate-visible exchange would let the subject of measurement influence its
oracle.

The rejected alternatives were letting the gate declare its oracle, which makes the expectation
self-authored; replaying in the live working tree, which admits dirt and mutates operator state;
validating both refs with the delivery or live schema, which confuses revision-local contracts; and
comparing only the final conformance bit, which hides different process results or artifact bytes.

## Decision

After the gate child exits and its execution sidecar is complete, the parent harness alone loads the
expected verdict outcome, replaces its public invocation with a worker whose argument vector and
environment contain no replay oracle, materializes each target commit as a fresh tree and removes
the private repository metadata before the stimulus runs, validates with each target ref's own
schema, schedules the four runs from operating-system randomness so invocation position does not
identify a side, compares each side's two raw runs for reproducibility, and emits the interpretation.
When both names resolve to the same commit, all four observations belong to one target identity: any
difference makes both reported sides non-reproducible instead of allowing an arbitrary pairing to
manufacture direction. The expected outcome is absent from every gate-visible role frame, argument,
environment value, stdin stream, recorder request, recorder response, and execution sidecar.

## Consequences

The gate retains authority to choose useful stimuli without gaining authority over their expected
meaning. Replay must manage private request transfer, independent checkout and cleanup lifecycles,
retain artifact bytes when comparing raw observations, and treat checkout and oracle failures
separately from stimulus results. The public CLI remains an operator interface, but none of its
parent-only values survives in replay process ancestry. Each run may mutate only its own
harness-created materialized tree, and repository metadata is absent while the stimulus runs, so one
run cannot communicate with another or the supplied repository through checkout administration
state. This boundary describes responsibility; it does not by itself prove an implementation
correct.
