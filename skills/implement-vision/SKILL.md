---
name: implement-vision
description: Execute a standalone or Effort-derived vision as the root implementing agent using planning, delegation, implementation PRs, and independent review, reconstructing progress from Git and GitHub evidence on every invocation. Use when the user hands an agent a vision to implement.
---

# Implement Vision

Act as the root owner of the vision-level outcome. Treat the supplied vision as the durable product intent, not as a prescribed code mechanism.

This skill relies only on capabilities every supported environment provides: spawning subagents, running commands, reading and writing files, and using Git and GitHub. Delegation and a fresh independent reviewer are therefore always available.

## Resolve the input and target

Accept exactly one of these inputs:

- an Effort issue number in the current repository;
- a canonical Effort issue URL, in which case derive the repository from the URL and do not use the caller's current repository identity;
- a repository-relative `planning/visions/` path.

Reject missing, extra, ambiguous, shorthand, and noncanonical issue identities. Resolve a number only after identifying the current repository with GitHub. A canonical URL may name a repository different from the caller's current checkout; resolve and operate on the canonical owner and repository encoded in that URL rather than rejecting it as cross-repository. Locate an existing checkout only after verifying that its normalized canonical remote matches that repository. If none exists, create a durable checkout of the URL-derived repository in a user-owned location. Never read, branch, create a worktree, or implement URL-derived work in an unrelated caller checkout.

For a path input, first identify the current checkout's canonical GitHub repository. The path must remain inside `planning/visions/`, name a regular file rather than a symlink, and have one canonical repository-relative identity. Read it only after those checks, then scan its provenance before classifying it. A path input is standalone only when it has no `Effort:` provenance line. If it has exactly one canonical `Effort:` line, derive the canonical repository and Effort identity from that URL, require its repository to match the normalized canonical remote of the checkout containing the path, and promote the path to that Effort identity. From that point it must run the same complete Effort validation as a number or canonical URL and never continue as standalone. The supplied path remains only a locator; it is not authoritative and does not bypass the ticket or its `Vision:` link.

For all three Effort entry forms, normalize to one canonical repository and Effort URL, load the ticket, and follow its single `Vision:` link. Do not require a person to repeat identity already recovered from valid provenance or to recover a session identifier, worktree path, branch name, or prior conversation.

Determine one intended target branch from repository policy and durable PR evidence, normally the repository default branch, and use that identity for publication, implementation PRs, effect checks, delivery records, and cleanup. Stop if the evidence conflicts. Fetch the intended target branch before treating any intent or implementation as durable.

## Validate the resolved input

Complete input validation and the target-branch durability gate below before any planning, delegation, branch creation, or substantive implementation. Read the entire candidate vision, repository instructions, relevant code, tests, and recent project context. Stop on missing, duplicated, malformed, foreign, ambiguous, or conflicting provenance or durable linkage. Give one precise explanation. Do not plan, delegate, or create any work, and end the turn normally without a continuation loop. Do not ask the human to supply an identity that the rejected evidence cannot establish safely.

For every invocation, fetch the intended target branch and read the vision from that fetched ref. Verify that the canonical path is a regular file tracked on the intended target branch. Compare the relevant local or commit-pinned document with the target copy and require exact content before using it as accepted intent.

### Target-branch durability gate

An Effort-derived vision must already satisfy the authoring publication contract through `grill-ticket` or a `to-vision` invocation that applies the same complete Effort validation and linkage safeguards. Verify that the ticket's one `Vision:` link, its commit-pinned copy, and the target-branch copy identify the same regular `planning/visions/` file with exactly matching `Program:` and `Effort:` provenance and exact content. Do not bootstrap publication for an Effort. On any missing, unmerged, stale, or conflicting target copy, stop before planning or substantive implementation and report the publication gap.

A standalone target-branch vision is a normal start or resume only when its exact regular-file copy is already published and verified. Do not publish drafts or bootstrap publication, even on a first run. On missing, unmerged, stale, or conflicting publication, stop before planning, delegation, branch creation, or substantive implementation; preserve and report the inspectable file, branch, commit, PR, and target state. Direct the authoring workflow to `to-vision` to publish and verify that same path before a later implementation invocation. Explicit draft-only output is not implementation-ready. Never manufacture Program or Effort provenance for standalone work.

### Effort provenance and durable linkage

