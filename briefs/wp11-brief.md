# WP11 — a retry must be able to run

The recovery ladder cannot fire. It is built, it is tested, and in production it dies on its first
attempt because the second dispatch of a package collides with the first on both the worktree path
and the branch name. When it dies, it leaves behind a record of a dispatch that never happened, and
the next driver start waits on that record forever.

This is the same shape as the defect this project spent a month on: an escape hatch that exists, is
documented, and cannot be reached. It was found on the first real run against a real repository.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Branch from `main` at commit `d7ab1cd` in a **new worktree**. The harness is merged; there is no
branch stack.

## Boundaries — violating these breaks live work

Six PCE runs are executing against this tool, **and a seventh is a live work-package driver run**
against RivRetrieve in a tmux session:

```
~/.local/bin/pce      -> /Users/nicolaslazaro/Desktop/work/pce/target/release/pce
~/.claude/skills/pce  -> /Users/nicolaslazaro/Desktop/work/pce/skills/pce
```

A `cargo build --release` in `/Users/nicolaslazaro/Desktop/work/pce` replaces the binary those runs
invoke, and a `git checkout` there rewrites the skills they read.

1. Work in your own git worktree, never the main checkout. Keep your own `CARGO_TARGET_DIR`.
2. Never run `install.sh`. Never modify anything under the main checkout path.
3. Do not push, merge, tag, or open PRs.
4. **Do not touch `/Users/nicolaslazaro/Desktop/work/RivRetrieve` or anything under
   `/tmp/pce-work-package-worktrees`.** A live run owns both. Build your own fixtures elsewhere.

## The tools, stated rather than assumed

**herdr** — `/Users/nicolaslazaro/Desktop/thirdparty/herdr`. **Installed 0.7.1; the checkout is
0.8.0. The installed binary is the authority.** herdr does not recognise prime-agent, so a worker
shows as `unknown` and vanishes on exit. Never consult herdr for completion.

**prime-agent** — `/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`, docs at
`packages/coding-agent/docs/`. Installed 0.7.2. Headless is `prime-agent -p`, reads piped stdin, no
`--output-schema`.

---

## What happened, exactly

A real run of the RivRetrieve graph. `RR7` completed cleanly — worker, preparation, criterion, gate.
`RR1`'s worker committed real code and reported `done`, and its criterion then failed honestly: the
worker wrote `tests/store/test_reader.py` while the criterion ran
`uv run pytest tests/store/test_reader_refusal.py`, exit 4, *file or directory not found*. The
judgement worked exactly as designed.

The ladder then attempted its retry rung, appended `recovery-rung-attempted` and
`worker-dispatched` for issuance 3, and asked herdr for a worktree:

```
fatal: '/tmp/pce-work-package-worktrees/pce-2ab9b6472f937b169c0405e2d260/00-rivretrieve'
       already exists
```

The driver exited. On restart it read its own journal, saw issuance 3 dispatched with no result, and
waited on a result file that no process would ever write.

Both halves trace to one line in `crates/core/src/herdr_dispatch.rs`:

```rust
digest.update(vision.as_str().len().to_be_bytes());
digest.update(vision.as_str().as_bytes());
digest.update(package.as_str().len().to_be_bytes());
digest.update(package.as_str().as_bytes());
```

The issuance is not in the digest, so every attempt at a package derives the identical worktree path
and the identical herdr agent name. The branch name collides for the same reason.

No test caught this. The recovery tests inject a fake worker and never reach herdr; the real drives
in WP4, WP7 and WP10 each dispatched every package exactly once.

---

## What must be true when you are done

### A second attempt at a package is dispatchable

Every dispatch of a package gets its own worktree path, its own herdr agent name, and its own branch.
Attempt two must not collide with attempt one on any of them, and a completed attempt's branch must
remain inspectable afterwards rather than being reused or deleted — the first attempt's commit is
evidence about why the retry was needed.

A retry starts from the package's authored base, not from the previous attempt's work. The ladder's
retry rung is defined as the same brief with a fresh agent, and inheriting the failed attempt's tree
would quietly make it something else.

### A spawn that fails is not a dispatch that might still be running

This is the half that turns a recoverable error into a deadlock, and it is a distinction the code
currently collapses.

