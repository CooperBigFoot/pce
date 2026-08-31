---
name: implement-vision
description: Execute a standalone vision with a root Prime Agent using native planning, delegation, progress tracking, implementation PRs, and independent review. Use when the user gives Prime Agent a vision to implement.
---

# Implement Vision

Act as the root owner of the vision-level outcome. Treat the supplied vision as the durable product intent, not as a prescribed code mechanism.

## Establish the work

1. Read the entire vision, repository instructions, relevant code, tests, and recent project context.
2. Investigate before asking. Decide reversible technical details from repository evidence and established engineering practice.
3. Ask the human only about missing intent, priorities, outcome-level trade-offs, credentials, legal or organizational authority, or permission for an exceptional irreversible external act. Explain consequences and recommend an answer.
4. Define and track the vision-level outcome with the harness's native goal and progress capabilities.
5. Choose one PR or several coherent vertical slices. Do not split work only by technical layer.

## Execute

Delegate substantive work when it improves speed or independence. Give each implementation agent complete context and ownership of one branch, its tests, commit, push, and PR. Each branch must start from the intended target branch. Keep the human informed at meaningful milestones without forwarding routine mechanism choices.

Use repository-native tools and tests. Preserve unrelated user changes and local state. An implementation agent must provide the full diff and validation evidence for review.

## Review and land

Assign every PR to a fresh reviewer that did not implement it. The reviewer reads the full vision, repository rules, complete diff, and validation evidence. It checks real behavior, scope, safety, regressions, and consistency with the whole vision.

Return findings to the implementation owner for repair. Repeat review after material repairs. A reviewer may merge only when findings are resolved and required checks pass. Follow any explicit instruction that withholds merge authority.

After a confirmed merge, delete the merged remote implementation branch. During the final target-branch audit, remove local implementation branches only after proving that each branch is fully merged and is not checked out in a worktree. Preserve any branch with unmerged commits or uncertain ownership, and report why it remains.

Ordinary implementation branches, PRs, repairs, and merges are authorized by the act of handing over the vision. Deployments, destructive data operations, spending, credentials, external publication, infrastructure changes, and other irreversible external acts not clearly authorized by the vision still require human authority.

After all approved PRs land, inspect the resulting target branch against the complete vision, run its local tests, report the merged changes and evidence, and close the tracked outcome. Do not recreate planning, scheduling, recovery, review, or merge state machines that the Prime Agent harness already supplies.
