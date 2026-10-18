---
name: implement-vision
description: Implement or resume a published vision from its path or associated issue.
---

# Implement Vision

Own the full vision outcome through testing, independent review, authorized merge, and verification on the actual target branch. An issue-linked or experiment vision follows the same process as a standalone vision.

## Resolve published intent

Follow the [checkout and preservation policy](checkout-preservation.md) before creating any checkout, and give the complete policy to delegates before they create one. Preserve unrelated user changes, staged files, branches, and worktrees.

Accept a repository-relative `planning/visions/` path, an issue number in the verified current repository, or a canonical GitHub issue URL. For a URL, derive the repository from that URL, not the caller's checkout. Verify the canonical remote of an existing checkout before using it; if none exists, create a durable canonical clone in a user-owned location under the preservation policy. Never implement URL-derived work in an unrelated repository.

For an issue, read its context and follow its vision link. For a path, recover any associated issue from ordinary links in the vision and relevant GitHub publication evidence. Do not invent an issue for standalone work. Resolve ambiguous links or conflicting repository identities before proceeding, rather than choosing a convenient interpretation. Read relevant Program context and real blockers where present; no special ticket schema is required. Respect another user's assignment and repository permissions rather than taking over.

Establish the intended target branch from repository policy and GitHub evidence, normally the default branch; stop if they conflict. Fetch it and verify that the vision is a tracked regular file inside `planning/visions/`, with no symlinked path components. Read the complete vision and require exact agreement between the supplied local or linked copy and the fetched target copy. A draft, pushed branch, or open publication PR is not published intent. On missing, stale, unmerged, or conflicting publication, stop before planning, delegation, branch creation, or implementation. Preserve the evidence and direct authoring back to `to-vision` for the same path. Do not bootstrap publication here.

## Reconstruct and plan remaining work

Every invocation must work without the original conversation. Read repository instructions, relevant source and tests, linked issues and comments, PRs, branches, worktrees, validation evidence, and target effects. Distinguish verified merged effects, active work, abandoned paths, incomplete local or pushed work, and the remaining outcome. Branch or PR status alone is not proof of delivery. Reuse current evidence within the invocation, refreshing after changes or waits, before consequential mutations, and for final verification.

If the outcome is already delivered, rerun current validation and verify target effects rather than creating another branch or PR. Reconcile an associated issue only if needed; do not duplicate an existing useful completion comment. Regrill only when evidence exposes a material flaw, missing outcome, or obsolete assumption in the vision.

Plan only the remaining outcome. Investigate facts and decide reversible engineering details yourself. Ask the user about missing intent, priorities, meaningful trade-offs, credentials, organizational authority, or exceptional irreversible external actions. Real dependencies constrain safe delivery; inspect whether their required effects exist, not merely whether a ticket is closed. Report concrete blockers without creating a dependency ledger.

## Implement and review

Give each implementation owner the complete vision, relevant source context, constraints, and the naming boundary below before design or editing. Each owner owns one branch from the intended target, its tests, commit, push, and PR. Choose one PR or coherent vertical slices, not arbitrary technical layers. Stage only intended changes.

Before implementing or reviewing code or tests, follow [test-first development](../test-first-development/SKILL.md). Before GitHub drafting, follow [github-writing](../github-writing/SKILL.md). Keep private data private. Match requirements to tests, source, and observed results; investigate missing or contradictory evidence.

Use ordinary links to the vision and associated issue in PRs. Keep the issue open while any outcome remains undelivered or unverified. Avoid auto-closing references in implementation PRs and commits; inspect GitHub closing relationships and the actual merge message before merge so no PR closes the issue before final target verification.

Assign every PR to a fresh reviewer who did not implement it. Supply the full vision, repository rules, complete diff, and validation evidence. Review behavior, safety, regressions, scope, names, and architectural fit. Review tests for independent expectations and meaningful coverage, not a passing count. Check that removed coverage protects a retired requirement or that its useful guarantee remains. Large changes require a necessity review: what can be removed without weakening the outcome?

Repair findings and repeat review after material changes. Merge only with resolved findings, passing required checks, repository permissions, and applicable approvals. Do not bypass protected branches. Honor explicit instructions withholding merge authority; an open PR is not completion.

Ordinary implementation branches, PRs, repairs, and merges are authorized by handing over the vision. Deployments, destructive data operations, spending, credentials, external publication, infrastructure changes, and other irreversible external acts not clearly authorized still need human authority.

## Verify completion

After approved PRs merge, fetch and inspect the actual intended target branch against the entire vision, run its required tests, and verify the delivered effects. Report failures or unfinished work, not completion based on a merged PR alone.

For an experiment vision, return the evidence to its originating discussion with a concise comment: what was established, what was not, relevant implications, and links to detailed results. Preserve negative and inconclusive findings. Do not select the production design or restart the parent discussion. An evidence-return link is not a closing relationship: the parent stays open for the human to resume with `grill-me <issue>`. If the return comment cannot be posted and read back, report unfinished reconciliation rather than a completed handoff.

Only when the full outcome is delivered and verified, close the issue the vision designates to complete and leave a short comment linking the implementation PRs and explaining the delivered result where useful. That outcome can be research or design. Read back the issue state and comment; report a GitHub failure as remaining reconciliation work. Publishing a vision does not close the issue, merging one partial PR does not complete it, and returned evidence does not close the originating discussion. A standalone vision has no issue to close. Do not automatically close the Program, reorganize its remaining tickets, or initiate backlog reconsideration.

Apply the checkout policy for the final checkout audit and post-merge branch cleanup. Report the delivered outcome, useful verification evidence, PR links, material limitations, and retained checkouts with reasons. Completion requires verified target effects and any associated issue closure, not a separate landing invocation.

## Product naming boundary

Every implementation owner must receive this complete naming boundary in its delegated context before designing or editing production code. The owner must read the complete vision and inspect the repository's existing architecture and vocabulary before choosing names or boundaries. Production identifiers and architectural boundaries must use established repository and domain vocabulary and express the component's stable responsibility. Do not substitute vague generic names such as `Manager`, `Runner`, or `Handler` for a clear domain capability.

Delivery identity must not determine production modules, packages, types, functions, commands, routes, services, runtime schemas, user-facing configuration keys, public APIs, or other maintained product architecture. Removing a ticket number is not enough when the renamed component remains organized around an Effort, ticket title, vision, or implementation phase rather than a stable product responsibility.

Do not introduce or expand ticket-shaped product architecture. Repair inherited ticket-shaped components that the implementation directly modifies or depends on, and report unrelated occurrences without turning the vision into repository-wide cleanup. Preserve compatibility for existing public interfaces, or use an explicit migration when a repair changes one.

Ticket identity remains valid delivery metadata and traceability evidence in issues, vision provenance, PR descriptions, branch and worktree names, commit messages, and historical evidence. It may also appear where traceability requires it in test names, fixtures, examples, or study-specific data configuration, although maintained artifacts should prefer behavioral or domain names where practical. Carry a necessary reference as explicit metadata or a comment rather than making it the artifact's organizing name.

Enforce this boundary through implementation and review. Do not add a repository-wide naming linter, impose a universal naming convention on downstream repositories, or perform unrelated cleanup. Do not rename PCE's Program or Effort workflow artifacts.