- A child that was spawned and then died, or was killed, or vanished: **we do not know what it did.**
  The issuance stays open and unaccounted forever, and nothing infers an outcome from silence. That
  rule is correct, it comes from #166, and it must not be weakened.
- A spawn that **provably never produced a child** — herdr refused, the worktree could not be
  created, the executable was not found: the binary knows with certainty that no process exists. It
  must record that, and the issuance must not leave the driver waiting on a result that cannot come.

Whatever you record must be derivable from disk after a restart like every other fact, and must not
be reachable by inference or timeout. Only a fact the spawning code observed at spawn time may
produce it.

A driver started against a journal containing such a record must make progress rather than wait.

### The abandoned issuance already on disk is not your problem

The live run's journal was hand-corrected to unblock it, and a backup sits beside it as
`driver.jsonl.before-truncate`. Do not build a migration. Make the case impossible going forward.

### Worktrees accumulate and nothing removes them

`/tmp/pce-work-package-worktrees` currently holds 86 directories from a single day. Adding the
issuance to the digest makes that strictly worse, since every retry now mints another.

Bound it. A worker worktree whose package has completed has no remaining reader — its branch holds
the work and survives the worktree's removal. Removing one that still holds uncommitted work would
destroy evidence, so removal must be conditional on the work being committed, and a worktree
belonging to a failed or parked package stays for inspection.

If you conclude that automatic removal is unsafe for a reason this brief has not anticipated, say so
and report what bounds the growth instead. Do not leave it unaddressed and unmentioned.

---

## Acceptance criteria

1. Dispatching the same package twice in the same vision produces two distinct worktree paths, two
   distinct herdr agent names, and two distinct branches. **Prove it through the real herdr path,
   not through an injected worker** — that substitution is why this defect shipped.
2. The second attempt's worktree is created from the package's authored base ref, and does not
   contain the first attempt's commit.
3. The first attempt's branch still resolves to its commit after the second attempt has run.
4. A dispatch whose spawn provably fails records that fact, and the issuance does not leave a
   restarted driver waiting.
5. A driver restarted against a journal whose last event is such a failure makes progress: it
   dispatches the package again rather than blocking.
6. A worker that **is** spawned and then SIGKILLed still leaves its issuance open and unaccounted
   forever, with nothing inferring an outcome from silence. Keep the existing test that proves this;
   criterion 4 must not weaken it.
7. A worker worktree whose package completed with its work committed is removed, and its branch
   still resolves. A worktree belonging to a failed or parked package is retained.
8. Every existing test passes unchanged — the readiness waves, the herdr sentinel, the RR2 briefs,
   the `TMPDIR` headroom assertion, the gate tests, the driver tests including the antichain barrier,
   the recovery tests including the one proving that removing the ladder changes dispatch, the
   dispatch-composition tests, the render tests, and the landing-skill doctrine tests.

**Plus one real end-to-end drive, executed and reported.** A two-package graph in a repository you
create, driven by real prime-agent 0.7.2 through real herdr 0.7.1, in which **one package fails its
criterion on the first attempt and succeeds on the retry.** That is the exact path that has never
once run, and it is the whole point of this package. Report the observed sequence, both worktrees,
both branches, and the journal.

Then render it with `pce package render` and report the output path.

Plus `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. One
concurrent pipe-drain test in `tests/dispatch.rs` is known to fail intermittently and pass on an
isolated rerun.

## Do not touch

- The graph and schema, brief and gate-brief composition, criteria execution and materialization,
  coordinated replay, amendments, the recovery ladder's rungs and budgets, the renderer, the wait
  mechanism. Extend rather than alter.
- The rule that a spawned-then-silent child is never resolved by inference or timeout.
- The live RivRetrieve run, its planning directory, and `/tmp/pce-work-package-worktrees`.
- The two open `pce:ticket` issues, #46 and #135. Open questions, not work items.

## Report back with

- What now goes into the worktree, agent-name and branch derivations, and why that set.
- How a provably-failed spawn is recorded, and how it stays distinct from a child that went silent.
- What the retry's worktree is based on, and what happens to the previous attempt's branch.
- How worktree growth is bounded, or why it cannot safely be.
- The real drive: the failing-then-retried package, both attempts, the journal, the rendered page.
- Anything you found that contradicts what this brief assumes.
