---
name: grill-ticket
description: Claim and deeply discover one explicit Program Effort, then create or revise its single linked vision. Use for /grill-ticket with an Effort issue URL or number.
---

# Grill Ticket

`$ARGUMENTS` must identify exactly one Effort issue by number or URL. Ask only for the missing identity. Never choose an Effort from a repository-wide singleton assumption.

Keep discovery in the ticket and its repository vision. Do not invoke `grill-with-docs`, edit `CONTEXT.md`, create ADRs, or use domain-modeling machinery.

## Validate and claim

Before questions or mutation, inspect the repository and use `gh` to load the issue, its comments and timeline, authenticated user, linked Program Map, all member Efforts and dependencies, relevant repository evidence, delivery records, and any linked vision. Require all of the following:

- the issue is open, has label `pce:effort`, and contains `<!-- pce:effort -->`;
- it has exactly one valid `Program: <canonical GitHub issue URL>` line pointing to an issue with label `pce:program` and `<!-- pce:program -->`;
- it has exactly one `Depends on:` line naming `none` or valid Efforts in that same Program, and the complete Program dependency graph is acyclic;
- the Program Map contains this canonical Effort URL exactly once as an open member and not as a landed outcome or duplicate entry;
- it has exactly one `Vision:` line that is `pending` or identifies one repository vision;
- a linked vision contains exactly one canonical `Program:` line and exactly one canonical `Effort:` line, both matching the ticket, Map, and repository identity.

Stop without mutation and report the malformed, closed, duplicate, mismatched, foreign-Program, cyclic, missing-membership, or otherwise ambiguous state. Do not claim or mechanically repair missing or duplicate Map membership. If unassigned, assign the authenticated GitHub user with `gh` and verify the assignment. If assigned only to that user, treat it as a resumable claim. If any other user is assigned, stop without mutation. Claiming does not require blockers to have landed; blockers constrain delivery, not discovery.

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

Make the confirmed vision durable before changing `Vision: pending`. Follow the repository's contribution rules. From the intended target branch, create or reuse a dedicated vision branch, commit only the confirmed vision change, push it, and verify the pushed commit. When repository policy requires a documentation PR, open it and take it through the repository's normal review process; otherwise use and verify the permitted reviewed publication path. The initial `/grill-ticket` request and confirmed summary authorize this ordinary durable publication and issue linkage; they do not authorize implementation or bypass protected-branch policy.

Replace `Vision: pending` with exactly one repository-relative Markdown link whose destination is the canonical GitHub blob URL pinned to the pushed commit that contains the file. Never publish a target-branch URL until that exact content is present there, and never link an uncommitted or unpushed file. Ensure the vision contains exactly one canonical `Program:` line and exactly one canonical `Effort:` line, matching the ticket and Map. Fetch the linked URL and reload the issue to verify both directions and line uniqueness.

The final handoff requires more than a pushed commit or open PR. Fetch the intended target branch after publication, read the same regular `planning/visions/` path from that ref, and verify that the vision has been merged into the intended target branch with exact content and matching provenance. If commit, push, PR, linking, merge, or target-copy verification fails, preserve the inspectable state, report the exact partial result, keep the verified commit-pinned issue link when safe, and do not describe the Effort as ready.

For a valid existing link, load the pinned durable content and revise that same regular repository path only after confirmation. Preserve settled content. Publish the revision as a new verified commit, update the ticket to its new commit-pinned URL, verify both directions, merge it through normal review, and reverify the target copy before handoff. Refuse symlinks, paths outside `planning/visions/`, missing or unreachable commits, competing links, or provenance mismatches.

Stop before implementation. Report the vision path, durable Git ref and URL, Effort URL, linkage and target-copy verification, and the exact handoff `implement-vision <Effort number or canonical URL>`. Only after those checks may the report call the Effort ready for `implement-vision <Effort number or canonical URL>`.
