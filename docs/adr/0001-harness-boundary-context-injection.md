# ADR-0001: Context injection at the harness boundary

## Status

Accepted

## Context

The enforcement split assigns each workflow invariant to a layer by its kind: prohibitions to
hooks at the tool boundary, verifications to the `pce` binary, durable per-repo facts to a
tracked file read only from the default branch. Replacing the rewritten state file with an
event log plus derived state exposes a need that fits none of them. The log is not only a
resume spine — it is what an orchestrator reads when its own context has been compacted
mid-run, and the feedback corpus records both that runs survive repeated compaction and that
per-round histories are the most useful thing available afterwards. Relying on the
orchestrator to notice it has lost context and to call the status verb is precisely the class
of self-applied prose obligation the Program's founding hypothesis says fails.

Claude Code exposes `PostCompact`, and `SessionStart` with `resume` and `compact` matchers,
each able to inject text through `hookSpecificOutput.additionalContext`. The credible
alternatives were to leave recovery to the orchestrator's own discipline, or to place the hook
in the tool-boundary prohibitions work, which is scoped to command shapes and to hooks that
must ship after the verbs they enforce.

## Decision

The enforcement split gains a fourth layer: context the orchestrator must hold but cannot be
relied on to fetch is injected at the harness boundary by a hook that **fails open**.

Its first instance rehydrates a compacted run. On `PostCompact`, and on `SessionStart`
matching `resume` and `compact` only, the hook runs the status verb in the current repository
and injects the run snapshot's recovery digest. It selects the run by most recent event-log
record, uses no run-marker file, and injects nothing — silently — when there is no candidate
or the choice is ambiguous. A cold `startup` session is deliberately excluded: it never held
the context, and the skill's own startup path already reads the snapshot.

Fail-open is the boundary condition that keeps this layer distinct from prohibitions.
Marker-free run selection is acceptable here and forbidden in the prohibition layer, because a
prohibition that silently fails to fire is a broken gate while an injection that silently fails
to fire costs only a convenience.

## Consequences

Recovery stops being an obligation the orchestrator must remember at the moment it is least
able to, and the status verb becomes the single computation serving both the ordinary read
path and the recovery path.

The hook lives outside this repository, so no merged pull request activates it; the deliverable
is the hook script plus `install.sh` wiring, and activation requires a human `install.sh` run,
which makes any acceptance depending on the hook firing a manual step. A fresh session that
inspects a paused run without invoking the orchestrator receives nothing automatically. The
recovery digest is a judgment about what matters and will be wrong for some run; it must name
what it elided rather than truncate silently. Future tickets may assume this layer exists,
which is what makes the decision costly to reverse.
