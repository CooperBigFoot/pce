# Checkout and preservation policy

A branch does not require a separate worktree. Use the current checkout when safe. Place every checkout any agent in the invocation creates, for any purpose, below `<repository>/.worktrees/`. This covers implementation, independent review, audit, red or failing reproduction, side-by-side comparison, and any other purpose, including git worktrees, clones, and plain copies of the tree. Reviewers and other delegated agents must receive this policy in their delegated context before creating a checkout. A single repository-owned hierarchy keeps all disposable checkouts visible for inspection and cleanup.

The initial canonical clone, created when no local checkout exists, is exempt from managed placement and disposable-checkout cleanup. It may live in a durable user-owned location and remain as the repository root. All additional agent-created checkouts live below that root's `.worktrees/` hierarchy. Report a canonical clone created by this invocation in the final enumeration with its retained role.

Keep the existing vision worktree layout:

```text
<repository>/.worktrees/visions/effort-<number>-<slug>/
```

For standalone work, use an equally descriptive child of `<repository>/.worktrees/visions/` without an invented Effort number. Non-vision checkouts use equally descriptive paths below `<repository>/.worktrees/`. Apart from the initial canonical clone exception, never put an agent-created checkout in `/private/tmp`, another temporary directory, or arbitrary sibling directories. A disposable checkout is not a recovery database or the canonical home of the vision.

Build output, dependency caches, compiled binaries, and other regenerable artifacts are never evidence. Preserve logs, receipts, diffs, and patches; never copy or retain a build directory to prove a result. The preservation rules concern source and history, not build output.

During the final target-branch audit, enumerate every checkout the invocation created, in every location, including those created by delegated agents. For each checkout, inspect status, branch reachability, upstream state, target merge evidence, and ownership. For plain copies without Git metadata, establish source provenance and compare source changes against the intended target; if this cannot be proven, preserve and report the uncertainty rather than assuming the copy is merged. After the associated publication or implementation is verified merged, remove its clean disposable checkout and fully merged local branch. Remove a remote implementation branch only under the existing post-merge rule in this reference. Preserve and report any checkout or branch with uncommitted, unpushed, unmerged, conflicting, or uncertain ownership evidence. Never move, delete, reset, or clean such evidence merely because another PR delivered the outcome. Cleanup is allowed only for verified merged work with no unique source changes and no ownership ambiguity. Remove only checkouts this invocation created; never remove a checkout another run or a human created, even if merged. Report every retained checkout and its reason, including the canonical root exception.

## Post-merge implementation branches

After a confirmed merge, delete the merged remote implementation branch. During the final target-branch audit, remove local implementation branches only after proving that each branch is fully merged and is not checked out in a worktree. Preserve any branch with unmerged commits or uncertain ownership, and report why it remains.
