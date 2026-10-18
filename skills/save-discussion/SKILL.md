---
name: save-discussion
description: Save an unfinished grill or design discussion to a GitHub issue, or branch an experiment issue from it. Use when the human says "let's save this" or authorizes an experiment.
---

# Save Discussion

Preserve useful understanding so a later `grill-me <issue>` can continue without the human carrying context between agents. Saving does not require finished discovery, implementation requirements, or a vision, and it publishes or implements nothing else.

## Choose the record

Append to the issue already associated with the discussion. Otherwise create a normal GitHub issue in the verified current repository, or the repository the human names, with the `discussion` label; create the label if the repository lacks it and permissions allow. If another user is assigned or owns the discussion, resolve that before writing rather than taking over. An explicit draft-only request permits no GitHub mutation. Use ordinary open/closed state, not status labels. These are GitHub issues, not GitHub Discussions. Do not migrate historical issues or labels.

Program membership adds shared context, not another protocol. Link a related Program when one exists; a standalone discussion needs none. Put Program-level findings on the Program and issue-specific findings on the issue. Link shared evidence instead of duplicating it. Respect repository permissions and privacy, especially when linking across repositories.

## Write for the next agent

Before writing GitHub text, follow [github-writing](../github-writing/SKILL.md). Write primarily for an agent continuing the work while keeping it readable for the human. Use what matters for this discussion: purpose, useful shared understanding, settled decisions and reasons, uncertainties, supporting evidence, and the next useful continuation. Separate settled understanding from open questions. This is neither a transcript nor a mandatory template, and incomplete understanding should read as a valid place to resume, not a specification to finish immediately.

Comments append context. Never delete or silently rewrite earlier understanding. When a decision changes, say what changed, why, and which earlier decision it supersedes. A concise synthesis comment can help a long discussion resume without replacing its history. Edit an existing body only for ordinary link maintenance that preserves its content.

Once saving is authorized, keep recording substantive new context on the same issue without the human asking again: material decisions, superseded decisions, and findings. Skip per-turn logs and comments with nothing new. Read back what you wrote and report the issue URL or the precise failure.

## Branch an experiment

When missing evidence blocks a decision and the human authorizes an experiment, record the evidence need on the originating discussion, create a linked experiment issue, and link it back from the originating discussion. Create the experiment issue with the `discussion` label in the repository where the work belongs. Give it enough context that its agent need not ask the human to reconstruct the reasoning:

- the decision requiring evidence and why it matters;
- agreed requirements, relevant findings, and uncertainties;
- a link to the originating discussion and the evidence to return there.

For example: choose a physical data layout under agreed access patterns, memory limits, and preservation requirements. Do not prescribe the methodology or a preferred winner; the human grills the experiment issue with `grill-me <issue>` to decide how to answer it. Use ordinary links, not a ledger or metadata schema. Several experiments can inform one discussion without changing its identity.
