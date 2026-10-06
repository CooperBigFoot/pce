---
name: implement-vision
description: Implement or resume a published vision. Accept a vision path or an Effort issue number or URL.
---

# Implement Vision

Act as the root owner of the vision-level outcome. Treat the supplied vision as the durable product intent, not as a prescribed code mechanism.

Delegation and independent review are available.

## Resolve the input and target

Load and follow the [checkout and preservation policy](checkout-preservation.md) for every invocation, before any checkout creation. Pass the complete policy to reviewers and other delegates before they create any checkout, even when the root uses an existing checkout. Loading it does not invoke another workflow.

Accept exactly one of these inputs:

- an Effort issue number in the current repository;
- a canonical Effort issue URL, in which case derive the repository from the URL and do not use the caller's current repository identity;
- a repository-relative `planning/visions/` path.

Reject missing, extra, ambiguous, shorthand, and noncanonical issue identities. Resolve a number only after identifying the current repository with GitHub. A canonical URL may name a repository different from the caller's current checkout; resolve and operate on the canonical owner and repository encoded in that URL rather than rejecting it as cross-repository. Locate an existing checkout only after verifying that its normalized canonical remote matches that repository. If none exists, create a durable checkout of the URL-derived repository in a user-owned location. Never read, branch, create a worktree, or implement URL-derived work in an unrelated caller checkout.

For a path input, first identify the current checkout's canonical GitHub repository. The path must remain inside `planning/visions/`, name a regular file rather than a symlink, and have one canonical repository-relative identity. Read it only after those checks, then scan its provenance before classifying it. A path input is standalone only when it has no `Effort:` provenance line. If it has exactly one canonical `Effort:` line, derive the canonical repository and Effort identity from that URL, require its repository to match the normalized canonical remote of the checkout containing the path, and promote the path to that Effort identity. From that point it must run the same complete Effort validation as a number or canonical URL and never continue as standalone. The supplied path remains only a locator; it is not authoritative and does not bypass the ticket or its `Vision:` link.

Once an input is classified as Effort-derived, load and follow [Effort implementation rules](effort-implementation.md) before applicable validation and before planning, delegation, branch creation, or substantive implementation. Standalone inputs do not load these rules.

For all three Effort entry forms, normalize to one canonical repository and Effort URL, load the ticket, and follow its single `Vision:` link. Do not require a person to repeat identity already recovered from valid provenance or to recover a session identifier, worktree path, branch name, or prior conversation.

Determine one intended target branch from repository policy and durable PR evidence, normally the repository default branch, and use that identity for publication, implementation PRs, effect checks, delivery records, and cleanup. Stop if the evidence conflicts. Fetch the intended target branch before treating any intent or implementation as durable.

Use one evidence-gathering pass for input validation and recovery reconstruction within this invocation when the evidence remains current. Do not repeat reads solely because execution reaches another instruction section. This is in-session reuse, not a persistent cache or replacement authority. Every fresh invocation must reconstruct from Git and GitHub without relying on a prior conversation. Refresh affected evidence after relevant state changes, external activity or waits that may make it stale, before consequential mutations, and for final verification. An earlier valid snapshot is not proof of current state. Investigate uncertain or conflicting evidence.

Only after input validation succeeds, classify prior work and plan the remaining outcome. Gathering recovery evidence alongside validation does not authorize early planning, delegation, branch creation, or implementation.

## Validate the resolved input

Complete input validation and the target-branch durability gate below before any planning, delegation, branch creation, or substantive implementation. Read the entire candidate vision, repository instructions, relevant code, tests, and recent project context. Stop on missing, duplicated, malformed, foreign, ambiguous, or conflicting provenance or durable linkage. Give one precise explanation. Do not plan, delegate, or create any work. Do not ask the human to supply an identity that the rejected evidence cannot establish safely.

For every invocation, fetch the intended target branch and read the vision from that fetched ref. The current fetch and reads gathered during input resolution may serve this gate; apply the refresh rules above rather than fetching twice solely for this section. Verify that the canonical path is a regular file tracked on the intended target branch. Compare the relevant local or commit-pinned document with the target copy and require exact content before using it as accepted intent.

### Target-branch durability gate

For Effort-derived inputs, complete the published-intent checks in [Effort implementation rules](effort-implementation.md) before proceeding.