Scan provenance before classifying any candidate vision, including the ticket-linked and target-branch copies. With no `Effort:` line, a path input remains standalone and follows the standalone workflow unchanged. Any vision reached through an Effort identity must contain exactly one canonical `Effort:` line and exactly one canonical `Program:` line. More than one line, a malformed or noncanonical URL, a foreign repository, a missing line where Effort provenance is required, or disagreement among copies is invalid rather than standalone.

For an Effort-derived vision, use `gh`, the repository, and those links to reconstruct the Effort, Program Map, dependencies, comments, delivery records, linked PRs, target-branch state, and validation. Validate that the open issue has the `pce:effort` label and `<!-- pce:effort -->` marker; its body has exactly one Program, dependency, and Vision declaration; the Map contains exactly one membership for it; and the issue, Map, repository, linked vision path, supplied path when present, and both provenance URLs all match. Stop on duplicate, missing, noncanonical, conflicting, or mismatched provenance or durable linkage rather than attaching delivery to the wrong ticket.

Before planning or delivery, parse the complete Program dependency graph. Require exactly one unambiguous `Depends on:` declaration on every involved Effort. Every dependency must be a structurally valid Effort in the same Program, and the graph must be acyclic. Stop on duplicates, conflicts, foreign-Program dependencies, missing membership, malformed references, or cycles; do not choose an interpretation.

Only after every applicable input, provenance, repository, ticket, Program Map, vision-link, commit-pinned-content, target-copy, and dependency check succeeds is the input valid. A number, URL, and recovered path then converge on the same validated Effort workflow.

## Automatic semantic decisions

Only after the complete input-validation gate succeeds, load this skill's
`semantic-decisions.md`. Follow its shared permission, invocation, and fallback
contract automatically at these checkpoints, for standalone and Effort work:

- During investigation, reconstruction of resumed work, and delegation preparation,
  batch `context` comparisons of gathered candidates against individual requirements.
  Keep mandatory instructions, the complete vision, gate evidence, and uncertain
  or conflicting sources available regardless of relevance scores. Delegates
  receive inspected source context, not model labels as a substitute.
- Before assigning the fresh independent reviewer, batch `review` comparisons of
  coherent changes against requirements and repository vocabulary. Use all five
  dimensions to supplement review focus, never to narrow the full remit.
- During validation, before completion claims, and before preparing or updating
  an Effort delivery record, batch `evidence` comparisons of individual requirements
  with candidate tests, source, and observed results. Investigate gaps and run
  missing validation before claiming success. Include current target evidence in
  the final audit; semantic support never verifies a merge or target effect.

Use `scripts/semantic_decisions.py` through the guide's stdin contract. No separate
user invocation or routine approval prompt is needed. A fallback returns work to
the reasoning agent with every existing gate intact. Jev cannot approve publication,
merge, delivery, or landing. Keep compact details and actual resulting actions
inspectable in the active session, not a replacement durable record.

## Plan the outcome

After input validation succeeds:

1. Own the vision-level outcome for the whole invocation. Work within the active turn, resuming as each delegated subagent completes. When human input is required, stop cleanly, explain exactly what is needed, and end the turn; a person re-invokes this skill to continue, and the durable evidence below carries the progress.
2. Investigate before asking. Decide reversible technical details from repository evidence and established engineering practice.
3. Ask the human only about missing intent, priorities, outcome-level trade-offs, credentials, legal or organizational authority, or permission for an exceptional irreversible external act. Explain consequences and recommend an answer.
4. Choose one PR or several coherent vertical slices. Do not split work only by technical layer.

## Reconstruct every run

Reconstruct prior work before planning, including after an apparent clean start. For an Effort, inspect the linked vision, Program Map, complete dependency graph, issue timeline and comments, authoritative delivery record, implementation PRs, target-branch commits and effects, branches, validation evidence, and structured local worktrees. For a standalone vision, inspect the fetched target-branch vision, related commits, branches and PRs discoverable from repository and GitHub evidence, target effects, validation evidence, and structured local worktrees. Reconstruction must work without the prior agent session or its conversation.

Classify every relevant unit of prior work into exactly the applicable five-way account: **merged** work whose target effects are verified; **open** work in an active PR or durable pushed branch; **abandoned** work whose branch or PR no longer provides a viable delivery path; **incomplete** surviving work, including local committed, uncommitted, or unpushed evidence; and **remaining** vision outcome not delivered by any of the preceding evidence. Record overlaps explicitly rather than treating a branch or PR status as proof of an effect. Verify the actual target-branch result, then plan and execute only the remaining outcome.

