# WP6 follow-up — the falsifier becomes two commits

Your WP6 work stands. The gate brief, the strict findings format, the short binary-owned `TMPDIR`,
and the seeded-defect run are all verified. The end-to-end run did the thing that matters: a
criterion passed while the artifact was wrong, and the gate caught it anyway.

This closes the three conflicts you raised, all of which were correct.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Continue on `feature/wp6-package-gate`, worktree `/Users/nicolaslazaro/Desktop/work/pce-wp6-gate`,
last commit `6a2e310`.

The stack, none of it merged: `repair/orchestration-dead-ends` → `feature/wp1-work-package-graph`
→ `feature/wp4-herdr-dispatch` → `feature/wp5-package-worker` → `feature/wp6-package-gate`.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool right now:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in `/Users/nicolaslazaro/Desktop/work/pce` replaces the binary those runs
invoke.

1. Work in your existing worktree, never the main checkout. Keep your own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.
4. Additive only: `pce dispatch codex`, `pce dispatch gate`, the anchored role registry, the
   placeholder vocabulary and the route-anchor doctrine in `skills/pce/SKILL.md` stay untouched.

## Reference material, since it will be needed

- herdr — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; checkout is 0.8.0.
  The installed binary is the authority.** herdr does not recognise prime-agent; never consult it
  for completion.
- prime-agent — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, docs at
  `packages/coding-agent/docs/`. Headless is `prime-agent -p`, reads piped stdin, has no
  `--output-schema`.

---

## Why this changes

You raised three conflicts. They share one cause.

**The pre-repair ref is not a good anchor.** Re-running a proposed command at a ref from before the
gate touched anything requires that ref's environment to still be workable — dependencies present,
fixtures in place, toolchain matching. That decays, and when it decays the replay fails for reasons
that have nothing to do with the finding.

Anchor the proof to commits the gate itself authors, minutes apart, and the replay becomes reliable.

## What must be true when you are done

### A gate that finds a defect produces exactly two commits, in order

1. **The witness commit** — introduces the falsifier and nothing else. At this commit the proposed
   criterion command must fail.
2. **The repair commit** — the bounded fix. At this commit the same command must pass.

A finding whose repair commit has no witness commit before it is not a finding. This makes the
repair bound structural rather than exhortative: the proof of the defect defines the scope of the
fix, and a fix without a proof cannot be represented.

Both commits are the gate's own work, made in the same environment, so the command is guaranteed
runnable at both. The gate still does not execute it — that remains WP7's.

### A finding carries one ref pair per repository it touched

Your report correctly identified that a single `pre_repair_ref` / `post_repair_ref` pair cannot
describe a package spanning repositories. Replace the flat pair with a per-repository mapping:
repository identity, witness ref, repair ref. A finding naming a repository the package does not
touch is malformed.

A single-repository finding is the degenerate case with one entry, not a special form.

### The witness/repair discipline is checked as far as it can be here

WP6 must not execute the proposed command — that prohibition stands. But two things are checkable
without executing anything, and both must be:

- Each named ref exists and is reachable in the repository it names.
- The witness commit is an ancestor of the repair commit in every repository the finding names.

An unreachable ref, or a repair commit that does not descend from its witness, is malformed. This
is the syntactic half; the semantic half is WP7's.

### What WP7 inherits, stated so it is not lost

Record these explicitly — in the code's own documentation, not only in a report — so the next
package cannot silently skip them:

- WP7 executes the proposed command at the witness ref, requires **failure**, executes it at the
  repair ref, requires **success**, and **rejects the finding** if either does not hold. A rejected
  finding must not be credited to the gate.
- WP7 converts the exposed command and refs into whatever recorded stimulus and evidence the
  existing `pce gate replay` surface consumes. You correctly noted that verb takes recorded
  evidence rather than a raw command; bridging that is WP7's, but the exposed accessors must carry
  everything the bridge needs.

---

## Acceptance criteria

1. A finding carries, per repository it touches, a repository identity, a witness ref and a repair
   ref. A finding naming an untouched repository is rejected.
2. A finding whose witness ref is not an ancestor of its repair ref, in any repository it names, is
   rejected.
3. A finding naming a ref that does not resolve in its repository is rejected.
4. An empty findings list remains a valid outcome, distinct from a missing outcome file.
5. Nothing in WP6 executes a proposed criterion command — keep the existing test that proves this.
6. The gate brief instructs the two-commit discipline explicitly: the falsifier alone first, the
   bounded repair second.
7. Every existing WP1, WP4, WP5 and WP6 test passes unchanged, including the five readiness waves,
   the herdr sentinel, the RR2 briefs, and the `TMPDIR` headroom assertion.

**Plus one real end-to-end run, executed and reported.** Repeat your seeded-defect scenario — a
criterion satisfied by an artifact that is nonetheless wrong — and show the gate producing two
commits in order, with the witness commit an ancestor of the repair commit, and the finding
carrying a per-repository ref pair. Verify by inspection that the command fails at the witness
commit and passes at the repair commit, and **report that observation as evidence rather than
encoding it as a WP6 check.**

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.

## Do not touch

- Executing criterion commands — WP7. The driver loop — WP7. Gates' dispatch plumbing beyond the
  findings format.
- WP1's graph and readiness, WP4's dispatch and completion, WP5's brief composition — extend, do
  not alter.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — WP9.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`.

## Report back with

- The revised findings format.
- How ancestry and ref reachability are checked without executing anything.
- The updated gate brief text for the two-commit discipline.
- The end-to-end evidence, including the two commits and their ancestry.
- Anything you found that contradicts what this brief assumes.
