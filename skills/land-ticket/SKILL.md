---
name: land-ticket
description: Verify and land one delivered Effort and update its Program Map. Use for /land-ticket with an Effort issue number or URL.
---

# Land Ticket

`$ARGUMENTS` must identify exactly one Effort issue by number or URL. Ask only for missing identity. Never assume one active Program.

Before authoring GitHub issues, PR bodies, or delivery and landing summaries, load and follow only the [GitHub writing skill](../github-writing/SKILL.md), not the publication procedure.

## Reconstruct evidence

From the repository root, inspect the explicit ticket, comments and timeline, its single linked vision, Program Map, dependencies, implementation PRs, target-branch commits and files, checks and validation, delivery record, and related Efforts. Validate the `pce:effort` / `<!-- pce:effort -->` and `pce:program` / `<!-- pce:program -->` contracts. Require exactly one Program, dependency, and Vision declaration on the ticket; exactly one Map membership; and exactly one canonical `Program:` line plus exactly one canonical `Effort:` line in the vision. All issue, Map, repository, and vision identities must match. Stop on duplicates, missing lines, foreign Programs, noncanonical URLs, or mismatches.

Parse the complete Program dependency graph before landing. Every involved Effort must have exactly one unambiguous `Depends on:` declaration naming only structurally valid Efforts in the same Program, and the graph must be acyclic. Stop on duplicates, conflicts, malformed references, missing membership, foreign-Program dependencies, or cycles.

Do not ask the human to explain code, PRs, tests, technical decisions, status, or any fact available through the repository or GitHub. Questions are only for genuine intent, priority, scope, outcome decisions, contradictions, or external authority.

The sole authoritative implementation record is exactly one Effort comment marked `<!-- pce:delivery -->`. It concisely records the delivered outcome, every merged PR URL, validation evidence, material deviations, and unresolved follow-up risks. Verify its claims against the target branch and GitHub rather than trusting the comment alone. If no marker exists but complete inspectable evidence permits safe mechanical reconstruction, create one and read it back. If exactly one exists, reconcile or update that same comment in place. If multiple markers or conflicting identities exist, stop without landing; never select one or append another. PRs must reference the Effort but must not have closed it before landing.

Use this predicate everywhere Frontier or blocker state is computed: an Effort is `landed` only when all four facts are verified: the Effort is closed; it has exactly one authoritative `<!-- pce:delivery -->` comment whose claims match merged PRs and target-branch evidence; it has exactly one `<!-- pce:landed -->` outcome comment linking its Program; and its canonical URL appears exactly once in that Map's landed-outcomes index and nowhere in open Efforts or Frontier. Closed alone never means landed. Cancelled, malformed, prematurely closed, duplicate-record, and conflicting-record Efforts fail this predicate.

## Requirement-to-evidence checks

Complete identity and dependency validation first. Match each requirement to candidate tests, source, and observed results before accepting delivery claims, repairing a delivery record, or landing. Investigate missing, indirect, unsupported, uncertain, or contradictory evidence; run required validation and inspect source and target effects. Assertions alone do not prove delivery. Verify merge status, exact content equality, provenance, dependency acyclicity, permissions, and landing authority from Git/GitHub evidence and the applicable gates. Keep the sole delivery record; do not create a second record for this reasoning.

## Recovery hierarchy

Apply the first matching result:

1. When delivery is complete but deterministic links or summaries are missing, repair only facts proven by inspectable evidence, verify them, and continue.
2. When implementation is incomplete and the vision remains valid, leave the Effort open and recommend resuming `implement-vision` with the same vision.
3. When implementation reveals a flawed outcome, missing requirement, or obsolete assumption in the vision, leave the Effort open and recommend rerunning `grill-ticket` on this same ticket and vision.
4. Never require regrilling only because an agent stopped, a PR split changed, or a comment was omitted.