When the entire outcome is already complete, rerun current validation, verify target effects and, for an Effort, require its existing authoritative delivery record to be valid before classifying the workflow as complete. A missing or stale required record is remaining reconciliation work under the delivery rules below. Report a truly already complete invocation as a no-op. For this no-op, do not create a branch, commit, worktree, PR, or delivery record. A stopped earlier agent, changed PR split, absent worktree, or missing status comment is not a reason to restart completed work or regrill. Regrill only when implementation evidence exposes a material flaw, missing outcome, or obsolete assumption in the vision.

Use this predicate everywhere blocker state is computed: an Effort is `landed` only when all four facts are verified: the Effort is closed; it has exactly one authoritative `<!-- pce:delivery -->` comment whose claims match merged PRs and target-branch evidence; it has exactly one `<!-- pce:landed -->` outcome comment linking its Program; and its canonical URL appears exactly once in that Map's landed-outcomes index and nowhere in open Efforts or Frontier. Closed alone never means landed. Cancelled, malformed, prematurely closed, duplicate-record, and conflicting-record Efforts fail this predicate.

Treat recorded dependencies as delivery blockers. Use ordinary tracker and engineering judgment to determine which investigation or implementation can proceed, but do not deliver or merge an outcome that relies on a blocker that fails the `landed` predicate. Report any blocker that prevents safe progress.

## Worktree and preservation policy

A branch does not require a separate worktree. Use the current checkout when safe. Place every checkout any agent in the invocation creates, for any purpose, below `<repository>/.worktrees/`. This covers implementation, independent review, audit, red or failing reproduction, side-by-side comparison, and any other purpose, including git worktrees, clones, and plain copies of the tree. Reviewers and other delegated agents must receive this policy in their delegated context before creating a checkout. A single repository-owned hierarchy keeps all disposable checkouts visible for inspection and cleanup.

The initial canonical clone, created when no local checkout exists, is exempt from managed placement and disposable-checkout cleanup. It may live in a durable user-owned location and remain as the repository root. All additional agent-created checkouts live below that root's `.worktrees/` hierarchy. Report a canonical clone created by this invocation in the final enumeration with its retained role.

Keep the existing vision worktree layout:

```text
<repository>/.worktrees/visions/effort-<number>-<slug>/
```

For standalone work, use an equally descriptive child of `<repository>/.worktrees/visions/` without an invented Effort number. Non-vision checkouts use equally descriptive paths below `<repository>/.worktrees/`. Apart from the initial canonical clone exception, never put an agent-created checkout in `/private/tmp`, another temporary directory, or arbitrary sibling directories. A disposable checkout is not a recovery database or the canonical home of the vision.

Build output, dependency caches, compiled binaries, and other regenerable artifacts are never evidence. Preserve logs, receipts, diffs, and patches; never copy or retain a build directory to prove a result. The preservation rules concern source and history, not build output.

During the final target-branch audit, enumerate every checkout the invocation created, in every location, including those created by delegated agents. For each checkout, inspect status, branch reachability, upstream state, target merge evidence, and ownership. For plain copies without Git metadata, establish source provenance and compare source changes against the intended target; if this cannot be proven, preserve and report the uncertainty rather than assuming the copy is merged. After the associated publication or implementation is verified merged, remove its clean disposable checkout and fully merged local branch. Remove a remote implementation branch only under the existing post-merge rule below. Preserve and report any checkout or branch with uncommitted, unpushed, unmerged, conflicting, or uncertain ownership evidence. Never move, delete, reset, or clean such evidence merely because another PR delivered the outcome. Cleanup is allowed only for verified merged work with no unique source changes and no ownership ambiguity. Remove only checkouts this invocation created; never remove a checkout another run or a human created, even if merged. Report every retained checkout and its reason, including the canonical root exception.

## Execute

### Product naming boundary

Every implementation owner must receive this complete naming boundary in its delegated context before designing or editing production code. The owner must read the complete vision and inspect the repository's existing architecture and vocabulary before choosing names or boundaries. Production identifiers and architectural boundaries must use established repository and domain vocabulary and express the component's stable responsibility. Do not substitute vague generic names such as `Manager`, `Runner`, or `Handler` for a clear domain capability.

Delivery identity must not determine production modules, packages, types, functions, commands, routes, services, runtime schemas, user-facing configuration keys, public APIs, or other maintained product architecture. Removing a ticket number is not enough when the renamed component remains organized around an Effort, ticket title, vision, or implementation phase rather than a stable product responsibility.

