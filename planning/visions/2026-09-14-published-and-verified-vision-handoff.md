# Published and verified vision handoff

## Outcome

After `to-vision` reports success, a fresh agent can invoke `implement-vision <repository-relative vision path or Effort identity>` without discovering unfinished vision publication. Move publication responsibility to the authoring workflow rather than requiring the implementing agent to complete it. This applies to standalone visions as well as Effort-derived visions.

## Current repository context

`skills/to-vision/SKILL.md` currently ends by requiring a local draft and deferring standalone publication to `implement-vision`. Replace that paragraph rather than appending contradictory requirements. `skills/implement-vision/SKILL.md` explicitly permits a new local standalone bootstrap, and the README describes that behavior. Both must be aligned with the new default. `skills/grill-ticket/SKILL.md` already requires durable publication, commit-pinned linkage, and exact target-branch verification for Effort visions; preserve those guarantees and align its publication safeguards.

`grill-me` remains a discovery and confirmation workflow. Confirmation alone does not publish a vision or authorize implementation.

## Publication contract

Invoking `to-vision` authorizes publication of the confirmed vision unless the user explicitly requests draft-only output. It authorizes the ordinary documentation PR merge, not implementation or merging research/implementation PRs. Preserve deterministic naming, reuse the existing regular `planning/visions/` path, and reconcile existing content without discarding settled intent.

After authoring:

1. Inspect contribution rules and establish the intended target branch. Create or reuse a dedicated vision branch. Respect existing repository worktree and user-state preservation rules.
2. Commit only the vision change. Account explicitly for ignored planning paths without staging unrelated files.
3. Push and verify that the remote commit contains the exact authored file.
4. Open or reuse a documentation PR targeting the intended branch. Obtain independent review and pass required checks. Never bypass protected-branch rules or required approvals.
5. For Effort-derived visions, validate the Effort, Program membership, repository identity, and matching provenance before issue mutation. Replace `Vision: pending`, or update the existing single vision link for a revision, with exactly one Markdown link displaying the repository-relative path and targeting the verified commit-pinned GitHub blob URL. Reload the issue and fetch linked content to verify both directions and declaration uniqueness. Reject ambiguous, conflicting, or malformed state rather than guessing or creating competing links.
6. Merge the documentation PR through the normal permitted process.
7. Fetch the intended target branch and verify that the same regular vision path contains the exact authored content. For Effort-derived visions, verify matching Program/Effort metadata and that the issue remains open.

Standalone visions do not require tickets or Program metadata. Do not invent provenance for them. Effort-derived paths retain their existing identity and cannot bypass ticket validation by being passed as paths.

Use nonblocking progress updates during checks and reviews, and resume when they finish. A pushed commit or open PR is not completion. If blocked, preserve inspectable partial state and report the precise blocker and relevant file, branch, commit, PR, and linkage state. Do not call the vision implementation-ready.

On success report the vision path, publication PR, verified Git refs, and exact `implement-vision` handoff. Do not start implementation automatically.

## Keep vision publication separate from delivery

An observed failure involved the PR description `Does not close #225.` GitHub treated the embedded closing keyword and issue reference as an instruction despite the negation. The documentation merge closed an Effort whose implementation was outstanding.

Vision publication must never close an Effort or mark it delivered or landed. Avoid GitHub auto-closing syntax in vision PR descriptions and commit messages, including negated phrases. Use neutral references such as `Related Effort: #225`. Check publication closing references before merge, including existing PR text and explicit closing relationships, and verify the Effort is still open after merge. Do not rely solely on prose stating that closure is not intended.

Unexpected closure is a publication error, not evidence of delivery. Report it, preserve evidence, and do not claim a successful handoff until the open-state requirement is restored and verified through permitted action. Do not blindly reopen issues whose closure may reflect unrelated legitimate work. `land-ticket` remains responsible for Effort closure after verified implementation delivery.

## Implementation boundary and compatibility

Remove the standalone publication-bootstrap exception from `implement-vision`. Keep its input validation and exact target-branch durability checks. Missing publication stops implementation; the normal `to-vision` path must resolve that prerequisite before handing off. Explicit draft-only output is supported but clearly not implementation-ready. A later authoring invocation can publish that same path.

Align the README, relevant skill descriptions and cross-references, and tests with the new ownership boundary. Keep `grill-ticket` publication consistent with the standalone authoring contract without weakening its Effort validation. Preserve six skills and the lean repository surface; do not add a runtime, package manager, CI workflow, or generated workflow state.

## Evidence of success

- Normal standalone authoring reaches reviewed, merged, exact target-branch content before reporting an implementation handoff.
- Effort authoring additionally establishes one verified commit-pinned link and matching provenance, and verifies the Effort remains open after merge.
- Explicit draft-only requests perform no publication and are not described as ready.
- Publication failures expose precise partial state and cannot produce a success report.
- Neither authoring workflow uses auto-closing references or treats vision publication as implementation delivery.
- `implement-vision` retains its verification gate but no longer performs draft publication. Documentation no longer directs users into that deferred-publication path.
- Tests cover the changed instruction contract and the observed negated-closing-reference failure where feasible. Follow repository regression-proof-before-fix rules: establish a failing test against the actual affected behavior or instruction path before changing it, then make that same test pass. Do not claim static text tests execute GitHub behavior.
- Run `python3 -m unittest discover -s tests -v` from the repository root. Installer changes are outside scope unless demonstrably necessary; if made, preserve unrelated state and test with an isolated temporary HOME.

## Scope and assumptions

This work changes skill instructions, supporting repository documentation, and standard-library tests. It does not implement the vision described by any downstream Effort, publish or close downstream tickets, or alter GitHub approval policy. Publication depends on available credentials, reviewers, and required checks; real blockers must remain explicit rather than being bypassed.
