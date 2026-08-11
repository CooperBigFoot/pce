# ADR-0013: Exceptions relocate merge proof

## Status

Accepted

## Context

The convention-derived step selector assumes a step lands directly on its milestone integration branch. One historical step instead landed through a renamed integration branch which was then promoted to the default branch. Treating that history as an override would let a declaration answer the merge question that GitHub and git are authoritative for; ignoring it would query the wrong selector forever.

## Decision

An append-once, node-scoped exceptional merge chain may identify a declared integration branch and two distinct positive pull-request numbers. The step head remains convention-derived and the promotion base is fixed to `main`. Landing proves the step-to-integration and integration-to-default hops independently through fresh GitHub identity/state/squash observations and git reachability observations. The declaration stores no result, squash OID, trusted boolean, or verification flag.

The binary retains all subprocess, repository, path, and network authority. Core receives typed observations, checks the declared selectors and pull-request identities, folds each hop through the shared merge decision table, and constructs an aggregate merged result only when both folds are merged. Ordinary status projection continues to use the convention-derived selector.

## Consequences

Exceptional landing checks require two GitHub queries, two fetched base branches, and up to two independent reachability checks. Any missing observation, selector or number mismatch, authority failure, non-merged state, squash disagreement, or unreachable squash commit refuses or leaves landing inconclusive according to the existing three-valued merge rules. The event log remains auditable without becoming a second merge authority, at the cost of a distinct exceptional snapshot variant and route-agreement validation.
