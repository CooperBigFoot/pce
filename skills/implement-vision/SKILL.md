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

## Detect Effort provenance

A vision is Effort-derived only when it contains an `Effort: <canonical GitHub issue URL>` provenance line. Standalone visions keep the workflow below unchanged.

For an Effort-derived vision, use `gh`, the repository, and the vision's `Program:` link to reconstruct the Effort, Program Map, dependencies, comments, delivery records, linked PRs, target-branch state, and validation. Validate that the open issue has the `pce:effort` label and `<!-- pce:effort -->` marker, that its unique Program and Vision links point back to this vision, and that the vision provenance agrees. Stop and report ambiguity rather than attaching delivery to the wrong ticket.

Reconstruct prior work before planning. Classify linked PRs and commits as merged, open, abandoned, or remaining, verify their actual target-branch effects, and continue only the remaining vision outcome. A stopped earlier agent, changed PR split, or missing status comment is not a reason to restart or regrill. Regrill is appropriate only when implementation evidence exposes a material flaw, missing outcome, or obsolete assumption in the vision.

Treat recorded dependencies as delivery blockers. Use ordinary tracker and engineering judgment to determine which investigation or implementation can proceed, but do not land an outcome that relies on an unlanded blocker. Report any blocker that prevents safe progress.

## Execute

Delegate substantive work when it improves speed or independence. Give each implementation agent complete context and ownership of one branch, its tests, commit, push, and PR. Each branch must start from the intended target branch. Keep the human informed at meaningful milestones without forwarding routine mechanism choices.

Use repository-native tools and tests. Preserve unrelated user changes and local state. An implementation agent must provide the full diff and validation evidence for review.

For an Effort-derived vision, every implementation PR body must include `Effort: <canonical GitHub issue URL>` as a plain reference. Do not use `close`, `closes`, `closed`, `fix`, `fixes`, `fixed`, `resolve`, `resolves`, or `resolved` with the Effort reference. An individual PR must never close the Effort, including when one PR happens to deliver the full vision. Keep detailed implementation and validation evidence on the PRs and Effort rather than expanding the Program Map.

## Review and land

Assign every PR to a fresh reviewer that did not implement it. The reviewer reads the full vision, repository rules, complete diff, and validation evidence. It checks real behavior, scope, safety, regressions, and consistency with the whole vision.

Return findings to the implementation owner for repair. Repeat review after material repairs. A reviewer may merge only when findings are resolved and required checks pass. Follow any explicit instruction that withholds merge authority.

After a confirmed merge, delete the merged remote implementation branch. During the final target-branch audit, remove local implementation branches only after proving that each branch is fully merged and is not checked out in a worktree. Preserve any branch with unmerged commits or uncertain ownership, and report why it remains.

Ordinary implementation branches, PRs, repairs, and merges are authorized by the act of handing over the vision. Deployments, destructive data operations, spending, credentials, external publication, infrastructure changes, and other irreversible external acts not clearly authorized by the vision still require human authority.

After all approved PRs land, inspect the resulting target branch against the complete vision, run its local tests, and report the merged changes and evidence.

For an Effort-derived vision, then reload the Effort and post or update one concise comment marked `<!-- pce:delivery -->`. It must contain:

- the delivered outcome;
- canonical URLs for every merged implementation PR;
- validation evidence, including commands and results;
- material deviations from the vision, or `None`;
- unresolved follow-up risks, or `None`.

Verify every PR is merged into the intended target, the reported effects exist on that branch, validation is current, and the durable comment can be read back. Report partial GitHub failure precisely and do not invent a delivery record. Leave the Effort open for `land-ticket`; implementation completion does not authorize its closure or the Program Map mutation.

Close the harness's tracked vision outcome only after final verification and, for an Effort, the delivery record is verified. Do not recreate planning, scheduling, recovery, review, or merge state machines that the Prime Agent harness already supplies.
