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
- its single `Depends on:` line names `none` or valid Efforts in that Program;
- its single `Vision:` line is `pending` or identifies one repository vision that links back to this Program and Effort.

Stop without mutation and report the malformed, closed, or ambiguous state. If unassigned, assign the authenticated GitHub user with `gh` and verify the assignment. If assigned only to that user, treat it as a resumable claim. If any other user is assigned, stop without mutation. Claiming does not require blockers to have landed; blockers constrain delivery, not discovery.

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

Keep the rest flexible and sufficient for a fresh Prime Agent. Commit the vision only when the repository workflow explicitly calls for a commit; vision creation itself must not start implementation.

Replace `Vision: pending` with exactly one repository-relative Markdown link to that file at the target branch's canonical GitHub URL. Ensure the vision's `Program:` and `Effort:` values are canonical issue URLs. Reload the issue and file to verify both directions. If linking fails after file creation, preserve the file, report the exact partial state, and do not invent success.

For a valid existing link, read and revise that same regular file only after confirmation. Preserve settled content. Verify both directions and add one concise issue comment describing the revision. Refuse symlinks, paths outside `planning/visions/`, missing files, competing links, or provenance mismatches.

Stop before implementation. Report the vision path, Effort URL, linkage verification, and the exact handoff: `Give <vision path> to a root Prime Agent using implement-vision.`
