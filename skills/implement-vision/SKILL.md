---
name: implement-vision
description: Implement or resume a published vision from its path or associated issue.
---

# Implement Vision

Own the full vision outcome through testing, independent review, authorized merge, and verification on the actual target branch. `grill-me → to-vision → implement-vision` separates confirmed intent, publication, and implementation; publication alone grants no implementation authority. Standalone, issue-linked, and experiment visions use the same safeguards.

## Resolve published intent

Follow the [checkout and preservation policy](checkout-preservation.md); give it in full to delegates before they create a checkout. Preserve unrelated changes, staged files, branches, worktrees, and uncertain ownership. Implementation owners use dedicated worktrees; read-only and supporting tasks need no separate checkout.

Accept a repository-relative `planning/visions/` path, an issue number in the verified current repository, or a canonical GitHub issue URL. Derive a URL's repository from the URL, verify the checkout's canonical remote, and never use an unrelated repository. If no checkout exists, create a durable canonical clone under the checkout policy. Follow issue context to its vision; recover associated issues from ordinary links and relevant publication evidence, but do not invent one for standalone work. Resolve conflicting identities or ambiguous links before proceeding. Respect assignments and repository permissions.

Establish the target branch from repository policy and GitHub evidence, normally the default; stop on conflict. Fetch it and require the complete supplied vision to match the target's tracked regular file inside `planning/visions/`, with no symlinked path components. A draft, pushed branch, or open publication PR is not published intent. Missing, stale, unmerged, or conflicting publication blocks planning, delegation, branch creation, and implementation: preserve evidence and return authoring to `to-vision` for the same path.

## Establish the remaining outcome

Work without relying on the original conversation. Read repository instructions and inspect relevant source, tests, history, issues, Program context, branches, PRs, and validation only as needed to establish what remains and proceed safely—not as an exhaustive audit. Distinguish delivered target effects from active, abandoned, or incomplete work; branch or PR status alone proves no outcome. Reuse evidence while valid, refreshing when changes or waits affect consequential decisions or final verification.

If already delivered, validate the current target effects instead of creating another PR. Reconcile an associated issue only as needed, without duplicate completion comments. Regrill only for a material flaw, missing outcome, or obsolete assumption. Investigate facts and choose reversible engineering details yourself; ask about missing intent, priorities, meaningful trade-offs, credentials, authority, or exceptional irreversible actions. Check actual dependency effects and report concrete blockers.

## Delegate and implement

Prefer smaller, capable models for substantive implementation, investigation, testing, and independent review when the work suits them—not just administrative tasks. Give delegates clear responsibilities and enough context to work independently. Reserve stronger models and deeper reasoning for demanding architecture, unresolved debugging, or complex cross-cutting changes. Match reasoning effort to difficulty and escalate when the work proves harder than expected.

Load only the preference reference matching the current harness: [Prime Agent](prime-agent.md) or [Antigravity CLI](antigravity-cli.md). The shared responsibility and difficulty guidance above applies regardless of harness.

Give implementation owners the complete vision, relevant source context, constraints, and naming boundary below before design or editing. One accountable owner can integrate bounded contributions into one coherent PR, owning tests, commit, push, and delivery from the intended target. Delegation does not itself require another branch, worktree, or PR. Avoid conflicting concurrent edits; add owners or coherent vertical delivery slices when the work warrants them, not to mirror agent count. Stage only intended changes.

Follow [test-first development](../test-first-development/SKILL.md) for code and tests and [github-writing](../github-writing/SKILL.md) for GitHub text. Keep private data private. Match requirements to source, meaningful tests, and observed results; investigate contradictory or missing evidence.

Experiment only when useful and within scope and authority. Prefer the simplest supported solution; discard prototypes without ongoing value. For experiments informing a meaningful decision, keep brief questions, observations, limitations, and decisions in `planning/experimentation/<vision-filename>.md`, linked to the vision—not an exhaustive log. Experiments replace neither testing nor independent review.

## Review and deliver

Every PR needs independent review by an agent who did not implement it; a smaller capable model can do this. Supply the complete vision, repository rules, diff, and validation evidence. Review behavior, safety, regressions, scope, naming, architectural fit, and meaningful test expectations. Removed coverage must protect a retired requirement or retain its useful guarantee elsewhere. For large changes, ask what can be removed without weakening the outcome.

Report material review gaps, including unexamined context and failed delegated reviews; receiving a diff is not proof of examining it, and partial review is not whole-PR approval. Resolve findings with justified fixes or reasoned dismissal. After changes, review their effects and verify prior findings and dismissal reasons; broaden review when warranted rather than automatically restarting it. A finding's absence from a later review does not prove resolution.

Link the vision and associated issue neutrally in PRs. Keep the issue open until its full outcome is delivered and verified; avoid auto-closing references in PRs and commits, and inspect closing relationships and the actual merge message before merge. Merge only with resolved findings, passing required checks, permissions, and applicable approvals. Never bypass protected branches. Honor withheld merge authority and report an open PR as unfinished delivery.

An implementation handoff authorizes ordinary branches, PRs, repairs, and merges unless constrained. Deployments, destructive operations, spending, credentials, external publication, infrastructure changes, and other irreversible external acts not clearly authorized still need human authority.

## Verify completion

After authorized merges, fetch and inspect the actual target against the entire vision, run required tests, and verify delivered effects. A merged PR alone is not completion; report failures and unfinished work.

For an experiment vision, return concise evidence and links to the originating discussion: what was established, what was not, and relevant implications, including negative or inconclusive findings. Do not choose the production design or restart the discussion. Read back the posted comment; failed reconciliation leaves the experiment issue open. The originating discussion stays open for the human to resume with `grill-me <issue>`.

Close only the issue the vision explicitly designates, after its full outcome is delivered and verified, with a useful short result and PR links. Research or design can be that outcome. A link alone grants no closing authority; ask if designation is unclear and leave it open meanwhile. Read back state and comments; report GitHub failures as unfinished reconciliation. Standalone work has no issue to close. Do not automatically close the Program, reorganize tickets, or initiate backlog reconsideration.

Apply the checkout policy's final audit and post-merge cleanup. Report the outcome, useful verification, PR links, material limitations, and retained checkouts with reasons. Completion requires verified target effects and designated issue closure, not a separate landing invocation.

## Product naming boundary

Use established repository and domain vocabulary and name production components for their stable responsibility, not a ticket, vision, or implementation phase. Inspect the architecture before choosing boundaries; vague names such as `Manager` are no substitute for a clear domain capability. Delivery identifiers belong in appropriate traceability metadata, not as the organizing principle of maintained product architecture.

Do not introduce or expand delivery-shaped production components. Do not repair inherited naming merely because this workflow encountered or depends on it; necessary changes follow the vision and compatibility requirements. Preserve public interfaces or use an explicit migration when changing them. No repository-wide naming linter, universal downstream convention, or unrelated cleanup is required. Keep PCE's Program and Effort workflow artifact names.