If evidence is ambiguous or any delivery blocker fails the `landed` predicate, do not close the Effort. Report the exact gap. Do not manufacture delivery or landing evidence.

## Target-copy and worktree closure gate

Before landing, fetch the intended target branch and read the linked vision from that fetched ref. Require one regular `planning/visions/` file tracked on the target branch, never a symlink or worktree-only path. Compare its exact matching provenance and content with the ticket's commit-pinned `Vision:` link. Verify that its single `Program:` and `Effort:` lines match the Map, ticket, repository, and canonical URLs. A pushed branch, open PR, local checkout, or delivery comment is not a substitute for the target-branch copy. Do not close the Effort when this check fails.

Enumerate every checkout under `<repository>/.worktrees/` that relates to the Effort, including canonical Effort worktrees below `<repository>/.worktrees/visions/effort-<number>-*/`, and relate them to publication and implementation branches. Include git worktrees, clones, and plain copies used for review, audit, reproduction, and comparison, or any other purpose. Inspect status, branch reachability, upstream state, target merge evidence, and ownership for each checkout. For plain copies without Git metadata, establish source provenance and compare source changes against the intended target; preserve and report any uncertainty.

Removal applies only to checkouts this invocation created. Never remove a checkout another run or a human created, even if merged; report it without treating it as authorized cleanup. Remove only safely removable clean disposable checkouts whose branch and effects are verified merged into the intended target and have no unique commits or source files; then remove only fully merged local branches that are no longer checked out. For incomplete evidence, preserve and report checkouts or branches with uncommitted, unpushed, unmerged, conflicting, or uncertain evidence. Do not close the Effort while a safely removable checkout owned by this invocation remains, or while an incomplete checkout exposes unresolved delivery state. Absence of a checkout is valid because durable reconstruction comes from Git and GitHub.

Build output, dependency caches, compiled binaries, and other regenerable artifacts are never evidence and are never a reason to preserve a checkout. Preserve logs, receipts, diffs, and patches, not build directories. These preservation rules protect source and history, not build output. Report every checkout this invocation created, in every location, and its removal or retention reason at the end. If this invocation created the initial canonical clone when no local checkout existed, report its retained repository-root role; it is exempt from managed placement and disposable-checkout cleanup.

## Land and update the Map

Once the complete vision and sole delivery record are verified on the target branch, create or update exactly one concise outcome comment marked `<!-- pce:landed -->` that links the Program and states the landed outcome. Add or update exactly one linked outcome-level Map line for the Effort, remove it from open Efforts and Frontier, close the Effort, and recompute Frontier using the `landed` predicate for every dependency. Perform read-back verification of the comment, Map, and issue state. On a partial rerun, reconcile the one existing landing comment and Map line from evidence instead of duplicating either. Stop on conflicting duplicate landing markers or Map entries.

Do not discover future work, interview about future work, or propose new Efforts. Investigate and report concrete delivery problems through the requirement-to-evidence checks and recovery hierarchy above. An explicitly requested `chart-program` re-survey remains the route for changing Program scope; landing does not authorize scope expansion.

When no open Efforts remain, verify delivery for all agreed outcomes before proposing completion. Do not propose completion with unresolved delivery gaps. Show a short completion summary with destination, linked landed outcomes, verified delivery, and exclusions. Ask once for authority to close the Program Map. Close it only after that explicit confirmation. Do not ask again if the answer is no; leave it open and report that state.

Report the Effort URL and closure, verified delivery evidence, Map change, recomputed Frontier, and Program status. Keep the report at outcome altitude and link detailed evidence.

## Legacy Program content

Legacy Fog text is not a prerequisite for charting, re-survey, landing, or proposing Program completion. Preserve unrelated issue content, including legacy text; do not silently delete it or convert it into tickets. Do not bulk-edit existing Programs. Only an explicitly requested `chart-program` re-survey can propose changes to agreed Program scope, subject to its approval and state gates.