A standalone target-branch vision is a normal start or resume only when its exact regular-file copy is already published and verified. Do not publish drafts or bootstrap publication, even on a first run. On missing, unmerged, stale, or conflicting publication, stop before planning, delegation, branch creation, or substantive implementation; preserve and report the inspectable file, branch, commit, PR, and target state. Direct the authoring workflow to `to-vision` to publish and verify that same path before a later implementation invocation. Explicit draft-only output is not implementation-ready. Never manufacture Program or Effort provenance for standalone work.

## Source and requirement evidence

After the input-validation gate succeeds, investigate gathered source evidence against the vision requirements. Keep mandatory instructions, the complete vision, gate evidence, and uncertain or conflicting sources available. Before delegation, give agents the complete vision, relevant constraints, and inspected source context; investigate conflicts rather than hiding them.

Match each requirement to candidate tests, source, and observed results during validation, before completion claims, and before preparing or updating an Effort delivery record. Investigate missing, indirect, unsupported, uncertain, or contradictory evidence; run required validation and inspect source and target effects. Include current target evidence in the final audit. Requirement-to-evidence reasoning does not replace deterministic Git/GitHub, exact-content, provenance, dependency, or authority gates, or the fresh independent full-diff review below.

## Reconstruct every run

Reconstruct prior work before planning, including after an apparent clean start. Reuse still-current evidence gathered for input validation, and gather any missing recovery evidence; do not repeat an unchanged read only to reconstruct it. For an Effort, inspect the linked vision, Program Map, complete dependency graph, issue timeline and comments, authoritative delivery record, implementation PRs, target-branch commits and effects, branches, validation evidence, and structured local worktrees. For a standalone vision, inspect the fetched target-branch vision, related commits, branches and PRs discoverable from repository and GitHub evidence, target effects, validation evidence, and structured local worktrees. Reconstruction must work without the prior agent session or its conversation.

Classify every relevant unit of prior work into exactly the applicable five-way account: **merged** work whose target effects are verified; **open** work in an active PR or durable pushed branch; **abandoned** work whose branch or PR no longer provides a viable delivery path; **incomplete** surviving work, including local committed, uncommitted, or unpushed evidence; and **remaining** vision outcome not delivered by any of the preceding evidence. Record overlaps explicitly rather than treating a branch or PR status as proof of an effect. Verify the actual target-branch result, then plan and execute only the remaining outcome.

When the entire outcome is already complete, rerun current validation, verify target effects and, for an Effort, require its existing authoritative delivery record to be valid before classifying the workflow as complete. A missing or stale required record is remaining reconciliation work under the delivery rules below. Report a truly already complete invocation as a no-op. For this no-op, do not create a branch, commit, worktree, PR, or delivery record. A stopped earlier agent, changed PR split, absent worktree, or missing status comment is not a reason to restart completed work or regrill. Regrill only when implementation evidence exposes a material flaw, missing outcome, or obsolete assumption in the vision.

Use this predicate everywhere blocker state is computed: an Effort is `landed` only when all four facts are verified: the Effort is closed; it has exactly one authoritative `<!-- pce:delivery -->` comment whose claims match merged PRs and target-branch evidence; it has exactly one `<!-- pce:landed -->` outcome comment linking its Program; and its canonical URL appears exactly once in that Map's landed-outcomes index and nowhere in open Efforts or Frontier. Closed alone never means landed. Cancelled, malformed, prematurely closed, duplicate-record, and conflicting-record Efforts fail this predicate.

