# Vision publication

Use after the `to-vision` authoring checks, not for draft-only output. This shared procedure publishes a normal vision whether or not it has an associated issue. It does not authorize implementation. Follow [github-writing](../github-writing/SKILL.md) for GitHub text.

## Publish the confirmed document

Establish the repository and intended target branch from repository policy and durable GitHub evidence; stop on conflicts. Follow the [checkout and preservation policy](../implement-vision/checkout-preservation.md) before checkout creation and pass it in full to delegates. Use the current checkout when safe, with a dedicated vision branch from the intended target. Preserve unrelated user state.

Review and commit only the confirmed vision change. Inspect ignore rules when planning paths are ignored; force-add only the exact authorized vision path if needed. Never include unrelated staged changes. Push and verify the remote commit has the same regular path and exact authored content.

Open or reuse a documentation PR against the intended target. Obtain independent review from an agent who did not author it, repair findings, and satisfy required checks and approvals. Do not bypass protected branches.

## Keep publication separate from delivery

Use neutral references to an associated issue, including an experiment's originating discussion. Before merging, inspect PR text, commit messages, GitHub closing relationships, and the exact prospective merge or squash title and body for auto-closing references. Negation does not make closing syntax safe. Remove unintended closing references through permitted edits and verify the correction. Submit the inspected merge payload explicitly or prove the actual payload matches it; otherwise stop. Publication must leave the issue open.

The authoring invocation authorizes the ordinary documentation merge, not research or implementation PR merges. Merge through the permitted process and verify the PR's target and merge result. Fetch the intended target branch and read the regular vision file there, requiring exact authored content rather than merely a path or merged status.

## Link and hand off

For an associated issue, reload it before mutation to confirm identity, ownership, and current context. Add or update an ordinary link to the verified target-branch vision, preserving unrelated content. Include a reciprocal issue link in the vision during authoring so path-based implementation can recover the association. No fixed metadata format, commit-pinned issue declaration, or Program membership validation is required. If links conflict, resolve the uncertainty rather than overwriting evidence or creating competing visions. Verify the link resolves to the intended content and that the issue remains open. Standalone visions need no issue or Program metadata. An experiment vision links its experiment issue, and the experiment issue links the verified vision.

Unexpected issue closure is not proof of delivery. Report it and preserve timeline evidence; do not blindly reopen an issue that may have been legitimately completed elsewhere. Resolve the state through permitted action before claiming an open-issue handoff.

Ensure the implementation checkout has the same canonical regular vision path and exact fetched target content. Restore it only when safe, without overwriting local changes or staged state. If blocked, preserve the publication checkout and report “published, but local handoff blocked.” Apply the preservation policy to verified merged publication work.

On any failure, report the precise blocker and available path, branch, commit, PR, issue link, and target-copy state. Keep inspectable partial work and resume the same path and PR rather than duplicating them. Do not call an unverified result implementation-ready.

After verification, report the vision path, concise summary, publication PR, pushed commit and fetched target SHA, implementation checkout, and any associated issue URL. Hand off with `implement-vision planning/visions/<vision>.md` or `implement-vision <issue number or URL>`. Do not start implementation automatically.