Do not introduce or expand ticket-shaped product architecture. Repair inherited ticket-shaped components that the implementation directly modifies or depends on, and report unrelated occurrences without turning the vision into repository-wide cleanup. Preserve compatibility for existing public interfaces, or use an explicit migration when a repair changes one.

Ticket identity remains valid delivery metadata and traceability evidence in issues, vision provenance, PR descriptions, delivery records, branch and worktree names, commit messages, and historical evidence. It may also appear where traceability requires it in test names, fixtures, examples, or study-specific data configuration, although maintained artifacts should prefer behavioral or domain names where practical. Carry a necessary reference as explicit metadata or a comment rather than making it the artifact's organizing name.

Enforce this boundary through implementation and review. Do not add a repository-wide naming linter, impose a universal naming convention on downstream repositories, or perform unrelated cleanup. Do not rename PCE's Program or Effort workflow artifacts.

Delegate substantive work when it improves speed or independence. Give each implementation agent complete context and ownership of one branch, its tests, commit, push, and PR. Each branch must start from the intended target branch. Keep the human informed at meaningful milestones without forwarding routine mechanism choices.

Use repository-native tools and tests. Preserve unrelated user changes and local state. An implementation agent must provide the full diff and validation evidence for review.

For an Effort-derived vision, every implementation PR body must include `Effort: <canonical GitHub issue URL>` as a plain reference. Do not use `close`, `closes`, `closed`, `fix`, `fixes`, `fixed`, `resolve`, `resolves`, or `resolved` with the Effort reference. An individual PR must never close the Effort, including when one PR happens to deliver the full vision. Keep detailed implementation and validation evidence on the PRs and Effort rather than expanding the Program Map.

## Review and land

Assign every PR to a fresh reviewer that did not implement it. The reviewer reads the full vision, repository rules, complete diff, and validation evidence. It checks real behavior, scope, safety, regressions, and consistency with the whole vision.

The reviewer must evaluate names and architectural boundaries in the context of the complete repository and vision, not merely search for a particular ticket-number pattern. Reject:

- newly introduced or expanded ticket-derived production identifiers;
- inherited ticket-shaped architecture that the implementation extends;
- Effort-shaped abstractions after cosmetic renames remove the literal delivery identifier;
- vague generic APIs that conceal rather than express a stable capability;
- compatibility breaks to an existing public interface without an explicit migration.

Return findings to the implementation owner for repair. Repeat review after material repairs. A reviewer may merge only when findings are resolved and required checks pass. Follow any explicit instruction that withholds merge authority.

After a confirmed merge, delete the merged remote implementation branch. During the final target-branch audit, remove local implementation branches only after proving that each branch is fully merged and is not checked out in a worktree. Preserve any branch with unmerged commits or uncertain ownership, and report why it remains.

Ordinary implementation branches, PRs, repairs, and merges are authorized by the act of handing over the vision. Deployments, destructive data operations, spending, credentials, external publication, infrastructure changes, and other irreversible external acts not clearly authorized by the vision still require human authority.

After all approved PRs land, inspect the resulting target branch against the complete vision, run its local tests, and report the merged changes and evidence.

For an Effort-derived vision, then reload every Effort comment. The sole authoritative delivery record is exactly one comment marked `<!-- pce:delivery -->`. If none exists, create one. If exactly one exists, reconcile its claims with current target-branch and GitHub evidence and update that same comment in place on rerun. If more than one exists, or its identity conflicts with the linked vision, Program, PRs, or target branch, stop and report the conflict without posting another record or completing the tracked outcome. The authoritative comment must contain:

- the delivered outcome;
- canonical URLs for every merged implementation PR;
- validation evidence, including commands and results;
- material deviations from the vision, or `None`;
- unresolved follow-up risks, or `None`.

Verify every PR is merged into the intended target, the reported effects exist on that branch, validation is current, exactly one delivery marker remains, and the updated durable comment can be read back. Deterministic reruns update or preserve that one comment; they never append a competing record. Report partial GitHub failure precisely and do not invent a delivery record. Leave the Effort open for `land-ticket`; implementation completion does not authorize its closure, a `<!-- pce:landed -->` record, or the Program Map mutation.

Report the vision outcome as complete only after final verification and, for an Effort, the delivery record is verified. Use the environment's own planning and delegation facilities rather than inventing replacement planning, scheduling, recovery, review, or merge machinery. Nothing outside the vision, Git, and GitHub evidence is the durable truth of the workflow: a lost session resumes when a person re-invokes this skill and it reconstructs from that evidence.
