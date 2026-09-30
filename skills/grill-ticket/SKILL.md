---
name: grill-ticket
description: Claim and discover one Effort, then publish its vision. Use for /grill-ticket with an Effort issue number or URL.
---

# Grill Ticket

`$ARGUMENTS` must identify exactly one Effort issue by number or URL. Ask only for the missing identity. Never choose an Effort from a repository-wide singleton assumption.

The ticket and its repository vision are the only discovery records. PCE's canonical `grill-me` interview is the only discovery mechanism.

## Validate and claim

Detect an explicit draft-only request before any mutation, including assignment. For draft-only, validate read-only and do not assign, change issue content, publish, or merge. Still require every state and ownership check in the required reference; draft-only does not bypass Effort validation.

Before questions or mutation in either mode, load and follow [read-only Effort validation](effort-validation.md). Do not proceed to assignment or discovery unless every state and ownership check succeeds.

For publication mode only: If unassigned, assign the authenticated GitHub user with `gh` and verify the assignment. If assigned only to that user, treat it as a resumable claim. For draft-only, an unassigned or same-user issue remains unchanged. Claiming does not require blockers to have landed; blockers constrain delivery, not discovery.

## Discover through the canonical interview

Load and follow PCE's canonical `grill-me` skill. It is the single source of truth for interview rounds, question shape, investigation, and confirmation; do not copy its loop. Apply it at deep vision altitude to this Effort only. Treat the ticket, Map, prior comments, repository, and existing vision as evidence. Resolve the contained outcome, success, boundaries, trade-offs, risks, and assumptions without decomposing implementation tasks.

On a same-user rerun, reconstruct settled content and ask only about changed, contradictory, or unresolved territory. Do not create a second vision and do not silently discard still-valid content.

## Create or revise exactly one vision

The initial `/grill-ticket` request explicitly authorizes vision materialization after the canonical completion confirmation. It does not authorize implementation.

For a new vision, derive a concise descriptive name from the confirmed understanding and call the deterministic helper from the installed `to-vision` skill:

```bash
python3 <path-to-to-vision>/scripts/create_vision.py "<derived name>"
```

Author a self-contained Effort vision whose first non-title metadata lines are:

```markdown
Program: <canonical GitHub issue URL>
Effort: <canonical GitHub issue URL>
```

Keep the rest flexible and sufficient for a fresh implementing agent. Vision creation itself must not start implementation.

Load and follow the `to-vision` authoring checks against confirmed decisions, including in draft-only mode.

For a valid existing link, load the pinned durable content and revise that same regular repository path only after confirmation. Preserve settled content. Refuse symlinks, paths outside `planning/visions/`, missing or unreachable commits, competing links, or provenance mismatches.

## Publish through the shared procedure

For publication or resumed publication, after the authoring checks, load and follow the [vision publication procedure](../to-vision/publication.md), the complete `to-vision` publication contract once. For draft-only, stop with the local draft result without loading or executing publication. It owns publication, the single link mutation, independent review, merge, closing-reference safeguards, fresh pre-mutation state validation, and final read-back verification. This skill owns discovery, initial validation and claim, and confirmed vision preparation, not a second publication tail. The initial `/grill-ticket` request and confirmed summary authorize ordinary documentation publication and issue linkage, not implementation, merging research or implementation PRs, or bypassing protected-branch policy.

Consume its verified result; do not repeat publication, linkage, or final verification. The result must establish that the exact vision is merged into the intended target branch, its single commit-pinned link and matching provenance are verified, Program membership remains valid, and the issue remains open. For draft-only or partial failure, consume and report that result instead, retaining the same path, branch, PR, and single link for resumption under the shared procedure. Never call an unverified result ready.

Stop before implementation. Report the shared procedure's vision path, publication PR, verified Git refs (pushed commit and fetched target SHA) and URL, Effort URL, linkage, open-state and target-copy verification, and the exact handoff `implement-vision <Effort number or canonical URL>`. Only a successful verified publication result makes the Effort ready for `implement-vision <Effort number or canonical URL>`.
