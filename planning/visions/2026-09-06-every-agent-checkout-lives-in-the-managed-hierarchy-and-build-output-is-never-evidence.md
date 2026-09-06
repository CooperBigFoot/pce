# Every agent checkout lives in the managed hierarchy and build output is never evidence

## Outcome

Any checkout a PCE agent creates, for any purpose, lives under the repository's `.worktrees/` hierarchy, is enumerated at the end of the invocation that created it, and is removed when it is clean and verified merged. Build output is never treated as evidence and never copied or retained for its own sake. Evidence is logs, receipts, diffs, and patches.

Today the policy covers "vision worktrees" and cleanup of the worktree an implementation used. Agents read review, audit, red-reproduction, and comparison checkouts as outside that rule. Nothing tells them build directories are not evidence.

## Why now

On 2026-09-06 a 2 TB machine ran out of disk. Rust `target/` directories held about 300 GB. About 165 GB of it came from a single Effort's work on 2026-09-01 and 2026-09-02 in the `orthographos` and `metis` repositories, all created by PCE agents outside the managed hierarchy:

- a sibling worktree for a fix branch, 33 GB, whose branch was already merged and which the final audit never removed;
- two audit worktrees created directly in `/private/tmp`, 41 GB, the location the skill text names as forbidden;
- three git-less copies of the tree, one per reviewed commit, 33 GB, made for independent review;
- one scratch directory holding two extra clones and eight copied `target/` directories kept as "build evidence", 58 GB, beside a few kilobytes of receipt markdown and logs.

Each of these followed the letter of the existing text. The sibling worktree and `/private/tmp` worktrees were not "the vision worktree". The review copies were not worktrees at all. The evidence directory preserved build output because nothing said build output is not evidence.

The remaining disk use was cargo's own lack of garbage collection inside long-lived main checkouts and old-generation PCE worktrees with genuinely unmerged branches. Neither is a PCE skill problem. The first is handled by machine-level and template changes outside this repository (see Companion work). The second is the preservation policy working as designed.

## The change

Three skill surfaces change. Wording is settled at the level of meaning. The implementing agent chooses phrasing that reads naturally beside the surrounding text and keeps the existing test contracts (see Tests).

### `skills/implement-vision/SKILL.md`, "Worktree and preservation policy"

1. **Widen the placement rule from vision worktrees to every checkout.** The rule currently reads "place every PCE-created vision worktree in the repository-owned hierarchy" and forbids `/private/tmp`, temporary directories, and arbitrary sibling directories for "an active vision worktree". It must instead cover every checkout any agent in the invocation creates, for any purpose: implementation, independent review, audit, red or failing reproduction, side-by-side comparison, or anything else. That includes git worktrees, clones, and plain copies of the tree. All of them live below `<repository>/.worktrees/`. The existing `<repository>/.worktrees/visions/effort-<number>-<slug>/` layout for vision worktrees is unchanged. Non-vision checkouts use an equally descriptive path under the same `.worktrees/` root, so the hierarchy remains the single place to look. Reviewers and other delegated agents must receive this rule in their delegated context, the way the naming boundary is already handed to implementation owners.

2. **Add the build-output rule.** Build output, dependency caches, compiled binaries, and other regenerable artifacts are never evidence. An agent preserves logs, receipts, diffs, and patches, and never copies or retains a build directory to prove a result. The preservation rules for uncommitted, unpushed, unmerged, and ambiguously owned work are unchanged; they concern source and history, not build output.

3. **Enumerate at the end.** The final target-branch audit enumerates every checkout the invocation created, in every location, not only the vision worktree. For each one it applies the existing inspection (status, branch reachability, upstream state, target merge evidence, ownership) and removes those that are clean and verified merged. It preserves and reports the rest. The audit stays scoped to checkouts this invocation created; it never removes a checkout another run or a human created, even a merged one. That ownership line is deliberate and is why the 33 GB merged sibling worktree above is a case for the creating run's audit, not for any later run.

### `skills/land-ticket/SKILL.md`, "Target-copy and worktree closure gate"

The gate enumerates "canonical Effort worktrees below `<repository>/.worktrees/visions/effort-<number>-*/`". Widen the enumeration to every checkout under `<repository>/.worktrees/` that relates to the Effort, with the same safe-removal and preservation rules already written there. The build-output rule applies here too: a `target/`, `node_modules/`, virtual environment, or similar directory inside a checkout is never a reason to preserve it.

### `README.md`, worktree paragraph

The paragraph "When isolation is needed, PCE-created checkouts live below `<repository>/.worktrees/visions/`…" describes the same policy for readers. Update it to say every PCE-created checkout, for any purpose, lives below `<repository>/.worktrees/`, and that build output is not evidence. Keep it one paragraph.

## Tests

`tests/test_skill_contracts.py` pins prompt phrases. Two tests touch this policy and must keep passing, extended rather than rewritten:

- `test_vision_worktrees_are_canonical_and_cleanup_is_evidence_safe` asserts the `effort-<number>-<slug>` layout, "standalone", "`/private/tmp`", "arbitrary sibling directories", the four preservation words, "preserve", "remove", and "verified merged". All of those remain true after the change.
- `test_land_ticket_requires_target_vision_and_worktree_closure_gate` asserts the gate section precedes "## Land and evolve the Map" and pins "canonical Effort worktrees", "safely removable", "preserve and report", and "Do not close the Effort". Keep the phrase "canonical Effort worktrees" in the gate, or update the assertion in the same change.

Add assertions that pin the new meaning: that the placement rule names review and audit checkouts (or an equivalent phrase making clear the rule is for every checkout, not only vision worktrees), that plain copies and clones are covered, that build output is never evidence, and that the final audit enumerates every checkout the invocation created. Add the matching assertion for `land-ticket`.

Evidence of success: `python3 -m unittest discover -s tests -v` passes, and a fresh reading of the three edited surfaces leaves no sentence that an agent could read as exempting a review, audit, or reproduction checkout from placement and cleanup.

## Constraints

- PCE stays language-neutral. Do not name Rust, cargo, `target/`, or any other toolchain in the skill text except as a passing example of what build output is. No skill acquires a cleanup command, sweep step, or profile setting.
- The ownership boundary is unchanged: cleanup applies only to checkouts the invocation created. Do not add a rule that sweeps every worktree registered to the repository.
- The preservation rules for uncommitted, unpushed, unmerged, conflicting, or ambiguously owned work are unchanged in meaning.
- Follow `AGENTS.md`: skill text is plain instruction with its reason visible, no emphasis scaffolding, no numeric caps.
- Do not touch `chart-program`, `grill-me`, `grill-ticket`, or `to-vision`. They create no checkouts.

## Companion work outside this vision

Settled during the grill, not part of this repository's change:

- The `rustplate` template (`../rustplate`, a separate repository) gains a dev profile in its root `Cargo.toml` setting debug information to line tables only, with a comment stating why. Test builds inherit it. Incremental compilation stays on.
- The machine gets a user-wide cargo config applying the same profile to every Rust repository immediately, plus `cargo-sweep` on a weekly launchd schedule over the work directory with a seven-day threshold.
- Rejected: disabling incremental compilation (small saving, slows every agent iteration) and a shared target directory across worktrees (concentrates the accumulation and adds lock contention between parallel agents).

The 2026-09-06 disk cleanup itself is already done. Free space went from 361 GiB to 616 GiB. The only evidence in the removed checkouts, two small uncommitted diffs, was saved as patch files in the work directory before removal.
