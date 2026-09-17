---
name: land-ticket
description: Verify and land one delivered Program Effort, repair safe records, evolve Fog, and optionally complete its Program. Use for /land-ticket with an Effort issue URL or number.
---

# Land Ticket

`$ARGUMENTS` must identify exactly one Effort issue by number or URL. Ask only for missing identity. Never assume one active Program.

## Reconstruct evidence

From the repository root, inspect the explicit ticket, comments and timeline, its single linked vision, Program Map, dependencies, implementation PRs, target-branch commits and files, checks and validation, delivery record, related Efforts, and Fog. Validate the `pce:effort` / `<!-- pce:effort -->` and `pce:program` / `<!-- pce:program -->` contracts. Require exactly one Program, dependency, and Vision declaration on the ticket; exactly one Map membership; and exactly one canonical `Program:` line plus exactly one canonical `Effort:` line in the vision. All issue, Map, repository, and vision identities must match. Stop on duplicates, missing lines, foreign Programs, noncanonical URLs, or mismatches.

Parse the complete Program dependency graph before landing. Every involved Effort must have exactly one unambiguous `Depends on:` declaration naming only structurally valid Efforts in the same Program, and the graph must be acyclic. Stop on duplicates, conflicts, malformed references, missing membership, foreign-Program dependencies, or cycles.

Do not ask the human to explain code, PRs, tests, technical decisions, status, or any fact available through the repository or GitHub. Questions are only for genuine intent, priority, scope, outcome decisions, contradictions, or external authority.

The sole authoritative implementation record is exactly one Effort comment marked `<!-- pce:delivery -->`. It concisely records the delivered outcome, every merged PR URL, validation evidence, material deviations, and unresolved follow-up risks. Verify its claims against the target branch and GitHub rather than trusting the comment alone. If no marker exists but complete inspectable evidence permits safe mechanical reconstruction, create one and read it back. If exactly one exists, reconcile or update that same comment in place. If multiple markers or conflicting identities exist, stop without landing; never select one or append another. PRs must reference the Effort but must not have closed it before landing.

Use this predicate everywhere Frontier or blocker state is computed: an Effort is `landed` only when all four facts are verified: the Effort is closed; it has exactly one authoritative `<!-- pce:delivery -->` comment whose claims match merged PRs and target-branch evidence; it has exactly one `<!-- pce:landed -->` outcome comment linking its Program; and its canonical URL appears exactly once in that Map's landed-outcomes index and nowhere in open Efforts or Frontier. Closed alone never means landed. Cancelled, malformed, prematurely closed, duplicate-record, and conflicting-record Efforts fail this predicate.

## Automatic evidence matching

After deterministic identity and dependency validation, load
`semantic-decisions.md` from the resolved installed `implement-vision` directory.
Before accepting delivery claims, repairing a delivery record, or landing, batch
`evidence` comparisons through its `scripts/semantic_decisions.py`: one requirement
against candidate tests, source excerpts, and observed target-branch validation.
Follow the guide's sharing permission and quiet fallback contract automatically.
Inspect missing, indirect, unsupported, uncertain, or contradictory evidence on
the normal reasoning path before completion claims. A model label is not proof:
run required validation and inspect source and target effects. Never use Jev to
determine merge status, equality, provenance, dependency acyclicity, permissions,
or landing authority. All recovery, target-copy, worktree, and closure gates remain
mandatory, including on fallback. Keep compact decisions and actual actions
inspectable in session only, without a second delivery record.

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

## Land and evolve the Map

Once the complete vision and sole delivery record are verified on the target branch, create or update exactly one concise outcome comment marked `<!-- pce:landed -->` that links the Program and states the landed outcome. Add or update exactly one linked outcome-level Map line for the Effort, remove it from open Efforts and Frontier, close the Effort, and recompute Frontier using the `landed` predicate for every dependency. Perform read-back verification of the comment, Map, and issue state. On a partial rerun, reconcile the one existing landing comment and Map line from evidence instead of duplicating either. Stop on conflicting duplicate landing markers or Map entries.

Inspect the Program's Fog against the landed result. Classify from evidence where established practice settles it. Load and follow canonical PCE `grill-me`, rather than copying its question loop, only when newly visible territory requires genuine human intent, priority, scope, or outcome decisions. After confirmation, show one concise mutation proposal in the same intent-level form as `chart-program`. Newly sharp Fog may become proposed `pce:effort` tickets with the deterministic Program, dependency, and pending-Vision contracts. Retained uncertainty stays Fog. Obtain approval before those issue or Map mutations, then execute and verify them. Do not create speculative tickets.

When no open Efforts and no substantive Fog remain, show a short completion summary with destination, linked landed outcomes, and exclusions. Ask once for authority to close the Program Map. Close it only after that explicit confirmation. Do not ask again if the answer is no; leave it open and report that state.

Report the Effort URL and closure, verified delivery evidence, Map change, recomputed Frontier and Fog, any new approved Efforts, and Program status. Keep the report at outcome altitude and link detailed evidence.
