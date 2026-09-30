# Effort implementation rules

Required for every Effort-derived input before validation, planning, delegation, branch creation, or substantive implementation. Apply only to Effort implementation; this reference does not invoke discovery, publication, or landing.

## Published intent

An Effort-derived vision must already satisfy the authoring publication contract through `grill-ticket` or a `to-vision` invocation that applies the same complete Effort validation and linkage safeguards. Verify that the ticket's one `Vision:` link, its commit-pinned copy, and the target-branch copy identify the same regular `planning/visions/` file with exactly matching `Program:` and `Effort:` provenance and exact content. Do not bootstrap publication for an Effort. On any missing, unmerged, stale, or conflicting target copy, stop before planning or substantive implementation and report the publication gap.

### Effort provenance and durable linkage

Scan provenance before classifying any candidate vision, including the ticket-linked and target-branch copies. With no `Effort:` line, a path input remains standalone and follows the standalone workflow unchanged. Any vision reached through an Effort identity must contain exactly one canonical `Effort:` line and exactly one canonical `Program:` line. More than one line, a malformed or noncanonical URL, a foreign repository, a missing line where Effort provenance is required, or disagreement among copies is invalid rather than standalone.

For an Effort-derived vision, use `gh`, the repository, and those links to reconstruct the Effort, Program Map, dependencies, comments, delivery records, linked PRs, target-branch state, and validation. Validate that the open issue has the `pce:effort` label and `<!-- pce:effort -->` marker; its body has exactly one Program, dependency, and Vision declaration; the Map contains exactly one membership for it; and the issue, Map, repository, linked vision path, supplied path when present, and both provenance URLs all match. Stop on duplicate, missing, noncanonical, conflicting, or mismatched provenance or durable linkage rather than attaching delivery to the wrong ticket.

Before planning or delivery, parse the complete Program dependency graph. Require exactly one unambiguous `Depends on:` declaration on every involved Effort. Every dependency must be a structurally valid Effort in the same Program, and the graph must be acyclic. Stop on duplicates, conflicts, foreign-Program dependencies, missing membership, malformed references, or cycles; do not choose an interpretation.

Only after every applicable input, provenance, repository, ticket, Program Map, vision-link, commit-pinned-content, target-copy, and dependency check succeeds is the input valid. A number, URL, and recovered path then converge on the same validated Effort workflow.

## Delivery blockers

Treat recorded dependencies as delivery blockers. Use ordinary tracker and engineering judgment to determine which investigation or implementation can proceed, but do not deliver or merge an outcome that relies on a blocker that fails the `landed` predicate. Report any blocker that prevents safe progress.

## Implementation PRs

For an Effort-derived vision, every implementation PR body must include `Effort: <canonical GitHub issue URL>` as a plain reference. Do not use `close`, `closes`, `closed`, `fix`, `fixes`, `fixed`, `resolve`, `resolves`, or `resolved` with the Effort reference. An individual PR must never close the Effort, including when one PR happens to deliver the full vision. Keep detailed implementation and validation evidence on the PRs and Effort rather than expanding the Program Map.

## Final delivery record

After all approved implementation PRs land and final target-branch validation passes, reload every Effort comment. The sole authoritative delivery record is exactly one comment marked `<!-- pce:delivery -->`. If none exists, create one. If exactly one exists, reconcile its claims with current target-branch and GitHub evidence and update that same comment in place on rerun. If more than one exists, or its identity conflicts with the linked vision, Program, PRs, or target branch, stop and report the conflict without posting another record or completing the tracked outcome. The authoritative comment must contain:

- the delivered outcome;
- canonical URLs for every merged implementation PR;
- validation evidence, including commands and results;
- material deviations from the vision, or `None`;
- unresolved follow-up risks, or `None`.

Verify every PR is merged into the intended target, the reported effects exist on that branch, validation is current, exactly one delivery marker remains, and the updated durable comment can be read back. Deterministic reruns update or preserve that one comment; they never append a competing record. Report partial GitHub failure precisely and do not invent a delivery record. Leave the Effort open for `land-ticket`; implementation completion does not authorize its closure, a `<!-- pce:landed -->` record, or the Program Map mutation.

