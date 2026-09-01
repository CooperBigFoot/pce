# Recoverable vision implementation handoffs

## Outcome

A person can start or resume vision implementation after a laptop, agent, or session crash without searching temporary directories or reconstructing the prior conversation. The same `implement-vision` workflow handles a new run and a resumed run. Durable repository and GitHub evidence, rather than an agent session or local worktree, identifies the intended outcome and completed work.

The common tracked-work entry point is an Effort issue:

```text
implement-vision 129
implement-vision https://github.com/owner/repository/issues/129
```

A standalone vision remains directly addressable:

```text
implement-vision planning/visions/<vision>.md
```

No separate `recover-implementation` skill is introduced.

## Durable handoff contract

Every implementation vision has one canonical repository-relative path under `planning/visions/`. Before substantive implementation begins, that vision must be committed and merged into the intended target branch, normally `main`.

A first invocation may accept a new local standalone vision created by `to-vision`. In that case, `implement-vision` treats durable publication as its bootstrap step: it publishes the vision through the repository's normal branch and review process, verifies the merged target-branch copy, and only then begins substantive implementation. If publication cannot complete, it stops with the inspectable vision and publication state preserved. An Effort-derived vision must already have completed this publication contract through `grill-ticket` before implementation starts.

For an Effort input, `implement-vision` resolves the issue in the current repository when given a number and from the canonical repository in a full URL. It validates the existing Program and Effort contracts, follows the issue's single `Vision:` link, verifies matching `Program:` and `Effort:` provenance, and confirms that the same vision is present on the intended target branch. The ticket is the convenient human recovery handle; the target branch is the durable source of product intent.

For a standalone path, `implement-vision` verifies that the path is a regular `planning/visions/` file tracked on the intended target branch. Standalone work does not acquire synthetic Program or Effort metadata merely to support recovery.

The vision-authoring and ticket-grilling workflows must make the durability state explicit. They must not describe an unmerged or worktree-only document as ready for implementation. The workflow may use the repository's ordinary branch and review policy to publish the document, but the final handoff occurs only after the target-branch copy is verified.

## Start and resume are one operation

Every `implement-vision` invocation reconstructs relevant state before planning new work. For an Effort this includes the linked vision, Program Map, dependencies, issue comments, authoritative delivery record, implementation PRs, target-branch commits, branches, validation evidence, and any structured local worktrees that still exist. For a standalone vision it includes the target-branch vision, related branches and PRs discoverable from repository evidence, and current target-branch effects.

The workflow classifies prior work as merged, open, abandoned, incomplete, or remaining and proceeds only with the remaining outcome. A stopped agent, missing session, changed PR split, or absent local worktree is not a reason to restart completed work or repeat discovery. A rerun against an already completed vision verifies and reports completion without manufacturing more work.

Uncommitted changes that existed only on lost storage are outside this recovery guarantee. When a surviving local worktree contains uncommitted or unpushed work, the workflow preserves and reports it rather than silently deleting or treating it as durable evidence.

## Worktree policy

A Git branch does not imply a separate worktree. Vision publication may use the repository's current checkout when that is safe. When isolation is needed, PCE-created vision worktrees use a predictable repository-owned hierarchy:

```text
<repository>/.worktrees/visions/effort-<number>-<slug>/
```

A standalone vision that needs isolation uses an equally descriptive child of `<repository>/.worktrees/visions/` without inventing an Effort number. PCE must not place active vision worktrees in `/private/tmp` or arbitrary sibling directories.

Worktrees are ephemeral checkout mechanisms, not the home of a vision and not a recovery database. Once a vision publication or implementation branch is merged into its target and the merge is verified, its worktree and fully merged local branch are removed. Incomplete worktrees with unmerged or uncertain changes are preserved at their canonical location and reported. `land-ticket` performs a final verification that the canonical vision exists on the target branch and that no safely removable Effort worktrees remain before closing the Effort.

## Observable success

The revised skills make these behaviors explicit and consistent:

- From the owning repository, `implement-vision <Effort number>` deterministically finds the linked vision and resumes remaining work.
- `implement-vision <canonical Effort URL>` works without relying on the caller's current repository identity.
- `implement-vision planning/visions/<vision>.md` starts or resumes standalone work.
- A first invocation with a new local standalone vision publishes and verifies that vision before substantive implementation; a resumed invocation reports and stops on any unresolved publication gap.
- Reinvocation after a crash reconstructs merged and active work from durable evidence and does not repeat completed changes.
- No workflow requires a person to know an agent session identifier or search `/private/tmp` for a vision.
- Any PCE-created vision worktree has a predictable path below `.worktrees/visions/`.
- Successfully merged worktrees are removed; worktrees with unmerged evidence are retained and identified.
- An Effort cannot land unless its linked vision is verified on the target branch.

Repository tests should exercise issue-number and URL resolution rules, standalone-path validation, target-branch durability checks, rerun behavior described by the skill contracts, canonical worktree placement, and safe cleanup boundaries where deterministic helpers are introduced. Skill instructions must remain usable without adding a PCE runtime, generated state, or a new recovery state machine.

## Boundaries

This work changes the six-skill workflow and any focused standard-library helpers or tests needed to enforce it. It does not create a compiled application, background service, session database, general worktree manager, or forensic undelete tool. GitHub issues, Git refs, merged target-branch content, PRs, and existing delivery records remain the durable coordination surface.

The workflow does not promise recovery of changes that were never committed or pushed and were physically lost. Its guarantee is that accepted vision intent and all work claimed as durable or complete can be reconstructed without the prior agent session.