For Effort delivery, apply the [delivery blocker rules](effort-implementation.md#delivery-blockers) before delivery or merge; report blockers that prevent safe progress.

## Plan the outcome

After input validation and reconstruction succeed, plan only the remaining outcome:

1. Investigate before asking. Decide reversible technical details from repository evidence and established engineering practice.
2. Ask the human only about missing intent, priorities, outcome-level trade-offs, credentials, legal or organizational authority, or permission for an exceptional irreversible external act. Explain consequences and recommend an answer.
3. Choose one PR or several coherent vertical slices. Do not split work only by technical layer.

## Execute

### Product naming boundary

Every implementation owner must receive this complete naming boundary in its delegated context before designing or editing production code. The owner must read the complete vision and inspect the repository's existing architecture and vocabulary before choosing names or boundaries. Production identifiers and architectural boundaries must use established repository and domain vocabulary and express the component's stable responsibility. Do not substitute vague generic names such as `Manager`, `Runner`, or `Handler` for a clear domain capability.

Delivery identity must not determine production modules, packages, types, functions, commands, routes, services, runtime schemas, user-facing configuration keys, public APIs, or other maintained product architecture. Removing a ticket number is not enough when the renamed component remains organized around an Effort, ticket title, vision, or implementation phase rather than a stable product responsibility.

Do not introduce or expand ticket-shaped product architecture. Repair inherited ticket-shaped components that the implementation directly modifies or depends on, and report unrelated occurrences without turning the vision into repository-wide cleanup. Preserve compatibility for existing public interfaces, or use an explicit migration when a repair changes one.

Ticket identity remains valid delivery metadata and traceability evidence in issues, vision provenance, PR descriptions, delivery records, branch and worktree names, commit messages, and historical evidence. It may also appear where traceability requires it in test names, fixtures, examples, or study-specific data configuration, although maintained artifacts should prefer behavioral or domain names where practical. Carry a necessary reference as explicit metadata or a comment rather than making it the artifact's organizing name.

Enforce this boundary through implementation and review. Do not add a repository-wide naming linter, impose a universal naming convention on downstream repositories, or perform unrelated cleanup. Do not rename PCE's Program or Effort workflow artifacts.

Each implementation owner owns one branch, its tests, commit, push, and PR. Each branch must start from the intended target branch.

Preserve unrelated user changes and local state. An implementation agent must provide the full diff and validation evidence for review.

Before authoring GitHub issues, PR bodies, or delivery and landing summaries, load and follow only the [GitHub writing skill](../github-writing/SKILL.md), not the publication procedure.

For every Effort implementation PR, apply the [PR reference rules](effort-implementation.md#implementation-prs).

### Test-first development

Before implementing or reviewing code or tests, load and follow the [test-first development skill](../test-first-development/SKILL.md).

## Review and land

Assign every PR to a fresh reviewer that did not implement it. The reviewer reads the full vision, repository rules, complete diff, and validation evidence. It checks real behavior, scope, safety, regressions, and consistency with the whole vision, including whether the change fits the repository without unnecessary behavior or complexity.

Review tests as design evidence, not a passing count. For each meaningful new case, check its promise, independent expectation, whether it reaches the intended rule, and whether a plausible broken implementation could still pass. Challenge duplicate coverage and implementation-shaped assertions. When coverage changes or is removed, verify that its useful guarantee remains protected or is intentionally no longer required. Large PRs require a necessity review: what can be removed without weakening the agreed outcome? This is not a minimal-patch rule or a reason to reject justified redesign.

The reviewer must evaluate names and architectural boundaries in the context of the complete repository and vision, not merely search for a particular ticket-number pattern. Reject:

- newly introduced or expanded ticket-derived production identifiers;
- inherited ticket-shaped architecture that the implementation extends;
- Effort-shaped abstractions after cosmetic renames remove the literal delivery identifier;
- vague generic APIs that conceal rather than express a stable capability;
- compatibility breaks to an existing public interface without an explicit migration.

Return findings to the implementation owner for repair. Repeat review after material repairs. A reviewer may merge only when findings are resolved and required checks pass. Follow any explicit instruction that withholds merge authority.

After merge, apply the [checkout and preservation policy](checkout-preservation.md) for branch removal and the final checkout audit.

Ordinary implementation branches, PRs, repairs, and merges are authorized by the act of handing over the vision. Deployments, destructive data operations, spending, credentials, external publication, infrastructure changes, and other irreversible external acts not clearly authorized by the vision still require human authority.

After all approved PRs land, inspect the resulting target branch against the complete vision, run its local tests, and report the merged changes and evidence.

For an Effort-derived vision, apply the [final delivery record rules](effort-implementation.md#final-delivery-record), reconcile the single authoritative record, and verify it before reporting completion. Leave the Effort open for `land-ticket`.

Report the vision outcome as complete only after final verification and, for an Effort, the delivery record is verified. Nothing outside the vision, Git, and GitHub evidence is the durable truth of the workflow. Reconstruct from that evidence on a later invocation.
