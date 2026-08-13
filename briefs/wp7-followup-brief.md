# WP7 follow-up — a criterion must fail for one reason only

Your driver stands. Restart-derived state, the antichain barrier test, coordinated multi-repository
replay, amendments, and branch-local parking are all verified — I re-ran the four tests and read the
materialization code rather than the summary. The barrier test in particular proves concurrency
instead of implying it, which is the version of that test that usually gets faked.

Two gaps remain. One is a correctness problem you could not have seen from the brief; the other is
the claim the brief asked you to prove and you correctly declined to fake.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Continue on `feature/wp7-driver`, worktree `/Users/nicolaslazaro/Desktop/work/pce-wp7-driver`, last
commit `9c1be9d`.

The stack, none of it merged: `repair/orchestration-dead-ends` → `feature/wp1-work-package-graph`
→ `feature/wp4-herdr-dispatch` → `feature/wp5-package-worker` → `feature/wp6-package-gate`
→ `feature/wp7-driver`.

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

- herdr — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; the checkout is 0.8.0.
  The installed binary is the authority.** herdr does not recognise prime-agent; never consult it
  for completion.
- prime-agent — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, docs at
  `packages/coding-agent/docs/`. Headless is `prime-agent -p`, reads piped stdin, no
  `--output-schema`.

---

## Part one: a fresh clone has no environment

Your materialization is `git clone --no-checkout --shared` plus a detached checkout. The isolation is
right, and the byte-identical source worktree afterwards is exactly what was asked for. Keep it.

But a fresh clone contains only committed, tracked files. No `.venv`, no `node_modules`, no
`target/`, no untracked fixtures, nothing gitignored.

RivRetrieve's first criterion is `uv run pytest tests/store/test_value_states.py`. In that clone it
resolves and builds a Python environment from nothing. Offline, or behind a credentialed index, it
does not merely run slowly — it exits non-zero, and your journal records a **failed criterion**.

That is the failure that matters. A criterion exists to distinguish a correct artifact from a
defective one. If it can also go red because a package index was unreachable, it no longer answers
that question, and the log gives a human no way to tell the two apart. The driver's entire authority
comes from the criteria being the judgement; a judgement with a second failure mode is not one.

### What must be true

**A materialized state is prepared before any criterion runs there.** Preparation is a per-repository
shell command, supplied to the driver alongside the repository it belongs to — the same shape as
`--repository NAME=PATH`, because it is the same kind of fact: machine-local, not part of a frozen
plan, and not something a graph author should have to know.

Run it once per materialization, not once per criterion. You already materialize a single clone per
criteria run, so this costs one invocation per package state.

**A preparation failure is a distinct outcome from a criterion failure.** Record it as environment
preparation, name the repository and the command, and do not attribute it to any criterion. A
package whose environment could not be prepared is neither passing nor failing its criteria — it has
not been judged at all, and the log must say so in those terms.

The same applies to the two coordinated replay states: both must be prepared before the proposed
command runs, or the witness/repair verdict inherits the same ambiguity.

### One thing to know before you reach for the existing contract

PCE already records a repository contract carrying `install` and `preflight`, via
`pce contract bootstrap`. It is the obvious home for this and it does not currently work as one:
those fields hold prose, not commands — real recorded values include `"None required for gates."`
and `"none"`. Do not try to execute them.

Take the command-line route above. Consolidating preparation into the tracked contract is a real
future simplification, but it requires making those fields executable, which changes a surface six
live runs depend on. Note it; do not do it here.

## Part two: the worker is judged on what it committed

A worker that builds the right artifact and does not commit it is invisible to a clone-based
materialization, and gets recorded as failing criteria it actually satisfied. That is a trap, and it
is one sentence away from not being one.

Add it to the composed worker brief: work that is not committed will not be judged, because the
criteria run against the commit and not against the working tree. Extend WP5's composition; do not
restructure it.

## Part three: the loop has never driven a real agent

Every package before this one closed with a real spawn. WP4 started a worker through herdr 0.7.1 and
recorded exit 3. WP5 ran prime-agent headless end to end with a brief on stdin. WP6 ran a seeded
defect through a real gate. Each of those runs found something the tests did not.

Your scope note is half right and half not. `HERDR_ENV=1` means *you are inside a pane*, and herdr
blocks nested launches — but WP4 and WP5 both spawned successfully without one, so a pane is not a
precondition. The other half is legitimate: authenticated prime-agent execution spends a real quota,
and declining to spend it unprompted was correct. It is now prompted.

### What must be true

One real drive, executed and reported. Keep it as small as it can be while still being real:

- A single-package graph, one repository, one criterion.
- The work is trivial — create a file with known contents.
- The worker is **actual prime-agent**, spawned through **actual herdr 0.7.1**, receiving the
  composed brief on stdin. Not an injected executable.
- The driver runs the loop: ready → dispatch → worker → prepare → criterion executed → complete →
  terminate.

Report the invocation, the observed sequence, and every recorded artifact, as you did for the
deterministic tests. If it does not work, that finding is worth more than a passing test and should
be reported as the result.

Keep the injected-worker tests exactly as they are. They are the deterministic coverage; this is the
one thing they cannot establish.

---

## Acceptance criteria

1. A per-repository preparation command is accepted by the driver and runs once per materialized
   state, before any criterion or proposed command executes there.
2. A preparation command that exits non-zero produces a recorded environment-preparation failure
   naming the repository and the command, and **no** criterion is recorded as failed.
3. A package whose preparation failed is distinguishable in the journal from one whose criteria
   failed, and from one that was never dispatched.
4. Both coordinated replay states are prepared before the proposed command runs in either.
5. Preparation runs in the materialized clone and leaves the source worktree byte-identical, as
   criteria already do.
6. A package with no preparation command behaves exactly as it does today.
7. The composed worker brief states that uncommitted work will not be judged.
8. Every existing WP1, WP4, WP5, WP6 and WP7 test passes unchanged — the five readiness waves, the
   herdr sentinel, the RR2 briefs, the `TMPDIR` headroom assertion, the five gate tests, and all
   four driver tests including the antichain barrier.

**Plus the real drive from part three, executed and reported.**

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test is known to fail intermittently and pass on rerun; it predates this work.

## Do not touch

- Materialization by detached shared clone, the antichain dispatch, restart derivation, amendments,
  parking, the rejection reasons — all verified. Extend, do not alter.
- Making the tracked repository contract's `install` and `preflight` fields executable.
- WP1's graph and schema. The bounded recovery ladder — WP3. The artifact surface — WP8.
- Retiring the step tier, `milestones.json`, `steps.json`, route anchors — WP9.
- The fourteen open `pce:ticket` issues on `CooperBigFoot/pce`.

## Report back with

- How a preparation command is supplied and where it runs.
- What an environment-preparation failure looks like in the journal, beside a failed criterion.
- The worker brief's new sentence.
- The real drive: the invocation, the observed sequence, the artifacts, and anything that broke.
- Anything you found that contradicts what this brief assumes.
