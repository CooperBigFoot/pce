---
name: grill-ticket
description: Claim and deeply discover one explicit Program Effort, then author, publish, and verify its single linked vision. Use for /grill-ticket with an Effort issue URL or number.
---

# Grill Ticket

`$ARGUMENTS` must identify exactly one Effort issue by number or URL. Ask only for the missing identity. Never choose an Effort from a repository-wide singleton assumption.

The ticket and its repository vision are the only discovery records. PCE's canonical `grill-me` interview is the only discovery mechanism.

## Validate and claim

Detect an explicit draft-only request before any mutation, including assignment. For draft-only, validate read-only and do not assign, change issue content, publish, or merge. Still require every state and ownership check below; draft-only does not bypass Effort validation.

Before questions or mutation, inspect the repository and use `gh` to load the issue, its comments and timeline, authenticated user, linked Program Map, all member Efforts and dependencies, relevant repository evidence, delivery records, and any linked vision. Require all of the following:

- the issue is open, has label `pce:effort`, and contains `<!-- pce:effort -->`;
- it has exactly one valid `Program: <canonical GitHub issue URL>` line pointing to an issue with label `pce:program` and `<!-- pce:program -->`;
- it has exactly one `Depends on:` line naming `none` or valid Efforts in that same Program, and the complete Program dependency graph is acyclic;
- the Program Map contains this canonical Effort URL exactly once as an open member and not as a landed outcome or duplicate entry;
- it has exactly one `Vision:` line that is `pending` or identifies one repository vision;
- a linked vision contains exactly one canonical `Program:` line and exactly one canonical `Effort:` line, both matching the ticket, Map, and repository identity.

Stop without mutation and report the malformed, closed, duplicate, mismatched, foreign-Program, cyclic, missing-membership, or otherwise ambiguous state. Do not claim or mechanically repair missing or duplicate Map membership. If any other user is assigned, stop without mutation in either mode. For publication mode only: If unassigned, assign the authenticated GitHub user with `gh` and verify the assignment. If assigned only to that user, treat it as a resumable claim. For draft-only, an unassigned or same-user issue remains unchanged. Claiming does not require blockers to have landed; blockers constrain delivery, not discovery.

## Discover through the canonical interview

Load and follow PCE's canonical `grill-me` skill. It is the single source of truth for interview rounds, question shape, investigation, and confirmation; do not copy its loop. Apply it at deep vision altitude to this Effort only. Treat the ticket, Map, prior comments, repository, and existing vision as evidence. Resolve the contained outcome, success, boundaries, trade-offs, risks, and assumptions without decomposing implementation tasks.

On a same-user rerun, reconstruct settled content and ask only about changed, contradictory, or unresolved territory. Do not create a second vision and do not silently discard still-valid content.

## Create or revise exactly one vision

The initial `/grill-ticket` request explicitly authorizes vision materialization after the canonical completion confirmation. It does not authorize implementation.

For a new vision, derive a concise descriptive name from the confirmed understanding and call the deterministic helper from the installed `to-vision` skill:

```bash
python3 <path-to-to-vision>/scripts/create_vision.py "<derived name>"
```

Author a standalone vision whose first non-title metadata lines are:

```markdown
Program: <canonical GitHub issue URL>
Effort: <canonical GitHub issue URL>
```

Keep the rest flexible and sufficient for a fresh implementing agent. Vision creation itself must not start implementation.

Make the confirmed vision durable before changing `Vision: pending`. Load and follow the `to-vision` publication contract, including its closing-reference safeguards, independent review, required checks and approvals, nonblocking progress updates, preservation rules, and exact target verification. Follow the repository's contribution rules. From the intended target branch, create or reuse a dedicated vision branch, commit only the confirmed vision change, account explicitly for ignored planning paths without staging unrelated files, push it, and verify the pushed commit contains the exact regular file. Open or reuse a documentation PR and take it through the normal review and merge process. The initial `/grill-ticket` request and confirmed summary authorize ordinary documentation publication and issue linkage, not implementation, merging research or implementation PRs, or bypassing protected-branch policy. Explicit draft-only output follows `to-vision`'s draft-only boundary and stops before publication or linkage; retain this Effort's provenance and same path for later publication.

Before any vision-link mutation, reload and repeat the complete Validate and claim state gate above. Reject malformed, duplicate, ambiguous, or conflicting state without mutation. Publication must never close the Effort or mark it delivered or landed. Use neutral references, never auto-closing syntax even in negated phrases. Inspect existing PR text, commit messages, and explicit closing relationships before merge as required by `to-vision`.

Replace `Vision: pending` with exactly one repository-relative Markdown link whose destination is the canonical GitHub blob URL pinned to the pushed commit that contains the file. Never publish a target-branch URL until that exact content is present there, and never link an uncommitted or unpushed file. Ensure the vision contains exactly one canonical `Program:` line and exactly one canonical `Effort:` line, matching the ticket and Map. Fetch the linked URL and reload the issue to verify both directions and line uniqueness.

The final handoff requires more than a pushed commit or open PR. Fetch the intended target branch after publication, read the same regular `planning/visions/` path from that ref, and verify that the vision has been merged into the intended target branch with exact content and matching provenance. Reload the issue and verify the issue remains open, its single commit-pinned link still matches, and Program membership remains valid. Unexpected closure is a publication error, not delivery; follow `to-vision` recovery safeguards, never blindly reopen, and do not claim success until open state is safely restored and verified. If commit, push, review, checks, PR, linking, merge, target-copy, or open-state verification fails, preserve the inspectable state, report the exact partial result, keep the verified commit-pinned issue link when safe, and do not describe the Effort as ready.

For a valid existing link, load the pinned durable content and revise that same regular repository path only after confirmation. Preserve settled content. Publish the revision as a new verified commit, update the ticket to its new commit-pinned URL, verify both directions, merge it through normal review, and reverify the target copy before handoff. Refuse symlinks, paths outside `planning/visions/`, missing or unreachable commits, competing links, or provenance mismatches.

Stop before implementation. Report the vision path, publication PR, verified Git refs (pushed commit and fetched target SHA) and URL, Effort URL, linkage, open-state and target-copy verification, and the exact handoff `implement-vision <Effort number or canonical URL>`. Only after those checks may the report call the Effort ready for `implement-vision <Effort number or canonical URL>`.
