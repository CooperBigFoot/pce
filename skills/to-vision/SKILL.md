---
name: to-vision
description: Author, publish, and verify the current conversation's confirmed understanding as a durable vision, unless draft-only output is requested. Use when the user asks to capture, write, or convert the discussion into an implementation vision.
---

# To Vision

Turn the completed discussion into one published handoff for a fresh implementing agent that cannot see the conversation. Invoking `to-vision` authorizes publication of the confirmed vision, including the ordinary documentation PR merge, unless the user explicitly requests draft-only output. It does not authorize implementation or merging research or implementation PRs. `grill-me` confirmation alone does not invoke this workflow or authorize publication.

## Name and path

When `$ARGUMENTS` contains a useful human-readable name, use it unchanged as the vision name. When it is empty, derive a concise, descriptive name from the confirmed shared understanding. Ask for a name only when the conversation does not contain enough information to choose a meaningful one. Do not reopen the completed discussion merely to name the file.

If revising an existing vision, reuse that same regular `planning/visions/` path, including on a later invocation to publish a draft. Refuse symlinks, symlinked path components, and paths outside `planning/visions/`. For a new vision only, from the target repository root run the deterministic creation helper with the explicit or derived name:

```bash
python3 <path-to-this-skill>/scripts/create_vision.py "<vision name>"
```

The helper prints `planning/visions/YYYY-MM-DD-<slug>.md`. It creates an empty file only when the path does not exist. A repeated call returns the existing path without changing its content.

Read an existing file before editing it. Never silently replace prior content. Reconcile the current shared understanding with it and preserve still-valid information.

## Authoring contract

Choose the structure that best communicates this specific work. There is no required template or heading set.

Write enough context that the fresh agent can implement without the prior conversation. Where relevant, communicate:

- the desired outcome and why it matters;
- externally observable evidence of success;
- scope boundaries and explicit exclusions;
- constraints and settled decisions;
- important repository facts learned during the grill;
- material risks or genuine uncertainty that remains.

Translate intent into useful technical context, but leave reversible mechanisms to the implementing agent. Do not invent unresolved questions to fill a section. Do not reopen decisions settled during the grill.

## Draft-only output

When the user explicitly requests draft-only output, write or revise the local draft and perform no commit, push, PR, issue mutation, or merge. Report its path and concise summary. State that it is not yet verified on the target branch and is not implementation-ready. A later authoring invocation can publish the same path. Stop without an implementation handoff.

## Publish and verify

This is the shared publication contract for standalone and Effort authoring. `grill-ticket` retains its discovery and validation gates and follows this contract after confirmation.

1. Inspect contribution rules and establish one intended target branch from repository policy and durable GitHub evidence. Stop on conflicting target evidence. Create or reuse a dedicated vision branch from that target. Use the current checkout when safe. Follow repository worktree rules and the `implement-vision` Worktree and preservation policy for every checkout created, including by reviewers: all additional checkouts belong below `<repository>/.worktrees/`; preserve unrelated user state and incomplete or uncertain source and history. Build output is never evidence.
2. Review the staged diff and commit only the confirmed vision change. Account explicitly for ignored planning paths: inspect ignore rules and, when permitted, force-add only the exact vision path. Never stage unrelated files or include previously staged unrelated changes in the commit.
3. Push and verify the remote commit contains the same regular vision path with exact authored content. Record the commit SHA and canonical GitHub blob URL. A local commit alone is insufficient.
4. Open or reuse a documentation PR targeting the intended branch. Obtain independent review from an agent that did not author the change, address findings, and pass required checks. Never bypass protected-branch rules or required approvals. Use nonblocking progress updates during checks and reviews; resume when they finish rather than polling or holding the turn open.
5. Before issue mutation, scan the vision's provenance. With no Effort provenance, keep it standalone: no tickets or Program metadata are required, and never invent provenance. An Effort-derived path retains its identity and cannot bypass ticket validation. For Effort provenance, load and apply the complete `grill-ticket` Validate and claim state gate: compose its state validation, not its assignment action. Apply its ownership checks without claiming the issue. This includes open state, Program membership, dependencies, repository identity, and matching provenance; stop without issue mutation on malformed, duplicate, ambiguous, or conflicting state. Require exactly one canonical `Program:` line and exactly one canonical `Effort:` line matching the ticket and Map. Replace the unique `Vision: pending` declaration, or update the existing single vision link for a revision, with exactly one Markdown link displaying the repository-relative path and targeting the verified commit-pinned GitHub blob URL. Preserve unrelated issue content. Reload the issue and fetch linked content to verify both directions, exact content, and declaration uniqueness. Never create competing links, link unpushed content, or substitute an unverified target-branch URL.
6. Before merge, inspect publication PR descriptions and all publication commit messages for auto-closing syntax, including negated phrases. GitHub can treat a closing keyword and issue reference as an instruction despite negation. Avoid `close`, `closes`, `closed`, `fix`, `fixes`, `fixed`, `resolve`, `resolves`, or `resolved` paired with an Effort reference. Use neutral references such as `Related Effort: #225`. Check existing PR text and explicit closing relationships reported by GitHub, not just newly authored prose. Inspect the prospective exact merge or squash title and body before submitting the merge, including any default message that GitHub would generate. Use a merge mechanism that can pass that inspected payload explicitly, or otherwise prove the actual submitted payload matches it; if neither is possible, stop rather than merge with an uninspected default. Remove unintended closing references and relationships through permitted edits, then reload and verify they are absent before merge. If inspection or safe correction is unavailable, stop and report the blocker. A statement that closure is not intended is not a safeguard.
7. Merge the documentation PR through the normal permitted process only after review, checks, linkage where applicable, and closing-reference safeguards pass. Verify the PR's merge and intended base branch.
8. Fetch the intended target branch after merge and read the same regular `planning/visions/` file from the fetched ref. Require exact authored content, not merely the presence of a path or merge status. For Effort-derived visions, reverify matching Program/Effort metadata, the single commit-pinned issue link and both directions, Program membership, and that the issue remains open.

Vision publication must never mark an Effort delivered or landed or close it. `land-ticket` owns Effort closure after verified implementation delivery. Unexpected closure is a publication error, not evidence of delivery. Report it and preserve the PR, commit, linkage, and issue timeline evidence. Do not blindly reopen an issue whose closure may reflect unrelated legitimate work. Do not claim a successful handoff until the open state is restored and verified through permitted action; obtain authority when safe correction is not established.

A pushed commit or open PR is not completion. On any commit, push, review, check, linkage, merge, target-copy, or open-state failure, preserve inspectable partial state and report the precise blocker plus file, branch, commit, PR, linkage state, and target-copy gap. Keep a verified commit-pinned issue link when safe. State that the vision is not implementation-ready. Resume from verified Git and GitHub evidence, reusing the path, branch, PR, and single link rather than creating duplicates or discarding settled intent.

Only after all applicable checks pass, report the vision path, concise summary, publication PR, verified Git refs (pushed commit and fetched target SHA), and exact `implement-vision planning/visions/<vision>.md` handoff. For an Effort, also report its URL and verified linkage and open state, with `implement-vision <Effort number or canonical URL>` as the handoff. Do not start implementation automatically.
