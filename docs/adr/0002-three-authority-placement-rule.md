# ADR-0002: The three-authority placement rule, and no stored tallies

## Status

Accepted

## Context

Replacing the rewritten state file was motivated by an observed failure: the run's maintained
round counters disagreed with the review artifacts they summarized. The rule proposed for the
replacement was "if git or `gh` knows it, never store it." That rule does not prevent the
failure it cites, because git never knew the round count — the drift was between a maintained
tally and the dispatch records that already implied it.

Two credible positions existed. The feedback review resolved that the log-append verb
increments the round counter in the same call, and one report recommended incrementing
counters in the same write that records the dispatch, so that a tally is at least written when
the event happens. Both keep a number beside the records that imply it.

## Decision

Every fact belongs to one of three authorities: git, `gh`, and the event log's own record
sequence. A fact any authority implies is never also stored; a fact no authority implies is
appended to the log once, when it happens.

No running tally exists anywhere. A round count is the number of dispatch records for that
artifact. A hold's open or closed status is the last open or close record for its key. The ref
a dispatch used rides on that dispatch's record, so there is no per-milestone ref map. This
supersedes the earlier resolution that the log-append verb increments a counter; the verb still
writes the dispatch record atomically when the dispatch happens, which is what that
recommendation was protecting, but it writes no second copy that could disagree.

## Consequences

The observed drift becomes impossible rather than merely detectable, and every derived quantity
has exactly one writer and one reading. Sibling work inherits the same treatment without
negotiating it: hold status, cost per dispatch, and rounds spent are all folds.

Every status read becomes a fold over the run's log rather than a field lookup, so the log must
stay cheap to read in full for the life of a run and the status verb must stay fast enough that
it is actually called. A fact is only as visible as the record that carries it: a dispatch made
outside the recording path leaves no trace at all, where a maintained tally would at least have
been conspicuously wrong. Reversing this means reintroducing a second source for facts that
already have one.
